#!/usr/bin/env bash
# ArbitrageX v2 — Stack Watchdog (runs every 1 min via cron)
#
# HARDENING: if the stack is partially or fully down (OOM, host reboot,
# Docker daemon crash, failed deploy, etc.), this script brings it back up.
# Idempotent: if everything is healthy, it does nothing (zero cost).
#
# DEPLOY-WATCHDOG-RACE gate (GEN-CI-FAIL attempts 1+3, 2026-08-31): the
# watchdog used to fire a CONCURRENT `docker compose up` while the deploy
# workflow's --force-recreate transiently dropped the container count below
# 24 — both processes then raced the same containers and the deploy died
# with "Error response from daemon: No such container: …". The watchdog now
# takes the SAME flock the deploy holds (/tmp/arbx-deploy.lock) and DEFERS
# (exit 0) while a deploy owns it. The stack is mid-recreation by the deploy
# itself in that window — "restoring" it concurrently is exactly the bug.
#
# ARBX-QUOTA-BACKOFF-01 (2026-09-04): si el crash de un contenedor es cuota
# RPC agotada (Alchemy 429 "Monthly capacity limit exceeded"), el
# force-recreate NO revive un endpoint sin cuota: cada intento quema más CU
# contra la cuota muerta Y resetea el backoff exponencial natural de docker
# (el recreate loop por minuto fue el amplificador que agotó la cuota
# mensual). Con el fallo detectado en los últimos logs, se omite el recreate
# 1 hora (marker file) y docker retiene su propio backoff.
#
# Install (one-time):
#   sudo cp scripts/vps/stack-watchdog.sh /usr/local/bin/arbx-watchdog.sh
#   sudo chmod +x /usr/local/bin/arbx-watchdog.sh
#   echo "* * * * * root /usr/local/bin/arbx-watchdog.sh >> /var/log/arbx-watchdog.log 2>&1" | sudo tee /etc/cron.d/arbx-watchdog
#
# Log: /var/log/arbx-watchdog.log

set -euo pipefail

cd /opt/arbitragex-v2
TS=$(date -u +%Y-%m-%dT%H:%M:%SZ)
EXPECTED=24

# ── DEPLOY LOCK: defer while a deploy owns the stack mutation window ──
DEPLOY_LOCK="/tmp/arbx-deploy.lock"
touch "$DEPLOY_LOCK" && chmod 666 "$DEPLOY_LOCK" 2>/dev/null || true
exec 9>"$DEPLOY_LOCK"
if ! flock -n 9; then
  echo "[$TS] WATCHDOG: deploy lock held (${DEPLOY_LOCK}) — deferring; deploy owns the stack mutation window"
  exit 0
fi

# ── LAYER 1: detect crash-looping containers (status: Restarting) ──
RESTARTING=$(docker ps --format '{{.Names}} {{.Status}}' 2>/dev/null | grep arbitragex | grep -i 'Restarting' | awk '{print $1}' || true)
if [ -n "$RESTARTING" ]; then
  for svc in $RESTARTING; do
    # Map container name → compose service name: arbitragex-v2-selector-api-1 → selector-api
    SERVICE=$(echo "$svc" | sed 's/^arbitragex-v2-//' | sed 's/-[0-9]*$//')
    # ARBX-QUOTA-BACKOFF-01: quota-dead upstream → recreate only burns more CU.
    if docker logs --tail 5 "$svc" 2>&1 | grep -qiE 'monthly capacity limit exceeded|quota.*exceed|429 too many'; then
      MARKER="/tmp/arbx-wd-quota-$SERVICE"
      AGE=$(( $(date +%s) - $(stat -c %Y "$MARKER" 2>/dev/null || echo 0) ))
      if [ "$AGE" -gt 3600 ]; then
        echo "[$TS] WATCHDOG: $svc crash por CUOTA RPC agotada (429) — recreate omitido 1h (no quemar cuota; docker retiene su backoff)"
        touch "$MARKER"
      fi
      continue
    fi
    rm -f "/tmp/arbx-wd-quota-$SERVICE" 2>/dev/null || true
    echo "[$TS] WATCHDOG: $svc is CRASH-LOOPING — force-recreating service: $SERVICE"
    docker compose --env-file .env -f docker/compose.prod.yml up -d --force-recreate "$SERVICE" 2>&1 | tail -1
  done
  sleep 5
fi

# ── LAYER 2: detect missing containers (count < 24) ──
RUNNING=$(docker ps --format '{{.Names}}' 2>/dev/null | grep -c arbitragex || echo "0")
if [ "$RUNNING" -ge "$EXPECTED" ]; then exit 0; fi
echo "[$TS] WATCHDOG: $RUNNING/$EXPECTED running — restoring full stack..."
docker compose --env-file .env -f docker/compose.prod.yml up -d \
  postgres redis anvil socket-proxy \
  searcher-rs sim-ctl relays-client recon selector-api math-engine \
  api-server edge frontend \
  token-enricher prometheus loki promtail grafana alertmanager minio vault \
  thanos-sidecar thanos-store thanos-query 2>&1 | tail -3
sleep 5
NEW=$(docker ps --format '{{.Names}}' 2>/dev/null | grep -c arbitragex || echo "0")
echo "[$TS] WATCHDOG: restore done — $NEW/$EXPECTED running"
