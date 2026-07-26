#!/usr/bin/env bash
# =============================================================================
# ARBITRAGEX-V2 — SSH CI/CD DRIVER (operator-as-runner)
# =============================================================================
# Local orchestrator that makes SSH deploys as meticulous as GitHub Actions:
#   pin SHA · lock · baseline · dry-run default · typed apply · verify · rollback
#
# Docs: docs/ops/SSH_CICD_PROTOCOL.md
#
# Usage (from repo root, Git Bash / Linux):
#   bash scripts/vps/ssh-cicd-driver.sh --sha <40hex> --change-type <type> --dry-run
#   bash scripts/vps/ssh-cicd-driver.sh --sha <40hex> --confirm-sha <40hex> \
#        --change-type <type> --apply --hot-path-ok [--strict]
#   bash scripts/vps/ssh-cicd-driver.sh --rollback [--sha <40hex> --confirm-sha <40hex>]
#
# Safety:
#   * DEFAULT is dry-run. --apply requires --confirm-sha == --sha.
#   * Never docker compose down. Never touches .env contents (presence only).
#   * Never flips live / ARBX_LIVE_EXEC_ENABLED / mainnet broadcast.
#   * secret/config-sensitive change_type is always blocked.
#   * Lock on VPS; concurrent apply aborts.
# =============================================================================
set -euo pipefail

# ---------------------------------------------------------------------------
# Defaults
# ---------------------------------------------------------------------------
SSH_HOST="${SSH_HOST:-arbx}"
SSH_OPTS="${SSH_OPTS:--o BatchMode=yes -o ConnectTimeout=15 -o StrictHostKeyChecking=accept-new}"
VPS_PATH="${VPS_PATH:-/opt/arbitragex-v2}"
COMPOSE_FILE="${COMPOSE_FILE:-docker/compose.prod.yml}"
LOCK_FILE="${LOCK_FILE:-/tmp/arbitragex_deploy.lock}"
ROLLBACK_SHA_FILE_VPS="${ROLLBACK_SHA_FILE_VPS:-.last-known-good-commit}"

TARGET_SHA=""
CONFIRM_SHA=""
CHANGE_TYPE=""
MODE="dry-run"          # dry-run | apply | rollback
HOT_PATH_OK=false
DB_BACKUP_OK=false
STRICT=false
GATE0=false
VERIFY_NOW=false
SERVICES_OVERRIDE=""

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
TS_UTC="$(date -u '+%Y%m%dT%H%M%SZ')"
ART_ROOT="${ROOT}/repo-vps-audits/SSH-CICD-${TS_UTC}"
mkdir -p "$ART_ROOT"

# ---------------------------------------------------------------------------
# Logging
# ---------------------------------------------------------------------------
log()  { echo "[$(date -u '+%Y-%m-%dT%H:%M:%SZ')] $*"; }
ok()   { printf '  [ \033[32mOK\033[0m ]  %s\n' "$*"; }
warn() { printf '  [ \033[33mWARN\033[0m ] %s\n' "$*"; }
fail() { printf '  [ \033[31mFAIL\033[0m ] %s\n' "$*"; }
die()  { fail "$*"; echo "FATAL: $*" >> "${ART_ROOT}/errors.log"; exit 1; }

ssh_vps() {
  # shellcheck disable=SC2086
  ssh $SSH_OPTS "$SSH_HOST" "$@"
}

ssh_vps_bash() {
  # Run a bash script on the VPS with set -euo pipefail
  # shellcheck disable=SC2086
  ssh $SSH_OPTS "$SSH_HOST" "bash -s" <<REMOTE
set -euo pipefail
cd '${VPS_PATH}'
$*
REMOTE
}

# ---------------------------------------------------------------------------
# Usage
# ---------------------------------------------------------------------------
usage() {
  cat <<'EOF'
Usage:
  ssh-cicd-driver.sh --sha <40hex> --change-type <type> --dry-run
  ssh-cicd-driver.sh --sha <40hex> --confirm-sha <40hex> --change-type <type> --apply [flags]
  ssh-cicd-driver.sh --rollback [--sha <40hex> --confirm-sha <40hex>]

Required for dry-run/apply:
  --sha <40hex>              Target commit (must exist on canonical remote)
  --change-type <type>       docs-only|frontend-only|edge-only|api-server-only|
                             searcher-rs/hot-path|relays-sim-selector|docker/compose|
                             database/migrations|mixed-change
                             (secret/config-sensitive is ALWAYS blocked)

Apply-only:
  --confirm-sha <40hex>      MUST equal --sha (intent confirmation)
  --apply                    Perform mutation (default is dry-run)
  --hot-path-ok              Required for hot-path / api / compose / mixed
  --db-backup-ok             Required for database/migrations
  --strict                   Paper-path R7 warnings become failures
  --services <csv>           Override service list (advanced)

Other:
  --dry-run                  Force dry-run (default)
  --rollback                 Restore last-known-good (or --sha if given)
  --gate0                    Also run capture-baseline.sh --dry-run on VPS
  --verify-now               Run verify-deploy during dry-run (read-only)
  --help

Env overrides:
  SSH_HOST (default: arbx)
  VPS_PATH (default: /opt/arbitragex-v2)
  COMPOSE_FILE (default: docker/compose.prod.yml)
  LOCK_FILE (default: /tmp/arbitragex_deploy.lock)

See: docs/ops/SSH_CICD_PROTOCOL.md
EOF
}

