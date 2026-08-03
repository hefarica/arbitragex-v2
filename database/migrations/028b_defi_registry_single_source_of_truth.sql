-- ArbitrageX v2 — Migration 028b: Single Source of Truth for canonical token addresses.
--
-- Doctrine (RULE 00 / arbx-no-hardcode-doctrine): no token address is resolved by
-- bare symbol in seed INSERTs. Canonical mainnet addresses live in ONE registry
-- table, version-controlled here, appended before the seeds (029+) consume it.
-- The token-enricher remains the dynamic on-chain confirmation path (resolved_via);
-- this registry is the operator-curated anti-corruption anchor for blue-chip tokens.
--
-- Hardening (anti-tamper / anti-sabotage):
--   * PK is the lowercase 0x address (the natural key). A duplicate symbol with a
--     DIFFERENT address cannot collide with or overwrite the canonical row.
--   * CHECK constraints pin address format (lowercase, 0x + 40 hex), symbol length
--     and decimals range.
--   * seed_version tracks which registry snapshot inserted the row, giving an audit
--     trail for any drift. Bump REGISTRY_SEED_VERSION on every curated change.
--   * INSERT ... ON CONFLICT (address) DO NOTHING: re-runs never mutate curated data.
--     A curated change is an explicit, reviewed UPDATE in a future migration — never
--     an implicit side effect of a re-run.
--   * A self-check aborts the migration loudly if any (chain_id, symbol) maps to
--     more than one active address. Fail-fast prevents silent drift poisoning every
--     downstream pool/route seed.

BEGIN;

CREATE TABLE IF NOT EXISTS defi_registry (
    address         TEXT PRIMARY KEY
                    CHECK (address ~ '^0x[a-f0-9]{40}$'),
    symbol          TEXT NOT NULL
                    CHECK (char_length(symbol) BETWEEN 1 AND 20),
    chain_id        INTEGER NOT NULL DEFAULT 1
                    CHECK (chain_id > 0),
    decimals        SMALLINT NOT NULL
                    CHECK (decimals BETWEEN 0 AND 36),
    verified_source TEXT NOT NULL DEFAULT 'operator_canonical_v1',
    seed_version    INTEGER NOT NULL DEFAULT 1,
    last_verified   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    is_active       BOOLEAN NOT NULL DEFAULT TRUE,
    UNIQUE (chain_id, symbol)
);

COMMENT ON TABLE defi_registry IS
  'Single Source of Truth for canonical token addresses. Seeds resolve by (symbol, address) here, never by bare symbol. Curated, append-only via migrations; the token-enricher confirms on-chain.';

-- Canonical Ethereum mainnet blue-chip set. Stored lowercase to match the
-- tokens.address normalization (CHECK on tokens enforces ^0x[0-9a-f]{40}$).
INSERT INTO defi_registry (address, symbol, chain_id, decimals, verified_source, seed_version) VALUES
  ('0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2', 'WETH',  1, 18, 'operator_canonical_v1', 1),
  ('0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48', 'USDC',  1,  6, 'operator_canonical_v1', 1),
  ('0xdac17f958d2ee523a2206206994597c13d831ec7', 'USDT',  1,  6, 'operator_canonical_v1', 1),
  ('0x6b175474e89094c44da98b954eedeac495271d0f', 'DAI',   1, 18, 'operator_canonical_v1', 1),
  ('0x2260fac5e5542a773aa44fbcfedf7c193bc2c599', 'WBTC',  1,  8, 'operator_canonical_v1', 1),
  ('0x7d1afa7b718fb893db30a3abc0cfc608aacfebb0', 'MATIC', 1, 18, 'operator_canonical_v1', 1),
  ('0x514910771af9ca656af840dff83e8264ecf986ca', 'LINK',  1, 18, 'operator_canonical_v1', 1),
  ('0x1f9840a85d5af5bf1d1762f925bdaddc4201f984', 'UNI',   1, 18, 'operator_canonical_v1', 1),
  ('0x9f8f72aa9304c8b593d555f12ef6589cc3a579a2', 'MKR',   1, 18, 'operator_canonical_v1', 1),
  ('0x7fc66500c84a76ad7e9c93437bfc5ac33e2ddae9', 'AAVE',  1, 18, 'operator_canonical_v1', 1),
  ('0x6b3595068778dd592e39a122f4f5a5cf09c90fe2', 'SUSHI', 1, 18, 'operator_canonical_v1', 1),
  ('0xc00e94cb662c3520282e6f5717214004a7f26888', 'COMP',  1, 18, 'operator_canonical_v1', 1),
  ('0x95ad61b0a150d79219dcf64e1e6cc01f0b64c4ce', 'SHIB',  1, 18, 'operator_canonical_v1', 1),
  ('0x6982508145454ce325ddbe47a25d4ec3d2311933', 'PEPE',  1, 18, 'operator_canonical_v1', 1)
ON CONFLICT (address) DO NOTHING;

-- Integrity self-check: abort loudly if a (chain_id, symbol) now maps to >1 active
-- address. Silent drift here would poison every pool seed that resolves by symbol.
DO $$
DECLARE
  bad INT;
BEGIN
  SELECT COUNT(*) INTO bad
  FROM (
    SELECT chain_id, symbol
    FROM defi_registry
    WHERE is_active
    GROUP BY chain_id, symbol
    HAVING COUNT(*) > 1
  ) dup;
  IF bad > 0 THEN
    RAISE EXCEPTION 'defi_registry integrity violation: % (chain_id,symbol) pairs map to >1 active address', bad;
  END IF;
END $$;

COMMIT;
