"use client";

/**
 * EdgeState — the dashboard's unified degraded-state surface.
 *
 * One component for every "the data isn't here" moment: an honest edge/RPC
 * failure, an empty registry, a disconnected stream, or a first-paint load.
 * Replaces the thin maroon "EDGE ERROR — ZERO TRUST" banners over dead black
 * space (smoke-test finding 2026-06-05) with a centered instrument-panel card:
 * a sonar-ping status orb, a plain-language headline, the failing endpoint and
 * structured reason codes in mono, a Retry action, and a GHOST of the content
 * that *would* be here — so a withheld panel reads as a deliberate, confident
 * state, never a crashed page.
 *
 * Doctrine: presentation only. It never fabricates data (RULE 00) — it dresses
 * the honest absence of it. The ghost is visibly inert (no values, blurred).
 */

import * as React from "react";
import {
  ShieldAlert,
  WifiOff,
  Inbox,
  RefreshCw,
  Loader2,
  type LucideIcon,
} from "lucide-react";

export type EdgeStateVariant = "error" | "offline" | "empty" | "loading";
export type EdgeGhost = "table" | "cards" | "chart" | "none";

type Tone = {
  icon: LucideIcon;
  label: string;
  /** static class strings so Tailwind's JIT keeps them */
  text: string;
  ringBg: string;
  chip: string;
  glow: string; // radial glow color (CSS var) for the orb halo
};

const TONES: Record<EdgeStateVariant, Tone> = {
  error: {
    icon: ShieldAlert,
    label: "EDGE ERROR",
    text: "text-destructive",
    ringBg: "bg-destructive",
    chip: "border-destructive/30 bg-destructive/15 text-destructive",
    glow: "var(--destructive)",
  },
  offline: {
    icon: WifiOff,
    label: "DISCONNECTED",
    text: "text-warning",
    ringBg: "bg-warning",
    chip: "border-warning/30 bg-warning/15 text-warning",
    glow: "var(--warning)",
  },
  empty: {
    icon: Inbox,
    label: "NO DATA",
    text: "text-info",
    ringBg: "bg-info",
    chip: "border-info/30 bg-info/15 text-info",
    glow: "var(--info)",
  },
  loading: {
    icon: Loader2,
    label: "SYNCING",
    text: "text-primary",
    ringBg: "bg-primary",
    chip: "border-primary/30 bg-primary/15 text-primary",
    glow: "var(--primary)",
  },
};

export interface EdgeStateProps {
  variant?: EdgeStateVariant;
  /** Bold headline, e.g. "Pool registry unreachable". */
  title: string;
  /** One plain sentence: what happened + why it's safe. */
  description?: React.ReactNode;
  /** The failing route, shown as a mono chip (e.g. GET /api/v1/pools). */
  endpoint?: string;
  /** Structured reason codes → scannable tags (fixes raw comma dumps). */
  reasons?: string[];
  /** "last good fetch" provenance, shown small + muted. */
  lastOk?: string;
  onRetry?: () => void;
  retrying?: boolean;
  /** Ghost of the intended content rendered behind the card. */
  ghost?: EdgeGhost;
  /** Column labels for the table ghost. */
  ghostColumns?: string[];
  className?: string;
}