# ---------------------------------------------------------------------------
# Args
# ---------------------------------------------------------------------------
while [ $# -gt 0 ]; do
  case "$1" in
    --sha) TARGET_SHA="${2:-}"; shift 2 ;;
    --confirm-sha) CONFIRM_SHA="${2:-}"; shift 2 ;;
    --change-type) CHANGE_TYPE="${2:-}"; shift 2 ;;
    --dry-run) MODE="dry-run"; shift ;;
    --apply) MODE="apply"; shift ;;
    --rollback) MODE="rollback"; shift ;;
    --hot-path-ok) HOT_PATH_OK=true; shift ;;
    --db-backup-ok) DB_BACKUP_OK=true; shift ;;
    --strict) STRICT=true; shift ;;
    --gate0) GATE0=true; shift ;;
    --verify-now) VERIFY_NOW=true; shift ;;
    --services) SERVICES_OVERRIDE="${2:-}"; shift 2 ;;
    --help|-h) usage; exit 0 ;;
    *) die "Unknown arg: $1" ;;
  esac
done

# ---------------------------------------------------------------------------
# Validate inputs (fail-closed)
# ---------------------------------------------------------------------------
is_sha40() { [[ "${1:-}" =~ ^[0-9a-f]{40}$ ]]; }

validate_change_type() {
  case "${CHANGE_TYPE}" in
    docs-only|frontend-only|edge-only|api-server-only|searcher-rs/hot-path|relays-sim-selector|docker/compose|database/migrations|mixed-change)
      return 0 ;;
    secret|secret/config-sensitive|config-sensitive)
      die "change_type=${CHANGE_TYPE} is ALWAYS BLOCKED by this protocol (use dedicated secrets runbook)" ;;
    "")
      if [ "$MODE" != "rollback" ]; then
        die "--change-type is required for dry-run/apply"
      fi
      ;;
    *)
      die "Invalid --change-type '${CHANGE_TYPE}'" ;;
  esac
}

if [ "$MODE" = "apply" ] || [ "$MODE" = "dry-run" ]; then
  is_sha40 "$TARGET_SHA" || die "--sha must be 40 lowercase hex (got: '${TARGET_SHA}')"
fi

if [ "$MODE" = "apply" ]; then
  is_sha40 "$CONFIRM_SHA" || die "--apply requires --confirm-sha (40 hex)"
  [ "$CONFIRM_SHA" = "$TARGET_SHA" ] || die "confirm_token mismatch: --confirm-sha != --sha (intent not confirmed)"
fi

if [ "$MODE" = "rollback" ] && [ -n "$TARGET_SHA" ]; then
  is_sha40 "$TARGET_SHA" || die "rollback --sha must be 40 hex"
  if [ -n "$CONFIRM_SHA" ] && [ "$CONFIRM_SHA" != "$TARGET_SHA" ]; then
    die "rollback confirm mismatch"
  fi
  if [ -z "$CONFIRM_SHA" ] && [ -n "$TARGET_SHA" ]; then
    die "rollback with explicit --sha also requires --confirm-sha equal to it"
  fi
fi

validate_change_type

# Hot-path gate (mirrors hardened-vps-deploy H0.5)
if [ "$MODE" = "apply" ]; then
  case "$CHANGE_TYPE" in
    searcher-rs/hot-path|api-server-only|docker/compose|mixed-change|relays-sim-selector)
      [ "$HOT_PATH_OK" = true ] || die "change_type=${CHANGE_TYPE} requires --hot-path-ok"
      ;;
  esac
  if [ "$CHANGE_TYPE" = "database/migrations" ]; then
    [ "$DB_BACKUP_OK" = true ] || die "database/migrations requires --db-backup-ok (backup verified)"
  fi
fi

# ---------------------------------------------------------------------------
# Service selection from change_type
# ---------------------------------------------------------------------------
services_for_type() {
  case "$1" in
    docs-only) echo "" ;;
    frontend-only) echo "frontend" ;;
    edge-only) echo "edge" ;;
    api-server-only) echo "api-server" ;;
    searcher-rs/hot-path) echo "searcher-rs" ;;
    relays-sim-selector) echo "relays-client sim-ctl selector-api" ;;
    docker/compose) echo "ALL" ;;
    database/migrations) echo "ALL" ;;
    mixed-change) echo "ALL" ;;
    *) echo "" ;;
  esac
}

