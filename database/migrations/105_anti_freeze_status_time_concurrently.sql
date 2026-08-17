-- 105_anti_freeze_status_time_concurrently.sql
--
-- FREEZE-01 / ANTI-FREEZE FASE 2.2 (2026-08-17).
--
-- RCA (#359, docs/incidents/2026-08-17-PIPELINE-FREEZE-PURGE-LOCKS.md):
-- migration 003 line 30 declares
--   CREATE INDEX IF NOT EXISTS idx_opp_status_time
--     ON opportunities(status, detected_at DESC);          -- NO CONCURRENTLY
-- On a FRESH boot (empty table) that is instantaneous and fine. But on the
-- LIVE prod table (15.8M rows) the index was MISSING on 2026-08-17 (most
-- plausible cause: manual DROP during the 2026-08-15 PGBLOAT-02 mass purge;
-- no auditable DROP in available logs), so the post-freeze deploy re-applied
-- 003, the build queued behind the retention DELETE's lock, and the searcher
-- inserts queued behind THAT -> 21h pipeline silence.
--
-- EVIDENCE (2026-08-17, prod): idx_opp_status_time now EXISTS and is valid
-- (899 MB) -- it was built during that freeze-window deploy. This migration
-- is therefore a NO-OP today; its job is to make the "index missing on a
-- live table" recovery SAFE forever after: CONCURRENTLY never blocks INSERTs.
--
-- L4 state pre-merge:
--   SELECT c.relname, i.indisvalid FROM pg_index i JOIN pg_class c
--   ON i.indexrelid=c.oid WHERE c.relname='idx_opp_status_time';
--   ->  idx_opp_status_time | t   (valid, 899 MB)
--
-- NOTE: this file overrides the runner's global statement_timeout (10min) —
-- a CONCURRENT build over ~16M rows can exceed it. Lock_timeout is NOT
-- raised: CREATE INDEX CONCURRENTLY takes only SHARE locks that don't
-- conflict with INSERTs.
-- IF a CONCURRENT build ever fails midway it leaves an INVALID index:
-- recovery is  DROP INDEX CONCURRENTLY IF EXISTS idx_opp_status_time;
-- followed by re-running this file (documented here, not automated).

SET statement_timeout = '40min';

CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_opp_status_time
  ON opportunities(status, detected_at DESC);
