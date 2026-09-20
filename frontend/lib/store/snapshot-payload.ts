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
