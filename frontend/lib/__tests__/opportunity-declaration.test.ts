// frontend/lib/__tests__/opportunity-declaration.test.ts
//
// ECON-DECLARE-01 (2026-09-27) — the gate for the operator's complaint:
// *"hay muchos valores que no se ven, no están declarados."*
//
// Two CI-enforceable gates, per the mission:
//   (i)  a wire key served by the API with NO declared provenance FAILS.
//   (ii) a card cell that renders `—` while the wire carries a value FAILS.
//
// Gate (i) is anchored on the REAL served key set, measured from the live feed
// (GET /api/opportunities/live, 33 items, 2026-09-27T02:06Z) rather than on a
// hand-written fixture — a registry that only covers its own fixture proves
// nothing about production.
import React from "react";
import { describe, it, expect } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";

import {
  DECLARATION_DASH,
  OPPORTUNITY_FIELD_REGISTRY,
  auditCellDeclarations,
  auditDashOverValue,
  auditServedWireKeys,
  auditUndeclaredEconomicFigures,
  basisFromWire,
  declarationOf,
  figureBasis,
  formatWeiNotional,
  readEconomicsDeclaration,
} from "../opportunity-declaration";
import { OpportunitySummaryGrid } from "@/components/opportunities/OpportunitySummaryGrid";
import { mapToOmniOpportunity, type OmniOpportunity } from "@/lib/store/types";

// ── The REAL served key set (measured, 33 live items, 2026-09-27T02:06Z) ─────
const LIVE_SERVED_KEYS = [
  "amount_in_wei",
  "block_number",
  "bridge",
  "bridge_fee_usd",
  "cartridge_id",
  "chain_base_token_symbol",
  "chain_id",
  "chain_id_out",
  "chains_used",
  "confirmations",
  "detected_at",
  "detector_id",
  "dex_a",
  "dex_b",
  "dexes_used",
  "expected_profit_usd",
  "first_seen_at",
  "id",
  "last_seen_at",
  "leg_symbols",
  "net_expected_profit_usd",
  "pair_symbol",
  "paper_status",
  "pipeline_latency_ms",
  "rejection_reason",
  "risk_score",
  "roi_pct",
  "route_group_key",
  "route_metadata",
  "simulated_amount_in_usd",
  "simulated_at",
  "simulated_cost_breakdown",
  "simulated_costs_total_usd",
  "simulated_gross_usd",
  "simulated_net_profit_usd",
  "simulated_notes",
  "simulated_roi_pct",
  "simulated_target",
  "status",
  "strategy_kind",
  "token_in",
  "token_in_info",
  "token_out",
  "token_out_info",
  "token_prices_usd",
  "trace_id",
] as const;

// The REAL route_metadata key set (measured in the same sample). No row declared
// `economics_amount_in_wei` / `economics_basis` — that absence is the finding.
const LIVE_ROUTE_METADATA_KEYS = [
  "route_metadata.decimals",
  "route_metadata.dex_adapters",
  "route_metadata.pool_addresses",
  "route_metadata.token_addresses",
] as const;

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

const rm2hop = {
  dex_adapters: ["uniswap-v2", "sushiswap"],
  token_addresses: ["0xa", "0xb", "0xa"],
  pool_addresses: ["0xpool1", "0xpool2"],
};

/** Read the rendered text of every `opp-cell-<label>` from the SSR markup. */
function cellTextFromMarkup(html: string): Record<string, string> {
  const out: Record<string, string> = {};
  const re = /data-testid="opp-cell-([^"]+)">([^<]*)</g;
  for (const m of html.matchAll(re)) out[m[1]!] = m[2]!;
  return out;
}

function renderGrid(opp: OmniOpportunity): string {
  return renderToStaticMarkup(React.createElement(OpportunitySummaryGrid, { opp }));
}

// =============================================================================
// GATE (i) — a wire key with no declared provenance
// =============================================================================

