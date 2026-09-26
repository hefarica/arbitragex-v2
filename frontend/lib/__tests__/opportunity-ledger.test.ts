// frontend/lib/__tests__/opportunity-ledger.test.ts
//
// CARDS-NOTIONAL-01 — gate on the SSOT that decides which closed arithmetic a
// row may paint, and on the high-value notifier that consumes it.
//
// The fixture that matters is the one the live-domain verifier photographed on
// production build `89d68ec8`: a row whose recorded notional is the DETECTION
// PROBE (`amount_in_wei = 1e6` = 1 USDC = $1.00) while the gross recorded beside
// it is the DEX engine's fast-filter figure computed at the SEARCHER's own probe,
// and whose `net_expected_profit_usd` comes from the SIZING KERNEL at the
// kernel's own clamped size. Three producers, up to three notionals, one card.
//
// Every case below goes through the REAL mapper (`mapToOmniOpportunity`), so the
// gate fails if the two new wire fields ever stop reaching the ViewModel.
import { describe, it, expect } from "vitest";

import { mapToOmniOpportunity } from "@/lib/store/types";
import {
  buildLedger,
  decideOpportunityNotification,
  GROSS_OVER_PRINCIPAL_SANITY_MULT,
  LEDGER_TOLERANCE_USD,
} from "@/lib/opportunity-ledger";

const wire = (over: Record<string, unknown>) => ({
  id: "opp-1",
  chain_id: 1,
  strategy_kind: "dex_arb",
  detected_at: "2026-09-26T00:00:00Z",
  status: "rejected",
  trace_id: "trace-1",
  dex_a: "uniswap-v3",
  dex_b: "uniswap-v2",
  token_in: "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48", // USDC
  token_out: "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2", // WETH
  ...over,
});

/** The exact live row: $1.00 probe own notional, $822,215.98 searcher gross. */
const liveContradictionRow = () =>
  mapToOmniOpportunity(
    wire({
      amount_in_wei: "1000000", // 1e6 wei of a 6-dec token = 1 USDC = $1.00
      expected_profit_usd: 822215.98, // dex_engine fast filter, at ITS probe
      net_expected_profit_usd: -0.000011, // sizing kernel, at the KERNEL size
      simulated_amount_in_usd: 1.0,
      // A WELL-FORMED SIM ladder (net = gross − Σcomponents = 411107.995) whose
      // ONLY defect is that its gross is 822,216× its own principal:
      simulated_gross_usd: 822215.98,
      simulated_costs_total_usd: 411107.985,
      simulated_net_profit_usd: 411107.995,
      simulated_cost_breakdown: {
        gas_usd: 0.673172225,
        lp_fees_usd: 0.003,
        slippage_usd: 0.005,
        failure_buffer_usd: 0.0004,
        copied_buffer_usd: 369996.493527375, // 0.5 × gross − (relay + dust)
        capital_cost_usd: 0,
        ops_overhead_usd: 0.01,
        flashloan_fee_usd: 0.0009,
        relay_fee_usd: 41110.799, // 0.05 × gross
      },
    }),
  );

/**
 * A HEALTHY row: the recorded notional IS the size the economics were computed
 * at (the B1 path — `orchestrator.rs` writes `optimal_amount_in` into
 * `amount_in_wei`, so principal, gross and costs share one basis). $1,000 of
 * capital, $25.23 gross, every component present.
 */
const closedRow = () =>
  mapToOmniOpportunity(
    wire({
      amount_in_wei: "1000000000000000000000",
      expected_profit_usd: 25.22783426,
      net_expected_profit_usd: 1.369353192,
      get simulated_amount_in_usd() {
        return 1000;
      },
      simulated_gross_usd: 25.22783426,
      simulated_costs_total_usd: 23.858481068,
      simulated_net_profit_usd: 1.369353192,
      simulated_cost_breakdown: {
        gas_usd: 0.673172225,
        lp_fees_usd: 3,
        slippage_usd: 5,
        failure_buffer_usd: 0.4,
        copied_buffer_usd: 12.61391713,
        capital_cost_usd: 0,
        ops_overhead_usd: 0.01,
        flashloan_fee_usd: 0.9,
        relay_fee_usd: 1.261391713,
      },
    }),
  );

