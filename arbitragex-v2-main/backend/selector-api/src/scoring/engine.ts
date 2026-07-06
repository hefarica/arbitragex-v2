/**
 * Score engine. Weights come from config; defaults documented here as fallback only.
 */
import type { Opportunity, SimulationResult, AppConfig } from "@arbx/shared";
import { computeFactors, type Factors } from "./factors.js";

export type ScoringWeights = {
  min_accept_score: number;
  liquidity: number;
  depth: number;
  safety: number;
  slippage: number;
  gas: number;
  risk: number;
};

export function weightsFromConfig(cfg: AppConfig): ScoringWeights {
  return {
    min_accept_score: cfg.scoring.min_accept_score,
    liquidity: cfg.scoring.weight_liquidity,
    depth:     cfg.scoring.weight_depth,
    safety:    cfg.scoring.weight_safety,
    slippage:  cfg.scoring.weight_slippage,
    gas:       cfg.scoring.weight_gas,
    risk:      cfg.scoring.weight_risk,
  };
}

export type ScoredOpportunity = {
  opportunity_id: string;
  score: number;
  factors: Factors;
};

export function scoreOpportunity(
  opp: Opportunity,
  sim: SimulationResult | null,
  safety_score: number,
  weights: ScoringWeights,
  maxGasPriceGwei: number,
): ScoredOpportunity {
  const f = computeFactors(opp, sim, safety_score, maxGasPriceGwei);
  const raw =
    f.liquidity * weights.liquidity +
    f.depth     * weights.depth +
    f.safety    * weights.safety +
    f.slippage  * weights.slippage +
    f.gas       * weights.gas +
    f.risk      * weights.risk;
  const score = Math.round(raw * 100) / 100;
  return { opportunity_id: opp.id, score, factors: f };
}
