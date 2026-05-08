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

export const OpportunityListItemSchema = z.object({
  id: z.string().uuid(),
  chain_id: z.number().int().positive(),
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
});
export type OpportunityListItem = z.infer<typeof OpportunityListItemSchema>;
