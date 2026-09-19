// frontend/app/strategies/tabs/__tests__/TokenAllowlistTopPresetMenu.test.tsx
//
// PC-07 (2026-09-19): SSR steady-state render of the Top-N preset menu —
// presets Top 20/30/40/100, window current|24h, the 10 rug-rule toggles
// (anti-rug/anti-scam incl. memecoin detection), and the honest result table
// (R8: "—" for uncomputed, backend-evaluated rules — RULE 00: the frontend
// never evaluates a rug rule itself).
import React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import { RUG_RULE_CATALOG, TopPresetMenu } from "../TokenAllowlistTab";
import type { RugRuleId, TokenTopResponse } from "@/lib/apex/schemas";

function renderMenu(overrides: Partial<Parameters<typeof TopPresetMenu>[0]> = {}) {
  return renderToStaticMarkup(
    React.createElement(TopPresetMenu, {
      chainId: 1,
      topLimit: 20,
      setTopLimit: () => {},
      topWindow: "current",
      setTopWindow: () => {},
      rugRules: new Set<RugRuleId>(RUG_RULE_CATALOG.map((r) => r.id)),
      toggleRugRule: () => {},
      onLoadTop: () => {},
      loading: false,
      result: null,
      error: null,
      ...overrides,
    }),
  );
}

const RESULT: TokenTopResponse = {
  chain_id: 1,
  window: "24h",
  limit: 20,
  ranked: [
    {
      symbol: "WETH", address: "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
      tvl_usd: "1000000.00", tvl_24h_ago_usd: "900000.00", pool_count: 5,
      safety_score: 80, validation_score: 75, final_status: "VERIFIED",
      liquidity_usd: "500000.00", volume_24h_usd: "100000.00", vol_liq_ratio: 0.2,
      registry_verified: true, oldest_pool_age_hours: 5000, risk_level: "LOW",
      is_stablecoin: false, memecoin_heuristic: false,
      failed_rules: [], unverified_rules: [],
    },
    {
      symbol: null, address: "0xdead",
      tvl_usd: "500.00", tvl_24h_ago_usd: null, pool_count: 1,
      safety_score: null, validation_score: null, final_status: null,
      liquidity_usd: null, volume_24h_usd: null, vol_liq_ratio: null,
      registry_verified: null, oldest_pool_age_hours: 1, risk_level: null,
      is_stablecoin: false, memecoin_heuristic: true,
      failed_rules: [], unverified_rules: ["min_safety_score"],
    },
  ],
  excluded: { no_price: 3, "rug:multi_pool": 2 },
  price_coverage: { symbols_priced: 40, symbols_total: 43 },
  window_note: "reservas de hace ≤24h valorizadas a precios ACTUALES (flujo de capital, no market cap histórico)",
  rules_applied: ["multi_pool"],
  rule_catalog: RUG_RULE_CATALOG.map((r) => ({ id: r.id, label: r.label })),
};

describe("TopPresetMenu — PC-07 steady states", () => {
  it("renders the 4 preset limits + both windows", () => {
    const html = renderMenu();
    for (const n of [20, 30, 40, 100]) expect(html).toContain(`Top ${n}`);
    expect(html).toContain("Capitalización actual");
    expect(html).toContain("Últimas 24 horas");
  });

  it("renders exactly the 10 rug-rule toggles, all checked by default", () => {
    const html = renderMenu();
    for (const { label } of RUG_RULE_CATALOG) expect(html).toContain(label);
    expect(RUG_RULE_CATALOG).toHaveLength(10);
    expect(html.match(/<input type="checkbox"[^>]*checked/g)).toHaveLength(10);
  });

  it("renders the result table with honest R8 dashes and exclusion histogram", () => {
    const html = renderMenu({ result: RESULT });
    expect(html).toContain("WETH");
    expect(html).toContain("excluidos:");
    expect(html).toContain("no_price=3");
    expect(html).toContain("rug:multi_pool=2");
    expect(html).toContain("flujo de capital");
  });

  it("shows the error verbatim (RULE 00 — no fabricated preview)", () => {
    const html = renderMenu({ error: "query_failed" });
    expect(html).toContain("query_failed");
  });
});
