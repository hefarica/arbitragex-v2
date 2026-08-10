"use client";
import React, { useEffect, useState, useCallback, useRef } from "react";
import { Zap, WifiOff, ShieldAlert, RefreshCw, Radio, EyeOff, Eye } from "lucide-react";
import { sanitizeForDisplay } from "@/lib/omega-lexicon";
import { toast } from "sonner";
import { OpportunityDetailDialog, type OpportunityDetail } from "@/components/OpportunityDetailDialog";
import { OpportunityTradeCard } from "@/components/OpportunityTradeCard";

// ─── Omni-Store Integration ───────────────────────────────────────────────────
import { useOmniOpportunities } from "@/lib/store/useOmniOpportunities";
import { useOmniStore } from "@/lib/store/omni-store";
import { mapToOmniOpportunity, type OmniOpportunity } from "@/lib/store/types";
import { getTradingConfig } from "@/lib/api-client";
import type { StrategyRuntimeConfig } from "@/lib/schemas";

// Re-export types for backward compatibility with OpportunityDetailDialog
export type {
  OmniOpportunity,
  TokenInfo,
  TokenValidationBlock,
  StrategyKind,
  OpportunityStatus,
  SimulatedCostBreakdown,
  SimulatedTarget,
} from "@/lib/store/types";

// ─── Component imports ───────────────────────────────────────────────────────
import { DegradedBanner } from "@/components/DegradedBanner";
import { useUserPrefs } from "@/lib/user-prefs";

/**
 * Stable route identity for the card grid key. A re-detected route (same
 * chain + strategy + token pair + DEX path) must update the SAME card in place
 * rather than remount a new one — so we key on the route identity, not the
 * per-detection row id. This is what stops the enter-animation flash each poll
 * and what makes a card disappear the moment its route drops from the snapshot.
 */
function routeKeyOf(opp: OmniOpportunity): string {
  return [
    opp.chain_id,
    opp.chain_id_out ?? "",
    opp.strategy_kind,
    opp.token_in,
    opp.token_out,
    opp.dex_a,
    opp.dex_b ?? "",
  ].join("|");
}

// FE-1: WS statuses. "LIVE" = WS connected. "STALE" = WS disconnected.
// "POLLING" = WS failed 3×, degraded to HTTP polling. "CONNECTING" = initial.
// HTTP fetch errors (manual refresh) surface via errorMsg, not feedStatus.


const POLL_INTERVAL_MS = 4_000;

export type OpportunitiesSnapshot = {
  opportunities: OmniOpportunity[];
  serverTime: string | null;
  source: string;
};

