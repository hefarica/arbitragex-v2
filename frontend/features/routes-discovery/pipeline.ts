/**
 * FE-MASTER · Market Event Pipeline KPI model (FE-0026 — §18).
 *
 * Pure, framework-free: the §18 funnel slots, each bound to the wire field
 * that ACTUALLY carries it (RouteDiscoveryTickSummary — the same shape the
 * worker publishes to the WS room and the durable snapshot). Absent group =
 * honest "—" (a knob-off or path-not-taken tick legitimately omits it).
 *
 * RULE 00 on filters: the §18 brief says "cada KPI filtra dataset" — but a
 * KPI can only filter what carries a per-route flag on the routes wire.
 * Only `strategies` does (applicable_strategies on every RouteEntry); the
 * rest are TICK AGGREGATES with no per-route flag, and fabricating one
 * would invent data. Aggregate KPIs render non-interactive with the reason
 * in their title; the sized / net-positive / sim-PASS slots have NO wire
 * today (the tick stops at dispatch — sizing/economics/sim live past it)
 * and render as honest gaps, never zeros.
 */

import type { RouteDiscoveryTickSummary } from "@/lib/apex/schemas";

export interface RouteEntryLike {
  applicable_strategies: string[];
}

export type PipelineKpiId =
  | "dirty_pools"
  | "pairs"
  | "evaluated"
  | "prefilter"
  | "hot_seeds"
  | "routes"
  | "strategies"
  | "sized"
  | "net_positive"
  | "sim_pass";

export interface PipelineKpi {
  id: PipelineKpiId;
  /** §18 display label. */
  label: string;
  /** §40 explain-yourself hint (no apostrophes, no " — "). */
  hint: string;
  /** null = not served on this tick (honest dash). */
  value: (tick: RouteDiscoveryTickSummary | null) => number | string | null;
  /** Present only when a per-route predicate exists (see header note). */
  filter?: (route: RouteEntryLike) => boolean;
}

/** Σ strategy_status_counts — the registry census, not a per-tick dispatch. */
function strategyCensus(tick: RouteDiscoveryTickSummary | null): number | null {
  const counts = tick?.strategy_status_counts;
  if (!counts) return null;
  let sum = 0;
  for (const n of Object.values(counts)) sum += n;
  return sum;
}

export const PIPELINE_KPIS: readonly PipelineKpi[] = [
  {
    id: "dirty_pools",
    label: "dirty pools",
    hint: "pool-dirty events drained into the dirty set this window (drain_drained)",
    value: (t) => t?.drain_drained ?? null,
  },
  {
    id: "pairs",
    label: "pairs",
    hint: "dirty pair seeds alive after the drain (dirty_seeds)",
    value: (t) => t?.dirty_seeds ?? null,
  },
  {
    id: "evaluated",
    label: "evaluados",
    hint: "routes evaluated by the F_e prefilter (fe_prefilter_evaluated — absent when the knob is OFF)",
    value: (t) => t?.fe_prefilter_evaluated ?? null,
  },
  {
    id: "prefilter",
    label: "prefilter",
    hint: "routes that passed the F_e reference (fe_prefilter_pass — signal, never proof)",
    value: (t) => t?.fe_prefilter_pass ?? null,
  },
  {
    id: "hot_seeds",
    label: "hot seeds",
    hint: "the hot seed the current dispatch policy was selected from (multi_hop_hot_seed — a name, not a count)",
    value: (t) => t?.multi_hop_hot_seed ?? null,
  },
  {
    id: "routes",
    label: "routes",
    hint: "routes found by the pair finder this tick (routes_found — the dataset below is this stage)",
    value: (t) => t?.routes_found ?? null,
    filter: () => true,
  },
  {
    id: "strategies",
    label: "strategies",
    hint: "registry status census this tick (Σ strategy_status_counts — workbook-wide, not per-tick dispatch); filter = routes with at least one applicable strategy",
    value: (t) => strategyCensus(t),
    filter: (r) => r.applicable_strategies.length > 0,
  },
  {
    id: "sized",
    label: "sized",
    hint: "no emitido en el wire del tick — sizing vive aguas abajo del dispatch (nivel-(b), jamás un cero fabricado)",
    value: () => null,
  },
  {
    id: "net_positive",
    label: "net positive",
    hint: "no emitido en el wire del tick — la economía neta se evalúa aguas abajo (nivel-(b), jamás un cero fabricado)",
    value: () => null,
  },
  {
    id: "sim_pass",
    label: "sim PASS",
    hint: "no emitido en el wire del tick — el simulador corre aguas abajo del dispatch (nivel-(b), jamás un cero fabricado)",
    value: () => null,
  },
];

export const PIPELINE_KPI_IDS: readonly PipelineKpiId[] = PIPELINE_KPIS.map((k) => k.id);

/** KPIs that own a real per-route predicate (the only clickable ones). */
export const FILTERABLE_KPIS: readonly PipelineKpiId[] = PIPELINE_KPIS
  .filter((k) => typeof k.filter === "function")
  .map((k) => k.id);

/**
 * Pure grid filter. `null` = no filter (all routes). An id WITHOUT a
 * predicate returns the input unchanged — call sites gate clickability on
 * FILTERABLE_KPIS so this arm is defensive only.
 */
export function filterRoutesByKpi<R extends RouteEntryLike>(
  routes: readonly R[],
  id: PipelineKpiId | null,
): R[] {
  if (id === null) return routes as R[];
  const kpi = PIPELINE_KPIS.find((k) => k.id === id);
  if (!kpi?.filter) return routes as R[];
  return routes.filter(kpi.filter);
}

// ─── FE-0027 — hop view-controls (§19 §20 §63) ─────────────────────────────

/**
 * The CONTROL range 2..7 is the runtime hot-path hop policy (workbook canon
 * spans further — up to 16 legs — but the FE-MASTER brief pins the control
 * surface to the runtime policy). Counts are NEVER pinned: they derive from
 * the served dataset.
 */
export const HOP_CONTROL_RANGE: readonly number[] = [2, 3, 4, 5, 6, 7];

export interface RouteHopsLike {
  hops: number;
}

/** Per-hop-count route counts FROM THE SERVED DATASET (no registry pins). */
export function hopCounts<R extends RouteHopsLike>(
  routes: readonly R[],
): Map<number, number> {
  const counts = new Map<number, number>();
  for (const r of routes) counts.set(r.hops, (counts.get(r.hops) ?? 0) + 1);
  return counts;
}

/**
 * Pure view filter over a multi-selected hop set. `null` = no filter.
 * Hops outside the set simply do not match — the caller decides whether an
 * empty selection means "none" or "all" (UI treats none-selected as null).
 */
export function filterRoutesByHops<R extends RouteHopsLike>(
  routes: readonly R[],
  active: ReadonlySet<number> | null,
): R[] {
  if (active === null || active.size === 0) return routes as R[];
  return routes.filter((r) => active.has(r.hops));
}
