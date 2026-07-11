-- Migration 100: paper_trade_runs execution_time_ms column (Task 6)
--
-- Adds execution_time_ms column for the Paper Executor to record
-- the latency of paper trade execution processing.
-- Required for OMEGA Pipeline Task 6: Paper Trade Execution Path

BEGIN;

ALTER TABLE paper_trade_runs ADD COLUMN IF NOT EXISTS execution_time_ms INTEGER;

-- Index for latency analysis and performance monitoring
CREATE INDEX IF NOT EXISTS idx_paper_trade_runs_exec_time
    ON paper_trade_runs(execution_time_ms)
    WHERE execution_time_ms IS NOT NULL;

DO $$
BEGIN
    RAISE NOTICE 'Migration 100: paper_trade_runs.execution_time_ms column ready for Paper Executor.';
END $$;

COMMIT;
