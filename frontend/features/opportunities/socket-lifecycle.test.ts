// frontend/features/opportunities/socket-lifecycle.test.ts
import { describe, expect, it, vi } from "vitest";

import { createOpportunitySocket, type SocketLike } from "./socket-lifecycle";

function makeFakeSocket() {
  const handlers = new Map<string, (...args: unknown[]) => void>();
  const socket: SocketLike & { trigger: (e: string, ...a: unknown[]) => void } = {
    on: vi.fn((event: string, handler: (...args: unknown[]) => void) => {
      handlers.set(event, handler);
      return socket;
    }),
    off: vi.fn((event: string) => {
      handlers.delete(event);
      return socket;
    }),
    emit: vi.fn(),
    disconnect: vi.fn(),
    trigger: (event, ...args) => handlers.get(event)?.(...args),
  };
  return socket;
}

// ─── Storm prevention contract ───────────────────────────────────────────────

describe("createOpportunitySocket — single-instance contract (storm prevention)", () => {
  it("invokes ioFactory exactly once per call", () => {
    const fake = makeFakeSocket();
    const ioFactory = vi.fn(() => fake);

    createOpportunitySocket({
      url: "http://x",
      ioFactory,
      onStatus: vi.fn(),
      onOpportunity: vi.fn(),
    });

    expect(ioFactory).toHaveBeenCalledTimes(1);
  });

  it("passes the configured url and connection opts to ioFactory", () => {
    const fake = makeFakeSocket();
    const ioFactory = vi.fn(() => fake);

    createOpportunitySocket({
      url: "https://api.example.test",
      ioFactory,
      onStatus: vi.fn(),
      onOpportunity: vi.fn(),
    });

    expect(ioFactory).toHaveBeenCalledWith(
      "https://api.example.test",
      expect.objectContaining({ reconnectionAttempts: 5, timeout: 2000 }),
    );
  });
});

// ─── Status mapping ──────────────────────────────────────────────────────────

describe("createOpportunitySocket — status mapping", () => {
  it("'connect' event sets status LIVE and emits subscribe:opportunities", () => {
    const fake = makeFakeSocket();
    const onStatus = vi.fn();
    createOpportunitySocket({
      url: "http://x",
      ioFactory: () => fake,
      onStatus,
      onOpportunity: vi.fn(),
    });

    fake.trigger("connect");

    expect(onStatus).toHaveBeenCalledWith("LIVE");
    expect(fake.emit).toHaveBeenCalledWith("subscribe:opportunities");
  });

  it("'disconnect' event sets status STALE", () => {
    const fake = makeFakeSocket();
    const onStatus = vi.fn();
    createOpportunitySocket({
      url: "http://x",
      ioFactory: () => fake,
      onStatus,
      onOpportunity: vi.fn(),
    });

    fake.trigger("disconnect");

    expect(onStatus).toHaveBeenCalledWith("STALE");
  });

  it("'connect_error' event sets status STALE (covers cold-backend case)", () => {
    const fake = makeFakeSocket();
    const onStatus = vi.fn();
    createOpportunitySocket({
      url: "http://x",
      ioFactory: () => fake,
      onStatus,
      onOpportunity: vi.fn(),
    });

    fake.trigger("connect_error", new Error("ECONNREFUSED"));

    expect(onStatus).toHaveBeenCalledWith("STALE");
  });

  it("does NOT call onStatus until an event fires (initial state owned by caller)", () => {
    const fake = makeFakeSocket();
    const onStatus = vi.fn();
    createOpportunitySocket({
      url: "http://x",
      ioFactory: () => fake,
      onStatus,
      onOpportunity: vi.fn(),
    });

    expect(onStatus).not.toHaveBeenCalled();
  });
});

// ─── Opportunity dispatch and dispose ────────────────────────────────────────

