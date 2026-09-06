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
 *
 * HOPS-LEDGER-04 — the per-leg ledger is OPTIONAL and all-or-nothing (mirrors
 * `attach_leg_ledger` in shared-rs): present ONLY on rows the sizing kernel
 * computed leg outputs for (Sized 2-leg V2/V3). Absent = not computed (R8) —
 * never interpret absence as zero, and NEVER attach amounts to §29 synthetic
 * legs:
 *   - `leg_amounts_in`    length = hops     (exact wei entering leg i)
 *   - `leg_amounts_out`   length = hops     (exact wei leaving leg i)
 *   - `leg_zero_for_one`  length = hops     (Uniswap token0→token1 convention)
 */
export interface RouteMetadataWire {
  token_addresses: string[];
  pool_addresses: string[];
  dex_adapters: string[];
  decimals?: Record<string, number>;
  leg_amounts_in?: string[];
  leg_amounts_out?: string[];
  leg_zero_for_one?: boolean[];
}

/**
 * FE-0028 (§19 hop arithmetic): hop count FROM the persisted topology —
 * `route_metadata.dex_adapters` carries one entry per leg, so its length IS
 * the hop count. Null when there is no topology (legacy rows / detection
 * failures) — absence is a state, never a zero (R8).
 */
export function deriveHopCount(rm: RouteMetadataWire | null): number | null {
  if (!rm || rm.dex_adapters.length === 0) return null;
  return rm.dex_adapters.length;
}

/**
 * A single resolved leg of the A→B cycle for UI rendering.
 * Honest: `pool` is "" when the scanner only knew the factory (R8).
 * FE-0030 (§29): legs produced by the LEGACY synthetic fallback carry
 * `synthetic: true` — renderers MUST surface the SYNTHETIC LEGACY VIEW marker
 * and never present them as ROUTE VERIFIED or operational hops.
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
  /** Present ONLY on legacy synthetic-fallback legs (FE-0030 §29). */
  synthetic?: true;
}

