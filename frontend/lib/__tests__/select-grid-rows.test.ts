// Gate D (operator order 2026-09-27, verbatim: "APLICA LA D") + SHOW-REJECTED-01
//
//   WHERE (rejection_reason IS NULL
//          OR (computation_status = 'computed' AND net_amount > 0))
//
// Contract enforced here:
//   OFF (default) → grid = viable rows + rejected rows with computed net > 0.
//   ON            → grid = every row that CARRIES data (the pre-D view).
//   A row with no computed figure is never painted on either setting; it stays
//   declared (counted) and is never filled with an assumption.
import { describe, expect, it } from "vitest";
import { mapToOmniOpportunity, type OmniOpportunity } from "@/lib/store/types";
import {
  hasComputedFigures,
  inDefaultScope,
  isRealLiveEconomicCard,
  netOf,
  selectGridRows,
} from "@/lib/opportunity-ledger";

const base = (id: string, over: Record<string, unknown>) =>
  mapToOmniOpportunity({
    id,
    chain_id: 1,
    strategy_kind: "dex_arb",
    detected_at: "2026-09-27T15:56:56Z",
    trace_id: "t",
    ...over,
  });

/** The canonical production shape: rejected by the gas floor, PRICED, net > 0. */
const win = base("win", {
  status: "rejected",
  rejection_reason: "gas_floor_breach:own_capital",
  economics: {
    computation_status: "computed",
    amount_in_usd: 999.9981579926539,
    gross_profit_usd: 2.4538765518973205,
    total_cost_usd: 0.682704575,
    net_profit_usd: 1.771171976897318,
    gas_usd: 0.672704575,
    other_costs_usd: 0.01,
    flash_fee_usd: 0,
    bribe_usd: 0,
    dex_fees_usd: null,
    slippage_usd: null,
  },
});

/** Rejected, priced, net ≤ 0 (spread zero: gross 0, net = −gas) — gate D hides it. */
const neg = base("neg", {
  status: "rejected",
  rejection_reason: "spread_zero_equilibrium",
  net_expected_profit_usd: -0.68305955,
  economics: {
    computation_status: "computed",
    amount_in_usd: 2692.658138079,
    gross_profit_usd: 0,
    total_cost_usd: 0.68305955,
    net_profit_usd: -0.68305955,
    gas_usd: 0.67305955,
    other_costs_usd: 0.01,
    flash_fee_usd: 0,
    bribe_usd: 0,
    dex_fees_usd: null,
    slippage_usd: null,
  },
});

/** Never rejected — always in scope, whatever its net. */
const viable = base("viable", {
  status: "detected",
  economics: {
    computation_status: "computed",
    amount_in_usd: 1000,
    gross_profit_usd: 20,
    total_cost_usd: 0.68,
    net_profit_usd: 19.32,
    gas_usd: 0.67,
    other_costs_usd: 0.01,
    flash_fee_usd: 0,
    bribe_usd: 0,
    dex_fees_usd: null,
    slippage_usd: null,
  },
});

/** The producer never priced it: no figure at all. */
const err = base("err", {
  status: "rejected",
  rejection_reason: "v3_quote_unavailable",
  economics: { computation_status: "error", error_reason: "v3_quote_unavailable" },
});

const ids = (rows: OmniOpportunity[]) => rows.map((r) => r.id);
const rows = [win, neg, viable, err];

describe("gate D — scope of the live grid", () => {
  it("classifies each real production shape", () => {
    expect(isRealLiveEconomicCard(win)).toBe(true);
    expect(isRealLiveEconomicCard(neg)).toBe(true);
    expect(isRealLiveEconomicCard(viable)).toBe(true);
    expect(isRealLiveEconomicCard(err)).toBe(false);

    expect(inDefaultScope(win)).toBe(true); //  rejected but priced POSITIVE
    expect(inDefaultScope(neg)).toBe(false); // rejected and priced NEGATIVE
    expect(inDefaultScope(viable)).toBe(true); // never rejected
    expect(inDefaultScope(err)).toBe(false); // never priced

    expect(hasComputedFigures(err)).toBe(false);
    expect(netOf(neg)).toBeCloseTo(-0.68305955, 8);
  });

  it("OFF (default): grid is the D scope — viable + rejected with net > 0", () => {
    const { grid, inScope, hiddenByGate, noData } = selectGridRows(rows, false);
    expect(ids(grid)).toEqual(["win", "viable"]);
    expect(ids(inScope)).toEqual(["win", "viable"]);
    expect(ids(hiddenByGate)).toEqual(["neg"]); // priced, but not a gain
    expect(ids(noData)).toEqual(["err"]); // no figure → declared only
    expect(grid.every(inDefaultScope)).toBe(true);
  });

  it("ON: every row that carries data is painted — and the data-less row still is not", () => {
    const { grid, noData } = selectGridRows(rows, true);
    expect(ids(grid)).toEqual(["win", "neg", "viable"]);
    expect(ids(noData)).toEqual(["err"]);
    // no duplication when a card is also outside the D scope
    expect(new Set(ids(grid)).size).toBe(ids(grid).length);
  });

  it("never re-stamps a row: the grid carries the same objects", () => {
    const { grid } = selectGridRows(rows, true);
    expect(grid[0]).toBe(win);
    expect(grid[1]).toBe(neg);
    expect(grid[2]).toBe(viable);
  });

  it("keeps input order inside each bucket", () => {
    const { grid } = selectGridRows([viable, err, neg, win], false);
    expect(ids(grid)).toEqual(["viable", "win"]);
  });

  it("an empty snapshot stays empty on both settings", () => {
    expect(selectGridRows([], false).grid).toEqual([]);
    expect(selectGridRows([], true).grid).toEqual([]);
  });
});
