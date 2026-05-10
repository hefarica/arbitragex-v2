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
// C4 fix (audit 2026-05-10): admin token gate on WS handshake.
import { getAdminToken } from "@/lib/admin-token";

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
  // GROSS profit (pre-cost). The dashboard's "Net Profit" column does NOT
  // display this any more — see C5 fix in OpportunitiesClient.
  expected_profit_usd: number | null;
  // C5 fix (audit 2026-05-10): NET profit (gross - gas - slippage - relay
  // fee - flashloan fee - failure buffer). Optional because older payloads
  // (pre-Phase-1 of the wiring fix) emit only gross.
  net_expected_profit_usd?: number | null;
  /** Derived by api-server from `rejection_reason IS NULL`. */
  paper_status?: "paper_viable" | "paper_rejected";
  /** Derived from chain_id (+ chain_id_out for bridge legs). */
  chains_used?: number[];
  /** Derived from dex_a + dex_b. */
  dexes_used?: string[];
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
  viableOnly: boolean = false,
): UseOpportunitiesStreamResult {
  const [opportunities, setOpportunities] = useState<OpportunityListItem[]>(initial);
  const [wsStatus, setWsStatus] = useState<WsStatus | "POLLING">("CONNECTING");

  // Stable refs — never cause re-renders, safe to read in closures.
  const errorCountRef = useRef(0);
  const usingPollingRef = useRef(false);
  const pollingTimerRef = useRef<ReturnType<typeof setInterval> | null>(null);
  const viableOnlyRef = useRef(viableOnly);
  // Keep the ref in sync without restarting the poll loop (the interval
  // closure reads the ref so the next tick honors the latest toggle).
  useEffect(() => { viableOnlyRef.current = viableOnly; }, [viableOnly]);

  // ─── HTTP polling fallback ────────────────────────────────────────────────
  const startPolling = useCallback(() => {
    if (usingPollingRef.current) return;
    usingPollingRef.current = true;
    setWsStatus("POLLING");

    const poll = async () => {
      try {
        // Respect the operator's "Viable only" toggle from the UI. When false,
        // the polling endpoint returns rejected opps too so the operator sees
        // pipeline activity in real time (with rejection_reason visible),
        // not just historical viables.
        const viable = viableOnlyRef.current;
        const res = await fetch(
          `${edgeUrl}/api/opportunities/live?viable_only=${viable}&limit=50`,
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

    // C4 fix (audit 2026-05-10): the backend WS gateway rejects every
    // handshake without a valid admin token (`extractHandshakeToken`).
    // Read the token from the operator's existing admin session and pass
    // it to the socket factory. When no admin session exists, skip the
    // WS connection entirely and degrade to polling immediately —
    // attempting an authless connection just produces noisy 401s.
    const adminToken = getAdminToken();
    if (!adminToken) {
      // No admin session → polling is the only honest option.
      // The dashboard already renders a "WS locked: admin session
      // required" state when wsStatus stays at "STALE" via the badge.
      setWsStatus("STALE");
      startPolling();
      return;
    }

    const handle = createOpportunitySocket({
      url: wsUrl,
      ioFactory: (url, opts) => io(url, opts),
      authToken: adminToken,
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
