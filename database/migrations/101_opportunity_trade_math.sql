-- 101_opportunity_trade_math.sql
-- PR 4 (opportunities consolidation): persist the FULL trade math the scanner
-- already computes (buy/sell price per DEX, amount_out, capital in/out, fees,
-- pools) so /api/opportunities/live can serve it and the card v2 (PR 6) can
-- render the full "capital in -> buy -> sell -> final out" picture.
--
-- ALL columns NULLable (R8 fail-honest): existing rows pass unchanged; the card
-- renders "—" for every new cell until the scanner wiring (PR 4b) populates them
-- from REAL pool quotes. No fabricated values (RULE 00) — the column simply
-- stays NULL when the scanner cannot produce the datum with real evidence.
--
-- Idempotent: ADD COLUMN IF NOT EXISTS (the project's standard pattern, mig 099).
-- FORWARD-ONLY. No DOWN. No destructive ops. No NOT NULL.
--
-- Non-duplicative (intentional omissions):
--   * gross_profit_usd is the EXISTING expected_profit_usd column (003:16) — the
--     canonical GROSS figure (scanner spread). NOT re-added; surfaced on the wire
--     under its existing name.
--   * net_profit_usd is the EXISTING net_expected_profit_usd column (mig 049) —
--     the canonical NET figure (spine evaluator). NOT re-added.
--   * route_legs topology is the EXISTING route_metadata JSONB (mig 099). A
--     richer per-leg {amount_in, amount_out} extension ships with PR 4b/4c when
--     the scanner can populate it; not duplicated here.

ALTER TABLE opportunities
    ADD COLUMN IF NOT EXISTS buy_price_usd      NUMERIC(20,8),
    ADD COLUMN IF NOT EXISTS sell_price_usd     NUMERIC(20,8),
    ADD COLUMN IF NOT EXISTS amount_out_wei     NUMERIC(78,0),
    ADD COLUMN IF NOT EXISTS amount_in_token    NUMERIC(40,18),
    ADD COLUMN IF NOT EXISTS amount_out_token   NUMERIC(40,18),
    ADD COLUMN IF NOT EXISTS amount_in_usd      NUMERIC(20,8),
    ADD COLUMN IF NOT EXISTS amount_out_usd     NUMERIC(20,8),
    ADD COLUMN IF NOT EXISTS start_value_usd    NUMERIC(20,8),
    ADD COLUMN IF NOT EXISTS end_value_usd      NUMERIC(20,8),
    ADD COLUMN IF NOT EXISTS net_roi_pct        NUMERIC(10,4),
    ADD COLUMN IF NOT EXISTS total_fees_usd     NUMERIC(20,8),
    ADD COLUMN IF NOT EXISTS pool_buy           TEXT,
    ADD COLUMN IF NOT EXISTS pool_sell          TEXT;

COMMENT ON COLUMN opportunities.buy_price_usd IS
    'PR4: real buy-side per-unit price (token_in USD) from the pool-A quote. NULL until scanner wiring (PR4b) populates from real reserves. R8 fail-honest.';
COMMENT ON COLUMN opportunities.sell_price_usd IS
    'PR4: real sell-side per-unit price (token_out USD) from the pool-B quote. NULL until PR4b.';
COMMENT ON COLUMN opportunities.amount_out_wei IS
    'PR4: raw amount_out (token_out received) in wei, surfaced as decimal string on the wire. NULL until PR4b (scanner computes this today and discards it).';
COMMENT ON COLUMN opportunities.amount_in_token IS
    'PR4: amount_in in human token units (amount_in_wei / 10^token_in_decimals). NULL until PR4b.';
COMMENT ON COLUMN opportunities.amount_out_token IS
    'PR4: amount_out in human token units. NULL until PR4b.';
COMMENT ON COLUMN opportunities.amount_in_usd IS
    'PR4: capital in USD (amount_in_token * token_in real price). NULL until PR4b (depends on PR5 price attach for the USD conversion).';
COMMENT ON COLUMN opportunities.amount_out_usd IS
    'PR4: capital out USD (amount_out_token * token_out real price). NULL until PR4b/PR5.';
COMMENT ON COLUMN opportunities.start_value_usd IS
    'PR4: trade start value USD (= amount_in_usd at the buy quote). NULL until PR4b.';
COMMENT ON COLUMN opportunities.end_value_usd IS
    'PR4: trade end value USD (= amount_out_usd at the sell quote). NULL until PR4b.';
COMMENT ON COLUMN opportunities.net_roi_pct IS
    'PR4: NET roi % (after all costs) — distinct from roi_pct (which is the gross/pre-cost spread). NULL until PR4b.';
COMMENT ON COLUMN opportunities.total_fees_usd IS
    'PR4: sum of every fee component (gas + lp + slippage + flashloan + relay + capital + failure_buffer + ops_overhead). NULL until PR4b.';
COMMENT ON COLUMN opportunities.pool_buy IS
    'PR4: pool address for the buy leg (dex_a side). NULL until PR4b.';
COMMENT ON COLUMN opportunities.pool_sell IS
    'PR4: pool address for the sell leg (dex_b side). NULL until PR4b.';
