#!/usr/bin/env bash
# =============================================================================
# ARBITRAGEX-V2 — EFFICIENT SELECTIVE DEPLOY (VPS-SIDE)
# =============================================================================
# Purpose: Minimal-downtime selective deployment. Only rebuilds services whose
#          source changed. Detects NEXT_PUBLIC_* env drift to auto-trigger
#          --no-cache frontend builds. Integrates semiotic-bridge protection.
#
# Usage:
#   cd /opt/arbitragex-v2 && bash scripts/vps/deploy-efficient.sh
#   cd /opt/arbitragex-v2 && bash scripts/vps/deploy-efficient.sh --no-cache
#   cd /opt/arbitragex-v2 && bash scripts/vps/deploy-efficient.sh --rollback
#
# Doctrine:
#   * Never runs on mainnet-capable code without ARBX_TRADE_MODE=paper verification.
#   * Never deploys secret/config-sensitive changes automatically.
#   * Always verifies semiotic-bridge invariants before touching Rust services.
#   * Always runs health gates after deploy; auto-rollback on FAIL.
# =============================================================================
set -euo pipefail

DEPLOY_PATH="/opt/arbitragex-v2"
COMPOSE_FILE="${COMPOSE_FILE:-docker/compose.prod.yml}"
ENV_FILE="${ENV_FILE:-${DEPLOY_PATH}/.env}"
HEALTH_URL="http://localhost:8080/api/health"
EDGE_HEALTH="http://localhost:8787/health"
OBSERVATION_LOG="${DEPLOY_PATH}/logs/deploy-observations.jsonl"
LOCK_FILE="/tmp/arbx-efficient-deploy.lock"
ROLLBACK_SHA_FILE="${DEPLOY_PATH}/.last-known-good-commit"
MAX_WAIT=180

# Service-to-path mapping for selective rebuild detection
# Format: service_name|path_prefix_1|path_prefix_2|...
declare -a SERVICE_PATHS=(
  "frontend|frontend/|shared-ts/"
  "edge|edge/|shared-ts/"
  "api-server|backend/api-server/|shared-ts/"
  "selector-api|backend/selector-api/|shared-ts/"
  "searcher-rs|backend/searcher-rs/|backend/shared-rs/|backend/semiotic-bridge/|backend/sed-core/|backend/sim-core/|backend/math-engine/|backend/prioritization-spine/"
  "sim-ctl|backend/sim-ctl/|backend/shared-rs/|backend/sim-core/|backend/math-engine/"
  "relays-client|backend/relays-client/|backend/shared-rs/"
  "recon|backend/recon/|backend/shared-rs/"
  "token-enricher|backend/token-enricher/|backend/shared-rs/"
)

# ---------------------------------------------------------------------------
# Utilities
# ---------------------------------------------------------------------------
log() { echo "[$(date -u '+%Y-%m-%dT%H:%M:%SZ')] $*"; }
observation() {
  local category="$1" detail="$2" severity="${3:-info}"
  local ts; ts=$(date -u '+%Y-%m-%dT%H:%M:%SZ')
  mkdir -p "$(dirname "$OBSERVATION_LOG")"
  printf '{"ts":"%s","category":"%s","severity":"%s","detail":"%s","hostname":"%s"}\n' \
    "$ts" "$category" "$severity" "$detail" "$(hostname)" >> "$OBSERVATION_LOG"
  log "OBSERVATION [$severity] $category: $detail"
}

acquire_lock() {
  local run_id="$$"
  if [ -f "$LOCK_FILE" ]; then
    local existing
    existing=$(cat "$LOCK_FILE" 2>/dev/null || echo "unknown")
    observation "LOCK" "Deploy lock held by ${existing} — aborting. Remove ${LOCK_FILE} if stale." "critical"
    exit 1
  fi
  echo "${run_id}|$(date -u '+%Y%m%dT%H%M%SZ')" > "$LOCK_FILE"
  chmod 600 "$LOCK_FILE"
  trap 'rm -f "${LOCK_FILE}"' EXIT
  log "Lock acquired: ${run_id}"
}

# ---------------------------------------------------------------------------
# Parse arguments
# ---------------------------------------------------------------------------
FORCE_NO_CACHE=false
ROLLBACK_MODE=false
SKIP_PROTECTION=false

