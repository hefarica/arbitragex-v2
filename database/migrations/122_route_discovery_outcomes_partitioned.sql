-- ArbitrageX v2 — Migration 122: route_discovery_outcomes → daily RANGE partitions
-- (DEUDA-3, PERF-STACK-2026-09-20)
--
-- Problem: route_discovery_outcomes grows ~1.32M rows/h (~15GB/day). Retention
-- (pg_retention.sh) keeps a 1-day window with batched DELETEs — WAL-heavy
-- (2026-09-04 13:36Z incident: 53.8M-row purge → 16GB WAL → disk 100% →
-- postmaster crash-loop) and the space only returns after VACUUM.
--
-- Fix: RANGE partitions by ts_ms (UTC day, name route_discovery_outcomes_pYYYYMMDD).
-- Retention becomes DROP PARTITION (instant, immediate space return, no WAL
-- burst). Idempotency: PG requires any UNIQUE constraint on a partitioned table
-- to include the partition key → UNIQUE(stream_id, ts_ms). A redelivered sink
-- message carries the SAME stream_id AND ts_ms (both copied from the Redis
-- message), so ON CONFLICT (stream_id, ts_ms) preserves at-least-once
-- idempotency exactly (sink change ships in this same deploy).
--
-- Conversion shape (precedent: scripts/rdo_table_swap.sh, ARBX-RETENTION-01):
--   1. build empty partitioned clone `_p` (LIKE ... INCLUDING DEFAULTS shares
--      the existing route_discovery_outcomes_id_seq — id continuity preserved)
--   2. copy only the recent tail (default 3h, operator-tunable via
--      -v rdo_copy_tail_ms=…): bounded single-statement WAL (~1.5GB peak, the
--      per-chunk peak the 09-04 precedent proved safe). Older rows are NOT
--      copied — their 5m rollup is materialized (route_discovery_outcome_rollup_5m)
--      and raw retention was already 1 day; they stay in the renamed legacy
--      table for verification and are dropped by the operator (see runbook).
--   3. atomic swap under ACCESS EXCLUSIVE (lock_timeout=10s from the runner's
--      FREEZE-01 lockguard): lock → copy the ≥watermark delta (live writers
--      caught up) → detach sequence ownership → rename old → rename new.
--
-- Runner compatibility (run_migrations.sh re-runs EVERY file EVERY deploy):
--   - rerun after success: table already partitioned → psql \if no-op (zero
--     table locks taken, rerun-lock-safety compliant).
--   - rerun after mid-flight failure: the stale `_p` clone is dropped and
--     rebuilt; the legacy table was never renamed (swap is transactional), so
--     nothing was lost.
--   - autocommit file (no wrapping BEGIN except the swap itself): CHECKPOINT
--     pacing statements are legal; each statement gets the runner's
--     lock_timeout=10s / statement_timeout=10min.
--   - fresh/CI database: table empty → copy moves 0 rows, swap is instant.
--
-- Partition bounds are exact UTC-day multiples of 86400000 ms (integer
-- arithmetic, no session-TZ dependence) so pg_retention.sh can derive them
-- from the name suffix with `date -u`.
--
-- Index names on the new table carry an `_part_` infix: the canonical names
-- (idx_rdo_chain_ts, …) stay on the legacy table until the operator drops it —
-- index names are schema-global and nothing in the repo references them by
-- name (selftest comments aside).
--
-- RUNBOOK (operator / -61, disk-gated, VPS):
--   a) deploy this migration (it runs automatically at deploy; watch the logs
--      for "Migration 122" — swap fails fast and visibly if locks are busy).
--   b) verify: SELECT count(*) / max(ts_ms) freshness on the new table;
--      rollup completeness for the dropped raw window (pg_retention.sh prints
--      rdo.rollup buckets_missing=0 before any purge).
--   c) free the disk: DROP TABLE route_discovery_outcomes_pre122; (metadata
--      unlink — instant, no WAL burst). Sequence is already OWNED BY NONE.
--   d) pg_retention.sh now maintains partitions (create tomorrow, drop
--      rollup-guarded expired ones) — no manual partition care needed.

-- Operator-tunable tail window (ms). Self-defaults so the standard runner
-- (which passes no such -v) never hits an unbound psql variable (121 pattern).
\if :{?rdo_copy_tail_ms}
\else
\set rdo_copy_tail_ms '10800000'
\endif

-- Session watermark + tail (temp table: same psql session, survives autocommit
-- statement boundaries). :'var' interpolation lives HERE, in plain SQL
-- position — psql does not interpolate inside DO $$ bodies.
CREATE TEMP TABLE rdo122_wm (wm BIGINT NOT NULL, tail_ms BIGINT NOT NULL);
INSERT INTO rdo122_wm VALUES (
    (extract(epoch FROM clock_timestamp()) * 1000)::bigint,
    COALESCE(NULLIF(:'rdo_copy_tail_ms', ''), '10800000')::bigint
);

