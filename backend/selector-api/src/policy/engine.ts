/**
 * Policy engine — prefilter + decide.
 *
 * prefilter runs BEFORE expensive work (token safety API, scoring).
 * decide runs AFTER score + safety are known.
 */

import type Redis from "ioredis";
import type { Opportunity, SimulationResult, AppConfig, CircuitBreaker } from "@arbx/shared";
import type { ScoredOpportunity } from "../scoring/engine.js";
import type { TokenSafetyRecord } from "../token_safety/cache.js";
import { pairAllowed } from "./blacklist.js";

export type RejectSeverity = "info" | "warning" | "critical";

export type Decision =
  | { kind: "accept"; score: number; reason: null }
  | { kind: "reject"; score: number | null; reason: string; severity: RejectSeverity };

export type PrefilterInput = {
  opportunity: Opportunity;
  killSwitchOn: boolean;
  tokenSafetyCb: CircuitBreaker;
};

export async function prefilter(
  redis: Redis,
  input: PrefilterInput,
): Promise<Decision | null> {
  const { opportunity: opp, killSwitchOn, tokenSafetyCb } = input;

  if (killSwitchOn) {
    return { kind: "reject", score: null, reason: "kill_switch_on", severity: "info" };
  }

  const bl = await pairAllowed(redis, opp.chain_id, opp.token_in, opp.token_out);
  if (!bl.allowed) {
    return {
      kind: "reject",
      score: null,
      reason: bl.reason ?? "blacklist_hit",
      severity: "warning",
    };
  }

  if (tokenSafetyCb.state() === "open") {
    return {
      kind: "reject",
      score: null,
      reason: "token_safety_circuit_open",
      severity: "warning",
    };
  }

  return null; // no rejection at prefilter stage
}

export type DecideInput = {
  scored: ScoredOpportunity;
  safety: TokenSafetyRecord;
  sim: SimulationResult | null;
  cfg: AppConfig;
};

export function decide(input: DecideInput): Decision {
  const { scored, safety, sim, cfg } = input;

  // 1. Safety hard floor.
  if (safety.safety_score < cfg.token_safety.min_acceptable_score) {
    return {
      kind: "reject", score: scored.score,
      reason: "safety_below_threshold", severity: "warning",
    };
  }

  // 2. Simulation explicit failure (only if run).
  if (sim && sim.passed === false) {
    return {
      kind: "reject", score: scored.score,
      reason: "simulation_failed", severity: "info",
    };
  }

  // 3. Revert-risk hard cap (2× configured target revert rate).
  if (sim && sim.revert_risk_pct != null && sim.revert_risk_pct > cfg.risk.max_revert_rate_pct * 2) {
    return {
      kind: "reject", score: scored.score,
      reason: "revert_risk_too_high", severity: "warning",
    };
  }

  // 4. Score threshold.
  if (scored.score < cfg.scoring.min_accept_score) {
    return {
      kind: "reject", score: scored.score,
      reason: "score_below_min", severity: "info",
    };
  }

  return { kind: "accept", score: scored.score, reason: null };
}
