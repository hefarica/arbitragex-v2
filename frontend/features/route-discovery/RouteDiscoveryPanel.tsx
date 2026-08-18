"use client";

/**
 * RouteDiscoveryPanel — OMEGA Route Discovery radar (enterprise-premium).
 *
 * Per the shadcn-enterprise-premium-ui doctrine: status strip → compact metric
 * cards → dense operational table → live feed. Reads the public REST snapshot
 * (useRouteDiscoveryRest, 5s poll). R8 fail-honest: "—" for nulls, explicit
 * STALE/empty states, no fabricated values. Shadow-only — never writes
 * opportunities.
 */

import * as React from "react";
import { RadarIcon, AlertCircleIcon } from "lucide-react";

import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Skeleton } from "@/components/ui/skeleton";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Separator } from "@/components/ui/separator";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { useRouteDiscoveryRest } from "@/lib/hooks/useRouteDiscoveryRest";
import type { MergedRoute, RouteDiscoveryTick } from "@/lib/hooks/useRouteDiscoveryTelemetry";
import {
  StatusPill,
  ModeBadge,
  ReadOnlyBadge,
  MetricCard,
  CopyHash,
  ProtocolChips,
  StrategyBadges,
  Chip,
  Freshness,
} from "./premium-ui";

function num(v: unknown): number | null {
  return typeof v === "number" && Number.isFinite(v) ? v : null;
}
function str(v: unknown): string {
  return typeof v === "string" && v.length > 0 ? v : "—";
}

function RouteKindBadge({ kind }: { kind?: string | null }) {
  if (!kind) return <span className="text-muted-foreground">—</span>;
  const triangular = kind === "triangular";
  return (
    <Badge variant={triangular ? "default" : "secondary"} className="font-mono text-[10px]">
      {kind}
    </Badge>
  );
}

function DispatchCell({ r }: { r: MergedRoute }) {
  if (r.dispatch_deferred) {
    return (
      <Badge variant="outline" className="text-[10px] text-warning border-warning/50">
        {r.dispatch_deferred}
      </Badge>
    );
  }
  if (r.dispatch_strategy) {
    return (
      <Badge variant="outline" className="text-[10px] text-success border-success/50">
        {r.dispatch_strategy}
      </Badge>
    );
  }
  return <span className="text-muted-foreground">—</span>;
}

function MetricGrid({ tick, loading }: { tick: RouteDiscoveryTick | null; loading: boolean }) {
  const t = (tick ?? {}) as Record<string, unknown>;
  // SHADOW-NO-ROUTE-CAPS: deferred = paused, resumes next tick (never lost).
  const deferred = typeof t["routes_deferred"] === "boolean" ? (t["routes_deferred"] as boolean) : null;
  const rejected = num(t["edges_rejected"]);
  return (
    <div className="grid grid-cols-2 gap-2 sm:grid-cols-4">
      <MetricCard label="Routes found" value={num(t["routes_found"])} loading={loading} />
      <MetricCard
        label="Dispatched"
        value={num(t["routes_dispatched"])}
        loading={loading}
        tone="success"
      />
      <MetricCard
        label="Deferred"
        value={deferred}
        loading={loading}
        tone={deferred ? "warning" : "default"}
        hint={
          deferred
            ? `paused @ ${String(t["deferred_cursor"] ?? "?")} — resumes next tick, nothing lost`
            : undefined
        }
      />
      <MetricCard
        label="Ladder"
        value={`${num(t["depth_pass"])}${t["ladder_complete"] ? " ✓" : " …"} · ${num(t["pass_emitted_total"])}`}
        loading={loading}
        hint="iterative-deepening depth / cumulative routes this ladder"
      />
      <MetricCard label="Latency" value={num(t["latency_ms"])} hint="ms / tick" loading={loading} />
      <MetricCard label="Pools" value={num(t["pools_total"])} loading={loading} />
      <MetricCard label="Edges built" value={num(t["edges_built"])} loading={loading} />
      <MetricCard
        label="Edges rejected"
        value={rejected}
        loading={loading}
        tone={rejected && rejected > 0 ? "warning" : "muted"}
      />
    </div>
  );
}