if [ -n "$SERVICES_OVERRIDE" ]; then
  SELECTED_SERVICES="${SERVICES_OVERRIDE//,/ }"
else
  SELECTED_SERVICES="$(services_for_type "${CHANGE_TYPE:-docs-only}")"
fi

# ---------------------------------------------------------------------------
# meta.json
# ---------------------------------------------------------------------------
cat > "${ART_ROOT}/meta.json" <<EOF
{
  "ts_utc": "${TS_UTC}",
  "mode": "${MODE}",
  "target_sha": "${TARGET_SHA}",
  "confirm_sha": "${CONFIRM_SHA}",
  "change_type": "${CHANGE_TYPE}",
  "selected_services": "${SELECTED_SERVICES}",
  "ssh_host": "${SSH_HOST}",
  "vps_path": "${VPS_PATH}",
  "strict": ${STRICT},
  "hot_path_ok": ${HOT_PATH_OK},
  "db_backup_ok": ${DB_BACKUP_OK},
  "operator_host": "$(hostname 2>/dev/null || echo unknown)",
  "local_repo": "${ROOT}"
}
EOF
echo "${SELECTED_SERVICES}" > "${ART_ROOT}/services_selected.txt"
log "Artifact dir: ${ART_ROOT}"
log "Mode=${MODE} sha=${TARGET_SHA:-none} change_type=${CHANGE_TYPE:-n/a}"

# ---------------------------------------------------------------------------
# Phase A — SSH connectivity + pre_state (read-only)
# ---------------------------------------------------------------------------
log "=== A: SSH preflight + pre_state (read-only) ==="
if ! ssh_vps "echo SSH_OK && hostname && uptime"; then
  die "SSH to ${SSH_HOST} failed (BatchMode). Fix key/alias before any deploy."
fi
ok "SSH connectivity"

ssh_vps "bash -s" > "${ART_ROOT}/pre_state.txt" 2>&1 <<REMOTE || die "pre_state capture failed"
set -euo pipefail
cd '${VPS_PATH}'
echo '=== META ==='
date -u +%Y-%m-%dT%H:%M:%SZ
hostname
echo '=== GIT HEAD ==='
git rev-parse HEAD
git log -1 --format='%H %ai %s' HEAD
echo '=== GIT STATUS ==='
git status --short || true
echo '=== GIT REMOTES ==='
git remote -v || true
echo '=== DOCKER PS ==='
docker ps --format 'table {{.Names}}\t{{.Status}}\t{{.RunningFor}}' || true
echo '=== DISK ==='
df -h '${VPS_PATH}' || true
echo '=== MEMORY ==='
free -m 2>/dev/null || true
echo '=== LOCK ==='
if [ -f '${LOCK_FILE}' ]; then echo "LOCK_PRESENT: \$(cat '${LOCK_FILE}')"; else echo 'LOCK_FREE'; fi
echo '=== TRADE MODE (name only) ==='
if [ -f .env ]; then
  grep -E '^(ARBX_TRADE_MODE|ARBX_LIVE_EXEC_ENABLED|ARBX_PAPER_MODE)=' .env | sed 's/=.*/=***present***/' || true
else
  echo '.env missing'
fi
echo '=== PAPERMODE REDIS ==='
docker exec arbitragex-v2-redis-1 redis-cli GET arbx:papermode:1 2>/dev/null || true
docker exec arbitragex-v2-redis-1 redis-cli GET arbx:papermode:chain:1 2>/dev/null || true
echo '=== ROLLBACK FILE ==='
if [ -f '${ROLLBACK_SHA_FILE_VPS}' ]; then cat '${ROLLBACK_SHA_FILE_VPS}'; else echo 'none'; fi
REMOTE
ok "pre_state captured → ${ART_ROOT}/pre_state.txt"

VPS_HEAD="$(grep -E '^[0-9a-f]{40}$' "${ART_ROOT}/pre_state.txt" | head -1 || true)"
if [ -z "$VPS_HEAD" ]; then
  # fallback parse from "git rev-parse" section
  VPS_HEAD="$(awk '/=== GIT HEAD ===/{getline; print; exit}' "${ART_ROOT}/pre_state.txt" | tr -d '[:space:]')"
fi
echo "$VPS_HEAD" > "${ART_ROOT}/vps_head_before.txt"
log "VPS HEAD before: ${VPS_HEAD:-unknown}"

if grep -q 'LOCK_PRESENT' "${ART_ROOT}/pre_state.txt"; then
  if [ "$MODE" = "apply" ] || [ "$MODE" = "rollback" ]; then
    die "Deploy lock already held on VPS (${LOCK_FILE}). Another deploy in progress or stale lock."
  else
    warn "Lock present on VPS — dry-run continues but APPLY would abort"
  fi
