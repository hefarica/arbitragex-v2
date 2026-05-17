-- ArbitrageX v2 — Migration 034: tokens registry (additive reconciliation)
--
-- BUG-FIX iter 12 (OMEGA-S5): the original 034 used
-- `CREATE TABLE IF NOT EXISTS tokens (...)`, but migration 021 already
-- creates `tokens` with a different (legacy) schema:
--   id UUID PRIMARY KEY, chain_id, address VARCHAR(42), symbol NOT NULL,
--   decimals NOT NULL, is_stablecoin, is_active, risk_level, created_at,
--   UNIQUE (chain_id, address).
-- The IF NOT EXISTS guard silently skipped the new schema, then
-- `CREATE INDEX ... ON tokens(last_seen_at DESC)` failed with
-- `column "last_seen_at" does not exist`.
--
-- Doctrinal fix forward (no skip, no relax, no destruction of data):
-- reconcile additively so the surviving table satisfies BOTH legacy
-- consumers (pre_execute_checklist.rs uses is_active, pool_sync_worker
-- uses is_active, pool_discovery uses id UUID) AND the new token_enricher
-- contract validated by backend/api-server/test/migrations.test.ts
-- (resolved_via NOT NULL, last_seen_at present, chk_address_format
-- rejects non-lowercase, UNIQUE/PK semantics on (chain_id, address)).
--
-- Idempotent: re-running is a no-op.

-- 1. Drop NOT NULL on symbol/decimals so the legacy 021 schema matches
--    the contract verified by migrations.test.ts ("is_nullable: YES").
--    The legacy 021 inserts (seeds 029/030) all provide symbol/decimals,
--    so existing rows remain valid.
ALTER TABLE tokens
  ALTER COLUMN symbol   DROP NOT NULL,
  ALTER COLUMN decimals DROP NOT NULL;

-- 2. Add the new columns required by token_enricher and tokenResolver.
--    All start nullable so the ADD does not fail on pre-existing rows;
--    we then backfill and tighten NOT NULL where the contract demands.
ALTER TABLE tokens
  ADD COLUMN IF NOT EXISTS logo_url      TEXT          NULL,
  ADD COLUMN IF NOT EXISTS resolved_via  TEXT          NULL,
  ADD COLUMN IF NOT EXISTS resolved_at   TIMESTAMPTZ   NULL,
  ADD COLUMN IF NOT EXISTS last_seen_at  TIMESTAMPTZ   NULL;

-- 3. Backfill resolved_via for any rows inserted by legacy seeds (021,
--    029, 030) before token_enricher took over. We mark them as
--    'onchain_partial' (data came from a chain registry but without the
--    full enricher pipeline) to preserve provenance honestly.
UPDATE tokens
   SET resolved_via = COALESCE(resolved_via, 'onchain_partial'),
       resolved_at  = COALESCE(resolved_at, created_at, NOW()),
       last_seen_at = COALESCE(last_seen_at, created_at, NOW())
 WHERE resolved_via IS NULL
    OR resolved_at  IS NULL
    OR last_seen_at IS NULL;

-- 4. Lock defaults and NOT NULL constraints as required by the contract.
ALTER TABLE tokens
  ALTER COLUMN resolved_at  SET DEFAULT NOW(),
  ALTER COLUMN last_seen_at SET DEFAULT NOW();

ALTER TABLE tokens
  ALTER COLUMN resolved_via SET NOT NULL,
  ALTER COLUMN resolved_at  SET NOT NULL,
  ALTER COLUMN last_seen_at SET NOT NULL;

-- 5. Constrain resolved_via to the canonical enum.
DO $$ BEGIN
  ALTER TABLE tokens
    ADD CONSTRAINT chk_tokens_resolved_via_enum
    CHECK (resolved_via IN (
      'onchain_full','onchain_partial','trustwallet_only','failed'
    ));
EXCEPTION WHEN duplicate_object THEN NULL; END $$;

-- 6. Address must be 0x-prefixed lowercase 40-hex (token_enricher
--    invariant; matches migrations.test.ts "rejects non-lowercase").
DO $$ BEGIN
  ALTER TABLE tokens
    ADD CONSTRAINT chk_address_format
    CHECK (address ~ '^0x[a-f0-9]{40}$');
EXCEPTION WHEN duplicate_object THEN NULL; END $$;

-- 7. Index for token_enricher's last-seen sweep.
CREATE INDEX IF NOT EXISTS idx_tokens_last_seen ON tokens(last_seen_at DESC);

-- 8. Grants for runtime roles.
GRANT SELECT, INSERT, UPDATE ON tokens TO arbx_rw;
GRANT SELECT ON tokens TO arbx_ro;
