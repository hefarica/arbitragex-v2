-- ArbitrageX v2 — Migration 032: trading_config simulation knobs + token_prices_usd.
--
-- Adds 6 nullable/JSONB columns to `trading_config` to back the operator-tunable
-- simulation knobs introduced in commit 0855010 (BUG-2 PriceOracle) and e2540c5
-- (per-token + per-strategy caps + UI target filters).
--
-- All columns are NULLABLE / DEFAULT empty so existing rows continue to work
-- without backfill — the Rust spine treats absent values as "use operational
-- capital_usd" via `effective_capital_for(token, strategy)`.
--
-- Doctrine: paper-trade preview (ARBX_PAPER_TRADE=true default) ensures these
-- knobs NEVER trigger on-chain execution. Pure visualization layer.
--   token_prices_usd                       → BUG-2 fix (per-token USD oracle)
--   simulation_capital_usd                 → global sim cap (override capital_usd)
--   simulation_per_token_amounts_usd       → per-token-input sim cap
--   simulation_per_strategy_caps_usd       → per-strategy sim cap
--   simulation_target_profit_usd           → UI filter (frontend-consumed)
--   simulation_target_roi_pct              → UI filter (frontend-consumed)

ALTER TABLE trading_config
  ADD COLUMN IF NOT EXISTS token_prices_usd                  JSONB         NOT NULL DEFAULT '{}'::jsonb,
  ADD COLUMN IF NOT EXISTS simulation_capital_usd            NUMERIC(20,2) NULL,
  ADD COLUMN IF NOT EXISTS simulation_per_token_amounts_usd  JSONB         NOT NULL DEFAULT '{}'::jsonb,
  ADD COLUMN IF NOT EXISTS simulation_per_strategy_caps_usd  JSONB         NOT NULL DEFAULT '{}'::jsonb,
  ADD COLUMN IF NOT EXISTS simulation_target_profit_usd      NUMERIC(20,4) NULL,
  ADD COLUMN IF NOT EXISTS simulation_target_roi_pct         NUMERIC(8,4)  NULL;

-- Sanity bounds: positive values only when present.
--
-- BUG-FIX iter 12: PostgreSQL does NOT support `ADD CONSTRAINT IF NOT EXISTS`
-- (unlike `ADD COLUMN IF NOT EXISTS` which IS supported). The previous form
-- raised `ERROR: syntax error at or near "NOT"` on first apply and silently
-- never ran in CI before iter 11 because no workflow invoked migrate.sh.
--
-- We use the same `DO $$ ... duplicate_object` pattern already adopted by
-- 001_roles.sql for idempotent role creation. This preserves the original
-- intent (re-runnable migration) using PostgreSQL-native syntax.
DO $$ BEGIN
  ALTER TABLE trading_config
    ADD CONSTRAINT trading_config_simulation_capital_positive
      CHECK (simulation_capital_usd IS NULL OR simulation_capital_usd > 0);
EXCEPTION WHEN duplicate_object THEN NULL; END $$;

DO $$ BEGIN
  ALTER TABLE trading_config
    ADD CONSTRAINT trading_config_simulation_target_profit_nonneg
      CHECK (simulation_target_profit_usd IS NULL OR simulation_target_profit_usd >= 0);
EXCEPTION WHEN duplicate_object THEN NULL; END $$;

DO $$ BEGIN
  ALTER TABLE trading_config
    ADD CONSTRAINT trading_config_simulation_target_roi_nonneg
      CHECK (simulation_target_roi_pct IS NULL OR simulation_target_roi_pct >= 0);
EXCEPTION WHEN duplicate_object THEN NULL; END $$;

COMMENT ON COLUMN trading_config.token_prices_usd
  IS 'Per-token USD prices keyed by symbol (case-insensitive at lookup). Operator override of the price-oracle Tier 2; consumed by shared_rs::price_oracle::ConfigPriceOracle.';
COMMENT ON COLUMN trading_config.simulation_capital_usd
  IS 'Global simulation cap (USD). When set, overrides operational capital_usd in the spine evaluator. Paper-trade preview only — does not affect on-chain execution.';
COMMENT ON COLUMN trading_config.simulation_per_token_amounts_usd
  IS 'Per-token-input simulation cap (USD), keyed by symbol. MIN with global + per-strategy caps.';
COMMENT ON COLUMN trading_config.simulation_per_strategy_caps_usd
  IS 'Per-strategy simulation cap (USD), keyed by strategy_kind. MIN with global + per-token caps.';
COMMENT ON COLUMN trading_config.simulation_target_profit_usd
  IS 'UI-only filter: minimum profit (USD) the operator wants surfaced in the dashboard. Backend persists everything (R8 Fail-Honest).';
COMMENT ON COLUMN trading_config.simulation_target_roi_pct
  IS 'UI-only filter: minimum ROI percent. Same semantics as simulation_target_profit_usd.';
