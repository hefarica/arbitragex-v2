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
-- RETIRED to no-op (2026-08-03): the pre-Sprint-3 archive+TRUNCATE cleanup ALREADY ran.
-- `opportunities` now holds ~12.7M legitimate live rows (actively populated by the searcher),
-- so the historical archive+TRUNCATE logic MUST NOT re-execute — it would TRUNCATE live data.
-- The prior guard `IF EXISTS (SELECT 1 FROM opportunities LIMIT 1)` is ALWAYS TRUE on a live
-- DB, re-entering the failing INSERT every run. This migration is now a pure no-op: it only
-- preserves the forensic archive table (CREATE IF NOT EXISTS) and writes NOTHING to
-- `opportunities`.

BEGIN;

CREATE TABLE IF NOT EXISTS opportunities_archive_pre_sprint3 (
  LIKE opportunities INCLUDING ALL
);

COMMIT;
