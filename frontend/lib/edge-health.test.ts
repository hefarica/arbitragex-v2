import { describe, it, expect } from "vitest";
import {
  classifyEdgeResult,
  aggregateEdgeHealth,
  type EdgeResult,
} from "./edge-health";

/**
 * Edge health taxonomy tests — locks in the doctrinal distinction
 * between "edge unreachable" and "upstreams degraded" so the spec in
 * tests/e2e/smoke.spec.ts cannot regress.
 */
describe("classifyEdgeResult", () => {
  it("classifies HTTP 5xx as UPSTREAM_DEGRADED (edge IS reachable)", () => {
    const res: EdgeResult<unknown> = {
      ok: false,
      error: "edge HTTP 503: upstream RPC unavailable",
    };
    const h = classifyEdgeResult(res);
    expect(h.kind).toBe("EDGE_REACHABLE_UPSTREAM_DEGRADED");
    expect(h.title).toBe("upstreams degraded");
    expect(h.title).not.toMatch(/edge unreachable/i);
    expect(h.title).not.toMatch(/edge error:/i);
  });

  it("classifies HTTP 4xx as UPSTREAM_DEGRADED", () => {
    const h = classifyEdgeResult({ ok: false, error: "edge HTTP 404: not found" });
    expect(h.kind).toBe("EDGE_REACHABLE_UPSTREAM_DEGRADED");
  });

  it("classifies timeout as UNREACHABLE", () => {
    const h = classifyEdgeResult({ ok: false, error: "edge timeout after 5000ms" });
    expect(h.kind).toBe("EDGE_UNREACHABLE");
    expect(h.title).toBe("edge unreachable");
  });

  it("classifies raw network error as UNREACHABLE", () => {
    const h = classifyEdgeResult({ ok: false, error: "fetch failed" });
    expect(h.kind).toBe("EDGE_UNREACHABLE");
  });

  it("classifies invalid JSON as SCHEMA_DRIFT (not unreachable)", () => {
    const h = classifyEdgeResult({
      ok: false,
      error: "edge returned invalid JSON: Unexpected token <",
    });
    expect(h.kind).toBe("EDGE_REACHABLE_SCHEMA_DRIFT");
    expect(h.title).not.toMatch(/edge unreachable/i);
  });

  it("classifies shape violation as SCHEMA_DRIFT", () => {
    const h = classifyEdgeResult({
      ok: false,
      error: "edge response shape invalid: data: required",
    });
    expect(h.kind).toBe("EDGE_REACHABLE_SCHEMA_DRIFT");
  });

  it("classifies ok result as LIVE", () => {
    const h = classifyEdgeResult({ ok: true, data: { hello: "world" } });
    expect(h.kind).toBe("EDGE_REACHABLE_LIVE");
    expect(h.diagnostic).toBe("");
  });

  // Doctrine guard: the literal banner string the spec forbids must
  // NEVER appear for the most common CI scenario (upstream 503 from
  // the edge gateway because no RPC is configured).
  it("503 upstream is NEVER labelled 'edge unreachable'", () => {
    const h = classifyEdgeResult({
      ok: false,
      error: "edge HTTP 503: upstream RPC missing",
    });
    expect(h.title).not.toBe("edge unreachable");
    expect(h.title).toBe("upstreams degraded");
  });
});

describe("aggregateEdgeHealth", () => {
  it("returns LIVE when all results are ok", () => {
    const h = aggregateEdgeHealth([
      { ok: true, data: 1 },
      { ok: true, data: 2 },
    ]);
    expect(h.kind).toBe("EDGE_REACHABLE_LIVE");
  });

  it("UNREACHABLE dominates UPSTREAM_DEGRADED", () => {
    const h = aggregateEdgeHealth([
      { ok: false, error: "edge HTTP 503: blah" },
      { ok: false, error: "edge timeout after 5000ms" },
    ]);
    expect(h.kind).toBe("EDGE_UNREACHABLE");
  });

  it("SCHEMA_DRIFT dominates UPSTREAM_DEGRADED", () => {
    const h = aggregateEdgeHealth([
      { ok: false, error: "edge HTTP 503: blah" },
      { ok: false, error: "edge returned invalid JSON: x" },
    ]);
    expect(h.kind).toBe("EDGE_REACHABLE_SCHEMA_DRIFT");
  });

  it("UPSTREAM_DEGRADED wins when no UNREACHABLE/SCHEMA_DRIFT present", () => {
    const h = aggregateEdgeHealth([
      { ok: true, data: 1 },
      { ok: false, error: "edge HTTP 503: blah" },
    ]);
    expect(h.kind).toBe("EDGE_REACHABLE_UPSTREAM_DEGRADED");
  });

  it("CI scenario — both upstreams 503 — surfaces 'upstreams degraded', NOT 'edge unreachable'", () => {
    const h = aggregateEdgeHealth([
      { ok: false, error: "edge HTTP 503: status upstream" },
      { ok: false, error: "edge HTTP 503: recon upstream" },
    ]);
    expect(h.kind).toBe("EDGE_REACHABLE_UPSTREAM_DEGRADED");
    expect(h.title).toBe("upstreams degraded");
    expect(h.title).not.toMatch(/edge unreachable/i);
    expect(h.title).not.toMatch(/edge error:/i);
  });

  it("empty input returns LIVE (no signal of failure)", () => {
    const h = aggregateEdgeHealth([]);
    expect(h.kind).toBe("EDGE_REACHABLE_LIVE");
  });
});
