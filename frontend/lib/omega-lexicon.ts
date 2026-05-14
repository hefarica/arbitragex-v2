/**
 * OMEGA Lexicón Absoluto — Presentation Layer Translation.
 *
 * This module provides PRESENTATION-ONLY translations from raw backend
 * field names (which use legacy financial terminology) to the QuantumX
 * OMEGA academic lexicon. It NEVER modifies API request/response shapes.
 *
 * Usage: import { omegaLabel } from "@/lib/omega-lexicon";
 *        <span>{omegaLabel("expected_profit_usd")}</span>
 *
 * The backend contract (schemas.ts field names, API endpoints, DB columns)
 * remains 100% untouched. This is a cosmetic translation layer only.
 */

/** Strategy kind → OMEGA display name */
const STRATEGY_LABELS: Record<string, string> = {
  dex_arb: "DEX Convergence",
  triangular_arb: "Triangular Resolution",
  flashloan_arb: "Flash Convergence",
  liquidation: "Entropy Liquidation",
  sandwich: "Temporal Reordering Defense",
  jit_liquidity: "JIT Liquidity Provisioning",
  cex_dex: "Cross-Domain Convergence",
  micro_arb: "Micro-Resolution HFT",
  cross_chain: "Cross-Topology Resolution",
};

/** Raw backend field → OMEGA display label */
const FIELD_LABELS: Record<string, string> = {
  // Profit → Yield / Rendimiento
  expected_profit_usd: "Expected Yield (USD)",
  net_expected_profit_usd: "Net Expected Yield (USD)",
  actual_profit_usd: "Realized Yield (USD)",
  simulated_net_profit_usd: "Simulated Net Yield (USD)",
  net_profit_wei: "Net Yield (wei)",
  min_profit_usd: "Min Yield Threshold (USD)",
  avg_profit_usd: "Avg Yield (USD)",
  avg_pnl_included_usd: "Avg Convergence Yield (USD)",
  profit_cumulative_usd: "Cumulative Yield (USD)",
  simulation_target_profit_usd: "Simulation Target Yield (USD)",
  capital_cost_rate_annual_pct: "Capital Entropy Rate (%/yr)",

  // Trading → Convergence
  trading_config: "Convergence Config",

  // ROI → Convergence Ratio
  roi_pct: "Convergence Ratio (%)",
  min_roi_pct: "Min Convergence Ratio (%)",

  // Risk → Entropy
  risk_score: "Entropy Score",
  risk_level: "Entropy Level",
  max_token_risk_score: "Max Token Entropy",

  // Strategy → Resolution Engine
  strategy_kind: "Resolution Engine",
  enabled_strategies: "Active Engines",

  // MEV → Resolution Protocol
  mev: "Resolution Protocol",

  // Wallet → Observer
  wallet: "Observer",
  wallets: "Observers",

  // Arbitrage → Convergence / Resolution
  arbitrage: "Topological Convergence",
  arb: "Convergence",

  // Paper mode → Ghost Protocol
  paper_mode: "Ghost Protocol",
  paper_viable: "Ghost-Viable",
  paper_rejected: "Ghost-Rejected",
  paper_status: "Ghost Status",

  // Slippage → Phase Drift
  max_slippage_pct: "Max Phase Drift (%)",
  slippage: "Phase Drift",

  // Gas → Execution Entropy
  max_gas_usd: "Max Execution Entropy (USD)",
  gas_used: "Execution Entropy Consumed",

  // Sandwich → Temporal Reordering
  sandwich: "Temporal Reordering",
  anti_sandwich: "Anti-Reordering Shield",

  // Flashloan → Flash Convergence
  flashloan_fee_pct: "Flash Convergence Fee (%)",
  requires_flashloan: "Requires Flash Convergence",

  // Bundle → Resolution Bundle
  bundle_hash: "Resolution Bundle Hash",

  // Opportunity → Signal
  opportunity: "Convergence Signal",
  opportunities: "Convergence Signals",
};

/** Category labels for strategy catalog */
const CATEGORY_LABELS: Record<string, string> = {
  "direct-arb": "Direct Convergence",
  "complex-arb": "Complex Convergence",
  "lending-liquidation": "Lending Entropy Resolution",
  "yield-temporal": "Temporal Yield Extraction",
  "defensive": "Defensive Protocol",
};

/**
 * Translate a raw backend field name to its OMEGA display label.
 * Returns the original if no translation exists (fail-safe).
 */
export function omegaLabel(rawField: string): string {
  return FIELD_LABELS[rawField] ?? rawField;
}

/**
 * Translate a strategy_kind value to its OMEGA display name.
 * Returns the original with underscores replaced if no translation exists.
 */
export function omegaStrategy(strategyKind: string): string {
  return STRATEGY_LABELS[strategyKind] ?? strategyKind.replace(/_/g, " ");
}

/**
 * Translate a strategy category to its OMEGA display name.
 */
export function omegaCategory(category: string): string {
  return CATEGORY_LABELS[category] ?? category;
}

/**
 * Bulk-translate an object's keys for display purposes.
 * Does NOT mutate the original — returns a new display-ready map.
 */
export function omegaDisplayMap(raw: Record<string, unknown>): Record<string, unknown> {
  const result: Record<string, unknown> = {};
  for (const [key, value] of Object.entries(raw)) {
    result[omegaLabel(key)] = value;
  }
  return result;
}
