-- ArbitrageX v2 — Migration 108: per-strategy identity in the Gate-C scoring circuit
--
-- STRAT-IDENT-01 (operator directive 2026-08-23): "no debes colocar clase de
-- estrategia, sino evaluar directamente cada estrategia y esta dirá cuáles son
-- las estructuras que le aplican y con ello armas el combo particular y
-- específico".
--
-- Before this migration the scoring circuit flattened strategy identity:
--   - `bayesian_priors` was keyed UNIQUE(token_pair): all 264 cartridges trading
--     WETH/USDC would share ONE calibration bucket.
--   - `scored_opportunities` carried no strategy identity at all — scores could
--     not be sliced per strategy.
--
-- This migration re-keys the calibration store per STRATEGY (the cartridge stem
-- for the 264 cartridges; the engine kind for the 5 core engines — each an
-- individual strategy) and persists the strategy identity on every score.
--
-- `bayesian_priors` is EMPTY on every environment (no writer exists yet as of
-- 2026-08-23 — verified), so re-keying drops no data.
--
-- Shape note (FREEZE-01 FASE 2.2, migration 105 pattern): the two index builds
-- run CONCURRENTLY — they take only SHARE locks that never block INSERTs, and
-- therefore MUST sit OUTSIDE any transaction block (Postgres rejects
-- CONCURRENTLY inside BEGIN). The ALTERs are metadata-only (nullable ADD
-- COLUMN / DROP CONSTRAINT) and idempotent, so each runs auto-committed, in
-- dependency order, with no wrapping BEGIN. statement_timeout is raised past
-- the runner's global 10min: a CONCURRENT build over a large live table can
-- exceed it. If a CONCURRENT build fails midway it leaves an INVALID index:
-- recovery is  DROP INDEX CONCURRENTLY IF EXISTS <name>;  and re-run this file.
--
-- Idempotent + additive. Paper-only telemetry (RULE 00 / §34: capital = 0).

-- ---------------------------------------------------------------------------
-- Pass 1 (plain DDL, metadata-only): carry the per-strategy identity.
-- Nullable + no backfill: rows scored before this migration have no honest
-- per-strategy value (R8 — never fabricate one).
-- ---------------------------------------------------------------------------
ALTER TABLE scored_opportunities
    ADD COLUMN IF NOT EXISTS strategy_key TEXT;

ALTER TABLE bayesian_priors
    ADD COLUMN IF NOT EXISTS strategy_key TEXT;

-- Drop the per-pair unique (constraint name from migration 097).
-- bayesian_priors is empty (verified) — no data implications.
ALTER TABLE bayesian_priors
    DROP CONSTRAINT IF EXISTS bayesian_priors_token_pair_key;

-- ---------------------------------------------------------------------------
-- Pass 2 (CONCURRENTLY — INSERT-safe on live tables, migration 105 pattern).
-- ---------------------------------------------------------------------------
SET statement_timeout = '40min';

CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_scored_opportunities_strategy
    ON scored_opportunities (strategy_key, created_at DESC)
    WHERE strategy_key IS NOT NULL;

CREATE UNIQUE INDEX CONCURRENTLY IF NOT EXISTS uq_bayesian_priors_strategy
    ON bayesian_priors (strategy_key)
    WHERE strategy_key IS NOT NULL;
