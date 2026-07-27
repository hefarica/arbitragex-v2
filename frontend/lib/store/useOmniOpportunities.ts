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
  edgeUrl: string;
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
 *   useOmniOpportunities({ edgeUrl: EDGE_URL });
 *   
 *   // Read from store (selector prevents unnecessary re-renders)
 *   const opportunities = useOmniStore((state) => state.opportunities);
 *   const wsStatus = useOmniStore((state) => state.wsStatus);
 * }
 * ```
 */
export function useOmniOpportunities({
  edgeUrl,
  viableOnly = false,
  initialOpportunities = [],
}: UseOmniOpportunitiesOptions) {
  // Store actions (stable references)
  const addOpportunity = useOmniStore((state) => state.addOpportunity);
  const setWsStatus = useOmniStore((state) => state.setWsStatus);
  const mergeSnapshot = useOmniStore((state) => state.mergeSnapshot);

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

  // Initialize store with initial opportunities (once).
  // mergeSnapshot on an empty store is a plain populate; using it here keeps the
  // initial paint on the same code path as the live poll (no clear+wipe ever).
  useEffect(() => {
    if (initializedRef.current) return;
    initializedRef.current = true;

    if (initialOpportunities.length > 0) {
      mergeSnapshot(initialOpportunities);
    }
  }, [initialOpportunities, mergeSnapshot]);

  // DEV-ONLY auto-preview seed. Locally (Windows dev box) no edge backend is
  // reachable → SSR snapshot empty + polling fails → panel would render blank,
  // blocking card-layout review. In dev, ~2.5s after mount, if the store is
  // still empty, seed clearly-labelled SAMPLE fixtures (trace_id="preview") so
  // the operator sees populated cards. NEVER runs in production: the NODE_ENV
  // guard makes this dead code (tree-shaken) and the dynamic import is excluded
  // from the prod bundle. Real edge data always wins — the seed only fires when
  // the store is genuinely empty. See preview-fixtures.ts + the "DESIGN PREVIEW"
  // banner in OpportunitiesClient. REMOVE this effect + preview-fixtures.ts
  // before opening the deploy PR.
  useEffect(() => {
    if (process.env.NODE_ENV === "production") return;
    if (initialOpportunities.length > 0) return; // real SSR snapshot present
    let cancelled = false;
    const t = setTimeout(async () => {
      if (cancelled) return;
      if (useOmniStore.getState().opportunities.length > 0) return; // got real data
      const { buildPreviewOpportunities } = await import("./preview-fixtures");
      if (cancelled) return;
      if (useOmniStore.getState().opportunities.length > 0) return; // re-check post-await
      mergeSnapshot(buildPreviewOpportunities());
    }, 2500);
    return () => {
      cancelled = true;
      clearTimeout(t);
    };
  }, [initialOpportunities, mergeSnapshot]);

  // HTTP polling fallback
  const startPolling = useCallback(() => {
    if (usingPollingRef.current) return;
    usingPollingRef.current = true;
    setWsStatus("POLLING");

    const poll = async () => {
      try {
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
        const rawItems: unknown[] = Array.isArray((data as { items?: unknown }).items)
          ? ((data as { items: unknown[] }).items)
          : Array.isArray(data)
          ? (data as unknown[])
          : [];

        // Merge the fresh snapshot into the store: dedup by stable route key,
        // preserve each row's detected_at + id (no flash, continuous age), and
        // prepend genuinely-new routes. Replaces the old clear+re-add which
        // wiped the store every cycle and made every row look "new" again.
        const mapped = rawItems.map((raw) =>
          mapToOmniOpportunity(raw as Record<string, unknown>),
        );
        mergeSnapshot(mapped);
      } catch {
        // Swallow — status badge already shows "POLLING" (degraded)
      }
    };

    poll();
    pollingTimerRef.current = setInterval(poll, POLL_INTERVAL_MS);
  }, [edgeUrl, setWsStatus, mergeSnapshot]);

  // WebSocket lifecycle
  useEffect(() => {
    if (typeof window === "undefined") return;

    // R8 Fail-Honest: No hardcoded fallback to localhost. 
    // Must be provided via NEXT_PUBLIC_WS_URL.
    const wsUrl = process.env.NEXT_PUBLIC_WS_URL;
    if (!wsUrl) {
      console.warn("[WS] NEXT_PUBLIC_WS_URL is undefined. WebSocket disabled.");
      setWsStatus("STALE");
      startPolling();
      return;
    }

    // C4 fix: admin token required for WS handshake
    const adminToken = getAdminToken();
    if (!adminToken) {
      setWsStatus("STALE");
      startPolling();
      return;
    }

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
  }, [edgeUrl, startPolling, setWsStatus, addOpportunity]);

  // Return nothing — consumers read directly from store
  // This enforces SSOT pattern
}
