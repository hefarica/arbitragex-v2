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
      "bg-indigo-500/15 text-indigo-300 border border-indigo-500/30",
  },
  triangular: {
    label: "TRIANGULAR",
    className:
      "bg-violet-500/15 text-violet-300 border border-violet-500/30",
  },
  backrun: {
    label: "BACKRUN",
    className:
      "bg-amber-500/15 text-amber-300 border border-amber-500/30",
  },
  liquidation: {
    label: "LIQUIDATION",
    className:
      "bg-rose-500/15 text-rose-300 border border-rose-500/30",
  },
  flashloan_arb: {
    label: "FLASHLOAN",
    className:
      "bg-cyan-500/15 text-cyan-300 border border-cyan-500/30",
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
      <span className="inline-flex items-center px-2 py-0.5 rounded text-xs font-bold uppercase tracking-wide bg-slate-500/15 text-slate-300 border border-slate-500/30">
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
