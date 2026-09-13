import { describe, it, expect, vi, beforeEach } from "vitest";
import { readFileSync } from "node:fs";
import type pg from "pg";
import { verifyCircuitBreakerInputs } from "./circuit-breaker-inputs.js";
import { verifyGRPC1 } from "./g-rpc-1.js";
import { verifyGSIM1 } from "./g-sim-1.js";
import { verifyGTOK1 } from "./g-tok-1.js";

// Test-only probe outcomes; production uses the original live verifiers.
vi.mock("./g-rpc-1.js", () => ({ verifyGRPC1: vi.fn() }));
vi.mock("./g-sim-1.js", () => ({ verifyGSIM1: vi.fn() }));
vi.mock("./g-tok-1.js", () => ({ verifyGTOK1: vi.fn() }));
const rpc = vi.mocked(verifyGRPC1), sim = vi.mocked(verifyGSIM1), token = vi.mocked(verifyGTOK1);
const item = (id: string, status: "red" | "yellow" | "green") => ({
  id, status, reason: "test observation", group: "risk_doctrines" as const,
  label: id, verified_at: "2026-09-13T00:00:00Z",
});
beforeEach(() => {
  vi.clearAllMocks();
  rpc.mockResolvedValue(item("G-RPC-1", "red"));
  sim.mockResolvedValue(item("G-SIM-1", "yellow"));
  token.mockResolvedValue(item("G-TOK-1", "red"));
});
describe("breaker-only readiness dependency graph", () => {
  it("preserves all three actual outcomes, without full go/no-go certification", async () => {
    const result = await verifyCircuitBreakerInputs({ pool: null });
    expect(result.items.map(i => [i.id, i.status])).toEqual([
      ["G-RPC-1", "red"], ["G-SIM-1", "yellow"], ["G-TOK-1", "red"],
    ]);
    expect(Object.keys(result)).toEqual(["items"]);
    expect(rpc).toHaveBeenCalledTimes(1);
    expect(sim).toHaveBeenCalledWith({ pool: null });
    expect(token).toHaveBeenCalledWith({ pool: null });
  });
  it("does not fan out again for overlapping polls", async () => {
    const a = verifyCircuitBreakerInputs({ pool: null });
    const b = verifyCircuitBreakerInputs({ pool: null });
    expect(a).toBe(b);
    await Promise.all([a, b]);
    expect(sim).toHaveBeenCalledTimes(1);
  });
  it("does not share across DB dependencies", async () => {
    const a = {} as pg.Pool, b = {} as pg.Pool;
    await Promise.all([verifyCircuitBreakerInputs({ pool: a }), verifyCircuitBreakerInputs({ pool: b })]);
    expect(sim).toHaveBeenCalledTimes(2);
  });
  it("propagates a failed probe instead of fabricating PASS and retries the next observation", async () => {
    sim.mockRejectedValueOnce(new Error("unreachable"));
    await expect(verifyCircuitBreakerInputs({ pool: null })).rejects.toThrow("unreachable");
    expect((await verifyCircuitBreakerInputs({ pool: null })).items[1]?.status).toBe("yellow");
  });
  it("the independent full report still runs every one of its 19 verifiers", () => {
    const source = readFileSync(new URL("./index.ts", import.meta.url), "utf8");
    const body = source.split("const items = await Promise.all([")[1]?.split("]);", 1)[0];
    expect(body).toBeDefined();
    const calls = [...body!.matchAll(/\b(verify[A-Za-z0-9]+)\(/g)].map(m => m[1]);
    expect(calls).toEqual([
      "verifyVNH1", "verifyVDB1", "verifyVAT1", "verifyPR1CSP", "verifyPR2Audit",
      "verifyMonitoring", "verifyRunbook", "verifyGRPC1", "verifyGNET1", "verifyGSIM1",
      "verifyGPEC1", "verifyGRIS1", "verifyGTOK1", "verifyGPAP1", "verifyGFL1",
      "verifyGMEV1", "verifyGDISK1", "verifyGPIPE1", "verifyAlerts",
    ]);
    expect(source).toContain("flip_blocked: summary.green !== summary.total");
  });
});
