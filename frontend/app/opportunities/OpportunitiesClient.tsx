"use client";
import React, { useEffect, useState, useCallback, startTransition, useRef } from "react";
import { Zap, WifiOff, ShieldAlert, RefreshCw, Radio, Clock, AlertTriangle, EyeOff, Eye } from "lucide-react";
import { useOpportunitiesStream } from "@/lib/hooks/useOpportunitiesStream";
import { toast } from "sonner";
import { OpportunityDetailDialog, type OpportunityDetail } from "@/components/OpportunityDetailDialog";
import { motion, AnimatePresence } from "framer-motion";

// ─── Local type mirrors ───────────────────────────────────────────────────────
// @arbx/shared is a VPS-only package (not installed in local/CI node_modules).
// We mirror the exact subset of OpportunityListItem needed here, matching
// shared-ts/src/api-contracts.ts exactly. No cross-package import needed.

/** Mirrors TokenInfoSchema from shared-ts/src/api-contracts.ts. */
interface TokenInfo {
  symbol: string | null;
  decimals: number | null;
  logo_url: string | null;
  resolved_via: "onchain_full" | "onchain_partial" | "trustwallet_only" | "failed";
}

/** Mirrors StrategyKind from shared-ts/src/contracts/index.ts. */
type StrategyKind =
  | "dex_arb"
  | "triangular"
  | "backrun"
  | "liquidation"
  | "flashloan_arb";

/** Mirrors StatusSchema from shared-ts/src/api-contracts.ts. */
type OpportunityStatus =
  | "detected"
  | "validated"
  | "simulated"
  | "scored"
  | "executing"
  | "executed"
  | "reconciled"
  | "rejected"
  | "failed";

/**
 * Mirrors OpportunityListItemSchema from shared-ts/src/api-contracts.ts.
 * All nullable fields per R8 fail-honest semantics.
 */
interface OpportunityListItem {
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

// ─── Component imports (Tasks 10 / 11) ───────────────────────────────────────
import { TokenChip } from "@/components/TokenChip";
import { StrategyBadge } from "@/components/StrategyBadge";
import { StatusPill } from "@/components/StatusPill";
import { CrossChainSlot } from "@/components/CrossChainSlot";
import {
  formatProfitUSD,
  formatPctOrDash,
  formatRiskOrDash,
} from "@/lib/format";
import { useUserPrefs } from "@/lib/user-prefs";

// ─── Tone → token-based class map (used for PROFIT cell) ─────────────────────
const TONE_CLASS: Record<string, string> = {
  positive: "text-success",
  negative: "text-destructive",
  zero:     "text-muted-foreground",
  neutral:  "text-muted-foreground",
  pending:  "text-muted-foreground/60 italic",
};

// FE-1: WS statuses. "LIVE" = WS connected. "STALE" = WS disconnected.
// "POLLING" = WS failed 3×, degraded to HTTP polling. "CONNECTING" = initial.
// HTTP fetch errors (manual refresh) surface via errorMsg, not feedStatus.
type FeedStatus = "POLLING" | "LIVE" | "STALE" | "CONNECTING";

const POLL_INTERVAL_MS = 4_000;

export type OpportunitiesSnapshot = {
  opportunities: OpportunityListItem[];
  serverTime: string | null;
  source: string;
};

export default function OpportunitiesClient({
  initialSnapshot,
}: {
  initialSnapshot: OpportunitiesSnapshot;
}) {
  const [snapshot, setSnapshot] = useState<OpportunitiesSnapshot>(initialSnapshot);
  const [isMounted, setIsMounted] = useState(false);
  const [errorMsg, setErrorMsg] = useState<string | null>(null);
  const [now, setNow] = useState<number>(0);
  // R1: viableOnly initialises to true (deterministic SSR-safe value).
  // localStorage read happens in useEffect — never in render.
  const [viableOnly, setViableOnly] = useState(true);
  const [simLoading, setSimLoading] = useState<string | null>(null);
  const [selectedOpp, setSelectedOpp] = useState<OpportunityDetail | null>(null);

  // FE-6: Track IDs already notified to avoid duplicate toasts across polls.
  // R1: useRef is SSR-safe — no access to window or localStorage.
  const seenNotifiedIds = useRef<Set<string>>(new Set());

  // FE-13: Read notification threshold from user prefs (localStorage, R1 compliant).
  const { prefs } = useUserPrefs();

  const EDGE_URL = process.env.NEXT_PUBLIC_EDGE_URL ?? "http://localhost:8787";

  // FE-1: WebSocket stream. R1 compliant — hook runs WS inside useEffect only.
  // When WS fails 3×, hook auto-degrades to HTTP polling at 4s.
  const { opportunities: streamOpportunities, wsStatus } = useOpportunitiesStream(
    initialSnapshot.opportunities,
    EDGE_URL,
  );

  // Derive feedStatus from wsStatus for display. "POLLING" is the degraded
  // HTTP-fallback state emitted by the hook after 3 WS failures.
  const feedStatus: FeedStatus = wsStatus;

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
      const profit = result.net_profit_usd ?? result.profit_usd;
      toast.success("Simulation complete", {
        description: profit != null ? `Net profit: $${profit.toFixed(4)}` : "No profit data returned",
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
  // Automatic data flow comes from the WS stream (useOpportunitiesStream).
  // On fetch error, we set errorMsg for the error banner — feedStatus is derived
  // from wsStatus, not from this fetch path.
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
      startTransition(() => {
        setSnapshot({
          opportunities: Array.isArray(data?.items) ? data.items : Array.isArray(data) ? data : [],
          serverTime: new Date().toISOString(),
          source: "client-rest-manual",
        });
      });
      setErrorMsg(null);
    } catch (e) {
      setErrorMsg((e as Error).message);
    }
  }, [EDGE_URL, viableOnly]);