/** §29 canonical marker string — one source of truth for every renderer. */
export const SYNTHETIC_LEGACY_VIEW_LABEL = "SYNTHETIC LEGACY VIEW";

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
  // FE-0029 (§28 fail-honest): the wire contract makes these MANDATORY
  // (opportunities columns are NOT NULL on every serving path), so a null here
  // means the payload was malformed/absent — the mapper NEVER papers over it
  // with a semantic default. Old fabrications removed: missing→"dex_arb",
  // missing→now(), missing→"detected", missing→chain 0, missing→"0" wei.
  // Renderers show "—"/UNKNOWN; nothing pretends the row was a dex_arb
  // detected right now on chain 0.
  id: string;
  chain_id: number | null;
  strategy_kind: StrategyKind | null;
  detected_at: string | null;
  trace_id: string;

  // === Extended Identity (FE-0028 §26 §27 — NO parallel model) ===
  // Extends OmniOpportunity in place. Two fields are REAL today; the rest are
  // level-(b) nullable gaps the mapper pins to null until their wire exists —
  // never fabricated (RULE 00 / R8).
  /**
   * Cartridge that detected this opportunity. ON THE WIRE today:
   * `opportunities.cartridge_id` (api-server opportunities-live SELECT → row
   * mapping). Null on legacy rows / non-cartridge detections.
   */
  cartridge_id: string | null;
  /**
   * Cycle length (§19 hop arithmetic) — DERIVED from the persisted topology
   * (`route_metadata.dex_adapters.length` via `deriveHopCount`). Null when
   * there is no route_metadata (legacy rows / detection failures).
   */
  hop_count: number | null;
  // ── Level-(b) gaps: no emitido en el wire — añadir al mapper cuando el
  //    backend lo persista/seleccione. null = "aún no servido", nunca un
  //    placeholder inventado (R8).
  /** Scanner route fingerprint (scanner.rs candidate_id) — not persisted/selected today. */
  candidate_id: string | null;
  /** Persistent route id — not on the opportunities wire today. */
  route_id: string | null;
  /** Pair id — not on the opportunities wire today. */
  pair_id: string | null;
  /** Detector id — not on the opportunities wire today. */
  detector_id: string | null;
  /** Quote/payout token address — not on the opportunities wire today. */
  quote_token: string | null;
  /** Quote graph/config/strategy versions — not on the opportunities wire today. */
  quote_version: number | null;
  graph_version: number | null;
  config_version: number | null;
  strategy_version: number | null;
  /** Per-gate pass/fail detail from the risk/evidence gauntlet — not emitted today. */
  gate_results: Record<string, unknown> | null;
  /** Data-quality flags (stale feeds, partial reserves, …) — not emitted today. */
  data_quality: Record<string, unknown> | null;
  // NOTE: "economía completa" already lives on this type — expected/net/roi/
  // risk below plus the full simulated_* block (cost breakdown, target,
  // notes). FE-0028 adds identity, not a second economics model.

  // === Route Information ===
  dex_a: string;
  dex_b: string | null;
  pair_symbol: string | null;
  token_in: string;
  token_out: string;
  /** Wire-mandatory; null = malformed payload (§28) — never a fabricated "0". */
  amount_in_wei: string | null;

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
  /** Wire-mandatory lifecycle; null = malformed payload (§28) — not "detected". */
  status: OpportunityStatus | null;
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

  // === Semantic verdict (FE-0031 §30) ===
  // Computed by the mapper via validateOpportunitySemantics(): the §30
  // semantic violations found on this row. EMPTY = validated clean. Non-empty
  // ⇒ the row renders QUARANTINED (marked, never hidden — §30 quarantine is a
  // visible state, not deletion).
  semantic_violations: SemanticViolation[];
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
  // Parsed once here so hop_count can derive from the SAME topology object the
  // ViewModel carries (FE-0028 §19).
  const routeMetadata = parseRouteMetadata(raw.route_metadata);
  const mapped: OmniOpportunity = {
    // Core Identity — FE-0029 (§28): wire-mandatory fields map to null when
    // the payload omits them. The old defaults (dex_arb / now() / detected /
    // chain 0) fabricated a coherent-looking row out of a malformed one —
    // worst case: a missing detected_at re-stamped NOW on every remap, so the
    // TTL prune saw age 0 and the card lived forever.
    id: String(raw.id ?? ""),
    chain_id: raw.chain_id != null ? Number(raw.chain_id) : null,
    strategy_kind:
      raw.strategy_kind != null ? (raw.strategy_kind as StrategyKind) : null,
    detected_at: raw.detected_at != null ? String(raw.detected_at) : null,
    trace_id: String(raw.trace_id ?? ""),

    // Extended Identity (FE-0028): cartridge_id is a real wire field
    // (opportunities.cartridge_id — REST live query SELECTs it; WS payloads
    // may omit it → null). hop_count derives from the parsed topology below.
    // The level-(b) fields have NO wire today → pinned null, never fabricated.
    cartridge_id: raw.cartridge_id != null ? String(raw.cartridge_id) : null,
    hop_count: deriveHopCount(routeMetadata),
    candidate_id: null,
    route_id: null,
    pair_id: null,
    detector_id: null,
    quote_token: null,
    quote_version: null,
    graph_version: null,
    config_version: null,
    strategy_version: null,
    gate_results: null,
    data_quality: null,

    // Route Information
    dex_a: String(raw.dex_a ?? ""),
    dex_b: raw.dex_b != null ? String(raw.dex_b) : null,
    pair_symbol: raw.pair_symbol != null ? String(raw.pair_symbol) : null,
    token_in: String(raw.token_in ?? ""),
    token_out: String(raw.token_out ?? ""),
    amount_in_wei: raw.amount_in_wei != null ? String(raw.amount_in_wei) : null,

    // Token Metadata
    token_in_info: (raw.token_in_info as TokenInfo) ?? null,
    token_out_info: (raw.token_out_info as TokenInfo) ?? null,
    chain_base_token_symbol:
      raw.chain_base_token_symbol != null ? String(raw.chain_base_token_symbol) : null,
    // HOPS-SYM-02 (RC2): the server-hydrated intermediate-leg symbols were
    // silently discarded here — the wire field exists (api-server hydrates it
    // from the tokens table + on-demand eth_call; the interface has declared
    // it since F2) but it never reached any component. Pass through verbatim:
    // absent/null stays null and renderers keep their honest shortAddr
    // fallback (R8).
    leg_symbols: (raw.leg_symbols as Record<string, string> | null) ?? null,

    // Profit Metrics
    expected_profit_usd:
      raw.expected_profit_usd != null ? Number(raw.expected_profit_usd) : null,
    net_expected_profit_usd:
      raw.net_expected_profit_usd != null ? Number(raw.net_expected_profit_usd) : null,
    roi_pct: raw.roi_pct != null ? Number(raw.roi_pct) : null,
    risk_score: raw.risk_score != null ? Number(raw.risk_score) : null,

    // Status — §28: absent status is null, never a fabricated "detected".
    status: (raw.status as OpportunityStatus | undefined) ?? null,
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
    route_metadata: routeMetadata,

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

    // §30 verdict — annotated below on the complete object.
    semantic_violations: [],
  };
  // FE-0031 (§30): compute AFTER the row is fully mapped — the validator
  // audits the finished ViewModel, so every path (WS/polling/SSR) quarantines
  // identically.
  mapped.semantic_violations = validateOpportunitySemantics(mapped);
  return mapped;
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
  // decimals — Rust serializes DecimalsMap as a newtype: {"map": {...}} (the
  // REAL wire shape; see shared-rs candidates.rs). Accept it AND a flat record
  // (legacy/tests); non-number values are dropped, never coerced.
  let decimals: Record<string, number> | undefined;
  if (obj.decimals != null && typeof obj.decimals === "object") {
    const raw = obj.decimals as Record<string, unknown>;
    const src =
      raw.map != null && typeof raw.map === "object"
        ? (raw.map as Record<string, unknown>)
        : raw;
    const out: Record<string, number> = {};
    for (const [k, v] of Object.entries(src)) {
      if (typeof v === "number") out[k] = v;
    }
    decimals = out;
  }
  // HOPS-LEDGER-04: project the optional per-leg ledger arrays. Undefined
  // (NOT empty) when absent — absence is the R8 state "not computed".
  const legAmountsIn = Array.isArray(obj.leg_amounts_in)
    ? (obj.leg_amounts_in as unknown[]).filter(
        (s): s is string => typeof s === "string",
      )
    : undefined;
  const legAmountsOut = Array.isArray(obj.leg_amounts_out)
    ? (obj.leg_amounts_out as unknown[]).filter(
        (s): s is string => typeof s === "string",
      )
    : undefined;
  const legZeroForOne = Array.isArray(obj.leg_zero_for_one)
    ? (obj.leg_zero_for_one as unknown[]).filter(
        (b): b is boolean => typeof b === "boolean",
    )
    : undefined;
  return {
    token_addresses: tokenAddresses,
    dex_adapters: dexAdapters,
    pool_addresses: poolAddresses,
    decimals,
    leg_amounts_in: legAmountsIn,
    leg_amounts_out: legAmountsOut,
    leg_zero_for_one: legZeroForOne,
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
 * FE-0030 (§29): the fallback is LEGACY DISPLAY ONLY — its legs carry
 * `synthetic: true` and renderers must show SYNTHETIC_LEGACY_VIEW_LABEL. It
 * is NEVER a ROUTE VERIFIED claim and NEVER an operational HOPS=2: the wire's
 * hop_count (deriveHopCount) stays null without persisted topology.
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
  // §29: marked `synthetic` — legacy display only, never ROUTE VERIFIED nor
  // operational HOPS=2.
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
      synthetic: true,
    },
    {
      index: 1,
      token_in: opp.token_out,
      token_out: opp.token_in,
      dex: dexB || dexA, // single-DEX 2-pool cycle
      pool: "",
      synthetic: true,
    },
  ];
}

