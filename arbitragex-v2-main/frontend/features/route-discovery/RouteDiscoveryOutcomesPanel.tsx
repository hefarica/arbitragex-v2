"use client";

/**
 * RouteDiscoveryOutcomesPanel — FASE B Gate-C hit-rate + reason distribution.
 *
 * The read-side of the passive `route_discovery_outcomes` sink: it answers the
 * operator's burning question — "why are there 0 opportunities?" — with the
 * honest reason breakdown (v3_sizing_pending, insufficient_pools, …) rather
 * than a fabricated zero. Polls the public REST snapshot
 * (useRouteDiscoveryOutcomes, 8s) via the edge. R8 fail-honest: 503 → STALE
 * with the upstream reason surfaced verbatim; "—" for null counts; explicit
 * empty state. Shadow / read-only — never writes opportunities or capital.
 */

import * as React from "react";
import { RadarIcon, AlertCircleIcon, PercentIcon } from "lucide-react";

import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
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
import { cn } from "@/lib/utils";
import {
  useRouteDiscoveryOutcomes,
  type OutcomeReasonRow,
} from "@/lib/hooks/useRouteDiscoveryOutcomes";
import { StatusPill, MetricCard, Freshness, ReadOnlyBadge } from "./premium-ui";

const WINDOWS: Array<{ label: string; hours: number }> = [
  { label: "24h", hours: 24 },
  { label: "3d", hours: 72 },
  { label: "7d", hours: 168 },
  { label: "14d", hours: 336 },
];

type Tone = "muted" | "warning" | "destructive" | "success" | "default";

function reasonTone(reason: string): Tone {
  const r = reason.toLowerCase();
  if (r === "(null)" || r === "(unknown)") return "muted";
  if (r.includes("pending")) return "warning";
  if (
    r.includes("insufficient") ||
    r.includes("missing") ||
    r.includes("no_") ||
    r.includes("failed") ||
    r.includes("empty") ||
    r.includes("zero")
  ) {
    return "destructive";
  }
  if (r.includes("opportunity") || r.includes("dispatch") || r.includes("profit")) return "success";
  return "default";
}

const BAR_FILL: Record<Tone, string> = {
  muted: "bg-muted-foreground/40",
  warning: "bg-warning/70",
  destructive: "bg-destructive/70",
  success: "bg-success/70",
  default: "bg-primary/60",
};

function pctOf(n: number, total: number | null): number {
  return total && total > 0 ? (n / total) * 100 : 0;
}

function ReasonRow({ row, total }: { row: OutcomeReasonRow; total: number | null }) {
  const pct = pctOf(row.n, total);
  const tone = reasonTone(row.reason);
  return (
    <div className="space-y-1">
      <div className="flex items-center justify-between gap-2 text-xs">
        <span className="truncate font-mono text-foreground" title={row.reason}>
          {row.reason}
        </span>
        <span className="shrink-0 tabular-nums text-muted-foreground">
          {row.n.toLocaleString()} · {pct.toFixed(1)}%
        </span>
      </div>
      <div className="h-1.5 w-full overflow-hidden rounded-full bg-muted">
        <div
          className={cn("h-full rounded-full transition-[width]", BAR_FILL[tone])}
          style={{ width: `${Math.min(pct, 100).toFixed(2)}%` }}
          aria-hidden
        />
      </div>
    </div>
  );
}

