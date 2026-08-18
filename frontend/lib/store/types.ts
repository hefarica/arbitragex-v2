/**
 * =============================================================================
 * OMEGA OMNI-STORE TYPES — ViewModel Pattern
 * =============================================================================
 *
 * This file defines the canonical types for the Omni-Store.
 * These types represent the "ViewModel" - data ready for UI consumption.
 *
 * Pattern:
 *   API Response → mapToOmniOpportunity() → OmniOpportunity → Store → UI
 *
 * This decouples the UI from API changes and provides defensive defaults.
 */

// =============================================================================
// Primitive Types (mirrored from shared-ts/api-contracts.ts)
// =============================================================================

/** Phase 1 Token Validation Engine block */
export interface TokenValidationBlock {
  status:
    | "VERIFIED"
    | "VIABLE"
    | "LOW_LIQUIDITY"
    | "ILLIQUID"
    | "NO_DATA"
    | "INVALID"
    | "PENDING";
  score: number;
  liquidity_usd: number | null;
  volume_24h_usd: number | null;
  pair_count: number | null;
  primary_dex: string | null;
  registry_source: string | null;
  validated_at: string;
  reasons: Array<{ key: string; delta: number; note: string }> | null;
}

/** Token metadata with validation info */
export interface TokenInfo {
  symbol: string | null;
  decimals: number | null;
  logo_url: string | null;
  resolved_via: "onchain_full" | "onchain_partial" | "trustwallet_only" | "failed";
  verified?: boolean;
  registry_symbol?: string | null;
  registry_name?: string | null;
  verified_notes?: string[] | null;
  validation?: TokenValidationBlock | null;
}

/**
 * Strategy kinds supported by the MEV engine. The backend sends 269 canonical
 * values (5 base families + 264 cartridge IDs — see shared-ts strategy-kinds.ts
 * and frontend lib/strategy-kinds.ts). `string` mirrors the permissive Zod in
 * schemas.ts and keeps every comparison (`=== "dex_arb"`) working.
 */
export type StrategyKind = string;

/** Opportunity lifecycle status */
export type OpportunityStatus =
  | "detected"
  | "validated"
  | "simulated"
  | "scored"
  | "executing"
  | "executed"
  | "reconciled"
  | "rejected"
  | "failed";

/** Simulated cost breakdown from Rust spine */
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

/** Simulated target from strategy config */
export interface SimulatedTarget {
  target_net_usd: number | null;
  target_roi_pct: number | null;
  target_source: "strategy_config" | "simulation_tab";
  binding_floor:
    | "usd-floor"
    | "roi-floor"
    | "roi-unreachable"
    | "net-per-usd-nonpositive"
    | "tie";
  estimation_basis: "observed-gross" | "roi-assumed";
  required_amount_in_usd: number;
  cap_amount_in_usd: number;
  suggested_amount_in_usd: number;
  suggested_net_usd: number;
  suggested_roi_pct: number;
  meets_target_at_cap: boolean;
  notes: string[];
}

// =============================================================================
// Route Metadata — multi-hop topology (migration 099, G-SIM-1 B2b)
// =============================================================================

/**
 * Persistent route topology stored as JSONB in `opportunities.route_metadata`.
 * Mirrors `shared_rs::candidates::RouteMetadata`. Null when the column is empty
 * (`'{}'`) — legacy rows or detection-time failures (R8 fail-honest).
 *
 * All arrays are parallel and ordered by hop:
 *   - `token_addresses`   length = hops + 1 (A → … → close)
 *   - `dex_adapters`      length = hops     (router/DEX label per leg)
 *   - `pool_addresses`    length = hops     (pool per leg; may contain "" when
 *                                           only the factory was known at scan)
 *   - `decimals`          address → uint8   (may be partial/empty)
 */
export interface RouteMetadataWire {
  token_addresses: string[];
  pool_addresses: string[];
  dex_adapters: string[];
  decimals?: Record<string, number>;
}

/**
 * A single resolved leg of the A→B cycle for UI rendering.
 * Honest: `pool` is "" when the scanner only knew the factory (R8).
 */
export interface RouteLeg {
  /** Hop index, 0-based. */
  index: number;
  /** Input token address for this leg. */
  token_in: string;
  /** Output token address for this leg (next token in the path). */
  token_out: string;
  /** DEX/router label (uniswap_v2_router, sushiswap, …). Honest "" when unknown. */
  dex: string;
  /** Pool address for this leg. Honest "" when only the factory was known. */
  pool: string;
}

// =============================================================================
// OmniOpportunity — The Canonical ViewModel
// =============================================================================

/**
 * The unified opportunity type for the Omni-Store.
 * Combines API fields with UI-computed fields.
 *
 * R8 Fail-Honest: All optional fields default to null, never fabricated.
 */
