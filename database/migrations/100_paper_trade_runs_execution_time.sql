-- Migration 100: paper_trade_runs execution_time_ms column (Task 6)
--
-- Adds execution_time_ms column for the Paper Executor to record
-- the latency of paper trade execution processing.
-- Required for OMEGA Pipeline Task 6: Paper Trade Execution Path

BEGIN;

-- RERUN-LOCK-SAFETY (GEN-CI-FAIL 2026-08-30): paper_trade_runs is written
-- continuously by the paper archiver; bare ALTER TABLE ... IF NOT EXISTS and
-- CREATE INDEX IF NOT EXISTS take writer-conflicting table locks before the
-- existence checks. Catalog-guarded no-op path takes no table lock
-- (lint-migration-rerun-lock-safety.sh).
DO $$
BEGIN
  IF NOT EXISTS (
    SELECT 1 FROM information_schema.columns
    WHERE table_schema = 'public' AND table_name = 'paper_trade_runs'
      AND column_name = 'execution_time_ms'
  ) THEN
    EXECUTE 'ALTER TABLE paper_trade_runs ADD COLUMN execution_time_ms INTEGER';
  END IF;
  IF NOT EXISTS (
    SELECT 1 FROM pg_indexes
    WHERE schemaname = 'public' AND indexname = 'idx_paper_trade_runs_exec_time'
  ) THEN
    EXECUTE 'CREATE INDEX idx_paper_trade_runs_exec_time ON paper_trade_runs(execution_time_ms) WHERE execution_time_ms IS NOT NULL';
  END IF;
END $$;

DO $$
BEGIN
    RAISE NOTICE 'Migration 100: paper_trade_runs.execution_time_ms column ready for Paper Executor.';
END $$;

COMMIT;
