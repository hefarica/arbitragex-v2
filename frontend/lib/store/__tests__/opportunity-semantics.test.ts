// frontend/lib/store/__tests__/opportunity-semantics.test.ts
//
// FE-0031 (§30): validateOpportunitySemantics quarantine is DECIDABLE (fires
// only on wire-grade evidence), VISIBLE (never hides the row), and honest
// (R8 nulls are "not computed", never violations). Every one of the 7 codes
// must fire on its malformed input; every honest shape must stay clean.
import { describe, it, expect } from "vitest";
import {
  mapToOmniOpportunity,
  validateOpportunitySemantics,
} from "@/lib/store/types";

const wire = (over: Record<string, unknown>) => ({
  id: "id",
  chain_id: 1,
  strategy_kind: "dex_arb",
  detected_at: "2026-08-11T00:00:00Z",
  status: "detected",
  trace_id: "t",
  dex_a: "uniswap-v2",
  dex_b: "sushiswap",
  token_in: "0xa",
  token_out: "0xb",
  block_number: 123,
  ...over,
});

// Well-formed persisted topology: closed 2-hop cycle a→b→a.
const closedCycleRm = {
  token_addresses: ["0xa", "0xb", "0xa"],
  dex_adapters: ["uniswap-v2", "sushiswap"],
  pool_addresses: ["0xpool1", "0xpool2"],
};

const mk = (over: Record<string, unknown>) => mapToOmniOpportunity(wire(over));

describe("validateOpportunitySemantics (§30)", () => {
  it("clean wire-grade row: zero violations", () => {
    expect(mk({ route_metadata: closedCycleRm }).semantic_violations).toEqual([]);
  });

  it("honest nulls are not violations: no profits, no simulated fields", () => {
    const clean = mk({
      route_metadata: closedCycleRm,
      expected_profit_usd: null,
      net_expected_profit_usd: null,
      simulated_net_profit_usd: null,
    });
    expect(clean.semantic_violations).toEqual([]);
  });

  it("missing_strategy_id fires when the wire omits strategy_kind", () => {
    expect(mk({ strategy_kind: undefined }).semantic_violations).toContain(
      "missing_strategy_id",
    );
  });

  it("no_route_identity fires only when there is NO basis at all", () => {
    // No route_metadata AND no legacy dex pair → no legs → no identity.
    expect(
      mk({ route_metadata: null, dex_a: null, dex_b: null }).semantic_violations,
    ).toContain("no_route_identity");
    // A legacy dex_a/dex_b row HAS a synthetic basis (§29) → not a violation.
    expect(
      mk({ route_metadata: null }).semantic_violations,
    ).not.toContain("no_route_identity");
  });

  it("hop_incoherent fires when tokens !== hops + 1", () => {
    expect(
      mk({
        route_metadata: {
          token_addresses: ["0xa", "0xb"], // 2 tokens for 2 hops — incoherent
          dex_adapters: ["uniswap-v2", "sushiswap"],
          pool_addresses: ["0xpool1", "0xpool2"],
        },
      }).semantic_violations,
    ).toContain("hop_incoherent");
  });

  it("legs_incoherent fires on a single-chain route that does not close", () => {
    expect(
      mk({
        route_metadata: {
          token_addresses: ["0xa", "0xb", "0xc"], // c ≠ a
          dex_adapters: ["uniswap-v2", "sushiswap"],
          pool_addresses: ["0xpool1", "0xpool2"],
        },
      }).semantic_violations,
    ).toContain("legs_incoherent");
  });

  it("cross-chain rows skip the cycle-closure check", () => {
    const opp = mk({
      chain_id_out: 42161,
      route_metadata: {
        token_addresses: ["0xa", "0xb", "0xc"],
        dex_adapters: ["uniswap-v2", "sushiswap"],
        pool_addresses: ["0xpool1", "0xpool2"],
      },
    });
    expect(opp.semantic_violations).not.toContain("legs_incoherent");
    // Direct call agrees (same viewmodel, same verdict).
    expect(validateOpportunitySemantics(opp)).not.toContain("legs_incoherent");
  });

  it("missing_block fires when the wire omits block_number", () => {
    expect(mk({ block_number: undefined }).semantic_violations).toContain(
      "missing_block",
    );
  });

  it("profit_not_numeric fires on present-but-non-finite profit", () => {
    // Wire vector: string payload → Number("oops") = NaN → present, not finite.
    expect(
      mk({ net_expected_profit_usd: "oops" }).semantic_violations,
    ).toContain("profit_not_numeric");
    // Explicit null = not computed (R8) → clean.
    expect(
      mk({ net_expected_profit_usd: null }).semantic_violations,
    ).not.toContain("profit_not_numeric");
  });

  it("degenerate_pair fires on a self-swap LEG, never on row-level in==out", () => {
    // Closed cycle at row level (token_in === token_out is the cycle
    // definition — first-leg-in / last-leg-out) with a real 2-hop topology:
    // NOT degenerate. This is the false-positive guard.
    const cycle = mk({
      token_in: "0xa",
      token_out: "0xa",
      route_metadata: closedCycleRm,
    });
    expect(cycle.semantic_violations).toEqual([]);

    // A persisted leg X→X is a provable no-op swap → degenerate.
    expect(
      mk({
        token_in: "0xa",
        token_out: "0xa",
        route_metadata: {
          token_addresses: ["0xa", "0xa"],
          dex_adapters: ["uniswap-v2"],
          pool_addresses: ["0xpool1"],
        },
      }).semantic_violations,
    ).toContain("degenerate_pair");
  });

  it("mapper integration: malformed payload accumulates every applicable code", () => {
    const opp = mk({
      strategy_kind: undefined,
      block_number: undefined,
      route_metadata: null,
      dex_a: null,
      dex_b: null,
      net_expected_profit_usd: "oops",
    });
    expect(opp.semantic_violations).toEqual(
      expect.arrayContaining([
        "missing_strategy_id",
        "no_route_identity",
        "missing_block",
        "profit_not_numeric",
      ]),
    );
  });

  it("validateOpportunitySemantics is deterministic: same viewmodel, same verdict", () => {
    const opp = mk({ strategy_kind: undefined });
    expect(validateOpportunitySemantics(opp)).toEqual(opp.semantic_violations);
  });
});
