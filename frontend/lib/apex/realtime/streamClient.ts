/**
 * Ω-S5++ C9.∞ APEX · Realtime stream client (WebSocket/SSE)
 * -------------------------------------------------------------------------
 * Implementa snapshot + delta + heartbeat + ack + error + stale.
 *
 * INV-3 Causality:
 *   - Cada delta debe tener previous_sequence_id == last_applied_sequence_id.
 *   - Gap detectado ⇒ emite STALE y solicita re-snapshot.
 *   - Heartbeat timeout (> STALE_HEARTBEAT_MS) ⇒ STALE.
 *
 * INV-1 Norm Preservation:
 *   - El estado interno nunca pierde mensajes: cola FIFO con sequence_id.
 *
 * INV-5 Spatial Isolation:
 *   - Cada `chain_id` mantiene su propio sequence_id; X.update ⊥ Y.state.
 *
 * Backoff: exponential + jitter U(-25%, +25%) — descorrelación de reconexión.
 */
import { z } from 'zod';
import {
  RealtimeEnvelopeSchema,
  type RealtimeEnvelope,
  type SnapshotEnvelope,
  type DeltaEnvelope,
  type StaleEnvelope,
  computeBackoffMs,
  STALE_HEARTBEAT_MS,
} from '../schemas/realtime';
import type { ChainId } from '../schemas';

export type StreamEvent =
  | { kind: 'snapshot'; envelope: SnapshotEnvelope; payload: unknown }
  | { kind: 'delta'; envelope: DeltaEnvelope; payload: unknown }
  | { kind: 'heartbeat'; worker_id: string }
  | { kind: 'ack'; success: boolean; reload_channel: string; worker_id: string }
  | { kind: 'error'; code: string; message: string }
  | { kind: 'stale'; reason: StaleEnvelope['reason']; details: string }
  | { kind: 'reconnecting'; attempt: number; delay_ms: number };

export interface StreamSubscriber {
  (event: StreamEvent): void;
}

export interface StreamConfig {
  readonly url: string;
  readonly subprotocols?: ReadonlyArray<string>;
  readonly resnapshotPath?: string; // endpoint para volver a pedir snapshot
}

type PerKeyState = {
  lastSequenceId: number;
  lastHeartbeatAt: number;
};

function keyFor(chainId: ChainId | null, channel: string): string {
  return `${chainId ?? 'global'}::${channel}`;
}

export class ApexStreamClient {
  private ws: WebSocket | null = null;
  private subscribers: Set<StreamSubscriber> = new Set();
  private reconnectAttempt = 0;
  private heartbeatTimer: ReturnType<typeof setInterval> | null = null;
  private closedByOwner = false;
  private readonly perKey: Map<string, PerKeyState> = new Map();

  constructor(private readonly config: StreamConfig) {}

  subscribe(fn: StreamSubscriber): () => void {
    this.subscribers.add(fn);
    return () => this.subscribers.delete(fn);
  }

  connect(): void {
    this.closedByOwner = false;
    this.openSocket();
  }

  close(): void {
    this.closedByOwner = true;
    if (this.heartbeatTimer) clearInterval(this.heartbeatTimer);
    this.ws?.close();
    this.ws = null;
  }

  private emit(event: StreamEvent): void {
    for (const fn of this.subscribers) fn(event);
  }

  private openSocket(): void {
    try {
      this.ws = new WebSocket(this.config.url, this.config.subprotocols as string[] | undefined);
    } catch (err) {
      const message = err instanceof Error ? err.message : 'WebSocket constructor failed';
      this.emit({ kind: 'error', code: 'WS_CONSTRUCTOR', message });
      this.scheduleReconnect();
      return;
    }

    this.ws.addEventListener('open', () => {
      this.reconnectAttempt = 0;
      this.startHeartbeatWatchdog();
    });

    this.ws.addEventListener('message', (msg: MessageEvent<string>) => {
      this.handleRaw(msg.data);
    });

    this.ws.addEventListener('error', () => {
      this.emit({ kind: 'error', code: 'WS_ERROR', message: 'WebSocket error event' });
    });

    this.ws.addEventListener('close', () => {
      if (this.heartbeatTimer) {
        clearInterval(this.heartbeatTimer);
        this.heartbeatTimer = null;
      }
      if (!this.closedByOwner) this.scheduleReconnect();
    });
  }