describe("createOpportunitySocket — opportunity dispatch and dispose", () => {
  it("'new_opportunity' event forwards payload to onOpportunity", () => {
    const fake = makeFakeSocket();
    const onOpportunity = vi.fn();
    createOpportunitySocket({
      url: "http://x",
      ioFactory: () => fake,
      onStatus: vi.fn(),
      onOpportunity,
    });

    const payload = {
      id: "abc",
      timestamp: 0,
      route: "WETH/USDC",
      expected_profit_usd: 12.5,
      net_roi_pct: 0.42,
      score: 91,
    };
    fake.trigger("new_opportunity", payload);

    expect(onOpportunity).toHaveBeenCalledWith(payload);
  });

  it("dispose() calls socket.disconnect() exactly once", () => {
    const fake = makeFakeSocket();
    const handle = createOpportunitySocket({
      url: "http://x",
      ioFactory: () => fake,
      onStatus: vi.fn(),
      onOpportunity: vi.fn(),
    });

    handle.dispose();

    expect(fake.disconnect).toHaveBeenCalledTimes(1);
  });
});

// ─── FE-0047: reconnect, C4 auth triple, dispose off-pin ────────────────────

describe("createOpportunitySocket — FE-0047 realtime contract", () => {
  it("reconnect re-emits subscribe:opportunities and re-reports LIVE (STALE→LIVE again)", () => {
    const fake = makeFakeSocket();
    const onStatus = vi.fn();
    createOpportunitySocket({
      url: "http://x",
      ioFactory: () => fake,
      onStatus,
      onOpportunity: vi.fn(),
    });

    // First session: connected, then the transport drops.
    fake.trigger("connect");
    fake.trigger("disconnect");
    // socket.io reconnects the SAME socket instance → 'connect' fires again.
    fake.trigger("connect");

    // LIVE re-reported after the STALE dip…
    expect(onStatus.mock.calls.filter(([s]) => s === "LIVE")).toHaveLength(2);
    expect(onStatus.mock.calls.filter(([s]) => s === "STALE")).toHaveLength(1);
    // …and the room subscription is RE-emitted (one per connect — a
    // reconnect without re-subscribe silently stops delivering events).
    expect(fake.emit).toHaveBeenCalledWith("subscribe:opportunities");
    expect(
      vi.mocked(fake.emit).mock.calls.filter((c) => c[0] === "subscribe:opportunities"),
    ).toHaveLength(2);
  });

  it("C4: an admin token rides ALL THREE transport channels of the handshake", () => {
    const fake = makeFakeSocket();
    const ioFactory = vi.fn((_url: string, _opts: unknown) => fake);
    createOpportunitySocket({
      url: "http://x",
      ioFactory,
      authToken: "tok-123",
      onStatus: vi.fn(),
      onOpportunity: vi.fn(),
    });

    const opts = vi.mocked(ioFactory).mock.calls[0]![1] as Record<string, unknown>;
    expect(opts["auth"]).toEqual({ token: "tok-123" });
    expect(opts["query"]).toEqual({ token: "tok-123" });
    expect(opts["extraHeaders"]).toEqual({ "x-arbx-admin-token": "tok-123" });
  });

  it("C4: NO auth channel is populated when no token is present (anonymous public room)", () => {
    const fake = makeFakeSocket();
    const ioFactory = vi.fn((_url: string, _opts: unknown) => fake);
    createOpportunitySocket({
      url: "http://x",
      ioFactory,
      onStatus: vi.fn(),
      onOpportunity: vi.fn(),
    });

    const opts = vi.mocked(ioFactory).mock.calls[0]![1] as Record<string, unknown>;
    expect(opts).not.toHaveProperty("auth");
    expect(opts).not.toHaveProperty("query");
    expect(opts).not.toHaveProperty("extraHeaders");
  });

  it("dispose() detaches ALL FOUR listeners (PERF 2026-08-10 off-pin)", () => {
    const fake = makeFakeSocket();
    const handle = createOpportunitySocket({
      url: "http://x",
      ioFactory: () => fake,
      onStatus: vi.fn(),
      onOpportunity: vi.fn(),
    });

    handle.dispose();

    const offEvents = vi.mocked(fake.off).mock.calls.map((c) => c[0]);
    expect(offEvents).toEqual(
      expect.arrayContaining(["connect", "disconnect", "connect_error", "new_opportunity"]),
    );
    expect(offEvents).toHaveLength(4);
  });
});
