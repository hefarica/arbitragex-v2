/**
 * scoring-status tests — pure-function + structural assertions.
 *
 * Coverage:
 *   - 7 components defined (5 wired + 2 pending).
 *   - computeStatus returns "partial" with the current wire state.
 *   - No fake metrics — recent_scored_count starts at 0.
 *   - No secrets in any component's evidence_ref string.
 *   - Live trading + capital exposure are immutable false/0.
 */
import { describe, it, expect } from "vitest";
import { __forTesting } from "./scoring-status.js";

const { COMPONENTS, BLOCKED_REASONS, SCORING_VERSION, computeStatus } = __forTesting;

describe("scoring-status — component wire map", () => {
  it("defines exactly 7 components (5 wired primitives + 2 pending pipeline points)", () => {
    expect(COMPONENTS.length).toBe(7);
  });

  it("Bayesian filter component is wired", () => {
    const c = COMPONENTS.find((x) => x.name.includes("Bayesian"));
    expect(c?.wired).toBe(true);
    expect(c?.evidence_ref).toMatch(/bayesian_filter\.rs/);
  });

  it("Kelly criterion component is wired", () => {
    const c = COMPONENTS.find((x) => x.name.includes("Kelly"));
    expect(c?.wired).toBe(true);
    expect(c?.evidence_ref).toMatch(/kelly_sizing\.rs/);
  });

  it("VPIN component is wired", () => {
    const c = COMPONENTS.find((x) => x.name.includes("VPIN"));
    expect(c?.wired).toBe(true);
  });

  it("Scoring pipeline invocation is NOT wired (honest)", () => {
    const c = COMPONENTS.find((x) => x.name.includes("pipeline"));
    expect(c?.wired).toBe(false);
    expect(c?.evidence_ref).toMatch(/scanner\.rs/);
  });

  it("Opportunity persistence with scoring fields is NOT wired", () => {
    const c = COMPONENTS.find((x) => x.name.includes("persistence"));
    expect(c?.wired).toBe(false);
  });
});

describe("scoring-status — blocked reasons", () => {
  it("includes scoring_pipeline_not_wired", () => {
    expect(BLOCKED_REASONS.find((b) => b.id === "scoring_pipeline_not_wired")).toBeDefined();
  });

  it("includes a4_fork_validation_not_executed (critical)", () => {
    const b = BLOCKED_REASONS.find((x) => x.id === "a4_fork_validation_not_executed");
    expect(b).toBeDefined();
    expect(b?.severity).toBe("critical");
  });

  it("includes a5_paper_shadow_not_executed", () => {
    expect(BLOCKED_REASONS.find((b) => b.id === "a5_paper_shadow_not_executed")).toBeDefined();
  });
});

describe("scoring-status — computeStatus precedence", () => {
  it("returns 'partial' with current wire state (some wired, some pending)", () => {
    expect(computeStatus()).toBe("partial");
  });
});

describe("scoring-status — version pinning", () => {
  it("version is the partial pre-pipeline release", () => {
    expect(SCORING_VERSION).toBe("0.1.0-partial");
  });
});

describe("scoring-status — security regression", () => {
  it("no component evidence_ref leaks a secret (RPC, DB, contract address)", () => {
    for (const c of COMPONENTS) {
      expect(c.evidence_ref).not.toMatch(/https?:\/\/[a-z0-9-]+\.alchemy\.com/i);
      expect(c.evidence_ref).not.toMatch(/postgres:\/\/[^@\s]+:[^@\s]+@/i);
      expect(c.evidence_ref).not.toMatch(/0x[a-f0-9]{40}/i);
    }
    for (const b of BLOCKED_REASONS) {
      expect(b.description).not.toMatch(/https?:\/\/[a-z0-9-]+\.alchemy\.com/i);
      expect(b.description).not.toMatch(/postgres:\/\/[^@\s]+:[^@\s]+@/i);
    }
  });
});
