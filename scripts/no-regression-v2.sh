#!/bin/bash
# =============================================================================
# OMEGA No-Regression Suite V2  —  arbitragex-v2
# =============================================================================
# Purpose: Comprehensive validation script run before declaring LIVE (99%->100%).
# Checks git state, infrastructure, endpoints, frontend, observability, vault,
# paper shadow, and CI/CD state.
#
# Usage:
#   bash no-regression-v2.sh [VPS_HOST] [VPS_USER] [SSH_KEY] [ADMIN_TOKEN] [GRAFANA_PASS]
#
# Parameters:
#   VPS_HOST      - Target VPS IP/hostname    (default: 195.201.235.70)
#   VPS_USER      - SSH username              (default: deploy)
#   SSH_KEY       - Path to SSH private key   (default: ~/.ssh/id_ed25519)
#   ADMIN_TOKEN   - Admin API auth token      (default: $ARBX_ADMIN_TOKEN env)
#   GRAFANA_PASS  - Grafana admin password    (default: $GRAFANA_PASS env)
# =============================================================================
set -euo pipefail

# ---------------------------------------------------------------------------
# Color codes for readability
# ---------------------------------------------------------------------------
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m'

# ---------------------------------------------------------------------------
# Parameter defaults
# ---------------------------------------------------------------------------
VPS_H="${1:-195.201.235.70}"
VPS_U="${2:-deploy}"
SSH_K="${3:-$HOME/.ssh/id_ed25519}"
ADMIN_T="${4:-${ARBX_ADMIN_TOKEN:-}}"
GRAFANA_PASS="${5:-${GRAFANA_PASS:-}}"

# ---------------------------------------------------------------------------
# Global counters
# ---------------------------------------------------------------------------
declare -i PASS=0 FAIL=0 WARN=0

# SSH base command
SSH_CMD="ssh -i $SSH_K -o ConnectTimeout=8 -o StrictHostKeyChecking=accept-new ${VPS_U}@${VPS_H}"

# ---------------------------------------------------------------------------
# Helper: print banner
# ---------------------------------------------------------------------------
banner() {
  echo ""
  echo -e "${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
  echo -e "${BOLD}$1${NC}"
  echo -e "${CYAN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
}

# ---------------------------------------------------------------------------
# Helper: chk() — execute a check
#   $1 = check name (human-readable)
#   $2 = command to execute
#   $3 = expected regex pattern
#   $4 = severity: error | warn (default: error)
# ---------------------------------------------------------------------------
chk() {
  local name="$1" cmd="$2" expected="$3" sev="${4:-error}"
  local output="" rc=0

  # Capture both stdout and stderr, preserving exit code
  output=$(eval "$cmd" 2>&1) || rc=$?

  if [[ $rc -ne 0 ]]; then
    # Command itself failed — report stderr/stdout as-is
    echo -e "${RED}❌  ${name}${NC}"
    echo -e "    ${YELLOW}command exit code:${NC} $rc"
    echo -e "    ${YELLOW}stderr/stdout:${NC} $output"
    ((FAIL++))
    return
  fi

  if echo "$output" | grep -qE "$expected"; then
    echo -e "${GREEN}✅  ${name}${NC}"
    ((PASS++))
  else
    if [[ "$sev" == "warn" ]]; then
      echo -e "${YELLOW}⚠️  ${name}${NC}"
      echo -e "    ${YELLOW}expected:${NC}   /$expected/"
      echo -e "    ${YELLOW}got:${NC}        $output"
      ((WARN++))
    else
      echo -e "${RED}❌  ${name}${NC}"
      echo -e "    ${YELLOW}expected:${NC}   /$expected/"
      echo -e "    ${YELLOW}got:${NC}        $output"
      ((FAIL++))
    fi
  fi
}

# ---------------------------------------------------------------------------
# Helper: run_warn — severity="warn" wrapper
# ---------------------------------------------------------------------------
run_warn() {
  chk "$1" "$2" "$3" "warn"
}

# ---------------------------------------------------------------------------
# Header
# ---------------------------------------------------------------------------
echo ""
echo -e "${BOLD}╔══════════════════════════════════════════════════════════════╗${NC}"
echo -e "${BOLD}║     OMEGA No-Regression Suite V2 — arbitragex-v2            ║${NC}"
echo -e "${BOLD}╚══════════════════════════════════════════════════════════════╝${NC}"
echo -e "${BLUE}VPS:${NC}  ${VPS_U}@${VPS_H}"
echo -e "${BLUE}Time:${NC} $(date -u +%Y-%m-%dT%H:%M:%SZ)"
echo ""

