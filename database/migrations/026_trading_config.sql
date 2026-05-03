-- ArbitrageX v2 — Migration 026: trading_config (operator-tunable strategy parameters).
--
-- Doctrine: this is the OPERATOR DASHBOARD layer. Unlike `execution_policy` (015) which
-- holds versioned signed-off hard caps for capital-at-risk audit, this table holds the
-- LIVE TUNABLE parameters that shape every opportunity scoring decision. Mutated from
-- the /config/trading frontend page; mirrored to Redis (`arbx:trading_config:<chain>`)
-- and pushed via pub/sub so searcher-rs can react in <1s without restart.
--
-- Every field MUST come from the operator. No defaults that touch capital. No hardcoded
-- token allowlists. No assumed price for base token. The system stays idle for a chain
-- until the operator inserts a row.
--
-- Relationship to execution_policy:
--   execution_policy.max_value_eth          → hard ceiling (immutable per version)
--   trading_config.capital_usd              → operator's daily deployable target (soft)
--   ALWAYS effective = min(hard_cap, soft_target).

CREATE TABLE IF NOT EXISTS trading_config (
  id                          UUID         PRIMARY KEY DEFAULT gen_random_uuid(),
  chain_id                    INTEGER      NOT NULL,

  -- ── Capital & token universe ─────────────────────────────────────────────
  capital_usd                 NUMERIC(20,2) NOT NULL CHECK (capital_usd >= 0),
  base_token_symbol           TEXT         NOT NULL,
  base_token_price_usd        NUMERIC(20,4) NOT NULL CHECK (base_token_price_usd > 0),
  allowed_token_symbols       TEXT[]       NOT NULL DEFAULT '{}',

  -- ── Profit gate thresholds ───────────────────────────────────────────────
  min_profit_usd              NUMERIC(20,4) NOT NULL CHECK (min_profit_usd >= 0),
  min_roi_pct                 NUMERIC(8,4)  NOT NULL CHECK (min_roi_pct >= 0),
  min_landing_probability     NUMERIC(4,3)  NOT NULL DEFAULT 0.500
                                            CHECK (min_landing_probability >= 0 AND min_landing_probability <= 1),
  min_liquidity_confidence    NUMERIC(4,3)  NOT NULL DEFAULT 0.700
                                            CHECK (min_liquidity_confidence >= 0 AND min_liquidity_confidence <= 1),
  max_token_risk_score        NUMERIC(4,3)  NOT NULL DEFAULT 1.000
                                            CHECK (max_token_risk_score >= 0 AND max_token_risk_score <= 1),

  -- ── Gas & slippage strategy ──────────────────────────────────────────────
  gas_price_strategy          TEXT         NOT NULL DEFAULT 'dynamic_basefee_plus_tip'
                                            CHECK (gas_price_strategy IN ('fixed','dynamic_basefee_plus_tip','percentile_75')),
  fixed_gas_price_gwei        NUMERIC(12,2),
  gas_estimate_units          INTEGER      NOT NULL DEFAULT 250000 CHECK (gas_estimate_units > 0),
  max_slippage_pct            NUMERIC(6,4) NOT NULL CHECK (max_slippage_pct >= 0 AND max_slippage_pct <= 50),
  failure_risk_buffer_pct     NUMERIC(6,4) NOT NULL DEFAULT 0.0010 CHECK (failure_risk_buffer_pct >= 0),
  flashloan_fee_pct           NUMERIC(6,4) NOT NULL DEFAULT 0.0009 CHECK (flashloan_fee_pct >= 0),

  -- ── Strategy mix (polymorphic; operator enables which classes run) ───────
  enabled_strategies          TEXT[]       NOT NULL DEFAULT '{}',
  -- Examples: 'dex_arb_v2v2', 'dex_arb_v2v3', 'triangular', 'cross_dex',
  --           'flashloan_arb', 'cex_dex' — searcher routes candidates by class.

  -- ── Lifecycle ────────────────────────────────────────────────────────────
  enabled                     BOOLEAN      NOT NULL DEFAULT TRUE,
  updated_by                  TEXT         NOT NULL,
  updated_at                  TIMESTAMPTZ  NOT NULL DEFAULT NOW(),
  created_at                  TIMESTAMPTZ  NOT NULL DEFAULT NOW(),

  CONSTRAINT trading_config_one_per_chain UNIQUE (chain_id),
  CONSTRAINT trading_config_fixed_requires_price
    CHECK (gas_price_strategy <> 'fixed' OR fixed_gas_price_gwei IS NOT NULL)
);

CREATE INDEX IF NOT EXISTS idx_trading_config_chain
  ON trading_config(chain_id) WHERE enabled = TRUE;

DROP TRIGGER IF EXISTS trg_trading_config_updated_at ON trading_config;
CREATE TRIGGER trg_trading_config_updated_at BEFORE UPDATE ON trading_config
  FOR EACH ROW EXECUTE FUNCTION arbx_touch_updated_at();

GRANT SELECT, INSERT, UPDATE ON trading_config TO arbx_rw;
GRANT SELECT ON trading_config TO arbx_ro;
