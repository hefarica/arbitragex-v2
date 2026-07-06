-- ArbitrageX v2 — Migration 039: flashloan_arb strategy lifecycle scaffold → live
--
-- Promotes the `flashloan_arb` strategy from `scaffold` (StrategyKind enum +
-- persistence accept the kind, no emitter) to `live` (FlashloanArbWorker now
-- scans MVP token pairs every block and emits opportunities to
-- `arbx:opps:detected` + the `opportunities` table).
--
-- Strategy details: borrow token A from a flash-loan provider (Aave V3 / Balancer
-- Vault) atomically, swap A→A across two V2 pools holding the SAME pair (e.g.
-- UniV2 WETH/USDC vs SushiV2 WETH/USDC), repay loan + premium, keep residual
-- profit. Capital cap is structural: own capital = 0; constraint is pool depth
-- + flash provider liquidity + min profit > gas + premium.
--
-- The UI badge on /strategies auto-updates because `lifecycle_status` is the
-- single source of truth read by the frontend.
--
-- Worker hardening (anti-Incidente #9):
--   - Worker-level sanity bound: rejects expected_profit_usd > 10% of borrow_usd
--     (real flash arbs are 0.05-0.5%; 10% means orientation/decimal/unit bug).
--   - Per-block dedup on (buy_pool, sell_pool, block) triple.
--   - Periodic INFO `tick_stats` log every 5 ticks for operator visibility.
--   - Diagnostic dump on every sanity_reject with full per-pool reserves +
--     orientation snapshot.
--
-- Rollback path: if the worker misbehaves, the operator can demote with
--   UPDATE strategy_catalog SET lifecycle_status = 'scaffold' WHERE kind = 'flashloan_arb';
-- and the badge reverts without redeploying.
--
-- Idempotent — safe to re-run.

UPDATE strategy_catalog
   SET lifecycle_status = 'live'
 WHERE kind = 'flashloan_arb';

-- Sanity: confirm the row exists post-update.
DO $$
DECLARE
  promoted_count INT;
BEGIN
  SELECT COUNT(*) INTO promoted_count
    FROM strategy_catalog
   WHERE kind = 'flashloan_arb' AND lifecycle_status = 'live';
  IF promoted_count = 0 THEN
    RAISE WARNING
      'migration 039: no row for kind=flashloan_arb found OR not promoted to live; '
      'verify migration 028 + 035 ran first';
  END IF;
END $$;
