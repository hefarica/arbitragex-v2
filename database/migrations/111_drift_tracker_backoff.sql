-- ArbitrageX v2 — Migration 111: drift-tracker attempt backoff (Stage 2a hardening)
--
-- The drift-tracker's pending query is `WHERE actual_timestamp IS NULL … ORDER BY
-- created_at LIMIT $batch`. Rows that can NEVER resolve (sim-ctl 501 while the
-- B2c executor path is unwired, settled block pruned from the fork, transient
-- parse gaps) stay NULL forever — and because the batch is ordered oldest-first,
-- a head of unresolvable rows STARVES every newer row: zero labels even with the
-- tracker ON.
--
-- This migration adds the re-enqueue-with-backoff surface the tracker defers
-- unresolvable rows onto:
--   - actual_attempt_count: consecutive unresolved attempts (monotonic).
--   - actual_next_attempt_at: earliest instant the row re-enters the batch.
--     Exponential backoff is computed by the tracker (base·2^attempts, capped);
--     NULL = never deferred (fresh row, eligible immediately).
--
-- Honesty (RULE 00 / R8): a deferred row is PENDING, not failed — actual_* stays
-- NULL until a passing re-execution labels it. Backoff only paces retries.
--
-- Idempotent.

BEGIN;

ALTER TABLE paper_trade_runs
    ADD COLUMN IF NOT EXISTS actual_attempt_count   INT NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS actual_next_attempt_at TIMESTAMPTZ;

CREATE INDEX IF NOT EXISTS idx_paper_trade_runs_next_attempt
    ON paper_trade_runs (actual_next_attempt_at)
    WHERE actual_next_attempt_at IS NOT NULL;

COMMENT ON COLUMN paper_trade_runs.actual_attempt_count IS
    'Stage 2a drift-tracker: consecutive unresolved re-execution attempts for this row (backoff exponent input).';
COMMENT ON COLUMN paper_trade_runs.actual_next_attempt_at IS
    'Stage 2a drift-tracker: earliest instant this pending row re-enters the resolution batch (exponential backoff). NULL = eligible now.';

COMMIT;
