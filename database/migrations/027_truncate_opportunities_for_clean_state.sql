-- ArbitrageX v2 — Migration 027: archive + TRUNCATE opportunities pre-Sprint-0 mock-profit pollution.
--
-- Doctrine: pre-Sprint-0, scanner.rs used rand::thread_rng().gen_range(5.0..55.0) to fill
-- missing profit values (RULE 00 violation). Sprint 0 (commit dc5d376 area, scanner.rs:246)
-- removed that bypass. Historical rows produced before Sprint 0 contain economically
-- meaningless random profits that pollute Sprint 3 PMI calculations.
--
-- This migration archives the polluted rows defensively (forensic reference, NOT used by app)
-- and truncates the live table so Sprint 3 starts from clean state.
--
-- Idempotent (2026-08-03): the original `INSERT INTO archive SELECT * FROM opportunities`
-- aborted re-runs with "INSERT has more expressions than target columns" because the archive
-- table was created with an older column set (fewer cols) and later migrations added columns
-- to `opportunities` — `SELECT *` then returned more cols than the archive had. Since the
-- TRUNCATE already ran on the first application, `opportunities` is empty on every re-run,
-- so the archive+truncate is now guarded behind a non-empty check. This preserves the
-- existing forensic archive (no DROP) and makes re-runs a true no-op.

BEGIN;

CREATE TABLE IF NOT EXISTS opportunities_archive_pre_sprint3 (
  LIKE opportunities INCLUDING ALL
);

-- Only archive+truncate when there are rows to archive. After the first run,
-- opportunities is empty (truncated), so this block is skipped — no column-
-- count mismatch can occur because the INSERT never executes.
DO $$
BEGIN
  IF EXISTS (SELECT 1 FROM opportunities LIMIT 1) THEN
    INSERT INTO opportunities_archive_pre_sprint3
      SELECT * FROM opportunities
      ON CONFLICT DO NOTHING;
    TRUNCATE opportunities RESTART IDENTITY CASCADE;
    RAISE NOTICE 'Migration 027: archived polluted rows and truncated opportunities.';
  ELSE
    RAISE NOTICE 'Migration 027: opportunities already empty — no-op (idempotent).';
  END IF;
END
$$;

COMMIT;
