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
