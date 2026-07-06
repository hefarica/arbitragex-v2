-- ArbitrageX v2 — Migration 008: token_safety_cache
-- TTL-based cache of external token safety assessments (GoPlus, honeypot.is, internal).

CREATE TABLE IF NOT EXISTS token_safety_cache (
  chain_id INTEGER NOT NULL,
  token_address TEXT NOT NULL,
  safety_score INTEGER NOT NULL CHECK (safety_score BETWEEN 0 AND 100),
  flags JSONB NOT NULL DEFAULT '{}'::jsonb,
  source TEXT NOT NULL,
  ttl_expires_at TIMESTAMPTZ NOT NULL,
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  PRIMARY KEY (chain_id, token_address)
);

CREATE INDEX IF NOT EXISTS idx_token_expiry ON token_safety_cache(ttl_expires_at);

GRANT SELECT, INSERT, UPDATE, DELETE ON token_safety_cache TO arbx_rw;
GRANT SELECT ON token_safety_cache TO arbx_ro;