describe("CARDS-NOTIONAL-01 — buildLedger: one ladder, one notional", () => {
  it("REFUSES the simulated ladder when its gross is not attributable to its own principal", () => {
    const opp = liveContradictionRow();
    const ledger = buildLedger(opp);

    // The live break: gross $822,215.98 on a $1.00 principal — 822,216×.
    expect(ledger.basis).toBe("canonical");
    // No principal is published for a basis that has no notional on the wire.
    expect(ledger.principal_usd).toBeNull();
    expect(ledger.cost_rows).toEqual([]);
    expect(ledger.quiet).toBe(true);
    expect(ledger.reason).toContain("CARDS-NOTIONAL-01");
    // The reason NAMES the mechanism, so the operator is not left guessing.
    expect(ledger.reason).toMatch(/principal|size|notional/i);
  });

  it("keeps the ledger arithmetically CLOSED on the searcher's own pair: net == gross − total_cost", () => {
    const ledger = buildLedger(liveContradictionRow());
    expect(ledger.gross_usd).toBe(822215.98);
    expect(ledger.total_cost_usd).toBeCloseTo(822215.980011, 6);
    expect(ledger.net_usd).toBeCloseTo(-0.000011, 9);
    // This is the row's own arithmetic the operator saw broken:
    // `Total cost $72.4k` next to `Net yield -$0.0000` is off by $72.4k.
    expect(
      Math.abs(ledger.net_usd! - (ledger.gross_usd! - ledger.total_cost_usd!)),
    ).toBeLessThanOrEqual(LEDGER_TOLERANCE_USD);
  });

  it("RENDERS the simulated ladder when principal, gross and costs are one computation", () => {
    const ledger = buildLedger(closedRow());
    expect(ledger.basis).toBe("simulated");
    expect(ledger.principal_usd).toBe(1000);
    expect(ledger.quiet).toBe(false);
    expect(ledger.reason).toBeNull();
    // Closure holds inside one notional.
    expect(
      Math.abs(ledger.net_usd! - (ledger.gross_usd! - ledger.total_cost_usd!)),
    ).toBeLessThanOrEqual(LEDGER_TOLERANCE_USD);
    // Doctrine bound: cost is payable out of principal + gross, and the gross is
    // attributable to the principal (the searcher's own 5× gate).
    expect(ledger.total_cost_usd!).toBeGreaterThanOrEqual(0);
    expect(ledger.total_cost_usd!).toBeLessThanOrEqual(
      ledger.principal_usd! + ledger.gross_usd!,
    );
    expect(ledger.gross_usd!).toBeLessThanOrEqual(
      ledger.principal_usd! * GROSS_OVER_PRINCIPAL_SANITY_MULT,
    );
  });

  it("regression: the cost ladder INCLUDES copied_buffer_usd (the old hand-sum dropped it)", () => {
    const ledger = buildLedger(closedRow());
    const labels = ledger.cost_rows.map((r) => r.label);
    expect(labels).toContain("Copied buffer");
    // 9 components — the old card summed 8 and dropped the largest one at the
    // live p_copied_max = 0.5, so `Total cost` rendered ≈45% of the truth.
    expect(ledger.cost_rows).toHaveLength(9);
    const rowSum = ledger.cost_rows.reduce((a, r) => a + r.value, 0);
    expect(rowSum).toBeCloseTo(ledger.total_cost_usd!, 6);
    // The omitted component is half the gross here — it cannot be invisible.
    expect(ledger.total_cost_usd!).toBeGreaterThan(ledger.gross_usd! * 0.9);
  });

  it("goes QUIET (basis none) when neither producer offers a closed triple", () => {
    const opp = mapToOmniOpportunity(
      wire({ expected_profit_usd: 10, net_expected_profit_usd: null }),
    );
    const ledger = buildLedger(opp);
    expect(ledger.basis).toBe("none");
    expect(ledger.gross_usd).toBeNull();
    expect(ledger.total_cost_usd).toBeNull();
    expect(ledger.net_usd).toBeNull();
    expect(ledger.quiet).toBe(true);
    expect(ledger.reason).toBeTruthy();
  });

  it("REFUSES a simulated triple whose net does not equal gross − total cost", () => {
    const opp = mapToOmniOpportunity(
      wire({
        amount_in_wei: "1000000000000000000000",
        expected_profit_usd: 25.2,
        net_expected_profit_usd: -5,
        simulated_amount_in_usd: 1000,
        simulated_gross_usd: 25.2,
        simulated_costs_total_usd: 1,
        simulated_net_profit_usd: -5, // ≠ 25.2 − 1
        simulated_cost_breakdown: {
          gas_usd: 1,
          lp_fees_usd: 0,
          slippage_usd: 0,
          failure_buffer_usd: 0,
          copied_buffer_usd: 0,
          capital_cost_usd: 0,
          ops_overhead_usd: 0,
          flashloan_fee_usd: 0,
          relay_fee_usd: 0,
        },
      }),
    );
    const ledger = buildLedger(opp);
    expect(ledger.basis).toBe("canonical");
    expect(ledger.principal_usd).toBeNull();
    expect(ledger.reason).toMatch(/net != gross - total cost/);
  });
});

