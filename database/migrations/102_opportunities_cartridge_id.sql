-- 102_opportunities_cartridge_id.sql
--
-- Formalizes schema that already exists in PROD (applied manually when the
-- 264-cartridge v3 migration shipped — the searcher's INSERT has persisted
-- cartridge_id ever since, but no migration ever backed it; CI-GATE-RELIABILITY
-- part 3 surfaced the drift: the integration-test schema (built ONLY from
-- committed migrations) lacked the column, so the live route's SELECT failed
-- with 503 query_failed on every PR run).
--
-- Idempotent: no-op where the column already exists (prod).
--
-- RERUN-LOCK-SAFETY (GEN-CI-FAIL 2026-08-30): opportunities is written
-- continuously by the searcher; bare ALTER TABLE ... IF NOT EXISTS and
-- CREATE INDEX IF NOT EXISTS take writer-conflicting table locks before the
-- existence checks and starve under the runner's lock_timeout=10s.
-- Catalog-guarded no-op path takes no table lock
-- (lint-migration-rerun-lock-safety.sh). When the guard fires on a POPULATED
-- table with the index genuinely missing, prefer a dedicated CONCURRENTLY
-- fixer (105 pattern) over relaxing this guard.
DO $$
BEGIN
  IF NOT EXISTS (
    SELECT 1 FROM information_schema.columns
    WHERE table_schema = 'public' AND table_name = 'opportunities'
      AND column_name = 'cartridge_id'
  ) THEN
    EXECUTE 'ALTER TABLE opportunities ADD COLUMN cartridge_id TEXT';
  END IF;
  IF NOT EXISTS (
    SELECT 1 FROM pg_indexes
    WHERE schemaname = 'public' AND indexname = 'idx_opportunities_cartridge_id'
  ) THEN
    EXECUTE 'CREATE INDEX idx_opportunities_cartridge_id ON opportunities (cartridge_id)';
  END IF;
END $$;
