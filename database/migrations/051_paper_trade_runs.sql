-- Migration 051: paper_trade_runs table (BE-3.4)
--
-- Records every opportunity that was suppressed by the paper-mode gate.
-- sim_* fields are populated immediately from the opportunity row.
-- actual_* fields are left NULL and will be populated by a future
-- drift-tracker worker that replays the route against current chain state
-- to measure how much the sim overestimated (or underestimated) the real profit.
--
-- profit_drift_pct = (actual_profit_usd - sim_expected_profit_usd)
--                    / sim_expected_profit_usd
-- A value of -0.05 means sim overestimated by 5%.
-- NULL when actual_* fields are not yet populated.

BEGIN;

CREATE TABLE IF NOT EXISTS paper_trade_runs (
    id                      UUID            PRIMARY KEY DEFAULT gen_random_uuid(),
    opportunity_id          UUID            NOT NULL REFERENCES opportunities(id) ON DELETE CASCADE,
    chain_id                INTEGER         NOT NULL,
    strategy_kind           TEXT            NOT NULL,

    -- Simulator prediction (populated at insert time)
    sim_expected_profit_usd NUMERIC(18, 6)  NOT NULL,
    sim_gas_cost_usd        NUMERIC(18, 6),
    sim_block_number        BIGINT,
    sim_timestamp           TIMESTAMPTZ     NOT NULL DEFAULT NOW(),

    -- Actual re-execution result (populated by future drift-tracker worker)
    -- NUMERIC(78, 0) covers u256 max (2^256-1 has 78 decimal digits).
    actual_amount_out_wei   NUMERIC(78, 0),
    actual_profit_usd       NUMERIC(18, 6),
    actual_gas_cost_usd     NUMERIC(18, 6),
    actual_block_number     BIGINT,
    actual_timestamp        TIMESTAMPTZ,

    -- Drift fractions (computed by drift-tracker, NULL until actual_* populated)
    profit_drift_pct        NUMERIC(10, 4),
    gas_drift_pct           NUMERIC(10, 4),

    created_at              TIMESTAMPTZ     NOT NULL DEFAULT NOW()
);

-- Primary access pattern: look up all paper runs for an opportunity.
CREATE INDEX IF NOT EXISTS idx_paper_trade_runs_opportunity
    ON paper_trade_runs(opportunity_id);

-- Secondary access pattern: aggregate drift metrics by strategy + chain over time.
CREATE INDEX IF NOT EXISTS idx_paper_trade_runs_strategy_chain
    ON paper_trade_runs(strategy_kind, chain_id, created_at DESC);

-- Diagnostic: count pending drift computations (actual_timestamp IS NULL).
CREATE INDEX IF NOT EXISTS idx_paper_trade_runs_pending_drift
    ON paper_trade_runs(created_at DESC)
    WHERE actual_timestamp IS NULL;

DO $$
DECLARE
    row_count INT;
BEGIN
    SELECT COUNT(*) INTO row_count FROM paper_trade_runs;
    RAISE NOTICE 'Migration 051: paper_trade_runs created. Existing rows: %', row_count;
END $$;

COMMIT;
