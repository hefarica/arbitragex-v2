#!/usr/bin/env bash
# =============================================================================
# ARBITRAGEX-V2 — POST-DEPLOY VERIFICATION & R7 TRAZABILITY (VPS-SIDE)
# =============================================================================
# Purpose: After any deploy (efficient, blue-green, or manual), run this script
#          to verify the full pipeline from searcher detection through to
#          dashboard serving. Implements the R7 E2E trazability pattern plus
#          semiotic-bridge-specific invariant verification.
#
# Usage:
#   cd /opt/arbitragex-v2 && bash scripts/vps/verify-deploy.sh
#   cd /opt/arbitragex-v2 && bash scripts/vps/verify-deploy.sh --strict
#
# Exit codes:
#   0 = All verifications passed (or warnings only in non-strict mode)
#   1 = One or more critical verifications failed
#   2 = Pre-requisite missing (docker, curl, etc.)
#
# Doctrine:
#   * Fail-honest: if a layer has no data, report the exact reason — never
#     fabricate a green status.
#   * Strict mode treats warnings as failures (for CI/operator gates).
# =============================================================================
set -uo pipefail

DEPLOY_PATH="/opt/arbitragex-v2"
COMPOSE_FILE="${COMPOSE_FILE:-docker/compose.prod.yml}"
OBSERVATION_LOG="${DEPLOY_PATH}/logs/verify-observations.jsonl"
STRICT_MODE=false
WARNINGS=0
FAILURES=0

# Endpoints
API_HEALTH="http://localhost:8080/api/health"
EDGE_HEALTH="http://localhost:8787/health"
FRONTEND_URL="http://localhost:5173"
PROMETHEUS_URL="http://localhost:9090"

# ---------------------------------------------------------------------------
# Utilities
# ---------------------------------------------------------------------------
log() { echo "[$(date -u '+%Y-%m-%dT%H:%M:%SZ')] $*"; }
ok()  { printf '  [ \033[32mOK\033[0m ]  %s\n' "$*"; }
warn() {
  printf '  [ \033[33mWARN\033[0m ] %s\n' "$*"
  WARNINGS=$((WARNINGS + 1))
}
fail() {
  printf '  [ \033[31mFAIL\033[0m ] %s\n' "$*"
  FAILURES=$((FAILURES + 1))
}
observation() {
  local category="$1" detail="$2" severity="${3:-info}"
  local ts; ts=$(date -u '+%Y-%m-%dT%H:%M:%SZ')
  mkdir -p "$(dirname "$OBSERVATION_LOG")"
  printf '{"ts":"%s","category":"%s","severity":"%s","detail":"%s"}\n' \
    "$ts" "$category" "$severity" "$detail" >> "$OBSERVATION_LOG"
}

# ---------------------------------------------------------------------------
# Argument parsing
# ---------------------------------------------------------------------------
for arg in "$@"; do
  case "$arg" in
    --strict) STRICT_MODE=true ;;
    --help|-h)
      echo "Usage: $0 [--strict]"
      echo "  --strict  Treat warnings as failures (exit 1 on any warning)"
      exit 0
      ;;
  esac
done

# ---------------------------------------------------------------------------
# 0. Prerequisites
# ---------------------------------------------------------------------------
log "=== ARBITRAGEX-V2 POST-DEPLOY VERIFICATION ==="
log "Mode: $(if $STRICT_MODE; then echo "STRICT"; else echo "NORMAL"; fi)"

for cmd in docker curl; do
  if ! command -v "$cmd" &>/dev/null; then
    log "FATAL: ${cmd} not found in PATH"
    exit 2
  fi
done

cd "$DEPLOY_PATH" || { log "FATAL: cannot cd to ${DEPLOY_PATH}"; exit 2; }

# ---------------------------------------------------------------------------
# 1. Layer 0 — Docker Container State
# ---------------------------------------------------------------------------
log "--- L0: Container State ---"
CONTAINER_COUNT=$(docker ps --format '{{.Names}}' 2>/dev/null | wc -l)
if [ "$CONTAINER_COUNT" -lt 10 ]; then
  fail "Only ${CONTAINER_COUNT} containers running (expected >= 10)"
  observation "L0" "Container count low: ${CONTAINER_COUNT}" "critical"
