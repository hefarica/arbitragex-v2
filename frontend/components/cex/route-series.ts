// frontend/components/cex/route-series.ts
//
// WO-PRICE-EXCHANGE-V1 (FE) — in-memory per-route value series feeding the
// card's Sparkline from the data that ALREADY flows (WS realtime + polling).
//
// Grouping key = dex_a + pair (operator order 2026-09-18): every opportunity
// that arrives for the same route appends its value to that route's series,
// so a re-detected route's card shows the local evolution of its gross yield.
//
// Memory discipline (same school as VISIBLE_CAP / MAX_SYMBOLS):
//   - ≤20 points per route (LRU trim of the oldest),
//   - ≤500 routes tracked (Map insertion-order eviction),
//   - consecutive identical values are deduped (the 1s age-ticker re-renders
//     a card every second with an UNCHANGED value — those must not flood the
//     series; only real feed changes are recorded).
//
// R1 / SSR: the module-level Map is only mutated from useEffect (client).
// Server renders never touch it, so SSR markup stays byte-identical. When the
// backend price_history mirror exists, swap this local store for the wire
// feed — the Sparkline's `points` prop stays the contract.
import { useEffect, useState } from "react";

export const MAX_SERIES_POINTS = 20;
const MAX_ROUTES = 500;

/** route_key → chronological values (oldest → newest). Client-only state. */
const series = new Map<string, number[]>();

/** Append a value to a route's series (LRU + dedupe + cap). Pure store op. */
export function pushRoutePoint(key: string, value: number): void {
  if (!Number.isFinite(value)) return;
  const cur = series.get(key);
  if (cur !== undefined) {
    // LRU refresh: re-insertion moves the key to the back of the Map order.
    series.delete(key);
    if (cur.length === 0 || cur[cur.length - 1] !== value) cur.push(value);
    if (cur.length > MAX_SERIES_POINTS) cur.splice(0, cur.length - MAX_SERIES_POINTS);
    series.set(key, cur);
    return;
  }
  if (series.size >= MAX_ROUTES) {
    const oldest = series.keys().next();
    if (!oldest.done) series.delete(oldest.value);
  }
  series.set(key, [value]);
}

/** Read a route's series (defensive copy of the live store). */
export function getRouteSeries(key: string): number[] {
  return (series.get(key) ?? []).slice();
}

/**
 * Feed + read one route's series from a card. The value is recorded in an
 * effect (client-only), so recording never perturbs render purity. Returns
 * null until ≥2 points exist — the card then renders the Sparkline.
 */
export function useRouteSeries(
  key: string,
  value: number | null | undefined,
): number[] | null {
  const [points, setPoints] = useState<number[] | null>(null);
  useEffect(() => {
    if (value == null || !Number.isFinite(value)) return;
    pushRoutePoint(key, value);
    setPoints(getRouteSeries(key));
  }, [key, value]);
  return points != null && points.length >= 2 ? points : null;
}
