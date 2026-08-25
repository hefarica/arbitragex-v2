/**
 * =============================================================================
 * FE-MASTER · Quote anchor slice for the Omni-Store (FE-0013..0015, P4)
 * =============================================================================
 *
 * Live quote-anchor view over GET /api/quote/anchor (EMIT-02 Layer-2): the
 * searcher publishes a 35s-TTL snapshot per tick — this slice mirrors the
 * PairsSlice pattern (snapshot fetch + direct setter for the realtime path,
 * cadence owned by the consumer / realtime provider FE-0008).
 *
 * RULE 00 / R8: `null` = never served (render "—"); the endpoint's honest
 * 503s (`quote_anchor_not_published`, `quote_anchor_snapshot_corrupted`,
 * `redis_unavailable`) land as the error STRING — the panel displays them
 * verbatim, never a fabricated anchor. Weights ride the payload as the
 * runtime-config mirror (§9): the slice NEVER recomputes scores (§79).
 */
import type { StoreApi } from "zustand";

import type { QuoteAnchorResponse } from "@/lib/apex/schemas";
import { getQuoteAnchor } from "@/lib/api-client";
import type { FetchStatus } from "./runtime-slices";

export interface QuoteAnchorSlice {
  /** Latest flattened 8-key view (null = never served). */
  quoteAnchor: QuoteAnchorResponse | null;
  quoteAnchorStatus: FetchStatus;
  quoteAnchorError: string | null;
  quoteAnchorUpdatedAt: string | null;
  /** Snapshot fetch (caller owns cadence — snapshot TTL is 35s). */
  fetchQuoteAnchor: (chainId?: number) => Promise<void>;
  /** Direct setter for the realtime push path. */
  setQuoteAnchor: (anchor: QuoteAnchorResponse) => void;
}

export function createQuoteAnchorSlice(
  set: StoreApi<QuoteAnchorSlice>["setState"],
  get: () => QuoteAnchorSlice,
): QuoteAnchorSlice {
  return {
    quoteAnchor: null,
    quoteAnchorStatus: "idle",
    quoteAnchorError: null,
    quoteAnchorUpdatedAt: null,
    fetchQuoteAnchor: async (chainId = 1) => {
      if (get().quoteAnchorStatus === "loading") return;
      set({ quoteAnchorStatus: "loading", quoteAnchorError: null });
      const res = await getQuoteAnchor(chainId);
      if (res.ok) {
        set({
          quoteAnchor: res.data,
          quoteAnchorStatus: "ready",
          quoteAnchorUpdatedAt: new Date().toISOString(),
        });
      } else {
        set({ quoteAnchorStatus: "error", quoteAnchorError: res.error });
      }
    },
    setQuoteAnchor: (anchor) =>
      set({ quoteAnchor: anchor, quoteAnchorStatus: "ready", quoteAnchorUpdatedAt: new Date().toISOString() }),
  };
}
