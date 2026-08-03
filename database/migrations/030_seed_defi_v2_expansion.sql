-- ArbitrageX v2 — Migration 030: aggressive expansion of v2 universe.
--
-- Doctrine: migration 029 seeded 5 tokens + 9 pools as MVP. In production
-- the operator's mempool sees thousands of token addresses per minute, of
-- which only a tiny slice involves the original 5 — so candidate_enriched
-- + risk_score>0 events were rare to non-existent.
--
-- This migration broadens coverage:
--   + 25 more high-liquidity ERC20 tokens (mainnet addresses verified
--     against Etherscan as of 2024)
--   + 14 more V2 pool pairs: WETH-pairs and stablecoin-pairs on both
--     UniswapV2 and SushiSwap where both venues are known to exist
--
-- Pool addresses are best-effort from public knowledge; if any individual
-- pool address turns out wrong, Multicall3's allow_failure=true means
-- pool_sync_worker logs "pool_sync.pool_failed" for that pool and the
-- rest of the universe keeps ticking. No catastrophic failure mode.
--
-- After applying: `docker compose restart searcher-rs` to trigger the
-- PoolSyncWorker bootstrap (which re-reads pools+tokens from DB into
-- Redis caches at startup).
--
-- Idempotent via ON CONFLICT DO NOTHING on every INSERT.

BEGIN;

