// frontend/lib/store/ws-ingest-buffer.ts
//
// FE-0047 (§73 §33) — the MEM-RENDER-01 WS ingest buffer as a pure seam.
//
// Extracted VERBATIM from useOmniOpportunities (behavior identical, zero
// React) so the §33 semantics the directive names are testable in the node
// env without renderHook:
//   - DUPLICATE: re-broadcasts of the same id collapse to ONE row per flush
//     window (Map key semantics — the store's in-place upsert then keeps
//     the card's grid position; same doctrine).
//   - OUT-OF-ORDER: within a window, last ARRIVAL wins the CONTENT while
//     the row keeps its FIRST-arrival position (JS Map.set does not move an
//     existing key). Arrival order is the only order a push channel has —
//     timestamps are display data, never sequencing authority.
//   - FLUSH: one batch per cadence, then empty — the store is touched once
//     per window, never per message.

import type { OmniOpportunity } from "./types";

export interface WsIngestBuffer {
  /** Buffer one mapped row (dedup by id, last arrival wins the content). */
  upsert(row: OmniOpportunity): void;
  /** Return the buffered batch and clear. Empty window → []. */
  flush(): OmniOpportunity[];
  /** Drop everything without emitting (consumer unmount / dispose path). */
  clear(): void;
}

export function createWsIngestBuffer(): WsIngestBuffer {
  const pending = new Map<string, OmniOpportunity>();
  return {
    upsert(row) {
      pending.set(row.id, row);
    },
    flush() {
      const batch = Array.from(pending.values());
      pending.clear();
      return batch;
    },
    clear() {
      pending.clear();
    },
  };
}
