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

/**
 * Operator-configured target floors. Both `net_usd` and `roi_pct` are MINIMUMS
 * (pisos): the route may exceed either, but to surface a sizing suggestion
 * both must be satisfiable simultaneously (AND semantics — `que satisfaga
 * ambos`). At least one of the two must be present (otherwise resolveTarget
 * returns null and inverse sizing is skipped).
 */
export interface SimulationTarget {
  /** Minimum net profit in USD the operator wants to make. */
  net_usd: number | null;
  /** Minimum return-on-investment percent (e.g. 2 means 2%). */
  roi_pct: number | null;
  source: "strategy_config" | "simulation_tab";
}

export interface InverseSizingResult {
  /** Echo of the operator's USD floor (null when only ROI was configured). */
  target_net_usd: number | null;
  /** Echo of the operator's ROI floor in percent (null when only USD was configured). */
  target_roi_pct: number | null;
  target_source: "strategy_config" | "simulation_tab";
  /**
   * Which floor binds at the suggested amount (i.e. which one fixed the
   * minimum required investment). One of:
   *   "usd-floor" — `(target_usd + fixed)/r` was the larger requirement.
   *   "roi-floor" — `fixed/(r − roi)` was the larger requirement.
   *   "roi-unreachable" — the observed/assumed `r` ≤ `roi_floor`, no finite
   *                       amount can satisfy the ROI floor (R8 fail-honest).
   *   "net-per-usd-nonpositive" — route doesn't even cover variable costs.
   *   "tie" — both required amounts equal within floating-point tolerance.
   */
  binding_floor:
    | "usd-floor"
    | "roi-floor"
    | "roi-unreachable"
    | "net-per-usd-nonpositive"
    | "tie";
  /**
   * Amount_in (USD) the linear model says is needed to satisfy BOTH floors,
   * BEFORE capital cap. Infinity when binding_floor is "roi-unreachable" or
   * "net-per-usd-nonpositive".
   */
  required_amount_in_usd: number;
  /** Effective capital cap for this (token, strategy) pair from trading_config. */
  cap_amount_in_usd: number;
  /** min(required, cap) — what we'd actually trade with. */
  suggested_amount_in_usd: number;
  /** Forward-simulated net at suggested_amount_in_usd. */
  suggested_net_usd: number;
  suggested_roi_pct: number;
  /** True when required ≤ cap (operator's capital config admits both floors). */
  meets_target_at_cap: boolean;
  /**
   * Estimation provenance:
   *   "observed-gross" — `r` came from the worker's recorded gross (preferred).
   *   "roi-assumed"    — no gross recorded; `r` was assumed = operator's ROI
   *                      floor (conservative — what you've told the system to
   *                      expect from the strategy). Less precise; clearly
   *                      labeled in the dashboard tooltip.
   */
  estimation_basis: "observed-gross" | "roi-assumed";
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
 * Resolve the operator's target floors per opportunity per the priority
 * spec'd in the plan: strategy_configs override → simulation_tab → null.
 *
 * Both `min_profit_usd` and `min_roi_pct` are MINIMUMS (pisos). When both
 * are configured under the strategy card, AND semantics apply in
 * `inverseSize`: the suggested amount must satisfy both. At least one of
 * the two must be > 0 — otherwise no target exists and inverse sizing is
 * skipped.
 *
 * Priority: per-strategy override wins over simulation-tab even when the
 * strategy provides only one of the two pisos. We do NOT mix pisos from
 * different sources (operator confusion risk).
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
  if (scfg) {
    const usd = scfg.min_profit_usd;
    const roi = scfg.min_roi_pct;
    const usdOk = usd != null && Number.isFinite(usd) && usd > 0;
    const roiOk = roi != null && Number.isFinite(roi) && roi > 0;
    if (usdOk || roiOk) {
      return {
        net_usd: usdOk ? usd : null,
        roi_pct: roiOk ? roi : null,
        source: "strategy_config",
      };
    }
  }
  // Priority 2: Simulación-tab globals.
  const stUsd = cfg.simulation_target_profit_usd;
  const stRoi = cfg.simulation_target_roi_pct;
  const stUsdOk = stUsd != null && Number.isFinite(stUsd) && stUsd > 0;
  const stRoiOk = stRoi != null && Number.isFinite(stRoi) && stRoi > 0;
  if (stUsdOk || stRoiOk) {
    return {
      net_usd: stUsdOk ? stUsd : null,
      roi_pct: stRoiOk ? stRoi : null,
      source: "simulation_tab",
    };
  }
  return null;
}

/**
 * Variable cost rate per USD borrowed — components that scale linearly with
 * `amount_in_usd`. Used by both observed-gross and ROI-assumed paths of the
 * inverse sizer so the cost math stays identical.
 */