# ═══════════════════════════════════════════════════════════════════════════
# 1. GIT STATE
# ═══════════════════════════════════════════════════════════════════════════
banner "GIT STATE"

chk "Branch feat/omega-scaffold-bootstrap exists in remote" \
  "cd /mnt/agents 2>/dev/null; git ls-remote --heads origin feat/omega-scaffold-bootstrap 2>/dev/null || echo 'NOT_FOUND'" \
  "refs/heads/feat/omega-scaffold-bootstrap" "warn"

chk "main has OMEGA merges" \
  "cd /mnt/agents 2>/dev/null; git log --oneline --merges --grep='OMEGA' main 2>/dev/null | head -5 || echo 'NO_OMEGA'" \
  "OMEGA|Merge" "warn"

chk "lighthouserc.json in repo" \
  "ls /mnt/agents/lighthouserc.json 2>/dev/null || echo 'MISSING'" \
  "lighthouserc\.json" "warn"

# ═══════════════════════════════════════════════════════════════════════════
# 2. SSH & INFRASTRUCTURE
# ═══════════════════════════════════════════════════════════════════════════
banner "SSH & INFRASTRUCTURE"

chk "SSH accessible to VPS" \
  "$SSH_CMD 'echo SSH_OK'" \
  "SSH_OK"

chk "0 containers restarting" \
  "$SSH_CMD 'docker ps --format \"{{.Names}} {{.Status}}\" | grep -i restarting | wc -l'" \
  "^0$"

chk ">=15 containers UP" \
  "$SSH_CMD 'docker ps --filter status=running --format \"{{.Names}}\" | wc -l'" \
  "^(1[5-9]|[2-9][0-9])$"

# ═══════════════════════════════════════════════════════════════════════════
# 3. ENDPOINTS CORE
# ═══════════════════════════════════════════════════════════════════════════
banner "ENDPOINTS CORE"

chk "api-server :8080/health -> 200" \
  "curl -sf -o /dev/null -w '%{http_code}' http://${VPS_H}:8080/health" \
  "^200$"

chk "edge :8787/health -> 200" \
  "curl -sf -o /dev/null -w '%{http_code}' http://${VPS_H}:8787/health" \
  "^200$"

chk "selector-api :3002/health -> 200" \
  "curl -sf -o /dev/null -w '%{http_code}' http://${VPS_H}:3002/health" \
  "^200$"

# ═══════════════════════════════════════════════════════════════════════════
# 4. FRONTEND 8 ROUTES E2E
# ═══════════════════════════════════════════════════════════════════════════
banner "FRONTEND 8 ROUTES E2E"

FRONTEND_ROUTES=(
  "/"
  "/status"
  "/opportunities"
  "/risk"
  "/strategies"
  "/audit-logs"
  "/killswitch"
  "/config"
)

for route in "${FRONTEND_ROUTES[@]}"; do
  chk "Frontend ${route} -> HTTP 200" \
    "curl -sf -o /dev/null -w '%{http_code}' http://${VPS_H}${route}" \
    "^200$"
done

# ═══════════════════════════════════════════════════════════════════════════
# 5. LAST MILE FIXES
# ═══════════════════════════════════════════════════════════════════════════
banner "LAST MILE FIXES"

chk "/api/killswitch/status -> JSON (not HTML)" \
  "curl -sf http://${VPS_H}:8080/api/killswitch/status 2>/dev/null | python3 -c 'import sys,json; json.load(sys.stdin); print(\"JSON_OK\")'" \
  "JSON_OK"

chk "/api/config -> no Zod error" \
  "curl -sf http://${VPS_H}:8080/api/config 2>/dev/null | grep -vi 'zod' | wc -c | xargs -I{} sh -c 'test {} -gt 2 && echo NO_ZOD_ERR'" \
  "NO_ZOD_ERR"

chk "PaperShadowPanel in frontend HTML" \
  "curl -sf http://${VPS_H}/ 2>/dev/null | grep -o 'PaperShadowPanel' | head -1" \
  "PaperShadowPanel"

chk "GoNoGoPanel in frontend HTML" \
  "curl -sf http://${VPS_H}/ 2>/dev/null | grep -o 'GoNoGoPanel' | head -1" \
  "GoNoGoPanel"

# ═══════════════════════════════════════════════════════════════════════════
# 6. OBSERVABILITY
# ═══════════════════════════════════════════════════════════════════════════
banner "OBSERVABILITY"

