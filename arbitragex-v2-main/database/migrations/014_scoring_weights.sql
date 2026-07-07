-- ArbitrageX v2 — Migration 014: scoring_weights (operator-editable, versioned).
--
-- No-hardcode doctrine: scoring weights are business decisions that drive which
-- opportunities get executed. They MUST be explicit, audited, and rotatable
-- without a code deploy.
--
-- Columns:
--   version            — monotonic, human-curated. Only the latest version with
--                        effective_at <= now() AND (superseded_at IS NULL OR > now()) is active.
--   weight_*           — component weights (non-negative; sum validated by CHECK).
--   min_accept_score   — threshold below which opportunities are rejected.
--   description        — why this set exists. Required for audit.
--   activated_by       — operator (onboarding/admin user) who wrote this row.
--   effective_at       — when this set starts applying.
--   superseded_at      — when a newer row replaced this one (NULL while active).
--
-- Only one row per chain_id may be "active" at any time — enforced by app logic
-- (transactional UPDATE on the old row + INSERT of new one).

CREATE TABLE IF NOT EXISTS scoring_weights (
  id                  UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
  chain_id            INTEGER     NOT NULL,
  version             INTEGER     NOT NULL,
  weight_liquidity    NUMERIC(6,4) NOT NULL CHECK (weight_liquidity >= 0 AND weight_liquidity <= 1),
  weight_depth        NUMERIC(6,4) NOT NULL CHECK (weight_depth     >= 0 AND weight_depth     <= 1),
  weight_safety       NUMERIC(6,4) NOT NULL CHECK (weight_safety    >= 0 AND weight_safety    <= 1),
  weight_slippage     NUMERIC(6,4) NOT NULL CHECK (weight_slippage  >= 0 AND weight_slippage  <= 1),
  weight_gas          NUMERIC(6,4) NOT NULL CHECK (weight_gas       >= 0 AND weight_gas       <= 1),
  weight_risk         NUMERIC(6,4) NOT NULL CHECK (weight_risk      >= 0 AND weight_risk      <= 1),
  min_accept_score    NUMERIC(6,2) NOT NULL CHECK (min_accept_score >= 0 AND min_accept_score <= 100),
  description         TEXT        NOT NULL,
  activated_by        TEXT        NOT NULL,
  effective_at        TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  superseded_at       TIMESTAMPTZ,
  created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  CONSTRAINT scoring_weights_uq UNIQUE (chain_id, version),
  -- tolerance: 0.999..1.001 covers rounding
  CONSTRAINT scoring_weights_sum_close_to_one CHECK (
    abs(weight_liquidity + weight_depth + weight_safety + weight_slippage + weight_gas + weight_risk - 1.0) < 0.005
  )
);

CREATE INDEX IF NOT EXISTS idx_scoring_weights_active
  ON scoring_weights(chain_id, effective_at DESC)
  WHERE superseded_at IS NULL;

GRANT SELECT, INSERT, UPDATE ON scoring_weights TO arbx_rw;
GRANT SELECT ON scoring_weights TO arbx_ro;
