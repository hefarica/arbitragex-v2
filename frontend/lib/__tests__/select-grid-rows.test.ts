// SHOW-REJECTED-01 (operator order 2026-09-27: "agregar un toggle 'Mostrar
// rechazadas'") — coverage for the grid-scope selector that backs the toggle.
//
// Contract:
//   OFF (default) → the grid is EXACTLY the real/live economic card set:
//                   previous behaviour, same rows, same order.
//   ON            → the diagnostics (rejected / not-computed detections) are
//                   painted as well, AFTER the economic cards, with their own
//                   reason — never a substituted figure, never a rewritten row.
import { describe, expect, it } from "vitest";
import { mapToOmniOpportunity, type OmniOpportunity } from "@/lib/store/types";
import { isRealLiveEconomicCard, selectGridRows } from "@/lib/opportunity-ledger";

/** Closed real/live ladder: gas + ops = total_cost, net = gross − total. */
const computedRow = (id: string) =>
  mapToOmniOpportunity({
    id,
    chain_id: 1,
    strategy_kind: "dex_arb",
    detected_at: "2026-09-27T15:56:56Z",
    trace_id: "t",
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

/** Source failure: the producer never priced the route. */
const errorRow = (id: string) =>
  mapToOmniOpportunity({
    id,
    chain_id: 1,
    strategy_kind: "dex_arb",
    detected_at: "2026-09-27T16:10:00Z",
    trace_id: "t",
    status: "rejected",
    rejection_reason: "v3_quote_unavailable",
    economics: { computation_status: "error", error_reason: "v3_quote_unavailable" },
  });

/** Priced only in part: still not a real/live card. */
const partialRow = (id: string) =>
  mapToOmniOpportunity({
    id,
    chain_id: 1,
    strategy_kind: "dex_arb",
    detected_at: "2026-09-27T16:11:00Z",
    trace_id: "t",
    status: "rejected",
    rejection_reason: "single_pool_no_spread",
    economics: { computation_status: "partial" },
  });

const ids = (rows: OmniOpportunity[]) => rows.map((r) => r.id);

describe("selectGridRows — toggle 'Mostrar rechazadas'", () => {
  const a = computedRow("a-computed");
  const b = errorRow("b-error");
  const c = partialRow("c-partial");
  const rows = [a, b, c];

  it("OFF (default): grid === real/live cards only — previous behaviour", () => {
    const { grid, economic, diagnostics } = selectGridRows(rows, false);
    expect(ids(grid)).toEqual(["a-computed"]);
    expect(ids(economic)).toEqual(["a-computed"]);
    expect(ids(diagnostics)).toEqual(["b-error", "c-partial"]);
    // every painted row is a real/live economic card — the old invariant
    expect(grid.every(isRealLiveEconomicCard)).toBe(true);
  });

  it("ON: the diagnostics are painted after the cards, same row objects", () => {
    const { grid } = selectGridRows(rows, true);
    expect(ids(grid)).toEqual(["a-computed", "b-error", "c-partial"]);
    // identity, not a copy: nothing is re-derived or re-stamped by the toggle
    expect(grid[1]).toBe(b);
    expect(grid[2]).toBe(c);
    // the diagnostics are precisely the rows that are NOT real/live cards
    expect(grid.filter(isRealLiveEconomicCard).map((r) => r.id)).toEqual(["a-computed"]);
  });

  it("keeps the input order of the economic cards", () => {
    const d = computedRow("d-computed");
    const { grid, economic } = selectGridRows([d, b, a, c], true);
    expect(ids(economic)).toEqual(["d-computed", "a-computed"]);
    expect(ids(grid)).toEqual(["d-computed", "a-computed", "b-error", "c-partial"]);
  });

  it("an empty snapshot stays empty on both settings", () => {
    expect(selectGridRows([], false).grid).toEqual([]);
    expect(selectGridRows([], true).grid).toEqual([]);
  });
});