function varCostRateFromCfg(cfg: TradingConfigSnapshot): number {
  return (
    cfg.flashloan_fee_pct +
    cfg.max_slippage_pct +
    cfg.failure_risk_buffer_pct +
    LP_FEE_FRACTION_DEFAULT +
    (cfg.capital_cost_rate_annual_pct / 100) * (BLOCK_TIME_SECONDS / SECONDS_PER_YEAR)
  );
}

/**
 * Solve the inverse sizing problem for a (USD floor, ROI floor) pair under
 * AND semantics — `que satisfaga ambos`. Returns the smallest `amount_in_usd`
 * such that:
 *
 *     net ≥ T_usd    AND    net/amount ≥ T_roi
 *
 * Reduces to two linear thresholds (both monotone in amount when r > 0):
 *
 *     amount ≥ (T_usd + f) / r          (USD floor)
 *     amount ≥ f / (r − T_roi)          (ROI floor; requires r > T_roi)
 *
 * The binding floor is the one with the larger required amount.
 *
 * Inputs:
 *   r — net-per-USD rate (gross-per-USD minus var-cost rate, possibly minus
 *       gross-scaling drains like p_copied_max and relay_rate baked into
 *       effective gross rate). For observed-gross path this comes from the
 *       worker's row; for roi-assumed path it comes from `target.roi_pct`.
 *   fixedCosts — gas + ops + relay floor (USD, not scaled with amount).
 */
function solveDualFloors(
  r: number,
  fixedCosts: number,
  targetUsd: number | null,
  targetRoiPct: number | null,
): { required: number; binding: InverseSizingResult["binding_floor"] } {
  if (r <= 0) return { required: Infinity, binding: "net-per-usd-nonpositive" };

  const tUsd = targetUsd ?? 0;
  // USD floor requirement (0 when no USD floor configured → trivially met).
  const reqUsd = targetUsd != null && targetUsd > 0
    ? (tUsd + fixedCosts) / r
    : 0;

  // ROI floor (only when configured AND r strictly above the ROI floor).
  // T_roi comes in as a percent (e.g. 2 means 2%) → convert to fraction.
  let reqRoi: number;
  let roiBlocked = false;
  if (targetRoiPct != null && targetRoiPct > 0) {
    const tRoi = targetRoiPct / 100;
    if (r <= tRoi) {
      roiBlocked = true;
      reqRoi = Infinity;
    } else {
      reqRoi = fixedCosts / (r - tRoi);
    }
  } else {
    reqRoi = 0;
  }

  if (roiBlocked) return { required: Infinity, binding: "roi-unreachable" };

  if (reqUsd > reqRoi) return { required: reqUsd, binding: "usd-floor" };
  if (reqRoi > reqUsd) return { required: reqRoi, binding: "roi-floor" };
  return { required: reqUsd, binding: "tie" };
}

