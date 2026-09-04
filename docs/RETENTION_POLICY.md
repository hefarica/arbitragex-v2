# Política de Retención de Datos — ARBX-RETENTION-01 (2026-09-04)

> Estado: **ACTIVA**. Ejecutor: `scripts/pg_retention.sh` (cron VPS `17 4 * * *`,
> log `/var/log/arbx-pg-retention.log`). Doctrina de ejecución: FREEZE-01
> (`docs/incidents/2026-08-17-PIPELINE-FREEZE-PURGE-LOCKS.md`) — batched,
> `lock_timeout=5s`, `statement_timeout` acotado por batch, skip ≠ error.

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

## Recuperación de espacio (por qué no basta el DELETE)

El DELETE batched marca espacio **reutilizable dentro de PG** (los 92.5GB del
volumen no encogen solos). Para DEVOLVER espacio al SO del VPS se ejecutó una
vez, como runbook manual (no en el cron):

```sql
-- rebuild de route_discovery_outcomes (una vez, 2026-09-04, ~34GB al SO):
BEGIN;  -- con lock_timeout=5s
SET LOCAL lock_timeout='5s';
CREATE TABLE route_discovery_outcomes_new (LIKE route_discovery_outcomes INCLUDING ALL);
-- catch-up del rango vivo (2d):
INSERT INTO route_discovery_outcomes_new SELECT * FROM route_discovery_outcomes WHERE ts_ms >= <cutoff>;
DROP TABLE route_discovery_outcomes;
ALTER TABLE route_discovery_outcomes_new RENAME TO route_discovery_outcomes;
COMMIT;
-- re-crear índice CONCURRENTLY si alguno quedó marcado inválido
```

Estado esperado post-política: PG steady ≈ 55-60GB (vs 92.5GB), RDO plano en
~32GB (2d), Loki acotado a 15d, build cache purgado semanal.

## Next step estructural (cuando el crecimiento lo pida)

Particiones por día en RDO (`PARTITION BY RANGE (ts_ms)` + `drop partition`
instantáneo sin DELETE ni bloat). Requiere migración de tabla + writer; hoy el
purge batched + rebuild ocasional es suficiente.

## Crontab VPS (referencia)

```
17 4 * * * /opt/arbitragex-v2/scripts/pg_retention.sh >> /var/log/arbx-pg-retention.log 2>&1
23 5 * * 0 docker builder prune -f >> /var/log/arbx-builder-prune.log 2>&1
```
