"use client";

/**
 * RouteDiscoveryPanel — OMEGA Route Discovery radar telemetry surface.
 *
 * Consumes the "route_discovery" WebSocket room via useRouteDiscoveryTelemetry.
 * Displays the live tick summary (topology metrics), a merged routes table
 * (candidate + applicability + dispatch), and a raw live event feed.
 *
 * R8 fail-honest: null values render as "—", never "0". The radar is SHADOW-ONLY
 * — it discovers/classifies topology and never writes opportunities. Zero-Mocks:
 * everything comes from the WS stream; empty = show empty/STALE.
 */

import * as React from "react";
import {
  RadarIcon,
  ActivityIcon,
  ClockIcon,
  AlertCircleIcon,
  ShieldCheckIcon,
} from "lucide-react";

import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Skeleton } from "@/components/ui/skeleton";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import {
  useRouteDiscoveryTelemetry,
  type MergedRoute,
  type RouteDiscoveryTick,
  type TelemetryStatus,
} from "@/lib/hooks/useRouteDiscoveryTelemetry";

function num(v: unknown): string {
  return typeof v === "number" && Number.isFinite(v) ? String(v) : "—";
}
function str(v: unknown): string {
  return typeof v === "string" && v.length > 0 ? v : "—";
}
function shortHash(h?: string): string {
  if (!h) return "—";
  return h.length > 18 ? `${h.slice(0, 10)}…${h.slice(-6)}` : h;
}

function Metric({ label, value }: { label: string; value: string }) {
  return (
    <div className="bg-muted/40 rounded p-2">
      <div className="text-xs text-muted-foreground">{label}</div>
      <div className="font-mono font-semibold">{value}</div>
    </div>
  );
}

function StatusBadge({ status }: { status: TelemetryStatus }) {
  if (status === "LIVE")
    return (
      <Badge variant="outline" className="text-success border-success">
        <ActivityIcon className="w-3 h-3 mr-1" />
        LIVE
      </Badge>
    );
  if (status === "CONNECTING")
    return (
      <Badge variant="outline" className="text-warning border-warning">
        <ClockIcon className="w-3 h-3 mr-1" />
        CONNECTING
      </Badge>
    );
  return (
    <Badge variant="outline" className="text-destructive border-destructive">
      STALE
    </Badge>
  );
}

function TickMetrics({ tick }: { tick: RouteDiscoveryTick }) {
  return (
    <div className="grid grid-cols-3 md:grid-cols-5 gap-2 text-sm">
      <Metric label="Algorithm" value={str(tick.algorithm)} />
      <Metric label="Pools" value={num(tick.pools_total)} />
      <Metric label="Edges built" value={num(tick.edges_built)} />
      <Metric label="Edges rejected" value={num(tick.edges_rejected)} />
      <Metric label="Latency (ms)" value={num(tick.latency_ms)} />
      <Metric label="Routes found" value={num(tick.routes_found)} />
      <Metric label="Dispatched" value={num(tick.routes_dispatched)} />
      <Metric label="Dropped (cap)" value={num(tick.routes_dropped_for_cap)} />
      <Metric
        label="Capped"
        value={typeof tick.routes_capped === "boolean" ? String(tick.routes_capped) : "—"}
      />
      <Metric label="Mode" value={str(tick.mode)} />
    </div>
  );
}

function RouteRow({ r }: { r: MergedRoute }) {
  return (
    <div className="grid grid-cols-12 gap-2 px-2 py-1.5 text-xs border-b border-border/40 items-center">
      <div className="col-span-3 font-mono truncate" title={r.route_hash}>
        {shortHash(r.route_hash)}
      </div>
      <div className="col-span-1">{str(r.route_kind)}</div>
      <div className="col-span-1 text-center">{num(r.hops)}</div>
      <div className="col-span-2 font-mono truncate">
        {r.protocols && r.protocols.length > 0 ? r.protocols.join("→") : "—"}
      </div>
      <div className="col-span-1 font-mono truncate">
        {r.fee_tiers && r.fee_tiers.length > 0
          ? r.fee_tiers.map((f) => (f === null ? "—" : f)).join("/")
          : "—"}
      </div>
      <div className="col-span-2 truncate" title={(r.applicable_strategies ?? []).join(", ")}>
        {r.applicable_strategies && r.applicable_strategies.length > 0
          ? r.applicable_strategies.join(", ")
          : "—"}
      </div>
      <div className="col-span-2 truncate">
        {r.dispatch_deferred ? (
          <Badge variant="outline" className="text-warning border-warning">
            {r.dispatch_deferred}
          </Badge>
        ) : r.dispatch_strategy ? (
          <Badge variant="outline" className="text-success border-success">
            {r.dispatch_strategy}
          </Badge>
        ) : (
          "—"
        )}
      </div>
    </div>
  );
}

