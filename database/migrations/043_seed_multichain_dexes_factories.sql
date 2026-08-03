-- ArbitrageX v2 — Migration 043: Phase 2 multichain catalog
-- Seeds 5 missing chains, relaxes dexes protocol_type CHECK constraint,
-- inserts 15 new DEX rows, and maps ~33 factory rows across 6 chains.
--
-- Design: top-6 DEXes by 24h volume per chain (researched 2025/2026).
-- Protocol-type mapping rationale:
--   SOLIDLY    — covers Velodrome V2, Aerodrome, THENA (all Solidly forks)
--   TRADERJOE_LB — TraderJoe Liquidity Book (concentrated liquidity, not V3-compatible)
--   PancakeSwap V2/V3, Quickswap V2/V3, Camelot V3, BiSwap, ApeSwap,
--   BaseSwap, Beethoven X — forks of Uniswap or Balancer; use existing types.
--
-- Idempotent: every INSERT uses ON CONFLICT DO NOTHING.
-- Reversible: see DOWN block comment at the end of this file.
--
-- Factory addresses are canonical/widely-documented mainnet values.
-- Addresses flagged UNVERIFIED are catalog-level placeholders; the operator
-- should replace them with on-chain-validated addresses before enabling
-- pool_sync_worker for those chains.

BEGIN;

-- ─────────────────────────────────────────────────────────────────────────────
-- 1. SEED 5 MISSING CHAINS
--    native_currency is NOT NULL in the DDL (021_defi_registries.sql).
-- ─────────────────────────────────────────────────────────────────────────────
INSERT INTO chains (chain_id, name, native_currency, explorer_url, is_active) VALUES
  (10,    'optimism',  'ETH',   'https://optimistic.etherscan.io', true),
  (56,    'bsc',       'BNB',   'https://bscscan.com',             true),
  (137,   'polygon',   'MATIC', 'https://polygonscan.com',         true),
  (8453,  'base',      'ETH',   'https://basescan.org',            true),
  (42161, 'arbitrum',  'ETH',   'https://arbiscan.io',             true)
ON CONFLICT (chain_id) DO NOTHING;

-- ─────────────────────────────────────────────────────────────────────────────
-- 2. RELAX CHECK CONSTRAINT ON dexes.protocol_type
--    Original: UNISWAP_V2, UNISWAP_V3, CURVE, BALANCER
--    Added: SOLIDLY, TRADERJOE_LB
-- ─────────────────────────────────────────────────────────────────────────────
ALTER TABLE dexes DROP CONSTRAINT IF EXISTS dexes_protocol_type_check;
ALTER TABLE dexes ADD CONSTRAINT dexes_protocol_type_check
  CHECK (protocol_type IN (
    'UNISWAP_V2',
    'UNISWAP_V3',
    'UNISWAP_V4',
    'CURVE',
    'BALANCER',
    'SOLIDLY',
    'TRADERJOE_LB',
    'FLUID_VAULT'
  ));

-- ─────────────────────────────────────────────────────────────────────────────
-- 3. INSERT NEW DEXes
--    SushiSwap / UniswapV2 / UniswapV3 already exist — excluded.
-- ─────────────────────────────────────────────────────────────────────────────
INSERT INTO dexes (name, protocol_type, is_active) VALUES
  ('Curve',          'CURVE',        true),
  ('Balancer',       'BALANCER',     true),
  ('PancakeSwap V3', 'UNISWAP_V3',   true),
  ('PancakeSwap V2', 'UNISWAP_V2',   true),
  ('THENA',          'SOLIDLY',      true),
  ('BiSwap',         'UNISWAP_V2',   true),
  ('ApeSwap',        'UNISWAP_V2',   true),
  ('Quickswap V3',   'UNISWAP_V3',   true),
  ('Quickswap V2',   'UNISWAP_V2',   true),
  ('Camelot V3',     'UNISWAP_V3',   true),
  ('TraderJoe',      'TRADERJOE_LB', true),
  ('Velodrome V2',   'SOLIDLY',      true),
  ('Beethoven X',    'BALANCER',     true),
  ('Aerodrome',      'SOLIDLY',      true),
  ('BaseSwap',       'UNISWAP_V2',   true)
ON CONFLICT (name) DO NOTHING;

