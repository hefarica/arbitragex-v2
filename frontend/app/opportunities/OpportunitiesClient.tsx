"use client";
import React, { useEffect, useState, useCallback, useRef } from "react";
import { Zap, WifiOff, ShieldAlert, RefreshCw, Radio, EyeOff, Eye, AlertTriangle, Clock, TrendingUp } from "lucide-react";
import { sanitizeForDisplay } from "@/lib/omega-lexicon";
import { toast } from "sonner";
import { OpportunityDetailDialog, type OpportunityDetail } from "@/components/OpportunityDetailDialog";
import { motion, AnimatePresence } from "framer-motion";

// ─── Omni-Store Integration ───────────────────────────────────────────────────
import { useOmniOpportunities } from "@/lib/store/useOmniOpportunities";
import { useOmniStore, routeKey } from "@/lib/store/omni-store";
import { mapToOmniOpportunity, type OmniOpportunity } from "@/lib/store/types";

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

// ─── Component imports (Tasks 10 / 11) ───────────────────────────────────────
import { TokenChip } from "@/components/TokenChip";
import { ChainBadge } from "@/components/ChainBadge";
import { StrategyBadge } from "@/components/StrategyBadge";
import { StatusPill } from "@/components/StatusPill";
import { DegradedBanner } from "@/components/DegradedBanner";
import {
  formatProfitUSD,
  formatPctOrDash,
} from "@/lib/format";
import { useUserPrefs } from "@/lib/user-prefs";

// ─── Tone → token-based class map (used for YIELD cell) ─────────────────────
const TONE_CLASS: Record<string, string> = {
  positive: "text-success",
  negative: "text-destructive",
  zero:     "text-muted-foreground",
  neutral:  "text-muted-foreground",
  pending:  "text-muted-foreground/60 italic",
};

/**
 * Compact USD formatter for the inverse-sizing hint (e.g. `$12.5k`, `$1.8M`).
 * Used only for the suggested-amount subline; primary yield numbers keep the
 * full `formatProfitUSD` precision.
 */
function formatUsdShort(value: number): string {
  if (!Number.isFinite(value)) return "—";
  const abs = Math.abs(value);
  if (abs >= 1_000_000) return `$${(value / 1_000_000).toFixed(2)}M`;
  if (abs >= 1_000)     return `$${(value / 1_000).toFixed(1)}k`;
  if (abs >= 1)         return `$${value.toFixed(2)}`;
  return `$${value.toFixed(4)}`;
}

/**
 * PR 6 — formats a raw amount_in_wei string into a human-readable token amount
 * for the Trade block's "Buy qty" cell. DISPLAY ONLY (rounded, never used for
 * profit math). When decimals are unknown, falls back to raw wei so the cell
 * never fabricates a unit count. R8: "—" when amount is missing/zero.
 */
