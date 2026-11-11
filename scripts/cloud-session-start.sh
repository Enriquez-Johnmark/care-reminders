#!/usr/bin/env bash
# SessionStart hook: prepares a Claude Code on the web session (Postgres,
# Mailpit, pinned Rust, sqlx-cli, migrations, commit signing). Does nothing
# on a developer's own machine.
set -uo pipefail
[ -n "${CLAUDE_CODE_REMOTE:-}" ] || exit 0
cd "${CLAUDE_PROJECT_DIR:-.}" || exit 0

log() { echo "cloud-session-start: $*" >&2; }

# Signs commits with the SSH key in GIT_SIGNING_KEY_B64 (registered on GitHub
# as a signing key). The key lives outside the repository, never in it.
if [ -n "${GIT_SIGNING_KEY_B64:-}" ]; then
    command -v ssh-keygen >/dev/null || { apt-get update -qq && apt-get install -y -qq openssh-client; } >/dev/null 2>&1
    key="$HOME/.ssh/cloud_signing"
    mkdir -p "$HOME/.ssh" && chmod 700 "$HOME/.ssh"
    (umask 077; printf '%s' "$GIT_SIGNING_KEY_B64" | base64 -d >"$key") && chmod 600 "$key"
    git config --local user.name Enriquez-Johnmark
    git config --local user.email enriquezjohnmark15@gmail.com
    git config --local gpg.format ssh
    git config --local gpg.ssh.program ssh-keygen
    git config --local user.signingkey "$key"
    git config --local commit.gpgsign true
fi

# Installs the toolchain pinned in rust-toolchain.toml, if rustup is present.
command -v rustup >/dev/null && rustup toolchain install >/dev/null 2>&1

[ -f .env ] || cp .env.example .env

# The cloud VM has Docker installed but doesn't always start the daemon.
if ! docker info >/dev/null 2>&1; then
    nohup dockerd >/tmp/dockerd.log 2>&1 &
    for _ in $(seq 1 30); do docker info >/dev/null 2>&1 && break; sleep 1; done
fi

port_open() { (exec 3<>"/dev/tcp/127.0.0.1/$1") 2>/dev/null; }

# Docker Hub rate-limits anonymous pulls (429); mirror.gcr.io serves the same
# official images, so pull from there and give them their usual names.
pull() {
    docker image inspect "$1" >/dev/null 2>&1 && return
    docker pull -q "$1" >/dev/null 2>&1 && return
    case "$1" in */*) mirror="mirror.gcr.io/$1" ;; *) mirror="mirror.gcr.io/library/$1" ;; esac
    docker pull -q "$mirror" >/dev/null 2>&1 && docker tag "$mirror" "$1"
}

# Another project in this session (Kompleto, Balanse, Lab Report Assistant) may already run
# Postgres on 5432 and Mailpit on 8025: use them, adding this project's role and
# database to that Postgres.
services=()
if port_open 5432; then
    container=$(docker ps --filter publish=5432 --format '{{.Names}}' | head -1)
    if [ -n "$container" ]; then
        admin=$(docker exec "$container" printenv POSTGRES_USER 2>/dev/null || echo postgres)
        docker exec "$container" psql -U "$admin" -d postgres -qc \
            "create role care_reminders login createdb password 'care_reminders'" >/dev/null 2>&1
        docker exec "$container" psql -U "$admin" -d postgres -qc \
            "create database care_reminders owner care_reminders" >/dev/null 2>&1
    fi
else
    services+=(postgres)
fi
port_open 8025 || services+=(mailpit)
for service in "${services[@]}"; do
    case "$service" in
        postgres) pull postgres:17 ;;
        mailpit) pull axllent/mailpit:v1.27.11 ;;
    esac
done
if [ ${#services[@]} -gt 0 ] && ! docker compose up -d --wait "${services[@]}" >/dev/null 2>&1; then
    log "docker compose failed; database tests will not run"
    exit 0
fi

# Slow steps run in the background so the session isn't held up.
(
    SQLX_OFFLINE=true cargo run --quiet -- migrate || log "migrations failed"
    # `cargo sqlx prepare` needs sqlx-cli, matching the sqlx in Cargo.toml.
    command -v sqlx >/dev/null || cargo install sqlx-cli --version '^0.9' \
        --no-default-features --features postgres,rustls --locked \
        || log "installing sqlx-cli failed"
) >/tmp/cloud-session-start.log 2>&1 &
exit 0
