"use client";
// frontend/lib/hooks/useViableKpis.ts
//
// XLS-DASH-01 (workbook 29_SUPER_DASHBOARD) — REST polling for the viable-KPI
// set: viable opportunities grouped by ROUTE HOPS (from route_metadata
// pool_addresses) and by strategy_kind, plus the window viability %. Served by
// api-server /api/v1/analytics/viable-kpis over REAL opportunities rows and
// re-exposed read-only by the edge at /api/viable-kpis.
//
// Mirrors useRouteDiscoveryOutcomes: configurable poll interval, per-request
// timeout, LIVE/STALE status, cleanup on unmount. R8 fail-honest:
//   - 503 {reason} surfaces verbatim in `unavailableReason` (db_unavailable /
//     query_failed), never masked as empty-but-ok.
//   - viability_pct is null when nothing was observed — rendered as "—",
//     never a fabricated 0%.

import { useEffect, useState } from "react";
import { getApiBaseUrl } from "@/lib/api-client";

const POLL_MS = 10000;
const FETCH_TIMEOUT_MS = 9000;

export type ViableKpisStatus = "CONNECTING" | "LIVE" | "STALE";

export interface ViableTotals {
  viable: number | null;
  routed: number | null;
  total: number | null;
  viability_pct: number | null;
}

export interface ViableByHopsRow {
  hops: number;
  n: number;
}

export interface ViableByKindRow {
  strategy_kind: string;
  n: number;
}

export interface UseViableKpisResult {
  totals: ViableTotals | null;
  byHops: ViableByHopsRow[];
  byKind: ViableByKindRow[];
  windowHours: number | null;
  status: ViableKpisStatus;
  updatedAt: number | null;
  /** Verbatim upstream 503 reason (R8) when the KPI set is unavailable. */
  unavailableReason: string | null;
}

type RawKpis = {
  ok?: boolean;
  window_hours?: unknown;
  reason?: unknown;
  data?: {
    totals?: Record<string, unknown> | null;
    by_hops?: unknown;
    by_kind?: unknown;
  } | null;
} | null;

function toNum(v: unknown): number | null {
  if (typeof v === "number") return Number.isFinite(v) ? v : null;
  if (typeof v === "string" && v.trim() !== "") {
    const n = Number(v);
    return Number.isFinite(n) ? n : null;
  }
  return null;
}

export function useViableKpis(hours: number): UseViableKpisResult {
  const [totals, setTotals] = useState<ViableTotals | null>(null);
  const [byHops, setByHops] = useState<ViableByHopsRow[]>([]);
  const [byKind, setByKind] = useState<ViableByKindRow[]>([]);
  const [windowHours, setWindowHours] = useState<number | null>(null);
  const [status, setStatus] = useState<ViableKpisStatus>("CONNECTING");
  const [updatedAt, setUpdatedAt] = useState<number | null>(null);
  const [unavailableReason, setUnavailableReason] = useState<string | null>(null);

  useEffect(() => {
    if (typeof window === "undefined") return;
    let alive = true;
    const base = getApiBaseUrl();

    const poll = async () => {
      const ctrl = new AbortController();
      const t = setTimeout(() => ctrl.abort(), FETCH_TIMEOUT_MS);
      let json: RawKpis = null;
      let httpReason: string | null = null;
      try {
        const res = await fetch(`${base}/api/viable-kpis?hours=${hours}`, {
          credentials: "include",
          signal: ctrl.signal,
        });
        if (res.ok) {
          json = (await res.json()) as RawKpis;
        } else {
          // R8: surface the api-server 503 reason verbatim — never mask as "ok".
          try {
            const b = (await res.json()) as { reason?: unknown };
            httpReason = typeof b?.reason === "string" ? b.reason : `http_${res.status}`;
          } catch {
            httpReason = `http_${res.status}`;
          }
        }
      } catch {
        httpReason = "edge_unreachable";
      } finally {
        clearTimeout(t);
      }
      if (!alive) return;

      if (json && json.ok === true && json.data) {
        const d = json.data;
        const tt = d.totals ?? null;
        setTotals(
          tt
            ? {
                viable: toNum(tt["viable"]),
                routed: toNum(tt["routed"]),
                total: toNum(tt["total"]),
                viability_pct: toNum(tt["viability_pct"]),
              }
            : null,
        );
        const rawHops = Array.isArray(d.by_hops) ? (d.by_hops as Array<Record<string, unknown>>) : [];
        setByHops(
          rawHops.map((r) => ({
            hops: toNum(r["hops"]) ?? 0,
            n: toNum(r["n"]) ?? 0,
          })),
        );
        const rawKind = Array.isArray(d.by_kind) ? (d.by_kind as Array<Record<string, unknown>>) : [];
        setByKind(
          rawKind.map((r) => ({
            strategy_kind: typeof r["strategy_kind"] === "string" ? (r["strategy_kind"] as string) : "(unknown)",
            n: toNum(r["n"]) ?? 0,
          })),
        );
        setWindowHours(toNum(json.window_hours));
        setUnavailableReason(null);
        setStatus("LIVE");
        setUpdatedAt(Date.now());
      } else {
        setUnavailableReason(httpReason ?? "no_data");
        setStatus("STALE");
      }
    };

    void poll();
    const id = setInterval(() => void poll(), POLL_MS);
    return () => {
      alive = false;
      clearInterval(id);
    };
  }, [hours]);

  return { totals, byHops, byKind, windowHours, status, updatedAt, unavailableReason };
}
