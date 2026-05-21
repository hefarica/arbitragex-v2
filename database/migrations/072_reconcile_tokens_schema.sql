-- ArbitrageX v2 — Migration 072: reconcile the tokens schema (progressive discovery)
--
-- Canonical domain truth (single source):
--   Discovered token = address + chain_id          (REQUIRED)
--   Enriched token   = + symbol + decimals + name  (OPTIONAL — filled later by
--                       RPC / indexer / oracle / metadata service)
--
-- Why this exists:
--   Migration 021 created `tokens` too strictly for an MEV/arbitrage system:
--   symbol/decimals were NOT NULL and chain_id was nullable, which blocks the
--   common flow of discovering a token by address/pool/event BEFORE its
--   metadata is resolved. Migration 034 then added the fail-honest enrichment
--   columns (logo_url, resolved_via, ...) assuming the relaxed model. The two
--   disagreed, so any insert of a freshly-discovered token failed.
--
--   This is a FORWARD-ONLY reconciliation: it does not edit the historical
--   migrations 021/034, never drops or truncates the table, and never loses
--   data. Every step is idempotent (guarded against re-runs) and defensive
--   (never fails on pre-existing production rows).

DO $$
BEGIN
  -- 1. symbol may be NULL until enriched.
  IF EXISTS (SELECT 1 FROM information_schema.columns
             WHERE table_name = 'tokens' AND column_name = 'symbol'
               AND is_nullable = 'NO') THEN
    ALTER TABLE tokens ALTER COLUMN symbol DROP NOT NULL;
  END IF;

  -- 2. decimals may be NULL until enriched.
  IF EXISTS (SELECT 1 FROM information_schema.columns
             WHERE table_name = 'tokens' AND column_name = 'decimals'
               AND is_nullable = 'NO') THEN
    ALTER TABLE tokens ALTER COLUMN decimals DROP NOT NULL;
  END IF;

  -- 3. name may be NULL until enriched (only if the column exists).
  IF EXISTS (SELECT 1 FROM information_schema.columns
             WHERE table_name = 'tokens' AND column_name = 'name'
               AND is_nullable = 'NO') THEN
    ALTER TABLE tokens ALTER COLUMN name DROP NOT NULL;
  END IF;

  -- 4. chain_id is REQUIRED in the multi-chain model. Enforce only when no
  --    existing NULL rows would be broken (defensive: never fail on prod data).
  IF EXISTS (SELECT 1 FROM information_schema.columns
             WHERE table_name = 'tokens' AND column_name = 'chain_id'
               AND is_nullable = 'YES')
     AND NOT EXISTS (SELECT 1 FROM tokens WHERE chain_id IS NULL) THEN
    ALTER TABLE tokens ALTER COLUMN chain_id SET NOT NULL;
  END IF;

  -- 5. resolved_via is REQUIRED (034 added it WITH a default, backfilling
  --    existing rows). Backfill any stragglers, then enforce NOT NULL.
  IF EXISTS (SELECT 1 FROM information_schema.columns
             WHERE table_name = 'tokens' AND column_name = 'resolved_via') THEN
    UPDATE tokens SET resolved_via = 'failed' WHERE resolved_via IS NULL;
    IF EXISTS (SELECT 1 FROM information_schema.columns
               WHERE table_name = 'tokens' AND column_name = 'resolved_via'
                 AND is_nullable = 'YES') THEN
      ALTER TABLE tokens ALTER COLUMN resolved_via SET NOT NULL;
    END IF;
  END IF;

  -- 6. EVM address format guard: lowercase `0x` + 40 hex chars. NOT VALID so
  --    pre-existing rows are never broken, but every NEW insert/update is
  --    checked (the discovery path must store normalized lowercase addresses).
  IF NOT EXISTS (SELECT 1 FROM pg_constraint
                 WHERE conname = 'chk_address_format'
                   AND conrelid = 'tokens'::regclass) THEN
    ALTER TABLE tokens
      ADD CONSTRAINT chk_address_format
      CHECK (address ~ '^0x[0-9a-f]{40}$') NOT VALID;
  END IF;

  -- 7. Preserve uniqueness of (chain_id, address). 021 already adds it inline;
  --    this is a defensive no-op when an equivalent unique index is present.
  IF NOT EXISTS (SELECT 1 FROM pg_indexes
                 WHERE tablename = 'tokens'
                   AND indexdef ~ '\(chain_id, address\)') THEN
    CREATE UNIQUE INDEX IF NOT EXISTS uq_tokens_chain_address
      ON tokens (chain_id, address);
  END IF;
END $$;
