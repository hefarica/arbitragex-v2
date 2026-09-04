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
BACKFILL_BUDGET_S="${ARBX_RDO_BACKFILL_BUDGET_S:-900}"
TABLE_BUDGET_S="${ARBX_TABLE_BUDGET_S:-1200}"
BATCH_LOCK_TIMEOUT="5s"                     # FREEZE-01: nunca esperar más tras un lock
BATCH_STMT_TIMEOUT="300s"                   # un batch acotado, no un mega-DELETE
VACUUM="${ARBX_RETENTION_VACUUM:-1}"

# tabla|columna_tiempo|formato(ts|ms)|ventana_días|batch_size|hook
# hooks: rdo (backfill eager antes del purge) · paper|reserves (upsert rollup diario)
TABLES=(
  "route_discovery_outcomes|ts_ms|ms|2|100000|rdo"
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

if [ "$DRY_RUN" = "0" ]; then
  bf_start=$SECONDS
  bf_rows=1
  while [ "$bf_rows" -gt 0 ] && [ $((SECONDS - bf_start)) -lt "$BACKFILL_BUDGET_S" ]; do
    out=$(rdo_backfill_chunk) || { log "rdo.backfill ERROR: $out"; break; }
    bf_rows=$(printf '%s' "$out" | grep -oE 'INSERT 0 [0-9]+' | grep -oE '[0-9]+$') || bf_rows=0
    [ "$bf_rows" -gt 0 ] && log "rdo.backfill chunk inserted rows=$bf_rows elapsed=$((SECONDS - bf_start))s"
  done
  log "rdo.backfill done missing_remaining=${bf_rows} elapsed=$((SECONDS - bf_start))s budget=${BACKFILL_BUDGET_S}s"
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
# 3. Purge por tabla — batched, guard por índice, skip != error
# ----------------------------------------------------------------------------
summary=()
total_deleted=0
ts_now_start=$SECONDS

for spec in "${TABLES[@]}"; do
  IFS='|' read -r tbl col fmt days batch hook <<<"$spec"
  t0=$SECONDS
  reason=""

  # Guard: índice con la columna de corte en posición leading — acepta
  # "(col)", "(col, ...)" y "(col DESC)". replace(...) quita las comillas de
  # pg_indexes para columnas tipo "timestamp".
  if ! psql_q "SELECT 1 FROM pg_indexes WHERE tablename='$tbl' AND replace(indexdef, chr(34), '') ~ '\\(${col}[ ,)]' LIMIT 1" | grep -q 1; then
    summary+=("$tbl:SKIP_missing_index")
    log "retention.skip table=$tbl reason=missing_index_on_$col (no seqscan purge — ver migración 116)"
    continue
  fi

  if [ "$fmt" = "ms" ]; then
    cutoff="(extract(epoch FROM now() - interval '$days days') * 1000)::bigint"
  else
    cutoff="now() - interval '$days days'"
  fi

  if [ "$DRY_RUN" = "1" ]; then
    n=$(psql_q "SELECT count(*) FROM (SELECT 1 FROM $tbl WHERE $col < $cutoff LIMIT 100001) s")
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
          -c "COPY (SELECT * FROM $tbl WHERE $col < $cutoff) TO STDOUT" \
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
  while :; do
    out=$(psql_batch "
      WITH victim AS (SELECT ctid FROM $tbl WHERE $col < $cutoff LIMIT $batch)
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
    [ "$n" -lt "$batch" ] && break
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
    docker exec -i "$PG_CONTAINER" psql -U postgres -d arbitragex -X -qAt \
      -c "SET statement_timeout='600s'; VACUUM (ANALYZE) $tbl" >/dev/null 2>&1 \
      || log "retention.vacuum table=$tbl failed (non-fatal)"
  fi
done

log "retention.summary dry_run=$DRY_RUN deleted_total=$total_deleted elapsed_total=$((SECONDS - ts_now_start))s ${summary[*]:-}"
exit 0
