"use client";

/**
 * =============================================================================
 * FE-MASTER · ArbxRealtimeProvider (FE-0008 — P2-STORE-RT, §33)
 * =============================================================================
 *
 * The ONE mount that owns the realtime connection policy. Mounted once in
 * the root layout; renders NOTHING. Two transports, per channel:
 *
 *   WS (socket.io → api-server, admin token on the handshake like the
 *   opportunities hook's C4 path):
 *     - `subscribe:route_discovery` → `route_discovery_telemetry` events.
 *       Only `route_discovery.tick` payloads pass acceptTickPayload (the
 *       room legitimately carries four other event types); a schema
 *       reject NEVER reaches setTick (RG-1 fail-closed, channel lastError
 *       records the drift). The worker publishes the SAME enriched
 *       tick_summary to the room and the durable snapshot, so the WS push
 *       and GET /api/route-discovery/tick are one shape.
 *     - `subscribe:runtime_ack` → `runtime_ack` broadcasts; the server
 *       re-checks the admin capability on the join and answers the ack with
 *       the verdict (ROOM-AUTH-01: connect alone never certifies the
 *       channel — a refused join rides lastError → ERROR, never LIVE).
 *       Accepted payloads feed RuntimeAckSlice.recordAck — malformed ones
 *       are dropped with an honest channel error, never recorded.
 *   REST loop (30s + immediate first pass):
 *     - pairs / quote_anchor: REST-native surfaces (no WS room today);
 *       a successful fetch marks the channel `live` on transport `rest`.
 *     - routes: fetchTick is the WS fallback (status `polling`) plus the
 *       initial pass so data lands before the first WS push.
 *
 * Staleness: a 15s sweep flags channels whose last ACCEPTED payload is
 * older than 3× their cadence budget (staleChannels — pure, tested).
 * runtime_ack is event-driven and has NO budget by design.
 *
 * Explicit v1 boundaries (RULE 00 — no fabricated states): the
 * opportunities page keeps its own tested page-local lifecycle
 * (useOpportunitiesStream + WS-POLL-1); cartridge `telemetry`, `metrics`,
 * `convergence` and `prices` rooms have no store consumers yet; health /
 * drift leaves wait on FE-0006/0007. FE-0009 renders RealtimeSlice.
 *
 * R1: every nondeterministic touch (socket, timers, Date) lives inside
 * useEffect — SSR renders nothing and hydration is byte-identical.
 */

// SSR-test support (repo pattern, cf. PairIntelligencePanel): the node test
// transformer's classic JSX path needs the React namespace in module scope.
import * as React from "react";
import { useEffect, useRef, type ReactNode } from "react";
import { io } from "socket.io-client";

import { getAdminToken } from "@/lib/admin-token";
import { getWsBaseUrl } from "@/lib/api-client";
import type { FetchStatus } from "@/lib/store/runtime-slices";
import { useOmniStore } from "@/lib/store/omni-store";
import {
  acceptRuntimeAck,
  acceptTickPayload,
  restFetchOutcome,
  runtimeAckJoinAck,
  runtimeAckRoomError,
  staleChannels,
} from "@/lib/store/realtime-slices";

/** Snapshot cadence: pairs/anchor TTL is 35s, the tick loop ~30s. */
const REST_POLL_MS = 30_000;
/** Staleness sweep interval (budgets are 3× cadence — see realtime-slices). */
const STALE_SWEEP_MS = 15_000;

