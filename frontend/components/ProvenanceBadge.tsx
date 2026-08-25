"use client";

/**
 * =============================================================================
 * ProvenanceBadge — FE-0007 (FE-MASTER §66)
 * =============================================================================
 *
 * Reusable provenance leaf: WHERE a displayed value came from. Sibling of
 * ResourceHealth (FE-0006) and RuntimeSettingState (FE-0005): pure
 * presentational, deterministic, zero fetch — the caller certifies the
 * provenance token, this badge renders it with its §40 explain-yourself
 * hint.
 *
 * The six §66 tokens (closed vocabulary — a value's origin, not its
 * correctness):
 *   REALTIME  — live wire (WS push) less than one cadence old
 *   SIMULATION— computed by the local simulator (no chain settlement)
 *   PAPER     — paper-trade ledger (scored, no broadcast)
 *   HISTORICAL— persisted past (PostgreSQL row), not live
 *   CONFIG    — declared operator configuration (config is data too)
 *   ESTIMATE  — derived/approximated (display-side fold, not a wire value)
 *
 * RULE 00 / R8: `detail = null` → the token's canonical copy; a provided
 * detail rides verbatim.
 */

// SSR-test support (repo pattern): classic JSX path needs the React namespace.
import * as React from "react";

import { BadgeCheck, FlaskConical, ScrollText, History, Settings2, Sigma, type LucideIcon } from "lucide-react";

export type Provenance =
  | "REALTIME"
  | "SIMULATION"
  | "PAPER"
  | "HISTORICAL"
  | "CONFIG"
  | "ESTIMATE";

type Tone = {
  icon: LucideIcon;
  chip: string;
  text: string;
};

const TONES: Record<Provenance, Tone> = {
  REALTIME: {
    icon: BadgeCheck,
    chip: "border-primary/30 bg-primary/15 text-primary",
    text: "text-primary",
  },
  SIMULATION: {
    icon: FlaskConical,
    chip: "border-info/30 bg-info/15 text-info",
    text: "text-info",
  },
  PAPER: {
    icon: ScrollText,
    chip: "border-warning/30 bg-warning/15 text-warning",
    text: "text-warning",
  },
  HISTORICAL: {
    icon: History,
    chip: "border-border bg-muted/70 text-muted-foreground",
    text: "text-muted-foreground",
  },
  CONFIG: {
    icon: Settings2,
    chip: "border-info/30 bg-info/15 text-info",
    text: "text-info",
  },
  ESTIMATE: {
    icon: Sigma,
    chip: "border-warning/30 bg-warning/15 text-warning",
    text: "text-warning",
  },
};

/**
 * §66 canonical copy. Hint discipline: no apostrophes (HTML-escaped in the
 * title) and no " — " — that separator is RESERVED for a caller detail.
 */
export const PROVENANCE_COPY: Record<Provenance, { label: string; hint: string }> = {
  REALTIME: {
    label: "REALTIME",
    hint: "value arrived on the live wire (WS push); not older than one transport cadence",
  },
  SIMULATION: {
    label: "SIMULATION",
    hint: "computed by the local simulator; no chain settlement backs this number",
  },
  PAPER: {
    label: "PAPER",
    hint: "paper-trade ledger; scored against a simulated execution, no broadcast",
  },
  HISTORICAL: {
    label: "HISTORICAL",
    hint: "persisted past (database row); replayed for inspection, not live",
  },
  CONFIG: {
    label: "CONFIG",
    hint: "declared operator configuration; a setting, not an observed value",
  },
  ESTIMATE: {
    label: "ESTIMATE",
    hint: "derived or approximated display-side; not a value the backend put on the wire",
  },
};

export interface ProvenanceBadgeProps {
  /** Caller-certified origin token (closed §66 vocabulary). */
  provenance: Provenance;
  /** Verbatim extra context; null = canonical copy only. */
  detail?: string | null;
  className?: string;
}

export function ProvenanceBadge({
  provenance,
  detail = null,
  className = "",
}: ProvenanceBadgeProps) {
  const tone = TONES[provenance];
  const copy = PROVENANCE_COPY[provenance];
  const hint = detail ? `${copy.hint} — ${detail}` : copy.hint;
  const Icon = tone.icon;

  return (
    <span
      title={hint}
      className={`inline-flex items-center gap-1 rounded-full border px-2 py-0.5 font-mono text-[10px] font-semibold uppercase tracking-[0.12em] ${tone.chip} ${className}`}
    >
      <Icon size={11} strokeWidth={2.4} className={tone.text} aria-hidden />
      {copy.label}
    </span>
  );
}

export default ProvenanceBadge;
