"use client";
import React, { useEffect, useState, useCallback, useRef, useMemo } from "react";
import { Zap, WifiOff, ShieldAlert, RefreshCw, Radio, EyeOff, Eye, ChevronDown, Search } from "lucide-react";
import { sanitizeForDisplay } from "@/lib/omega-lexicon";
import { toast } from "sonner";
import { OpportunityDetailDialog } from "@/components/OpportunityDetailDialog";
import { OpportunityTradeCard } from "@/components/OpportunityTradeCard";
import { WindowTotalSegment } from "@/components/opportunities/WindowTotalSegment";
import { QuarantinedEventsAuditTrail } from "@/components/opportunities/QuarantinedEventsAuditTrail";
// CONSOLIDACIÓN 2026-09-19 (orden operador): los MOTORES de la página exchange
// (filtro familiar/cadena/yield, price ticker G-PRICE-1, badge paper/live, cap
// de memoria) se portan aquí SIN cambiar el estilo visual de esta página.
import { PriceTicker } from "@/components/opportunities/exchange/PriceTicker";
import {
  applyExchangeFilters,
  DEFAULT_FILTERS,
  type ExchangeFilters,
} from "@/components/opportunities/exchange/ExchangeFilterBar";
import { familyOf } from "@/lib/strategy-kinds";
import { usePaperModeState } from "@/hooks/usePaperModeState";

// ─── Omni-Store Integration ───────────────────────────────────────────────────
import { useOmniOpportunities } from "@/lib/store/useOmniOpportunities";
import { useOmniStore } from "@/lib/store/omni-store";
import { type OmniOpportunity } from "@/lib/store/types";
import { parseSnapshotItems, parseWindowTotal } from "@/lib/store/snapshot-payload";
import { routeGroupKeyOf } from "@/lib/store/route-key";
import { getApiBaseUrl, getPublicEdgeBaseUrl } from "@/lib/api-client";

// Re-export store types for downstream consumers (FE-0034: the detail dialog
// no longer needs the mirror — it imports OmniOpportunity from the store).
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
// CARDS-NOTIONAL-01 — the notifier's gate is the SAME SSOT the card ladder
// uses, so the toast can never announce a figure the card refuses to paint.
import { decideOpportunityNotification, selectGridRows } from "@/lib/opportunity-ledger";

// Stable route identity for the card grid key. A re-detected route (same
// chain + strategy + token pair + DEX path) must update the SAME card in place
// rather than remount a new one — so we key on the route identity, not the
// per-detection row id. This is what stops the enter-animation flash each poll
// and what makes a card disappear the moment its route drops from the snapshot.
// CARDS-DEDUP-HOPS: the shared bit-exact key (same as the api-server GROUP BY
// twin) so REST and WS dedup identically, and re-detections refresh the card's
// economics in place with zero remount (operator order 2026-09-20).

// FE-1: WS statuses. "LIVE" = WS connected. "STALE" = WS disconnected.
// "POLLING" = WS failed 3×, degraded to HTTP polling. "CONNECTING" = initial.
// HTTP fetch errors (manual refresh) surface via errorMsg, not feedStatus.


const POLL_INTERVAL_MS = 4_000;

/** Hard cap on simultaneously mounted cards — memory-discipline bound
 *  (portado del motor de la página exchange, sin cambio de estilo). */
const VISIBLE_CAP = 60;

/** Declared-reason breakdown for the gate-D banner, e.g.
 *  "spread_zero_equilibrium 12 · non_positive_profit 3". Pure; counts only. */
function reasonBreakdown(rows: OmniOpportunity[]): string {
  const counts = new Map<string, number>();
  for (const r of rows) {
    const key = r.rejection_reason ?? "sin_razon";
    counts.set(key, (counts.get(key) ?? 0) + 1);
  }
  return Array.from(counts.entries())
    .sort((a, b) => b[1] - a[1])
    .slice(0, 4)
    .map(([k, n]) => `${k} ${n}`)
    .join(" · ");
}

