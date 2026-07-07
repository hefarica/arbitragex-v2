/**
 * Trading-config snapshot loader for the simulation pipeline.
 *
 * Reads the operator's `trading_config` row for a chain from Redis (the same
 * `arbx:trading_config:<chain_id>` key that the searcher subscribes to) and
 * deserialises it into a TypeScript struct mirroring the subset of
 * `shared_rs::trading_config::TradingConfigState` that the simulation needs.
 *
 * Per-request in-process cache 5s — when `/opportunities/live` returns 50
 * rows on the same chain, we hit Redis once, not 50 times. The TTL is short
 * because the operator can edit the config from `/strategies` and expects
 * the simulation to reflect the new values on the next page load.
 *
 * R8 fail-honest: when Redis misses or the JSON is malformed, returns null
 * and the route falls back to no-simulation. Never invents defaults.
 */

import type { Redis } from "ioredis";

export interface StrategyRuntimeConfig {
  enabled: boolean;
  min_profit_usd: number | null;
  min_roi_pct: number | null;
  // Other fields exist on the Rust side but aren't used by the simulation
  // (route_constraints, dex allowlist, etc. live elsewhere).
}

export type GasPriceStrategy = "fixed" | "dynamic_basefee_plus_tip" | "percentile_75";

/**
 * Subset of `TradingConfigState` the simulation needs. Field names match
 * the Rust struct verbatim so a future Rust→TS codegen can swap this out.
 */
export interface TradingConfigSnapshot {
  // ── Capital + denomination ──────────────────────────────────────────────
  capital_usd: number;
  base_token_symbol: string;
  base_token_price_usd: number;
  /** Per-symbol USD prices entered by the operator (token_prices_usd). */
  token_prices_usd: Record<string, number>;

  // ── Gas + slippage strategy ─────────────────────────────────────────────
  gas_estimate_units: number;
  gas_price_strategy: GasPriceStrategy;
  fixed_gas_price_gwei: number | null;
  max_slippage_pct: number;
  failure_risk_buffer_pct: number;
  flashloan_fee_pct: number;

  // ── Cost-component scalars (Sprint A + B + C + C2) ──────────────────────
  capital_cost_rate_annual_pct: number;
  ops_overhead_usd_per_attempt: number;
  p_copied_max: number;
  p_copied_volume_threshold_usd: number;
  spread_sanity_mult: number;

  // ── Strategy mix ────────────────────────────────────────────────────────
  enabled_strategies: string[];
  strategy_configs: Record<string, StrategyRuntimeConfig>;

  // ── Simulación tab knobs ────────────────────────────────────────────────
  simulation_capital_usd: number | null;
  simulation_per_token_amounts_usd: Record<string, number>;
  simulation_per_strategy_caps_usd: Record<string, number>;
  simulation_target_profit_usd: number | null;
  simulation_target_roi_pct: number | null;
}

interface CacheEntry {
  snapshot: TradingConfigSnapshot | null;
  fetched_at: number;
}

const CACHE_TTL_MS = 5_000;
const cache = new Map<number, CacheEntry>();

function redisKey(chain_id: number): string {
  return `arbx:trading_config:${chain_id}`;
}

/**
 * Type-narrow helpers — Redis returns `unknown`; we coerce defensively so
 * a partial / legacy row never crashes the simulator. Missing fields use
 * doctrinal defaults (these mirror the `serde(default = ...)` attrs on
 * the Rust struct so behaviour is the same on both sides).
 */
function num(v: unknown, fallback: number): number {
  if (typeof v === "number" && Number.isFinite(v)) return v;
  if (typeof v === "string") {
    const n = Number(v);
    if (Number.isFinite(n)) return n;
  }
  return fallback;
}

function numOrNull(v: unknown): number | null {
  if (v === null || v === undefined) return null;
  if (typeof v === "number" && Number.isFinite(v)) return v;
  if (typeof v === "string") {
    const n = Number(v);
    if (Number.isFinite(n)) return n;
  }
  return null;
}

function str(v: unknown, fallback: string): string {
  return typeof v === "string" ? v : fallback;
}

