/**
 * OpportunityCard tests using react-dom/server renderToStaticMarkup.
 *
 * Aligned with existing test style (see components/__tests__/StatusPill.test.tsx):
 *   - No jsdom, no @testing-library/react.
 *   - Static markup is sufficient because the card has no internal state,
 *     no effects and no event bindings that need firing to validate output.
 *
 * Coverage:
 *   - Renders pair symbol, both DEX names and status label.
 *   - CTA is disabled when status is "detected" (R8 fail-honest: never
 *     allow execution of unvalidated opportunities from the grid).
 *   - CTA is enabled when status is "validated".
 *   - Rejection reason surfaces when status is "rejected".
 */
import React from "react";
import { describe, it, expect } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";
import { OpportunityCard } from "../OpportunityCard";
import type { OmniOpportunity, OpportunityStatus } from "@/lib/store/types";

function baseOpp(overrides: Partial<OmniOpportunity> = {}): OmniOpportunity {
  return {
    id: "opp-test-1",
    chain_id: 1,
    strategy_kind: "dex_arb",
    status: "validated",
    dex_a: "UniswapV3",
    dex_b: "SushiSwap",
    pair_symbol: "WETH/USDC",
    token_in: "0x0000000000000000000000000000000000000000",
    token_out: "0x0000000000000000000000000000000000000001",
    token_in_info: { symbol: "WETH", address: "0x0000000000000000000000000000000000000000", decimals: 18 },
    token_out_info: { symbol: "USDC", address: "0x0000000000000000000000000000000000000001", decimals: 6 },
    chain_base_token_symbol: "ETH",
    amount_in_wei: "1000000000000000000",
    expected_profit_usd: 12.5,
    simulated_net_profit_usd: 11.2,
    roi_pct: 0.85,
    gas_used: 210000,
    simulated_cost_breakdown: { gas_usd: 1.3 },
    ...overrides,
  } as OmniOpportunity;
}

describe("OpportunityCard", () => {
  it("renders pair symbol and both DEX names", () => {
    const html = renderToStaticMarkup(<OpportunityCard opportunity={baseOpp()} />);
    expect(html).toContain("WETH/USDC");
    expect(html).toContain("UniswapV3");
    expect(html).toContain("SushiSwap");
  });

  it("renders status label", () => {
    const html = renderToStaticMarkup(<OpportunityCard opportunity={baseOpp()} />);
    // StatusPill uppercases labels.
    expect(html.toUpperCase()).toContain("VALIDATED");
  });

  it("disables CTA when status is 'detected' (fail-honest gate)", () => {
    const html = renderToStaticMarkup(
      <OpportunityCard opportunity={baseOpp({ status: "detected" })} />
    );
    // React serialises the `disabled` attribute as bare "disabled" (or disabled="").
    // Assert the primary CTA button is disabled.
    expect(html).toMatch(/<button[^>]*disabled(=""|\s|>)/i);
  });

  it("enables CTA when status is 'validated'", () => {
    const html = renderToStaticMarkup(
      <OpportunityCard opportunity={baseOpp({ status: "validated" })} />
    );
    // Ensure at least one button is NOT disabled (i.e. the enabled CTA).
    const buttons = html.match(/<button[^>]*>/gi) ?? [];
    const hasEnabledButton = buttons.some((b) => !/disabled/i.test(b));
    expect(hasEnabledButton).toBe(true);
  });

  it("surfaces rejection reason when status is 'rejected'", () => {
    const html = renderToStaticMarkup(
      <OpportunityCard
        opportunity={baseOpp({
          status: "rejected" as OpportunityStatus,
          rejection_reason: "SlippageTooHigh",
        })}
      />
    );
    expect(html).toContain("SlippageTooHigh");
  });
});
