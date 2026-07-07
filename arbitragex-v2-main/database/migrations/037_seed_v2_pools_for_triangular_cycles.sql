-- ArbitrageX v2 — Migration 037: seed two V2 pools to unblock triangular cycles.
--
-- Diagnosis (2026-05-07): TriangularWorker tick_stats reported
-- dominant_skip = "missing_pool" with 70/90 attempts (78%) failing because
-- 7 of 9 cycles reference pool pairs absent from the V2 pool index. Of
-- those 7, two are unblockable today by adding the missing V2 pool rows:
--
--   1. WETH-WBTC-USDC cycle  → needs WBTC/USDC V2 pool (currently missing)
--   2. USDC-DAI-USDT cycle   → needs DAI/USDT V2 pool (currently missing)
--   3. WETH-USDT-DAI cycle   → ALSO needs DAI/USDT V2 (same pool)
--
-- The other 4 missing cycles (PEPE-WETH-USDC, SHIB-WETH-USDC,
-- MKR-WETH-USDC, COMP-WETH-USDC) require pools like USDC/PEPE V2 that
-- DO NOT EXIST on Uniswap V2 — long-tail tokens trade primarily on V3.
-- Those cycles are removed from MVP_CYCLES in the same commit; future
-- work extends the worker to use V3 quoter for multi-DEX cycles.
--
-- Pool addresses below are well-known public Uniswap V2 mainnet contracts.
-- ON CONFLICT DO NOTHING is idempotent. If any address turns out wrong,
-- pool_sync_worker logs "pool_sync.pool_failed" for that pool only and
-- the rest of the universe keeps ticking (allow_failure=true on Multicall3).
--
-- After applying: restart searcher-rs to re-bootstrap the pool index from
-- PG into Redis (pool_sync_worker reads pools+tokens at startup).

BEGIN;

-- ── WBTC/USDC on Uniswap V2 ──────────────────────────────────────────────
-- token0 (lower address) = USDC (0xa0b8...), token1 = WBTC (0x2260...).
-- Pool 0x004375dff511095cc5a197a54140a24efef3a416 is the USDC/WBTC pair
-- on Uniswap V2 (verifiable on Etherscan).
INSERT INTO pools (chain_id, factory_id, address, token0_id, token1_id, fee_tier)
SELECT 1, f.id, '0x004375dff511095cc5a197a54140a24efef3a416',
       (SELECT id FROM tokens WHERE chain_id=1 AND symbol='USDC'),
       (SELECT id FROM tokens WHERE chain_id=1 AND symbol='WBTC'),
       30
FROM factories f JOIN dexes d ON d.id=f.dex_id WHERE d.name='UniswapV2' AND f.chain_id=1
ON CONFLICT (chain_id, address) DO NOTHING;

-- ── DAI/USDT on Uniswap V2 ───────────────────────────────────────────────
-- token0 (lower address) = DAI (0x6b17...), token1 = USDT (0xdac1...).
-- Pool 0xb20bd5d04be54f870d5c0d3ca85d82b34b836405 is the DAI/USDT pair
-- on Uniswap V2 (verifiable on Etherscan).
INSERT INTO pools (chain_id, factory_id, address, token0_id, token1_id, fee_tier)
SELECT 1, f.id, '0xb20bd5d04be54f870d5c0d3ca85d82b34b836405',
       (SELECT id FROM tokens WHERE chain_id=1 AND symbol='DAI'),
       (SELECT id FROM tokens WHERE chain_id=1 AND symbol='USDT'),
       30
FROM factories f JOIN dexes d ON d.id=f.dex_id WHERE d.name='UniswapV2' AND f.chain_id=1
ON CONFLICT (chain_id, address) DO NOTHING;

-- Sanity: confirm both pools landed (or were already present).
DO $$
DECLARE
  n INT;
BEGIN
  SELECT COUNT(*) INTO n FROM pools p
    WHERE p.chain_id = 1
      AND p.address IN (
        '0x004375dff511095cc5a197a54140a24efef3a416',
        '0xb20bd5d04be54f870d5c0d3ca85d82b34b836405'
      );
  IF n < 2 THEN
    RAISE WARNING 'migration 037: expected 2 pools present after upsert, found %', n;
  END IF;
END $$;

COMMIT;
