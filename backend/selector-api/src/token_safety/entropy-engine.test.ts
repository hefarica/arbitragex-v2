/**
 * entropy-engine.test.ts — RESEARCH/SHADOW. Locks the EntropyEngine contract:
 * determinism (the v2.0 Math.sin PRNG was NOT reproducible — this verifies the
 * mulberry32 fix), bounds, fail-honest neutral values, constructor validation.
 */
import { describe, it, expect } from "vitest";
import { EntropyEngine } from "./entropy-engine.js";

describe("EntropyEngine — constructor validation", () => {
  it("rejects windowSize < 2", () => {
    expect(() => new EntropyEngine(1)).toThrow();
  });
  it("rejects alpha out of (0,1)", () => {
    expect(() => new EntropyEngine(60, 0)).toThrow();
    expect(() => new EntropyEngine(60, 1)).toThrow();
    expect(() => new EntropyEngine(60, -0.5)).toThrow();
    expect(() => new EntropyEngine(60, 1.5)).toThrow();
  });
  it("accepts valid params", () => {
    expect(() => new EntropyEngine(60, 0.5)).not.toThrow();
  });
});

describe("EntropyEngine — calculateMemoryStrength", () => {
  it("returns 0 for insufficient data (< windowSize)", () => {
    const e = new EntropyEngine(60);
    expect(e.calculateMemoryStrength([1, 2, 3])).toBe(0);
  });
  it("returns a non-negative finite number for valid data", () => {
    const e = new EntropyEngine(10);
    const data = Array.from({ length: 20 }, (_, i) => Math.sin(i / 3) * 100);
    const m = e.calculateMemoryStrength(data);
    expect(m).toBeGreaterThanOrEqual(0);
    expect(Number.isFinite(m)).toBe(true);
  });
});

describe("EntropyEngine — calculateHurstExponent", () => {
  it("returns 0.5 (neutral) for insufficient data (< 50 points)", () => {
    const e = new EntropyEngine(10);
    expect(e.calculateHurstExponent([1, 2, 3, 4])).toBe(0.5);
  });
  it("is bounded in [0, 1]", () => {
    const e = new EntropyEngine(10);
    const data = Array.from({ length: 200 }, () => Math.random() * 100);
    const H = e.calculateHurstExponent(data);
    expect(H).toBeGreaterThanOrEqual(0);
    expect(H).toBeLessThanOrEqual(1);
  });
  it("detects persistence (H > 0.5) in a strongly trending series", () => {
    const e = new EntropyEngine(10);
    // Cumulative-sum random walk => persistent.
    let acc = 100;
    const data = Array.from({ length: 500 }, (_, i) => {
      acc += (i % 7 === 0 ? -1 : 2) + Math.random() * 0.5;
      return acc;
    });
    const H = e.calculateHurstExponent(data);
    // Persistent trend should push H above 0.5 (loose bound — the heuristic
    // std-of-differences fit isn't a textbook R/S, but directionally correct).
    expect(H).toBeGreaterThan(0.5);
  });
});

describe("EntropyEngine — calculateManifoldMetrics (the Math.sin PRNG fix)", () => {
  it("is DETERMINISTIC — same inputs produce identical entropy (mulberry32, not Math.sin)", () => {
    const e = new EntropyEngine(10);
    const m1 = e.calculateManifoldMetrics(100, 5);
    const m2 = e.calculateManifoldMetrics(100, 5);
    expect(m1.entropy).toBe(m2.entropy); // bit-for-bit equal — the v2.0 Math.sin broke this
    expect(m1.expectedValue).toBe(m2.expectedValue);
  });
  it("returns expectedValue = currentPrice (symmetric distribution)", () => {
    const e = new EntropyEngine(10);
    const m = e.calculateManifoldMetrics(123.45, 2);
    expect(m.expectedValue).toBe(123.45);
  });
  it("returns entropy > 0 for a non-degenerate distribution", () => {
    const e = new EntropyEngine(10);
    const m = e.calculateManifoldMetrics(100, 5);
    expect(m.entropy).toBeGreaterThan(0);
  });
  it("is fail-honest on invalid inputs (NaN/negative vol)", () => {
    const e = new EntropyEngine(10);
    const m = e.calculateManifoldMetrics(NaN, 5);
    expect(m.entropy).toBe(0);
    expect(m.expectedValue).toBe(0);
    const m2 = e.calculateManifoldMetrics(100, -1);
    expect(m2.entropy).toBe(0);
  });
});

describe("EntropyEngine — analyzeMarket (composite)", () => {
  it("returns insufficient_data for short input + neutral values (fail-honest)", () => {
    const e = new EntropyEngine(60);
    const r = e.analyzeMarket([1, 2, 3]);
    expect(r.status).toBe("insufficient_data");
    expect(r.memoryStrength).toBe(0);
    expect(r.hurstExponent).toBe(0.5);
  });
  it("returns computed for valid input with bounded Hurst + non-negative memory", () => {
    const e = new EntropyEngine(30);
    const data = Array.from({ length: 100 }, (_, i) => 100 + Math.sin(i / 5) * 10);
    const r = e.analyzeMarket(data);
    expect(r.status).toBe("computed");
    expect(r.memoryStrength).toBeGreaterThanOrEqual(0);
    expect(r.hurstExponent).toBeGreaterThanOrEqual(0);
    expect(r.hurstExponent).toBeLessThanOrEqual(1);
    expect(r.spectralEntropy).toBeGreaterThanOrEqual(0);
  });
  it("is deterministic across runs (same data → same score)", () => {
    const e = new EntropyEngine(30);
    const data = Array.from({ length: 100 }, (_, i) => 100 + Math.sin(i / 5) * 10);
    const r1 = e.analyzeMarket(data);
    const r2 = e.analyzeMarket(data);
    expect(r1).toEqual(r2);
  });
});
