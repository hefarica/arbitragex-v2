#!/bin/bash
# ════════════════════════════════════════════════════════════════════════════
# OMEGA F-11 Paper Shadow – Daily Monitoring System
# ════════════════════════════════════════════════════════════════════════════
# Author:      Omega Trading Systems
# VPS:         <VPS_IP>
# Schedule:    23:55 UTC daily via cron
# Purpose:     Evaluate 4 safety criteria. When 14 consecutive "green days"
#              are reached, declare the system LIVE-ready.
#
# Criteria:
#   1. P&L  > 0  (net profit today)
#   2. Core services all UP (api-server, selector-api, edge-1, frontend)
#   3. Killswitch NOT activated by error
#   4. Zero CRITICAL alerts in Alertmanager
# ════════════════════════════════════════════════════════════════════════════
set -euo pipefail

# ───────────────────────────────────────────────────────────────────────────
# 0.  Colours & helpers
# ───────────────────────────────────────────────────────────────────────────
readonly RED='\033[0;31m'
readonly GREEN='\033[0;32m'
readonly YELLOW='\033[1;33m'
readonly CYAN='\033[0;36m'
readonly NC='\033[0m'  # No Color

# Emoji constants (explicit ASCII fallback if term doesn't support)
readonly E_OK="✅"
readonly E_FAIL="❌"
readonly E_WARN="⚠️"
readonly E_TARGET="🎯"

ok()   { echo -e "${GREEN}${E_OK}${NC} $*"; }
fail() { echo -e "${RED}${E_FAIL}${NC} $*"; }
warn() { echo -e "${YELLOW}${E_WARN}${NC} $*"; }
info() { echo -e "${CYAN}  →${NC} $*"; }

# ───────────────────────────────────────────────────────────────────────────
# 1.  Configuration
# ───────────────────────────────────────────────────────────────────────────
readonly LOG_DIR="/var/log/omega"
readonly COUNTER_FILE="${LOG_DIR}/consecutive-green-days"
readonly HISTORY_CSV="${LOG_DIR}/paper-shadow-history.csv"
readonly LIVE_READY="${LOG_DIR}/LIVE_READY.txt"
readonly API_BASE="http://localhost:8080"
readonly ALERTMANAGER_URL="http://localhost:9093/api/v2/alerts?active=true&silenced=false"

readonly TODAY_UTC="$(date -u '+%Y-%m-%d')"
readonly NOW_TS="$(date -u '+%H:%M:%SZ')"
readonly THRESHOLD=14

# Ensure log directory exists
if [[ ! -d "${LOG_DIR}" ]]; then
    if ! mkdir -p "${LOG_DIR}" 2>/dev/null; then
        fail "Cannot create log directory ${LOG_DIR}"
        exit 1
    fi
fi

# ───────────────────────────────────────────────────────────────────────────
# 2.  Idempotency guard – don't double-count the same calendar day
# ───────────────────────────────────────────────────────────────────────────
if [[ -f "${HISTORY_CSV}" ]]; then
    LAST_DATE="$(tail -n1 "${HISTORY_CSV}" | awk -F',' '{print $1}' 2>/dev/null || true)"
    if [[ "${LAST_DATE}" == "${TODAY_UTC}" ]]; then
        warn "Ya se ejecutó el check para ${TODAY_UTC}. Saltando (idempotencia)."
        CURRENT_COUNT="$(cat "${COUNTER_FILE}" 2>/dev/null || echo 0)"
        info "Contador actual: ${CURRENT_COUNT} / ${THRESHOLD}"
        exit 0
    fi
fi

# ───────────────────────────────────────────────────────────────────────────
# 3.  Load previous counter (default 0)
# ───────────────────────────────────────────────────────────────────────────
CURRENT_COUNT=0
if [[ -f "${COUNTER_FILE}" ]]; then
    CURRENT_COUNT="$(cat "${COUNTER_FILE}" 2>/dev/null || echo 0)"
fi
# Sanitise: must be an integer
if ! [[ "${CURRENT_COUNT}" =~ ^[0-9]+$ ]]; then
    CURRENT_COUNT=0
fi

# ───────────────────────────────────────────────────────────────────────────
# 4.  Header
# ───────────────────────────────────────────────────────────────────────────
echo ""
echo "[${NOW_TS}] === PAPER SHADOW CHECK ${TODAY_UTC} ==="
echo ""

