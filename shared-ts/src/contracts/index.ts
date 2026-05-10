import { z } from "zod";

export const StrategyKind = z.enum([
  "dex_arb", "triangular", "backrun", "liquidation", "flashloan_arb",
]);
export type StrategyKind = z.infer<typeof StrategyKind>;

const HexAddr = z.string().regex(/^0x[0-9a-fA-F]{40}$/);
const HexTx   = z.string().regex(/^0x[0-9a-fA-F]{64}$/);
export const BigIntStr = z.string().regex(/^[0-9]+$/);
const Uuid = z.string().uuid();
const IsoDate = z.string().datetime({ offset: true });

export const OpportunitySchema = z.object({
  id: Uuid,
  chain_id: z.number().int().positive(),
  strategy_kind: StrategyKind,
  dex_a: z.string().min(1),
  dex_b: z.string().nullable(),
  pair_symbol: z.string().min(1),
  token_in: HexAddr,
  token_out: HexAddr,
  amount_in_wei: BigIntStr,
  // expected_profit_usd is GROSS spread × price (no gas/slippage subtracted).
  // The canonical NET-profit gate is `net_expected_profit_usd`, populated by
  // the prioritization-spine evaluator after subtracting all 7 cost components
  // (gas, slippage, relay_fee, p_fail × revert_cost, etc.). Always prefer
  // net_expected_profit_usd; fall back to expected_profit_usd only when the
  // spine path was not run (cold-start or oracle gap).
  expected_profit_usd: z.number().nullable(),
  net_expected_profit_usd: z.number().nullable().optional(),
  roi_pct: z.number().nullable(),
  risk_score: z.number().nullable(),
  block_number: z.number().int().nonnegative().nullable(),
  // Cross-chain bridging fields (added in BE-01 Sprint A migration 047).
  // Null for single-chain opportunities; populated for cex_dex / cross_chain.
  chain_id_out: z.number().int().positive().nullable().optional(),
  bridge: z.string().nullable().optional(),
  bridge_fee_usd: z.number().nullable().optional(),
  detected_at: IsoDate,
  trace_id: Uuid,
}).strict();
export type Opportunity = z.infer<typeof OpportunitySchema>;

export const SimulationResultSchema = z.object({
  opportunity_id: Uuid,
  passed: z.boolean(),
  gas_estimate_wei: BigIntStr.nullable(),
  gas_price_wei: BigIntStr.nullable(),
  slippage_pct: z.number().nullable(),
  revert_risk_pct: z.number().nullable(),
  simulated_profit_usd: z.number().nullable(),
  // "revm" added 2026-05-10 audit re-run #2: simulator-v2 backend ships SimResult
  // with this label. Without this enum entry, every revm SimulationResult fails
  // Zod parse at the selector-api boundary (cs-validator MAJOR #3).
  simulator: z.enum(["anvil","tenderly","hardhat","revm","not_implemented"]),
  fail_reason: z.string().nullable(),
  simulated_at: IsoDate,
  trace_id: Uuid,
}).strict();
export type SimulationResult = z.infer<typeof SimulationResultSchema>;

export const ExecutionRequestSchema = z.object({
  opportunity_id: Uuid,
  simulation_id: Uuid,
  relay_preference: z.array(z.string().min(1)).min(1),
  max_gas_price_wei: BigIntStr,
  deadline_block: z.number().int().positive(),
  trace_id: Uuid,
}).strict();
export type ExecutionRequest = z.infer<typeof ExecutionRequestSchema>;

export const ExecutionResultSchema = z.object({
  opportunity_id: Uuid,
  status: z.enum(["submitted","included","reverted","dropped","replaced","not_implemented","not_submitted"]),
  tx_hash: HexTx.nullable(),
  relay_used: z.string().nullable(),
  block_included: z.number().int().nonnegative().nullable(),
  gas_used_wei: BigIntStr.nullable(),
  actual_profit_usd: z.number().nullable(),
  error_message: z.string().nullable(),
  submitted_at: IsoDate,
  trace_id: Uuid,
}).strict();
export type ExecutionResult = z.infer<typeof ExecutionResultSchema>;

// ReconReportSchema: post-execution record. `expected_profit_usd` is the
// pre-trade estimate recorded at scoring time — always known, never NULL.
// (Contrast with OpportunitySchema where it is nullable until simulated.)
export const ReconReportSchema = z.object({
  opportunity_id: Uuid,
  expected_profit_usd: z.number(),
  actual_profit_usd: z.number(),
  variance_usd: z.number(),
  variance_pct: z.number(),
  actual_gas_used_wei: BigIntStr.nullable(),
  notes: z.string().nullable(),
  created_at: IsoDate,
  trace_id: Uuid,
}).strict();
export type ReconReport = z.infer<typeof ReconReportSchema>;

export const KillSwitchStateSchema = z.object({
  enabled: z.boolean(),
  reason: z.string().nullable(),
  triggered_by: z.string().nullable(),
  updated_at: IsoDate,
}).strict();
export type KillSwitchState = z.infer<typeof KillSwitchStateSchema>;

/** Canonical 501 payload for unimplemented paths. */
export type NotImplementedPayload = {
  error: "not_implemented";
  requires: string[];
  sprint: string;
  detail: string;
};
export function notImplemented(requires: string[], sprint: string, detail: string): NotImplementedPayload {
  return { error: "not_implemented", requires, sprint, detail };
}