function strArray(v: unknown): string[] {
  return Array.isArray(v) ? v.filter((x): x is string => typeof x === "string") : [];
}

function numRecord(v: unknown): Record<string, number> {
  if (!v || typeof v !== "object") return {};
  const out: Record<string, number> = {};
  for (const [k, val] of Object.entries(v as Record<string, unknown>)) {
    const n = num(val, NaN);
    if (Number.isFinite(n)) out[k] = n;
  }
  return out;
}

function strategyConfigRecord(v: unknown): Record<string, StrategyRuntimeConfig> {
  if (!v || typeof v !== "object") return {};
  const out: Record<string, StrategyRuntimeConfig> = {};
  for (const [k, val] of Object.entries(v as Record<string, unknown>)) {
    if (!val || typeof val !== "object") continue;
    const o = val as Record<string, unknown>;
    out[k] = {
      enabled: typeof o["enabled"] === "boolean" ? o["enabled"] : true,
      min_profit_usd: numOrNull(o["min_profit_usd"]),
      min_roi_pct: numOrNull(o["min_roi_pct"]),
    };
  }
  return out;
}

function gasPriceStrategy(v: unknown): GasPriceStrategy {
  if (v === "fixed" || v === "dynamic_basefee_plus_tip" || v === "percentile_75") {
    return v;
  }
  return "dynamic_basefee_plus_tip";
}

function parseSnapshot(raw: string): TradingConfigSnapshot | null {
  let parsed: unknown;
  try {
    parsed = JSON.parse(raw);
  } catch {
    return null;
  }
  if (!parsed || typeof parsed !== "object") return null;
  const o = parsed as Record<string, unknown>;

  return {
    capital_usd: num(o["capital_usd"], 0),
    base_token_symbol: str(o["base_token_symbol"], "WETH"),
    base_token_price_usd: num(o["base_token_price_usd"], 0),
    token_prices_usd: numRecord(o["token_prices_usd"]),

    gas_estimate_units: num(o["gas_estimate_units"], 250_000),
    gas_price_strategy: gasPriceStrategy(o["gas_price_strategy"]),
    fixed_gas_price_gwei: numOrNull(o["fixed_gas_price_gwei"]),
    max_slippage_pct: num(o["max_slippage_pct"], 0.005),
    failure_risk_buffer_pct: num(o["failure_risk_buffer_pct"], 0.001),
    flashloan_fee_pct: num(o["flashloan_fee_pct"], 0.0009),

    capital_cost_rate_annual_pct: num(o["capital_cost_rate_annual_pct"], 0),
    ops_overhead_usd_per_attempt: num(o["ops_overhead_usd_per_attempt"], 0.01),
    p_copied_max: num(o["p_copied_max"], 0.5),
    p_copied_volume_threshold_usd: num(o["p_copied_volume_threshold_usd"], 1_000_000),
    spread_sanity_mult: num(o["spread_sanity_mult"], 3.0),

    enabled_strategies: strArray(o["enabled_strategies"]),
    strategy_configs: strategyConfigRecord(o["strategy_configs"]),

    simulation_capital_usd: numOrNull(o["simulation_capital_usd"]),
    simulation_per_token_amounts_usd: numRecord(o["simulation_per_token_amounts_usd"]),
    simulation_per_strategy_caps_usd: numRecord(o["simulation_per_strategy_caps_usd"]),
    simulation_target_profit_usd: numOrNull(o["simulation_target_profit_usd"]),
    simulation_target_roi_pct: numOrNull(o["simulation_target_roi_pct"]),
  };
}

/**
 * Fetch the trading_config snapshot for a chain. Cached 5s in-process to
 * absorb the typical /opportunities/live request burst (≤50 rows × N
 * pollers). On Redis miss or malformed JSON returns null (caller skips
 * simulation for that chain — R8 fail-honest).
 */
