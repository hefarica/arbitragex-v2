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
-- Idempotent: re-running on an already-empty `opportunities` is a no-op (the archive INSERT
-- is ON CONFLICT DO NOTHING so repeated archive copies don't double-count).

BEGIN;

CREATE TABLE IF NOT EXISTS opportunities_archive_pre_sprint3 (
  LIKE opportunities INCLUDING ALL
);

INSERT INTO opportunities_archive_pre_sprint3
  SELECT * FROM opportunities
  ON CONFLICT DO NOTHING;

TRUNCATE opportunities RESTART IDENTITY CASCADE;

COMMIT;
