-- ArbitrageX v2 — Migration 017: price_oracles (operator-configured price feeds).
--
-- No-hardcode doctrine: recon uses oracles to verify PnL. Oracle addresses must
-- come from the operator; we don't ship with a list of "trusted" price feeds
-- because trust is the operator's call.

CREATE TABLE IF NOT EXISTS price_oracles (
  id             UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
  chain_id       INTEGER     NOT NULL,
  kind           TEXT        NOT NULL CHECK (kind IN ('chainlink','uniswap_twap','custom')),
  token_address  TEXT        NOT NULL, -- the asset this oracle prices
  quote_symbol   TEXT        NOT NULL, -- 'USD' | 'ETH' | 'USDC' | ...
  oracle_address TEXT        NOT NULL, -- contract address of the feed
  decimals       INTEGER     NOT NULL DEFAULT 8,
  enabled        BOOLEAN     NOT NULL DEFAULT FALSE,
  description    TEXT,
  created_by     TEXT        NOT NULL,
  created_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  CONSTRAINT price_oracles_uq UNIQUE (chain_id, kind, token_address, quote_symbol)
);

CREATE INDEX IF NOT EXISTS idx_price_oracles_lookup
  ON price_oracles(chain_id, token_address, enabled);

DROP TRIGGER IF EXISTS trg_price_oracles_updated_at ON price_oracles;
CREATE TRIGGER trg_price_oracles_updated_at BEFORE UPDATE ON price_oracles
  FOR EACH ROW EXECUTE FUNCTION arbx_touch_updated_at();

GRANT SELECT, INSERT, UPDATE ON price_oracles TO arbx_rw;
GRANT SELECT ON price_oracles TO arbx_ro;
