// frontend/components/__tests__/CardCellPaint.gate.test.tsx
//
// CARDS-PAINT-GATE-01 (2026-09-27) — the gate the operator asked for: a cell
// that HAS a wire value must paint a number, and a cell must never paint a
// `—` (nor a false `$0.00`) while the row carries the figure.
//
// Why it exists (measured, not theorised). Audit of
// `OpportunityTradeCard.tsx` + `OpportunitySummaryGrid.tsx` against the REAL
// wire (`GET http://127.0.0.1:8787/api/opportunities/live`, read-only tunnel),
// 323 unique rows over 45 polls at 2026-09-27T02:2xZ, rendering every row with
// `renderToStaticMarkup` and comparing, cell by cell, {field present in wire?}
// against {cell painted?}:
//
//   26 rows  `net_expected_profit_usd` ≈ -1.2e-5, `expected_profit_usd` null
//            → grid `Net` painted `-$0.00` (a computed loss shown as exactly
//              zero) and the card's headline + ladder painted `—`.
//    5 rows  `simulated_amount_in_usd` present ($0.0000044 … $0.99987154) with a
//            searcher gross measured at another size (710273.79750992 vs
//            0.99987154) → grid `in` painted `—`: a figure the forward-sim HAD
//              computed vanished.
//    3 rows  (55-row span) a net present with no canonical gross and an
//            unpayable SIM triple (`buildLedger` → basis `none`) → headline and
//            ladder net painted `—` while the grid painted the same figure.
//
// The cure keeps CARDS-NOTIONAL-01 intact — no ladder ever mixes two notionals —
// and is asserted here in both directions: the invariant (every wire-present
// value paints) AND its negation (the pre-patch paint map is flagged), so the
// gate cannot pass vacuously.
import React from "react";
import { describe, it, expect, vi } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";

vi.mock("@/components/ui/skeleton", () => ({
  Skeleton: (props: React.ComponentProps<"div">) =>
    React.createElement("div", { "data-slot": "skeleton", ...props }),
}));

import {
  OpportunityTradeCard,
  opportunityTradeCardPropsEqual,
  type OpportunityTradeCardProps,
} from "../OpportunityTradeCard";
import { OpportunitySummaryGrid } from "@/components/opportunities/OpportunitySummaryGrid";
import { mapToOmniOpportunity, type OmniOpportunity } from "@/lib/store/types";

// ─── Rendering helpers (same extractors the card's own suites use) ────────────

const DASH = "—";

const wire = (over: Record<string, unknown>) => ({
  id: "opp-1",
  chain_id: 1,
  strategy_kind: "dex_arb",
  detected_at: "2026-09-27T02:20:00Z",
  status: "rejected",
  trace_id: "trace-1",
  dex_a: "uniswap-v2",
  dex_b: "sushiswap",
  token_in: "0xa",
  token_out: "0xb",
  block_number: 123,
  ...over,
});

type Mapped = ReturnType<typeof mapToOmniOpportunity>;

function cardHtml(opp: Mapped, isMounted = false): string {
  return renderToStaticMarkup(
    React.createElement(OpportunityTradeCard, {
      opp,
      now: 0,
      isMounted,
      simLoading: false,
      onExecute: () => {},
      onInspect: () => {},
    }),
  );
}

/** Grid cells in document order — label → value (`summaryCells` order). */
function gridCells(html: string): Array<{ label: string; value: string }> {
  const out: Array<{ label: string; value: string }> = [];
  const re =
    /<div class="text-\[9px\] uppercase tracking-wide text-muted-foreground">([^<]*)<\/div><div class="truncate">([^<]*)<\/div>/g;
  let m: RegExpExecArray | null;
  while ((m = re.exec(html)) !== null) out.push({ label: m[1]!, value: m[2]! });
  return out;
}

