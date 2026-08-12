"use client";
import React, { useEffect, useState, useCallback, useMemo } from "react";
import { Zap, WifiOff, ShieldAlert, RefreshCw, Radio, ChevronDown } from "lucide-react";
import { sanitizeForDisplay } from "@/lib/omega-lexicon";
import { toast } from "sonner";
import { getApiBaseUrl } from "@/lib/api-client";
import { OpportunityDetailDialog, type OpportunityDetail } from "@/components/OpportunityDetailDialog";
import { OpportunityExchangeCard } from "@/components/opportunities/exchange/OpportunityExchangeCard";
import {
  ExchangeFilterBar,
  applyExchangeFilters,
  DEFAULT_FILTERS,
  type ExchangeFilters,
} from "@/components/opportunities/exchange/ExchangeFilterBar";

// ─── Omni-Store Integration ───────────────────────────────────────────────────
import { useOmniOpportunities } from "@/lib/store/useOmniOpportunities";
import { useOmniStore } from "@/lib/store/omni-store";
import { mapToOmniOpportunity, type OmniOpportunity } from "@/lib/store/types";
import { getTradingConfig } from "@/lib/api-client";
import type { StrategyRuntimeConfig } from "@/lib/schemas";
import { usePaperModeState } from "@/hooks/usePaperModeState";

// ─── Component imports ───────────────────────────────────────────────────────
import { DegradedBanner } from "@/components/DegradedBanner";

/**
 * Stable route identity for the card grid key (mirrors OpportunitiesClient) so a
 * re-detected route updates the SAME card in place rather than remounting.
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

const POLL_INTERVAL_MS = 4_000;
/** Hard cap on simultaneously mounted cards — the memory-discipline bound.
 * Combined with each card's `content-visibility:auto`, this keeps the grid's
 * footprint proportional to the viewport, not the feed length. */
const VISIBLE_CAP = 60;

export type OpportunitiesSnapshot = {
  opportunities: OmniOpportunity[];
  serverTime: string | null;
  source: string;
};

