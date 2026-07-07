/**
 * Ω-S5++ C9.∞ APEX · Chain entity schemas
 * INV-7 Ghost Invariant: executor_balance_wei ≡ "0" siempre
 * INV-B No-Live:        mainnet_enabled = false hasta Crucible OK
 */
import { z } from 'zod';
import {
  ChainIdSchema,
  EvmAddressSchema,
  GhostBalanceSchema,
  Bytes32Schema,
  IsoTimestampSchema,
  SequenceIdSchema,
  ReadinessStateSchema,
  IdempotencyKeySchema,
  OperatorIdSchema,
  BlockedResponseSchema,
  UnavailableResponseSchema,
  ErrorResponseSchema,
} from './_primitives';

// ── Entidad canónica ────────────────────────────────────────────────────
export const ChainEntitySchema = z.object({
  chain_id: ChainIdSchema,
  name: z.string().min(1),
  enabled: z.boolean(),
  paper_mode: z.boolean(),
  mainnet_enabled: z.literal(false), // INV-B
  rpc_http_count: z.number().int().min(0),
  rpc_ws_count: z.number().int().min(0),
  executor_signer: EvmAddressSchema,
  executor_balance_wei: GhostBalanceSchema, // INV-7
  readiness: ReadinessStateSchema,
  blockers: z.array(z.string()),
  config_hash: Bytes32Schema,
  updated_at: IsoTimestampSchema,
  sequence_id: SequenceIdSchema,
}).strict();
export type ChainEntity = z.infer<typeof ChainEntitySchema>;

export const ChainListResponseSchema = z.object({
  status: z.literal('OK'),
  chains: z.array(ChainEntitySchema),
  sequence_id: SequenceIdSchema,
  evaluated_at: IsoTimestampSchema,
}).strict();

// ── Request schemas (CRUD) ──────────────────────────────────────────────
export const ChainCreateRequestSchema = z.object({
  chain_id: ChainIdSchema,
  name: z.string().min(1).max(64),
  rpc_http_url: z.string().url(),
  rpc_ws_url: z.string().url().nullable(),
  native_symbol: z.string().min(1).max(8),
  block_time_ms: z.number().int().positive().max(60_000),
  notes: z.string().max(512).nullable(),
  idempotency_key: IdempotencyKeySchema,
  actor_operator_id: OperatorIdSchema,
}).strict();
export type ChainCreateRequest = z.infer<typeof ChainCreateRequestSchema>;

export const ChainEditRequestSchema = z.object({
  chain_id: ChainIdSchema,
  rpc_ws_url: z.string().url().nullable().optional(),
  block_time_ms: z.number().int().positive().max(60_000).optional(),
  notes: z.string().max(512).nullable().optional(),
  config_hash_before: Bytes32Schema,
  idempotency_key: IdempotencyKeySchema,
  actor_operator_id: OperatorIdSchema,
}).strict();
export type ChainEditRequest = z.infer<typeof ChainEditRequestSchema>;

export const ChainToggleRequestSchema = z.object({
  chain_id: ChainIdSchema,
  enabled: z.boolean(),
  config_hash_before: Bytes32Schema,
  idempotency_key: IdempotencyKeySchema,
  actor_operator_id: OperatorIdSchema,
}).strict();
export type ChainToggleRequest = z.infer<typeof ChainToggleRequestSchema>;

// ── Response schemas (unión discriminada) ───────────────────────────────
export const ChainMutationOkSchema = z.object({
  status: z.literal('OK'),
  chain: ChainEntitySchema,
  config_hash_before: Bytes32Schema,
  config_hash_after: Bytes32Schema,
  audit_event_id: z.string().uuid(),
  reload_channel: z.string(),
  ack_pending: z.literal(true),
  evaluated_at: IsoTimestampSchema,
}).strict();
export type ChainMutationOk = z.infer<typeof ChainMutationOkSchema>;

export const ChainMutationResponseSchema = z.discriminatedUnion('status', [
  ChainMutationOkSchema,
  BlockedResponseSchema,
  UnavailableResponseSchema,
  ErrorResponseSchema,
]);
export type ChainMutationResponse = z.infer<typeof ChainMutationResponseSchema>;
