/**
 * GoNoGoPanel SSR-only static markup tests.
 *
 * Like SystemGuardBanner.test.tsx, useEffect doesn't fire under
 * renderToStaticMarkup. We assert the structural invariants visible at
 * initial server render:
 *   - "Live trading: OFF" badge rendered (NEVER removed regardless of state)
 *   - "GO / NO-GO decision" title rendered
 *   - The disclaimer "no UI control to flip Live to ON" rendered
 *   - The static-render loading state does NOT show GO verdicts
 *   - There is NO submit/broadcast/private-relay/live-enable button anywhere
 */
import React from "react";
import { describe, it, expect } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";
import { GoNoGoPanel } from "../GoNoGoPanel";

const html = renderToStaticMarkup(<GoNoGoPanel />);

describe("GoNoGoPanel (SSR initial state)", () => {
  it("renders the GO / NO-GO decision title", () => {
    expect(html).toMatch(/GO ?\/ ?NO-GO decision/i);
  });

  it("asserts 'Live trading: OFF' badge on every render", () => {
    expect(html).toContain("Live trading: OFF");
  });

  it("renders the structural barrier disclaimer", () => {
    expect(html).toMatch(/no UI control to flip Live to ON/i);
  });

  it("renders loading state initially (not a GO verdict)", () => {
    expect(html).toMatch(/Loading decision/i);
  });

  it("regression: never renders a 'flip to live' button", () => {
    expect(html).not.toMatch(/>Flip to live</i);
    expect(html).not.toMatch(/>Enable live</i);
    expect(html).not.toMatch(/>Activate live</i);
    expect(html).not.toMatch(/>Submit transaction</i);
    expect(html).not.toMatch(/>Broadcast bundle</i);
    expect(html).not.toMatch(/>Send to mainnet</i);
  });

  it("regression: SSR markup does NOT contain '>GO<' verdict badge", () => {
    // Only NO-GO should appear in any verdict-shaped span pre-data. After
    // data arrives via useEffect, the badge could in principle show GO IF
    // the backend ever returns go_live=true (which it doesn't in P2).
    expect(html).not.toMatch(/<span[^>]*>GO<\/span>/);
  });
});
