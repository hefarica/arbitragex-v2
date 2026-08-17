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
ALTER TABLE opportunities ADD COLUMN IF NOT EXISTS cartridge_id TEXT;

-- Index for cartridge-scoped dashboard queries (CONCURRENTLY per doctrine:
-- lint-migration-index-locks; on a fresh boot the table is empty = instant).
-- Wrapped in an exception guard for re-runs where the index already exists
-- (CREATE INDEX CONCURRENTLY lacks IF NOT EXISTS).
DO $$ BEGIN
  CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_opportunities_cartridge_id
    ON opportunities (cartridge_id);
EXCEPTION WHEN duplicate_table THEN NULL; END $$;
