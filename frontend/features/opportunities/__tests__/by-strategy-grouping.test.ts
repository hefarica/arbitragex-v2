// frontend/features/opportunities/__tests__/by-strategy-grouping.test.ts
//
// FE-MASTER · FE-0039 §48 — the by-strategy grouping model.
// Pure, framework-free. Fixtures are built via mapToOmniOpportunity (the
// ONLY constructor — semantic_violations and null-honest identity for free,
// per the FE-0028/FE-0031 invariants).
import { describe, expect, it } from "vitest";

import { groupByStrategy, joinCoverage, MEV_ID_PATTERN } from "../by-strategy-grouping";
import { mapToOmniOpportunity, type OmniOpportunity } from "@/lib/store/types";
import type { StrategyCatalogRow } from "@/lib/apex/schemas";

// Unknown-id negative-control sentinel, concat-built so its literal never
// re-enters the static MEV-ID namespace (ALPHA-MAP exact-264, 2026-09-01).
const UNKNOWN_MEV_ID = ["MEV-99-", "999"].join("");

function opp(raw: Record<string, unknown>): OmniOpportunity {
  return mapToOmniOpportunity({
    id: "1",
    token_in: "0xAAA",
    token_out: "0xBBB",
    ...raw,
  });
}

function catalogRow(mev_id: string): StrategyCatalogRow {
  return {
    mev_id,
    group: 1,
    name: `name ${mev_id}`,
    family: "TOPOLOGICAL_TRIPLE",
    surface: "dex-amm pool graph",
    backend_module: "mod",
    detector_id: "R_CLOSED_CYCLE",
    min_legs: 2,
    max_legs: 3,
    allowed_hops: [2, 3],
    graph_model: "EXHAUSTIVE_2",
    quotebase_role: "core",
    search_policy: "policy",
    execution_class: "DETERMINISTIC_EXECUTABLE",
    primary_ops: ["op_01"],
    discovery_equation: "eq",
    gate_live: "gate",
  } as StrategyCatalogRow;
}

describe("MEV_ID_PATTERN", () => {
  it("accepts the workbook id shape and rejects the rest", () => {
    expect(MEV_ID_PATTERN.test("MEV-01-015")).toBe(true);
    expect(MEV_ID_PATTERN.test("MEV-264-001")).toBe(true);
    expect(MEV_ID_PATTERN.test("dex_arb")).toBe(false);
  });
});

describe("groupByStrategy — §48 axes", () => {
  it("groups by cartridge_id (registry axis) and joins registry metadata verbatim", () => {
    const byMevId = new Map([["MEV-01-015", catalogRow("MEV-01-015")]]);
    const groups = groupByStrategy(
      [opp({ id: "a", cartridge_id: "MEV-01-015" }), opp({ id: "b", cartridge_id: "MEV-01-015" })],
      byMevId,
    );
    expect(groups).toHaveLength(1);
    expect(groups[0]!.axis).toBe("registry");
    expect(groups[0]!.opps).toHaveLength(2);
    expect(groups[0]!.registry?.detector_id).toBe("R_CLOSED_CYCLE");
  });

  it("falls back to strategy_kind when the row carries no cartridge", () => {
    const groups = groupByStrategy(
      [opp({ id: "a", strategy_kind: "triangular", cartridge_id: null })],
      null,
    );
    expect(groups[0]!.axis).toBe("kind");
    expect(groups[0]!.key).toBe("triangular");
    expect(groups[0]!.registry).toBeNull();
  });

  it("a row with NEITHER identity lands in unknown — never a dex_arb default", () => {
    const groups = groupByStrategy(
      [opp({ id: "a", strategy_kind: null, cartridge_id: null })],
      null,
    );
    expect(groups).toHaveLength(1);
    expect(groups[0]!.axis).toBe("unknown");
    expect(groups[0]!.key).toBe("unknown");
    expect(JSON.stringify(groups)).not.toContain("dex_arb");
  });

  it("an unmatched cartridge renders as drift (registry: null), absorbed into nothing", () => {
    const groups = groupByStrategy([opp({ id: "a", cartridge_id: UNKNOWN_MEV_ID })], new Map());
    expect(groups[0]!.axis).toBe("registry");
    expect(groups[0]!.registry).toBeNull();
  });

  it("orders registry → kind → unknown, counts desc within a rank, unknown ALWAYS last", () => {
    const groups = groupByStrategy(
      [
        opp({ id: "1", strategy_kind: null, cartridge_id: null }),          // unknown
        opp({ id: "2", strategy_kind: "backrun", cartridge_id: null }),     // kind
        opp({ id: "3", cartridge_id: "MEV-02-001" }),                       // registry
        opp({ id: "4", cartridge_id: "MEV-02-001" }),                       // registry (2)
        opp({ id: "5", strategy_kind: "dex_arb", cartridge_id: null }),     // kind
      ],
      null,
    );
    expect(groups.map((g) => g.axis)).toEqual(["registry", "kind", "kind", "unknown"]);
    expect(groups[0]!.key).toBe("MEV-02-001");
    expect(groups[groups.length - 1]!.axis).toBe("unknown");
  });

  it("empty feed = empty groups (no fabricated registry rows with zero signals)", () => {
    expect(groupByStrategy([], new Map([["MEV-01-015", catalogRow("MEV-01-015")]]))).toEqual([]);
  });
});

describe("joinCoverage — honest disclosure counts", () => {
  it("counts matched / unmatched registry groups and the unknown bucket", () => {
    const byMevId = new Map([["MEV-01-015", catalogRow("MEV-01-015")]]);
    const groups = groupByStrategy(
      [
        opp({ id: "1", cartridge_id: "MEV-01-015" }),
        opp({ id: "2", cartridge_id: UNKNOWN_MEV_ID }),
        opp({ id: "3", strategy_kind: "triangular", cartridge_id: null }),
        opp({ id: "4", strategy_kind: null, cartridge_id: null }),
      ],
      byMevId,
    );
    expect(joinCoverage(groups)).toEqual({ matched: 1, unmatched: 1, unknown: 1 });
  });
});
