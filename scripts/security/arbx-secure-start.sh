#!/usr/bin/env bash
# =============================================================================
# ArbitrageX v2 — Secure Start (arbx-secure-start.sh)
# =============================================================================
# Arranque seguro enterprise-grade con AES-256-GCM.
# Idempotente: puede ejecutarse N veces sin efectos secundarios.
# Persistente: diseñado para ser invocado por systemd en cada reinicio del VPS.
#
# FLUJO:
#   1. Pre-flight checks (dependencias, archivos, permisos)
#   2. Derivar MEK desde el Excel (o desde ARBX_EXCEL_PATH env var)
#   3. Desencriptar .env.enc → /dev/shm (tmpfs, RAM-only, chmod 600)
#   4. Levantar Docker Compose (idempotente: --no-recreate si ya corren)
#   5. Verificar health de los servicios críticos
#   6. Shredear el archivo temporal (siempre, via trap)
#   7. Reportar estado final
#
# USO:
#   bash arbx-secure-start.sh [--excel /ruta/al/Excel.xlsx] [--check-only] [--force-recreate]
#
# VARIABLES DE ENTORNO (alternativa a --excel):
#   ARBX_EXCEL_PATH   Ruta al archivo Excel de secrets
#   ARBX_COMPOSE_CMD  Comando de docker compose (default: docker compose)
# =============================================================================

set -euo pipefail

# ─── Colores y símbolos ───────────────────────────────────────────────────────
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
BOLD='\033[1m'
RESET='\033[0m'

OK="${GREEN}✔${RESET}"
FAIL="${RED}✘${RESET}"
WARN="${YELLOW}⚠${RESET}"
INFO="${CYAN}ℹ${RESET}"
ARROW="${BLUE}→${RESET}"

# ─── Constantes ───────────────────────────────────────────────────────────────
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" &>/dev/null && pwd)"
PROJECT_ROOT="$(dirname "$(dirname "$SCRIPT_DIR")")"
ENV_ENC_PATH="${PROJECT_ROOT}/.env.enc"
ENV_TMP_PATH="/dev/shm/.arbx.env.$(id -u).tmp"
COMPOSE_FILE="${PROJECT_ROOT}/docker/compose.prod.yml"
VAULT_CRYPTO="${SCRIPT_DIR}/vault_crypto.py"
LOG_FILE="${PROJECT_ROOT}/logs/secure-start.log"
LOCK_FILE="/tmp/arbx-secure-start.lock"
VERSION="2.0.0"

# Servicios críticos que deben estar healthy al finalizar
CRITICAL_SERVICES=("arbitragex-v2-edge-1" "arbitragex-v2-api-server-1" "arbitragex-v2-frontend-1")

# ─── Argumentos ───────────────────────────────────────────────────────────────
EXCEL_PATH="${ARBX_EXCEL_PATH:-}"
CHECK_ONLY=false
FORCE_RECREATE=false
SILENT=false

while [[ $# -gt 0 ]]; do
  case "$1" in
    --excel)       EXCEL_PATH="$2"; shift 2 ;;
    --check-only)  CHECK_ONLY=true; shift ;;
    --force-recreate) FORCE_RECREATE=true; shift ;;
    --silent)      SILENT=true; shift ;;
    --help|-h)
      echo "Usage: $0 [--excel /path/to/Excel.xlsx] [--check-only] [--force-recreate] [--silent]"
      echo ""
      echo "  --excel PATH        Path to ArbitrageX_Secrets_Config.xlsx (MEK source)"
      echo "  --check-only        Run pre-flight checks only, do not start services"
      echo "  --force-recreate    Force container recreation even if already running"
      echo "  --silent            Suppress banner (for systemd/cron use)"
      echo ""
      echo "  ENV: ARBX_EXCEL_PATH  Alternative to --excel"
      exit 0
      ;;
    *) echo "Unknown argument: $1"; exit 1 ;;
  esac
done

# ─── Logging ──────────────────────────────────────────────────────────────────
mkdir -p "$(dirname "$LOG_FILE")"

log() {
  local level="$1"; shift
  local msg="$*"
  local ts
  ts="$(date '+%Y-%m-%dT%H:%M:%S%z')"
  echo "{\"ts\":\"$ts\",\"level\":\"$level\",\"msg\":\"$msg\"}" >> "$LOG_FILE"
}

print() {
  [[ "$SILENT" == "true" ]] && return
  echo -e "$*"
}

