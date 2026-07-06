-- ArbitrageX v2 — Migration 100: opportunity trade math (PR 4 enrichment)
--
-- Purpose: Persist the full per-opportunity trade math so /api/opportunities/live
-- can serve the complete picture (buy/sell price, amount_in/out in wei+token+USD,
-- start/end value, gross/net profit, net ROI, fee breakdown, pool pair, route legs)
-- instead of only the aggregate gross (expected_profit_usd, mig 003:16) and the
-- spine net (net_expected_profit_usd, mig 049:18).
--
-- Today the scanner computes per-pool amount_out quotes (scanner.rs:1638,
-- 1675-1680) and the spine evaluator computes a 9-component cost breakdown, but
-- both are discarded after the aggregate is written. mig 049 keeps the net
-- aggregate only; mig 099 keeps route topology only. PR 4 plumbs the full math
-- through to the row so the live card can render real evidence, not aggregates.
--
-- Doctrine (RULE 00 Zero-Mocks + R8 Fail-Honest + Gate 2 net-profit):
--   * Every column is NULLable. NULL = "scanner could not produce real evidence".
--     The API serializer MUST be null-safe (existing rows without these columns
--     and new rows where the scanner had no quote both deserialize); the
--     frontend card renders "—".
--   * No NOT NULL, no DEFAULT — existing (pre-PR-4) rows must still pass INSERT
--     and SELECT cleanly, and backfilling would fabricate values (R8 violation).
--   * cost_breakdown_json and route_legs are JSONB but kept NULLable (unlike
--     route_metadata in mig 099:29 which is NOT NULL DEFAULT '{}') because a
--     missing breakdown is honest evidence-of-absence, not an empty object.
--   * net_profit_usd / net_roi_pct are the authoritative Gate-2 fields; the
--     legacy net_expected_profit_usd (mig 049, NUMERIC(18,6)) is left
--     untouched so submit_engine Check 7 and its fallback path stay intact.
--
-- Idempotent: ALTER TABLE ... ADD COLUMN IF NOT EXISTS (the project's column
-- pattern from mig 033:10-13 and mig 099:28-29). Forward-only, no DOWN.
-- Non-destructive: ADD COLUMN only — no drops, no type changes, no constraint
-- additions beyond column defaults.
--
-- Type conventions used:
--   USD values      → NUMERIC(20,8)   (matches expected_profit_usd 003:16,
--                                       bridge_fee_usd 033:13)
--   wei / raw ints  → NUMERIC(78,0)   (matches amount_in_wei 003:15)
--   token amounts   → NUMERIC(40,18)  (decimal-adjusted quantity, 18 dp max)
--   percentages     → NUMERIC(10,4)   (matches roi_pct 003:17)
--   addresses       → TEXT            (matches token_in / dex_a 003:10,13)
--   breakdown/legs  → JSONB

BEGIN;

ALTER TABLE opportunities
    ADD COLUMN IF NOT EXISTS buy_price_usd        NUMERIC(20,8)  NULL,
    ADD COLUMN IF NOT EXISTS sell_price_usd       NUMERIC(20,8)  NULL,
    ADD COLUMN IF NOT EXISTS amount_out_wei       NUMERIC(78,0)  NULL,
    ADD COLUMN IF NOT EXISTS amount_out_token     NUMERIC(40,18) NULL,
    ADD COLUMN IF NOT EXISTS amount_out_usd       NUMERIC(20,8)  NULL,
    ADD COLUMN IF NOT EXISTS amount_in_token      NUMERIC(40,18) NULL,
    ADD COLUMN IF NOT EXISTS amount_in_usd        NUMERIC(20,8)  NULL,
    ADD COLUMN IF NOT EXISTS start_value_usd      NUMERIC(20,8)  NULL,
    ADD COLUMN IF NOT EXISTS end_value_usd        NUMERIC(20,8)  NULL,
    ADD COLUMN IF NOT EXISTS gross_profit_usd     NUMERIC(20,8)  NULL,
    ADD COLUMN IF NOT EXISTS net_profit_usd       NUMERIC(20,8)  NULL,
    ADD COLUMN IF NOT EXISTS net_roi_pct          NUMERIC(10,4)  NULL,
    ADD COLUMN IF NOT EXISTS total_fees_usd       NUMERIC(20,8)  NULL,
    ADD COLUMN IF NOT EXISTS cost_breakdown_json  JSONB          NULL,
    ADD COLUMN IF NOT EXISTS pool_buy             TEXT           NULL,
    ADD COLUMN IF NOT EXISTS pool_sell            TEXT           NULL,
    ADD COLUMN IF NOT EXISTS route_legs           JSONB          NULL;

