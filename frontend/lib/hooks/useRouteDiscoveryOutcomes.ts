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

// RDO-SUMMARY-HANG (2026-08-31): this is aggregate analytics over the durable
// outcomes table (not a live tick wire), and the upstream aggregation over a
// 26M+ row window costs seconds — an 8s poll re-fired it before the previous
// call finished. 60s matches the analytics cadence; the edge serves a 15s
// cached copy in between.
const POLL_MS = 60000;
const FETCH_TIMEOUT_MS = 65000;

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

/** FE-0038 (§47): by-strategy grouping — cartridge_id is the strategy key the sink persists. */
export interface OutcomeCartridgeRow {
  cartridge_id: string;
  n: number;
  opportunities: number;
}

/** FE-0038 (§47): by-pair grouping — raw token addresses, verbatim. */
export interface OutcomePairRow {
  token_in: string;
  token_out: string;
  n: number;
  opportunities: number;
}

export interface UseRouteDiscoveryOutcomesResult {
  totals: OutcomeTotals | null;
  byReason: OutcomeReasonRow[];
  byChain: OutcomeChainRow[];
  byCartridge: OutcomeCartridgeRow[];
  byPair: OutcomePairRow[];
  /**
   * FE-0038 §47: true when the response actually CARRIED the grouping keys.
   * Distinguishes "served but zero rows in window" (false honesty about the
   * window) from "api-server older than the FE-0038 deploy" — same []
   * payload, different truth (R8).
   */
  groupingsServed: boolean;
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
    // FE-0038 §47 — absent on an api-server older than the grouping deploy:
    // absence is a real state, parsed to [] (never a fabricated default row).
    by_cartridge?: unknown;
    by_pair?: unknown;
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
  const [byCartridge, setByCartridge] = useState<OutcomeCartridgeRow[]>([]);
  const [byPair, setByPair] = useState<OutcomePairRow[]>([]);
  const [groupingsServed, setGroupingsServed] = useState(false);
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
        // FE-0038 §47 groupings — same tolerant parse as by_chain.
        setGroupingsServed(d.by_cartridge !== undefined || d.by_pair !== undefined);
        const rawCartridges = Array.isArray(d.by_cartridge)
          ? (d.by_cartridge as Array<Record<string, unknown>>)
          : [];
        setByCartridge(
          rawCartridges.map((r) => ({
            cartridge_id:
              typeof r["cartridge_id"] === "string" && r["cartridge_id"] !== ""
                ? (r["cartridge_id"] as string)
                : "(null)",
            n: toNum(r["n"]) ?? 0,
            opportunities: toNum(r["opportunities"]) ?? 0,
          })),
        );
        const rawPairs = Array.isArray(d.by_pair)
          ? (d.by_pair as Array<Record<string, unknown>>)
          : [];
        setByPair(
          rawPairs.map((r) => ({
            token_in: typeof r["token_in"] === "string" ? (r["token_in"] as string) : "—",
            token_out: typeof r["token_out"] === "string" ? (r["token_out"] as string) : "—",
            n: toNum(r["n"]) ?? 0,
            opportunities: toNum(r["opportunities"]) ?? 0,
          })),
        );
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

  return {
    totals,
    byReason,
    byChain,
    byCartridge,
    byPair,
    groupingsServed,
    windowHours,
    status,
    updatedAt,
    unavailableReason,
  };
}
