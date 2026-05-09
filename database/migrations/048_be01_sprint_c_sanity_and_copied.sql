-- ============================================================================
-- Migration 048: BE-01 Sprint C — spread sanity config + p_copied heuristic config
--
-- Adds three operator-tunable columns to `trading_config`:
--
--   spread_sanity_mult           NUMERIC(8,4)   DEFAULT 3.0
--     Multiplier for the AMM-vs-oracle spread sanity gate (Component 9).
--     Reject if observed_rate / reference_rate < 1/mult OR > mult.
--     Range [1.0, 100.0] enforced by CHECK constraint.
--
--   p_copied_volume_threshold_usd  NUMERIC(20,2)  DEFAULT 1_000_000.0
--     Baseline 24h volume (USD) for the copy-trade probability heuristic
--     (Component 5). At vol==threshold → p_copied=0. At 10×threshold → 10%.
--     Range > 0 is implied by the formula (log10 is undefined at 0).
--
--   p_copied_max                 NUMERIC(6,4)   DEFAULT 0.5
--     Maximum p_copied allowed (cap applied before cost computation).
--     Range [0.0, 1.0] enforced by CHECK constraint.
--
-- Idempotent: IF NOT EXISTS / IF NOT EXISTS on constraints.
-- Reversible: DOWN block provided.
-- R8 fail-honest: all three columns have NOT NULL defaults so existing rows
-- inherit sane values without operator action.
-- ============================================================================

BEGIN;

-- ────────────────────────────────────────────────────────────────────────────
-- STEP 1: Add the three columns (idempotent with IF NOT EXISTS)
-- ────────────────────────────────────────────────────────────────────────────

ALTER TABLE trading_config
    ADD COLUMN IF NOT EXISTS spread_sanity_mult NUMERIC(8, 4) NOT NULL DEFAULT 3.0,
    ADD COLUMN IF NOT EXISTS p_copied_volume_threshold_usd NUMERIC(20, 2) NOT NULL DEFAULT 1000000.0,
    ADD COLUMN IF NOT EXISTS p_copied_max NUMERIC(6, 4) NOT NULL DEFAULT 0.5;

-- ────────────────────────────────────────────────────────────────────────────
-- STEP 2: CHECK constraints (safe to add after column defaults are applied)
-- ────────────────────────────────────────────────────────────────────────────

ALTER TABLE trading_config
    ADD CONSTRAINT trading_config_spread_sanity_mult_check
        CHECK (spread_sanity_mult >= 1.0 AND spread_sanity_mult <= 100.0),
    ADD CONSTRAINT trading_config_p_copied_max_check
        CHECK (p_copied_max >= 0.0 AND p_copied_max <= 1.0);

-- ────────────────────────────────────────────────────────────────────────────
-- STEP 3: Verification — report row counts with new columns populated
-- ────────────────────────────────────────────────────────────────────────────

DO $$
DECLARE row_count INT;
BEGIN
    SELECT COUNT(*) INTO row_count FROM trading_config
        WHERE spread_sanity_mult IS NOT NULL
          AND p_copied_volume_threshold_usd IS NOT NULL
          AND p_copied_max IS NOT NULL;
    RAISE NOTICE 'Migration 048: % rows have spread_sanity + p_copied configs populated', row_count;
END $$;

COMMIT;

-- ============================================================================
-- DOWN (rollback)
-- ============================================================================
-- ALTER TABLE trading_config
--     DROP CONSTRAINT IF EXISTS trading_config_spread_sanity_mult_check,
--     DROP CONSTRAINT IF EXISTS trading_config_p_copied_max_check,
--     DROP COLUMN IF EXISTS spread_sanity_mult,
--     DROP COLUMN IF EXISTS p_copied_volume_threshold_usd,
--     DROP COLUMN IF EXISTS p_copied_max;
