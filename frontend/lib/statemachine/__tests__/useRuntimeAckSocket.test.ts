/**
 * Fuzzing I/O tests for the Runtime ACK WSS bridge (Fase 4 directive).
 *
 * Strategy: test the pure factory `createRuntimeAckSocket` (no React),
 * matching the repo's existing pattern at
 * `frontend/features/opportunities/socket-lifecycle.test.ts`. Vitest is
 * configured with `environment: "node"`, so renderHook is intentionally
 * avoided — the factory carries 100% of the behaviour under test.
 *
 * Coverage:
 *   F-1 RG-2 Anti-Correlation Drift — 100 noise events + 1 match.
 *   F-2 RG-1 Anti-Mock Schema      — malformed payload Fail-Closed.
 *   F-3 RG-3 Anti-Stale UI         — heartbeat watchdog transitions to 'error'.
 */

import { describe, expect, it, vi, beforeEach, afterEach } from "vitest";
import {
  createRuntimeAckSocket,
  type RuntimeAckSocketLike,
  type RuntimeAckBroadcast,
} from "../useRuntimeAckSocket";

interface FakeSocket extends RuntimeAckSocketLike {
  trigger: (event: string, ...args: unknown[]) => void;
}

function makeFakeSocket(): FakeSocket {
  const handlers = new Map<string, (...args: unknown[]) => void>();
  const socket: FakeSocket = {
    on: vi.fn((event: string, handler: (...args: unknown[]) => void) => {
      handlers.set(event, handler);
      return socket;
    }),
    emit: vi.fn(),
    disconnect: vi.fn(),
    trigger: (event, ...args) => handlers.get(event)?.(...args),
  };
  return socket;
}

const TARGET_EVENT_ID = "11111111-2222-4333-8444-555555555555";

function validPayload(overrides: Partial<RuntimeAckBroadcast> = {}): RuntimeAckBroadcast {
  return {
    event_id: TARGET_EVENT_ID,
    resource: "trading-config",
    chain_id: 137,
    idempotency_key: "key-abc-001",
    config_hash_before: "a".repeat(64),
    config_hash_after: "b".repeat(64),
    worker_id: "searcher-rs-0",
    layer: "arc_swap",
    status: "applied",
    latency_ms: 42,
    error: null,
    ...overrides,
  };
}

// ---------------------------------------------------------------------------
// F-1 — RG-2 Anti-Correlation Drift
// ---------------------------------------------------------------------------

describe("createRuntimeAckSocket — F-1 RG-2 correlation drift", () => {
  it("ignores 100 random event_ids and only triggers onAck for the matching event_id", () => {
    const fake = makeFakeSocket();
    const onAck = vi.fn();
    const onNack = vi.fn();
    const onStatus = vi.fn();

    createRuntimeAckSocket({
      url: "wss://test.invalid",
      eventId: TARGET_EVENT_ID,
      ioFactory: () => fake,
      onStatus,
      onAck,
      onNack,
    });

    fake.trigger("connect");

    for (let i = 0; i < 100; i++) {
      const noiseId =
        "00000000-0000-4000-8000-" + i.toString(16).padStart(12, "0");
      fake.trigger("runtime_ack", validPayload({ event_id: noiseId }));
    }
    expect(onAck).not.toHaveBeenCalled();
    expect(onNack).not.toHaveBeenCalled();

    fake.trigger("runtime_ack", validPayload());

    expect(onAck).toHaveBeenCalledTimes(1);
    expect(onNack).not.toHaveBeenCalled();
    const received = onAck.mock.calls[0]?.[0] as RuntimeAckBroadcast;
    expect(received.event_id).toBe(TARGET_EVENT_ID);
    expect(received.status).toBe("applied");
  });

  it("routes status='rejected' to onNack, not onAck (terminal nack)", () => {
    const fake = makeFakeSocket();
    const onAck = vi.fn();
    const onNack = vi.fn();

    createRuntimeAckSocket({
      url: "wss://test.invalid",
      eventId: TARGET_EVENT_ID,
      ioFactory: () => fake,
      onStatus: vi.fn(),
      onAck,
      onNack,
    });

    fake.trigger("connect");
    fake.trigger("runtime_ack", validPayload({ status: "rejected", error: "config invalid" }));

    expect(onAck).not.toHaveBeenCalled();
    expect(onNack).toHaveBeenCalledTimes(1);
    expect(onNack.mock.calls[0]?.[0]).toBe("rejected");
  });
});

// ---------------------------------------------------------------------------
// F-2 — RG-1 Anti-Mock Schema (Fail-Closed on malformed payloads)
// ---------------------------------------------------------------------------

describe("createRuntimeAckSocket — F-2 RG-1 schema Fail-Closed", () => {
  it("rejects malformed payload via Zod safeParse without crashing", () => {
    const fake = makeFakeSocket();
    const onAck = vi.fn();
    const onNack = vi.fn();
    const onStatus = vi.fn();

    createRuntimeAckSocket({
      url: "wss://test.invalid",
      eventId: TARGET_EVENT_ID,
      ioFactory: () => fake,
      onStatus,
      onAck,
      onNack,
    });

    fake.trigger("connect");

    const garbageCases: unknown[] = [
      { garbage: true, event_id: null },
      null,
      undefined,
      "not an object",
      42,
      [],
      { ...validPayload(), event_id: "not-a-uuid" },
      { ...validPayload(), config_hash_after: "not-hex" },
      { ...validPayload(), layer: "unknown-layer" },
      { ...validPayload(), status: "exploded" },
      { ...validPayload(), chain_id: -1 },
      { ...validPayload(), resource: "" },
    ];

    expect(() => {
      for (const g of garbageCases) {
        fake.trigger("runtime_ack", g);
      }
    }).not.toThrow();

    expect(onAck).not.toHaveBeenCalled();
    expect(onNack).not.toHaveBeenCalled();
  });
});

// ---------------------------------------------------------------------------
// F-3 — RG-3 Anti-Stale UI (heartbeat watchdog)
// ---------------------------------------------------------------------------

describe("createRuntimeAckSocket — F-3 RG-3 stale watchdog", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  it("transitions status to 'error' after heartbeatStaleMs without traffic", () => {
    const fake = makeFakeSocket();
    const onStatus = vi.fn();

    createRuntimeAckSocket({
      url: "wss://test.invalid",
      eventId: TARGET_EVENT_ID,
      ioFactory: () => fake,
      timeoutMs: 60000,
      heartbeatStaleMs: 1000,
      onStatus,
      onAck: vi.fn(),
      onNack: vi.fn(),
    });

    expect(onStatus).toHaveBeenCalledWith("connecting");

    fake.trigger("connect");
    expect(onStatus).toHaveBeenCalledWith("connected");

    onStatus.mockClear();
    vi.advanceTimersByTime(1500);

    expect(onStatus).toHaveBeenCalledWith("error");
  });

  it("fires onNack('timeout') after timeoutMs without an ack", () => {
    const fake = makeFakeSocket();
    const onNack = vi.fn();
    const onAck = vi.fn();

    createRuntimeAckSocket({
      url: "wss://test.invalid",
      eventId: TARGET_EVENT_ID,
      ioFactory: () => fake,
      timeoutMs: 1000,
      heartbeatStaleMs: 60000,
      onStatus: vi.fn(),
      onAck,
      onNack,
    });

    fake.trigger("connect");
    vi.advanceTimersByTime(1500);

    expect(onAck).not.toHaveBeenCalled();
    expect(onNack).toHaveBeenCalledTimes(1);
    expect(onNack.mock.calls[0]?.[0]).toBe("timeout");
  });
});
