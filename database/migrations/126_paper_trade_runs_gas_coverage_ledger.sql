-- ArbitrageX v2 — Migration 126: paper_trade_runs gas-coverage ledger (GAS-COVERAGE-01)
--
-- PROBLEM (measured in the code, not assumed).
--   The A.6 actual-gas breaker (backend/api-server/src/routes/risk-circuit-breakers.ts,
--   gas_burn_breaker) publishes a coverage pair to the wire:
--       rowsInWindow  = COUNT(*)                                -> evidence.paths.actual.expected
--       withActualGas = COUNT(actual_gas_cost_usd)              -> evidence.paths.actual.measured
--   `expected` is therefore EVERY row of the window, while the only writer of
--   actual_gas_cost_usd is the recon drift-tracker
--   (backend/recon/src/drift_tracker.rs, one SQL text: `actual_gas_cost_usd = $3`
--   bound to `compute_gas_cost_usd(...)`). That population and that measurement
--   cannot converge, for two independent reasons:
--     (a) NO PRODUCER OF THE VALUE. `compute_gas_cost_usd` ends in `None`
--         unconditionally (drift_tracker.rs:452-461 — `let _ = eth; None`): the
--         ETH-USD feed is a documented MVP placeholder. Every resolved row is
--         written with actual_gas_cost_usd = NULL, so the numerator is 0 by
--         construction and the breaker can never leave NOT_AVAILABLE.
--     (b) THE DENOMINATOR HAS NO COMPLETION MARKER. A row can only be measured
--         if the drift-tracker's pending scan selects it (actual_timestamp IS NULL
--         AND calibration_eligible AND sim_attempts < max AND sim_block_number IS
--         NOT NULL AND opportunity_id IS NOT NULL AND route_metadata non-empty).
--         Rows failing any of those gates are not "pending measurement": they are
--         terminal. Economic/market rejects close with actual_profit_usd = 0 and
--         NO broadcast — they burn no gas at all, so gas is legitimately N/A, not
--         missing. Counting them as `expected` makes "partial coverage" a
--         permanent state that no amount of waiting can clear, and the operator
--         reads it as a stuck pipeline when it is a definition defect.
--
--   The frontend (frontend/features/risk/RiskCircuitPanel.tsx) already refuses to
--   call 0/0 "measured"; that fixes the LABEL. This migration fixes the
--   DENOMINATOR: it makes the per-row measurement state explicit so a coverage
--   denominator can be *defined* instead of inferred from a COUNT(*).
--
-- WHAT THIS FILE DOES (additive, nullable, idempotent, rerun-lock-safe):
--   1. `gas_measurement_state` — the explicit per-row measurement ledger, with a
--      closed vocabulary and a CHECK constraint. `unavailable` is the honest
--      "we tried and could not compute it" state and MUST NOT be reclassified to
--      `not_applicable` without evidence (that would hide the (a) defect).
--   2. `gas_measurement_updated_at` — when the classification was last written
--      (provenance for the state; NOT the settlement time — actual_timestamp owns
--      that).
--   3. `idx_ptr_chain_created_gas` (chain_id, created_at DESC) INCLUDE
--      (actual_gas_cost_usd) — the breaker's window aggregate currently has NO
--      supporting index: the existing leading-chain_id index is
--      (strategy_kind, chain_id, created_at DESC), which cannot serve the
--      created_at range without an equality on strategy_kind, so the query runs as
--      a seq scan on the 90-day ledger on every /risk poll.
--   4. `idx_ptr_gas_unmeasured` — partial index over the rows that are NOT yet
--      classified measured: the invariant query's population and the operator's
--      "what is left" scan.
--
-- NOT DONE HERE (owned by other surfaces, recorded so the defect is not hidden):
--   · The value producer (drift_tracker::compute_gas_cost_usd must convert
--     gas_used × gas_price_wei via an ETH-USD price — the Redis
--     `arbx:token_prices:<chain>:ETH` key the same file already reads for token
--     valuation — and return Some/None honestly).
--   · The READER change that uses this ledger as the denominator
--     (risk-circuit-breakers.ts) and the wire/contract change that lets the
--     API distinguish "no expected measurement" from "expected and missing".
--   This migration only makes the truth derivable in the database.
--
-- Ordering: 126 > 125 (lexical). The runner re-applies every file on every deploy
-- and skips a file whose sha256 already matches schema_migrations, so the backfill
-- below is written to converge on any state and is a no-op once converged.
-- RERUN-LOCK-SAFETY: paper_trade_runs is a HOT table (paper archiver writes
-- continuously) — every lock-taking DDL here is catalog-guarded or CONCURRENTLY
-- (automation/tools/lint-migration-rerun-lock-safety.sh,
-- automation/tools/lint-migration-index-locks.sh).

SET statement_timeout = '40min';

-- ============================================================================
-- 1. Columns (HOT TABLE -> DO block catalog-guarded: the no-op path takes no
--    table lock, so a re-run during a purge burst cannot abort the deploy).
--    Nullable on purpose: NULL = state not yet classified by the backfill or the
--    writer. NULL is NOT "not applicable" — absence is its own state (R8).
-- ============================================================================
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_schema = 'public' AND table_name = 'paper_trade_runs'
          AND column_name = 'gas_measurement_state'
    ) THEN
        EXECUTE $ddl$
            ALTER TABLE paper_trade_runs
                ADD COLUMN gas_measurement_state TEXT
                CHECK (gas_measurement_state IN (
                    'not_attempted',   -- drift-tracker has not evaluated this row yet
                    'measured',        -- actual_gas_cost_usd carries a real observation
                    'not_applicable',  -- settled without broadcast (economic/market reject): no gas burned
                    'unavailable',     -- evaluated, but the harness could not compute the value (BUG SURFACE)
                    'impossible'       -- structurally unresolvable row (never selected by the drift scan)
                ))
        $ddl$;
    END IF;

    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_schema = 'public' AND table_name = 'paper_trade_runs'
          AND column_name = 'gas_measurement_updated_at'
    ) THEN
        EXECUTE 'ALTER TABLE paper_trade_runs ADD COLUMN gas_measurement_updated_at TIMESTAMPTZ';
    END IF;