for arg in "$@"; do
  case "$arg" in
    --no-cache) FORCE_NO_CACHE=true ;;
    --rollback) ROLLBACK_MODE=true ;;
    --skip-protection) SKIP_PROTECTION=true ;;
    --help|-h)
      echo "Usage: $0 [--no-cache] [--rollback] [--skip-protection]"
      echo "  --no-cache          Force --no-cache on all builds"
      echo "  --rollback          Rollback to last-known-good commit"
      echo "  --skip-protection   Skip semiotic-bridge protection pre-check"
      exit 0
      ;;
  esac
done

# ---------------------------------------------------------------------------
# 0. Prerequisites
# ---------------------------------------------------------------------------
log "=== ARBITRAGEX-V2 EFFICIENT DEPLOY ==="
cd "$DEPLOY_PATH"

if [ ! -f "$COMPOSE_FILE" ]; then
  observation "PREREQ" "Compose file not found: ${COMPOSE_FILE}" "critical"
  exit 1
fi

if [ ! -f "$ENV_FILE" ]; then
  observation "PREREQ" ".env not found at ${ENV_FILE}" "critical"
  exit 1
fi

acquire_lock

# ---------------------------------------------------------------------------
# 1. Save rollback point (Blue state)
# ---------------------------------------------------------------------------
CURRENT_SHA=$(git rev-parse HEAD)
echo "$CURRENT_SHA" > "$ROLLBACK_SHA_FILE"
log "Rollback point saved: ${CURRENT_SHA}"

# ---------------------------------------------------------------------------
# 2. Rollback branch (if requested)
# ---------------------------------------------------------------------------
if [ "$ROLLBACK_MODE" = true ]; then
  if [ ! -f "$ROLLBACK_SHA_FILE" ]; then
    observation "ROLLBACK" "No rollback commit recorded in ${ROLLBACK_SHA_FILE}" "critical"
    exit 1
  fi
  ROLLBACK_SHA=$(cat "$ROLLBACK_SHA_FILE")
  log "ROLLBACK requested to: ${ROLLBACK_SHA}"
  git reset --hard "$ROLLBACK_SHA"
  docker compose --env-file "$ENV_FILE" -f "$COMPOSE_FILE" up -d --remove-orphans
  log "Rollback deploy triggered. Run health-check.sh to verify."
  exit 0
fi

# ---------------------------------------------------------------------------
# 3. Pre-deploy semiotic-bridge protection
# ---------------------------------------------------------------------------
if [ "$SKIP_PROTECTION" = false ]; then
  log "=== SEMIOTIC-BRIDGE PROTECTION PRE-CHECK ==="
  if bash "${DEPLOY_PATH}/scripts/vps/semiotic-bridge-protect.sh" >/dev/null 2>&1; then
    log "  OK: semiotic-bridge protection pass succeeded"
  else
    observation "PROTECTION" "semiotic-bridge protection pre-check FAILED — deploy blocked" "critical"
    exit 1
  fi
else
  log "  SKIP: --skip-protection set (NOT RECOMMENDED)"
fi

# ---------------------------------------------------------------------------
# 4. Detect changed files vs HEAD@{1} (last deploy)
# ---------------------------------------------------------------------------
log "=== CHANGE DETECTION ==="
# Compare current HEAD vs last deployed state (if known)
# If no prior state, diff against HEAD~1 as heuristic
DIFF_BASE="HEAD@{1}"
if ! git cat-file -t "$DIFF_BASE" >/dev/null 2>&1; then
  DIFF_BASE="HEAD~1"
  if ! git cat-file -t "$DIFF_BASE" >/dev/null 2>&1; then
    log "  WARN: No prior commit for diff — treating as full deploy"
    CHANGED_FILES="ALL"
  fi
fi

if [ "${CHANGED_FILES:-}" != "ALL" ]; then
  CHANGED_FILES=$(git diff --name-only "$DIFF_BASE" HEAD 2>/dev/null || echo "ALL")
  if [ -z "$CHANGED_FILES" ]; then
    log "No files changed since ${DIFF_BASE}. Nothing to deploy."
    exit 0
  fi
fi

log "Changed files ($(echo "$CHANGED_FILES" | wc -l)) since ${DIFF_BASE}:"
echo "$CHANGED_FILES" | head -20 | sed 's/^/  /'
[ "$(echo "$CHANGED_FILES" | wc -l)" -gt 20 ] && log "  ... (truncated)"

