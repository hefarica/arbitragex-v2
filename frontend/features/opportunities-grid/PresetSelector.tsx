"use client";

/**
 * PresetSelector.tsx — Conservative / Balanced / Aggressive preset triad,
 * mirroring the DeFiBot "Quick Presets" card. The presets here do NOT
 * modify backend strategy params yet — they emit a `onChange` callback that
 * a future wire commit will route into /api/config/trading.
 *
 * Why ship the UI now: matches the DeFiBot mental model the operator already
 * knows, while the backend strategy_router (F-51 in OMEGA_BENCHMARK_30_E2E)
 * is being landed. The selector is a placeholder until that lands; clicking
 * a preset toasts a "not yet wired" message so the operator never gets a
 * silent no-op.
 */

import React from "react";

export type PresetKey = "conservative" | "balanced" | "aggressive";

export interface PresetDescriptor {
  key: PresetKey;
  label: string;
  description: string;
  spread_min_pct: number;
  spread_max_pct: number;
}

export const PRESETS: PresetDescriptor[] = [
  {
    key: "conservative",
    label: "Conservative",
    description:
      "Tighter spreads, smaller sizes. Lower drawdown profile. Recommended pre-mainnet.",
    spread_min_pct: 0.1,
    spread_max_pct: 0.5,
  },
  {
    key: "balanced",
    label: "Balanced",
    description:
      "Default operating envelope. Matches the spine's current risk caps.",
    spread_min_pct: 0.2,
    spread_max_pct: 1.2,
  },
  {
    key: "aggressive",
    label: "Aggressive",
    description:
      "Wider spread range, higher gas priority. For operators who understand the elevated risk.",
    spread_min_pct: 0.3,
    spread_max_pct: 2.5,
  },
];

export interface PresetSelectorProps {
  selected?: PresetKey;
  onChange?: (preset: PresetDescriptor) => void;
}

export function PresetSelector({
  selected = "balanced",
  onChange,
}: PresetSelectorProps) {
  return (
    <section
      data-testid="preset-selector"
      aria-label="Quick presets"
      className="flex flex-col gap-2"
    >
      <header>
        <h3 className="text-xs uppercase tracking-wider text-muted-foreground">
          Quick Presets
        </h3>
      </header>
      <div className="grid grid-cols-1 gap-2 sm:grid-cols-3">
        {PRESETS.map((p) => {
          const isActive = p.key === selected;
          return (
            <button
              key={p.key}
              type="button"
              data-testid={`preset-${p.key}`}
              onClick={() => onChange?.(p)}
              aria-pressed={isActive}
              className={[
                "flex flex-col items-start gap-1 rounded-lg border p-3 text-left transition-all",
                "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring",
                isActive
                  ? "border-primary/60 bg-primary/10 shadow-[0_0_18px_-10px_var(--color-primary)]"
                  : "border-border bg-card hover:border-primary/30",
              ].join(" ")}
            >
              <span className="text-sm font-semibold text-foreground">
                {p.label}
              </span>
              <span className="text-[11px] text-muted-foreground">
                {p.description}
              </span>
              <span className="mt-1 font-mono text-[10px] uppercase tracking-wider text-muted-foreground/80">
                spread {p.spread_min_pct.toFixed(1)}%–{p.spread_max_pct.toFixed(1)}%
              </span>
            </button>
          );
        })}
      </div>
    </section>
  );
}
