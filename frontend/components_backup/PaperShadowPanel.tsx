"use client";

/**
 * PaperShadowPanel — tracks the A.5 paper-shadow runtime progress.
 *
 * Shows:
 *   - Consecutive green days counter (target: 14)
 *   - Progress bar with percentage
 *   - P&L today and accumulated
 *   - Status: ACTIVE / INACTIVE / COMPLETED
 *
 * Fetches from /api/metrics/paper-shadow every 60s.
 *
 * R8 fail-honest: when the endpoint is absent (expected in P2 before the
 * paper-shadow service is wired), the panel renders an INACTIVE state with
 * the verbatim error — never fabricating metrics.
 */

import * as React from "react";
import { TrendingUp, TrendingDown, AlertCircle, Loader2 } from "lucide-react";
import { getApiBaseUrl } from "@/lib/api-client";
import { cn } from "@/lib/utils";

// ─── Types ───────────────────────────────────────────────────────────────────

interface PaperShadowMetrics {
  consecutive_green_days: number;
  target_days: number;
  pnl_today_usd: number;
  pnl_accumulated_usd: number;
  status: "ACTIVE" | "INACTIVE" | "COMPLETED";
  started_at: string | null;
  last_trade_at: string | null;
  total_trades: number;
  green_trades: number;
  red_trades: number;
}

interface PaperShadowResponse {
  metrics: PaperShadowMetrics;
  generated_at: string;
}

type LoadState =
  | { kind: "loading" }
  | { kind: "ok"; data: PaperShadowResponse }
  | { kind: "error"; detail: string };

// ─── Constants ───────────────────────────────────────────────────────────────

const POLL_MS = 60_000;

// ─── Fetch helper ────────────────────────────────────────────────────────────

async function fetchPaperShadow(): Promise<LoadState> {
  try {
    const res = await fetch(`${getApiBaseUrl()}/api/metrics/paper-shadow`, {
      headers: { accept: "application/json" },
      credentials: "include",
      signal: AbortSignal.timeout(10_000),
    });

    if (!res.ok) {
      const text = await res.text();
      return {
        kind: "error",
        detail: `HTTP ${res.status}: ${text.slice(0, 200)}`,
      };
    }

    const raw: unknown = await res.json();

    // Lightweight validation — we trust the backend shape but verify the
    // critical fields exist so the UI doesn't crash on malformed data.
    const payload = raw as Record<string, unknown>;
    if (!payload.metrics || typeof (payload.metrics as Record<string, unknown>).consecutive_green_days !== "number") {
      return { kind: "error", detail: "Invalid response shape from /api/metrics/paper-shadow" };
    }

    return { kind: "ok", data: raw as PaperShadowResponse };
  } catch (err) {
    const msg = (err as Error).message ?? "unknown error";
    if ((err as Error).name === "AbortError") {
      return { kind: "error", detail: "Timeout after 10s" };
    }
    return { kind: "error", detail: msg };
  }
}

// ─── Component ───────────────────────────────────────────────────────────────