else
  ok "${CONTAINER_COUNT} containers running"
fi

# Check for restart loops (same logic as hardened-vps-deploy.yml)
for c in arbitragex-v2-api-server-1 arbitragex-v2-frontend-1 arbitragex-v2-edge-1 \
         arbitragex-v2-postgres-1 arbitragex-v2-redis-1 arbitragex-v2-searcher-rs-1; do
  RC=$(docker inspect "$c" --format '{{.RestartCount}}' 2>/dev/null || echo 0)
  if [ "$RC" -gt 3 ]; then
    fail "${c} has ${RC} restarts (restart loop suspected)"
    observation "L0" "${c} restart loop: ${RC} restarts" "critical"
  fi
done
if [ "$FAILURES" -eq 0 ]; then
  ok "No restart loops detected in hot-path containers"
fi

# ---------------------------------------------------------------------------
# 2. Layer 1 — Data Plane (Postgres + Redis)
# ---------------------------------------------------------------------------
log "--- L1: Data Plane ---"

# Redis
REDIS_PONG=$(docker exec arbitragex-v2-redis-1 redis-cli ping 2>/dev/null || echo "FAIL")
if [ "$REDIS_PONG" = "PONG" ]; then
  ok "Redis responds to PING"
else
  fail "Redis did not respond PONG (got: ${REDIS_PONG})"
  observation "L1" "Redis unreachable" "critical"
fi

# Postgres
if docker exec arbitragex-v2-postgres-1 pg_isready -h localhost -p 5432 >/dev/null 2>&1; then
  ok "PostgreSQL is ready"
else
  fail "PostgreSQL is NOT ready"
  observation "L1" "PostgreSQL not ready" "critical"
fi

# R7: Redis stream length (opportunities detected)
OPP_STREAM_LEN=$(docker exec arbitragex-v2-redis-1 redis-cli XLEN arbx:opps:detected 2>/dev/null || echo "?")
if [ "$OPP_STREAM_LEN" = "?" ]; then
  warn "Cannot read Redis stream arbx:opps:detected"
  observation "L1" "Redis stream arbx:opps:detected unreadable" "warning"
elif [ "$OPP_STREAM_LEN" -eq 0 ]; then
  warn "Redis stream arbx:opps:detected is empty (searcher may be idle or discovery failed)"
  observation "L1" "arbx:opps:detected empty" "warning"
else
  ok "Redis stream arbx:opps:detected has ${OPP_STREAM_LEN} entries"
fi

# R7: PostgreSQL latest opportunity timestamp
PG_LATEST=$(docker exec arbitragex-v2-postgres-1 psql -U postgres -d arbitragex -tAc \
  "SELECT MAX(detected_at)::text FROM opportunities;" 2>/dev/null || echo "")
if [ -z "$PG_LATEST" ] || [ "$PG_LATEST" = "NULL" ]; then
  warn "No opportunities in PostgreSQL (PG empty or connection issue)"
  observation "L1" "opportunities table empty or unreachable" "warning"
else
  ok "PostgreSQL latest opportunity: ${PG_LATEST}"
fi

# ---------------------------------------------------------------------------
# 3. Layer 2 — API Server
# ---------------------------------------------------------------------------
log "--- L2: API Server ---"
API_STATUS=$(curl -o /dev/null -s -w '%{http_code}' --max-time 5 "$API_HEALTH" || echo "000")
if [ "$API_STATUS" = "200" ]; then
  ok "API server /api/health returns 200"
else
  fail "API server /api/health returned ${API_STATUS}"
  observation "L2" "API health returned ${API_STATUS}" "critical"
fi

# R7: Opportunities endpoint
OPPS_STATUS=$(curl -o /dev/null -s -w '%{http_code}' --max-time 10 \
  "http://localhost:8080/api/opportunities/live" || echo "000")
if [ "$OPPS_STATUS" = "200" ]; then
  ok "API /api/opportunities/live returns 200"
elif [ "$OPPS_STATUS" = "204" ]; then
  warn "API /api/opportunities/live returns 204 (no opportunities currently)"
  observation "L2" "Opportunities endpoint empty (204)" "warning"
