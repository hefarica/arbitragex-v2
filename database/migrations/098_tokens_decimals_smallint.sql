-- 098_tokens_decimals_smallint.sql
-- Align tokens.decimals to SMALLINT (INT2) — the type ALL Rust code expects:
--   reader  backend/searcher-rs/src/sim_encoder_pg.rs  decodes Option<i16>
--   writers backend/token-enricher/src/persistence.rs + searcher-rs/src/pool_discovery.rs bind `as i16`
-- A drifted INTEGER (INT4) column makes the sqlx decode fail with
--   "Rust type Option<i16> (as SQL type INT2) is not compatible with SQL type INT4"
-- which fires pg_decimals_provider.refresh_failed every 60s, starving the V3 decimals feed ->
-- v3_spot_price_from_sqrt emits garbage -> phantom ~$9M expected_profit -> cap_clamp_failed rejects,
-- so NO candidate ever reaches rejection_reason=None and zero paper trades are emitted.
-- This captures the live hot-fix applied on the VPS 2026-06-16 into version control so a rebuilt
-- DB ends up with the correct type. Idempotent + safe: decimals values are 0..36 (fit smallint);
-- only alters when the column is not already smallint.
DO $$
BEGIN
  IF EXISTS (
    SELECT 1 FROM information_schema.columns
    WHERE table_name = 'tokens'
      AND column_name = 'decimals'
      AND data_type <> 'smallint'
  ) THEN
    ALTER TABLE tokens ALTER COLUMN decimals TYPE smallint;
  END IF;
END $$;
