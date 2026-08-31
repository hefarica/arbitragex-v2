-- ============================================================================
-- Migration 054: DB schema audit fixes (audit M11, 2026-05-10)
--
-- 1. GRANTs: review per-table permissions for arbx_rw / arbx_ro / arbx_migrator
-- 2. Foreign keys: add ON DELETE / ON UPDATE clauses to prevent dangling refs
-- 3. Precision: financial NUMERIC columns must be high enough precision
-- 4. Indexes: cover hot path queries that 050+051 missed
--
-- REVERSIBILITY: see rollback block at bottom (commented out, activate manually).
--
-- IDEMPOTENCY: all DDL uses IF NOT EXISTS / DROP IF EXISTS / DO $$ with
-- EXCEPTION handlers. Safe to re-run on partial or complete schemas.
-- ============================================================================

BEGIN;

-- =======================
-- Section 1: GRANTs review
-- =======================
-- audit_log is a tamper-evident forensic trail.
-- arbx_ro: SELECT only (already enforced since migration 011/019, guarded here).
-- arbx_rw: SELECT + INSERT only (no UPDATE/DELETE). Already enforced since
--           migration 011 REVOKE. These blocks are defense-in-depth guards that
--           re-assert the constraint in case a future migration accidentally
--           re-grants broader privileges.

-- Guard 1: arbx_ro must not have INSERT/UPDATE/DELETE/TRUNCATE on audit_log.
-- PostgreSQL REVOKE of a privilege not held is a silent no-op — no error raised.
-- The EXCEPTION block catches the case where the role itself does not exist
-- (error code 42704 = undefined_object) or the table is absent (42P01).
DO $$
BEGIN
    EXECUTE 'REVOKE INSERT, UPDATE, DELETE, TRUNCATE ON audit_log FROM arbx_ro';
EXCEPTION
    WHEN undefined_object OR undefined_table THEN
        RAISE NOTICE 'Section 1: audit_log or arbx_ro role not found — REVOKE skipped';
END $$;

-- Guard 2: arbx_rw must not have UPDATE/DELETE/TRUNCATE on audit_log.
-- Migration 011/019 already issued this REVOKE; this is a belt-and-suspenders
-- re-assertion so the permission is verifiable at every schema version.
DO $$
BEGIN
    EXECUTE 'REVOKE UPDATE, DELETE, TRUNCATE ON audit_log FROM arbx_rw';
EXCEPTION
    WHEN undefined_object OR undefined_table THEN
        RAISE NOTICE 'Section 1: audit_log or arbx_rw role not found — REVOKE skipped';
END $$;

DO $$
BEGIN
    RAISE NOTICE 'Migration 054 Section 1: GRANT hardening applied to audit_log';
END $$;

-- =======================
-- Section 2: Foreign key ON DELETE clauses
-- =======================

-- 2a. opportunities.strategy_kind: TEXT column with CHECK constraint — NOT a FK
-- to strategy_catalog. No FK exists to add ON DELETE behaviour to. The check
-- below will correctly find zero matching FK constraints and skip gracefully.
-- Retained as documentation that the audit explicitly reviewed this relationship.
DO $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM information_schema.table_constraints
        WHERE table_schema = 'public'
          AND table_name = 'opportunities'
          AND constraint_type = 'FOREIGN KEY'
          AND constraint_name LIKE '%strategy%'
    ) THEN
        -- FK on strategy_kind was added by a later migration; re-add with NO ACTION
        -- (preserve historical opportunity records if strategy is removed).
        EXECUTE 'ALTER TABLE opportunities DROP CONSTRAINT IF EXISTS opportunities_strategy_kind_fkey';
        RAISE NOTICE 'Section 2: opportunities.strategy_kind FK dropped for re-add with explicit NO ACTION';
    ELSE
        RAISE NOTICE 'Section 2: opportunities.strategy_kind has no FK (uses CHECK constraint) — skip';
    END IF;
END $$;

-- 2b. pool_reserves.pool_id → pools(id): currently REFERENCES pools(id) with
-- implicit NO ACTION. Orphan reserves (pool deleted) are useless — CASCADE is
-- the correct semantics. Drop and re-add with ON DELETE CASCADE.
-- The system-generated constraint name follows PostgreSQL's default convention:
-- <table>_<column>_fkey. We also look it up via pg_constraint as a fallback.
DO $$
DECLARE
    fk_name TEXT;
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.tables
        WHERE table_schema = 'public' AND table_name = 'pool_reserves'
    ) THEN
        RAISE NOTICE 'Section 2: pool_reserves table absent — FK CASCADE skip';
        RETURN;
    END IF;

    -- Look up the actual FK constraint name on pool_reserves(pool_id)
    SELECT conname INTO fk_name
    FROM pg_constraint
    WHERE conrelid = 'pool_reserves'::regclass
      AND contype = 'f'
      AND conkey = ARRAY[(
          SELECT attnum FROM pg_attribute
          WHERE attrelid = 'pool_reserves'::regclass AND attname = 'pool_id'
      )::smallint];

    IF fk_name IS NOT NULL THEN
        EXECUTE format('ALTER TABLE pool_reserves DROP CONSTRAINT %I', fk_name);
        RAISE NOTICE 'Section 2: dropped FK constraint % from pool_reserves', fk_name;
    ELSE
        RAISE NOTICE 'Section 2: pool_reserves.pool_id FK not found — adding fresh';
    END IF;

    -- Re-add with explicit ON DELETE CASCADE
    ALTER TABLE pool_reserves
        ADD CONSTRAINT pool_reserves_pool_id_fkey
        FOREIGN KEY (pool_id) REFERENCES pools(id) ON DELETE CASCADE;

    RAISE NOTICE 'Section 2: pool_reserves.pool_id FK re-added with ON DELETE CASCADE';
