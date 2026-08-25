"use client";

/**
 * =============================================================================
 * ResourceHealth — FE-0006 (FE-MASTER §65)
 * =============================================================================
 *
 * Reusable one-line resource health leaf: a named surface (an endpoint, a
 * Redis-backed snapshot, a WS room) with its honest liveness state. PURE
 * PRESENTATIONAL — the CALLER derives the state (from RealtimeSlice's
 * channels, a fetch status, an endpoint probe); this component never fetches
 * and never computes freshness itself.
 *
 * Why no age rendering: "45s ago" needs Date.now() at render time —
 * nondeterministic under SSR (R1). The leaf renders the state the caller
 * certifies plus a verbatim detail string; age math belongs to a client-only
 * consumer if one is ever wanted.
 *
 * §40 gate explainability: every state explains itself via title/COPY — the
 * operator never guesses what DEGRADED means.
 *
 * RULE 00 / R8: `detail = null` falls back to the state's canonical copy —
 * an honest generic, never a fabricated per-resource story.
 */

// SSR-test support (repo pattern): classic JSX path needs the React namespace.
import * as React from "react";

import {
  AlertTriangle,
  CheckCircle2,
  Clock3,
  MinusCircle,
  XCircle,
  type LucideIcon,
} from "lucide-react";

export type ResourceHealthState =
  | "HEALTHY"
  | "DEGRADED"
  | "STALE"
  | "FAILED"
  | "UNCONFIGURED";

type Tone = {
  icon: LucideIcon;
  /** static class strings so Tailwind's JIT keeps them */
  chip: string;
  text: string;
};

const ok = (): Tone => ({
  icon: CheckCircle2,
  chip: "border-primary/30 bg-primary/15 text-primary",
  text: "text-primary",
});
const warn = (icon: LucideIcon): Tone => ({
  icon,
  chip: "border-warning/30 bg-warning/15 text-warning",
  text: "text-warning",
});
const bad = (icon: LucideIcon): Tone => ({
  icon,
  chip: "border-destructive/30 bg-destructive/15 text-destructive",
  text: "text-destructive",
});
const muted = (): Tone => ({
  icon: MinusCircle,
  chip: "border-border bg-muted/70 text-muted-foreground",
  text: "text-muted-foreground",
});

const TONES: Record<ResourceHealthState, Tone> = {
  HEALTHY: ok(),
  DEGRADED: warn(AlertTriangle),
  STALE: warn(Clock3),
  FAILED: bad(XCircle),
  UNCONFIGURED: muted(),
};

/** §65 canonical copy — label + explain-yourself hint (§40). */
/**
 * Copy discipline: hints avoid apostrophes (HTML-escaped in title) and the
 * " — " separator — that em-dash is RESERVED for appending a caller detail.
 */
export const RESOURCE_HEALTH_COPY: Record<
  ResourceHealthState,
  { label: string; hint: string }
> = {
  HEALTHY: {
    label: "HEALTHY",
    hint: "serving fresh real data on its native transport",
  },
  DEGRADED: {
    label: "DEGRADED",
    hint: "serving real data on a fallback transport (REST polling instead of the WS room, or a degraded upstream)",
  },
  STALE: {
    label: "STALE",
    hint: "transport nominally up but no accepted payload within the freshness budget of the channel",
  },
  FAILED: {
    label: "FAILED",
    hint: "last accepted delivery failed (fetch or push error); data shown may lag",
  },
  UNCONFIGURED: {
    label: "UNCONFIGURED",
    hint: "the surface was never configured or served; absence is a state, not an error",
  },
};

export interface ResourceHealthProps {
  /** Resource name the operator recognizes ("pairs snapshot", "runtime_ack room"). */
  label: string;
  /** Caller-certified state — this leaf never derives it. */
  state: ResourceHealthState;
  /** Verbatim extra context (an error string, a key name); null = canonical copy only. */
  detail?: string | null;
  className?: string;
}

export function ResourceHealth({
  label,
  state,
  detail = null,
  className = "",
}: ResourceHealthProps) {
  const tone = TONES[state];
  const copy = RESOURCE_HEALTH_COPY[state];
  const hint = detail ? `${copy.hint} — ${detail}` : copy.hint;
  const Icon = tone.icon;

  return (
    <div
      role="status"
      className={`inline-flex max-w-full items-center gap-x-2 gap-y-1 font-mono text-[11px] ${className}`}
    >
      <span className="truncate font-sans text-xs font-medium text-foreground/80">
        {label}
      </span>
      <span
        title={hint}
        className={`inline-flex items-center gap-1 rounded-full border px-2 py-0.5 text-[10px] font-semibold uppercase tracking-[0.12em] ${tone.chip}`}
      >
        <Icon size={11} strokeWidth={2.4} className={tone.text} aria-hidden />
        {copy.label}
      </span>
    </div>
  );
}

export default ResourceHealth;