describe("ECON-DECLARE-01 gate (i) — every served wire key is DECLARED", () => {
  it("the REAL live wire key set has ZERO undeclared keys", () => {
    const findings = auditServedWireKeys([
      ...LIVE_SERVED_KEYS,
      ...LIVE_ROUTE_METADATA_KEYS,
    ]);
    // A failure here prints the exact key whose provenance is missing.
    expect(findings.map((f) => f.field)).toEqual([]);
  });

  it("registry declares the two ECON-DECLARE-01 keys a producer writes", () => {
    // The declaration the producer stamps must itself be a declared wire key,
    // otherwise the closure would re-open the very gap it closes.
    expect(auditServedWireKeys(["route_metadata.economics_amount_in_wei"])).toEqual([]);
    expect(auditServedWireKeys(["route_metadata.economics_basis"])).toEqual([]);
  });

  it("an undeclared key FAILS with a machine reason (the gate bites)", () => {
    const findings = auditServedWireKeys(["totally_new_field"]);
    expect(findings).toHaveLength(1);
    expect(findings[0]!.field).toBe("totally_new_field");
    expect(findings[0]!.reason).toContain("NO entry in");
    expect(findings[0]!.reason).toContain("no está declarado");
  });

  it("an open map is covered by its declared ancestor but a sibling is not", () => {
    // token_prices_usd is an open map: its per-symbol members are covered.
    expect(auditServedWireKeys(["token_prices_usd.USDC"])).toEqual([]);
    // …but a NEW top-level map is not covered by anything.
    expect(auditServedWireKeys(["brand_new_map.USDC"])).toHaveLength(1);
  });

  it("every registry entry is self-consistent (no half-declared field)", () => {
    for (const e of OPPORTUNITY_FIELD_REGISTRY) {
      expect(e.field.trim(), "field path must be non-empty").not.toBe("");
      expect(e.label.trim(), `${e.field}: label must be non-empty`).not.toBe("");
      expect(e.meaning.trim(), `${e.field}: meaning must be non-empty`).not.toBe("");
      expect(e.surface.length, `${e.field}: must declare a surface`).toBeGreaterThan(0);
    }
  });

  it("no registry field is declared twice", () => {
    const seen = new Set<string>();
    for (const e of OPPORTUNITY_FIELD_REGISTRY) {
      expect(seen.has(e.field), `duplicate registry entry: ${e.field}`).toBe(false);
      seen.add(e.field);
    }
  });
});

describe("ECON-DECLARE-01 gate (i, runtime) — economic figures declare their basis", () => {
  it("a row whose producer declared the basis reports NO undeclared figure", () => {
    const opp = mapToOmniOpportunity(
      wire({
        route_metadata: {
          ...rm2hop,
          economics_amount_in_wei: "1000000000000000000",
          economics_basis: { gross: "probe", net: "kernel", amount: "intent" },
        },
        expected_profit_usd: 12.5,
        net_expected_profit_usd: -50,
      }),
    );
    // roi_pct/risk_score have no vocabulary word on the wire yet, so they are
    // reported only when present — here they are absent (R8: absence ≠ defect).
    expect(auditUndeclaredEconomicFigures(opp)).toEqual([]);
  });

  it("a row carrying figures with NO declaration is REPORTED (never silently tolerated)", () => {
    const opp = mapToOmniOpportunity(
      wire({
        route_metadata: rm2hop,
        expected_profit_usd: 12.5,
        net_expected_profit_usd: -50,
        roi_pct: 0.5,
        risk_score: 0.2,
      }),
    );
    const undeclared = auditUndeclaredEconomicFigures(opp);
    expect(undeclared).toContain("expected_profit_usd");
    expect(undeclared).toContain("net_expected_profit_usd");
    // The two fields the live feed shows 0/33 on have no basis vocabulary at all.
    expect(undeclared).toContain("roi_pct");
    expect(undeclared).toContain("risk_score");
  });
});

// =============================================================================
// GATE (ii) — a cell must never render — over a value the wire carries
// =============================================================================