export function inverseSize(
  row: SimulatorRow,
  cfg: TradingConfigSnapshot,
  target: SimulationTarget,
  forward: SimulationResult | null,
): InverseSizingResult | null {
  // At least one floor must be configured (otherwise resolveTarget would have
  // returned null upstream — defensive guard).
  if (target.net_usd == null && target.roi_pct == null) return null;

  const varCostRate = varCostRateFromCfg(cfg);
  const relayRate = row.chain_id === ETHEREUM_MAINNET ? 0.05 : 0;
  const capAmountInUsd = effectiveCapitalFor(
    cfg,
    row.token_in_symbol ?? "",
    row.strategy_kind,
  );
  const notes: string[] = [];

  // ── Pick the net-per-usd rate ────────────────────────────────────────────
  let netPerUsd: number;
  let estimationBasis: "observed-gross" | "roi-assumed";
  let fixedCosts: number;
  let fixedCostsWithRelayFloor: number;

  if (forward != null && forward.amount_in_usd > 0) {
    // Observed-gross path: extrapolate linearly from the row's recorded gross.
    const grossPerUsd = forward.gross_usd / forward.amount_in_usd;
    const effectiveGrossRate = grossPerUsd * (1 - cfg.p_copied_max - relayRate);
    netPerUsd = effectiveGrossRate - varCostRate;
    fixedCosts =
      forward.cost_breakdown.gas_usd +
      forward.cost_breakdown.ops_overhead_usd;
    fixedCostsWithRelayFloor =
      fixedCosts + (row.chain_id === ETHEREUM_MAINNET ? 0.5 : 0);
    estimationBasis = "observed-gross";
    notes.push("linear-extrap");
  } else {
    // ROI-assumed path (Path B): no gross recorded — happens when the worker
    // rejected the row before the profit math ran (safety_below_threshold et
    // al). Use the operator's ROI floor as the assumed `gross_per_usd` rate:
    // a conservative pinch that says "the strategy's gross-ROI gate would
    // have accepted this route only if its raw spread was at least T_roi%,
    // so size as if that's all we get".
    //
    // R8 honesty: clearly labeled as `roi-assumed` so the dashboard tooltip
    // distinguishes it from observed-gross extrapolation. When neither a ROI
    // floor nor a forward exist, we cannot extrapolate honestly and skip
    // inverse sizing entirely (caller passes target.roi_pct=null with
    // forward=null → return null below).
    if (target.roi_pct == null || target.roi_pct <= 0) {
      return null;
    }
    const assumedGrossPerUsd = target.roi_pct / 100;
    const effectiveGrossRate =
      assumedGrossPerUsd * (1 - cfg.p_copied_max - relayRate);
    netPerUsd = effectiveGrossRate - varCostRate;
    // Estimate fixed costs from cfg directly (gas + ops). No row-level
    // forward to read them from.
    const gwei = resolveGasPriceGwei(cfg);
    const gasUsd =
      gwei > 0
        ? (cfg.gas_estimate_units * gwei * 1e9) / 1e18 * cfg.base_token_price_usd
        : 0;
    fixedCosts = gasUsd + cfg.ops_overhead_usd_per_attempt;
    fixedCostsWithRelayFloor =
      fixedCosts + (row.chain_id === ETHEREUM_MAINNET ? 0.5 : 0);
    estimationBasis = "roi-assumed";
    notes.push("roi-based-estimate");
    // Honesty note: in Path B the operator's ROI floor IS the assumed gross
    // rate, so the *net* ROI after our cost model will always be below the
    // operator's ROI floor unless the actual route over-delivers. Surface
    // this so the dashboard tooltip doesn't pretend otherwise. We only
    // enforce the USD floor in Path B; the ROI floor is treated as the
    // spine's gross gate (assumed met by virtue of the row's existence).
    if (target.roi_pct != null) notes.push("roi-floor-assumed-by-spine-gate");
  }

  // ── Solve floors (AND semantics in Path A, USD-only in Path B) ───────────
  //
  // In Path A (observed gross): both floors are enforced under AND semantics
  // — required_amount = max(req_usd, req_roi).
  // In Path B (roi-assumed): the ROI floor is the assumed gross rate, so the
  // post-cost net ROI mathematically can't exceed it. Treat the ROI floor as
  // implicitly met by the spine's gate and enforce only the USD floor here.
  const { required, binding } = solveDualFloors(
    netPerUsd,
    fixedCostsWithRelayFloor,
    target.net_usd,
    estimationBasis === "observed-gross" ? target.roi_pct : null,
  );

  if (binding === "net-per-usd-nonpositive" || binding === "roi-unreachable") {
    notes.push(binding);
    return {
      target_net_usd: target.net_usd,
      target_roi_pct: target.roi_pct,
      target_source: target.source,
      binding_floor: binding,
      required_amount_in_usd: required,
      cap_amount_in_usd: capAmountInUsd,
      suggested_amount_in_usd: 0,
      suggested_net_usd: forward?.net_usd ?? 0,
      suggested_roi_pct: 0,
      meets_target_at_cap: false,
      estimation_basis: estimationBasis,
      notes,
    };
  }

  const meetsTargetAtCap = required <= capAmountInUsd;
  const suggestedAmountInUsd = Math.min(required, capAmountInUsd);

  // Forward-simulate at the suggested amount.
  // Observed-gross path: scale variable costs and gross linearly from forward.
  // ROI-assumed path: use the assumed gross_per_usd directly.
  let suggestedGross: number;
  if (estimationBasis === "observed-gross" && forward) {
    const scale = forward.amount_in_usd > 0
      ? suggestedAmountInUsd / forward.amount_in_usd
      : 0;
    suggestedGross = forward.gross_usd * scale;
  } else {
    const assumed = (target.roi_pct ?? 0) / 100;
    suggestedGross = suggestedAmountInUsd * assumed;
  }
  const suggestedVarCosts = suggestedAmountInUsd * varCostRate;
  const suggestedCopied = suggestedGross * cfg.p_copied_max;
  const suggestedRelay = row.chain_id === ETHEREUM_MAINNET
    ? Math.max(suggestedGross * 0.05, 0.5)
    : 0;
  const suggestedNet =
    suggestedGross - suggestedVarCosts - fixedCosts - suggestedCopied - suggestedRelay;
  const suggestedRoiPct =
    suggestedAmountInUsd > 0 ? (suggestedNet / suggestedAmountInUsd) * 100 : 0;

  if (!meetsTargetAtCap) notes.push("cap-bound");

  return {
    target_net_usd: target.net_usd,
    target_roi_pct: target.roi_pct,
    target_source: target.source,
    binding_floor: binding,
    required_amount_in_usd: required,
    cap_amount_in_usd: capAmountInUsd,
    suggested_amount_in_usd: suggestedAmountInUsd,
    suggested_net_usd: suggestedNet,
    suggested_roi_pct: suggestedRoiPct,
    meets_target_at_cap: meetsTargetAtCap,
    estimation_basis: estimationBasis,
    notes,
  };
}