else
  fail "API /api/opportunities/live returned ${OPPS_STATUS}"
  observation "L2" "Opportunities endpoint failed: ${OPPS_STATUS}" "critical"
fi

# ---------------------------------------------------------------------------
# 4. Layer 3 — Edge Worker
# ---------------------------------------------------------------------------
log "--- L3: Edge Worker ---"
EDGE_STATUS=$(curl -o /dev/null -s -w '%{http_code}' --max-time 5 "$EDGE_HEALTH" || echo "000")
if [ "$EDGE_STATUS" = "200" ]; then
  ok "Edge /health returns 200"
else
  fail "Edge /health returned ${EDGE_STATUS}"
  observation "L3" "Edge health failed: ${EDGE_STATUS}" "critical"
fi

# CSP validation: ensure no localhost leakage in production
CSP_HEADER=$(curl -sI --max-time 5 "http://localhost:8787/" 2>/dev/null | grep -i 'content-security-policy' || true)
if [ -n "$CSP_HEADER" ]; then
  if echo "$CSP_HEADER" | grep -qiE 'localhost|127\.0\.0\.1|0\.0\.0\.0'; then
    fail "CSP header contains localhost reference — env propagation violation (RULE 04)"
    observation "L3" "CSP leaks localhost — NEXT_PUBLIC_* baked wrong" "critical"
  else
    ok "CSP header clean (no localhost leakage)"
  fi
else
  warn "No CSP header received from edge"
fi

# ---------------------------------------------------------------------------
# 5. Layer 4 — Frontend
# ---------------------------------------------------------------------------
log "--- L4: Frontend ---"
FE_STATUS=$(curl -o /dev/null -s -w '%{http_code}' --max-time 10 "$FRONTEND_URL" || echo "000")
if [ "$FE_STATUS" = "200" ] || [ "$FE_STATUS" = "307" ] || [ "$FE_STATUS" = "308" ]; then
  ok "Frontend root returns ${FE_STATUS}"
else
  fail "Frontend root returned ${FE_STATUS}"
  observation "L4" "Frontend root failed: ${FE_STATUS}" "critical"
fi

# ---------------------------------------------------------------------------
# 6. Layer 5 — Observability
# ---------------------------------------------------------------------------
log "--- L5: Observability ---"
PROM_STATUS=$(curl -o /dev/null -s -w '%{http_code}' --max-time 5 "${PROMETHEUS_URL}/-/healthy" || echo "000")
if [ "$PROM_STATUS" = "200" ]; then
  ok "Prometheus /-/healthy returns 200"
else
  warn "Prometheus /-/healthy returned ${PROM_STATUS}"
  observation "L5" "Prometheus unhealthy: ${PROM_STATUS}" "warning"
fi

# ---------------------------------------------------------------------------
# 7. Layer 6 — Semiotic-Bridge Invariants (if touched)
# ---------------------------------------------------------------------------
log "--- L6: Semiotic-Bridge Invariants ---"
SEMIOTIC_SRC_CHANGED=$(git diff --name-only HEAD@{1} HEAD 2>/dev/null | grep -c 'semiotic-bridge/' || true)
if [ "${SEMIOTIC_SRC_CHANGED:-0}" -gt 0 ]; then
  log "  semiotic-bridge source changed in this deploy — running full protection pass"
  if bash "${DEPLOY_PATH}/scripts/vps/semiotic-bridge-protect.sh" >/dev/null 2>&1; then
    ok "semiotic-bridge protection pass succeeded (source changed)"
  else
    fail "semiotic-bridge protection pass FAILED after source change"
    observation "L6" "Protection pass failed post-change" "critical"
  fi
else
  # Lightweight check: verify cargo metadata still resolves
  if cd "${DEPLOY_PATH}/backend" && cargo metadata --format-version 1 -p semiotic-bridge >/dev/null 2>&1; then
    ok "semiotic-bridge cargo metadata resolves (no source change)"
  else
    warn "semiotic-bridge cargo metadata failed — dependency drift suspected"
    observation "L6" "cargo metadata drift" "warning"
  fi
fi

