import type { OpportunityRow } from "@/lib/schemas";
import { deriveHopCount, parseRouteMetadata } from "@/lib/store/types";
import { terminalOpportunityState } from "@/lib/opportunity-presentation";

// Additive raw metadata carried by this endpoint, parsed below before use.
// The older OpportunityRow schema is unchanged; an absent field stays absent.
export type HomeOpportunitySource = OpportunityRow & { route_metadata?: unknown };

// Map a real OpportunityRow onto the XRayCard props. Every field derives from
// the API payload; anything the API leaves null renders as an honest "—".
export function toXRayProps(opp: HomeOpportunitySource) {
  const topology = parseRouteMetadata(opp.route_metadata);
  const roi = opp.roi_pct != null && Number.isFinite(opp.roi_pct) ? opp.roi_pct : null;
  const pair = opp.pair_symbol ?? `${opp.token_in.slice(0, 6)}…/${opp.token_out.slice(0, 6)}…`;
  const legs = deriveHopCount(topology);
  return {
    pair,
    yield: roi == null ? "—" : `${roi >= 0 ? "+" : ""}${roi.toFixed(2)}%`,
    // AUDIT-2026-08-29 (R8): unscored ≠ 0%. null (A.8 scorer hasn't scored
    // this opportunity) propagates as null — the card renders "— unscored",
    // never a fabricated "0% conf".
    confidence:
      opp.confidence_score_bps != null
        ? Math.round(opp.confidence_score_bps / 100)
        : null,
    legs,
    ago: opp.detected_at,
    route: topology && legs != null ? topology.dex_adapters.join(" → ") : "— (topología no resuelta)",
    fees:
      roi != null
        ? `convergence ${roi.toFixed(2)}%`
        : "—",
    tlsAmount: "—",
    simVerdict: terminalOpportunityState(opp) ?? opp.sim_classification ?? opp.simulation_status ?? "pendiente",
    // The home contract has no validated token-safety evidence.
    safetyA: null,
    safetyB: null,
  };
}
