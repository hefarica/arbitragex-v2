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
  chain_id: number;
  /** When true, also renders the numeric chain_id alongside the short label
   *  (useful in compact tables where the operator may not know "ARB" = 42161). */
  withId?: boolean;
  /** Size variant. "sm" is the table-row default; "md" is for headers. */
  size?: "sm" | "md";
}

export function ChainBadge({ chain_id, withId = false, size = "sm" }: ChainBadgeProps) {
  const info = chainInfoSync(chain_id);
  const sizeCls = size === "md"
    ? "px-2.5 py-1 text-xs"
    : "px-1.5 py-0.5 text-[10px]";
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
