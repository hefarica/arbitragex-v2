-- ArbitrageX v2 — Migration 046: chain-aware min_profit_usd floors (economic defect E2).
--
-- Problem (economics-validator E2): the old default of min_profit_usd = 1.0 was
-- an accounting illusion. Ethereum gas alone costs $9-50 per arbitrage transaction
-- (450k gas × 7 gwei × $3000/ETH raw; 3x safety + 20% failed-tx cost = $35-50).
-- A $1 floor causes the system to "execute" opportunities that lose $40-49 net in
-- gas every single time.
--
-- Fix: seed realistic chain-specific floors derived from observed gas costs:
--   Ethereum (1):    $50  — 450k gas × 7gwei × $3000/ETH × 3x safety
--   Arbitrum (42161): $10  — L2 execution + L1 calldata overhead
--   BSC (56):         $5   — 300k gas × 3gwei × BNB price
--   Optimism (10):    $5   — L2 gas + L1 data
--   Base (8453):      $5   — L2 gas + L1 data
--   Polygon (137):    $2   — cheapest MATIC gas; still 2x the broken $1 floor
--
-- These values update EXISTING rows via ON CONFLICT. Rows that do not yet exist
-- (chain not yet configured by the operator) are inserted with safe placeholder
-- values so the CHECK constraint (min_profit_usd >= 0) is satisfied.
--
-- Idempotent: safe to re-run. The DO $$ ... $$ invariant check verifies the
-- migration actually took effect before COMMIT.

BEGIN;

INSERT INTO trading_config (
    chain_id,
    capital_usd,
    base_token_symbol,
    base_token_price_usd,
    allowed_token_symbols,
    min_profit_usd,
    min_roi_pct,
    max_slippage_pct,
    gas_estimate_units,
    enabled_strategies,
    enabled,
    updated_by
) VALUES
    (1,     0, 'WETH', 3000.00, '{}', 50.0, 0.1, 0.5, 450000, '{}', false, 'migration_046'),
    (42161, 0, 'WETH', 3000.00, '{}', 10.0, 0.1, 0.5, 300000, '{}', false, 'migration_046'),
    (56,    0, 'WBNB', 600.00,  '{}', 5.0,  0.1, 0.5, 300000, '{}', false, 'migration_046'),
    (10,    0, 'WETH', 3000.00, '{}', 5.0,  0.1, 0.5, 300000, '{}', false, 'migration_046'),
    (8453,  0, 'WETH', 3000.00, '{}', 5.0,  0.1, 0.5, 300000, '{}', false, 'migration_046'),
    (137,   0, 'WMATIC',1.00,   '{}', 2.0,  0.1, 0.5, 300000, '{}', false, 'migration_046')
ON CONFLICT (chain_id) DO UPDATE
    SET min_profit_usd = EXCLUDED.min_profit_usd,
        updated_by     = 'migration_046',
        updated_at     = NOW();

-- Invariant: all 6 target chains must have min_profit_usd > 1.0 after this migration.
-- If any row is missing or retains the broken $1 value, the migration FAILS loudly.
DO $$
DECLARE
    row_count INT;
BEGIN
    SELECT COUNT(*) INTO row_count
    FROM trading_config
    WHERE chain_id IN (1, 42161, 56, 10, 8453, 137)
      AND min_profit_usd > 1.0;

    IF row_count < 6 THEN
        RAISE EXCEPTION
            'Migration 046 invariant violated: expected 6 chains with min_profit_usd > 1.0, found %. '
            'All chains (1=ETH $50, 42161=ARB $10, 56=BSC $5, 10=OP $5, 8453=Base $5, 137=Matic $2) '
            'must be updated.',
            row_count;
    END IF;
END $$;

COMMIT;
