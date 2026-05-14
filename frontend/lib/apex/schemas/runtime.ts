/**
 * Ω-S5++ C9.∞ APEX · Runtime/realtime schemas
 * Readiness decision, opportunities, simulation outcomes, agent teams.
 */
import { z } from 'zod';
import {
  ChainIdSchema,
  EvmAddressSchema,
  Bytes32Schema,
  IsoTimestampSchema,
  SequenceIdSchema,
  ReadinessStateSchema,
  UsdDecimalSchema,
  WeiAmountSchema,
  BpsSchema,
  BlockedResponseSchema,
  UnavailableResponseSchema,
} from './_primitives';

// ─── Readiness decision (GO/NO-GO) ──────────────────────────────────────
export const ReadinessDecisionSchema = z.object({
  status: z.literal('OK'),
  global_state: ReadinessStateSchema,
  go_no_go: z.enum(['GO', 'NO_GO']),
  by_chain: z.record(
    z.string().regex(/^\d+$/), // chain_id como string
    z.object({
      state: ReadinessStateSchema,
      blockers: z.array(z.string()),
    }).strict(),
  ),
  global_blockers: z.array(z.string()),
  crucible_hours: z.number().min(0),
  crucible_success_pct: z.number().min(0).max(100),
  crucible_non_doctrinal_reverts: z.number().int().min(0),
  evaluated_at: IsoTimestampSchema,
  sequence_id: SequenceIdSchema,
}).strict();
export type ReadinessDecision = z.infer<typeof ReadinessDecisionSchema>;

export const ReadinessResponseSchema = z.discriminatedUnion('status', [
  ReadinessDecisionSchema,
  BlockedResponseSchema,
  UnavailableResponseSchema,
]);
export type ReadinessResponse = z.infer<typeof ReadinessResponseSchema>;

// ─── Opportunity (live stream item) ─────────────────────────────────────
export const OpportunityKindSchema = z.enum([
  'DUAL_HOP',
  'TRIPLE_HOP',
  'CROSS_DOMAIN',
]);

export const OpportunityItemSchema = z.object({
  opportunity_id: z.string().uuid(),
  chain_id: ChainIdSchema,
  kind: OpportunityKindSchema,
  path_tokens: z.array(EvmAddressSchema).min(2),
  path_pools: z.array(EvmAddressSchema).min(1),
  amount_in_wei: WeiAmountSchema,
  expected_out_wei: WeiAmountSchema,
  expected_profit_wei: WeiAmountSchema,
  expected_profit_usd: UsdDecimalSchema,
  confidence_score: z.number().min(0).max(1),
  decoherence_cost_wei: WeiAmountSchema, // VDC
  net_profit_wei: WeiAmountSchema,
  net_profit_passes_gate: z.boolean(),
  evidence_complete: z.boolean(),
  state: z.enum([
    'DETECTED',
    'SIMULATED',
    'EVIDENCE_GATED',
    'NET_PROFIT_GATED',
    'PAPER_EXECUTED',
    'BLOCKED',
    'EXPIRED',
  ]),
  detected_at: IsoTimestampSchema,
  sequence_id: SequenceIdSchema,
}).strict();
export type OpportunityItem = z.infer<typeof OpportunityItemSchema>;

// ─── Simulation outcome ─────────────────────────────────────────────────
export const SimulationOutcomeSchema = z.object({
  simulation_id: z.string().uuid(),
  opportunity_id: z.string().uuid(),
  chain_id: ChainIdSchema,
  success: z.boolean(),
  revert_reason: z.string().nullable(),
  is_doctrinal_revert: z.boolean().nullable(), // null si success=true
  gas_used: z.number().int().min(0),
  gas_price_wei: WeiAmountSchema,
  net_profit_wei: WeiAmountSchema,
  simulated_at: IsoTimestampSchema,
  sequence_id: SequenceIdSchema,
}).strict();
export type SimulationOutcome = z.infer<typeof SimulationOutcomeSchema>;

// ─── Agent team status ──────────────────────────────────────────────────
export const AgentTeamSchema = z.object({
  team_id: z.string().uuid(),
  name: z.string().min(1),
  members_active: z.number().int().min(0),
  members_total: z.number().int().min(0),
  evidence_complete: z.boolean(),
  last_heartbeat_at: IsoTimestampSchema,
  readiness: ReadinessStateSchema,
  sequence_id: SequenceIdSchema,
}).strict();
export type AgentTeam = z.infer<typeof AgentTeamSchema>;

// ─── Bayesian Allocator signal (espejo de feedback.rs) ──────────────────
export const AllocatorSignalSchema = z.object({
  chain_id: ChainIdSchema,
  strategy_id: z.string().uuid(),
  posterior_alpha: z.number().positive(),
  posterior_beta: z.number().positive(),
  yield_ratio: z.number(),
  variance: z.number().min(0),
  half_kelly_fraction: z.number().min(0).max(1),
  assigned_cap_usd: UsdDecimalSchema, // empieza en "0.00"
  emitted_at: IsoTimestampSchema,
  ttl_seconds: z.literal(300), // mirror feedback.rs TTL
  sequence_id: SequenceIdSchema,
}).strict();
export type AllocatorSignal = z.infer<typeof AllocatorSignalSchema>;

// ─── Drift detection ────────────────────────────────────────────────────
export const DriftEventSchema = z.object({
  drift_id: z.string().uuid(),
  chain_id: ChainIdSchema,
  entity_type: z.string(),
  entity_id: z.string(),
  config_hash_db: Bytes32Schema,
  config_hash_runtime: Bytes32Schema,
  detected_at: IsoTimestampSchema,
  sequence_id: SequenceIdSchema,
}).strict();
export type DriftEvent = z.infer<typeof DriftEventSchema>;