export function EdgeState({
  variant = "error",
  title,
  description,
  endpoint,
  reasons,
  lastOk,
  onRetry,
  retrying = false,
  ghost = "table",
  ghostColumns = ["", "", "", ""],
  className = "",
}: EdgeStateProps) {
  const tone = TONES[variant];
  const Icon = tone.icon;
  const spin = variant === "loading";

  return (
    <div
      className={`relative isolate flex min-h-[58vh] w-full items-center justify-center overflow-hidden rounded-2xl ${className}`}
      role={variant === "error" || variant === "offline" ? "alert" : "status"}
      aria-busy={variant === "loading"}
    >
      {/* GHOST: the structure that WOULD be here — inert, blurred, scanned.
          Scanline keyframe (.edge-scan) lives in globals.css. */}
      {ghost !== "none" && (
        <div
          aria-hidden
          className="pointer-events-none absolute inset-0 select-none opacity-[0.12] [mask-image:radial-gradient(120%_90%_at_50%_50%,transparent_18%,#000_72%)]"
        >
          <GhostContent kind={ghost} columns={ghostColumns} />
          <span className="edge-scan absolute inset-x-0 top-0 h-px" />
        </div>
      )}

      {/* CARD */}
      <div
        data-slot="card"
        className="relative z-10 mx-4 flex w-full max-w-md flex-col items-center gap-5 rounded-2xl border border-border bg-card/70 px-8 py-10 text-center shadow-2xl"
      >
        {/* sonar orb */}
        <div className="relative grid h-16 w-16 place-items-center">
          <span
            className={`absolute inset-0 rounded-full ${tone.ringBg} opacity-20 motion-safe:animate-ping`}
          />
          <span
            className={`absolute inset-2 rounded-full ${tone.ringBg} opacity-10`}
          />
          <span
            className="absolute -inset-3 rounded-full"
            style={{
              background: `radial-gradient(closest-side, ${tone.glow} 0%, transparent 70%)`,
              opacity: 0.28,
            }}
          />
          <span
            className={`relative grid h-11 w-11 place-items-center rounded-full border border-border/60 bg-background/70 ${tone.text}`}
          >
            <Icon size={20} className={spin ? "motion-safe:animate-spin" : ""} strokeWidth={2.2} />
          </span>
        </div>

        {/* status label */}
        <span
          className={`inline-flex items-center gap-1.5 rounded-full border px-2.5 py-1 font-mono text-[10px] font-semibold uppercase tracking-[0.18em] ${tone.chip}`}
        >
          <span className={`h-1.5 w-1.5 rounded-full ${tone.ringBg} motion-safe:animate-pulse`} />
          {tone.label}
        </span>

        <div className="space-y-1.5">
          <h2 className="text-lg font-semibold tracking-tight text-foreground">{title}</h2>
          {description && (
            <p className="mx-auto max-w-sm text-sm leading-relaxed text-muted-foreground">
              {description}
            </p>
          )}
        </div>

        {endpoint && (
          <code className="max-w-full truncate rounded-md border border-border bg-muted/70 px-2.5 py-1 font-mono text-[11px] text-foreground/70">
            {endpoint}
          </code>
        )}

        {reasons && reasons.length > 0 && (
          <div className="flex flex-wrap justify-center gap-1.5">
            {reasons.map((r, i) => (
              <span
                key={`reason-${i}`}
                className="rounded border border-border bg-muted/70 px-1.5 py-0.5 font-mono text-[10px] text-foreground/70"
              >
                {r}
              </span>
            ))}
          </div>
        )}

        {onRetry && (
          <button
            type="button"
            onClick={onRetry}
            disabled={retrying}
            className="group mt-1 inline-flex items-center gap-2 rounded-lg border border-border bg-secondary/70 px-4 py-2 text-sm font-medium text-secondary-foreground outline-none hover:border-primary/50 hover:bg-secondary focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-ring disabled:opacity-60 motion-safe:transition-colors"
          >
            <RefreshCw
              size={15}
              className={`text-primary ${retrying ? "motion-safe:animate-spin" : "motion-safe:transition-transform motion-safe:group-hover:rotate-90"}`}
            />
            {retrying ? "Retrying…" : "Retry"}
          </button>
        )}

        {lastOk && (
          <p className="font-mono text-[10px] uppercase tracking-wider text-muted-foreground/70">
            last good fetch · {lastOk}
          </p>
        )}
      </div>
    </div>
  );
}

function GhostContent({ kind, columns }: { kind: EdgeGhost; columns: string[] }) {
  if (kind === "cards") {
    return (
      <div className="grid h-full grid-cols-3 gap-4 p-8">
        {Array.from({ length: 6 }).map((_, i) => (
          <div key={i} className="space-y-3 rounded-xl border border-border bg-muted/40 p-4">
            <div className="h-3 w-1/2 rounded bg-muted" />
            <div className="h-7 w-2/3 rounded bg-muted" />
            <div className="h-2 w-full rounded bg-muted" />
          </div>
        ))}
      </div>
    );
  }
  if (kind === "chart") {
    return (
      <div className="flex h-full items-end gap-2 p-10">
        {Array.from({ length: 24 }).map((_, i) => (
          <div
            key={i}
            className="flex-1 rounded-t bg-muted"
            style={{ height: `${20 + ((i * 37) % 70)}%` }}
          />
        ))}
      </div>
    );
  }
  // table (default)
  return (
    <div className="p-6">
      <div className="overflow-hidden rounded-xl border border-border">
        <div className="grid grid-cols-4 gap-4 border-b border-border bg-muted/60 px-4 py-3">
          {columns.slice(0, 4).map((c, i) => (
            <div key={i} className="h-2.5 w-2/3 rounded bg-muted" />
          ))}
        </div>
        {Array.from({ length: 8 }).map((_, r) => (
          <div key={r} className="grid grid-cols-4 gap-4 border-b border-border px-4 py-4">
            {Array.from({ length: 4 }).map((_, c) => (
              <div key={c} className="h-3 rounded bg-muted" style={{ width: `${50 + ((r + c) * 11) % 45}%` }} />
            ))}
          </div>
        ))}
      </div>
    </div>
  );
}

export default EdgeState;