describe("ECON-DECLARE-01 gate (ii) — no dash over a present wire value", () => {
  it("the LIVE cross-notional row (gross $1,313,772.38 vs principal 4.353e-06) shows BOTH", () => {
    // This is the measured shape that made CARDS-NOTIONAL-01 hide the notional:
    // the gross is 3.0e11 × the principal, so the pair is NOT one arithmetic.
    // Hiding the principal is what the operator reported as "no se ven". The
    // declaration is what lets both be shown, each labelled with its basis.
    const opp = mapToOmniOpportunity(
      wire({
        route_metadata: {
          ...rm2hop,
          economics_amount_in_wei: "1000000000000000000",
          economics_basis: { gross: "probe", net: "kernel", amount: "intent" },
        },
        expected_profit_usd: 1313772.3818,
        net_expected_profit_usd: -50,
        simulated_amount_in_usd: 0.000004353,
      }),
    );
    const html = renderGrid(opp);
    const cells = cellTextFromMarkup(html);

    // The gate itself must pass on this row.
    expect(auditDashOverValue(opp, cells)).toEqual([]);
    // And the principal is VISIBLE (not a dash) with its basis declared.
    expect(cells["in"]).not.toBe(DECLARATION_DASH);
    expect(html).toContain("$0.00");
    // The gross is visible too — both figures, neither hidden.
    expect(cells["Gross"]).not.toBe(DECLARATION_DASH);
    expect(html).toContain("$1.31M");
    // The mismatch is STATED, not hidden.
    expect(html).toContain("CARDS-NOTIONAL-01");
    expect(html).toContain("se midieron en tamaños distintos");
  });

  it("EVERY live row shape passes the gate (the measured 18/33 + 15/33 split)", () => {
    // gross present, net absent (8 rows) | gross absent, net present (5 rows)
    // | both present (10 rows) | neither (10 rows) — all four must be gate-clean.
    const shapes: Array<Record<string, unknown>> = [
      { expected_profit_usd: 4.15, simulated_amount_in_usd: 9.84 },
      { net_expected_profit_usd: -0.5, simulated_amount_in_usd: 9.84 },
      { expected_profit_usd: 0.0002, net_expected_profit_usd: -50, simulated_amount_in_usd: 2699.72 },
      {},
    ];
    for (const shape of shapes) {
      const opp = mapToOmniOpportunity(wire({ route_metadata: rm2hop, ...shape }));
      const cells = cellTextFromMarkup(renderGrid(opp));
      expect(auditDashOverValue(opp, cells), `shape ${JSON.stringify(shape)}`).toEqual([]);
    }
  });

  it("the gate BITES: a dash over a present value FAILS with a machine reason", () => {
    const opp = mapToOmniOpportunity(
      wire({
        route_metadata: rm2hop,
        expected_profit_usd: 52.13863753,
        detector_id: "det-1",
        pipeline_latency_ms: 7,
      }),
    );
    // Simulate a regression that hides the gross behind a dash.
    const findings = auditDashOverValue(opp, { Gross: DECLARATION_DASH });
    expect(findings.map((f) => f.field)).toContain("expected_profit_usd");
    expect(findings[0]!.reason).toContain("a computed value is hidden");

    // An EMPTY cell is the same defect as a dash.
    const empty = auditDashOverValue(opp, { detector: "" });
    expect(empty.map((f) => f.field)).toContain("detector_id");
  });

  it("the gate is honest: a field ABSENT from the wire may legitimately render —", () => {
    // Nothing is hidden when there is nothing to show (R8). A gate that failed
    // here would be a gate that forces fabrication.
    const opp = mapToOmniOpportunity(wire({ route_metadata: rm2hop }));
    const cells = cellTextFromMarkup(renderGrid(opp));
    expect(auditDashOverValue(opp, cells)).toEqual([]);
    expect(cells["Gross"]).toBe(DECLARATION_DASH);
    expect(cells["Risk"]).toBe(DECLARATION_DASH);
  });

  it("every cell the grid renders is DECLARED in CELL_FIELD_BINDING", () => {
    const opp = mapToOmniOpportunity(
      wire({
        route_metadata: rm2hop,
        expected_profit_usd: 1,
        simulated_amount_in_usd: 1,
      }),
    );
    const labels = Object.keys(cellTextFromMarkup(renderGrid(opp)));
    expect(labels.length).toBeGreaterThan(0);
    expect(auditCellDeclarations(labels)).toEqual([]);
  });
});

// =============================================================================
// The declaration reader — absence is a state, never a guess
// =============================================================================

