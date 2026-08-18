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
import { useEffect, useState } from "react";
import { RadarIcon, ZapIcon, NetworkIcon } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { DegradedBanner } from "@/components/DegradedBanner";
import { SourceMeta } from "@/components/SourceMeta";

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
    /** SHADOW-NO-ROUTE-CAPS: deferred = budget paused the exhaustive
     *  enumeration; the cursor resumes next tick. NOT a loss. */
    routes_deferred: boolean;
    deferred_cursor?: string | null;
    depth_pass?: number;
    pass_emitted_total?: number;
    ladder_complete?: boolean;
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

  const routes = data.routes?.routes ?? [];
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

      {/* Status strip */}
      {tick && (
        <div className="flex flex-wrap items-center gap-4 rounded-lg border p-4" data-testid="routes-discovery-status">
          <div className="flex items-center gap-2">
            <RadarIcon className="h-4 w-4 text-primary" />
            <span className="text-sm font-medium">Algorithm:</span>
            <Badge variant="outline" className="text-xs">{tick.algorithm}</Badge>
          </div>
          <span className="text-sm text-muted-foreground">Edges built: <strong>{tick.edges_built}</strong></span>
          <span className="text-sm text-muted-foreground">Pools: <strong>{tick.pools_total}</strong></span>
          <span className="text-sm text-muted-foreground">Latency: <strong>{tick.latency_ms}ms</strong></span>
          <Badge
            variant={tick.routes_deferred ? "default" : "secondary"}
            className="text-xs"
            title={
              tick.routes_deferred
                ? `Exhaustive enumeration paused by tick budget — resumes at cursor ${tick.deferred_cursor ?? "?"} next tick. No routes are lost (SHADOW-NO-ROUTE-CAPS).`
                : "Enumeration running within budget this tick"
            }
          >
            {tick.routes_deferred
              ? `deferred (continues @ ${tick.deferred_cursor ?? "?"})`
              : "routes ok"}
          </Badge>
          <Badge variant="outline" className="text-xs capitalize">mode: {tick.mode}</Badge>
        </div>
      )}

      {/* Routes grid */}
      {routes.length === 0 ? (
        <div className="rounded-lg border p-8 text-center text-sm text-muted-foreground" data-testid="routes-empty">
          No routes discovered yet — the route-discovery worker may still be initializing.
        </div>
      ) : (
        <section data-testid="routes-grid">
          <h2 className="mb-3 text-sm font-semibold text-muted-foreground">
            Discovered Routes ({routes.length})
          </h2>
          <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4">
            {routes.map((route, idx) => (
              <RouteCard key={route.route_hash} route={route} idx={idx} />
            ))}
          </div>
        </section>
      )}
    </div>
  );
}