export interface OmniOpportunity {
  // === Core Identity ===
  id: string;
  chain_id: number;
  strategy_kind: StrategyKind;
  detected_at: string;
  trace_id: string;

  // === Route Information ===
  dex_a: string;
  dex_b: string | null;
  pair_symbol: string | null;
  token_in: string;
  token_out: string;
  amount_in_wei: string;

  // === Token Metadata (UI-enriched) ===
  token_in_info: TokenInfo | null;
  token_out_info: TokenInfo | null;
  chain_base_token_symbol: string | null;
  /**
   * F2 (audit §11 RC1): symbols for INTERMEDIATE route legs (multi-hop
   * cycles), hydrated by the api-server from the tokens table + on-demand
   * eth_call. Keyed by lowercased address; only resolved addresses present.
   * null/absent on legacy payloads → the card's honest shortAddr fallback.
   */
  leg_symbols?: Record<string, string> | null;

  // === Profit Metrics ===
  expected_profit_usd: number | null;
  net_expected_profit_usd: number | null;
  roi_pct: number | null;
  risk_score: number | null;

  // === Status ===
  status: OpportunityStatus;
  rejection_reason: string | null;
  paper_status: "paper_viable" | "paper_rejected" | null;

  // === Block Context ===
  block_number: number | null;

  // === Cross-Chain (for bridge arbitrage) ===
  chain_id_out: number | null;
  bridge: string | null;
  bridge_fee_usd: number | null;
  chains_used: number[];
  dexes_used: string[];

  // === Multi-hop Route Topology (migration 099) ===
  // Full A→B cycle (2..N legs). Null for legacy rows / detection failures —
  // callers fall back to dex_a/dex_b. Drives the per-leg route ledger.
  route_metadata: RouteMetadataWire | null;

  // === Simulation Results (Rust spine) ===
  simulated_net_profit_usd: number | null;
  simulated_amount_in_usd: number | null;
  simulated_roi_pct: number | null;
  simulated_cost_breakdown: SimulatedCostBreakdown | null;
  simulated_target: SimulatedTarget | null;
  simulated_at: string | null;
  simulated_notes: string[] | null;

  // === Confidence & Gas (UI display) ===
  confidence_score_bps: number | null;
  gas_used: number | null;
}

// =============================================================================
// Mapper Function — API Response → OmniOpportunity
// =============================================================================

/**
 * Transforms raw API data into a sanitized OmniOpportunity.
 * Provides defensive defaults for missing fields.
 *
 * @param raw - The raw opportunity from API/WebSocket
 * @returns A sanitized OmniOpportunity ready for the store
 */
export function mapToOmniOpportunity(raw: Record<string, unknown>): OmniOpportunity {
  return {
    // Core Identity
    id: String(raw.id ?? ""),
    chain_id: Number(raw.chain_id ?? 0),
    strategy_kind: (raw.strategy_kind as StrategyKind) ?? "dex_arb",
    detected_at: String(raw.detected_at ?? new Date().toISOString()),
    trace_id: String(raw.trace_id ?? ""),

    // Route Information
    dex_a: String(raw.dex_a ?? ""),
    dex_b: raw.dex_b != null ? String(raw.dex_b) : null,
    pair_symbol: raw.pair_symbol != null ? String(raw.pair_symbol) : null,
    token_in: String(raw.token_in ?? ""),
    token_out: String(raw.token_out ?? ""),
    amount_in_wei: String(raw.amount_in_wei ?? "0"),

    // Token Metadata
    token_in_info: (raw.token_in_info as TokenInfo) ?? null,
    token_out_info: (raw.token_out_info as TokenInfo) ?? null,
    chain_base_token_symbol:
      raw.chain_base_token_symbol != null ? String(raw.chain_base_token_symbol) : null,

    // Profit Metrics
    expected_profit_usd:
      raw.expected_profit_usd != null ? Number(raw.expected_profit_usd) : null,
    net_expected_profit_usd:
      raw.net_expected_profit_usd != null ? Number(raw.net_expected_profit_usd) : null,
    roi_pct: raw.roi_pct != null ? Number(raw.roi_pct) : null,
    risk_score: raw.risk_score != null ? Number(raw.risk_score) : null,

    // Status
    status: (raw.status as OpportunityStatus) ?? "detected",
    rejection_reason: raw.rejection_reason != null ? String(raw.rejection_reason) : null,
    paper_status: (raw.paper_status as "paper_viable" | "paper_rejected") ?? null,

    // Block Context
    block_number: raw.block_number != null ? Number(raw.block_number) : null,

    // Cross-Chain
    chain_id_out: raw.chain_id_out != null ? Number(raw.chain_id_out) : null,
    bridge: raw.bridge != null ? String(raw.bridge) : null,
    bridge_fee_usd:
      raw.bridge_fee_usd != null ? Number(raw.bridge_fee_usd) : null,
    chains_used: Array.isArray(raw.chains_used)
      ? (raw.chains_used as number[])
      : [],
    dexes_used: Array.isArray(raw.dexes_used)
      ? (raw.dexes_used as string[])
      : [],
    // Multi-hop route topology (migration 099). Coerce the wire JSONB into the
    // typed shape; null when absent/empty (R8 fail-honest). Arrays default to []
    // so downstream renderers never hit `undefined`.
    route_metadata: parseRouteMetadata(raw.route_metadata),

    // Simulation Results
    simulated_net_profit_usd:
      raw.simulated_net_profit_usd != null
        ? Number(raw.simulated_net_profit_usd)
        : null,
    simulated_amount_in_usd:
      raw.simulated_amount_in_usd != null
        ? Number(raw.simulated_amount_in_usd)
        : null,
    simulated_roi_pct:
      raw.simulated_roi_pct != null ? Number(raw.simulated_roi_pct) : null,
    simulated_cost_breakdown:
      (raw.simulated_cost_breakdown as SimulatedCostBreakdown) ?? null,
    simulated_target: (raw.simulated_target as SimulatedTarget) ?? null,
    simulated_at: raw.simulated_at != null ? String(raw.simulated_at) : null,
    simulated_notes: Array.isArray(raw.simulated_notes)
      ? (raw.simulated_notes as string[])
      : null,

    // Confidence & Gas
    confidence_score_bps:
      raw.confidence_score_bps != null ? Number(raw.confidence_score_bps) : null,
    gas_used: raw.gas_used != null ? Number(raw.gas_used) : null,
  };
}