# ---------------------------------------------------------------------------
# 8. Layer 7 — Paper-Trade Mode Verification (DOCTRINE)
# ---------------------------------------------------------------------------
log "--- L7: Trade Mode Gate ---"
TRADE_MODE=$(grep '^ARBX_TRADE_MODE=' "$DEPLOY_PATH/.env" 2>/dev/null | cut -d= -f2 || echo "")
if [ "$TRADE_MODE" = "paper" ]; then
  ok "ARBX_TRADE_MODE=paper (shadow mode active)"
elif [ "$TRADE_MODE" = "shadow" ]; then
  ok "ARBX_TRADE_MODE=shadow (read-only mode active)"
else
  fail "ARBX_TRADE_MODE='${TRADE_MODE}' — NOT paper/shadow. LIVE MODE DETECTED."
  observation "L7" "Trade mode is NOT paper/shadow: ${TRADE_MODE}" "critical"
fi

LIVE_EXEC=$(grep '^ARBX_LIVE_EXEC_ENABLED=' "$DEPLOY_PATH/.env" 2>/dev/null | cut -d= -f2 || echo "")
if [ "$LIVE_EXEC" = "true" ]; then
  fail "ARBX_LIVE_EXEC_ENABLED=true — live barrier disarmed in env (unexpected for paper posture)"
  observation "L7" "ARBX_LIVE_EXEC_ENABLED=true" "critical"
else
  ok "ARBX_LIVE_EXEC_ENABLED is not true (default-deny intact)"
fi

# Kill-switch: verify killswitch.json exists and is readable
if [ -f "${DEPLOY_PATH}/killswitch.json" ]; then
  ok "killswitch.json exists"
else
  warn "killswitch.json not found at repo root"
  observation "L7" "killswitch.json missing" "warning"
fi

# ---------------------------------------------------------------------------
# 8b. Layer 7b — Paper-path R7 (detect → validate → sim → paper)
# Fail-honest: report exact stream lengths; never fabricate green.
# detected>0 && validated=0 is the known "selector starve" failure mode.
# ---------------------------------------------------------------------------
log "--- L7b: Paper-path R7 (streams + paper freshness) ---"
xlen() {
  docker exec arbitragex-v2-redis-1 redis-cli XLEN "$1" 2>/dev/null || echo "ERR"
}
DET_N=$(xlen arbx:opps:detected)
VAL_N=$(xlen arbx:opps:validated)
SIM_N=$(xlen arbx:opps:simulated)
EXE_N=$(xlen arbx:opps:executed)
ok "streams detected=${DET_N} validated=${VAL_N} simulated=${SIM_N} executed=${EXE_N}"
observation "L7b" "streams detected=${DET_N} validated=${VAL_N} simulated=${SIM_N} executed=${EXE_N}" "info"

if [ "$DET_N" != "ERR" ] && [ "$DET_N" != "?" ] && [ "${DET_N:-0}" -gt 0 ] 2>/dev/null; then
  if [ "$VAL_N" = "0" ]; then
    warn "arbx:opps:validated=0 while detected=${DET_N} — selector not publishing accepts (paper-path broken upstream of sim)"
    observation "L7b" "validated=0 with detected>0" "warning"
  else
    ok "arbx:opps:validated has ${VAL_N} entries"
  fi
  if [ "$SIM_N" = "0" ] && [ "$VAL_N" = "0" ]; then
    warn "arbx:opps:simulated=0 (sim-ctl has nothing to consume or is idle)"
    observation "L7b" "simulated=0" "warning"
  elif [ "$SIM_N" != "ERR" ] && [ "$SIM_N" != "0" ]; then
    ok "arbx:opps:simulated has ${SIM_N} entries"
  fi
fi

# paper_trade_runs freshness (stale > 48h while detected flowing = warn)
PAPER_LAST=$(docker exec arbitragex-v2-postgres-1 psql -U postgres -d arbitragex -tAc \
  "SELECT COALESCE(MAX(created_at)::text,'null') FROM paper_trade_runs;" 2>/dev/null || echo "ERR")
if [ "$PAPER_LAST" = "ERR" ] || [ "$PAPER_LAST" = "null" ] || [ -z "$PAPER_LAST" ]; then
  warn "paper_trade_runs empty or unreadable (last=${PAPER_LAST})"
  observation "L7b" "paper_trade_runs empty/unreadable" "warning"