-- ─────────────────────────────────────────────────────────────────────────────
-- 4. FACTORY ROWS
--    Schema: (id UUID PK, dex_id UUID, chain_id INT, address VARCHAR(42),
--             created_at TIMESTAMPTZ, UNIQUE(chain_id, address))
--    Note: factories table has NO is_active column.
--    All addresses lowercase, 42 chars.
-- ─────────────────────────────────────────────────────────────────────────────

-- ── Ethereum chain_id=1 (3 existing rows: UniswapV2, SushiSwap, UniswapV3)
--    Adding: Curve, Balancer, PancakeSwap V3

INSERT INTO factories (dex_id, chain_id, address)
SELECT id, 1, '0xb9fc157394af804a3578134a6585c0dc9cc990d4'
FROM dexes WHERE name = 'Curve'
ON CONFLICT (chain_id, address) DO NOTHING;

INSERT INTO factories (dex_id, chain_id, address)
SELECT id, 1, '0xba12222222228d8ba445958a75a0704d566bf2c8'
FROM dexes WHERE name = 'Balancer'
ON CONFLICT (chain_id, address) DO NOTHING;

INSERT INTO factories (dex_id, chain_id, address)
SELECT id, 1, '0x0bfbcf9fa4f9c56b0f40a671ad40e0805a091865'
FROM dexes WHERE name = 'PancakeSwap V3'
ON CONFLICT (chain_id, address) DO NOTHING;

-- ── BSC chain_id=56 (6 rows)

INSERT INTO factories (dex_id, chain_id, address)
SELECT id, 56, '0x0bfbcf9fa4f9c56b0f40a671ad40e0805a091865'
FROM dexes WHERE name = 'PancakeSwap V3'
ON CONFLICT (chain_id, address) DO NOTHING;

INSERT INTO factories (dex_id, chain_id, address)
SELECT id, 56, '0xca143ce32fe78f1f7019d7d551a6402fc5350c73'
FROM dexes WHERE name = 'PancakeSwap V2'
ON CONFLICT (chain_id, address) DO NOTHING;

-- THENA factory (Solidly-fork on BSC)
INSERT INTO factories (dex_id, chain_id, address)
SELECT id, 56, '0x306f06c147f064a010530292a1eb6737c3e378e4'
FROM dexes WHERE name = 'THENA'
ON CONFLICT (chain_id, address) DO NOTHING;

INSERT INTO factories (dex_id, chain_id, address)
SELECT id, 56, '0x858e3312ed3a876947ea49d572a7c42de08af7ee'
FROM dexes WHERE name = 'BiSwap'
ON CONFLICT (chain_id, address) DO NOTHING;

INSERT INTO factories (dex_id, chain_id, address)
SELECT id, 56, '0x0841bd0b734e4f5853f0dd8d7ea041c241fb0da6'
FROM dexes WHERE name = 'ApeSwap'
ON CONFLICT (chain_id, address) DO NOTHING;

-- SushiSwap on BSC — same factory address as all non-Ethereum deployments
INSERT INTO factories (dex_id, chain_id, address)
SELECT id, 56, '0xc35dadb65012ec5796536bd9864ed8773abc74c4'
FROM dexes WHERE name = 'SushiSwap'
ON CONFLICT (chain_id, address) DO NOTHING;

-- ── Polygon chain_id=137 (6 rows)

-- UniswapV3 on Polygon — same canonical address as Ethereum
INSERT INTO factories (dex_id, chain_id, address)
SELECT id, 137, '0x1f98431c8ad98523631ae4a59f267346ea31f984'
FROM dexes WHERE name = 'UniswapV3'
ON CONFLICT (chain_id, address) DO NOTHING;

-- Quickswap V3 (Algebra-based) factory
INSERT INTO factories (dex_id, chain_id, address)
SELECT id, 137, '0x411b0facc3489691f28ad58c47006af5e3ab3a28'
FROM dexes WHERE name = 'Quickswap V3'
ON CONFLICT (chain_id, address) DO NOTHING;

-- Quickswap V2 factory
INSERT INTO factories (dex_id, chain_id, address)
SELECT id, 137, '0x5757371414417b8c6caad45baef941abc7d3ab32'
FROM dexes WHERE name = 'Quickswap V2'
ON CONFLICT (chain_id, address) DO NOTHING;

