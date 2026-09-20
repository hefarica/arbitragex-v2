-- 123_bayesian_priors_token_pair_nullable.sql
-- (renumbered from 122: #610 takes 122_route_discovery_outcomes_partitioned)
-- Deuda 4 / STRAT-IDENT-01 follow-up: bayesian_priors rows are keyed by
-- strategy_key (unique partial index uq_bayesian_priors_strategy, migration
-- 108). token_pair is legacy identity from migration 097 (its UNIQUE
-- constraint was already dropped in 108); strategy-keyed rows written by the
-- searcher-rs beta_priors consolidator have no honest pair value (R8 — the
-- pair stays as context in the record, never as the calibration bucket).
--
-- Table verified EMPTY on every environment (no writer existed as of
-- 2026-09-20) — no data implications. Idempotent: DROP NOT NULL on an
-- already-nullable column is a no-op.

ALTER TABLE bayesian_priors
    ALTER COLUMN token_pair DROP NOT NULL;

COMMENT ON COLUMN bayesian_priors.strategy_key IS
    'STRAT-IDENT-01 identity (cartridge stem / engine kind). Unique via uq_bayesian_priors_strategy (partial). Writer: searcher-rs beta_priors consolidator (ARBX_BETA_PRIORS_MODE).';