# ───────────────────────────────────────────────────────────────────────────
# 5.  Criterion 1 – P&L > 0
# ───────────────────────────────────────────────────────────────────────────
PNL_VAL="0"
PNL_PASS=1
info "Comprobando P&L …"
if PNL_RAW="$(curl -sf --max-time 10 "${API_BASE}/api/metrics/pnl/today" 2>/dev/null)"; then
    # Attempt to extract a numeric value (JSON field "value", "pnl", plain float)
    PNL_NUM="$(echo "${PNL_RAW}" | grep -oP '(?<="value"\s*:\s*)-?\d+\.?\d*' 2>/dev/null \
               || echo "${PNL_RAW}" | grep -oP '(?<="pnl"\s*:\s*)-?\d+\.?\d*' 2>/dev/null \
               || echo "${PNL_RAW}" | grep -oP '-?\d+\.?\d*' 2>/dev/null \
               || echo '0')"
    # Trim whitespace / newlines
    PNL_NUM="$(echo "${PNL_NUM}" | head -n1 | tr -d '[:space:]')"
    if [[ -n "${PNL_NUM}" ]] && [[ "${PNL_NUM}" =~ ^-?[0-9]+\.?[0-9]*$ ]]; then
        PNL_VAL="${PNL_NUM}"
    else
        PNL_VAL="0"
    fi

    if awk "BEGIN {exit !(${PNL_VAL} > 0)}"; then
        ok "P&L positivo: \$${PNL_VAL} USD"
    else
        fail "P&L no positivo: \$${PNL_VAL} USD"
        PNL_PASS=0
    fi
else
    fail "No se pudo contactar el endpoint de P&L (curl falló)"
    PNL_PASS=0
fi

# ───────────────────────────────────────────────────────────────────────────
# 6.  Criterion 2 – 0 core services DOWN
# ───────────────────────────────────────────────────────────────────────────
CORE_DOWN=0
CORE_NAMES=("api-server" "selector-api" "edge-1" "frontend")
CORE_FAIL_NAMES=""
info "Comprobando servicios core …"

DOCKER_OUTPUT="$(docker ps --format '{{.Names}}\t{{.Status}}' 2>/dev/null || true)"
for svc in "${CORE_NAMES[@]}"; do
    if echo "${DOCKER_OUTPUT}" | grep -q "^${svc}\b"; then
        # Container exists – check if "Up" is in the status line
        if echo "${DOCKER_OUTPUT}" | grep "^${svc}\b" | grep -q "Up"; then
            :  # healthy
        else
            ((CORE_DOWN++)) || true
            CORE_FAIL_NAMES="${CORE_FAIL_NAMES}${svc} "
            fail "Servicio core CAÍDO: ${svc}"
        fi
    else
        ((CORE_DOWN++)) || true
        CORE_FAIL_NAMES="${CORE_FAIL_NAMES}${svc} "
        fail "Servicio core NO ENCONTRADO: ${svc}"
    fi
done

if [[ "${CORE_DOWN}" -eq 0 ]]; then
    ok "Todos los servicios core UP (${#CORE_NAMES[@]}/${#CORE_NAMES[@]})"
else
    fail "Servicios core caídos: ${CORE_DOWN} — ${CORE_FAIL_NAMES}"
fi

# ───────────────────────────────────────────────────────────────────────────
# 7.  Criterion 3 – Killswitch NOT activated by error
# ───────────────────────────────────────────────────────────────────────────
KS_ERR=0
info "Comprobando killswitch …"
if [[ -z "${ARBX_ADMIN_TOKEN:-}" ]]; then
    warn "ARBX_ADMIN_TOKEN no definido — asumiendo killswitch ACTIVO por seguridad"
    KS_ERR=1
else
    if KS_RAW="$(curl -sf --max-time 10 "${API_BASE}/admin/killswitch/status" \
                  -H "Authorization: Bearer ${ARBX_ADMIN_TOKEN}" 2>/dev/null)"; then
        # Check if killswitch is active / triggered by error
        if echo "${KS_RAW}" | grep -qiE '("active"\s*:\s*true|"triggered"\s*:\s*true|"activated"|"error")'; then
            fail "Killswitch ACTIVADO por error"
            KS_ERR=1
        else
            ok "Killswitch inactivo"
        fi
    else
        fail "No se pudo contactar killswitch (curl falló) — asumiendo ACTIVO"
        KS_ERR=1
    fi
fi

