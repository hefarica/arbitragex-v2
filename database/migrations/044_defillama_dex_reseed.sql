-- ============================================================================
-- Migration 044: DefiLlama-driven DEX catalog re-seed
-- Replaces/augments migration 043 with authoritative live volume+TVL data.
-- Source: https://api.llama.fi/overview/dexs/{chain} (fetched 2026-05-08)
-- Filter bar: vol_24h >= $5M (ethereum/bsc/arbitrum) or $1M (optimism/base/polygon),
--             TVL >= $10M, category == "Dexs"
-- Blocklist applied: dydx (deprecated), hashflow (flagged), tessera-v (Solana-only)
-- R8 (fail-honest): chains with fewer than 5 qualifying DEXes seeded as-is.
--   Polygon: 2 qualifying | Arbitrum: 4 qualifying | Optimism: 1 qualifying
--
-- SCHEMA ADDITIONS (this migration extends the schema):
--   1. factories.is_active BOOLEAN  — enables deactivating unverified rows without DELETE
--   2. dexes.protocol_type += UNISWAP_V4  — singleton hooks architecture (V4, PancakeSwap Infinity)
--
-- Idempotent: all INSERTs use ON CONFLICT; all ALTERs use IF NOT EXISTS / IF EXISTS.
-- Reversible: DOWN block at end of file.
-- ============================================================================

BEGIN;

-- ────────────────────────────────────────────────────────────────────────────
-- STEP 0: Schema extensions
-- ────────────────────────────────────────────────────────────────────────────

-- 0a. Add is_active to factories (no is_active column exists yet per migration 021/043)
ALTER TABLE factories
  ADD COLUMN IF NOT EXISTS is_active BOOLEAN NOT NULL DEFAULT TRUE;

-- 0b. Extend protocol_type CHECK to include UNISWAP_V4
--     (043 already covers UNISWAP_V2, UNISWAP_V3, CURVE, BALANCER, SOLIDLY, TRADERJOE_LB)
ALTER TABLE dexes DROP CONSTRAINT IF EXISTS dexes_protocol_type_check;
ALTER TABLE dexes ADD CONSTRAINT dexes_protocol_type_check
  CHECK (protocol_type IN (
    'UNISWAP_V2',
    'UNISWAP_V3',
    'UNISWAP_V4',
    'CURVE',
    'BALANCER',
    'SOLIDLY',
    'TRADERJOE_LB'
  ));

-- ────────────────────────────────────────────────────────────────────────────
-- STEP 1: Deactivate the 8 UNVERIFIED factory rows from migration 043
--         These addresses were catalog-level placeholders, not on-chain verified.
--         They are NOT deleted (preserves referential integrity and audit trail).
-- ────────────────────────────────────────────────────────────────────────────
UPDATE factories SET is_active = FALSE
WHERE address IN (
    '0x722272d36ef0da72ff51c5a65db7b870e2e8d4ee',  -- Curve on Polygon (placeholder)
    '0x1a3c9b1d2f0529d97f2afc5136cc23e58f1fd35b',  -- Camelot V3 on Arbitrum (circulating docs)
    '0x8e42f2f4101563bf679975178e880fd87d3efd4e',  -- TraderJoe LB on Arbitrum (v2.1, unconfirmed)
    '0xb17b674d9c5cb2e441f8e196a2f048a81355d031',  -- Curve on Arbitrum (unconfirmed)
    '0x2db0e83599a91b508ac268a6197b8b14f5e72840',  -- Curve on Optimism (unconfirmed)
    '0x71524b4f93c58fcbf659783284e38825f0622859',  -- SushiSwap on Base (unconfirmed)
    '0xfda619b6d20975be80a10332cd39b9a4b0faa8bb',  -- BaseSwap on Base (unconfirmed)
    '0x4f8846ae9380b90d2e71d5e3d042dff3e7ebb40d'   -- Curve on Base (unconfirmed)
);

-- ────────────────────────────────────────────────────────────────────────────
-- STEP 2: Update volume_24h_usd + tvl_usd for existing DEXes from DefiLlama
--         Snapshot date: 2026-05-08.
--         These are aggregate protocol figures (all chains combined) as stored
--         in the dexes table — per-chain breakdown lives in factories + pools.
--         NULL tvl = column-level NULL per R8 (no data available in this snapshot).
-- ────────────────────────────────────────────────────────────────────────────

