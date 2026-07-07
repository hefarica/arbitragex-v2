-- ============================================================================
-- Migration 050: Phase 3 DB hardening
--
-- Items: DB-1, DB-2, DB-3, DB-4, DB-6
-- Deferred: DB-5 (pgBouncer = docker-compose infra task, not SQL migration)
--           DB-7 (WAL level = already 'replica' by default; no change needed)
--
-- Indices on hot query paths flagged in gap analysis 2026-05-08:
--   DB-1: pools(factory_id)                      — JOIN performance
--   DB-2: pool_reserves(pool_id, block_number)   — latest-reserves-per-pool
--   DB-3: pool_ticks(pool_id, block_number)       — UniV3 tick lookups
--   DB-4: routes(chain_id, hash)                  — cross-chain dedup
--         NOTE: routes.hash already has a UNIQUE constraint (implying a
--         single-column unique index). The composite (chain_id, hash) index
--         is still needed for queries with an equality predicate on chain_id
--         in addition to hash, which the planner will prefer over the
--         single-column unique index.
--
-- Autovacuum tuning for high-write tables (DB-6):
--   opportunities: vacuum at 5% bloat (default 20%), analyze at 2% (default 10%)
--   executions:    same
--
-- All DDL is idempotent: CREATE INDEX IF NOT EXISTS, DO $$ existence checks,
-- and SET (...) storage parameters are safe to re-run.
--
-- REVERSIBILITY: see rollback block at bottom (commented out, activate manually).
-- ============================================================================

BEGIN;

-- ── DB-1 ──────────────────────────────────────────────────────────────────
-- pools.factory_id — improves JOIN pools → factories on the hot scanner path.
-- Partial: rows with factory_id IS NULL carry no JOIN value; excluding them
-- keeps the index compact and prevents NULL-key index bloat on rows that
-- were inserted before factory linkage was complete.
CREATE INDEX IF NOT EXISTS idx_pools_factory_id
    ON pools(factory_id)
    WHERE factory_id IS NOT NULL;

-- ── DB-2 ──────────────────────────────────────────────────────────────────
-- pool_reserves(pool_id, block_number DESC)
-- Query pattern: SELECT ... FROM pool_reserves
--               WHERE pool_id = $1 ORDER BY block_number DESC LIMIT 1;
-- DESC on block_number makes this index directly usable by the ORDER BY
-- without a separate sort step (index scan returns rows in DESC order already).
CREATE INDEX IF NOT EXISTS idx_pool_reserves_pool_block
    ON pool_reserves(pool_id, block_number DESC);

-- ── DB-3 ──────────────────────────────────────────────────────────────────
-- pool_ticks(pool_id, block_number DESC) — same pattern as DB-2 for UniV3.
-- pool_ticks is defined in migration 022 and is always present; the
-- information_schema guard is retained as a belt-and-suspenders safeguard
-- in case the schema is applied out of order in a test environment.
DO $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM information_schema.tables
        WHERE table_schema = 'public' AND table_name = 'pool_ticks'
    ) THEN
        EXECUTE 'CREATE INDEX IF NOT EXISTS idx_pool_ticks_pool_block
            ON pool_ticks(pool_id, block_number DESC)';
        RAISE NOTICE 'Migration 050: idx_pool_ticks_pool_block created (or already existed)';
    ELSE
        RAISE NOTICE 'Migration 050: pool_ticks table absent — idx_pool_ticks_pool_block skipped';
    END IF;
END $$;

-- ── DB-4 ──────────────────────────────────────────────────────────────────
-- routes(chain_id, hash) — cross-chain dedup query: WHERE chain_id=$1 AND hash=$2.
-- routes.hash has an existing UNIQUE constraint (single-column unique index).
-- The composite index is distinct from and complementary to that constraint:
-- the planner will choose it for queries that filter on both columns, and
-- will fall back to the unique index for hash-only lookups.
-- Existence guard: both columns confirmed present in migration 022, but the
-- guard is retained for safety in partial-apply environments.
DO $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_schema = 'public' AND table_name = 'routes' AND column_name = 'chain_id'
    ) AND EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_schema = 'public' AND table_name = 'routes' AND column_name = 'hash'
    ) THEN
        EXECUTE 'CREATE INDEX IF NOT EXISTS idx_routes_chain_hash
            ON routes(chain_id, hash)';
        RAISE NOTICE 'Migration 050: idx_routes_chain_hash created (or already existed)';
    ELSE
        RAISE NOTICE 'Migration 050: routes.chain_id or routes.hash absent — idx_routes_chain_hash skipped';
    END IF;
