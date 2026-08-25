// frontend/app/strategies/tabs/__tests__/DetectorDetailDrawer.test.tsx
//
// FE-MASTER · FE-0025 — detector detail drawer, SSR-branch tests.
//
// The pure core (DetectorDetailBody) renders the FULL 14-key canon record of
// one detector family verbatim: the EXACT workbook sentences, the family hop
// envelope with the intersection note, the display-only frontend_config
// phrases (d9 amendment — never a derived knob spec), and the honest-gap
// block (per-detector runtime state is NOT in the static wire). The Sheet
// wrapper owns open/close and stays untested here by design (Radix portal,
// node env — same split as StrategyDetailDrawer/PairDetailDrawer).
import React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import { DetectorDetailBody } from "../DetectorDetailDrawer";
import type { DetectorPolicyView } from "@/lib/apex/schemas";

const ROW: DetectorPolicyView = {
  detector_id: "R_CLOSED_CYCLE",
  strategies_count: 12,
  example_surface: "dex-amm pool graph",
  example_mev: "MEV-01-015",
  execution_class: "DETERMINISTIC_EXECUTABLE",
  primary_ops: ["op_27 Path Ordering", "op_16 Kelly Criterion"],
  secondary_ops: ["op_05 Kalman Filter"],
  exact_discovery_criterion: "enumerate closed cycles over the effective universe",
  required_data: "reserves per pool (on-chain), fee_bps per venue",
  frontend_config: ["solver timeout", "reserve safety floor"],
  graph_policy: "EXHAUSTIVE_2",
  hop_envelope: { min: 2, max: 7 },
  hot_seed: "SEED_CANDIDATE",
  do_not_do: "never assume a route is executable without simulation",
};

function render(row: DetectorPolicyView = ROW) {
  return renderToStaticMarkup(React.createElement(DetectorDetailBody, { row }));
}

describe("DetectorDetailBody — SSR branches (FE-0025 · §25)", () => {
  it("renders the canon record verbatim: identity, count, graph policy, envelope", () => {
    const html = render();
    expect(html).toContain("R_CLOSED_CYCLE");
    expect(html).toContain("dex-amm pool graph");
    expect(html).toContain("MEV-01-015");
    expect(html).toContain("12");
    expect(html).toContain("EXHAUSTIVE_2");
    expect(html).toContain("2–7");
  });

  it("renders the workbook sentences verbatim: criterion, required data, do-not rule", () => {
    const html = render();
    expect(html).toContain("enumerate closed cycles over the effective universe");
    expect(html).toContain("reserves per pool (on-chain), fee_bps per venue");
    expect(html).toContain("never assume a route is executable without simulation");
    expect(html).toContain("NO hacer (DO_NOT_RULES");
  });

  it("hop envelope carries the family-intersection note (backend invariant, FE only displays)", () => {
    const html = render();
    expect(html).toContain("INTERSECTA los min/max_legs");
    expect(html).toContain("nunca escapa a su familia");
  });

  it("hot_seed + execution_class + tier render as badges; SEED_CANDIDATE gets its variant", () => {
    const html = render();
    expect(html).toContain("SEED_CANDIDATE");
    expect(html).toContain("DETERMINISTIC_EXECUTABLE");
    expect(html).toContain("tier executable");
    // The tier fold repeats DP-005 (same TS mirror) — never a second opinion.
    const observe = render({ ...ROW, execution_class: "OBSERVE_ONLY" });
    expect(observe).toContain("tier observation");
  });

  it("a class outside the closed vocabulary shows the honest unknown badge — never a default tier", () => {
    const html = render({ ...ROW, execution_class: "QUANTUM_SUPERPOSED" });
    expect(html).toContain("tier desconocido (drift)");
    expect(html).not.toMatch(/tier (observation|signal|candidate|executable)</);
  });

  it("frontend_config renders the EXACT phrases display-only — the amendment note is present", () => {
    const html = render();
    expect(html).toContain("solver timeout");
    expect(html).toContain("reserve safety floor");
    expect(html).toContain("Frases EXACTAS del workbook");
    expect(html).toContain("jamás se derivan tipos ni unidades");
    // Nothing masquerading as a knob spec (no key:unit fabrication).
    expect(html).not.toMatch(/solver timeout[^<]*:/);
  });

  it("ops render as chips: primary and secondary, empty secondary renders the dash", () => {
    const html = render();
    expect(html).toContain("op_27 Path Ordering");
    expect(html).toContain("op_16 Kelly Criterion");
    expect(html).toContain("op_05 Kalman Filter");
    const empty = render({ ...ROW, secondary_ops: [] });
    expect(empty).toContain("—");
  });

  it("honest gaps: per-detector runtime state and the member-strategy join are listed as NOT emitted", () => {
    const html = render();
    expect(html).toContain("No emitido en el catálogo");
    expect(html).toContain("detecciones/h");
    expect(html).toContain("Workbook 264");
    // Nothing masquerading as a runtime value.
    expect(html).not.toMatch(/p95[^<]*\d+\s*ms/);
  });
});
