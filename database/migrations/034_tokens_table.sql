-- ArbitrageX v2 — Migration 034: tokens registry
-- Multi-chain by design (PK compound). Populated by token_enricher_worker.
-- R8 fail-honest: each NULL field means "we tried but couldn't resolve".

CREATE TABLE IF NOT EXISTS tokens (
  chain_id      INTEGER     NOT NULL,
  address       TEXT        NOT NULL,
  symbol        TEXT        NULL,
  decimals      SMALLINT    NULL,
  logo_url      TEXT        NULL,
  resolved_via  TEXT        NOT NULL
    CHECK (resolved_via IN ('onchain_full','onchain_partial','trustwallet_only','failed')),
  resolved_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  last_seen_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  PRIMARY KEY (chain_id, address),
  CONSTRAINT chk_address_format CHECK (address ~ '^0x[a-f0-9]{40}$')
);

CREATE INDEX IF NOT EXISTS idx_tokens_last_seen ON tokens(last_seen_at DESC);

GRANT SELECT, INSERT, UPDATE ON tokens TO arbx_rw;
GRANT SELECT ON tokens TO arbx_ro;