function RoutesTable({ routes }: { routes: MergedRoute[] }) {
  return (
    <div className="rounded-lg border">
      <div className="max-h-[22rem] overflow-auto">
        <Table>
          <TableHeader className="sticky top-0 bg-card">
            <TableRow>
              <TableHead className="text-[11px] uppercase tracking-wide">Route hash</TableHead>
              <TableHead className="text-[11px] uppercase tracking-wide">Kind</TableHead>
              <TableHead className="text-[11px] uppercase tracking-wide text-center">Hops</TableHead>
              <TableHead className="text-[11px] uppercase tracking-wide">Protocols</TableHead>
              <TableHead className="text-[11px] uppercase tracking-wide">Fees</TableHead>
              <TableHead className="text-[11px] uppercase tracking-wide">Applicable</TableHead>
              <TableHead className="text-[11px] uppercase tracking-wide">Dispatch</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {routes.map((r) => (
              <TableRow key={r.route_hash}>
                <TableCell>
                  <CopyHash value={r.route_hash} />
                </TableCell>
                <TableCell>
                  <RouteKindBadge kind={r.route_kind} />
                </TableCell>
                <TableCell className="text-center font-mono text-xs tabular-nums">
                  {num(r.hops) ?? "—"}
                </TableCell>
                <TableCell>
                  <ProtocolChips protocols={r.protocols} />
                </TableCell>
                <TableCell>
                  {r.fee_tiers && r.fee_tiers.length > 0 ? (
                    <span className="inline-flex flex-wrap gap-1">
                      {r.fee_tiers.map((f, i) => (
                        <Chip key={i}>{f === null ? "—" : f}</Chip>
                      ))}
                    </span>
                  ) : (
                    <span className="text-muted-foreground">—</span>
                  )}
                </TableCell>
                <TableCell>
                  <StrategyBadges strategies={r.applicable_strategies} />
                </TableCell>
                <TableCell>
                  <DispatchCell r={r} />
                </TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      </div>
    </div>
  );
}

export function RouteDiscoveryPanel() {
  const { lastTick, routes, recent, status, updatedAt } = useRouteDiscoveryRest();
  const connecting = status === "CONNECTING";
  const hasData = lastTick !== null || routes.length > 0;

  return (
    <Card data-slot="route-discovery-panel">
      <CardHeader className="gap-3">
        <div className="flex flex-wrap items-center justify-between gap-2">
          <CardTitle className="flex items-center gap-2 text-base">
            <RadarIcon className="h-5 w-5 text-primary" />
            OMEGA Route Discovery Radar
          </CardTitle>
          <div className="flex items-center gap-2">
            <Freshness at={updatedAt} />
            <StatusPill status={status} />
          </div>
        </div>
        <div className="flex flex-wrap items-center gap-2">
          <ModeBadge mode={(lastTick as { mode?: string } | null)?.mode ?? null} />
          <Badge variant="outline" className="font-mono text-[10px]">
            {str((lastTick as { algorithm?: string } | null)?.algorithm)}
          </Badge>
          <ReadOnlyBadge label="shadow-only · does not write opportunities" />
        </div>
      </CardHeader>
      <CardContent className="space-y-4">
        {status === "STALE" && !hasData ? (
          <Alert variant="destructive">
            <AlertCircleIcon className="h-4 w-4" />
            <AlertTitle>No telemetry snapshot</AlertTitle>
            <AlertDescription className="text-sm">
              The route-discovery snapshot is unavailable — the api-server/edge may be
              unreachable, or the radar is not running in shadow
              (ARBX_ROUTE_DISCOVERY_MODE=shadow). Polling /api/route-discovery every 5s.
            </AlertDescription>
          </Alert>
        ) : (
          <MetricGrid tick={lastTick} loading={connecting && !hasData} />
        )}

        {routes.length > 0 && (
          <div className="space-y-2">
            <div className="flex items-center justify-between">
              <h4 className="text-sm font-semibold">Discovered routes</h4>
              <span className="text-xs text-muted-foreground">
                showing {routes.length}
                {(lastTick as { routes_deferred?: boolean } | null)?.routes_deferred
                  ? " · deferred (enumeration continues next tick)"
                  : ""}
              </span>
            </div>
            <RoutesTable routes={routes} />
          </div>
        )}

        {connecting && !hasData && (
          <div className="space-y-2">
            <Skeleton className="h-4 w-40" />
            <Skeleton className="h-24 w-full" />
          </div>
        )}

        {hasData && recent.length > 0 && (
          <div className="space-y-2">
            <Separator />
            <h4 className="text-sm font-semibold">Live event feed</h4>
            <div className="max-h-44 space-y-0.5 overflow-y-auto rounded-md bg-muted/30 p-2 font-mono text-[11px]">
              {recent.slice(0, 14).map((m, i) => {
                const ev = (m as { event?: string }).event ?? "?";
                const rh = (m as { route_hash?: string }).route_hash;
                const reason = (m as { reason?: string }).reason;
                return (
                  <div key={i} className="flex items-center gap-2 truncate text-muted-foreground">
                    <span className="text-foreground">{ev}</span>
                    {rh ? <span className="truncate">{`${rh.slice(0, 10)}…`}</span> : null}
                    {reason ? <span>{`reason=${reason}`}</span> : null}
                  </div>
                );
              })}
            </div>
          </div>
        )}
      </CardContent>
    </Card>
  );
}
