-- ArbitrageX v2 — Migration 018: onboarding_progress (progressive solicitation state).
--
-- Tracks which onboarding phase (1–5) each operator has completed. Gates feature
-- activation: a feature in phase N requires onboarding_progress.phase_<N>_completed_at
-- to be non-null.
--
-- Only one row for the operator org is expected (single-tenant platform); kept
-- as a table (not a config row) so future multi-operator lands cleanly.

CREATE TABLE IF NOT EXISTS onboarding_progress (
  id                          UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
  org_id                      TEXT        NOT NULL DEFAULT 'default',

  -- Phase 1 — install & boot (tokens, Vault unsealed, Grafana pwd).
  phase_1_completed_at        TIMESTAMPTZ,
  phase_1_completed_by        TEXT,
  phase_1_vault_sealed_healthy BOOLEAN    NOT NULL DEFAULT FALSE,

  -- Phase 2 — connect (RPC, Slack, Grafana).
  phase_2_completed_at        TIMESTAMPTZ,
  phase_2_completed_by        TEXT,
  phase_2_rpc_probe_ok        BOOLEAN     NOT NULL DEFAULT FALSE,

  -- Phase 3 — advanced (sim provider, token safety, scoring weights, blacklist).
  phase_3_completed_at        TIMESTAMPTZ,
  phase_3_completed_by        TEXT,

  -- Phase 4 — real testing (relays, flashbots signer, CF tunnel).
  phase_4_completed_at        TIMESTAMPTZ,
  phase_4_completed_by        TEXT,
  phase_4_signer_zero_balance_verified BOOLEAN NOT NULL DEFAULT FALSE,

  -- Phase 5 — production (PagerDuty, B2, age, incident contacts, paper-mode off).
  phase_5_completed_at        TIMESTAMPTZ,
  phase_5_completed_by        TEXT,
  phase_5_paper_mode_off_at   TIMESTAMPTZ, -- the moment paper_mode was flipped off

  created_at                  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at                  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  CONSTRAINT onboarding_progress_uq_org UNIQUE (org_id)
);

DROP TRIGGER IF EXISTS trg_onboarding_progress_updated_at ON onboarding_progress;
CREATE TRIGGER trg_onboarding_progress_updated_at BEFORE UPDATE ON onboarding_progress
  FOR EACH ROW EXECUTE FUNCTION arbx_touch_updated_at();

-- Bootstrap: seed the singleton row so api-server doesn't need upsert-on-boot.
INSERT INTO onboarding_progress (org_id) VALUES ('default')
  ON CONFLICT (org_id) DO NOTHING;

GRANT SELECT, INSERT, UPDATE ON onboarding_progress TO arbx_rw;
GRANT SELECT ON onboarding_progress TO arbx_ro;