# ─── Cleanup (siempre, incluso en error o SIGINT) ─────────────────────────────
cleanup() {
  local exit_code=$?
  if [[ -f "$ENV_TMP_PATH" ]]; then
    if command -v shred &>/dev/null; then
      shred -u -z -n 3 "$ENV_TMP_PATH" 2>/dev/null || rm -f "$ENV_TMP_PATH"
    else
      rm -P "$ENV_TMP_PATH" 2>/dev/null || rm -f "$ENV_TMP_PATH"
    fi
    log "info" "Temporary decrypted env shredded from /dev/shm"
  fi
  [[ -f "$LOCK_FILE" ]] && rm -f "$LOCK_FILE"
  if [[ $exit_code -ne 0 ]]; then
    log "error" "arbx-secure-start exited with code $exit_code"
  fi
}
trap cleanup EXIT INT TERM

# ─── Banner ───────────────────────────────────────────────────────────────────
print_banner() {
  [[ "$SILENT" == "true" ]] && return
  echo -e ""
  echo -e "${BOLD}${BLUE}╔══════════════════════════════════════════════════════════╗${RESET}"
  echo -e "${BOLD}${BLUE}║     ArbitrageX v2 — Secure Start  v${VERSION}              ║${RESET}"
  echo -e "${BOLD}${BLUE}║     AES-256-GCM · tmpfs · Idempotent · Enterprise         ║${RESET}"
  echo -e "${BOLD}${BLUE}╚══════════════════════════════════════════════════════════╝${RESET}"
  echo -e ""
}

# ─── Función de error con instrucciones claras ────────────────────────────────
die() {
  local code="$1"; shift
  local msg="$*"
  print ""
  print "${FAIL} ${BOLD}${RED}ERROR [${code}]:${RESET} ${msg}"
  print ""
  case "$code" in
    E001)
      print "${ARROW} El archivo .env.enc no existe en: ${ENV_ENC_PATH}"
      print "${ARROW} Solución: ejecuta primero ${BOLD}bash scripts/security/encrypt_env.sh${RESET}"
      ;;
    E002)
      print "${ARROW} No se encontró el archivo Excel de secrets."
      print "${ARROW} Opciones:"
      print "   1. Pasa la ruta: ${BOLD}bash arbx-secure-start.sh --excel /ruta/Excel.xlsx${RESET}"
      print "   2. Define la variable: ${BOLD}export ARBX_EXCEL_PATH=/ruta/Excel.xlsx${RESET}"
      print "   3. Copia el archivo a: ${BOLD}${PROJECT_ROOT}/ArbitrageX_Secrets_Config.xlsx${RESET}"
      ;;
    E003)
      print "${ARROW} Python3 no está instalado o no está en el PATH."
      print "${ARROW} Solución: ${BOLD}apt-get install -y python3${RESET}"
      ;;
    E004)
      print "${ARROW} Falta el paquete Python 'cryptography' o 'openpyxl'."
      print "${ARROW} Solución: ${BOLD}pip3 install cryptography openpyxl --break-system-packages${RESET}"
      ;;
    E005)
      print "${ARROW} Docker o Docker Compose no están disponibles."
      print "${ARROW} Solución: instala Docker Engine y el plugin compose."
      ;;
    E006)
      print "${ARROW} La desencriptación falló. Causas posibles:"
      print "   - El archivo Excel no es el correcto (clave incorrecta)"
      print "   - El archivo .env.enc está corrupto o fue modificado"
      print "${ARROW} Verifica el Master Key Hash con: ${BOLD}python3 vault_crypto.py get-key-hash --excel <Excel>${RESET}"
      ;;
    E007)
      print "${ARROW} Otro proceso de arranque seguro ya está en ejecución."
      print "${ARROW} Si es un proceso huérfano: ${BOLD}rm -f ${LOCK_FILE}${RESET}"
      ;;
    E008)
      print "${ARROW} /dev/shm no está disponible (tmpfs no montado)."
      print "${ARROW} Solución: ${BOLD}mount -t tmpfs tmpfs /dev/shm${RESET}"
      ;;
    E009)
      print "${ARROW} Uno o más servicios críticos no pasaron el health check."
      print "${ARROW} Revisa los logs: ${BOLD}docker compose -f ${COMPOSE_FILE} logs --tail=50${RESET}"
      ;;
  esac
  print ""
  log "error" "code=${code} msg=${msg}"
  exit 1
}

