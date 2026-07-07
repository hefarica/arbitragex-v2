-- ArbitrageX v2 — Migration 097: scored_opportunities + bayesian_priors (Gate C telemetry)
--
-- Gate C confidence-scoring telemetry. The Rust searcher computes a ConfidenceScore
-- per PAPER opportunity (Bayesian posterior + Kelly sizing) and XADDs it to the Redis
-- stream `arbx:scoring:scored`; a passive api-server archiver consumes it into
-- `scored_opportunities`. `bayesian_priors` holds per-pair calibrated priors seeded by
-- the A.5 paper-shadow window; `gate_c_validation` is the durable evidence channel for
-- A.4 fork validation (the api-server cannot read a local repo marker file).
--
-- DOCTRINE: This is PAPER-ONLY telemetry, NOT live execution. `recommended_usd` is a
-- HYPOTHETICAL paper-bankroll sizing figure (ARBX_KELLY_MAX_CAPITAL_USD), never real
-- capital, never a position taken. No signer, no broadcast, no capital. Zero-Mocks:
-- only REAL computed fields are persisted. stream_id UNIQUE → idempotent at-least-once
-- consume. Additive + idempotent: safe to re-run.

BEGIN;

-- ---------------------------------------------------------------------------
-- scored_opportunities — one row per scored PAPER opportunity (Gate C telemetry).
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS scored_opportunities (
    id                  BIGSERIAL PRIMARY KEY,
    stream_id           TEXT UNIQUE,                       -- Redis XADD id; idempotent consume
    opportunity_id      TEXT NOT NULL,
    token_pair          TEXT NOT NULL,
    posterior_prob      DOUBLE PRECISION NOT NULL,
    kelly_fraction      DOUBLE PRECISION NOT NULL,
    recommended_usd     DOUBLE PRECISION NOT NULL,         -- HYPOTHETICAL paper sizing, NOT real capital
    net_profit_usd      DOUBLE PRECISION,
    bayesian_accepted   BOOLEAN NOT NULL DEFAULT true,
    prior_log_odds      DOUBLE PRECISION,
    chain_id            INTEGER,
    source_context      TEXT,                              -- e.g. 'flat_prior' | 'calibrated'
    scoring_mode        TEXT NOT NULL DEFAULT 'paper',
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_scored_opportunities_pair_time
    ON scored_opportunities (token_pair, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_scored_opportunities_created
    ON scored_opportunities (created_at DESC);
CREATE INDEX IF NOT EXISTS idx_scored_opportunities_opportunity_id
    ON scored_opportunities (opportunity_id);

COMMENT ON TABLE scored_opportunities IS
    'Gate C paper-only confidence-scoring telemetry. recommended_usd is a hypothetical paper-bankroll figure, never real capital. No live execution.';

-- ---------------------------------------------------------------------------
-- bayesian_priors — per-pair calibrated priors, seeded by the A.5 paper-shadow window.
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS bayesian_priors (
    id                  BIGSERIAL PRIMARY KEY,
    token_pair          TEXT NOT NULL UNIQUE,
    log_odds            DOUBLE PRECISION NOT NULL DEFAULT 0.0,
    observation_count   BIGINT NOT NULL DEFAULT 0,
    profitable_count    BIGINT NOT NULL DEFAULT 0,
    last_updated        TIMESTAMPTZ NOT NULL DEFAULT now()
);

COMMENT ON TABLE bayesian_priors IS
    'Gate C per-pair Bayesian priors calibrated from the A.5 paper-shadow window. observation_count gates the CALIBRATED dashboard state.';

-- ---------------------------------------------------------------------------
-- gate_c_validation — durable evidence channel for A.4 fork validation.
-- A row with gate='a4_fork_validation', status='passed' is written ONLY when the
-- ignored multistep_fork test actually passed. The dashboard reads this (never code alone).
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS gate_c_validation (
    id                  BIGSERIAL PRIMARY KEY,
    gate                TEXT NOT NULL,
    status              TEXT NOT NULL,
    evidence_ref        TEXT,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_gate_c_validation_gate_time
    ON gate_c_validation (gate, created_at DESC);

COMMENT ON TABLE gate_c_validation IS
    'Gate C validation evidence (e.g. A.4 fork validation passed). Written by scripts/run_a4_fork_validation.sh only on a real passing run.';

-- ---------------------------------------------------------------------------
-- gate_c_metrics — per-pair aggregate for the Gate C dashboard.
-- ---------------------------------------------------------------------------
CREATE OR REPLACE VIEW gate_c_metrics AS
SELECT
    token_pair,
    COUNT(*)                                          AS total_scored,
    COUNT(*) FILTER (WHERE bayesian_accepted = true)  AS accepted_total,
    COUNT(*) FILTER (WHERE net_profit_usd > 0)        AS net_positive_total,
    AVG(posterior_prob)                               AS avg_posterior,
    AVG(kelly_fraction)                               AS avg_kelly_fraction,
    MAX(created_at)                                   AS last_scored_at,
    MIN(created_at)                                   AS first_scored_at
FROM scored_opportunities
GROUP BY token_pair;

COMMENT ON VIEW gate_c_metrics IS
    'Gate C telemetry aggregate (paper-only). Read by the A.5 day-7 calibration review.';

DO $$
BEGIN
    RAISE NOTICE 'Migration 097: scored_opportunities + bayesian_priors + gate_c_validation + gate_c_metrics ready (Gate C paper telemetry).';
END $$;

COMMIT;
