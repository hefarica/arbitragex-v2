/**
 * DappBadge — glass-neon header badge (docs/atlas_264 prototype).
 *
 * Pins the operator's two requirements:
 *   1. The QuantumX logo is a data-URI derived from the REAL
 *      `frontend/app/icon.svg` — the sync guard below fails CI the moment the
 *      icon changes without updating the constant, so every card's logo
 *      provably follows icon.svg.
 *   2. LED semantics: "live" → LIVE label (evaluated card), "pending" →
 *      PENDING label (detection diagnostic), both visible as text next to the
 *      pulsing dot (a11y: the dot itself is aria-hidden).
 */
import React from "react";
import { describe, it, expect } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

import {
  DappBadge,
  QUANTUMX_LOGO_DATA_URI,
  QUANTUMX_LOGO_SVG_SOURCE,
} from "../DappBadge";
import { LedIndicator } from "../LedIndicator";

const ICON_SVG_PATH = fileURLToPath(new URL("../../../../app/icon.svg", import.meta.url));

describe("QuantumX logo data-URI — single source of truth is app/icon.svg", () => {
  it("embedded SVG source is verbatim app/icon.svg (sync guard)", () => {
    const iconFile = readFileSync(ICON_SVG_PATH, "utf8");
    expect(QUANTUMX_LOGO_SVG_SOURCE.trim()).toBe(iconFile.trim());
  });

  it("data-URI is a properly encoded svg payload", () => {
    expect(QUANTUMX_LOGO_DATA_URI.startsWith("data:image/svg+xml,")).toBe(true);
    // '#' must be percent-encoded or the URI is truncated at the first color.
    expect(QUANTUMX_LOGO_DATA_URI).not.toContain("#");
    expect(QUANTUMX_LOGO_DATA_URI).toContain(encodeURIComponent("#0C1230"));
  });
});

describe("DappBadge", () => {
  it("renders the full-width badge: logo · label · strategy name · LIVE", () => {
    const html = renderToStaticMarkup(
      <DappBadge label="Evaluada" strategyName="DEX Convergence" led="live" />,
    );
    expect(html).toContain('src="data:image/svg+xml,');
    expect(html).toContain('alt="QuantumX"');
    expect(html).toContain("Evaluada");
    expect(html).toContain("DEX Convergence");
    expect(html).toContain("LIVE");
  });

  it("pending state renders the PENDING label", () => {
    const html = renderToStaticMarkup(
      <DappBadge label="Detección" strategyName="Sin evaluar" led="pending" variant="warn" />,
    );
    expect(html).toContain("Detección");
    expect(html).toContain("Sin evaluar");
    expect(html).toContain("PENDING");
  });

  it("renders a trailing element (Inspect affordance)", () => {
    const html = renderToStaticMarkup(
      <DappBadge
        label="Evaluada"
        strategyName="DEX Convergence"
        led="live"
        trailing={<button aria-label="Inspect details">i</button>}
      />,
    );
    expect(html).toContain('aria-label="Inspect details"');
  });
});

describe("LedIndicator", () => {
  it("renders the pulsing dot as decorative (aria-hidden) for both states", () => {
    for (const state of ["live", "pending"] as const) {
      const html = renderToStaticMarkup(<LedIndicator state={state} />);
      expect(html).toContain('aria-hidden="true"');
    }
  });
});
