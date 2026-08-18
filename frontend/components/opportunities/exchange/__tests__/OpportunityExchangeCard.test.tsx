/**
 * RU-A (SSOT card) — the two-state gate that ends "fallback opportunities".
 *
 * A hollow detection (status=detected + rejected + no economics anywhere) is a
 * DIAGNOSTIC, not an opportunity. These tests pin the contract:
 *   - Shell ⇒ renders the Detection face ("Detección · Sin evaluar" badge), the
 *     decoded WHY, the raw machine reason — and NEVER the trading skeleton
 *     (no EXECUTE button, no computed ledger rows; the atlas_264 model face
 *     honestly renders the ledger labels with "—" placeholders, so the guards
 *     below pin structure, not casing).
 *   - Evaluated row ⇒ the full trading card (net yield + ledger + execute).
 *   - A row with ANY computed economics (even a simulated net) is NOT a shell.
 */
import React from "react";
import { describe, it, expect } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";

import { OpportunityExchangeCard, isUnevaluatedShell } from "../OpportunityExchangeCard";
import type { OmniOpportunity } from "@/lib/store/types";

function makeOpp(over: Partial<Record<string, unknown>> = {}): OmniOpportunity {
  return {
    id: "opp-1",
    chain_id: 1,
    chain_id_out: null,
    strategy_kind: "mev_01_023_path_inconsistency_arbitrage",
    dex_a: "unknown",
    dex_b: null,
    token_in: "0xaaaa",
    token_out: "0xbbbb",
    token_in_info: null,
    token_out_info: null,
    amount_in_wei: "0",
    expected_profit_usd: null,
    net_expected_profit_usd: null,
    roi_pct: null,
    risk_score: null,
    status: "detected",
    rejection_reason: "cartridge_unmapped_strategy_label:route_graph_engine",
    trace_id: "t-1",
    detected_at: "2026-08-17T14:04:24Z",
    updated_at: "2026-08-17T14:04:24Z",
    bridge: null,
    bridge_fee_usd: null,
    route_metadata: {},
    simulated_net_profit_usd: null,
    simulated_amount_in_usd: null,
    simulated_cost_breakdown: null,
    simulated_target: null,
    chain_base_token_symbol: "ETH",
    ...over,
  } as unknown as OmniOpportunity;
}

const props = (opp: OmniOpportunity) => ({
  opp,
  now: new Date("2026-08-17T14:04:40Z").getTime(),
  isMounted: true,
  simLoading: false,
  strategyConfig: null,
  modeLabel: "paper" as const,
  onExecute: () => {},
  onInspect: () => {},
});

describe("isUnevaluatedShell", () => {
  it("true for detected+rejected+no economics", () => {
    expect(isUnevaluatedShell(makeOpp())).toBe(true);
  });
  it("false when a simulated net exists (economics ran somewhere)", () => {
    expect(isUnevaluatedShell(makeOpp({ simulated_net_profit_usd: 1.2 }))).toBe(false);
  });
  it("false when accepted (no rejection_reason)", () => {
    expect(
      isUnevaluatedShell(makeOpp({ status: "viable", rejection_reason: null, expected_profit_usd: 3.5 })),
    ).toBe(false);
  });
});

describe("OpportunityExchangeCard — SSOT two states", () => {
  it("shell renders the DETECTION face with decoded reason — never the trading skeleton", () => {
    const html = renderToStaticMarkup(<OpportunityExchangeCard {...props(makeOpp())} />);
    // atlas_264 model badge splits the old single string into badge words.
    expect(html).toContain("Detección");
    expect(html).toContain("Sin evaluar");
    expect(html).toContain("cartridge_unmapped_strategy_label:route_graph_engine");
    // model footer wording (Sin evaluación económica no hay Execute…).
    expect(html).toContain("Sin evaluación económica no hay Execute");
    // trading skeleton must NOT dress a shell — structural guards (casing-proof):
    // no execute button element, none of the evaluated-face-only ledger rows
    // ("Gross out (AMM)", "Ruta", "Interés a pagar"); the diag face DOES show
    // "Net Yield —" and "Interés —" placeholders per the atlas_264 model.
    expect(html).not.toContain('class="btn"');
    expect(html).not.toContain("Gross out (AMM)");
    expect(html).not.toContain("Ruta");
    expect(html).not.toContain("Interés a pagar");
  });

  it("evaluated row keeps the full trading card", () => {
    const opp = makeOpp({
      status: "viable",
      rejection_reason: null,
      expected_profit_usd: 42.5,
      net_expected_profit_usd: 18.2,
      roi_pct: 1.7,
      route_metadata: {
        dex_adapters: ["uniswap_v2", "sushiswap"],
        token_addresses: ["0xaaaa", "0xbbbb", "0xaaaa"],
        pool_addresses: ["0xpool1", "0xpool2"],
      },
    });
    const html = renderToStaticMarkup(<OpportunityExchangeCard {...props(opp)} />);
    // model labels are title-case ("Net Yield") and the button is uppercase.
    expect(html).toContain("Net Yield");
    expect(html).toContain("EXECUTE");
    expect(html).not.toContain("Detección — sin evaluar");
  });
});
