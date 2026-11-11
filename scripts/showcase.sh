#!/usr/bin/env bash
# Makes the screenshots and the walkthrough in docs/showcase/ on a fresh
# database: Example Family Practice with its administrator and a care
# coordinator, its made-up patients on HAPI FHIR, and Claude's drafts replayed
# from the recordings (no key, no cost). The server's clock starts at 10 am on
# Tuesday 10 November 2026 in New York (a test build only), so reminders go
# out inside the sending hours whatever the real time is. Needs Postgres and
# Mailpit (docker compose up -d), a built web app and ffmpeg; it starts HAPI
# FHIR itself.
set -euo pipefail
cd "$(dirname "$0")/.."

db=care_reminders_showcase
export DATABASE_URL="postgres://care_reminders:care_reminders@localhost:5432/$db"
export MASTER_KEY="${MASTER_KEY:-0000000000000000000000000000000000000000000000000000000000000001}"
export CARE_REMINDERS_NOW="2026-11-10T15:00:00Z"
export PUBLIC_BASE_URL=http://127.0.0.1:3004
bin=target/debug/care-reminders
SQLX_OFFLINE=true cargo build --quiet --features failpoints
psql_admin() { docker compose exec -T postgres psql -q -U care_reminders -d postgres -c "$1"; }
psql_admin "drop database if exists $db"
psql_admin "create database $db"
$bin migrate >/dev/null
fhir=http://127.0.0.1:8090/fhir
curl -sf "$fhir/metadata" >/dev/null || docker compose --profile fhir up -d hapi
for _ in $(seq 1 90); do curl -sf "$fhir/metadata" >/dev/null && break; sleep 2; done
curl -sf "$fhir/metadata" >/dev/null || { echo "HAPI FHIR didn't start at $fhir" >&2; exit 1; }
$bin load-made-up --url "$fhir" >/dev/null
clinic=$($bin clinic add --name "Example Family Practice" --email admin@family.example.test)
$bin clinic add-coordinator --clinic "$clinic" --email carla@family.example.test --name "Carla Reyes" >/dev/null
$bin clinic set-phone --clinic "$clinic" --phone "(555) 010-0100" >/dev/null
$bin clinic set-fhir-server --clinic "$clinic" --url "$fhir" >/dev/null
# The worker's hourly fetch waits an hour, so the walkthrough's button does the first one.
docker compose exec -T postgres psql -q -U care_reminders -d "$db" \
  -c "update fhir_servers set last_fetched_at = '$CARE_REMINDERS_NOW'" >/dev/null
curl -s -X DELETE http://127.0.0.1:8025/api/v1/messages >/dev/null

BIND_ADDR=127.0.0.1:3004 WEB_DIR=web/dist WORKER_TICK_SECS=2 $bin serve --recorded >target/showcase-server.log 2>&1 &
server=$!
trap 'kill $server' EXIT
for _ in $(seq 1 30); do curl -sf http://127.0.0.1:3004/health >/dev/null && break; sleep 1; done

(cd web && CLINIC="$clinic" pnpm exec playwright test --config showcase/playwright.config.ts --trace off)
