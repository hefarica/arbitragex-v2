import { z } from "zod";

export const TokenInfoSchema = z.object({
  symbol: z.string().nullable(),
  decimals: z.number().int().min(0).max(255).nullable(),
  logo_url: z.string().url().nullable(),
  resolved_via: z.enum(["onchain_full", "onchain_partial", "trustwallet_only", "failed"]),
});
export type TokenInfo = z.infer<typeof TokenInfoSchema>;

export const StrategyKindSchema = z.enum([
  "dex_arb", "triangular", "backrun", "liquidation", "flashloan_arb",
]);

export const StatusSchema = z.enum([
  "detected", "validated", "simulated", "scored", "executing",
  "executed", "reconciled", "rejected", "failed",
]);

export const OpportunityListItemSchema = z.object({
  id: z.string().uuid(),
  chain_id: z.number().int().positive(),
  strategy_kind: StrategyKindSchema,
  dex_a: z.string(),
  dex_b: z.string().nullable(),
  pair_symbol: z.string(),
  token_in: z.string().regex(/^0x[a-fA-F0-9]{40}$/),
  token_in_info: TokenInfoSchema.nullable(),
  token_out: z.string().regex(/^0x[a-fA-F0-9]{40}$/),
  token_out_info: TokenInfoSchema.nullable(),
  amount_in_wei: z.string(),
  expected_profit_usd: z.number().nullable(),
  roi_pct: z.number().nullable(),
  risk_score: z.number().nullable(),
  block_number: z.number().int().nullable(),
  status: StatusSchema,
  detected_at: z.string().datetime({ offset: true }),
  trace_id: z.string().uuid(),
  chain_id_out: z.number().int().positive().nullable(),
  bridge: z.string().nullable(),
  bridge_fee_usd: z.number().nullable(),
});
export type OpportunityListItem = z.infer<typeof OpportunityListItemSchema>;
