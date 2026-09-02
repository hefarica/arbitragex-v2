-- 115_route_discovery_outcomes_rollup_5m.sql
--
-- RDO-SUMMARY-503 (2026-09-02): the /api/v1/route-discovery-outcomes/summary
-- aggregations time out at production scale. At diagnosis the table held
-- 76.1M rows / 37 GB (~1.32M rows/h) and a 24h window = 35.6M rows;
-- EXPLAIN ANALYZE measured 90.7s for the totals aggregate alone (1.84M
-- buffer reads ≈ 14 GB from disk + 648k temp blocks). No index can make an
-- on-demand GROUP BY over ~32M wide rows fit a 15s statement budget — the
-- honest 503 (query_failed, #502) keeps firing, /operator stays FAIL, and
-- every FE poll burns ~90s of I/O per aggregate.
--
-- Canonical fix: a 5-minute-grain ROLLUP of exactly the dimensions the
-- summary groups by (totals / reason / chain / cartridge / pair). Dimension
-- cardinality is tiny (measured over 1h: 81 reasons, 1 chain, 270
-- cartridges, 20 pairs → ≈370 rollup rows per bucket, ≈107k rows per 24h),
-- so the summary reads the rollup for complete buckets and scans RAW only
-- for the two ≤5-minute window edges. The api-server keeps the rollup
-- topped up (route-discovery-outcomes-api.ts ensureRollups — oldest-missing
-- first, ≤24 buckets per request, INSERT .. ON CONFLICT DO NOTHING); this
-- migration seeds the full history ONCE.
--
-- Only COMPLETE buckets are ever stored (rows strictly below the current
-- bucket start), so a completed bucket can never collide with a stale
-- partial row — the in-progress bucket is always served from raw.
--
-- Rerun safety (lint-migration-rerun-lock-safety.sh): the runner re-runs
-- every file on every deploy. The backfill is guarded by an emptiness probe
-- inside a DO block — the no-op path takes no lock on route_discovery_outcomes
-- beyond a trivial SELECT. The one-time INSERT..SELECT takes only
-- AccessShare (read) on route_discovery_outcomes and RowExclusive on the NEW
-- rollup table: no DDL locks, no writer conflicts (FREEZE-01 doctrine).
-- statement_timeout raised past the runner default for the one-time
-- full-history scan (~2-4 min at 37 GB); the guarded re-run is instant.
SET statement_timeout = '9min';

CREATE TABLE IF NOT EXISTS route_discovery_outcome_rollup_5m (
    dim           text   NOT NULL,  -- 'reason' | 'chain' | 'cartridge' | 'pair' | '__totals__'
    key           text   NOT NULL,  -- dim value; pair = token_in||'|'||token_out; totals = ''
    bucket_ms     bigint NOT NULL,  -- UTC 5-minute floor (bucket start)
    n             bigint NOT NULL DEFAULT 0,
    opportunities bigint NOT NULL DEFAULT 0,
    with_reserves bigint NOT NULL DEFAULT 0,
    profit_gt0    bigint NOT NULL DEFAULT 0,
    PRIMARY KEY (dim, key, bucket_ms)
);

DO $$
DECLARE
    current_bucket_start bigint := floor(extract(epoch FROM now()) * 1000)::bigint / 300000 * 300000;
BEGIN
    -- One-time seed of COMPLETE historical buckets only (rows strictly below
    -- the current bucket start — see header). Re-runs are an instant no-op.
    IF NOT EXISTS (SELECT 1 FROM route_discovery_outcome_rollup_5m LIMIT 1) THEN
        INSERT INTO route_discovery_outcome_rollup_5m
            (dim, key, bucket_ms, n, opportunities, with_reserves, profit_gt0)
        SELECT '__totals__', '',
               r.ts_ms / 300000::bigint * 300000,
               count(*),
               count(*) FILTER (WHERE r.is_opportunity),
               count(*) FILTER (WHERE r.had_reserves),
               count(*) FILTER (WHERE r.estimated_profit > 0)
        FROM route_discovery_outcomes r
        WHERE r.ts_ms < current_bucket_start
        GROUP BY 3
        UNION ALL
        SELECT 'reason', COALESCE(NULLIF(r.reason, ''), '(null)'),
               r.ts_ms / 300000::bigint * 300000,
               count(*), count(*) FILTER (WHERE r.is_opportunity), 0, 0
        FROM route_discovery_outcomes r
        WHERE r.ts_ms < current_bucket_start
        GROUP BY 2, 3
        UNION ALL
        SELECT 'chain', r.chain_id::text,
               r.ts_ms / 300000::bigint * 300000,
               count(*), count(*) FILTER (WHERE r.is_opportunity), 0, 0
        FROM route_discovery_outcomes r
        WHERE r.ts_ms < current_bucket_start
        GROUP BY 2, 3
        UNION ALL
        SELECT 'cartridge', COALESCE(NULLIF(r.cartridge_id, ''), '(null)'),
               r.ts_ms / 300000::bigint * 300000,
               count(*), count(*) FILTER (WHERE r.is_opportunity), 0, 0
        FROM route_discovery_outcomes r
        WHERE r.ts_ms < current_bucket_start
        GROUP BY 2, 3
        UNION ALL
        SELECT 'pair', COALESCE(r.token_in, '') || '|' || COALESCE(r.token_out, ''),
               r.ts_ms / 300000::bigint * 300000,
               count(*), count(*) FILTER (WHERE r.is_opportunity), 0, 0
        FROM route_discovery_outcomes r
        WHERE r.ts_ms < current_bucket_start
        GROUP BY 2, 3
        ON CONFLICT DO NOTHING;
    END IF;
END $$;
