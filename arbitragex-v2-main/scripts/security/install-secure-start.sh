#!/usr/bin/env bash
# =============================================================================
# ArbitrageX v2 — Instalador del Sistema de Arranque Seguro
# install-secure-start.sh
# =============================================================================
# Ejecutar UNA SOLA VEZ en el VPS para configurar todo.
# Idempotente: puede ejecutarse múltiples veces sin efectos negativos.
#
# USO:
#   bash install-secure-start.sh [--excel /ruta/al/Excel.xlsx]
#
# REQUISITOS:
#   - Ejecutar como root en el VPS
#   - El archivo .env.enc debe existir (ejecuta encrypt_env.sh primero)
#   - El Excel debe estar disponible para la primera verificación
# =============================================================================

set -euo pipefail

RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'
BLUE='\033[0;34m'; BOLD='\033[1m'; RESET='\033[0m'
OK="${GREEN}✔${RESET}"; FAIL="${RED}✘${RESET}"; INFO="${BLUE}ℹ${RESET}"

PROJECT_ROOT="/opt/arbitragex-v2"
SCRIPTS_DIR="${PROJECT_ROOT}/scripts/security"
SERVICE_NAME="arbx-secure-start"
SERVICE_FILE="/etc/systemd/system/${SERVICE_NAME}.service"
EXCEL_PATH="${1:-${ARBX_EXCEL_PATH:-${PROJECT_ROOT}/ArbitrageX_Secrets_Config.xlsx}}"

# Parsear --excel
while [[ $# -gt 0 ]]; do
  case "$1" in
    --excel) EXCEL_PATH="$2"; shift 2 ;;
    *) shift ;;
  esac
done

echo -e ""
echo -e "${BOLD}${BLUE}╔══════════════════════════════════════════════════════════╗${RESET}"
echo -e "${BOLD}${BLUE}║  ArbitrageX v2 — Instalador de Arranque Seguro           ║${RESET}"
echo -e "${BOLD}${BLUE}╚══════════════════════════════════════════════════════════╝${RESET}"
echo -e ""

# ─── Verificar root ───────────────────────────────────────────────────────────
if [[ "$EUID" -ne 0 ]]; then
  echo -e "${FAIL} Este script debe ejecutarse como root."
  exit 1
fi
echo -e "${OK} Ejecutando como root"

# ─── Verificar que el proyecto existe ────────────────────────────────────────
if [[ ! -d "$PROJECT_ROOT" ]]; then
  echo -e "${FAIL} Directorio del proyecto no encontrado: ${PROJECT_ROOT}"
  exit 1
fi
echo -e "${OK} Proyecto encontrado: ${PROJECT_ROOT}"

# ─── Instalar dependencias Python ────────────────────────────────────────────
echo -e ""
echo -e "${INFO} Instalando dependencias Python..."
if python3 -c "import cryptography, openpyxl" 2>/dev/null; then
  echo -e "${OK} cryptography y openpyxl ya instalados"
else
  pip3 install cryptography openpyxl --break-system-packages -q
  echo -e "${OK} cryptography y openpyxl instalados"
fi

# ─── Verificar .env.enc ───────────────────────────────────────────────────────
echo -e ""
if [[ ! -f "${PROJECT_ROOT}/.env.enc" ]]; then
  echo -e "${FAIL} .env.enc no encontrado. Ejecuta primero:"
  echo -e "   bash ${SCRIPTS_DIR}/encrypt_env.sh"
  exit 1
fi
echo -e "${OK} .env.enc presente ($(wc -c < "${PROJECT_ROOT}/.env.enc") bytes)"

# ─── Verificar que el Excel puede derivar la clave ───────────────────────────
echo -e ""
if [[ -f "$EXCEL_PATH" ]]; then
  echo -e "${INFO} Verificando Master Key desde Excel..."
  KEY_HASH=$(python3 "${SCRIPTS_DIR}/vault_crypto.py" get-key-hash --excel "$EXCEL_PATH" 2>&1)
  if [[ $? -eq 0 ]]; then
    echo -e "${OK} ${KEY_HASH}"
    # Verificar que desencripta correctamente
    python3 "${SCRIPTS_DIR}/vault_crypto.py" decrypt \
      --excel "$EXCEL_PATH" \
      --in-file "${PROJECT_ROOT}/.env.enc" \
      --out-file "/dev/shm/.arbx.install.verify.tmp" 2>&1
    if [[ $? -eq 0 ]]; then
      echo -e "${OK} Desencriptación verificada correctamente"
      shred -u "/dev/shm/.arbx.install.verify.tmp" 2>/dev/null || \
        rm -f "/dev/shm/.arbx.install.verify.tmp"
    else
      echo -e "${FAIL} La desencriptación falló. Verifica que el Excel es el correcto."
      exit 1
    fi
  else
    echo -e "${FAIL} No se pudo derivar la clave: ${KEY_HASH}"
    exit 1
  fi
