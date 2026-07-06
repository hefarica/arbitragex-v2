-- Migration 059: Pool Discovery Tables
-- Description: Adds tables to track unindexed pairs discovered dynamically from the mempool, 
-- and opportunity observations tied to these dynamic discoveries.

BEGIN;

-- Table to track token pairs that are seen in the mempool but are not yet registered
-- in our static pools/factories catalog.
CREATE TABLE IF NOT EXISTS observed_unindexed_pairs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    chain_id BIGINT NOT NULL,
    token0_addr VARCHAR(42) NOT NULL,
    token1_addr VARCHAR(42) NOT NULL,
    router_addr VARCHAR(42),
    source_event VARCHAR(64) NOT NULL, -- e.g. public_mempool, private_hint
    first_seen_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_seen_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    observation_count BIGINT NOT NULL DEFAULT 1,
    is_resolved BOOLEAN NOT NULL DEFAULT FALSE, -- Set to true once a pool is successfully mapped
    resolved_pool_addr VARCHAR(42)
);

-- Unique constraint on pair + router to deduplicate tracking
CREATE UNIQUE INDEX IF NOT EXISTS idx_observed_pairs_unique 
ON observed_unindexed_pairs (chain_id, token0_addr, token1_addr, router_addr);

-- Index for querying unresolved pairs efficiently
CREATE INDEX IF NOT EXISTS idx_observed_pairs_unresolved
ON observed_unindexed_pairs (is_resolved) WHERE is_resolved = FALSE;


-- Table to track observations of opportunities specifically involving 
-- dynamically discovered pools, useful for paper trading and offline analysis.
CREATE TABLE IF NOT EXISTS opportunity_observations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    chain_id BIGINT NOT NULL,
    strategy_kind VARCHAR(64) NOT NULL,
    opportunity_id UUID REFERENCES opportunities(id) ON DELETE SET NULL,
    tx_hash VARCHAR(66) NOT NULL,
    discovered_pool_addrs JSONB NOT NULL, -- JSON array of pool addresses involved
    estimated_profit_usd DOUBLE PRECISION,
    estimated_gas_usd DOUBLE PRECISION,
    is_viable BOOLEAN NOT NULL DEFAULT FALSE,
    rejection_reason VARCHAR(255),
    observed_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Index on observation time and chain
CREATE INDEX IF NOT EXISTS idx_opportunity_observations_time 
ON opportunity_observations (chain_id, observed_at DESC);

COMMIT;