END $$;

-- ── DB-6 ──────────────────────────────────────────────────────────────────
-- Aggressive autovacuum for high-write tables.
--
-- Default PostgreSQL thresholds are designed for OLTP workloads with moderate
-- write rates. ArbitrageX inserts hundreds of opportunity rows per minute and
-- updates status through an 8-state machine, causing table bloat that degrades
-- sequential scans faster than the default 20%/10% thresholds can reclaim.
--
-- autovacuum_vacuum_scale_factor = 0.05  → trigger vacuum when dead tuples
--   exceed 5% of live row count (vs default 20%).
-- autovacuum_analyze_scale_factor = 0.02 → trigger ANALYZE at 2% change
--   (vs default 10%), keeping planner statistics fresh for a table whose
--   hot-path indices are queried with high cardinality predicates.
-- autovacuum_vacuum_cost_delay = 10      → 10ms I/O throttle delay between
--   vacuum cost cycles (vs default 2ms in PG14+ or 20ms in older versions).
--   This is deliberately slightly more aggressive to keep up with write rate
--   without starving foreground queries. Tune down to 20 if I/O contention
--   is observed on the VPS.
--
-- Note: ALTER TABLE SET (...) is idempotent — re-running this migration does
-- not change behaviour if parameters are already set to these values.

ALTER TABLE opportunities SET (
    autovacuum_vacuum_scale_factor  = 0.05,
    autovacuum_analyze_scale_factor = 0.02,
    autovacuum_vacuum_cost_delay    = 10
);

ALTER TABLE executions SET (
    autovacuum_vacuum_scale_factor  = 0.05,
    autovacuum_analyze_scale_factor = 0.02,
    autovacuum_vacuum_cost_delay    = 10
);

-- ── SANITY CHECK ──────────────────────────────────────────────────────────
-- Report how many of the 4 expected indices are present after migration.
-- Conditional indices (DB-3, DB-4) may legitimately be absent if the tables
-- were skipped — the notice will reflect the real count.
DO $$
DECLARE
    indices_found  INT;
    expected_count INT := 4;
BEGIN
    SELECT COUNT(*) INTO indices_found
    FROM pg_indexes
    WHERE schemaname = 'public'
      AND indexname IN (
          'idx_pools_factory_id',
          'idx_pool_reserves_pool_block',
          'idx_pool_ticks_pool_block',
          'idx_routes_chain_hash'
      );

    RAISE NOTICE 'Migration 050 complete: %/% expected indices present in pg_indexes (% may be legitimately absent if conditional tables were skipped)',
        indices_found, expected_count, (expected_count - indices_found);
END $$;

COMMIT;

-- ============================================================================
-- ROLLBACK (activate manually — do NOT uncomment in normal deploy)
-- ============================================================================
-- BEGIN;
-- DROP INDEX IF EXISTS idx_pools_factory_id;
-- DROP INDEX IF EXISTS idx_pool_reserves_pool_block;
-- DROP INDEX IF EXISTS idx_pool_ticks_pool_block;
-- DROP INDEX IF EXISTS idx_routes_chain_hash;
-- ALTER TABLE opportunities RESET (
--     autovacuum_vacuum_scale_factor,
--     autovacuum_analyze_scale_factor,
--     autovacuum_vacuum_cost_delay
-- );
-- ALTER TABLE executions RESET (
--     autovacuum_vacuum_scale_factor,
--     autovacuum_analyze_scale_factor,
--     autovacuum_vacuum_cost_delay
-- );
-- COMMIT;