fi

# ---------------------------------------------------------------------------
# Phase B — Paper posture guard (read-only)
# ---------------------------------------------------------------------------
log "=== B: Paper posture guard ==="
if grep -q 'ARBX_TRADE_MODE=\*\*\*present\*\*\*' "${ART_ROOT}/pre_state.txt" || grep -q 'ARBX_TRADE_MODE' "${ART_ROOT}/pre_state.txt"; then
  # Pull actual trade mode value carefully (not full env)
  TRADE_MODE_VAL="$(ssh_vps "grep -E '^ARBX_TRADE_MODE=' '${VPS_PATH}/.env' 2>/dev/null | cut -d= -f2- | tr -d '\"' | tr -d \"'\" | tail -1" || true)"
  echo "trade_mode=${TRADE_MODE_VAL}" > "${ART_ROOT}/trade_mode.txt"
  case "${TRADE_MODE_VAL}" in
    paper|shadow|"")
      ok "ARBX_TRADE_MODE=${TRADE_MODE_VAL:-unset→treat-as-check-verify}"
      ;;
    live)
      die "ARBX_TRADE_MODE=live on VPS — this protocol refuses to proceed (capital risk)"
      ;;
    *)
      warn "Unexpected ARBX_TRADE_MODE='${TRADE_MODE_VAL}' — verify will enforce paper/shadow"
      ;;
  esac
else
  warn ".env trade mode line not found in pre_state summary"
fi

LIVE_EXEC_VAL="$(ssh_vps "grep -E '^ARBX_LIVE_EXEC_ENABLED=' '${VPS_PATH}/.env' 2>/dev/null | cut -d= -f2- | tr -d '\"' | tr -d \"'\" | tail -1" || true)"
echo "live_exec=${LIVE_EXEC_VAL}" >> "${ART_ROOT}/trade_mode.txt"
if [ "${LIVE_EXEC_VAL}" = "true" ]; then
  die "ARBX_LIVE_EXEC_ENABLED=true — protocol refuses (mainnet/testnet live barrier armed outside this flow)"
fi
ok "Live exec not enabled"

# ---------------------------------------------------------------------------
# Phase C — Ensure target SHA exists on VPS (fetch only; no reset)
# ---------------------------------------------------------------------------
if [ -n "$TARGET_SHA" ]; then
  log "=== C: Fetch + verify target object on VPS ==="
  FETCH_OUT="${ART_ROOT}/fetch_verify.txt"
  set +e
  ssh_vps "bash -s" > "$FETCH_OUT" 2>&1 <<REMOTE
set -euo pipefail
cd '${VPS_PATH}'
# Prefer github remote if present, else origin (operator must ensure canonical)
if git remote get-url github >/dev/null 2>&1; then
  echo "remote=github"
  git fetch github --no-tags --prune 2>&1 | tail -20
elif git remote get-url origin >/dev/null 2>&1; then
  echo "remote=origin"
  git fetch origin --no-tags --prune 2>&1 | tail -20
else
  echo "NO_REMOTE"
  exit 2
fi
if git cat-file -t '${TARGET_SHA}' 2>/dev/null | grep -q commit; then
  echo "TARGET_OK"
  git log -1 --format='%H %ai %s' '${TARGET_SHA}'
else
  echo "TARGET_MISSING"
  exit 3
fi
REMOTE
  FETCH_RC=$?
  set -e
  cat "$FETCH_OUT" | tail -30
  if [ "$FETCH_RC" -ne 0 ] || ! grep -q 'TARGET_OK' "$FETCH_OUT"; then
    die "target_sha ${TARGET_SHA} not present on VPS after fetch. Push to canonical remote first, ensure VPS remote tracks it."
  fi
  ok "target_sha exists on VPS after fetch"

  # Diff names VPS_HEAD..TARGET
  ssh_vps "bash -s" > "${ART_ROOT}/diff_names.txt" 2>&1 <<REMOTE || true
set -euo pipefail
cd '${VPS_PATH}'
if [ -n '${VPS_HEAD}' ] && git cat-file -t '${VPS_HEAD}' >/dev/null 2>&1; then
  echo "diff ${VPS_HEAD}..${TARGET_SHA}"
  git diff --name-only '${VPS_HEAD}' '${TARGET_SHA}' || true
else
  echo "diff_base_unknown"
  git diff --name-only HEAD '${TARGET_SHA}' || true
fi
REMOTE
  ok "diff names → ${ART_ROOT}/diff_names.txt"

  # Block secrets in diff
  if grep -qE '^\.env$|^\.env\.|/\.env$|\.pem$|\.key$|secrets/' "${ART_ROOT}/diff_names.txt" 2>/dev/null; then
    die "Diff includes secret/env paths — blocked. Remove from commit or use secrets runbook."
  fi
