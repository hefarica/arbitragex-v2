#!/bin/bash
# =============================================================================
# ArbitrageX v2 — Firewall Monitor
# /usr/local/bin/arbx-firewall-monitor.sh
# =============================================================================
# Verifica periódicamente que no exista la regla de bloqueo iptables
# que impide el acceso externo a los puertos de desarrollo (8787, 5173).
#
# USO:
#   Manual: sudo bash /usr/local/bin/arbx-firewall-monitor.sh
#   Systemd: systemctl start arbx-firewall-monitor
#
# CRON (cada 5 minutos):
#   */5 * * * * root /usr/local/bin/arbx-firewall-monitor.sh --check
# =============================================================================

set -euo pipefail

LOG_FILE="/var/log/arbx-firewall-monitor.log"
RULE_PATTERN="DROP.*multiport.*dports.*8787"

log() {
    local level="$1"
    local msg="$2"
    local ts
    ts="$(date '+%Y-%m-%dT%H:%M:%S%z')"
    echo "[$ts] [$level] $msg" | tee -a "$LOG_FILE" > /dev/null
}

check_and_fix() {
    local found=0

    # Eliminar todas las reglas coincidentes desde el final hacia el principio
    # para mantener los números de línea válidos durante el borrado.
    while iptables -L DOCKER-USER -n | grep -qE "$RULE_PATTERN"; do
        found=1
        log "WARN" "Blocking rule found in DOCKER-USER chain! Removing..."

        local line_num
        line_num=$(iptables -L DOCKER-USER -n --line-numbers | grep -E "$RULE_PATTERN" | tail -n1 | awk '{print $1}')

        if [[ -n "$line_num" ]]; then
            iptables -D DOCKER-USER "$line_num"
            log "INFO" "Blocking rule removed from line $line_num"
        fi
    done

    if [[ "$found" -eq 1 ]]; then
        # Guardar las reglas actualizadas
        if iptables-save > /etc/iptables/rules.v4 2>/dev/null; then
            log "INFO" "iptables rules saved to /etc/iptables/rules.v4"
        else
            log "WARN" "Could not save iptables rules to /etc/iptables/rules.v4"
        fi
        return 1  # Indica que se encontró y eliminó al menos una regla
    fi

    return 0  # No se encontró regla de bloqueo
}

main() {
    local check_only=false

    if [[ "${1:-}" == "--check" ]]; then
        check_only=true
    fi

    # Crear log file si no existe
    touch "$LOG_FILE" 2>/dev/null || true

    if [[ "$check_only" == true ]]; then
        # Modo silencioso para cron
        if ! check_and_fix > /dev/null 2>&1; then
            # Se eliminó una regla - notificar
            logger -t arbx-firewall-monitor "ALERT: Blocking rule auto-removed from iptables"
        fi
    else
        # Modo verbose para ejecución manual
        echo "ArbitrageX v2 — Firewall Monitor"
        echo "================================="
        echo ""

        if check_and_fix; then
            echo "✓ No blocking rules found. Firewall is clean."
            echo ""
            echo "Current DOCKER-USER chain:"
            iptables -L DOCKER-USER -n
        else
            echo "✗ Blocking rule was found and removed!"
            echo ""
            echo "Updated DOCKER-USER chain:"
            iptables -L DOCKER-USER -n
        fi
    fi
}

main "$@"
