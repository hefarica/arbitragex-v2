"use client";

/**
 * FE-MASTER · Hop view-controls (FE-0027 — §19 §20 §63).
 *
 * Hop chips 2..7 that FILTER THE VIEW. The §20 contract is explicit and
 * rendered verbatim: this is a VIEW_FILTER — it never mutates the hot-path
 * hop policy. What the runtime is ACTUALLY doing this tick rides the wire
 * (`multi_hop_hops_effective` on the tick) and is shown beside the controls
 * so the operator can never mistake the two.
 *
 * Counts come from the served dataset (hopCounts — never registry pins).
 */

// SSR-test support (repo pattern): classic JSX path needs the React namespace.
import * as React from "react";

import { HOP_CONTROL_RANGE } from "./pipeline";

interface Props {
  /** Dataset-derived per-hop counts (a hop with no served routes shows 0). */
  counts: ReadonlyMap<number, number>;
  /** Multi-selected hops; null = no filter. */
  active: ReadonlySet<number> | null;
  onToggle: (hop: number) => void;
  /** The runtime's effective hop bounds THIS tick (wire, nullable). */
  effectiveBounds: readonly [number, number] | null;
}

export function HopControls({ counts, active, onToggle, effectiveBounds }: Props) {
  return (
    <section
      aria-label="Hops view filter"
      className="flex flex-wrap items-center gap-1.5 rounded-lg border p-3"
      data-testid="hop-controls"
    >
      <span className="font-mono text-[10px] font-semibold uppercase tracking-[0.12em] text-muted-foreground">
        Hops
      </span>
      {HOP_CONTROL_RANGE.map((h) => {
        const on = active?.has(h) ?? false;
        const n = counts.get(h) ?? 0;
        return (
          <button
            key={h}
            type="button"
            aria-pressed={on}
            onClick={() => onToggle(h)}
            title={`FILTRAR LA VISTA por rutas de ${h} hops (${n} servidas) — no cambia el cap del hot-path`}
            className={`inline-flex items-center gap-1 rounded-md border px-2 py-0.5 font-mono text-xs transition-colors focus-visible:outline focus-visible:outline-2 focus-visible:outline-ring ${
              on
                ? "border-primary bg-primary/10 text-primary"
                : "border-border text-foreground hover:bg-muted/50"
            }`}
          >
            h{h}
            <span className="tabular-nums text-muted-foreground">{n}</span>
          </button>
        );
      })}
      <span
        className="ml-2 font-mono text-[10px] uppercase tracking-[0.12em] text-muted-foreground"
        title="VIEW_FILTER: estos chips solo filtran la vista. El cap de hops del hot-path es política runtime — cambiarlo es una mutación de configuración con ACK (§20 §63), nunca un click aquí."
      >
        VIEW_FILTER
      </span>
      <span
        className="font-mono text-[10px] text-muted-foreground"
        title="multi_hop_hops_effective del tick — los bounds de hops que el runtime está aplicando AHORA (wire, no configuración local)"
      >
        runtime efectivo:{" "}
        {effectiveBounds === null ? "—" : `[${effectiveBounds[0]},${effectiveBounds[1]}]`}
      </span>
    </section>
  );
}
