import React from "react";

/**
 * CrossChainSlot.tsx — Pure display component for cross-chain bridge metadata.
 *
 * R1 Mounted Snapshot compliance:
 *   - No Date.now(), no Math.random(), no window/document, no hooks.
 *   - SSR render === CSR render. Zero hydration risk.
 *
 * Returns null when chain_id_out is null (single-chain opportunity).
 * Returns bridge + chain names when chain_id_out is set.
 *
 * CHAIN_NAMES covers the 6 chains supported by the enricher (SOP §10 + SP-A scope).
 * Unsupported chain ids fall back to "chain <id>".
 *
 * Defect #6 compliance: same 6 chains as the enricher. Linea (59144) and others
 * produce "chain 59144" — acceptable for SP-A scope, no silent failure.
 */

/** 6 chains supported by the enricher (SOP §10 / Sprint SP-A). */
const CHAIN_NAMES: Record<number, string> = {
  1: "ETH",
  42161: "ARB",
  10: "OP",
  8453: "BASE",
  137: "MATIC",
  56: "BSC",
};

/** Minimal prop shape: only the cross-chain fields from OpportunityListItem. */
export interface CrossChainSlotProps {
  opp: {
    chain_id: number;
    chain_id_out: number | null;
    bridge: string | null;
  };
}

/**
 * Returns null (renders as empty) for single-chain opportunities.
 * Returns bridge + chain labels for cross-chain opportunities.
 */
export function CrossChainSlot({ opp }: CrossChainSlotProps) {
  if (opp.chain_id_out === null) {
    return null;
  }

  const chainIn = CHAIN_NAMES[opp.chain_id] ?? `chain ${opp.chain_id}`;
  const chainOut = CHAIN_NAMES[opp.chain_id_out] ?? `chain ${opp.chain_id_out}`;
  const bridge = opp.bridge ?? "unknown";

  return (
    <span className="inline-flex items-center gap-1 text-xs font-mono">
      <span className="text-muted-foreground">{chainIn}</span>
      <span className="text-muted-foreground/60" aria-hidden="true">→</span>
      <span className="text-muted-foreground">{chainOut}</span>
      <span className="ml-1 px-1.5 py-0.5 rounded bg-info/15 text-info border border-info/30 text-xs font-bold uppercase">
        {bridge}
      </span>
    </span>
  );
}
