-- ============================================================================
-- Migración 068 — Operator Parametrization Sovereignty (C9.4)
-- ----------------------------------------------------------------------------
-- Extiende la 9-Layer Coherence Rule añadiendo L8 (Operator Authz) y
-- L9 (Operator Audit) sobre la 7-Layer base (Frontend → API → Handler →
-- PG/Redis → Hot-reload → runtime_ack → Audit).
--
-- Doctrina:
--   - Cap USD del operador inicia en 0.00 (Ghost Protocol absoluto).
--   - Sólo `sovereign` puede elevar cap_usd_ceiling mediante firma criptográfica.
--   - `observer` no muta nada. `steward` muta sólo dentro de allowed_*.
--   - Cada acción operacional registra operator_id + signing_pubkey en audit_event.
-- ============================================================================

BEGIN;

CREATE TABLE IF NOT EXISTS operator_parametrization (
  operator_id          TEXT PRIMARY KEY,
  display_name         TEXT NOT NULL,
  signing_pubkey       TEXT NOT NULL UNIQUE,
  role                 TEXT NOT NULL CHECK (role IN ('observer','steward','sovereign')),
  cap_usd_ceiling      NUMERIC(20,6) NOT NULL DEFAULT 0.00
                         CHECK (cap_usd_ceiling >= 0.00),
  allowed_chains       JSONB NOT NULL DEFAULT '[]'::jsonb,
  allowed_registries   JSONB NOT NULL DEFAULT '[]'::jsonb,
  ui_preferences       JSONB NOT NULL DEFAULT '{}'::jsonb,
  feature_overrides    JSONB NOT NULL DEFAULT '{}'::jsonb,
  enabled              BOOLEAN NOT NULL DEFAULT TRUE,
  created_at           TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at           TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  config_hash          TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_op_param_role        ON operator_parametrization(role);
CREATE INDEX IF NOT EXISTS idx_op_param_enabled     ON operator_parametrization(enabled);
CREATE INDEX IF NOT EXISTS idx_op_param_signing_pk  ON operator_parametrization(signing_pubkey);

CREATE TABLE IF NOT EXISTS operator_session_state (
  session_id           TEXT PRIMARY KEY,
  operator_id          TEXT NOT NULL REFERENCES operator_parametrization(operator_id) ON DELETE CASCADE,
  active_chain_id      INTEGER,
  active_registry      TEXT,
  filters_state        JSONB NOT NULL DEFAULT '{}'::jsonb,
  last_seen            TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  created_at           TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_op_session_operator  ON operator_session_state(operator_id);
CREATE INDEX IF NOT EXISTS idx_op_session_last_seen ON operator_session_state(last_seen);

-- ---------------------------------------------------------------------------
-- Extiende audit_event con columnas L8/L9 si no existen
-- ---------------------------------------------------------------------------
DO $$
BEGIN
  IF EXISTS (SELECT 1 FROM information_schema.tables WHERE table_name='audit_event') THEN
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns
                   WHERE table_name='audit_event' AND column_name='operator_id') THEN
      ALTER TABLE audit_event ADD COLUMN operator_id TEXT;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns
                   WHERE table_name='audit_event' AND column_name='operator_pubkey') THEN
      ALTER TABLE audit_event ADD COLUMN operator_pubkey TEXT;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns
                   WHERE table_name='audit_event' AND column_name='operator_role') THEN
      ALTER TABLE audit_event ADD COLUMN operator_role TEXT;
    END IF;
  END IF;
END $$;

-- ---------------------------------------------------------------------------
-- Seed: operador soberano de bootstrap con cap $0.00 (Ghost Protocol).
-- ---------------------------------------------------------------------------
INSERT INTO operator_parametrization
  (operator_id, display_name, signing_pubkey, role, cap_usd_ceiling,
   allowed_chains, allowed_registries, ui_preferences, feature_overrides, config_hash)
VALUES
  ('op_bootstrap_sovereign',
   'Bootstrap Sovereign',
   '0x0000000000000000000000000000000000000000000000000000000000000001',
   'sovereign',
   0.00,
   '[]'::jsonb,
   '["chain","rpc","dex","token","pool","wallet","strategy","contract","risk_gate","capital_gate","relay","agent"]'::jsonb,
   '{"locale":"es","density":"comfortable"}'::jsonb,
   '{}'::jsonb,
   'sha256:bootstrap')
ON CONFLICT (operator_id) DO NOTHING;

-- ---------------------------------------------------------------------------
-- feature_manifest: registra la propia capacidad de soberanía como feature
-- ---------------------------------------------------------------------------
DO $$
BEGIN
  IF EXISTS (SELECT 1 FROM information_schema.tables WHERE table_name='feature_manifest') THEN
    ALTER TABLE feature_manifest 
      ADD COLUMN IF NOT EXISTS ui_path TEXT,
      ADD COLUMN IF NOT EXISTS backend_route TEXT,
      ADD COLUMN IF NOT EXISTS requires_layers JSONB DEFAULT '[]'::jsonb,
      ADD COLUMN IF NOT EXISTS enabled BOOLEAN DEFAULT TRUE;

    -- NOTE: feature_manifest (mig 067) requires layer/state_hash/panel_path
    -- (all NOT NULL, no default). Omitting them aborted the chain at 068.
    -- Provide them using 067's seed pattern (layer='backend', state_hash=zeros,
    -- panel_path mirrors ui_path). ON CONFLICT (feature_key) is valid — 067:87
    -- declares feature_key NOT NULL UNIQUE.
    INSERT INTO feature_manifest
      (feature_key, ui_path, backend_route, requires_layers, enabled, description,
       layer, state_hash, panel_path)
    VALUES
      ('operator_parametrization', '/omega-s5/operator', '/api/operator/me',
       '["api","handler","pg","authz","audit"]'::jsonb, TRUE,
       'Operator Parametrization Sovereignty (C9.4)',
       'backend', repeat('0',64), '/omega-s5/operator'),
      ('operator_gate_authz', '/omega-s5/registry', '/api/operator/authorize',
       '["api","handler","authz","audit"]'::jsonb, TRUE,
       '9-Layer Coherence — L8 Operator Authz / L9 Operator Audit',
       'backend', repeat('0',64), '/omega-s5/registry')
    ON CONFLICT (feature_key) DO NOTHING;
  END IF;
END $$;

COMMIT;