-- No backfill: pre-PR-4 rows retain NULL on every new column (R8 fail-honest).
-- Imputing price/amount/profit values for historical rows would fabricate
-- evidence the scanner did not produce.

COMMENT ON COLUMN opportunities.buy_price_usd IS
    'PR 4: USD price of token_in at the buy leg (entry price). NULL if scanner has no real quote (R8).';
COMMENT ON COLUMN opportunities.sell_price_usd IS
    'PR 4: USD price of token_out at the sell leg (exit price). NULL if scanner has no real quote (R8).';
COMMENT ON COLUMN opportunities.amount_out_wei IS
    'PR 4: raw get_amounts_out quote in wei / token smallest unit. NULL if scanner discarded the quote (scanner.rs:1638,1675-1680).';
COMMENT ON COLUMN opportunities.amount_out_token IS
    'PR 4: amount_out decimal-adjusted to token units. NULL if token decimals unknown (R8).';
COMMENT ON COLUMN opportunities.amount_out_usd IS
    'PR 4: USD value of amount_out at sell_price_usd. NULL if price unknown (R8).';
COMMENT ON COLUMN opportunities.amount_in_token IS
    'PR 4: amount_in decimal-adjusted to token units. NULL if token decimals unknown (R8).';
COMMENT ON COLUMN opportunities.amount_in_usd IS
    'PR 4: USD value of amount_in at buy_price_usd. NULL if price unknown (R8).';
COMMENT ON COLUMN opportunities.start_value_usd IS
    'PR 4: USD value of the position at entry (capital deployed). NULL if not computed (R8).';
COMMENT ON COLUMN opportunities.end_value_usd IS
    'PR 4: USD value of the position at exit (proceeds). NULL if not computed (R8).';
COMMENT ON COLUMN opportunities.gross_profit_usd IS
    'PR 4: gross profit (end_value - start_value) before fees/slippage/gas. Mirrors but does NOT replace expected_profit_usd (worker-level gross, mig 003:16). NULL if not computed (R8).';
COMMENT ON COLUMN opportunities.net_profit_usd IS
    'PR 4 Gate-2: net profit after all fees/slippage/gas. Authoritative net field for the live card. Coexists with legacy net_expected_profit_usd (mig 049:18, NUMERIC(18,6)) which drives submit_engine Check 7. NULL if spine evaluator did not run (R8).';
COMMENT ON COLUMN opportunities.net_roi_pct IS
    'PR 4: net ROI = net_profit_usd / start_value_usd * 100. Precision matches roi_pct (NUMERIC(10,4), mig 003:17). NULL if start_value_usd is 0 or NULL (R8).';
COMMENT ON COLUMN opportunities.total_fees_usd IS
    'PR 4: sum of all fee components (gas + bridge + protocol + slippage + flash-loan + ...) from cost_breakdown_json. NULL if breakdown absent (R8).';
COMMENT ON COLUMN opportunities.cost_breakdown_json IS
    'PR 4: full 9-component cost breakdown persisted on CANONICAL spine rows {gas, bridge_fee, protocol_fee, slippage_cost, flash_loan_fee, ...}. Today discarded post-aggregate (mig 049 keeps net only). JSONB, NULLable (R8).';
COMMENT ON COLUMN opportunities.pool_buy IS
    'PR 4: liquidity-pool address bought from (entry leg). TEXT, no FK (matches dex_a / token_in convention mig 003:10,13). NULL if single-hop or unknown (R8).';
COMMENT ON COLUMN opportunities.pool_sell IS
    'PR 4: liquidity-pool address sold into (exit leg). TEXT, no FK. NULL if single-hop or unknown (R8).';
COMMENT ON COLUMN opportunities.route_legs IS
    'PR 4: ordered JSONB array of route legs [{dex, pool, token_in, token_out, amount_in, amount_out}, ...]. Distinct from route_metadata (mig 099:29, pool/token topology only) — route_legs carries the per-leg amounts. NULL if route not reconstructed (R8).';

COMMIT;
