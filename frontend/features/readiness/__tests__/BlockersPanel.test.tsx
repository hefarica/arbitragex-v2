/**
 * BlockersPanel SSR-only static markup tests.
 *
 * useEffect doesn't fire under renderToStaticMarkup, so we only see the
 * initial "loading" state — sufficient to confirm:
 *   - The card title and description render.
 *   - No "All blockers resolved" appears prematurely (R8: never claim resolved
 *     before data arrives).
 *   - The R8 disclaimer ("Env values are redacted") is rendered.
 */
import React from "react";
import { describe, it, expect } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";
import { BlockersPanel } from "../BlockersPanel";

const html = renderToStaticMarkup(<BlockersPanel />);

describe("BlockersPanel (SSR initial state)", () => {
  it("renders the Readiness blockers title", () => {
    expect(html).toMatch(/Readiness blockers/i);
  });

  it("renders the redaction disclaimer", () => {
    expect(html).toMatch(/redacted/i);
    expect(html).toMatch(/present/);
  });

  it("renders loading state, NOT 'All blockers resolved'", () => {
    expect(html).toMatch(/Loading blockers/i);
    expect(html).not.toMatch(/All readiness blockers resolved/i);
  });

  it("does NOT render any 'GO live' or live-enable button", () => {
    expect(html).not.toMatch(/>Activate live</);
    expect(html).not.toMatch(/>Submit transaction</);
    expect(html).not.toMatch(/>Broadcast bundle</);
  });
});
