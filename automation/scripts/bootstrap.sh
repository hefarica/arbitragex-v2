#!/usr/bin/env bash
# ArbitrageX v2 — bootstrap.sh
# End-to-end bring-up of the prod-like local stack. Fails fast on any missing prerequisite.
set -Eeuo pipefail

log() { printf '[%s] %s\n' "$(date -u +%H:%M:%S)" "$*"; }
die() { log "FATAL: $*"; exit 1; }

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$REPO_ROOT"

log "repo root: $REPO_ROOT"

# 1. Required binaries
for bin in docker node python3; do
  command -v "$bin" >/dev/null 2>&1 || die "required binary not found: $bin"
done
if ! docker compose version >/dev/null 2>&1; then
  die "docker compose (v2) required"
fi

# 2. .env present
if [[ ! -f .env ]]; then
  log "no .env — copying .env.example; EDIT IT before real runs."
  cp .env.example .env
  log "edit .env and re-run bootstrap"
  exit 2
fi

# 3. Config schema validation
log "validating configs/app.toml"
python3 automation/tools/validate-config.py configs/app.toml configs/schemas/app.schema.json

# 4. Build images
log "building docker images (this can take a while on first run)"
docker compose --env-file .env -f docker/compose.prod.yml build --no-cache

# 5. Start data plane only
log "starting data plane: postgres + redis + observability"
docker compose --env-file .env -f docker/compose.prod.yml up -d postgres redis prometheus grafana alertmanager loki promtail

# 6. Wait for postgres
log "waiting for postgres..."
for i in $(seq 1 30); do
  if docker compose --env-file .env -f docker/compose.prod.yml exec -T postgres pg_isready -U postgres -d arbitragex >/dev/null 2>&1; then
    log "postgres ready"; break
  fi
  sleep 2
  [[ $i -eq 30 ]] && die "postgres did not become ready"
done

# 7. Apply migrations
log "applying DB migrations"
"$REPO_ROOT/automation/scripts/migrate.sh"

# 8. Start application services
log "starting application services"
docker compose --env-file .env -f docker/compose.prod.yml up -d searcher-rs sim-ctl relays-client recon selector-api api-server edge frontend

# 9. Health check
sleep 5
log "running health checks"
"$REPO_ROOT/automation/scripts/health-check.sh"

log "bootstrap OK"
