-- ArbitrageX v2 — Migration 031
-- Sub-proyecto 2: Uniswap V3 enrichment
-- Adds UniswapV3 dex, V3 factory, and 30 on-chain validated V3 pool addresses.
-- All addresses validated via factory.getPool() + eth_getCode at migration creation time
-- (2026-05-04, against Alchemy mainnet RPC). 30/30 returned non-empty bytecode (codelen=44286).
-- Idempotent via ON CONFLICT clauses.

BEGIN;

-- 1. UniswapV3 dex
INSERT INTO dexes (name, protocol_type, is_active)
VALUES ('UniswapV3', 'UNISWAP_V3', TRUE)
ON CONFLICT (name) DO NOTHING;

-- 2. V3 factory (canonical mainnet)
INSERT INTO factories (dex_id, chain_id, address)
SELECT id, 1, LOWER('0x1F98431c8aD98523631AE4a59f267346ea31F984')
FROM dexes WHERE name = 'UniswapV3'
ON CONFLICT (chain_id, address) DO NOTHING;

-- 3. 30 V3 pools — token0_id is the LOWER address by hex value (V3 invariant).
--    pool addresses come from factory.getPool() + eth_getCode validation.
INSERT INTO pools (chain_id, factory_id, address, token0_id, token1_id, fee_tier, is_active)
VALUES
  -- USDC/WETH 0.01% (1)
  (1,
   (SELECT id FROM factories WHERE chain_id=1 AND address=LOWER('0x1F98431c8aD98523631AE4a59f267346ea31F984')),
   LOWER('0xe0554a476a092703abdb3ef35c80e0d76d32939f'),
   (SELECT id FROM tokens WHERE chain_id=1 AND address=LOWER('0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48')),
   (SELECT id FROM tokens WHERE chain_id=1 AND address=LOWER('0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2')),
   100, TRUE),
  -- USDC/WETH 0.05% (2)
  (1,
   (SELECT id FROM factories WHERE chain_id=1 AND address=LOWER('0x1F98431c8aD98523631AE4a59f267346ea31F984')),
   LOWER('0x88e6a0c2ddd26feeb64f039a2c41296fcb3f5640'),
   (SELECT id FROM tokens WHERE chain_id=1 AND address=LOWER('0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48')),
   (SELECT id FROM tokens WHERE chain_id=1 AND address=LOWER('0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2')),
   500, TRUE),
  -- USDC/WETH 0.30% (3)
  (1,
   (SELECT id FROM factories WHERE chain_id=1 AND address=LOWER('0x1F98431c8aD98523631AE4a59f267346ea31F984')),
   LOWER('0x8ad599c3a0ff1de082011efddc58f1908eb6e6d8'),
   (SELECT id FROM tokens WHERE chain_id=1 AND address=LOWER('0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48')),
   (SELECT id FROM tokens WHERE chain_id=1 AND address=LOWER('0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2')),
   3000, TRUE),
  -- WETH/USDT 0.05% (4)
  (1,
   (SELECT id FROM factories WHERE chain_id=1 AND address=LOWER('0x1F98431c8aD98523631AE4a59f267346ea31F984')),
   LOWER('0x11b815efb8f581194ae79006d24e0d814b7697f6'),
   (SELECT id FROM tokens WHERE chain_id=1 AND address=LOWER('0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2')),
   (SELECT id FROM tokens WHERE chain_id=1 AND address=LOWER('0xdac17f958d2ee523a2206206994597c13d831ec7')),
   500, TRUE),
  -- WETH/USDT 0.30% (5)
  (1,
   (SELECT id FROM factories WHERE chain_id=1 AND address=LOWER('0x1F98431c8aD98523631AE4a59f267346ea31F984')),
   LOWER('0x4e68ccd3e89f51c3074ca5072bbac773960dfa36'),
   (SELECT id FROM tokens WHERE chain_id=1 AND address=LOWER('0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2')),
   (SELECT id FROM tokens WHERE chain_id=1 AND address=LOWER('0xdac17f958d2ee523a2206206994597c13d831ec7')),
   3000, TRUE),
  -- DAI/WETH 0.05% (6)
  (1,
   (SELECT id FROM factories WHERE chain_id=1 AND address=LOWER('0x1F98431c8aD98523631AE4a59f267346ea31F984')),
   LOWER('0x60594a405d53811d3bc4766596efd80fd545a270'),
   (SELECT id FROM tokens WHERE chain_id=1 AND address=LOWER('0x6b175474e89094c44da98b954eedeac495271d0f')),
   (SELECT id FROM tokens WHERE chain_id=1 AND address=LOWER('0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2')),
   500, TRUE),
  -- DAI/WETH 0.30% (7)
  (1,
   (SELECT id FROM factories WHERE chain_id=1 AND address=LOWER('0x1F98431c8aD98523631AE4a59f267346ea31F984')),
   LOWER('0xc2e9f25be6257c210d7adf0d4cd6e3e881ba25f8'),
   (SELECT id FROM tokens WHERE chain_id=1 AND address=LOWER('0x6b175474e89094c44da98b954eedeac495271d0f')),
   (SELECT id FROM tokens WHERE chain_id=1 AND address=LOWER('0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2')),
   3000, TRUE),
  -- USDC/USDT 0.01% (8)
  (1,
   (SELECT id FROM factories WHERE chain_id=1 AND address=LOWER('0x1F98431c8aD98523631AE4a59f267346ea31F984')),
   LOWER('0x3416cf6c708da44db2624d63ea0aaef7113527c6'),
   (SELECT id FROM tokens WHERE chain_id=1 AND address=LOWER('0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48')),
   (SELECT id FROM tokens WHERE chain_id=1 AND address=LOWER('0xdac17f958d2ee523a2206206994597c13d831ec7')),
   100, TRUE),
  -- DAI/USDC 0.01% (9)
  (1,
   (SELECT id FROM factories WHERE chain_id=1 AND address=LOWER('0x1F98431c8aD98523631AE4a59f267346ea31F984')),
   LOWER('0x5777d92f208679db4b9778590fa3cab3ac9e2168'),
   (SELECT id FROM tokens WHERE chain_id=1 AND address=LOWER('0x6b175474e89094c44da98b954eedeac495271d0f')),
   (SELECT id FROM tokens WHERE chain_id=1 AND address=LOWER('0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48')),
   100, TRUE),
  -- DAI/USDT 0.01% (10)
  (1,
   (SELECT id FROM factories WHERE chain_id=1 AND address=LOWER('0x1F98431c8aD98523631AE4a59f267346ea31F984')),
   LOWER('0x48da0965ab2d2cbf1c17c09cfb5cbe67ad5b1406'),
   (SELECT id FROM tokens WHERE chain_id=1 AND address=LOWER('0x6b175474e89094c44da98b954eedeac495271d0f')),
   (SELECT id FROM tokens WHERE chain_id=1 AND address=LOWER('0xdac17f958d2ee523a2206206994597c13d831ec7')),
   100, TRUE),
  -- WBTC/WETH 0.05% (11)
  (1,
   (SELECT id FROM factories WHERE chain_id=1 AND address=LOWER('0x1F98431c8aD98523631AE4a59f267346ea31F984')),
   LOWER('0x4585fe77225b41b697c938b018e2ac67ac5a20c0'),
   (SELECT id FROM tokens WHERE chain_id=1 AND address=LOWER('0x2260fac5e5542a773aa44fbcfedf7c193bc2c599')),
   (SELECT id FROM tokens WHERE chain_id=1 AND address=LOWER('0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2')),
   500, TRUE),
  -- WBTC/WETH 0.30% (12)
  (1,
   (SELECT id FROM factories WHERE chain_id=1 AND address=LOWER('0x1F98431c8aD98523631AE4a59f267346ea31F984')),
   LOWER('0xcbcdf9626bc03e24f779434178a73a0b4bad62ed'),
   (SELECT id FROM tokens WHERE chain_id=1 AND address=LOWER('0x2260fac5e5542a773aa44fbcfedf7c193bc2c599')),
   (SELECT id FROM tokens WHERE chain_id=1 AND address=LOWER('0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2')),
   3000, TRUE),
  -- WBTC/USDC 0.30% (13)
  (1,
   (SELECT id FROM factories WHERE chain_id=1 AND address=LOWER('0x1F98431c8aD98523631AE4a59f267346ea31F984')),
   LOWER('0x99ac8ca7087fa4a2a1fb6357269965a2014abc35'),
   (SELECT id FROM tokens WHERE chain_id=1 AND address=LOWER('0x2260fac5e5542a773aa44fbcfedf7c193bc2c599')),
   (SELECT id FROM tokens WHERE chain_id=1 AND address=LOWER('0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48')),
   3000, TRUE),
  -- WBTC/USDT 0.30% (14)
  (1,
   (SELECT id FROM factories WHERE chain_id=1 AND address=LOWER('0x1F98431c8aD98523631AE4a59f267346ea31F984')),
   LOWER('0x9db9e0e53058c89e5b94e29621a205198648425b'),
   (SELECT id FROM tokens WHERE chain_id=1 AND address=LOWER('0x2260fac5e5542a773aa44fbcfedf7c193bc2c599')),
   (SELECT id FROM tokens WHERE chain_id=1 AND address=LOWER('0xdac17f958d2ee523a2206206994597c13d831ec7')),
   3000, TRUE),
  -- LINK/WETH 0.30% (15)
  (1,
   (SELECT id FROM factories WHERE chain_id=1 AND address=LOWER('0x1F98431c8aD98523631AE4a59f267346ea31F984')),
   LOWER('0xa6cc3c2531fdaa6ae1a3ca84c2855806728693e8'),
   (SELECT id FROM tokens WHERE chain_id=1 AND address=LOWER('0x514910771af9ca656af840dff83e8264ecf986ca')),
   (SELECT id FROM tokens WHERE chain_id=1 AND address=LOWER('0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2')),
   3000, TRUE),
  -- UNI/WETH 0.30% (16)
  (1,
   (SELECT id FROM factories WHERE chain_id=1 AND address=LOWER('0x1F98431c8aD98523631AE4a59f267346ea31F984')),
   LOWER('0x1d42064fc4beb5f8aaf85f4617ae8b3b5b8bd801'),
   (SELECT id FROM tokens WHERE chain_id=1 AND address=LOWER('0x1f9840a85d5af5bf1d1762f925bdaddc4201f984')),
   (SELECT id FROM tokens WHERE chain_id=1 AND address=LOWER('0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2')),
   3000, TRUE),
  -- AAVE/WETH 0.30% (17)
  (1,
   (SELECT id FROM factories WHERE chain_id=1 AND address=LOWER('0x1F98431c8aD98523631AE4a59f267346ea31F984')),
   LOWER('0x5ab53ee1d50eef2c1dd3d5402789cd27bb52c1bb'),
   (SELECT id FROM tokens WHERE chain_id=1 AND address=LOWER('0x7fc66500c84a76ad7e9c93437bfc5ac33e2ddae9')),
   (SELECT id FROM tokens WHERE chain_id=1 AND address=LOWER('0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2')),
   3000, TRUE),
  -- MKR/WETH 0.30% (18)
  (1,
   (SELECT id FROM factories WHERE chain_id=1 AND address=LOWER('0x1F98431c8aD98523631AE4a59f267346ea31F984')),
   LOWER('0xe8c6c9227491c0a8156a0106a0204d881bb7e531'),
   (SELECT id FROM tokens WHERE chain_id=1 AND address=LOWER('0x9f8f72aa9304c8b593d555f12ef6589cc3a579a2')),
   (SELECT id FROM tokens WHERE chain_id=1 AND address=LOWER('0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2')),
   3000, TRUE),
  -- COMP/WETH 0.30% (19)
  (1,
   (SELECT id FROM factories WHERE chain_id=1 AND address=LOWER('0x1F98431c8aD98523631AE4a59f267346ea31F984')),
   LOWER('0xea4ba4ce14fdd287f380b55419b1c5b6c3f22ab6'),
   (SELECT id FROM tokens WHERE chain_id=1 AND address=LOWER('0xc00e94cb662c3520282e6f5717214004a7f26888')),
   (SELECT id FROM tokens WHERE chain_id=1 AND address=LOWER('0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2')),
   3000, TRUE),
  -- WETH/CRV 0.30% (20)
  (1,
   (SELECT id FROM factories WHERE chain_id=1 AND address=LOWER('0x1F98431c8aD98523631AE4a59f267346ea31F984')),
   LOWER('0x919fa96e88d67499339577fa202345436bcdaf79'),
   (SELECT id FROM tokens WHERE chain_id=1 AND address=LOWER('0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2')),
   (SELECT id FROM tokens WHERE chain_id=1 AND address=LOWER('0xd533a949740bb3306d119cc777fa900ba034cd52')),
   3000, TRUE),
  -- LDO/WETH 0.30% (21)
  (1,
   (SELECT id FROM factories WHERE chain_id=1 AND address=LOWER('0x1F98431c8aD98523631AE4a59f267346ea31F984')),
   LOWER('0xa3f558aebaecaf0e11ca4b2199cc5ed341edfd74'),
   (SELECT id FROM tokens WHERE chain_id=1 AND address=LOWER('0x5a98fcbea516cf06857215779fd812ca3bef1b32')),
   (SELECT id FROM tokens WHERE chain_id=1 AND address=LOWER('0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2')),
   3000, TRUE),
  -- WSTETH/WETH 0.01% (22)
  (1,
   (SELECT id FROM factories WHERE chain_id=1 AND address=LOWER('0x1F98431c8aD98523631AE4a59f267346ea31F984')),
   LOWER('0x109830a1aaad605bbf02a9dfa7b0b92ec2fb7daa'),
   (SELECT id FROM tokens WHERE chain_id=1 AND address=LOWER('0x7f39c581f595b53c5cb19bd0b3f8da6c935e2ca0')),
   (SELECT id FROM tokens WHERE chain_id=1 AND address=LOWER('0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2')),
   100, TRUE),
  -- RETH/WETH 0.05% (23)
  (1,
   (SELECT id FROM factories WHERE chain_id=1 AND address=LOWER('0x1F98431c8aD98523631AE4a59f267346ea31F984')),
   LOWER('0xa4e0faa58465a2d369aa21b3e42d43374c6f9613'),
   (SELECT id FROM tokens WHERE chain_id=1 AND address=LOWER('0xae78736cd615f374d3085123a210448e74fc6393')),
   (SELECT id FROM tokens WHERE chain_id=1 AND address=LOWER('0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2')),
   500, TRUE),
  -- SHIB/WETH 0.30% (24)
  (1,
   (SELECT id FROM factories WHERE chain_id=1 AND address=LOWER('0x1F98431c8aD98523631AE4a59f267346ea31F984')),
   LOWER('0x2f62f2b4c5fcd7570a709dec05d68ea19c82a9ec'),
   (SELECT id FROM tokens WHERE chain_id=1 AND address=LOWER('0x95ad61b0a150d79219dcf64e1e6cc01f0b64c4ce')),
   (SELECT id FROM tokens WHERE chain_id=1 AND address=LOWER('0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2')),
   3000, TRUE),
  -- PEPE/WETH 0.30% (25)
  (1,
   (SELECT id FROM factories WHERE chain_id=1 AND address=LOWER('0x1F98431c8aD98523631AE4a59f267346ea31F984')),
   LOWER('0x11950d141ecb863f01007add7d1a342041227b58'),
   (SELECT id FROM tokens WHERE chain_id=1 AND address=LOWER('0x6982508145454ce325ddbe47a25d4ec3d2311933')),
   (SELECT id FROM tokens WHERE chain_id=1 AND address=LOWER('0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2')),
   3000, TRUE),
  -- MANA/WETH 0.30% (26)
  (1,
   (SELECT id FROM factories WHERE chain_id=1 AND address=LOWER('0x1F98431c8aD98523631AE4a59f267346ea31F984')),
   LOWER('0x8661ae7918c0115af9e3691662f605e9c550ddc9'),
   (SELECT id FROM tokens WHERE chain_id=1 AND address=LOWER('0x0f5d2fb29fb7d3cfee444a200298f468908cc942')),
   (SELECT id FROM tokens WHERE chain_id=1 AND address=LOWER('0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2')),
   3000, TRUE),
  -- SAND/WETH 0.30% (27)
  (1,
   (SELECT id FROM factories WHERE chain_id=1 AND address=LOWER('0x1F98431c8aD98523631AE4a59f267346ea31F984')),
   LOWER('0x5859ebe6fd3bbc6bd646b73a5dbb09a5d7b6e7b7'),
   (SELECT id FROM tokens WHERE chain_id=1 AND address=LOWER('0x3845badade8e6dff049820680d1f14bd3903a5d0')),
   (SELECT id FROM tokens WHERE chain_id=1 AND address=LOWER('0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2')),
   3000, TRUE),
  -- APE/WETH 0.30% (28)
  (1,
   (SELECT id FROM factories WHERE chain_id=1 AND address=LOWER('0x1F98431c8aD98523631AE4a59f267346ea31F984')),
   LOWER('0xac4b3dacb91461209ae9d41ec517c2b9cb1b7daf'),
   (SELECT id FROM tokens WHERE chain_id=1 AND address=LOWER('0x4d224452801aced8b2f0aebe155379bb5d594381')),
   (SELECT id FROM tokens WHERE chain_id=1 AND address=LOWER('0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2')),
   3000, TRUE),
  -- WETH/ENS 0.30% (29)
  (1,
   (SELECT id FROM factories WHERE chain_id=1 AND address=LOWER('0x1F98431c8aD98523631AE4a59f267346ea31F984')),
   LOWER('0x92560c178ce069cc014138ed3c2f5221ba71f58a'),
   (SELECT id FROM tokens WHERE chain_id=1 AND address=LOWER('0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2')),
   (SELECT id FROM tokens WHERE chain_id=1 AND address=LOWER('0xc18360217d8f7ab5e7c516566761ea12ce7f9d72')),
   3000, TRUE),
  -- MATIC/WETH 0.30% (30)
  (1,
   (SELECT id FROM factories WHERE chain_id=1 AND address=LOWER('0x1F98431c8aD98523631AE4a59f267346ea31F984')),
   LOWER('0x290a6a7460b308ee3f19023d2d00de604bcf5b42'),
   (SELECT id FROM tokens WHERE chain_id=1 AND address=LOWER('0x7d1afa7b718fb893db30a3abc0cfc608aacfebb0')),
   (SELECT id FROM tokens WHERE chain_id=1 AND address=LOWER('0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2')),
   3000, TRUE)
ON CONFLICT (chain_id, address) DO NOTHING;

COMMIT;
