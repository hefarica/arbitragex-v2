/**
 * FE-MASTER · Route-discovery funnel stage model (FE-0038 — §46).
 *
 * Pure, framework-free: the "Market Events → … → Reconciled" chain as stage
 * descriptors, each bound to the wire field that ACTUALLY carries it. The
 * upstream half rides the route-discovery tick (same wire the §18 header
 * reads); the downstream tail reads the outcomes sink (24h window) and the
 * recon summary (Reconciled). Windows are DISCLOSED per stage — tick counters
 * and windowed totals never share a scale, so the view renders them as a
 * tagged strip, never as one proportional bar chart.
 *
 * RULE 00: null = the field is absent from this tick (knob OFF / path not
 * taken) or the downstream wire is unavailable — an honest dash, never a
 * zero. No stage is derived from another (§79).
 */

import type { RouteDiscoveryTickSummary } from "@/lib/apex/schemas";
import type { OutcomeTotals } from "@/lib/hooks/useRouteDiscoveryOutcomes";

export type FunnelWindow = "tick" | "24h";

export interface FunnelStage {
  id: string;
  label: string;
  /** null = not carried by the wire right now (honest dash, R8). */
  value: number | null;
  /** Which window the count belongs to — disclosed, never mixed. */
  window: FunnelWindow;
  /** Wire field name — display-only provenance, never parsed. */
  source: string;
  hint: string;
}

export interface ReconTotalsLike {
  /** The §46 terminus count — the only recon field the funnel reads. */
  included: number;
}

export function buildRouteFunnelStages(
  tick: RouteDiscoveryTickSummary | null,
  outcomesTotals: OutcomeTotals | null,
  reconTotals: ReconTotalsLike | null,
): FunnelStage[] {
  return [
    {
      id: "dirty_pools",
      label: "market events → dirty pools",
      value: tick?.drain_drained ?? null,
      window: "tick",
      source: "drain_drained",
      hint: "pool reserve updates drained into the dirty set this tick (the route-discovery inflow)",
    },
    {
      id: "pairs",
      label: "pair seeds",
      value: tick?.dirty_seeds ?? null,
      window: "tick",
      source: "dirty_seeds",
      hint: "dirty pair seeds alive after the drain",
    },
    {
      id: "evaluated",
      label: "evaluados",
      value: tick?.fe_prefilter_evaluated ?? null,
      window: "tick",
      source: "fe_prefilter_evaluated",
      hint: "routes evaluated by the F_e prefilter — absent when the knob is OFF (honest gap)",
    },
    {
      id: "prefilter",
      label: "prefilter pass",
      value: tick?.fe_prefilter_pass ?? null,
      window: "tick",
      source: "fe_prefilter_pass",
      hint: "routes above the F_e reference (signal, never proof)",
    },
    {
      id: "routes",
      label: "routes found",
      value: tick?.routes_found ?? null,
      window: "tick",
      source: "routes_found",
      hint: "routes the pair finder returned this tick",
    },
    {
      id: "dispatched",
      label: "routes dispatched",
      value: tick?.routes_dispatched ?? null,
      window: "tick",
      source: "routes_dispatched",
      hint: "routes handed to strategy dispatch",
    },
    {
      id: "outcomes",
      label: "outcomes resueltos",
      value: outcomesTotals?.total ?? null,
      window: "24h",
      source: "route_discovery_outcomes.totals.total",
      hint: "resolved outcomes in the durable sink (window 24h — NOT comparable to tick counters)",
    },
    {
      id: "opportunities",
      label: "opportunities",
      value: outcomesTotals?.opportunities ?? null,
      window: "24h",
      source: "route_discovery_outcomes.totals.opportunities",
      hint: "outcomes that resolved as opportunities (window 24h)",
    },
    {
      id: "reconciled",
      label: "Reconciled",
      value: reconTotals?.included ?? null,
      window: "24h",
      source: "recon.summary.totals.included",
      hint: "ledger rows reconciled (recon summary, window 24h) — the funnel terminus",
    },
  ];
}

/**
 * §46 honesty note rendered with the strip: cross-window scaling prohibition.
 */
export const FUNNEL_WINDOW_NOTE =
  "Contadores del tick y ventanas 24h NO comparten escala — cada valor es verbatim de su wire y su ventana está etiquetada. Nulo ⇒ ausencia real (R8), jamás un cero.";