// =============================================================================
// Route helpers — topology → UI legs
// =============================================================================

/**
 * Coerces the raw `route_metadata` JSONB into the typed wire shape.
 * Returns null when absent, non-object, or an empty object (R8 fail-honest:
 * no topology = no topology; never a half-fabricated `RouteMetadataWire`).
 */
export function parseRouteMetadata(
  raw: unknown,
): RouteMetadataWire | null {
  if (raw == null || typeof raw !== "object") return null;
  const obj = raw as Record<string, unknown>;
  const tokenAddresses = Array.isArray(obj.token_addresses)
    ? (obj.token_addresses as unknown[]).filter((s): s is string => typeof s === "string")
    : [];
  const dexAdapters = Array.isArray(obj.dex_adapters)
    ? (obj.dex_adapters as unknown[]).filter((s): s is string => typeof s === "string")
    : [];
  const poolAddresses = Array.isArray(obj.pool_addresses)
    ? (obj.pool_addresses as unknown[]).filter((s): s is string => typeof s === "string")
    : [];
  // Require at least one hop + a closing token to be meaningful.
  if (dexAdapters.length === 0 || tokenAddresses.length < 2) return null;
  const decimals =
    obj.decimals != null && typeof obj.decimals === "object"
      ? (obj.decimals as Record<string, number>)
      : undefined;
  return {
    token_addresses: tokenAddresses,
    dex_adapters: dexAdapters,
    pool_addresses: poolAddresses,
    decimals,
  };
}

/**
 * Resolves the ordered legs of the A→B cycle for UI rendering.
 *
 * Priority:
 *   1. `route_metadata` — the full persisted topology (2..N legs).
 *   2. Fallback from `dex_a`/`dex_b` + `token_in`/`token_out` — a synthetic
 *      2-leg BUY→SELL cycle so the operator always sees the route shape even
 *      for legacy rows. Honest "—" dex labels when both are blank.
 *
 * R8: returns an empty array only when there is genuinely no route to show.
 */
export function deriveLegs(opp: OmniOpportunity): RouteLeg[] {
  const rm = opp.route_metadata;
  if (rm && rm.dex_adapters.length > 0 && rm.token_addresses.length >= 2) {
    const legs: RouteLeg[] = [];
    const hops = rm.dex_adapters.length;
    for (let i = 0; i < hops; i++) {
      const tokenIn = rm.token_addresses[i] ?? "";
      const tokenOut = rm.token_addresses[i + 1] ?? "";
      legs.push({
        index: i,
        token_in: tokenIn,
        token_out: tokenOut,
        dex: rm.dex_adapters[i] ?? "",
        pool: rm.pool_addresses[i] ?? "",
      });
    }
    return legs;
  }

  // Fallback: synthetic 2-leg BUY→SELL cycle from the minimal Opportunity.
  const dexA = opp.dex_a ?? "";
  const dexB = opp.dex_b ?? "";
  if (!dexA && !dexB) return [];
  return [
    {
      index: 0,
      token_in: opp.token_in,
      token_out: opp.token_out,
      dex: dexA,
      pool: "",
    },
    {
      index: 1,
      token_in: opp.token_out,
      token_out: opp.token_in,
      dex: dexB || dexA, // single-DEX 2-pool cycle
      pool: "",
    },
  ];
}

