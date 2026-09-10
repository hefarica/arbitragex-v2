#!/usr/bin/env bash
# ============================================================================
# ARBX-DISK-GUARD-01 — guard de capacidad de disco (2026-09-10)
#
# Diagnóstico (audits/retention-02-2026-09-10): el disco (150GB) NO se llena
# por tiempo sino por CAPACIDAD: cada deploy deposita ~20.8GB de buildkit cache
# en pocos layers gigantes (record de 1.1GB), y el cron semanal de builder
# prune (RETENTION-01) no contiene 2-4 deploys/día. El ocupante estructural es
# el volume de postgres (113GB, route_discovery_outcomes ~82GB en equilibrio
# inserta≈borra 20M filas/día).
#
# Umbral (peor evento unitario = 1 deploy ≈ 8GB temp build + 20GB cache + 1GB WAL):
#   libre >= 30GB (~80% usado) → zona segura, silencioso (R9: sin log-flood)
#   libre <  30GB → WARN: builder prune --min-free-space=20GB + image prune
#   libre <  15GB → CRIT: además volume prune (anónimos sin uso)
# (--min-free-space existe desde docker 28: poda cache hasta lograr X libres.
#  El viejo --keep-storage quedó deprecado; verificado con --help en el VPS.)
#
# Cron VPS (root): 13 * * * * /opt/arbitragex-v2/scripts/disk_guard.sh >> /var/log/arbx-disk-guard.log 2>&1
# El cron semanal de builder prune se RETIENE como backstop (el guard ya poda
# hasta 20GB libres cuando dispara; el semanal no tiene costo).
# ============================================================================
set -uo pipefail

WARN_MB=$((30 * 1024))
CRIT_MB=$((15 * 1024))

avail_mb() { df / | awk 'NR==2 {print int($4/1024)}'; }

AVAIL=$(avail_mb)
TS=$(date -u +%Y-%m-%dT%H:%M:%SZ)

if [ "$AVAIL" -ge "$WARN_MB" ]; then
  exit 0
fi

USED_PCT=$(df / | awk 'NR==2 {gsub(/%/,""); print $5}')
if [ "$AVAIL" -lt "$CRIT_MB" ]; then
  echo "$TS disk_guard level=CRIT avail_mb=$AVAIL used_pct=$USED_PCT accion=builder_prune+image_prune+volume_prune"
  docker builder prune -af --min-free-space 20GB
  docker image prune -f
  docker volume prune -f
else
  echo "$TS disk_guard level=WARN avail_mb=$AVAIL used_pct=$USED_PCT accion=builder_prune+image_prune"
  docker builder prune -af --min-free-space 20GB
  docker image prune -f
fi
echo "$TS disk_guard post avail_mb=$(avail_mb)"
