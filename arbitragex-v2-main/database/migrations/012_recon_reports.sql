-- ArbitrageX v2 — Migration 012: recon_reports
-- Post-hoc analysis of each executed opportunity: receipt + log decode + variance.
-- Separate from `executions` to keep the submit-intent (execution) distinct from
-- the ground-truth analysis (recon_report).

CREATE TABLE IF NOT EXISTS recon_reports (
  id                         UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  opportunity_id             UUID NOT NULL REFERENCES opportunities(id) ON DELETE CASCADE,
  execution_id               UUID REFERENCES executions(id),
  tx_hash                    TEXT,
  chain_id                   INTEGER NOT NULL,

  expected_amount_out_wei    NUMERIC(78,0),
  actual_amount_out_wei      NUMERIC(78,0),
  variance_native_units      NUMERIC(78,0),
  variance_pct               NUMERIC(10,4),

  expected_profit_usd        NUMERIC(20,8),
  actual_profit_usd          NUMERIC(20,8),
  pnl_source                 TEXT NOT NULL CHECK (pnl_source IN (
                                  'native_only','oracle_chainlink',
                                  'oracle_uniswap_twap','derived','unavailable')),

  actual_gas_used_wei        NUMERIC(78,0),
  actual_gas_price_wei       NUMERIC(78,0),

  fail_reason                TEXT,
  raw_receipt                JSONB,
  trace_id                   UUID NOT NULL,
  created_at                 TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_recon_opp  ON recon_reports(opportunity_id);
CREATE INDEX IF NOT EXISTS idx_recon_time ON recon_reports(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_recon_tx   ON recon_reports(tx_hash)
    WHERE tx_hash IS NOT NULL;

GRANT SELECT, INSERT ON recon_reports TO arbx_rw;
GRANT SELECT ON recon_reports TO arbx_ro;
