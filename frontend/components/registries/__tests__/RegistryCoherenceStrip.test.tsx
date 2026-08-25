// frontend/components/registries/__tests__/RegistryCoherenceStrip.test.tsx
//
// FE-0040 (§56/§57) — the total-coherence strip. The verdict is wire-owned:
// presence vs absence of engine drift observations IS the verdict (§79 — the
// FE never recomputes hashes). R8: an empty list with a backend `reason`
// (table absent) or a poll failure renders NO COMPUTADO, never COHERENT.
import React from "react";
import { describe, it, expect } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";

import {
  RegistryCoherenceStrip,
  coherenceVerdict,
  layerGroup,
} from "../RegistryCoherenceStrip";
import type { DriftObservation } from "@/lib/registries/types-omni";

const obs = (over: Partial<DriftObservation>): DriftObservation => ({
  id: "0f0e0d0c-0b0a-4009-8008-706050403020",
  resource: "chain",
  chain_id: 1,
  layer_a: "postgresql",
  layer_b: "redis_pubsub",
  hash_a: "a".repeat(64),
  hash_b: "b".repeat(64),
  diff_count: 3,
  diff_summary: {},
  severity: "warn",
  observed_at: "2026-08-24T00:00:00Z",
  resolved_at: null,
  ...over,
});

function strip(over: Partial<Parameters<typeof RegistryCoherenceStrip>[0]>): string {
  return renderToStaticMarkup(
    React.createElement(RegistryCoherenceStrip, {
      resource: "chain",
      observations: [],
      pollError: null,
      reason: null,
      loading: false,
      frontendRows: null,
      frontendRefreshedAt: null,
      ...over,
    }),
  );
}

describe("coherenceVerdict (pure)", () => {
  it("empty + genuinely answered → COHERENT", () => {
    expect(coherenceVerdict([], null, null)).toEqual({ kind: "COHERENT" });
  });

  it("≥1 observation → DRIFT with count", () => {
    expect(coherenceVerdict([obs({})], null, null)).toEqual({ kind: "DRIFT", count: 1 });
  });

  it("poll failure blocks the COHERENT claim (R8)", () => {
    expect(coherenceVerdict([], "HTTP 502", null)).toEqual({
      kind: "NOT_COMPUTED",
      why: "poll_failed: HTTP 502",
    });
  });

  it("backend structural reason blocks the COHERENT claim — table absent ≠ consistent", () => {
    expect(coherenceVerdict([], null, "drift_observations_table_absent")).toEqual({
      kind: "NOT_COMPUTED",
      why: "drift_observations_table_absent",
    });
    // poll error wins over reason when both are present
    expect(coherenceVerdict([], "HTTP 500", "drift_observations_table_absent")).toEqual({
      kind: "NOT_COMPUTED",
      why: "poll_failed: HTTP 500",
    });
  });

  it("observations win over a stale reason (divergence never hidden)", () => {
    expect(coherenceVerdict([obs({})], null, "some_reason")).toEqual({
      kind: "DRIFT",
      count: 1,
    });
  });
});

describe("layerGroup (pure)", () => {
  it("maps every wire vocabulary to the four canonical groups + TOML", () => {
    expect(layerGroup("postgresql")).toBe("DB");
    expect(layerGroup("persistence")).toBe("DB");
    expect(layerGroup("redis")).toBe("Redis");
    expect(layerGroup("redis_pubsub")).toBe("Redis");
    expect(layerGroup("runtime")).toBe("Runtime");
    expect(layerGroup("searcher_rs")).toBe("Runtime");
    expect(layerGroup("arc_swap")).toBe("Runtime");
    expect(layerGroup("frontend")).toBe("Frontend");
    expect(layerGroup("frontend_refresh")).toBe("Frontend");
    expect(layerGroup("toml")).toBe("TOML");
  });

  it("unknown layer stays verbatim — never dropped (§28)", () => {
    expect(layerGroup("celer_bridge")).toBe("celer_bridge");
  });
});