  // R1: localStorage read happens here — never during render (SSR has no localStorage).
  useEffect(() => {
    const stored = localStorage.getItem("arbx-opps-viable-only");
    if (stored === "false") setViableOnly(false);
  }, []);

  const onToggleViableOnly = useCallback((newValue: boolean) => {
    setViableOnly(newValue);
    localStorage.setItem("arbx-opps-viable-only", String(newValue));
  }, []);

  // FE-6: Fire a toast for every new opportunity that clears the threshold.
  // R1: streamOpportunities is populated only after mount (WS or polling).
  // seenNotifiedIds persists across re-renders via useRef so we never
  // re-toast the same opportunity across WS reconnects or poll cycles.
  useEffect(() => {
    if (!isMounted) return;
    for (const opp of streamOpportunities) {
      if (seenNotifiedIds.current.has(opp.id)) continue;
      seenNotifiedIds.current.add(opp.id);
      const profit = opp.expected_profit_usd ?? 0;
      if (profit >= prefs.notification_threshold_usd) {
        toast.success(`High-value opportunity — ${opp.strategy_kind}`, {
          description: `Net profit $${profit.toFixed(2)} · chain ${opp.chain_id} · ${opp.dex_a}${opp.dex_b ? ` → ${opp.dex_b}` : ""}`,
          duration: 8_000,
        });
      }
    }
  }, [streamOpportunities, isMounted, prefs.notification_threshold_usd]);

  // R1: setIsMounted + setNow are the only non-WS side effects needed here.
  // Polling interval is now managed inside useOpportunitiesStream.
  useEffect(() => {
    setIsMounted(true);
    setNow(Date.now());
  }, []);

  useEffect(() => {
    const ticker = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(ticker);
  }, []);

  // FE-1: Opportunities come from the WS stream (streamOpportunities).
  // snapshot is kept only for the manual Force Refresh button fallback path.
  const opportunities = streamOpportunities;
  const lastRefresh = snapshot.serverTime ? new Date(snapshot.serverTime) : null;
  const viableCount = opportunities.filter((o) => o.status !== "rejected" && o.status !== "failed").length;
  const rejectedCount = opportunities.filter((o) => o.status === "rejected").length;

  const isError = feedStatus === 'STALE';