-- UniswapV3: #1 on Arbitrum ($134M), #2 on ETH ($299M), present on all chains
--   Aggregate 24h vol across chains: $298.67M(ETH) + $134.21M(ARB) + $35.61M(BSC) +
--   $67.27M(Base) + $14.95M(Polygon) + $4.16M(OP) = ~$554.9M
UPDATE dexes SET
  volume_24h_usd = 554870000.00,
  tvl_usd        = 1730400000.00  -- ETH $1035M + ARB $218M + BSC $96M + Base $327M + Polygon $34M
WHERE name = 'UniswapV3';

-- Curve: #4 on ETH ($130M), significant multichain presence
UPDATE dexes SET
  volume_24h_usd = 129960000.00,
  tvl_usd        = 1591900000.00  -- Ethereum-dominant
WHERE name = 'Curve';

-- Balancer: $27M on ETH, present on ARB/OP/Polygon
UPDATE dexes SET
  volume_24h_usd = 27410000.00,
  tvl_usd        = NULL  -- Balancer V2 TVL not broken out cleanly from V3 in this snapshot
WHERE name = 'Balancer';

-- PancakeSwap V3: #1 on BSC ($342M), #2 on Base ($117M), ETH ($37M), ARB ($8M excluded TVL)
UPDATE dexes SET
  volume_24h_usd = 503680000.00,  -- BSC + Base + ETH
  tvl_usd        = 294300000.00   -- BSC $259M + Base $27M + ETH TVL N/A
WHERE name = 'PancakeSwap V3';

-- PancakeSwap V2: #3 on BSC ($64M), TVL $1.75B (BSC-dominant)
UPDATE dexes SET
  volume_24h_usd = 63650000.00,
  tvl_usd        = 1748900000.00
WHERE name = 'PancakeSwap V2';

-- Camelot V3: #4 on Arbitrum ($5M), niche but qualifies
UPDATE dexes SET
  volume_24h_usd = 5140000.00,
  tvl_usd        = 15280000.00
WHERE name = 'Camelot V3';

-- Velodrome V2 (Optimism Solidly fork): Velodrome V3 superseded it on volume rankings
--   Velodrome V2 is the older contract; V3 is the active one per DefiLlama
--   V2 is not in top-5 for any chain but keeping existing row active
UPDATE dexes SET
  volume_24h_usd = NULL,  -- not in top-5 on any qualifying chain
  tvl_usd        = NULL
WHERE name = 'Velodrome V2';

-- Aerodrome (V1/SOLIDLY on Base): $12.35M vol, but displaced by Aerodrome Slipstream
--   Aerodrome V1 is separate from Aerodrome Slipstream (CL version)
--   Keeping V1 row active as it still qualifies above $1M Base threshold
UPDATE dexes SET
  volume_24h_usd = 12350000.00,
  tvl_usd        = NULL  -- TVL fetch failed (encoding error in DefiLlama response)
WHERE name = 'Aerodrome';

-- ────────────────────────────────────────────────────────────────────────────
-- STEP 3: Insert new DEXes from DefiLlama not yet in the catalog
--         Four new entries surfaced by the 2026-05-08 DefiLlama snapshot:
--           1. Uniswap V4    — singleton PoolManager, hooks architecture
--           2. Fluid DEX     — InstaDapp smart liquidity collateral DEX
--           3. Aerodrome Slipstream — Aerodrome's concentrated liquidity fork on Base
--           4. PancakeSwap Infinity — PancakeSwap's V4/hooks-based DEX
-- ────────────────────────────────────────────────────────────────────────────

-- Uniswap V4: #1 ETH ($635M), BSC ($47M), Polygon ($28M), Base ($87M), Arbitrum ($22M)
--   Protocol uses a singleton PoolManager per chain — recorded in factories as the manager address.
--   Not is_active=false; factory addresses are verified from Uniswap Labs official deployment (Jan 2025).
INSERT INTO dexes (name, protocol_type, is_active, volume_24h_usd, tvl_usd)
VALUES ('Uniswap V4', 'UNISWAP_V4', TRUE,
        865490000.00,   -- ETH $635M + BSC $47M + Polygon $28M + Base $87M + ARB $22M + small
        699100000.00)   -- ETH $560M + Base $48M + ARB $45M + BSC $25M + Polygon $23M
