"use client";
// frontend/lib/hooks/useRouteDiscoveryTelemetry.ts
//
// QUANTUM FULLSTACK SYMMETRY — live OMEGA Route Discovery radar telemetry.
//
// Architecture (mirrors useCartridgeTelemetry — RULE 02: WebSocket DIRECT to the
// api-server, NEVER via the edge):
//   - Connects to Socket.IO via NEXT_PUBLIC_WS_URL (api-server :8080)
//   - Client emits "subscribe:route_discovery" on connect
//   - Server emits "route_discovery_telemetry" for every message the Rust radar
//     publishes to the Redis channel `arbx:route_discovery:telemetry`
//   - Admin-token gate on the WS handshake (same as every other room)
//   - R8 fail-honest: status "STALE" when disconnected; never fabricates data
//
// The 5 event shapes share only the `event` discriminator (observational). The
// hook keeps a bounded ring buffer of raw events and derives: the last tick, and
// a merged route table (candidate + applicability + intent, keyed by route_hash).

import { useEffect, useState, useRef, useCallback, useMemo, startTransition } from "react";
import { io, type Socket } from "socket.io-client";
import { getAdminToken } from "@/lib/admin-token";

// ─── Type mirror (telemetry.rs event builders) ─────────────────────────────────
export interface RouteDiscoveryTick {
  event: "route_discovery.tick";
  algorithm?: string;
  pools_total?: number;
  edges_built?: number;
  edges_rejected?: number;
  routes_found?: number;
  routes_dispatched?: number;
  routes_dropped_for_cap?: number;
  routes_capped?: boolean;
  telemetry_emitted?: number;
  latency_ms?: number;
  mode?: string;
  [k: string]: unknown;
}
export interface RouteCandidateEvent {
  event: "route_discovery.route_candidate";
  route_hash?: string;
  route_kind?: string;
  hops?: number;
  tokens?: string[];
  pools?: string[];
  protocols?: string[];
  fee_tiers?: (number | null)[];
  directions?: string[];
  algorithm?: string;
  [k: string]: unknown;
}
export interface StrategyApplicabilityEvent {
  event: "route_discovery.strategy_applicability";
  route_hash?: string;
  route_kind?: string;
  applicable_strategies?: string[];
  rejected_strategies?: { strategy: string; reason: string }[];
  [k: string]: unknown;
}
export interface RejectedEvent {
  event: "route_discovery.rejected";
  pool?: string;
  reason?: string;
  [k: string]: unknown;
}
export interface RouteIntentEmittedEvent {
  event: "route_intent.emitted";
  route_hash?: string;
  strategy?: string;
  mode?: string;
  dispatch_deferred?: string | null;
  [k: string]: unknown;
}
export type RouteDiscoveryEvent =
  | RouteDiscoveryTick
  | RouteCandidateEvent
  | StrategyApplicabilityEvent
  | RejectedEvent
  | RouteIntentEmittedEvent
  | { event?: string;[k: string]: unknown };

/** A route row merged from candidate + applicability + intent events. */
export interface MergedRoute {
  route_hash: string;
  route_kind?: string;
  hops?: number;
  tokens?: string[];
  pools?: string[];
  protocols?: string[];
  fee_tiers?: (number | null)[];
  directions?: string[];
  applicable_strategies?: string[];
  rejected_strategies?: { strategy: string; reason: string }[];
  dispatch_strategy?: string;
  dispatch_deferred?: string | null;
}

export type TelemetryStatus = "CONNECTING" | "LIVE" | "STALE";

export interface UseRouteDiscoveryTelemetryResult {
  /** Raw rolling buffer (oldest first, newest last). */
  messages: RouteDiscoveryEvent[];
  /** Most recent tick summary, or null before any arrives. */
  lastTick: RouteDiscoveryTick | null;
  /** Recent unique routes (newest first), merged from candidate/applicability/intent. */
  routes: MergedRoute[];
  status: TelemetryStatus;
  clear: () => void;
}

const MAX_WS_ERRORS = 3;
// One radar tick floods ~400+ events (candidate+applicability per route, then
// rejected + dispatch intents). The buffer must hold MORE than one tick so a
// route's candidate (emitted first) is still present to merge with its later
// applicability/intent events — otherwise the routes table loses topology.
const MAX_BUFFER = 1500;
const MAX_ROUTES = 100;

