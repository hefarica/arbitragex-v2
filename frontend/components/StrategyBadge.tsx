import React from "react";
import { familyOf, familyColour, isBaseStrategy } from "@/lib/strategy-kinds";

/**
 * StrategyKind is now `string` — accepts any strategy_kind value from the
 * backend (5 base + 264 cartridge IDs). Kept as a named export for backward
 * compat with files that import `{ StrategyKind }` from this module.
 */
export type StrategyKind = string;

/**
 * StrategyBadge.tsx — Pure display component for strategy kind labels.
 *
 * Renders ALL 269 strategy kinds (5 base + 264 cartridges). Base kinds get
 * their canonical label/colour; cartridge kinds get their MEV family prefix
 * (e.g., "MEV-01") with a family-coloured badge.
 *
 * R1 Mounted Snapshot compliance:
 *   - No Date.now(), no Math.random(), no window/document, no hooks.
 *   - SSR render === CSR render. Zero hydration risk.
 */

interface StrategyMeta {
  label: string;
  /** Tailwind class string for the badge appearance. */
  className: string;
}

/**
 * Base 5 strategies — exhaustive map with canonical OMEGA labels.
 */
const STRATEGY_MAP: Record<string, StrategyMeta> = {
  dex_arb: {
    label: "DEX CONVERGENCE",
    className: "bg-primary/10 text-primary border border-primary/30",
  },
  triangular: {
    label: "TRIANGULAR RESOLUTION",
    className: "bg-accent text-accent-foreground border border-border",
  },
  backrun: {
    label: "TEMPORAL BACKRUN",
    className: "bg-warning/10 text-warning border border-warning/30",
  },
  liquidation: {
    label: "ENTROPY LIQUIDATION",
    className: "bg-destructive/10 text-destructive border border-destructive/30",
  },
  flashloan_arb: {
    label: "FLASH CONVERGENCE",
    className: "bg-info/10 text-info border border-info/30",
  },
};

/**
 * Plain-text display label for a strategy_kind — the same three-branch
 * mapping StrategyBadge renders, as a string (for headers which need the
 * name without the badge chrome, e.g. the exchange card badge).
 * FE-0029 (§28): null (malformed payload, no fabricated kind) → "—".
 */
export function strategyLabel(strategy_kind: string | null): string {
  if (strategy_kind == null) return "—";
  if (isBaseStrategy(strategy_kind)) {
    return STRATEGY_MAP[strategy_kind]?.label ?? String(strategy_kind);
  }
  if (strategy_kind.startsWith("mev_") || strategy_kind.startsWith("cartridge_")) {
    return familyOf(strategy_kind);
  }
  return String(strategy_kind);
}

export interface StrategyBadgeProps {
  /** Accepts any strategy_kind string — base families + 264 cartridge IDs.
   *  FE-0029 (§28): null = malformed payload → UNKNOWN badge, never a
   *  fabricated kind. */
  strategy_kind: string | null;
}

export function StrategyBadge({ strategy_kind }: StrategyBadgeProps) {
  // FE-0029 (§28): absent kind — an honest UNKNOWN, never a claimed family.
  if (strategy_kind == null) {
    return (
      <span className="inline-flex items-center px-2 py-0.5 rounded text-xs font-bold uppercase tracking-wide bg-muted/60 text-muted-foreground border border-border">
        UNKNOWN
      </span>
    );
  }

  // Base 5: use canonical label/colour.
  if (isBaseStrategy(strategy_kind)) {
    const meta = STRATEGY_MAP[strategy_kind];
    if (meta) {
      return (
        <span
          className={`inline-flex items-center px-2 py-0.5 rounded text-xs font-bold uppercase tracking-wide ${meta.className}`}
        >
          {meta.label}
        </span>
      );
    }
  }

  // Cartridge (264): show MEV family prefix with family-coloured badge.
  if (strategy_kind.startsWith("mev_") || strategy_kind.startsWith("cartridge_")) {
    const fam = familyOf(strategy_kind);
    return (
      <span
        className={`inline-flex items-center px-2 py-0.5 rounded text-xs font-bold uppercase tracking-wide ${familyColour(strategy_kind)}`}
      >
        {fam}
      </span>
    );
  }

  // R8 fail-honest: unknown kind — surface the raw value, never hide it.
  return (
    <span className="inline-flex items-center px-2 py-0.5 rounded text-xs font-bold uppercase tracking-wide bg-muted/60 text-muted-foreground border border-border">
      {String(strategy_kind)}
    </span>
  );
}
