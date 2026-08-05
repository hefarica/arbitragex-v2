-- ArbitrageX v2 — Migration 103: §IV math-evidence → scoring foundation
--
-- Stage 1 of the "math-evidence → scoring" wiring (dictamen §IV). Adds:
--   1. Per-opportunity evidence vector snapshot, co-located with the Gate-C score
--      (scored_opportunities.evidence_vector) — the raw material for Stage 2
--      calibration.
--   2. The calibrated per-operator log-LR store (math_operator_calibration) —
--      EMPTY by default (log_lr = 0 ⇒ LR = e^0 = 1 ⇒ no contribution ⇒ honest
--      flat prior). Filled by the Stage 2 offline calibration job from labeled
--      (evidence, realized-Y) data.
--
-- Honesty gate: with an empty calibration store the §IV posterior collapses to
-- the flat prior (source_context = 'flat_prior') — the motor is wired but does
-- NOT drive scoring until calibrated. No fabricated LRs (RULE 00 / R8).
--
-- Idempotent.

BEGIN;

ALTER TABLE scored_opportunities
    ADD COLUMN IF NOT EXISTS evidence_vector    JSONB,
    ADD COLUMN IF NOT EXISTS evidence_computed_at TIMESTAMPTZ;

CREATE TABLE IF NOT EXISTS math_operator_calibration (
    operator_id    SMALLINT PRIMARY KEY,                       -- 1..31
    log_lr         DOUBLE PRECISION NOT NULL DEFAULT 0.0,      -- ln LR_k (Stage 2)
    sample_count   BIGINT       NOT NULL DEFAULT 0,
    calibrated_at  TIMESTAMPTZ,
    token_pair     TEXT                                        -- NULL = global; per-pair rows if calibrated that way
);

COMMENT ON TABLE math_operator_calibration IS
    'Stage 2 calibration store: per-operator log likelihood-ratio ln LR_k for the §IV posterior (log-odds = prior + Σ log_lr_k · e_k). Empty (log_lr=0 ⇒ LR=1) until the offline calibration job fills it from labeled (evidence_vector, paper_trade_runs.actual_profit_usd) data. Default 0 = honest flat prior.';

COMMIT;
