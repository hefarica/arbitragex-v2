-- ArbitrageX v2 — Migration 003: opportunities
-- Central table: every detected opportunity flows through this, state machine via `status`.

CREATE TABLE IF NOT EXISTS opportunities (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  chain_id INTEGER NOT NULL,
  strategy_kind TEXT NOT NULL CHECK (strategy_kind IN (
    'dex_arb','triangular','backrun','liquidation','flashloan_arb'
  )),
  dex_a TEXT NOT NULL,
  dex_b TEXT,
  pair_symbol TEXT,
  token_in TEXT NOT NULL,
  token_out TEXT NOT NULL,
  amount_in_wei NUMERIC(78,0) NOT NULL,
  expected_profit_usd NUMERIC(20,8),
  roi_pct NUMERIC(10,4),
  risk_score NUMERIC(10,4),
  block_number BIGINT,
  status TEXT NOT NULL DEFAULT 'detected' CHECK (status IN (
    'detected','validated','simulated','scored','executing',
    'executed','reconciled','rejected','failed'
  )),
  rejection_reason TEXT,
  trace_id UUID NOT NULL,
  detected_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- FREEZE-01/02: these indexes exist since boot; run_migrations.sh re-applies
-- every file on EVERY deploy, so a plain `CREATE INDEX IF NOT EXISTS` still
-- queues ShareLock against the table (even when the index already exists) —
-- behind a retention purge that starves every INSERT (PG fair lock queue; two
-- incidents 2026-08-17). CONCURRENTLY takes ShareUpdateExclusiveLock, which
-- does NOT conflict with the hot path's RowExclusiveLock (INSERT/DELETE).
-- Requires autocommit (no BEGIN block) — the runner pipes without -1. ✓
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_opp_status_time ON opportunities(status, detected_at DESC);
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_opp_chain_strategy ON opportunities(chain_id, strategy_kind);
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_opp_trace ON opportunities(trace_id);

CREATE OR REPLACE FUNCTION arbx_touch_updated_at() RETURNS trigger AS $$
BEGIN NEW.updated_at := NOW(); RETURN NEW; END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_opp_updated_at ON opportunities;
CREATE TRIGGER trg_opp_updated_at BEFORE UPDATE ON opportunities
  FOR EACH ROW EXECUTE FUNCTION arbx_touch_updated_at();

GRANT SELECT, INSERT, UPDATE ON opportunities TO arbx_rw;
GRANT SELECT ON opportunities TO arbx_ro;
