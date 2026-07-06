import { describe, it, expect } from "vitest";
import {
  RISK_POSTURES,
  RISK_POSTURE_ORDER,
  RISK_DISCLAIMER,
  resolvePosture,
  type RiskPostureId,
  type ResolvedRiskLimits,
} from "./riskPresets";

const ALL: RiskPostureId[] = ["conservative", "balanced", "aggressive"];

// The 7 mandatory dimensions from arbx-risk-limits-enforcement.
const SEVEN_DIMS: (keyof ResolvedRiskLimits)[] = [
  "dailyLossCapUsd",
  "perTokenCapUsd",
  "perPoolCapUsd",
  "perDexCapUsd",
  "perChainCapUsd",
  "perStrategyCapUsd",
  "maxConsecutiveLosses",
];

describe("risk presets — structure", () => {
  it("exposes exactly the three canonical postures in order", () => {
    expect(RISK_POSTURE_ORDER).toEqual(["conservative", "balanced", "aggressive"]);
    expect(Object.keys(RISK_POSTURES).sort()).toEqual(ALL.slice().sort());
  });

  it("every resolved posture carries ALL 7 mandatory dimensions (omission = defect)", () => {
    for (const id of ALL) {
      const r = resolvePosture(id, { capitalUsd: 1000 });
      for (const dim of SEVEN_DIMS) {
        expect(r[dim], `${id}.${String(dim)} present`).toBeTypeOf("number");
        expect(Number.isFinite(r[dim] as number)).toBe(true);
      }
    }
  });
});

describe("risk presets — fail-honest (no invented capital)", () => {
  for (const bad of [0, -1, -100000, Number.NaN, Number.POSITIVE_INFINITY]) {
    it(`capital=${bad} resolves every USD cap to 0 with a warning`, () => {
      const r = resolvePosture("balanced", { capitalUsd: bad });
      expect(r.deployedCapitalUsd).toBe(0);
      expect(r.dailyLossCapUsd).toBe(0);
      expect(r.perTokenCapUsd).toBe(0);
      expect(r.perPoolCapUsd).toBe(0);
      expect(r.perDexCapUsd).toBe(0);
      expect(r.perChainCapUsd).toBe(0);
      expect(r.perStrategyCapUsd).toBe(0);
      expect(r.warnings.length).toBeGreaterThan(0);
    });
  }
});

describe("risk presets — caps are fractions of operator capital (no-hardcode)", () => {
  it("no resolved cap exceeds deployed capital, which never exceeds operator capital", () => {
    const capitalUsd = 10_000;
    for (const id of ALL) {
      const r = resolvePosture(id, { capitalUsd });
      expect(r.deployedCapitalUsd).toBeLessThanOrEqual(capitalUsd);
      for (const dim of SEVEN_DIMS) {
        if (dim === "maxConsecutiveLosses") continue;
        expect(r[dim] as number, `${id}.${String(dim)} <= deployed`).toBeLessThanOrEqual(
          r.deployedCapitalUsd + 1e-9,
        );
      }
    }
  });

  it("scales linearly with capital (pure fraction math, no literals)", () => {
    const a = resolvePosture("aggressive", { capitalUsd: 1000 });
    const b = resolvePosture("aggressive", { capitalUsd: 2000 });
    expect(b.deployedCapitalUsd).toBeCloseTo(a.deployedCapitalUsd * 2, 6);
    expect(b.dailyLossCapUsd).toBeCloseTo(a.dailyLossCapUsd * 2, 6);
    expect(b.perTokenCapUsd).toBeCloseTo(a.perTokenCapUsd * 2, 6);
  });
});