ON CONFLICT (name) DO UPDATE
  SET volume_24h_usd = EXCLUDED.volume_24h_usd,
      tvl_usd        = EXCLUDED.tvl_usd;

-- Fluid DEX: #3 ETH ($277M), #3 ARB ($7M)
--   Architecture: vault-based liquidity aggregation (no traditional factory).
--   is_active=false: factory address cannot be mapped to a single canonical contract.
--   The Fluid Liquidity contract (0x52aa...) is the protocol entry point but is NOT
--   equivalent to a UniswapV2-style factory. Operator must map before enabling pool_sync.
INSERT INTO dexes (name, protocol_type, is_active, volume_24h_usd, tvl_usd)
VALUES ('Fluid DEX', 'UNISWAP_V3', FALSE,
        284130000.00,   -- ETH $277M + ARB $7M
        944800000.00)   -- Total protocol TVL (no per-chain breakdown available in DefiLlama API)
ON CONFLICT (name) DO UPDATE
  SET volume_24h_usd = EXCLUDED.volume_24h_usd,
      tvl_usd        = EXCLUDED.tvl_usd,
      is_active      = EXCLUDED.is_active;

-- Aerodrome Slipstream: #1 on Base ($457M), V3-style concentrated liquidity
--   Factory: CLFactory at 0x5e7bb104d84c7cb9b682aac2f3d509f5f406809a (Base only)
--   Distinct from 'Aerodrome' (SOLIDLY V1 fork). This is the CL upgrade.
INSERT INTO dexes (name, protocol_type, is_active, volume_24h_usd, tvl_usd)
VALUES ('Aerodrome Slipstream', 'UNISWAP_V3', TRUE,
        457530000.00,
        214100000.00)
ON CONFLICT (name) DO UPDATE
  SET volume_24h_usd = EXCLUDED.volume_24h_usd,
      tvl_usd        = EXCLUDED.tvl_usd;

-- PancakeSwap Infinity: #2 on BSC ($153M), hooks-based V4-style DEX
--   CLPoolManager on BSC: address UNVERIFIED — the protocol launched late 2024.
--   is_active=false until factory address is confirmed on-chain by operator.
--   TODO: map CLPoolManager address (PancakeSwap Infinity uses poolManager, not factory).
INSERT INTO dexes (name, protocol_type, is_active, volume_24h_usd, tvl_usd)
VALUES ('PancakeSwap Infinity', 'UNISWAP_V4', FALSE,
        153330000.00,
        73900000.00)
ON CONFLICT (name) DO UPDATE
  SET volume_24h_usd = EXCLUDED.volume_24h_usd,
      tvl_usd        = EXCLUDED.tvl_usd,
      is_active      = EXCLUDED.is_active;

-- ────────────────────────────────────────────────────────────────────────────
-- STEP 4: Insert factory rows for verified DEXes
--
-- Uniswap V4 PoolManager addresses (official Uniswap Labs deployment, January 2025).
-- Source: https://docs.uniswap.org/contracts/v4/deployments
-- For V4, the "factory" concept maps to the singleton PoolManager contract.
-- Each chain has exactly one PoolManager; pool creation goes through it.
--
-- Aerodrome Slipstream CLFactory (Base only).
-- Source: Aerodrome official docs / Base deployment.
--
-- Note: Fluid DEX and PancakeSwap Infinity have is_active=false at DEX level,
-- so factory rows for them are NOT inserted here — no point mapping a factory
-- to a DEX that the pool_sync_worker cannot use.
-- ────────────────────────────────────────────────────────────────────────────

-- ── Uniswap V4 PoolManager — Ethereum (chain_id=1)
INSERT INTO factories (dex_id, chain_id, address, is_active)
SELECT id, 1, '0x000000000004444c5dc75cb358380d2e3de08a90', TRUE
FROM dexes WHERE name = 'Uniswap V4'
ON CONFLICT (chain_id, address) DO UPDATE SET is_active = TRUE;

