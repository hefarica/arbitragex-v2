/**
 * SystemGuardBanner tests using react-dom/server renderToStaticMarkup.
 *
 * Why static markup:
 *   - Same toolkit-alignment constraint as StatusPill.test.tsx: no jsdom.
 *   - SSR-only rendering shows the initial "loading" state plus the
 *     hardcoded guard tiles (Live OFF, Paper ON, $0, NO-GO).
 *     Those tiles are the structural barrier to live trading and MUST
 *     be present on every page render.
 *   - useEffect (fetch + interval) doesn't fire under renderToStaticMarkup,
 *     so no network is attempted and no mocks are needed.
 *
 * What this test guards (regression alarm):
 *   - LIVE / Private relay / Submit / Broadcast = OFF  (4× ":OFF")
 *   - Paper = ON                                       (1× ":ON" exact)
 *   - Capital = $0
 *   - GO live = NO-GO
 *   - A.4 fork + A.5 paper-shadow = "…" on static render (verdict arrives
 *     from the readiness decision endpoint via useEffect — AUDIT-2026-08-29
 *     P0: UI = RuntimeVerifierStatus, no hardcoded BLOCKED/PASS)
 *   - Banner never claims Live=ON or GO=GO
 */
import React from "react";
import { describe, it, expect } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";
import { SystemGuardBanner } from "../SystemGuardBanner";

const html = renderToStaticMarkup(<SystemGuardBanner />);

function countOccurrences(needle: string): number {
  let count = 0;
  let idx = 0;
  while ((idx = html.indexOf(needle, idx)) !== -1) {
    count++;
    idx += needle.length;
  }
  return count;
}

describe("SystemGuardBanner", () => {
  it("renders the system guard label", () => {
    expect(html).toMatch(/System guard/i);
  });

  it("renders all four OFF tiles (live + relay + submit + broadcast)", () => {
    expect(html).toContain(">Live:</span>");
    expect(html).toContain(">Private relay:</span>");
    expect(html).toContain(">Submit:</span>");
    expect(html).toContain(">Broadcast:</span>");
    // Exactly four tiles end with the "OFF" value.
    expect(countOccurrences(">OFF</span>")).toBe(4);
  });

  it("renders Paper=ON exactly once (structural assertion)", () => {
    expect(html).toContain(">Paper:</span>");
    expect(countOccurrences(">ON</span>")).toBe(1);
  });

  it("renders Capital=$0 (zero capital exposure)", () => {
    expect(html).toContain(">Capital:</span>");
    expect(html).toContain(">$0</span>");
  });

  it("renders GO live=NO-GO (live trading remains gated)", () => {
    expect(html).toContain(">GO live:</span>");
    expect(html).toContain(">NO-GO</span>");
  });

  it("renders A.4 + A.5 as pending (…) on static render — verdict is runtime data", () => {
    expect(html).toContain(">A.4 fork:</span>");
    expect(html).toContain(">A.5 paper-shadow:</span>");
    // AUDIT-2026-08-29 P0: verdicts come from the readiness decision endpoint
    // (useEffect); static render must show neither BLOCKED nor PASS.
    expect(html.match(/>A\.4 fork:<\/span><span class="font-semibold">([^<]*)</)?.[1]).toBe("…");
    expect(html.match(/>A\.5 paper-shadow:<\/span><span class="font-semibold">([^<]*)</)?.[1]).toBe("…");
    expect(countOccurrences(">BLOCKED</span>")).toBe(0);
    expect(countOccurrences(">PASS</span>")).toBe(0);
  });

  it("renders loading state on initial server render (no fetch fired)", () => {
    expect(html).toMatch(/Loading runtime/i);
  });

  it("regression alarm: never renders Live=ON or GO=GO", () => {
    // If a future edit flips Live to ON or removes NO-GO, this fails.
    expect(html).not.toContain(">Live:</span><span class=\"font-semibold\">ON");
    expect(html).not.toContain(">GO live:</span><span class=\"font-semibold\">GO");
  });
});
