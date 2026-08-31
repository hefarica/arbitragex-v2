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

-- RERUN-LOCK-SAFETY (GEN-CI-FAIL 2026-08-30): `CREATE INDEX IF NOT EXISTS`
-- requests the table ShareLock BEFORE the existence check, and DROP/CREATE
-- TRIGGER take ShareRowExclusive. On the LIVE opportunities table (continuous
-- searcher INSERTs + purge DELETEs) the FREEZE-01 lockguard (lock_timeout=10s)
-- turns that wait into a deploy abort (observed: deploy of ac08da8b attempt 2,
-- [4/9] MIGRATION GATE FAILED at idx_opp_status_time). The guarded no-op path
-- below takes NO table lock: the IF reads catalogs only and EXECUTE runs only
-- when the object is genuinely absent (fresh boot = empty table = instant
-- build; a genuinely-missing index on a POPULATED table is served by a
-- dedicated CONCURRENTLY fixer, per the 105 pattern). Enforced by
-- automation/tools/lint-migration-rerun-lock-safety.sh.
DO $$
BEGIN
  IF NOT EXISTS (
    SELECT 1 FROM pg_indexes
    WHERE schemaname = 'public' AND indexname = 'idx_opp_status_time'
  ) THEN
    EXECUTE 'CREATE INDEX idx_opp_status_time ON opportunities(status, detected_at DESC)';
  END IF;
  IF NOT EXISTS (
    SELECT 1 FROM pg_indexes
    WHERE schemaname = 'public' AND indexname = 'idx_opp_chain_strategy'
  ) THEN
    EXECUTE 'CREATE INDEX idx_opp_chain_strategy ON opportunities(chain_id, strategy_kind)';
  END IF;
  IF NOT EXISTS (
    SELECT 1 FROM pg_indexes
    WHERE schemaname = 'public' AND indexname = 'idx_opp_trace'
  ) THEN
    EXECUTE 'CREATE INDEX idx_opp_trace ON opportunities(trace_id)';
  END IF;
END $$;

CREATE OR REPLACE FUNCTION arbx_touch_updated_at() RETURNS trigger AS $$
BEGIN NEW.updated_at := NOW(); RETURN NEW; END;
$$ LANGUAGE plpgsql;

DO $$
BEGIN
  IF NOT EXISTS (
    SELECT 1 FROM pg_trigger
    WHERE tgname = 'trg_opp_updated_at'
      AND tgrelid = 'public.opportunities'::regclass
  ) THEN
    EXECUTE 'CREATE TRIGGER trg_opp_updated_at BEFORE UPDATE ON opportunities FOR EACH ROW EXECUTE FUNCTION arbx_touch_updated_at()';
  END IF;
END $$;

GRANT SELECT, INSERT, UPDATE ON opportunities TO arbx_rw;
GRANT SELECT ON opportunities TO arbx_ro;
