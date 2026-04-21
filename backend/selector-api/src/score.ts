import type { Opportunity, SimulationResult } from "@arbx/shared";

/** Multi-factor selector score.
 *  S1: deterministic formula using six explicit factors with documented weights.
 *  S3 replaces this with learned weights driven by strategy_scores + relay_scores.
 */
export type ScoreFactors = {
  liquidity: number;   // 0..100
  depth: number;       // 0..100
  safety: number;      // 0..100 from token_safety_cache
  slippage: number;    // 0..100 (lower is better -> inverted before sum)
  gas: number;         // 0..100 (lower is better -> inverted before sum)
  risk: number;        // 0..100 (lower is better -> inverted before sum)
};

export type ScoredOpportunity = {
  opportunity_id: string;
  score: number;
  decision: "accept" | "reject";
  reason: string | null;
  factors: ScoreFactors;
};

const WEIGHTS = {
  liquidity: 0.20,
  depth:     0.15,
  safety:    0.20,
  slippage:  0.15,
  gas:       0.15,
  risk:      0.15,
};
const MIN_ACCEPT_SCORE = 55;

export function computeFactors(opp: Opportunity, sim: SimulationResult | null, safety_score = 50): ScoreFactors {
  const liquidity = Math.min(100, Math.max(0, Math.log10(Math.max(1, opp.expected_profit_usd * 20)) * 20));
  const depth = opp.dex_b ? 60 : 40;
  const safety = Math.min(100, Math.max(0, safety_score));
  const slippagePct = sim?.slippage_pct ?? 2.0;
  const slippage = Math.min(100, Math.max(0, 100 - slippagePct * 30));
  const gasPct = Number(sim?.gas_estimate_wei ?? "0") > 0 ? 70 : 40;
  const revertPct = sim?.revert_risk_pct ?? 50;
  const risk = Math.min(100, Math.max(0, 100 - revertPct));
  return { liquidity, depth, safety, slippage, gas: gasPct, risk };
}

export function scoreOpportunity(opp: Opportunity, sim: SimulationResult | null, safety_score = 50): ScoredOpportunity {
  const f = computeFactors(opp, sim, safety_score);
  const raw =
    f.liquidity * WEIGHTS.liquidity +
    f.depth     * WEIGHTS.depth +
    f.safety    * WEIGHTS.safety +
    f.slippage  * WEIGHTS.slippage +
    f.gas       * WEIGHTS.gas +
    f.risk      * WEIGHTS.risk;
  const score = Math.round(raw * 100) / 100;

  let decision: "accept" | "reject" = "accept";
  let reason: string | null = null;
  if (f.safety < 50) { decision = "reject"; reason = "safety_below_threshold"; }
  else if (sim && sim.passed === false) { decision = "reject"; reason = "simulation_failed"; }
  else if (score < MIN_ACCEPT_SCORE) { decision = "reject"; reason = "score_below_min"; }

  return {
    opportunity_id: opp.id,
    score,
    decision,
    reason,
    factors: f,
  };
}
