/**
 * XRayCard tests — AUDIT-2026-08-29 feed label honesty, extended by
 * A8-CONF-01 (2026-10-03).
 *
 * SSR-only static markup (renderToStaticMarkup, same toolkit alignment as
 * SystemGuardBanner.test.tsx: no jsdom, no network).
 *
 * What this guards (R8 fail-honest — the audit caught the exact inverse of each
 * assertion live on the DApp):
 *   - an unresolved field renders `n/c · <reason>` with the reason BOTH in the
 *     text and in data-field-reason — never a mute "—" the operator must guess
 *   - a computed value carries its provenance in data-field-source
 *   - a real zero (`state:"zero"`) still renders its value verbatim: the
 *     distinction "exactly zero" vs "not computed" lives in the state, not in
 *     a hidden glyph
 *   - the ✓ glyph appears ONLY for success-family sim verdicts
 */
import React from "react";
import { describe, it, expect } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";
import { XRayCard } from "../XRayCard";
import type { ResolvedField } from "@/lib/home-opportunity";

const computed = (text: string, source = "producer"): ResolvedField => ({ state: "computed", text, source });
const zero = (text: string, source = "producer"): ResolvedField => ({ state: "zero", text, source });
const nc = (reason: string): ResolvedField => ({ state: "not_computed", text: `n/c · ${reason}`, reason });

const base = {
  pair: "WETH/USDC",
  yield: computed("+0.42%", "opportunities.roi_pct"),
  confidence: computed("87% conf", "scored_opportunities.posterior_prob"),
  legs: computed("2 legs", "route_metadata.token_addresses"),
  ago: "2026-08-29T18:00:00Z",
  route: computed("uniswap-v2 → sushi", "route_metadata.dex_adapters"),
  decoherence: computed("slippage $0.42", "economics.slippage_usd"),
  tlsAmount: nc("no_producer__flash_loan_principal_not_emitted"),
  simVerdict: computed("success", "opportunities.status + rejection_reason"),
  tokenSafety: computed("A 75 VERIFIED · B 75 VERIFIED", "token_in_info.validation.score"),
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
  it("unscored confidence carries its REASON — neither '0% conf' nor a mute dash", () => {
    const html = render({
      confidence: nc("no_scored_row_for_opportunity"),
      simVerdict: computed("rejected", "opportunities.status + rejection_reason"),
    });
    expect(html).toContain("n/c · no_scored_row_for_opportunity");
    expect(html).toContain('data-field-reason="no_scored_row_for_opportunity"');
    expect(html).not.toContain("0% conf");
    expect(html).not.toContain("conf (unscored)");
  });

  it("scored confidence renders the value plus its provenance", () => {
    const html = render({
      confidence: computed("87% conf", "scored_opportunities.posterior_prob"),
    });
    expect(html).toContain(">87% conf</span>");
    expect(html).toContain('data-field-source="scored_opportunities.posterior_prob"');
  });

  it("a real zero is rendered as a value; only the STATE marks it as zero (R8)", () => {
    const html = render({ confidence: zero("0% conf", "scored_opportunities.posterior_prob") });
    expect(html).toMatch(/data-field-state="zero"[^>]*>0% conf</);
    // The confidence chip specifically must NOT be a not-computed declaration:
    // a producer DID compute this, and it computed exactly zero.
    expect(html).not.toMatch(/data-field-state="not_computed"[^>]*>conf n\/c/);
  });

  it("✓ only for success-family verdicts; 'rejected' renders verbatim", () => {
    const rejected = render({
      confidence: nc("no_scored_row_for_opportunity"),
      simVerdict: computed("rejected", "opportunities.status + rejection_reason"),
    });
    expect(rejected).toContain("rejected");
    expect(rejected).not.toContain("✓");

    const ok = render({ simVerdict: computed("success") });
    expect(ok).toContain("✓ success");

    const simOk = render({ simVerdict: computed("SIM_SUCCESS") });
    expect(simOk).toContain("✓ SIM_SUCCESS");

    const reverted = render({ simVerdict: computed("SIM_REVERT") });
    expect(reverted).toContain("SIM_REVERT");
    expect(reverted).not.toContain("✓");
  });

  it("an n/c sim verdict can never earn a checkmark, whatever its reason text says", () => {
    const html = render({ simVerdict: nc("success-but-uncomputed") });
    expect(html).not.toContain("✓");
  });
});

describe("XRayCard unknown route/safety evidence", () => {
  it("declares unknown hops, route and token safety with reasons, never as zero", () => {
    const html = render({
      confidence: nc("no_scored_row_for_opportunity"),
      simVerdict: nc("no_producer__sim_classification_not_emitted"),
      legs: nc("route_topology_unresolved"),
      route: nc("route_topology_unresolved"),
      tokenSafety: nc("validation_pending_first_sighting"),
    });
    expect(html).toContain("n/c · route_topology_unresolved");
    expect(html).toContain("n/c · validation_pending_first_sighting");
    expect(html).toContain("n/c · no_producer__sim_classification_not_emitted");
    expect(html).not.toContain("✓");
    // No field may degrade to a bare dash.
    expect(html).not.toMatch(/>—</);
  });

  it("TLS AMOUNT renders its declared missing-producer reason, not a dash", () => {
    const html = render({ tlsAmount: nc("no_producer__flash_loan_principal_not_emitted") });
    expect(html).toContain("TLS AMOUNT");
    expect(html).toContain("no_producer__flash_loan_principal_not_emitted");
    expect(html).not.toMatch(/TLS AMOUNT[\s\S]{0,200}>—</);
  });
});
