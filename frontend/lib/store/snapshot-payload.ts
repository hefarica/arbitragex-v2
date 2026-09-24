// frontend/lib/store/snapshot-payload.ts
//
// FE-SNAPSHOT-01: the /api/opportunities/live payload parser as a pure seam
// (same doctrine as ws-ingest-buffer). The hook's polling path used to parse
// inline; the always-on snapshot reconcile (LIVE mode refresh guarantee ≤5s)
// shares this so both paths apply IDENTICAL parsing semantics.
//
// R8 fail-honest: a payload that is neither { items: [...] } nor a bare array
// yields an honest empty list — never a fabricated row, never a crash.
import { mapToOmniOpportunity, type OmniOpportunity } from "./types";

export function parseSnapshotItems(data: unknown): OmniOpportunity[] {
  const rawItems: unknown[] =
    data != null && Array.isArray((data as { items?: unknown }).items)
    ? (data as { items: unknown[] }).items
    : Array.isArray(data)
      ? (data as unknown[])
      : [];
  return rawItems.map((raw) => mapToOmniOpportunity(raw as Record<string, unknown>));
}

/**
 * AUDIT-CARDS-MINOR (§2) · WO-H4: the envelope's `window_total` — distinct
 * routes in the ≤5 min window (COUNT(*) OVER () on the api-server LIVE_QUERY),
 * as opposed to the (capped) `items` array the grid renders.
 *
 * R8 fail-honest: absent / null / non-finite → null — an old edge or a
 * bare-array payload never fabricates a 0. A REAL 0 (computed empty window,
 * the api-server's `?? 0` on zero rows) passes through: computed-and-exactly-
 * zero is honest and must be distinguishable from "not computed".
 */
export function parseWindowTotal(data: unknown): number | null {
  if (data == null || Array.isArray(data)) return null;
  const wt = (data as { window_total?: unknown }).window_total;
  return typeof wt === "number" && Number.isFinite(wt) ? wt : null;
}