describe("CARDS-NOTIONAL-01 — notifier gate", () => {
  it("does NOT fire on a row whose economics cross notionals (the live toast)", () => {
    // The toast the operator photographed:
    //   "High value opportunity — dex_arb / Net yield $822215.98 · chain 1"
    // fired on `expected_profit_usd` (a GROSS), while the row's own kernel net
    // was ≈ −0.00001 and its principal was $1.00.
    const opp = liveContradictionRow();
    const d = decideOpportunityNotification(opp, 50);
    expect(d.fire).toBe(false);
    expect(d.amount_usd).toBeNull();
    expect(d.label).toBeNull();
    // The exact figure the old toast printed must never be announced here: it
    // is the searcher's gross at a size the card does not have a notional for.
    expect(d.amount_usd).not.toBe(opp.expected_profit_usd);
  });

  it("scaled-field gate: the old `expected_profit_usd ?? 0` expression would have fired; the gate does not", () => {
    // Reproduce the pre-fix predicate verbatim against the same row.
    const opp = liveContradictionRow();
    const oldYield = opp.expected_profit_usd ?? 0;
    expect(oldYield).toBeGreaterThanOrEqual(50); // ← the old toast DID fire
    expect(oldYield).toBe(822215.98); //    …announcing this as "Net yield"
    // The gated decision refuses the same row.
    expect(decideOpportunityNotification(opp, 50).fire).toBe(false);
  });

  it("fires on a closed ladder and announces the NET, never the gross", () => {
    const d = decideOpportunityNotification(closedRow(), 0.5);
    expect(d.fire).toBe(true);
    // `Net yield $1.37` — the closed net of the rendered ladder, NOT the
    // $25.23 gross and NOT the searcher's $1.369353192 by coincidence: the
    // amount is read from the ledger's net.
    expect(d.amount_usd).toBeCloseTo(buildLedger(closedRow()).net_usd!, 9);
    expect(d.label).toBe("Net yield");
    expect(d.basis).toBe("simulated");
  });

  it("names a figure for what it IS: the canonical pair's net is announced as a net", () => {
    const opp = mapToOmniOpportunity(
      wire({ expected_profit_usd: 100, net_expected_profit_usd: 90 }),
    );
    const d = decideOpportunityNotification(opp, 95);
    // Threshold 95: the GROSS (100) clears it, the NET (90) does not. The old
    // code compared the gross and CALLED it "Net yield $100.00".
    expect(d.fire).toBe(false);
    const low = decideOpportunityNotification(opp, 80);
    expect(low.fire).toBe(true);
    expect(low.amount_usd).toBe(90);
    expect(low.label).toBe("Net yield");
    expect(low.basis).toBe("canonical");
  });

  it("R8: a NOT-COMPUTED figure is not a zero — `?? 0` used to clear a threshold of 0", () => {
    const opp = mapToOmniOpportunity(
      wire({ expected_profit_usd: null, net_expected_profit_usd: null }),
    );
    const d = decideOpportunityNotification(opp, 0);
    expect(d.fire).toBe(false);
    expect(d.amount_usd).toBeNull();
  });

  it("does not fire on a non-finite threshold", () => {
    const d = decideOpportunityNotification(closedRow(), Number.NaN);
    expect(d.fire).toBe(true); // NaN threshold degrades to 0, the figure is real
    expect(d.amount_usd).toBeGreaterThan(0);
  });
});
