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

// ---------------------------------------------------------------------------
// F-4 — settle idempotence & terminal status routing (FE-0046 §73 §3)
// ---------------------------------------------------------------------------

describe("createRuntimeAckSocket — F-4 settle idempotence & routing", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  it("routes status='received' to onAck (applied|received both mean persistence confirmed)", () => {
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
    fake.trigger("runtime_ack", validPayload({ status: "received" }));

    expect(onAck).toHaveBeenCalledTimes(1);
    expect(onAck.mock.calls[0]?.[0].status).toBe("received");
    expect(onNack).not.toHaveBeenCalled();
  });

  it("routes status='failed' to onNack with the wire status as reason", () => {
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
    fake.trigger("runtime_ack", validPayload({ status: "failed" }));

    expect(onAck).not.toHaveBeenCalled();
    expect(onNack).toHaveBeenCalledTimes(1);
    expect(onNack.mock.calls[0]?.[0]).toBe("failed");
    expect(onNack.mock.calls[0]?.[1]?.status).toBe("failed");
  });

  it("ignores broadcasts arriving AFTER settle — the lifecycle fires once (I-1)", () => {
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
    fake.trigger("runtime_ack", validPayload({ status: "applied" }));
    // A late second broadcast — even a NACK — must not reopen a settled cycle.
    fake.trigger("runtime_ack", validPayload({ status: "rejected" }));
    fake.trigger("runtime_ack", validPayload({ status: "timeout" }));

    expect(onAck).toHaveBeenCalledTimes(1);
    expect(onNack).not.toHaveBeenCalled();
  });

  it("timeout does NOT fire after a settle beat it to it", () => {
    const fake = makeFakeSocket();
    const onNack = vi.fn();

    createRuntimeAckSocket({
      url: "wss://test.invalid",
      eventId: TARGET_EVENT_ID,
      ioFactory: () => fake,
      timeoutMs: 1000,
      heartbeatStaleMs: 60000,
      onStatus: vi.fn(),
      onAck: vi.fn(),
      onNack,
    });

    fake.trigger("connect");
    vi.advanceTimersByTime(500);
    fake.trigger("runtime_ack", validPayload({ status: "applied" }));
    vi.advanceTimersByTime(5000);

    expect(onNack).not.toHaveBeenCalled();
  });
});

// ---------------------------------------------------------------------------
// F-5 — connect lifecycle, C4 auth triple-channel, dispose
// ---------------------------------------------------------------------------

