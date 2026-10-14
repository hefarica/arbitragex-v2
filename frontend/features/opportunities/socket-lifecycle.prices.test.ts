// frontend/features/opportunities/socket-lifecycle.prices.test.ts
// G-PRICE-1 v4 (t5) — prices arm of createOpportunitySocket:
// optional onPriceTick/onPricesSnapshot callbacks. Contract (v4 patch):
//   - callbacks NOT passed ⇒ NO extra listeners registered, dispose detaches
//     exactly the original 4 (socket observable identical to pre-t5).
//   - callbacks passed ⇒ conditional listeners prices:tick / prices:snapshot
//     registered AND detached on dispose (stable identity).
//   - connect emits `subscribe:prices` payload-less (global public room) in
//     addition to `subscribe:opportunities`.
// The pre-existing suite (socket-lifecycle.test.ts) stays untouched and green.
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

const BASE_OPTS = {
  url: "http://x",
  onStatus: vi.fn(),
  onOpportunity: vi.fn(),
};

describe("createOpportunitySocket — prices arm (G-PRICE-1 v4, t5)", () => {
  it("no price callbacks ⇒ socket identical: 4 listeners only, dispose detaches 4", () => {
    const fake = makeFakeSocket();
    const ioFactory = vi.fn(() => fake);
    const handle = createOpportunitySocket({ ...BASE_OPTS, ioFactory });

    // exactly the 4 pre-t5 events registered — no prices listeners
    const onEvents = vi.mocked(fake.on).mock.calls.map((c) => c[0]);
    expect(onEvents).toEqual(["connect", "disconnect", "connect_error", "new_opportunity"]);

    handle.dispose();
    const offEvents = vi.mocked(fake.off).mock.calls.map((c) => c[0]);
    expect(offEvents).toHaveLength(4);
    expect(offEvents).toEqual(
      expect.arrayContaining(["connect", "disconnect", "connect_error", "new_opportunity"]),
    );
  });

  it("onPriceTick only ⇒ 5 listeners (prices:tick added), dispose detaches 5", () => {
    const fake = makeFakeSocket();
    const handle = createOpportunitySocket({
      ...BASE_OPTS,
      ioFactory: () => fake,
      onPriceTick: vi.fn(),
    });

    const onEvents = vi.mocked(fake.on).mock.calls.map((c) => c[0]);
    expect(onEvents).toHaveLength(5);
    expect(onEvents).toContain("prices:tick");
    expect(onEvents).not.toContain("prices:snapshot");

    handle.dispose();
    const offEvents = vi.mocked(fake.off).mock.calls.map((c) => c[0]);
    expect(offEvents).toHaveLength(5);
    expect(offEvents).toContain("prices:tick");
  });

  it("both callbacks ⇒ 6 listeners, dispose detaches all 6", () => {
    const fake = makeFakeSocket();
    const handle = createOpportunitySocket({
      ...BASE_OPTS,
      ioFactory: () => fake,
      onPriceTick: vi.fn(),
      onPricesSnapshot: vi.fn(),
    });

    expect(vi.mocked(fake.on).mock.calls).toHaveLength(6);
    handle.dispose();
    expect(vi.mocked(fake.off).mock.calls).toHaveLength(6);
    expect(vi.mocked(fake.off).mock.calls.map((c) => c[0])).toEqual(
      expect.arrayContaining(["prices:tick", "prices:snapshot"]),
    );
  });

  it("connect emits subscribe:opportunities AND payload-less subscribe:prices", () => {
    const fake = makeFakeSocket();
    createOpportunitySocket({ ...BASE_OPTS, ioFactory: () => fake });

    fake.trigger("connect");

    expect(fake.emit).toHaveBeenCalledWith("subscribe:opportunities");
    const priceCalls = vi.mocked(fake.emit).mock.calls.filter((c) => c[0] === "subscribe:prices");
    expect(priceCalls).toHaveLength(1);
    // payload-less: emit("subscribe:prices") — exactly one arg (α11 dispatcher
    // routes undefined payloads to the v4 bus arm, never to the legacy arm).
    expect(priceCalls[0]).toHaveLength(1);
  });

  it("reconnect re-emits payload-less subscribe:prices (one per connect)", () => {
    const fake = makeFakeSocket();
    createOpportunitySocket({ ...BASE_OPTS, ioFactory: () => fake });

    fake.trigger("connect");
    fake.trigger("disconnect");
    fake.trigger("connect");

    const priceCalls = vi.mocked(fake.emit).mock.calls.filter((c) => c[0] === "subscribe:prices");
    expect(priceCalls).toHaveLength(2);
    expect(priceCalls.every((c) => c.length === 1)).toBe(true);
  });

  it("'prices:tick' forwards payload to onPriceTick", () => {
    const fake = makeFakeSocket();
    const onPriceTick = vi.fn();
    createOpportunitySocket({ ...BASE_OPTS, ioFactory: () => fake, onPriceTick });

    const tick = { pair: "WETH/USDC", mid: 3000.5, ts: 1, src: "binance", seq: 7 };
    fake.trigger("prices:tick", tick);

    expect(onPriceTick).toHaveBeenCalledWith(tick);
  });

  it("'prices:snapshot' forwards payload to onPricesSnapshot", () => {
    const fake = makeFakeSocket();
    const onPricesSnapshot = vi.fn();
    createOpportunitySocket({ ...BASE_OPTS, ioFactory: () => fake, onPricesSnapshot });

    const payload = { ticks: [] };
    fake.trigger("prices:snapshot", payload);

    expect(onPricesSnapshot).toHaveBeenCalledWith(payload);
  });
});
