import React from "react";

/**
 * StrategyBadge.tsx — Pure display component for resolution engine kind labels.
 *
 * OMEGA Lexicón: All labels use the QuantumX academic terminology.
 * Backend field names (strategy_kind values) are preserved in props/state
 * but NEVER displayed raw to the operator.
 *
 * R1 Mounted Snapshot compliance:
 *   - No Date.now(), no Math.random(), no window/document, no hooks.
 *   - SSR render === CSR render. Zero hydration risk.
 */

/** Mirrors StrategyKind from shared-ts/src/contracts/index.ts — no cross-package import. */
export type StrategyKind =
  | "dex_arb"
  | "triangular"
  | "backrun"
  | "liquidation"
  | "flashloan_arb";

interface StrategyMeta {
  label: string;
  /** Tailwind class string for the badge appearance. */
  className: string;
}

/**
 * Exhaustive map: Record<StrategyKind, StrategyMeta>.
 * TypeScript enforces all 5 keys are present.
 * OMEGA Lexicón applied to all display labels.
 */
const STRATEGY_MAP: Record<StrategyKind, StrategyMeta> = {
  dex_arb: {
    label: "DEX CONVERGENCE",
    className:
      "bg-primary/10 text-primary border border-primary/30",
  },
  triangular: {
    label: "TRIANGULAR RESOLUTION",
    className:
      "bg-accent text-accent-foreground border border-border",
  },
  backrun: {
    label: "TEMPORAL BACKRUN",
    className:
      "bg-warning/10 text-warning border border-warning/30",
  },
  liquidation: {
    label: "ENTROPY LIQUIDATION",
    className:
      "bg-destructive/10 text-destructive border border-destructive/30",
  },
  flashloan_arb: {
    label: "FLASH CONVERGENCE",
    className:
      "bg-info/10 text-info border border-info/30",
  },
};

export interface StrategyBadgeProps {
  strategy_kind: StrategyKind;
}

export function StrategyBadge({ strategy_kind }: StrategyBadgeProps) {
  const meta = STRATEGY_MAP[strategy_kind];

  // meta can only be undefined at runtime if an unknown string slips past TypeScript.
  // Defensive fallback per R8 fail-honest: surface the raw value, never hide it.
  if (!meta) {
    return (
      <span className="inline-flex items-center px-2 py-0.5 rounded text-xs font-bold uppercase tracking-wide bg-muted/60 text-muted-foreground border border-border">
        {String(strategy_kind)}
      </span>
    );
  }

  return (
    <span
      className={`inline-flex items-center px-2 py-0.5 rounded text-xs font-bold uppercase tracking-wide ${meta.className}`}
    >
      {meta.label}
    </span>
  );
}
