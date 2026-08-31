-- ArbitrageX v2 — Migration 004: simulations
-- One simulation per attempt; an opportunity may have multiple (e.g., retried with tweaked params).

CREATE TABLE IF NOT EXISTS simulations (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  opportunity_id UUID NOT NULL REFERENCES opportunities(id) ON DELETE CASCADE,
  simulator TEXT NOT NULL CHECK (simulator IN ('anvil','tenderly','hardhat','not_implemented')),
  gas_estimate_wei NUMERIC(78,0),
  gas_price_wei NUMERIC(78,0),
  slippage_pct NUMERIC(10,4),
  revert_risk_pct NUMERIC(10,4),
  simulated_profit_usd NUMERIC(20,8),
  passed BOOLEAN NOT NULL DEFAULT FALSE,
  fail_reason TEXT,
  raw_trace JSONB,
  trace_id UUID NOT NULL,
  simulated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- RERUN-LOCK-SAFETY (GEN-CI-FAIL 2026-08-30): simulations is written
-- continuously by sim-ctl post-SIMWIRE-02c; bare CREATE INDEX IF NOT EXISTS
-- takes ShareLock before the existence check and starves under the runner's
-- lock_timeout=10s. Catalog-guarded no-op path takes no table lock
-- (lint-migration-rerun-lock-safety.sh).
DO $$
BEGIN
  IF NOT EXISTS (
    SELECT 1 FROM pg_indexes
    WHERE schemaname = 'public' AND indexname = 'idx_sim_opp'
  ) THEN
    EXECUTE 'CREATE INDEX idx_sim_opp ON simulations(opportunity_id)';
  END IF;
  IF NOT EXISTS (
    SELECT 1 FROM pg_indexes
    WHERE schemaname = 'public' AND indexname = 'idx_sim_passed_time'
  ) THEN
    EXECUTE 'CREATE INDEX idx_sim_passed_time ON simulations(passed, simulated_at DESC)';
  END IF;
END $$;

GRANT SELECT, INSERT ON simulations TO arbx_rw;
GRANT SELECT ON simulations TO arbx_ro;
