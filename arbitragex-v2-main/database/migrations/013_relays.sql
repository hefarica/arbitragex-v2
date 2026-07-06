-- ArbitrageX v2 — Migration 013: relays (operator-editable catalog).
--
-- No-hardcode doctrine: relays are operator choices, not compile-time decisions.
-- The catalog formerly lived in `configs/app.toml` [[relays]] and is now the
-- single source of truth. The config file becomes a seed-only input used during
-- initial onboarding via `INSERT ... ON CONFLICT DO NOTHING`.
--
-- Fields:
--   name           — unique relay identifier (flashbots / bloxroute / eden / beaver / titan / custom).
--   chain_id       — which chain this relay submits to.
--   endpoint       — POST endpoint URL. NULL or empty means "not yet configured" — cannot be enabled.
--   auth_scheme    — how the caller authenticates (none / x-flashbots-signature / bearer / header).
--   auth_secret_ref — Vault path (NOT the secret itself). e.g. "secret/arbitragex/prod/relays/flashbots_auth"
--   enabled        — on/off switch (constraint: false if endpoint is NULL/empty).
--   priority       — tiebreaker when multiple relays eligible (lower = preferred).
--   notes          — operator-visible memo.

CREATE TABLE IF NOT EXISTS relays (
  id              UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
  name            TEXT        NOT NULL,
  chain_id        INTEGER     NOT NULL,
  endpoint        TEXT,
  auth_scheme     TEXT        NOT NULL DEFAULT 'none'
                              CHECK (auth_scheme IN ('none','x-flashbots-signature','bearer','header-auth','custom')),
  auth_secret_ref TEXT,
  enabled         BOOLEAN     NOT NULL DEFAULT FALSE,
  priority        INTEGER     NOT NULL DEFAULT 100,
  notes           TEXT,
  created_by      TEXT        NOT NULL,
  created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  CONSTRAINT relays_uq_name_chain UNIQUE (name, chain_id),
  CONSTRAINT relays_enabled_needs_endpoint CHECK (enabled = FALSE OR (endpoint IS NOT NULL AND length(endpoint) >= 8))
);

CREATE INDEX IF NOT EXISTS idx_relays_chain_enabled ON relays(chain_id, enabled, priority);

DROP TRIGGER IF EXISTS trg_relays_updated_at ON relays;
CREATE TRIGGER trg_relays_updated_at BEFORE UPDATE ON relays
  FOR EACH ROW EXECUTE FUNCTION arbx_touch_updated_at();

GRANT SELECT, INSERT, UPDATE ON relays TO arbx_rw;
GRANT SELECT ON relays TO arbx_ro;