EXCEPTION
    WHEN undefined_table OR undefined_column THEN
        RAISE NOTICE 'Section 2: pool_reserves or pools not available — FK CASCADE skipped';
END $$;

-- =======================
-- Section 3: Financial precision audit
-- =======================
-- Policy: wei values use NUMERIC(78, 0) (covers u256 max = 10^77).
--         USD profit/cost/gas use NUMERIC(20, 8) minimum.
--         Any column below (18, 6) is flagged for operator review.
-- This section ONLY emits RAISE NOTICE — operator decides if ALTER is needed.
-- R8: do NOT silently coerce existing data with an ALTER COLUMN TYPE.

DO $$
DECLARE
    col RECORD;
    verdict TEXT;
BEGIN
    FOR col IN
        SELECT
            c.table_name,
            c.column_name,
            c.data_type,
            c.numeric_precision,
            c.numeric_scale
        FROM information_schema.columns c
        WHERE c.table_schema = 'public'
          AND c.data_type = 'numeric'
          AND c.table_name IN ('opportunities', 'executions', 'paper_trade_runs',
                                'pool_reserves', 'pool_ticks')
        ORDER BY c.table_name, c.column_name
    LOOP
        -- Classify: NUMERIC without explicit precision (NULL, NULL) = unconstrained
        -- NUMERIC(p, s): financial USD columns need p >= 18, s >= 6
        --                wei columns need p = 78, s = 0
        IF col.numeric_precision IS NULL THEN
            verdict := 'NOTICE: unconstrained NUMERIC — consider explicit precision for planarity';
        ELSIF col.numeric_scale = 0 AND col.numeric_precision >= 78 THEN
            verdict := 'OK (wei / integer value)';
        ELSIF col.numeric_scale >= 6 AND col.numeric_precision >= 18 THEN
            verdict := 'OK (USD-grade precision)';
        ELSIF col.numeric_scale >= 4 AND col.numeric_precision >= 10 THEN
            verdict := 'ACCEPTABLE (rate/pct column)';
        ELSE
            verdict := 'REVIEW: precision may be insufficient for USD financial values';
        END IF;

        RAISE NOTICE 'Section 3 precision: %.% → NUMERIC(%,%) | %',
            col.table_name,
            col.column_name,
            COALESCE(col.numeric_precision::TEXT, '?'),
            COALESCE(col.numeric_scale::TEXT, '?'),
            verdict;
    END LOOP;

    RAISE NOTICE 'Section 3: precision audit complete — operator review NOTICEs above';
END $$;

-- =======================
-- Section 4: Missing indexes on hot paths not covered by 050+051
-- =======================

-- 4a. opportunities: time-bucketed queries with chain + status filter.
-- Hot pattern: WHERE chain_id = $1 AND status IN ('detected','validated','simulated')
--              ORDER BY detected_at DESC LIMIT $n
-- Partial index on early-lifecycle statuses keeps the index compact; rows in
-- terminal states ('executed','rejected','failed') are not re-queried by the
-- scanner hot path.
-- Valid status values from migration 003 CHECK constraint:
--   'detected','validated','simulated','scored','executing',
--   'executed','reconciled','rejected','failed'
-- RERUN-LOCK-SAFETY (GEN-CI-FAIL 2026-08-30): catalog-guarded so the no-op
-- re-run takes no table lock on the hot opportunities table
-- (lint-migration-rerun-lock-safety.sh).
DO $$
BEGIN
  IF NOT EXISTS (
    SELECT 1 FROM pg_indexes
    WHERE schemaname = 'public' AND indexname = 'idx_opportunities_detected_chain_status'
  ) THEN
    EXECUTE 'CREATE INDEX idx_opportunities_detected_chain_status ON opportunities(detected_at DESC, chain_id, status) WHERE status IN (''detected'', ''validated'', ''simulated'')';
  END IF;
END $$;

