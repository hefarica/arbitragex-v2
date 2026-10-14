"use client";

import * as React from "react";
import type { ResolvedField } from "@/lib/home-opportunity";

/**
 * X-Ray card — one opportunity, as observed.
 *
 * A8-CONF-01 (2026-10-03): every field arrives as a `ResolvedField`, i.e. a
 * REAL value (with its source) or an EXPLICIT not-computed declaration (with
 * its reason). This component therefore has no branch that can paint a mute
 * "—": an unresolved field renders `n/c · <reason>`, which the operator can
 * read and audit. `data-field-state` / `data-field-reason` expose the same
 * distinction to tests and to the DOM inspector.
 *
 * R8: null ≠ 0. `state: "zero"` is reserved for a producer that computed
 * exactly zero — it is never produced by a missing datum.
 */
interface XRayCardProps {
  pair: string;
  /** Convergence (ROI), or its not-computed reason. */
  yield: ResolvedField;
  /** A.8 confidence, or its not-computed reason. */
  confidence: ResolvedField;
  /** Hop count from the persisted topology. */
  legs: ResolvedField;
  ago: string;
  route: ResolvedField;
  /** Decohérencia de Estado (slippage). */
  decoherence: ResolvedField;
  /** Temporal Liquidity Superposition principal. */
  tlsAmount: ResolvedField;
  simVerdict: ResolvedField;
  tokenSafety: ResolvedField;
}

export function XRayCard({
  pair,
  yield: yieldField,
  confidence,
  legs,
  ago,
  route,
  decoherence,
  tlsAmount,
  simVerdict,
  tokenSafety,
}: XRayCardProps) {
  return (
    <article className="xray-card" data-interactive>
      <div className="xray-main transition-opacity duration-300">
        <div className="flex items-baseline justify-between mb-3.5">
          <span className="font-semibold text-[17px] tracking-tight">{pair}</span>
          <span className="font-mono text-base text-[var(--success)] font-bold">
            <FieldValue field={yieldField} />
          </span>
        </div>
        <div className="flex gap-4 font-mono text-[10.5px] text-[var(--muted)] tracking-wide">
          <span>
            <FieldValue field={confidence} bold />
          </span>
          <span>
            <FieldValue field={legs} bold />
          </span>
          <span>{ago}</span>
          <span className="text-[var(--primary)]">[PAPER]</span>
        </div>
      </div>

      <div className="xray-panel">
        <div className="font-mono text-[9.5px] tracking-widest uppercase text-[var(--primary-2)] mb-2">
          X-Ray · route breakdown
        </div>
        <div className="space-y-1">
          <XRayRow label="ROUTE" field={route} bold />
          <XRayRow label="DECOHERENCIA" field={decoherence} bold />
          <XRayRow label="TLS AMOUNT" field={tlsAmount} bold />
          {/* AUDIT-2026-08-29: the ✓ glyph is a claim — only render it for
              success-family verdicts. "rejected"/"failed"/not-computed show the
              verbatim text without a green checkmark (R8). */}
          <XRayRow
            label="SIM VERDICT"
            field={simVerdict}
            ok={isSuccessVerdict(simVerdict)}
          />
          <XRayRow label="TOKEN SAFETY" field={tokenSafety} bold />
        </div>
      </div>
    </article>
  );
}

interface XRayRowProps {
  label: string;
  field: ResolvedField;
  bold?: boolean;
  ok?: boolean;
}

/** True only for success-family verdicts. Anything else — "rejected", "failed",
 * an `n/c · …` declaration — keeps the verbatim text WITHOUT a ✓: a checkmark
 * is an earned claim (R8). */
function isSuccessVerdict(field: ResolvedField): boolean {
  if (field.state === "not_computed") return false;
  const v = field.text.toLowerCase();
  return v === "success" || v === "sim_success" || v === "included";
}

/**
 * Renders a resolved field. Computed/zero values carry their provenance in
 * `title` (hover = "where did this come from"); not-computed values carry their
 * REASON in both the text and `title` (R8: absent evidence is labelled, never
 * implied by an empty glyph).
 */
function FieldValue({ field, bold, ok }: { field: ResolvedField; bold?: boolean; ok?: boolean }) {
  const className =
    field.state === "not_computed"
      ? "text-[var(--muted)]"
      : ok
        ? "text-[var(--success)]"
        : bold
          ? "text-[var(--foreground)] font-bold"
          : "";
  return (
    <span
      className={className}
      data-field-state={field.state}
      data-field-source={field.state === "not_computed" ? undefined : field.source}
      data-field-reason={field.state === "not_computed" ? field.reason : undefined}
      title={field.state === "not_computed" ? field.reason : field.source}
    >
      {/* The ✓ is an earned claim: it only ever precedes a success-family
          verdict (see isSuccessVerdict). */}
      {ok ? "✓ " : ""}
      {field.text}
    </span>
  );
}

function XRayRow({ label, field, bold, ok }: XRayRowProps) {
  return (
    <div className="flex justify-between gap-3 font-mono text-[10.5px] tracking-wide py-0.5 text-[var(--muted)]">
      <span className="shrink-0">{label}</span>
      <span className="text-right">
        <FieldValue field={field} bold={bold} ok={ok} />
      </span>
    </div>
  );
}