-- SushiSwap on Polygon
INSERT INTO factories (dex_id, chain_id, address)
SELECT id, 137, '0xc35dadb65012ec5796536bd9864ed8773abc74c4'
FROM dexes WHERE name = 'SushiSwap'
ON CONFLICT (chain_id, address) DO NOTHING;

-- Curve address registry / factory on Polygon (UNVERIFIED — placeholder)
INSERT INTO factories (dex_id, chain_id, address)
SELECT id, 137, '0x722272d36ef0da72ff51c5a65db7b870e2e8d4ee'
FROM dexes WHERE name = 'Curve'
ON CONFLICT (chain_id, address) DO NOTHING;

-- Balancer vault on Polygon (same address as Ethereum)
INSERT INTO factories (dex_id, chain_id, address)
SELECT id, 137, '0xba12222222228d8ba445958a75a0704d566bf2c8'
FROM dexes WHERE name = 'Balancer'
ON CONFLICT (chain_id, address) DO NOTHING;

-- ── Arbitrum chain_id=42161 (6 rows)

-- UniswapV3 on Arbitrum — same canonical address
INSERT INTO factories (dex_id, chain_id, address)
SELECT id, 42161, '0x1f98431c8ad98523631ae4a59f267346ea31f984'
FROM dexes WHERE name = 'UniswapV3'
ON CONFLICT (chain_id, address) DO NOTHING;

-- Camelot V3 factory (UNVERIFIED — address circulates in public docs)
INSERT INTO factories (dex_id, chain_id, address)
SELECT id, 42161, '0x1a3c9b1d2f0529d97f2afc5136cc23e58f1fd35b'
FROM dexes WHERE name = 'Camelot V3'
ON CONFLICT (chain_id, address) DO NOTHING;

-- SushiSwap on Arbitrum
INSERT INTO factories (dex_id, chain_id, address)
SELECT id, 42161, '0xc35dadb65012ec5796536bd9864ed8773abc74c4'
FROM dexes WHERE name = 'SushiSwap'
ON CONFLICT (chain_id, address) DO NOTHING;

-- TraderJoe LB factory on Arbitrum (UNVERIFIED — v2.1 factory address)
INSERT INTO factories (dex_id, chain_id, address)
SELECT id, 42161, '0x8e42f2f4101563bf679975178e880fd87d3efd4e'
FROM dexes WHERE name = 'TraderJoe'
ON CONFLICT (chain_id, address) DO NOTHING;

-- Curve factory on Arbitrum (UNVERIFIED)
INSERT INTO factories (dex_id, chain_id, address)
SELECT id, 42161, '0xb17b674d9c5cb2e441f8e196a2f048a81355d031'
FROM dexes WHERE name = 'Curve'
ON CONFLICT (chain_id, address) DO NOTHING;

-- Balancer vault on Arbitrum (same address as Ethereum)
INSERT INTO factories (dex_id, chain_id, address)
SELECT id, 42161, '0xba12222222228d8ba445958a75a0704d566bf2c8'
FROM dexes WHERE name = 'Balancer'
ON CONFLICT (chain_id, address) DO NOTHING;

-- ── Optimism chain_id=10 (5 rows — Aerodrome is Base-native, not on Optimism)

-- Velodrome V2 factory on Optimism
INSERT INTO factories (dex_id, chain_id, address)
SELECT id, 10, '0xf1046053aa5682b4f9a81b5481394da16be5ff5a'
FROM dexes WHERE name = 'Velodrome V2'
ON CONFLICT (chain_id, address) DO NOTHING;

-- UniswapV3 on Optimism — same canonical address
INSERT INTO factories (dex_id, chain_id, address)
SELECT id, 10, '0x1f98431c8ad98523631ae4a59f267346ea31f984'
FROM dexes WHERE name = 'UniswapV3'
ON CONFLICT (chain_id, address) DO NOTHING;

-- Curve factory on Optimism (UNVERIFIED)
INSERT INTO factories (dex_id, chain_id, address)
SELECT id, 10, '0x2db0e83599a91b508ac268a6197b8b14f5e72840'
FROM dexes WHERE name = 'Curve'
ON CONFLICT (chain_id, address) DO NOTHING;

-- Beethoven X on Optimism — Balancer fork; uses Balancer vault address
INSERT INTO factories (dex_id, chain_id, address)
SELECT id, 10, '0xba12222222228d8ba445958a75a0704d566bf2c8'
FROM dexes WHERE name = 'Beethoven X'
ON CONFLICT (chain_id, address) DO NOTHING;

