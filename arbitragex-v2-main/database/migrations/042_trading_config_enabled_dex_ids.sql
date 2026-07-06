-- ArbitrageX v2 — Migration 042: Phase 2 route-finder per-chain DEX selection.
--
-- Adds two features:
--
-- 1. trading_config.enabled_dex_ids (UUID[])
--    Per-chain list of dex.id values the operator has explicitly enabled for
--    the route-finder. NULL means "no explicit selection — use all is_active
--    dexes" (backwards-compatible with all existing rows).
--    A GIN index makes the @> containment operator efficient.
--
-- 2. dexes.volume_24h_usd / dexes.tvl_usd (NUMERIC(20,2) NULL)
--    Cached DEX-level volume/TVL snapshots surfaced by the UI for sorting.
--    Populated by an external enricher (Phase 3); NULL until then.
--
-- Reversibility:
--   ALTER TABLE trading_config DROP COLUMN IF EXISTS enabled_dex_ids;
--   DROP INDEX IF EXISTS idx_trading_config_enabled_dex_ids;
--   ALTER TABLE dexes DROP COLUMN IF EXISTS volume_24h_usd;
--   ALTER TABLE dexes DROP COLUMN IF EXISTS tvl_usd;

BEGIN;

ALTER TABLE trading_config
  ADD COLUMN IF NOT EXISTS enabled_dex_ids UUID[] NULL;

CREATE INDEX IF NOT EXISTS idx_trading_config_enabled_dex_ids
  ON trading_config USING GIN (enabled_dex_ids);

ALTER TABLE dexes
  ADD COLUMN IF NOT EXISTS volume_24h_usd NUMERIC(20,2) NULL,
  ADD COLUMN IF NOT EXISTS tvl_usd        NUMERIC(20,2) NULL;

COMMIT;