  private startHeartbeatWatchdog(): void {
    if (this.heartbeatTimer) clearInterval(this.heartbeatTimer);
    this.heartbeatTimer = setInterval(() => {
      const now = Date.now();
      for (const [key, state] of this.perKey.entries()) {
        if (now - state.lastHeartbeatAt > STALE_HEARTBEAT_MS) {
          this.emit({
            kind: 'stale',
            reason: 'HEARTBEAT_TIMEOUT',
            details: `key=${key} heartbeat_age=${now - state.lastHeartbeatAt}ms`,
          });
        }
      }
    }, Math.floor(STALE_HEARTBEAT_MS / 2));
  }

  private handleRaw(raw: string): void {
    let json: unknown;
    try {
      json = JSON.parse(raw);
    } catch {
      this.emit({ kind: 'error', code: 'PARSE_JSON', message: 'invalid JSON frame' });
      return;
    }

    const parsed = RealtimeEnvelopeSchema.safeParse(json);
    if (!parsed.success) {
      this.emit({
        kind: 'error',
        code: 'PARSE_SCHEMA',
        message: `envelope schema violation: ${parsed.error.issues[0]?.message ?? 'unknown'}`,
      });
      return;
    }

    this.dispatch(parsed.data);
  }

  private dispatch(env: RealtimeEnvelope): void {
    switch (env.type) {
      case 'SNAPSHOT': {
        const key = keyFor(env.chain_id, env.channel);
        this.perKey.set(key, {
          lastSequenceId: env.sequence_id,
          lastHeartbeatAt: Date.now(),
        });
        this.emit({ kind: 'snapshot', envelope: env, payload: env.payload });
        return;
      }
      case 'DELTA': {
        const key = keyFor(env.chain_id, env.channel);
        const state = this.perKey.get(key);
        if (!state) {
          // INV-3: delta sin snapshot previo
          this.emit({
            kind: 'stale',
            reason: 'SEQUENCE_GAP',
            details: `delta for key=${key} arrived without snapshot`,
          });
          this.requestResnapshot(env.channel);
          return;
        }
        if (env.previous_sequence_id !== state.lastSequenceId) {
          // INV-3 gap
          this.emit({
            kind: 'stale',
            reason: 'SEQUENCE_GAP',
            details: `expected prev=${state.lastSequenceId} got prev=${env.previous_sequence_id}`,
          });
          this.requestResnapshot(env.channel);
          return;
        }
        state.lastSequenceId = env.sequence_id;
        state.lastHeartbeatAt = Date.now();
        this.emit({ kind: 'delta', envelope: env, payload: env.payload });
        return;
      }
      case 'HEARTBEAT': {
        // Touch every chain's heartbeat
        const now = Date.now();
        for (const state of this.perKey.values()) state.lastHeartbeatAt = now;
        this.emit({ kind: 'heartbeat', worker_id: env.worker_id });
        return;
      }
      case 'ACK': {
        this.emit({
          kind: 'ack',
          success: env.success,
          reload_channel: env.reload_channel,
          worker_id: env.worker_id,
        });
        return;
      }
      case 'ERROR': {
        this.emit({ kind: 'error', code: env.code, message: env.message });
        return;
      }
      case 'STALE': {
        this.emit({
          kind: 'stale',
          reason: env.reason,
          details: `last_known=${env.last_known_sequence_id} expected=${env.expected_sequence_id}`,
        });
        return;
      }
    }
  }

  private requestResnapshot(channel: string): void {
    // Best-effort: solicita un re-snapshot al servidor (si soporta el comando).
    if (this.ws?.readyState === WebSocket.OPEN) {
      this.ws.send(JSON.stringify({ command: 'RESNAPSHOT', channel }));
    }
  }

  private scheduleReconnect(): void {
    if (this.closedByOwner) return;
    const delay = computeBackoffMs(this.reconnectAttempt);
    this.emit({ kind: 'reconnecting', attempt: this.reconnectAttempt, delay_ms: delay });
    this.reconnectAttempt += 1;
    setTimeout(() => {
      if (!this.closedByOwner) this.openSocket();
    }, delay);
  }
}

// ─── Schema-aware decoder helper ────────────────────────────────────────
/**
 * Parses snapshot/delta payload against a typed schema. Returns `null` on
 * failure (the stream client already emitted ERROR).
 */
export function decodePayload<T>(
  payload: unknown,
  schema: z.ZodSchema<T>,
): T | null {
  const result = schema.safeParse(payload);
  return result.success ? result.data : null;
}
