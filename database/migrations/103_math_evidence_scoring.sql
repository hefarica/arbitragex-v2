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

-- ADD COLUMN IF NOT EXISTS still takes ACCESS EXCLUSIVE before checking the
-- column. Replaying this migration on a busy, already-upgraded VPS therefore
-- used to hit lock_timeout on every deployment (run 34436938352).
-- Check the catalog first; only missing columns require DDL. Existing columns
-- must have the expected type: schema drift is an error, not an assumed pass.
DO $migration$
DECLARE
    column_name text;
    expected_type regtype;
    actual_type oid;
BEGIN
    FOR column_name, expected_type IN
        SELECT * FROM (VALUES
            ('evidence_vector', 'jsonb'::regtype),
            ('evidence_computed_at', 'timestamptz'::regtype)
        ) AS required_columns(name, type_id)
    LOOP
        SELECT atttypid INTO actual_type
          FROM pg_attribute
         WHERE attrelid = 'public.scored_opportunities'::regclass
           AND attname = column_name AND attnum > 0 AND NOT attisdropped;
        IF actual_type IS NULL THEN
            EXECUTE format('ALTER TABLE public.scored_opportunities ADD COLUMN %I %s',
                           column_name, expected_type);
        ELSIF actual_type <> expected_type::oid THEN
            RAISE EXCEPTION 'Schema drift: scored_opportunities.% has type %, expected %',
                column_name, actual_type::regtype, expected_type;
        END IF;
    END LOOP;
END
$migration$;

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
