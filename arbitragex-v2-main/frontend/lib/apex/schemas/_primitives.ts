/**
 * Ω-S5++ C9.∞ APEX · Schema Primitives
 * -------------------------------------------------------------------------
 * Tipos primitivos canónicos. Toda entidad debe componerse desde aquí.
 *
 * INV-2 Schema Hermiticity: estos primitivos son los únicos átomos
 * permitidos para construir DTOs. Si una entidad usa string libre donde
 * existe un primitivo aquí, la hermiticidad falla.
 */
import { z } from 'zod';

// ── Cadenas soportadas (7) ──────────────────────────────────────────────
export const ChainIdSchema = z.union([
  z.literal(1), // ethereum
  z.literal(56), // bsc
  z.literal(137), // polygon
  z.literal(42161), // arbitrum
  z.literal(10), // optimism
  z.literal(8453), // base
  z.literal(43114), // avalanche
]);
export type ChainId = z.infer<typeof ChainIdSchema>;

export const CHAIN_NAMES: Readonly<Record<ChainId, string>> = {
  1: 'ethereum',
  56: 'bsc',
  137: 'polygon',
  42161: 'arbitrum',
  10: 'optimism',
  8453: 'base',
  43114: 'avalanche',
} as const;

// ── Direcciones EVM ─────────────────────────────────────────────────────
export const EvmAddressSchema = z
  .string()
  .regex(/^0x[a-fA-F0-9]{40}$/, 'INV-2: dirección EVM inválida (no 0x + 40 hex)');
export type EvmAddress = z.infer<typeof EvmAddressSchema>;

// ── Bytes32 (tx hash, config hash) ──────────────────────────────────────
export const Bytes32Schema = z
  .string()
  .regex(/^0x[a-f0-9]{64}$/, 'INV-2: bytes32 inválido');
export type Bytes32 = z.infer<typeof Bytes32Schema>;

// ── Dinero: SIEMPRE string decimal serializado, NUNCA number ────────────
// Γ.MoneySafety: float en dinero rompe la conservación de carga.
export const WeiAmountSchema = z
  .string()
  .regex(/^\d+$/, 'INV-2: wei debe ser entero positivo decimal-string');
export type WeiAmount = z.infer<typeof WeiAmountSchema>;

export const UsdDecimalSchema = z
  .string()
  .regex(/^\d+(\.\d{1,6})?$/, 'INV-2: USD con hasta 6 decimales como string');
export type UsdDecimal = z.infer<typeof UsdDecimalSchema>;

// Ghost Protocol: balance debe ser exactamente "0"
export const GhostBalanceSchema = z.literal('0');
export type GhostBalance = z.infer<typeof GhostBalanceSchema>;

// ── Basis points / percent con unidad ───────────────────────────────────
export const BpsSchema = z.number().int().min(0).max(10_000);
export type Bps = z.infer<typeof BpsSchema>;

export const PercentUnitSchema = z.object({
  value: z.number().finite(),
  unit: z.literal('percent'),
}).strict();
export type PercentUnit = z.infer<typeof PercentUnitSchema>;

export const BpsUnitSchema = z.object({
  value: BpsSchema,
  unit: z.literal('bps'),
}).strict();
export type BpsUnit = z.infer<typeof BpsUnitSchema>;

// ── Timestamps SIEMPRE ISO-8601 con offset ──────────────────────────────
// Γ.TimeTimezone: timestamps sin TZ rompen causalidad Lorentz-invariante.
export const IsoTimestampSchema = z
  .string()
  .datetime({ offset: true, message: 'INV-2: timestamp debe ser ISO-8601 con offset' });
export type IsoTimestamp = z.infer<typeof IsoTimestampSchema>;

// ── sequence_id monotónico ──────────────────────────────────────────────
export const SequenceIdSchema = z.number().int().positive();
export type SequenceId = z.infer<typeof SequenceIdSchema>;

// ── Idempotency key (UUID v4) ───────────────────────────────────────────
export const IdempotencyKeySchema = z
  .string()
  .uuid('INV-4: idempotency_key debe ser UUID');
