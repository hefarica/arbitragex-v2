-- ArbitrageX v2 — Migration 034: tokens registry
-- Multi-chain by design (PK compound). Populated by token_enricher_worker.
-- R8 fail-honest: each NULL field means "we tried but couldn't resolve".

ALTER TABLE tokens
    ADD COLUMN IF NOT EXISTS logo_url TEXT NULL,
    ADD COLUMN IF NOT EXISTS resolved_via TEXT DEFAULT 'onchain_full' CHECK (resolved_via IN ('onchain_full','onchain_partial','trustwallet_only','failed')),
    ADD COLUMN IF NOT EXISTS resolved_at TIMESTAMPTZ DEFAULT NOW(),
    ADD COLUMN IF NOT EXISTS last_seen_at TIMESTAMPTZ DEFAULT NOW();

CREATE INDEX IF NOT EXISTS idx_tokens_last_seen ON tokens(last_seen_at DESC);

GRANT SELECT, INSERT, UPDATE ON tokens TO arbx_rw;
GRANT SELECT ON tokens TO arbx_ro;
