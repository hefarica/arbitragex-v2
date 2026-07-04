-- 099_opportunities_route_metadata.sql
-- G-SIM-1 PR-B2b Fase 2 (A1): persist complete route topology alongside
-- each opportunity so sim-ctl can reconstruct an OpportunityCandidate
-- without a second divergent encoder.
--
-- The minimal Opportunity contract (token_in/token_out, dex_a/dex_b) only
-- carries a 2-hop view. Multi-hop arbitrage routes need the full topology:
-- pool_addresses[], token_addresses[], dex_adapters[], and decimals per token.
--
-- route_metadata JSONB stores exactly what build_round_trip_context_from_candidate
-- consumes. Default '{}' for backward compat with pre-existing rows.
--
-- Schema:
--   {
--     "pool_addresses": ["0x...", ...],
--     "token_addresses": ["0x...", ...],
--     "dex_adapters": ["uniswap_v2_router", ...],
--     "decimals": {"0xtoken_lower": 18, ...}
--   }
--
-- Design notes:
--   * JSONB (not columns) because route length is variable (2-8 hops).
--   * No NOT NULL constraint — legacy rows and detection-time failures
--     persist '{}' rather than failing the INSERT (R8 fail-honest).
--   * GIN index on route_metadata->'pool_addresses' for future "find all
--     opportunities using pool X" queries (cross-opportunity analytics).

ALTER TABLE opportunities
    ADD COLUMN IF NOT EXISTS route_metadata JSONB NOT NULL DEFAULT '{}'::jsonb;

CREATE INDEX IF NOT EXISTS idx_opportunities_route_pools
    ON opportunities USING GIN ((route_metadata->'pool_addresses') jsonb_path_ops);

COMMENT ON COLUMN opportunities.route_metadata IS
    'G-SIM-1 B2b: complete route topology {pool_addresses[], token_addresses[], dex_adapters[], decimals{}} for sim-ctl OpportunityCandidate reconstruction. Empty {} for legacy rows or detection-time failures.';
