"use client";
// frontend/lib/hooks/useRouteDiscoveryOutcomes.ts
//
// FASE B Gate-C — REST polling for the route-discovery OUTCOMES analytics.
// This is the read-side of the passive sink `route_discovery_outcomes`: the
// shadow emitter writes resolved outcomes (with the Paso 9 `reason` column),
// api-server aggregates them at /api/v1/route-discovery-outcomes/summary, and
// the edge re-exposes it at /api/route-discovery-outcomes/summary (public read,
// no admin token). The panel renders the hit-rate + the reason distribution —
// the honest "why 0% opportunities".
//
// Mirrors useRouteDiscoveryRest: configurable poll interval, per-request
// timeout, LIVE/STALE status, cleanup on unmount. R8 fail-honest:
//   - api-server returns 503 {reason} when Postgres is unavailable → STALE +
//     `unavailableReason` surfaced verbatim (db_unavailable / query_failed).
//   - count(*)::bigint comes off node-postgres as STRINGS; ::int as numbers.
//     toNum() normalizes both and returns null for anything unparseable —
//     never fabricates a count.

import { useEffect, useState } from "react";
import { getApiBaseUrl } from "@/lib/api-client";

const POLL_MS = 8000;
const FETCH_TIMEOUT_MS = 9000;

export type OutcomesStatus = "CONNECTING" | "LIVE" | "STALE";

export interface OutcomeTotals {
  total: number | null;
  opportunities: number | null;
  with_reserves: number | null;
  profit_gt0: number | null;
  chains: number | null;
  cartridges: number | null;
}

export interface OutcomeReasonRow {
  reason: string;
  n: number;
}

export interface OutcomeChainRow {
  chain_id: number | null;
  n: number;
  opportunities: number;
}

export interface UseRouteDiscoveryOutcomesResult {
  totals: OutcomeTotals | null;
  byReason: OutcomeReasonRow[];
  byChain: OutcomeChainRow[];
  windowHours: number | null;
  status: OutcomesStatus;
  updatedAt: number | null;
  /** Verbatim upstream 503 reason (R8) when the series is unavailable. */
  unavailableReason: string | null;
}

type RawSummary = {
  ok?: boolean;
  window_hours?: unknown;
  reason?: unknown;
  data?: {
    totals?: Record<string, unknown> | null;
    by_reason?: unknown;
    by_chain?: unknown;
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

export function useRouteDiscoveryOutcomes(hours: number): UseRouteDiscoveryOutcomesResult {
  const [totals, setTotals] = useState<OutcomeTotals | null>(null);
  const [byReason, setByReason] = useState<OutcomeReasonRow[]>([]);
  const [byChain, setByChain] = useState<OutcomeChainRow[]>([]);
  const [windowHours, setWindowHours] = useState<number | null>(null);
  const [status, setStatus] = useState<OutcomesStatus>("CONNECTING");
  const [updatedAt, setUpdatedAt] = useState<number | null>(null);
  const [unavailableReason, setUnavailableReason] = useState<string | null>(null);

  useEffect(() => {
    if (typeof window === "undefined") return;
    let alive = true;
    const base = getApiBaseUrl();

    const poll = async () => {
      const ctrl = new AbortController();
      const t = setTimeout(() => ctrl.abort(), FETCH_TIMEOUT_MS);
      let json: RawSummary = null;
      let httpReason: string | null = null;
      try {
        const res = await fetch(`${base}/api/route-discovery-outcomes/summary?hours=${hours}`, {
          credentials: "include",
          signal: ctrl.signal,
        });
        if (res.ok) {
          json = (await res.json()) as RawSummary;
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
                total: toNum(tt["total"]),
                opportunities: toNum(tt["opportunities"]),
                with_reserves: toNum(tt["with_reserves"]),
                profit_gt0: toNum(tt["profit_gt0"]),
                chains: toNum(tt["chains"]),
                cartridges: toNum(tt["cartridges"]),
              }
            : null,
        );
        const rawReasons = Array.isArray(d.by_reason) ? (d.by_reason as Array<Record<string, unknown>>) : [];
        setByReason(
          rawReasons.map((r) => ({
            reason: typeof r["reason"] === "string" && r["reason"] !== "" ? (r["reason"] as string) : "(unknown)",
            n: toNum(r["n"]) ?? 0,
          })),
        );
        const rawChains = Array.isArray(d.by_chain) ? (d.by_chain as Array<Record<string, unknown>>) : [];
        setByChain(
          rawChains.map((r) => ({
            chain_id: toNum(r["chain_id"]),
            n: toNum(r["n"]) ?? 0,
            opportunities: toNum(r["opportunities"]) ?? 0,
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

  return { totals, byReason, byChain, windowHours, status, updatedAt, unavailableReason };
}
