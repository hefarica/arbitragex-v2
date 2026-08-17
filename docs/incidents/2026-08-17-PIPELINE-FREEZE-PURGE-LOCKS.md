# 🔴 INCIDENTE + DIRECTIVA DE COORDINACIÓN — LECTURA OBLIGATORIA PARA TODA SESIÓN/AGENTE

> **Fecha**: 2026-08-17 (restauración 01:46:10Z) · **Severidad**: P1 (pipeline de detección muerto 21h)
> **Estado**: FLUJO RESTAURADO Y VERIFICADO · sostén 1h en observación hasta ~02:46Z
> **Si eres otra sesión/agente trabajando este repo**: lee TODO antes de tocar PG, `.env`, contenedores o readiness. Lo que sigue integra y protege el trabajo de HOY (PRs #343-#358).

## 1. Qué pasó (RCA con evidencia — no opinable)

**`opportunities` quedó congelada 21 horas** (última fila pre-incidente 2026-08-16 04:41:28Z → primera fila post-fix 2026-08-17 01:46:08Z). G-PAP-1 lo reportaba ("last 20.3h ago") — la compuerta existía; nadie la miraba (eso lo cura H2).

**Mecanismo exacto** (leído de `pg_stat_activity` en vivo):
- Un purge de retención (sesiones `psql`, config `jit=off; enable_indexscan=off…`, iniciadas ~03:07-03:24Z) retuvo locks sobre `opportunities` **sin `lock_timeout`**.
- Detrás de él se encolaron: el propio `DELETE` (Lock:transactionid, 77,273s), un `CREATE INDEX idx_opp_status_time` (Lock:relation, 75,644s) y **~44 `INSERT INTO opportunities`** del pipeline (Lock:relation, ~20.9h c/u).
- Al momento del diagnóstico: **active=68, idle=17** conexiones. Los timeouts de pool del searcher (`pool timed out while waiting for an open connection`) eran **síntoma**, no causa.

## 2. Qué se hizo (restaurar primero — §37)

1. **Terminación quirúrgica de 19 backends `psql` atascados** (transacciones de mantenimiento ruedan back; son re-ejecutables). El queue de locks desapareció (verificado: 0 esperas por Lock).
2. **Pools del `.env` del VPS corregidos según R8-05** (nunca aplicados): `DATABASE_POOL_MAX=12` (searcher-rs/shared-rs — `db_pool.rs:40`) y `PG_POOL_MAX` 35→**20** (api-server — `index.ts:320`). Mapeo verificado en código. ⚠️ **api-server aún corre con su pool viejo de 35** — tomará el 20 en su próximo restart programado.
3. Restart de SOLO searcher-rs (boot sano: `db.connected` intento 1, RPC pool 6 providers, sin hang del orchestrator).
4. **PR #358**: timeout de la sonda PR-1 3000→8000ms basado en 10 HEAD medidos (frío 2611ms SSR / p50 caliente 23ms) — el AbortError era el SSR frío post-redeploy, no la red.
5. **Verificación cruzada de la restauración**: PG (`MAX(detected_at)`=46s) + monitor independiente (`FLOW_RESTORED ts=01:46:29Z age=20s`).

## 3. Qué está en vuelo (programa ONDA — prompt maestro del operador 2026-08-16)

- **ONDA 1** (compuerta dura): sostén 1h de frescura <5min en observación → luego FASE 3 (gas no-nulo en filas paper nuevas, R8-03) y L4-D01 (telemetría `dispatch_deferred`/adapter — ojo: `edges_rejected=109/109` debe deshielarse ahora que fluyen pools).
- **ONDA 2** (hardening, se abre con VERDE-ONDA-1): H1 orchestrator fail-operational (R9-06) · H2 alerta `PIPELINE_SILENCE` (>10min sin filas — hoy existiera habría gritado a las 04:50Z) · H3 pgbouncer + **mapa único de pools** (buscado el "fix invertido") · H4 ya casi cerrado (#358) · H5 trío CI (fast-uri/nanoid/RUSTSEC) · H6 evidencias G-SIM-1 vía `/admin/readiness-evidence`.
- **ONDA 3** (matriz de adapters): A-TRI→A-AMM→A-ORA→A-LEND, A-MEM condicional. Externas en sombra; mev_06 nunca sale.

## 4. 🔒 NO HAGAS (protección del trabajo en curso)

1. **NO re-ejecutes el purge de PG** sin: `SET lock_timeout='5s'`, `statement_timeout` acotado, fuera de hora pico, y anuncio en este archivo. El purge de ayer causó las 21h.
2. **NO reinicies searcher-rs ni api-server** hasta que el sostén 1h cierre (~02:46Z) — el restart de searcher forma parte de la evidencia en curso. El restart pendiente de api-server (para tomar `PG_POOL_MAX=20`) lo ejecuta el dueño del programa ONDA.
3. **NO toques las vars de pool del `.env`** (`DATABASE_POOL_MAX=12`, `PG_POOL_MAX=20` recién aplicadas) ni propongas "arreglar" el pool contrario (el fix invertido ya se propuso una vez — el mapa está en §2).
4. **NO toques**: readiness verifiers (PR #358 en vuelo), maquinaria G-SIM-1 (#348/#349/#352/#355 — capabilities/registry/verifier v2/productores), tablas `readiness_evidence*` (append-only).
5. **NO asumas flake** en checks CI rojos: el trío {npm audit, cargo audit, TS-integration} es el conocido; cualquier OTRO rojo se investiga leyendo el log (ONDA 2 H5 lo adjudica).
6. **Disciplina §36**: verifica `git branch --show-current` antes de commitear; main se mueve rápido (10 PRs hoy) — `update-branch` antes de merge.

## 5. Cómo se integra con lo de hoy

Todo lo de hoy es coherente y está deployado con SHA verificado (REGLA 0h): #343 stablecoins (+retractación USDT), #344 HSTS/SEC-3, #345 V-AT-1 (L4 15/1/1), #346 multichain, #347 CI-GATE-RELIABILITY, #348-#352 G-SIM-1 F1-F3 (el tablero habla topología), #353/#354 MC-CRED/MC-RPC, #355 productores de evidencia (primera corrida automática verde). El freeze fue **el único** estado rojo del día y ya está curado con RCA documentada. La fuente de verdad del feed es `MAX(opportunities.detected_at)` — nunca telemetría de decode.

## 6. 🔴 RECURRENCIA FREEZE-02 (mismo día, 04:17Z) — generador eliminado

**Qué pasó**: el crontab del VPS (`17 4 * * * pg_retention.sh`) ejecutó el purge a las **04:17:01Z** — el script NO implementaba la disciplina del §4.1 (era el single-shot `synchronous_commit=off + DELETE directo` de PGBLOAT-02). Con ~8.6M filas/día y cascadas FK (`risk_events.opportunity_id` SET NULL), el DELETE retuvo RowExclusiveLock **41 min**. Detrás se encoló un `CREATE INDEX IF NOT EXISTS idx_opp_status_time` (04:32:31Z, psql manual — el índice YA existía: `IF NOT EXISTS` igual pide ShareLock para verificar) y tras él 12 INSERT del pipeline. **Feed congelado 26.6 min** (04:32:05Z → 04:58:42Z).

**Restauración**: terminación quirúrgica de pid 1679 (purge) + pid 3588 (CREATE INDEX) a las 04:58:4xZ; cola de locks a 0 en <3s; primera fila nueva 04:58:42Z (verificado con monitor until-loop sobre `MAX(detected_at)`). Rollback del DELETE = barato (abort de deletes = flip de clog, sin undo físico).

**Fix del generador (este PR)**: `scripts/pg_retention.sh` reescrito con la disciplina §4.1 — `lock_timeout=5s` + `statement_timeout=60s` por batch, batches de 10k filas con commit separado (hold de locks de segundos; INSERT/DDL intercalan entre batches), presupuesto de run `RUN_BUDGET_S=480s` (el backlog restante drena en corridas siguientes), y pausa 0.3s entre batches. Cron y horario quedan iguales (04:17 = fuera de pico).

**Regla permanente adicional (§4.1-bis)**: cualquier DDL de mantenimiento sobre `opportunities` usa `CREATE INDEX CONCURRENTLY` y NUNCA `CREATE INDEX [IF NOT EXISTS]` plano — `IF NOT EXISTS` igual encola ShareLock aunque el índice exista, y la cola justa de PG hambrea los INSERT detrás del DDL en espera. Si el índice ya existe, verifica contra `pg_class` (como hace este script) en lugar de lanzar el DDL.
