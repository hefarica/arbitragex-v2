#!/usr/bin/env bash
# ArbitrageX v2 — Stack Watchdog (runs every 1 min via cron)
#
# HARDENING: if the stack is partially or fully down (OOM, host reboot,
# Docker daemon crash, failed deploy, etc.), this script brings it back up.
# Idempotent: if everything is healthy, it does nothing (zero cost).
#
# Install (one-time):
#   sudo cp scripts/vps/stack-watchdog.sh /usr/local/bin/arbx-watchdog.sh
#   sudo chmod +x /usr/local/bin/arbx-watchdog.sh
#   echo "* * * * * root /usr/local/bin/arbx-watchdog.sh >> /var/log/arbx-watchdog.log 2>&1" | sudo tee /etc/cron.d/arbx-watchdog
#
# Log: /var/log/arbx-watchdog.log

set -euo pipefail

COMPOSE_DIR="/opt/arbitragex-v2"
COMPOSE_FILE="docker/compose.prod.yml"
ENV_FILE=".env"
EXPECTED_SERVICES=24
MIN_CRITICAL=4  # postgres, redis, api-server, edge — if any missing → restore

cd "$COMPOSE_DIR"

# Count running containers
RUNNING=$(docker ps --format '{{.Names}}' 2>/dev/null | grep -c arbitragex || echo "0")

# Quick check: if full stack is up, exit immediately (zero cost)
if [ "$RUNNING" -ge "$EXPECTED_SERVICES" ]; then
  exit 0
fi

# Something is down — diagnose + restore
TIMESTAMP=$(date -u +%Y-%m-%dT%H:%M:%SZ)
echo "[$TIMESTAMP] WATCHDOG: only $RUNNING/$EXPECTED_SERVICES containers running — restoring..."

# Check which critical services are missing
for svc in postgres redis api-server edge; do
  if ! docker ps --format '{{.Names}}' 2>/dev/null | grep -q "arbitragex-v2-${svc}-1"; then
    echo "[$TIMESTAMP] WATCHDOG: $svc is DOWN"
  fi
done

# RESTORE: bring up everything (idempotent — running containers are untouched)
docker compose --env-file "$ENV_FILE" -f "$COMPOSE_FILE" up -d \
  postgres redis anvil socket-proxy \
  searcher-rs sim-ctl relays-client recon selector-api math-engine \
  api-server edge frontend \
  token-enricher prometheus loki promtail grafana alertmanager minio vault \
  thanos-sidecar thanos-store thanos-query \
  2>&1 | tail -5

# Verify
sleep 5
NEW_RUNNING=$(docker ps --format '{{.Names}}' 2>/dev/null | grep -c arbitragex || echo "0")
echo "[$TIMESTAMP] WATCHDOG: restore complete — $NEW_RUNNING/$EXPECTED_SERVICES containers now running"

if [ "$NEW_RUNNING" -lt "$MIN_CRITICAL" ]; then
  echo "[$TIMESTAMP] WATCHDOG: CRITICAL — still below $MIN_CRITICAL after restore. Manual intervention required."
fi