export type IdempotencyKey = z.infer<typeof IdempotencyKeySchema>;

// ── Operator IDs ────────────────────────────────────────────────────────
export const OperatorIdSchema = z.string().uuid();
export type OperatorId = z.infer<typeof OperatorIdSchema>;

export const OperatorRoleSchema = z.enum(['observer', 'steward', 'sovereign']);
export type OperatorRole = z.infer<typeof OperatorRoleSchema>;

// ── Estados de readiness universales ────────────────────────────────────
export const ReadinessStateSchema = z.enum([
  'PASS',
  'PARTIAL',
  'BLOCKED',
  'UNAVAILABLE',
  'NO_DATA',
  'PENDING_RUNTIME_ACK',
]);
export type ReadinessState = z.infer<typeof ReadinessStateSchema>;

// ── Respuestas universales de fallo honesto ─────────────────────────────
export const BlockedResponseSchema = z.object({
  status: z.literal('BLOCKED'),
  reason: z.string().min(1),
  next_action: z.string().min(1),
  unblock_requirements: z.array(z.string()),
  blocker_codes: z.array(z.string()),
  evaluated_at: IsoTimestampSchema,
}).strict();
export type BlockedResponse = z.infer<typeof BlockedResponseSchema>;

export const UnavailableResponseSchema = z.object({
  status: z.literal('UNAVAILABLE'),
  reason: z.string().min(1),
  retry_after_ms: z.number().int().min(0).nullable(),
  evaluated_at: IsoTimestampSchema,
}).strict();
export type UnavailableResponse = z.infer<typeof UnavailableResponseSchema>;

export const ErrorResponseSchema = z.object({
  status: z.literal('ERROR'),
  code: z.string().min(1),
  message: z.string().min(1),
  details: z.record(z.string(), z.unknown()).optional(),
  evaluated_at: IsoTimestampSchema,
}).strict();
export type ErrorResponse = z.infer<typeof ErrorResponseSchema>;

// ── Runtime acknowledgement universal ───────────────────────────────────
export const RuntimeAckSchema = z.object({
  ack: z.literal(true),
  worker_id: z.string().min(1),
  applied_at: IsoTimestampSchema,
  config_hash_after: Bytes32Schema,
  sequence_id: SequenceIdSchema,
}).strict();
export type RuntimeAck = z.infer<typeof RuntimeAckSchema>;

export const RuntimeAckFailureSchema = z.object({
  ack: z.literal(false),
  reason: z.string().min(1),
  worker_id: z.string().nullable(),
  evaluated_at: IsoTimestampSchema,
}).strict();
export type RuntimeAckFailure = z.infer<typeof RuntimeAckFailureSchema>;

// ── Audit event universal ───────────────────────────────────────────────
export const AuditEventSchema = z.object({
  audit_event_id: z.string().uuid(),
  actor_operator_id: OperatorIdSchema,
  actor_role: OperatorRoleSchema,
  action: z.string().min(1),
  entity_type: z.string().min(1),
  entity_id: z.string().min(1),
  chain_id: ChainIdSchema.nullable(),
  before: z.record(z.string(), z.unknown()).nullable(),
  after: z.record(z.string(), z.unknown()).nullable(),
  config_hash_before: Bytes32Schema.nullable(),
  config_hash_after: Bytes32Schema.nullable(),
  idempotency_key: IdempotencyKeySchema,
  rollback_available: z.boolean(),
  created_at: IsoTimestampSchema,
}).strict();
export type AuditEvent = z.infer<typeof AuditEventSchema>;

// ── Reload event (Redis pub/sub) ────────────────────────────────────────
export const ReloadEventSchema = z.object({
  channel: z.string().regex(/^arbx:config:[a-z_]+:[a-zA-Z0-9_-]+:reload$/),
  chain_id: ChainIdSchema.nullable(),
  entity_type: z.string(),
  entity_id: z.string(),
  config_hash_after: Bytes32Schema,
  emitted_at: IsoTimestampSchema,
  sequence_id: SequenceIdSchema,
}).strict();
export type ReloadEvent = z.infer<typeof ReloadEventSchema>;