fi

# ---------------------------------------------------------------------------
# Paper-path R7 probe (read-only) — always collect evidence
# ---------------------------------------------------------------------------
log "=== D: Paper-path R7 probe (read-only) ==="
ssh_vps "bash -s" > "${ART_ROOT}/paper_path_r7.log" 2>&1 <<'REMOTE' || true
set -u
cd /opt/arbitragex-v2
echo "=== STREAMS ==="
for s in arbx:opps:detected arbx:opps:validated arbx:opps:simulated arbx:opps:executed arbx:opps:simulated:dlq arbx:scoring:scored arbx:hot:simulated arbx:hot:paper_executed arbx:impact_events; do
  n=$(docker exec arbitragex-v2-redis-1 redis-cli XLEN "$s" 2>/dev/null || echo ERR)
  echo "$s=$n"
done
echo "=== WATCHLISTS ==="
for k in arbx:lending_watchlist:aave_v3:1 arbx:aave_v3_watchlist:1 arbx:lending_watchlist:aave_v3:11155111; do
  n=$(docker exec arbitragex-v2-redis-1 redis-cli SCARD "$k" 2>/dev/null || echo ERR)
  echo "$k=$n"
done
echo "=== PAPER_TRADE_RUNS ==="
docker exec arbitragex-v2-postgres-1 psql -U postgres -d arbitragex -tAc \
  "SELECT COUNT(*)::text || ' total; last=' || COALESCE(MAX(created_at)::text,'null') FROM paper_trade_runs;" 2>/dev/null || echo "paper_trade_runs=ERR"
echo "=== OPP LAST HOUR ==="
docker exec arbitragex-v2-postgres-1 psql -U postgres -d arbitragex -tAc \
  "SELECT COALESCE(rejection_reason,'<accepted>') || '=' || COUNT(*)::text FROM opportunities WHERE detected_at > NOW() - INTERVAL '1 hour' GROUP BY 1 ORDER BY COUNT(*) DESC LIMIT 12;" 2>/dev/null || true
echo "=== RELAYS BOOT HINT ==="
docker logs arbitragex-v2-relays-client-1 2>&1 | grep -E 'relays_consumer\.(spawned|skipped)|signer\.(missing|loaded)|live_exec' | tail -15 || true
echo "=== SIM-CTL BOOT HINT ==="
docker logs arbitragex-v2-sim-ctl-1 2>&1 | grep -E 'sim_consumer\.|b2c\.env_missing|service\.boot' | tail -10 || true
echo "=== SELECTOR BOOT HINT ==="
docker logs arbitragex-v2-selector-api-1 2>&1 | grep -E 'consumer\.(started|group)|loop_err' | tail -10 || true
echo "=== PRICE WORKER ==="
docker logs arbitragex-v2-searcher-rs-1 2>&1 | grep -E 'price_worker\.tick_done' | tail -5 || true
REMOTE
ok "paper_path_r7.log written"

# Evaluate paper-path severity for reports
PAPER_FAIL=0
PAPER_WARN=0
VALIDATED_N="$(grep -E '^arbx:opps:validated=' "${ART_ROOT}/paper_path_r7.log" | tail -1 | cut -d= -f2 || echo ERR)"
SIMULATED_N="$(grep -E '^arbx:opps:simulated=' "${ART_ROOT}/paper_path_r7.log" | tail -1 | cut -d= -f2 || echo ERR)"
DETECTED_N="$(grep -E '^arbx:opps:detected=' "${ART_ROOT}/paper_path_r7.log" | tail -1 | cut -d= -f2 || echo ERR)"

if [ "${VALIDATED_N}" = "0" ] && [ "${DETECTED_N}" != "0" ] && [ "${DETECTED_N}" != "ERR" ]; then
  PAPER_WARN=$((PAPER_WARN + 1))
  warn "paper-path: validated=0 while detected=${DETECTED_N} (selector not accepting / not publishing)"
fi
if [ "${SIMULATED_N}" = "0" ] && [ "${VALIDATED_N}" = "0" ]; then
  PAPER_WARN=$((PAPER_WARN + 1))
  warn "paper-path: simulated=0 (sim-ctl starved or down)"
fi
if grep -q 'relays_consumer.skipped' "${ART_ROOT}/paper_path_r7.log"; then
  PAPER_WARN=$((PAPER_WARN + 1))
  warn "paper-path: relays_consumer.skipped (no paper sink without signer — known gap)"
fi
if grep -q 'price_worker.tick_done' "${ART_ROOT}/paper_path_r7.log" && grep -q 'attempted":0\|attempted=0' "${ART_ROOT}/paper_path_r7.log" 2>/dev/null; then
  PAPER_WARN=$((PAPER_WARN + 1))
