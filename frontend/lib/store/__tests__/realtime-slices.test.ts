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
  restFetchOutcome,
  runtimeAckJoinAck,
  runtimeAckRoomError,
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

// ─── restFetchOutcome — WO-GAP3 (2026-09-17) ─────────────────────────────────
//
// Regression: the provider refreshed only lastMessageAt on a successful REST
// snapshot and never wrote `status`, so pairs/quote_anchor stayed `connecting`
// forever while data flowed every 30s — and the WO-08 socket aggregate
// inherited that worst state (header read `socket CONNECTING · pairs
// CONNECTING · quote_anchor CONNECTING` permanently with the feed LIVE).
// The outcome contract: ready ⇒ accepted (caller MUST write a status that
// leaves `connecting`); error+message ⇒ failed with the REAL error (R8);
// anything else ⇒ inflight, nothing certified.

describe("restFetchOutcome — REST-pass channel outcome (WO-GAP3)", () => {
  it("ready ⇒ accepted: an accepted snapshot must be able to leave `connecting`", () => {
    expect(restFetchOutcome("ready", null)).toEqual({ kind: "accepted" });
  });

  it("error with a message ⇒ failed, the real error rides verbatim (R8)", () => {
    expect(restFetchOutcome("error", "edge 503: upstream")).toEqual({
      kind: "failed",
      lastError: "edge 503: upstream",
    });
  });

  it("idle / loading / error-without-message ⇒ inflight (nothing to certify, no fabrication)", () => {
    expect(restFetchOutcome("idle", null)).toEqual({ kind: "inflight" });
    expect(restFetchOutcome("loading", null)).toEqual({ kind: "inflight" });
    expect(restFetchOutcome("error", null)).toEqual({ kind: "inflight" });
  });
});

// ─── runtimeAckJoinAck / runtimeAckRoomError — ROOM-AUTH-01 (2026-09-17) ────
//
// Regression: the provider stamped `runtime_ack → live` the moment the shared
// socket CONNECTED, without waiting for the server's join verdict. The server
// rejects anonymous joins with 42["error",{"code":"unauthorized",
// "room":"runtime_ack"}] (QA-WS §5.2), so the header read `runtime_ack LIVE`
// over a room the socket was never admitted to. The verdict contract:
// ack ok ⇒ the ONLY path (besides an accepted broadcast) to certify `live`;
// ack nok / room-scoped error ⇒ lastError verbatim → §34 projection shows
// ERROR, never LIVE (R8: a refusal is a fact, never masked).

describe("runtimeAckJoinAck — join ack verdict (ROOM-AUTH-01)", () => {
  it("ok:true ⇒ certified (the caller may write status live)", () => {
    expect(runtimeAckJoinAck({ ok: true })).toEqual({ ok: true });
  });

  it("refusal ⇒ lastError carries the server's code verbatim (R8)", () => {
    const v = runtimeAckJoinAck({ ok: false, code: "unauthorized" });
    expect(v.ok).toBe(false);
    if (!v.ok) {
      expect(v.lastError).toContain("unauthorized");
      expect(v.lastError).toContain("admin-gated");
    }
  });

  it("garbage / missing ack reply ⇒ refused with code unknown, never certified (fail-closed)", () => {
    expect(runtimeAckJoinAck(undefined).ok).toBe(false);
    expect(runtimeAckJoinAck(null).ok).toBe(false);
    const v = runtimeAckJoinAck({});
    expect(v.ok).toBe(false);
    if (!v.ok) expect(v.lastError).toContain("unknown");
  });
});

describe("runtimeAckRoomError — room-scoped structured error (ROOM-AUTH-01)", () => {
  it("the exact observed frame maps to an honest lastError", () => {
    const lastError = runtimeAckRoomError({ code: "unauthorized", room: "runtime_ack" });
    expect(lastError).not.toBeNull();
    expect(lastError).toContain("unauthorized");
  });

  it("errors for OTHER rooms / shapes are ignored (scoped strictly)", () => {
    expect(runtimeAckRoomError({ code: "unauthorized", room: "opportunities" })).toBeNull();
    expect(runtimeAckRoomError({ code: "rate_limited" })).toBeNull();
    expect(runtimeAckRoomError(null)).toBeNull();
    expect(runtimeAckRoomError("error")).toBeNull();
  });

  it("room match with a non-string code still fails honest with unknown", () => {
    const lastError = runtimeAckRoomError({ code: 42, room: "runtime_ack" });
    expect(lastError).toContain("unknown");
  });
});
