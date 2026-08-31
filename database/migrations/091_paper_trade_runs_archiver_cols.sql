-- ArbitrageX v2 — Migration 091: paper_trade_runs archiver columns
--
-- The Shadow Archiver (api-server) persists detected-opportunity sim telemetry
-- into paper_trade_runs. The 051 schema lacks two fields the archiver records:
--   reason      — the opportunity's rejection_reason (NULL when accepted).
--   route_hash  — a deterministic fingerprint of the route (from real payload
--                 fields), for grouping paper runs by route over time.
-- Additive + nullable + idempotent: safe on existing prod DBs (no backfill,
-- no NOT NULL). Runs after 051 (lexical 091 > 051).

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
      AND column_name = 'reason'
  ) THEN
    EXECUTE 'ALTER TABLE paper_trade_runs ADD COLUMN reason TEXT';
  END IF;
  IF NOT EXISTS (
    SELECT 1 FROM information_schema.columns
    WHERE table_schema = 'public' AND table_name = 'paper_trade_runs'
      AND column_name = 'route_hash'
  ) THEN
    EXECUTE 'ALTER TABLE paper_trade_runs ADD COLUMN route_hash TEXT';
  END IF;
  IF NOT EXISTS (
    SELECT 1 FROM pg_indexes
    WHERE schemaname = 'public' AND indexname = 'idx_paper_trade_runs_route_hash'
  ) THEN
    EXECUTE 'CREATE INDEX idx_paper_trade_runs_route_hash ON paper_trade_runs(route_hash) WHERE route_hash IS NOT NULL';
  END IF;
END $$;

DO $$
BEGIN
    RAISE NOTICE 'Migration 091: paper_trade_runs.reason + route_hash columns ready for the Shadow Archiver.';
END $$;

COMMIT;