describe("createRuntimeAckSocket — F-5 connect/auth/dispose", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  it("subscribes to the runtime_ack room upon connect", () => {
    const fake = makeFakeSocket();

    createRuntimeAckSocket({
      url: "wss://test.invalid",
      eventId: TARGET_EVENT_ID,
      ioFactory: () => fake,
      onStatus: vi.fn(),
      onAck: vi.fn(),
      onNack: vi.fn(),
    });

    fake.trigger("connect");
    expect(fake.emit).toHaveBeenCalledWith("subscribe:runtime_ack");
  });

  it("passes the C4 auth triple (auth/query/extraHeaders) when a token is provided", () => {
    const fake = makeFakeSocket();
    const seen: Record<string, unknown>[] = [];

    createRuntimeAckSocket({
      url: "wss://test.invalid",
      eventId: TARGET_EVENT_ID,
      authToken: "tok-123",
      ioFactory: (_url, opts) => {
        seen.push(opts ?? {});
        return fake;
      },
      onStatus: vi.fn(),
      onAck: vi.fn(),
      onNack: vi.fn(),
    });

    expect(seen).toHaveLength(1);
    const opts = seen[0] as Record<string, Record<string, unknown>>;
    expect(opts["auth"]).toEqual({ token: "tok-123" });
    expect(opts["query"]).toEqual({ token: "tok-123" });
    expect(opts["extraHeaders"]).toEqual({ "x-arbx-admin-token": "tok-123" });
  });

  it("omits the auth block entirely when no token is provided", () => {
    const fake = makeFakeSocket();
    const seen: Record<string, unknown>[] = [];

    createRuntimeAckSocket({
      url: "wss://test.invalid",
      eventId: TARGET_EVENT_ID,
      ioFactory: (_url, opts) => {
        seen.push(opts ?? {});
        return fake;
      },
      onStatus: vi.fn(),
      onAck: vi.fn(),
      onNack: vi.fn(),
    });

    expect(seen[0]).not.toHaveProperty("auth");
    expect(seen[0]).not.toHaveProperty("query");
    expect(seen[0]).not.toHaveProperty("extraHeaders");
  });

  it("surfaces disconnected/reconnecting while unsettled, silent after settle", () => {
    const fake = makeFakeSocket();
    const onStatus = vi.fn();

    createRuntimeAckSocket({
      url: "wss://test.invalid",
      eventId: TARGET_EVENT_ID,
      ioFactory: () => fake,
      onStatus,
      onAck: vi.fn(),
      onNack: vi.fn(),
    });

    fake.trigger("connect");
    fake.trigger("connect_error");
    expect(onStatus).toHaveBeenCalledWith("reconnecting");
    fake.trigger("disconnect");
    expect(onStatus).toHaveBeenCalledWith("disconnected");

    fake.trigger("runtime_ack", validPayload({ status: "applied" }));
    onStatus.mockClear();
    fake.trigger("disconnect");
    expect(onStatus).not.toHaveBeenCalled();
  });

  it("dispose() settles, disconnects the socket and cancels the pending timeout", () => {
    const fake = makeFakeSocket();
    const onNack = vi.fn();

    const handle = createRuntimeAckSocket({
      url: "wss://test.invalid",
      eventId: TARGET_EVENT_ID,
      ioFactory: () => fake,
      timeoutMs: 1000,
      heartbeatStaleMs: 60000,
      onStatus: vi.fn(),
      onAck: vi.fn(),
      onNack,
    });

    fake.trigger("connect");
    handle.dispose();

    expect(fake.disconnect).toHaveBeenCalledTimes(1);
    vi.advanceTimersByTime(10000);
    expect(onNack).not.toHaveBeenCalled();
    // Late broadcast after dispose cannot reopen the cycle.
    fake.trigger("runtime_ack", validPayload({ status: "applied" }));
    expect(onNack).not.toHaveBeenCalled();
  });

  it("dispose() never throws even if the underlying disconnect throws", () => {
    const boom: FakeSocket = makeFakeSocket();
    boom.disconnect = () => {
      throw new Error("already closed");
    };

    const handle = createRuntimeAckSocket({
      url: "wss://test.invalid",
      eventId: TARGET_EVENT_ID,
      ioFactory: () => boom,
      onStatus: vi.fn(),
      onAck: vi.fn(),
      onNack: vi.fn(),
    });

    expect(() => handle.dispose()).not.toThrow();
  });
});

// ---------------------------------------------------------------------------
// F-6 — watchdog rearm semantics (RG-3: traffic keeps the channel alive)
// ---------------------------------------------------------------------------

describe("createRuntimeAckSocket — F-6 watchdog rearm on traffic", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  it("a VALID foreign-event_id broadcast rearms the watchdog (ignored, not stale)", () => {
    const fake = makeFakeSocket();
    const onStatus = vi.fn();
    const onAck = vi.fn();

    createRuntimeAckSocket({
      url: "wss://test.invalid",
      eventId: TARGET_EVENT_ID,
      ioFactory: () => fake,
      timeoutMs: 60000,
      heartbeatStaleMs: 1000,
      onStatus,
      onAck,
      onNack: vi.fn(),
    });

    fake.trigger("connect");
    vi.advanceTimersByTime(800);
    // Foreign but VALID traffic: RG-2 ignores the payload, yet the channel
    // is demonstrably alive — the watchdog must rearm, not fire.
    fake.trigger(
      "runtime_ack",
      validPayload({ event_id: "99999999-8888-4777-8666-555555555555" }),
    );
    vi.advanceTimersByTime(800);
    expect(onStatus).not.toHaveBeenCalledWith("error");
    expect(onAck).not.toHaveBeenCalled();

    // Silence past the REARMED window (not the original one) → error.
    vi.advanceTimersByTime(300);
    expect(onStatus).toHaveBeenCalledWith("error");
  });
});
