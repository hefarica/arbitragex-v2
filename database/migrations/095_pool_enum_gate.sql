-- Migration 095: pool enumeration gate — enum_source provenance column.
--
-- Supports Block 2 (proactive top-TVL pool enumeration worker). Idempotent +
-- non-destructive: safe to run twice, never drops or rewrites existing data.
--
-- NOTE: `pools.is_active` ALREADY exists (migration 022) and PoolSyncWorker
-- already filters `is_active = TRUE` (V2 + V3 load paths), so no activation gate
-- needs adding. This migration only adds the provenance column so enumerated
-- pools are distinguishable from seeds/reactive discovery, and so a future tick
-- can reactivate an is_active=false pool once its tokens become token-safe.
--
-- Verify after apply:
--   SELECT enum_source, is_active, COUNT(*) FROM pools
--   GROUP BY enum_source, is_active ORDER BY enum_source, is_active;

-- 1. Provenance column (nullable; backfilled below). Where each pool came from:
--    'seed' (pre-existing), 'reactive' (discover_from_intent), 'subgraph_tvl'
--    (Block 2 enumeration worker).
ALTER TABLE pools ADD COLUMN IF NOT EXISTS enum_source TEXT;

-- 2. Backfill existing rows to a conservative provenance so they are never
--    confused with newly-enumerated pools (only rows still NULL — idempotent).
UPDATE pools SET enum_source = 'seed' WHERE enum_source IS NULL;

-- 3. Index for the worker's per-chain active-pool dedup + provenance analytics.
CREATE INDEX IF NOT EXISTS idx_pools_chain_active_source
    ON pools (chain_id, is_active, enum_source);
