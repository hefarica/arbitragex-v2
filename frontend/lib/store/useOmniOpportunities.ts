/**
 * =============================================================================
 * OMEGA OMNI-OPPORTUNITIES HOOK — Store-Integrated WebSocket Stream
 * =============================================================================
 *
 * This hook bridges the WebSocket stream to the Omni-Store.
 * It replaces the fragment pattern of useState + useOpportunitiesStream.
 *
 * Architecture:
 *   - WebSocket → addOpportunity (store action)
 *   - Component → useOpportunities() (store selector)
 *
 * R8 Fail-Honest: Status surfaces errors; never fabricates data.
 */

"use client";

import { useEffect, useRef, useCallback, startTransition } from "react";
import { io } from "socket.io-client";
import { createOpportunitySocket, type WsStatus } from "@/features/opportunities/socket-lifecycle";
import { getAdminToken } from "@/lib/admin-token";
import { getApiBaseUrl, getWsBaseUrl } from "@/lib/api-client";
import { useOmniStore } from "./omni-store";
import { mapToOmniOpportunity, type OmniOpportunity } from "./types";

// =============================================================================
// Constants
// =============================================================================

const MAX_WS_ERRORS = 3;
const POLL_INTERVAL_MS = 5000;
const MAX_ITEMS = 200;

// =============================================================================
// Types
// =============================================================================

interface UseOmniOpportunitiesOptions {
  viableOnly?: boolean;
  initialOpportunities?: OmniOpportunity[];
}

// =============================================================================
// Hook Implementation
// =============================================================================

/**
 * Hook that connects WebSocket stream to Omni-Store.
 * 
 * Usage:
 * ```tsx
 * function MyComponent() {
 *   // Connect the stream
 *   useOmniOpportunities({ viableOnly: false });
 *   
 *   // Read from store (selector prevents unnecessary re-renders)
 *   const opportunities = useOmniStore((state) => state.opportunities);
 *   const wsStatus = useOmniStore((state) => state.wsStatus);
 * }
 * ```
 */
export function useOmniOpportunities({
  viableOnly = false,
  initialOpportunities = [],
}: UseOmniOpportunitiesOptions) {
  // Store actions (stable references)
  const addOpportunity = useOmniStore((state) => state.addOpportunity);
  const setOpportunities = useOmniStore((state) => state.setOpportunities);
  const setWsStatus = useOmniStore((state) => state.setWsStatus);

  // Refs for stable closure access
  const errorCountRef = useRef(0);
  const usingPollingRef = useRef(false);
  const pollingTimerRef = useRef<ReturnType<typeof setInterval> | null>(null);
  const viableOnlyRef = useRef(viableOnly);
  const initializedRef = useRef(false);

  // Keep viableOnly ref in sync
  useEffect(() => {
    viableOnlyRef.current = viableOnly;
  }, [viableOnly]);

  // Initialize store with initial opportunities (once)
  useEffect(() => {
    if (initializedRef.current) return;
    initializedRef.current = true;

    if (initialOpportunities.length > 0) {
      // PERF: one batch update, not clear + N addOpportunity calls.
      setOpportunities(initialOpportunities);
    }
  }, [initialOpportunities, setOpportunities]);

  // HTTP polling fallback
  const startPolling = useCallback(() => {
    if (usingPollingRef.current) return;
    usingPollingRef.current = true;
    setWsStatus("POLLING");

    const poll = async () => {
      try {
        const viable = viableOnlyRef.current;
        const res = await fetch(
          `${getApiBaseUrl()}/api/opportunities/live?viable_only=${viable}&limit=50`,
          {
            headers: { accept: "application/json" },
            signal: AbortSignal.timeout(POLL_INTERVAL_MS),
            cache: "no-store",
          },
        );
        if (!res.ok) return;
        const data: unknown = await res.json();
        const rawItems: unknown[] = Array.isArray((data as { items?: unknown }).items)
          ? ((data as { items: unknown[] }).items)
          : Array.isArray(data)
          ? (data as unknown[])
          : [];

        // PERF: one batch replacement instead of clear + N addOpportunity calls.
        // Each addOpportunity triggered a separate Zustand update + devtools
        // serialization; with 50 items every 4-5s that caused severe memory churn.
        setOpportunities(rawItems.map((raw) => mapToOmniOpportunity(raw as Record<string, unknown>)));
      } catch {
        // Swallow — status badge already shows "POLLING" (degraded)
      }
    };

    poll();
    pollingTimerRef.current = setInterval(poll, POLL_INTERVAL_MS);
  }, [setWsStatus, setOpportunities]);

  // WebSocket lifecycle
  useEffect(() => {
    if (typeof window === "undefined") return;

    // R8 Fail-Honest: No hardcoded fallback to localhost.
    // Must be provided via NEXT_PUBLIC_WS_URL.
    const wsUrl = getWsBaseUrl();
    if (!wsUrl) {
      console.warn("[WS] NEXT_PUBLIC_WS_URL is undefined. WebSocket disabled.");
      setWsStatus("STALE");
      startPolling();
      return;
    }

    // WS-POLL-1 (2026-08-20): the opportunities room is PUBLIC by design — the
    // api-server gateway accepts anonymous connections (io.use only FLAGS the
    // admin capability for the runtime_ack room; it never rejects). The old
    // hard bail here forced every admin-session-less visitor straight into
    // STALE+polling even with a perfectly healthy server ("FEED POLLING"
    // chip). Attach the admin token when the session has one (keeps the C4
    // capability path); connect anonymously otherwise and let the transport
    // itself decide (3-error degrade below still applies).
    const adminToken = getAdminToken() ?? undefined;

    const handle = createOpportunitySocket({
      url: wsUrl,
      ioFactory: (url, opts) => io(url, opts),
      authToken: adminToken,
      onStatus: (status: WsStatus) => {
        if (usingPollingRef.current) return;

        setWsStatus(status);

        if (status === "STALE") {
          errorCountRef.current += 1;
          if (errorCountRef.current >= MAX_WS_ERRORS) {
            handle.dispose();
            startPolling();
          }
        } else if (status === "LIVE") {
          errorCountRef.current = 0;
        }
      },
      onOpportunity: (opp) => {
        // Transform raw WS data through mapper before storing
        addOpportunity(mapToOmniOpportunity(opp as unknown as Record<string, unknown>));
      },
    });

    return () => {
      handle.dispose();
      if (pollingTimerRef.current !== null) {
        clearInterval(pollingTimerRef.current);
        pollingTimerRef.current = null;
      }
      // R8 fail-honest: once this stream consumer unmounts the live socket is
      // gone, so reset the SSOT status. Keeps the global header indicator
      // truthful (reads IDLE) instead of a stale LIVE/STALE on other pages.
      usingPollingRef.current = false;
      setWsStatus("DISCONNECTED");
    };
  }, [startPolling, setWsStatus, addOpportunity]);

  // Return nothing — consumers read directly from store
  // This enforces SSOT pattern
}
