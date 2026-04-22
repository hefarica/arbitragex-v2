#!/usr/bin/env bash
# bootstrap-local.sh — one-shot local stack bring-up for ArbitrageX v2.
#
# Brings the dev compose stack up, waits for Postgres to be ready, applies
# all migrations, prints the health of every service, and reports what's
# left for the operator to do (Alchemy key, onboarding, etc).
#
# Designed to be re-runnable. Idempotent. Never writes productive secrets.
#
# Usage:
#   bash automation/scripts/bootstrap-local.sh
#
# Exit codes:
#   0  stack up, migrations applied, health probed
#   1  compose failed to start
#   2  Postgres never became healthy
#   3  migrate.sh failed
#   4  health probe found a service down

set -Eeuo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$REPO_ROOT"

COMPOSE_FILE="docker/compose.dev.yml"

# shellcheck disable=SC1091
[[ -f .env ]] && set -a && . .env && set +a

step() { printf '\n\033[1;36m==> %s\033[0m\n' "$*"; }
ok()   { printf '\033[1;32m  ✓\033[0m %s\n' "$*"; }
warn() { printf '\033[1;33m  !\033[0m %s\n' "$*"; }
fail() { printf '\033[1;31m  ✗\033[0m %s\n' "$*"; }

# ─── 1. compose up ────────────────────────────────────────────────────
step "bringing up docker compose stack (${COMPOSE_FILE})"
if ! docker compose -f "$COMPOSE_FILE" up -d --remove-orphans; then
  fail "docker compose up failed"
  exit 1
fi
ok "compose up"

# ─── 2. wait for postgres ─────────────────────────────────────────────
step "waiting for postgres to become healthy (up to 60s)"
ATTEMPTS=30
for i in $(seq 1 $ATTEMPTS); do
  if docker compose -f "$COMPOSE_FILE" exec -T postgres \
       pg_isready -U postgres -d arbitragex >/dev/null 2>&1; then
    ok "postgres ready (after ${i}s)"
    break
  fi
  if [[ $i -eq $ATTEMPTS ]]; then
    fail "postgres never responded to pg_isready"
    docker compose -f "$COMPOSE_FILE" logs --tail=30 postgres
    exit 2
  fi
  sleep 2
done

# ─── 3. apply migrations ──────────────────────────────────────────────
step "applying database migrations"
# Prefer local psql if present; otherwise migrate.sh falls back to a
# container. Either path is idempotent (schema_migrations table).
if bash automation/scripts/migrate.sh; then
  ok "migrations applied"
else
  fail "migrate.sh failed"
  exit 3
fi

# ─── 4. confirm table set ─────────────────────────────────────────────
step "verifying table set"
EXPECTED_TABLES=(
  opportunities simulations executions strategy_scores relay_scores
  token_safety_cache risk_events incident_log audit_log recon_reports
  relays scoring_weights execution_policy risk_policy price_oracles
  onboarding_progress schema_migrations
)
PRESENT="$(docker compose -f "$COMPOSE_FILE" exec -T postgres \
  psql -U postgres -d arbitragex -tAc \
  "SELECT tablename FROM pg_tables WHERE schemaname='public' ORDER BY tablename;")"
MISSING=()
for t in "${EXPECTED_TABLES[@]}"; do
  if ! grep -qx "$t" <<<"$PRESENT"; then MISSING+=("$t"); fi
done
if [[ ${#MISSING[@]} -eq 0 ]]; then
  ok "all 17 expected tables present"
else
  warn "missing tables: ${MISSING[*]}"
fi

# ─── 5. health probe every service ────────────────────────────────────
step "probing service /health endpoints"
declare -A SERVICES=(
  [api-server]="http://localhost:8080/health"
  [edge]="http://localhost:8787/health"
  [frontend]="http://localhost:5173"
  [selector-api]="http://localhost:3002/health"
  [sim-ctl]="http://localhost:3003/health"
  [recon]="http://localhost:3004/health"
  [relays-client]="http://localhost:3005/health"
  [searcher-rs]="http://localhost:9001/health"
  [prometheus]="http://localhost:9090/-/healthy"
  [grafana]="http://localhost:3000/api/health"
  [alertmanager]="http://localhost:9093/-/healthy"
)
DOWN_COUNT=0
for svc in "${!SERVICES[@]}"; do
  url="${SERVICES[$svc]}"
  if curl -sf -m 3 "$url" >/dev/null 2>&1; then
    ok "$svc — $url"
  else
    warn "$svc down — $url (may take another 30s after compose up)"
    DOWN_COUNT=$((DOWN_COUNT+1))
  fi
done

# ─── 6. what the operator still has to do ─────────────────────────────
step "operator TODO"
RPC_WS_SET=0
if [[ -n "${RPC_WS_1:-}" ]]; then RPC_WS_SET=1; fi

if [[ "$RPC_WS_SET" -eq 0 ]]; then
  warn "RPC_WS_1 / RPC_HTTP_1 not set — searcher-rs is idle. No opportunities will appear until you:"
  cat <<'EOF'
      1. Create a free Alchemy app:   https://dashboard.alchemy.com/
         (New app → Ethereum Mainnet → copy HTTP + WSS URLs.)
      2. Add to .env in repo root:
           RPC_HTTP_1=https://eth-mainnet.g.alchemy.com/v2/<KEY>
           RPC_WS_1=wss://eth-mainnet.g.alchemy.com/v2/<KEY>
      3. Restart searcher-rs only:
           docker compose -f docker/compose.dev.yml restart searcher-rs
      4. Confirm:
           docker compose -f docker/compose.dev.yml logs --tail=20 searcher-rs \
             | grep scanner.subscribed
EOF
else
  ok "RPC_WS_1 is set — searcher-rs should be detecting (check logs)"
fi

ONBOARDING_PHASE1="$(docker compose -f "$COMPOSE_FILE" exec -T postgres \
  psql -U postgres -d arbitragex -tAc \
  "SELECT COALESCE(phase_1_completed_at::text, '') FROM onboarding_progress WHERE org_id='default';" \
  2>/dev/null || echo "")"
if [[ -z "$ONBOARDING_PHASE1" ]]; then
  warn "onboarding phase 1 not recorded yet — visit http://localhost:5173/onboarding/1-init"
else
  ok "onboarding phase 1 recorded at $ONBOARDING_PHASE1"
fi

# ─── 7. summary ───────────────────────────────────────────────────────
step "bootstrap summary"
echo "  operator console  → http://localhost:5173"
echo "  edge (dev-local)  → http://localhost:8787"
echo "  api-server        → http://localhost:8080"
echo "  grafana           → http://localhost:3000"
echo "  prometheus        → http://localhost:9090"
echo
if [[ $DOWN_COUNT -gt 0 ]]; then
  warn "$DOWN_COUNT service(s) not responding yet. Re-run in 30s or check:"
  echo "  docker compose -f $COMPOSE_FILE ps"
  echo "  docker compose -f $COMPOSE_FILE logs --tail=50 <service>"
  exit 4
fi
ok "stack is up, DB is migrated, frontend ready at http://localhost:5173"