END $$;

-- ============================================================================
-- 2. Indexes (CONCURRENTLY: ShareUpdateExclusiveLock does not block the paper
--    archiver's INSERTs; the runner executes this file as plain psql script, so
--    CONCURRENTLY is legal and rerun is a catalog no-op).
-- ============================================================================

-- Window access path for the A.6 breaker aggregate:
--   WHERE chain_id = $1 AND created_at >= now() - make_interval(hours => $2)
--   COUNT(*), COUNT(actual_gas_cost_usd), SUM(actual_gas_cost_usd)
-- A covering index makes it an index-only scan and removes the seq scan.
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_ptr_chain_created_gas
    ON paper_trade_runs (chain_id, created_at DESC)
    INCLUDE (actual_gas_cost_usd);

-- The "what is left to measure / classify" population: everything not yet
-- measured. Kept partial so it stays small next to the measured majority.
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_ptr_gas_unmeasured
    ON paper_trade_runs (chain_id, created_at DESC)
    WHERE gas_measurement_state IS DISTINCT FROM 'measured';

-- ============================================================================
-- 3. Backfill — classify EVERY existing row from evidence the row itself holds.
--    Conservative by design: `unavailable` and `not_attempted` are the default
--    for the unresolved majority, so the migration NEVER converts a genuine hole
--    (defect (a)) into `not_applicable`. `not_applicable` is asserted ONLY for
--    rows the drift-tracker itself labelled as an economic/market rejection —
--    those close with actual_profit_usd = 0.0 and no broadcast (drift_tracker.rs
--    Attempt::NotPassed), so no gas was burned and there is nothing to measure.
--    `impossible` mirrors the drift-tracker pending scan's gates, i.e. rows that
--    no future tick can select (including `sim_attempts >= max`, the ladder's
--    exhausted state — kept in step with the same test in
--    scripts/paper-gas-coverage-audit.sh --apply so both paths converge).
--    Idempotent: the WHERE clause only touches rows still NULL; once converged
--    this statement matches 0 rows (the runner also skips the file entirely when
--    its checksum is unchanged).
--
--    `sim_attempts` threshold: the ladder's exhausted state (drift_tracker
--    ARBX_DRIFT_TRACKER_MAX_ATTEMPTS, runtime default 10 — drift_tracker.rs:64)
--    cannot be read from the DB, so the SQL text carries that default and an
--    operator can override it for a single run with
--    `SET arbx.gas_coverage_max_attempts = 'N';` before the file executes
--    (psql `-c`/PGOPTIONS). 10 IS A DECLARED PARAMETER, not a measurement: if the
--    runtime changes, pass the new value or re-run
--    scripts/paper-gas-coverage-audit.sh --apply --max-attempts N.
-- ============================================================================
BEGIN;

UPDATE paper_trade_runs
   SET gas_measurement_state = CASE
         WHEN actual_gas_cost_usd IS NOT NULL                       THEN 'measured'
         WHEN sim_fail_family IN ('economic', 'market')             THEN 'not_applicable'
         WHEN calibration_eligible IS FALSE                         THEN 'impossible'
         WHEN sim_block_number IS NULL                              THEN 'impossible'
         WHEN opportunity_id IS NULL                                THEN 'impossible'
         WHEN actual_timestamp IS NOT NULL                          THEN 'unavailable'
         WHEN sim_attempts >= COALESCE(
                NULLIF(current_setting('arbx.gas_coverage_max_attempts', true), '')::int, 10)
                                                                    THEN 'impossible'
         ELSE 'not_attempted'
       END,
       gas_measurement_updated_at = now()
 WHERE gas_measurement_state IS NULL;

COMMIT;

-- ============================================================================
-- 4. Invariants documented on the columns (read by the invariant script and by
--    operators; COMMENT is metadata-only, no table lock beyond the catalog).
-- ============================================================================
DO $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace
        WHERE n.nspname = 'public' AND c.relname = 'paper_trade_runs'
    ) THEN
        EXECUTE $c$
            COMMENT ON COLUMN paper_trade_runs.gas_measurement_state IS
            'GAS-COVERAGE-01 per-row state of the actual-gas measurement. not_attempted = drift scan has not evaluated it; measured = actual_gas_cost_usd is a real observation; not_applicable = settled WITHOUT broadcast (economic/market reject), no gas burned; unavailable = evaluated, value not computable (BUG SURFACE, never reclassify to not_applicable without evidence); impossible = structurally unresolvable by the drift scan (calibration_eligible=false, sim_block_number NULL, opportunity_id NULL after retention SET NULL). NULL = not yet classified (absence is a state, not a zero).'
        $c$;
        EXECUTE $c$
            COMMENT ON COLUMN paper_trade_runs.gas_measurement_updated_at IS
            'GAS-COVERAGE-01: when gas_measurement_state was last classified. Provenance of the STATE only — actual_timestamp remains the settlement time.'
        $c$;
    END IF;
END $$;

DO $$
BEGIN
    RAISE NOTICE 'Migration 126: paper_trade_runs gas-coverage ledger ready (state + 2 indexes + backfill).';
END $$;
