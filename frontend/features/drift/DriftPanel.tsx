"use client";

import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Skeleton } from "@/components/ui/skeleton";
import { useDriftDetection } from "@/lib/drift/useDriftDetection";
import { ACTION_STATE_LABELS } from "@/lib/statemachine/types";

/**
 * Maps a DriftStatus to the Badge visual variant.
 */
function driftStatusVariant(
  status: "consistent" | "drift_detected" | "partial" | "unknown",
): "default" | "secondary" | "destructive" | "outline" {
  switch (status) {
    case "consistent":
      return "outline";
    case "drift_detected":
      return "destructive";
    case "partial":
      return "secondary";
    case "unknown":
    default:
      return "default";
  }
}

/**
 * Maps a DriftStatus to a human-readable label.
 */
function driftStatusLabel(status: "consistent" | "drift_detected" | "partial" | "unknown"): string {
  switch (status) {
    case "consistent":
      return "CONSISTENT";
    case "drift_detected":
      return "DRIFT DETECTED";
    case "partial":
      return "PARTIAL";
    case "unknown":
      return "UNKNOWN";
  }
}

/**
 * Maps a drift action to a Badge variant for per-row display.
 */
function driftActionVariant(
  action: "mismatch" | "missing_in_a" | "missing_in_b" | "stale",
): "default" | "secondary" | "destructive" | "outline" {
  switch (action) {
    case "mismatch":
      return "destructive";
    case "missing_in_a":
    case "missing_in_b":
      return "secondary";
    case "stale":
      return "outline";
    default:
      return "default";
  }
}

/**
 * DriftPanel — real-time cross-layer consistency monitor.
 *
 * Polls the drift endpoint every 15 seconds and renders:
 *   - Overall status badge (CONSISTENT / DRIFT DETECTED / PARTIAL)
 *   - Per-entity difference list with layer mismatch details
 *   - Loading skeleton while the first check is in flight
 *   - Last-check timestamp
 *
 * Place this panel on the Ops Dashboard or System Status page for
 * at-a-glance confidence that Postgres, Redis, and runtime are in sync.
 */
export function DriftPanel() {
  const { report, isLoading, lastError, checkDrift } = useDriftDetection(15_000);

  const status = report?.status ?? "unknown";

  return (
    <Card className="w-full">
      <CardHeader className="flex flex-row items-center justify-between">
        <CardTitle className="flex items-center gap-2 text-base font-semibold">
          <span
            className="inline-block h-2.5 w-2.5 rounded-full"
            aria-hidden="true"
            style={{
              backgroundColor:
                status === "consistent"
                  ? "#22c55e"
                  : status === "drift_detected"
                    ? "#ef4444"
                    : status === "partial"
                      ? "#f59e0b"
                      : "#6b7280",
            }}
          />
          Drift Detection
          <Badge variant={driftStatusVariant(status)}>{driftStatusLabel(status)}</Badge>
        </CardTitle>
        <div className="flex items-center gap-2">
          {report?.checked_at && (
            <span className="text-xs text-muted-foreground">
              Checked {new Date(report.checked_at).toLocaleTimeString()}
            </span>
          )}
          <button
            type="button"
            onClick={() => void checkDrift()}
            disabled={isLoading}
            className="text-xs rounded-md border px-2 py-1 hover:bg-accent disabled:opacity-50"
            aria-label="Refresh drift check now"
            title="Refresh now"
          >
            {isLoading ? "Checking…" : "Refresh"}
          </button>
        </div>
      </CardHeader>

      <CardContent className="space-y-3">
        {/* Loading state */}
        {isLoading && !report && (
          <div className="space-y-2">
            <Skeleton className="h-4 w-full" />
            <Skeleton className="h-4 w-3/4" />
            <Skeleton className="h-4 w-1/2" />
          </div>
        )}

        {/* Error state */}
        {lastError && (
          <div className="rounded-md border border-destructive/50 bg-destructive/5 p-3 text-sm text-destructive">
            <strong>Check failed:</strong> {lastError}
          </div>
        )}

        {/* Summary metrics */}
        {report && (
          <div className="grid grid-cols-3 gap-2 text-center text-sm">
            <div className="rounded-md border p-2">
              <div className="text-muted-foreground text-xs">Entities</div>
              <div className="font-mono font-semibold">{report.total_entities_checked}</div>
            </div>
            <div className="rounded-md border p-2">
              <div className="text-muted-foreground text-xs">Differences</div>
              <div
                className={`font-mono font-semibold ${report.total_differences > 0 ? "text-destructive" : ""}`}
              >
                {report.total_differences}
              </div>
            </div>
            <div className="rounded-md border p-2">
              <div className="text-muted-foreground text-xs">Layers OK</div>
              <div className="font-mono font-semibold">
                {report.layers_reachable.length}/3
              </div>
            </div>
          </div>
        )}

        {/* Difference list */}
        {report && report.differences.length > 0 && (
          <div className="max-h-64 overflow-y-auto rounded-md border">
            <div className="bg-muted px-3 py-1.5 text-xs font-medium text-muted-foreground">
              Entity differences
            </div>
            {report.differences.map((diff, i) => (
              <div
                key={`${diff.entity_type}:${diff.entity_id}:${i}`}
                className="flex items-center justify-between border-b px-3 py-2 text-sm last:border-b-0"
              >
                <div className="flex items-center gap-2 min-w-0">
                  <span className="font-mono text-xs truncate">
                    {diff.entity_type}:{diff.entity_id}
                  </span>
                  <span className="text-muted-foreground text-xs whitespace-nowrap">
                    {diff.source_a} ≠ {diff.source_b}
                  </span>
                </div>
                <Badge variant={driftActionVariant(diff.action)} className="ml-2 shrink-0 text-xs">
                  {diff.action}
                </Badge>
              </div>
            ))}
          </div>
        )}

        {/* Empty / consistent state */}
        {report && report.differences.length === 0 && report.status === "consistent" && (
          <div className="rounded-md border border-green-500/20 bg-green-500/5 p-3 text-sm text-green-700">
            All layers are consistent — no drift detected across{" "}
            {report.total_entities_checked} checked entities.
          </div>
        )}

        {/* Unreachable layers warning */}
        {report && report.layers_unreachable.length > 0 && (
          <div className="rounded-md border border-amber-500/20 bg-amber-500/5 p-3 text-sm text-amber-700">
            <strong>Warning:</strong> Unreachable layers —{" "}
            {report.layers_unreachable.join(", ")}. Drift status may be incomplete.
          </div>
        )}
      </CardContent>
    </Card>
  );
}
