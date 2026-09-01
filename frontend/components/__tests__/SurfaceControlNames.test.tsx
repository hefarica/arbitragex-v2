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
// The Radix Switch BubbleInput mirror and the catalog checkboxes (fetch-gated)
// are verified on the live deploy re-probe — renderToStaticMarkup runs no
// effects and the form renders null until mounted.
import React from "react";
import { describe, it, expect } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";

import { Phase3Client } from "@/features/onboarding/Phase3Client";
import { Dropzone } from "@/components/ui/dropzone";
import { Switch } from "@/components/ui/switch";
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

describe("Switch — aria-label passthrough (the effect mirrors it onto Radix's hidden form proxy)", () => {
  it("forwards aria-label to the root button", () => {
    const html = renderToStaticMarkup(<Switch aria-label="Compact density" />);
    expect(html).toContain('aria-label="Compact density"');
  });
});