describe("RegistryCoherenceStrip (§56)", () => {
  it("COHERENT: 0 unresolved observations, query genuinely answered", () => {
    const html = strip({});
    expect(html).toContain('data-testid="coherence-verdict"');
    expect(html).toContain(">COHERENT<");
    expect(html).toContain("0 observaciones sin resolver");
    // the §79 disclaimer rides with the coherent verdict
    expect(html).toContain("el FE no recomputa hashes (§79)");
  });

  it("NO COMPUTADO (reason): table absent is never COHERENT (R8)", () => {
    const html = strip({ reason: "drift_observations_table_absent" });
    expect(html).toContain("NO COMPUTADO");
    expect(html).toContain("drift_observations_table_absent");
    expect(html).toContain("Ausencia de chequeo NO es COHERENT");
    expect(html).not.toContain(">COHERENT<");
  });

  it("NO COMPUTADO (poll failed): honest HTTP error surfaced verbatim", () => {
    const html = strip({ pollError: "HTTP 502" });
    expect(html).toContain("poll_failed: HTTP 502");
    expect(html).not.toContain(">COHERENT<");
  });

  it("DRIFT: count + per-observation rows with layer pair, hash pair, diffs", () => {
    const html = strip({
      observations: [
        obs({ severity: "warn", diff_count: 3 }),
        obs({
          id: "0f0e0d0c-0b0a-4009-8008-706050403021",
          layer_a: "searcher_rs",
          layer_b: "frontend",
          severity: "error",
          diff_count: 7,
        }),
      ],
    });
    expect(html).toContain(">DRIFT · 2<");
    expect(html).toContain("2 observación(es) sin resolver");
    // row 1: canonical pair + shortened hashes (8…6 elision of 64-hex)
    expect(html).toContain("postgresql ↔ redis_pubsub");
    expect(html).toContain("aaaaaaaa…aaaaaa");
    expect(html).toContain("≠");
    expect(html).toContain("bbbbbbbb…bbbbbb");
    expect(html).toContain("3 diffs");
    // row 2: runtime ↔ frontend flagging both chips
    expect(html).toContain("searcher_rs ↔ frontend");
    expect(html).toContain("7 diffs");
    expect(html).not.toContain(">COHERENT<");
  });

  it("chips: flagged layers turn destructive; unflagged stay neutral", () => {
    const html = strip({ observations: [obs({})] }); // DB ↔ Redis
    const dbChip = html.split('data-testid="coherence-chip-DB"')[1]?.split("</span>")[0] ?? "";
    const redisChip = html.split('data-testid="coherence-chip-Redis"')[1]?.split("</span>")[0] ?? "";
    const runtimeChip = html.split('data-testid="coherence-chip-Runtime"')[1]?.split("</span>")[0] ?? "";
    const feChip = html.split('data-testid="coherence-chip-Frontend"')[1]?.split("</span>")[0] ?? "";
    expect(dbChip).toContain("border-destructive/40");
    expect(redisChip).toContain("border-destructive/40");
    // Runtime/Frontend not named by this observation → neutral, NOT "verified"
    expect(runtimeChip).not.toContain("border-destructive/40");
    expect(feChip).not.toContain("border-destructive/40");
    expect(runtimeChip).toContain("border-border");
  });

  it("unknown layer gets its own verbatim chip — nothing dropped (§28)", () => {
    const html = strip({
      observations: [obs({ layer_a: "celer_bridge", layer_b: "redis" })],
    });
    expect(html).toContain('data-testid="coherence-chip-celer_bridge"');
  });

  it("Frontend chip renders the FE's own view as VALUES (rows / sin filas)", () => {
    const withRows = strip({ frontendRows: 5, frontendRefreshedAt: "2026-08-24T00:00:00Z" });
    expect(withRows).toContain("Frontend · 5 filas · refrescado");
    const none = strip({});
    expect(none).toContain("Frontend · sin filas · —");
  });

  it("first check in flight → PRIMER CHEQUEO, no premature verdict", () => {
    const html = strip({ loading: true });
    expect(html).toContain("PRIMER CHEQUEO");
    expect(html).not.toContain(">COHERENT<");
    expect(html).not.toContain("NO COMPUTADO");
  });

  it("declares the nivel-(b) gap: DriftReport v2 endpoint not emitted by backend", () => {
    const html = strip({});
    expect(html).toContain("/api/v1/drift/status");
    expect(html).toContain("nivel-(b)");
    expect(html).toContain("/api/system/drift");
  });

  it("R1: pure render is byte-identical across invocations", () => {
    const props = { observations: [obs({})], frontendRows: 2 };
    expect(strip(props)).toBe(strip(props));
  });
});
