"use client";
import React, { useEffect, useState, useCallback, useRef } from "react";
import { Zap, WifiOff, ShieldAlert, RefreshCw, Radio, EyeOff, Eye, AlertTriangle, Clock } from "lucide-react";
import { sanitizeForDisplay } from "@/lib/omega-lexicon";
import { toast } from "sonner";
import { OpportunityDetailDialog, type OpportunityDetail } from "@/components/OpportunityDetailDialog";
import { motion, AnimatePresence } from "framer-motion";

// ─── Omni-Store Integration ───────────────────────────────────────────────────
import { useOmniOpportunities } from "@/lib/store/useOmniOpportunities";
import { useOmniStore } from "@/lib/store/omni-store";
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
import { DexPath } from "@/components/DexPath";
import { StrategyBadge } from "@/components/StrategyBadge";
import { StatusPill } from "@/components/StatusPill";
import { CrossChainSlot } from "@/components/CrossChainSlot";
import { DegradedBanner } from "@/components/DegradedBanner";
import {
  formatProfitUSD,
  formatPctOrDash,
  formatRiskOrDash,
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
  // FASE OMEGA — Filtro Hamiltonian: solo oportunidades detectadas por cartuchos
  const [hamiltonianOnly, setHamiltonianOnly] = useState(false);

  useOmniOpportunities({
    edgeUrl: EDGE_URL,
    viableOnly,
    initialOpportunities: initialSnapshot.opportunities,
  });

  // Selectors from Omni-Store (SSOT)
  const opportunities = useOmniStore((state) => state.opportunities);
  const wsStatus = useOmniStore((state) => state.wsStatus);
  const clearOpportunities = useOmniStore((state) => state.clearOpportunities);
  const addOpportunity = useOmniStore((state) => state.addOpportunity);

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

  // FASE OMEGA — EXECUTE handler: Envía oportunidad al Execution Core
  const handleExecute = useCallback(async (opp: OmniOpportunity) => {
    if (!opp.hamiltonian_detected || !opp.cartridge_id) {
      toast.error("Execution rejected", {
        description: "Only Hamiltonian-detected opportunities with cartridge binding can be executed",
      });
      return;
    }

    if (!opp.cartridge_confidence || opp.cartridge_confidence < 0.7) {
      toast.error("Confidence too low", {
        description: `Cartridge confidence ${(opp.cartridge_confidence ?? 0).toFixed(2)} < 0.70 threshold`,
      });
      return;
    }

    setSimLoading(opp.id);
    try {
      // Construir ExecutionRequest según contrato Backend
      const executionRequest = {
        opportunity_id: opp.id,
        tx_hash: opp.trace_id, // Usamos trace_id como tx_hash de referencia
        cartridge_id: opp.cartridge_id,
        route: [
          {
            step_type: "flash_loan" as const,
            protocol: "aave_v3",
            target: "0x87870Bca3F3fD6335C3F4ce8392D69350B4fA4E2",
            token_in: opp.token_in,
            token_out: opp.token_out,
            amount: opp.amount_in_wei,
          },
          {
            step_type: "swap" as const,
            protocol: opp.dex_a,
            target: "0xE592427A0AEce92De3Edee1F18E0157C05861564",
            token_in: opp.token_in,
            token_out: opp.token_out,
            amount: opp.amount_in_wei,
          },
        ],
        simulation_params: {
          amount_in: opp.amount_in_wei,
          expected_out: Math.floor((opp.expected_profit_usd ?? 0) * 1e6).toString(),
          min_out: Math.floor(((opp.expected_profit_usd ?? 0) * 0.95) * 1e6).toString(),
          slippage_bps: 50, // 0.5% slippage tolerance
        },
        client_meta: {
          source: "dapp_opportunities_client",
          timestamp_ms: Date.now(),
        },
      };

      const res = await fetch(`${EDGE_URL}/api/v1/execution`, {
        method: "POST",
        credentials: "include",
        headers: {
          "content-type": "application/json",
          accept: "application/json",
        },
        body: JSON.stringify(executionRequest),
        signal: AbortSignal.timeout(10000),
      });

      if (res.status === 202) {
        const result = await res.json() as { execution_id: string; status: string; estimated_start_ms: number };
        toast.success("Execution queued", {
          description: `ID: ${result.execution_id.slice(0, 16)}... Start in ~${result.estimated_start_ms}ms`,
        });
      } else if (res.status === 409) {
        toast.warning("Already executing", {
          description: "This opportunity is already in the execution queue",
        });
      } else if (res.status === 400) {
        const err = await res.json();
        toast.error("Validation failed", { description: err.error || "Invalid request" });
      } else {
        toast.error(`Execution failed: HTTP ${res.status}`);
      }
    } catch (e) {
      const err = e as Error;
      if (err.name === "AbortError") {
        toast.error("Execution timed out after 10s");
      } else {
        toast.error("Execution error", { description: err.message });
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
      // Clear and repopulate store via mapper
      clearOpportunities();
      items.forEach((raw: Record<string, unknown>) => {
        addOpportunity(mapToOmniOpportunity(raw));
      });
      setLastRefresh(new Date());
      setErrorMsg(null);
    } catch (e) {
      setErrorMsg((e as Error).message);
    }
  }, [EDGE_URL, viableOnly, clearOpportunities, addOpportunity]);

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

  // FASE OMEGA — Filtrar por detección Hamiltonian si está activado
  const displayedOpportunities = hamiltonianOnly
    ? opportunities.filter((o) => o.hamiltonian_detected)
    : opportunities;
  const hamiltonianCount = opportunities.filter((o) => o.hamiltonian_detected).length;

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
          {/* FASE OMEGA — Hamiltonian-only toggle */}
          <button
            type="button"
            onClick={() => setHamiltonianOnly(!hamiltonianOnly)}
            className={`flex items-center gap-2 px-3 py-1.5 rounded-lg border text-xs font-semibold transition-colors ${
              hamiltonianOnly
                ? "bg-primary/20 border-primary/50 text-primary hover:bg-primary/30"
                : "bg-muted/50 border-border text-muted-foreground hover:bg-muted"
            }`}
            title={hamiltonianOnly ? "Showing Hamiltonian-detected only — click to show all" : "Showing all — click to show only Hamiltonian-detected"}
            aria-pressed={hamiltonianOnly ? "true" : "false"}
          >
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"
              className={hamiltonianOnly ? "text-primary" : ""}
            >
              <circle cx="12" cy="12" r="3"/>
              <path d="M12 2v4M12 18v4M4.93 4.93l2.83 2.83M16.24 16.24l2.83 2.83M2 12h4M18 12h4M4.93 19.07l2.83-2.83M16.24 7.76l2.83-2.83"/>
            </svg>
            <span>{hamiltonianOnly ? `Hamiltonian (${hamiltonianCount})` : `All sources (${opportunities.length})`}</span>
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

      <div data-slot="card" className="bg-card text-card-foreground border border-border rounded-2xl shadow-2xl overflow-hidden">
        <table className="w-full text-left border-collapse">
          <thead>
            <tr className="bg-muted text-muted-foreground text-sm uppercase tracking-wider">
              <th className="p-4 border-b border-border">Age / Time</th>
              <th className="p-4 border-b border-border">Route</th>
              <th className="p-4 border-b border-border">Status</th>
              {/* C5 (audit 2026-05-10): split Gross vs Net columns. Net is the
                  honest after-cost number; Gross is the AMM-quoted swap delta. */}
              <th className="p-4 border-b border-border text-right" title="Gross yield before gas/slippage/relay fees (AMM quote)">Gross (USD)</th>
              <th className="p-4 border-b border-border text-right" title="Net yield after gas, slippage, relay fee, flash convergence fee, failure buffer">Net Yield (USD)</th>
              <th className="p-4 border-b border-border text-right">Net Convergence Ratio</th>
              <th className="p-4 border-b border-border text-center">Score</th>
              <th className="p-4 border-b border-border text-center">Confidence</th>
              <th className="p-4 border-b border-border text-right">Gas Used</th>
              <th className="p-4 border-b border-border text-center">Action</th>
            </tr>
          </thead>
          <tbody>
            <AnimatePresence>
              {displayedOpportunities.map((opp) => {
                const detectedTime = new Date(opp.detected_at).getTime();
                const ageSecs = isMounted ? Math.floor((now - detectedTime) / 1000) : 0;
                const isStale = ageSecs > 12;
                // R8: risk_score is nullable. Use 0 as fail-safe for triage logic only
                // (null → not critical triage, which is the safe direction).
                const scorePercent = Number(opp.risk_score ?? 0) * 100;
                const isCriticalTriage = scorePercent > 95;

                // C5 fix (audit 2026-05-10): split GROSS vs NET cells. Gross
                // is the AMM-quoted swap delta (expected_profit_usd); Net is
                // post-cost (net_expected_profit_usd). R8: both can be null.
                //
                // 2026-05-10 target-driven simulation extension: when the
                // canonical Rust spine has not yet written net (cold-start,
                // gate-rejected before math), the api-server's TS forward
                // simulator may have emitted `simulated_net_profit_usd`.
                // Display priority for the Net column:
                //   1. canonical net (spine output, plain rendering)
                //   2. simulated net (with [SIM] info pill + tilde prefix)
                //   3. "—" (truly unknown)
                // The simulated number is honestly labelled — never confused
                // with the canonical truth.
                const gross = formatProfitUSD(opp.expected_profit_usd);
                const canonicalNet = opp.net_expected_profit_usd ?? null;
                const simulatedNet = opp.simulated_net_profit_usd ?? null;
                const netSource: "canonical" | "simulated" | "none" =
                  canonicalNet != null ? "canonical"
                    : simulatedNet != null ? "simulated"
                    : "none";
                const netValue = canonicalNet ?? simulatedNet;
                const net = formatProfitUSD(netValue);

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

                    {/* ── ROUTE column ──
                          New layout (2026-05-10 operator request):
                            Row 1: ChainBadge(s) — single chain or in→out badges for cross-chain
                            Row 2: TokenChip in  → TokenChip out  (symbol stacked on truncated addr)
                            Row 3: StrategyBadge + DEX path
                            Row 4: CrossChainSlot (extra metadata for bridges — null for single chain)
                       */}
                    <td className="p-4 align-top" data-status={opp.status}>
                      <div className="flex flex-col gap-2">
                        {/* Chain identity row.
                            2026-05-10 operator request: discreet base-token
                            badge ("WETH", "USDC", "MATIC", ...) sits next to
                            the chain pill so the operator can see at a glance
                            which token funds the leg on this chain.
                            chain_base_token_symbol is null when no
                            trading_config snapshot is available — we hide the
                            badge in that case (no fabricated default). */}
                        {/* 2026-05-11 operator request: StrategyBadge moves
                            to the chain-identity line so the strategy name
                            sits next to the chain + base-token badges,
                            mirroring how the operator reads the row top-down
                            (what chain + what base token + what strategy). */}
                        <div className="flex items-center gap-1.5 flex-wrap">
                          <ChainBadge chain_id={opp.chain_id} withId />
                          {opp.chain_base_token_symbol && (
                            <span
                              className="text-[10px] px-1.5 py-0.5 rounded bg-muted/50 text-muted-foreground/90 border border-border/60 font-mono uppercase tracking-wide"
                              title={`Base token for chain ${opp.chain_id} — starts/ends in ${opp.chain_base_token_symbol} (operator-configured)`}
                            >
                              base: {opp.chain_base_token_symbol}
                            </span>
                          )}
                          {opp.chain_id_out != null && opp.chain_id_out !== opp.chain_id && (
                            <>
                              <span className="text-muted-foreground/50 text-xs" aria-hidden="true">→</span>
                              <ChainBadge chain_id={opp.chain_id_out} withId />
                            </>
                          )}
                          <StrategyBadge strategy_kind={opp.strategy_kind} />
                        </div>
                        {/* Token pair: symbol (bold) + truncated address (mono, muted) per side */}
                        <div className="flex items-start gap-2 min-w-0">
                          <div className="min-w-0 flex-1">
                            <TokenChip
                              token_address={opp.token_in}
                              info={opp.token_in_info}
                              chain_id={opp.chain_id}
                            />
                          </div>
                          <span
                            className="text-muted-foreground/60 text-base leading-6 shrink-0 px-1"
                            aria-hidden="true"
                          >
                            →
                          </span>
                          <div className="min-w-0 flex-1">
                            <TokenChip
                              token_address={opp.token_out}
                              info={opp.token_out_info}
                              chain_id={opp.chain_id_out ?? opp.chain_id}
                            />
                          </div>
                        </div>
                        {/* DEX route with BUY/SELL/VIA/TARGET semantics.
                            StrategyBadge moved up to the chain-identity line
                            on 2026-05-11; this row now carries only the
                            BUY/SELL/VIA path. */}
                        <div className="flex items-center gap-2 flex-wrap pt-0.5">
                          <DexPath
                            strategy_kind={opp.strategy_kind}
                            dex_a={opp.dex_a}
                            dex_b={opp.dex_b}
                          />
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
                          {sanitizeForDisplay(opp.rejection_reason)}
                        </span>
                      )}
                    </td>

                    {/* ── GROSS column (audit 2026-05-10 C5 fix) — pre-cost AMM quote */}
                    <td className="p-4 text-right" data-col="gross">
                      <span
                        className="font-mono text-sm text-muted-foreground"
                        title="Gross yield = expected_amount_out − amount_in (USD). Pre-gas, pre-slippage, pre-relay-fee. R8: '—' when not yet computed."
                      >
                        {gross.display}
                      </span>
                    </td>

                    {/* ── NET YIELD column — R8 fail-honest, post-cost truth.
                          Priority: canonical spine net → TS simulated net (with
                          [SIM] info pill) → "—". Sub-line: target-driven
                          inverse sizing hint when the operator has configured
                          a target via /strategies card or the Simulación tab. */}
                    <td className="p-4 text-right" data-col="yield">
                      <div className="group relative inline-block cursor-help">
                        <div className="flex flex-col items-end gap-0.5">
                          <div className="flex items-center gap-1.5 justify-end">
                            <span
                              className={`font-mono font-bold text-base drop-shadow-md border-b border-dashed border-current/30 ${TONE_CLASS[net.tone] ?? 'text-muted-foreground'}`}
                              title={
                                netSource === "canonical"
                                  ? "Net yield (canonical spine output) = gross − gas − slippage − relay fee − flash convergence fee − failure buffer."
                                  : netSource === "simulated"
                                  ? "Net yield (TS forward simulation) — operator's trading_config applied at the row's recorded amount_in. Canonical spine value not yet available."
                                  : "Net yield not yet computed — neither spine output nor simulator could produce a number. R8 fail-honest: '—'."
                              }
                            >
                              {netSource === "simulated" ? `~${net.display}` : net.display}
                            </span>
                            {netSource === "simulated" && (
                              <span
                                className="text-[9px] font-bold px-1.5 py-0.5 rounded bg-info/15 text-info border border-info/40 uppercase tracking-wider"
                                title="Forward simulation — canonical spine net pending"
                              >
                                SIM
                              </span>
                            )}
                          </div>
                          {/* Target-driven sizing hint — operator-configured
                              floors (USD min_profit and/or Convergence Ratio min_pct).
                              AND semantics: the smallest amount that
                              satisfies BOTH floors is suggested.
                              Visual tone:
                                green  → meets target within cap, finite size
                                yellow → cap-bound or infeasible (Convergence Ratio
                                         unreachable / net-per-usd-nonpositive). */}
                          {opp.simulated_target && (() => {
                            const t = opp.simulated_target;
                            const infeasible =
                              t.binding_floor === "roi-unreachable"
                              || t.binding_floor === "net-per-usd-nonpositive";
                            const ok = t.meets_target_at_cap && !infeasible;
                            const titleParts: string[] = [];
                            if (t.target_net_usd != null) titleParts.push(`min $${t.target_net_usd.toFixed(2)} net`);
                            if (t.target_roi_pct != null) titleParts.push(`min ${t.target_roi_pct.toFixed(2)}% Convergence Ratio`);
                            const src = t.target_source === "strategy_config" ? "/strategies card" : "Simulación tab";
                            const basis = t.estimation_basis === "roi-assumed" ? " · Convergence Ratio-assumed (no gross recorded)" : "";
                            return (
                              <div
                                className={`text-[10px] font-mono flex items-center gap-1 ${ok ? "text-success/90" : "text-warning"}`}
                                title={`Floors: ${titleParts.join(" AND ")} from ${src}${basis}`}
                              >
                                <span aria-hidden="true">{ok ? "→" : "⚠"}</span>
                                {t.binding_floor === "roi-unreachable" ? (
                                  <span>Convergence Ratio {t.target_roi_pct?.toFixed(1)}% inalcanzable</span>
                                ) : t.binding_floor === "net-per-usd-nonpositive" ? (
                                  <span>costos &gt; gross</span>
                                ) : ok ? (
                                  <span>
                                    {t.target_net_usd != null && `$${t.target_net_usd.toFixed(0)}`}
                                    {t.target_net_usd != null && t.target_roi_pct != null && " · "}
                                    {t.target_roi_pct != null && `${t.target_roi_pct.toFixed(1)}%`}
                                    {" @ borrow "}{formatUsdShort(t.required_amount_in_usd)}
                                  </span>
                                ) : (
                                  <span>cap {formatUsdShort(t.cap_amount_in_usd)} → max ${t.suggested_net_usd.toFixed(2)}</span>
                                )}
                                {t.estimation_basis === "roi-assumed" && (
                                  <span className="ml-1 px-1 rounded text-[8px] bg-warning/15 text-warning border border-warning/30 uppercase">est</span>
                                )}
                              </div>
                            );
                          })()}
                        </div>
                        {/* Tooltip popover: extended cost breakdown when
                            simulated values are present + target attribution.
                            Also shows for Path-B rows (no forward but
                            simulated_target exists) so the operator can see
                            target-source attribution even on opportunities
                            without a recorded gross. */}
                        {(opp.expected_profit_usd != null
                          || opp.net_expected_profit_usd != null
                          || opp.simulated_net_profit_usd != null
                          || opp.simulated_target != null) && (
                          <div data-slot="popover-content" className="absolute bottom-full right-0 mb-2 w-80 p-3 bg-popover text-popover-foreground border border-border rounded-lg shadow-xl opacity-0 group-hover:opacity-100 transition-opacity pointer-events-none z-10 text-left">
                            <div className="text-xs font-sans space-y-1">
                              <div className="flex justify-between">
                                <span>Gross (AMM quote):</span>
                                <span className={`font-mono ${TONE_CLASS[gross.tone] ?? 'text-muted-foreground'}`}>{gross.display}</span>
                              </div>
                              <div className="flex justify-between border-b border-border pb-1 mb-1 font-bold">
                                <span>
                                  Net ({netSource === "simulated" ? "simulated" : netSource === "canonical" ? "spine" : "—"}):
                                </span>
                                <span className={`font-mono ${TONE_CLASS[net.tone] ?? 'text-muted-foreground'}`}>
                                  {netSource === "simulated" ? `~${net.display}` : net.display}
                                </span>
                              </div>
                              {opp.bridge_fee_usd != null && (
                                <div className="flex justify-between text-muted-foreground">
                                  <span>Bridge Fee:</span>
                                  <span className="font-mono">${opp.bridge_fee_usd.toFixed(4)}</span>
                                </div>
                              )}
                              {/* 8-component simulated cost breakdown */}
                              {opp.simulated_cost_breakdown && (
                                <div className="pt-1 border-t border-border/50 space-y-0.5">
                                  <div className="text-muted-foreground uppercase tracking-wider text-[9px] mb-1">
                                    SIM cost breakdown (USD)
                                  </div>
                                  {[
                                    ["Gas", opp.simulated_cost_breakdown.gas_usd],
                                    ["Flash Convergence fee", opp.simulated_cost_breakdown.flashloan_fee_usd],
                                    ["LP fees", opp.simulated_cost_breakdown.lp_fees_usd],
                                    ["Slippage", opp.simulated_cost_breakdown.slippage_usd],
                                    ["Failure buffer", opp.simulated_cost_breakdown.failure_buffer_usd],
                                    ["Copied buffer", opp.simulated_cost_breakdown.copied_buffer_usd],
                                    ["Capital cost", opp.simulated_cost_breakdown.capital_cost_usd],
                                    ["Ops overhead", opp.simulated_cost_breakdown.ops_overhead_usd],
                                    ["Relay fee", opp.simulated_cost_breakdown.relay_fee_usd],
                                  ].map(([label, value]) => (
                                    <div key={String(label)} className="flex justify-between text-muted-foreground/80">
                                      <span>{label}:</span>
                                      <span className="font-mono">${(value as number).toFixed(4)}</span>
                                    </div>
                                  ))}
                                  {opp.simulated_amount_in_usd != null && (
                                    <div className="flex justify-between text-muted-foreground/80 pt-1 border-t border-border/30">
                                      <span>Amount_in (USD):</span>
                                      <span className="font-mono">{formatUsdShort(opp.simulated_amount_in_usd)}</span>
                                    </div>
                                  )}
                                </div>
                              )}
                              {/* Target sizing attribution */}
                              {opp.simulated_target && (
                                <div className="pt-1 border-t border-border/50 space-y-0.5">
                                  <div className="text-muted-foreground uppercase tracking-wider text-[9px] mb-1">
                                    Target sizing — AND semantics (both floors must hold)
                                  </div>
                                  <div className="flex justify-between">
                                    <span>Source:</span>
                                    <span className="font-mono">
                                      {opp.simulated_target.target_source === "strategy_config"
                                        ? "/strategies card"
                                        : "Simulación tab"}
                                    </span>
                                  </div>
                                  {opp.simulated_target.target_net_usd != null && (
                                    <div className="flex justify-between">
                                      <span>USD floor (min yield):</span>
                                      <span className="font-mono">${opp.simulated_target.target_net_usd.toFixed(2)}</span>
                                    </div>
                                  )}
                                  {opp.simulated_target.target_roi_pct != null && (
                                    <div className="flex justify-between">
                                      <span>Convergence Ratio floor (min %):</span>
                                      <span className="font-mono">{opp.simulated_target.target_roi_pct.toFixed(2)}%</span>
                                    </div>
                                  )}
                                  <div className="flex justify-between">
                                    <span>Binding floor:</span>
                                    <span className={`font-mono ${
                                      opp.simulated_target.binding_floor === "roi-unreachable"
                                      || opp.simulated_target.binding_floor === "net-per-usd-nonpositive"
                                        ? "text-destructive"
                                        : "text-foreground"
                                    }`}>
                                      {opp.simulated_target.binding_floor}
                                    </span>
                                  </div>
                                  <div className="flex justify-between">
                                    <span>Estimation basis:</span>
                                    <span className={`font-mono ${opp.simulated_target.estimation_basis === "roi-assumed" ? "text-warning" : "text-foreground"}`}>
                                      {opp.simulated_target.estimation_basis}
                                    </span>
                                  </div>
                                  {opp.simulated_target.estimation_basis === "roi-assumed" && (
                                    <div className="text-[10px] text-warning/80 italic pt-0.5">
                                      No gross recorded — sizing assumes route delivers your min convergence ratio floor as its gross rate.
                                    </div>
                                  )}
                                  <div className="flex justify-between pt-1 border-t border-border/30">
                                    <span>Required amount_in:</span>
                                    <span className="font-mono">
                                      {Number.isFinite(opp.simulated_target.required_amount_in_usd)
                                        ? formatUsdShort(opp.simulated_target.required_amount_in_usd)
                                        : "∞"}
                                    </span>
                                  </div>
                                  <div className="flex justify-between">
                                    <span>Effective cap:</span>
                                    <span className="font-mono">{formatUsdShort(opp.simulated_target.cap_amount_in_usd)}</span>
                                  </div>
                                  <div className="flex justify-between">
                                    <span>Suggested:</span>
                                    <span className={`font-mono ${opp.simulated_target.meets_target_at_cap ? 'text-success' : 'text-warning'}`}>
                                      {formatUsdShort(opp.simulated_target.suggested_amount_in_usd)}
                                      {!opp.simulated_target.meets_target_at_cap && " (cap-bound)"}
                                    </span>
                                  </div>
                                  <div className="flex justify-between">
                                    <span>Suggested net:</span>
                                    <span className="font-mono">${opp.simulated_target.suggested_net_usd.toFixed(2)}</span>
                                  </div>
                                </div>
                              )}
                              {/* Notes (linear-extrap, cap-bound, etc.) */}
                              {opp.simulated_notes && opp.simulated_notes.length > 0 && (
                                <div className="pt-1 border-t border-border/50 text-muted-foreground/70 italic">
                                  Notes: {opp.simulated_notes.join(", ")}
                                </div>
                              )}
                              {!opp.simulated_cost_breakdown && netSource !== "simulated" && (
                                <div className="flex justify-between text-muted-foreground/70">
                                  <span>Gas / Bribe / Slippage breakdown:</span>
                                  <span className="italic">Pendiente revm sim</span>
                                </div>
                              )}
                              {opp.paper_status && (
                                <div className="flex justify-between text-muted-foreground/70 pt-1 border-t border-border/50">
                                  <span>Paper status:</span>
                                  <span className="font-mono">{opp.paper_status}</span>
                                </div>
                              )}
                            </div>
                          </div>
                        )}
                      </div>
                    </td>

                    {/* ── NET Convergence Ratio column — R8 fail-honest via formatPctOrDash ── */}
                    <td className="p-4 text-right font-mono text-foreground" data-col="convergence-ratio">
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

                    {/* ── CONFIDENCE column — R8: null → "—" ── */}
                    <td className="p-4 text-center" data-col="confidence">
                      <span className="font-mono text-sm">
                        {opp.confidence_score_bps != null
                          ? `${(opp.confidence_score_bps / 100).toFixed(2)}%`
                          : "—"}
                      </span>
                    </td>

                    {/* ── GAS USED column — R8: null → "—" ── */}
                    <td className="p-4 text-right font-mono text-sm" data-col="gas-used">
                      {opp.gas_used != null
                        ? opp.gas_used.toLocaleString()
                        : "—"}
                    </td>

                    {/* ── ACTION column ── */}
                    {/*
                      C6 fix (audit 2026-05-10): the SIMULATE button calls
                      POST /api/opportunities/:id/simulate, an endpoint the
                      backend doesn't expose yet. Until the spine wires the
                      revm shadow-sim to a public route, the button is
                      visibly disabled with a "backend pending" tooltip and
                      reduced styling — never promising live action it
                      cannot deliver. Click is allowed only as a no-op
                      diagnostic that surfaces the 404 toast.

                      EXECUTE button: Enabled only for Hamiltonian-detected
                      opportunities with cartridge confidence >= 0.7 */}
                    <td className="p-4 text-center">
                      <div className="flex items-center gap-2 justify-center">
                        <button
                          type="button"
                          disabled
                          aria-disabled="true"
                          onClick={(e) => { e.stopPropagation(); handleSimulate(opp.id); }}
                          className="px-3 py-1 rounded text-[10px] font-bold uppercase tracking-wide border border-dashed border-muted-foreground/40 bg-muted/30 text-muted-foreground cursor-not-allowed"
                          title="Backend pending — POST /api/opportunities/:id/simulate is not yet implemented (spine revm shadow-sim ticket open)"
                        >
                          {simLoading === opp.id ? "…" : "SIM"}
                        </button>
                        <button
                          type="button"
                          disabled={!opp.hamiltonian_detected || !opp.cartridge_confidence || opp.cartridge_confidence < 0.7}
                          onClick={(e) => { e.stopPropagation(); handleExecute(opp); }}
                          className={`px-3 py-1 rounded text-[10px] font-bold uppercase tracking-wide border transition-colors ${
                            opp.hamiltonian_detected && opp.cartridge_confidence && opp.cartridge_confidence >= 0.7
                              ? "bg-success text-success-foreground border-success hover:bg-success/90"
                              : "bg-muted text-muted-foreground border-muted-foreground/30 cursor-not-allowed"
                          }`}
                          title={
                            opp.hamiltonian_detected && opp.cartridge_confidence && opp.cartridge_confidence >= 0.7
                              ? "Execute opportunity"
                              : "Hamiltonian detection required"
                          }
                        >
                          EXECUTE
                        </button>
                      </div>
                    </td>
                  </motion.tr>
                );
              })}
            </AnimatePresence>
            {opportunities.length === 0 && (
              <tr>
                <td colSpan={9} className="p-8 text-center text-muted-foreground italic">No opportunities detected. Searcher scanning mempool...</td>
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