# ─── PRE-FLIGHT CHECKS ────────────────────────────────────────────────────────
preflight() {
  print "${BOLD}[1/6] Pre-flight checks${RESET}"

  # Lock: evitar ejecuciones paralelas
  if [[ -f "$LOCK_FILE" ]]; then
    local lock_pid
    lock_pid=$(cat "$LOCK_FILE" 2>/dev/null || echo "")
    if [[ -n "$lock_pid" ]] && kill -0 "$lock_pid" 2>/dev/null; then
      die E007 "Lock file exists with PID $lock_pid"
    else
      rm -f "$LOCK_FILE"
    fi
  fi
  echo $$ > "$LOCK_FILE"

  # /dev/shm
  if [[ ! -d "/dev/shm" ]]; then
    die E008 "/dev/shm not available"
  fi
  print "  ${OK} /dev/shm (tmpfs) disponible"

  # Python3
  if ! command -v python3 &>/dev/null; then
    die E003 "python3 not found"
  fi
  local py_ver
  py_ver=$(python3 --version 2>&1)
  print "  ${OK} ${py_ver}"

  # Dependencias Python
  if ! python3 -c "import cryptography, openpyxl" 2>/dev/null; then
    die E004 "cryptography or openpyxl not installed"
  fi
  print "  ${OK} Dependencias Python (cryptography, openpyxl)"

  # vault_crypto.py
  if [[ ! -f "$VAULT_CRYPTO" ]]; then
    die E001 "vault_crypto.py not found at $VAULT_CRYPTO"
  fi
  print "  ${OK} vault_crypto.py presente"

  # .env.enc
  if [[ ! -f "$ENV_ENC_PATH" ]]; then
    die E001 ".env.enc not found"
  fi
  local enc_size
  enc_size=$(wc -c < "$ENV_ENC_PATH")
  print "  ${OK} .env.enc presente (${enc_size} bytes)"

  # Excel
  if [[ -z "$EXCEL_PATH" ]]; then
    # Buscar en ubicaciones estándar
    for candidate in \
      "${PROJECT_ROOT}/ArbitrageX_Secrets_Config.xlsx" \
      "${HOME}/ArbitrageX_Secrets_Config.xlsx" \
      "/tmp/ArbitrageX_Secrets_Config.xlsx"; do
      if [[ -f "$candidate" ]]; then
        EXCEL_PATH="$candidate"
        break
      fi
    done
  fi
  if [[ -z "$EXCEL_PATH" ]] || [[ ! -f "$EXCEL_PATH" ]]; then
    die E002 "Excel secrets file not found"
  fi
  print "  ${OK} Excel: ${EXCEL_PATH}"

  # Docker
  if ! command -v docker &>/dev/null; then
    die E005 "docker not found"
  fi
  if ! docker compose version &>/dev/null; then
    die E005 "docker compose plugin not found"
  fi
  print "  ${OK} Docker $(docker --version | awk '{print $3}' | tr -d ',')"

  # compose.prod.yml
  if [[ ! -f "$COMPOSE_FILE" ]]; then
    die E001 "compose.prod.yml not found at $COMPOSE_FILE"
  fi
  print "  ${OK} compose.prod.yml presente"

  log "info" "preflight=passed excel=${EXCEL_PATH} enc_size=${enc_size}"
}

# ─── DERIVAR CLAVE Y DESENCRIPTAR ─────────────────────────────────────────────
decrypt_env() {
  print ""
  print "${BOLD}[2/6] Derivando Master Key desde Excel${RESET}"

  local key_hash
  key_hash=$(python3 "$VAULT_CRYPTO" get-key-hash --excel "$EXCEL_PATH" 2>&1)
  if [[ $? -ne 0 ]]; then
    die E006 "Failed to derive MEK: $key_hash"
  fi
  print "  ${OK} ${key_hash}"
  log "info" "mek_derived ${key_hash}"

  print ""
  print "${BOLD}[3/6] Desencriptando .env.enc → /dev/shm (RAM)${RESET}"

  if ! python3 "$VAULT_CRYPTO" decrypt \
      --excel "$EXCEL_PATH" \
      --in-file "$ENV_ENC_PATH" \
      --out-file "$ENV_TMP_PATH" 2>&1; then
    die E006 "Decryption failed"
  fi

  chmod 600 "$ENV_TMP_PATH"
  local line_count
  line_count=$(wc -l < "$ENV_TMP_PATH")
  print "  ${OK} Desencriptado: ${line_count} líneas en /dev/shm (chmod 600)"
  log "info" "decryption=ok lines=${line_count} path=/dev/shm"
}

