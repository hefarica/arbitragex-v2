-- ArbitrageX v2 — Migration 041: seed 4 V3 long-tail/USDC pools.
--
-- Unblocks the 4 long-tail triangular cycles (PEPE/SHIB/MKR/COMP × WETH ×
-- USDC) shipped in commit dcc1677 (B v2). Those cycles need a USDC<->X V3
-- hop (no V2 pool exists for these pairs against USDC). After mig 037
-- restored long-tail tokens to the allowlist + B v2 added V3 quoter
-- support, the only remaining gap was the V3 pool index entry — this
-- migration closes it.
--
-- Pool addresses fetched on-chain via factory.getPool() against
-- 0x1F98431c8aD98523631AE4a59f267346ea31F984 (Uniswap V3 mainnet
-- factory) on 2026-05-07. Each entry validated to be non-zero.
-- Selected fee tier 3000 (0.30%) for all 4 — the standard tier
-- with confirmed pool existence across all four pairs.
--
-- token0/token1 ordering follows the V3 invariant (lower address = token0):
--   PEPE (0x6982...) < USDC (0xa0b8...) → token0=PEPE
--   SHIB (0x95ad...) < USDC (0xa0b8...) → token0=SHIB
--   MKR  (0x9f8f...) < USDC (0xa0b8...) → token0=MKR
--   COMP (0xc00e...) > USDC (0xa0b8...) → token0=USDC, token1=COMP
--
-- After applying: restart searcher-rs to refresh PoolSyncWorker bootstrap
-- (which writes V3 pool index to Redis from the pools table).

BEGIN;

INSERT INTO pools (chain_id, factory_id, address, token0_id, token1_id, fee_tier, is_active)
VALUES
  -- PEPE/USDC 0.30% — fee tier 3000, on-chain factory.getPool() verified.
  (1,
   (SELECT id FROM factories WHERE chain_id=1 AND address=LOWER('0x1F98431c8aD98523631AE4a59f267346ea31F984')),
   LOWER('0x261d53f3cd0b38dabbab252dcc8adeaa8c67bcba'),
   (SELECT id FROM tokens WHERE chain_id=1 AND address=LOWER('0x6982508145454ce325ddbe47a25d4ec3d2311933')), -- PEPE (token0, lower)
   (SELECT id FROM tokens WHERE chain_id=1 AND address=LOWER('0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48')), -- USDC
   3000, TRUE),

  -- SHIB/USDC 0.30%
  (1,
   (SELECT id FROM factories WHERE chain_id=1 AND address=LOWER('0x1F98431c8aD98523631AE4a59f267346ea31F984')),
   LOWER('0xe9131a276fd07af729b5537a429346e8affc67e0'),
   (SELECT id FROM tokens WHERE chain_id=1 AND address=LOWER('0x95ad61b0a150d79219dcf64e1e6cc01f0b64c4ce')), -- SHIB (token0, lower)
   (SELECT id FROM tokens WHERE chain_id=1 AND address=LOWER('0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48')), -- USDC
   3000, TRUE),

  -- MKR/USDC 0.30%
  (1,
   (SELECT id FROM factories WHERE chain_id=1 AND address=LOWER('0x1F98431c8aD98523631AE4a59f267346ea31F984')),
   LOWER('0x0f9d9d1cce530c91f075455efef2d9386375df3d'),
   (SELECT id FROM tokens WHERE chain_id=1 AND address=LOWER('0x9f8f72aa9304c8b593d555f12ef6589cc3a579a2')), -- MKR (token0, lower)
   (SELECT id FROM tokens WHERE chain_id=1 AND address=LOWER('0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48')), -- USDC
   3000, TRUE),

  -- COMP/USDC 0.30% — note: USDC is token0 here because USDC < COMP by address.
  (1,
   (SELECT id FROM factories WHERE chain_id=1 AND address=LOWER('0x1F98431c8aD98523631AE4a59f267346ea31F984')),
   LOWER('0xf15054bc50c39ad15fdc67f2aedd7c2c945ca5f6'),
   (SELECT id FROM tokens WHERE chain_id=1 AND address=LOWER('0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48')), -- USDC (token0)
   (SELECT id FROM tokens WHERE chain_id=1 AND address=LOWER('0xc00e94cb662c3520282e6f5717214004a7f26888')), -- COMP
   3000, TRUE)
ON CONFLICT (chain_id, address) DO NOTHING;

-- Sanity: confirm all 4 pools are resolvable (token0_id and token1_id are NOT NULL).
DO $$
DECLARE
  bad_count INT;
BEGIN
  SELECT COUNT(*) INTO bad_count
    FROM pools
   WHERE chain_id = 1
     AND address IN (
       LOWER('0x261d53f3cd0b38dabbab252dcc8adeaa8c67bcba'),
       LOWER('0xe9131a276fd07af729b5537a429346e8affc67e0'),
       LOWER('0x0f9d9d1cce530c91f075455efef2d9386375df3d'),
       LOWER('0xf15054bc50c39ad15fdc67f2aedd7c2c945ca5f6')
     )
     AND (token0_id IS NULL OR token1_id IS NULL);
  IF bad_count > 0 THEN
    RAISE EXCEPTION 'migration 041: % pool rows have NULL token0_id/token1_id (token row missing for PEPE/SHIB/MKR/COMP/USDC?)', bad_count;
  END IF;
END $$;

COMMIT;
