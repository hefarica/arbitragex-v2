import { z } from "zod";
import { StrategyKind, BigIntStr } from "./contracts/index.js";

// ─────── Phase 2 Route-Finder: DEX and Pool catalog schemas ───────────────────
//
// The enum is intentionally broader than the DB CHECK constraint
// (which only allows UNISWAP_V2, UNISWAP_V3, CURVE, BALANCER).
// Phase 3 will add SOLIDLY and PANCAKE decoders; accepting them here
// avoids a schema migration when those rows appear. No rows with those
// values exist yet so the endpoint will never return them in practice.

export const DexInfoSchema = z.object({
  id:             z.string().uuid(),
  name:           z.string(),
  protocol_type:  z.enum(["UNISWAP_V2", "UNISWAP_V3", "CURVE", "BALANCER", "SOLIDLY", "PANCAKE"]),
  is_active:      z.boolean(),
  chain_id:       z.number().int().positive(),
  factory_count:  z.number().int().nonnegative(),
  pool_count:     z.number().int().nonnegative(),
  volume_24h_usd: z.number().nullable(),
  tvl_usd:        z.number().nullable(),
  // Per-chain provenance from dex_chain_metrics (migration 045). Optional for
  // backward-compat with consumers built before the COALESCE landed; null when
  // the per-chain row is missing and the response falls back to global aggregates.
  metrics_source: z.enum(["defillama", "subgraph", "manual", "estimated"]).nullable().optional(),
  metrics_updated_at: z.string().datetime({ offset: true }).nullable().optional(),
  // computed: dex.id is in trading_config.enabled_dex_ids OR enabled_dex_ids IS NULL
  enabled:        z.boolean(),
});
export type DexInfo = z.infer<typeof DexInfoSchema>;

export const DexesResponseSchema = z.object({
  count:    z.number().int().nonnegative(),
  chain_id: z.number().int().positive(),
  items:    z.array(DexInfoSchema),
});
export type DexesResponse = z.infer<typeof DexesResponseSchema>;

export const PoolInfoSchema = z.object({
  id:              z.string().uuid(),
  chain_id:        z.number().int().positive(),
  address:         z.string().regex(/^0x[a-fA-F0-9]{40}$/),
  dex_id:          z.string().uuid(),
  dex_name:        z.string(),
  protocol_type:   z.string(),
  fee_tier:        z.number().int().nullable(),
  token0_symbol:   z.string().nullable(),
  token0_address:  z.string(),
  token1_symbol:   z.string().nullable(),
  token1_address:  z.string(),
  is_active:       z.boolean(),
});
export type PoolInfo = z.infer<typeof PoolInfoSchema>;

export const PoolsResponseSchema = z.object({
  count:    z.number().int().nonnegative(),
  chain_id: z.number().int().positive(),
  filters:  z.object({
    dex_id:        z.string().uuid().nullable(),
    protocol_type: z.string().nullable(),
  }),
  items: z.array(PoolInfoSchema),
});
export type PoolsResponse = z.infer<typeof PoolsResponseSchema>;

// Re-export StrategyKind as the canonical enum. No StrategyKindSchema alias is
// emitted — grep confirmed zero external consumers of that name.
export { StrategyKind };

export const TokenInfoSchema = z.object({
  symbol: z.string().nullable(),
  decimals: z.number().int().min(0).max(255).nullable(),
  logo_url: z.string().url().nullable(),
  resolved_via: z.enum(["onchain_full", "onchain_partial", "trustwallet_only", "failed"]),
});
export type TokenInfo = z.infer<typeof TokenInfoSchema>;

export const StatusSchema = z.enum([
  "detected", "validated", "simulated", "scored", "executing",
  "executed", "reconciled", "rejected", "failed",
]);

/**
 * Cost-breakdown block emitted by the api-server's target-driven simulation
 * pipeline (`backend/api-server/src/simulation/computeSimulatedNet.ts`).
 * Mirrors the 8 cost components from `roi_engine.rs:182-212` (TS port).
 * Every field is a USD value ≥ 0. When the row's canonical Rust spine net is
 * available, this block is null (no simulation needed).
 */
export const SimulatedCostBreakdownSchema = z.object({
  gas_usd: z.number().nonnegative(),
  lp_fees_usd: z.number().nonnegative(),
  slippage_usd: z.number().nonnegative(),
  failure_buffer_usd: z.number().nonnegative(),
  copied_buffer_usd: z.number().nonnegative(),
  capital_cost_usd: z.number().nonnegative(),
  ops_overhead_usd: z.number().nonnegative(),
  flashloan_fee_usd: z.number().nonnegative(),
  relay_fee_usd: z.number().nonnegative(),
});

