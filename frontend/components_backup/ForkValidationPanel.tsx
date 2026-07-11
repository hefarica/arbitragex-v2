"use client";

/**
 * ForkValidationPanel — A.4 fork validation state indicator.
 *
 * Shows:
 *   - Block number of the pinned fork
 *   - RPC latency to the archive node
 *   - Simulations run today
 *   - Overall status: HEALTHY / DEGRADED / UNAVAILABLE
 *
 * Fetches from /api/sim-ctl/fork-status every 15s.
 *
 * R8 fail-honest: when the endpoint is absent (expected in paper mode
 * before A.4 fork validation is wired), the panel renders DEGRADED
 * with "sim-ctl" as the source — never fabricating block numbers.
 */

import * as React from "react";
import {
  Cpu,
  Loader2,
  AlertTriangle,
  CheckCircle2,
  Zap,
  BarChart3,
  Clock,
} from "lucide-react";
import { getApiBaseUrl } from "@/lib/api-client";
import { cn } from "@/lib/utils";

// ─── Types ───────────────────────────────────────────────────────────────────

interface ForkStatusMetrics {
  block_number: number;
  rpc_latency_ms: number;
  simulations_today: number;
  fork_age_seconds: number;
  rpc_url_redacted: string;
  executor_address: string | null;
  status: "HEALTHY" | "DEGRADED" | "UNAVAILABLE";
}

interface ForkStatusResponse {
  metrics: ForkStatusMetrics;
  generated_at: string;
}

type LoadState =
  | { kind: "loading" }
  | { kind: "ok"; data: ForkStatusResponse }
  | { kind: "error"; detail: string };

const POLL_MS = 15_000;

// ─── Fetch helper ────────────────────────────────────────────────────────────

async function fetchForkStatus(): Promise<LoadState> {
  try {
    const res = await fetch(`${getApiBaseUrl()}/api/sim-ctl/fork-status`, {
      headers: { accept: "application/json" },
      credentials: "include",
      signal: AbortSignal.timeout(8_000),
    });

    if (!res.ok) {
      const text = await res.text();
      // 404 = endpoint not mounted yet (expected in paper mode)
      if (res.status === 404) {
        return {
          kind: "error",
          detail: "DEGRADED — sim-ctl endpoint not available (expected in paper mode)",
        };
      }
      return {
        kind: "error",
        detail: `HTTP ${res.status}: ${text.slice(0, 200)}`,
      };
    }

    const raw: unknown = await res.json();
    const payload = raw as Record<string, unknown>;

    if (!payload.metrics || typeof (payload.metrics as Record<string, unknown>).block_number !== "number") {
      return { kind: "error", detail: "Invalid response shape from /api/sim-ctl/fork-status" };
    }

    return { kind: "ok", data: raw as ForkStatusResponse };
  } catch (err) {
    const msg = (err as Error).message ?? "unknown error";
    if ((err as Error).name === "AbortError") {
      return { kind: "error", detail: "Timeout after 8s" };
    }
    return { kind: "error", detail: msg };
  }
}

// ─── Component ───────────────────────────────────────────────────────────────

