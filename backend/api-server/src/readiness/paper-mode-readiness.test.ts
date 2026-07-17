import { describe, it, expect } from "vitest";
import {
  gradePaperReadiness,
  type PaperAccumulationState,
  type PaperReadinessGrade,
} from "./paper-mode-readiness.js";
import type { PaperModeState } from "./paper-mode-state.js";

function makeAuthority(
  overrides: Partial<PaperModeState> = {},
): PaperModeState {
  return {
    enabled: true,
    chain_id: 1,
    source: "redis",
    confidence: "explicit",
    degraded: false,
    conflict: false,
    updated_at: "2026-07-17T00:00:00Z",
    reasons: [],
    chains: [],
    ...overrides,
  };
}

function makeAccumulation(
  overrides: Partial<PaperAccumulationState> = {},
): PaperAccumulationState {
  return {
    days_accumulated: 7,
    recent_opportunities: 10,
    last_opportunity_at: "2026-07-17T00:00:00Z",
    pipeline_active: true,
    degraded: false,
    reasons: [],
    ...overrides,
  };
}

describe("gradePaperReadiness()", () => {
  it("GREEN when explicit + >=7d + pipeline active", () => {
    const authority = makeAuthority({ confidence: "explicit" });
    const accumulation = makeAccumulation({
      days_accumulated: 7,
      pipeline_active: true,
    });

    const grade: PaperReadinessGrade = gradePaperReadiness(
      authority,
      accumulation,
    );

    expect(grade.status).toBe("green");
    expect(grade.authority_confidence).toBe("explicit");
    expect(grade.accumulation_days).toBe(7);
    expect(grade.reason).toContain("explicit");
  });

  it("YELLOW when explicit + <7d", () => {
    const authority = makeAuthority({ confidence: "explicit" });
    const accumulation = makeAccumulation({
      days_accumulated: 3,
      pipeline_active: true,
    });

    const grade: PaperReadinessGrade = gradePaperReadiness(
      authority,
      accumulation,
    );

    expect(grade.status).toBe("yellow");
    expect(grade.reason).toBe("not enough days");
  });

  it("RED when conflict", () => {
    const authority = makeAuthority({
      confidence: "explicit",
      conflict: true,
    });
    const accumulation = makeAccumulation({
      days_accumulated: 7,
      pipeline_active: true,
    });

    const grade: PaperReadinessGrade = gradePaperReadiness(
      authority,
      accumulation,
    );

    expect(grade.status).toBe("red");
    expect(grade.reason).toContain("conflict");
  });

  it("YELLOW when inferred + pipeline active", () => {
    const authority = makeAuthority({
      confidence: "inferred",
      source: "env",
    });
    const accumulation = makeAccumulation({
      days_accumulated: 14,
      pipeline_active: true,
    });

    const grade: PaperReadinessGrade = gradePaperReadiness(
      authority,
      accumulation,
    );

    expect(grade.status).toBe("yellow");
    expect(grade.authority_confidence).toBe("inferred");
  });

  it("YELLOW when explicit_legacy (never green)", () => {
    const authority = makeAuthority({
      confidence: "explicit_legacy",
      source: "redis",
      degraded: true,
    });
    const accumulation = makeAccumulation({
      days_accumulated: 30,
      pipeline_active: true,
    });

    const grade: PaperReadinessGrade = gradePaperReadiness(
      authority,
      accumulation,
    );

    expect(grade.status).toBe("yellow");
    expect(grade.authority_confidence).toBe("explicit_legacy");
  });
});
