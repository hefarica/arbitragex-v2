import type { OpportunityRow } from "@/lib/schemas";
import { deriveHopCount, parseRouteMetadata } from "@/lib/store/types";
import { terminalOpportunityState } from "@/lib/opportunity-presentation";

// Additive raw metadata carried by this endpoint, parsed below before use.
// The older OpportunityRow schema is unchanged; an absent field stays absent.
export type HomeOpportunitySource = OpportunityRow & { route_metadata?: unknown };

// =============================================================================
// R8/R10 CARD-FIELD CONTRACT (A8-CONF-01, 2026-10-03)
// =============================================================================
//
// The X-Ray card used to render four different KINDS of empty with the same
// glyph: a "—" for "producer ran and returned null", a "—" for "no producer
// exists anywhere in the stack", a fabricated "pendiente" default for a field
// nothing ever populated, and a hardcoded null for a value the API was in fact
// shipping. An operator cannot tell those apart, which is the silent
// degradation R8/R10 forbid.
//
// Every card field is therefore resolved to exactly two shapes: a REAL value
// (with the artefact it came from) or an EXPLICIT not-computed declaration
// (with a machine-readable reason). There is no third option, so the card can
// neither paint a mute "—" nor paint a 0 that no producer computed.

/** A card field: a real value, or a declaration of why there is none. */
export type ResolvedField =
  /** `computed` = real value; `zero` = the producer computed EXACTLY zero. */
  | { state: "computed" | "zero"; text: string; source: string }
  | { state: "not_computed"; text: string; reason: string };

/** Build a not-computed field. `text` is always renderable — never a bare "—". */
function notComputed(reason: string): ResolvedField {
  return { state: "not_computed", reason, text: `n/c · ${reason}` };
}

/**
 * Producer ledger — declared in code so "which fields still have no producer"
 * is a readable, reviewable fact rather than tribal knowledge.
 *
 * COMPUTED end-to-end:
 *   confidence   ← scored_opportunities.posterior_prob (Gate-C ConfidenceScore
 *                   via searcher-rs → XADD arbx:scoring:scored → archiver → PG
 *                   → api-server a8ScoringFromRow → confidence_score_bps)
 *   token_safety ← token_in_info.validation.score / token_out_info.validation.score
 *                   (api-server tokenValidation worker → table token_validations)
 *   sim_verdict  ← opportunities.status + rejection_reason (terminal lifecycle)
 *   yield/legs/route ← opportunities.roi_pct, route_metadata.{token_addresses,
 *                   dex_adapters} (persisted at detection)
 *
 * DECLARED WITHOUT PRODUCER (see the resolvers below for the exact reason code):
 *   decoherence  — Decohérencia de Estado is SLIPPAGE; this feed carries
 *                  roi_pct (convergence) and, when the producer emits it,
 *                  economics.slippage_usd with its own declared reason
 *                  (`priced_by_amm_curve`). Rendering convergence under a
 *                  slippage label — the previous behaviour — was a mislabel.
 *   tls_amount   — TLS = Temporal Liquidity Superposition (flash-loan
 *                  principal). No field on /api/opportunities/live carries a
 *                  flash-loan principal. economics.flash_fee_usd is a FEE, so it
 *                  is not a substitute, and reading a route notional as "the
 *                  flash amount" would be a guess (RULE 00).
 */
export const CARD_FIELD_PRODUCER = {
  confidence: "scored_opportunities.posterior_prob",
  token_safety: "token_in_info.validation.score / token_out_info.validation.score",
  sim_verdict: "opportunities.status + rejection_reason",
  yield: "opportunities.roi_pct",
  legs: "route_metadata.token_addresses",
  route: "route_metadata.dex_adapters",
  decoherence: null,
  tls_amount: null,
} as const satisfies Record<string, string | null>;

/** Machine-readable reasons. Stable strings — the UI and the audit share them. */
export const CARD_FIELD_REASON = {
  roiNull: "roi_pct_null",
  routeTopologyUnresolved: "route_topology_unresolved",
  /** The payload carries no confidence member at all (pre-wire producer). */
  confidenceAbsentOnWire: "no_confidence_field_on_wire",
  /** Wire declares confidence_state=not_computed but shipped no reason. */
  confidenceNotComputedFallback: "no_scored_row_for_opportunity",
  decoherenceNoProducer: "no_slippage_producer_on_feed",
  tlsAmountNoProducer: "no_producer__flash_loan_principal_not_emitted",
  tokenSafetyPending: "validation_pending_first_sighting",
  simVerdictNoProducer: "no_producer__sim_classification_not_emitted",
} as const;

