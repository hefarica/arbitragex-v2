"use client";
/**
 * RoutesDiscoveryClient — live-polling view of route-discovery radar.
 *
 * R1 Mounted-Snapshot Pattern: receives initialData from the Server Component,
 * then polls /api/route-discovery/routes + /api/route-discovery/status every 8s.
 * Fail-honest: poll errors surface an inline banner; last good snapshot preserved.
 *
 * Safety: observe-only. No capital, no execution, no broadcast.
 * Zero-Mocks: all data from /api/route-discovery/*; no fabricated defaults.
 */
// SSR-test support (repo pattern, cf. MarketEventPipelineHeader): classic JSX
// path needs React in module scope. Added by d9 during FE-0036 (tab mount).
import * as React from "react";
import { useEffect, useState } from "react";
import { RadarIcon, ZapIcon, NetworkIcon } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { DegradedBanner } from "@/components/DegradedBanner";
import { SourceMeta } from "@/components/SourceMeta";
import { useRouteTick } from "@/lib/store/omni-store";

import { MarketEventPipelineHeader } from "./MarketEventPipelineHeader";
import { HopControls } from "./HopControls";
// FE-0036 (§43/§44): the Performance tab over the same provider-fed tick.
import { PerformancePanel } from "./PerformancePanel";
import {
  filterRoutesByHops,
  filterRoutesByKpi,
  hopCounts,
  type PipelineKpiId,
} from "./pipeline";

const POLL_INTERVAL_MS = 8_000;

// ─── Types ────────────────────────────────────────────────────────────────────
interface RejectedStrategy {
  reason: string;
  strategy: string;
}

interface RouteEntry {
  route_hash: string;
  route_kind: string;
  hops: number;
  tokens: string[];
  pools: string[];
  protocols: string[];
  fee_tiers: number[];
  directions: string[];
  applicable_strategies: string[];
  rejected_strategies: RejectedStrategy[];
  dispatch_strategy: string;
  dispatch_deferred: string | null;
}

interface DiscoveryStatusData {
  last_tick?: {
    algorithm: string;
    chain_id: number;
    edges_built: number;
    edges_rejected: number;
    latency_ms: number;
    mode: string;
    pools_total: number;
    routes_capped: boolean;
    multi_hop_profitable_cycles: number;
  };
}

interface RoutesData {
  routes: RouteEntry[];
}

interface DiscoverySnapshot {
  routes: RoutesData | null;
  status: { ok: boolean; mode: string; updated_at: string; data: DiscoveryStatusData } | null;
}

// ─── Route card ───────────────────────────────────────────────────────────────
function RouteCard({ route, idx }: { route: RouteEntry; idx: number }) {
  const shortHash = route.route_hash.slice(0, 10) + "…";
  return (
    <Card data-testid={`route-card-${idx}`} className="text-sm">
      <CardHeader className="pb-2">
        <div className="flex items-start justify-between gap-2">
          <CardTitle className="flex items-center gap-2 text-xs font-mono text-muted-foreground">
            <NetworkIcon className="h-3.5 w-3.5 shrink-0" />
            {shortHash}
          </CardTitle>
          <Badge variant="outline" className="text-xs capitalize">{route.route_kind}</Badge>
        </div>
      </CardHeader>
      <CardContent className="space-y-2">
        <div className="flex flex-wrap gap-1">
          <span className="text-xs text-muted-foreground">Hops: {route.hops}</span>
          <span className="text-xs text-muted-foreground">·</span>
          <span className="text-xs text-muted-foreground">Protocols: {route.protocols.join(", ")}</span>
        </div>
        {route.applicable_strategies.length > 0 && (
          <div className="flex flex-wrap gap-1">
            {route.applicable_strategies.map(s => (
              <Badge key={s} variant="secondary" className="text-xs">{s}</Badge>
            ))}
          </div>
        )}
        {route.dispatch_strategy && (
          <div className="flex items-center gap-1 text-xs">
            <ZapIcon className="h-3 w-3 text-primary" />
            <span className="font-medium">Dispatch:</span>
            <span className="text-muted-foreground">{route.dispatch_strategy}</span>
            {route.dispatch_deferred && (
              <Badge variant="outline" className="text-xs ml-1">{route.dispatch_deferred}</Badge>
            )}
          </div>
        )}
      </CardContent>
    </Card>
  );
}

