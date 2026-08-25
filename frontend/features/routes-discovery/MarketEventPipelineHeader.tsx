"use client";

/**
 * FE-MASTER · Market Event Pipeline header (FE-0026 — §18).
 *
 * The §18 funnel strip over the route-discovery page: ten KPI slots in
 * doctrine order, values straight from the tick (props in — the caller
 * owns the store read and the filter state; this header is pure).
 *
 * Clickable = FILTERABLE_KPIS only (a real per-route predicate on the
 * routes wire). Aggregate slots render non-interactive with the reason in
 * their title; never-served slots (sized / net positive / sim PASS) render
 * an honest dash, never a zero.
 */

// SSR-test support (repo pattern): classic JSX path needs the React namespace.
import * as React from "react";

import {
  FILTERABLE_KPIS,
  PIPELINE_KPIS,
  type PipelineKpiId,
} from "./pipeline";
import type { RouteDiscoveryTickSummary } from "@/lib/apex/schemas";

const DASH = "—";

interface Props {
  tick: RouteDiscoveryTickSummary | null;
  /** Active KPI filter (null = none). */
  active: PipelineKpiId | null;
  /** Toggle a KPI filter (header is stateless — the page owns it). */
  onToggle: (id: PipelineKpiId) => void;
}

export function MarketEventPipelineHeader({ tick, active, onToggle }: Props) {
  return (
    <section
      aria-label="Market Event Pipeline"
      className="flex flex-wrap items-center gap-1.5 rounded-lg border p-3"
      data-testid="market-event-pipeline"
    >
      {PIPELINE_KPIS.map((kpi) => {
        const value = kpi.value(tick);
        const filterable = FILTERABLE_KPIS.includes(kpi.id);
        const isActive = active === kpi.id;
        return (
          <button
            key={kpi.id}
            type="button"
            disabled={!filterable}
            aria-pressed={filterable ? isActive : undefined}
            onClick={filterable ? () => onToggle(kpi.id) : undefined}
            title={
              filterable
                ? kpi.hint
                : `${kpi.hint} — aggregate sin flag por-ruta: no se fabrica filtro`
            }
            className={`inline-flex items-center gap-1.5 rounded-md border px-2 py-1 text-xs transition-colors font-mono ${
              isActive
                ? "border-primary bg-primary/10 text-primary"
                : filterable
                  ? "border-border text-foreground hover:bg-muted/50 focus-visible:outline focus-visible:outline-2 focus-visible:outline-ring"
                  : "cursor-default border-border/60 text-muted-foreground"
            }`}
          >
            <span className="text-muted-foreground">{kpi.label}</span>
            <span className={`font-semibold tabular-nums ${value === null ? "text-muted-foreground/60" : ""}`}>
              {value === null ? DASH : String(value)}
            </span>
          </button>
        );
      })}
    </section>
  );
}
