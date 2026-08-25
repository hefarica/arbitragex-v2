// frontend/lib/store/__tests__/realtime-slices.test.ts
//
// FE-MASTER · FE-0008 — realtime slice policy helpers, direct unit tests.
//
// The provider's effects are client-only (never under renderToStaticMarkup);
// what IS testable headlessly is the POLICY: which payloads get accepted
// into the store (fail-closed gates) and which channels go stale (budgets).
import { describe, expect, it } from "vitest";

import {
  REALTIME_CHANNELS,
  STALENESS_BUDGET_MS,
  acceptRuntimeAck,
  acceptTickPayload,
  createRealtimeSlice,
  staleChannels,
  type RealtimeChannelState,
} from "../realtime-slices";

const TICK_OK = {
  event: "route_discovery.tick",
  chain_id: 1,
  algorithm: "dfs_bounded",
  routes_found: 5,
  routes_dispatched: 2,
  detector_mask: { event: "pool_reserve_update", admitted: 51, total: 60, selected_admitted: true },
  required_data_gate: null,
};

describe("acceptTickPayload — route_discovery_telemetry gate (RG-1 fail-closed)", () => {
  it("accepts a real tick payload and passes it through verbatim", () => {
    const v = acceptTickPayload(TICK_OK);
    expect(v.ok).toBe(true);
    if (v.ok) {
      expect(v.tick.routes_found).toBe(5);
      expect(v.tick.detector_mask?.admitted).toBe(51);
    }
  });

  it("IGNORES the room's other event types (they are normal traffic, not errors)", () => {
    const v = acceptTickPayload({ event: "route_discovery.route_candidate", chain_id: 1, route_hash: "0xabc" });
    expect(v).toEqual({ ok: false, kind: "ignored", reason: "not_tick_event" });
  });

  it("ignores a payload with no event discriminator at all", () => {
    expect(acceptTickPayload({ chain_id: 1 })).toEqual({
      ok: false,
      kind: "ignored",
      reason: "not_tick_event",
    });
  });

  it("REJECTS a tick event whose shape drifted (wrong type) — never reaches setTick", () => {
    const v = acceptTickPayload({ ...TICK_OK, chain_id: "one" });
    expect(v.ok).toBe(false);
    if (!v.ok && v.kind === "rejected") {
      expect(v.reason).toContain("schema_reject");
      expect(v.reason).toContain("chain_id");
    } else {
      throw new Error("expected rejected");
    }
  });

  it("REJECTS a detector_mask that admits more detectors than exist (cross-field)", () => {
    const v = acceptTickPayload({
      ...TICK_OK,
      detector_mask: { event: "pool_reserve_update", admitted: 61, total: 60, selected_admitted: null },
    });
    expect(v.ok).toBe(false);
  });

  it("REJECTS a sidecar key riding the tick (.strict())", () => {
    const v = acceptTickPayload({ ...TICK_OK, extra: 1 });
    expect(v.ok).toBe(false);
  });
});

const ACK_OK = {
  event_id: "11111111-2222-3333-4444-555555555555",
  resource: "trading_config",
  chain_id: 1,
  idempotency_key: "op-1",
  config_hash_before: null,
  config_hash_after: "a".repeat(64),
  worker_id: "searcher-rs-1",
  layer: "searcher_rs",
  status: "applied",
};

describe("acceptRuntimeAck — runtime_ack gate", () => {
  it("accepts a schema-valid broadcast verbatim", () => {
    const v = acceptRuntimeAck(ACK_OK);
    expect(v.ok).toBe(true);
    if (v.ok) expect(v.ack.resource).toBe("trading_config");
  });

  it("rejects a malformed broadcast (no event_id) with an honest reason — never recorded", () => {
    const v = acceptRuntimeAck({ ...ACK_OK, event_id: "not-a-uuid" });
    expect(v.ok).toBe(false);
    if (!v.ok) expect(v.reason).toContain("schema_reject");
  });
});

describe("staleChannels — budget sweep (R8: null never stale)", () => {
  const ch = (ageMs: number | null): RealtimeChannelState => ({
    transport: "ws",
    status: "live",
    lastMessageAt: ageMs === null ? null : new Date(Date.now() - ageMs).toISOString(),
    lastError: null,
  });
  const channels = {
    routes: ch(10_000), // fresh
    runtime_ack: ch(365 * 24 * 3600 * 1000), // a year old — STILL not stale
    pairs: ch(200_000), // 200s > 105s budget
    quote_anchor: ch(null), // never accepted → connecting, not stale
  } as const;

  it("flags ONLY channels past their cadence budget", () => {
    expect(staleChannels(channels, Date.now())).toEqual(["pairs"]);
  });

  it("runtime_ack has NO budget (event-driven — a fabricated cadence would fake a gap)", () => {
    expect(STALENESS_BUDGET_MS.runtime_ack).toBe(Number.POSITIVE_INFINITY);
  });

  it("budgets are 3× the real cadences (routes 90s, pairs/anchor 105s)", () => {
    expect(STALENESS_BUDGET_MS.routes).toBe(90_000);
    expect(STALENESS_BUDGET_MS.pairs).toBe(105_000);
    expect(STALENESS_BUDGET_MS.quote_anchor).toBe(105_000);
  });
});

describe("createRealtimeSlice — initial state + writers", () => {
  /** zustand-like setState shim handling both object and function patches. */
  function boot() {
    let state!: ReturnType<typeof createRealtimeSlice>;
    state = createRealtimeSlice((patch) => {
      const p = typeof patch === "function" ? (patch as (s: unknown) => unknown)(state) : patch;
      state = { ...state, ...(p as object) } as typeof state;
    });
    return {
      get: () => state,
      setChannel: state.setChannel,
      setWsConnected: state.setWsConnected,
    };
  }

  it("boots every channel honest: connecting, no payload, no error", () => {
    const store = boot();
    expect(REALTIME_CHANNELS).toEqual(["routes", "runtime_ack", "pairs", "quote_anchor"]);
    expect(store.get().wsConnected).toBe(false);
    for (const id of REALTIME_CHANNELS) {
      expect(store.get().channels[id]).toEqual({
        transport: "rest",
        status: "connecting",
        lastMessageAt: null,
        lastError: null,
      });
    }
  });

  it("setChannel patches ONE channel without touching siblings", () => {
    const store = boot();
    store.setChannel("routes", { transport: "ws", status: "live", lastMessageAt: "2026-08-24T00:00:00Z" });
    expect(store.get().channels.routes?.status).toBe("live");
    expect(store.get().channels.pairs?.status).toBe("connecting");
    expect(store.get().wsConnected).toBe(false);
    store.setWsConnected(true);
    expect(store.get().wsConnected).toBe(true);
  });
});
