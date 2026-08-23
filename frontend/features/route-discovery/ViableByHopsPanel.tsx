"use client";

/**
 * ViableByHopsPanel — XLS-DASH-01 (workbook 29_SUPER_DASHBOARD KPI set).
 *
 * Serves the workbook's route-funnel KPIs from REAL opportunities rows via
 * useViableKpis (edge → api-server /api/v1/analytics/viable-kpis):
 *   - Viable by HOPS — viable opportunities grouped by
 *     jsonb_array_length(route_metadata->'pool_addresses'), the persisted
 *     multi-hop topology.
 *   - Viable by KIND — grouped by canonical strategy_kind (cartridge stems).
 *   - Viability % — viable / total in window (null → "—", R8 never 0%).
 * Memory-disciplined: no framer-motion (MEM-RENDER-01); CSS transitions only.
 * Read-only / observe-only.
 */

import * as React from "react";
import { LayersIcon, AlertCircleIcon } from "lucide-react";

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
import { useViableKpis } from "@/lib/hooks/useViableKpis";
import { StatusPill, MetricCard, Freshness, ReadOnlyBadge } from "./premium-ui";

const WINDOWS: Array<{ label: string; hours: number }> = [
  { label: "24h", hours: 24 },
  { label: "3d", hours: 72 },
  { label: "7d", hours: 168 },
  { label: "14d", hours: 336 },
];

export function ViableByHopsPanel() {
  const [hours, setHours] = React.useState(24);
  const { totals, byHops, byKind, windowHours, status, updatedAt, unavailableReason } =
    useViableKpis(hours);

  const connecting = status === "CONNECTING";
  const viable = totals?.viable ?? null;
  const hasData = totals !== null;
  const loading = connecting && !hasData;
  const maxHopsN = byHops.reduce((m, r) => Math.max(m, r.n), 0);
  const totalKindN = byKind.reduce((s, r) => s + r.n, 0);

  return (
    <Card data-slot="viable-by-hops-panel">
      <CardHeader className="gap-3">
        <div className="flex flex-wrap items-center justify-between gap-2">
          <CardTitle className="flex items-center gap-2 text-base">
            <LayersIcon className="h-5 w-5 text-primary" />
            Viable Routes — by Hops &amp; Kind
          </CardTitle>
          <div className="flex items-center gap-2">
            <Freshness at={updatedAt} />
            <StatusPill status={status} />
          </div>
        </div>
        <div className="flex flex-wrap items-center justify-between gap-2">
          <div className="flex items-center gap-2">
            <ReadOnlyBadge label="read-only · real opportunities rows" />
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
            <AlertTitle>Viable-KPI set unavailable</AlertTitle>
            <AlertDescription className="text-sm">
              The viable-by-hops summary could not be read
              {unavailableReason ? (
                <>
                  {" "}
                  (<span className="font-mono">{unavailableReason}</span>)
                </>
              ) : null}
              . api-server returns 503 when Postgres is unreachable; the panel never
              fabricates a zero. Polling /api/viable-kpis every 10s.
            </AlertDescription>
          </Alert>
        ) : (
          <div className="grid grid-cols-2 gap-2 sm:grid-cols-4">
            <MetricCard label="Viable" value={viable} loading={loading} tone={viable && viable > 0 ? "success" : "muted"} />
            <MetricCard
              label="Viability"
              value={totals?.viability_pct !== null && totals?.viability_pct !== undefined ? `${totals.viability_pct}%` : null}
              hint="viable / detected"
              loading={loading}
              tone={totals?.viability_pct && totals.viability_pct > 0 ? "success" : "muted"}
            />
            <MetricCard label="Detected" value={totals?.total ?? null} loading={loading} />
            <MetricCard label="With route topology" value={totals?.routed ?? null} loading={loading} />
          </div>
        )}

        {hasData && byHops.length === 0 && viable !== null && viable > 0 && (
          <Alert>
            <AlertCircleIcon className="h-4 w-4" />
            <AlertTitle>No persisted route topology in window</AlertTitle>
            <AlertDescription className="text-sm">
              {viable.toLocaleString()} viable opportunities exist but none carry a populated
              route_metadata topology yet (legacy rows or pre-multi-path emit). Hops grouping
              covers exactly the rows the searcher persisted legs for — never estimated.
            </AlertDescription>
          </Alert>
        )}

        {byHops.length > 0 && (
          <div className="space-y-3">
            <div className="flex items-center justify-between">
              <h4 className="text-sm font-semibold">Viable by hops</h4>
              <span className="text-xs text-muted-foreground">
                route_metadata pool count per viable row
              </span>
            </div>
            <div className="space-y-2.5 rounded-lg border p-3">
              {byHops.map((r) => {
                const pct = maxHopsN > 0 ? (r.n / maxHopsN) * 100 : 0;
                return (
                  <div key={r.hops} className="space-y-1">
                    <div className="flex items-center justify-between gap-2 text-xs">
                      <span className="font-mono text-foreground">
                        {r.hops} {r.hops === 1 ? "hop" : "hops"}
                      </span>
                      <span className="shrink-0 tabular-nums text-muted-foreground">
                        {r.n.toLocaleString()}
                      </span>
                    </div>
                    <div className="h-1.5 w-full overflow-hidden rounded-full bg-muted">
                      <div
                        className="h-full rounded-full bg-primary/60 transition-[width]"
                        style={{ width: `${Math.min(pct, 100).toFixed(2)}%` }}
                        aria-hidden
                      />
                    </div>
                  </div>
                );
              })}
            </div>
          </div>
        )}

        {byKind.length > 0 && (
          <div className="space-y-2">
            <Separator />
            <h4 className="text-sm font-semibold">Viable by strategy kind</h4>
            <div className="rounded-lg border">
              <Table>
                <TableHeader className="bg-card">
                  <TableRow>
                    <TableHead className="text-[11px] uppercase tracking-wide">Strategy kind</TableHead>
                    <TableHead className="text-[11px] uppercase tracking-wide text-right">Viable</TableHead>
                    <TableHead className="text-[11px] uppercase tracking-wide text-right">Share</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {byKind.map((k) => (
                    <TableRow key={k.strategy_kind}>
                      <TableCell className="max-w-[280px] truncate font-mono text-xs" title={k.strategy_kind}>
                        {k.strategy_kind}
                      </TableCell>
                      <TableCell
                        className={cn(
                          "text-right font-mono text-xs tabular-nums",
                          k.n > 0 ? "text-success" : "text-muted-foreground",
                        )}
                      >
                        {k.n.toLocaleString()}
                      </TableCell>
                      <TableCell className="text-right font-mono text-xs tabular-nums text-muted-foreground">
                        {totalKindN > 0 ? `${((k.n / totalKindN) * 100).toFixed(1)}%` : "—"}
                      </TableCell>
                    </TableRow>
                  ))}
                </TableBody>
              </Table>
            </div>
          </div>
        )}
      </CardContent>
    </Card>
  );
}