else
  ok "paper_trade_runs last row: ${PAPER_LAST}"
  # Age in hours via postgres
  PAPER_AGE_H=$(docker exec arbitragex-v2-postgres-1 psql -U postgres -d arbitragex -tAc \
    "SELECT FLOOR(EXTRACT(EPOCH FROM (NOW() - MAX(created_at)))/3600)::int FROM paper_trade_runs;" 2>/dev/null || echo "ERR")
  if [ "$PAPER_AGE_H" != "ERR" ] && [ "${PAPER_AGE_H:-0}" -gt 48 ] 2>/dev/null; then
    warn "paper_trade_runs stale: last row ${PAPER_AGE_H}h ago (paper sink idle)"
    observation "L7b" "paper_trade_runs stale_hours=${PAPER_AGE_H}" "warning"
  fi
fi

# relays consumer posture (log hint only — no secret)
if docker logs arbitragex-v2-relays-client-1 2>&1 | tail -n 5000 | grep -q 'relays_consumer.skipped'; then
  warn "relays_client booted with relays_consumer.skipped (signer/rpc/db gate) — paper path may not consume simulated"
  observation "L7b" "relays_consumer.skipped observed in logs" "warning"
elif docker logs arbitragex-v2-relays-client-1 2>&1 | tail -n 5000 | grep -q 'relays_consumer.spawned'; then
  ok "relays_consumer.spawned observed in logs"
fi

# ---------------------------------------------------------------------------
# 8c. L0 — nginx canonical drift (NGINX-CONF-DRIFT-01)
# El archivo vivo debe ser byte-exacto al canónico versionado; un enabled
# divergido rompe RULE 02 (/socket.io/ via edge) o mete limit_req mal keyeado
# tras Cloudflare. Fail-honest: divergencia = FAIL, no warning.
# ---------------------------------------------------------------------------
log "--- L0: nginx canonical drift ---"
NGINX_CANON="config/nginx/arbitragex.conf"
NGINX_LIVE="/etc/nginx/sites-enabled/arbitragex"
if [ -f "$NGINX_CANON" ] && [ -e "$NGINX_LIVE" ]; then
  if diff -q "$NGINX_LIVE" "$NGINX_CANON" >/dev/null 2>&1; then
    ok "nginx live == repo canonical (${NGINX_LIVE})"
    observation "L0" "nginx_canonical_match" "info"
  else
    fail "nginx DRIFT: ${NGINX_LIVE} != ${NGINX_CANON} — correr scripts/vps/install-nginx-canonical.sh"
    observation "L0" "nginx_canonical_drift" "critical"
  fi
elif [ -f "$NGINX_CANON" ]; then
  warn "nginx live ausente (${NGINX_LIVE}) — entry point caído o instalado en otra ruta"
  observation "L0" "nginx_live_missing" "warning"
fi

# ---------------------------------------------------------------------------
# 9. Final verdict
# ---------------------------------------------------------------------------
log "=== VERIFICATION COMPLETE ==="
log "Failures: ${FAILURES}  Warnings: ${WARNINGS}"

if [ "$FAILURES" -gt 0 ]; then
  observation "VERDICT" "Post-deploy verification FAILED: ${FAILURES} failure(s), ${WARNINGS} warning(s)" "critical"
  log "VERDICT: FAIL — ${FAILURES} critical failure(s)"
  exit 1
fi

if [ "$WARNINGS" -gt 0 ] && [ "$STRICT_MODE" = true ]; then
  observation "VERDICT" "Post-deploy verification BLOCKED (strict): ${WARNINGS} warning(s)" "warning"
  log "VERDICT: BLOCKED — ${WARNINGS} warning(s) in strict mode"
  exit 1
fi

if [ "$WARNINGS" -gt 0 ]; then
  observation "VERDICT" "Post-deploy verification PARTIAL: ${WARNINGS} warning(s)" "warning"
  log "VERDICT: PARTIAL — ${WARNINGS} warning(s) (non-critical)"
  exit 0
fi

observation "VERDICT" "Post-deploy verification PASSED" "info"
log "VERDICT: PASS — All layers verified"
exit 0
