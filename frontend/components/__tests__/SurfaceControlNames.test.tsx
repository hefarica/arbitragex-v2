// frontend/components/__tests__/SurfaceControlNames.test.tsx
//
// DAPP-SURFACE (2026-09-01, Holy Grail workbook c719f4e9): the 5 PARTIAL
// surfaces failed Responsive/A11y because their interactive controls had no
// accessible name under the auditor's exact predicate:
//   aria-label || aria-labelledby || (el.id && label[for=id]) || innerText.trim()
// An implicit wrapping <label> does NOT satisfy it (input has no innerText and
// no id), and hidden file inputs count too (hidden ≠ aria-hidden).
//
// These tests pin the markup contract for the statically-renderable pieces.
// The Radix Switch BubbleInput mirror was replaced (2026-09-01B) by a
// self-contained Switch that renders no proxy input at all; the catalog
// checkboxes (fetch-gated) are still verified on the live deploy re-probe —
// renderToStaticMarkup runs no effects and the form renders null until mounted.
import React from "react";
import { describe, it, expect } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";
import fs from "node:fs";

import { Phase3Client } from "@/features/onboarding/Phase3Client";
import { Dropzone } from "@/components/ui/dropzone";
import { Switch } from "@/components/ui/switch";
import { ThemeOverrideSelect } from "@/components/settings/ThemeOverrideSelect";
import type { OnboardingStatus } from "@/lib/schemas";

const status: OnboardingStatus = {
  org_id: "test-org",
  phase_1_completed_at: "2026-08-31T00:00:00Z",
  phase_1_completed_by: "op",
  phase_1_vault_sealed_healthy: true,
  phase_2_completed_at: "2026-08-31T00:00:00Z",
  phase_2_completed_by: "op",
  phase_2_rpc_probe_ok: true,
  phase_3_completed_at: null,
  phase_3_completed_by: null,
  phase_4_completed_at: null,
  phase_4_completed_by: null,
  phase_4_signer_zero_balance_verified: false,
  phase_5_completed_at: null,
  phase_5_completed_by: null,
  phase_5_paper_mode_off_at: null,
  created_at: "2026-08-31T00:00:00Z",
  updated_at: "2026-08-31T00:00:00Z",
};

describe("Phase3Client — sim_provider radios (was the /onboarding/3-advanced REGRESSED)", () => {
  const html = renderToStaticMarkup(<Phase3Client initialSnapshot={status} />);

  it("every sim_provider radio has an explicit id linked to its label via htmlFor", () => {
    for (const p of ["anvil", "tenderly"]) {
      expect(html).toContain(`id="sim_provider_${p}"`);
      expect(html).toContain(`for="sim_provider_${p}"`);
    }
  });

  it("still renders the visible provider text (the name is real, not a decorative id)", () => {
    expect(html).toContain("<code>anvil</code>");
    expect(html).toContain("<code>tenderly</code>");
  });
});

describe("Dropzone — hidden file input (was a /strategies/forge hit)", () => {
  it("carries an aria-label naming the accepted script types", () => {
    const html = renderToStaticMarkup(<Dropzone accept={["rhai"]} onChange={() => {}} />);
    expect(html).toContain('aria-label="Upload strategy script file (.rhai)"');
  });

  it("adapts the label to the configured accept list", () => {
    const html = renderToStaticMarkup(<Dropzone accept={["wasm"]} onChange={() => {}} />);
    expect(html).toContain('aria-label="Upload strategy script file (.wasm)"');
  });
});