// =============================================================================
// HOPS-LEDGER-04 — per-leg wei ledger (sized rows only)
// =============================================================================

/**
 * One leg of the per-hop ledger: what enters, what leaves, which direction —
 * all in EXACT wei strings (BigInt domain, never f64).
 *
 * `cycle_delta_wei` is present ONLY on the closing leg of a closed cycle
 * (final out − initial in, both in the opening token's wei): the hop-by-hop
 * running gain/loss the operator asked for. Null elsewhere — a delta between
 * different tokens' wei is meaningless (R8), and a partial ledger is worse
 * than none.
 */
export interface LegLedgerEntry {
  /** Hop index, 0-based — aligns with deriveLegs/RouteLeg.index. */
  index: number;
  /** Exact wei entering this leg (of token_addresses[index]). */
  amount_in_wei: string;
  /** Exact wei leaving this leg (of token_addresses[index+1]). */
  amount_out_wei: string;
  /** Uniswap token0→token1 swap direction (deployment fact, not pool state). */
  zero_for_one: boolean;
  /** Closed-cycle delta in opening-token wei — closing leg only, else null. */
  cycle_delta_wei: string | null;
}

/**
 * Derives the per-leg ledger from the persisted topology. Returns null when
 * there is no honest ledger: no route_metadata, any ledger array absent
 * (not-Sized rows, triangular kernel — R8 "not computed"), or lengths that
 * don't align with the persisted hops (all-or-nothing, mirroring the Rust
 * `attach_leg_ledger` gate).
 *
 * NEVER called on §29 synthetic legs — the fallback legs fabricate no amounts
 * by construction, and this function only reads `route_metadata`.
 */
