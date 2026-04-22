-- ArbitrageX v2 — Migration 016: risk_policy (operator-editable thresholds, versioned).
--
-- No-hardcode doctrine: recon thresholds and revert-rate triggers must be explicit.
-- Same versioning pattern as execution_policy.

CREATE TABLE IF NOT EXISTS risk_policy (
  id                            UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
  chain_id                      INTEGER     NOT NULL,
  version                       INTEGER     NOT NULL,
  max_revert_rate_pct           NUMERIC(6,3) NOT NULL CHECK (max_revert_rate_pct >= 0 AND max_revert_rate_pct <= 100),
  max_execution_variance_pct    NUMERIC(6,3) NOT NULL CHECK (max_execution_variance_pct >= 0),
  min_token_safety_score        INTEGER     NOT NULL CHECK (min_token_safety_score >= 0 AND min_token_safety_score <= 100),
  simulation_required_for_new_routes BOOLEAN NOT NULL DEFAULT TRUE,
  anomaly_window_minutes        INTEGER     NOT NULL CHECK (anomaly_window_minutes >= 1),
  anomaly_min_samples           INTEGER     NOT NULL CHECK (anomaly_min_samples >= 1),
  anomaly_revert_rate_pct       NUMERIC(6,3) NOT NULL CHECK (anomaly_revert_rate_pct >= 0 AND anomaly_revert_rate_pct <= 100),
  auto_trip_on_high_revert_rate BOOLEAN     NOT NULL DEFAULT TRUE,
  description                   TEXT        NOT NULL,
  activated_by                  TEXT        NOT NULL,
  effective_at                  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  superseded_at                 TIMESTAMPTZ,
  created_at                    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  CONSTRAINT risk_policy_uq UNIQUE (chain_id, version)
);

CREATE INDEX IF NOT EXISTS idx_risk_policy_active
  ON risk_policy(chain_id, effective_at DESC)
  WHERE superseded_at IS NULL;

GRANT SELECT, INSERT, UPDATE ON risk_policy TO arbx_rw;
GRANT SELECT ON risk_policy TO arbx_ro;
