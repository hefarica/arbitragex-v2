#!/usr/bin/env bash
# ============================================================================
# ARBX-RETENTION-01 — pg_retention.sh v2 (2026-09-04)
#
# v1 (PGBLOAT-02) purgaba SOLO opportunities con UN DELETE no-batcheado de
# millones de filas y statement_timeout=20min: falló TODOS los días desde
# 2026-08-18 (/var/log/arbx-pg-retention.log), mientras route_discovery_outcomes
# crecía ~15GB/día sin ninguna retención. FREEZE-01
# (docs/incidents/2026-08-17-PIPELINE-FREEZE-PURGE-LOCKS.md) define la doctrina:
#   - batched DELETE con LIMIT, lock_timeout=5s, statement_timeout corto por batch
#   - lock busy → SKIP la tabla (skip != error), nunca encolarse tras locks
#   - R9: un summary por tabla, no un log por fila
# v2 añade: multi-tabla con ventanas por antigüedad, backfill EAGER del rollup
# 5m de RDO ANTES de purgar crudo (la API solo hace backfill lazy ≤24 buckets),
# rollups diarios paper/reserves (upsert idempotente de 4 días), VACUUM ANALYZE
# post-purge y archivo zstd opcional. Detalle de ventanas: docs/RETENTION_POLICY.md.
#
# Modo: bash scripts/pg_retention.sh [--dry-run]
# Cron VPS (root): 17 4 * * * /opt/arbitragex-v2/scripts/pg_retention.sh >> /var/log/arbx-pg-retention.log 2>&1
# ============================================================================
set -uo pipefail

DRY_RUN=0
[ "${1:-}" = "--dry-run" ] && DRY_RUN=1

PG_CONTAINER="${PG_CONTAINER:-arbitragex-v2-postgres-1}"
ARCHIVES_DIR="${ARBX_RETENTION_ARCHIVE_DIR:-/opt/arbitragex-v2/archives}"
DO_ARCHIVE="${ARBX_RETENTION_ARCHIVE:-0}"   # 1 = COPY→zstd antes de borrar (ver RETENTION_POLICY.md)
# DAPP-ARCHIVE-UI-01: el toggle del operador (UI /operations → /api/admin/archive/auto)
# persiste en retention_settings.archive_auto y PREVALECE sobre el default 0 de
# arriba. El env ARBX_RETENTION_ARCHIVE=1 sigue siendo un override manual válido.
AUTO_DB="$(docker exec "$PG_CONTAINER" psql -U postgres -d arbitragex -At -c \
  "SELECT COALESCE(value->>'enabled','') FROM retention_settings WHERE key='archive_auto'" 2>/dev/null | tr -d '[:space:]' || true)"
if [ "$DO_ARCHIVE" != "1" ] && [ "$AUTO_DB" = "true" ]; then
  DO_ARCHIVE=1
fi
BACKFILL_BUDGET_S="${ARBX_RDO_BACKFILL_BUDGET_S:-900}"
TABLE_BUDGET_S="${ARBX_TABLE_BUDGET_S:-1200}"
# 2026-09-04 incidente (13:36Z): la primera corrida purgó 53.8M filas RDO a
# ~85K/s y el WAL acumulado (~16GB antes de que los checkpoints automáticos
# reciclaran) llenó / al 100% → postgres crash-loop "postmaster.pid: No space
# left on device". Dos mitigaciones:
#   CHECKPOINT_EVERY: cada N batches se fuerza CHECKPOINT → el WAL se recicla
#     al ritmo del purge y el pico queda acotado a ~1GB.
#   MAX_ROWS_PER_TABLE: tope de filas por corrida/tabla — un backlog gigante
#     se dosifica en días en vez de una sola ráfaga.
CHECKPOINT_EVERY="${ARBX_RETENTION_CHECKPOINT_EVERY:-20}"
MAX_ROWS_PER_TABLE="${ARBX_RETENTION_MAX_ROWS:-20000000}"
BATCH_LOCK_TIMEOUT="5s"                     # FREEZE-01: nunca esperar más tras un lock
BATCH_STMT_TIMEOUT="300s"                   # un batch acotado, no un mega-DELETE
VACUUM="${ARBX_RETENTION_VACUUM:-1}"