-- Gate: already partitioned → whole file is a no-op (takes no table locks).
SELECT EXISTS (
    SELECT 1
      FROM pg_partitioned_table pt
      JOIN pg_class c ON c.oid = pt.partrelid
     WHERE c.relname = 'route_discovery_outcomes'
) AS rdo122_partitioned
\gset
\if :rdo122_partitioned
\echo 'Migration 122: route_discovery_outcomes already partitioned — no-op.'
\else

-- Stale clone from an aborted previous attempt (cold: no writers) — rebuild.
DROP TABLE IF EXISTS route_discovery_outcomes_p;

CREATE TABLE route_discovery_outcomes_p
    (LIKE route_discovery_outcomes INCLUDING DEFAULTS)
    PARTITION BY RANGE (ts_ms);

-- Partition key must be part of PK/UNIQUE (PG declarative partitioning rule).
ALTER TABLE route_discovery_outcomes_p
    ADD CONSTRAINT rdo_part_pkey PRIMARY KEY (id, ts_ms),
    ADD CONSTRAINT uq_rdo_part_stream_ts UNIQUE (stream_id, ts_ms);

CREATE INDEX idx_rdo_part_chain_ts ON route_discovery_outcomes_p (chain_id, ts_ms);
CREATE INDEX idx_rdo_part_opportunity ON route_discovery_outcomes_p (is_opportunity, ts_ms);
CREATE INDEX idx_rdo_part_pool_hint ON route_discovery_outcomes_p (pool_hint) WHERE pool_hint IS NOT NULL AND pool_hint <> '';
CREATE INDEX idx_rdo_part_ts ON route_discovery_outcomes_p (ts_ms);

-- Daily partitions covering [wm - tail, today + 2 days], bounds as exact
-- UTC-day multiples of 86400000 ms. Idempotent via to_regclass.
DO $$
DECLARE
    hi_ms  bigint;
    day_lo bigint;
    daytag text;
BEGIN
    SELECT ((extract(epoch FROM clock_timestamp())::bigint * 1000)
            / 86400000 + 2) * 86400000
      INTO hi_ms;
    SELECT floor((wm - tail_ms) / 86400000.0)::bigint * 86400000
      INTO day_lo
      FROM rdo122_wm;
    WHILE day_lo <= hi_ms LOOP
        daytag := to_char(to_timestamp(day_lo / 1000.0) AT TIME ZONE 'UTC', 'YYYYMMDD');
        IF to_regclass(format('route_discovery_outcomes_p%s', daytag)) IS NULL THEN
            EXECUTE format(
                'CREATE TABLE route_discovery_outcomes_p%s PARTITION OF route_discovery_outcomes_p FOR VALUES FROM (%s) TO (%s)',
                daytag, day_lo, day_lo + 86400000);
        END IF;
        day_lo := day_lo + 86400000;
    END LOOP;
END $$;

-- Bounded tail copy: ONE statement = ONE transaction ≈ the per-chunk WAL peak
-- the 09-04 precedent proved safe (~1.5GB for 3h of rows at ~1.32M rows/h).
INSERT INTO route_discovery_outcomes_p
SELECT * FROM route_discovery_outcomes
 WHERE ts_ms >= (SELECT wm - tail_ms FROM rdo122_wm)
   AND ts_ms <  (SELECT wm FROM rdo122_wm);

-- Recycle WAL right after the copy (autocommit: CHECKPOINT is legal here).
CHECKPOINT;

-- Atomic swap. Lock order: legacy table locked exclusively, live-writer delta
-- [wm, ∞) copied under the lock, then renames. lock_timeout=10s comes from the
-- runner's PGOPTIONS (FREEZE-01 lockguard) — a busy table aborts the deploy
-- visibly and the legacy table is untouched (transactional rollback).
BEGIN;
LOCK TABLE route_discovery_outcomes IN ACCESS EXCLUSIVE MODE;
INSERT INTO route_discovery_outcomes_p
SELECT * FROM route_discovery_outcomes
 WHERE ts_ms >= (SELECT wm FROM rdo122_wm);
-- Own the sequence by nothing so DROP TABLE ..._pre122 cannot take the shared
-- id sequence (default nextval() on the new table) down with it.
ALTER SEQUENCE route_discovery_outcomes_id_seq OWNED BY NONE;
ALTER TABLE route_discovery_outcomes RENAME TO route_discovery_outcomes_pre122;
ALTER TABLE route_discovery_outcomes_p RENAME TO route_discovery_outcomes;
COMMIT;

ANALYZE route_discovery_outcomes;

\echo 'Migration 122: route_discovery_outcomes is now partitioned (daily, ts_ms). Legacy rows live in route_discovery_outcomes_pre122 — operator drops it after verification (runbook in file header).'
\endif
