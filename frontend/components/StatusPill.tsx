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
    className: "bg-slate-500/15 text-slate-300 border border-slate-500/30",
  },
  validated: {
    label: "VALIDATED",
    className: "bg-blue-500/15 text-blue-300 border border-blue-500/30",
  },
  simulated: {
    label: "SIMULATED",
    className: "bg-indigo-500/15 text-indigo-300 border border-indigo-500/30",
  },
  scored: {
    label: "SCORED",
    className: "bg-violet-500/15 text-violet-300 border border-violet-500/30",
  },
  executing: {
    label: "EXECUTING",
    className:
      "bg-amber-500/20 text-amber-300 border border-amber-500/50 animate-pulse",
  },
  executed: {
    label: "EXECUTED",
    className: "bg-emerald-500/15 text-emerald-300 border border-emerald-500/30",
  },
  reconciled: {
    label: "RECONCILED",
    className: "bg-green-500/15 text-green-300 border border-green-500/30",
  },
  rejected: {
    label: "REJECTED",
    className: "bg-rose-500/15 text-rose-300 border border-rose-500/30",
  },
  failed: {
    label: "FAILED",
    className: "bg-red-500/20 text-red-300 border border-red-600/50",
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
      <span className="inline-flex items-center px-2 py-0.5 rounded text-xs font-bold tracking-wide bg-slate-500/15 text-slate-300 border border-slate-500/30">
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
