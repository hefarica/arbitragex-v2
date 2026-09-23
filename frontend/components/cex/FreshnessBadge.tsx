// frontend/components/cex/FreshnessBadge.tsx
//
// WO-PRICE-EXCHANGE-V1 (FE) — "source + freshness" corner badge for the
// trading card: entry token symbol + detection age with a StatusPill-style
// colored dot (semantic tokens, same pattern as StatusPill.tsx).
//
// Pure display: the age TEXT and the age-derived level are computed by the
// caller (client-only, R1) and passed as props — this file holds no Date, no
// window, no hooks, so SSR === CSR for any given props.
import React from "react";

export type FreshnessLevel = "fresh" | "warm" | "stale" | "unknown";

/** Dot thresholds (operator order 2026-09-18): green <30s, amber <2m, red ≥2m. */
export const FRESH_MAX_SECS = 30;
export const WARM_MAX_SECS = 120;

/** Pure age → level map. null/NaN age (no detected_at, or pre-mount) → unknown. */
export function freshnessLevel(ageSecs: number | null): FreshnessLevel {
  if (ageSecs == null || !Number.isFinite(ageSecs)) return "unknown";
  if (ageSecs < FRESH_MAX_SECS) return "fresh";
  if (ageSecs < WARM_MAX_SECS) return "warm";
  return "stale";
}

const DOT_CLASS: Record<FreshnessLevel, string> = {
  fresh: "bg-success",
  warm: "bg-warning",
  stale: "bg-destructive",
  unknown: "bg-muted-foreground/50",
};

const TITLE_LABEL: Record<FreshnessLevel, string> = {
  fresh: "detected <30s ago",
  warm: "detected <2m ago",
  stale: "detected ≥2m ago",
  unknown: "no detection age available",
};

export interface FreshnessBadgeProps {
  /** Entry token symbol (token_in). null → honest "—", never fabricated. */
  symbol: string | null;
  /** Preformatted age text (e.g. "45s"). null until the client mounts (R1). */
  agoText: string | null;
  level: FreshnessLevel;
}

export function FreshnessBadge({ symbol, agoText, level }: FreshnessBadgeProps) {
  return (
    <span
      className="inline-flex items-center gap-1 px-1.5 py-0.5 rounded border border-border/60 bg-muted/30 font-mono uppercase tracking-wide text-[9px] text-muted-foreground"
      title={`Source token + freshness — ${TITLE_LABEL[level]}`}
      data-testid="freshness-badge"
      data-level={level}
    >
      <span
        aria-hidden="true"
        className={`inline-block h-1.5 w-1.5 rounded-full shrink-0 ${DOT_CLASS[level]}`}
      />
      <span className="truncate max-w-[90px]">{symbol ?? "—"}</span>
      <span aria-hidden="true" className="opacity-50">
        ·
      </span>
      <span suppressHydrationWarning>{agoText ?? "--"}</span>
    </span>
  );
}
