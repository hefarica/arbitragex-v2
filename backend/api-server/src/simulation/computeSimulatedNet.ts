/**
 * Target-driven net-profit simulation for `/opportunities/live`.
 *
 * Two pure functions:
 *
 *   forwardSimulate(row, cfg)
 *     Computes the simulated net profit at the row's recorded `amount_in_wei`,
 *     using the operator's `trading_config` cost components. Mirrors the 8-
 *     component formula in `backend/math-engine/src/roi_engine.rs:149-212`.
 *
 *   inverseSize(row, cfg, target)
 *     First-order linear extrapolation: given a target net profit in USD,
 *     solve for the `amount_in_usd` that would achieve it, clamped to the
 *     effective capital cap. Surfaces "borrow $X to hit $Y" for the
 *     dashboard's Net Profit cell.
 *
 * R8 fail-honest: every output carries `source: "simulation"`, notes
 * explain approximations (e.g. linear-extrap), and a row with insufficient
 * inputs (no gross, no token price) returns null so the dashboard renders
 * "—" rather than a fabricated number.
 *
 * Numbers explicitly NOT used:
 *  - revm / on-chain quote — that's simulator-v2's job (A3, Sprint 4)
 *  - p_fail / p_copied from dex_chain_metrics — proxy path only, since the
 *    api-server doesn't query the metrics table on the hot path
 */

import { effectiveCapitalFor, type TradingConfigSnapshot } from "./tradingConfigSnapshot.js";
import { resolveGasPriceGwei } from "./tradingConfigSnapshot.js";

// ── Inputs ─────────────────────────────────────────────────────────────────

/**
 * Subset of OpportunityLiveRow the simulator needs. Keeping a narrow shape
 * here lets the caller pass either a PG result row or a hand-built test
 * fixture without re-declaring the full schema.
 */
export interface SimulatorRow {
  chain_id: number;
  strategy_kind: string;
  amount_in_wei: string;
  expected_profit_usd: number | null;
  token_in: string;
  token_in_symbol: string | null;
  token_in_decimals: number | null;
}

// ── Outputs ─────────────────────────────────────────────────────────────────

export interface SimulatedCostBreakdown {
  gas_usd: number;
  lp_fees_usd: number;
  slippage_usd: number;
  failure_buffer_usd: number;
  copied_buffer_usd: number;
  capital_cost_usd: number;
  ops_overhead_usd: number;
  flashloan_fee_usd: number;
  relay_fee_usd: number;
}

export interface SimulationResult {
  amount_in_usd: number;
  gross_usd: number;
  net_usd: number;
  roi_pct: number;
  cost_breakdown: SimulatedCostBreakdown;
  source: "simulation";
  notes: string[];
}

export interface SimulationTarget {
  net_usd: number;
  source: "strategy_config" | "simulation_tab";
}

export interface InverseSizingResult {
  target_net_usd: number;
  target_source: "strategy_config" | "simulation_tab";
  /** Amount_in (USD) the linear model says is needed to hit target_net_usd, BEFORE capital cap. */
  required_amount_in_usd: number;
  /** Effective capital cap for this (token, strategy) pair from trading_config. */
  cap_amount_in_usd: number;
  /** min(required, cap) — what we'd actually trade with. */
  suggested_amount_in_usd: number;
  /** Forward-simulated net at suggested_amount_in_usd. */
  suggested_net_usd: number;
  suggested_roi_pct: number;
  /** True when required ≤ cap (operator's capital config admits the target). */
  meets_target_at_cap: boolean;
  notes: string[];
}

// ── Constants ──────────────────────────────────────────────────────────────

/** Default V2 LP fee in fraction (30 bps). */
const LP_FEE_FRACTION_DEFAULT = 0.003;
/** Approximate Ethereum block time in seconds (used by capital-cost calc). */
const BLOCK_TIME_SECONDS = 12;
/** Seconds per year. */
const SECONDS_PER_YEAR = 31_536_000;
/** Ethereum mainnet chain id. */
const ETHEREUM_MAINNET = 1;

// ── Helpers ────────────────────────────────────────────────────────────────

/**
 * Resolve the USD price for a token using the operator's price table or
 * the base-token shortcut. Returns null when we cannot price the token.
 */