export function ArbxRealtimeProvider({ children }: { children?: ReactNode }) {
  // Interval closures must read live liveness without re-subscribing.
  const wsConnectedRef = useRef(false);

  useEffect(() => {
    // ── helpers over the store (getState keeps this out of the render) ─────
    const store = useOmniStore;
    const stamp = () => new Date().toISOString();
    // WO-GAP3 (2026-09-17): an accepted payload must also WRITE the status —
    // refreshing lastMessageAt alone left REST-native channels `connecting`
    // forever while their snapshots flowed every 30s (mislabeled header).
    const markFresh = (id: "routes" | "runtime_ack" | "pairs" | "quote_anchor") =>
      store.getState().setChannel(id, {
        status: "live",
        lastMessageAt: stamp(),
        lastError: null,
      });
    // WO-GAP3 (2026-09-17): a FAILED snapshot surfaces its real error (R8)
    // instead of silently staying `connecting` with no lastError.
    const applyRestOutcome = (
      id: "routes" | "pairs" | "quote_anchor",
      status: FetchStatus,
      error: string | null,
    ) => {
      const outcome = restFetchOutcome(status, error);
      if (outcome.kind === "accepted") markFresh(id);
      else if (outcome.kind === "failed") {
        store.getState().setChannel(id, { lastError: outcome.lastError });
      }
    };

    // ── REST loop: REST-native channels + WS fallback for routes ──────────
    const restPass = async () => {
      const s = store.getState();
      await s.fetchPairs();
      const afterPairs = store.getState();
      applyRestOutcome("pairs", afterPairs.pairsStatus, afterPairs.pairsError);

      await s.fetchQuoteAnchor();
      const afterAnchor = store.getState();
      applyRestOutcome(
        "quote_anchor",
        afterAnchor.quoteAnchorStatus,
        afterAnchor.quoteAnchorError,
      );

      // Routes: REST only when the socket is not delivering (fallback) —
      // the initial pass runs unconditionally below so data lands first.
      // WO-GAP3 (2026-09-17): re-check liveness AFTER the awaits — a pass
      // that started mid-disconnect must not clobber the reconnect handler's
      // `live` with `polling` (source of the LIVE↔DEGRADED flap while the
      // socket was actually up).
      if (!wsConnectedRef.current) {
        await s.fetchTick();
        if (wsConnectedRef.current) return; // socket re-delivering; WS owns routes
        const afterTick = store.getState();
        if (afterTick.tickStatus === "ready") {
          markFresh("routes");
          store.getState().setChannel("routes", {
            transport: "rest",
            status: "polling",
          });
        } else if (afterTick.tickError !== null) {
          // WO-GAP3 (2026-09-17): REST fallback failure rides verbatim.
          store.getState().setChannel("routes", {
            lastError: afterTick.tickError,
          });
        }
      }
    };
    void restPass(); // immediate first pass (data before the first WS push)
    const restTimer = setInterval(() => void restPass(), REST_POLL_MS);

    // ── Staleness sweep (pure classifier over the live slice) ─────────────
    const sweep = () => {
      for (const id of staleChannels(store.getState().channels, Date.now())) {
        store.getState().setChannel(id, { status: "stale" });
      }
    };
    const staleTimer = setInterval(sweep, STALE_SWEEP_MS);

    // ── ONE socket.io connection for every WS channel ─────────────────────
    const socket = io(getWsBaseUrl(), {
      transports: ["websocket", "polling"],
      auth: { token: getAdminToken() ?? undefined },
    });

    socket.on("connect", () => {
      wsConnectedRef.current = true;
      const s = store.getState();
      s.setWsConnected(true);
      socket.emit("subscribe:route_discovery");
      // ROOM-AUTH-01 (2026-09-17): runtime_ack is an admin-gated room — the
      // server re-checks the capability on EVERY join and rejects anonymous
      // ones (`42["error",{"code":"unauthorized","room":"runtime_ack"}]`,
      // QA-WS §5.2). Connect alone certifies NOTHING: the channel may only
      // go `live` on the join verdict (ack below) or an accepted broadcast
      // (`runtime_ack` handler); a refusal rides lastError → ERROR chip (R8).
      socket.emit(
        "subscribe:runtime_ack",
        (res: unknown) => {
          const verdict = runtimeAckJoinAck(res);
          if (verdict.ok) {
            store.getState().setChannel("runtime_ack", {
              transport: "ws",
              status: "live",
              lastError: null,
            });
          } else {
            store.getState().setChannel("runtime_ack", {
              transport: "ws",
              lastError: verdict.lastError,
            });
          }
        },
      );
      s.setChannel("routes", { transport: "ws", status: "live" });
    });

    // ROOM-AUTH-01 (2026-09-17): the server ALSO emits a structured `error`
    // event on a refused join (the non-ack path — e.g. a server older than
    // this fix). Scoped strictly to room "runtime_ack"; every other error
    // emitter on the shared socket is not this channel's business.
    socket.on("error", (raw: unknown) => {
      const lastError = runtimeAckRoomError(raw);
      if (lastError !== null) {
        store.getState().setChannel("runtime_ack", { lastError });
      }
    });

    socket.on("route_discovery_telemetry", (raw: unknown) => {
      const verdict = acceptTickPayload(raw);
      if (verdict.ok) {
        store.getState().setTick(verdict.tick);
        markFresh("routes");
      } else if (verdict.kind === "rejected") {
        // Connection is fine; the PAYLOAD drifted. Honest error, no setTick.
        store.getState().setChannel("routes", { lastError: verdict.reason });
      }
      // kind === "ignored": the room's other four event types — normal.
    });

    socket.on("runtime_ack", (raw: unknown) => {
      const verdict = acceptRuntimeAck(raw);
      if (verdict.ok) {
        store.getState().recordAck(verdict.ack);
        markFresh("runtime_ack");
      } else {
        store.getState().setChannel("runtime_ack", { lastError: verdict.reason });
      }
    });

    socket.on("disconnect", () => {
      wsConnectedRef.current = false;
      const s = store.getState();
      s.setWsConnected(false);
      // Routes falls back to the REST loop above (`polling`); runtime_ack is
      // a passive recorder with no REST fallback — it goes dark honestly.
      s.setChannel("routes", { transport: "rest", status: "polling" });
      s.setChannel("runtime_ack", { transport: "rest", status: "disconnected" });
    });

    return () => {
      clearInterval(restTimer);
      clearInterval(staleTimer);
      wsConnectedRef.current = false;
      socket.disconnect();
      const s = store.getState();
      s.setWsConnected(false);
      for (const id of ["routes", "runtime_ack", "pairs", "quote_anchor"] as const) {
        s.setChannel(id, { status: "disconnected" });
      }
    };
  }, []);

  return <>{children ?? null}</>;
}