// ── Per-field resolvers (pure; unit-tested directly) ─────────────────────────

/** Convergence (ROI). null/NaN/Infinity = not reported by the producer. */
export function resolveYield(roi: number | null | undefined): ResolvedField {
  if (roi == null || !Number.isFinite(roi)) {
    return notComputed(CARD_FIELD_REASON.roiNull);
  }
  return {
    state: roi === 0 ? "zero" : "computed",
    text: `${roi >= 0 ? "+" : ""}${roi.toFixed(2)}%`,
    source: "opportunities.roi_pct",
  };
}

/**
 * A.8 confidence. Basis points → percent, with the three facts kept distinct:
 *   - no scored row / wire declares not_computed → NOT COMPUTED + reason
 *   - posterior exactly 0                        → CERO REAL  ("0% conf")
 *   - posterior > 0 but < 1 bp                   → COMPUTED   ("<0.01% conf")
 * The last case is why `posterior_prob` rides the wire un-rounded: rounding to
 * integer bps would dress a real 0.0027% as a hard 0.
 */
export function resolveConfidence(opp: {
  confidence_score_bps?: number | null;
  posterior_prob?: number | null;
  confidence_state?: "computed" | "not_computed" | null;
  confidence_reason?: string | null;
  confidence_source?: string | null;
}): ResolvedField {
  const bps = opp.confidence_score_bps;
  if (
    opp.confidence_state === "not_computed" ||
    bps == null ||
    !Number.isInteger(bps)
  ) {
    const reason =
      opp.confidence_reason ??
      (opp.confidence_state == null
        ? CARD_FIELD_REASON.confidenceAbsentOnWire
        : CARD_FIELD_REASON.confidenceNotComputedFallback);
    return { state: "not_computed", reason, text: `conf n/c · ${reason}` };
  }
  const source = opp.confidence_source ?? "confidence_score_bps";
  const pct = bps / 100;
  const raw = opp.posterior_prob;
  if (bps === 0) {
    if (raw == null || raw === 0) {
      return { state: "zero", text: "0% conf", source };
    }
    return { state: "computed", text: "<0.01% conf", source };
  }
  if (pct < 1) {
    return { state: "computed", text: `${pct.toFixed(2)}% conf`, source };
  }
  return { state: "computed", text: `${Math.round(pct)}% conf`, source };
}

/** Hop count from the persisted topology. Never derived from venue cardinality. */
export function resolveLegs(legs: number | null): ResolvedField {
  if (legs == null) {
    return notComputed(CARD_FIELD_REASON.routeTopologyUnresolved);
  }
  return {
    state: legs === 0 ? "zero" : "computed",
    text: `${legs} legs`,
    source: "route_metadata.token_addresses",
  };
}

/** Route breakdown from the persisted adapter list. */
export function resolveRoute(
  topology: { dex_adapters: string[] } | null,
  legs: number | null,
): ResolvedField {
  if (topology == null || legs == null || topology.dex_adapters.length === 0) {
    return notComputed(CARD_FIELD_REASON.routeTopologyUnresolved);
  }
  return {
    state: "computed",
    text: topology.dex_adapters.join(" → "),
    source: "route_metadata.dex_adapters",
  };
}

/**
 * Decohérencia de Estado = slippage. The producer's own R10 declaration is
 * preferred over anything this module could say: `economics.slippage_usd` when
 * computed, else `economics.not_computed_reasons.slippage_usd` (e.g.
 * `priced_by_amm_curve`), else the economics error that voided the whole block.
 * Convergence is NOT rendered here — it already has its own field (`yield`).
 */
export function resolveDecoherence(opp: HomeOpportunitySource): ResolvedField {
  const econ = opp.economics;
  const slip = econ?.slippage_usd;
  if (typeof slip === "number" && Number.isFinite(slip)) {
    return {
      state: slip === 0 ? "zero" : "computed",
      text: `slippage $${slip.toFixed(2)}`,
      source: "economics.slippage_usd",
    };
  }
  const declared = econ?.not_computed_reasons?.["slippage_usd"];
  if (declared) return notComputed(declared);
  if (econ == null) return notComputed(CARD_FIELD_REASON.decoherenceNoProducer);
  const suffix = econ.error_reason ? `:${econ.error_reason}` : "";
  return notComputed(`economics_${econ.computation_status ?? "absent"}${suffix}`);
}

