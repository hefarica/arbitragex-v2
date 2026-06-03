"use client";

/**
 * Premium UI primitives for the OMEGA telemetry surfaces.
 *
 * Built per the shadcn-enterprise-premium-ui doctrine: composition over
 * decoration, semantic tokens (no hardcoded colors), compact metric cards,
 * mono hashes with copy, meaningful state colors, accessible controls. Pure
 * presentation — never fabricates data ("—" for null/unknown).
 */

import * as React from "react";
import { CopyIcon, CheckIcon } from "lucide-react";
import { cn } from "@/lib/utils";
import { Badge } from "@/components/ui/badge";
import { Skeleton } from "@/components/ui/skeleton";

export type FeedStatus = "CONNECTING" | "LIVE" | "STALE";

/** Connection pill with a status dot. Uses the project's semantic tokens. */
export function StatusPill({ status }: { status: FeedStatus }) {
  const map: Record<FeedStatus, { dot: string; text: string; ring: string }> = {
    LIVE: { dot: "bg-success", text: "text-success", ring: "ring-success/30" },
    CONNECTING: { dot: "bg-warning animate-pulse", text: "text-warning", ring: "ring-warning/30" },
    STALE: { dot: "bg-destructive", text: "text-destructive", ring: "ring-destructive/30" },
  };
  const m = map[status];
  return (
    <span
      className={cn(
        "inline-flex items-center gap-1.5 rounded-full px-2.5 py-0.5 text-[11px] font-medium ring-1 ring-inset",
        m.ring,
        m.text,
      )}
    >
      <span className={cn("h-1.5 w-1.5 rounded-full", m.dot)} aria-hidden />
      {status}
    </span>
  );
}

/** Runtime mode badge (off/shadow/paper/active). active is destructive. */
export function ModeBadge({ mode }: { mode?: string | null }) {
  if (!mode) return null;
  const active = mode.toLowerCase() === "active";
  return (
    <Badge
      variant={active ? "destructive" : "secondary"}
      className="text-[10px] font-medium uppercase tracking-wide"
    >
      {mode}
    </Badge>
  );
}

/** Read-only / shadow guardrail badge. */
export function ReadOnlyBadge({ label }: { label: string }) {
  return (
    <Badge variant="outline" className="gap-1 text-[10px] text-muted-foreground">
      {label}
    </Badge>
  );
}

/** Compact metric card — mono tabular value, "—" for null, skeleton on load. */
export function MetricCard({
  label,
  value,
  hint,
  loading,
  tone = "default",
}: {
  label: string;
  value?: string | number | boolean | null;
  hint?: string;
  loading?: boolean;
  tone?: "default" | "success" | "warning" | "muted";
}) {
  const toneClass =
    tone === "success"
      ? "text-success"
      : tone === "warning"
        ? "text-warning"
        : tone === "muted"
          ? "text-muted-foreground"
          : "text-foreground";
  const display =
    value === null || value === undefined
      ? "—"
      : typeof value === "boolean"
        ? value
          ? "yes"
          : "no"
        : value;
  return (
    <div className="glass-surface rounded-lg border p-3">
      <div className="text-[11px] font-medium uppercase tracking-wide text-muted-foreground">
        {label}
      </div>
      {loading ? (
        <Skeleton className="mt-1.5 h-7 w-16" />
      ) : (
        <div className={cn("mt-0.5 font-mono text-2xl font-semibold tracking-tight tabular-nums", toneClass)}>
          {display}
        </div>
      )}
      {hint ? <div className="mt-0.5 text-[11px] text-muted-foreground">{hint}</div> : null}
    </div>
  );
}

/** Truncated monospace hash with a copy-to-clipboard button. */
export function CopyHash({ value }: { value?: string | null }) {
  const [copied, setCopied] = React.useState(false);
  if (!value) return <span className="text-muted-foreground">—</span>;
  const short = value.length > 18 ? `${value.slice(0, 10)}…${value.slice(-6)}` : value;
  const copy = async () => {
    try {
      await navigator.clipboard.writeText(value);
      setCopied(true);
      setTimeout(() => setCopied(false), 1200);
    } catch {
      /* clipboard unavailable — non-fatal */
    }
  };
  return (
    <span className="inline-flex items-center gap-1">
      <span className="font-mono text-xs" title={value}>
        {short}
      </span>
      <button
        type="button"
        onClick={copy}
        aria-label="Copy hash"
        className="rounded p-0.5 text-muted-foreground transition-colors hover:text-foreground focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring"
      >
        {copied ? <CheckIcon className="h-3 w-3 text-success" /> : <CopyIcon className="h-3 w-3" />}
      </button>
    </span>
  );
}

/** Small monospace chip for protocols / fee tiers / directions. */
export function Chip({
  children,
  tone = "default",
}: {
  children: React.ReactNode;
  tone?: "default" | "v2" | "v3";
}) {
  const cls =
    tone === "v3"
      ? "border-primary/40 text-primary"
      : tone === "v2"
        ? "border-border text-foreground"
        : "border-border text-muted-foreground";
  return (
    <span className={cn("inline-flex items-center rounded border px-1.5 py-0 text-[10px] font-mono leading-5", cls)}>
      {children}
    </span>
  );
}

/** Protocol chips with v2/v3 toning. */
export function ProtocolChips({ protocols }: { protocols?: string[] }) {
  if (!protocols || protocols.length === 0) return <span className="text-muted-foreground">—</span>;
  return (
    <span className="inline-flex flex-wrap gap-1">
      {protocols.map((p, i) => (
        <Chip key={i} tone={p === "v3" ? "v3" : p === "v2" ? "v2" : "default"}>
          {p}
        </Chip>
      ))}
    </span>
  );
}

/** Applicable strategy badges. */
export function StrategyBadges({ strategies }: { strategies?: string[] }) {
  if (!strategies || strategies.length === 0) return <span className="text-muted-foreground">—</span>;
  return (
    <span className="inline-flex flex-wrap gap-1">
      {strategies.map((s, i) => (
        <Badge key={i} variant="outline" className="font-mono text-[10px]">
          {s}
        </Badge>
      ))}
    </span>
  );
}

/** "updated 12:34:56" freshness label from an epoch-ms timestamp. */
export function Freshness({ at }: { at: number | null }) {
  if (at === null) return null;
  const d = new Date(at);
  const hh = String(d.getHours()).padStart(2, "0");
  const mm = String(d.getMinutes()).padStart(2, "0");
  const ss = String(d.getSeconds()).padStart(2, "0");
  return <span className="text-[11px] text-muted-foreground">updated {hh}:{mm}:{ss}</span>;
}