// ─── Main client ───────────────────────────────────────────────────────────────
interface Props {
  initialData: DiscoverySnapshot;
}

export function RoutesDiscoveryClient({ initialData }: Props) {
  const [data, setData] = useState<DiscoverySnapshot>(initialData);
  const [pollError, setPollError] = useState<string | null>(null);
  const [lastOk, setLastOk] = useState<number | null>(null);
  // FE-0026 (§18): the funnel header reads the provider-fed tick (WS push +
  // REST fallback — FE-0008 owns the cadence) and owns the KPI filter state.
  // SSR reads the store INITIAL state (tick null → honest dashes) — R1-safe.
  const routeTick = useRouteTick();
  const [activeKpi, setActiveKpi] = useState<PipelineKpiId | null>(null);
  const toggleKpi = (id: PipelineKpiId) =>
    setActiveKpi((prev) => (prev === id ? null : id));
  // FE-0027 (§19/§20): hop chips are a VIEW filter over the same grid — they
  // never touch the runtime hop policy (the effective bounds ride the tick).
  const [activeHops, setActiveHops] = useState<Set<number> | null>(null);
  const toggleHop = (h: number) =>
    setActiveHops((prev) => {
      const next = new Set(prev ?? []);
      if (next.has(h)) next.delete(h);
      else next.add(h);
      return next.size === 0 ? null : next;
    });
  // FE-0036 (§43/§44): Radar | Performance view switch over the SAME tick —
  // pure view state, the radar poll and the funnel/hop filters are untouched.
  const [view, setView] = useState<"radar" | "performance">("radar");

  useEffect(() => {
    let alive = true;

    const poll = async () => {
      try {
        const [routesRes, statusRes] = await Promise.all([
          fetch("/api/route-discovery/routes", { cache: "no-store", headers: { accept: "application/json" } }),
          fetch("/api/route-discovery/status", { cache: "no-store", headers: { accept: "application/json" } }),
        ]);
        if (!alive) return;
        // Fail-honest (RULE 00): an HTTP error status is NOT a network throw, so
        // it never reaches catch{}. Surface it verbatim and PRESERVE the last good
        // snapshot — nulling it here would render an upstream failure as a healthy
        // "no routes discovered yet" empty box.
        if (!routesRes.ok || !statusRes.ok) {
          setPollError(`edge poll failed — routes HTTP ${routesRes.status}, status HTTP ${statusRes.status}`);
          return;
        }
        const routes = await routesRes.json();
        const status = await statusRes.json();
        setData({ routes: routes?.data ?? null, status });
        setPollError(null);
        setLastOk(Date.now());
      } catch (e) {
        if (alive) setPollError((e as Error).message);
      }
    };

    const timer = setInterval(() => void poll(), POLL_INTERVAL_MS);
    return () => {
      alive = false;
      clearInterval(timer);
    };
  }, []);

  const allRoutes = data.routes?.routes ?? [];
  // Both view filters compose (KPI predicate ∧ hop set) over the SAME set.
  const routes = filterRoutesByHops(
    filterRoutesByKpi(allRoutes, activeKpi),
    activeHops,
  );
  const counts = hopCounts(allRoutes);
  const tick = data.status?.data?.last_tick;

  return (
    <div className="space-y-6" data-testid="routes-discovery-panel">
      {pollError && (
        <DegradedBanner
          title="Route-discovery poll failed — showing last known routes"
          reason={pollError}
          endpoint="GET /api/route-discovery/routes"
        />
      )}

      {data.status && data.status.ok === false && (
        <DegradedBanner
          title="Route-discovery worker degraded"
          reason={data.status.mode || "status not ok"}
          endpoint="GET /api/route-discovery/status"
          lastOk={data.status.updated_at}
        />
      )}

      <SourceMeta source="edge" at={lastOk} pollMs={POLL_INTERVAL_MS} className="px-1" />

      {/* FE-0036 (§43/§44): view tabs — Radar (routes grid + §18/§19 views) |
          Performance (10_LATENCY stages). Same provider-fed tick, pure view. */}
      <div
        role="tablist"
        aria-label="Route Discovery view"
        data-testid="routes-view-tabs"
        className="flex gap-1.5"
      >
        {(["radar", "performance"] as const).map((v) => (
          <button
            key={v}
            type="button"
            role="tab"
            aria-selected={view === v}
            onClick={() => setView(v)}
            className={`rounded-md border px-3 py-1.5 text-xs font-mono transition-colors ${
              view === v
                ? "border-primary bg-primary/10 text-primary"
                : "border-border text-muted-foreground hover:bg-muted/50 focus-visible:outline focus-visible:outline-2 focus-visible:outline-ring"
            }`}
          >
            {v}
          </button>
        ))}
      </div>

      {/* FE-0036 (§43/§44): Performance view — the latency half of the tick. */}
      {view === "performance" && <PerformancePanel tick={routeTick} />}

      {/* FE-0026 (§18): market event pipeline — KPI slots from the provider-fed
          tick; filterable slots (routes/strategies) toggle the grid filter. */}
      {view === "radar" && (
        <MarketEventPipelineHeader tick={routeTick} active={activeKpi} onToggle={toggleKpi} />
      )}

      {/* FE-0027 (§19/§20/§63): hop view-filter — VIEW_FILTER, never a runtime
          mutation; the effective bounds beside it come from the tick wire. */}
      {view === "radar" && (
      <HopControls
        counts={counts}
        active={activeHops}
        onToggle={toggleHop}
        effectiveBounds={routeTick?.multi_hop_hops_effective ?? null}
      />
      )}

      {/* Status strip */}
      {view === "radar" && tick && (
        <div className="flex flex-wrap items-center gap-4 rounded-lg border p-4" data-testid="routes-discovery-status">
          <div className="flex items-center gap-2">
            <RadarIcon className="h-4 w-4 text-primary" />
            <span className="text-sm font-medium">Algorithm:</span>
            <Badge variant="outline" className="text-xs">{tick.algorithm}</Badge>
          </div>
          <span className="text-sm text-muted-foreground">Edges built: <strong>{tick.edges_built}</strong></span>
          <span className="text-sm text-muted-foreground">Pools: <strong>{tick.pools_total}</strong></span>
          <span className="text-sm text-muted-foreground">Latency: <strong>{tick.latency_ms}ms</strong></span>
          <Badge variant={tick.routes_capped ? "destructive" : "secondary"} className="text-xs">
            {tick.routes_capped ? "routes capped" : "routes ok"}
          </Badge>
          <Badge variant="outline" className="text-xs capitalize">mode: {tick.mode}</Badge>
        </div>
      )}

      {/* Routes grid */}
      {view === "radar" &&
        (routes.length === 0 ? (
          <div className="rounded-lg border p-8 text-center text-sm text-muted-foreground" data-testid="routes-empty">
            {allRoutes.length > 0 && (activeKpi !== null || activeHops !== null)
              ? `Ninguna de las ${allRoutes.length} rutas servidas pasa el filtro activo.`
              : "No routes discovered yet — the route-discovery worker may still be initializing."}
          </div>
        ) : (
          <section data-testid="routes-grid">
            <h2 className="mb-3 text-sm font-semibold text-muted-foreground">
              Discovered Routes ({routes.length}
              {(activeKpi !== null || activeHops !== null) && routes.length !== allRoutes.length
                ? ` de ${allRoutes.length}`
                : ""}
              )
            </h2>
            <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4">
              {routes.map((route, idx) => (
                <RouteCard key={route.route_hash} route={route} idx={idx} />
              ))}
            </div>
          </section>
        ))}
    </div>
  );
}