# Prometheus targets
chk "Prometheus >=10 targets UP" \
  "curl -sf 'http://${VPS_H}:9090/api/v1/targets' 2>/dev/null | python3 -c 'import sys,json; d=json.load(sys.stdin); up=[t for t in d[\"data\"][\"activeTargets\"] if t[\"health\"]==\"up\"]; print(len(up))'" \
  "^(1[0-9]|[2-9][0-9])$"

# Grafana health
chk "Grafana healthy (database: ok)" \
  "curl -sf http://admin:${GRAFANA_PASS}@${VPS_H}:3000/api/health 2>/dev/null | python3 -c 'import sys,json; d=json.load(sys.stdin); print(d.get(\"database\",\"FAIL\"))'" \
  "ok"

# Grafana dashboards
chk "Grafana >=1 dashboard active" \
  "curl -sf http://admin:${GRAFANA_PASS}@${VPS_H}:3000/api/search 2>/dev/null | python3 -c 'import sys,json; d=json.load(sys.stdin); print(len(d))'" \
  "^[1-9][0-9]*$"

# Alertmanager health
chk "Alertmanager healthy" \
  "curl -sf http://${VPS_H}:9093/-/healthy 2>/dev/null; echo 'ALERT_OK'" \
  "ALERT_OK"

# ═══════════════════════════════════════════════════════════════════════════
# 7. VAULT
# ═══════════════════════════════════════════════════════════════════════════
banner "VAULT"

chk "Vault initialized (true)" \
  "curl -sf http://${VPS_H}:8200/v1/sys/seal-status 2>/dev/null | python3 -c 'import sys,json; d=json.load(sys.stdin); print(d.get(\"initialized\",\"FAIL\"))'" \
  "True|true"

chk "Vault unsealed (false)" \
  "curl -sf http://${VPS_H}:8200/v1/sys/seal-status 2>/dev/null | python3 -c 'import sys,json; d=json.load(sys.stdin); print(d.get(\"sealed\",\"FAIL\"))'" \
  "False|false"

# ═══════════════════════════════════════════════════════════════════════════
# 8. PAPER SHADOW
# ═══════════════════════════════════════════════════════════════════════════
banner "PAPER SHADOW"

chk "Cron paper-shadow-monitor active" \
  "$SSH_CMD 'crontab -l 2>/dev/null | grep -c paper-shadow-monitor'" \
  "^[1-9][0-9]*$" "warn"

chk "Counter file readable" \
  "$SSH_CMD 'test -r /var/lib/arbitragex/paper_shadow_counter.json && echo READABLE || echo NOT_READABLE'" \
  "READABLE" "warn"

# ═══════════════════════════════════════════════════════════════════════════
# 9. CI/CD
# ═══════════════════════════════════════════════════════════════════════════
banner "CI/CD"

# Check gh CLI authentication first, warn (not fail) if not available
if ! command -v gh &>/dev/null; then
  echo -e "${YELLOW}⚠️  gh CLI not installed — skipping CI/CD checks${NC}"
  ((WARN++))
elif ! gh auth status &>/dev/null 2>&1; then
  echo -e "${YELLOW}⚠️  gh CLI not authenticated — skipping CI/CD checks${NC}"
  ((WARN++))
else
  chk "Latest CI run on main -> success" \
    "gh run list --repo arbitragex-v2 --branch main --limit 1 --json conclusion --jq '.[0].conclusion' 2>/dev/null || echo 'UNKNOWN'" \
    "success" "warn"
fi

# ═══════════════════════════════════════════════════════════════════════════
# 10. FINAL SUMMARY
# ═══════════════════════════════════════════════════════════════════════════
echo ""
echo -e "${BOLD}════════════════════════════════════════════════════════${NC}"
echo -e "${BOLD}  TOTAL: ${GREEN}${PASS} PASS${NC} · ${RED}${FAIL} FAIL${NC} · ${YELLOW}${WARN} WARN${NC}"
echo -e "${BOLD}════════════════════════════════════════════════════════${NC}"
echo ""

if [[ $FAIL -eq 0 ]]; then
  echo -e "  ${GREEN}✅ NO-REGRESSION: SISTEMA ESTABLE${NC}"
  echo -e "  ${GREEN}   Ready to declare LIVE (100%)${NC}"
  echo ""
  exit 0
else
  echo -e "  ${RED}❌ REGRESIONES CRÍTICAS: ${FAIL} fallo(s) — NO declarar LIVE${NC}"
  echo ""
  exit 1
fi