# tabla|columna_tiempo|formato(ts|ms)|ventana_días|batch_size|hook
# hooks: rdo (backfill eager antes del purge) · paper|reserves (upsert rollup diario)
TABLES=(
  "route_discovery_outcomes|ts_ms|ms|1|100000|rdo"
  "opportunities|detected_at|ts|60|20000|"
  "pool_reserves|timestamp|ts|30|25000|reserves"
  "risk_events|created_at|ts|90|25000|"
  "scored_opportunities|created_at|ts|60|20000|"
  "simulations|simulated_at|ts|90|20000|"
  "opportunity_observations|observed_at|ts|60|20000|"
  "paper_trade_runs|created_at|ts|90|20000|paper"
)

log()  { printf '%s %s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$*"; }

psql_q() {  # psql_q <sql> → stdout; rc != 0 en error
  docker exec -i "$PG_CONTAINER" psql -U postgres -d arbitragex -X \
    -v ON_ERROR_STOP=1 -qAt -c "$1"
}

psql_batch() {  # ejecuta un batch con timeouts acotados; stderr visible para el caller
  # SIN -q: quiet suprime los command tags ("DELETE n" / "INSERT 0 n") y el
  # caller los usa para contar filas — con -q la primera corrida real reportó
  # deleted=0 tras borrar un batch entero sin contarlo (fail-honest violado).
  docker exec -i "$PG_CONTAINER" psql -U postgres -d arbitragex -X \
    -v ON_ERROR_STOP=1 -At \
    -c "SET lock_timeout='$BATCH_LOCK_TIMEOUT'; SET statement_timeout='$BATCH_STMT_TIMEOUT'; $1" 2>&1
}

# ----------------------------------------------------------------------------
# 0. Checks globales (fail-honest: sin PG no se inventa nada, se aborta)
# ----------------------------------------------------------------------------
if ! docker inspect "$PG_CONTAINER" >/dev/null 2>&1; then
  log "FATAL: container $PG_CONTAINER not running — nothing purged"
  exit 1
fi
if ! psql_q 'SELECT 1' >/dev/null 2>&1; then
  log "FATAL: psql cannot connect — nothing purged"
  exit 1
fi

# DDL activo (migración en vuelo) → mejor otro día completo (FREEZE-01).
# pid <> pg_backend_pid() es OBLIGATORIO: esta query contiene las palabras del
# patrón en su propio texto y sin la exclusión se auto-detectaría a sí misma
# cada noche (SKIP silencioso perpetuo — el fallo que v1 tuvo 17 días).
if psql_q "SELECT count(*) FROM pg_stat_activity WHERE pid <> pg_backend_pid() AND state = 'active' AND query ~* 'ALTER TABLE|CREATE INDEX|DROP TABLE'" | grep -qv '^0$'; then
  log "SKIP-RUN: DDL activity detected on postgres — retention deferred (fail-operational)"
  exit 0
fi

if [ "$DRY_RUN" = "1" ]; then
  log "retention.dry-run mode — counts only, no DELETE"
fi

# ----------------------------------------------------------------------------
# 1. Backfill EAGER del rollup 5m de RDO (antes de purgar el crudo).
#    La agregación es la MISMA de ENSURE_ROLLUP_SQL
#    (backend/api-server/src/routes/route-discovery-outcomes-api.ts) para que
#    eager y lazy produzcan filas idénticas (ON CONFLICT DO NOTHING igualmente).
# ----------------------------------------------------------------------------
rdo_backfill_chunk() {
  psql_batch "
    WITH oldest AS (
      SELECT min(ts_ms) / 300000::bigint * 300000 AS b
      FROM route_discovery_outcomes
    ),
    last_complete AS (
      SELECT floor(extract(epoch FROM now()) * 1000)::bigint
             / 300000 * 300000 - 300000 AS b
    ),
    todo AS (
      SELECT g.bucket
      FROM generate_series(
             COALESCE((SELECT b FROM oldest), (SELECT b FROM last_complete)),
             (SELECT b FROM last_complete),
             300000) AS g(bucket)
      WHERE NOT EXISTS (
        SELECT 1 FROM route_discovery_outcome_rollup_5m rr
        WHERE rr.dim = '__totals__' AND rr.bucket_ms = g.bucket
      )
      ORDER BY 1
      LIMIT 48
    )
    INSERT INTO route_discovery_outcome_rollup_5m (dim, key, bucket_ms, n, opportunities, with_reserves, profit_gt0)
    SELECT agg.dim, agg.key, t.bucket, agg.n, agg.opportunities, agg.with_reserves, agg.profit_gt0
    FROM todo t
    CROSS JOIN LATERAL (
      SELECT '__totals__' AS dim, '' AS key,
             count(*)::bigint AS n,
             count(*) FILTER (WHERE r.is_opportunity)::bigint AS opportunities,
             count(*) FILTER (WHERE r.had_reserves)::bigint AS with_reserves,
             count(*) FILTER (WHERE r.estimated_profit > 0)::bigint AS profit_gt0
      FROM route_discovery_outcomes r
      WHERE r.ts_ms >= t.bucket AND r.ts_ms < t.bucket + 300000
      UNION ALL
      SELECT 'reason', COALESCE(NULLIF(r.reason, ''), '(null)'),
             count(*)::bigint, count(*) FILTER (WHERE r.is_opportunity)::bigint, 0::bigint, 0::bigint
      FROM route_discovery_outcomes r
      WHERE r.ts_ms >= t.bucket AND r.ts_ms < t.bucket + 300000
      GROUP BY 2
      UNION ALL
      SELECT 'chain', r.chain_id::text,
             count(*)::bigint, count(*) FILTER (WHERE r.is_opportunity)::bigint, 0::bigint, 0::bigint
      FROM route_discovery_outcomes r
      WHERE r.ts_ms >= t.bucket AND r.ts_ms < t.bucket + 300000
      GROUP BY 2
      UNION ALL
      SELECT 'cartridge', COALESCE(NULLIF(r.cartridge_id, ''), '(null)'),
             count(*)::bigint, count(*) FILTER (WHERE r.is_opportunity)::bigint, 0::bigint, 0::bigint
      FROM route_discovery_outcomes r
      WHERE r.ts_ms >= t.bucket AND r.ts_ms < t.bucket + 300000
      GROUP BY 2
      UNION ALL
      SELECT 'pair', COALESCE(r.token_in, '') || '|' || COALESCE(r.token_out, ''),
             count(*)::bigint, count(*) FILTER (WHERE r.is_opportunity)::bigint, 0::bigint, 0::bigint
      FROM route_discovery_outcomes r
      WHERE r.ts_ms >= t.bucket AND r.ts_ms < t.bucket + 300000
      GROUP BY 2
    ) agg
    ON CONFLICT DO NOTHING" | tail -n 1
}

# ROLLUP-GAP-01 (2026-09-22): zero-fill honesto de buckets __totals__.
# Días cuyas horas iniciales no tienen outcomes crudos jamás reciben fila
# __totals__ (la agregación lateral del backfill solo corre donde hubo crudo
# que contar tras el oldest mutable), y el coverage gate del purge salta el
# día entero por esos huecos. Materializamos la fila cero SOLO donde NO
# existe crudo en el bucket (RULE 00/R8: un bucket con crudo pendiente de
# rollup NUNCA se rellena con cero — eso escondería datos reales).
rdo_zero_fill() {
  psql_batch "
    WITH oldest AS (
      SELECT min(ts_ms) / 300000::bigint * 300000 AS b
      FROM route_discovery_outcomes
    ),
    last_complete AS (
      SELECT floor(extract(epoch FROM now()) * 1000)::bigint
             / 300000 * 300000 - 300000 AS b
    ),
    todo AS (
      SELECT g.bucket
      FROM generate_series(
             COALESCE((SELECT b FROM oldest), (SELECT b FROM last_complete)),
             (SELECT b FROM last_complete),
             300000) AS g(bucket)
      WHERE NOT EXISTS (
        SELECT 1 FROM route_discovery_outcome_rollup_5m rr
        WHERE rr.dim = '__totals__' AND rr.bucket_ms = g.bucket
      )
      AND NOT EXISTS (
        SELECT 1 FROM route_discovery_outcomes r
        WHERE r.ts_ms >= g.bucket AND r.ts_ms < g.bucket + 300000
      )
    )
    INSERT INTO route_discovery_outcome_rollup_5m (dim, key, bucket_ms, n, opportunities, with_reserves, profit_gt0)
    SELECT '__totals__', '', bucket, 0::bigint, 0::bigint, 0::bigint, 0::bigint
    FROM todo
    ON CONFLICT DO NOTHING" | tail -n 1
}

if [ "$DRY_RUN" = "0" ]; then
  bf_start=$SECONDS
  bf_rows=1
  while [ "$bf_rows" -gt 0 ] && [ $((SECONDS - bf_start)) -lt "$BACKFILL_BUDGET_S" ]; do
    out=$(rdo_backfill_chunk) || { log "rdo.backfill ERROR: $out"; break; }
    bf_rows=$(printf '%s' "$out" | grep -oE 'INSERT 0 [0-9]+' | grep -oE '[0-9]+$') || bf_rows=0
    [ "$bf_rows" -gt 0 ] && log "rdo.backfill chunk inserted rows=$bf_rows elapsed=$((SECONDS - bf_start))s"
  done
  log "rdo.backfill done missing_remaining=${bf_rows} elapsed=$((SECONDS - bf_start))s budget=${BACKFILL_BUDGET_S}s"
  zf_out=$(rdo_zero_fill) || zf_out="err"
  zf_rows=$(printf '%s' "$zf_out" | grep -oE 'INSERT 0 [0-9]+' | grep -oE '[0-9]+$') || zf_rows=0
  log "rdo.zero_fill inserted_rows=$zf_rows out=$zf_out"
fi

# ----------------------------------------------------------------------------
# 2. Rollups diarios — upsert idempotente de los últimos 4 días
#    (re-agrega días que aún reciben filas; el seed one-time vivió en la
#    migración 116, esto es solo mantenimiento incremental).
# ----------------------------------------------------------------------------
if [ "$DRY_RUN" = "0" ]; then
  if ! psql_batch "
    INSERT INTO paper_trade_runs_daily (
        day, chain_id, strategy_kind, runs, runs_with_actual,
        sim_profit_sum, sim_profit_n, actual_profit_sum, actual_profit_n,
        actual_profit_gt0, sim_fails)
    SELECT
        (created_at AT TIME ZONE 'UTC')::date,
        chain_id,
        COALESCE(NULLIF(strategy_kind, ''), '(unset)'),
        count(*),
        count(*) FILTER (WHERE actual_timestamp IS NOT NULL),
        COALESCE(sum(sim_expected_profit_usd), 0),
        count(sim_expected_profit_usd),
        COALESCE(sum(actual_profit_usd), 0),
        count(actual_profit_usd),
        count(*) FILTER (WHERE actual_profit_usd > 0),
        count(*) FILTER (WHERE sim_fail_family IS NOT NULL)
    FROM paper_trade_runs
    WHERE created_at >= now() - interval '4 days'
    GROUP BY 1, 2, 3
    ON CONFLICT (day, chain_id, strategy_kind) DO UPDATE SET
        runs = EXCLUDED.runs, runs_with_actual = EXCLUDED.runs_with_actual,
        sim_profit_sum = EXCLUDED.sim_profit_sum, sim_profit_n = EXCLUDED.sim_profit_n,
        actual_profit_sum = EXCLUDED.actual_profit_sum, actual_profit_n = EXCLUDED.actual_profit_n,
        actual_profit_gt0 = EXCLUDED.actual_profit_gt0, sim_fails = EXCLUDED.sim_fails" >/dev/null; then
    log "rollup.paper_daily ERROR (skipped — purge continues, seed ya materializó el histórico)"
  fi
  if ! psql_batch "
    INSERT INTO pool_reserves_daily (day, pool_id, last_block, reserve0, reserve1, snapshot_at)
    SELECT DISTINCT ON (pool_id, (\"timestamp\" AT TIME ZONE 'UTC')::date)
        (\"timestamp\" AT TIME ZONE 'UTC')::date,
        pool_id, block_number, reserve0, reserve1, \"timestamp\"
    FROM pool_reserves
    WHERE \"timestamp\" >= now() - interval '4 days'
    ORDER BY pool_id, (\"timestamp\" AT TIME ZONE 'UTC')::date, \"timestamp\" DESC
    ON CONFLICT (day, pool_id) DO UPDATE SET
        last_block = EXCLUDED.last_block, reserve0 = EXCLUDED.reserve0,
        reserve1 = EXCLUDED.reserve1, snapshot_at = EXCLUDED.snapshot_at" >/dev/null; then
    log "rollup.reserves_daily ERROR (skipped — purge continues, seed ya materializó el histórico)"
  fi
fi

# ----------------------------------------------------------------------------
# 2b. RDO particionado (migración 122): CREATE futuras + DROP PARTITION en vez
#     de DELETE batcheado. DROP de partición = unlink de metadatos: el espacio
#     vuelve al SO al instante y sin la ráfaga de WAL del incidente
#     2026-09-04 13:36Z. Si la tabla NO está particionada (VPS pre-122, CI),
#     este bloque entero es no-op y el flujo v2 (§3) sigue siendo el dueño.
# ----------------------------------------------------------------------------
RDO_PARTITIONED=0
if psql_q "SELECT count(*) FROM pg_partitioned_table pt JOIN pg_class c ON c.oid = pt.partrelid WHERE c.relname = 'route_discovery_outcomes'" | grep -qv '^0$'; then
  RDO_PARTITIONED=1
fi

if [ "$RDO_PARTITIONED" = "1" ]; then
  # Ventana cruda idéntica a la entrada RDO de TABLES (§3): 1 día.
  RDO_WINDOW_DAYS=1
  rdo_parts_dropped=0
  rdo_parts_archived=0
  rdo_parts_skipped=0

  # 2b.1 — crear las particiones de mañana y +2 (mismo formato y bounds
  # UTC-día exactos que la migración 122). Sin esto, el primer INSERT tras
  # medianoche falla con "no partition of relation" y el sink acumula
  # persist_err (at-least-once lo recupera, pero mejor no tocar el borde).
  if ! psql_batch "
    DO \$\$
    DECLARE
      today_lo bigint;
      p_lo     bigint;
      hi_ms    bigint;
      daytag   text;
    BEGIN
      today_lo := (extract(epoch FROM clock_timestamp())::bigint * 1000) / 86400000 * 86400000;
      hi_ms    := (today_lo / 86400000 + 2) * 86400000;
      p_lo     := today_lo + 86400000;
      WHILE p_lo <= hi_ms LOOP
        daytag := to_char(to_timestamp(p_lo / 1000.0) AT TIME ZONE 'UTC', 'YYYYMMDD');
        IF to_regclass(format('route_discovery_outcomes_p%s', daytag)) IS NULL THEN
          EXECUTE format('CREATE TABLE route_discovery_outcomes_p%s PARTITION OF route_discovery_outcomes FOR VALUES FROM (%s) TO (%s)', daytag, p_lo, p_lo + 86400000);
        END IF;
        p_lo := p_lo + 86400000;
      END LOOP;
    END \$\$" >/dev/null; then
    log "rdo.partitions ERROR creating future partitions (next run retries — see migration 122 runbook)"
  fi

  # 2b.2 — DROP de particiones ÍNTEGRAMENTE más viejas que la ventana. La del
  # borde (parcialmente dentro de la ventana) nunca se toca, así que en
  # producción conviven ~2 particiones — unos GB más que la ventana exacta de
  # filas del v2, a cambio de cero DELETE.
  cutoff_ms=$(psql_q "SELECT (extract(epoch FROM now() - interval '$RDO_WINDOW_DAYS days') * 1000)::bigint")
  parts=$(psql_q "SELECT c.relname FROM pg_inherits i JOIN pg_class c ON c.oid = i.inhrelid WHERE i.inhparent = 'route_discovery_outcomes'::regclass AND c.relname ~ '^route_discovery_outcomes_p[0-9]{8}\$' ORDER BY 1")
  for part in $parts; do
    daytag=${part##*_p}
    day_start_ms=$(( $(date -u -d "${daytag:0:4}-${daytag:4:2}-${daytag:6:2}T00:00:00Z" +%s) * 1000 ))
    day_end_ms=$(( day_start_ms + 86400000 ))
    [ "$day_end_ms" -le "$cutoff_ms" ] || continue

    if [ "$DRY_RUN" = "1" ]; then
      summary+=("rdo_partition:$daytag:dryrun_drop")
      log "retention.dry-run partition=$part would_drop"
      continue
    fi

    # Guard de rollup (misma política que §3 rdo): jamás borrar un día que el
    # rollup 5m aún no materializó — el backfill EAGER de §1 corre ANTES de
    # este bloque. Partición vacía (día sin datos) no necesita rollup.
    nrows=$(psql_q "SELECT count(*) FROM $part")
    if [ "$nrows" != "0" ]; then
      missing=$(psql_q "
        SELECT count(*) FROM generate_series($day_start_ms, $day_end_ms - 300000, 300000) g(bucket)
        WHERE NOT EXISTS (SELECT 1 FROM route_discovery_outcome_rollup_5m rr
                          WHERE rr.dim = '__totals__' AND rr.bucket_ms = g.bucket)") || missing="err"
      if [ "$missing" != "0" ]; then
        rdo_parts_skipped=$((rdo_parts_skipped + 1))
        log "retention.skip partition=$part reason=rollup_backfill_pending buckets_missing=$missing"
        continue
      fi
    fi

    # Archivo opt-in: snapshot zstd de la partición ANTES de borrarla.
    if [ "$DO_ARCHIVE" = "1" ]; then
      arch="$ARCHIVES_DIR/route_discovery_outcomes/$part.tsv.zst"
      if [ -e "$arch" ]; then
        log "retention.archive partition=$part reuse=$arch"
      else
        mkdir -p "$ARCHIVES_DIR/route_discovery_outcomes"
        if docker exec -i "$PG_CONTAINER" psql -U postgres -d arbitragex -X -qAt \
            -c "COPY (SELECT * FROM $part) TO STDOUT" \
            | zstd -q -T0 -o "$arch" 2>/dev/null; then
          rdo_parts_archived=$((rdo_parts_archived + 1))
          log "retention.archive partition=$part file=$arch bytes=$(stat -c%s "$arch" 2>/dev/null || echo '?')"
        else
          rm -f "$arch"
          log "retention.archive partition=$part FAILED — drop SKIPPED (fail-honest: sin archivo no se borra)"
          rdo_parts_skipped=$((rdo_parts_skipped + 1))
          continue
        fi
      fi
    fi

    if out=$(psql_batch "DROP TABLE $part"); then
      rdo_parts_dropped=$((rdo_parts_dropped + 1))
      log "retention.partition drop=$part rows=$nrows"
    else
      rdo_parts_skipped=$((rdo_parts_skipped + 1))
      log "retention.partition drop=$part FAILED: $(printf '%s' "$out" | head -c 200 | tr '\n' ' ')"
    fi
  done
  summary+=("route_discovery_outcomes:dropped_partitions=${rdo_parts_dropped}:archived=${rdo_parts_archived}:skipped=${rdo_parts_skipped}")
fi

# ----------------------------------------------------------------------------
# 3. Purge por tabla — batched, guard por índice, skip != error
# ----------------------------------------------------------------------------
summary=()
total_deleted=0
ts_now_start=$SECONDS

for spec in "${TABLES[@]}"; do
  IFS='|' read -r tbl col fmt days batch hook <<<"$spec"
  t0=$SECONDS
  reason=""

  # Migración 122: RDO particionado se mantiene por DROP PARTITION (§2b) —
  # el DELETE batcheado de §3 ya no aplica a esa tabla.
  if [ "$tbl" = "route_discovery_outcomes" ] && [ "$RDO_PARTITIONED" = "1" ]; then
    continue
  fi

  # Guard: índice con la columna de corte en posición leading — acepta
  # "(col)", "(col, ...)" y "(col DESC)". replace(...) quita las comillas de
  # pg_indexes para columnas tipo "timestamp".
  # rc!=0 = la query NO corrió (BD en recovery/caída) → db_error, NO un falso
  # "missing_index" (el summary del incidente 13:36Z diagnosticó mal 6 tablas).
  idxout=$(psql_q "SELECT 1 FROM pg_indexes WHERE tablename='$tbl' AND replace(indexdef, chr(34), '') ~ '\\(${col}[ ,)]' LIMIT 1")
  if [ $? -ne 0 ]; then
    summary+=("$tbl:SKIP_db_unreachable")
    log "retention.skip table=$tbl reason=db_unreachable (psql failed — check postgres)"
    continue
  fi
  if [ -z "$idxout" ]; then
    summary+=("$tbl:SKIP_missing_index")
    log "retention.skip table=$tbl reason=missing_index_on_$col (no seqscan purge — ver migración 116)"
    continue
  fi

  if [ "$fmt" = "ms" ]; then
    cutoff="(extract(epoch FROM now() - interval '$days days') * 1000)::bigint"
  else
    cutoff="now() - interval '$days days'"
  fi

  # RETENTION-FK-01 (2026-09-05): opportunities es padre de 3 FKs ON DELETE
  # SET NULL sobre columnas NOT NULL (paper_trade_runs, risk_events,
  # opportunity_observations) — borrar un padre con hijos vivos aborta la
  # transacción y hace rollback del lote completo (evidencia: cron 04:17 del
  # 09-05 con opportunities:deleted=3900000:error SET NULL y MIN(detected_at)
  # congelado en 2026-07-04). Guard: solo víctimas SIN hijos; las referenciadas
  # esperan la ventana (90d) del propio hijo. Los índices de soporte ya existen
  # (idx_paper_trade_runs_opportunity, idx_risk_events_opportunity_id,
  # idx_opportunity_observations_opportunity_id) → NOT EXISTS indexado.
  fk_guard=""
  if [ "$tbl" = "opportunities" ]; then
    fk_guard="AND NOT EXISTS (SELECT 1 FROM paper_trade_runs p WHERE p.opportunity_id = o.id)
              AND NOT EXISTS (SELECT 1 FROM risk_events r WHERE r.opportunity_id = o.id)
              AND NOT EXISTS (SELECT 1 FROM opportunity_observations x WHERE x.opportunity_id = o.id)"
  fi

  if [ "$DRY_RUN" = "1" ]; then
    n=$(psql_q "SELECT count(*) FROM (SELECT 1 FROM $tbl o WHERE o.$col < $cutoff $fk_guard LIMIT 100001) s")
    summary+=("$tbl:dryrun_older_${days}d=${n}")
    log "retention.dry-run table=$tbl window=${days}d eligible=${n}"
    continue
  fi

  # Archivo opt-in: snapshot zstd del rango a purgar ANTES de borrarlo.
  if [ "$DO_ARCHIVE" = "1" ]; then
    cutoff_day=$(date -u -d "$days days ago" +%Y%m%d)
    arch="$ARCHIVES_DIR/$tbl/$tbl-upto-$cutoff_day.tsv.zst"
    if [ -e "$arch" ]; then
      log "retention.archive table=$tbl reuse=$arch"
    else
      mkdir -p "$ARCHIVES_DIR/$tbl"
      if docker exec -i "$PG_CONTAINER" psql -U postgres -d arbitragex -X -qAt \
          -c "COPY (SELECT * FROM $tbl o WHERE o.$col < $cutoff $fk_guard) TO STDOUT" \
          | zstd -q -T0 -o "$arch" 2>/dev/null; then
        log "retention.archive table=$tbl file=$arch bytes=$(stat -c%s "$arch" 2>/dev/null || echo '?')"
      else
        rm -f "$arch"
        log "retention.archive table=$tbl FAILED — purge SKIPPED (fail-honest: sin archivo no se borra)"
        summary+=("$tbl:SKIP_archive_failed")
        continue
      fi
    fi
  fi

  # RDO: jamás purgar buckets que el rollup aún no materializó (lazy ≤24/día
  # no alcanza para 5 días de backlog) — verificación dura post-backfill.
  if [ "$hook" = "rdo" ]; then
    missing=$(psql_q "
      WITH oldest AS (SELECT min(ts_ms)/300000::bigint*300000 AS b FROM route_discovery_outcomes)
      SELECT count(*) FROM generate_series((SELECT b FROM oldest), $cutoff - $cutoff % 300000, 300000) g(bucket)
      WHERE NOT EXISTS (SELECT 1 FROM route_discovery_outcome_rollup_5m rr
                        WHERE rr.dim='__totals__' AND rr.bucket_ms=g.bucket)") || missing="err"
    if [ "$missing" != "0" ]; then
      summary+=("$tbl:SKIP_rollup_missing=$missing")
      log "retention.skip table=$tbl reason=rollup_backfill_pending buckets_missing=$missing (backfill budget agotado)"
      continue
    fi
  fi

  deleted=0
  batches_since_ckpt=0
  while :; do
    out=$(psql_batch "
      WITH victim AS (SELECT ctid FROM $tbl o WHERE o.$col < $cutoff $fk_guard LIMIT $batch)
      DELETE FROM $tbl t USING victim v WHERE t.ctid = v.ctid")
    rc=$?
    if [ $rc -ne 0 ]; then
      if printf '%s' "$out" | grep -q 'lock timeout'; then
        reason="lock_timeout_skip"
      elif printf '%s' "$out" | grep -q 'statement timeout'; then
        reason="statement_timeout_budget"
      else
        reason="error:$(printf '%s' "$out" | head -c 300 | tr '\n' ' ')"
      fi
      break
    fi
    n=$(printf '%s' "$out" | grep -oE 'DELETE [0-9]+' | grep -oE '[0-9]+$') || n=0
    deleted=$((deleted + n))
    batches_since_ckpt=$((batches_since_ckpt + 1))
    [ "$n" -lt "$batch" ] && break
    # Pacing WAL (incidente 2026-09-04 13:36Z): CHECKPOINT periódico para que
    # los segments se reciclen al ritmo del purge en vez de acumularse.
    if [ $((batches_since_ckpt % CHECKPOINT_EVERY)) -eq 0 ]; then
      docker exec -i "$PG_CONTAINER" psql -U postgres -d arbitragex -X -qAt \
        -c "SET statement_timeout='600s'; CHECKPOINT" >/dev/null 2>&1 || true
    fi
    if [ "$deleted" -ge "$MAX_ROWS_PER_TABLE" ]; then
      reason="daily_row_cap_${MAX_ROWS_PER_TABLE}_resume_tomorrow"
      break
    fi
    if [ $((SECONDS - t0)) -gt "$TABLE_BUDGET_S" ]; then
      reason="table_budget_exceeded_resume_tomorrow"
      break
    fi
  done

  total_deleted=$((total_deleted + deleted))
  elapsed=$((SECONDS - t0))
  if [ -n "$reason" ]; then
    summary+=("$tbl:deleted=${deleted}:${reason}")
    log "retention.table table=$tbl deleted=$deleted elapsed=${elapsed}s stop='$reason'"
  else
    summary+=("$tbl:deleted=${deleted}:complete")
    log "retention.table table=$tbl deleted=$deleted elapsed=${elapsed}s complete"
  fi

  # VACUUM ANALYZE (ShareUpdateExclusive: no bloquea writers; devuelve espacio
  # reutilizable a la tabla y refresca stats para los planes de la API)
  if [ "$VACUUM" = "1" ] && [ "$deleted" -gt 0 ]; then
    # Each -c is a separate request. SET + VACUUM in one -c creates an
    # implicit transaction, where PostgreSQL refuses VACUUM (SQLSTATE 25001).
    # Keep this as ordinary VACUUM: never rewrite a relation with VACUUM FULL.
    docker exec -i "$PG_CONTAINER" psql -U postgres -d arbitragex -X -qAt \
      -v ON_ERROR_STOP=1 \
      -c "SET lock_timeout='$BATCH_LOCK_TIMEOUT'" \
      -c "SET statement_timeout='600s'" \
      -c "VACUUM (ANALYZE) $tbl" >/dev/null 2>&1 \
      || log "retention.vacuum table=$tbl failed (non-fatal)"
  fi
done

log "retention.summary dry_run=$DRY_RUN deleted_total=$total_deleted elapsed_total=$((SECONDS - ts_now_start))s ${summary[*]:-}"
exit 0
