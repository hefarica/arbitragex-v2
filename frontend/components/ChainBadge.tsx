import React from "react";
import { chainInfoSync } from "@/lib/chains";

/**
 * ChainBadge.tsx — Pure display pill identifying the chain an opportunity
 * lives on. Renders the chain short label (ETH, ARB, BASE…) plus the
 * canonical chain name on hover (title attribute) with brand-aligned
 * color tokens defined in `lib/chains.ts::getChainStyle`.
 *
 * R1 Mounted Snapshot compliance: pure component, no hooks, no Date/Math
 * randomness, deterministic across SSR + CSR.
 *
 * R8 fail-honest: unknown chain_id renders as "#<id>" with neutral slate
 * styling — never claims a chain we don't know.
 */

export interface ChainBadgeProps {
  /** FE-0029 (§28): null = malformed payload (no fabricated chain 0) → "—". */
  chain_id: number | null;
  /** When true, also renders the numeric chain_id alongside the short label
   *  (useful in compact tables where the operator may not know "ARB" = 42161). */
  withId?: boolean;
  /** Size variant. "sm" is the table-row default; "md" is for headers. */
  size?: "sm" | "md";
}

export function ChainBadge({ chain_id, withId = false, size = "sm" }: ChainBadgeProps) {
  const sizeCls = size === "md"
    ? "px-2.5 py-1 text-xs"
    : "px-1.5 py-0.5 text-[10px]";
  // FE-0029 (§28): absent chain — honest dash, never a "chain 0" claim.
  if (chain_id == null) {
    return (
      <span
        title="chain_id ausente en el payload (§28) — no se fabrica"
        className={`inline-flex items-center gap-1 rounded-md border font-bold tracking-wide uppercase bg-muted/60 text-muted-foreground border-border ${sizeCls}`}
      >
        <span aria-hidden="true" className="size-1.5 rounded-full bg-muted-foreground/40" />
        <span>—</span>
      </span>
    );
  }
  const info = chainInfoSync(chain_id);
  return (
    <span
      title={`${info.name} (chain_id ${chain_id})`}
      className={`inline-flex items-center gap-1 rounded-md border font-bold tracking-wide uppercase ${sizeCls} ${info.color ?? ""} ${info.bg ?? ""} ${info.border ?? ""}`}
    >
      <span
        aria-hidden="true"
        className={`size-1.5 rounded-full ${info.color?.replace("text-", "bg-") ?? "bg-slate-300"}`}
      />
      <span>{info.short}</span>
      {withId && <span className="opacity-60 font-normal">#{chain_id}</span>}
    </span>
  );
}
