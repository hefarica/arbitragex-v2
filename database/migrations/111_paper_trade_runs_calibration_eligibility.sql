-- ArbitrageX v2 — Migration 111: paper_trade_runs calibration eligibility (S4-03)
--
-- S4 "simulation label quality": Stage 2b calibration must never consume a
-- label produced by a BROKEN FIXTURE. A sim that fails because the signer
-- holds no token_in (TRANSFER_FROM_FAILED), the fork is unreachable, or the
-- gas oracle is absent says nothing about the strategy — feeding it to
-- calibration as "strategy lost" poisons the priors (S4-03 no-contamination
-- gate, runbook accepted 2026-08-29).
--
-- Columns (writer: recon drift-tracker):
--   sim_fail_family      NULL = not attempted / passed;
--                        'structural' | 'economic' | 'market' (S4-02 taxonomy,
--                        shared_rs::sim_taxonomy::classify_fail_reason).
--   calibration_eligible TRUE  = the row MAY feed Stage 2b when a label lands
--                                (PASS or ECONOMIC/MARKET reject — the market
--                                rejected it at the settled block: Y = loss).
--                        FALSE = terminal structural failure. No label, no
--                                retry (retrying a broken fixture fixes
--                                nothing), excluded from the pending scan.
--                        DEFAULT TRUE: historical rows are eligible; Stage 2b
--                        only consumes rows with actual_* labels anyway, and a
--                        row only ever flips to FALSE via a classified
--                        structural attempt.
--   sim_attempts         drift-tracker attempt count (pending backoff).
--   sim_last_attempt_at  last attempt timestamp (pending backoff).
--
-- Backoff (drift-tracker tick scan): a PENDING row (501 / parse error /
-- backend not configured) is retried at 30s * 2^min(attempts,7), capped at
-- ~64min, until ARBX_DRIFT_TRACKER_MAX_ATTEMPTS (default 10). A STRUCTURAL
-- row is terminal: calibration_eligible=FALSE removes it from the scan.
-- An ECONOMIC/MARKET reject is terminal WITH a label: actual_timestamp is set
-- (resolved), actual_profit_usd = 0 (exactly zero — the realized yield of a
-- rejected execution), sim_fail_family records why.

ALTER TABLE paper_trade_runs
  ADD COLUMN IF NOT EXISTS sim_fail_family TEXT
    CHECK (sim_fail_family IN ('structural', 'economic', 'market')),
  ADD COLUMN IF NOT EXISTS calibration_eligible BOOLEAN NOT NULL DEFAULT TRUE,
  ADD COLUMN IF NOT EXISTS sim_attempts INTEGER NOT NULL DEFAULT 0,
  ADD COLUMN IF NOT EXISTS sim_last_attempt_at TIMESTAMPTZ;

-- Pending-scan index: the drift-tracker tick selects unresolved, still-eligible
-- rows ordered by created_at. Partial index keeps it tiny as the table grows.
CREATE INDEX IF NOT EXISTS idx_ptr_pending_calibration
  ON paper_trade_runs (created_at)
  WHERE actual_timestamp IS NULL AND calibration_eligible;

COMMENT ON COLUMN paper_trade_runs.sim_fail_family IS
  'S4-02 taxonomy of the sim attempt: structural|economic|market (NULL = not attempted or passed)';
COMMENT ON COLUMN paper_trade_runs.calibration_eligible IS
  'S4-03 gate: FALSE = terminal structural sim failure, never a calibration label';
COMMENT ON COLUMN paper_trade_runs.sim_attempts IS 'drift-tracker attempt count for backoff';
