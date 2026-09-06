-- 118_opportunities_detected_at_breakdown_idx.sql
--
-- REJECT-BREAKDOWN-EXPORT-01 (2026-09-06).
--
-- The rejection-breakdown aggregate (backend/api-server/src/routes/
-- rejection-breakdown.ts) reads detected_at windows over opportunities rows
-- that carry rejection_reason. Adversarial review (14-agent workflow,
-- verified high-confidence) proved the only detected_at-leading index is
-- PARTIAL — idx_opportunities_detected_chain_status (054 §4:
-- WHERE status IN ('detected','validated','simulated')) — and therefore
-- EXCLUDES exactly the status='rejected' rows this endpoint aggregates
-- (searcher-rs persistence status_from_rejection_reason). Every request was
-- a full seq scan of the ~60d retention table (observed 48.4K rows/24h →
-- ~2.9M rows), TWICE, regardless of the hours window — a pool-exhaustion
-- vector through the public edge even inside the per-IP rate limit.
--
-- A plain detected_at index serves BOTH the GROUP BY and the totals COUNT
-- as window-bounded range scans (24h ≈ 48K index entries ≈ milliseconds).
-- The route additionally bounds each statement with SET LOCAL
-- statement_timeout = 15s (the #502 RDO-SUMMARY-HANG pattern) as
-- belt-and-braces; this index is the structural fix.
--
-- 105 pattern (doctrine, MIGRATION_HISTORY.md): on hot tables a genuinely
-- missing index on a populated table is built CONCURRENTLY —
-- ShareUpdateExclusiveLock never blocks searcher INSERTs/purge DELETEs, and
-- the lint (automation/tools/lint-migration-rerun-lock-safety.sh) accepts
-- CONCURRENTLY on the no-op re-run path. Statement timeout raised for the
-- build (a CONCURRENT build over ~2.9M rows can exceed the runner default).
-- IF a CONCURRENT build fails midway it leaves an INVALID index; recovery:
--   DROP INDEX CONCURRENTLY IF EXISTS idx_opp_detected_at;
-- followed by re-running this file (documented here, not automated).

SET statement_timeout = '40min';

CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_opp_detected_at
  ON opportunities(detected_at DESC);