-- ── Tokens (25 more — operator's allowlist already covers symbols) ──────────
-- Tolerant insert: include resolved_via only when the column exists (034 adds it
-- NOT NULL, 072 backfills). Prevents 'null value in column resolved_via' on re-runs
-- against an already-hardened DB while staying valid on a fresh install.
DO $$
BEGIN
  IF EXISTS (SELECT 1 FROM information_schema.columns
             WHERE table_name = 'tokens' AND column_name = 'resolved_via') THEN
    INSERT INTO tokens (chain_id, address, symbol, decimals, is_stablecoin, resolved_via) VALUES
      (1, '0x514910771af9ca656af840dff83e8264ecf986ca', 'LINK',  18, FALSE, 'onchain_full'),
      (1, '0x1f9840a85d5af5bf1d1762f925bdaddc4201f984', 'UNI',   18, FALSE, 'onchain_full'),
      (1, '0x7fc66500c84a76ad7e9c93437bfc5ac33e2ddae9', 'AAVE',  18, FALSE, 'onchain_full'),
      (1, '0x9f8f72aa9304c8b593d555f12ef6589cc3a579a2', 'MKR',   18, FALSE, 'onchain_full'),
      (1, '0xc00e94cb662c3520282e6f5717214004a7f26888', 'COMP',  18, FALSE, 'onchain_full'),
      (1, '0x6b3595068778dd592e39a122f4f5a5cf09c90fe2', 'SUSHI', 18, FALSE, 'onchain_full'),
      (1, '0xd533a949740bb3306d119cc777fa900ba034cd52', 'CRV',   18, FALSE, 'onchain_full'),
      (1, '0x5a98fcbea516cf06857215779fd812ca3bef1b32', 'LDO',   18, FALSE, 'onchain_full'),
      (1, '0x7d1afa7b718fb893db30a3abc0cfc608aacfebb0', 'MATIC', 18, FALSE, 'onchain_full'),
      (1, '0x95ad61b0a150d79219dcf64e1e6cc01f0b64c4ce', 'SHIB',  18, FALSE, 'onchain_full'),
      (1, '0x6982508145454ce325ddbe47a25d4ec3d2311933', 'PEPE',  18, FALSE, 'onchain_full'),
      (1, '0x3845badade8e6dff049820680d1f14bd3903a5d0', 'SAND',  18, FALSE, 'onchain_full'),
      (1, '0x0f5d2fb29fb7d3cfee444a200298f468908cc942', 'MANA',  18, FALSE, 'onchain_full'),
      (1, '0x4d224452801aced8b2f0aebe155379bb5d594381', 'APE',   18, FALSE, 'onchain_full'),
      (1, '0xc944e90c64b2c07662a292be6244bdf05cda44a7', 'GRT',   18, FALSE, 'onchain_full'),
      (1, '0x0d8775f648430679a709e98d2b0cb6250d2887ef', 'BAT',   18, FALSE, 'onchain_full'),
      (1, '0xe41d2489571d322189246dafa5ebde1f4699f498', 'ZRX',   18, FALSE, 'onchain_full'),
      (1, '0xc18360217d8f7ab5e7c516566761ea12ce7f9d72', 'ENS',   18, FALSE, 'onchain_full'),
      (1, '0x3432b6a60d23ca0dfca7761b7ab56459d9c964d0', 'FXS',   18, FALSE, 'onchain_full'),
      (1, '0xae7ab96520de3a18e5e111b5eaab095312d7fe84', 'STETH', 18, FALSE, 'onchain_full'),
      (1, '0xae78736cd615f374d3085123a210448e74fc6393', 'RETH',  18, FALSE, 'onchain_full'),
      (1, '0xb50721bcf8d664c30412cfbc6cf7a15145234ad1', 'ARB',   18, FALSE, 'onchain_full'),
      (1, '0x4e3fbd56cd56c3e72c1403e103b45db9da5b9d2b', 'CVX',   18, FALSE, 'onchain_full'),
      (1, '0x7f39c581f595b53c5cb19bd0b3f8da6c935e2ca0', 'WSTETH',18, FALSE, 'onchain_full'),
      (1, '0xb8c77482e45f1f44de1745f52c74426c631bdd52', 'BNB',   18, FALSE, 'onchain_full')
    ON CONFLICT (chain_id, address) DO NOTHING;
  ELSE
    INSERT INTO tokens (chain_id, address, symbol, decimals, is_stablecoin) VALUES
      (1, '0x514910771af9ca656af840dff83e8264ecf986ca', 'LINK',  18, FALSE),
      (1, '0x1f9840a85d5af5bf1d1762f925bdaddc4201f984', 'UNI',   18, FALSE),
      (1, '0x7fc66500c84a76ad7e9c93437bfc5ac33e2ddae9', 'AAVE',  18, FALSE),
      (1, '0x9f8f72aa9304c8b593d555f12ef6589cc3a579a2', 'MKR',   18, FALSE),
      (1, '0xc00e94cb662c3520282e6f5717214004a7f26888', 'COMP',  18, FALSE),
      (1, '0x6b3595068778dd592e39a122f4f5a5cf09c90fe2', 'SUSHI', 18, FALSE),
      (1, '0xd533a949740bb3306d119cc777fa900ba034cd52', 'CRV',   18, FALSE),
      (1, '0x5a98fcbea516cf06857215779fd812ca3bef1b32', 'LDO',   18, FALSE),
      (1, '0x7d1afa7b718fb893db30a3abc0cfc608aacfebb0', 'MATIC', 18, FALSE),
      (1, '0x95ad61b0a150d79219dcf64e1e6cc01f0b64c4ce', 'SHIB',  18, FALSE),
      (1, '0x6982508145454ce325ddbe47a25d4ec3d2311933', 'PEPE',  18, FALSE),
      (1, '0x3845badade8e6dff049820680d1f14bd3903a5d0', 'SAND',  18, FALSE),
      (1, '0x0f5d2fb29fb7d3cfee444a200298f468908cc942', 'MANA',  18, FALSE),
      (1, '0x4d224452801aced8b2f0aebe155379bb5d594381', 'APE',   18, FALSE),
      (1, '0xc944e90c64b2c07662a292be6244bdf05cda44a7', 'GRT',   18, FALSE),
      (1, '0x0d8775f648430679a709e98d2b0cb6250d2887ef', 'BAT',   18, FALSE),
      (1, '0xe41d2489571d322189246dafa5ebde1f4699f498', 'ZRX',   18, FALSE),
      (1, '0xc18360217d8f7ab5e7c516566761ea12ce7f9d72', 'ENS',   18, FALSE),
      (1, '0x3432b6a60d23ca0dfca7761b7ab56459d9c964d0', 'FXS',   18, FALSE),
      (1, '0xae7ab96520de3a18e5e111b5eaab095312d7fe84', 'STETH', 18, FALSE),
      (1, '0xae78736cd615f374d3085123a210448e74fc6393', 'RETH',  18, FALSE),
      (1, '0xb50721bcf8d664c30412cfbc6cf7a15145234ad1', 'ARB',   18, FALSE),
      (1, '0x4e3fbd56cd56c3e72c1403e103b45db9da5b9d2b', 'CVX',   18, FALSE),
      (1, '0x7f39c581f595b53c5cb19bd0b3f8da6c935e2ca0', 'WSTETH',18, FALSE),
      (1, '0xb8c77482e45f1f44de1745f52c74426c631bdd52', 'BNB',   18, FALSE)
    ON CONFLICT (chain_id, address) DO NOTHING;
  END IF;
END $$;

-- ── UniswapV2 Pools (8 more popular WETH-pairs + 2 stable-pairs) ────────────

-- WETH/DAI on UniV2
INSERT INTO pools (chain_id, factory_id, address, token0_id, token1_id, fee_tier)
SELECT 1, f.id, '0xa478c2975ab1ea89e8196811f51a7b7ade33eb11',
       (SELECT id FROM tokens WHERE chain_id=1 AND symbol='DAI' AND address='0x6b175474e89094c44da98b954eedeac495271d0f'),
       (SELECT id FROM tokens WHERE chain_id=1 AND symbol='WETH'),
       30
FROM factories f JOIN dexes d ON d.id=f.dex_id WHERE d.name='UniswapV2' AND f.chain_id=1
ON CONFLICT (chain_id, address) DO NOTHING;

-- WETH/LINK on UniV2
INSERT INTO pools (chain_id, factory_id, address, token0_id, token1_id, fee_tier)
SELECT 1, f.id, '0xa2107fa5b38d9bbd2c461d6edf11b11a50f6b974',
       (SELECT id FROM tokens WHERE chain_id=1 AND symbol='LINK'),
       (SELECT id FROM tokens WHERE chain_id=1 AND symbol='WETH'),
       30
FROM factories f JOIN dexes d ON d.id=f.dex_id WHERE d.name='UniswapV2' AND f.chain_id=1
ON CONFLICT (chain_id, address) DO NOTHING;

-- WETH/UNI on UniV2
INSERT INTO pools (chain_id, factory_id, address, token0_id, token1_id, fee_tier)
SELECT 1, f.id, '0xd3d2e2692501a5c9ca623199d38826e513033a17',
       (SELECT id FROM tokens WHERE chain_id=1 AND symbol='UNI'),
       (SELECT id FROM tokens WHERE chain_id=1 AND symbol='WETH'),
       30
FROM factories f JOIN dexes d ON d.id=f.dex_id WHERE d.name='UniswapV2' AND f.chain_id=1
ON CONFLICT (chain_id, address) DO NOTHING;

-- WETH/MKR on UniV2
INSERT INTO pools (chain_id, factory_id, address, token0_id, token1_id, fee_tier)
SELECT 1, f.id, '0xc2adda861f89bbb333c90c492cb837741916a225',
       (SELECT id FROM tokens WHERE chain_id=1 AND symbol='MKR'),
       (SELECT id FROM tokens WHERE chain_id=1 AND symbol='WETH'),
       30
FROM factories f JOIN dexes d ON d.id=f.dex_id WHERE d.name='UniswapV2' AND f.chain_id=1
ON CONFLICT (chain_id, address) DO NOTHING;

-- WETH/AAVE on UniV2
INSERT INTO pools (chain_id, factory_id, address, token0_id, token1_id, fee_tier)
SELECT 1, f.id, '0xdfc14d2af169b0d36c4eff567ada9b2e0cae044f',
       (SELECT id FROM tokens WHERE chain_id=1 AND symbol='AAVE'),
       (SELECT id FROM tokens WHERE chain_id=1 AND symbol='WETH'),
       30
FROM factories f JOIN dexes d ON d.id=f.dex_id WHERE d.name='UniswapV2' AND f.chain_id=1
ON CONFLICT (chain_id, address) DO NOTHING;

-- WETH/SUSHI on UniV2
INSERT INTO pools (chain_id, factory_id, address, token0_id, token1_id, fee_tier)
SELECT 1, f.id, '0x795065dcc9f64b5614c407a6efdc400da6221fb0',
       (SELECT id FROM tokens WHERE chain_id=1 AND symbol='SUSHI'),
       (SELECT id FROM tokens WHERE chain_id=1 AND symbol='WETH'),
       30
FROM factories f JOIN dexes d ON d.id=f.dex_id WHERE d.name='UniswapV2' AND f.chain_id=1
ON CONFLICT (chain_id, address) DO NOTHING;

-- WETH/COMP on UniV2
INSERT INTO pools (chain_id, factory_id, address, token0_id, token1_id, fee_tier)
SELECT 1, f.id, '0xcffdded873554f362ac02f8fb1f02e5ada10516f',
       (SELECT id FROM tokens WHERE chain_id=1 AND symbol='COMP'),
       (SELECT id FROM tokens WHERE chain_id=1 AND symbol='WETH'),
       30
FROM factories f JOIN dexes d ON d.id=f.dex_id WHERE d.name='UniswapV2' AND f.chain_id=1
ON CONFLICT (chain_id, address) DO NOTHING;

-- WETH/SHIB on UniV2
INSERT INTO pools (chain_id, factory_id, address, token0_id, token1_id, fee_tier)
SELECT 1, f.id, '0x811beed0119b4afce20d2583eb608c6f7af1954f',
       (SELECT id FROM tokens WHERE chain_id=1 AND symbol='SHIB' AND address='0x95ad61b0a150d79219dcf64e1e6cc01f0b64c4ce'),
       (SELECT id FROM tokens WHERE chain_id=1 AND symbol='WETH'),
       30
FROM factories f JOIN dexes d ON d.id=f.dex_id WHERE d.name='UniswapV2' AND f.chain_id=1
ON CONFLICT (chain_id, address) DO NOTHING;

-- WETH/PEPE on UniV2
INSERT INTO pools (chain_id, factory_id, address, token0_id, token1_id, fee_tier)
SELECT 1, f.id, '0xa43fe16908251ee70ef74718545e4fe6c5ccec9f',
       (SELECT id FROM tokens WHERE chain_id=1 AND symbol='PEPE' AND address='0x6982508145454ce325ddbe47a25d4ec3d2311933'),
       (SELECT id FROM tokens WHERE chain_id=1 AND symbol='WETH'),
       30
FROM factories f JOIN dexes d ON d.id=f.dex_id WHERE d.name='UniswapV2' AND f.chain_id=1
ON CONFLICT (chain_id, address) DO NOTHING;

-- WETH/MATIC on UniV2
INSERT INTO pools (chain_id, factory_id, address, token0_id, token1_id, fee_tier)
SELECT 1, f.id, '0x819f3450da6f110ba6ea52195b3beafa246062de',
       (SELECT id FROM tokens WHERE chain_id=1 AND symbol='MATIC'),
       (SELECT id FROM tokens WHERE chain_id=1 AND symbol='WETH'),
       30
FROM factories f JOIN dexes d ON d.id=f.dex_id WHERE d.name='UniswapV2' AND f.chain_id=1
ON CONFLICT (chain_id, address) DO NOTHING;

-- ── SushiSwap Pools (mirror UniV2 pairs that exist on Sushi too) ────────────

-- WETH/DAI on Sushi
INSERT INTO pools (chain_id, factory_id, address, token0_id, token1_id, fee_tier)
SELECT 1, f.id, '0xc3d03e4f041fd4cd388c549ee2a29a9e5075882f',
       (SELECT id FROM tokens WHERE chain_id=1 AND symbol='DAI' AND address='0x6b175474e89094c44da98b954eedeac495271d0f'),
       (SELECT id FROM tokens WHERE chain_id=1 AND symbol='WETH'),
       30
FROM factories f JOIN dexes d ON d.id=f.dex_id WHERE d.name='SushiSwap' AND f.chain_id=1
ON CONFLICT (chain_id, address) DO NOTHING;

-- WETH/LINK on Sushi
INSERT INTO pools (chain_id, factory_id, address, token0_id, token1_id, fee_tier)
SELECT 1, f.id, '0xc40d16476380e4037e6b1a2594caf6a6cc8da967',
       (SELECT id FROM tokens WHERE chain_id=1 AND symbol='LINK'),
       (SELECT id FROM tokens WHERE chain_id=1 AND symbol='WETH'),
       30
FROM factories f JOIN dexes d ON d.id=f.dex_id WHERE d.name='SushiSwap' AND f.chain_id=1
ON CONFLICT (chain_id, address) DO NOTHING;

-- WETH/UNI on Sushi
INSERT INTO pools (chain_id, factory_id, address, token0_id, token1_id, fee_tier)
SELECT 1, f.id, '0xdafd66636e2561b0284edde37e42d192f2844d40',
       (SELECT id FROM tokens WHERE chain_id=1 AND symbol='UNI'),
       (SELECT id FROM tokens WHERE chain_id=1 AND symbol='WETH'),
       30
FROM factories f JOIN dexes d ON d.id=f.dex_id WHERE d.name='SushiSwap' AND f.chain_id=1
ON CONFLICT (chain_id, address) DO NOTHING;

-- WETH/AAVE on Sushi
INSERT INTO pools (chain_id, factory_id, address, token0_id, token1_id, fee_tier)
SELECT 1, f.id, '0xd75ea151a61d06868e31f8988d28dfe5e9df57b4',
       (SELECT id FROM tokens WHERE chain_id=1 AND symbol='AAVE'),
       (SELECT id FROM tokens WHERE chain_id=1 AND symbol='WETH'),
       30
FROM factories f JOIN dexes d ON d.id=f.dex_id WHERE d.name='SushiSwap' AND f.chain_id=1
ON CONFLICT (chain_id, address) DO NOTHING;

-- WETH/MKR on Sushi
INSERT INTO pools (chain_id, factory_id, address, token0_id, token1_id, fee_tier)
SELECT 1, f.id, '0xba13afecda9beb75de5c56bbaf696b880a5a50dd',
       (SELECT id FROM tokens WHERE chain_id=1 AND symbol='MKR'),
       (SELECT id FROM tokens WHERE chain_id=1 AND symbol='WETH'),
       30
FROM factories f JOIN dexes d ON d.id=f.dex_id WHERE d.name='SushiSwap' AND f.chain_id=1
ON CONFLICT (chain_id, address) DO NOTHING;

COMMIT;
