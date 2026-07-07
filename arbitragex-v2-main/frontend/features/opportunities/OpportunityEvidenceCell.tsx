"use client";

/**
 * OpportunityEvidenceCell — rendering primitives for the simulation/evidence
 * columns of the opportunities table.
 *
 * R8 fail-honest contract (CRITICAL — read before editing):
 *   - `null`            → renders "Unavailable" (gray dim) — backend never
 *                         emitted this field for this opportunity.
 *   - `undefined`       → renders "Unavailable" — same as null.
 *   - `""` (empty str)  → renders "Unavailable" — present but unset.
 *   - `0` (real zero)   → renders "0" — measured zero, NOT unavailable.
 *   - non-empty string  → renders the value verbatim or short hash.
 *
 * NEVER:
 *   - Convert null → 0.
 *   - Convert null → "—" as if it were a real measured dash.
 *   - Mark an opportunity as viable when sim_classification is null.
 *   - Fabricate trace_hash or net_profit values.
 */

import * as React from "react";
import { Badge } from "@/components/ui/badge";
import { cn } from "@/lib/utils";

function Unavailable({ reason }: { reason?: string }) {
  return (
    <span
      data-slot="evidence-unavailable"
      className="text-xs italic text-muted-foreground/60"
      title={reason ?? "Backend did not emit this field for this row."}
    >
      Unavailable
    </span>
  );
}

function SimClassification({ value }: { value: string | null }) {
  if (value == null || value === "") return <Unavailable reason="No simulation classification yet." />;
  const upper = value.toUpperCase();
  const tone =
    upper === "SIM_SUCCESS" ? "success"
    : upper === "SIM_REVERT" ? "destructive"
    : upper === "SIM_HALT" ? "destructive"
    : upper === "SIM_SKIP" ? "outline"
    : "outline";
  return (
    <Badge
      data-slot="evidence-sim-classification"
      data-value={upper}
      variant={tone === "success" ? "info" : tone === "destructive" ? "destructive" : "outline"}
      className="font-mono text-[10px]"
    >
      {upper}
    </Badge>
  );
}

function NetProfit({ usd, wei }: { usd: number | null; wei: string | null }) {
  // USD is the operator-facing primary value. Wei is the wire-truth fallback
  // shown as a tooltip when USD is null but wei is non-null (rare path — the
  // backend usually emits both or neither).
  if (usd === null && (wei === null || wei === "")) {
    return <Unavailable reason="Simulation has not produced net yield yet." />;
  }
  if (usd === null) {
    return (
      <span
        data-slot="evidence-net-yield"
        data-source="wei-only"
        className="text-xs italic text-muted-foreground"
        title={`USD conversion pending — raw wei: ${wei}`}
      >
        wei only
      </span>
    );
  }
  // usd === 0 is a real measured zero. Render "$0.00", not Unavailable.
  const sign = usd > 0 ? "text-success" : usd < 0 ? "text-destructive" : "text-muted-foreground";
  const formatted = new Intl.NumberFormat("en-US", {
    style: "currency",
    currency: "USD",
    minimumFractionDigits: 2,
    maximumFractionDigits: 2,
  }).format(usd);
  return (
    <span data-slot="evidence-net-yield" data-source="usd" className={cn("font-semibold", sign)}>
      {formatted}
    </span>
  );
}

function GasUsed({ value }: { value: string | number | null }) {
  if (value === null || value === undefined || value === "") {
    return <Unavailable reason="Gas not measured yet (sim pending or skipped)." />;
  }
  // 0 is allowed as a real measurement (sim was halted before any opcode ran).
  const text = typeof value === "number" ? value.toLocaleString("en-US") : value;
  return (
    <span data-slot="evidence-gas-used" className="text-xs">
      {text}
    </span>
  );
}

function Hash({ value }: { value: string | null }) {
  if (value === null || value === "") return <Unavailable reason="No trace hash recorded yet." />;
  // Display first 6 + last 4 chars of the hex. Tooltip carries the full hash.
  const cleaned = value.startsWith("0x") ? value : `0x${value}`;
  const short =
    cleaned.length > 14 ? `${cleaned.slice(0, 8)}…${cleaned.slice(-4)}` : cleaned;
  return (
    <code
      data-slot="evidence-hash"
      className="font-mono text-[10px] text-muted-foreground"
      title={cleaned}
    >
      {short}
    </code>
  );
}

function Gates({
  evidence,
  netProfit,
}: {
  evidence: string | null;
  netProfit: string | null;
}) {
  if ((evidence === null || evidence === "") && (netProfit === null || netProfit === "")) {
    return <Unavailable reason="No gate verdicts yet (sim or evidence pipeline pending)." />;
  }
  return (
    <div className="flex flex-wrap gap-1">
      <GateBadge label="EV" value={evidence} />
      <GateBadge label="NET" value={netProfit} />
    </div>
  );
}

function GateBadge({ label, value }: { label: string; value: string | null }) {
  if (value === null || value === "") {
    return (
      <Badge
        variant="outline"
        className="font-mono text-[9px] text-muted-foreground/60"
        title={`${label} gate not run`}
      >
        {label}: —
      </Badge>
    );
  }
  const upper = value.toUpperCase();
  const isPass = upper === "PASS";
  return (
    <Badge
      data-slot={`gate-${label.toLowerCase()}`}
      variant={isPass ? "info" : "destructive"}
      className="font-mono text-[9px]"
      title={`${label} gate: ${upper}`}
    >
      {label}: {upper}
    </Badge>
  );
}

// A.8: render basis-points scores honestly. null → "Unavailable", real value
// → bps with explanatory tooltip. Backend pipeline currently NEVER emits these,
// so today's UI is uniformly "Unavailable" — exactly the R8 fail-honest output.
function ScoreBps({
  value,
  label,
  hint,
}: {
  value: number | null | undefined;
  label: string;
  hint?: string;
}) {
  if (value === null || value === undefined) {
    return <Unavailable reason={`Backend has not produced a ${label} for this opportunity yet (A.8 pipeline pending).`} />;
  }
  // bps = basis points = value / 10000. Render as both raw bps and pct.
  const pct = (value / 100).toFixed(2);
  return (
    <span
      data-slot={`evidence-${label.toLowerCase()}-bps`}
      className="font-mono text-xs"
      title={hint ?? `${value} basis points (${pct}%)`}
    >
      {value}
      <span className="ml-1 text-[10px] text-muted-foreground">bps</span>
    </span>
  );
}

function ScoringDecision({ value }: { value: string | null }) {
  if (value === null || value === "") {
    return <Unavailable reason="No scoring decision recorded for this opportunity yet (A.8 pipeline pending)." />;
  }
  const upper = value.toUpperCase();
  const isAccept = upper === "ACCEPT_PAPER";
  const isBlocked = upper.startsWith("BLOCKED");
  const variant = isAccept ? "info" : isBlocked ? "outline" : "destructive";
  return (
    <Badge data-slot="evidence-scoring-decision" variant={variant} className="font-mono text-[10px]">
      {upper}
    </Badge>
  );
}

export const OpportunityEvidenceCell = {
  SimClassification,
  NetProfit,
  GasUsed,
  Hash,
  Gates,
  ScoreBps,
  ScoringDecision,
};