function formatTokenAmount(
  wei: string | null | undefined,
  decimals: number | null | undefined,
  symbol: string | null | undefined,
): string {
  if (!wei || wei === "0") return "—";
  const sym = symbol ?? "";
  const d = decimals ?? null;
  if (d == null) return sym ? `${wei} ${sym} (raw)` : `${wei} wei`;
  // Number is acceptable for DISPLAY (the wei's low-order digits are
  // insignificant after dividing by 10^decimals). Not used for any calc.
  const units = Number(wei) / Math.pow(10, d);
  if (!Number.isFinite(units)) return "—";
  const str = units >= 1000 ? units.toFixed(2) : units >= 1 ? units.toFixed(4) : units.toPrecision(3);
  return sym ? `${str} ${sym}` : str;
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
  const mergeSnapshot = useOmniStore((state) => state.mergeSnapshot);

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
  const seenNotifiedIds = useRef<Set<string>>(new Set());

  // FE-13: Read notification threshold from user prefs (localStorage, R1 compliant).
  const { prefs } = useUserPrefs();

  // Derive feedStatus from wsStatus for display. "POLLING" is the degraded
  // HTTP-fallback state emitted by the hook after 3 WS failures.
  const feedStatus = wsStatus;

  // FE-4: SIMULATE handler.
  // NOTE: The api-server does not yet expose POST /api/opportunities/:id/simulate.
  // R8 fail-honest: surfacing the gap via toast rather than silencing it.
  // Backend ticket required: POST /api/opportunities/:id/simulate → { profit_usd, gas_cost_usd, net_profit_usd }
  const handleSimulate = useCallback(async (opportunityId: string) => {
    setSimLoading(opportunityId);
    try {
      const res = await fetch(`${EDGE_URL}/api/opportunities/${opportunityId}/simulate`, {
        method: "POST",
        credentials: "include",
        headers: { "content-type": "application/json", accept: "application/json" },
        signal: AbortSignal.timeout(8000),
      });
      if (res.status === 404) {
        toast.error("Simulate endpoint not yet implemented (backend Sprint TBD)", {
          description: `POST /api/opportunities/${opportunityId}/simulate → 404`,
        });
        return;
      }
      if (!res.ok) {
        toast.error(`Simulation failed: HTTP ${res.status}`);
        return;
      }
      const result = await res.json() as { profit_usd?: number; net_profit_usd?: number };
      const yieldVal = result.net_profit_usd ?? result.profit_usd;
      toast.success("Simulation complete", {
        description: yieldVal != null ? `Net yield: $${yieldVal.toFixed(4)}` : "No yield data returned",
      });
    } catch (e) {
      const err = e as Error;
      if (err.name === "AbortError") {
        toast.error("Simulation timed out after 8s");
      } else {
        toast.error("Simulation error", { description: err.message });
      }
    } finally {
      setSimLoading(null);
    }
  }, [EDGE_URL]);

  // FE-1: fetchOpportunities is now ONLY used by the manual "Force refresh" button.
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
      // Merge snapshot (dedup by route key, preserve age, no flash) via mapper.
      mergeSnapshot(items.map((raw: Record<string, unknown>) => mapToOmniOpportunity(raw)));
      setLastRefresh(new Date());
      setErrorMsg(null);
    } catch (e) {
      setErrorMsg((e as Error).message);
    }
  }, [EDGE_URL, viableOnly, mergeSnapshot]);

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

  useEffect(() => {
    const ticker = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(ticker);
  }, []);

  // FE-1: Opportunities come from Omni-Store (SSOT).
  const viableCount = opportunities.filter((o) => o.status !== "rejected" && o.status !== "failed").length;
  const rejectedCount = opportunities.filter((o) => o.status === "rejected").length;
  // DEV-ONLY preview-seed marker — true when the store holds SAMPLE fixtures
  // (trace_id="preview" from preview-fixtures.ts, seeded by the dev auto-seed
  // in useOmniOpportunities when no edge backend is reachable locally). Drives
  // the "DESIGN PREVIEW" banner so the operator never confuses them with live
  // data. Never true in production (the seed is NODE_ENV-gated dead code).
  const isPreviewMode = opportunities.some((o) => o.trace_id === "preview");

  const isError = feedStatus === 'STALE';

  return (
    <div className={`p-8 min-h-screen transition-colors duration-500 text-foreground ${isError ? 'bg-destructive/5' : ''}`}>
      {isPreviewMode && (
        <div className="mb-6 p-4 bg-info/10 border border-info/40 rounded-xl flex items-center gap-3 text-info">
          <span className="text-xs font-bold uppercase tracking-widest px-2 py-1 rounded bg-info/20 border border-info/50 whitespace-nowrap">
            Design Preview
          </span>
          <div className="text-sm leading-relaxed">
            <span className="font-bold">Sample fixtures shown</span> — no edge backend is reachable
            locally, so populated SAMPLE opportunities are rendered for layout review only
            (<span className="font-mono">trace_id&nbsp;=&nbsp;preview</span>). These are not live
            detections. The instant real data arrives from the edge it replaces these.
          </div>
        </div>
      )}
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

      {/* ── Card grid (2026-07-05 operator request: styled trading-dashboard
            panels, supremely clear). Replaces the dense 10-column table.
            R8 fail-honest preserved: gross/net/convergence/score/status/age all
            surface here; the deep 9-component cost breakdown + target sizing
            remain available via the hover popover on the Net metric and the
            Inspect dialog. Key fix: list key is the STABLE routeKey (not
            opp.id), so re-detecting the same route updates the card in place
            instead of remounting it — no more enter-animation flash every
            poll cycle (see omni-store mergeSnapshot + systematic-debugging). */}
      <div className="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-4">
        <AnimatePresence>
          {opportunities.map((opp) => {
            const detectedTime = new Date(opp.detected_at).getTime();
            const ageSecs = isMounted ? Math.max(0, Math.floor((now - detectedTime) / 1000)) : 0;
            const isStale = ageSecs > 12;
            // R8: risk_score nullable; 0 as fail-safe for triage logic only.
            const scorePercent = Number(opp.risk_score ?? 0) * 100;
            const isCriticalTriage = scorePercent > 95;

            // Net priority: canonical spine → TS simulated (with SIM pill) → "—".
            const gross = formatProfitUSD(opp.expected_profit_usd);
            const canonicalNet = opp.net_expected_profit_usd ?? null;
            const simulatedNet = opp.simulated_net_profit_usd ?? null;
            const netSource: "canonical" | "simulated" | "none" =
              canonicalNet != null ? "canonical"
                : simulatedNet != null ? "simulated"
                : "none";
            const net = formatProfitUSD(canonicalNet ?? simulatedNet);

            const roi = opp.roi_pct;
            const roiTone: "pos" | "neg" | "muted" =
              roi == null ? "muted" : roi > 0 ? "pos" : roi < 0 ? "neg" : "muted";

            // ── PR 6 trade-math cells. Honest: many stay "—" until PR 4/5 land. ──
            const buyQty = formatTokenAmount(
              opp.amount_in_wei,
              opp.token_in_info?.decimals,
              opp.token_in_info?.symbol,
            );
            // Final out: honest ONLY on the SIM path (both legs same forward-sim).
            // Verifier caveat: never mix canonical net with simulated amount_in.
            const simInUsd = opp.simulated_amount_in_usd ?? null;
            const simNetUsd = opp.simulated_net_profit_usd ?? null;
            const endValueUsd: number | null =
              simInUsd != null && simNetUsd != null ? simInUsd + simNetUsd : null;

            // Target PASS/FAIL against the /strategies-applied target.
            const tgt = opp.simulated_target;
            const targetVerdict: { label: string; tone: "pass" | "fail" | "na" } = (() => {
              if (tgt == null) return { label: "no target", tone: "na" as const };
              const infeasible =
                tgt.binding_floor === "roi-unreachable" ||
                tgt.binding_floor === "net-per-usd-nonpositive";
              return tgt.meets_target_at_cap && !infeasible
                ? { label: "PASS", tone: "pass" as const }
                : { label: "FAIL", tone: "fail" as const };
            })();

            return (
              <motion.div
                key={routeKey(opp)}
                initial={{ opacity: 0, y: 8, scale: 0.98 }}
                animate={{ opacity: 1, y: 0, scale: 1 }}
                exit={{ opacity: 0, scale: 0.98 }}
                transition={{ duration: 0.25 }}
                onClick={() => setSelectedOpp(opp)}
                className={`relative bg-card text-card-foreground border rounded-2xl p-4 shadow-lg hover:shadow-xl transition-all cursor-pointer overflow-hidden ${
                  isCriticalTriage ? "border-warning/40" : "border-border hover:border-primary/40"
                }`}
              >
                {isCriticalTriage && (
                  <div className="absolute left-0 top-0 bottom-0 w-1 bg-gradient-to-b from-warning to-success animate-pulse" />
                )}

                {/* ── HEADER: chain + strategy + status + age | ROI% ── */}
                <div className="flex items-center justify-between gap-2 mb-2.5 min-w-0">
                  <div className="flex items-center gap-1.5 flex-wrap min-w-0">
                    <ChainBadge chain_id={opp.chain_id} />
                    <StrategyBadge strategy_kind={opp.strategy_kind} />
                    <StatusPill status={opp.status} rejection_reason={opp.rejection_reason} />
                    {opp.chain_base_token_symbol && (
                      <span className="text-[10px] px-1.5 py-0.5 rounded bg-muted/50 text-muted-foreground/90 border border-border/60 font-mono uppercase tracking-wide">
                        {opp.chain_base_token_symbol}
                      </span>
                    )}
                  </div>
                  <div
                    className={`flex items-center gap-1 text-xs font-mono flex-shrink-0 ${isStale ? "text-destructive" : "text-muted-foreground"}`}
                    title={`Snapshot ${new Date(opp.detected_at).toISOString()}`}
                  >
                    {isStale ? <AlertTriangle size={11} className="animate-pulse" /> : <Clock size={11} />}
                    <span suppressHydrationWarning>{isMounted ? `${ageSecs}s` : "--"}</span>
                  </div>
                </div>

                {/* Token pair + ROI% */}
                <div className="flex items-center justify-between gap-2 mb-3 min-w-0">
                  <div className="flex items-center gap-2 min-w-0">
                    <div className="min-w-0 flex-1">
                      <TokenChip token_address={opp.token_in} info={opp.token_in_info} chain_id={opp.chain_id} />
                    </div>
                    <span className="text-muted-foreground/60 shrink-0" aria-hidden="true">→</span>
                    <div className="min-w-0 flex-1">
                      <TokenChip token_address={opp.token_out} info={opp.token_out_info} chain_id={opp.chain_id_out ?? opp.chain_id} />
                    </div>
                  </div>
                  <div
                    className={`flex items-center gap-1 font-bold text-base whitespace-nowrap ${roiTone === "pos" ? "text-success" : roiTone === "neg" ? "text-destructive" : "text-muted-foreground"}`}
                    title="Net Convergence Ratio (ROI %) — fail-honest '—' when not computed"
                  >
                    {roiTone === "pos" && <TrendingUp size={14} />}
                    {formatPctOrDash(opp.roi_pct)}
                  </div>
                </div>

                {/* ── BLOQUE 1: RESULTADO (executive) ── */}
                <div className="grid grid-cols-2 gap-2 mb-3">
                  <div className="rounded-lg bg-muted/40 p-2">
                    <div className="text-[9px] uppercase tracking-wide text-muted-foreground">Net yield</div>
                    <div className="flex items-center gap-1">
                      <span
                        className={`font-mono text-lg font-bold ${TONE_CLASS[net.tone] ?? "text-muted-foreground"}`}
                        title={netSource === "canonical" ? "Canonical spine net = gross − all costs" : netSource === "simulated" ? "TS forward-sim net (canonical pending)" : "Not yet computed (R8: '—')"}
                      >
                        {netSource === "simulated" ? `~${net.display}` : net.display}
                      </span>
                      {netSource === "simulated" && (
                        <span className="text-[9px] font-bold px-1 rounded bg-info/15 text-info border border-info/40">SIM</span>
                      )}
                    </div>
                  </div>
                  <div className="rounded-lg bg-muted/40 p-2">
                    <div className="text-[9px] uppercase tracking-wide text-muted-foreground">
                      Target · {tgt?.target_source === "strategy_config" ? "/strategies" : tgt?.target_source === "simulation_tab" ? "Sim tab" : "none"}
                    </div>
                    <div className={`font-mono text-lg font-bold ${targetVerdict.tone === "pass" ? "text-success" : targetVerdict.tone === "fail" ? "text-destructive" : "text-muted-foreground"}`}>
                      {targetVerdict.label}
                    </div>
                    {tgt && (
                      <div className="text-[9px] text-muted-foreground font-mono">
                        {tgt.target_net_usd != null && `$${tgt.target_net_usd.toFixed(0)}`}
                        {tgt.target_net_usd != null && tgt.target_roi_pct != null && " · "}
                        {tgt.target_roi_pct != null && `${tgt.target_roi_pct.toFixed(1)}%`}
                      </div>
                    )}
                  </div>
                </div>

                {/* ── BLOQUE 2: TRADE (capital in · buy · sell · final out) ── */}
                <div className="grid grid-cols-4 gap-1.5 mb-2 text-center">
                  <div className="rounded-md bg-muted/30 p-1.5 min-w-0">
                    <div className="text-[8px] uppercase tracking-wide text-muted-foreground">Capital in</div>
                    <div className="font-mono text-[11px] text-foreground truncate" title="simulated_amount_in_usd">
                      {simInUsd != null ? formatUsdShort(simInUsd) : "—"}
                    </div>
                  </div>
                  <div className="rounded-md bg-success/5 border border-success/20 p-1.5 min-w-0">
                    <div className="text-[8px] uppercase tracking-wide text-success truncate" title={opp.dex_a || "—"}>Buy · {opp.dex_a || "—"}</div>
                    <div className="font-mono text-[11px] text-foreground truncate" title={`amount_in: ${buyQty}`}>{buyQty}</div>
                    <div className="font-mono text-[9px] text-muted-foreground italic" title="buy_price_usd pending PR 4 backend wiring">px —</div>
                  </div>
                  <div className="rounded-md bg-destructive/5 border border-destructive/20 p-1.5 min-w-0">
                    <div className="text-[8px] uppercase tracking-wide text-destructive truncate" title={opp.dex_b ?? "single-DEX"}>Sell · {opp.dex_b || "1-leg"}</div>
                    <div className="font-mono text-[11px] text-muted-foreground italic" title="amount_out pending PR 4 backend wiring">qty —</div>
                    <div className="font-mono text-[9px] text-muted-foreground italic" title="sell_price_usd pending PR 4">px —</div>
                  </div>
                  <div className="rounded-md bg-muted/30 p-1.5 min-w-0">
                    <div className="text-[8px] uppercase tracking-wide text-muted-foreground">Final out</div>
                    <div className="font-mono text-[11px] text-foreground truncate" title="capital_in + net (SIM path only; honest when both non-null)">
                      {endValueUsd != null ? `${formatUsdShort(endValueUsd)}` : "—"}
                      {endValueUsd != null && <span className="text-[8px] text-info ml-0.5">sim</span>}
                    </div>
                  </div>
                </div>

                {/* ── BLOQUE 3: PRECIO REAL (token_in · token_out · source · age) ── */}
                <div className="grid grid-cols-2 gap-1.5 mb-2">
                  <div className="rounded-md bg-muted/30 p-1.5 flex items-center justify-between min-w-0">
                    <span className="text-[9px] text-muted-foreground uppercase truncate">{opp.token_in_info?.symbol ?? "token_in"}</span>
                    <span className="font-mono text-[11px] text-muted-foreground italic flex-shrink-0" title="Real price pending PR 5 (Redis arbx:token_prices:&lt;chain&gt;)">—</span>
                  </div>
                  <div className="rounded-md bg-muted/30 p-1.5 flex items-center justify-between min-w-0">
                    <span className="text-[9px] text-muted-foreground uppercase truncate">{opp.token_out_info?.symbol ?? "token_out"}</span>
                    <span className="font-mono text-[11px] text-muted-foreground italic flex-shrink-0" title="Real price pending PR 5">—</span>
                  </div>
                </div>
                <div className="text-[9px] text-muted-foreground/70 italic mb-3 leading-relaxed">
                  Real token prices · buy/sell px · amount_out · final out = <span className="font-mono">pending backend wiring (PR 4/5)</span>. Shown honestly as "—" — never fabricated (RULE 00).
                </div>

                {/* ── EXPANDIBLE: derivation · applied config · route ── */}
                <details
                  className="mb-3 rounded-lg border border-border bg-muted/20"
                  onClick={(e) => e.stopPropagation()}
                  onToggle={(e) => e.stopPropagation()}
                >
                  <summary className="cursor-pointer p-2 text-xs font-semibold text-muted-foreground select-none hover:text-foreground">
                    ▾ How net yield is derived · applied config · route
                  </summary>
                  <div className="p-2 pt-0 space-y-2.5 text-xs">

                    {/* Derivation chain */}
                    <div>
                      <div className="text-[9px] uppercase tracking-wide text-muted-foreground mb-1">How net yield is derived</div>
                      <div className="font-mono space-y-0.5">
                        <div className="flex justify-between">
                          <span>Gross (AMM spread)</span>
                          <span className={TONE_CLASS[gross.tone] ?? "text-muted-foreground"}>{gross.display}</span>
                        </div>
                        {opp.simulated_cost_breakdown ? (
                          ([
                            ["− Gas", opp.simulated_cost_breakdown.gas_usd],
                            ["− LP fees (30bps proxy)", opp.simulated_cost_breakdown.lp_fees_usd],
                            ["− Slippage", opp.simulated_cost_breakdown.slippage_usd],
                            ["− Flash convergence fee", opp.simulated_cost_breakdown.flashloan_fee_usd],
                            ["− Relay fee", opp.simulated_cost_breakdown.relay_fee_usd],
                            ["− Capital cost", opp.simulated_cost_breakdown.capital_cost_usd],
                            ["− Failure buffer", opp.simulated_cost_breakdown.failure_buffer_usd],
                            ["− Copied buffer", opp.simulated_cost_breakdown.copied_buffer_usd],
                            ["− Ops overhead", opp.simulated_cost_breakdown.ops_overhead_usd],
                          ] as Array<[string, number]>).map(([label, value]) => (
                            <div key={label} className="flex justify-between text-muted-foreground">
                              <span>{label}</span>
                              <span>−${value.toFixed(4)}</span>
                            </div>
                          ))
                        ) : (
                          <div className="text-muted-foreground/60 italic">
                            Fee breakdown: pending revm sim (canonical spine persists aggregate net only — mig 049).
                          </div>
                        )}
                        <div className="flex justify-between border-t border-border pt-0.5 font-bold">
                          <span>= Net yield{netSource === "simulated" ? " (SIM)" : ""}</span>
                          <span className={TONE_CLASS[net.tone] ?? "text-muted-foreground"}>
                            {netSource === "simulated" ? `~${net.display}` : net.display}
                          </span>
                        </div>
                      </div>
                    </div>

                    {/* Applied config */}
                    <div>
                      <div className="text-[9px] uppercase tracking-wide text-muted-foreground mb-1">Applied config (from /strategies)</div>
                      {tgt ? (
                        <div className="font-mono space-y-0.5">
                          <div className="flex justify-between"><span>source</span><span>{tgt.target_source === "strategy_config" ? "/strategies" : "Sim tab"}</span></div>
                          {tgt.target_net_usd != null && (
                            <div className="flex justify-between"><span>min net USD</span><span>${tgt.target_net_usd.toFixed(2)}</span></div>
                          )}
                          {tgt.target_roi_pct != null && (
                            <div className="flex justify-between"><span>min ROI %</span><span>{tgt.target_roi_pct.toFixed(2)}%</span></div>
                          )}
                          <div className="flex justify-between">
                            <span>binding floor</span>
                            <span className={tgt.binding_floor === "roi-unreachable" || tgt.binding_floor === "net-per-usd-nonpositive" ? "text-destructive" : "text-foreground"}>
                              {tgt.binding_floor}
                            </span>
                          </div>
                          <div className="flex justify-between">
                            <span>suggested borrow</span>
                            <span>{Number.isFinite(tgt.suggested_amount_in_usd) ? formatUsdShort(tgt.suggested_amount_in_usd) : "—"}</span>
                          </div>
                        </div>
                      ) : (
                        <div className="text-muted-foreground/60 italic">No target applied (inverse-sizing not run for this row).</div>
                      )}
                    </div>

                    {/* Technical route */}
                    <div>
                      <div className="text-[9px] uppercase tracking-wide text-muted-foreground mb-1">Technical route</div>
                      <div className="font-mono space-y-0.5 break-all">
                        <div>
                          chain {opp.chain_id}
                          {opp.chain_id_out != null && opp.chain_id_out !== opp.chain_id ? ` → ${opp.chain_id_out}` : ""} · block {opp.block_number ?? "—"}
                        </div>
                        <div className="text-muted-foreground">in&nbsp; {opp.token_in}</div>
                        <div className="text-muted-foreground">out {opp.token_out}</div>
                        <div>{opp.dex_a}{opp.dex_b ? ` → ${opp.dex_b}` : " · single-DEX"}</div>
                        <div className="text-muted-foreground">trace {opp.trace_id || "—"}</div>
                      </div>
                    </div>
                  </div>
                </details>

                {/* ── ACTION ── */}
                <button
                  type="button"
                  onClick={(e) => { e.stopPropagation(); setSelectedOpp(opp); }}
                  className="w-full py-2 rounded-lg bg-primary text-primary-foreground text-xs font-bold uppercase tracking-wide hover:bg-primary/90 active:scale-[0.99] transition-all"
                >
                  Inspect details
                </button>
              </motion.div>
            );
          })}

        </AnimatePresence>
        {opportunities.length === 0 && (
          <div className="col-span-full p-8 text-center text-muted-foreground italic border border-dashed border-border rounded-2xl">
            No opportunities detected. Searcher scanning mempool…
          </div>
        )}
      </div>

      {/* FE-10: Opportunity detail sheet — click any row to open */}
      <OpportunityDetailDialog
        opportunity={selectedOpp}
        onClose={() => setSelectedOpp(null)}
      />
    </div>
  );
}
