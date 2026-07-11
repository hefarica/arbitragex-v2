"use client";

/**
 * BlockersPanel — list of active readiness blockers with severity.
 *
 * Shows:
 *   - Each blocker: title, description, required action, severity badge
 *   - Severity levels: critical (red), high (orange), medium (yellow), low (blue)
 *   - Summary counts at the top
 *   - "✅ Sin bloqueadores" when the list is empty
 *
 * Fetches from /api/v1/readiness/blockers every 30s.
 *
 * R8 fail-honest: renders the verbatim blocker list from the backend.
 * When the endpoint errors, displays the error message — never "0 blockers".
 */

import * as React from "react";
import {
  AlertTriangle,
  AlertCircle,
  Info,
  ChevronDown,
  ChevronUp,
  Loader2,
  CheckCircle2,
  ShieldAlert,
} from "lucide-react";
import { getReadinessBlockers } from "@/lib/api-client";
import type { ReadinessBlocker, ReadinessBlockersResponse } from "@/lib/schemas";
import { cn } from "@/lib/utils";

// ─── Types ───────────────────────────────────────────────────────────────────

type LoadState =
  | { kind: "loading" }
  | { kind: "ok"; data: ReadinessBlockersResponse }
  | { kind: "error"; detail: string };

const POLL_MS = 30_000;

// ─── Severity helpers ────────────────────────────────────────────────────────

function severityRank(severity: string): number {
  switch (severity) {
    case "critical": return 4;
    case "high": return 3;
    case "medium": return 2;
    case "low": return 1;
    default: return 0;
  }
}

function severityIcon(severity: string) {
  switch (severity) {
    case "critical":
      return <ShieldAlert className="size-3.5 text-red-500" aria-hidden="true" />;
    case "high":
      return <AlertTriangle className="size-3.5 text-orange-500" aria-hidden="true" />;
    case "medium":
      return <AlertCircle className="size-3.5 text-amber-500" aria-hidden="true" />;
    default:
      return <Info className="size-3.5 text-blue-500" aria-hidden="true" />;
  }
}

function severityBadgeClass(severity: string): string {
  switch (severity) {
    case "critical":
      return "bg-red-500/10 text-red-700 dark:text-red-300 border-red-500/30";
    case "high":
      return "bg-orange-500/10 text-orange-700 dark:text-orange-300 border-orange-500/30";
    case "medium":
      return "bg-amber-500/10 text-amber-700 dark:text-amber-300 border-amber-500/30";
    default:
      return "bg-blue-500/10 text-blue-700 dark:text-blue-300 border-blue-500/30";
  }
}

// ─── Component ───────────────────────────────────────────────────────────────