# ───────────────────────────────────────────────────────────────────────────
# 8.  Criterion 4 – 0 CRITICAL alerts active
# ───────────────────────────────────────────────────────────────────────────
CRIT_COUNT=0
info "Consultando Alertmanager …"
if ALERTS_RAW="$(curl -sf --max-time 10 "${ALERTMANAGER_URL}" 2>/dev/null)"; then
    # Count alerts with severity == critical (JSON parsing via grep for portability)
    CRIT_COUNT="$(echo "${ALERTS_RAW}" | grep -oP '(?<="severity"\s*:\s*")[^"]*' 2>/dev/null | grep -ci '^critical$' || echo 0)"
    CRIT_COUNT="$(echo "${CRIT_COUNT}" | tr -d '[:space:]')"
    if ! [[ "${CRIT_COUNT}" =~ ^[0-9]+$ ]]; then
        CRIT_COUNT=0
    fi
    if [[ "${CRIT_COUNT}" -gt 0 ]]; then
        fail "Alertas CRÍTICAS activas: ${CRIT_COUNT}"
    else
        ok "0 alertas críticas activas"
    fi
else
    fail "No se pudo contactar Alertmanager (curl falló) — asumiendo 0 alertas"
    CRIT_COUNT=0
fi

# ───────────────────────────────────────────────────────────────────────────
# 9.  Aggregate result
# ───────────────────────────────────────────────────────────────────────────
ALL_PASS=0
if [[ "${PNL_PASS}" -eq 1 && "${CORE_DOWN}" -eq 0 && "${KS_ERR}" -eq 0 && "${CRIT_COUNT}" -eq 0 ]]; then
    ALL_PASS=1
fi

# ───────────────────────────────────────────────────────────────────────────
# 10. Update counter & build result string
# ───────────────────────────────────────────────────────────────────────────
if [[ "${ALL_PASS}" -eq 1 ]]; then
    # Green day
    ((CURRENT_COUNT++)) || true
    RESULT="GREEN"
    ok "DÍA VERDE #${CURRENT_COUNT} / ${THRESHOLD} — P&L: \$${PNL_VAL} USD"
else
    # Red day – reset counter
    OLD_COUNT="${CURRENT_COUNT}"
    CURRENT_COUNT=0
    RESULT="RED"
    # Build failure reason string
    REASONS=""
    [[ "${PNL_PASS}"   -eq 0 ]] && REASONS="${REASONS}PNL_NEGATIVE "
    [[ "${CORE_DOWN}"  -gt 0 ]] && REASONS="${REASONS}SERVICES_DOWN:${CORE_DOWN} "
    [[ "${KS_ERR}"     -gt 0 ]] && REASONS="${REASONS}KS_ACTIVATED "
    [[ "${CRIT_COUNT}" -gt 0 ]] && REASONS="${REASONS}CRIT_ALERTS:${CRIT_COUNT} "
    fail "DÍA NO VERDE — ${REASONS}— CONTADOR REINICIADO A 0 (era ${OLD_COUNT})"
fi

# Persist counter
echo "${CURRENT_COUNT}" > "${COUNTER_FILE}"

# ───────────────────────────────────────────────────────────────────────────
# 11. Append to history CSV (header if file doesn't exist)
# ───────────────────────────────────────────────────────────────────────────
if [[ ! -f "${HISTORY_CSV}" ]]; then
    echo "DATE,RESULT,PNL,CORE_DOWN,KS_ERR,CRIT,CURRENT" > "${HISTORY_CSV}"
fi

# Escape potential commas in PNL_VAL for CSV safety
PNL_CSV="${PNL_VAL//,/.}"
echo "${TODAY_UTC},${RESULT},${PNL_CSV},${CORE_DOWN},${KS_ERR},${CRIT_COUNT},${CURRENT_COUNT}" >> "${HISTORY_CSV}"

# ───────────────────────────────────────────────────────────────────────────
# 12. LIVE_READY signal (counter reached threshold)
# ───────────────────────────────────────────────────────────────────────────
if [[ "${CURRENT_COUNT}" -ge "${THRESHOLD}" ]]; then
    if [[ ! -f "${LIVE_READY}" ]]; then
        touch "${LIVE_READY}"
    fi
    echo "${E_TARGET} PAPER SHADOW 14/14 — SISTEMA LISTO PARA LIVE — ${TODAY_UTC} ${NOW_TS}" >> "${LIVE_READY}"
    echo ""
    echo -e "${GREEN}${E_TARGET} PAPER SHADOW 14/14 — SISTEMA LISTO PARA LIVE${NC}"
fi

# ───────────────────────────────────────────────────────────────────────────
# 13. Summary
# ───────────────────────────────────────────────────────────────────────────
echo ""
echo "Contador: ${CURRENT_COUNT} / ${THRESHOLD}"

# Calculate ETA LIVE (days remaining, then add to TODAY)
DAYS_REMAINING=$(( THRESHOLD - CURRENT_COUNT ))
if [[ "${DAYS_REMAINING}" -le 0 ]]; then
    echo "ETA LIVE: ${TODAY_UTC} ← LISTO AHORA"
else
    ETA_DATE="$(date -u -d "${TODAY_UTC} +${DAYS_REMAINING} days" '+%Y-%m-%d' 2>/dev/null \
                || date -u -v+"${DAYS_REMAINING}"d -f '%Y-%m-%d' "${TODAY_UTC}" '+%Y-%m-%d' 2>/dev/null \
                || echo 'N/A (calcular manualmente)')"
    echo "ETA LIVE: ${ETA_DATE}"
fi
echo ""
echo "[${NOW_TS}] === PAPER SHADOW FIN ${TODAY_UTC} ==="
echo ""
