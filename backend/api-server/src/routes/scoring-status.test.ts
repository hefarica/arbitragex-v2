/**
 * scoring-status tests — evidence-based state derivation (pure functions).
 *
 * The route now DERIVES the three Gate-C blockers from real runtime evidence
 * (Postgres), so these tests pin the derivation logic rather than frozen
 * constants. Covers: pipeline states (blocked / wired-no-sample / wired-sample),
 * A.4 pending↔passed, the four A.5 states, blocked-reason resolution, and the
 * security regression (no secrets in evidence_ref).
 */
import { describe, it, expect } from "vitest";
import { __forTesting } from "./scoring-status.js";

const {
  SCORING_VERSION,
  emptyEvidence,
  deriveScoringPipelineState,
  deriveA4State,
  deriveA5State,
  buildComponents,
  buildBlockedReasons,
  computeStatus,
} = __forTesting;

describe("scoring-status — scoring_pipeline_state (structural, not rows-only)", () => {
  it("BLOCKED when the scored_opportunities table is absent", () => {
    const e = emptyEvidence(); // tableExists = false
    expect(deriveScoringPipelineState(e)).toBe("BLOCKED");
  });

  it("WIRED_NO_RUNTIME_SAMPLE when wired + archiver on but no rows (quiet market is not a failure)", () => {
    const e = { ...emptyEvidence(), tableExists: true, archiverEnabled: true, totalScored: 0 };
    expect(deriveScoringPipelineState(e)).toBe("WIRED_NO_RUNTIME_SAMPLE");
  });

  it("BLOCKED when the table exists but the archiver is dormant (no consumer = not really wired)", () => {
    const e = { ...emptyEvidence(), tableExists: true, archiverEnabled: false, totalScored: 0 };
    expect(deriveScoringPipelineState(e)).toBe("BLOCKED");
  });

  it("WIRED_RUNTIME_SAMPLE once ≥1 scored row exists", () => {
    const e = { ...emptyEvidence(), tableExists: true, totalScored: 5 };
    expect(deriveScoringPipelineState(e)).toBe("WIRED_RUNTIME_SAMPLE");
  });
});

describe("scoring-status — A.4 fork validation (evidence only)", () => {
  it("A4_PENDING without a gate_c_validation row", () => {
    expect(deriveA4State({ ...emptyEvidence(), a4Passed: false })).toBe("A4_PENDING");
  });
  it("A4_PASSED only with the evidence row", () => {
    expect(deriveA4State({ ...emptyEvidence(), a4Passed: true })).toBe("A4_PASSED");
  });
});

describe("scoring-status — A.5 paper-shadow four-state machine", () => {
  it("PAPER_SHADOW_NOT_STARTED with no activity", () => {
    expect(deriveA5State(emptyEvidence())).toBe("PAPER_SHADOW_NOT_STARTED");
  });

  it("PAPER_SHADOW_WARMING with activity but short span / low volume", () => {
    const e = { ...emptyEvidence(), tableExists: true, totalScored: 10, firstScoredAt: "2026-06-06T00:00:00.000Z", lastScoredAt: "2026-06-06T06:00:00.000Z" };
    expect(deriveA5State(e)).toBe("PAPER_SHADOW_WARMING");
  });

  it("CALIBRATED_CANDIDATE after ≥7-day span + enough volume, no promoted priors", () => {
    const e = { ...emptyEvidence(), tableExists: true, totalScored: 150, firstScoredAt: "2026-05-20T00:00:00.000Z", lastScoredAt: "2026-06-06T00:00:00.000Z", calibratedPriors: 0 };
    expect(deriveA5State(e)).toBe("CALIBRATED_CANDIDATE");
  });

  it("CALIBRATED only with real per-pair priors (observation_count ≥ threshold)", () => {
    const e = { ...emptyEvidence(), tableExists: true, totalScored: 150, firstScoredAt: "2026-05-20T00:00:00.000Z", lastScoredAt: "2026-06-06T00:00:00.000Z", calibratedPriors: 3 };
    expect(deriveA5State(e)).toBe("CALIBRATED");
  });
});

describe("scoring-status — components reflect evidence", () => {
  it("pipeline + persistence components are NOT wired when blocked", () => {
    const e = emptyEvidence();
    const comps = buildComponents(e, false);
    expect(comps.length).toBe(7);
    expect(comps.find((c) => c.name.includes("pipeline"))?.wired).toBe(false);
    expect(comps.find((c) => c.name.includes("persistence"))?.wired).toBe(false);
  });

  it("all 7 components wired ⇒ computeStatus 'enabled' once table exists + archiver on + pipeline wired", () => {
    const e = { ...emptyEvidence(), tableExists: true, archiverEnabled: true, totalScored: 1 };
    const comps = buildComponents(e, true);
    expect(comps.every((c) => c.wired)).toBe(true);
    expect(computeStatus(comps)).toBe("enabled");
  });

  it("partial when some components pending", () => {
    const comps = buildComponents(emptyEvidence(), false);
    expect(computeStatus(comps)).toBe("partial");
  });
});

describe("scoring-status — blocked reasons resolve with evidence", () => {
  it("lists all three blockers when nothing is done", () => {
    const reasons = buildBlockedReasons("BLOCKED", "A4_PENDING", "PAPER_SHADOW_NOT_STARTED");
    expect(reasons.find((b) => b.id === "scoring_pipeline_not_wired")).toBeDefined();
    const a4 = reasons.find((b) => b.id === "a4_fork_validation_not_executed");
    expect(a4?.severity).toBe("critical");
    expect(reasons.find((b) => b.id === "a5_paper_shadow_not_executed")).toBeDefined();
  });

  it("drops scoring_pipeline_not_wired once wired (no runtime sample is fine)", () => {
    const reasons = buildBlockedReasons("WIRED_NO_RUNTIME_SAMPLE", "A4_PENDING", "PAPER_SHADOW_NOT_STARTED");
    expect(reasons.find((b) => b.id === "scoring_pipeline_not_wired")).toBeUndefined();
  });

  it("fully clears when wired + A4 passed + calibrated", () => {
    const reasons = buildBlockedReasons("WIRED_RUNTIME_SAMPLE", "A4_PASSED", "CALIBRATED");
    expect(reasons.length).toBe(0);
  });
});

describe("scoring-status — version + security regression", () => {
  it("version is the evidence-based release", () => {
    expect(SCORING_VERSION).toBe("0.2.0-evidence");
  });

  it("no component evidence_ref leaks a secret (RPC, DB, contract address)", () => {
    for (const c of buildComponents(emptyEvidence(), false)) {
      expect(c.evidence_ref).not.toMatch(/https?:\/\/[a-z0-9-]+\.alchemy\.com/i);
      expect(c.evidence_ref).not.toMatch(/postgres:\/\/[^@\s]+:[^@\s]+@/i);
      expect(c.evidence_ref).not.toMatch(/0x[a-f0-9]{40}/i);
    }
  });
});