describe("Switch — self-contained (2026-09-01B): no proxy input at any instant", () => {
  it("forwards aria-label to the root button", () => {
    const html = renderToStaticMarkup(<Switch aria-label="Compact density" />);
    expect(html).toContain('aria-label="Compact density"');
  });

  it("renders a real button[role=switch][type=button] and ZERO input elements — even inside a form", () => {
    // Radix's BubbleInput proxy (aria-hidden checkbox, no accessible name,
    // rendered by default at SSR) is what the surface census counted as the
    // unlabeled control on /settings and /config/trading. This fork must
    // never emit ANY <input>.
    const html = renderToStaticMarkup(
      <form onSubmit={() => undefined}>
        <label htmlFor="enabled">Enabled</label>
        <Switch id="enabled" checked onCheckedChange={() => undefined} />
      </form>
    );
    expect(html).toContain('role="switch"');
    expect(html).toContain('type="button"');
    expect(html).toContain('aria-checked="true"');
    expect(html).not.toContain("<input");
    expect(html).not.toContain("<select");
  });

  it("labels by id + label[for] without aria-label (trading-config 'enabled' pattern)", () => {
    const html = renderToStaticMarkup(
      <>
        <label htmlFor="sw1">Atomic execution</label>
        <Switch id="sw1" />
      </>
    );
    expect(html).toContain('id="sw1"');
    expect(html).toContain('role="switch"');
    expect(html).not.toContain("<input");
  });
});

describe("ThemeOverrideSelect — native select labeled at SSR", () => {
  it("renders <option> children and id at SSR (innerText non-empty at every instant)", () => {
    // Regression for the /settings Radix hidden native-select proxy that had
    // no accessible name during SSR/pre-hydration.
    const html = renderToStaticMarkup(
      <>
        <label htmlFor="theme_override">Theme override</label>
        <ThemeOverrideSelect value="system" onChange={() => undefined} />
      </>
    );
    expect(html).toContain('id="theme_override"');
    expect(html).toContain('<option value="system"');
    expect(html).toContain('<option value="dark"');
    expect(html).not.toContain("aria-hidden");
  });
});

// DAPP-A11Y (2026-09-02, workbook 20260902_152349Z): the auditor counted
// unlabeled=1 on /monitor — the icon-only reconnect <Button> that renders in
// SSR whenever the Socket.IO stream starts disconnected (initial state), then
// disappears once connected. That instant is exactly what a load-time census
// sees. Pin the accessible name so the census passes at EVERY lifecycle
// instant, not just the settled one.
import { MetricsStream } from "@/app/monitor/components/MetricsStream";

describe("MetricsStream — reconnect icon button (the /monitor unlabeled=1)", () => {
  const html = renderToStaticMarkup(<MetricsStream />);

  it("disconnected initial state renders the reconnect control at all (SSR census sees it)", () => {
    expect(html).toContain("Reconectar stream de métricas");
  });

  it("the icon-only button carries an explicit accessible name", () => {
    expect(html).toMatch(/<button[^>]*aria-label="Reconectar stream de métricas"[^>]*>/);
  });
});

// ── 48_SURFACE_CERT Responsive overflow closures (2026-09-02, post-deploy sweep) ──
describe("Alert grid tracks + Strategies tabs wrap (the /readiness + /strategies overflow)", () => {
  const read = (f: string) => fs.readFileSync(f, "utf8");

  it("ui/alert.tsx uses minmax(0,1fr) tracks — a bare 1fr is minmax(auto,1fr) whose min-content blows out on unbreakable titles", () => {
    const src = read("components/ui/alert.tsx");
    expect(src).toContain("grid-cols-[calc(var(--spacing)*4)_minmax(0,1fr)]");
    expect(src).toContain("grid-cols-[0_minmax(0,1fr)]");
    expect(src).not.toContain("_1fr]");
  });

  it("StrategiesClient TabsList wraps — 11 triggers at w-fit overflow every viewport < ~1280px", () => {
    const src = read("app/strategies/StrategiesClient.tsx");
    expect(src).toMatch(/<TabsList className="h-auto flex-wrap justify-start">/);
  });

  it("readiness CardTitle is min-w-0 — the grid ITEM (not the span) holds CardHeader's implicit auto track at the nowrap span's max-content", () => {
    const src = read("app/readiness/page.tsx");
    expect(src).toContain('<CardTitle className="min-w-0 flex items-center justify-between text-base gap-2">');
  });
});