-- 4b. executions: status + time queries for reconciliation and monitoring.
-- Hot pattern: WHERE status = $1 ORDER BY submitted_at DESC LIMIT $n
-- executions has no chain_id column (chain context is on opportunities, joined
-- via opportunity_id if needed). Index uses submitted_at (the actual timestamp
-- column on this table, not created_at which does not exist here).
DO $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM information_schema.tables
        WHERE table_schema = 'public' AND table_name = 'executions'
    ) THEN
        EXECUTE 'CREATE INDEX IF NOT EXISTS idx_executions_status_submitted
                 ON executions(status, submitted_at DESC)';
        RAISE NOTICE 'Section 4: idx_executions_status_submitted created (or already existed)';
    ELSE
        RAISE NOTICE 'Section 4: executions table absent — idx_executions_status_submitted skipped';
    END IF;
END $$;

-- 4c. audit_log: time-range queries with actor filter for compliance reporting.
-- Hot pattern: WHERE created_at >= $start AND created_at < $end AND actor = $actor
-- audit_log is partitioned by range (created_at). The index on the parent table
-- propagates to all existing and future partitions (PostgreSQL 11+).
-- Existing indexes idx_audit_actor_time(actor, created_at DESC) and
-- idx_audit_action_time(action, created_at DESC) lead with a categorical column.
-- This new index leads with created_at for time-range-first queries, which is
-- a genuinely distinct access pattern (planner will not use the existing indexes
-- for queries that filter on time range without an equality predicate on actor).
DO $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM information_schema.tables
        WHERE table_schema = 'public' AND table_name = 'audit_log'
    ) THEN
        EXECUTE 'CREATE INDEX IF NOT EXISTS idx_audit_log_created_actor
                 ON audit_log(created_at DESC, actor)';
        RAISE NOTICE 'Section 4: idx_audit_log_created_actor created (or already existed)';
    ELSE
        RAISE NOTICE 'Section 4: audit_log table absent — idx_audit_log_created_actor skipped';
    END IF;
END $$;

-- =======================
-- Section 5: Sanity NOTICE — aggregate counts post-migration
-- =======================

DO $$
DECLARE
    grant_count INT;
    fk_count    INT;
    idx_count   INT;
    new_idx_count INT;
BEGIN
    -- Total grant entries for arbx roles across all tables
    SELECT COUNT(*) INTO grant_count
    FROM information_schema.table_privileges
    WHERE grantee IN ('arbx_rw', 'arbx_ro', 'arbx_migrator');

    -- Total FK constraints in public schema
    SELECT COUNT(*) INTO fk_count
    FROM information_schema.table_constraints
    WHERE table_schema = 'public'
      AND constraint_type = 'FOREIGN KEY';

    -- Total indexes in public schema
    SELECT COUNT(*) INTO idx_count
    FROM pg_indexes
    WHERE schemaname = 'public';

    -- Verify this migration's new indexes are present
    SELECT COUNT(*) INTO new_idx_count
    FROM pg_indexes
    WHERE schemaname = 'public'
      AND indexname IN (
          'idx_opportunities_detected_chain_status',
          'idx_executions_status_submitted',
          'idx_audit_log_created_actor'
      );

    RAISE NOTICE 'Migration 054 audit summary:';
    RAISE NOTICE '  GRANT entries (arbx_rw + arbx_ro + arbx_migrator): %', grant_count;
    RAISE NOTICE '  FK constraints in public schema: %', fk_count;
    RAISE NOTICE '  Total pg_indexes in public: %', idx_count;
    RAISE NOTICE '  New 054 indexes present: %/3', new_idx_count;
    RAISE NOTICE 'Migration 054: GRANT cleanup + FK ON DELETE clauses + new indexes applied';
END $$;

COMMIT;

-- ============================================================================
-- ROLLBACK (activate manually — do NOT uncomment in normal deploy)
-- ============================================================================
-- BEGIN;
--
-- -- Section 4 rollback: drop new indexes
-- DROP INDEX IF EXISTS idx_opportunities_detected_chain_status;
-- DROP INDEX IF EXISTS idx_executions_status_submitted;
-- DROP INDEX IF EXISTS idx_audit_log_created_actor;
--
-- -- Section 2 rollback: restore pool_reserves FK to implicit NO ACTION
-- DO $$
-- BEGIN
--     IF EXISTS (
--         SELECT 1 FROM information_schema.tables
--         WHERE table_schema = 'public' AND table_name = 'pool_reserves'
--     ) THEN
--         EXECUTE 'ALTER TABLE pool_reserves DROP CONSTRAINT IF EXISTS pool_reserves_pool_id_fkey';
--         EXECUTE 'ALTER TABLE pool_reserves ADD CONSTRAINT pool_reserves_pool_id_fkey
--                  FOREIGN KEY (pool_id) REFERENCES pools(id)';
--     END IF;
-- END $$;
--
-- -- Section 1 rollback: no action needed — REVOKEs of ungranted privileges are no-ops.
-- -- The grants that SHOULD exist were already set in migrations 011/019 and remain.
--
-- COMMIT;
