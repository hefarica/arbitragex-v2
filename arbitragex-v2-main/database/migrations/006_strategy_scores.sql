-- ArbitrageX v2 — Migration 006: strategy_scores
-- Adaptive scoring of strategies per (kind, chain, window). Populated by the learning loop (S6).

CREATE TABLE IF NOT EXISTS strategy_scores (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  strategy_kind TEXT NOT NULL,
  chain_id INTEGER NOT NULL,
  window_start TIMESTAMPTZ NOT NULL,
  window_end TIMESTAMPTZ NOT NULL,
  sample_count INTEGER NOT NULL,
  success_rate NUMERIC(10,6),
  avg_profit_usd NUMERIC(20,8),
  revert_rate NUMERIC(10,6),
  score NUMERIC(10,4),
  enabled BOOLEAN NOT NULL DEFAULT TRUE,
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  CONSTRAINT strategy_scores_uq UNIQUE (strategy_kind, chain_id, window_end)
);

CREATE INDEX IF NOT EXISTS idx_strat_recent ON strategy_scores(chain_id, strategy_kind, window_end DESC);

GRANT SELECT, INSERT, UPDATE ON strategy_scores TO arbx_rw;
GRANT SELECT ON strategy_scores TO arbx_ro;
