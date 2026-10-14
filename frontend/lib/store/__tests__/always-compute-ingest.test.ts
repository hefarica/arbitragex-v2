// ALWAYS-COMPUTE (operator mandate 2026-09-27) — ingest-boundary tolerance.
//
// Two contracts:
//   1. The api-server serializes the non-finite required floor as the STRING
//      "Infinity" (JSON cannot carry Infinity; `JSON.stringify(Infinity)` is
//      the ambiguous null that made the field vanish on 41/41 live rows). The
//      store mapper must be TOLERANT: sentinel → Number.POSITIVE_INFINITY +
//      `required_is_infinite: true`. The ViewModel type stays `number | null`,
//      so every existing consumer keeps compiling.
//   2. The `economics` computation object passes through the mapper with
//      numeric-string tolerance (deliverable #4 hardening): non-empty numeric
//      strings become numbers; junk degrades to null — never a coerced 0.
//
// Part 1 is deliberately written against ONLY pre-existing public symbols
// (`mapToOmniOpportunity`) so it FAILS AT RUNTIME on the pre-patch tree
// (where the raw string passes through untouched) — the pre-patch proof gate.
import { describe, expect, it } from "vitest";
import { mapToOmniOpportunity, parseEconomics } from "../types";

const wire = (over: Record<string, unknown>) => ({
  id: "opp-ac",
  chain_id: 1,
  strategy_kind: "dex_arb",
  detected_at: "2026-09-27T00:00:00Z",
  status: "rejected",
  trace_id: "trace-ac",
  dex_a: "uniswap_v2",
  token_in: "0xa",
  token_out: "0xb",
  block_number: 1,
  ...over,
});

describe("ALWAYS-COMPUTE — required_amount_in_usd Infinity tolerance", () => {
  it('normalizes the "Infinity" sentinel to Number.POSITIVE_INFINITY + companion flag', () => {
    const mapped = mapToOmniOpportunity(
      wire({
        simulated_target: {
          target_net_usd: 25,
          target_roi_pct: null,
          target_source: "simulation_tab",
          binding_floor: "net-per-usd-nonpositive",
          estimation_basis: "observed-gross",
          required_amount_in_usd: "Infinity",
          required_is_infinite: true,
          cap_amount_in_usd: 1000,
          suggested_amount_in_usd: 0,
          suggested_net_usd: -50,
          suggested_roi_pct: 0,
          meets_target_at_cap: false,
          notes: ["net-per-usd-nonpositive"],
        },
      }),
    );
    const t = mapped.simulated_target!;
    // The sentinel is a COMPUTED verdict, not an absence: the value survives
    // as a number the guards understand (!isFinite) and the boolean mirrors it.
    expect(t.required_amount_in_usd).toBe(Number.POSITIVE_INFINITY);
    expect(t.required_is_infinite).toBe(true);
    expect(t.binding_floor).toBe("net-per-usd-nonpositive");
  });

  it("keeps finite required floors as plain numbers (no regression)", () => {
    const mapped = mapToOmniOpportunity(
      wire({
        simulated_target: {
          target_net_usd: 25,
          target_roi_pct: null,
          target_source: "simulation_tab",
          binding_floor: "usd-floor",
          estimation_basis: "observed-gross",
          required_amount_in_usd: 1175.42,
          cap_amount_in_usd: 100000,
          suggested_amount_in_usd: 1175.42,
          suggested_net_usd: 25.01,
          suggested_roi_pct: 2.13,
          meets_target_at_cap: true,
          notes: [],
        },
      }),
    );
    expect(mapped.simulated_target!.required_amount_in_usd).toBe(1175.42);
    expect(mapped.simulated_target!.required_is_infinite).toBe(false);
  });

  it("tolerates numeric strings on the target block (boundary hardening)", () => {
    const mapped = mapToOmniOpportunity(
      wire({
        simulated_target: {
          target_net_usd: "25",
          target_roi_pct: null,
          target_source: "simulation_tab",
          binding_floor: "usd-floor",
          estimation_basis: "observed-gross",
          required_amount_in_usd: "1175.42",
          cap_amount_in_usd: "100000",
          suggested_amount_in_usd: 0,
          suggested_net_usd: 0,
          suggested_roi_pct: 0,
          meets_target_at_cap: false,
          notes: [],
        },
      }),
    );
    expect(mapped.simulated_target!.target_net_usd).toBe(25);
    expect(mapped.simulated_target!.required_amount_in_usd).toBe(1175.42);
    expect(mapped.simulated_target!.cap_amount_in_usd).toBe(100000);
  });
});

describe("ALWAYS-COMPUTE — economics object ingest", () => {
  it("passes the complete object through the mapper with numeric-string tolerance", () => {
    const mapped = mapToOmniOpportunity(
      wire({
        expected_profit_usd: 12.5,
        net_expected_profit_usd: -50,
        economics: {
          computation_status: "computed",
          error_reason: null,
          amount_in_wei: "1000000000000000000",
          amount_out_wei: null,
          amount_in_usd: "2350", // numeric string → number
          amount_out_usd: 2362.5,
          gross_profit_usd: 12.5,
          gas_usd: "0.18",
          dex_fees_usd: null,
          flash_fee_usd: 2.12,
          bribe_usd: 0,
          slippage_usd: null,
          other_costs_usd: 0.01,
          total_cost_usd: 2.31,
          net_profit_usd: -50,
          roi_pct: -2.13,
          target_net_usd: 25,
          target_delta_usd: -75,
          meets_target: false,
          quote_block: 123,
          simulation_block: null,
          legs: [
            { token_in: "0xa", token_out: "0xb", amount_in_wei: "1", amount_out_wei: "2" },
          ],
          not_computed_reasons: { dex_fees_usd: "included_in_amount_out_post_fee" },
        },
      }),
    );
    const e = mapped.economics!;
    expect(e.computation_status).toBe("computed");
    expect(e.amount_in_usd).toBe(2350);
    expect(e.gas_usd).toBe(0.18);
    expect(e.net_profit_usd).toBe(-50);
    expect(e.target_delta_usd).toBe(-75);
    expect(e.legs).toHaveLength(1);
  });

  it("degrades junk to null and rejects malformed status (R8, never a coerced 0)", () => {
    expect(parseEconomics(null)).toBeNull();
    expect(parseEconomics("nope" as never)).toBeNull();
    expect(parseEconomics({ computation_status: "bogus" })).toBeNull();
    const e = parseEconomics({
      computation_status: "partial",
      gross_profit_usd: "abc",
      net_profit_usd: "  ",
      roi_pct: "1e999",
      amount_in_wei: "0x1",
    })!;
    expect(e.gross_profit_usd).toBeNull();
    expect(e.net_profit_usd).toBeNull();
    expect(e.roi_pct).toBeNull();
    expect(e.amount_in_wei).toBeNull();
  });
});
