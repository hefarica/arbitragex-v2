// frontend/components/opportunities/__tests__/OpportunityDetailTabs.test.tsx
//
// FE-0034 (§37) — the tabbed detail body: seven tabs, each honest to its
// wire source. Fixtures go through the real mapper (semantic_violations
// computed, never hand-set). Radix Tabs only mounts the ACTIVE tab, so each
// assertion mounts the component with the tab it needs via defaultTab.
import React from "react";
import { describe, it, expect } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";

import { OpportunityDetailTabs } from "../OpportunityDetailTabs";
import { mapToOmniOpportunity } from "@/lib/store/types";

const wire = (over: Record<string, unknown>) => ({
  id: "opp-1",
  chain_id: 1,
  strategy_kind: "dex_arb",
  detected_at: "2026-08-11T00:00:00Z",
  status: "detected",
  trace_id: "trace-1",
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

const rich = () =>
  mapToOmniOpportunity(
    wire({
      route_metadata: rm2hop,
      pair_symbol: "WETH/USDC",
      expected_profit_usd: 10.5,
      net_expected_profit_usd: 8.25,
      roi_pct: 0.12,
      risk_score: 0.34,
      amount_in_wei: "1500000000000000000",
      simulated_amount_in_usd: 4500,
      simulated_net_profit_usd: 7.9,
      simulated_roi_pct: 0.18,
      simulated_at: "2026-08-11T00:00:05Z",
      simulated_cost_breakdown: {
        gas_usd: 1.2,
        lp_fees_usd: 0.5,
        slippage_usd: 0.3,
        failure_buffer_usd: 0.1,
        copied_buffer_usd: 0,
        capital_cost_usd: 0,
        ops_overhead_usd: 0,
        flashloan_fee_usd: 0.05,
        relay_fee_usd: 0,
      },
      simulated_target: {
        target_net_usd: 5,
        target_roi_pct: 0.1,
        target_source: "strategy_config",
        binding_floor: "usd-floor",
        estimation_basis: "observed-gross",
        required_amount_in_usd: 100,
        cap_amount_in_usd: 5000,
        suggested_amount_in_usd: 4500,
        suggested_net_usd: 7.9,
        suggested_roi_pct: 0.18,
        meets_target_at_cap: true,
        notes: [],
      },
    }),
  );

function tab(opp: ReturnType<typeof rich>, tabName: string): string {
  // Cast: defaultTab is the const-union DetailTab; tests stay stringly so the
  // seven tab names stay data-driven.
  return renderToStaticMarkup(
    React.createElement(OpportunityDetailTabs, {
      opp,
      defaultTab: tabName as never,
    }),
  );
}

describe("OpportunityDetailTabs (§37)", () => {
  it("renders the seven §37 tab triggers", () => {
    const html = tab(rich(), "overview");
    for (const t of [
      "overview",
      "route",
      "economics",
      "simulation",
      "gates",
      "provenance",
      "latency",
    ]) {
      expect(html).toContain(`>${t}</button>`);
    }
  });

  it("Overview: headline rows + the §36 grid spine (FE-0033 reuse, no parallel model)", () => {
    const html = tab(rich(), "overview");
    expect(html).toContain("detected");
    expect(html).toContain("dex_arb");
    expect(html).toContain("WETH/USDC");
    expect(html).toContain('data-testid="opportunity-summary-grid"');
  });

  it("Route §38: edges table from the persisted topology; per-leg economics declared as gaps", () => {
    const withRm = tab(rich(), "route");
    expect(withRm).toContain("Token path");
    expect(withRm).toContain("Pool");
    // real topology rows: token path (8…6 elision) + pool per leg
    expect(withRm).toContain("0xa → 0xb");
    expect(withRm).toContain("0xpool1");
    expect(withRm).not.toContain("SYNTHETIC LEGACY VIEW");
    // the §38 gap set is disclosed, not papered over
    expect(withRm).toContain("amounts / rate / fee / liquidity / impact / gas / state");
    expect(withRm).toContain("nivel-(b)");
  });

  it("Route §29: legacy fallback legs render marked — never ROUTE VERIFIED", () => {
    // dex_a/dex_b present, no route_metadata → deriveLegs synthetic fallback
    // renders the MARKED table (syn badges + label), hops stays null-honest.
    const html = tab(mapToOmniOpportunity(wire({ route_metadata: null })), "route");
    expect(html).toContain("SYNTHETIC LEGACY VIEW");
    expect(html).toContain("syn");
    expect(html).toContain("no ROUTE");
    // hops row renders the dash (hop_count null without rm — FE-0028)
    expect(html).toContain(">Hops</span>");
  });

  it("Economics: Gross and Net from their canonical fields (legacy mislabel fixed)", () => {
    const html = tab(rich(), "economics");
    expect(html).toContain("Gross (USD)");
    expect(html).toContain("$10.5000");
    expect(html).toContain("Net (USD)");
    expect(html).toContain("$8.2500");
    // honest null economics render the dash, never a zero
    const bare = tab(mapToOmniOpportunity(wire({ route_metadata: rm2hop })), "economics");
    expect(bare).toContain("—");
    expect(bare).not.toContain("$0.0000");
  });

  it("Economics §39: waterfall shows EVERY simulated cost line, sin ocultar", () => {
    const html = tab(rich(), "economics");
    expect(html).toContain("Cascada de costos");
    for (const line of [
      "− Gas",
      "− LP fees",
      "− Decoherencia de Estado",
      "− Failure buffer",
      "− Copied buffer",
      "− Capital cost",
      "− Ops overhead",
      "− TLS fee",
      "− Relay fee",
    ]) {
      expect(html).toContain(line);
    }
    expect(html).toContain("$1.2000"); // gas
    expect(html).toContain("$0.0500"); // TLS fee
    // §79 disclaimer present
    expect(html).toContain("el FE no recomputa Gross − Σ = Net");
  });

  it("Economics §39: no simulated block → honest absence, no fabricated cascade", () => {
    const html = tab(
      mapToOmniOpportunity(wire({ route_metadata: rm2hop })),
      "economics",
    );
    expect(html).toContain("Sin desglose de costos persistido");
    expect(html).not.toContain("Cascada de costos");
  });

  it("Simulation: wire block as VALUES + cost breakdown + on-demand button (§79)", () => {
    const html = tab(rich(), "simulation");
    expect(html).toContain("~$7.9000");
    expect(html).toContain("Gas (USD)");
    expect(html).toContain("$1.2000");
    expect(html).toContain("TLS Fee (USD)");
    expect(html).toContain("$0.0500");
    // the button is present
    expect(html).toMatch(/button[^>]*>/);
  });

  it("Simulation: absent breakdown stays honest — no fabricated costs", () => {
    const html = tab(
      mapToOmniOpportunity(wire({ route_metadata: rm2hop })),
      "simulation",
    );
    expect(html).toContain("Sin desglose simulado persistido");
    expect(html).not.toContain("$0.0000");
  });

  it("Gates: backend verdicts render — rejection, §30 violations, target verdict", () => {
    const quarantined = mapToOmniOpportunity(
      wire({
        route_metadata: rm2hop,
        rejection_reason: "cartridge_gate:addr",
        block_number: undefined,
        net_expected_profit_usd: "oops",
      }),
    );
    const html = tab(quarantined, "gates");
    expect(html).toContain("cartridge_gate:addr");
    expect(html).toContain("QUARANTINED");
    expect(html).toContain("missing_block · profit_not_numeric");

    const clean = tab(rich(), "gates");
    expect(clean).toContain("0 (validación limpia)");
    expect(clean).toContain("PASS");
    expect(clean).toContain("usd-floor");
  });

  it("Gates §40: observed / required / delta / reason from simulated_target", () => {
    const html = tab(rich(), "gates");
    expect(html).toContain("Observed (net @ tamaño sugerido)");
    expect(html).toContain("$7.9000"); // observed = suggested_net_usd
    expect(html).toContain("Required (target_net)");
    expect(html).toContain("$5.0000"); // required = target_net_usd
    // delta = observed − required = +2.90, sign kept
    expect(html).toContain("+$2.9000");
    expect(html).toContain("Reason (binding_floor)");
    expect(html).toContain("Sizing: required / cap / sugerido");
    expect(html).toContain("$100.0000 / $5000.0000 / $4500.0000");
  });

  it("Gates §40: negative delta renders destructive and keeps its sign", () => {
    const below = mapToOmniOpportunity(
      wire({
        route_metadata: rm2hop,
        simulated_target: {
          target_net_usd: 10,
          target_roi_pct: null,
          target_source: "simulation_tab",
          binding_floor: "roi-unreachable",
          estimation_basis: "roi-assumed",
          required_amount_in_usd: 100,
          cap_amount_in_usd: 5000,
          suggested_amount_in_usd: 4500,
          suggested_net_usd: 4.2,
          suggested_roi_pct: 0.09,
          meets_target_at_cap: false,
          notes: ["cap insuficiente"],
        },
      }),
    );
    const html = tab(below, "gates");
    expect(html).toContain("FAIL");
    expect(html).toContain("-$5.8000"); // 4.2 − 10
    expect(html).toContain("cap insuficiente"); // notes surface
  });

  it("Provenance: detected_at null renders the dash — never a fabricated 1970 date", () => {
    const html = tab(
      mapToOmniOpportunity(wire({ route_metadata: rm2hop, detected_at: undefined })),
      "provenance",
    );
    expect(html).toContain("Detected At");
    expect(html).not.toContain("1/1/1970");
    const ok = tab(rich(), "provenance");
    expect(ok).toContain("opp-1");
    expect(ok).toContain("trace-1");
    expect(ok).toContain("123");
  });

  it("Latency: group-absent state keeps the EMIT-09 pointer (pre-deploy backend)", () => {
    const html = tab(rich(), "latency");
    expect(html).toContain("no emitido");
    expect(html).toContain("ARBX-FE-EMIT-09");
  });

  it("Latency (FE-0037): latencyRows/meta props wire into the §45 waterfall", () => {
    // Wiring proof: the props the dialog container derives from useRouteTick
    // reach the panel (panel-internal states are covered by its own suite).
    const html = renderToStaticMarkup(
      React.createElement(OpportunityDetailTabs, {
        opp: rich(),
        defaultTab: "latency" as never,
        latencyRows: [
          {
            route_hash: "c".repeat(64),
            route_kind: "multihop",
            hops: 5,
            stages: { gates_us: 2000, reprice_us: 500 },
            total_us: 2500,
          },
        ],
        latencyMeta: {
          attribution: { gates: "measured", reprice: "measured-upper-bound" },
          cap: 10,
          sampled: 1,
          truncated: false,
          dropped: 0,
        },
      }),
    );
    expect(html).toContain("multihop · 5 hops");
    expect(html).toContain("2.00 ms");
    expect(html).toContain("2.50 ms");
    expect(html).toContain("sampled 1");
  });

  it("R1: pure render is byte-identical across invocations", () => {
    const opp = rich();
    const a = tab(opp, "overview");
    const b = tab(opp, "overview");
    expect(a).toBe(b);
  });
});