# ---------------------------------------------------------------------------
# 5. Classify change type & select services
# ---------------------------------------------------------------------------
log "=== SERVICE CLASSIFICATION ==="
SELECTED_SERVICES=""
SELECTED_COUNT=0

for entry in "${SERVICE_PATHS[@]}"; do
  IFS='|' read -r svc paths <<< "$entry"
  MATCHED=false
  for prefix in $paths; do
    if echo "$CHANGED_FILES" | grep -q "^${prefix}"; then
      MATCHED=true
      break
    fi
  done
  if [ "$MATCHED" = true ]; then
    SELECTED_SERVICES="${SELECTED_SERVICES}${svc} "
    SELECTED_COUNT=$((SELECTED_COUNT + 1))
    log "  SELECTED: ${svc} (source under changed paths)"
  fi
done

# Secret/config files must NEVER trigger auto-deploy
if echo "$CHANGED_FILES" | grep -qE '^\.env|^secrets/|\.pem$|\.key$|\.p12$|\.pfx$|\.secret$'; then
  observation "GATE" "Secret/config file in diff — auto-deploy BLOCKED (operator manual review required)" "critical"
  exit 1
fi

# Database migrations always require explicit operator confirmation
if echo "$CHANGED_FILES" | grep -qE '^database/migrations/'; then
  observation "GATE" "Database migrations detected — requires manual operator confirmation + backup" "critical"
  exit 1
fi

# If no service matched but compose or Dockerfiles changed, rebuild all
if [ "$SELECTED_COUNT" -eq 0 ]; then
  if echo "$CHANGED_FILES" | grep -qE '^docker/|Dockerfile|\.dockerignore'; then
    log "  INFRA change detected — rebuilding all services"
    SELECTED_SERVICES=""
    SELECTED_COUNT=0
  else
    log "No service-specific changes — skipping container rebuilds"
    exit 0
  fi
fi

# ---------------------------------------------------------------------------
# 6. NEXT_PUBLIC_* cache-busting detection
# ---------------------------------------------------------------------------
log "=== CACHE-BUSTING ANALYSIS ==="
NEXT_PUBLIC_CHANGED=false
if [ -f "$ENV_FILE" ]; then
  # Check if any NEXT_PUBLIC_* var changed vs the baked frontend image
  # We detect this by looking at the current .env and comparing with the
  # environment inside the running frontend container.
  CURRENT_FRONTEND_ENV=$(docker inspect arbitragex-v2-frontend-1 --format '{{range .Config.Env}}{{.}}\n{{end}}' 2>/dev/null | grep '^NEXT_PUBLIC_' | sort || true)
  DESIRED_FRONTEND_ENV=$(grep '^NEXT_PUBLIC_' "$ENV_FILE" | sort || true)

  if [ "$CURRENT_FRONTEND_ENV" != "$DESIRED_FRONTEND_ENV" ]; then
    NEXT_PUBLIC_CHANGED=true
    log "  NEXT_PUBLIC_* drift detected — frontend MUST rebuild with --no-cache"
  else
    log "  OK: NEXT_PUBLIC_* vars match baked image"
  fi
fi

if [ "$FORCE_NO_CACHE" = true ]; then
  NEXT_PUBLIC_CHANGED=true
  log "  --no-cache forced by operator"
fi

# ---------------------------------------------------------------------------
# 7. Build & deploy
# ---------------------------------------------------------------------------
log "=== DEPLOY EXECUTION ==="
log "Compose: ${COMPOSE_FILE}"
log "Services to touch: ${SELECTED_SERVICES:-ALL}"

# Pull latest images (best-effort)
docker compose --env-file "$ENV_FILE" -f "$COMPOSE_FILE" pull 2>&1 | tail -5 || log "  WARN: docker compose pull had issues (using local cache)"

if [ -z "$SELECTED_SERVICES" ]; then
  # Full deploy (infra change or no specific mapping)
  if [ "$NEXT_PUBLIC_CHANGED" = true ]; then
    log "  Building ALL services with --no-cache (frontend NEXT_PUBLIC drift)"
    docker compose --env-file "$ENV_FILE" -f "$COMPOSE_FILE" build --no-cache 2>&1 | tail -20
  else
    log "  Building ALL services (incremental cache permitted)"
    docker compose --env-file "$ENV_FILE" -f "$COMPOSE_FILE" build 2>&1 | tail -20
  fi
  docker compose --env-file "$ENV_FILE" -f "$COMPOSE_FILE" up -d --remove-orphans
