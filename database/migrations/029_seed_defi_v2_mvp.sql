-- ArbitrageX v2 — Migration 029: seed DeFi v2 MVP universe (Ethereum mainnet).
--
-- Doctrine: pool_sync_worker / scanner enrichment / amm_math need a populated
-- registry to operate. Pool addresses below are PUBLIC mainnet contracts,
-- verifiable on Etherscan — they are not secrets and do not violate RULE 00.
-- Operator can extend or disable individual rows via SQL. Future sub-project
-- adds an admin endpoint for UI-driven extension.
--
-- Idempotent via ON CONFLICT DO NOTHING on every INSERT.

BEGIN;

-- 1) Chain
INSERT INTO chains (chain_id, name, native_currency, explorer_url) VALUES
  (1, 'ethereum', 'ETH', 'https://etherscan.io')
ON CONFLICT (chain_id) DO NOTHING;

-- 2) DEXes
INSERT INTO dexes (id, name, protocol_type) VALUES
  (gen_random_uuid(), 'UniswapV2', 'UNISWAP_V2'),
  (gen_random_uuid(), 'SushiSwap', 'UNISWAP_V2')
ON CONFLICT (name) DO NOTHING;

-- 3) Factories (resolved via dex name lookup; safe under ON CONFLICT)
INSERT INTO factories (dex_id, chain_id, address)
SELECT id, 1, '0x5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f' FROM dexes WHERE name='UniswapV2'
ON CONFLICT (chain_id, address) DO NOTHING;
INSERT INTO factories (dex_id, chain_id, address)
SELECT id, 1, '0xC0AEe478e3658e2610c5F7A4A2E1777cE9e4f2Ac' FROM dexes WHERE name='SushiSwap'
ON CONFLICT (chain_id, address) DO NOTHING;

-- 4) Tokens (5 blue-chip Ethereum). decimals are mainnet ground truth.
-- Idempotent insert that works whether or not resolved_via column exists yet
-- (034 adds it with NOT NULL, 072 backfills). We include it if present.
DO $$
BEGIN
  IF EXISTS (SELECT 1 FROM information_schema.columns
             WHERE table_name = 'tokens' AND column_name = 'resolved_via') THEN
    INSERT INTO tokens (chain_id, address, symbol, decimals, is_stablecoin, resolved_via) VALUES
      (1, '0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2', 'WETH', 18, FALSE, 'onchain_full'),
      (1, '0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48', 'USDC',  6, TRUE,  'onchain_full'),
      (1, '0xdac17f958d2ee523a2206206994597c13d831ec7', 'USDT',  6, TRUE,  'onchain_full'),
      (1, '0x6b175474e89094c44da98b954eedeac495271d0f', 'DAI',  18, TRUE,  'onchain_full'),
      (1, '0x2260fac5e5542a773aa44fbcfedf7c193bc2c599', 'WBTC',  8, FALSE, 'onchain_full')
    ON CONFLICT (chain_id, address) DO NOTHING;
  ELSE
    INSERT INTO tokens (chain_id, address, symbol, decimals, is_stablecoin) VALUES
      (1, '0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2', 'WETH', 18, FALSE),
      (1, '0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48', 'USDC',  6, TRUE),
      (1, '0xdac17f958d2ee523a2206206994597c13d831ec7', 'USDT',  6, TRUE),
      (1, '0x6b175474e89094c44da98b954eedeac495271d0f', 'DAI',  18, TRUE),
      (1, '0x2260fac5e5542a773aa44fbcfedf7c193bc2c599', 'WBTC',  8, FALSE)
    ON CONFLICT (chain_id, address) DO NOTHING;
  END IF;
END $$;

-- 5) Pools — UniswapV2
INSERT INTO pools (chain_id, factory_id, address, token0_id, token1_id, fee_tier)
SELECT 1, f.id, '0xb4e16d0168e52d35cacd2c6185b44281ec28c9dc',
       (SELECT id FROM tokens WHERE chain_id=1 AND symbol='USDC'),
       (SELECT id FROM tokens WHERE chain_id=1 AND symbol='WETH'),
       30
FROM factories f JOIN dexes d ON d.id=f.dex_id WHERE d.name='UniswapV2' AND f.chain_id=1
ON CONFLICT (chain_id, address) DO NOTHING;
INSERT INTO pools (chain_id, factory_id, address, token0_id, token1_id, fee_tier)
SELECT 1, f.id, '0x0d4a11d5eeaac28ec3f61d100daf4d40471f1852',
       (SELECT id FROM tokens WHERE chain_id=1 AND symbol='WETH'),
       (SELECT id FROM tokens WHERE chain_id=1 AND symbol='USDT'),
       30