export function RouteDiscoveryOutcomesPanel() {
  const [hours, setHours] = React.useState(24);
  const { totals, byReason, byChain, windowHours, status, updatedAt, unavailableReason } =
    useRouteDiscoveryOutcomes(hours);

  const connecting = status === "CONNECTING";
  const total = totals?.total ?? null;
  const opps = totals?.opportunities ?? null;
  const hasData = total !== null;
  const hitRateStr =
    total !== null && total > 0 && opps !== null ? `${((opps / total) * 100).toFixed(4)}%` : null;
  const loading = connecting && !hasData;

  return (
    <Card data-slot="route-discovery-outcomes-panel">
      <CardHeader className="gap-3">
        <div className="flex flex-wrap items-center justify-between gap-2">
          <CardTitle className="flex items-center gap-2 text-base">
            <RadarIcon className="h-5 w-5 text-primary" />
            Gate-C — Route Discovery Outcomes
          </CardTitle>
          <div className="flex items-center gap-2">
            <Freshness at={updatedAt} />
            <StatusPill status={status} />
          </div>
        </div>
        <div className="flex flex-wrap items-center justify-between gap-2">
          <div className="flex items-center gap-2">
            <ReadOnlyBadge label="shadow-only · hit-rate series, no opportunities written" />
            {windowHours !== null ? (
              <Badge variant="outline" className="font-mono text-[10px]">
                window {windowHours}h
              </Badge>
            ) : null}
          </div>
          <div className="flex items-center gap-1">
            {WINDOWS.map((w) => (
              <Button
                key={w.hours}
                type="button"
                size="sm"
                variant={hours === w.hours ? "default" : "outline"}
                className="h-7 px-2.5 text-[11px]"
                onClick={() => setHours(w.hours)}
              >
                {w.label}
              </Button>
            ))}
          </div>
        </div>
      </CardHeader>

      <CardContent className="space-y-4">
        {status === "STALE" && !hasData ? (
          <Alert variant="destructive">
            <AlertCircleIcon className="h-4 w-4" />
            <AlertTitle>Outcomes series unavailable</AlertTitle>
            <AlertDescription className="text-sm">
              The route-discovery outcomes summary could not be read
              {unavailableReason ? (
                <>
                  {" "}
                  (<span className="font-mono">{unavailableReason}</span>)
                </>
              ) : null}
              . api-server returns 503 when Postgres is unreachable; the panel never
              fabricates a zero. Polling /api/route-discovery-outcomes/summary every 8s.
            </AlertDescription>
          </Alert>
        ) : (
          <div className="grid grid-cols-2 gap-2 sm:grid-cols-3 lg:grid-cols-6">
            <MetricCard label="Outcomes" value={total} loading={loading} />
            <MetricCard
              label="Opportunities"
              value={opps}
              loading={loading}
              tone={opps && opps > 0 ? "success" : "muted"}
            />
            <MetricCard
              label="Hit-rate"
              value={hitRateStr}
              hint="opps / outcomes"
              loading={loading}
              tone={opps && opps > 0 ? "success" : "muted"}
            />
            <MetricCard label="With reserves" value={totals?.with_reserves ?? null} loading={loading} />
            <MetricCard
              label="Profit > 0"
              value={totals?.profit_gt0 ?? null}
              loading={loading}
              tone={totals?.profit_gt0 && totals.profit_gt0 > 0 ? "success" : "muted"}
            />
            <MetricCard label="Cartridges" value={totals?.cartridges ?? null} loading={loading} />
          </div>
        )}

        {hasData && total === 0 && (
          <Alert>
            <AlertCircleIcon className="h-4 w-4" />
            <AlertTitle>No outcomes in window</AlertTitle>
            <AlertDescription className="text-sm">
              The shadow emitter has not resolved any route-discovery outcomes in the
              last {windowHours ?? hours}h. This is an honest empty state, not an error.
            </AlertDescription>
          </Alert>
        )}

        {hasData && total !== null && total > 0 && byReason.length > 0 && (
          <div className="space-y-3">
            <div className="flex items-center justify-between">
              <h4 className="flex items-center gap-1.5 text-sm font-semibold">
                <PercentIcon className="h-4 w-4 text-muted-foreground" />
                Resolution reason distribution
              </h4>
              <span className="text-xs text-muted-foreground">why outcomes did / didn&apos;t convert</span>
            </div>
            <div className="space-y-2.5 rounded-lg border p-3">
              {byReason.map((r) => (
                <ReasonRow key={r.reason} row={r} total={total} />
              ))}
            </div>
            <p className="text-[11px] text-muted-foreground">
              <span className="font-mono">(null)</span> = outcomes persisted before the Paso 9
              reason column was populated (no reason emitted by the cartridge for that tick).
            </p>
          </div>
        )}

        {hasData && total !== null && total > 0 && byChain.length > 0 && (
          <div className="space-y-2">
            <Separator />
            <h4 className="text-sm font-semibold">By chain</h4>
            <div className="rounded-lg border">
              <Table>
                <TableHeader className="bg-card">
                  <TableRow>
                    <TableHead className="text-[11px] uppercase tracking-wide">Chain ID</TableHead>
                    <TableHead className="text-[11px] uppercase tracking-wide text-right">Outcomes</TableHead>
                    <TableHead className="text-[11px] uppercase tracking-wide text-right">Opportunities</TableHead>
                    <TableHead className="text-[11px] uppercase tracking-wide text-right">Hit-rate</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {byChain.map((c) => {
                    const hr = c.n > 0 ? `${((c.opportunities / c.n) * 100).toFixed(4)}%` : "—";
                    return (
                      <TableRow key={c.chain_id ?? "unknown"}>
                        <TableCell className="font-mono text-xs">{c.chain_id ?? "—"}</TableCell>
                        <TableCell className="text-right font-mono text-xs tabular-nums">
                          {c.n.toLocaleString()}
                        </TableCell>
                        <TableCell
                          className={cn(
                            "text-right font-mono text-xs tabular-nums",
                            c.opportunities > 0 ? "text-success" : "text-muted-foreground",
                          )}
                        >
                          {c.opportunities.toLocaleString()}
                        </TableCell>
                        <TableCell className="text-right font-mono text-xs tabular-nums text-muted-foreground">
                          {hr}
                        </TableCell>
                      </TableRow>
                    );
                  })}
                </TableBody>
              </Table>
            </div>
          </div>
        )}
      </CardContent>
    </Card>
  );
}
