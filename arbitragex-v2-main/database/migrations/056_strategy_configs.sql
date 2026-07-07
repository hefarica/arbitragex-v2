-- ArbitrageX v2 — Migration 056: per-strategy fine-grained config (Decision A — JSONB).
--
-- Adds `trading_config.strategy_configs JSONB`. Each entry, keyed by
-- `strategy_kind` string (e.g. "dex_arb_v2v2", "triangular", "flashloan_arb",
-- "liquidation", "cex_dex"), carries the operator's runtime config for that
-- strategy: enabled flag, allowed chains, allowed DEX UUIDs, allowed protocol
-- types, profit/ROI/slippage/gas thresholds, simulation capital cap, route
-- constraints (min/max legs, atomicity, allowed base/quote tokens, min
-- pool TVL/volume), token allowlist override.
--
-- Why JSONB (Decision A from 2026-05-10 brainstorming):
--   The rest of trading_config is a row-per-chain hot-mirrored to Redis on
--   every PUT. A normalised side-table would force a JOIN in the hot-path
--   (api-server's rowToRedisState) and double the source of truth. JSONB
--   fits the existing pattern (token_prices_usd, simulation_per_*_caps_usd
--   are already JSONB), serialises atomically to Redis, and the GIN index
--   keeps audit queries efficient.
--
-- CHECK constraint:
--   Defensive: enforces that the JSON is a top-level object, not an array
--   or scalar. Without this, an upstream Zod miswire could persist a list
--   and the Rust deserialiser would silently drop it (the same schema-drift
--   class as the original enabled_dex_ids bug — see anti_reincidencia.md).
--
-- Redis projection (api-server's rowToRedisState):
--   strategy_configs: Record<strategy_kind, StrategyRuntimeConfig>
--   serialised verbatim into the Redis JSON the Rust struct deserialises.
--
-- Reversibility:
--   ALTER TABLE trading_config DROP COLUMN IF EXISTS strategy_configs;
--   DROP INDEX IF EXISTS idx_trading_config_strategy_configs;

BEGIN;

ALTER TABLE trading_config
  ADD COLUMN IF NOT EXISTS strategy_configs JSONB NOT NULL DEFAULT '{}'::jsonb;

ALTER TABLE trading_config
  ADD CONSTRAINT chk_strategy_configs_is_object
  CHECK (jsonb_typeof(strategy_configs) = 'object');

CREATE INDEX IF NOT EXISTS idx_trading_config_strategy_configs
  ON trading_config USING GIN (strategy_configs);

COMMIT;
