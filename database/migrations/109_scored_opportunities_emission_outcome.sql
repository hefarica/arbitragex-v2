-- ArbitrageX v2 — Migration 109: emission-outcome labels on Gate C scoring telemetry
--
-- ARBX-RDY-02 (A.8 scoring wiring): the Rust OpportunityEmitter now scores
-- EVERY paper opportunity — accepted AND rejected — because Bayesian prior
-- calibration needs the negative class (A.5 doctrine). Prod rejects ~100% of
-- candidates (16.7K/day rejected, 0 accepted), so an accept-only scoring feed
-- starved `bayesian_priors` at zero observations. Each record on
-- `arbx:scoring:scored` now carries `emission_outcome` ("accepted" |
-- "rejected") and `rejection_reason` (verbatim, NULL on the accepted path).
--
-- DEFAULT 'accepted' is honest: every pre-existing row came from the accept
-- path (before RDY-02 `score_and_publish` was called only from
-- `emit_accepted`). Idempotent + additive; paper-only telemetry (RULE 00 /
-- §34: capital = 0).
ALTER TABLE scored_opportunities
    ADD COLUMN IF NOT EXISTS emission_outcome TEXT NOT NULL DEFAULT 'accepted',
    ADD COLUMN IF NOT EXISTS rejection_reason TEXT;
