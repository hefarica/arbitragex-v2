/**
 * XRayCard tests — AUDIT-2026-08-29 feed label honesty.
 *
 * SSR-only static markup (renderToStaticMarkup, same toolkit alignment as
 * SystemGuardBanner.test.tsx: no jsdom, no network).
 *
 * What this guards (R8 fail-honest — the audit caught the exact inverse
 * of each assertion live on the DApp):
 *   - unscored confidence (null) renders "— … unscored", NEVER "0% conf"
 *   - scored confidence renders the number + "% conf"
 *   - the ✓ glyph appears ONLY for success-family sim verdicts —
 *     "pendiente"/"reverted" render verbatim without a green checkmark
 */
import React from "react";
import { describe, it, expect } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";
import { XRayCard } from "../XRayCard";

const base = {
  pair: "WETH/USDC",
  yield: "+0.42%",
  legs: 2,
  ago: "2026-08-29T18:00:00Z",
  route: "uniswap-v2 → sushi",
  fees: "convergence 0.42%",
  tlsAmount: "—",
  safetyA: 0,
  safetyB: 0,
};

type Props = Parameters<typeof XRayCard>[0];

function render(overrides: Partial<Props>): string {
  // Spread-merge cast: Partial allows explicitly-undefined members, which
  // TS (correctly) won't accept over required props — the runtime call site
  // below never passes undefined.
  const props = { ...base, ...overrides } as Props;
  return renderToStaticMarkup(<XRayCard {...props} />);
}

describe("XRayCard label honesty (AUDIT-2026-08-29)", () => {
  it("unscored confidence → '—' + '(unscored)', never '0% conf'", () => {
    const html = render({ confidence: null, simVerdict: "pendiente" });
    expect(html).toMatch(/—\s*<\/b>\s*conf \(unscored\)|>—<\/b>conf \(unscored\)/);
    expect(html).not.toContain("0% conf");
  });

  it("scored confidence → 'NN% conf'", () => {
    const html = render({ confidence: 87, simVerdict: "success" });
    expect(html).toContain(">87</b>");
    expect(html).toContain("% conf");
  });

  it("✓ only for success-family verdicts; 'pendiente' renders verbatim", () => {
    const pend = render({ confidence: null, simVerdict: "pendiente" });
    expect(pend).toContain("pendiente");
    expect(pend).not.toContain("✓");

    const ok = render({ confidence: 50, simVerdict: "success" });
    expect(ok).toContain("✓ success");

    const simOk = render({ confidence: 50, simVerdict: "SIM_SUCCESS" });
    expect(simOk).toContain("✓ SIM_SUCCESS");

    const reverted = render({ confidence: 50, simVerdict: "SIM_REVERT" });
    expect(reverted).toContain("SIM_REVERT");
    expect(reverted).not.toContain("✓");
  });
});