/**
 * Target-driven inverse sizing result. Computed when the operator has set a
 * net-profit target on this strategy (priority: `/strategies` card override →
 * Simulación-tab target). The dashboard renders the suggested amount_in next
 * to the forward-simulated net.
 *
 * `target_source` says where the target came from so the tooltip can attribute
 * it honestly. `meets_target_at_cap` is true when the operator's effective
 * capital cap admits the target; false when the cap binds and we surface the
 * max achievable net at the cap.
 */
export const SimulatedTargetSchema = z.object({
  /**
   * Operator's USD floor (min_profit_usd). Null when only a ROI floor was
   * configured under the strategy card or Simulación tab.
   */
  target_net_usd: z.number().nullable(),
  /**
   * Operator's ROI floor in percent (min_roi_pct, e.g. 2 means 2%). Null when
   * only a USD floor was configured.
   */
  target_roi_pct: z.number().nullable(),
  target_source: z.enum(["strategy_config", "simulation_tab"]),
  /**
   * Which floor binds the suggested amount under AND semantics (the larger
   * required amount wins). "roi-unreachable" / "net-per-usd-nonpositive"
   * are honest infeasibility flags (R8 fail-honest).
   */
  binding_floor: z.enum([
    "usd-floor",
    "roi-floor",
    "roi-unreachable",
    "net-per-usd-nonpositive",
    "tie",
  ]),
  /**
   * How the simulator estimated the net-per-usd rate:
   *   "observed-gross" — extrapolated from the row's recorded gross profit.
   *   "roi-assumed"    — fallback when no gross was recorded; assumes the
   *                      route delivers exactly the operator's ROI floor
   *                      as its gross rate. Path B; clearly labeled in
   *                      the dashboard tooltip.
   */
  estimation_basis: z.enum(["observed-gross", "roi-assumed"]),
  required_amount_in_usd: z.number(),
  cap_amount_in_usd: z.number().nonnegative(),
  suggested_amount_in_usd: z.number().nonnegative(),
  suggested_net_usd: z.number(),
  suggested_roi_pct: z.number(),
  meets_target_at_cap: z.boolean(),
  notes: z.array(z.string()),
});

export const OpportunityListItemSchema = z.object({
  id: z.string().uuid(),
  chain_id: z.number().int().positive(),
  // Operator-managed base token for the row's chain (WETH on Ethereum, USDC
  // on Base, etc.). Discreet badge in the dashboard so the operator never
  // infers it from the chain id. Loaded from `arbx:trading_config:<chain>`.
  chain_base_token_symbol: z.string().nullable().optional(),
  strategy_kind: StrategyKind,
  dex_a: z.string().min(1),
  dex_b: z.string().min(1).nullable(),
  pair_symbol: z.string().min(1).nullable(),
  token_in: z.string().regex(/^0x[a-fA-F0-9]{40}$/),
  token_in_info: TokenInfoSchema.nullable(),
  token_out: z.string().regex(/^0x[a-fA-F0-9]{40}$/),
  token_out_info: TokenInfoSchema.nullable(),
  amount_in_wei: BigIntStr,
  expected_profit_usd: z.number().nullable(),
  roi_pct: z.number().nullable(),
  risk_score: z.number().nullable(),
  block_number: z.number().int().nonnegative().nullable(),
  rejection_reason: z.string().nullable(),
  status: StatusSchema,
  detected_at: z.string().datetime({ offset: true }),
  trace_id: z.string().uuid(),
  chain_id_out: z.number().int().positive().nullable(),
  bridge: z.string().nullable(),
  bridge_fee_usd: z.number().nullable(),
  // Target-driven simulation block — R8 fail-honest: every field nullable so
  // the wire shape stays backward compatible and rows with insufficient
  // inputs render "—" in the dashboard rather than a fabricated number.
  // Populated only when the canonical Rust spine net is null AND the row's
  // chain has a `trading_config` snapshot in Redis AND the row's token can
  // be priced.
  simulated_net_profit_usd: z.number().nullable().optional(),
  simulated_amount_in_usd: z.number().nullable().optional(),
  simulated_roi_pct: z.number().nullable().optional(),
  simulated_cost_breakdown: SimulatedCostBreakdownSchema.nullable().optional(),
  simulated_target: SimulatedTargetSchema.nullable().optional(),
  simulated_at: z.string().datetime({ offset: true }).nullable().optional(),
  simulated_notes: z.array(z.string()).nullable().optional(),
});
export type OpportunityListItem = z.infer<typeof OpportunityListItemSchema>;
export type SimulatedCostBreakdown = z.infer<typeof SimulatedCostBreakdownSchema>;
export type SimulatedTarget = z.infer<typeof SimulatedTargetSchema>;
