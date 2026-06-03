"use client";
// frontend/lib/hooks/useRouteDiscoveryRest.ts
//
// QUANTUM FULLSTACK SYMMETRY — REST polling for route-discovery + cartridge
// telemetry. This is the RELIABLE in-browser data path: the api-server exposes
// public read-only snapshots (fed by its Redis pub/sub cache) at
// /api/route-discovery/* and /api/cartridges/*, reached via the edge
// (NEXT_PUBLIC_EDGE_URL). Unlike the WS hooks, this needs no admin token in the
// browser (the real token is an httpOnly cookie that JS cannot read), so the
// panels show live data without an admin session.
//
// Mirrors the project's established polling pattern (StatusClient/OperationsClient):
// 5s interval, per-request timeout, LIVE/STALE status, no retry storm, cleanup
// on unmount. R8 fail-honest: ok=false / fetch failure → STALE, never fabricate.

import { useEffect, useState } from "react";
import { getApiBaseUrl } from "@/lib/api-client";
import type {
  MergedRoute,
  RouteDiscoveryTick,
  RouteDiscoveryEvent,
  TelemetryStatus,
} from "./useRouteDiscoveryTelemetry";
import type { CartridgeTelemetry } from "./useCartridgeTelemetry";

const POLL_MS = 5000;
const FETCH_TIMEOUT_MS = 8000;

async function getJson(url: string): Promise<unknown | null> {
  const ctrl = new AbortController();
  const t = setTimeout(() => ctrl.abort(), FETCH_TIMEOUT_MS);
  try {
    const res = await fetch(url, { credentials: "include", signal: ctrl.signal });
    if (!res.ok) return null;
    return await res.json();
  } catch {
    return null;
  } finally {
    clearTimeout(t);
  }
}

export interface UseRouteDiscoveryRestResult {
  lastTick: RouteDiscoveryTick | null;
  routes: MergedRoute[];
  recent: RouteDiscoveryEvent[];
  status: TelemetryStatus;
  updatedAt: number | null;
}

export function useRouteDiscoveryRest(): UseRouteDiscoveryRestResult {
  const [lastTick, setLastTick] = useState<RouteDiscoveryTick | null>(null);
  const [routes, setRoutes] = useState<MergedRoute[]>([]);
  const [recent, setRecent] = useState<RouteDiscoveryEvent[]>([]);
  const [status, setStatus] = useState<TelemetryStatus>("CONNECTING");
  const [updatedAt, setUpdatedAt] = useState<number | null>(null);

  useEffect(() => {
    if (typeof window === "undefined") return;
    let alive = true;
    const base = getApiBaseUrl();

    const poll = async () => {
      const [latest, routesRes] = await Promise.all([
        getJson(`${base}/api/route-discovery/latest`),
        getJson(`${base}/api/route-discovery/routes?limit=100`),
      ]);
      if (!alive) return;

      const l = latest as { ok?: boolean; data?: { tick?: RouteDiscoveryTick; recent?: RouteDiscoveryEvent[] } } | null;
      if (l && l.ok && l.data) {
        setLastTick(l.data.tick ?? null);
        setRecent(Array.isArray(l.data.recent) ? l.data.recent : []);
        setStatus("LIVE");
        setUpdatedAt(Date.now());
      } else {
        setStatus("STALE");
      }

      const r = routesRes as { ok?: boolean; data?: { routes?: MergedRoute[] } } | null;
      if (r && r.ok && Array.isArray(r.data?.routes)) {
        setRoutes(r.data.routes);
      }
    };

    void poll();
    const id = setInterval(() => void poll(), POLL_MS);
    return () => {
      alive = false;
      clearInterval(id);
    };
  }, []);

  return { lastTick, routes, recent, status, updatedAt };
}

export interface UseCartridgeRestResult {
  messages: CartridgeTelemetry[];
  status: TelemetryStatus;
  updatedAt: number | null;
}

export function useCartridgeRest(): UseCartridgeRestResult {
  const [messages, setMessages] = useState<CartridgeTelemetry[]>([]);
  const [status, setStatus] = useState<TelemetryStatus>("CONNECTING");
  const [updatedAt, setUpdatedAt] = useState<number | null>(null);

  useEffect(() => {
    if (typeof window === "undefined") return;
    let alive = true;
    const base = getApiBaseUrl();

    const poll = async () => {
      const res = await getJson(`${base}/api/cartridges/telemetry/latest?limit=60`);
      if (!alive) return;
      const r = res as { ok?: boolean; data?: { messages?: CartridgeTelemetry[] } } | null;
      if (r && r.ok && Array.isArray(r.data?.messages)) {
        // REST returns newest-first; reverse to oldest-first to match the
        // panel's ring-buffer assumption (it slices the tail + reverses).
        setMessages([...r.data.messages].reverse());
        setStatus("LIVE");
        setUpdatedAt(Date.now());
      } else {
        setStatus("STALE");
      }
    };

    void poll();
    const id = setInterval(() => void poll(), POLL_MS);
    return () => {
      alive = false;
      clearInterval(id);
    };
  }, []);

  return { messages, status, updatedAt };
}
