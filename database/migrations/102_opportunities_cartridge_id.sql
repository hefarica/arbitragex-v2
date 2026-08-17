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

-- Index for cartridge-scoped dashboard queries (CONCURRENTLY per doctrine
-- lint-migration-index-locks — cannot run inside a DO/transaction block, so
-- plain statement with IF NOT EXISTS for idempotency).
-- NOTE: non-CONCURRENT deliberately (pre-doctrine <105): the integration
-- harness applies this via node-pg simple-query multi-statement = implicit
-- transaction, where CONCURRENTLY is illegal (SQLSTATE 25001). Safe: fresh
-- boot = empty table (instant); prod re-apply = index exists (skip). Future
-- indexes on populated tables: CONCURRENTLY (lint enforces >=105).
CREATE INDEX IF NOT EXISTS idx_opportunities_cartridge_id
  ON opportunities (cartridge_id);
