-- ArbitrageX v2 — Migration 057: operator credentials store with health status.
--
-- The /settings/credentials page is the operator's single surface for entering
-- every external credential the platform needs (RPC HTTP/WS per chain,
-- Coingecko, Alchemy Prices, Flashbots signer, BloxRoute, Titan, CEX APIs,
-- GitHub for token-enricher, internal admin/edge tokens). Each row carries:
--
--   - provider      logical type (e.g. "rpc_http", "rpc_ws", "binance",
--                   "flashbots_signer", "coingecko", "alchemy_prices",
--                   "bloxroute", "titan", "github_token", "admin_token",
--                   "edge_token")
--   - scope         "global" or "chain:<id>" (e.g. "chain:1", "chain:42161")
--                   for credentials that vary per chain
--   - secret_value  the credential itself. Stored as TEXT today; migration
--                   058 will add at-rest encryption via pgcrypto once the
--                   Vault transit key is provisioned. The /admin/credentials
--                   GET endpoint NEVER returns this column raw — only the
--                   last-4 character suffix.
--   - status        "untested" | "valid" | "invalid" | "expired"
--   - last_validated_at + last_validation_error  set by the validator
--   - metadata      extra per-provider fields (e.g. binance has both api_key
--                   and api_secret; flashbots_signer carries the derived
--                   address; rpc_http may carry a fallback CSV)
--
-- Doctrine — R8 fail-honest:
--   The status NEVER goes green from "user said so". It only becomes "valid"
--   after the api-server actually executes the provider-specific test and
--   the test passes. Untested = yellow. Invalid = red. Same as readiness.
--
-- No-hardcode discipline (RULE 00):
--   The credentials never appear in source. They are entered through the UI
--   and persist here. The .env on the VPS is still the boot source for now;
--   a follow-up sprint will plumb the searcher-rs runtime to refresh from
--   this table via a Redis-mirrored projection (mirroring the trading_config
--   pattern from migration 056).
--
-- Reversibility:
--   DROP TABLE service_credentials;

BEGIN;

CREATE TABLE IF NOT EXISTS service_credentials (
  id                     UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  provider               TEXT NOT NULL,
  scope                  TEXT NOT NULL DEFAULT 'global',
  display_name           TEXT NOT NULL,
  secret_value           TEXT,
  status                 TEXT NOT NULL DEFAULT 'untested',
  last_validated_at      TIMESTAMPTZ,
  last_validation_error  TEXT,
  metadata               JSONB NOT NULL DEFAULT '{}'::jsonb,
  created_at             TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at             TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  created_by             TEXT,
  updated_by             TEXT,
  CONSTRAINT service_credentials_provider_scope_key UNIQUE (provider, scope),
  CONSTRAINT chk_credentials_status CHECK (status IN ('untested','valid','invalid','expired')),
  CONSTRAINT chk_credentials_metadata_obj CHECK (jsonb_typeof(metadata) = 'object')
);

CREATE INDEX IF NOT EXISTS idx_service_credentials_provider ON service_credentials (provider);
CREATE INDEX IF NOT EXISTS idx_service_credentials_status ON service_credentials (status);
CREATE INDEX IF NOT EXISTS idx_service_credentials_metadata ON service_credentials USING GIN (metadata);

-- updated_at trigger for honest "last edit" tracking (matches trading_config pattern).
CREATE OR REPLACE FUNCTION trg_service_credentials_set_updated_at()
RETURNS TRIGGER AS $$
BEGIN
  NEW.updated_at = NOW();
  RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS service_credentials_set_updated_at ON service_credentials;
CREATE TRIGGER service_credentials_set_updated_at
  BEFORE UPDATE ON service_credentials
  FOR EACH ROW
  EXECUTE FUNCTION trg_service_credentials_set_updated_at();

COMMIT;