function tokenUsdPrice(cfg: TradingConfigSnapshot, symbol: string | null): number | null {
  if (!symbol) return null;
  const upper = symbol.toUpperCase();
  // First: base token symbol matches → use the canonical operator-set base price.
  if (cfg.base_token_symbol && cfg.base_token_symbol.toUpperCase() === upper) {
    return cfg.base_token_price_usd > 0 ? cfg.base_token_price_usd : null;
  }
  // Second: per-token operator-managed price map (case-insensitive).
  for (const [k, v] of Object.entries(cfg.token_prices_usd)) {
    if (k.toUpperCase() === upper && v > 0) return v;
  }
  return null;
}

/**
 * Compute the USD value of the token_in amount the worker recorded. Returns
 * null when we cannot price (no symbol, no decimals, no price entry).
 */
function amountInUsd(row: SimulatorRow, cfg: TradingConfigSnapshot): number | null {
  if (!row.token_in_symbol || row.token_in_decimals == null) return null;
  const price = tokenUsdPrice(cfg, row.token_in_symbol);
  if (price == null) return null;
  // BigInt for precision on large amounts; convert to float at the last step.
  let amount: number;
  try {
    const bi = BigInt(row.amount_in_wei);
    const decimals = row.token_in_decimals;
    // Avoid Number overflow: divide via BigInt first, then convert.
    const scale = 10n ** BigInt(decimals);
    const whole = Number(bi / scale);
    const frac = Number(bi % scale) / Number(scale);
    amount = whole + frac;
  } catch {
    return null;
  }
  if (!Number.isFinite(amount) || amount <= 0) return null;
  return amount * price;
}

/**
 * Gas cost in USD, mirrors `TradingConfigState::gas_cost_usd` (Rust
 * trading_config.rs:400-404). Uses base_token_price_usd as the ETH→USD
 * conversion (since gas is paid in native ETH on Ethereum, native MATIC
 * on Polygon, etc — and base_token_price_usd is the operator's view of the
 * native asset's USD price).
 */
function gasCostUsd(cfg: TradingConfigSnapshot): number {
  const gwei = resolveGasPriceGwei(cfg);
  if (gwei <= 0) return 0;
  const gasUnits = cfg.gas_estimate_units;
  // gas_units × gas_price_gwei × 1e9 wei/gwei ÷ 1e18 wei/native × native_price
  return (gasUnits * gwei * 1e9) / 1e18 * cfg.base_token_price_usd;
}

/**
 * Cold-start relay fee floor for Ethereum mainnet. Mirrors the spine's
 * fallback when no `arbx:relay_fee_ewma:{chain}:{strategy}` Redis key
 * has been written yet: `max(gross × 5%, $0.50)`. L2s pay no priority
 * bribe (deterministic sequencer inclusion) → 0.
 */
function relayFeeUsd(chainId: number, gross: number): number {
  if (chainId !== ETHEREUM_MAINNET) return 0;
  return Math.max(gross * 0.05, 0.5);
}

// ── Forward simulation ─────────────────────────────────────────────────────