fi
# stale paper_trade_runs
if grep -q 'paper_trade_runs' "${ART_ROOT}/paper_path_r7.log"; then
  LAST_PAPER_LINE="$(grep -E 'total; last=' "${ART_ROOT}/paper_path_r7.log" | tail -1 || true)"
  echo "$LAST_PAPER_LINE" > "${ART_ROOT}/paper_last.txt"
fi

if [ "$STRICT" = true ] && [ "$PAPER_WARN" -gt 0 ] && [ "$MODE" = "apply" ]; then
  # On apply+strict, paper-path broken is a post-verify concern; pre-apply we only warn
  # unless operator expects green already. Document in report.
  warn "strict mode: ${PAPER_WARN} paper-path warning(s) pre-apply (will fail post-verify if still broken)"
fi

# ---------------------------------------------------------------------------
# DRY-RUN exit path
# ---------------------------------------------------------------------------
if [ "$MODE" = "dry-run" ]; then
  log "=== DRY-RUN REPORT (no mutation) ==="
  if [ "$VERIFY_NOW" = true ]; then
    log "Running verify-deploy.sh on VPS (read-only side effects: observation log only)"
    ssh_vps "bash -s" > "${ART_ROOT}/verify_deploy.log" 2>&1 <<REMOTE || true
set -uo pipefail
cd '${VPS_PATH}'
if [ -f scripts/vps/verify-deploy.sh ]; then
  bash scripts/vps/verify-deploy.sh || true
else
  echo 'verify-deploy.sh missing on VPS tree'
fi
REMOTE
  fi
  if [ "$GATE0" = true ]; then
    ssh_vps "bash -s" > "${ART_ROOT}/gate0.log" 2>&1 <<REMOTE || true
set -uo pipefail
cd '${VPS_PATH}'
if [ -f scripts/vps/capture-baseline.sh ]; then
  bash scripts/vps/capture-baseline.sh --dry-run || true
else
  echo 'capture-baseline.sh missing'
fi
REMOTE
  fi

  cat > "${ART_ROOT}/DRY_RUN_REPORT.md" <<EOF
# SSH CI/CD Dry-Run Report