FROM factories f JOIN dexes d ON d.id=f.dex_id WHERE d.name='UniswapV2' AND f.chain_id=1
ON CONFLICT (chain_id, address) DO NOTHING;
INSERT INTO pools (chain_id, factory_id, address, token0_id, token1_id, fee_tier)
SELECT 1, f.id, '0xbb2b8038a1640196fbe3e38816f3e67cba72d940',
       (SELECT id FROM tokens WHERE chain_id=1 AND symbol='WBTC'),
       (SELECT id FROM tokens WHERE chain_id=1 AND symbol='WETH'),
       30
FROM factories f JOIN dexes d ON d.id=f.dex_id WHERE d.name='UniswapV2' AND f.chain_id=1
ON CONFLICT (chain_id, address) DO NOTHING;
INSERT INTO pools (chain_id, factory_id, address, token0_id, token1_id, fee_tier)
SELECT 1, f.id, '0x3041cbd36888becc7bbcbc0045e3b1f144466f5f',
       (SELECT id FROM tokens WHERE chain_id=1 AND symbol='USDC'),
       (SELECT id FROM tokens WHERE chain_id=1 AND symbol='USDT'),
       30
FROM factories f JOIN dexes d ON d.id=f.dex_id WHERE d.name='UniswapV2' AND f.chain_id=1
ON CONFLICT (chain_id, address) DO NOTHING;
INSERT INTO pools (chain_id, factory_id, address, token0_id, token1_id, fee_tier)
SELECT 1, f.id, '0xae461ca67b15dc8dc81ce7615e0320da1a9ab8d5',
       (SELECT id FROM tokens WHERE chain_id=1 AND symbol='DAI' AND address='0x6b175474e89094c44da98b954eedeac495271d0f'),
       (SELECT id FROM tokens WHERE chain_id=1 AND symbol='USDC'),
       30
FROM factories f JOIN dexes d ON d.id=f.dex_id WHERE d.name='UniswapV2' AND f.chain_id=1
ON CONFLICT (chain_id, address) DO NOTHING;

-- 5b) Pools — SushiSwap
INSERT INTO pools (chain_id, factory_id, address, token0_id, token1_id, fee_tier)
SELECT 1, f.id, '0x397ff1542f962076d0bfe58ea045ffa2d347aca0',
       (SELECT id FROM tokens WHERE chain_id=1 AND symbol='USDC'),
       (SELECT id FROM tokens WHERE chain_id=1 AND symbol='WETH'),
       30
FROM factories f JOIN dexes d ON d.id=f.dex_id WHERE d.name='SushiSwap' AND f.chain_id=1
ON CONFLICT (chain_id, address) DO NOTHING;
INSERT INTO pools (chain_id, factory_id, address, token0_id, token1_id, fee_tier)
SELECT 1, f.id, '0x06da0fd433c1a5d7a4faa01111c044910a184553',
       (SELECT id FROM tokens WHERE chain_id=1 AND symbol='WETH'),
       (SELECT id FROM tokens WHERE chain_id=1 AND symbol='USDT'),
       30
FROM factories f JOIN dexes d ON d.id=f.dex_id WHERE d.name='SushiSwap' AND f.chain_id=1
ON CONFLICT (chain_id, address) DO NOTHING;
INSERT INTO pools (chain_id, factory_id, address, token0_id, token1_id, fee_tier)
SELECT 1, f.id, '0xceff51756c56ceffca006cd410b03ffc46dd3a58',
       (SELECT id FROM tokens WHERE chain_id=1 AND symbol='WBTC'),
       (SELECT id FROM tokens WHERE chain_id=1 AND symbol='WETH'),
       30
FROM factories f JOIN dexes d ON d.id=f.dex_id WHERE d.name='SushiSwap' AND f.chain_id=1
ON CONFLICT (chain_id, address) DO NOTHING;
INSERT INTO pools (chain_id, factory_id, address, token0_id, token1_id, fee_tier)
SELECT 1, f.id, '0xaaf5110db6e744ff70fb339de037b990a20bdace',
       (SELECT id FROM tokens WHERE chain_id=1 AND symbol='DAI' AND address='0x6b175474e89094c44da98b954eedeac495271d0f'),
       (SELECT id FROM tokens WHERE chain_id=1 AND symbol='USDC'),
       30
FROM factories f JOIN dexes d ON d.id=f.dex_id WHERE d.name='SushiSwap' AND f.chain_id=1
ON CONFLICT (chain_id, address) DO NOTHING;

COMMIT;