export function deriveLegLedger(opp: OmniOpportunity): LegLedgerEntry[] | null {
  const rm = opp.route_metadata;
  if (!rm) return null;
  const hops = rm.dex_adapters.length;
  if (hops === 0) return null;
  const { leg_amounts_in: amountsIn, leg_amounts_out: amountsOut, leg_zero_for_one: zeroForOne } = rm;
  if (
    !amountsIn ||
    !amountsOut ||
    !zeroForOne ||
    amountsIn.length !== hops ||
    amountsOut.length !== hops ||
    zeroForOne.length !== hops
  ) {
    return null;
  }
  const entries: LegLedgerEntry[] = amountsIn.map((amount_in_wei, i) => ({
    index: i,
    amount_in_wei,
    amount_out_wei: amountsOut[i] ?? "",
    zero_for_one: zeroForOne[i] ?? false,
    cycle_delta_wei: null,
  }));
  // Closing-leg delta: only when the topology closes the cycle back to the
  // opening token. BigInt-exact; a non-numeric payload leaves it null (R8 —
  // never fabricate a figure from garbage).
  const tokens = rm.token_addresses;
  if (tokens.length === hops + 1 && tokens[0] && tokens[0] === tokens[hops]) {
    const initialIn = entries[0]?.amount_in_wei;
    const closing = entries[hops - 1];
    const finalOut = closing?.amount_out_wei;
    if (initialIn && finalOut && closing) {
      try {
        closing.cycle_delta_wei = (
          BigInt(finalOut) - BigInt(initialIn)
        ).toString();
      } catch {
        // leave null — not computed
      }
    }
  }
  return entries;
}

// =============================================================================
// FE-0031 — Semantic validation (§30): QUARANTINED, never hidden
// =============================================================================

