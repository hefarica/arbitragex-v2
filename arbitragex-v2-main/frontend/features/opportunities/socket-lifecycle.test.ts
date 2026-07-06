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