function asString(v: unknown): string | undefined {
  return typeof v === "string" ? v : undefined;
}

export function useRouteDiscoveryTelemetry(): UseRouteDiscoveryTelemetryResult {
  const [messages, setMessages] = useState<RouteDiscoveryEvent[]>([]);
  const [status, setStatus] = useState<TelemetryStatus>("CONNECTING");
  const errorCountRef = useRef(0);

  const clear = useCallback(() => setMessages([]), []);

  useEffect(() => {
    if (typeof window === "undefined") return;

    const wsUrl = process.env.NEXT_PUBLIC_WS_URL ?? "http://localhost:3000";

    const adminToken = getAdminToken();
    if (!adminToken) {
      setStatus("STALE");
      return;
    }

    const socket: Socket = io(wsUrl, {
      auth: { token: adminToken },
      transports: ["websocket", "polling"],
    });

    socket.on("connect", () => {
      setStatus("LIVE");
      errorCountRef.current = 0;
      socket.emit("subscribe:route_discovery");
    });

    socket.on("route_discovery_telemetry", (data: RouteDiscoveryEvent) => {
      startTransition(() => {
        setMessages((prev) => {
          const base =
            prev.length >= MAX_BUFFER ? prev.slice(prev.length - MAX_BUFFER + 1) : prev;
          return [...base, data];
        });
      });
    });

    socket.on("connect_error", () => {
      errorCountRef.current += 1;
      if (errorCountRef.current >= MAX_WS_ERRORS) {
        setStatus("STALE");
        socket.disconnect();
      }
    });

    socket.on("disconnect", () => {
      setStatus("STALE");
    });

    return () => {
      socket.disconnect();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const lastTick = useMemo<RouteDiscoveryTick | null>(() => {
    for (let i = messages.length - 1; i >= 0; i--) {
      if (messages[i]?.event === "route_discovery.tick") {
        return messages[i] as RouteDiscoveryTick;
      }
    }
    return null;
  }, [messages]);

  const routes = useMemo<MergedRoute[]>(() => {
    // Merge candidate + applicability + intent by route_hash; most-recently-seen last.
    const map = new Map<string, MergedRoute>();
    for (const ev of messages) {
      const hash = asString((ev as { route_hash?: unknown }).route_hash);
      if (!hash) continue;
      const t = ev.event;
      if (
        t !== "route_discovery.route_candidate" &&
        t !== "route_discovery.strategy_applicability" &&
        t !== "route_intent.emitted"
      ) {
        continue;
      }
      const existing = map.get(hash);
      // Re-insert to move to the end (recency ordering).
      if (existing) map.delete(hash);
      const merged: MergedRoute = existing ?? { route_hash: hash };
      if (t === "route_discovery.route_candidate") {
        const c = ev as RouteCandidateEvent;
        merged.route_kind = c.route_kind ?? merged.route_kind;
        merged.hops = c.hops ?? merged.hops;
        merged.tokens = c.tokens ?? merged.tokens;
        merged.pools = c.pools ?? merged.pools;
        merged.protocols = c.protocols ?? merged.protocols;
        merged.fee_tiers = c.fee_tiers ?? merged.fee_tiers;
        merged.directions = c.directions ?? merged.directions;
      } else if (t === "route_discovery.strategy_applicability") {
        const a = ev as StrategyApplicabilityEvent;
        merged.route_kind = a.route_kind ?? merged.route_kind;
        merged.applicable_strategies = a.applicable_strategies ?? merged.applicable_strategies;
        merged.rejected_strategies = a.rejected_strategies ?? merged.rejected_strategies;
      } else if (t === "route_intent.emitted") {
        const ri = ev as RouteIntentEmittedEvent;
        merged.dispatch_strategy = ri.strategy ?? merged.dispatch_strategy;
        merged.dispatch_deferred = ri.dispatch_deferred ?? merged.dispatch_deferred ?? null;
      }
      map.set(hash, merged);
    }
    const all = Array.from(map.values());
    return all.slice(-MAX_ROUTES).reverse();
  }, [messages]);

  return { messages, lastTick, routes, status, clear };
}