export default function OpportunitiesClient({
  initialSnapshot,
}: {
  initialSnapshot: OpportunitiesSnapshot;
}) {
  // ─── Omni-Store Integration ───────────────────────────────────────────────
  // Connect WebSocket stream to the store (replaces useOpportunitiesStream)
  const EDGE_URL = process.env.NEXT_PUBLIC_EDGE_URL ?? "";
  const [viableOnly, setViableOnly] = useState(false);
  
  useOmniOpportunities({
    edgeUrl: EDGE_URL,
    viableOnly,
    initialOpportunities: initialSnapshot.opportunities,
  });

  // Selectors from Omni-Store (SSOT)
  const opportunities = useOmniStore((state) => state.opportunities);
  const wsStatus = useOmniStore((state) => state.wsStatus);
  const setOpportunities = useOmniStore((state) => state.setOpportunities);

  // ─── UI State (local, not in store) ───────────────────────────────────────
  const [isMounted, setIsMounted] = useState(false);
  const [errorMsg, setErrorMsg] = useState<string | null>(null);
  const [now, setNow] = useState<number>(0);
  const [simLoading, setSimLoading] = useState<string | null>(null);
  const [selectedOpp, setSelectedOpp] = useState<OpportunityDetail | null>(null);
  const [lastRefresh, setLastRefresh] = useState<Date | null>(
    initialSnapshot.serverTime ? new Date(initialSnapshot.serverTime) : null
  );

  // FE-6: Track IDs already notified to avoid duplicate toasts across polls.
  // R1: useRef is SSR-safe — no access to window or localStorage.
  // PERF (2026-08-10): prune IDs that are no longer in the live list so this
  // Set does not grow unbounded as the mempool churns through unique detections.
  const seenNotifiedIds = useRef<Set<string>>(new Set());
  const opportunityIds = useRef<Set<string>>(new Set());

  useEffect(() => {
    const currentIds = new Set(opportunities.map((o) => o.id));
    opportunityIds.current = currentIds;
    for (const id of seenNotifiedIds.current) {
      if (!currentIds.has(id)) {
        seenNotifiedIds.current.delete(id);
      }
    }
  }, [opportunities]);

  // FE-13: Read notification threshold from user prefs (localStorage, R1 compliant).
  const { prefs } = useUserPrefs();

  // ── Declared strategy config (trading_config.strategy_configs) ─────────────
  // R8 fail-honest: if the config endpoint fails or a strategy has no entry,
  // the card receives null and renders "—" (never fabricated). SSR-safe: the
  // fetch lives in useEffect, not in render, so no hydration mismatch (R1).
  const [strategyConfigs, setStrategyConfigs] = useState<Record<
    string,
    StrategyRuntimeConfig | null
  >>({});
  useEffect(() => {
    const chainId = initialSnapshot.opportunities[0]?.chain_id ?? 1;
    let cancelled = false;
    getTradingConfig(chainId)
      .then((cfg) => {
        if (cancelled || !cfg) return;
        setStrategyConfigs(
          (cfg as { strategy_configs?: Record<string, StrategyRuntimeConfig> })
            .strategy_configs ?? {},
        );
      })
      .catch(() => {
        // Fail-honest: leave the map empty so cards show "—".
      });
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Derive feedStatus from wsStatus for display. "POLLING" is the degraded
  // HTTP-fallback state emitted by the hook after 3 WS failures.
  const feedStatus = wsStatus;

  // FE-4: EXECUTE (shadow) handler.
  // Shadow execution — POST /api/v1/opportunities/:id/simulate → sim-ctl + Anvil
  // fork. Read-only (capital $0), returns pass/fail + gas + trace evidence.
  const handleSimulate = useCallback(async (opportunityId: string) => {
    setSimLoading(opportunityId);
    try {
      const res = await fetch(`${EDGE_URL}/api/v1/opportunities/${opportunityId}/simulate`, {
        method: "POST",
        credentials: "include",
        headers: { "content-type": "application/json", accept: "application/json" },
        body: JSON.stringify({ route_source: "simctl_lookup" }),
        signal: AbortSignal.timeout(15000),
      });
      const body = (await res.json().catch(() => ({}))) as {
        result?: {
          passed?: boolean;
          gas_estimate_wei?: string | null;
          trace_id?: string | null;
          fail_reason?: string | null;
          slippage_pct?: number | null;
        };
        error?: string;
        detail?: string;
      };
      if (!res.ok) {
        toast.error(`Shadow sim unavailable (HTTP ${res.status})`, {
          description: body.error ?? body.detail ?? "sim-ctl unreachable",
        });
        return;
      }
      const r = body.result ?? {};
      if (r.passed) {
        toast.success("Shadow sim PASSED", {
          description: `gas ${r.gas_estimate_wei ?? "—"} · trace ${r.trace_id?.slice(0, 8) ?? "—"}`,
        });
      } else {
        toast.error("Shadow sim FAILED", {
          description: r.fail_reason ?? "route did not converge on the fork",
        });
      }
    } catch (e) {
      const err = e as Error;
      if (err.name === "AbortError" || err.name === "TimeoutError") {
        toast.error("Shadow sim timed out after 15s");
      } else {
        toast.error("Shadow sim error", { description: err.message });
      }
      throw e;
    } finally {
      setSimLoading(null);
    }
  }, [EDGE_URL]);

  // Memoize callbacks so React.memo on OpportunityTradeCard isn't defeated
  // every time the parent re-renders (e.g. from the age ticker).
  const onInspect = useCallback((opp: OmniOpportunity) => {
    setSelectedOpp(opp as unknown as OpportunityDetail);
  }, []);

  const onExecute = useCallback(
    (opportunityId: string) => handleSimulate(opportunityId),
    [handleSimulate],
  );
  // It clears the store and repopulates via HTTP, then the WS stream continues.
  const fetchOpportunities = useCallback(async () => {
    try {
      const url = `${EDGE_URL}/api/opportunities/live?viable_only=${viableOnly}&limit=50`;
      const res = await fetch(url, {
        headers: { accept: "application/json" },
        signal: AbortSignal.timeout(4000),
        cache: "no-store",
      });
      if (!res.ok) {
        setErrorMsg(`Edge returned ${res.status}`);
        return;
      }
      const data = await res.json();
      const items = Array.isArray(data?.items) ? data.items : Array.isArray(data) ? data : [];
      // PERF: batch store update instead of clear + 50 addOpportunity calls.
      setOpportunities(items.map((raw: Record<string, unknown>) => mapToOmniOpportunity(raw)));
      setLastRefresh(new Date());
      setErrorMsg(null);
    } catch (e) {
      setErrorMsg((e as Error).message);
    }
  }, [EDGE_URL, viableOnly, setOpportunities]);

  // R1: localStorage read happens here — never during render (SSR has no localStorage).
  // 2026-05-10: bumped the storage key from "arbx-opps-viable-only" to "-v2" so
  // operators whose pre-fix sessions had the old key set to "true" (under the
  // legacy default-true behavior) get a fresh default-false on first load
  // with the new build. The old key is also actively cleared to keep
  // localStorage tidy across re-bumps. Operator's explicit choice in this
  // build is still persisted under the new key.
  useEffect(() => {
    try { localStorage.removeItem("arbx-opps-viable-only"); } catch { /* private mode */ }
    const stored = localStorage.getItem("arbx-opps-viable-only-v2");
    if (stored === "true") setViableOnly(true);
  }, []);

  const onToggleViableOnly = useCallback((newValue: boolean) => {
    setViableOnly(newValue);
    try { localStorage.setItem("arbx-opps-viable-only-v2", String(newValue)); } catch { /* private mode */ }
  }, []);

  // FE-6: Fire a toast for every new opportunity that clears the threshold.
  // R1: opportunities come from Omni-Store (SSOT).
  // seenNotifiedIds persists across re-renders via useRef so we never
  // re-toast the same opportunity across WS reconnects or poll cycles.
  useEffect(() => {
    if (!isMounted) return;
    for (const opp of opportunities) {
      if (seenNotifiedIds.current.has(opp.id)) continue;
      seenNotifiedIds.current.add(opp.id);
      const yieldVal = opp.expected_profit_usd ?? 0;
      if (yieldVal >= prefs.notification_threshold_usd) {
        toast.success(`High-value opportunity — ${opp.strategy_kind}`, {
          description: `Net yield $${yieldVal.toFixed(2)} · chain ${opp.chain_id} · ${opp.dex_a}${opp.dex_b ? ` → ${opp.dex_b}` : ""}`,
          duration: 8_000,
        });
      }
    }
  }, [opportunities, isMounted, prefs.notification_threshold_usd]);

  // R1: setIsMounted + setNow are the only non-WS side effects needed here.
  useEffect(() => {
    setIsMounted(true);
    setNow(Date.now());
  }, []);

  // PERF (2026-08-09): the age ticker previously ran every 1000ms, re-rendering
  // all ~200 OpportunityTradeCards every second — a real CPU/memory churner on a
  // live feed. The "Last refresh" label already ticks off `lastRefresh` (independent
  // state), so `now` only feeds each card's relative-age text. 30s is plenty for a
  // human-readable age; combined with the card's React.memo this collapses the
  // per-second full-list re-render to near zero.
  useEffect(() => {
    const ticker = setInterval(() => setNow(Date.now()), 30000);
    return () => clearInterval(ticker);
  }, []);

  // FE-1: Opportunities come from Omni-Store (SSOT).
  const viableCount = opportunities.filter((o) => o.status !== "rejected" && o.status !== "failed").length;
  const rejectedCount = opportunities.filter((o) => o.status === "rejected").length;

  const isError = feedStatus === 'STALE';

  return (
    <div className={`p-8 min-h-screen transition-colors duration-500 text-foreground ${isError ? 'bg-destructive/5' : ''}`}>
      <div className="flex justify-between items-center border-b border-border pb-4 mb-8">
        <div>
          <h1 className={`text-4xl font-extrabold tracking-tight bg-clip-text text-transparent ${isError ? 'bg-gradient-to-r from-destructive to-destructive/70' : 'bg-gradient-to-r from-primary to-success'}`}>
            {sanitizeForDisplay("Live MEV Feed")}
          </h1>
          <p className="text-muted-foreground mt-2 text-sm" suppressHydrationWarning>
            {feedStatus === "LIVE"
              ? "Live stream via WebSocket"
              : feedStatus === "POLLING"
              ? `Fallback: polling edge every ${POLL_INTERVAL_MS / 1000}s`
              : feedStatus === "STALE"
              ? "Stream disconnected — reconnecting…"
              : feedStatus === "CONNECTING"
              ? "Connecting to stream…"
              : "Edge connection error"}
            {" · "}
            {isMounted && lastRefresh ? `Last refresh: ${lastRefresh.toLocaleTimeString()}` : "Loading..."}
          </p>
          {/* Counter: shown only after mount to avoid SSR mismatch */}
          {isMounted && (
            <p className="text-xs mt-1 text-muted-foreground">
              {viableOnly ? (
                <span>
                  <span className="text-success font-semibold">{opportunities.length}</span> viable
                </span>
              ) : (
                <span>
                  <span className="text-success font-semibold">{viableCount}</span> viable
                  {" / "}
                  <span className="text-foreground font-semibold">{opportunities.length}</span> total
                  {rejectedCount > 0 && (
                    <span className="text-destructive"> ({rejectedCount} rejected)</span>
                  )}
                </span>
              )}
            </p>
          )}
        </div>

        <div className="flex items-center gap-3">
          {/* Viable-only toggle — R1: state is client-only, localStorage read in useEffect */}
          <button
            type="button"
            onClick={() => onToggleViableOnly(!viableOnly)}
            className={`flex items-center gap-2 px-3 py-1.5 rounded-lg border text-xs font-semibold transition-colors ${
              viableOnly
                ? "bg-success/10 border-success/40 text-success hover:bg-success/20"
                : "bg-destructive/10 border-destructive/40 text-destructive hover:bg-destructive/20"
            }`}
            title={viableOnly ? "Showing viable only — click to show all including rejected" : "Showing all including rejected — click to show viable only"}
            aria-pressed={viableOnly ? "true" : "false"}
          >
            {viableOnly ? <Eye size={14} /> : <EyeOff size={14} />}
            <span>{viableOnly ? "Viable only" : "Show all"}</span>
          </button>
          <button
            type="button"
            onClick={fetchOpportunities}
            className="p-2 rounded-lg bg-muted hover:bg-accent transition-colors border border-border"
            title="Force refresh"
          >
            <RefreshCw size={16} className="text-muted-foreground" />
          </button>
          <div className={`flex items-center gap-2 px-4 py-2 rounded-full border shadow-lg ${
            feedStatus === 'LIVE'       ? 'bg-success/10 border-success/40 text-success' :
            feedStatus === 'POLLING'    ? 'bg-info/10 border-info/40 text-info' :
            feedStatus === 'CONNECTING' ? 'bg-muted border-border text-muted-foreground' :
            /* STALE */                   'bg-warning/10 border-warning/40 text-warning animate-pulse'
          }`}>
            {feedStatus === 'LIVE'       ? <Zap size={18} /> :
             feedStatus === 'POLLING'    ? <Radio size={18} className="animate-pulse" /> :
             feedStatus === 'CONNECTING' ? <Radio size={18} className="animate-pulse" /> :
             /* STALE */                   <WifiOff size={18} />}
            <span className="text-sm font-bold tracking-widest">{feedStatus}</span>
          </div>
        </div>
      </div>

      {/* R8 fail-honest: surface WS disconnection and HTTP errors clearly. */}
      {feedStatus === 'STALE' && (
        <div className="mb-8 p-4 bg-warning/10 border border-warning/30 rounded-xl flex items-center gap-4 text-warning">
          <WifiOff size={24} />
          <div>
            <h3 className="font-bold">STREAM DISCONNECTED</h3>
            <p className="text-sm">WebSocket connection lost — reconnecting. Displayed data may be stale.</p>
          </div>
        </div>
      )}

      {/* Manual refresh error — shown when Force Refresh fetch fails (R8 fail-honest). */}
      {errorMsg !== null && (
        <div className="mb-8 p-4 bg-destructive/10 border border-destructive/30 rounded-xl flex items-center gap-4 text-destructive">
          <ShieldAlert size={24} />
          <div>
            <h3 className="font-bold">EDGE REFRESH ERROR</h3>
            <p className="text-sm">Manual refresh failed: {errorMsg}</p>
          </div>
        </div>
      )}

      {/* R8 fail-honest: the server-rendered initial snapshot failed and the live
          feed has not yet taken over — say so, rather than letting the empty
          "scanning" state below imply a healthy first paint. Clears as soon as
          the WebSocket connects (LIVE) or degrades to HTTP polling (POLLING). */}
      {initialSnapshot.source === "server-fetch-failed" && feedStatus !== "LIVE" && feedStatus !== "POLLING" && (
        <DegradedBanner
          title="Initial server snapshot unavailable — waiting for the live feed"
          reason="server-side fetch of /api/opportunities/live failed"
          endpoint="GET /api/opportunities/live"
        />
      )}

      {(feedStatus === 'LIVE' || feedStatus === 'POLLING' || feedStatus === 'CONNECTING') && opportunities.length === 0 && (
        <div className="mb-8 p-4 bg-muted/50 border border-border rounded-xl flex items-center gap-4 text-muted-foreground shadow-inner">
          <div className="relative flex h-3 w-3">
            <span className="animate-ping absolute inline-flex h-full w-full rounded-full bg-success opacity-75"></span>
            <span className="relative inline-flex rounded-full h-3 w-3 bg-success"></span>
          </div>
          <div>
            <h3 className="font-bold text-success tracking-wide">SCANNING MEMPOOL IN REAL-TIME</h3>
            <p className="text-sm mt-1">
              {viableOnly
                ? "No viable opportunities yet. Toggle \"Show all\" to inspect rejected detections."
                : sanitizeForDisplay("Searcher-rs is actively hunting for resolution routes. Opportunities will appear here instantly.")}
            </p>
          </div>
        </div>
      )}

      {/* ── Opportunity trade cards (revived card-v2 pattern, commit e9bcadd,
            extended per operator execution spec). One card per route; keyed by
            the STABLE route identity so a re-detected route updates the card in
            place (no remount / no enter-animation flash each poll) and the card
            disappears when the route drops out of the snapshot. R8 fail-honest:
            unknown figures render "—", never fabricated. ── */}
      {/* AnimatePresence removed from the high-churn live grid (2026-08-10).
          Exit animations retained DOM nodes for 250ms every poll; with 200 live
          cards that accumulated nodes/memory. Items still animate on enter via
          motion.div initial/animate. */}
      <div className="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-4">
        {opportunities.map((opp) => (
          <OpportunityTradeCard
            key={routeKeyOf(opp)}
            opp={opp}
            now={now}
            isMounted={isMounted}
            simLoading={simLoading === opp.id}
              strategyConfig={strategyConfigs[opp.strategy_kind] ?? null}
              onExecute={onExecute}
              onInspect={onInspect}
            />
          ))}
      </div>

      {/* FE-10: Opportunity detail sheet — click any row to open */}
      <OpportunityDetailDialog
        opportunity={selectedOpp}
        onClose={() => setSelectedOpp(null)}
      />
    </div>
  );
}
