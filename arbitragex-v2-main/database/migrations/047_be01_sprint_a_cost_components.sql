-- Migration 047: BE-01 Sprint A — Cost Component Scalars
--
-- Adds two operator-configurable scalars to trading_config:
--   capital_cost_rate_annual_pct  — opportunity cost of capital (component 6)
--   ops_overhead_usd_per_attempt  — amortised per-attempt infra cost (component 7)
--
-- Both columns have sensible defaults that preserve existing semantics:
--   0.0    → free capital assumption (no opportunity cost)
--   0.01   → $0.01 per attempt (conservative infra amortisation)
--
-- CHECK constraints enforce operator-safe bounds (no negative costs; no
-- runaway values that would reject every opportunity).
--
-- Sprint A scope: these columns are READ by TradingConfigState (shared-rs)
-- and consumed by ConfigAwareEvaluator in prioritization-spine. They are NOT
-- yet exposed in the api-server PUT endpoint — that wiring lands in Sprint B
-- when the dashboard cost-breakdown UI is built.

BEGIN;

ALTER TABLE trading_config
    ADD COLUMN IF NOT EXISTS capital_cost_rate_annual_pct NUMERIC(8, 6) NOT NULL DEFAULT 0.0,
    ADD COLUMN IF NOT EXISTS ops_overhead_usd_per_attempt NUMERIC(10, 4) NOT NULL DEFAULT 0.01;

-- Sanity bounds: capital cost rate 0–50% APR; ops overhead $0–$100 per attempt.
-- Upper bounds are deliberately permissive to avoid blocking unusual deployments
-- while still catching obvious operator configuration errors.
ALTER TABLE trading_config
    DROP CONSTRAINT IF EXISTS trading_config_capital_cost_rate_check,
    ADD CONSTRAINT trading_config_capital_cost_rate_check
        CHECK (capital_cost_rate_annual_pct >= 0.0 AND capital_cost_rate_annual_pct <= 50.0);

ALTER TABLE trading_config
    DROP CONSTRAINT IF EXISTS trading_config_ops_overhead_check,
    ADD CONSTRAINT trading_config_ops_overhead_check
        CHECK (ops_overhead_usd_per_attempt >= 0.0 AND ops_overhead_usd_per_attempt <= 100.0);

-- Post-migration sanity report: every row that had a successful migration.
DO $$
DECLARE row_count INT;
BEGIN
    SELECT COUNT(*) INTO row_count FROM trading_config
        WHERE capital_cost_rate_annual_pct IS NOT NULL
          AND ops_overhead_usd_per_attempt IS NOT NULL;
    RAISE NOTICE 'Migration 047: % row(s) have new cost component columns populated (capital_cost_rate_annual_pct, ops_overhead_usd_per_attempt)', row_count;
END $$;

COMMIT;
