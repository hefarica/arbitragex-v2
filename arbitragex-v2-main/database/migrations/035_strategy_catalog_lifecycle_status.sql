-- ArbitrageX v2 — Migration 035: strategy_catalog.lifecycle_status
--
-- Replaces the binary `is_implemented` boolean with a 5-state lifecycle field
-- so the UI honestly reflects WHERE each strategy actually is along the road
-- from idea → production. The old boolean conflated "designed but no emitter"
-- with "no code at all", which produced the operator complaint that the
-- /strategies tab "marks all 10 as enabled" while only 1 actually emits.
--
-- States (RULE 00 — zero mocks; UI must NEVER pretend a scaffolded strategy
-- is producing opportunities):
--   live           — searcher emits opportunities of this kind in production.
--                    Toggle in UI actually enables/disables the emitter via
--                    `trading_config.enabled_strategies` gate.
--   designed       — design doc complete and reviewed (e.g. cex_dex v4 in
--                    docs/superpowers/plans/), no Rust emitter yet. Toggle
--                    is informational only.
--   scaffold       — `StrategyKind` enum variant exists, persistence layer
--                    accepts the kind, but no scanner produces opportunities
--                    of this type. Toggle is informational only.
--   not_started    — string identifier reserved, no enum variant, no design
--                    doc, no code. Toggle is informational only.
--   defensive_only — sandwich detection used to protect our own swaps;
--                    NEVER executed offensively. Toggle is forced-on by UI
--                    (existing behaviour preserved).
--
-- The `is_implemented` boolean is retained for backward compat but should
-- be considered deprecated; UI reads `lifecycle_status` exclusively.

ALTER TABLE strategy_catalog ADD COLUMN IF NOT EXISTS lifecycle_status TEXT;

ALTER TABLE strategy_catalog DROP CONSTRAINT IF EXISTS strategy_catalog_lifecycle_status_check;
ALTER TABLE strategy_catalog ADD CONSTRAINT strategy_catalog_lifecycle_status_check
  CHECK (lifecycle_status IS NULL OR lifecycle_status IN (
    'live', 'designed', 'scaffold', 'not_started', 'defensive_only'
  ));

-- Backfill — truth as of 2026-05-07. Update this section in future migrations
-- when a strategy advances states (e.g. triangular: scaffold → live).
UPDATE strategy_catalog SET lifecycle_status = 'live'
  WHERE kind = 'dex_arb';

UPDATE strategy_catalog SET lifecycle_status = 'designed'
  WHERE kind = 'cex_dex';
  -- Design doc: docs/superpowers/plans/2026-05-06-cex-dex-strategy.md (v4)

UPDATE strategy_catalog SET lifecycle_status = 'scaffold'
  WHERE kind IN ('triangular', 'flashloan_arb', 'liquidation');
  -- Enum variants: shared-rs/src/contracts.rs::StrategyKind

UPDATE strategy_catalog SET lifecycle_status = 'not_started'
  WHERE kind IN ('jit_liquidity_v3', 'cross_chain', 'micro_hft', 'mev_boost_block');

UPDATE strategy_catalog SET lifecycle_status = 'defensive_only'
  WHERE kind = 'sandwich_def';

-- Sanity: every existing row should have a lifecycle_status now.
DO $$
DECLARE
  null_count INT;
BEGIN
  SELECT COUNT(*) INTO null_count FROM strategy_catalog WHERE lifecycle_status IS NULL;
  IF null_count > 0 THEN
    RAISE WARNING 'strategy_catalog: % rows still have NULL lifecycle_status — add UPDATE clause above', null_count;
  END IF;
END $$;