export function PaperShadowPanel() {
  const [state, setState] = React.useState<LoadState>({ kind: "loading" });

  React.useEffect(() => {
    let alive = true;

    const tick = async () => {
      const next = await fetchPaperShadow();
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
          Paper Shadow
        </div>
        <div className="mt-4 space-y-3">
          <div className="h-4 w-full animate-pulse rounded bg-muted" />
          <div className="h-4 w-3/4 animate-pulse rounded bg-muted" />
          <div className="h-8 w-full animate-pulse rounded bg-muted" />
        </div>
      </div>
    );
  }

  // ─── Error ───────────────────────────────────────────────────────────────

  if (state.kind === "error") {
    return (
      <div className="rounded-lg border border-destructive/30 bg-destructive/5 p-4 shadow-sm">
        <div className="flex items-center gap-2 text-sm font-semibold text-destructive">
          <AlertCircle className="size-4" aria-hidden="true" />
          Paper Shadow
        </div>
        <p className="mt-2 text-xs text-destructive/80">
          {state.detail}
        </p>
        <p className="mt-1 text-xs text-muted-foreground">
          The paper-shadow service may not be running yet. This is expected in P2.
        </p>
      </div>
    );
  }

  // ─── OK ──────────────────────────────────────────────────────────────────

  const { metrics } = state.data;
  const progressPct = Math.min(
    100,
    Math.round((metrics.consecutive_green_days / metrics.target_days) * 100),
  );

  const statusColor =
    metrics.status === "ACTIVE"
      ? "text-emerald-600 dark:text-emerald-400"
      : metrics.status === "COMPLETED"
        ? "text-blue-600 dark:text-blue-400"
        : "text-amber-600 dark:text-amber-400";

  const statusBg =
    metrics.status === "ACTIVE"
      ? "bg-emerald-500"
      : metrics.status === "COMPLETED"
        ? "bg-blue-500"
        : "bg-amber-500";

  const pnlTodayPositive = metrics.pnl_today_usd >= 0;
  const pnlAccumulatedPositive = metrics.pnl_accumulated_usd >= 0;

  return (
    <div className="rounded-lg border border-border bg-card p-4 shadow-sm">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          <span className={cn("inline-block size-2.5 rounded-full", statusBg)} />
          <h3 className="text-sm font-semibold text-card-foreground">Paper Shadow</h3>
        </div>
        <span className={cn("text-xs font-bold uppercase tracking-wide", statusColor)}>
          {metrics.status}
        </span>
      </div>

      {/* Progress bar */}
      <div className="mt-4">
        <div className="flex items-center justify-between text-xs text-muted-foreground">
          <span>Progress</span>
          <span className="font-mono font-medium text-foreground">
            {metrics.consecutive_green_days} / {metrics.target_days} days ({progressPct}%)
          </span>
        </div>
        <div className="mt-1.5 h-2.5 w-full overflow-hidden rounded-full bg-muted">
          <div
            className={cn(
              "h-full rounded-full transition-all duration-500",
              metrics.status === "COMPLETED"
                ? "bg-blue-500"
                : "bg-emerald-500",
            )}
            style={{ width: `${progressPct}%` }}
            role="progressbar"
            aria-valuenow={progressPct}
            aria-valuemin={0}
            aria-valuemax={100}
          />
        </div>
      </div>

      {/* P&L grid */}
      <div className="mt-4 grid grid-cols-2 gap-3">
        {/* Today */}
        <div className="rounded-md border border-border bg-muted/30 p-2.5">
          <div className="flex items-center gap-1 text-xs text-muted-foreground">
            {pnlTodayPositive ? (
              <TrendingUp className="size-3 text-emerald-500" aria-hidden="true" />
            ) : (
              <TrendingDown className="size-3 text-red-500" aria-hidden="true" />
            )}
            P&L Today
          </div>
          <p
            className={cn(
              "mt-1 font-mono text-sm font-semibold",
              pnlTodayPositive
                ? "text-emerald-600 dark:text-emerald-400"
                : "text-red-600 dark:text-red-400",
            )}
          >
            {pnlTodayPositive ? "+" : ""}
            {metrics.pnl_today_usd.toFixed(2)} USD
          </p>
        </div>

        {/* Accumulated */}
        <div className="rounded-md border border-border bg-muted/30 p-2.5">
          <div className="flex items-center gap-1 text-xs text-muted-foreground">
            {pnlAccumulatedPositive ? (
              <TrendingUp className="size-3 text-emerald-500" aria-hidden="true" />
            ) : (
              <TrendingDown className="size-3 text-red-500" aria-hidden="true" />
            )}
            Accumulated
          </div>
          <p
            className={cn(
              "mt-1 font-mono text-sm font-semibold",
              pnlAccumulatedPositive
                ? "text-emerald-600 dark:text-emerald-400"
                : "text-red-600 dark:text-red-400",
            )}
          >
            {pnlAccumulatedPositive ? "+" : ""}
            {metrics.pnl_accumulated_usd.toFixed(2)} USD
          </p>
        </div>
      </div>

      {/* Stats row */}
      <div className="mt-3 flex items-center gap-4 text-xs text-muted-foreground">
        <span>Trades: <strong className="text-foreground">{metrics.total_trades}</strong></span>
        <span className="text-emerald-500">Green: <strong>{metrics.green_trades}</strong></span>
        <span className="text-red-500">Red: <strong>{metrics.red_trades}</strong></span>
      </div>

      {/* Timestamp */}
      <p className="mt-3 text-[11px] text-muted-foreground">
        Updated: {new Date(state.data.generated_at).toLocaleTimeString()}
      </p>
    </div>
  );
}
