#!/usr/bin/env bash
# SessionStart hook: prepares a Claude Code on the web session (commit
# signing for now; the build's tools join it once there is code). Does
# nothing on a developer's own machine.
set -uo pipefail
[ -n "${CLAUDE_CODE_REMOTE:-}" ] || exit 0
cd "${CLAUDE_PROJECT_DIR:-.}" || exit 0

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
exit 0