else
  echo -e "${YELLOW}⚠ Excel no encontrado en ${EXCEL_PATH}${RESET}"
  echo -e "  El servicio systemd buscará el Excel en esa ruta al arrancar."
  echo -e "  Asegúrate de que esté disponible antes de reiniciar el VPS."
fi

# ─── Permisos de los scripts ──────────────────────────────────────────────────
echo -e ""
echo -e "${INFO} Configurando permisos..."
chmod +x "${SCRIPTS_DIR}/arbx-secure-start.sh"
chmod +x "${SCRIPTS_DIR}/encrypt_env.sh"
chmod +x "${SCRIPTS_DIR}/start_secure.sh"
chmod +x "${SCRIPTS_DIR}/vault_crypto.py"
chmod 600 "${PROJECT_ROOT}/.env.enc"
echo -e "${OK} Permisos configurados"

# ─── Crear directorio de logs ─────────────────────────────────────────────────
mkdir -p "${PROJECT_ROOT}/logs"
chmod 750 "${PROJECT_ROOT}/logs"
echo -e "${OK} Directorio de logs: ${PROJECT_ROOT}/logs/"

# ─── Instalar servicio systemd ────────────────────────────────────────────────
echo -e ""
echo -e "${INFO} Instalando servicio systemd..."

# Copiar el archivo .service con la ruta del Excel configurada
sed "s|ARBX_EXCEL_PATH=.*|ARBX_EXCEL_PATH=${EXCEL_PATH}|g" \
  "${SCRIPTS_DIR}/arbx-secure-start.service" > "$SERVICE_FILE"

systemctl daemon-reload
echo -e "${OK} systemd daemon recargado"

# Habilitar el servicio para que arranque en cada boot
systemctl enable "${SERVICE_NAME}.service"
echo -e "${OK} Servicio habilitado: ${SERVICE_NAME}.service"

# ─── Deshabilitar el docker.service para que NO arranque solo ─────────────────
# Queremos que SOLO arbx-secure-start.service inicie los contenedores
# Docker sí debe arrancar, pero los contenedores no deben iniciarse solos
echo -e ""
echo -e "${INFO} Configurando Docker para no auto-iniciar contenedores..."

# Asegurarse de que los contenedores no tengan restart policy automática
# (el systemd service se encarga del arranque)
CONTAINERS=$(docker ps -a --filter "name=arbitragex-v2" -q 2>/dev/null || echo "")
if [[ -n "$CONTAINERS" ]]; then
  # Cambiar restart policy a 'no' para que systemd sea el único que los inicie
  echo "$CONTAINERS" | xargs -I{} docker update --restart=no {} 2>/dev/null || true
  echo -e "${OK} Restart policy de contenedores → 'no' (systemd es el controlador)"
fi

# ─── Verificación final ───────────────────────────────────────────────────────
echo -e ""
echo -e "${INFO} Verificación final del servicio..."
systemctl is-enabled "${SERVICE_NAME}.service" && \
  echo -e "${OK} Servicio habilitado en systemd" || \
  echo -e "${FAIL} El servicio NO está habilitado"

# ─── Resumen ──────────────────────────────────────────────────────────────────
echo -e ""
echo -e "${BOLD}${GREEN}╔══════════════════════════════════════════════════════════╗${RESET}"
echo -e "${BOLD}${GREEN}║  INSTALACIÓN COMPLETADA                                  ║${RESET}"
echo -e "${BOLD}${GREEN}╚══════════════════════════════════════════════════════════╝${RESET}"
echo -e ""
echo -e "${OK} ${BOLD}Sistema de arranque seguro instalado y configurado.${RESET}"
echo -e ""
echo -e "  ${INFO} En cada reinicio del VPS, systemd ejecutará automáticamente:"
echo -e "     ${BOLD}bash ${SCRIPTS_DIR}/arbx-secure-start.sh --silent${RESET}"
echo -e ""
echo -e "  ${INFO} Para iniciar manualmente:"
echo -e "     ${BOLD}systemctl start ${SERVICE_NAME}${RESET}"
echo -e ""
echo -e "  ${INFO} Para ver el estado:"
echo -e "     ${BOLD}systemctl status ${SERVICE_NAME}${RESET}"
echo -e "     ${BOLD}journalctl -u ${SERVICE_NAME} -f${RESET}"
echo -e ""
echo -e "  ${INFO} Para ver el log estructurado:"
echo -e "     ${BOLD}tail -f ${PROJECT_ROOT}/logs/secure-start.log | python3 -m json.tool${RESET}"
echo -e ""
echo -e "  ${YELLOW}⚠ IMPORTANTE: El Excel debe estar en:${RESET}"
echo -e "     ${BOLD}${EXCEL_PATH}${RESET}"
echo -e "     antes de cada reinicio del VPS (o configura ARBX_EXCEL_PATH)."
echo -e ""
