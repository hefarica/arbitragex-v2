-- ArbitrageX v2 — Migration 005: executions
-- Submitted / included / reverted txs via private relays.

CREATE TABLE IF NOT EXISTS executions (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  opportunity_id UUID NOT NULL REFERENCES opportunities(id) ON DELETE CASCADE,
  simulation_id UUID REFERENCES simulations(id),
  relay_name TEXT NOT NULL,
  tx_hash TEXT UNIQUE,
  bundle_hash TEXT,
  block_included BIGINT,
  expected_profit_usd NUMERIC(20,8),
  actual_profit_usd NUMERIC(20,8),
  gas_used_wei NUMERIC(78,0),
  gas_price_effective_wei NUMERIC(78,0),
  status TEXT NOT NULL CHECK (status IN (
    'submitted','included','reverted','dropped','replaced','not_implemented'
  )),
  error_message TEXT,
  raw_receipt JSONB,
  trace_id UUID NOT NULL,
  submitted_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  confirmed_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_exec_opp ON executions(opportunity_id);
CREATE INDEX IF NOT EXISTS idx_exec_status_time ON executions(status, submitted_at DESC);
CREATE INDEX IF NOT EXISTS idx_exec_relay_time ON executions(relay_name, submitted_at DESC);
CREATE INDEX IF NOT EXISTS idx_exec_tx ON executions(tx_hash) WHERE tx_hash IS NOT NULL;

GRANT SELECT, INSERT, UPDATE ON executions TO arbx_rw;
GRANT SELECT ON executions TO arbx_ro;