  return (
    <div className={`p-8 min-h-screen transition-colors duration-500 text-foreground ${isError ? 'bg-destructive/5' : ''}`}>
      <div className="flex justify-between items-center border-b border-border pb-4 mb-8">
        <div>
          <h1 className={`text-4xl font-extrabold tracking-tight bg-clip-text text-transparent ${isError ? 'bg-gradient-to-r from-destructive to-destructive/70' : 'bg-gradient-to-r from-primary to-success'}`}>
            Live MEV Feed
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
            aria-pressed={viableOnly}
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
                : "Searcher-rs is actively hunting for arbitrage routes. Opportunities will appear here instantly."}
            </p>
          </div>
        </div>
      )}

      <div data-slot="card" className="bg-card text-card-foreground border border-border rounded-2xl shadow-2xl overflow-hidden">
        <table className="w-full text-left border-collapse">
          <thead>
            <tr className="bg-muted text-muted-foreground text-sm uppercase tracking-wider">
              <th className="p-4 border-b border-border">Age / Time</th>
              <th className="p-4 border-b border-border">Route</th>
              <th className="p-4 border-b border-border">Status</th>
              <th className="p-4 border-b border-border text-right">Net Profit (USD)</th>
              <th className="p-4 border-b border-border text-right">Net ROI</th>
              <th className="p-4 border-b border-border text-center">Score</th>
              <th className="p-4 border-b border-border text-center">Action</th>
            </tr>
          </thead>
          <tbody>
            <AnimatePresence>
              {opportunities.map((opp) => {
                const detectedTime = new Date(opp.detected_at).getTime();
                const ageSecs = isMounted ? Math.floor((now - detectedTime) / 1000) : 0;
                const isStale = ageSecs > 12;
                // R8: risk_score is nullable. Use 0 as fail-safe for triage logic only
                // (null → not critical triage, which is the safe direction).
                const scorePercent = Number(opp.risk_score ?? 0) * 100;
                const isCriticalTriage = scorePercent > 95;

                // Format profit with R8-compliant helper (null → "—").
                const profit = formatProfitUSD(opp.expected_profit_usd);

                return (
                  <motion.tr
                    key={opp.id}
                    initial={{ opacity: 0, x: -20, backgroundColor: "oklch(0.78 0.13 215 / 0.2)" }}
                    animate={{ opacity: 1, x: 0, backgroundColor: isCriticalTriage ? "oklch(0.82 0.14 75 / 0.05)" : "transparent" }}
                    exit={{ opacity: 0 }}
                    transition={{ duration: 0.5 }}
                    onClick={() => setSelectedOpp(opp)}
                    className={`border-b hover:bg-muted/40 transition-all cursor-pointer ${isCriticalTriage ? 'border-warning/30 relative' : 'border-border/50'}`}
                  >
                    {/* ── AGE / TIME column ── */}
                    <td className="p-4 font-mono text-xs">
                      {isCriticalTriage && (
                        <div className="absolute left-0 top-0 bottom-0 w-1 bg-gradient-to-b from-warning to-success animate-pulse"></div>
                      )}
                      <div className="flex flex-col gap-1">
                        <div className={`flex items-center gap-1.5 font-bold ${isStale ? 'text-destructive' : 'text-success'}`}>
                          {isStale ? <AlertTriangle size={12} className="animate-pulse" /> : <Clock size={12} />}
                          <span suppressHydrationWarning>{isMounted ? `${ageSecs}s ago` : '--'}</span>
                        </div>
                        <div className="text-muted-foreground" suppressHydrationWarning>
                          {isMounted ? new Date(opp.detected_at).toLocaleTimeString([], { hour12: false, hour: '2-digit', minute: '2-digit', second: '2-digit' }) : '--:--:--'}
                        </div>
                      </div>
                    </td>

                    {/* ── ROUTE column — TokenChip + StrategyBadge + CrossChainSlot + DEX path ── */}
                    <td className="p-4" data-status={opp.status}>
                      <div className="flex flex-col gap-1.5">
                        {/* Token pair with rich metadata */}
                        <div className="flex items-center gap-1.5">
                          <TokenChip
                            token_address={opp.token_in}
                            info={opp.token_in_info}
                            chain_id={opp.chain_id}
                          />
                          <span className="text-muted-foreground/60 text-xs" aria-hidden="true">→</span>
                          <TokenChip
                            token_address={opp.token_out}
                            info={opp.token_out_info}
                            chain_id={opp.chain_id_out ?? opp.chain_id}
                          />
                        </div>
                        {/* Strategy badge + DEX path */}
                        <div className="flex items-center gap-2 flex-wrap">
                          <StrategyBadge strategy_kind={opp.strategy_kind} />
                          <span className="text-xs font-mono text-primary">
                            {opp.dex_a}
                            {opp.dex_b != null && (
                              <><span className="text-muted-foreground"> → </span>{opp.dex_b}</>
                            )}
                          </span>
                        </div>
                        {/* Cross-chain slot — renders null for single-chain opps */}
                        <CrossChainSlot opp={opp} />
                      </div>
                    </td>

                    {/* ── STATUS column — StatusPill with rejection_reason tooltip + visible badge ── */}
                    <td className="p-4">
                      <StatusPill
                        status={opp.status}
                        rejection_reason={opp.rejection_reason}
                      />
                      {/* R8 fail-honest: show rejection reason as visible badge in show-all mode */}
                      {!viableOnly && opp.rejection_reason && (
                        <span className="mt-1 flex items-center gap-1 text-xs text-destructive font-mono">
                          <span className="inline-block w-1 h-1 rounded-full bg-destructive flex-shrink-0" aria-hidden="true" />
                          {opp.rejection_reason}
                        </span>
                      )}
                    </td>

                    {/* ── NET PROFIT column — R8 fail-honest via formatProfitUSD ── */}
                    <td className="p-4 text-right" data-col="profit">
                      <div className="group relative inline-block cursor-help">
                        <span className={`font-mono font-bold text-base drop-shadow-md border-b border-dashed border-current/30 ${TONE_CLASS[profit.tone] ?? 'text-muted-foreground'}`}>
                          {profit.display}
                        </span>
                        {/* Tooltip: only rendered when profit data is present */}
                        {opp.expected_profit_usd != null && (
                          <div data-slot="popover-content" className="absolute bottom-full right-0 mb-2 w-64 p-3 bg-popover text-popover-foreground border border-border rounded-lg shadow-xl opacity-0 group-hover:opacity-100 transition-opacity pointer-events-none z-10 text-left">
                            <div className="text-xs font-sans">
                              <div className="flex justify-between border-b border-border pb-1 mb-1">
                                <span>Ganancia Neta (Est):</span>
                                <span className={`font-mono ${TONE_CLASS[profit.tone] ?? 'text-muted-foreground'}`}>{profit.display}</span>
                              </div>
                              {opp.bridge_fee_usd != null && (
                                <div className="flex justify-between text-muted-foreground">
                                  <span>Bridge Fee:</span>
                                  <span className="font-mono">${opp.bridge_fee_usd.toFixed(4)}</span>
                                </div>
                              )}
                              <div className="flex justify-between text-muted-foreground/70">
                                <span>Desglose de Gas:</span>
                                <span className="italic">Pendiente Sim.</span>
                              </div>
                              <div className="flex justify-between text-muted-foreground/70">
                                <span>Bribe (MEV):</span>
                                <span className="italic">Pendiente Sim.</span>
                              </div>
                            </div>
                          </div>
                        )}
                      </div>
                    </td>

                    {/* ── NET ROI column — R8 fail-honest via formatPctOrDash ── */}
                    <td className="p-4 text-right font-mono text-foreground" data-col="roi">
                      <span className="bg-muted/50 px-2 py-1 rounded border border-border">
                        {formatPctOrDash(opp.roi_pct)}
                      </span>
                    </td>

                    {/* ── SCORE column — R8 fail-honest via formatRiskOrDash ── */}
                    <td className="p-4 text-center" data-col="risk">
                      <span className={`px-3 py-1 rounded-full text-xs font-bold border ${scorePercent > 95 ? 'bg-warning/20 text-warning border-warning/50 animate-pulse' : scorePercent > 70 ? 'bg-success/10 text-success border-success/30' : 'bg-info/10 text-info border-info/30'}`}>
                        {formatRiskOrDash(opp.risk_score)}
                      </span>
                    </td>

                    {/* ── ACTION column ── */}
                    <td className="p-4 text-center">
                      <button
                        type="button"
                        disabled={simLoading === opp.id}
                        onClick={(e) => { e.stopPropagation(); handleSimulate(opp.id); }}
                        className={`px-4 py-1.5 rounded text-xs font-bold transition-colors shadow-lg disabled:opacity-50 disabled:cursor-not-allowed ${isCriticalTriage ? 'bg-warning text-warning-foreground hover:bg-warning/90' : 'bg-primary text-primary-foreground hover:bg-primary/90'}`}
                        title="POST /api/opportunities/:id/simulate — backend not yet implemented"
                      >
                        {simLoading === opp.id ? "SIMULATING…" : "SIMULATE"}
                      </button>
                    </td>
                  </motion.tr>
                );
              })}
            </AnimatePresence>
            {opportunities.length === 0 && (
              <tr>
                <td colSpan={7} className="p-8 text-center text-muted-foreground italic">No opportunities detected. Searcher scanning mempool...</td>
              </tr>
            )}
          </tbody>
        </table>
      </div>

      {/* FE-10: Opportunity detail sheet — click any row to open */}
      <OpportunityDetailDialog
        opportunity={selectedOpp}
        onClose={() => setSelectedOpp(null)}
      />
    </div>
  );
}
