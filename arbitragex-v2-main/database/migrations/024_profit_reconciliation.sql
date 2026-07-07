-- Profit Reconciliation Table
-- The ultimate source of truth for realized PnL on-chain.

CREATE TABLE profit_reconciliation (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    opportunity_id UUID, -- References opportunities(id)
    execution_id UUID, -- References execution_receipts(id)
    chain_id INT NOT NULL,
    tx_hash VARCHAR(66) UNIQUE NOT NULL,
    block_number BIGINT NOT NULL,
    wallet_address VARCHAR(42) NOT NULL,
    balance_before_eth NUMERIC(38, 18) NOT NULL,
    balance_after_eth NUMERIC(38, 18) NOT NULL,
    gas_used BIGINT NOT NULL,
    gas_cost_native NUMERIC(38, 18) NOT NULL,
    flashloan_fee_usd NUMERIC(10, 2) DEFAULT 0,
    protocol_fee_usd NUMERIC(10, 2) DEFAULT 0,
    expected_profit_usd NUMERIC(15, 2) NOT NULL,
    realized_profit_usd NUMERIC(15, 2) NOT NULL,
    reconciliation_status VARCHAR(20) CHECK (reconciliation_status IN ('PENDING', 'CONFIRMED', 'FAILED', 'DISCREPANCY')),
    audit_hash VARCHAR(66) NOT NULL,
    explorer_url TEXT NOT NULL,
    created_at TIMESTAMPTZ DEFAULT NOW()
);