export function ForkValidationPanel() {
  const [state, setState] = React.useState<LoadState>({ kind: "loading" });

  React.useEffect(() => {
    let alive = true;

    const tick = async () => {
      const next = await fetchForkStatus();
      if (alive) setState(next);
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
          Fork Validation
        </div>
        <div className="mt-4 space-y-3">
          <div className="h-4 w-full animate-pulse rounded bg-muted" />
          <div className="h-4 w-2/3 animate-pulse rounded bg-muted" />
          <div className="grid grid-cols-3 gap-2">
            <div className="h-16 animate-pulse rounded bg-muted" />
            <div className="h-16 animate-pulse rounded bg-muted" />
            <div className="h-16 animate-pulse rounded bg-muted" />
          </div>
        </div>
      </div>
    );
  }

  // ─── Error / Degraded ────────────────────────────────────────────────────

  if (state.kind === "error") {
    const isDegraded = state.detail.includes("DEGRADED") || state.detail.includes("404");
    return (
      <div
        className={cn(
          "rounded-lg border p-4 shadow-sm",
          isDegraded
            ? "border-amber-500/30 bg-amber-500/5"
            : "border-destructive/30 bg-destructive/5",
        )}
      >
        <div className="flex items-center gap-2 text-sm font-semibold">
          {isDegraded ? (
            <>
              <AlertTriangle className="size-4 text-amber-500" aria-hidden="true" />
              <span className="text-amber-700 dark:text-amber-300">Fork Validation</span>
            </>
          ) : (
            <>
              <AlertTriangle className="size-4 text-destructive" aria-hidden="true" />
              <span className="text-destructive">Fork Validation</span>
            </>
          )}
        </div>

        <div className="mt-4 flex flex-col items-center gap-2 py-3">
          <Cpu className={cn("size-10", isDegraded ? "text-amber-500" : "text-destructive")} aria-hidden="true" />
          <span className={cn(
            "text-lg font-bold",
            isDegraded
              ? "text-amber-600 dark:text-amber-400"
              : "text-destructive",
          )}>
            {isDegraded ? "DEGRADED" : "ERROR"}
          </span>
        </div>

        <p className={cn(
          "mt-2 text-xs",
          isDegraded ? "text-amber-700/80 dark:text-amber-300/80" : "text-destructive/80",
        )}>
          {state.detail}
        </p>

        {isDegraded && (
          <p className="mt-1 text-xs text-muted-foreground">
            Fork validation (A.4) is not yet active. This is expected in paper mode.
            The sim-ctl service will provide this data once A.4 prerequisites are met.
          </p>
        )}
      </div>
    );
  }

  // ─── OK ──────────────────────────────────────────────────────────────────

  const { data } = state;
  const { metrics } = data;

  const isHealthy = metrics.status === "HEALTHY";

  const forkAgeMin = Math.floor(metrics.fork_age_seconds / 60);
  const forkAgeHr = Math.floor(forkAgeMin / 60);
  const forkAgeDisplay = forkAgeHr > 0
    ? `${forkAgeHr}h ${forkAgeMin % 60}m`
    : `${forkAgeMin}m`;

  return (
    <div className="rounded-lg border border-border bg-card p-4 shadow-sm">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2 text-sm font-semibold text-card-foreground">
          <Cpu className="size-4 text-muted-foreground" aria-hidden="true" />
          Fork Validation
        </div>
        <span
          className={cn(
            "text-xs font-bold uppercase tracking-wide",
            isHealthy
              ? "text-emerald-600 dark:text-emerald-400"
              : "text-amber-600 dark:text-amber-400",
          )}
        >
          {metrics.status}
        </span>
      </div>

      {/* Status icon */}
      <div className="mt-3 flex items-center gap-2">
        {isHealthy ? (
          <CheckCircle2 className="size-5 text-emerald-500" aria-hidden="true" />
        ) : (
          <AlertTriangle className="size-5 text-amber-500" aria-hidden="true" />
        )}
        <span className="text-xs text-muted-foreground">
          {isHealthy
            ? "A.4 fork validation is running"
            : "A.4 fork validation is degraded"}
        </span>
      </div>

      {/* Metrics grid */}
      <div className="mt-4 grid grid-cols-3 gap-2">
        {/* Block number */}
        <div className="rounded-md border border-border bg-muted/30 p-2.5 text-center">
          <BarChart3 className="mx-auto size-4 text-muted-foreground" aria-hidden="true" />
          <p className="mt-1 font-mono text-sm font-bold text-foreground">
            {metrics.block_number.toLocaleString()}
          </p>
          <p className="text-[10px] text-muted-foreground">Block #</p>
        </div>

        {/* RPC latency */}
        <div className="rounded-md border border-border bg-muted/30 p-2.5 text-center">
          <Zap className="mx-auto size-4 text-muted-foreground" aria-hidden="true" />
          <p className={cn(
            "mt-1 font-mono text-sm font-bold",
            metrics.rpc_latency_ms < 100
              ? "text-emerald-600 dark:text-emerald-400"
              : metrics.rpc_latency_ms < 500
                ? "text-amber-600 dark:text-amber-400"
                : "text-red-600 dark:text-red-400",
          )}>
            {metrics.rpc_latency_ms}ms
          </p>
          <p className="text-[10px] text-muted-foreground">RPC Latency</p>
        </div>

        {/* Simulations */}
        <div className="rounded-md border border-border bg-muted/30 p-2.5 text-center">
          <Clock className="mx-auto size-4 text-muted-foreground" aria-hidden="true" />
          <p className="mt-1 font-mono text-sm font-bold text-foreground">
            {metrics.simulations_today.toLocaleString()}
          </p>
          <p className="text-[10px] text-muted-foreground">Sims Today</p>
        </div>
      </div>

      {/* Details row */}
      <div className="mt-3 space-y-1 text-xs text-muted-foreground">
        <p>
          <strong className="text-foreground">Fork age:</strong> {forkAgeDisplay}
        </p>
        <p>
          <strong className="text-foreground">RPC:</strong> {metrics.rpc_url_redacted}
        </p>
        {metrics.executor_address && (
          <p className="font-mono text-[11px]">
            <strong className="text-foreground">Executor:</strong>{" "}
            {metrics.executor_address.slice(0, 12)}…{metrics.executor_address.slice(-8)}
          </p>
        )}
      </div>

      {/* Timestamp */}
      <p className="mt-3 text-[11px] text-muted-foreground">
        Updated: {new Date(data.generated_at).toLocaleTimeString()}
      </p>
    </div>
  );
}
