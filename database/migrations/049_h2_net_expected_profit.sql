-- Migration 049: H2 mainnet landmine — add net_expected_profit_usd column
--
-- Problem: Opportunity.expected_profit_usd carries WORKER-LEVEL GROSS profit
-- (set at scanner.rs:875 as math_outcome.gross_profit_usd). The spine evaluator
-- computes real net profit via calc_net_profit_and_roi and stores it in
-- OpportunityEvidence.net_expected_profit, but never writes it back to the
-- opportunities row. submit_engine Check 7 therefore gates on gross, not net.
--
-- Fix: Add net_expected_profit_usd column. Spine evaluator populates it.
-- submit_engine reads it with fallback to gross for pre-spine rows (R8 fail-honest).
--
-- Backfill: existing rows get NULL — their net profit is unknown without
-- re-running the evaluator. NULL is honest; imputing a value would violate R8.

BEGIN;

-- RERUN-LOCK-SAFETY (GEN-CI-FAIL 2026-08-30): ALTER TABLE takes
-- AccessExclusiveLock on the hot opportunities table before the IF NOT
-- EXISTS check. Catalog-guarded no-op path takes no table lock
-- (lint-migration-rerun-lock-safety.sh).
DO $$
BEGIN
  IF NOT EXISTS (
    SELECT 1 FROM information_schema.columns
    WHERE table_schema = 'public' AND table_name = 'opportunities'
      AND column_name = 'net_expected_profit_usd'
  ) THEN
    EXECUTE 'ALTER TABLE opportunities ADD COLUMN net_expected_profit_usd NUMERIC(18, 6) NULL';
  END IF;
END $$;

-- No backfill: pre-existing rows retain NULL (R8 fail-honest).
-- New rows will have the column populated by the spine evaluator path.

COMMIT;