/** §30 violation vocabulary (closed — renderers list codes verbatim). */
export type SemanticViolation =
  /** No route identity at all: no route_metadata, no synthetic basis (§29). */
  | "no_route_identity"
  /** strategy_kind absent (malformed payload — FE-0029 maps it to null). */
  | "missing_strategy_id"
  /** Degenerate pair: token_in === token_out (the AGLD⇄AGLD case). */
  | "degenerate_pair"
  /** Topology hop arithmetic broken: tokens ≠ hops + 1. */
  | "hop_incoherent"
  /** Leg chain broken: doesn't link leg-to-leg / doesn't close the cycle. */
  | "legs_incoherent"
  /** No block context — a row the §30 contract expects block_number on. */
  | "missing_block"
  /** A profit field is present but not a finite number (NaN/Infinity). */
  | "profit_not_numeric";

/** §30 canonical quarantine marker — one source of truth for renderers. */
export const QUARANTINED_LABEL = "QUARANTINED";

/**
 * Pure §30 semantic gate. Returns the violation list — EMPTY means validated
 * clean. A non-empty list marks the row QUARANTINED for display (marked, not
 * hidden).
 *
 * Fail-honest boundaries (RULE 00 / R8):
 *   - Honest NULLS are NOT violations per se: null profit = not computed;
 *     null route_metadata on a row that still carries dex_a/dex_b has a
 *     §29 synthetic display basis. What quarantines is INCOHERENCE — a row
 *     presenting itself as an opportunity while its semantics don't hold.
 *   - Cross-chain rows (chain_id_out != null) do not close a same-chain
 *     cycle — the closure check is single-chain only.
 */
export function validateOpportunitySemantics(opp: OmniOpportunity): SemanticViolation[] {
  const v: SemanticViolation[] = [];

  // route_id/strategy_id identity.
  const legs = deriveLegs(opp);
  if (opp.strategy_kind == null) v.push("missing_strategy_id");
  if (opp.route_metadata == null && legs.length === 0) v.push("no_route_identity");

  // Degenerate pair: a PERSISTED route leg that swaps a token for itself.
  // Row-level token_in === token_out is NOT a violation — the row pair is
  // first-leg-in / last-leg-out (searcher-rs cartridge_boot: "R8: token_in/out
  // from intent legs"), so in === out is the DEFINITION of a closed cycle.
  // Only a self-swap LEG (X→X inside the route) is a provable no-op. Synthetic
  // §29 legs are display shape — not audited here.
  const wireLegs = legs.filter((l) => !l.synthetic);
  if (wireLegs.some((l) => l.token_in !== "" && l.token_in === l.token_out)) {
    v.push("degenerate_pair");
  }

  // hop coherence + leg linkage over the PERSISTED topology (wire-grade only;
  // synthetic §29 legs are display shape, not an operational route to audit).
  const rm = opp.route_metadata;
  if (rm != null) {
    if (rm.token_addresses.length !== rm.dex_adapters.length + 1) v.push("hop_incoherent");
    // Chain must link: token_addresses[i+1] is leg i's out — trivially true by
    // construction, so the REAL linkage check is the cycle close on
    // single-chain rows: last address === first address.
    const singleChain = opp.chain_id_out == null;
    if (
      singleChain &&
      rm.token_addresses.length >= 2 &&
      rm.token_addresses[rm.token_addresses.length - 1] !== rm.token_addresses[0]
    ) {
      v.push("legs_incoherent");
    }
  }

  // Block context (the wire SELECTs block_number — absent means the row never
  // anchored itself to a block).
  if (opp.block_number == null) v.push("missing_block");

  // Profit numerics: PRESENT values must be finite. null = not computed (R8),
  // never a violation.
  const profits = [
    opp.expected_profit_usd,
    opp.net_expected_profit_usd,
    opp.roi_pct,
    opp.risk_score,
    opp.simulated_net_profit_usd,
    opp.simulated_amount_in_usd,
    opp.simulated_roi_pct,
  ];
  if (profits.some((p) => p != null && !Number.isFinite(p))) v.push("profit_not_numeric");

  return v;
}

