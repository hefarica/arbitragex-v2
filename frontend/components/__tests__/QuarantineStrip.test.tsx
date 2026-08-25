/**
 * FE-0031 (§30) — QuarantineStrip render contract.
 *
 * The strip is the VISIBLE quarantine marker: empty violations render
 * NOTHING (clean rows stay visually clean); any violation renders a
 * role="alert" strip carrying the QUARANTINED label plus the exact codes.
 * Quarantine is never a hidden state (§30) — this pins both arms.
 */
import React from "react";
import { describe, it, expect } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";

import { QuarantineStrip } from "../QuarantineStrip";

describe("QuarantineStrip (§30)", () => {
  it("renders nothing on an empty violation list", () => {
    const html = renderToStaticMarkup(
      <QuarantineStrip violations={[]} />,
    );
    expect(html).toBe("");
  });

  it("renders the QUARANTINED alert with the exact violation codes", () => {
    const html = renderToStaticMarkup(
      <QuarantineStrip
        violations={["missing_strategy_id", "missing_block"]}
      />,
    );
    expect(html).toContain("QUARANTINED");
    expect(html).toContain("missing_strategy_id · missing_block");
    expect(html).toContain('role="alert"');
  });

  it("joins every code — the operator reads the full diagnosis, not a count", () => {
    const html = renderToStaticMarkup(
      <QuarantineStrip
        violations={[
          "no_route_identity",
          "hop_incoherent",
          "profit_not_numeric",
        ]}
      />,
    );
    for (const code of ["no_route_identity", "hop_incoherent", "profit_not_numeric"]) {
      expect(html).toContain(code);
    }
  });
});
