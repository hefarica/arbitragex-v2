# Política de Retención de Datos — ARBX-RETENTION-01 (2026-09-04)

> Estado: **ACTIVA y EJECUTADA**. Ejecutor: `scripts/pg_retention.sh` (cron VPS
> `17 4 * * *`, log `/var/log/arbx-pg-retention.log`). Doctrina de ejecución:
> FREEZE-01 (`docs/incidents/2026-08-17-PIPELINE-FREEZE-PURGE-LOCKS.md`) — batched,
> `lock_timeout=5s`, `statement_timeout` acotado por batch, skip ≠ error —
> MÁS **WAL-pacing**: `CHECKPOINT` cada 20 batches (ver abajo).
>
> **Resultado de la compactación inicial (2026-09-04):** PG 92.5GB → **32GB**,
> `route_discovery_outcomes` 75GB → **15GB** (30.9M filas = 1d), disco VPS `/`
> 136GB usados (94%) → **69GB usados (48%)**. ~66GB devueltos al SO.

## Principio

Los datos de trading de una DApp viven en **capas por granularidad**, no en una
tabla eterna: el **detalle crudo** sirve horas/días (debug, trazabilidad
inmediata); el **resumen agregado** sirve siempre (estadística, calibración,
UI). La regla es *rollup-first, purge-after*: ninguna fila cruda se elimina sin
que su información agregada ya esté materializada en la capa de resumen. El
historial del operador nunca se pierde — solo cambia de granularidad.

```
detalle crudo (PG, ventana N días)
        │  rollup 5m (RDO) / rollup diario (paper, reserves)  ← PERSISTENTE
        ▼
resumen acumulado (PG, para siempre: KB-MB por día)
        │  ARBX_RETENTION_ARCHIVE=1 → COPY→zstd (opcional)
        ▼
archivo comprimido (/opt/arbitragex-v2/archives) → rsync off-VPS (operador)
        │
        ▼
purge batched del crudo (espacio reutilizable dentro de PG)
```

## Reglas por tabla (la tabla que pidió el operador)