-- ── Uniswap V4 PoolManager — BSC (chain_id=56)
INSERT INTO factories (dex_id, chain_id, address, is_active)
SELECT id, 56, '0x28e2ea090877bf75740558f6bfb36a5ffee9e9b5', TRUE
FROM dexes WHERE name = 'Uniswap V4'
ON CONFLICT (chain_id, address) DO UPDATE SET is_active = TRUE;

-- ── Uniswap V4 PoolManager — Polygon (chain_id=137)
INSERT INTO factories (dex_id, chain_id, address, is_active)
SELECT id, 137, '0x67366782805870060151383f4bbff9ddb0279674', TRUE
FROM dexes WHERE name = 'Uniswap V4'
ON CONFLICT (chain_id, address) DO UPDATE SET is_active = TRUE;

-- ── Uniswap V4 PoolManager — Base (chain_id=8453)
INSERT INTO factories (dex_id, chain_id, address, is_active)
SELECT id, 8453, '0x498581ff718922c3f8e6a244956af099b2652b4b', TRUE
FROM dexes WHERE name = 'Uniswap V4'
ON CONFLICT (chain_id, address) DO UPDATE SET is_active = TRUE;

-- ── Uniswap V4 PoolManager — Arbitrum (chain_id=42161)
INSERT INTO factories (dex_id, chain_id, address, is_active)
SELECT id, 42161, '0x360e68faccca8ca495c1b759fd9eee466db9fb32', TRUE
FROM dexes WHERE name = 'Uniswap V4'
ON CONFLICT (chain_id, address) DO UPDATE SET is_active = TRUE;

-- ── Uniswap V4 PoolManager — Optimism (chain_id=10)
--    Optimism has only Uniswap V3 qualifying (V4 does not meet the $1M vol threshold on OP).
--    Inserting factory row anyway as operational preparation; DEX row is active.
INSERT INTO factories (dex_id, chain_id, address, is_active)
SELECT id, 10, '0x9a489505a00ce272eaa5e07dba6491314cae3796', FALSE
FROM dexes WHERE name = 'Uniswap V4'
ON CONFLICT (chain_id, address) DO UPDATE SET is_active = FALSE;

-- ── Aerodrome Slipstream CLFactory — Base (chain_id=8453)
--    Source: Aerodrome official deployment registry for Base CL pools.
INSERT INTO factories (dex_id, chain_id, address, is_active)
SELECT id, 8453, '0x5e7bb104d84c7cb9b682aac2f3d509f5f406809a', TRUE
FROM dexes WHERE name = 'Aerodrome Slipstream'
ON CONFLICT (chain_id, address) DO UPDATE SET is_active = TRUE;

-- ── Re-activate the Camelot V3 factory row now that TVL is confirmed ($15.28M)
--    The address 0x1a3c9b1d2f0529d97f2afc5136cc23e58f1fd35b was flagged UNVERIFIED in 043.
--    DefiLlama confirms Camelot V3 on Arbitrum: vol $5.14M, TVL $15.28M. Address circulates
--    in Camelot official docs; treat as PROVISIONAL until on-chain verified.
--    Keeping is_active=FALSE — operator must verify bytecode before enabling pool_sync.
--    (No UPDATE needed: already set to FALSE in STEP 1 and remains so.)

-- ────────────────────────────────────────────────────────────────────────────
-- STEP 5: Sanity check assertions
-- ────────────────────────────────────────────────────────────────────────────
DO $$
DECLARE
  v_chain_count     INT;
  v_dex_count       INT;
  v_factory_active  INT;
  v_factory_total   INT;
  v_v4_factories    INT;
