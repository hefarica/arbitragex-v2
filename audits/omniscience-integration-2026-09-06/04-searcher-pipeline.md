# N4 — searcher-pipeline (searcher-rs, trazabilidad R7 end-to-end)

- **Agente:** Verificador N4 "searcher-pipeline" (round-table integración omniscience)
- **Superficie:** searcher-rs — ¿pipeline VIVO produciendo datos reales AHORA? Cadencia vs código local.
- **Estado:** EN_CURSO
- **Hora inicio (local):** 2026-09-06 (~20:0x UTC-3 aprox — se fijará con fecha real al cerrar)
- **Branch local de trabajo:** `a6-cbprom-01` (HEAD f46a0522; origin = github.com/hefarica/arbitragex-v2.git)

## Plan de verificación (charter)

1. VPS read-only: `docker logs arbitragex-v2-searcher-rs-1 --tail 200` → señales de detección / simulator.
2. VPS read-only: `docker exec arbitragex-v2-redis-1 redis-cli XLEN arbx:opps:detected`.
3. VPS read-only: `docker exec arbitragex-v2-postgres-1 psql -U postgres -d arbitragex -c "SELECT MAX(detected_at), COUNT(*) FROM opportunities"`.
4. VPS read-only: `curl -s 127.0.0.1:8787/api/opportunities/live | head -c 800`.
5. Código local: cadencia esperada del searcher-rs (intervalos de loop, emisión a Redis, persistencia PG, lectura del edge).
6. Deltas temporales: comparar SHA desplegado en VPS (`git rev-parse HEAD` en /opt/arbitragex-v2) vs `origin/main` vs HEAD local.
7. Veredicto: VIVO / DEGRADADO / ROTO por capa, con evidencia cruda.

## Reglas respetadas

- Todo read-only (docker logs/ps/inspect, redis-cli read-only, psql SELECT, curl interno VPS).
- Escritura únicamente en este archivo de reporte.
- Reporte en español, evidencia con comando + output real (RULE 00 / fail-honest R8).