export async function getTradingConfigForChain(
  redis: Redis | null,
  chain_id: number,
): Promise<TradingConfigSnapshot | null> {
  const now = Date.now();
  const cached = cache.get(chain_id);
  if (cached && now - cached.fetched_at < CACHE_TTL_MS) {
    return cached.snapshot;
  }

  if (!redis) {
    cache.set(chain_id, { snapshot: null, fetched_at: now });
    return null;
  }

  let raw: string | null = null;
  try {
    raw = await redis.get(redisKey(chain_id));
  } catch {
    // Redis transient — cache null briefly so we don't spam the failing client.
    cache.set(chain_id, { snapshot: null, fetched_at: now });
    return null;
  }
  const snapshot = raw ? parseSnapshot(raw) : null;
  cache.set(chain_id, { snapshot, fetched_at: now });
  return snapshot;
}

/**
 * Case-insensitive per-token cap lookup, mirrors
 * `TradingConfigState::lookup_per_token_cap` (trading_config.rs:453-462).
 */
function lookupPerTokenCap(cfg: TradingConfigSnapshot, symbol: string): number | null {
  if (!symbol) return null;
  const upper = symbol.toUpperCase();
  for (const [k, v] of Object.entries(cfg.simulation_per_token_amounts_usd)) {
    if (k.toUpperCase() === upper) return v;
  }
  return null;
}

/**
 * Case-insensitive per-strategy cap lookup, mirrors
 * `TradingConfigState::lookup_per_strategy_cap` (trading_config.rs:465-474).
 */
function lookupPerStrategyCap(cfg: TradingConfigSnapshot, strategy_kind: string): number | null {
  if (!strategy_kind) return null;
  const upper = strategy_kind.toUpperCase();
  for (const [k, v] of Object.entries(cfg.simulation_per_strategy_caps_usd)) {
    if (k.toUpperCase() === upper) return v;
  }
  return null;
}

/**
 * Effective capital cap for a (token_symbol, strategy_kind) pair, MIN of all
 * applicable caps. Direct TS port of `TradingConfigState::effective_capital_for`
 * (trading_config.rs:432-450).
 */
export function effectiveCapitalFor(
  cfg: TradingConfigSnapshot,
  token_symbol: string,
  strategy_kind: string,
): number {
  const tokenCap = lookupPerTokenCap(cfg, token_symbol);
  const strategyCap = lookupPerStrategyCap(cfg, strategy_kind);
  const globalCap = cfg.simulation_capital_usd;

  const applicable: number[] = [];
  if (globalCap != null) applicable.push(globalCap);
  if (tokenCap != null) applicable.push(tokenCap);
  if (strategyCap != null) applicable.push(strategyCap);

  if (applicable.length === 0) return cfg.capital_usd;
  return Math.min(...applicable);
}

/**
 * Per-strategy config lookup, case-insensitive on the key. Mirrors
 * `TradingConfigState::strategy_config` (trading_config.rs:485-494).
 */
export function strategyConfigFor(
  cfg: TradingConfigSnapshot,
  strategy_kind: string,
): StrategyRuntimeConfig | null {
  if (!strategy_kind) return null;
  const needle = strategy_kind.toLowerCase();
  for (const [k, v] of Object.entries(cfg.strategy_configs)) {
    if (k.toLowerCase() === needle) return v;
  }
  return null;
}

/**
 * Resolve the gas price in gwei using the operator's strategy. When no live
 * signals are available (api-server doesn't subscribe to the basefee feed
 * yet), passes 0 for both — `Fixed` falls back to `fixed_gas_price_gwei`,
 * `DynamicBasefeePlusTip` returns 1.0 (the minimum tip), `Percentile75`
 * returns 0. Same conservative path the workers use.
 */
export function resolveGasPriceGwei(cfg: TradingConfigSnapshot): number {
  switch (cfg.gas_price_strategy) {
    case "fixed":
      return cfg.fixed_gas_price_gwei ?? 0;
    case "dynamic_basefee_plus_tip":
      // No live basefee available here → just the operator's tip floor.
      // Bounded at 1.0 gwei mirrors the Rust max(_, 1.0) on the tip term.
      return 1.0;
    case "percentile_75":
      return 0;
  }
}

/** Test-only helper to clear the in-process cache. */
export function _clearSnapshotCacheForTests(): void {
  cache.clear();
}
