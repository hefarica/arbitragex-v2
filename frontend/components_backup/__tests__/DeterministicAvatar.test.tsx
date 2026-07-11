/**
 * DeterministicAvatar tests using react-dom/server renderToStaticMarkup.
 *
 * Why not @testing-library/react:
 *   - Not installed; adding it requires jsdom + operator approval (toolkit-alignment memory).
 *   - DeterministicAvatar is a pure SVG component: no hooks, no events, no DOM interaction.
 *   - renderToStaticMarkup is sufficient to validate the 3 key invariants.
 *
 * Trade-off: no event simulation, no hooks testing — acceptable here.
 * When Task 11's TokenChip requires onError testing, that's the moment to request
 * @testing-library + jsdom approval.
 */
import React from "react";
import { describe, it, expect } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";
import { DeterministicAvatar } from "../DeterministicAvatar";

describe("DeterministicAvatar", () => {
  it("renders an SVG element with a circle", () => {
    const html = renderToStaticMarkup(
      <DeterministicAvatar seed="0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2" />
    );
    expect(html).toMatch(/<svg[\s>]/);
    expect(html).toMatch(/<circle[\s>]/);
  });

  it("same seed produces identical SVG markup (deterministic — R1 SSR=CSR)", () => {
    const seed = "0xabcdef0123456789";
    const a = renderToStaticMarkup(<DeterministicAvatar seed={seed} />);
    const b = renderToStaticMarkup(<DeterministicAvatar seed={seed} />);
    expect(a).toBe(b);
  });

  it("different seeds produce different SVG markup", () => {
    const a = renderToStaticMarkup(
      <DeterministicAvatar seed="0x1111111111111111" />
    );
    const b = renderToStaticMarkup(
      <DeterministicAvatar seed="0x2222222222222222" />
    );
    expect(a).not.toBe(b);
  });

  it("accepts optional className prop without breaking output", () => {
    const html = renderToStaticMarkup(
      <DeterministicAvatar seed="0xdeadbeef" className="size-8 rounded-full" />
    );
    expect(html).toMatch(/class="size-8 rounded-full"/);
    expect(html).toMatch(/<circle[\s>]/);
  });

  it("handles empty seed gracefully (no crash, still renders SVG)", () => {
    const html = renderToStaticMarkup(<DeterministicAvatar seed="" />);
    expect(html).toMatch(/<svg[\s>]/);
    expect(html).toMatch(/<circle[\s>]/);
  });
});