BEGIN
  SELECT COUNT(*) INTO v_chain_count   FROM chains WHERE is_active = TRUE;
  SELECT COUNT(*) INTO v_dex_count     FROM dexes  WHERE is_active = TRUE;
  SELECT COUNT(*) INTO v_factory_active FROM factories WHERE is_active = TRUE;
  SELECT COUNT(*) INTO v_factory_total  FROM factories;
  SELECT COUNT(*) INTO v_v4_factories
    FROM factories f
    JOIN dexes d ON d.id = f.dex_id
    WHERE d.name = 'Uniswap V4' AND f.is_active = TRUE;

  RAISE NOTICE 'Post-044: active_chains=%, active_dexes=%, active_factories=% (total=%), v4_active_factories=%',
    v_chain_count, v_dex_count, v_factory_active, v_factory_total, v_v4_factories;

  -- Hard assertions
  IF v_chain_count < 6 THEN
    RAISE EXCEPTION 'ASSERTION FAILED: expected >= 6 active chains, got %', v_chain_count;
  END IF;
  IF v_v4_factories < 5 THEN
    RAISE EXCEPTION 'ASSERTION FAILED: expected >= 5 active Uniswap V4 factory rows, got %', v_v4_factories;
  END IF;
  IF v_dex_count < 10 THEN
    RAISE EXCEPTION 'ASSERTION FAILED: expected >= 10 active dexes, got %', v_dex_count;
  END IF;
END $$;

COMMIT;

-- ════════════════════════════════════════════════════════════════════════════
-- DOWN (reversible — run manually if rollback needed)
-- ════════════════════════════════════════════════════════════════════════════
--
-- BEGIN;
--
-- -- Remove new factory rows added in this migration
-- DELETE FROM factories
--   WHERE address IN (
--     '0x000000000004444c5dc75cb358380d2e3de08a90',  -- V4 ETH
--     '0x28e2ea090877bf75740558f6bfb36a5ffee9e9b5',  -- V4 BSC
--     '0x67366782805870060151383f4bbff9ddb0279674',  -- V4 Polygon
--     '0x498581ff718922c3f8e6a244956af099b2652b4b',  -- V4 Base
--     '0x360e68faccca8ca495c1b759fd9eee466db9fb32',  -- V4 Arbitrum
--     '0x9a489505a00ce272eaa5e07dba6491314cae3796',  -- V4 Optimism
--     '0x5e7bb104d84c7cb9b682aac2f3d509f5f406809a'   -- Aerodrome Slipstream Base
--   );
--
-- -- Re-activate the 8 previously unverified factory rows
-- UPDATE factories SET is_active = TRUE
-- WHERE address IN (
--     '0x722272d36ef0da72ff51c5a65db7b870e2e8d4ee',
--     '0x1a3c9b1d2f0529d97f2afc5136cc23e58f1fd35b',
--     '0x8e42f2f4101563bf679975178e880fd87d3efd4e',
--     '0xb17b674d9c5cb2e441f8e196a2f048a81355d031',
--     '0x2db0e83599a91b508ac268a6197b8b14f5e72840',
--     '0x71524b4f93c58fcbf659783284e38825f0622859',
--     '0xfda619b6d20975be80a10332cd39b9a4b0faa8bb',
--     '0x4f8846ae9380b90d2e71d5e3d042dff3e7ebb40d'
-- );
--
-- -- Remove new DEX rows
-- DELETE FROM dexes WHERE name IN (
--   'Uniswap V4', 'Fluid DEX', 'Aerodrome Slipstream', 'PancakeSwap Infinity'
-- );
--
-- -- Revert volume_24h_usd + tvl_usd to NULL for existing DEXes
-- UPDATE dexes SET volume_24h_usd = NULL, tvl_usd = NULL
-- WHERE name IN (
--   'UniswapV3', 'Curve', 'Balancer', 'PancakeSwap V3', 'PancakeSwap V2',
--   'Camelot V3', 'Velodrome V2', 'Aerodrome'
-- );
--
-- -- Remove is_active column from factories (destructive — only if clean rollback to 043)
-- ALTER TABLE factories DROP COLUMN IF EXISTS is_active;
--
-- -- Restore protocol_type CHECK to 043 set
-- ALTER TABLE dexes DROP CONSTRAINT IF EXISTS dexes_protocol_type_check;
-- ALTER TABLE dexes ADD CONSTRAINT dexes_protocol_type_check
--   CHECK (protocol_type IN (
--     'UNISWAP_V2', 'UNISWAP_V3', 'CURVE', 'BALANCER', 'SOLIDLY', 'TRADERJOE_LB'
--   ));
--
-- COMMIT;
