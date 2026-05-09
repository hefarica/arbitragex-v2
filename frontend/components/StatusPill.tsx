import React from "react";

/**
 * StatusPill.tsx — Pure display component for opportunity pipeline status.
 *
 * R1 Mounted Snapshot compliance:
 *   - No Date.now(), no Math.random(), no window/document, no hooks.
 *   - SSR render === CSR render. Zero hydration risk.
 *
 * Covers all 9 StatusSchema values from shared-ts/src/api-contracts.ts:
 *   "detected" | "validated" | "simulated" | "scored" | "executing" |
 *   "executed" | "reconciled" | "rejected" | "failed"
 *
 * R8 fail-honest: rejection_reason is surfaced in the title attribute when
 * status="rejected". React automatically HTML-escapes attribute values, so
 * no manual escaping is needed.
 */

/** Mirrors StatusSchema from shared-ts/src/api-contracts.ts — no cross-package import. */
export type OpportunityStatus =
  | "detected"
  | "validated"
  | "simulated"
  | "scored"
  | "executing"
  | "executed"
  | "reconciled"
  | "rejected"
  | "failed";

interface StatusMeta {
  label: string;
  className: string;
}

/**
 * Exhaustive map: Record<OpportunityStatus, StatusMeta>.
 * TypeScript enforces all 9 keys are present.
 */
const STATUS_MAP: Record<OpportunityStatus, StatusMeta> = {
  detected: {
    label: "DETECTED",
    className: "bg-muted/60 text-muted-foreground border border-border",
  },
  validated: {
    label: "VALIDATED",
    className: "bg-info/10 text-info border border-info/30",
  },
  simulated: {
    label: "SIMULATED",
    className: "bg-primary/10 text-primary border border-primary/30",
  },
  scored: {
    label: "SCORED",
    className: "bg-accent text-accent-foreground border border-border",
  },
  executing: {
    label: "EXECUTING",
    className:
      "bg-warning/15 text-warning border border-warning/40 animate-pulse",
  },
  executed: {
    label: "EXECUTED",
    className: "bg-success/15 text-success border border-success/40",
  },
  reconciled: {
    label: "RECONCILED",
    className: "bg-success/10 text-success border border-success/30",
  },
  rejected: {
    label: "REJECTED",
    className: "bg-destructive/10 text-destructive border border-destructive/30",
  },
  failed: {
    label: "FAILED",
    className: "bg-destructive/20 text-destructive border border-destructive/50",
  },
};

export interface StatusPillProps {
  status: OpportunityStatus;
  /**
   * Surfaced in the `title` attribute when status="rejected".
   * React HTML-escapes this automatically — no manual sanitization needed.
   */
  rejection_reason?: string | null;
}

export function StatusPill({ status, rejection_reason }: StatusPillProps) {
  const meta = STATUS_MAP[status];

  // Defensive fallback: unknown status string slipped past TypeScript at runtime.
  if (!meta) {
    return (
      <span className="inline-flex items-center px-2 py-0.5 rounded text-xs font-bold tracking-wide bg-muted/60 text-muted-foreground border border-border">
        {String(status).toUpperCase()}
      </span>
    );
  }

  // Attach rejection_reason to title when present and status is "rejected".
  // React escapes attribute values, so special chars in rejection_reason are safe.
  const titleAttr =
    status === "rejected" && rejection_reason
      ? rejection_reason
      : undefined;

  return (
    <span
      title={titleAttr}
      className={`inline-flex items-center px-2 py-0.5 rounded text-xs font-bold tracking-wide ${meta.className}`}
    >
      {meta.label}
    </span>
  );
}