| Tabla | Rol | Detalle crudo | Resumen que SOBREVIVE | Origen del resumen |
|---|---|---|---|---|
| `route_discovery_outcomes` | telemetría de discovery (29M filas/día, ~15GB/día) | **2 días** | `route_discovery_outcome_rollup_5m` (5 dims: totals/reason/chain/cartridge/pair) — persistente | backfill eager del script + lazy de la API (#511) |
| `opportunities` | oportunidades detectadas | **60 días** | rollup 5m RDO ya cuenta `is_opportunity`; detail >60d sin valor operativo | `pg_retention.sh` |
| `pool_reserves` | sink write-only de reservas (runtime lee Redis) | **30 días** | `pool_reserves_daily` (último snapshot por pool/día) — persistente | migración 116 seed + upsert diario |
| `risk_events` | decisiones del risk engine | **90 días** | los conteos viven en RDO summary (razones) — persistente | `pg_retention.sh` |
| `scored_opportunities` | stream de scoring | **60 días** | distribución estadística ya consumida por calibración (labels S4) | `pg_retention.sh` |
| `simulations` | resultados de simulación | **90 días** | agregados de drift viven en paper ledger + rollups paper | `pg_retention.sh` |
| `opportunity_observations` | observaciones de pricing | **60 días** | — (telemetría volátil por diseño, R8) | `pg_retention.sh` |
| `paper_trade_runs` | **LEDGER del operador** | **90 días crudo + SIEMPRE en rollup** | `paper_trade_runs_daily` (runs/profits/fails por día×chain×estrategia) — persistente; FK cambiada CASCADE→SET NULL para que el purge de `opportunities` jamás borre el ledger | migración 116 seed + upsert diario |

### NUNCA se tocan (congeladas)

- `paper_trade_runs_daily`, `pool_reserves_daily`,
  `route_discovery_outcome_rollup_5m` — resumen acumulado, viven para siempre.
- Todo esquema de configuración/identidad (cartuchos, estrategias, pools
  registry, operadores).
- Volúmenes Docker de datos (`postgres_data`, `redis_data`).
- Redis `arbx:opps:detected` — su lifecycle ya está gobernado por el pipeline
  (XLEN estable ~10K, no crece).

### Loki (logs)

`monitoring/loki/loki-config.yml`: compactor activado + `retention_period: 360h`
(15 días). Los logs >15d se eliminan solos. Antes: sin retención, crecía
infinito (6.9GB al 2026-09-04).

### Fuera de PG

- **Docker build cache**: `docker builder prune` semanal (cron domingo, ver
  abajo). Se regenera solo — es cache, no dato.
- **`backend/target` (host)**: artefactos de build host; el deploy construye en
  stage interno `/build/target` → el directorio host puede borrarse cuando el
  disco apriete (así se rescató el ENOSPC del 2026-09-04).

## Archivo (cold tier) — opcional, off por defecto

`ARBX_RETENTION_ARCHIVE=1` hace que el script, ANTES de borrar cada rango,
materialice `COPY → zstd` en `/opt/arbitragex-v2/archives/<tabla>/`. Sin
archivo exitoso, esa tabla NO se purga esa noche (fail-honest). Los `.zst`
permanecen en el VPS hasta que el operador los rsync-e fuera (no hay object
storage configurado — no se inventa uno):

```bash
# operador, desde otra máquina con disco:
rsync -avz arbx:/opt/arbitragex-v2/archives/ ./arbx-archives/
# y tras verificar integridad (zstd -t), liberar el VPS:
ssh arbx 'rm -rf /opt/arbitragex-v2/archives/<tabla>/'
```

## RCA — por qué existía el problema (evidencia)

1. **`pg_retention.sh` v1 falló todos los días desde 2026-08-18**
   (`/var/log/arbx-pg-retention.log`: "purge aborted: statement_timeout=20min
   exceeded"). Un solo `DELETE` de 13.7M filas + FK `risk_events.opportunity_id`
   ON DELETE SET NULL **sin índice** = seqscan 2.8GB por fila borrada.
2. **`route_discovery_outcomes` no tenía NINGUNA retención**: 64GB / 125.7M
   filas / 4.3 días de historia al 2026-09-04 (~29M filas/día). Es el 69% del
   volumen PG total (92.5GB).
3. **Loki sin retention**, docker build cache ~13GB regenerándose por deploy.
4. Consecuencia: `/` al 98% durante el deploy del 2026-09-04 (ENOSPC ×2).

## Ventanas: por qué esos números

- **RDO 2 días**: la UI consume `summary?hours=24` desde el rollup (#511), no
  desde el crudo. 2d da margen de re-backfill ante un incidente. El crudo >2d
  no tiene lector.
- **opportunities 60d**: es la ventana máxima que consulta la UI de feed
  (7-30d) con margen de forense.
- **pool_reserves 30d**: el runtime lee Redis; PG es solo historia. El rollup
  diario conserva la serie para siempre.
- **paper 90d**: ledger del operador — el crudo se mantiene 90d (consultable
  paginado) y el rollup diario vive eterno. FK SET NULL lo desacopla del purge
  de opportunities.
- **risk/sims 90d**: ventanas de auditoría estadística; los agregados ya viven
  en rollups/ledger.

## WAL-pacing (lección del incidente 2026-09-04 13:36Z)

Un DELETE batched "exitoso" puede llenar el disco por sí solo: cada DELETE
genera WAL y los checkpoints automáticos no reciclan a tiempo bajo ráfaga.
El primer purge (53.8M filas en 10.5min) acumuló ~16GB de WAL → disco 100% →
PG crash-loop + AOF de Redis corrupto por el mismo disk-full. Regla:

- `CHECKPOINT` cada **20 batches** (~2M filas) → pico WAL ~1.5GB.
- Cap por run: `MAX_ROWS_PER_TABLE=20M` (env `ARBX_RETENTION_MAX_ROWS`).
- Monitorear `df -h /` Y el tamaño de `pg_wal` durante purgas masivas.
- `psql -q` NUNCA en loops count-based: suprime los command tags (`DELETE n`).

Rescate (si ya ocurrió): `docker builder prune -f` libera para que PG complete
recovery solo; AOF redis con `redis-check-aof --fix` (trunca cola ilegible).

## Recuperación de espacio (por qué no basta el DELETE)

El DELETE batched marca espacio **reutilizable dentro de PG** (el volumen no
encoge solo). Para DEVOLVER espacio al SO se usaron dos runbooks manuales
(committed al repo, ejecutados una vez el 2026-09-04):

- **`scripts/rdo_emergency_compact.sh`** (el usado): guard rollup-completo →
  purge batched (cutoff 1d) con CHECKPOINT cada 20 batches →
  `VACUUM (FULL, ANALYZE)` standalone. Resultado: 75GB → 15GB, ~66GB al SO.
  Gotchas: VACUUM FULL no puede correr en bloque transaccional (un `-c`
  multi-statement crea txn implícita — usar `-c` standalone con
  `-e PGOPTIONS='-c lock_timeout=30s -c statement_timeout=1800s'`) y necesita
  libre ≥ tamaño de la tabla reescrita CON la vieja viva (~17GB para este caso).
- **`scripts/rdo_table_swap.sh`** (alternativa si hay ~45GB libres):
  `LIKE INCLUDING ALL` + catch-up por trozos + swap atómico single-txn (AEL)
  + `ALTER SEQUENCE ... OWNED BY NONE` para no perder nextval.

Estado post-política (verificado 2026-09-04): PG **32GB** (vs 92.5GB), RDO
plano en ~15GB/1-2d, Loki acotado a 15d, build cache purgado semanal.

## Next step estructural (cuando el crecimiento lo pida)

Particiones por día en RDO (`PARTITION BY RANGE (ts_ms)` + `drop partition`
instantáneo sin DELETE ni bloat). Requiere migración de tabla + writer; hoy el
purge batched + rebuild ocasional es suficiente.

## Crontab VPS (referencia)

```
17 4 * * * /opt/arbitragex-v2/scripts/pg_retention.sh >> /var/log/arbx-pg-retention.log 2>&1
23 5 * * 0 docker builder prune -f >> /var/log/arbx-builder-prune.log 2>&1
```

> Historial: existía además una entrada legacy `/etc/cron.d/arbx-retention`
> (hourly, `/usr/local/bin/arbx-pg-retention.sh`: DELETE single-statement sin
> batching ni timeouts — anti-doctrina FREEZE-01) dejada por un fix antiguo
> del "76GB bloat". **DESACTIVADA 2026-09-04** (copia
> `arbx-retention.disabled-2026-09-04`, línea comentada). El purge v3 diario
> la reemplaza: su ventana 2d ⊃ la del legacy 7d, por lo que su DELETE ya
> nunca encontraba filas — solo riesgo, cero beneficio.
