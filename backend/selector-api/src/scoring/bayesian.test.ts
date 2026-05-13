import { describe, it, expect } from "vitest";
import { BayesianToxicityEngine } from "./bayesian.js";

const approx = (a: number, b: number, tol = 1e-6) => Math.abs(a - b) < tol;

describe("BayesianToxicityEngine — construction + invariants", () => {
  it("constructs with valid prior + defaults", () => {
    const e = new BayesianToxicityEngine({ prior: 0.5 });
    expect(e.getToxicity()).toBe(0.5);
  });

  it("rejects prior outside [0, 1]", () => {
    expect(() => new BayesianToxicityEngine({ prior: -0.1 })).toThrow(/prior/);
    expect(() => new BayesianToxicityEngine({ prior: 1.1 })).toThrow(/prior/);
    expect(() => new BayesianToxicityEngine({ prior: Number.NaN })).toThrow(/prior/);
  });

  it("rejects non-positive Beta shape parameters", () => {
    expect(() =>
      new BayesianToxicityEngine({ prior: 0.5, likelihoodToxic: { alpha: 0, beta: 5 } }),
    ).toThrow(/likelihoodToxic/);
    expect(() =>
      new BayesianToxicityEngine({ prior: 0.5, likelihoodSafe: { alpha: 5, beta: -1 } }),
    ).toThrow(/likelihoodSafe/);
  });
});

describe("BayesianToxicityEngine — Bayes update directionality", () => {
  it("high revertRate (0.8) increases toxic posterior", () => {
    const e = new BayesianToxicityEngine({ prior: 0.5 });
    const next = e.updateOnRevertRate(0.8);
    expect(next).toBeGreaterThan(0.5);
    expect(e.getToxicity()).toBe(next);
  });

  it("low revertRate (0.1) decreases toxic posterior", () => {
    const e = new BayesianToxicityEngine({ prior: 0.5 });
    const next = e.updateOnRevertRate(0.1);
    expect(next).toBeLessThan(0.5);
  });

  it("symmetric defaults: posterior at r ≈ 0.5 stays near prior", () => {
    // Defaults: toxic Beta(5,5), safe Beta(2,8).
    // At r=0.5: log-likelihoods are 4·log(0.5)+4·log(0.5) = 8·log(0.5)
    //                              1·log(0.5)+7·log(0.5) = 8·log(0.5)
    // Equal log-likelihoods → posterior stays at prior.
    const e = new BayesianToxicityEngine({ prior: 0.5 });
    const next = e.updateOnRevertRate(0.5);
    expect(approx(next, 0.5, 1e-9)).toBe(true);
  });

  it("repeated high revertRate compounds toward 1", () => {
    const e = new BayesianToxicityEngine({ prior: 0.5 });
    for (let i = 0; i < 5; i++) e.updateOnRevertRate(0.85);
    expect(e.getToxicity()).toBeGreaterThan(0.95);
  });

  it("repeated low revertRate compounds toward 0", () => {
    const e = new BayesianToxicityEngine({ prior: 0.5 });
    for (let i = 0; i < 5; i++) e.updateOnRevertRate(0.05);
    expect(e.getToxicity()).toBeLessThan(0.05);
  });

  it("posterior is always in [0, 1] across many updates (property)", () => {
    const e = new BayesianToxicityEngine({ prior: 0.5 });
    for (let i = 0; i < 1_000; i++) {
      const r = Math.random();
      const p = e.updateOnRevertRate(r);
      expect(p).toBeGreaterThanOrEqual(0);
      expect(p).toBeLessThanOrEqual(1);
    }
  });
});

describe("BayesianToxicityEngine — invalid input handling (R8 fail-honest)", () => {
  it("NaN revertRate leaves posterior unchanged", () => {
    const e = new BayesianToxicityEngine({ prior: 0.42 });
    const next = e.updateOnRevertRate(Number.NaN);
    expect(next).toBe(0.42);
    expect(e.getToxicity()).toBe(0.42);
  });

  it("revertRate > 1 leaves posterior unchanged", () => {
    const e = new BayesianToxicityEngine({ prior: 0.42 });
    expect(e.updateOnRevertRate(1.5)).toBe(0.42);
  });

  it("revertRate < 0 leaves posterior unchanged", () => {
    const e = new BayesianToxicityEngine({ prior: 0.42 });
    expect(e.updateOnRevertRate(-0.1)).toBe(0.42);
  });

  it("Infinity leaves posterior unchanged", () => {
    const e = new BayesianToxicityEngine({ prior: 0.42 });
    expect(e.updateOnRevertRate(Number.POSITIVE_INFINITY)).toBe(0.42);
  });
});

describe("BayesianToxicityEngine — classify + reset", () => {
  it("classify returns toxic=true when posterior >= threshold", () => {
    const e = new BayesianToxicityEngine({ prior: 0.7 });
    const r = e.classify(0.65);
    expect(r.toxic).toBe(true);
    expect(r.posterior).toBe(0.7);
  });

  it("classify returns toxic=false when posterior < threshold", () => {
    const e = new BayesianToxicityEngine({ prior: 0.5 });
    const r = e.classify(0.65);
    expect(r.toxic).toBe(false);
    expect(r.posterior).toBe(0.5);
  });

  it("classify rejects threshold outside [0, 1]", () => {
    const e = new BayesianToxicityEngine({ prior: 0.5 });
    expect(() => e.classify(-0.1)).toThrow();
    expect(() => e.classify(1.1)).toThrow();
  });

  it("reset re-seeds the posterior", () => {
    const e = new BayesianToxicityEngine({ prior: 0.5 });
    e.updateOnRevertRate(0.95);
    expect(e.getToxicity()).toBeGreaterThan(0.5);
    e.reset(0.1);
    expect(e.getToxicity()).toBe(0.1);
  });

  it("reset rejects out-of-range prior", () => {
    const e = new BayesianToxicityEngine({ prior: 0.5 });
    expect(() => e.reset(-0.1)).toThrow();
    expect(() => e.reset(1.1)).toThrow();
  });
});

describe("BayesianToxicityEngine — custom likelihood shapes", () => {
  it("operator-tuned Beta shapes override defaults", () => {
    // Strongly informative likelihoods: toxic Beta(10, 2) peaks near 1,
    // safe Beta(2, 10) peaks near 0. A single observation at r=0.9 should
    // push the posterior nearly to 1.
    const e = new BayesianToxicityEngine({
      prior: 0.5,
      likelihoodToxic: { alpha: 10, beta: 2 },
      likelihoodSafe: { alpha: 2, beta: 10 },
    });
    const next = e.updateOnRevertRate(0.9);
    expect(next).toBeGreaterThan(0.99);
  });
});