-- SushiSwap on Optimism
INSERT INTO factories (dex_id, chain_id, address)
SELECT id, 10, '0xc35dadb65012ec5796536bd9864ed8773abc74c4'
FROM dexes WHERE name = 'SushiSwap'
ON CONFLICT (chain_id, address) DO NOTHING;

-- ── Base chain_id=8453 (6 rows)

-- Aerodrome factory on Base (Solidly fork)
INSERT INTO factories (dex_id, chain_id, address)
SELECT id, 8453, '0x420dd381b31aef6683db6b902084cb0ffece40da'
FROM dexes WHERE name = 'Aerodrome'
ON CONFLICT (chain_id, address) DO NOTHING;

-- UniswapV3 on Base — dedicated Base factory (differs from canonical)
INSERT INTO factories (dex_id, chain_id, address)
SELECT id, 8453, '0x33128a8fc17869897dce68ed026d694621f6fdfd'
FROM dexes WHERE name = 'UniswapV3'
ON CONFLICT (chain_id, address) DO NOTHING;

-- SushiSwap on Base (UNVERIFIED)
INSERT INTO factories (dex_id, chain_id, address)
SELECT id, 8453, '0x71524b4f93c58fcbf659783284e38825f0622859'
FROM dexes WHERE name = 'SushiSwap'
ON CONFLICT (chain_id, address) DO NOTHING;

-- BaseSwap factory on Base (UNVERIFIED)
INSERT INTO factories (dex_id, chain_id, address)
SELECT id, 8453, '0xfda619b6d20975be80a10332cd39b9a4b0faa8bb'
FROM dexes WHERE name = 'BaseSwap'
ON CONFLICT (chain_id, address) DO NOTHING;

-- PancakeSwap V3 on Base — same factory as Ethereum/BSC
INSERT INTO factories (dex_id, chain_id, address)
SELECT id, 8453, '0x0bfbcf9fa4f9c56b0f40a671ad40e0805a091865'
FROM dexes WHERE name = 'PancakeSwap V3'
ON CONFLICT (chain_id, address) DO NOTHING;

-- Curve on Base (UNVERIFIED)
INSERT INTO factories (dex_id, chain_id, address)
SELECT id, 8453, '0x4f8846ae9380b90d2e71d5e3d042dff3e7ebb40d'
FROM dexes WHERE name = 'Curve'
ON CONFLICT (chain_id, address) DO NOTHING;

COMMIT;

-- ─────────────────────────────────────────────────────────────────────────────
-- DOWN (reversible — run manually if rollback needed)
-- ─────────────────────────────────────────────────────────────────────────────
-- BEGIN;
--
-- -- Remove factory rows for the 5 new chains
-- DELETE FROM factories WHERE chain_id IN (10, 56, 137, 8453, 42161);
--
-- -- Remove factory rows added for Ethereum in this migration
-- DELETE FROM factories
--   WHERE chain_id = 1
--     AND address IN (
--       '0xb9fc157394af804a3578134a6585c0dc9cc990d4',
--       '0xba12222222228d8ba445958a75a0704d566bf2c8',
--       '0x0bfbcf9fa4f9c56b0f40a671ad40e0805a091865'
--     );
--
-- -- Remove new DEXes (not the original 3)
-- DELETE FROM dexes WHERE name IN (
--   'Curve','Balancer','PancakeSwap V3','PancakeSwap V2','THENA',
--   'BiSwap','ApeSwap','Quickswap V3','Quickswap V2','Camelot V3',
--   'TraderJoe','Velodrome V2','Beethoven X','Aerodrome','BaseSwap'
-- );
--
-- -- Revert CHECK constraint to original set
-- ALTER TABLE dexes DROP CONSTRAINT IF EXISTS dexes_protocol_type_check;
-- ALTER TABLE dexes ADD CONSTRAINT dexes_protocol_type_check
--   CHECK (protocol_type IN ('UNISWAP_V2','UNISWAP_V3','CURVE','BALANCER'));
--
-- -- Remove 5 new chains
-- DELETE FROM chains WHERE chain_id IN (10, 56, 137, 8453, 42161);
--
-- COMMIT;
