"use client";

/**
 * TerminalStatsBar.tsx — 4-card metrics strip at the top of the operator
 * terminal, mirroring the DeFiBot Dashboard layout (Total P&L · Win Rate ·
 * Total Volume · Best Trade) but populated EXCLUSIVELY from validated
 * spine-emitted metrics.
 *
 * R8 fail-honest:
 *   - Each card accepts `value: number | null`. When null we render "—"
 *     with the explanatory caption "no data yet". We never fabricate a
 *     "0 trades" line if the backend has not yet emitted that aggregate.
 *
 * Wiring expectation: a future commit will hydrate `value` from
 * /api/operator/metrics. Until then the cards render the null state
 * (consistent with paper-shadow / pre-mainnet honesty).
 */

import React from "react";
import { TrendingUp, Target, BarChart3, Award } from "lucide-react";

import { fmtMoney, fmtPct100 } from "@/lib/formatters";

export interface TerminalStat {
  label: string;
  value: number | null;
  format: "money" | "percent";
  tone?: "positive" | "neutral";
  caption?: string;
}

export interface TerminalStatsBarProps {
  stats?: TerminalStat[];
  /** Caption appended below each card when value is null. */
  noDataCaption?: string;
}

const DEFAULT_STATS: TerminalStat[] = [
  { label: "Total P&L (24h)", value: null, format: "money", tone: "positive" },
  { label: "Win Rate (24h)", value: null, format: "percent" },
  { label: "Total Volume (24h)", value: null, format: "money" },
  { label: "Best Trade (24h)", value: null, format: "money", tone: "positive" },
];

const ICONS = [TrendingUp, Target, BarChart3, Award];

export function TerminalStatsBar({
  stats = DEFAULT_STATS,
  noDataCaption = "Awaiting first execution · gates fail-closed",
}: TerminalStatsBarProps) {
  return (
    <div
      data-testid="terminal-stats-bar"
      className="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-4"
    >
      {stats.map((s, i) => {
        const Icon = ICONS[i % ICONS.length];
        const hasValue = s.value != null && Number.isFinite(s.value);
        const display = hasValue
          ? s.format === "money"
            ? fmtMoney(s.value)
            : fmtPct100(s.value)
          : "—";
        return (
          <article
            key={s.label}
            className="flex flex-col gap-1 rounded-xl border border-border bg-card p-4 shadow-sm transition-colors hover:border-primary/30"
          >
            <div className="flex items-center justify-between">
              <span className="text-[10px] uppercase tracking-wider text-muted-foreground">
                {s.label}
              </span>
              <Icon
                className={[
                  "h-4 w-4",
                  hasValue && s.tone === "positive"
                    ? "text-success"
                    : "text-muted-foreground/60",
                ].join(" ")}
                aria-hidden
              />
            </div>
            <span
              className={[
                "font-mono tabular-nums text-2xl font-semibold",
                hasValue && s.tone === "positive"
                  ? "text-success"
                  : hasValue
                  ? "text-foreground"
                  : "text-muted-foreground/60",
              ].join(" ")}
            >
              {display}
            </span>
            <span className="text-[11px] text-muted-foreground/80">
              {hasValue ? s.caption ?? "" : noDataCaption}
            </span>
          </article>
        );
      })}
    </div>
  );
}
