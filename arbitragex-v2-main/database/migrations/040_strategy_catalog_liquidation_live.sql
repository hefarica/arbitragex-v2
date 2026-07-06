-- ArbitrageX v2 — Migration 040: liquidation strategy lifecycle scaffold → live
--
-- Promotes the `liquidation` strategy from `scaffold` (StrategyKind enum +
-- persistence accept the kind, no emitter) to `live` (LiquidationWorker now
-- scans operator-managed Aave V3 watchlist members every 30s and emits
-- opportunities to `arbx:opps:detected` + the `opportunities` table).
--
-- Strategy details: monitor an operator-curated set of Aave V3 borrower
-- addresses (Redis SET `arbx:aave_v3_watchlist:<chain>`). For each member
-- read `Pool.getUserAccountData(user)` via Multicall3 to obtain
-- `(totalCollateralBase, totalDebtBase, _, _, _, healthFactor)`. When
-- `healthFactor < 1.05e18` (1.05 buffer ahead of the on-chain trigger of
-- HF<1.0), estimate net profit:
--   debt_to_repay = min(totalDebtBase × 0.50 close-factor, $250k cap)
--   gross_profit  = debt_to_repay × bonus_pct        (MVP: 5% default)
--   net_profit    = gross_profit − $30 gas
-- Emit when net_profit ≥ $1 and the worker-level sanity bound (20% of
-- debt_to_repay) holds.
--
-- The UI badge on /strategies auto-updates because `lifecycle_status` is the
-- single source of truth read by the frontend.
--
-- Worker hardening (anti-Incidente #9):
--   - Worker-level sanity bound: rejects gross_profit_usd > 20% of debt
--     (real Aave V3 bonuses cap at ~10%; 20% catches calculation bugs
--     without false-positives on legitimate high-bonus assets).
--   - Per-block dedup on (user_address, block_number) tuple.
--   - Periodic INFO `tick_stats` log every 5 ticks (~150s) for operator
--     visibility into the dominant skip bucket.
--   - Diagnostic dump on every sanity_reject with full per-position context.
--
-- Operator watchlist seeding: `SADD arbx:aave_v3_watchlist:1 0x<borrower>`.
-- Empty watchlist = worker skips the cycle (R8 fail-honest, no fabricated
-- targets). The `skip_empty_watchlist` counter surfaces the gap to the
-- operator without crashing the worker.
--
-- Rollback path: if the worker misbehaves, the operator can demote with
--   UPDATE strategy_catalog SET lifecycle_status = 'scaffold' WHERE kind = 'liquidation';
-- and the badge reverts without redeploying.
--
-- Idempotent — safe to re-run.

UPDATE strategy_catalog
   SET lifecycle_status = 'live'
 WHERE kind = 'liquidation';

-- Sanity: confirm the row exists post-update.
DO $$
DECLARE
  promoted_count INT;
BEGIN
  SELECT COUNT(*) INTO promoted_count
    FROM strategy_catalog
   WHERE kind = 'liquidation' AND lifecycle_status = 'live';
  IF promoted_count = 0 THEN
    RAISE WARNING
      'migration 040: no row for kind=liquidation found OR not promoted to live; '
      'verify migration 028 + 035 ran first';
  END IF;
END $$;
