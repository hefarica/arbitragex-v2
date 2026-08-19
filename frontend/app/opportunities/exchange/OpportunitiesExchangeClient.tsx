"use client";
import React, { useEffect, useState, useCallback, useMemo } from "react";
import { WifiOff, ShieldAlert, RefreshCw, ChevronDown } from "lucide-react";
import { sanitizeForDisplay } from "@/lib/omega-lexicon";
import { toast } from "sonner";
import { getApiBaseUrl } from "@/lib/api-client";
import { OpportunityDetailDialog, type OpportunityDetail } from "@/components/OpportunityDetailDialog";
import { OpportunityExchangeCard } from "@/components/opportunities/exchange/OpportunityExchangeCard";
import { PriceTicker } from "@/components/opportunities/exchange/PriceTicker";
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

  // ── G-PRICE-1 — live price tape symbols (deduped, from the visible pairs) ──
  const tickerSymbols = useMemo(() => {
    const syms = new Set<string>();
    for (const opp of visible) {
      for (const leg of (opp.pair_symbol ?? "").split(/[/\-]+/)) {
        const s = leg.trim().toUpperCase();
        if (s) syms.add(s);
      }
    }
    return Array.from(syms);
  }, [visible]);

  const isError = feedStatus === "STALE";

  // Semantic LED for the FEED chip: LIVE → green on, STALE → orange hot,
  // POLLING/CONNECTING/other → amber wait.
  const feedLedClass =
    feedStatus === "LIVE" ? "led on" : feedStatus === "STALE" ? "led wait led-hot" : "led wait";
  const feedTextClass =
    feedStatus === "LIVE" ? "led-text-live" : feedStatus === "STALE" ? "led-text-hot" : "led-text-wait";

  return (
    <div className={`atlas-scope atlas-ground min-h-screen ${isError ? "atlas-error" : ""}`}>
      <header>
        <h1>{sanitizeForDisplay("Exchange Feed")}</h1>
        <p className="sub">
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
          {/* R1: only the clock segment is non-deterministic — suppress on this
              span alone, never on the whole container. */}
          <span suppressHydrationWarning>
            {isMounted && lastRefresh ? `Last refresh: ${lastRefresh.toLocaleTimeString()}` : "Loading..."}
          </span>
        </p>

        {/* Feed chips: refresh + matching/total counts + live LED status */}
        <div className="chips">
          <button
            type="button"
            onClick={fetchOpportunities}
            className="chip"
            title="Force refresh"
          >
            <RefreshCw size={11} /> REFRESH
          </button>
          {isMounted && (
            <>
              <span className="chip">
                MATCHING <b>{filtered.length}</b>
              </span>
              <span className="chip">
                TOTAL DETECTED <b>{opportunities.length}</b>
              </span>
            </>
          )}
          <span className="chip" title="Feed transport status">
            <span className={feedLedClass} />
            FEED <b className={feedTextClass}>{feedStatus}</b>
          </span>
        </div>
      </header>

      <ExchangeFilterBar
        opportunities={opportunities}
        filters={filters}
        onChange={setFilters}
        modeLabel={modeLabel}
        modeConfidence={paperMode.data.confidence}
      />

      {/* G-PRICE-1 — exchange-style snapshot+push USD price tape */}
      <PriceTicker chainId={primaryChainId} edgeUrl={EDGE_URL} symbols={tickerSymbols} />

      {feedStatus === "STALE" && (
        <div className="glass-panel warn mb-6 flex items-center gap-4">
          <WifiOff size={24} />
          <div>
            <h3 className="font-bold">STREAM DISCONNECTED</h3>
            <p className="text-sm">WebSocket connection lost — reconnecting. Displayed data may be stale.</p>
          </div>
        </div>
      )}

      {errorMsg !== null && (
        <div className="glass-panel err mb-6 flex items-center gap-4">
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
        <div className="glass-panel mb-6 flex items-center gap-4">
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
      <div className="atlas-cards2">
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
            className="chip"
          >
            <ChevronDown size={12} /> Show {Math.min(VISIBLE_CAP, filtered.length - visible.length)} more
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
