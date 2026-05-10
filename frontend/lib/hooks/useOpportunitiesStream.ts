"use client";
// frontend/lib/hooks/useOpportunitiesStream.ts
//
// FE-1: WebSocket reactivation for the opportunities feed.
//
// Architecture:
//   - Wraps createOpportunitySocket (features/opportunities/socket-lifecycle.ts)
//     which is already tested against the api-server event contract:
//       server emits  → "new_opportunity"
//       client emits  → "subscribe:opportunities" (on connect)
//   - R1 compliant: all WS and DOM access is inside useEffect; initial render
//     is pure useState(initial) — identical to the SSR snapshot.
//   - Fallback: after MAX_WS_ERRORS consecutive connect_error events the hook
//     silently reactivates HTTP polling at POLL_INTERVAL_MS.
//   - R8 fail-honest: WsStatus "STALE" is surfaced; the caller decides how to
//     communicate the gap to the operator. This hook NEVER hides it.

import { useEffect, useRef, useState, useCallback, startTransition } from "react";
import { io } from "socket.io-client";
import {
  createOpportunitySocket,
  type WsStatus,
} from "@/features/opportunities/socket-lifecycle";

// ─── Canonical type mirror ────────────────────────────────────────────────────
// Kept here so useOpportunitiesStream does not import from OpportunitiesClient.
// Must stay in sync with OpportunityListItemSchema in shared-ts.
export interface TokenInfo {
  symbol: string | null;
  decimals: number | null;
  logo_url: string | null;
  resolved_via: "onchain_full" | "onchain_partial" | "trustwallet_only" | "failed";
}

export type StrategyKind =
  | "dex_arb"
  | "triangular"
  | "backrun"
  | "liquidation"
  | "flashloan_arb";

export type OpportunityStatus =
  | "detected"
  | "validated"
  | "simulated"
  | "scored"
  | "executing"
  | "executed"
  | "reconciled"
  | "rejected"
  | "failed";

export interface OpportunityListItem {
  id: string;
  chain_id: number;
  strategy_kind: StrategyKind;
  dex_a: string;
  dex_b: string | null;
  pair_symbol: string | null;
  token_in: string;
  token_in_info: TokenInfo | null;
  token_out: string;
  token_out_info: TokenInfo | null;
  amount_in_wei: string;
  expected_profit_usd: number | null;
  roi_pct: number | null;
  risk_score: number | null;
  block_number: number | null;
  rejection_reason: string | null;
  status: OpportunityStatus;
  detected_at: string;
  trace_id: string;
  chain_id_out: number | null;
  bridge: string | null;
  bridge_fee_usd: number | null;
}

// ─── Configuration constants ──────────────────────────────────────────────────
const MAX_WS_ERRORS = 3;
const POLL_INTERVAL_MS = 4_000;
const MAX_ITEMS = 200;

// ─── Public API ───────────────────────────────────────────────────────────────
export interface UseOpportunitiesStreamResult {
  opportunities: OpportunityListItem[];
  wsStatus: WsStatus | "POLLING";
}

export function useOpportunitiesStream(
  initial: OpportunityListItem[],
  edgeUrl: string,
): UseOpportunitiesStreamResult {
  const [opportunities, setOpportunities] = useState<OpportunityListItem[]>(initial);
  const [wsStatus, setWsStatus] = useState<WsStatus | "POLLING">("CONNECTING");

  // Stable refs — never cause re-renders, safe to read in closures.
  const errorCountRef = useRef(0);
  const usingPollingRef = useRef(false);
  const pollingTimerRef = useRef<ReturnType<typeof setInterval> | null>(null);

  // ─── HTTP polling fallback ────────────────────────────────────────────────
  const startPolling = useCallback(() => {
    if (usingPollingRef.current) return;
    usingPollingRef.current = true;
    setWsStatus("POLLING");

    const poll = async () => {
      try {
        const res = await fetch(
          `${edgeUrl}/api/opportunities/live?viable_only=true&limit=50`,
          {
            headers: { accept: "application/json" },
            signal: AbortSignal.timeout(POLL_INTERVAL_MS),
            cache: "no-store",
          },
        );
        if (!res.ok) return;
        const data: unknown = await res.json();
        const items: OpportunityListItem[] = Array.isArray(
          (data as { items?: unknown }).items,
        )
          ? ((data as { items: OpportunityListItem[] }).items)
          : Array.isArray(data)
          ? (data as OpportunityListItem[])
          : [];
        startTransition(() => setOpportunities(items));
      } catch {
        // Swallow — R8: status badge already shows "POLLING" (degraded).
      }
    };

    poll();
    pollingTimerRef.current = setInterval(poll, POLL_INTERVAL_MS);
  }, [edgeUrl]);

  // ─── Opportunity dedup + prepend ──────────────────────────────────────────
  const addOpportunity = useCallback((opp: OpportunityListItem) => {
    startTransition(() => {
      setOpportunities((prev) => {
        if (prev.some((p) => p.id === opp.id)) return prev;
        return [opp, ...prev].slice(0, MAX_ITEMS);
      });
    });
  }, []);

  // ─── WebSocket lifecycle (R1: entire body runs after mount) ──────────────
  useEffect(() => {
    if (typeof window === "undefined") return;

    const wsUrl = process.env.NEXT_PUBLIC_WS_URL ?? "http://localhost:3000";

    const handle = createOpportunitySocket({
      url: wsUrl,
      ioFactory: (url, opts) => io(url, opts),
      onStatus: (status: WsStatus) => {
        if (usingPollingRef.current) return; // already degraded — ignore WS status noise

        setWsStatus(status);

        if (status === "STALE") {
          // A connect_error also triggers "STALE" via socket-lifecycle.
          errorCountRef.current += 1;
          if (errorCountRef.current >= MAX_WS_ERRORS) {
            handle.dispose();
            startPolling();
          }
        } else if (status === "LIVE") {
          // Connection recovered — reset error counter.
          errorCountRef.current = 0;
        }
      },
      onOpportunity: (opp) => {
        // socket-lifecycle's Opportunity is a minimal typing subset.
        // The api-server actually emits a full OpportunityListItem payload
        // (PostgreSQL NOTIFY JSON). Safe to cast via unknown.
        addOpportunity(opp as unknown as OpportunityListItem);
      },
    });

    return () => {
      handle.dispose();
      if (pollingTimerRef.current !== null) {
        clearInterval(pollingTimerRef.current);
        pollingTimerRef.current = null;
      }
    };
    // edgeUrl and startPolling are stable (startPolling is useCallback with edgeUrl dep).
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  return { opportunities, wsStatus };
}
