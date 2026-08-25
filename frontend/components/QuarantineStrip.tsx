/**
 * QuarantineStrip — FE-0031 (§30) shared renderer.
 *
 * Pure: renders the QUARANTINED marker with the exact violation codes when a
 * row failed validateOpportunitySemantics(). §30 quarantine is a VISIBLE
 * state — the row stays on the grid, marked; it is never silently hidden.
 * Empty list renders nothing.
 */
import React from "react";
import { QUARANTINED_LABEL, type SemanticViolation } from "@/lib/store/types";

export function QuarantineStrip({ violations }: { violations: SemanticViolation[] }) {
  if (violations.length === 0) return null;
  return (
    <div
      role="alert"
      title="§30: la fila violó la semántica de oportunidad (validateOpportunitySemantics). QUARANTINED = visible y marcada, jamás oculta — los códigos listan la violación exacta."
      className="flex flex-wrap items-center gap-1.5 rounded-md border border-destructive/40 bg-destructive/10 px-2 py-1 text-[10px] font-bold uppercase tracking-wide text-destructive"
    >
      {QUARANTINED_LABEL}
      <span className="font-mono font-normal normal-case tracking-normal opacity-80">
        {violations.join(" · ")}
      </span>
    </div>
  );
}
