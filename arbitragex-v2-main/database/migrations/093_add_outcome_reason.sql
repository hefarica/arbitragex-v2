-- ArbitrageX v2 — Migration 093: route_discovery_outcomes.reason (FASE B Paso 9)
--
-- Observability column: the cartridge's explicit `reason` string (e.g.
-- "v3_sizing_pending", "insufficient_pools", "below_gas_threshold",
-- "token_meta_unavailable") propagated Rhai -> Rust -> Redis -> Postgres so the
-- durable outcomes series can explain WHY is_opportunity=false — distinguishing a
-- STRUCTURAL ceiling (e.g. V3 sizing unimplemented) from real insufficiency.
--
-- NULLABLE: backfill-free, does NOT break existing rows (pre-Paso-9 rows keep NULL).
-- Additive + idempotent (IF NOT EXISTS). NO-ACTIVE: telemetry only — no FK to capital
-- tables, no executor, never touches arbx:opps:detected.

BEGIN;

ALTER TABLE public.route_discovery_outcomes ADD COLUMN IF NOT EXISTS reason TEXT;

DO $$
BEGIN
    RAISE NOTICE 'Migration 093: route_discovery_outcomes.reason added (Fase B Paso 9 observability).';
END $$;

COMMIT;
