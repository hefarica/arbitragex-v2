// frontend/components/__tests__/RuntimeSettingState.test.tsx
//
// FE-MASTER · FE-0044 (§73) — RuntimeSettingState, direct SSR-branch tests.
//
// The reusable one-line setting state (FE-0005, §3/§14/§64). Until now it was
// only exercised THROUGH the UniverseSaveCoherency wrapper (3 steady states);
// these tests pin the component's OWN rendering contract:
//   - role=status line: label → configured → effective ("—" when null, R8);
//   - version chip: v{configured}→{effective}, "→—" when not served, absent
//     entirely when no version pair is passed;
//   - steady-state badge truth table reachable without effects (EFFECTIVE /
//     CONFIGURED / NOT_EXPOSED / DRIFT §47) each carrying its §40
//     explainability hint verbatim in the badge title.
// The mutation lifecycle (WAITING_ACK/APPLIED/VERIFIED/REJECTED/TIMEOUT via
// useRuntimeAckSocket) is FE-0046 territory — the socket hook never fires in
// a static render, and fabricating its firing here would test a mock, not
// the wire (RULE 00).
import React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import { RuntimeSettingState } from "../RuntimeSettingState";

const BASE = {
  label: "min profit",
  configured: 25,
  effective: 25,
  ackEventId: null,
} as const;

describe("RuntimeSettingState — steady states, direct component (§73)", () => {
  it("renders the status line: label → configured → effective, role=status", () => {
    const html = renderToStaticMarkup(<RuntimeSettingState {...BASE} />);
    expect(html).toContain('role="status"');
    expect(html).toContain("min profit");
    expect(html).toContain(">25</span>");
    // No version pair passed → no version chip.
    expect(html).not.toContain("configured version → effective version");
    expect(html).not.toContain("—");
  });

  it("effective === configured (no version) → EFFECTIVE with its §40 hint", () => {
    const html = renderToStaticMarkup(<RuntimeSettingState {...BASE} />);
    expect(html).toContain("EFFECTIVE");
    expect(html).toContain('title="Runtime reports exactly the configured value."');
  });

  it("effective ≠ configured → CONFIGURED (saved, not converged), dash nowhere", () => {
    const html = renderToStaticMarkup(
      <RuntimeSettingState {...BASE} effective={50} />,
    );
    expect(html).toContain("CONFIGURED");
    expect(html).toContain('title="Saved. Runtime has not reported convergence with this value."');
    // Both chips render their own value — the divergence is the signal.
    expect(html).toContain(">25</span>");
    expect(html).toContain(">50</span>");
    expect(html).not.toContain("—");
  });

  it("effective null → NOT EXPOSED with the honest dash (R8: never zero)", () => {
    const html = renderToStaticMarkup(
      <RuntimeSettingState {...BASE} effective={null} />,
    );
    expect(html).toContain("NOT EXPOSED");
    expect(html).toContain('title="Runtime has not served this value — not computed, never zero."');
    expect(html).toContain(">—</span>");
  });

  it("version disagree → DRIFT (§47) with the version chip v{put}→{served}", () => {
    const html = renderToStaticMarkup(
      <RuntimeSettingState
        {...BASE}
        version={{ configured: 4, effective: 3 }}
      />,
    );
    expect(html).toContain("DRIFT");
    expect(html).toContain('title="Configured and effective versions disagree."');
    expect(html).toContain("v4→3");
    expect(html).toContain('title="configured version → effective version (null = not served)"');
  });

  it("version served null → chip shows v{put}→— (not served yet, R8)", () => {
    const html = renderToStaticMarkup(
      <RuntimeSettingState
        {...BASE}
        version={{ configured: 7, effective: null }}
      />,
    );
    expect(html).toContain("v7→—");
  });

  it("values agree and versions agree → EFFECTIVE with the version pair", () => {
    const html = renderToStaticMarkup(
      <RuntimeSettingState
        {...BASE}
        version={{ configured: 9, effective: 9 }}
      />,
    );
    expect(html).toContain("EFFECTIVE");
    expect(html).toContain("v9→9");
  });

  it("versions agree but values differ → CONFIGURED (versions prove no DRIFT, not convergence)", () => {
    // §47: version agreement only rules OUT drift; EFFECTIVE still requires
    // the served value to equal the configured one.
    const html = renderToStaticMarkup(
      <RuntimeSettingState
        {...BASE}
        effective={30}
        version={{ configured: 9, effective: 9 }}
      />,
    );
    expect(html).toContain("CONFIGURED");
    expect(html).toContain("v9→9");
    expect(html).not.toContain("DRIFT");
  });
});
