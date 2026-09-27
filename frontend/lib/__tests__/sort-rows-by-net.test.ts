// ORDER-01 (operator order 2026-09-27): the grid is ordered by PROFIT
// descending — the row that makes money is the FIRST card, whatever order the
// WebSocket stream or a snapshot batch delivered. Pure and deterministic: two
// identical polls must not shuffle the cards.
import { describe, expect, it } from "vitest";
import { mapToOmniOpportunity, type OmniOpportunity } from "@/lib/store/types";
import { netOf, sortRowsByNetDesc } from "@/lib/opportunity-ledger";

const row = (id: string, over: Record<string, unknown>) =>
  mapToOmniOpportunity({
    id,
    chain_id: 1,
    strategy_kind: "dex_arb",
    detected_at: "2026-09-27T15:56:56Z",
    trace_id: "t",
    ...over,
  });

const economics = (net: number | null) => ({
  computation_status: "computed",
  amount_in_usd: 1000,
  gross_profit_usd: net == null ? null : net + 0.68,
  total_cost_usd: 0.68,
  net_profit_usd: net,
  gas_usd: 0.67,
  other_costs_usd: 0.01,
});

describe("sortRowsByNetDesc", () => {
  const ganancia = row("ganancia", {
    status: "rejected",
    rejection_reason: "gas_floor_breach:own_capital",
    detected_at: "2026-09-27T15:56:56Z",
    economics: economics(1.771171976897318),
  });
  const perdida = row("perdida", {
    status: "rejected",
    rejection_reason: "non_positive_profit",
    detected_at: "2026-09-27T16:10:00Z",
    economics: economics(-68.08559029916856),
  });
  const sinDatos = row("sinDatos", {
    status: "rejected",
    rejection_reason: "v3_quote_unavailable",
    detected_at: "2026-09-27T16:11:00Z",
    economics: { computation_status: "error", error_reason: "v3_quote_unavailable" },
  });
  const viable = row("viable", {
    status: "detected",
    detected_at: "2026-09-27T16:05:00Z",
    economics: economics(19.32),
  });

  it("puts the profitable row first, whatever the input order was", () => {
    const ordered = sortRowsByNetDesc([perdida, sinDatos, viable, ganancia]);
    expect(ordered.map((r) => r.id)).toEqual(["viable", "ganancia", "perdida", "sinDatos"]);
  });

  it("a row with NO figure goes last (absence is not a zero, not a gain)", () => {
    const ordered = sortRowsByNetDesc([sinDatos, perdida]);
    expect(ordered.map((r) => r.id)).toEqual(["perdida", "sinDatos"]);
    expect(netOf(sinDatos)).toBeNull();
  });

  it("ties break on the most recent detection", () => {
    const a = row("a", { status: "detected", detected_at: "2026-09-27T15:00:00Z", economics: economics(5) });
    const b = row("b", { status: "detected", detected_at: "2026-09-27T16:00:00Z", economics: economics(5) });
    expect(sortRowsByNetDesc([a, b]).map((r) => r.id)).toEqual(["b", "a"]);
  });

  it("is deterministic and does NOT mutate the input", () => {
    const input = [perdida, sinDatos, viable, ganancia];
    const snapshot = input.map((r) => r.id);
    const first = sortRowsByNetDesc(input).map((r) => r.id);
    const second = sortRowsByNetDesc(input).map((r) => r.id);
    expect(first).toEqual(second); // no shuffling between two identical polls
    expect(input.map((r) => r.id)).toEqual(snapshot); // input untouched
  });

  it("an empty list stays empty", () => {
    expect(sortRowsByNetDesc([])).toEqual([]);
  });
});