- **UTC:** ${TS_UTC}
- **Target SHA:** \`${TARGET_SHA}\`
- **VPS HEAD before:** \`${VPS_HEAD:-unknown}\`
- **change_type:** ${CHANGE_TYPE}
- **services that WOULD be touched:** \`${SELECTED_SERVICES:-none}\`
- **paper-path warnings:** ${PAPER_WARN}
- **detected/validated/simulated:** ${DETECTED_N}/${VALIDATED_N}/${SIMULATED_N}

## Would do on --apply

1. Acquire lock \`${LOCK_FILE}\`
2. Save rollback SHA = current VPS HEAD
3. \`git reset --hard ${TARGET_SHA}\` (code only; .env untouched)
4. Selective compose build/up for: ${SELECTED_SERVICES:-ALL/none}
5. verify-deploy.sh + paper-path R7
6. On critical verify fail → auto-rollback

## Explicitly would NOT do

- docker compose down / prune
- touch .env secrets
- ARBX_TRADE_MODE=live / ARBX_LIVE_EXEC_ENABLED
- migrations without --db-backup-ok
- unlock foreign locks

## Next command (if you accept this plan)

\`\`\`bash
bash scripts/vps/ssh-cicd-driver.sh \\
  --sha ${TARGET_SHA} --confirm-sha ${TARGET_SHA} \\
  --change-type ${CHANGE_TYPE} \\
  --apply --hot-path-ok$([ "$STRICT" = true ] && echo ' --strict')
\`\`\`

## Artifacts

All files under \`${ART_ROOT}\`
EOF
  ok "DRY_RUN_REPORT.md written"
  log "DRY-RUN complete. No VPS mutation performed."
  log "Read: ${ART_ROOT}/DRY_RUN_REPORT.md"
  exit 0
fi

# ---------------------------------------------------------------------------
# APPLY / ROLLBACK — mutation path
# ---------------------------------------------------------------------------
acquire_remote_lock() {
  log "=== E: Acquire VPS lock ==="
  local content="ssh-cicd|$$|${TS_UTC}|${TARGET_SHA:-rollback}|${MODE}"
  ssh_vps "bash -s" <<REMOTE || die "Failed to acquire lock"
set -euo pipefail
if [ -f '${LOCK_FILE}' ]; then
  echo "LOCK_BUSY:\$(cat '${LOCK_FILE}')"
  exit 1
fi
echo '${content}' > '${LOCK_FILE}'
chmod 600 '${LOCK_FILE}'
echo LOCK_ACQUIRED
REMOTE
  ok "Lock acquired"
}

release_remote_lock() {
  ssh_vps "rm -f '${LOCK_FILE}'" 2>/dev/null || true
  log "Lock released"
}

trap 'release_remote_lock' EXIT

acquire_remote_lock

if [ "$MODE" = "rollback" ]; then
  log "=== ROLLBACK ==="
  RB_SHA="${TARGET_SHA}"
  if [ -z "$RB_SHA" ]; then
    RB_SHA="$(ssh_vps "cat '${VPS_PATH}/${ROLLBACK_SHA_FILE_VPS}' 2>/dev/null" | tr -d '[:space:]' || true)"
  fi
  is_sha40 "$RB_SHA" || die "No valid rollback SHA (pass --sha or ensure ${ROLLBACK_SHA_FILE_VPS} on VPS)"
  log "Rolling back to ${RB_SHA}"

  ssh_vps "bash -s" > "${ART_ROOT}/rollback_apply.log" 2>&1 <<REMOTE || die "rollback git/compose failed — see rollback_apply.log"
set -euo pipefail
cd '${VPS_PATH}'
git cat-file -t '${RB_SHA}' | grep -q commit
# save what we leave
git rev-parse HEAD > /tmp/arbx_pre_rollback_head.txt || true
git reset --hard '${RB_SHA}'
docker compose --env-file .env -f '${COMPOSE_FILE}' up -d --remove-orphans
echo ROLLBACK_COMPOSE_DONE
REMOTE
  ok "Rollback compose issued"

else
  # APPLY
  log "=== F: Save rollback point + hard reset to target ==="
  ssh_vps "bash -s" > "${ART_ROOT}/apply_git.log" 2>&1 <<REMOTE || die "apply git failed — see apply_git.log"
set -euo pipefail
cd '${VPS_PATH}'
BEFORE=\$(git rev-parse HEAD)
echo "\$BEFORE" > '${ROLLBACK_SHA_FILE_VPS}'
echo "\$BEFORE" > /tmp/arbx_rollback_sha_${TS_UTC}.txt
echo "rollback_saved=\$BEFORE"
# Dirty tree check: allow only if status empty OR operator accepted hot-path
STATUS=\$(git status --short)
if [ -n "\$STATUS" ]; then
  echo "DIRTY_TREE:"
  echo "\$STATUS"
  # Fail closed on dirty tree for non-docs — prevents clobbering manual hotfixes
  echo "DIRTY_TREE_ABORT"
  exit 9
fi
git reset --hard '${TARGET_SHA}'
echo "HEAD_NOW=\$(git rev-parse HEAD)"
test "\$(git rev-parse HEAD)" = '${TARGET_SHA}'
echo APPLY_GIT_OK
REMOTE
  if grep -q 'DIRTY_TREE_ABORT' "${ART_ROOT}/apply_git.log"; then
    die "VPS worktree dirty — refusing reset (would clobber local changes). Inspect pre_state / apply_git.log"
  fi
  ok "VPS HEAD is now ${TARGET_SHA}"
  echo "$VPS_HEAD" > "${ART_ROOT}/rollback_sha.txt"

  log "=== G: Compose build/up (selective) ==="
  case "${SELECTED_SERVICES}" in
    "" )
      log "docs-only / no services — skip compose build"
      echo "NO_COMPOSE" > "${ART_ROOT}/compose.log"
      ;;
    ALL )
      ssh_vps "bash -s" > "${ART_ROOT}/compose.log" 2>&1 <<REMOTE || die "compose ALL failed"
set -euo pipefail
cd '${VPS_PATH}'
# Prefer efficient script if present
if [ -f scripts/vps/deploy-efficient.sh ]; then
  # efficient assumes already at desired SHA; it diffs HEAD@{1} — we just build all
  docker compose --env-file .env -f '${COMPOSE_FILE}' build
  docker compose --env-file .env -f '${COMPOSE_FILE}' up -d --remove-orphans
else
  docker compose --env-file .env -f '${COMPOSE_FILE}' up -d --build --remove-orphans
fi
echo COMPOSE_ALL_OK
REMOTE
      ;;
    * )
      # shellcheck disable=SC2086
      SVC_LIST="${SELECTED_SERVICES}"
      ssh_vps "bash -s" > "${ART_ROOT}/compose.log" 2>&1 <<REMOTE || die "compose selective failed — see compose.log"
set -euo pipefail
cd '${VPS_PATH}'
for svc in ${SVC_LIST}; do
  echo "=== build \$svc ==="
  if [ "\$svc" = "frontend" ]; then
    # RULE 03: if NEXT_PUBLIC drift unknown, prefer no-cache for frontend-only safety
    docker compose --env-file .env -f '${COMPOSE_FILE}' build --no-cache "\$svc"
  else
    docker compose --env-file .env -f '${COMPOSE_FILE}' build "\$svc"
  fi
  docker compose --env-file .env -f '${COMPOSE_FILE}' up --no-deps -d "\$svc"
done
echo COMPOSE_SELECTIVE_OK
REMOTE
      ;;
  esac
  ok "Compose phase finished"
fi

# ---------------------------------------------------------------------------
# H — Post verify
# ---------------------------------------------------------------------------
log "=== H: Post-deploy verify ==="
set +e
ssh_vps "bash -s" > "${ART_ROOT}/verify_deploy.log" 2>&1 <<REMOTE
set -uo pipefail
cd '${VPS_PATH}'
if [ -f scripts/vps/verify-deploy.sh ]; then
  bash scripts/vps/verify-deploy.sh $([ "$STRICT" = true ] && echo --strict)
  exit \$?
else
  echo 'verify-deploy.sh missing'
  exit 2
fi
REMOTE
VERIFY_RC=$?
set -e
log "verify-deploy exit=${VERIFY_RC}"

# Re-probe paper path after apply
ssh_vps "bash -s" > "${ART_ROOT}/paper_path_r7_post.log" 2>&1 <<'REMOTE' || true
set -u
for s in arbx:opps:detected arbx:opps:validated arbx:opps:simulated arbx:opps:executed; do
  n=$(docker exec arbitragex-v2-redis-1 redis-cli XLEN "$s" 2>/dev/null || echo ERR)
  echo "$s=$n"
done
docker logs arbitragex-v2-relays-client-1 2>&1 | grep -E 'relays_consumer\.(spawned|skipped)|service\.boot' | tail -8 || true
REMOTE

ssh_vps "bash -s" > "${ART_ROOT}/post_state.txt" 2>&1 <<REMOTE || true
set -euo pipefail
cd '${VPS_PATH}'
echo "HEAD=\$(git rev-parse HEAD)"
git log -1 --oneline
docker ps --format 'table {{.Names}}\t{{.Status}}' | head -40
REMOTE

# ---------------------------------------------------------------------------
# I — Auto-rollback on verify critical failure (apply only)
# ---------------------------------------------------------------------------
if [ "$MODE" = "apply" ] && [ "$VERIFY_RC" -ne 0 ]; then
  fail "verify-deploy FAILED (rc=${VERIFY_RC}) — auto-rollback"
  RB="$(cat "${ART_ROOT}/rollback_sha.txt" 2>/dev/null || true)"
  if is_sha40 "$RB"; then
    ssh_vps "bash -s" > "${ART_ROOT}/auto_rollback.log" 2>&1 <<REMOTE || fail "auto-rollback itself failed — MANUAL INTERVENTION"
set -euo pipefail
cd '${VPS_PATH}'
git reset --hard '${RB}'
docker compose --env-file .env -f '${COMPOSE_FILE}' up -d --remove-orphans
echo AUTO_ROLLBACK_DONE
REMOTE
    ok "Auto-rollback to ${RB} issued"
  else
    fail "No rollback SHA available — VPS may be mid-state; inspect ${ART_ROOT}"
  fi
  cat > "${ART_ROOT}/APPLY_REPORT.md" <<EOF
# APPLY FAILED + AUTO-ROLLBACK

- target: \`${TARGET_SHA}\`
- verify_rc: ${VERIFY_RC}
- rollback_to: \`${RB:-unknown}\`
- artifact: \`${ART_ROOT}\`

Inspect verify_deploy.log and auto_rollback.log.
EOF
  die "Apply aborted after failed verify (rollback attempted)"
fi

# Strict paper-path post check
if [ "$STRICT" = true ]; then
  POST_VAL="$(grep -E '^arbx:opps:validated=' "${ART_ROOT}/paper_path_r7_post.log" 2>/dev/null | tail -1 | cut -d= -f2 || echo ERR)"
  if [ "$POST_VAL" = "0" ] && [ "${DETECTED_N}" != "0" ]; then
    warn "strict: validated still 0 post-apply (code fix may not address selector rejects yet)"
    # Do not auto-rollback solely for paper-path warn unless operator wants — documented
  fi
fi

cat > "${ART_ROOT}/APPLY_REPORT.md" <<EOF
# SSH CI/CD Apply Report

- **UTC:** ${TS_UTC}
- **Mode:** ${MODE}
- **Target / result SHA:** \`${TARGET_SHA:-rollback}\`
- **Previous VPS HEAD:** \`${VPS_HEAD:-unknown}\`
- **change_type:** ${CHANGE_TYPE:-n/a}
- **services:** \`${SELECTED_SERVICES:-n/a}\`
- **verify_rc:** ${VERIFY_RC}
- **artifact:** \`${ART_ROOT}\`

## Posture

- trade mode file: see trade_mode.txt
- live exec must remain disabled

## Next

- Watch paper_path_r7_post.log for validated/simulated movement
- If bad: \`bash scripts/vps/ssh-cicd-driver.sh --rollback\`
EOF

ok "APPLY/ROLLBACK complete"
log "Report: ${ART_ROOT}/APPLY_REPORT.md"
exit ${VERIFY_RC}