export default function OpportunitiesExchangeClient({
  initialSnapshot,
}: {
  initialSnapshot: OpportunitiesSnapshot;
}) {
  const EDGE_URL = getApiBaseUrl();

  useOmniOpportunities({
    viableOnly: false,
    initialOpportunities: initialSnapshot.opportunities,
  });

  // Selectors from Omni-Store (SSOT)
  const opportunities = useOmniStore((state) => state.opportunities);
  const wsStatus = useOmniStore((state) => state.wsStatus);
  const setOpportunities = useOmniStore((state) => state.setOpportunities);

  // ─── UI State ──────────────────────────────────────────────────────────────
  const [isMounted, setIsMounted] = useState(false);
  const [errorMsg, setErrorMsg] = useState<string | null>(null);
  const [now, setNow] = useState<number>(0);
  const [simLoading, setSimLoading] = useState<string | null>(null);
  const [selectedOpp, setSelectedOpp] = useState<OpportunityDetail | null>(null);
  const [lastRefresh, setLastRefresh] = useState<Date | null>(
    initialSnapshot.serverTime ? new Date(initialSnapshot.serverTime) : null,
  );
  const [filters, setFilters] = useState<ExchangeFilters>(DEFAULT_FILTERS);
  const [cap, setCap] = useState<number>(VISIBLE_CAP);

  // ─── Effective execution terminus (paper / live) — read-only ───────────────
  // usePaperModeState is fail-safe: DEFAULT_SAFE_STATE has enabled=false with
  // confidence "default_safe". We only trust "live" when the server reports
  // paper OFF with a real (non-default_safe) reading; anything degraded reads
  // as "paper" (the safe terminus). Mode switching itself lives in /config +
  // /killswitch (§34) — this badge is display-only.
  const primaryChainId = initialSnapshot.opportunities[0]?.chain_id ?? 1;
  const paperMode = usePaperModeState(primaryChainId);
  const modeLabel: "paper" | "live" = paperMode.data.enabled
    ? "paper"
    : paperMode.data.confidence !== "default_safe"
      ? "live"
      : "paper";

  // ── Declared strategy config (trading_config.strategy_configs) ─────────────
  const [strategyConfigs, setStrategyConfigs] = useState<Record<
    string,
    StrategyRuntimeConfig | null
  >>({});
  useEffect(() => {
    let cancelled = false;
    getTradingConfig(primaryChainId)
      .then((cfg) => {
        if (cancelled || !cfg.ok) return;
        // TradingConfigResponse is a discriminated union on `configured`; the
        // configured branch carries strategy_configs. Defensive cast keeps the
        // panel rendering "—" when the branch isn't configured (R8 fail-honest).
        const data = cfg.data as {
          configured?: boolean;
          strategy_configs?: Record<string, StrategyRuntimeConfig>;
        };
        if (!data.configured) return;
        setStrategyConfigs(data.strategy_configs ?? {});
      })
      .catch(() => {
        // Fail-honest: leave the map empty so cards show "—".
      });
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const feedStatus = wsStatus;

  // ── EXECUTE (shadow) handler — mode-invariant, read-only (capital $0) ──────
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

  const onInspect = useCallback((opp: OmniOpportunity) => {
    setSelectedOpp(opp as unknown as OpportunityDetail);
  }, []);
  const onExecute = useCallback(
    (opportunityId: string) => handleSimulate(opportunityId),
    [handleSimulate],
  );

  const fetchOpportunities = useCallback(async () => {
    try {
      const url = `${EDGE_URL}/api/opportunities/live?viable_only=false&limit=50`;
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
      setOpportunities(items.map((raw: Record<string, unknown>) => mapToOmniOpportunity(raw)));
      setLastRefresh(new Date());
      setErrorMsg(null);
    } catch (e) {
      setErrorMsg((e as Error).message);
    }
  }, [EDGE_URL, setOpportunities]);

  useEffect(() => {
    setIsMounted(true);
    setNow(Date.now());
  }, []);

  // PERF: 30s age ticker (same discipline as OpportunitiesClient).
  useEffect(() => {
    const ticker = setInterval(() => setNow(Date.now()), 30000);
    return () => clearInterval(ticker);
  }, []);

  // Reset the visible cap whenever the filter narrows so the operator sees the
  // top of the fresh result set.
  useEffect(() => {
    setCap(VISIBLE_CAP);
  }, [filters]);

  // ── Derived visible list (filtered + capped) ───────────────────────────────
  const filtered = useMemo(
    () => applyExchangeFilters(opportunities, filters),
    [opportunities, filters],
  );
  const visible = useMemo(() => filtered.slice(0, cap), [filtered, cap]);

  const isError = feedStatus === "STALE";

  return (
    <div className={`p-8 min-h-screen transition-colors duration-500 text-foreground ${isError ? "bg-destructive/5" : ""}`}>
      <div className="flex justify-between items-center border-b border-border pb-4 mb-6">
        <div>
          <h1 className={`text-4xl font-extrabold tracking-tight bg-clip-text text-transparent ${isError ? "bg-gradient-to-r from-destructive to-destructive/70" : "bg-gradient-to-r from-primary to-success"}`}>
            {sanitizeForDisplay("Exchange Feed")}
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
          {isMounted && (
            <p className="text-xs mt-1 text-muted-foreground">
              <span className="text-success font-semibold">{filtered.length}</span> matching
              {" · "}
              <span className="text-foreground font-semibold">{opportunities.length}</span> total detected
            </p>
          )}
        </div>

        <div className="flex items-center gap-3">
          <button
            type="button"
            onClick={fetchOpportunities}
            className="p-2 rounded-lg bg-muted hover:bg-accent transition-colors border border-border"
            title="Force refresh"
          >
            <RefreshCw size={16} className="text-muted-foreground" />
          </button>
          <div className={`flex items-center gap-2 px-4 py-2 rounded-full border shadow-lg ${
            feedStatus === "LIVE"       ? "bg-success/10 border-success/40 text-success" :
            feedStatus === "POLLING"    ? "bg-info/10 border-info/40 text-info" :
            feedStatus === "CONNECTING" ? "bg-muted border-border text-muted-foreground" :
            /* STALE */                   "bg-warning/10 border-warning/40 text-warning animate-pulse"
          }`}>
            {feedStatus === "LIVE"       ? <Zap size={18} /> :
             feedStatus === "POLLING"    ? <Radio size={18} className="animate-pulse" /> :
             feedStatus === "CONNECTING" ? <Radio size={18} className="animate-pulse" /> :
             /* STALE */                   <WifiOff size={18} />}
            <span className="text-sm font-bold tracking-widest">{feedStatus}</span>
          </div>
        </div>
      </div>

      <ExchangeFilterBar
        opportunities={opportunities}
        filters={filters}
        onChange={setFilters}
        modeLabel={modeLabel}
        modeConfidence={paperMode.data.confidence}
      />

      {feedStatus === "STALE" && (
        <div className="mb-6 p-4 bg-warning/10 border border-warning/30 rounded-xl flex items-center gap-4 text-warning">
          <WifiOff size={24} />
          <div>
            <h3 className="font-bold">STREAM DISCONNECTED</h3>
            <p className="text-sm">WebSocket connection lost — reconnecting. Displayed data may be stale.</p>
          </div>
        </div>
      )}

      {errorMsg !== null && (
        <div className="mb-6 p-4 bg-destructive/10 border border-destructive/30 rounded-xl flex items-center gap-4 text-destructive">
          <ShieldAlert size={24} />
          <div>
            <h3 className="font-bold">EDGE REFRESH ERROR</h3>
            <p className="text-sm">Manual refresh failed: {errorMsg}</p>
          </div>
        </div>
      )}

      {initialSnapshot.source === "server-fetch-failed" && feedStatus !== "LIVE" && feedStatus !== "POLLING" && (
        <DegradedBanner
          title="Initial server snapshot unavailable — waiting for the live feed"
          reason="server-side fetch of /api/opportunities/live failed"
          endpoint="GET /api/opportunities/live"
        />
      )}

      {(feedStatus === "LIVE" || feedStatus === "POLLING" || feedStatus === "CONNECTING") && filtered.length === 0 && (
        <div className="mb-6 p-4 bg-muted/50 border border-border rounded-xl flex items-center gap-4 text-muted-foreground shadow-inner">
          <div className="relative flex h-3 w-3">
            <span className="animate-ping absolute inline-flex h-full w-full rounded-full bg-success opacity-75"></span>
            <span className="relative inline-flex rounded-full h-3 w-3 bg-success"></span>
          </div>
          <div>
            <h3 className="font-bold text-success tracking-wide">
              {opportunities.length === 0 ? "SCANNING — FEED WARMING UP" : "NO MATCHES FOR FILTERS"}
            </h3>
            <p className="text-sm mt-1">
              {opportunities.length === 0
                ? "No detections in the live window yet. The searcher emits in bursts; opportunities will appear as they're detected."
                : "No opportunities match the current family/chain/yield filters. Loosen them to see more."}
            </p>
          </div>
        </div>
      )}

      {/* ── Card grid (memory-disciplined: capped mount + content-visibility) ── */}
      <div className="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-4">
        {visible.map((opp) => (
          <OpportunityExchangeCard
            key={routeKeyOf(opp)}
            opp={opp}
            now={now}
            isMounted={isMounted}
            simLoading={simLoading === opp.id}
            strategyConfig={strategyConfigs[opp.strategy_kind] ?? null}
            modeLabel={modeLabel}
            onExecute={onExecute}
            onInspect={onInspect}
          />
        ))}
      </div>

      {/* ── Cap control: surface what's hidden + load more ── */}
      {filtered.length > visible.length && (
        <div className="mt-6 flex flex-col items-center gap-2">
          <p className="text-xs text-muted-foreground">
            Showing <span className="text-foreground font-semibold">{visible.length}</span> of{" "}
            <span className="text-foreground font-semibold">{filtered.length}</span> matching — the rest are
            deferred (memory-discipline cap).
          </p>
          <button
            type="button"
            onClick={() => setCap((c) => c + VISIBLE_CAP)}
            className="inline-flex items-center gap-1.5 px-4 py-2 rounded-lg border border-border bg-muted/40 hover:bg-muted/70 text-xs font-semibold transition-colors"
          >
            <ChevronDown size={14} /> Show {Math.min(VISIBLE_CAP, filtered.length - visible.length)} more
          </button>
        </div>
      )}

      <OpportunityDetailDialog
        opportunity={selectedOpp}
        onClose={() => setSelectedOpp(null)}
      />
    </div>
  );
}