/** Ladder rows (LedgerRow) — the repo's own proven extractor. */
function ledgerCells(html: string): Array<{ label: string; value: string }> {
  const out: Array<{ label: string; value: string }> = [];
  const re =
    /class="min-w-0 flex-1 truncate">([^<]*)<\/span>(?:<span class="min-w-0 max-w-\[45%\][^>]*>\([^<]*\)<\/span>)?<\/span><span[^>]*class="shrink-0 whitespace-nowrap tabular-nums[^"]*"[^>]*>([^<]*)<\/span>/g;
  let m: RegExpExecArray | null;
  while ((m = re.exec(html)) !== null) out.push({ label: m[1]!, value: m[2]! });
  return out;
}

/** The executive "Net yield" headline cell. */
function headlineNet(html: string): string | null {
  const m = html.match(
    /tracking-wide text-muted-foreground">Net yield<\/div><div class="flex items-center gap-1"><span[^>]*>([^<]*)<\/span>/,
  );
  return m ? m[1]! : null;
}

const at = (cells: Array<{ label: string; value: string }>, label: string): string | null => {
  const exact = cells.find((c) => c.label === label);
  if (exact) return exact.value;
  const pref = cells.find((c) => c.label.startsWith(label));
  return pref ? pref.value : null;
};

/** Everything the audit reads off ONE rendered card. */
interface Paint {
  grid: Record<string, string>;
  headline: string | null;
  ladder: Record<string, string>;
}

function paintOf(opp: Mapped, isMounted = false): Paint {
  const gridHtml = renderToStaticMarkup(React.createElement(OpportunitySummaryGrid, { opp }));
  const card = cardHtml(opp, isMounted);
  const grid: Record<string, string> = {};
  for (const c of gridCells(gridHtml)) grid[c.label] = c.value;
  const ladder: Record<string, string> = {};
  for (const c of ledgerCells(card)) ladder[c.label] = c.value;
  return { grid, headline: headlineNet(card), ladder };
}

// ─── The audited contract: wire field → the cells that must paint it ──────────

const isNum = (v: unknown): v is number => typeof v === "number" && Number.isFinite(v);
const FALSE_ZERO = /^-?\$0\.0+$/;

/** A cell that hides a present figure, or fabricates a zero, is a violation. */
export function paintViolations(raw: Record<string, unknown>, p: Paint): string[] {
  const v: string[] = [];
  const dash = (s: string | null | undefined) => s == null || s.trim() === DASH;

  const numFields: Array<[string, string]> = [
    ["Net", "net_expected_profit_usd"],
    ["Sim", "simulated_net_profit_usd"],
    ["Gross", "expected_profit_usd"],
    ["in", "simulated_amount_in_usd"],
    ["Risk", "risk_score"],
  ];
  for (const [cell, field] of numFields) {
    if (isNum(raw[field]) && dash(p.grid[cell])) {
      v.push(`${cell}:WIRE-HAS-VALUE-BUT-DASH(${field})`);
    }
    if (isNum(raw[field]) && raw[field] !== 0 && FALSE_ZERO.test((p.grid[cell] ?? "").trim())) {
      v.push(`${cell}:NONZERO-PAINTED-AS-ZERO(${field})`);
    }
  }
  // Text/derived cells
  if (raw["detector_id"] != null && dash(p.grid["detector"])) v.push("detector:WIRE-HAS-VALUE-BUT-DASH");
  if (isNum(raw["pipeline_latency_ms"]) && dash(p.grid["latencia"])) v.push("latencia:WIRE-HAS-VALUE-BUT-DASH");
  if (raw["strategy_kind"] != null && dash(p.grid["strategy"])) v.push("strategy:WIRE-HAS-VALUE-BUT-DASH");
  if (raw["dex_a"] != null && dash(p.grid["ruta"])) v.push("ruta:WIRE-HAS-VALUE-BUT-DASH");
  if (Array.isArray((raw["route_metadata"] as Record<string, unknown> | undefined)?.["dex_adapters"]) &&
      dash(p.grid["hops"])) {
    v.push("hops:WIRE-HAS-VALUE-BUT-DASH");
  }

  // The executive net: the wire's own precedence (canonical spine, else the
  // TS forward-sim net) — the SAME rule the grid's `Net` cell applies.
  const netPresent = isNum(raw["net_expected_profit_usd"]) || isNum(raw["simulated_net_profit_usd"]);
  if (netPresent && dash(p.headline)) v.push("headlineNet:WIRE-HAS-VALUE-BUT-DASH");
  if (netPresent && dash(at2(p.ladder, "Net yield"))) v.push("ladderNet:WIRE-HAS-VALUE-BUT-DASH");
  for (const [name, value] of [
    ["headlineNet", p.headline],
    ["ladderNet", at2(p.ladder, "Net yield")],
  ] as const) {
    const n = isNum(raw["net_expected_profit_usd"]) ? raw["net_expected_profit_usd"] : raw["simulated_net_profit_usd"];
    if (isNum(n) && n !== 0 && FALSE_ZERO.test((value ?? "").trim())) {
      v.push(`${name}:NONZERO-PAINTED-AS-ZERO`);
    }
  }
  return v;
}

const at2 = (map: Record<string, string>, prefix: string): string | null => {
  if (map[prefix] != null) return map[prefix]!;
  const k = Object.keys(map).find((key) => key.startsWith(prefix));
  return k ? map[k]! : null;
};

// ─── Fixtures: real wire rows (verbatim values captured from the live feed) ───

/**
 * The 26-row shape of the 45-poll capture: the sizing kernel computed a net, the
 * wire carries NO canonical gross. Live ids 9185b4fd / 2b50dcec, net verbatim.
 */
const NET_ONLY_WIRE = {
  expected_profit_usd: null,
  net_expected_profit_usd: -0.000012,
  rejection_reason: "non_positive_profit",
};

/**
 * Live id 23d521b0 / e980d28b (measured 2026-09-27T02:0xZ): the SIM's own
 * notional beside a searcher gross 710,273× larger. The pair is NOT one
 * arithmetic (the searcher's own 5× bound refuses it) — and the notional must be
 * shown anyway, marked, while the ladder keeps publishing no principal.
 */
const NOTIONAL_NOT_ATTRIBUTABLE_WIRE = {
  amount_in_wei: "1000000000000000000",
  expected_profit_usd: 710273.79750992,
  net_expected_profit_usd: -0.000005,
  roi_pct: null,
  simulated_amount_in_usd: 0.99987154,
  simulated_gross_usd: 710273.79750992,
  simulated_costs_total_usd: 390651.2828588113,
  simulated_net_profit_usd: 319622.51465110865,
  simulated_roi_pct: 31966357.863442,
};

/**
 * The SIM triple CLOSED at its own notional and ATTRIBUTABLE to it — the shape
 * the card must keep rendering unmarked (regression half of the gate).
 */
const SIM_CLOSED_ATTRIBUTABLE_WIRE = {
  amount_in_wei: "1000000000000000000000",
  expected_profit_usd: 25.22783426,
  net_expected_profit_usd: 1.369353192,
  simulated_amount_in_usd: 1000,
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
};

/**
 * The observed "basis none" shape (live 5da89486 / f10ad33c / 1941483c at
 * 02:0xZ): canonical gross 0 with no canonical net, and a SIM triple that is not
 * payable out of its own principal — so `buildLedger` refuses every basis while
 * the wire DOES carry a computed net.
 */
const SIM_NOT_PAYABLE_WIRE = {
  amount_in_wei: "1000000",
  expected_profit_usd: 0,
  net_expected_profit_usd: null,
  simulated_amount_in_usd: 1.0,
  simulated_gross_usd: 0,
  simulated_costs_total_usd: 1.19,
  simulated_net_profit_usd: -1.19,
  simulated_roi_pct: -119,
};

/**
 * CONSTRUCTED (not captured): a 4-hop cycle carrying the per-leg kernel ledger
 * plus the non-attributable notional — the combination the operator's screenshot
 * showed (HOPS 4 with every summary cell dashed). `decimals` is the map the real
 * wire row carries (`route_metadata.decimals`, one entry per participant).
 */
const DEEP_4HOP_WIRE = {
  ...NOTIONAL_NOT_ATTRIBUTABLE_WIRE,
  route_metadata: {
    dex_adapters: ["UniswapV2", "UniswapV3", "UniswapV3", "SushiSwap"],
    token_addresses: ["0xa", "0xb", "0xc", "0xd", "0xa"],
    pool_addresses: ["0xp1", "0xp2", "0xp3", "0xp4"],
    decimals: { "0xa": 18, "0xb": 18, "0xc": 6, "0xd": 18 },
    leg_amounts_in: ["0", "1069822000000000000", "1069822", "1000000"],
    leg_amounts_out: ["1069822000000000000", "1069822", "1000000", "999998"],
    leg_zero_for_one: [true, false, true, false],
  },
};

/**
 * Live shape (audit, 2026-09-27): a searcher gross with NO net at all and no SIM
 * triple — nothing closes, so the ladder paints no arithmetic. The figure is NOT
 * hidden: the grid publishes it. This case pins the boundary between "hidden"
 * and "deliberately quiet", so a future change cannot silence the grid either.
 */
const GROSS_ONLY_WIRE = {
  expected_profit_usd: 818328.08,
  net_expected_profit_usd: null,
  rejection_reason: "non_positive_profit",
};

const FIXTURES: Array<{ id: string; raw: Record<string, unknown> }> = [
  { id: "net-only (26 live rows)", raw: NET_ONLY_WIRE },
  { id: "gross-only, no closed ladder (live audit)", raw: GROSS_ONLY_WIRE },
  { id: "notional not attributable (live 23d521b0)", raw: NOTIONAL_NOT_ATTRIBUTABLE_WIRE },
  { id: "sim closed + attributable (regression)", raw: SIM_CLOSED_ATTRIBUTABLE_WIRE },
  { id: "sim not payable → basis none (live 5da89486)", raw: SIM_NOT_PAYABLE_WIRE },
  { id: "deep 4-hop + leg ledger (constructed)", raw: DEEP_4HOP_WIRE },
];

describe("CARDS-PAINT-GATE-01 — a wire value is always painted", () => {
  it("no cell hides a present value, on any audited wire shape", () => {
    const report: string[] = [];
    for (const f of FIXTURES) {
      const raw = wire(f.raw);
      const opp = mapToOmniOpportunity(raw as never);
      const p = paintOf(opp);
      const violations = paintViolations(raw, p);
      report.push(
        `${f.id}: ${violations.length === 0 ? "OK" : violations.join(", ")}` +
          `\n    grid: ${Object.entries(p.grid).map(([k, val]) => `${k}=${val}`).join(" | ")}` +
          `\n    headline=${p.headline} ladderNet=${at2(p.ladder, "Net yield")}`,
      );
      expect(violations, `${f.id} → ${violations.join(", ")}`).toEqual([]);
    }
    console.log(`\n===== CARDS-PAINT-GATE-01 =====\n${report.join("\n")}`);
  });

  it("the net-only row paints the kernel's figure in ALL THREE surfaces, never a false zero", () => {
    const raw = wire(NET_ONLY_WIRE);
    const p = paintOf(mapToOmniOpportunity(raw as never));
    // One value, one rendering — the grid, the headline and the ladder agree.
    expect(p.grid["Net"]).toBe("-$0.000012");
    expect(p.headline).toBe("-$0.000012");
    expect(at2(p.ladder, "Net yield")).toBe("-$0.000012");
    // R8: null is still the honest dash — the fix never fabricates.
    expect(p.grid["Gross"]).toBe(DASH);
    expect(p.grid["Sim"]).toBe(DASH);
    expect(p.grid["in"]).toBe(DASH);
  });

  it("a gross with NO closed ladder is still published — by the grid, not the ladder", () => {
    const raw = wire(GROSS_ONLY_WIRE);
    const p = paintOf(mapToOmniOpportunity(raw as never));
    // NOT hidden: the wire-grade cell shows the searcher's own gross…
    expect(p.grid["Gross"]).toBe("$818328.08");
    // …while the capital path stays quiet (nothing closes), with the reason in
    // every dashed cell's `title` and in the basis note (CARDS-QUIET-01).
    expect(at2(p.ladder, "Gross out")).toBe(DASH);
    expect(at2(p.ladder, "Net yield")).toBe(DASH);
    expect(at2(p.ladder, "Total cost")).toBe(DASH);
    // …and the grid's Net cell has nothing to show either (the wire has no net).
    expect(p.grid["Net"]).toBe(DASH);
    expect(p.headline).toBe(DASH);
  });

  it("CARDS-NOTIONAL-01 is intact: a marked notional in the grid, NO principal in the ladder", () => {
    const raw = wire(NOTIONAL_NOT_ATTRIBUTABLE_WIRE);
    const p = paintOf(mapToOmniOpportunity(raw as never));
    // The grid publishes the forward-sim notional with its producer mark…
    expect(p.grid["in"]).toBe("~$1.00"); // la grilla imprime 2 decimales sobre el medio centavo
    // …and the capital path still refuses to publish a principal beside a gross
    // measured at another size (the 710,273× pair the operator photographed).
    expect(at2(p.ladder, "Flash loan in")).toBe(DASH);
    expect(at2(p.ladder, "Repay")).toBe(DASH);
    // The searcher's own closed pair is what the ladder shows — one arithmetic.
    expect(at2(p.ladder, "Gross out")).toBe("$710.3k");
    expect(at2(p.ladder, "Net yield")).toBe("-$0.000005");
  });

  it("a CLOSED, attributable ladder renders the full capital path exactly as before", () => {
    const raw = wire(SIM_CLOSED_ATTRIBUTABLE_WIRE);
    const p = paintOf(mapToOmniOpportunity(raw as never));
    expect(p.grid["in"]).toBe("$1000.00"); // unmarked: the gross belongs to it
    expect(at2(p.ladder, "Flash loan in")).toBe("$1.0k"); // el ladder compacta sobre 1e3; la grilla mantiene el exacto
    expect(at2(p.ladder, "Net yield")).toBe("$1.37"); // el label dice "(SIM)"; el "~" vive en el titular
    expect(p.headline).toBe("~+$1.37"); // "~" marca el productor forward-sim
    // …and the cost ladder of that SAME basis is painted (not dashed).
    expect(at2(p.ladder, "Copied buffer")).not.toBe(DASH);
    expect(at2(p.ladder, "Total cost")).not.toBe(DASH);
  });

  it("per-hop ledger cells paint the kernel's exact amounts (deep rows)", () => {
    const raw = wire(DEEP_4HOP_WIRE);
    const opp = mapToOmniOpportunity(raw as never);
    const card = cardHtml(opp);
    expect(card).toContain("Hop 1/4");
    expect(card).toContain("Hop 4/4");
    // Exact wei from `leg_amounts_in/out`, unit-scaled — never a dashes-only row.
    expect(card).toContain("1.069822");
    expect(card).toContain("ciclo ");
  });

  it("the paint is not gated by isMounted (R1 SSR path paints the same cells)", () => {
    for (const f of FIXTURES) {
      const raw = wire(f.raw);
      const opp = mapToOmniOpportunity(raw as never);
      const ssr = paintOf(opp, false);
      const mounted = paintOf(opp, true);
      expect(mounted.grid).toEqual(ssr.grid);
      expect(mounted.headline).toBe(ssr.headline);
      expect(at2(mounted.ladder, "Net yield")).toBe(at2(ssr.ladder, "Net yield"));
      expect(at2(mounted.ladder, "Flash loan in")).toBe(at2(ssr.ladder, "Flash loan in"));
    }
  });

  // ── NON-VACUITY ────────────────────────────────────────────────────────────
  //
  // The gate is only a gate if it FAILS on the paint map the pre-patch
  // components produced. These maps are the OBSERVED pre-patch output of the
  // live-render harness for the same two fixtures (grid `Net` = `-$0.00`,
  // headline `—`, ladder net `—`, grid `in` = `—`), so this is the real
  // regression being re-detected, not a hand-made strawman.
  it("NON-VACUITY: flags the PRE-PATCH paint map (dash where the wire has the value)", () => {
    const netOnly = paintViolations(wire(NET_ONLY_WIRE), {
      grid: { Net: "-$0.00", Gross: DASH, Sim: DASH, in: DASH, Risk: DASH, latencia: "41ms" },
      headline: DASH,
      ladder: { "Net yield": DASH },
    });
    expect(netOnly).toContain("headlineNet:WIRE-HAS-VALUE-BUT-DASH");
    expect(netOnly).toContain("ladderNet:WIRE-HAS-VALUE-BUT-DASH");
    expect(netOnly).toContain("Net:NONZERO-PAINTED-AS-ZERO(net_expected_profit_usd)");

    const notional = paintViolations(wire(NOTIONAL_NOT_ATTRIBUTABLE_WIRE), {
      grid: { Net: "-$0.00", Gross: "$710273.80", Sim: "~$319622.51", in: DASH, Risk: DASH },
      headline: "-$0.00",
      ladder: { "Net yield": "-$0.0000", "Flash loan in (TLS) · WETH": DASH },
    });
    expect(notional).toContain("in:WIRE-HAS-VALUE-BUT-DASH(simulated_amount_in_usd)");
    expect(notional).toContain("Net:NONZERO-PAINTED-AS-ZERO(net_expected_profit_usd)");
    expect(notional).toContain("headlineNet:NONZERO-PAINTED-AS-ZERO");
  });

  it("NON-VACUITY: flags a single removed paint (one dash is enough to fail)", () => {
    const raw = wire(NOTIONAL_NOT_ATTRIBUTABLE_WIRE);
    const p = paintOf(mapToOmniOpportunity(raw as never));
    expect(paintViolations(raw, p)).toEqual([]); // painted ⇒ clean
    const withoutIn = { ...p, grid: { ...p.grid, in: DASH } };
    expect(paintViolations(raw, withoutIn)).toContain(
      "in:WIRE-HAS-VALUE-BUT-DASH(simulated_amount_in_usd)",
    );
  });
});

// ─── Memo closure: a field the card PAINTS must break memo equality ───────────

const baseOpp = mapToOmniOpportunity(
  wire({
    ...SIM_CLOSED_ATTRIBUTABLE_WIRE,
    detector_id: "dex_engine",
    pipeline_latency_ms: 1573,
    risk_score: 0.34,
    pair_symbol: "WETH/USDC",
    chain_base_token_symbol: "WETH",
    rejection_reason: "non_positive_profit",
    token_in_info: {
      symbol: "WETH",
      registry_symbol: "WETH",
      decimals: 18,
      logo_url: null,
      resolved_via: "onchain_full",
    },
    token_out_info: {
      symbol: "USDC",
      registry_symbol: "USDC",
      decimals: 6,
      logo_url: null,
      resolved_via: "onchain_full",
    },
    token_prices_usd: { WETH: 2700 },
    leg_symbols: { "0xc": "DAI" },
    semantic_violations: [],
    route_metadata: {
      dex_adapters: ["uniswap-v2", "sushiswap"],
      token_addresses: ["0xa", "0xb", "0xa"],
      pool_addresses: ["0xp1", "0xp2"],
    },
  }),
);

const propsOf = (opp: OmniOpportunity): OpportunityTradeCardProps => ({
  opp,
  now: 0,
  isMounted: false,
  simLoading: false,
  // Stable references: the comparator compares these by identity, so a fresh
  // arrow per call would make EVERY comparison unequal and the memo gate
  // vacuous (it would "pass" whatever the field list said).
  onExecute: NOOP_EXECUTE,
  onInspect: NOOP_INSPECT,
});

const NOOP_EXECUTE = () => {};
const NOOP_INSPECT = () => {};

/** A value that is guaranteed `!==`/`!==JSON` its input, for any field type. */
function changed(value: unknown): unknown {
  if (value === null || value === undefined) return { __changed: true };
  if (typeof value === "string") return `${value}·x`;
  if (typeof value === "number") return value + 1;
  if (typeof value === "boolean") return !value;
  if (Array.isArray(value)) return [...value, "x"];
  if (typeof value === "object") return { ...(value as Record<string, unknown>), __changed: true };
  return "x";
}

/**
 * Fields the card/grid/ladder CANNOT paint (never read by the render). Anything
 * not listed here must break memo equality when it changes — that is what keeps
 * a value from being painted one frame late (or never).
 */
const NOT_PAINTED_BY_CARD: Array<keyof OmniOpportunity> = [
  "cartridge_id", // painted by /strategies, not by this card
  "candidate_id",
  "route_id",
  "pair_id",
  "quote_token",
  "quote_version",
  "graph_version",
  "config_version",
  "strategy_version",
  "gate_results",
  "data_quality",
  "raw_expected_profit_usd", // audited by the detail view, not painted here
  "raw_net_expected_profit_usd",
  "raw_simulated_net_profit_usd",
  "paper_status",
  "block_number",
  "bridge",
  "bridge_fee_usd",
  "chains_used",
  "dexes_used",
  "simulated_at",
  "simulated_notes",
];

/** Objects the comparator reaches into field-by-field, not as a blob. */
const SUBFIELD_MUTATOR: Partial<Record<keyof OmniOpportunity, (v: never) => unknown>> = {
  token_in_info: (v: { symbol?: string | null }) => ({ ...v, symbol: `${v.symbol ?? "X"}·x` }),
  token_out_info: (v: { symbol?: string | null }) => ({ ...v, symbol: `${v.symbol ?? "X"}·x` }),
};

describe("CARDS-MEMO-COVER-01 — the comparator covers every painted field", () => {
  const keys = Object.keys(baseOpp) as Array<keyof OmniOpportunity>;

  it("identical props stay memo-equal (the memo still works)", () => {
    expect(opportunityTradeCardPropsEqual(propsOf(baseOpp), propsOf({ ...baseOpp }))).toBe(true);
  });

  it("every field the card paints breaks memo equality when it changes", () => {
    const unpainted = new Set<string>(NOT_PAINTED_BY_CARD);
    const offenders: string[] = [];
    const checked: string[] = [];
    for (const key of keys) {
      const mutate = SUBFIELD_MUTATOR[key] ?? ((v: unknown) => changed(v));
      const next = { ...baseOpp, [key]: mutate(baseOpp[key] as never) } as OmniOpportunity;
      expect(next[key], `mutator for ${key} must actually change the value`).not.toEqual(
        baseOpp[key],
      );
      const equal = opportunityTradeCardPropsEqual(propsOf(baseOpp), propsOf(next));
      if (unpainted.has(key)) {
        // Declared unpainted ⇒ changing it must NOT force a re-render.
        if (!equal) offenders.push(`${key}: declared NOT_PAINTED but breaks equality`);
      } else {
        checked.push(key);
        if (equal) offenders.push(`${key}: PAINTED but the memo starves it (stale cell)`);
      }
    }
    expect(offenders, offenders.join("; ")).toEqual([]);
    // Non-vacuity: the loop really walked the painted surface.
    expect(checked.length).toBeGreaterThan(30);
    expect(keys.length).toBeGreaterThan(50);
  });

  it("the simulated_* block specifically — the fields whose late arrival was hidden", () => {
    const withSim = baseOpp;
    const withoutSim: OmniOpportunity = {
      ...baseOpp,
      simulated_net_profit_usd: null,
      simulated_amount_in_usd: null,
      simulated_gross_usd: null,
      simulated_costs_total_usd: null,
      simulated_roi_pct: null,
      simulated_cost_breakdown: null,
    };
    // A batch that ONLY brings the forward-sim results must repaint the card.
    expect(opportunityTradeCardPropsEqual(propsOf(withoutSim), propsOf(withSim))).toBe(false);
    // …and the paint really does differ (the mechanism, end to end).
    const before = paintOf(withoutSim);
    const after = paintOf(withSim);
    expect(before.grid["in"]).toBe(DASH);
    expect(after.grid["in"]).toBe("$1000.00");
  });
});
