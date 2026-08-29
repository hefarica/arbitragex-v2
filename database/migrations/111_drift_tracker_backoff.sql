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
-- FREEZE-01 / ANTI-FREEZE FASE 2.2 (lint-migration-index-locks):
-- paper_trade_runs is an EXISTING populated table (paper history, ~580K rows),
-- so the partial index below is built CONCURRENTLY — a plain build would take
-- a lock that blocks INSERTs for the whole build (the 21h FREEZE-01 shape).
-- CONCURRENTLY cannot run inside a transaction block, which is why this file
-- is deliberately NOT wrapped in BEGIN/COMMIT (house pattern: migration 105).
-- The ADD COLUMNs are metadata-only (constant DEFAULT, no table rewrite) and
-- each autocommits under its own brief lock. statement_timeout is raised for
-- the build only; lock_timeout is NOT raised (CONCURRENTLY takes only SHARE
-- locks that don't conflict with INSERTs).
-- IF the CONCURRENT build ever fails midway it leaves an INVALID index:
-- recovery is  DROP INDEX CONCURRENTLY IF EXISTS idx_paper_trade_runs_next_attempt;
-- followed by re-running this file (documented here, not automated).
--
-- Idempotent.

ALTER TABLE paper_trade_runs
    ADD COLUMN IF NOT EXISTS actual_attempt_count   INT NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS actual_next_attempt_at TIMESTAMPTZ;

SET statement_timeout = '10min';

CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_paper_trade_runs_next_attempt
    ON paper_trade_runs (actual_next_attempt_at)
    WHERE actual_next_attempt_at IS NOT NULL;

RESET statement_timeout;

COMMENT ON COLUMN paper_trade_runs.actual_attempt_count IS
    'Stage 2a drift-tracker: consecutive unresolved re-execution attempts for this row (backoff exponent input).';
COMMENT ON COLUMN paper_trade_runs.actual_next_attempt_at IS
    'Stage 2a drift-tracker: earliest instant this pending row re-enters the resolution batch (exponential backoff). NULL = eligible now.';
