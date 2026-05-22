"use client";

/**
 * AgentTeamsPanel — grid of agent cards with status indicators.
 *
 * Shows:
 *   - Grid of agent cards with colored status pulse (green/yellow/red)
 *   - Each agent: id, name, role (category), status, last action / next action
 *   - Summary bar with PASS / BLOCKED / PARTIAL / NO-GO counts
 *
 * Fetches from /api/v1/agents/status every 10s.
 *
 * R8 fail-honest: verdicts are workspace-verified and runtime-overlayed.
 * We never recolor a BLOCKED agent as green.
 */

import * as React from "react";
import {
  Loader2,
  ShieldAlert,
  CheckCircle2,
  AlertTriangle,
  XCircle,
  HelpCircle,
  Activity,
} from "lucide-react";
import { getAgentTeamsStatus } from "@/lib/api-client";
import type { AgentStatusRow, AgentsStatusResponse } from "@/lib/schemas";
import { cn } from "@/lib/utils";

// ─── Types ───────────────────────────────────────────────────────────────────

type LoadState =
  | { kind: "loading" }
  | { kind: "ok"; data: AgentsStatusResponse }
  | { kind: "error"; detail: string };

const POLL_MS = 10_000;

// ─── Status helpers ──────────────────────────────────────────────────────────

function statusColor(status: string): string {
  switch (status) {
    case "healthy":
      return "bg-emerald-500";
    case "degraded":
      return "bg-amber-500";
    case "blocked":
      return "bg-red-500";
    default:
      return "bg-gray-400";
  }
}

function verdictIcon(verdict: string) {
  switch (verdict) {
    case "PASS":
      return <CheckCircle2 className="size-3.5 text-emerald-500" aria-hidden="true" />;
    case "BLOCKED":
      return <XCircle className="size-3.5 text-red-500" aria-hidden="true" />;
    case "PARTIAL":
      return <AlertTriangle className="size-3.5 text-amber-500" aria-hidden="true" />;
    case "NO_GO":
      return <ShieldAlert className="size-3.5 text-red-500" aria-hidden="true" />;
    case "NOT_RUN":
      return <HelpCircle className="size-3.5 text-gray-400" aria-hidden="true" />;
    default:
      return <HelpCircle className="size-3.5 text-gray-400" aria-hidden="true" />;
  }
}

function verdictBadgeClass(verdict: string): string {
  switch (verdict) {
    case "PASS":
      return "bg-emerald-500/10 text-emerald-700 dark:text-emerald-300 border-emerald-500/30";
    case "BLOCKED":
      return "bg-red-500/10 text-red-700 dark:text-red-300 border-red-500/30";
    case "PARTIAL":
      return "bg-amber-500/10 text-amber-700 dark:text-amber-300 border-amber-500/30";
    case "NO_GO":
      return "bg-red-500/10 text-red-700 dark:text-red-300 border-red-500/30";
    default:
      return "bg-gray-500/10 text-gray-600 dark:text-gray-400 border-gray-500/30";
  }
}

function riskColor(risk: string): string {
  switch (risk) {
    case "critical": return "text-red-500";
    case "high": return "text-orange-500";
    case "medium": return "text-amber-500";
    default: return "text-emerald-500";
  }
}

// ─── Component ───────────────────────────────────────────────────────────────

