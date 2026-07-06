/**
 * Ω-S5++ C9.∞ APEX · Realtime WebSocket/SSE schemas
 * Snapshot + Delta + Heartbeat + Ack + Error + Stale
 * INV-3 Causality: ningún delta sin snapshot previo, sequence_id monotónico
 */
import { z } from 'zod';
import {
  ChainIdSchema,
  IsoTimestampSchema,
  SequenceIdSchema,
} from './_primitives';

// ── Envelope: cada mensaje WS/SSE debe envolverse así ───────────────────
export const RealtimeEnvelopeBaseSchema = z.object({
  event_id: z.string().uuid(),
  chain_id: ChainIdSchema.nullable(),
  emitted_at: IsoTimestampSchema,
  sequence_id: SequenceIdSchema,
});

// ── Snapshot (estado inicial completo) ──────────────────────────────────
export const SnapshotEnvelopeSchema = RealtimeEnvelopeBaseSchema.extend({
  type: z.literal('SNAPSHOT'),
  channel: z.string().min(1),
  payload: z.unknown(), // parseado por schema específico del consumer
}).strict();
export type SnapshotEnvelope = z.infer<typeof SnapshotEnvelopeSchema>;

// ── Delta (cambio incremental) ──────────────────────────────────────────
export const DeltaEnvelopeSchema = RealtimeEnvelopeBaseSchema.extend({
  type: z.literal('DELTA'),
  channel: z.string().min(1),
  op: z.enum(['CREATE', 'UPDATE', 'DELETE']),
  entity_id: z.string().min(1),
  previous_sequence_id: SequenceIdSchema, // INV-3 causalidad
  payload: z.unknown(),
}).strict();
export type DeltaEnvelope = z.infer<typeof DeltaEnvelopeSchema>;

// ── Heartbeat ───────────────────────────────────────────────────────────
export const HeartbeatEnvelopeSchema = RealtimeEnvelopeBaseSchema.extend({
  type: z.literal('HEARTBEAT'),
  worker_id: z.string().min(1),
  uptime_seconds: z.number().int().min(0),
}).strict();
export type HeartbeatEnvelope = z.infer<typeof HeartbeatEnvelopeSchema>;

// ── Runtime ack (confirmación de hot-reload) ────────────────────────────
export const AckEnvelopeSchema = RealtimeEnvelopeBaseSchema.extend({
  type: z.literal('ACK'),
  reload_channel: z.string().min(1),
  config_hash_after: z.string().regex(/^0x[a-f0-9]{64}$/),
  worker_id: z.string().min(1),
  success: z.boolean(),
  failure_reason: z.string().nullable(),
}).strict();
export type AckEnvelope = z.infer<typeof AckEnvelopeSchema>;

// ── Error ───────────────────────────────────────────────────────────────
export const ErrorEnvelopeSchema = RealtimeEnvelopeBaseSchema.extend({
  type: z.literal('ERROR'),
  code: z.string().min(1),
  message: z.string().min(1),
}).strict();
export type ErrorEnvelope = z.infer<typeof ErrorEnvelopeSchema>;

// ── Stale (gap detectado en sequence_id) ────────────────────────────────
export const StaleEnvelopeSchema = RealtimeEnvelopeBaseSchema.extend({
  type: z.literal('STALE'),
  reason: z.enum(['SEQUENCE_GAP', 'HEARTBEAT_TIMEOUT', 'WORKER_DOWN']),
  last_known_sequence_id: SequenceIdSchema,
  expected_sequence_id: SequenceIdSchema,
}).strict();
export type StaleEnvelope = z.infer<typeof StaleEnvelopeSchema>;

// ── Unión discriminada ──────────────────────────────────────────────────
export const RealtimeEnvelopeSchema = z.discriminatedUnion('type', [
  SnapshotEnvelopeSchema,
  DeltaEnvelopeSchema,
  HeartbeatEnvelopeSchema,
  AckEnvelopeSchema,
  ErrorEnvelopeSchema,
  StaleEnvelopeSchema,
]);
export type RealtimeEnvelope = z.infer<typeof RealtimeEnvelopeSchema>;

// ── Stale detection thresholds ──────────────────────────────────────────
export const STALE_HEARTBEAT_MS = 15_000 as const;
export const RECONNECT_BASE_MS = 500 as const;
export const RECONNECT_MAX_MS = 30_000 as const;
export const RECONNECT_JITTER_PCT = 0.25 as const;

/**
 * Backoff con jitter — fórmula estocástica Γ.RealtimeEng:
 *   delay = min(RECONNECT_MAX_MS, RECONNECT_BASE_MS * 2^attempt) * (1 + U(-jitter, +jitter))
 *
 * Distribución uniforme U cumple la propiedad de descorrelación necesaria
 * para evitar reconnect storms (teorema del muestreo de Poisson).
 */
export function computeBackoffMs(attempt: number, rand: () => number = Math.random): number {
  if (attempt < 0 || !Number.isInteger(attempt)) {
    throw new Error('INV-3: attempt debe ser entero no-negativo');
  }
  const expDelay = Math.min(RECONNECT_MAX_MS, RECONNECT_BASE_MS * 2 ** attempt);
  const jitterRange = expDelay * RECONNECT_JITTER_PCT;
  const jitter = (rand() * 2 - 1) * jitterRange;
  return Math.max(0, Math.floor(expDelay + jitter));
}