export function forwardSimulate(
  row: SimulatorRow,
  cfg: TradingConfigSnapshot,
): SimulationResult | null {
  const grossUsd = row.expected_profit_usd;
  if (grossUsd == null || !Number.isFinite(grossUsd)) return null;

  const amountInUsdVal = amountInUsd(row, cfg);
  if (amountInUsdVal == null) {
    // Without an amount_in USD we cannot compute variable costs honestly.
    return null;
  }

  const notes: string[] = [];

  // Component 1: gas
  const gas_usd = gasCostUsd(cfg);
  // Component 2: LP fees — default 30bps V2 tier when route legs unknown.
  // The api-server doesn't have per-leg fee tiers; this is a first-order proxy.
  const lp_fees_usd = amountInUsdVal * LP_FEE_FRACTION_DEFAULT;
  if (lp_fees_usd > 0) notes.push("lp-fee=30bps-proxy");

  // Component 3: slippage — use max_slippage_pct against amount_in_usd
  // (we don't have effective amount_out separately; this matches the
  // proxy branch of `roi_engine.rs:160`).
  const slippage_usd = amountInUsdVal * cfg.max_slippage_pct;

  // Component 4: failure buffer (proxy path — no p_fail on the api-server).
  const failure_buffer_usd = amountInUsdVal * cfg.failure_risk_buffer_pct;

  // Component 5: copied buffer — apply the cap as a worst-case proxy.
  // (The spine has the actual p_copied per-pool from dex_chain_metrics; we
  // assume the max as a conservative upper bound.)
  const copied_buffer_usd = grossUsd * cfg.p_copied_max;
  if (cfg.p_copied_max > 0) notes.push(`p-copied-max=${cfg.p_copied_max}`);

  // Component 6: capital opportunity cost.
  const capital_cost_usd =
    amountInUsdVal *
    (cfg.capital_cost_rate_annual_pct / 100) *
    (BLOCK_TIME_SECONDS / SECONDS_PER_YEAR);

  // Component 7: ops/infra overhead per attempt (fixed scalar).
  const ops_overhead_usd = cfg.ops_overhead_usd_per_attempt;

  // Flashloan fee: amount_in × pct.
  const flashloan_fee_usd = amountInUsdVal * cfg.flashloan_fee_pct;

  // Component 8: relay bribe (cold-start floor).
  const relay_fee_usd = relayFeeUsd(row.chain_id, grossUsd);
  if (relay_fee_usd > 0) notes.push("relay-cold-start-floor");

  const net_usd =
    grossUsd -
    gas_usd -
    flashloan_fee_usd -
    lp_fees_usd -
    slippage_usd -
    failure_buffer_usd -
    capital_cost_usd -
    ops_overhead_usd -
    copied_buffer_usd -
    relay_fee_usd;

  const roi_pct = amountInUsdVal > 0 ? (net_usd / amountInUsdVal) * 100 : 0;

  return {
    amount_in_usd: amountInUsdVal,
    gross_usd: grossUsd,
    net_usd,
    roi_pct,
    cost_breakdown: {
      gas_usd,
      lp_fees_usd,
      slippage_usd,
      failure_buffer_usd,
      copied_buffer_usd,
      capital_cost_usd,
      ops_overhead_usd,
      flashloan_fee_usd,
      relay_fee_usd,
    },
    source: "simulation",
    notes,
  };
}

// ── Inverse sizing ─────────────────────────────────────────────────────────

/**
 * Resolve the operator's target net profit per opportunity per the priority
 * spec'd in the plan: strategy_configs override → simulation_tab → null.
 *
 * Treats zero, negative, NaN, undefined as "not set" (R8: never invent a
 * target from a missing config value).
 */
export function resolveTarget(
  cfg: TradingConfigSnapshot,
  strategy_kind: string,
): SimulationTarget | null {
  // Priority 1: per-strategy override.
  const scfg = (() => {
    if (!strategy_kind) return null;
    const needle = strategy_kind.toLowerCase();
    for (const [k, v] of Object.entries(cfg.strategy_configs)) {
      if (k.toLowerCase() === needle) return v;
    }
    return null;
  })();
  const sk = scfg?.min_profit_usd;
  if (sk != null && Number.isFinite(sk) && sk > 0) {
    return { net_usd: sk, source: "strategy_config" };
  }
  // Priority 2: simulation_target_profit_usd from Simulación tab.
  const st = cfg.simulation_target_profit_usd;
  if (st != null && Number.isFinite(st) && st > 0) {
    return { net_usd: st, source: "simulation_tab" };
  }
  return null;
}

