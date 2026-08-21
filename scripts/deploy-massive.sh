#!/bin/bash
# ═══════════════════════════════════════════════════════════════════════════
# DEPLOY MASIVO — habilita todo lo desarrollado
# Ejecutar cuando ≥10 de los 12 PRs hayan fusionado
# ═══════════════════════════════════════════════════════════════════════════
set -euo pipefail
cd /opt/arbitragex-v2

echo "════════ DEPLOY MASIVO — $(date -u +%FT%TZ) ════════"
echo "HEAD antes: $(git rev-parse HEAD)"

# 1. Pull latest main
git pull origin main
echo "HEAD después: $(git rev-parse HEAD)"

# 2. Aplicar migraciones (pool_cycles 107, etc.)
echo "── Migraciones ──"
bash database/run_migrations.sh 2>&1 | tail -3

# 3. Setear env vars para nuevas funcionalidades
echo "── Env vars nuevas ──"
# Route scanner proactivo (RU-3): scan por bloque con anchors
grep -q "^ARBX_ROUTE_SCANNER_MODE=" .env || echo "ARBX_ROUTE_SCANNER_MODE=on" >> .env
# sim-ctl B2c real-sim (A2 secuela): backend revm apuntando al fork Anvil
grep -q "^SIM_BACKEND=" .env || echo "SIM_BACKEND=revm" >> .env
grep -q "^REVM_RPC_URL=" .env || echo "REVM_RPC_URL=http://anvil:8545" >> .env
echo "Env vars OK"

# 4. Rebuild servicios tocados por los PRs
SERVICES="searcher-rs sim-ctl api-server frontend token-enricher relays-client edge"
echo "── Build: ${SERVICES} ──"
for svc in ${SERVICES}; do
    echo "  Building ${svc}..."
    CONFIRM_PROD_DEPLOY=true COMPOSE_FILE=docker/compose.prod.yml \
        docker compose --env-file .env -f docker/compose.prod.yml \
        build --no-cache "${svc}" 2>&1 | tail -1
done

# 5. Recrear contenedores
echo "── Deploy: ${SERVICES} ──"
CONFIRM_PROD_DEPLOY=true COMPOSE_FILE=docker/compose.prod.yml \
    docker compose --env-file .env -f docker/compose.prod.yml \
    up -d ${SERVICES}

# 6. Verificar salud
echo "── Verificación ──"
sleep 20
docker ps --format "{{.Names}}\t{{.Status}}" | grep -E "$(echo ${SERVICES} | tr ' ' '|')" | while read line; do
    echo "  ${line}"
done

# 7. Verificar pipeline fluye
echo "── Pipeline ──"
MAX_AGE=$(docker exec arbitragex-v2-postgres-1 psql -U postgres -d arbitragex -At -c \
    "SELECT EXTRACT(EPOCH FROM now() - MAX(detected_at)) FROM opportunities")
echo "  Último insert hace: ${MAX_AGE}s"

# 8. Verificar §IV blockers cerrados
echo "── §IV Motor ──"
# A1: route_metadata debe estar poblado
RM_COUNT=$(docker exec arbitragex-v2-postgres-1 psql -U postgres -d arbitragex -At -c \
    "SELECT COUNT(*) FROM opportunities WHERE route_metadata IS NOT NULL AND route_metadata != '{}'::jsonb AND detected_at > now() - interval '5 minutes'")
echo "  A1 route_metadata (últimos 5min): ${RM_COUNT} filas"

# A2: executor deployado
EXECUTOR_CHECK=$(docker exec arbitragex-v2-sim-ctl-1 curl -s http://localhost:3003/capabilities 2>/dev/null | grep -c "real_sim" || echo "0")
echo "  A2 executor capabilities: ${EXECUTOR_CHECK}"

echo "════════ DEPLOY COMPLETO ════════"
