/**
 * CrossChainSlot tests using react-dom/server renderToStaticMarkup.
 *
 * Why not @testing-library/react:
 *   - Not installed; requires jsdom + operator approval (toolkit-alignment memory).
 *   - CrossChainSlot is a pure component with no hooks, no events.
 *   - renderToStaticMarkup validates null-return and chain/bridge rendering.
 *
 * 2 tests:
 *   1. Returns null (renders as "") when chain_id_out is null.
 *   2. Renders bridge name + source/dest chain labels when chain_id_out is set.
 */
import React from "react";
import { describe, it, expect } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";
import { CrossChainSlot } from "../CrossChainSlot";

describe("CrossChainSlot", () => {
  it("renders empty (returns null) when chain_id_out is null", () => {
    const html = renderToStaticMarkup(
      <CrossChainSlot opp={{ chain_id: 1, chain_id_out: null, bridge: null }} />
    );
    // React renders `null` as empty string in renderToStaticMarkup.
    expect(html).toBe("");
  });

  it("renders bridge + chain names when chain_id_out is set", () => {
    const html = renderToStaticMarkup(
      <CrossChainSlot opp={{ chain_id: 1, chain_id_out: 42161, bridge: "across" }} />
    );
    expect(html).toContain("across");
    // chain_id=1 → "ETH", chain_id_out=42161 → "ARB"
    expect(html).toContain("ETH");
    expect(html).toContain("ARB");
  });
});
