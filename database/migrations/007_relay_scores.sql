-- ArbitrageX v2 — Migration 007: relay_scores
-- Per-relay performance ledger; used by selector-api to route bundles preferentially.

CREATE TABLE IF NOT EXISTS relay_scores (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  relay_name TEXT NOT NULL,
  chain_id INTEGER NOT NULL,
  window_start TIMESTAMPTZ NOT NULL,
  window_end TIMESTAMPTZ NOT NULL,
  submitted INTEGER NOT NULL DEFAULT 0,
  included INTEGER NOT NULL DEFAULT 0,
  reverted INTEGER NOT NULL DEFAULT 0,
  dropped INTEGER NOT NULL DEFAULT 0,
  inclusion_rate NUMERIC(10,6),
  avg_latency_ms NUMERIC(12,2),
  score NUMERIC(10,4),
  enabled BOOLEAN NOT NULL DEFAULT TRUE,
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  CONSTRAINT relay_scores_uq UNIQUE (relay_name, chain_id, window_end)
);

CREATE INDEX IF NOT EXISTS idx_relay_recent ON relay_scores(chain_id, relay_name, window_end DESC);

GRANT SELECT, INSERT, UPDATE ON relay_scores TO arbx_rw;
GRANT SELECT ON relay_scores TO arbx_ro;
