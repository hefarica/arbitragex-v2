/**
 * Ω-S5++ C9.∞ APEX · Tests realtime
 * Validan: backoff con jitter, schemas de envelope, isolation por chain.
 */
import { describe, expect, it } from 'vitest';
import {
  computeBackoffMs,
  RECONNECT_BASE_MS,
  RECONNECT_MAX_MS,
  RECONNECT_JITTER_PCT,
  SnapshotEnvelopeSchema,
  DeltaEnvelopeSchema,
  HeartbeatEnvelopeSchema,
  AckEnvelopeSchema,
  StaleEnvelopeSchema,
  RealtimeEnvelopeSchema,
} from '../frontend/lib/apex/schemas/realtime';

describe('computeBackoffMs', () => {
  it('attempt=0 returns base ± jitter', () => {
    const delay = computeBackoffMs(0, () => 0.5); // jitter = 0
    expect(delay).toBe(RECONNECT_BASE_MS);
  });

  it('grows exponentially up to the cap', () => {
    const noJitter = (): number => 0.5;
    expect(computeBackoffMs(1, noJitter)).toBe(RECONNECT_BASE_MS * 2);
    expect(computeBackoffMs(2, noJitter)).toBe(RECONNECT_BASE_MS * 4);
    expect(computeBackoffMs(100, noJitter)).toBe(RECONNECT_MAX_MS);
  });

  it('jitter range respects ±25%', () => {
    const minJ = computeBackoffMs(3, () => 0); // -25%
    const maxJ = computeBackoffMs(3, () => 1); // +25%
    const expDelay = RECONNECT_BASE_MS * 8;
    expect(minJ).toBeGreaterThanOrEqual(Math.floor(expDelay * (1 - RECONNECT_JITTER_PCT)));
    expect(maxJ).toBeLessThanOrEqual(Math.ceil(expDelay * (1 + RECONNECT_JITTER_PCT)));
  });

  it('throws on negative attempt', () => {
    expect(() => computeBackoffMs(-1)).toThrow();
  });
});

describe('Realtime envelope schemas', () => {
  const baseEnvelope = {
    event_id: '11111111-1111-4111-8111-111111111111',
    chain_id: 1,
    emitted_at: '2026-05-14T18:00:00+00:00',
    sequence_id: 42,
  };

  it('SNAPSHOT envelope parses', () => {
    const v = {
      ...baseEnvelope,
      type: 'SNAPSHOT',
      channel: 'arbx:chains',
      payload: { foo: 'bar' },
    };
    expect(SnapshotEnvelopeSchema.safeParse(v).success).toBe(true);
  });

  it('DELTA without previous_sequence_id is rejected', () => {
    const v = {
      ...baseEnvelope,
      type: 'DELTA',
      channel: 'arbx:chains',
      op: 'UPDATE',
      entity_id: 'chain-1',
      payload: {},
    };
    expect(DeltaEnvelopeSchema.safeParse(v).success).toBe(false);
  });

  it('STALE envelope discriminates by reason enum', () => {
    const v = {
      ...baseEnvelope,
      type: 'STALE',
      reason: 'SEQUENCE_GAP',
      last_known_sequence_id: 10,
      expected_sequence_id: 12,
    };
    expect(StaleEnvelopeSchema.safeParse(v).success).toBe(true);
  });

  it('ACK with config_hash_after wrong format is rejected', () => {
    const v = {
      ...baseEnvelope,
      type: 'ACK',
      reload_channel: 'arbx:config:chains:1:reload',
      config_hash_after: 'not-a-hash',
      worker_id: 'searcher-1',
      success: true,
      failure_reason: null,
    };
    expect(AckEnvelopeSchema.safeParse(v).success).toBe(false);
  });

  it('Heartbeat without worker_id is rejected', () => {
    const v = { ...baseEnvelope, type: 'HEARTBEAT', uptime_seconds: 10 };
    expect(HeartbeatEnvelopeSchema.safeParse(v).success).toBe(false);
  });

  it('Discriminated union narrows by type', () => {
    const v = {
      ...baseEnvelope,
      type: 'HEARTBEAT',
      worker_id: 'spine-1',
      uptime_seconds: 100,
    };
    const parsed = RealtimeEnvelopeSchema.safeParse(v);
    expect(parsed.success).toBe(true);
    if (parsed.success && parsed.data.type === 'HEARTBEAT') {
      expect(parsed.data.worker_id).toBe('spine-1');
    }
  });
});
