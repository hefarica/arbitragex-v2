import type { PaperModeState, PaperModeConfidence } from "./paper-mode-state.js";

export type PaperAccumulationState = {
  days_accumulated: number;
  recent_opportunities: number;
  last_opportunity_at: string | null;
  pipeline_active: boolean;
  degraded: boolean;
  reasons: string[];
};

export type PaperReadinessStatus = "green" | "yellow" | "red";

export type PaperReadinessGrade = {
  status: PaperReadinessStatus;
  reason: string;
  authority_confidence: PaperModeConfidence;
  accumulation_days: number;
};

export function gradePaperReadiness(
  authority: PaperModeState,
  accumulation: PaperAccumulationState,
): PaperReadinessGrade {
  const days = accumulation.days_accumulated;

  if (authority.conflict) {
    return {
      status: "red",
      reason: "paper mode authority reports a conflict",
      authority_confidence: authority.confidence,
      accumulation_days: days,
    };
  }

  if (!authority.enabled) {
    return {
      status: "red",
      reason: "paper mode is not enabled",
      authority_confidence: authority.confidence,
      accumulation_days: days,
    };
  }

  if (authority.confidence === "explicit") {
    if (!accumulation.pipeline_active) {
      return {
        status: "yellow",
        reason: "pipeline stalled",
        authority_confidence: authority.confidence,
        accumulation_days: days,
      };
    }

    if (days < 7) {
      return {
        status: "yellow",
        reason: "not enough days",
        authority_confidence: authority.confidence,
        accumulation_days: days,
      };
    }

    return {
      status: "green",
      reason: "paper mode explicit and accumulation sufficient",
      authority_confidence: authority.confidence,
      accumulation_days: days,
    };
  }

  // explicit_legacy, inferred, default_safe, observed -> yellow max.
  const reason = authority.degraded
    ? "paper mode confidence is degraded"
    : "paper mode confidence is insufficient for green";

  return {
    status: "yellow",
    reason,
    authority_confidence: authority.confidence,
    accumulation_days: days,
  };
}
