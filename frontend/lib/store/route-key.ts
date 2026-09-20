import type { OmniOpportunity } from "./types";

/**
 * Stable route identity — ONE card per key (operator order 2026-09-20:
 * re-detections of the same route must update the SAME card, not mint a new
 * one per timestamp).
 *
 * MUST stay bit-identical to the api-server LIVE_QUERY GROUP BY twin
 * (backend/api-server/src/routes/opportunities-live.ts):
 *
 *   concat_ws('|', o.chain_id::text, COALESCE(o.chain_id_out::text, ''),
 *             COALESCE(o.strategy_kind, ''), o.token_in, o.token_out,
 *             o.dex_a, COALESCE(o.dex_b, ''))
 *
 * concat_ws skips NULLs in SQL, so each NULL-capable side is COALESCEd to ''
 * and JS `join("|")` renders null/undefined as '' — both sides emit the same
 * bytes. Drift between the two = different card counts on REST vs WS for the
 * same data (merge-authority hard condition, debate 2026-09-20). The exact
 * format is pinned by route-key.test.ts; if you change one side, change both.
 */
export function routeGroupKeyOf(opp: OmniOpportunity): string {
  return [
    opp.chain_id,
    opp.chain_id_out ?? "",
    opp.strategy_kind ?? "",
    opp.token_in,
    opp.token_out,
    opp.dex_a,
    opp.dex_b ?? "",
  ].join("|");
}