function LoadingRows() {
  return (
    <div className="space-y-2">
      <Skeleton className="h-4 w-3/4" />
      <Skeleton className="h-4 w-1/2" />
      <Skeleton className="h-4 w-2/3" />
    </div>
  );
}

export function RouteDiscoveryPanel() {
  const { lastTick, routes, messages, status } = useRouteDiscoveryTelemetry();
  const hasData = lastTick !== null || routes.length > 0;

  return (
    <Card data-slot="route-discovery-panel">
      <CardHeader>
        <CardTitle className="flex items-center gap-2 text-base flex-wrap">
          <RadarIcon className="w-5 h-5 text-primary" />
          OMEGA Route Discovery Radar
          <StatusBadge status={status} />
          <Badge variant="outline" className="text-muted-foreground">
            <ShieldCheckIcon className="w-3 h-3 mr-1" />
            shadow-only · does not write opportunities
          </Badge>
        </CardTitle>
      </CardHeader>
      <CardContent className="space-y-4">
        {status === "CONNECTING" && !hasData && <LoadingRows />}
        {status === "STALE" && !hasData && (
          <Alert variant="destructive">
            <AlertCircleIcon className="w-4 h-4" />
            <AlertTitle>WebSocket disconnected</AlertTitle>
            <AlertDescription className="text-sm">
              Waiting for route-discovery telemetry. Admin session required for WebSocket
              access, and the radar must be running in shadow
              (ARBX_ROUTE_DISCOVERY_MODE=shadow).
            </AlertDescription>
          </Alert>
        )}

        {lastTick && (
          <div>
            <h4 className="text-sm font-semibold mb-2">Last tick</h4>
            <TickMetrics tick={lastTick} />
          </div>
        )}

        {routes.length > 0 && (
          <div>
            <h4 className="text-sm font-semibold mb-2">
              Discovered routes ({routes.length})
            </h4>
            <div className="rounded border border-border/40">
              <div className="grid grid-cols-12 gap-2 px-2 py-1.5 text-[11px] uppercase text-muted-foreground border-b border-border/60">
                <div className="col-span-3">route_hash</div>
                <div className="col-span-1">kind</div>
                <div className="col-span-1 text-center">hops</div>
                <div className="col-span-2">protocols</div>
                <div className="col-span-1">fees</div>
                <div className="col-span-2">applicable</div>
                <div className="col-span-2">dispatch</div>
              </div>
              <div className="max-h-80 overflow-y-auto">
                {routes.map((r) => (
                  <RouteRow key={r.route_hash} r={r} />
                ))}
              </div>
            </div>
          </div>
        )}

        {hasData && (
          <div>
            <h4 className="text-sm font-semibold mb-2">
              Live event feed (last {Math.min(messages.length, 12)})
            </h4>
            <div className="font-mono text-[11px] bg-muted/30 rounded p-2 max-h-48 overflow-y-auto space-y-0.5">
              {messages
                .slice(-12)
                .reverse()
                .map((m, i) => (
                  <div key={i} className="truncate text-muted-foreground">
                    <span className="text-foreground">{str(m.event)}</span>{" "}
                    {(m as { route_hash?: string }).route_hash
                      ? shortHash((m as { route_hash?: string }).route_hash)
                      : (m as { reason?: string }).reason
                        ? `reason=${(m as { reason?: string }).reason}`
                        : ""}
                  </div>
                ))}
            </div>
          </div>
        )}
      </CardContent>
    </Card>
  );
}