/** Human label for the lookback selector. */
const WINDOW_CHOICES: Array<{ seconds: number; label: string }> = [
  { seconds: 300, label: "5 min" },
  { seconds: 900, label: "15 min" },
  { seconds: 3600, label: "1 h" },
  { seconds: 21600, label: "6 h" },
  { seconds: 86400, label: "24 h" },
];

function windowLabel(seconds: number): string {
  return WINDOW_CHOICES.find((w) => w.seconds === seconds)?.label ?? `${seconds}s`;
}

export type OpportunitiesSnapshot = {
  opportunities: OmniOpportunity[];
  serverTime: string | null;
  source: string;
};

export default function OpportunitiesClient({
  initialSnapshot,
  initialShowRejected = false,
  initialWindowSeconds = 300,
}: {
  initialSnapshot: OpportunitiesSnapshot;
  /** SHOW-REJECTED-01: server-read deep link (`?show_rejected=1`) for the
   *  "Mostrar rechazadas" toggle. Default OFF — the real/live card set. */
  initialShowRejected?: boolean;
  /** WINDOW-01: server-read deep link (`?window_seconds=3600`) for the live
   *  lookback, in seconds. Default 300 s — the api-server's own default, so the
   *  behaviour is unchanged unless the operator widens it. */
  initialWindowSeconds?: number;
}) {
  // ─── Omni-Store Integration ───────────────────────────────────────────────
  // Connect WebSocket stream to the store (replaces useOpportunitiesStream)
  const EDGE_URL = getApiBaseUrl();
  // FE-EDGE-DIRECT-01: public cards reads go edge-direct (single feed origin).
  // EDGE_URL stays same-origin for handleSimulate — the POST carries the
  // host-only admin session cookie, which would not travel cross-origin.
  const PUBLIC_EDGE_URL = getPublicEdgeBaseUrl();
  const [viableOnly, setViableOnly] = useState(false);
  // WINDOW-01 (operator order 2026-09-27): the profitable detections are
  // REJECTED rows, and the canonical one (+1.7712 net) was 936 s old when it
  // was invisible — outside the 5-minute live window, not hidden by a filter.
  // The lookback is operator-controlled and rides the snapshot request; the
  // store's vigency TTL follows the same value.
  const [windowSeconds, setWindowSeconds] = useState<number>(initialWindowSeconds);

  useOmniOpportunities({
    viableOnly,
    initialOpportunities: initialSnapshot.opportunities,
    maxAgeSeconds: windowSeconds,
  });

  // Selectors from Omni-Store (SSOT)
  // SSR-FIRSTPAINT-01 (2026-09-26): the store value is `[]` on the server —
  // `useOmniOpportunities` seeds it from `initialOpportunities` inside an
  // EFFECT (useOmniOpportunities.ts:109-118), and effects never run during SSR.
  // Reading the store directly therefore rendered ZERO cards in the server HTML
  // while the fetched snapshot sat unused: PROVEN in production (3 consecutive
  // `GET /opportunities` renders contained `data-opp-id` × 0 and the
  // "0 matching" counter; `Cache-Control: private, no-cache, no-store`, so it
  // is a live dynamic render; and the api-server log shows the page's own
  // snapshot query answering 200 for
  // `/api/v1/opportunities/live?order=profit_usd`). Every computed value of the
  // snapshot was discarded for the first paint. Derived below as
  // `opportunities` (R1 Mounted Snapshot Pattern).
  const storeOpportunities = useOmniStore((state) => state.opportunities);
  const wsStatus = useOmniStore((state) => state.wsStatus);
  const setOpportunities = useOmniStore((state) => state.setOpportunities);
  // AUDIT-CARDS-MINOR (§2): WO-H4 window_total from the LAST live-snapshot
  // envelope — distinct routes in the ≤5 min window. R8: null until a
  // snapshot carrying the field arrives (absent ≠ 0, never invented).
  const windowTotal = useOmniStore((state) => state.windowTotal);
  const setWindowTotal = useOmniStore((state) => state.setWindowTotal);

  // ─── UI State (local, not in store) ───────────────────────────────────────
  const [isMounted, setIsMounted] = useState(false);
  const [errorMsg, setErrorMsg] = useState<string | null>(null);
  const [now, setNow] = useState<number>(0);
  const [simLoading, setSimLoading] = useState<string | null>(null);
  const [selectedOpp, setSelectedOpp] = useState<OmniOpportunity | null>(null);
  const [lastRefresh, setLastRefresh] = useState<Date | null>(
    initialSnapshot.serverTime ? new Date(initialSnapshot.serverTime) : null
  );
  const [filters, setFilters] = useState<ExchangeFilters>(() => ({
    ...DEFAULT_FILTERS,
    showRejected: initialShowRejected,
  }));
  const [cap, setCap] = useState<number>(VISIBLE_CAP);

  // ── R1 Mounted Snapshot Pattern: which source owns the grid ────────────────
  // Until mount, the SERVER-rendered snapshot is the display source, so the
  // first paint shows the cards (and their computed figures) the Server
  // Component already fetched. The store is empty on the server and on the very
  // first client render, so both sides render the SAME prop-derived markup
  // (byte-identical ⇒ no hydration mismatch, no Date.now/Math.random). The
  // mount effect flips `isMounted`, after which the live store owns the grid; an
  // empty store must NOT fall back to the snapshot again (that would resurrect
  // pruned cards), which is exactly why this keys on `isMounted` and not on
  // `storeOpportunities.length === 0`.
  const opportunities = isMounted ? storeOpportunities : initialSnapshot.opportunities;

  // ── Motor badge paper/live (display-only, fail-safe) — portado del exchange ──
  const primaryChainId = initialSnapshot.opportunities[0]?.chain_id ?? 1;
  const paperMode = usePaperModeState(primaryChainId);
  const modeLabel: "paper" | "live" = paperMode.data.enabled
    ? "paper"
    : paperMode.data.confidence !== "default_safe"
      ? "live"
      : "paper";

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

  // AUDIT-CARDS-MINOR (§1): the declared per-strategy trading-config fetch
  // (GET /api/trading-config → strategyConfigs state) is REMOVED — it only fed
  // OpportunityTradeCard's dead `strategyConfig` prop, which never rendered
  // anything (the card's "Applied strategy config" block reads
  // opp.simulated_target). One phantom request per mount less; /strategies and
  // /config/trading keep their own getTradingConfig consumers.

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
    setSelectedOpp(opp);
  }, []);

  const onExecute = useCallback(
    (opportunityId: string) => handleSimulate(opportunityId),
    [handleSimulate],
  );
  // It clears the store and repopulates via HTTP, then the WS stream continues.
  const fetchOpportunities = useCallback(async () => {
    try {
      const url = `${PUBLIC_EDGE_URL}/api/opportunities/live?viable_only=${viableOnly}&limit=50&order=profit_usd`;
      const res = await fetch(url, {
        headers: { accept: "application/json" },
        signal: AbortSignal.timeout(4000),
        cache: "no-store",
      });
      if (!res.ok) {
        setErrorMsg(`Edge returned ${res.status}`);
        return;
      }
      const data: unknown = await res.json();
      // PERF: batch store update instead of clear + 50 addOpportunity calls.
      // FE-SNAPSHOT-01: same parser seam as the 5s reconcile loop.
      setOpportunities(parseSnapshotItems(data));
      // AUDIT-CARDS-MINOR (§2): the envelope's window_total (WO-H4) rides the
      // same snapshot — null when the payload doesn't carry it (R8).
      setWindowTotal(parseWindowTotal(data));
      setLastRefresh(new Date());
      setErrorMsg(null);
    } catch (e) {
      setErrorMsg((e as Error).message);
    }
  }, [PUBLIC_EDGE_URL, viableOnly, setOpportunities, setWindowTotal]);

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
  //
  // CARDS-NOTIONAL-01 (2026-09-26): the toast used to read
  //   const yieldVal = opp.expected_profit_usd ?? 0;
  //   ... `Net yield $${yieldVal.toFixed(2)}`
  // Two defects on one line: (1) `expected_profit_usd` is the DEX engine's fast
  // filter GROSS, computed at the engine's own probe — it was announced as a
  // "Net yield", which is how the operator read
  // `Net yield $822215.98` for a row whose own kernel net was ≈ −0.00001;
  // (2) `?? 0` turned "not computed" into a number that clears a threshold of 0
  // (R8: None ≠ Some(0)).
  //
  // The gate is now the SAME SSOT the card ladder uses, so the toast can only
  // ever announce a figure that is closed inside one notional, and it names it
  // for what it is. A row whose economics cross notionals announces nothing.
  useEffect(() => {
    if (!isMounted) return;
    for (const opp of opportunities) {
      if (seenNotifiedIds.current.has(opp.id)) continue;
      seenNotifiedIds.current.add(opp.id);
      const decision = decideOpportunityNotification(
        opp,
        prefs.notification_threshold_usd,
      );
      if (!decision.fire || decision.amount_usd == null || decision.label == null) {
        continue;
      }
      toast.success(`High-value opportunity — ${opp.strategy_kind}`, {
        description: `${decision.label} $${decision.amount_usd.toFixed(2)} · chain ${opp.chain_id} · ${opp.dex_a}${opp.dex_b ? ` → ${opp.dex_b}` : ""} · ${decision.basis === "simulated" ? "SIM" : "spine"}`,
        duration: 8_000,
      });
    }
  }, [opportunities, isMounted, prefs.notification_threshold_usd]);

  // R1: setIsMounted + setNow are the only non-WS side effects needed here.
  useEffect(() => {
    setIsMounted(true);
    setNow(Date.now());
  }, []);

  // FE-SNAPSHOT-01 (2026-09-20): back to 1000ms. The 2026-08-09 note below
  // demoted this to 30s because a 1s tick re-rendered every card; since then
  // OpportunityTradeCard's React.memo comparator (CARDS-DEDUP-HOPS) bails
  // unless the card's OWN data or its DISPLAYED age-second changed, so a 1s
  // tick now costs N comparator calls and only re-renders cards whose "hace Xs"
  // label actually moved. 1s is the operator requirement: the vigency line is
  // the live "still available?" signal — a 30s tick froze it and made a dead
  // route indistinguishable from a stable one for half a minute.
  useEffect(() => {
    const ticker = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(ticker);
  }, []);

  // FE-1: Opportunities come from Omni-Store (SSOT).
  const viableCount = opportunities.filter((o) => o.status !== "rejected" && o.status !== "failed").length;
  const rejectedCount = opportunities.filter((o) => o.status === "rejected").length;

  // ── Motor de filtros (portado del exchange: familia/cadena/búsqueda/yield) ──
  const filtered = useMemo(
    () => applyExchangeFilters(opportunities, filters),
    [opportunities, filters],
  );
  // REAL-LIVE-CARDS-SSOT-01: a trading card is not a detection shell. Rejected
  // rows remain visible when they were genuinely computed; source failures stay
  // diagnostics until the producer can calculate them.
  // SHOW-REJECTED-01 (operator order 2026-09-27): the diagnostics are one toggle
  // away instead of a bare count — same rows, same reasons, never a substitute
  // figure. OFF ⇒ grid === real/live cards (unchanged behaviour).
  const {
    economic: economicCards,
    inScope,
    hiddenByGate: hiddenRejectedRows,
    noData: noDataRows,
    grid: gridRows,
  } = useMemo(() => selectGridRows(filtered, filters.showRejected), [filtered, filters.showRejected]);
  const declaredCount = noDataRows.length;
  const visible = useMemo(() => gridRows.slice(0, cap), [gridRows, cap]);

  /** "Mostrar rechazadas" (operator order 2026-09-27). Turning it ON must also
   *  release the viable-only narrowing (both the filter-object flag and the
   *  stream flag), otherwise the rows it reveals are dropped before the grid. */
  const toggleShowRejected = () => {
    const next = !filters.showRejected;
    setFilters({ ...filters, showRejected: next, viableOnly: false });
    if (next) setViableOnly(false);
  };

  // Familias presentes en el feed (motor del exchange, sin estilos atlas).
  const families = useMemo(() => {
    const set = new Set<string>(["triangular", "cross_chain", "liquidation", "flashloan_arb"]);
    for (const o of opportunities) {
      if (o.strategy_kind != null) set.add(familyOf(o.strategy_kind));
    }
    return Array.from(set).sort((a, b) => a.localeCompare(b));
  }, [opportunities]);
  const allFamiliesEnabled = filters.enabledFamilies.size === 0;
  const toggleFamily = (fam: string) => {
    const next = new Set<string>(allFamiliesEnabled ? families : filters.enabledFamilies);
    if (next.has(fam)) next.delete(fam);
    else next.add(fam);
    setFilters({ ...filters, enabledFamilies: next });
  };
  const feedChains = useMemo(() => {
    const m = new Map<number, number>();
    for (const o of opportunities) {
      if (o.chain_id == null) continue; // R8: malformed row joins no chain bucket
      m.set(o.chain_id, (m.get(o.chain_id) ?? 0) + 1);
    }
    return Array.from(m.entries()).sort((a, b) => b[1] - a[1]);
  }, [opportunities]);
  // Distinct hop counts in the live feed for the hops filter (operator order
  // 2026-09-20). hop_count null = legacy row without route_metadata — no bucket.
  const feedHops = useMemo(() => {
    const set = new Set<number>();
    for (const o of opportunities) {
      if (o.hop_count != null) set.add(o.hop_count);
    }
    return Array.from(set).sort((a, b) => a - b);
  }, [opportunities]);

  // ── G-PRICE-1: símbolos del feed para la cinta de precios ──
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

  // Reset del cap al cambiar filtros (motor de memoria del exchange).
  useEffect(() => {
    setCap(VISIBLE_CAP);
  }, [filters]);

  const isError = feedStatus === 'STALE';

  return (
    <div className={`p-8 min-h-screen transition-colors duration-500 text-foreground ${isError ? 'bg-destructive/5' : ''}`}>
      <div className="flex flex-wrap justify-between items-center gap-4 border-b border-border pb-4 mb-8">
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
              {/* AUDIT-CARDS-MINOR (§2): shown-vs-window — the payload's
                  window_total next to the grid count. Renders nothing while
                  windowTotal is null (R8: absent ≠ 0). */}
              <WindowTotalSegment
                shown={opportunities.length}
                windowTotal={windowTotal}
              />
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

      {/* ── Motor de filtros (estilo propio de esta página, no atlas) ── */}
      <div className="mb-6 flex flex-wrap items-center gap-2 text-xs">
        <span className="text-muted-foreground font-semibold uppercase tracking-wide">Filters</span>
        {families.map((fam) => {
          const enabled = allFamiliesEnabled || filters.enabledFamilies.has(fam);
          return (
            <button
              key={fam}
              type="button"
              onClick={() => toggleFamily(fam)}
              className={`px-2.5 py-1 rounded-lg border font-semibold transition-colors ${
                enabled
                  ? "bg-primary/10 border-primary/40 text-primary hover:bg-primary/20"
                  : "bg-muted border-border text-muted-foreground hover:bg-accent"
              }`}
            >
              {fam}
            </button>
          );
        })}
        <select
          value={filters.chainId}
          onChange={(e) =>
            setFilters({
              ...filters,
              chainId: e.target.value === "all" ? "all" : Number(e.target.value),
            })
          }
          className="px-2.5 py-1 rounded-lg border border-border bg-muted text-foreground"
          title="Filter by chain"
        >
          <option value="all">All chains</option>
          {feedChains.map(([chainId, count]) => (
            <option key={chainId} value={chainId}>
              chain {chainId} ({count})
            </option>
          ))}
        </select>
        {/* Hops filter (operator order 2026-09-20) — exact hop count from
            route_metadata; options derived from the live feed. */}
        <select
          value={String(filters.hops)}
          onChange={(e) =>
            setFilters({
              ...filters,
              hops: e.target.value === "all" ? "all" : Number(e.target.value),
            })
          }
          className="px-2.5 py-1 rounded-lg border border-border bg-muted text-foreground"
          title="Filter by hop count"
        >
          <option value="all">All hops</option>
          {feedHops.map((h) => (
            <option key={h} value={h}>
              {h} hops
            </option>
          ))}
        </select>
        <div className="relative">
          <Search size={12} className="absolute left-2 top-1/2 -translate-y-1/2 text-muted-foreground" />
          <input
            type="text"
            value={filters.search}
            onChange={(e) => setFilters({ ...filters, search: e.target.value })}
            placeholder="token / dex / strategy"
            className="pl-7 pr-2.5 py-1 rounded-lg border border-border bg-muted text-foreground w-44"
          />
        </div>
        <input
          type="number"
          min={0}
          step="0.5"
          value={filters.minYieldUsd ?? ""}
          onChange={(e) =>
            setFilters({
              ...filters,
              minYieldUsd: e.target.value === "" ? null : Number(e.target.value),
            })
          }
          placeholder="min yield $"
          className="px-2.5 py-1 rounded-lg border border-border bg-muted text-foreground w-28"
          title="Minimum expected yield (USD)"
        />
        {/* WINDOW-01: lookback del feed (max_age_seconds del api-server, clamp
            [10s, 24h]). Las ganancias son rechazadas que expiran de la ventana
            de 5 min: sin ampliarla, la fila +1.7712 no está en el wire. */}
        <select
          value={String(windowSeconds)}
          onChange={(e) => setWindowSeconds(Number(e.target.value))}
          data-testid="window-seconds"
          className="px-2.5 py-1 rounded-lg border border-border bg-muted text-foreground"
          title="Ventana de lookback del feed (max_age_seconds)"
        >
          {WINDOW_CHOICES.map((w) => (
            <option key={w.seconds} value={String(w.seconds)}>
              ventana {w.label}
            </option>
          ))}
        </select>
        {/* SHOW-REJECTED-01 (operator order 2026-09-27): paint the detections
            that have no closed real/live ladder — each with its own recorded
            reason. It reveals rows; it never substitutes a figure. */}
        <button
          type="button"
          data-testid="toggle-show-rejected"
          onClick={toggleShowRejected}
          aria-pressed={filters.showRejected}
          className={`px-2.5 py-1 rounded-lg border font-semibold transition-colors ${
            filters.showRejected
              ? "bg-primary/10 border-primary/40 text-primary hover:bg-primary/20"
              : "bg-muted border-border text-muted-foreground hover:bg-accent"
          }`}
          title={
            filters.showRejected
              ? "Mostrando TODAS las detecciones (rechazadas / sin economía computada incluidas) — click para volver a solo real/live"
              : "Mostrar rechazadas: pinta también las detecciones rechazadas o sin economía computada, con su razón real"
          }
        >
          {filters.showRejected ? (
            <Eye size={12} className="inline align-[-2px]" />
          ) : (
            <EyeOff size={12} className="inline align-[-2px]" />
          )}{" "}
          Mostrar rechazadas
        </button>
        <span className={`px-2.5 py-1 rounded-full border font-bold ${
          modeLabel === "paper"
            ? "bg-info/10 border-info/40 text-info"
            : "bg-destructive/10 border-destructive/40 text-destructive"
        }`} title={`Effective execution terminus (read-only): ${modeLabel} · confidence ${paperMode.data.confidence}`}>
          TERMINUS: {modeLabel.toUpperCase()}
        </span>
        <span className="text-muted-foreground">
          <span className="text-foreground font-semibold">{filtered.length}</span> matching
        </span>
      </div>

      {/* G-PRICE-1 — cinta de precios en vivo (motor portado del exchange) */}
      <PriceTicker chainId={primaryChainId} edgeUrl={PUBLIC_EDGE_URL} symbols={tickerSymbols} />

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
      {(hiddenRejectedRows.length > 0 || declaredCount > 0) && (
        <div className="mb-3 flex flex-wrap items-center gap-2 rounded-lg border border-border bg-muted/30 px-3 py-2 text-xs text-muted-foreground">
          <span>
            <span className="font-semibold text-foreground">Gate D</span>
            {": se muestran las rechazadas con economía computada y net > 0"}
            {hiddenRejectedRows.length > 0
              ? ` · ${hiddenRejectedRows.length} rechazadas CON datos y net ≤ 0 ` +
                `${filters.showRejected ? "se están mostrando" : "quedan fuera"} ` +
                `(${reasonBreakdown(hiddenRejectedRows)})`
              : ""}
            {declaredCount > 0
              ? ` · ${declaredCount} detecciones sin economía computada, declaradas y nunca rellenadas con supuestos ` +
                `(${reasonBreakdown(noDataRows)})`
              : ""}
          </span>
          <button
            type="button"
            data-testid="banner-toggle-show-rejected"
            onClick={toggleShowRejected}
            className="px-2 py-1 rounded border border-border bg-background hover:bg-accent transition-colors text-xs font-semibold"
          >
            {filters.showRejected ? "Aplicar gate D" : "Mostrar rechazadas"}
          </button>
        </div>
      )}

      {/* Gate D con la grilla vacía: se dice POR QUÉ, con los números reales,
          en vez de dejar que el silencio parezca un sistema caído. */}
      {opportunities.length > 0 && gridRows.length === 0 && (
        <div className="mb-6 rounded-xl border border-border bg-muted/40 px-4 py-3 text-sm text-muted-foreground">
          Gate D activo: en la ventana de <span className="text-foreground font-semibold">{windowLabel(windowSeconds)}</span>{" "}
          no hay filas viables ni rechazadas con net &gt; 0.
          {hiddenRejectedRows.length > 0
            ? ` ${hiddenRejectedRows.length} rechazadas con datos quedan fuera por net ≤ 0.`
            : ""}
          {declaredCount > 0 ? ` ${declaredCount} detecciones sin economía computada.` : ""}{" "}
          Usá «Mostrar rechazadas» para inspeccionarlas o ampliá la ventana.
        </div>
      )}
      <div className="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-4">
        {visible.map((opp) => (
          <OpportunityTradeCard
            key={routeGroupKeyOf(opp)}
            opp={opp}
            now={now}
            isMounted={isMounted}
            simLoading={simLoading === opp.id}
              modeLabel={modeLabel}
              onExecute={onExecute}
              onInspect={onInspect}
            />
          ))}
      </div>

      {/* Motor de memoria (portado del exchange): revelar lo diferido + cargar más */}
      {gridRows.length > visible.length && (
        <div className="mt-6 flex flex-col items-center gap-2">
          <p className="text-xs text-muted-foreground">
            Showing <span className="text-foreground font-semibold">{visible.length}</span> of{" "}
            <span className="text-foreground font-semibold">{gridRows.length}</span>{" "}
            {filters.showRejected ? "detecciones" : "real/live cards"} — the rest are
            deferred (memory-discipline cap).
          </p>
          <button
            type="button"
            onClick={() => setCap((c) => c + VISIBLE_CAP)}
            className="flex items-center gap-2 px-3 py-1.5 rounded-lg border border-border bg-muted hover:bg-accent transition-colors text-xs font-semibold"
          >
            <ChevronDown size={12} /> Show {Math.min(VISIBLE_CAP, gridRows.length - visible.length)} more
          </button>
        </div>
      )}

      {/* FE-0032 (§31): Audit Trail — the quarantined rows of THIS snapshot in
          the §31 columns (pure aggregation of the same store data, no second
          fetch; §30 keeps them visible on the grid, this lists the details). */}
      <QuarantinedEventsAuditTrail opportunities={opportunities} />

      {/* FE-10: Opportunity detail sheet — click any row to open */}
      <OpportunityDetailDialog
        opportunity={selectedOpp}
        onClose={() => setSelectedOpp(null)}
      />
    </div>
  );
}
