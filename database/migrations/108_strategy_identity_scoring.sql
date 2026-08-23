-- ArbitrageX v2 — Migration 108: per-strategy identity in the Gate-C scoring circuit
--
-- STRAT-IDENT-01 (operator directive 2026-08-23): "no debes colocar clase de
-- estrategia, sino evaluar directamente cada estrategia y esta dirá cuáles son
-- las estructuras que le aplican y con ello armas el combo particular y
-- específico".
--
-- Before this migration the scoring circuit flattened strategy identity:
--   - `bayesian_priors` was keyed UNIQUE(token_pair): all 264 cartridges trading
--     WETH/USDC would share ONE calibration bucket.
--   - `scored_opportunities` carried no strategy identity at all — scores could
--     not be sliced per strategy.
--
-- This migration re-keys the calibration store per STRATEGY (the cartridge stem
-- for the 264 cartridges; the engine kind for the 5 core engines — each an
-- individual strategy) and persists the strategy identity on every score.
--
-- `bayesian_priors` is EMPTY on every environment (no writer exists yet as of
-- 2026-08-23 — verified), so re-keying drops no data.
--
-- Idempotent + additive. Paper-only telemetry (RULE 00 / §34: capital = 0).

BEGIN;

-- ---------------------------------------------------------------------------
-- scored_opportunities: carry the per-strategy identity.
-- Nullable + no backfill: rows scored before this migration have no honest
-- per-strategy value (R8 — never fabricate one).
-- ---------------------------------------------------------------------------
ALTER TABLE scored_opportunities
    ADD COLUMN IF NOT EXISTS strategy_key TEXT;

CREATE INDEX IF NOT EXISTS idx_scored_opportunities_strategy
    ON scored_opportunities (strategy_key, created_at DESC)
    WHERE strategy_key IS NOT NULL;

-- ---------------------------------------------------------------------------
-- bayesian_priors: per-STRATEGY calibration bucket.
-- Table is empty (no writer exists); replace the per-pair unique with a
-- per-strategy unique. token_pair stays as secondary context.
-- ---------------------------------------------------------------------------
ALTER TABLE bayesian_priors
    ADD COLUMN IF NOT EXISTS strategy_key TEXT;

-- Drop the per-pair unique (constraint name from migration 097).
ALTER TABLE bayesian_priors
    DROP CONSTRAINT IF EXISTS bayesian_priors_token_pair_key;

CREATE UNIQUE INDEX IF NOT EXISTS uq_bayesian_priors_strategy
    ON bayesian_priors (strategy_key)
    WHERE strategy_key IS NOT NULL;

COMMIT;
