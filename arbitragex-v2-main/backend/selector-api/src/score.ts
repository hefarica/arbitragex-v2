/**
 * BACKWARD-COMPAT shim for the S1 `/score` HTTP endpoint.
 * New code should import from `./scoring/engine` directly.
 *
 * The S1 score endpoint is kept as a synchronous decision helper for admin/debug
 * use; the production flow is the Redis Streams consumer (see ./consumer.ts).
 */

import type { Opportunity, SimulationResult } from "@arbx/shared";
import { scoreOpportunity as scoreV2, weightsFromConfig } from "./scoring/engine.js";

export type ScoreFactors = {
  liquidity: number; depth: number; safety: number;
  slippage: number; gas: number; risk: number;
};

export type ScoredOpportunity = {
  opportunity_id: string;
  score: number;
  decision: "accept" | "reject";
  reason: string | null;
  factors: ScoreFactors;
};

// Keep the S1 signature (3 args) so existing tests pass; use default weights and
// a conservative 200 gwei cap when config isn't threaded here.
const DEFAULT_WEIGHTS = {
  min_accept_score: 55,
  liquidity: 0.20, depth: 0.15, safety: 0.20,
  slippage: 0.15, gas: 0.15, risk: 0.15,
};
const DEFAULT_GAS_CAP_GWEI = 200;

export function scoreOpportunity(
  opp: Opportunity,
  sim: SimulationResult | null,
  safety_score = 50,
): ScoredOpportunity {
  const s = scoreV2(opp, sim, safety_score, DEFAULT_WEIGHTS, DEFAULT_GAS_CAP_GWEI);
  let decision: "accept" | "reject" = "accept";
  let reason: string | null = null;
  if (s.factors.safety < 50) { decision = "reject"; reason = "safety_below_threshold"; }
  else if (sim && sim.passed === false) { decision = "reject"; reason = "simulation_failed"; }
  else if (s.score < DEFAULT_WEIGHTS.min_accept_score) { decision = "reject"; reason = "score_below_min"; }
  return { opportunity_id: s.opportunity_id, score: s.score, decision, reason, factors: s.factors };
}