/**
 * TLS AMOUNT — flash-loan principal. NO PRODUCER on this feed: grep-verified
 * that `tls_amount` / `flash_amount` appear nowhere in the API or the frontend,
 * and `economics.flash_fee_usd` is the fee, not the principal. Declared, not
 * filled (the field is a candidate for a real producer in a later change).
 */
export function resolveTlsAmount(): ResolvedField {
  return notComputed(CARD_FIELD_REASON.tlsAmountNoProducer);
}

/**
 * SIM VERDICT. Producer: the terminal lifecycle the row really recorded
 * (`status`/`rejection_reason`). The previous `?? "pendiente"` default was a
 * FABRICATED state — nothing ever reported "pending" — so an absent verdict is
 * now declared instead of invented. `sim_classification`/`simulation_status`
 * are still read first-class if a producer ever emits them; today no PG column
 * and no wire field carries them.
 */
export function resolveSimVerdict(opp: {
  status?: string | null;
  rejection_reason?: string | null;
  paper_status?: string | null;
  sim_classification?: string | null;
  simulation_status?: string | null;
}): ResolvedField {
  const terminal = terminalOpportunityState(opp);
  if (terminal != null) {
    return {
      state: "computed",
      text: terminal,
      source: "opportunities.status + rejection_reason",
    };
  }
  const sim = opp.sim_classification ?? opp.simulation_status;
  if (sim != null && sim !== "") {
    return {
      state: "computed",
      text: sim,
      source: "opportunities.sim_classification / simulation_status",
    };
  }
  return notComputed(CARD_FIELD_REASON.simVerdictNoProducer);
}

/**
 * TOKEN SAFETY — the two legs' real composite scores. Producer:
 * `token_in_info.validation` / `token_out_info.validation` (0-100
 * composeFinalScore + status taxonomy). Null on first sighting, when the async
 * validation worker has not written the row back yet: that is "pending", which
 * is a different fact from "safe".
 */
export function resolveTokenSafety(opp: HomeOpportunitySource): ResolvedField {
  const a = opp.token_in_info?.validation ?? null;
  const b = opp.token_out_info?.validation ?? null;
  if (a == null && b == null) {
    return notComputed(CARD_FIELD_REASON.tokenSafetyPending);
  }
  const fmt = (v: { score?: number | null; status?: string | null } | null, tag: string) => {
    if (v == null) return `${tag} n/c`;
    const score = v.score == null ? "n/c" : String(v.score);
    return `${tag} ${score} ${v.status ?? "n/c"}`;
  };
  const bothZero = a?.score === 0 && b?.score === 0;
  return {
    state: bothZero ? "zero" : "computed",
    text: `${fmt(a, "A")} · ${fmt(b, "B")}`,
    source: "token_in_info.validation.score / token_out_info.validation.score",
  };
}

// ── Card projection ──────────────────────────────────────────────────────────

/**
 * Map a real OpportunityRow onto the XRayCard props. Every field derives from
 * the API payload, and every field that the payload cannot support arrives as
 * an explicit not-computed declaration with its reason — never a "—" the
 * operator has to interpret, and never a 0 standing in for a missing datum.
 */
export function toXRayProps(opp: HomeOpportunitySource) {
  const topology = parseRouteMetadata(opp.route_metadata);
  const legs = deriveHopCount(topology);
  const pair =
    opp.pair_symbol ?? `${opp.token_in.slice(0, 6)}…/${opp.token_out.slice(0, 6)}…`;
  return {
    pair,
    yield: resolveYield(opp.roi_pct),
    confidence: resolveConfidence(opp),
    legs: resolveLegs(legs),
    ago: opp.detected_at,
    route: resolveRoute(topology, legs),
    decoherence: resolveDecoherence(opp),
    tlsAmount: resolveTlsAmount(),
    simVerdict: resolveSimVerdict(opp),
    tokenSafety: resolveTokenSafety(opp),
  };
}

export type XRayProps = ReturnType<typeof toXRayProps>;