export function AgentTeamsPanel() {
  const [state, setState] = React.useState<LoadState>({ kind: "loading" });

  React.useEffect(() => {
    let alive = true;

    const tick = async () => {
      const result = await getAgentTeamsStatus();
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
          Agent Teams
        </div>
        <div className="mt-4 grid grid-cols-1 gap-2 sm:grid-cols-2 lg:grid-cols-3">
          {Array.from({ length: 6 }).map((_, i) => (
            <div key={i} className="h-24 animate-pulse rounded-md bg-muted" />
          ))}
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
          Agent Teams
        </div>
        <p className="mt-2 text-xs text-destructive/80">{state.detail}</p>
      </div>
    );
  }

  // ─── OK ──────────────────────────────────────────────────────────────────

  const { data } = state;
  const summary = data.summary;

  return (
    <div className="rounded-lg border border-border bg-card p-4 shadow-sm">
      {/* Header */}
      <div className="flex flex-wrap items-center justify-between gap-2">
        <div className="flex items-center gap-2 text-sm font-semibold text-card-foreground">
          <Activity className="size-4 text-muted-foreground" aria-hidden="true" />
          Agent Teams
        </div>
        {/* Summary pills */}
        <div className="flex flex-wrap items-center gap-1.5 text-[11px]">
          <SummaryPill count={summary.pass} label="PASS" color="emerald" />
          <SummaryPill count={summary.blocked} label="BLOCKED" color="red" />
          <SummaryPill count={summary.partial} label="PARTIAL" color="amber" />
          <SummaryPill count={summary.no_go} label="NO-GO" color="red" />
          <span className="ml-1 text-muted-foreground">
            {summary.total} total
          </span>
        </div>
      </div>

      {/* Overall status bar */}
      <div className={cn(
        "mt-2 rounded-md border px-2.5 py-1.5 text-xs font-medium",
        data.overall_status === "ready"
          ? "border-emerald-500/30 bg-emerald-500/10 text-emerald-700 dark:text-emerald-300"
          : data.overall_status === "partial"
            ? "border-amber-500/30 bg-amber-500/10 text-amber-700 dark:text-amber-300"
            : "border-red-500/30 bg-red-500/10 text-red-700 dark:text-red-300",
      )}>
        Overall: <strong className="uppercase">{data.overall_status}</strong>
        {" — "}
        {summary.pass} of {summary.total} agents PASS
      </div>

      {/* Agent grid */}
      <div className="mt-4 grid grid-cols-1 gap-2 sm:grid-cols-2 lg:grid-cols-3">
        {data.agents.map((agent) => (
          <AgentCard key={agent.id} agent={agent} />
        ))}
      </div>

      {/* Timestamp */}
      <p className="mt-3 text-[11px] text-muted-foreground">
        Updated: {new Date(data.generated_at).toLocaleTimeString()}
      </p>
    </div>
  );
}

// ─── Summary pill ────────────────────────────────────────────────────────────

function SummaryPill({
  count,
  label,
  color,
}: {
  count: number;
  label: string;
  color: "emerald" | "red" | "amber";
}) {
  if (count === 0) return null;
  return (
    <span
      className={cn(
        "inline-flex items-center gap-1 rounded border px-1.5 py-0.5 font-semibold",
        color === "emerald" && "border-emerald-500/30 bg-emerald-500/10 text-emerald-700 dark:text-emerald-300",
        color === "red" && "border-red-500/30 bg-red-500/10 text-red-700 dark:text-red-300",
        color === "amber" && "border-amber-500/30 bg-amber-500/10 text-amber-700 dark:text-amber-300",
      )}
    >
      {count} {label}
    </span>
  );
}

// ─── Agent card ──────────────────────────────────────────────────────────────

function AgentCard({ agent }: { agent: AgentStatusRow }) {
  const [expanded, setExpanded] = React.useState(false);

  return (
    <div
      className={cn(
        "relative rounded-md border p-2.5 transition-shadow hover:shadow-sm",
        agent.status === "blocked"
          ? "border-red-500/20 bg-red-500/5"
          : agent.status === "degraded"
            ? "border-amber-500/20 bg-amber-500/5"
            : "border-border bg-muted/20",
      )}
    >
      {/* Status pulse dot */}
      <div className="absolute right-2 top-2 flex items-center gap-1.5">
        <span className="relative flex size-2.5">
          <span
            className={cn(
              "absolute inline-flex size-full animate-ping rounded-full opacity-75",
              statusColor(agent.status),
            )}
          />
          <span
            className={cn(
              "relative inline-flex size-2.5 rounded-full",
              statusColor(agent.status),
            )}
          />
        </span>
      </div>

      {/* Name + verdict */}
      <div className="flex items-center gap-1.5 pr-6">
        {verdictIcon(agent.verdict)}
        <h4 className="truncate text-xs font-bold text-foreground">
          {agent.name}
        </h4>
      </div>

      {/* Category + verdict badge */}
      <div className="mt-1 flex flex-wrap items-center gap-1">
        <span className="text-[10px] uppercase tracking-wide text-muted-foreground">
          {agent.category}
        </span>
        <span
          className={cn(
            "rounded border px-1 py-0.5 text-[10px] font-bold",
            verdictBadgeClass(agent.verdict),
          )}
        >
          {agent.verdict}
        </span>
        <span className={cn("text-[10px] font-medium", riskColor(agent.risk))}>
          {agent.risk} risk
        </span>
      </div>

      {/* Source */}
      <p className="mt-1 text-[10px] text-muted-foreground">
        Source: {agent.source}
      </p>

      {/* Expandable: evidence + next action */}
      <button
        type="button"
        onClick={() => setExpanded((v) => !v)}
        className="mt-1.5 text-[10px] font-medium text-muted-foreground underline-offset-2 hover:text-foreground hover:underline"
      >
        {expanded ? "Less" : "More"}
      </button>

      {expanded && (
        <div className="mt-2 space-y-1.5 border-t border-border pt-2 text-[11px] text-muted-foreground">
          {agent.evidence.length > 0 && (
            <ul className="space-y-0.5">
              {agent.evidence.map((ev, i) => (
                <li key={i} className="flex gap-1">
                  <span className="shrink-0 text-muted-foreground">•</span>
                  <span>{ev}</span>
                </li>
              ))}
            </ul>
          )}
          {agent.next_action && (
            <p className="mt-1.5 font-medium text-foreground">
              Next: {agent.next_action}
            </p>
          )}
          {agent.operator_required && (
            <p className="text-amber-600 dark:text-amber-400">
              Operator action required
            </p>
          )}
          {agent.blocks.length > 0 && (
            <p className="text-red-500">
              Blocks: {agent.blocks.join(", ")}
            </p>
          )}
        </div>
      )}
    </div>
  );
}