describe("risk presets — monotonicity (exposure rises, safety tightens)", () => {
  const C = resolvePosture("conservative", { capitalUsd: 100_000 });
  const B = resolvePosture("balanced", { capitalUsd: 100_000 });
  const A = resolvePosture("aggressive", { capitalUsd: 100_000 });

  it("capital exposure: conservative < balanced < aggressive", () => {
    expect(C.deployedCapitalUsd).toBeLessThan(B.deployedCapitalUsd);
    expect(B.deployedCapitalUsd).toBeLessThan(A.deployedCapitalUsd);
  });

  it("every per-dimension cap is non-decreasing with aggression", () => {
    for (const dim of ["perTokenCapUsd", "perPoolCapUsd", "perDexCapUsd", "perChainCapUsd", "perStrategyCapUsd", "dailyLossCapUsd"] as const) {
      expect(C[dim], `C<=B ${dim}`).toBeLessThanOrEqual(B[dim]);
      expect(B[dim], `B<=A ${dim}`).toBeLessThanOrEqual(A[dim]);
    }
  });

  it("safety margins tighten as aggression drops (conservative is strictest)", () => {
    // Lower slippage tolerance and higher ROI floor == more conservative.
    expect(C.maxSlippagePct).toBeLessThan(B.maxSlippagePct);
    expect(B.maxSlippagePct).toBeLessThan(A.maxSlippagePct);
    expect(C.minRoiPct).toBeGreaterThan(B.minRoiPct);
    expect(B.minRoiPct).toBeGreaterThan(A.minRoiPct);
    // Fewer consecutive losses tolerated == halts sooner == more conservative.
    expect(C.maxConsecutiveLosses).toBeLessThanOrEqual(B.maxConsecutiveLosses);
    expect(B.maxConsecutiveLosses).toBeLessThanOrEqual(A.maxConsecutiveLosses);
  });
});

describe("risk presets — ethics (no predatory strategy at any level)", () => {
  const FORBIDDEN = ["sandwich", "frontrun", "frontrun_user", "oracle", "jit", "reorg", "victim"];

  it("no posture enables a strategy kind matching a predatory pattern", () => {
    for (const id of ALL) {
      const r = resolvePosture(id, { capitalUsd: 1000 });
      for (const kind of r.enabledStrategyKinds) {
        for (const bad of FORBIDDEN) {
          expect(kind.toLowerCase().includes(bad), `${id} enables ${kind}`).toBe(false);
        }
      }
    }
  });

  it("strategy breadth is a non-shrinking set with aggression", () => {
    const c = new Set(resolvePosture("conservative", { capitalUsd: 1000 }).enabledStrategyKinds);
    const b = new Set(resolvePosture("balanced", { capitalUsd: 1000 }).enabledStrategyKinds);
    const a = new Set(resolvePosture("aggressive", { capitalUsd: 1000 }).enabledStrategyKinds);
    for (const k of c) expect(b.has(k), `balanced ⊇ conservative: ${k}`).toBe(true);
    for (const k of b) expect(a.has(k), `aggressive ⊇ balanced: ${k}`).toBe(true);
  });
});

describe("risk presets — purity & warnings", () => {
  it("is deterministic: same input → deep-equal output", () => {
    const x = resolvePosture("balanced", { capitalUsd: 1234.56 });
    const y = resolvePosture("balanced", { capitalUsd: 1234.56 });
    expect(x).toEqual(y);
  });

  it("warns below the posture's recommended minimum, not above it", () => {
    const below = resolvePosture("aggressive", { capitalUsd: 100 }); // min 2000
    const above = resolvePosture("aggressive", { capitalUsd: 5000 });
    expect(below.warnings.some((w) => w.includes("recommended"))).toBe(true);
    expect(above.warnings.some((w) => w.includes("recommended"))).toBe(false);
  });

  it("disclaimer makes no performance claim", () => {
    // Promissory MULTIWORD phrases only — single words like "guaranteed",
    // "earn", "returns" legitimately appear inside negations ("implied or
    // guaranteed", "not what you can earn"), which is the whole point of a
    // disclaimer. We only forbid constructions that cannot be a negation.
    const promiseWords = ["win rate", "you will earn", "expected profit", "guaranteed profit", "guaranteed return"];
    for (const w of promiseWords) {
      expect(RISK_DISCLAIMER.toLowerCase().includes(w), `disclaimer must not promise: "${w}"`).toBe(false);
    }
    expect(RISK_DISCLAIMER.toLowerCase()).toContain("not returns");
    expect(RISK_DISCLAIMER.toLowerCase()).toContain("do not predict");
  });
});