export function BlockersPanel() {
  const [state, setState] = React.useState<LoadState>({ kind: "loading" });

  React.useEffect(() => {
    let alive = true;

    const tick = async () => {
      const result = await getReadinessBlockers();
      if (!alive) return;
      if (result.ok) {
        setState({ kind: "ok", data: result.data });
      } else {
        setState({ kind: "error", detail: result.error });
      }
    };

    void tick();
    const id = setInterval(tick, POLL_MS);
    return () => {
      alive = false;
      clearInterval(id);
    };
  }, []);

  // ─── Loading ─────────────────────────────────────────────────────────────

  if (state.kind === "loading") {
    return (
      <div className="rounded-lg border border-border bg-card p-4 shadow-sm">
        <div className="flex items-center gap-2 text-sm font-semibold text-card-foreground">
          <Loader2 className="size-4 animate-spin text-muted-foreground" aria-hidden="true" />
          Active Blockers
        </div>
        <div className="mt-4 space-y-3">
          <div className="h-4 w-full animate-pulse rounded bg-muted" />
          <div className="h-4 w-3/4 animate-pulse rounded bg-muted" />
          <div className="h-16 w-full animate-pulse rounded bg-muted" />
          <div className="h-16 w-full animate-pulse rounded bg-muted" />
        </div>
      </div>
    );
  }

  // ─── Error ───────────────────────────────────────────────────────────────

  if (state.kind === "error") {
    return (
      <div className="rounded-lg border border-destructive/30 bg-destructive/5 p-4 shadow-sm">
        <div className="flex items-center gap-2 text-sm font-semibold text-destructive">
          <ShieldAlert className="size-4" aria-hidden="true" />
          Active Blockers
        </div>
        <p className="mt-2 text-xs text-destructive/80">{state.detail}</p>
      </div>
    );
  }

  // ─── OK ──────────────────────────────────────────────────────────────────

  const { data } = state;
  const sortedBlockers = [...data.blockers].sort(
    (a, b) => severityRank(b.severity) - severityRank(a.severity),
  );

  return (
    <div className="rounded-lg border border-border bg-card p-4 shadow-sm">
      {/* Header with counts */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2 text-sm font-semibold text-card-foreground">
          <ShieldAlert className="size-4 text-muted-foreground" aria-hidden="true" />
          Active Blockers
          {data.blockers.length > 0 && (
            <span className="inline-flex min-w-[1.25rem] items-center justify-center rounded-full bg-destructive px-1.5 py-0.5 text-[10px] font-bold text-destructive-foreground">
              {data.blockers.length}
            </span>
          )}
        </div>
        {sortedBlockers.length > 0 && (
          <div className="flex items-center gap-2 text-[11px] text-muted-foreground">
            {data.summary.critical > 0 && (
              <span className="text-red-500">{data.summary.critical} critical</span>
            )}
            {data.summary.high > 0 && (
              <span className="text-orange-500">{data.summary.high} high</span>
            )}
            {data.summary.medium > 0 && (
              <span className="text-amber-500">{data.summary.medium} med</span>
            )}
            {data.summary.low > 0 && (
              <span className="text-blue-500">{data.summary.low} low</span>
            )}
          </div>
        )}
      </div>

      {/* Empty state */}
      {sortedBlockers.length === 0 ? (
        <div className="mt-6 flex flex-col items-center gap-2 py-6">
          <CheckCircle2 className="size-10 text-emerald-500" aria-hidden="true" />
          <p className="text-sm font-semibold text-emerald-600 dark:text-emerald-400">
            ✅ Sin bloqueadores
          </p>
          <p className="text-xs text-muted-foreground">
            All readiness checks passed. No blockers detected.
          </p>
        </div>
      ) : (
        /* Blocker list */
        <ul className="mt-4 space-y-2 max-h-[420px] overflow-y-auto pr-1">
          {sortedBlockers.map((blocker) => (
            <BlockerCard key={blocker.id} blocker={blocker} />
          ))}
        </ul>
      )}

      {/* Timestamp */}
      <p className="mt-3 text-[11px] text-muted-foreground">
        Updated: {new Date(data.generated_at).toLocaleTimeString()}
      </p>
    </div>
  );
}

// ─── Blocker card ────────────────────────────────────────────────────────────

function BlockerCard({ blocker }: { blocker: ReadinessBlocker }) {
  const [expanded, setExpanded] = React.useState(false);

  return (
    <li className="rounded-md border border-border bg-muted/20">
      <button
        type="button"
        onClick={() => setExpanded((v) => !v)}
        className="flex w-full items-start gap-2 px-2.5 py-2 text-left hover:bg-muted/40 transition-colors"
        aria-expanded={expanded}
      >
        <span className="mt-0.5 shrink-0">{severityIcon(blocker.severity)}</span>
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2">
            <span className="truncate text-xs font-semibold text-foreground">
              {blocker.title}
            </span>
            <span
              className={cn(
                "shrink-0 rounded border px-1 py-0.5 text-[10px] font-bold uppercase tracking-wide",
                severityBadgeClass(blocker.severity),
              )}
            >
              {blocker.severity}
            </span>
          </div>
          <p className="mt-0.5 truncate text-[11px] text-muted-foreground">
            {blocker.description}
          </p>
        </div>
        {expanded ? (
          <ChevronUp className="size-3.5 shrink-0 text-muted-foreground" aria-hidden="true" />
        ) : (
          <ChevronDown className="size-3.5 shrink-0 text-muted-foreground" aria-hidden="true" />
        )}
      </button>

      {expanded && (
        <div className="border-t border-border px-2.5 py-2 text-xs text-muted-foreground">
          <p>
            <strong className="text-foreground">Description:</strong> {blocker.description}
          </p>
          <p className="mt-1.5">
            <strong className="text-foreground">Action:</strong>{" "}
            {blocker.required_action}
          </p>
          {blocker.operator_required && (
            <p className="mt-1.5 font-semibold text-amber-600 dark:text-amber-400">
              Operator action required
            </p>
          )}
          {blocker.blocks.length > 0 && (
            <p className="mt-1.5">
              <strong className="text-foreground">Blocks:</strong>{" "}
              {blocker.blocks.join(", ")}
            </p>
          )}
        </div>
      )}
    </li>
  );
}