export function inverseSize(
  row: SimulatorRow,
  cfg: TradingConfigSnapshot,
  target: SimulationTarget,
  forward: SimulationResult,
): InverseSizingResult | null {
  // gross-per-USD rate observed at the worker's recorded amount_in.
  if (forward.amount_in_usd <= 0) return null;
  const grossPerUsd = forward.gross_usd / forward.amount_in_usd;

  // Variable cost rate per USD borrowed (scales linearly with amount_in_usd):
  //   - flashloan_fee_pct
  //   - max_slippage_pct
  //   - failure_risk_buffer_pct
  //   - lp_fee_fraction (30bps)
  //   - capital_cost rate × block_time/year
  const varCostRate =
    cfg.flashloan_fee_pct +
    cfg.max_slippage_pct +
    cfg.failure_risk_buffer_pct +
    LP_FEE_FRACTION_DEFAULT +
    (cfg.capital_cost_rate_annual_pct / 100) * (BLOCK_TIME_SECONDS / SECONDS_PER_YEAR);

  // The variable copied_buffer and relay_fee scale with GROSS not amount_in.
  // Combined "leakage" against gross: copied + relay (both ratios of gross).
  // For first-order extrapolation, we approximate gross_at_required ≈ required × grossPerUsd
  // and treat the copied/relay drains as adjusted gross rate:
  //   effective_gross_rate = grossPerUsd × (1 − p_copied_max − relay_rate)
  const relayRate = row.chain_id === ETHEREUM_MAINNET ? 0.05 : 0;
  const effectiveGrossRate = grossPerUsd * (1 - cfg.p_copied_max - relayRate);

  // Fixed costs that don't scale with amount_in.
  const fixedCosts =
    forward.cost_breakdown.gas_usd +
    forward.cost_breakdown.ops_overhead_usd;

  // Honest about relay floor:
  // relay_fee = max(gross × 5%, $0.50). When the proportional 5% gives < $0.50,
  // the floor binds and adds another fixed cost. Approximate by adding 0.50
  // to fixed_costs when the EWMA is the floor.
  const fixedCostsWithRelayFloor =
    fixedCosts + (row.chain_id === ETHEREUM_MAINNET ? 0.5 : 0);

  // Solve linear: target_net = amount × (effective_gross_rate − var_cost_rate) − fixed
  //              → required = (target + fixed) / (effective_gross_rate − var_cost_rate)
  const netPerUsd = effectiveGrossRate - varCostRate;
  const notes: string[] = ["linear-extrap"];

  if (netPerUsd <= 0) {
    // The opp's gross rate doesn't even cover variable costs — no amount of
    // capital makes this profitable. R8: surface the truth.
    notes.push("net-per-usd-nonpositive");
    return {
      target_net_usd: target.net_usd,
      target_source: target.source,
      required_amount_in_usd: Infinity,
      cap_amount_in_usd: effectiveCapitalFor(cfg, row.token_in_symbol ?? "", row.strategy_kind),
      suggested_amount_in_usd: 0,
      suggested_net_usd: forward.net_usd,
      suggested_roi_pct: 0,
      meets_target_at_cap: false,
      notes,
    };
  }

  const requiredAmountInUsd = (target.net_usd + fixedCostsWithRelayFloor) / netPerUsd;
  const capAmountInUsd = effectiveCapitalFor(cfg, row.token_in_symbol ?? "", row.strategy_kind);

  const meetsTargetAtCap = requiredAmountInUsd <= capAmountInUsd;
  const suggestedAmountInUsd = Math.min(requiredAmountInUsd, capAmountInUsd);

  // Forward-simulate at the suggested amount: scale variable costs and gross
  // linearly, keep fixed costs constant. Honest about linearity.
  const scale = forward.amount_in_usd > 0 ? suggestedAmountInUsd / forward.amount_in_usd : 0;
  const suggestedGross = forward.gross_usd * scale;
  const suggestedVarCosts = suggestedAmountInUsd * varCostRate;
  const suggestedCopied = suggestedGross * cfg.p_copied_max;
  const suggestedRelay = row.chain_id === ETHEREUM_MAINNET
    ? Math.max(suggestedGross * 0.05, 0.5)
    : 0;
  const suggestedNet =
    suggestedGross - suggestedVarCosts - fixedCosts - suggestedCopied - suggestedRelay;
  const suggestedRoiPct = suggestedAmountInUsd > 0 ? (suggestedNet / suggestedAmountInUsd) * 100 : 0;

  if (!meetsTargetAtCap) notes.push("cap-bound");

  return {
    target_net_usd: target.net_usd,
    target_source: target.source,
    required_amount_in_usd: requiredAmountInUsd,
    cap_amount_in_usd: capAmountInUsd,
    suggested_amount_in_usd: suggestedAmountInUsd,
    suggested_net_usd: suggestedNet,
    suggested_roi_pct: suggestedRoiPct,
    meets_target_at_cap: meetsTargetAtCap,
    notes,
  };
}
