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
//   - FLUSH: one batch per cadence, then empty — the store is touched once
//     per window, never per message.
//
// ── STREAM-SEQ-01 (§11.4) — OUT-OF-ORDER IS NO LONGER "LAST ARRIVAL WINS" ────
// This buffer used to document `OUT-OF-ORDER: within a window, last ARRIVAL
// wins the CONTENT`. That was an honest description of a real limitation, and
// the prompt names it as a defect to fix: "No permitas que un evento antiguo
// sobrescriba uno nuevo."
//
// Arrival order is not authority on a push channel: a NOTIFY replay, a
// reconnected socket and an overlapping REST snapshot all deliver
// indistinguishable messages, so a route observed at T1 can land AFTER the same
// route observed at T2 > T1 and overwrite the newer economics with the older
// ones. The buffer now applies `decideUpsert` (stream-contract.ts), which
// compares the row's OWN vigency clock — `last_seen_at ?? detected_at`, the same
// clock `pruneStale` evicts on — and REFUSES a strictly older observation.
//
// The two cases that must NOT regress, and are pinned by tests:
//   · a later computation on the SAME detection (migration 107 economics /
//     status update: same clock) is `same` → still an accepted UPDATE;
//   · a row we cannot date (`incomparable`) is still accepted — data we cannot
//     date is never silently dropped (same doctrine as pruneStale).
//
// Cross-id sibling check: a RE-DETECTION of the same plan arrives with a NEW id.
// Keying dedup on `id` alone let an older re-detection overwrite a newer one
// whenever both were buffered in the same window, so the sibling lookup below
// resolves the PLAN key (strategy + route + declared notional) and applies the
// same verdict across ids.

import type { OmniOpportunity } from "./types";
import {
  countOutcome,
  decideUpsert,
  EMPTY_LEDGER_COUNTERS,
  planKeyOf,
  type StreamLedgerCounters,
  type UpsertOutcome,
} from "./stream-contract";

export interface WsIngestBuffer {
  /**
   * Buffer one mapped row. Returns the sequencing verdict so a caller can
   * surface WHY a row was dropped (never a silent discard).
   * Dedup by id; an OLDER observation of the same id OR of the same plan is
   * rejected and the newer content is kept.
   */
  upsert(row: OmniOpportunity): UpsertOutcome;
  /** Return the buffered batch and clear. Empty window → []. */
  flush(): OmniOpportunity[];
  /** Drop everything without emitting (consumer unmount / dispose path). */
  clear(): void;
  /** Sequencing counters for the current window (diagnostics, §11.4). */
  counters(): StreamLedgerCounters;
}

export function createWsIngestBuffer(): WsIngestBuffer {
  const pending = new Map<string, OmniOpportunity>();
  /** plan key → the id holding that plan's newest buffered observation. */
  const planIndex = new Map<string, string>();
  let counters: StreamLedgerCounters = { ...EMPTY_LEDGER_COUNTERS };

  return {
    upsert(row) {
      const existing = pending.get(row.id);
      if (existing !== undefined) {
        const decision = decideUpsert(existing, row);
        counters = countOutcome(counters, decision.outcome);
        if (decision.apply) pending.set(row.id, row);
        return decision.outcome;
      }
      // New id: is it an older re-detection of a plan already buffered?
      const planKey = planKeyOf(row);
      const siblingId = planIndex.get(planKey);
      if (siblingId !== undefined && siblingId !== row.id) {
        const sibling = pending.get(siblingId);
        if (sibling !== undefined) {
          const decision = decideUpsert(sibling, row);
          if (!decision.apply) {
            counters = countOutcome(counters, decision.outcome);
            return decision.outcome;
          }
        }
      }
      pending.set(row.id, row);
      planIndex.set(planKey, row.id);
      counters = countOutcome(counters, "accepted_new");
      return "accepted_new";
    },
    flush() {
      const batch = Array.from(pending.values());
      pending.clear();
      planIndex.clear();
      counters = { ...EMPTY_LEDGER_COUNTERS };
      return batch;
    },
    clear() {
      pending.clear();
      planIndex.clear();
      counters = { ...EMPTY_LEDGER_COUNTERS };
    },
    counters() {
      return counters;
    },
  };
}
