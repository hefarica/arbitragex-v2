import { z } from "zod";
import { StrategyKind, BigIntStr } from "./contracts/index.js";

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
  pair_symbol: z.string().min(1),
  token_in: z.string().regex(/^0x[a-fA-F0-9]{40}$/),
  token_in_info: TokenInfoSchema.nullable(),
  token_out: z.string().regex(/^0x[a-fA-F0-9]{40}$/),
  token_out_info: TokenInfoSchema.nullable(),
  amount_in_wei: BigIntStr,
  expected_profit_usd: z.number().nullable(),
  roi_pct: z.number().nullable(),
  risk_score: z.number().nullable(),
  block_number: z.number().int().nonnegative().nullable(),
  status: StatusSchema,
  detected_at: z.string().datetime({ offset: true }),
  trace_id: z.string().uuid(),
  chain_id_out: z.number().int().positive().nullable(),
  bridge: z.string().nullable(),
  bridge_fee_usd: z.number().nullable(),
});
export type OpportunityListItem = z.infer<typeof OpportunityListItemSchema>;
