# PLAN DE CAMBIO — merge #577/#578/#579 + deploy veras (2026-09-17)
> Autorización del operador: "Con merge + deploy veras" (2026-09-17, tras PUSH autorizado).
> Ejecutar SOLO con CI de c38203fd verde (monitor bq3fa8oyu vigila).

## Causa
G2 imposible: 7.755 sims/24h, 0 passed. 55% de fallos = STF/TRANSFER_FROM_FAILED
(signer sin fondos en fork) → SIM-FUND-01 (#578). 4.6% = router Pancake ausente de
catálogo → PANCAKE-ROUTER-01 (#579). #577 cierra la publicación de rechazadas al stream
validado (higiene del selector).

## Secuencia

### Fase M — merges (API, en orden, update-branch si 405 merge-cascade)
1. `PUT /pulls/577/merge` (merge_method=merge, head fdb40401, state clean)
2. `PUT /pulls/578/merge` — si 405 "required status checks": `POST /pulls/578/update-branch`, ronda 2
3. `PUT /pulls/579/merge` (head c38203fd) — ídem update-branch
4. Verificar: `git ls-remote origin main` == SHA esperado

### Fase D — deploy veras (VPS, ssh arbx)
Pre:
- Confirmar lock `/tmp/arbx-deploy.lock` stale → remover (verificado stale 05:49, sin proceso)
- `df -h /` (hoy 41G libres, OK > 30 floor) + build cache 21GB reclai­mable si aprieta
1. `cd /opt/arbitragex-v2 && git fetch origin && git reset --hard origin/main` (verificar SHA)
2. Migraciones: `database/run_migrations.sh` (canónico, idempotente — SI el diff toca SQL; los 3 PRs no tocan esquema → verificar diff primero, si no hay .sql, saltar con evidencia)
3. Build de servicios cambiados (shared-rs/searcher-rs/sim-ctl — cadena Rust):
   `docker compose --env-file .env -f docker/compose.dev.yml build searcher-rs sim-ctl` (+dependientes del catálogo: selector-api NO toca Rust)
4. `docker compose --env-file .env -f docker/compose.dev.yml up -d searcher-rs sim-ctl`
5. L4 deploy-veras: `docker exec searcher git rev-parse HEAD` == main SHA; smoke
   `curl localhost:8787/api/opportunities/live | head`; invariante R7:
   `XLEN arbx:opps:detected` creciendo; `docker ps` 24/24 healthy

### Fase V — verificación del objetivo (por qué hicimos todo esto)
- SIM-FUND-01: logs sim-ctl sin TRANSFER_FROM_FAILED/STF nuevos
- Métrica G2: `SELECT COUNT(*) FROM simulations WHERE passed` > 0 (la PRIMERA de la historia)
- Pancake: 0 `router not in catalog` nuevos en 1h
- Si 0 passed persiste a los 30 min: siguiente eslabón (zero_amount_in 18%) — NO re-deployar a ciegas

## Rollback
- Git: revert de merges + redeploy (emergencia §37: restaurar primero)
- Servicios: imagen previa sigue en el host (23 imágenes) → `up -d` con SHA anterior
- Datos: los 3 PRs no mutan esquema; PG sin rollback necesario

## Presupuesto e interrupción
- Build Rust 2 servicios: ~10-20 min, pico de disco ~10-20GB (margen 41G — vigilar df cada build)
- Criterio de abort: disco <15G, build error, o L4 mismatch de SHA