describe("ECON-DECLARE-01 — reading the producer's declaration", () => {
  it("returns an all-absent declaration for a legacy row (no coercion)", () => {
    const d = readEconomicsDeclaration(rm2hop);
    expect(d.declared).toBe(false);
    expect(d.notionalWei).toBeNull();
    expect(d.gross).toBeNull();
    expect(d.net).toBeNull();
    expect(d.amount).toBeNull();
  });

  it("reads the declared words and the exact wei verbatim", () => {
    const d = readEconomicsDeclaration({
      ...rm2hop,
      economics_amount_in_wei: "1000000",
      economics_basis: { gross: "probe", net: "kernel", amount: "stamped" },
    });
    expect(d.declared).toBe(true);
    expect(d.notionalWei).toBe("1000000");
    expect(d.gross).toBe("probe");
    expect(d.net).toBe("kernel");
    expect(d.amount).toBe("stamped_probe");
  });

  it("refuses to guess: an unknown basis word reads null, not a nearest match", () => {
    expect(basisFromWire("probable")).toBeNull();
    expect(basisFromWire("estimado")).toBeNull();
    expect(basisFromWire(null)).toBeNull();
    const d = readEconomicsDeclaration({
      ...rm2hop,
      economics_basis: { gross: "probably_probe" },
    });
    expect(d.gross).toBeNull();
    expect(d.declared).toBe(false);
  });

  it("a malformed notional is dropped by the mapper, never repaired", () => {
    for (const bad of ["", "0x10", "-5", "1e18", " 12 "]) {
      const opp = mapToOmniOpportunity(
        wire({
          route_metadata: { ...rm2hop, economics_amount_in_wei: bad },
        }),
      );
      expect(
        declarationOf(opp).notionalWei,
        `"${bad}" must not be accepted as a notional`,
      ).toBeNull();
    }
  });

  it("figureBasis labels a declared figure and marks an undeclared one", () => {
    const declared = mapToOmniOpportunity(
      wire({
        route_metadata: {
          ...rm2hop,
          economics_amount_in_wei: "1000000",
          economics_basis: { gross: "probe", net: "kernel", amount: "intent" },
        },
        expected_profit_usd: 1,
        net_expected_profit_usd: 1,
      }),
    );
    expect(figureBasis(declared, "gross")).toBe("probe");
    expect(figureBasis(declared, "net")).toBe("kernel");
    expect(figureBasis(declared, "amount")).toBe("intent");
    // roi/risk have no vocabulary on the wire — always honestly undeclared.
    expect(figureBasis(declared, "roi")).toBe("undeclared");
    expect(figureBasis(declared, "risk")).toBe("undeclared");

    const legacy = mapToOmniOpportunity(
      wire({ route_metadata: rm2hop, expected_profit_usd: 1 }),
    );
    expect(figureBasis(legacy, "gross")).toBe("undeclared");
  });

  it("the grid renders the basis label beside every shown figure", () => {
    const opp = mapToOmniOpportunity(
      wire({
        route_metadata: {
          ...rm2hop,
          economics_amount_in_wei: "1000000000000000000",
          economics_basis: { gross: "probe", net: "kernel", amount: "intent" },
        },
        expected_profit_usd: 10.5,
        net_expected_profit_usd: -50,
      }),
    );
    const html = renderGrid(opp);
    expect(html).toContain('data-basis="probe"');
    expect(html).toContain("@probe");
    expect(html).toContain("@kernel");
    // A row with the declaration shows the declared notional in the `in` title.
    expect(html).toContain("economics_amount_in_wei=1000000000000000000");
  });

  it("an undeclared row says so in the cell title instead of inventing a basis", () => {
    const opp = mapToOmniOpportunity(
      wire({ route_metadata: rm2hop, expected_profit_usd: 10.5 }),
    );
    const html = renderGrid(opp);
    expect(html).toContain("@undeclared");
    expect(html).toContain("no trae basis declarada por su productor");
  });
});

describe("ECON-DECLARE-01 — formatWeiNotional never unit-guesses", () => {
  it("collapses powers of ten and leaves everything else verbatim", () => {
    expect(formatWeiNotional("1000000000000000000")).toBe("1e18");
    expect(formatWeiNotional("1000000")).toBe("1e6");
    expect(formatWeiNotional("4353")).toBe("4353");
    expect(formatWeiNotional(null)).toBe(DECLARATION_DASH);
    expect(formatWeiNotional("")).toBe(DECLARATION_DASH);
  });

  it("a non-power-of-ten 18-decimal amount is NOT divided by 1e18", () => {
    // The HOPS-UNITS-01 defect was reading a wei amount with a guessed decimals.
    // This formatter refuses that: the exact value stays visible.
    expect(formatWeiNotional("1234567890123456789")).toBe("123456…e18");
  });
});