# ─── LEVANTAR SERVICIOS (IDEMPOTENTE) ─────────────────────────────────────────
start_services() {
  print ""
  print "${BOLD}[4/6] Levantando servicios Docker Compose${RESET}"

  # compose.prod.yml usa env_file: - ../.env (hardcoded)
  # Creamos un symlink atómico: PROJECT_ROOT/.env → /dev/shm (tmpfs)
  # El symlink se elimina junto con el archivo en el trap EXIT
  local ENV_LINK="${PROJECT_ROOT}/.env"

  # Si ya existe un .env real (no symlink), hacer backup
  if [[ -f "$ENV_LINK" ]] && [[ ! -L "$ENV_LINK" ]]; then
    mv "$ENV_LINK" "${ENV_LINK}.bak.$(date +%s)"
    print "  ${WARN} .env existente movido a backup"
  fi

  # Crear symlink .env → /dev/shm/.arbx.env.tmp
  ln -sf "$ENV_TMP_PATH" "$ENV_LINK"
  print "  ${OK} Symlink .env → /dev/shm (tmpfs)"

  local compose_args=("compose" "-f" "$COMPOSE_FILE")

  if [[ "$FORCE_RECREATE" == "true" ]]; then
    print "  ${WARN} Modo --force-recreate: recreando contenedores..."
    docker "${compose_args[@]}" up -d --force-recreate 2>&1
    log "info" "compose=force-recreate"
  else
    local running_count
    running_count=$(docker ps --filter "name=arbitragex-v2" --filter "status=running" -q | wc -l)
    if [[ "$running_count" -gt 15 ]]; then
      print "  ${INFO} ${running_count} contenedores ya en ejecución — modo idempotente (--no-recreate)"
      docker "${compose_args[@]}" up -d --no-recreate 2>&1
      log "info" "compose=no-recreate running=${running_count}"
    else
      print "  ${INFO} Iniciando ${running_count} servicios..."
      docker "${compose_args[@]}" up -d 2>&1
      log "info" "compose=up running_before=${running_count}"
    fi
  fi

  # Eliminar el symlink inmediatamente (el archivo en /dev/shm se elimina en trap EXIT)
  rm -f "$ENV_LINK"
  print "  ${OK} Symlink .env eliminado"
  print "  ${OK} docker compose up completado"
}

# ─── HEALTH CHECK ─────────────────────────────────────────────────────────────
health_check() {
  print ""
  print "${BOLD}[5/6] Verificando health de servicios críticos${RESET}"

  local max_wait=120
  local interval=5
  local elapsed=0
  local all_healthy=false

  while [[ $elapsed -lt $max_wait ]]; do
    local healthy_count=0
    for svc in "${CRITICAL_SERVICES[@]}"; do
      local status
      status=$(docker inspect --format='{{.State.Health.Status}}' "$svc" 2>/dev/null || echo "not_found")
      if [[ "$status" == "healthy" ]]; then
        healthy_count=$((healthy_count + 1))
      fi
    done

    if [[ $healthy_count -eq ${#CRITICAL_SERVICES[@]} ]]; then
      all_healthy=true
      break
    fi

    print "  ${WARN} Esperando health checks (${elapsed}s/${max_wait}s) — ${healthy_count}/${#CRITICAL_SERVICES[@]} healthy..."
    sleep $interval
    ((elapsed += interval))
  done

  if [[ "$all_healthy" == "true" ]]; then
    for svc in "${CRITICAL_SERVICES[@]}"; do
      print "  ${OK} ${svc}: healthy"
    done
    log "info" "health_check=passed services=${#CRITICAL_SERVICES[@]}"
  else
    # Mostrar estado actual aunque falle
    for svc in "${CRITICAL_SERVICES[@]}"; do
      local status
      status=$(docker inspect --format='{{.State.Health.Status}}' "$svc" 2>/dev/null || echo "not_found")
      print "  ${FAIL} ${svc}: ${status}"
    done
    die E009 "Health check timeout after ${max_wait}s"
  fi
}

# ─── REPORTE FINAL ────────────────────────────────────────────────────────────
final_report() {
  print ""
  print "${BOLD}[6/6] Estado final del sistema${RESET}"

  local total
  total=$(docker ps --filter "name=arbitragex-v2" --filter "status=running" -q | wc -l)
  print "  ${OK} ${BOLD}${total} servicios corriendo${RESET}"

  # Mostrar tabla de servicios
  print ""
  docker ps --filter "name=arbitragex-v2" \
    --format "  {{.Names}}\t{{.Status}}" 2>/dev/null | \
    grep -v "^$" | head -20 || true

  print ""
  print "${BOLD}${GREEN}╔══════════════════════════════════════════════════════════╗${RESET}"
  print "${BOLD}${GREEN}║  ArbitrageX v2 — ONLINE  ·  Secrets: AES-256-GCM        ║${RESET}"
  print "${BOLD}${GREEN}║  .env.enc en disco  ·  Plaintext: NUNCA en disco         ║${RESET}"
  print "${BOLD}${GREEN}╚══════════════════════════════════════════════════════════╝${RESET}"
  print ""

  log "info" "startup=complete services=${total}"
}

# ─── MAIN ─────────────────────────────────────────────────────────────────────
main() {
  log "info" "arbx-secure-start v${VERSION} started pid=$$"

  print_banner

  preflight

  if [[ "$CHECK_ONLY" == "true" ]]; then
    print ""
    print "${OK} ${BOLD}Pre-flight checks pasados. Sistema listo para arranque seguro.${RESET}"
    print "${INFO} Ejecuta sin --check-only para iniciar los servicios."
    log "info" "check-only=true all_checks=passed"
    exit 0
  fi

  decrypt_env
  start_services
  health_check
  final_report
}

main "$@"
