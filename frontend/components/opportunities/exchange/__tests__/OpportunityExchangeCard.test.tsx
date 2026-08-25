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
import { shortAddr } from "@/lib/format";
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
    semantic_violations: [],
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
  it("true for the live-feed hollow case: NO rejection_reason, no economics (gate v2)", () => {
    // The production feed showed 50/50 "Evaluada · LIVE" cards whose entire
    // ledger was "—" (degenerate AGLD⇄AGLD rows) — rows that escape the old
    // status+reason gate. Zero economics ANYWHERE ⇒ DETECCIÓN face.
    expect(
      isUnevaluatedShell(makeOpp({ rejection_reason: null })),
    ).toBe(true);
  });
  it("false when ANY economics exist — even partial (simulated amount only)", () => {
    expect(
      isUnevaluatedShell(
        makeOpp({ rejection_reason: null, simulated_amount_in_usd: 12000 }),
      ),
    ).toBe(false);
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

  it("hollow row WITHOUT rejection reason renders the DETECCIÓN face honestly (gate v2)", () => {
    // The production AGLD⇄AGLD case: no reason recorded, no economics. The
    // face must NOT claim REJECTED (nothing rejected it) and must surface the
    // honest no-economics WHY instead of a decoded machine reason.
    const html = renderToStaticMarkup(
      <OpportunityExchangeCard
        {...props(
          makeOpp({
            rejection_reason: null,
            token_in: "0x3235999x",
            token_out: "0x3235999x",
          }),
        )}
      />,
    );
    expect(html).toContain("Detección");
    expect(html).toContain("Sin evaluar");
    // honest WHY for reason-less rows:
    expect(html).toContain("Sin evaluación económica computada");
    // R8: never claim REJECTED when no rejection exists:
    expect(html).not.toContain("→ REJECTED");
    // machine-reason row degrades to "—" instead of the string "unknown":
    expect(html).not.toContain(">unknown<");
    // degenerate raw-data flag surfaces (same token both legs):
    expect(html).toContain("token_in == token_out");
    // and never the trading skeleton:
    expect(html).not.toContain('class="btn"');
    expect(html).not.toContain("Gross out (AMM)");
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
    // FE-0030 (§29): persisted topology is ROUTE-grade — NO synthetic marker.
    expect(html).not.toContain("SYNTHETIC LEGACY VIEW");
    // and the LP-fees line carries the persisted leg count.
    expect(html).toContain("LP fees (2 legs)");
  });

  // ── FE-0030 (§29): legacy rows render the synthetic view MARKED ───────────
  it("legacy row (no route_metadata) renders SYNTHETIC LEGACY VIEW on the Ruta row", () => {
    const opp = makeOpp({
      status: "viable",
      rejection_reason: null,
      expected_profit_usd: 42.5,
      net_expected_profit_usd: 18.2,
      roi_pct: 1.7,
      route_metadata: null, // legacy row → synthetic 2-leg fallback (mapper normalizes {} to null)
      dex_a: "uniswap-v2",
      dex_b: "sushiswap",
    });
    const html = renderToStaticMarkup(<OpportunityExchangeCard {...props(opp)} />);
    expect(html).toContain("SYNTHETIC LEGACY VIEW");
    // §29: never an operational per-leg count off a synthetic view.
    expect(html).not.toContain("(2 legs)");
    // the marker explains itself (title carries the §29 rationale).
    expect(html).toContain("NO es ROUTE VERIFIED");
  });

  // ── F2 (audit §11 RC1): intermediate route legs show their currency code ──
  const A = "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
  const B = "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
  const C = "0xcccccccccccccccccccccccccccccccccccccccc";
  const D = "0xdddddddddddddddddddddddddddddddddddddddd";

  it("route summary resolves intermediate legs via leg_symbols — no shortAddr", () => {
    const opp = makeOpp({
      status: "viable",
      rejection_reason: null,
      expected_profit_usd: 42.5,
      net_expected_profit_usd: 18.2,
      roi_pct: 1.7,
      token_in: A,
      token_out: D,
      route_metadata: {
        dex_adapters: ["uniswap_v2", "sushiswap", "uniswap_v3"],
        token_addresses: [A, B, C, D],
        pool_addresses: ["0xpool1", "0xpool2", "0xpool3"],
      },
      leg_symbols: { [B]: "PEPE", [C]: "USDC" },
    });
    const html = renderToStaticMarkup(<OpportunityExchangeCard {...props(opp)} />);
    // Intermediate legs render their currency codes…
    expect(html).toContain("PEPE");
    expect(html).toContain("USDC");
    // …and never the truncated intermediate addresses.
    expect(html).not.toContain(shortAddr(B));
    expect(html).not.toContain(shortAddr(C));
  });

  it("absent leg_symbols keeps the honest shortAddr fallback (R8)", () => {
    const opp = makeOpp({
      status: "viable",
      rejection_reason: null,
      expected_profit_usd: 42.5,
      net_expected_profit_usd: 18.2,
      roi_pct: 1.7,
      token_in: A,
      token_out: D,
      route_metadata: {
        dex_adapters: ["uniswap_v2", "sushiswap", "uniswap_v3"],
        token_addresses: [A, B, C, D],
        pool_addresses: ["0xpool1", "0xpool2", "0xpool3"],
      },
      // No leg_symbols (legacy payload) → intermediates fall back honestly.
    });
    const html = renderToStaticMarkup(<OpportunityExchangeCard {...props(opp)} />);
    expect(html).toContain(shortAddr(B));
    expect(html).toContain(shortAddr(C));
  });

  // ── F1 (audit §11 RC2): registry_symbol as display fallback ────────────────
  it("pair token falls back to registry_symbol when symbol is null", () => {
    const opp = makeOpp({
      status: "viable",
      rejection_reason: null,
      expected_profit_usd: 42.5,
      net_expected_profit_usd: 18.2,
      roi_pct: 1.7,
      token_in: A,
      token_out: D,
      token_in_info: {
        symbol: null,
        decimals: null,
        logo_url: null,
        resolved_via: "failed",
        registry_symbol: "WETH",
      },
      route_metadata: {
        dex_adapters: ["uniswap_v2"],
        token_addresses: [A, D],
        pool_addresses: ["0xpool1"],
      },
    });
    const html = renderToStaticMarkup(<OpportunityExchangeCard {...props(opp)} />);
    // registry_symbol is real curated-list data — it surfaces as the chip's
    // symbol label instead of the truncated address. (The dedicated
    // "Contratos" row keeps showing raw addresses by design.)
    expect(html).toContain("<span>WETH</span>");
    expect(html).not.toContain(`<span>${shortAddr(A)}</span>`);
  });

  // FE-0031 (§30): a semantically-violating row is QUARANTINED in place —
  // visible on BOTH faces, never hidden.
  it("violations render the QUARANTINED strip on the DETECTION face", () => {
    const html = renderToStaticMarkup(
      <OpportunityExchangeCard
        {...props(
          makeOpp({ semantic_violations: ["missing_strategy_id", "missing_block"] }),
        )}
      />,
    );
    expect(html).toContain("QUARANTINED");
    expect(html).toContain("missing_strategy_id · missing_block");
    expect(html).toContain('role="alert"');
  });

  it("violations render the QUARANTINED strip on the EVALUATED face too", () => {
    const rm = {
      dex_adapters: ["uniswap_v2", "sushiswap"],
      token_addresses: ["0xaaaa", "0xbbbb", "0xaaaa"],
      pool_addresses: ["0xpool1", "0xpool2"],
    };
    const html = renderToStaticMarkup(
      <OpportunityExchangeCard
        {...props(
          makeOpp({
            status: "viable",
            rejection_reason: null,
            expected_profit_usd: 3.5,
            route_metadata: rm,
            semantic_violations: ["hop_incoherent"],
          }),
        )}
      />,
    );
    expect(html).toContain("QUARANTINED");
    expect(html).toContain("hop_incoherent");
    // clean rows on the same face never carry the marker
    const clean = renderToStaticMarkup(
      <OpportunityExchangeCard
        {...props(
          makeOpp({
            status: "viable",
            rejection_reason: null,
            expected_profit_usd: 3.5,
            route_metadata: rm,
          }),
        )}
      />,
    );
    expect(clean).not.toContain("QUARANTINED");
  });
});
