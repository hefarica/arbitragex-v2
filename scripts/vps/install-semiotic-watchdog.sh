#!/usr/bin/env bash
# =============================================================================
# SEMIOTIC-BRIDGE WATCHDOG INSTALLER (VPS-SIDE)
# =============================================================================
# Purpose: Idempotently installs a systemd timer + service that runs the
#          semiotic-bridge protection script every 5 minutes.
#
# Usage (as root or via sudo on VPS):
#   bash scripts/vps/install-semiotic-watchdog.sh
#
# Uninstall:
#   systemctl disable --now semiotic-bridge-watchdog.timer
#   rm -f /etc/systemd/system/semiotic-bridge-watchdog.{service,timer}
# =============================================================================
set -euo pipefail

DEPLOY_PATH="/opt/arbitragex-v2"
PROTECT_SCRIPT="${DEPLOY_PATH}/scripts/vps/semiotic-bridge-protect.sh"
LOG_DIR="${DEPLOY_PATH}/logs"
SERVICE_NAME="semiotic-bridge-watchdog"

if [ "$EUID" -ne 0 ]; then
  echo "ERROR: This installer must run as root (systemd unit files in /etc/systemd/system)." >&2
  exit 1
fi

if [ ! -f "$PROTECT_SCRIPT" ]; then
  echo "ERROR: Protection script not found: ${PROTECT_SCRIPT}" >&2
  echo "Ensure the repo is cloned to ${DEPLOY_PATH} and scripts/vps/ are present." >&2
  exit 1
fi

mkdir -p "$LOG_DIR"
chown root:root "$PROTECT_SCRIPT"
chmod 755 "$PROTECT_SCRIPT"

# ---------------------------------------------------------------------------
# systemd service unit
# ---------------------------------------------------------------------------
cat > "/etc/systemd/system/${SERVICE_NAME}.service" << EOF
[Unit]
Description=Semiotic-Bridge Protection & Integrity Watchdog
After=network.target

[Service]
Type=oneshot
ExecStart=${PROTECT_SCRIPT} --watchdog
StandardOutput=append:${LOG_DIR}/semiotic-watchdog.log
StandardError=append:${LOG_DIR}/semiotic-watchdog.log
Environment="DEPLOY_PATH=${DEPLOY_PATH}"
WorkingDirectory=${DEPLOY_PATH}
EOF

# ---------------------------------------------------------------------------
# systemd timer unit
# ---------------------------------------------------------------------------
cat > "/etc/systemd/system/${SERVICE_NAME}.timer" << EOF
[Unit]
Description=Run semiotic-bridge protection every 5 minutes

[Timer]
OnBootSec=2min
OnUnitActiveSec=5min
AccuracySec=30s
Persistent=true

[Install]
WantedBy=timers.target
EOF

# ---------------------------------------------------------------------------
# Reload & enable
# ---------------------------------------------------------------------------
systemctl daemon-reload
systemctl enable --now "${SERVICE_NAME}.timer"

echo "=== INSTALLATION COMPLETE ==="
echo "Service:  ${SERVICE_NAME}.service"
echo "Timer:    ${SERVICE_NAME}.timer"
echo "Script:   ${PROTECT_SCRIPT}"
echo "Logs:     ${LOG_DIR}/semiotic-watchdog.log"
echo ""
echo "Status:"
systemctl status "${SERVICE_NAME}.timer" --no-pager || true
echo ""
echo "Next runs:"
systemctl list-timers "${SERVICE_NAME}.timer" --no-pager || true