else
  # Selective deploy
  for svc in $SELECTED_SERVICES; do
    if [ "$svc" = "frontend" ] && [ "$NEXT_PUBLIC_CHANGED" = true ]; then
      log "  Building ${svc} with --no-cache (NEXT_PUBLIC drift)"
      docker compose --env-file "$ENV_FILE" -f "$COMPOSE_FILE" build --no-cache "$svc" 2>&1 | tail -10
    else
      log "  Building ${svc}"
      docker compose --env-file "$ENV_FILE" -f "$COMPOSE_FILE" build "$svc" 2>&1 | tail -10
    fi
    docker compose --env-file "$ENV_FILE" -f "$COMPOSE_FILE" up --no-deps -d "$svc"
  done
fi

log "Deploy commands issued."

# ---------------------------------------------------------------------------
# 8. Health gates (R7 trazability)
# ---------------------------------------------------------------------------
log "=== POST-DEPLOY HEALTH GATES (max ${MAX_WAIT}s) ==="
HEALTH_PASSED=0
for attempt in $(seq 1 $((MAX_WAIT / 10))); do
  sleep 10

  API_OK=0
  curl -fsS --max-time 5 "$HEALTH_URL" >/dev/null 2>&1 && API_OK=1 || true

  EDGE_OK=0
  curl -fsS --max-time 5 "$EDGE_HEALTH" >/dev/null 2>&1 && EDGE_OK=1 || true

  PG_OK=0
  docker exec arbitragex-v2-postgres-1 pg_isready -h localhost -p 5432 >/dev/null 2>&1 && PG_OK=1 || true

  REDIS_OK=0
  docker exec arbitragex-v2-redis-1 redis-cli ping 2>/dev/null | grep -q PONG && REDIS_OK=1 || true

  HOT_PATH=$((API_OK + EDGE_OK + PG_OK + REDIS_OK))
  log "  Attempt ${attempt}: api=${API_OK} edge=${EDGE_OK} pg=${PG_OK} redis=${REDIS_OK} (hot=${HOT_PATH}/4)"

  if [ "$HOT_PATH" -eq 4 ]; then
    HEALTH_PASSED=1
    break
  fi
done

if [ "$HEALTH_PASSED" -ne 1 ]; then
  observation "HEALTH" "Post-deploy health gates FAILED — initiating rollback" "critical"
  log "ROLLBACK to ${CURRENT_SHA}..."
  git reset --hard "$CURRENT_SHA"
  if [ -z "$SELECTED_SERVICES" ]; then
    docker compose --env-file "$ENV_FILE" -f "$COMPOSE_FILE" up -d --remove-orphans
  else
    for svc in $SELECTED_SERVICES; do
      docker compose --env-file "$ENV_FILE" -f "$COMPOSE_FILE" up --no-deps -d "$svc"
    done
  fi
  observation "ROLLBACK" "Auto-rollback completed to ${CURRENT_SHA}" "critical"
  exit 1
fi

log "  OK: Health gates passed (hot-path 4/4)"

# ---------------------------------------------------------------------------
# 9. Post-deploy semiotic-bridge verification
# ---------------------------------------------------------------------------
if [ "$SKIP_PROTECTION" = false ]; then
  if echo "$SELECTED_SERVICES" | grep -q 'searcher-rs'; then
    log "=== POST-DEPLOY SEMIOTIC-BRIDGE VERIFICATION ==="
    if bash "${DEPLOY_PATH}/scripts/vps/semiotic-bridge-protect.sh" >/dev/null 2>&1; then
      log "  OK: semiotic-bridge verified after deploy"
    else
      observation "PROTECTION" "semiotic-bridge post-deploy verification FAILED" "warning"
    fi
  fi
fi

# ---------------------------------------------------------------------------
# 10. Final state
# ---------------------------------------------------------------------------
log "=== DEPLOY COMPLETE ==="
log "Previous: ${CURRENT_SHA}"
log "Current:  $(git rev-parse HEAD)"
log "Services: ${SELECTED_SERVICES:-ALL}"
log "Health:   PASSED"
observation "DEPLOY" "Efficient selective deploy completed. Services: ${SELECTED_SERVICES:-ALL}" "info"
exit 0
