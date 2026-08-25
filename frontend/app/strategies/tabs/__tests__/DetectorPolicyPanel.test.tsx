// frontend/app/strategies/tabs/__tests__/DetectorPolicyPanel.test.tsx
//
// FE-MASTER · ARBX-DP-005 — Detector Policy panel, SSR-branch tests.
//
// Node env (no jsdom): renderToStaticMarkup over the pure render branches,
// same seam as PairIntelligencePanel.test.tsx — `useOmniStore` mocked at the
// module boundary (zustand v5 SSR reads INITIAL state, so the mock IS the
// state). The tier fold itself is pinned by lib/apex/__tests__/signal-tier
// .test.ts against the REAL generated catalog; here the fixture is synthetic
// to exercise the panel's own branches: doctrine ordering, per-tier counts,
// the honest unknown bucket, and the R8 null/error states.
import React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { beforeEach, describe, expect, it, vi } from "vitest";

const storeState = vi.hoisted(() => ({ current: {} as Record<string, unknown> }));

vi.mock("@/lib/store/omni-store", () => ({
  useOmniStore: (selector: (s: Record<string, unknown>) => unknown) =>
    selector(storeState.current),
}));

import { DetectorPolicyPanel } from "../DetectorPolicyPanel";
import type { DetectorPolicyView } from "@/lib/apex/schemas";

/** Full DetectorPolicyView-shaped row (field-name drift fails loudly here). */
function row(partial: Partial<DetectorPolicyView>): DetectorPolicyView {
  return {
    detector_id: "D_TEST",
    strategies_count: 3,
    example_surface: "cex-dex book",
    example_mev: "MEV-01-001",
    execution_class: "DETERMINISTIC_EXECUTABLE",
    primary_ops: ["op_01"],
    secondary_ops: [],
    exact_discovery_criterion: "criterion sentence",
    required_data: "reserves",
    frontend_config: ["solver timeout"],
    graph_policy: "EXHAUSTIVE_2",
    hop_envelope: { min: 2, max: 3 },
    hot_seed: "OBSERVE_EVIDENCE",
    do_not_do: "do not",
    ...partial,
  };
}

const OBS = row({ detector_id: "D_OBS", execution_class: "OBSERVE_ONLY" });
const SIG = row({ detector_id: "D_SIG", execution_class: "EXTERNAL_DATA_REQUIRED" });
const CAND = row({ detector_id: "D_CAND", execution_class: "DETERMINISTIC_IF_ADAPTER" });
const EXE = row({ detector_id: "D_EXE", execution_class: "DETERMINISTIC_EXECUTABLE" });
const MYSTERY = row({ detector_id: "D_MYST", execution_class: "QUANTUM_SUPERPOSED" });

function seed(partial: {
  catalog?: DetectorPolicyView[] | null;
  status?: string;
  error?: string | null;
}) {
  storeState.current = {
    detectorCatalog: partial.catalog ?? null,
    detectorCatalogStatus: partial.status ?? "ready",
    detectorCatalogError: partial.error ?? null,
    detectorCatalogUpdatedAt: partial.catalog ? "2026-08-24T00:00:00.000Z" : null,
    fetchDetectorCatalog: vi.fn(),
  };
}

function render() {
  return renderToStaticMarkup(React.createElement(DetectorPolicyPanel));
}

beforeEach(() => {
  seed({ catalog: null, status: "idle" });
});

describe("DetectorPolicyPanel — SSR branches (ARBX-DP-005 · P7 §25)", () => {
  it("renders the four tier feeds in doctrine order (observation → executable)", () => {
    seed({ catalog: [OBS, SIG, CAND, EXE] });
    const html = render();
    const i = (t: string) => html.indexOf(`data-tier="${t}"`);
    expect(i("observation")).toBeGreaterThanOrEqual(0);
    expect(i("signal")).toBeGreaterThan(i("observation"));
    expect(i("candidate")).toBeGreaterThan(i("signal"));
    expect(i("executable")).toBeGreaterThan(i("candidate"));
  });

  it("renders payload values verbatim: ids, classes, hot_seed, example_mev, counts", () => {
    seed({ catalog: [OBS, SIG, CAND, EXE] });
    const html = render();
    expect(html).toContain("D_OBS");
    expect(html).toContain("OBSERVE_ONLY");
    expect(html).toContain("EXTERNAL_DATA_REQUIRED");
    expect(html).toContain("DETERMINISTIC_IF_ADAPTER");
    expect(html).toContain("DETERMINISTIC_EXECUTABLE");
    expect(html).toContain("OBSERVE_EVIDENCE");
    expect(html).toContain("MEV-01-001");
    expect(html).toContain(">3</td>"); // strategies_count, no recomputation
  });

  it("each tier card carries its own count (1 detector per tier here)", () => {
    seed({ catalog: [OBS, SIG, CAND, EXE] });
    const html = render();
    expect(html).toContain("1 detector");
    expect(html).not.toContain("1 detectores");
  });

  it("a class outside the closed vocabulary lands in the honest UNKNOWN bucket — never a default tier", () => {
    seed({ catalog: [OBS, SIG, CAND, EXE, MYSTERY] });
    const html = render();
    expect(html).toContain('data-tier="unknown"');
    expect(html).toContain("QUANTUM_SUPERPOSED");
    // "1 detector" appears in all FIVE headers: the four canonical buckets
    // (each still holding exactly their own row — MYSTERY was NOT absorbed
    // into any of them) plus the unknown bucket itself.
    expect((html.match(/1 detector/g) ?? []).length).toBe(5);
  });

  it("an empty tier renders the honest zero message, not a fabricated row", () => {
    seed({ catalog: [OBS] }); // only observation populated
    const html = render();
    expect(html).toContain("Sin detectores en este tier");
  });

  it("null catalog (never served) renders the honest dash — no tier cards", () => {
    seed({ catalog: null, status: "idle" });
    const html = render();
    expect(html).toContain("—");
    expect(html).not.toContain('data-tier=');
  });

  it("error state renders the endpoint reason verbatim", () => {
    seed({ catalog: null, status: "error", error: "HTTP 503: catalog not generated" });
    const html = render();
    expect(html).toContain("HTTP 503: catalog not generated");
    expect(html).toContain('role="alert"');
  });

  // FE-0025: every tier row opens the §25 detail drawer — the affordance
  // (cursor + per-row title) must be present on ALL rows, and the drawer
  // itself is a controlled Sheet that stays closed in SSR (no selected row).
  it("FE-0025: tier rows carry the drawer affordance on every row", () => {
    seed({ catalog: [OBS, SIG, CAND, EXE, MYSTERY] });
    const html = render();
    for (const id of ["D_OBS", "D_SIG", "D_CAND", "D_EXE"]) {
      expect(html).toContain(`title="Detalle ${id} (§25)"`);
    }
    expect((html.match(/cursor-pointer/g) ?? []).length).toBe(4);
  });
});
