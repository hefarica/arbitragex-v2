-- ArbitrageX v2 — Migration 103: strategy_kind accepts cartridge stems
--
-- Each cartridge is now a CANONICAL strategy_kind. The searcher persists the
-- cartridge .rhai stem (e.g. 'mev_01_001_dex_dex_arbitrage') as `strategy_kind`
-- via `Opportunity::canonical_strategy_kind()` (cartridge_id preferred, else the
-- 5 base families). The static 5-value CHECK from migration 003 can no longer
-- hold the dynamic 264+ set (auto-generated in shared-ts/contracts/strategy-kinds.ts
-- from backend/searcher-rs/cartridges/strategies/*.rhai), so the CHECK is dropped.
-- Canonical strategy_kinds are validated at the application layer (the canonical
-- TS enum + the Rust cartridge registry). The 5 base families remain valid.
--
-- RERUN-LOCK-SAFETY (GEN-CI-FAIL 2026-08-30): DROP CONSTRAINT is an ALTER TABLE
-- subcommand — AccessExclusiveLock on the hot opportunities table on every
-- re-run even when the constraint is already gone. Catalog-guarded no-op path
-- takes no table lock (lint-migration-rerun-lock-safety.sh).
DO $$
BEGIN
  IF EXISTS (
    SELECT 1 FROM pg_constraint
    WHERE conname = 'opportunities_strategy_kind_check'
      AND conrelid = 'public.opportunities'::regclass
  ) THEN
    EXECUTE 'ALTER TABLE opportunities DROP CONSTRAINT opportunities_strategy_kind_check';
  END IF;
END $$;
