import React from "react";

/**
 * StrategyBadge.tsx — Pure display component for MEV strategy kind labels.
 *
 * R1 Mounted Snapshot compliance:
 *   - No Date.now(), no Math.random(), no window/document, no hooks.
 *   - SSR render === CSR render. Zero hydration risk.
 *
 * Covers all 5 StrategyKind values from shared-ts/src/contracts/index.ts:
 *   "dex_arb" | "triangular" | "backrun" | "liquidation" | "flashloan_arb"
 *
 * Exhaustiveness: the STRATEGY_MAP Record covers every literal, and TypeScript
 * will error at compile time if a new variant is added to StrategyKind without
 * updating this file (Record<StrategyKind, ...> enforces it).
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
 */
const STRATEGY_MAP: Record<StrategyKind, StrategyMeta> = {
  dex_arb: {
    label: "DEX ARB",
    className:
      "bg-primary/10 text-primary border border-primary/30",
  },
  triangular: {
    label: "TRIANGULAR",
    className:
      "bg-accent text-accent-foreground border border-border",
  },
  backrun: {
    label: "BACKRUN",
    className:
      "bg-warning/10 text-warning border border-warning/30",
  },
  liquidation: {
    label: "LIQUIDATION",
    className:
      "bg-destructive/10 text-destructive border border-destructive/30",
  },
  flashloan_arb: {
    label: "FLASHLOAN",
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
