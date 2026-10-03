// frontend/lib/store/stream-contract.ts
//
// STREAM-SEQ-01 (§11.4 / §11.5 of PROMPT_ARBITRAGEX_BUSQUEDA_AUTONOMA_FEES_264)
// =============================================================================
// The card-stream's sequencing contract: STABLE KEYS, ORDERED EVENTS and
// IDEMPOTENT application.
//
// §11.4 asks for four things this module owns, in one place so the buffer, the
// store and the card cannot disagree:
//   · claves estables por estrategia y por plan
//       → `strategyKeyOf` / `planKeyOf` (pure, derived from wire fields only)
//   · eventos secuenciados e idempotentes
//       → `StreamEnvelope` + `sequenceVerdict` + `decideUpsert`
//   · actualización coherente de grupos de campos
//       → `planKeyOf` puts the DECLARED notional in the key, so one plan record
//         is one arithmetic: a net computed at size X can never land on legs
//         quoted at size Y. `planDigestOf` is the field-group identity.
//   · reconexión y resync · no permitas que un evento antiguo sobrescriba uno
//     nuevo
//       → `compareObservationOrder` + `decideUpsert` reject an OLDER event; a
//         replayed/duplicated event is a NO-OP (never a second application).
//
// ORDER AUTHORITY (why the clock, and what "older" means here)
// -----------------------------------------------------------
// A push channel has arrival order, and arrival order is NOT authority: a
// reconnect replays, and two producers (WS row, REST snapshot) interleave.
// The repo already has ONE vigency clock per row — `last_seen_at ?? detected_at`
// — and `omni-store.pruneStale` already evicts on it. This module reuses that
// exact clock as the ordering authority, so eviction and sequencing cannot
// disagree about which observation of a route is the live one:
//   · the grouped REST snapshot carries `last_seen_at = MAX(detected_at)` of the
//     group, so a snapshot always outranks the raw rows it aggregates;
//   · a same-instant re-detection with newly-computed fields is `same`, i.e. an
//     accepted UPDATE (migration 107 pushes economics onto an existing row and
//     must not be dropped);
//   · a strictly OLDER observation (a delayed replay, or an out-of-order WS
//     push that lost the race) is REJECTED.
// R8 / fail-honest: a row we cannot date (`last_seen_at` and `detected_at` both
// absent/unparseable) yields `incomparable` — we accept it and SAY SO. We never
// fabricate a timestamp to win an ordering contest, and we never silently drop
// data we cannot date (same doctrine as `pruneStale`).
//
// The producer-side `seq` (backend/api-server/src/stream/opportunity-stream.ts)
// is used as the PRIMARY authority when the envelope carries it; the clock is
// the fallback for the raw `new_opportunity` leg, which carries no envelope.
// Both paths are exercised by tests; neither is dead wire.

import { routeGroupKeyOf } from "./route-key";
import type { OmniOpportunity } from "./types";

/** Wire version of the event envelope. Bump when a payload group changes shape. */
export const STREAM_SCHEMA_VERSION = 2;

// =============================================================================
// Stable keys
// =============================================================================

/**
 * STABLE PER-STRATEGY KEY — the identity a search state belongs to.
 *
 * `chain_id` + `strategy_kind` are the two fields that identify "which strategy
 * is searching" on this wire (the same pair the api-server groups cartridges
 * by). Both are nullable on a malformed payload (§28) — an empty slot is kept
 * as `""` so the key stays a stable string and two malformed rows of the same
 * shape collapse to one bucket instead of crashing the map. `concat_ws`-style
 * rendering, same discipline as `routeGroupKeyOf`.
 */
export function strategyKeyOf(opp: Pick<OmniOpportunity, "chain_id" | "strategy_kind">): string {
  return [opp.chain_id ?? "", opp.strategy_kind ?? ""].join("|");
}

/**
 * The DECLARED plan size in exact wei — the notional at which this row's
 * economic figures were computed (`route_metadata.economics_amount_in_wei`,
 * ECON-DECLARE-01). `null` = no producer declared one (legacy row, or no
 * economics to attribute). This is the field that makes a "plan" a single
 * arithmetic instead of a bag of figures: §11.4's "no mezcles piernas de una
 * ruta con neto de otra" is enforced by putting it INSIDE the plan key.
 */
export function declaredPlanSizeWei(opp: OmniOpportunity): string | null {
  const wei = opp.route_metadata?.economics_amount_in_wei;
  return typeof wei === "string" && /^-?\d+$/.test(wei.trim()) ? wei.trim() : null;
}

/** Marker used when no notional was declared — a STATE, never a fabricated 0. */
export const UNDECLARED_SIZE = "undeclared";

/**
 * STABLE PER-PLAN KEY — strategy + route identity + declared notional.
 *
 * Two plans differ when (and only when) the strategy, the route topology, or
 * the size the figures were computed at differ. That is exactly the set of
 * conditions under which §11.4 says the numbers of one plan must not be mixed
 * with another: changing size, fee, financing or route is a new plan.
 */
export function planKeyOf(opp: OmniOpportunity): string {
  return `${strategyKeyOf(opp)}#${routeGroupKeyOf(opp)}@${
    declaredPlanSizeWei(opp) ?? UNDECLARED_SIZE
  }`;
}

// =============================================================================
// Observation order — the fallback authority
// =============================================================================

/**
 * The single vigency clock this repo already evicts on
 * (`omni-store.pruneStale`): `last_seen_at ?? detected_at`, in ms epoch.
 * `null` when the row carries no parseable stamp (R8: undated ≠ epoch 0).
 */
export function observationClockMs(opp: OmniOpportunity): number | null {
  const raw = opp.last_seen_at ?? opp.detected_at;
  if (raw == null) return null;
  const t = Date.parse(raw);
  return Number.isFinite(t) ? t : null;
}

export type OrderVerdict = "newer" | "same" | "older" | "incomparable";

/**
 * Compare two observations of the SAME key (plan or route group).
 * `older` is the verdict §11.4 forbids from overwriting.
 */
export function compareObservationOrder(
  prev: OmniOpportunity,
  next: OmniOpportunity,
): OrderVerdict {
  const a = observationClockMs(prev);
  const b = observationClockMs(next);
  if (a === null || b === null) return "incomparable";
  if (b > a) return "newer";
  if (b === a) return "same";
  return "older";
}

// =============================================================================
// Field-group identity (idempotency / duplicate detection)
// =============================================================================

/**
 * Digest of the FIELD GROUP a plan's update moves together. Two events with the
 * same digest are the SAME application — the second one is a NO-OP
 * (idempotent), which is what makes a reconnect replay safe.
 *
 * SCOPE (learned the hard way — `omni-store-grouping.test.ts` caught it): the
 * first version listed a handful of fields by hand and OMITTED the canonical
 * economics pair (`expected_profit_usd` / `net_expected_profit_usd`). A row that
 * arrived with a newly computed canonical net therefore digested IDENTICAL to
 * the row already held, and the idempotency check silently swallowed a real
 * update — the exact "card frozen until the next poll" failure this contract
 * exists to prevent, reintroduced by the guard itself.
 *
 * The digest is now taken over the WHOLE row with sorted keys, so a new wire
 * field cannot be added without it participating in idempotency. Cost is one
 * bounded stringify per message (rows are small and the store already
 * `JSON.stringify`s comparable blobs in the card's memo comparator).
 */
export function planDigestOf(opp: OmniOpportunity): string {
  return `${planKeyOf(opp)}\u0000${stableStringify(opp as unknown)}`;
}

/** Deterministic JSON with sorted object keys (arrays keep their order). */
function stableStringify(value: unknown): string {
  if (value === null || typeof value !== "object") return JSON.stringify(value) ?? "null";
  if (Array.isArray(value)) return `[${value.map(stableStringify).join(",")}]`;
  const entries = Object.entries(value as Record<string, unknown>)
    // `undefined` is not representable in JSON and must not create a phantom key.
    .filter(([, v]) => v !== undefined)
    .sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0));
  return `{${entries.map(([k, v]) => `${JSON.stringify(k)}:${stableStringify(v)}`).join(",")}}`;
}

/**
 * A row carrying the server's GROUP BY aggregates (`first_seen_at` /
 * `confirmations`) came from the REST snapshot leg, not from a single
 * observation. The existing precedence contract (CARDS-PRECEDENCE-01 rule 1)
 * makes it authoritative VERBATIM, and it is the only leg that can legitimately
 * CLEAR a field (the operator's "a fresh snapshot row still overrides
 * everything"). It is therefore not subject to the single-observation clock
 * gate below — that gate governs the RAW push leg, which is where arrival order
 * is genuinely meaningless.
 */
export function isGroupedSnapshotRow(opp: OmniOpportunity): boolean {
  return opp.first_seen_at != null || opp.confirmations != null;
}

// =============================================================================
// Sequenced upsert — the single decision point
// =============================================================================

export type UpsertOutcome =
  | "accepted_new"
  | "accepted_update"
  | "accepted_incomparable"
  | "duplicate"
  | "stale_rejected";

export interface UpsertDecision {
  outcome: UpsertOutcome;
  /** Machine reason, always present: why this event was applied or dropped. */
  reason: string;
  /** True when the store must write `incoming` over `prev`. */
  apply: boolean;
}

/**
 * THE decision. Given the row currently held for a key and an incoming row for
 * the same key, say whether the incoming one may be applied.
 *
 * `prev === null` → the key is new: apply.
 * Same digest → duplicate: do NOT apply (idempotent replay).
 * Grouped snapshot row → apply: the SSOT leg may clear and replace fields.
 * Older clock → stale: do NOT apply (§11.4: an old event never overwrites a new
 *   one). The rejection is REPORTED, never silent.
 * Otherwise → apply as an update; the clock is `same` (a later computation on
 *   the same detection, e.g. the migration-107 economics update) or `newer`.
 */
export function decideUpsert(
  prev: OmniOpportunity | null,
  incoming: OmniOpportunity,
): UpsertDecision {
  if (prev == null) {
    return { outcome: "accepted_new", reason: "no_prior_observation", apply: true };
  }
  if (prev === incoming) {
    return { outcome: "duplicate", reason: "identical_reference", apply: false };
  }
  if (planDigestOf(prev) === planDigestOf(incoming)) {
    return { outcome: "duplicate", reason: "identical_application_digest", apply: false };
  }
  // The REST snapshot leg is the source of truth for the whole inventory and is
  // the ONLY leg allowed to clear a field; it is not a single observation whose
  // arrival order is in question (CARDS-PRECEDENCE-01 rule 1).
  if (isGroupedSnapshotRow(incoming)) {
    return { outcome: "accepted_update", reason: "grouped_snapshot_is_ssot", apply: true };
  }
  const order = compareObservationOrder(prev, incoming);
  if (order === "older") {
    return { outcome: "stale_rejected", reason: "older_observation_clock", apply: false };
  }
  if (order === "incomparable") {
    // Accept, but never pretend we ordered it (R8).
    return { outcome: "accepted_incomparable", reason: "undated_no_order_authority", apply: true };
  }
  return { outcome: "accepted_update", reason: `clock_${order}`, apply: true };
}

// =============================================================================
// Event envelope (§11.4 — "eventos secuenciados e idempotentes")
// =============================================================================

export const STREAM_EVENT_NAME = "opportunity_stream_event";

/**
 * Per-strategy ordered emission of one plan observation.
 *
 * `seq` is monotonic per `strategy_key` (the producer's authority);
 * `snapshot_id` names the source-of-truth snapshot the observation belongs to,
 * so a resync can tell "I am behind" from "I am current".
 * `payload` is the SAME row the raw `new_opportunity` leg carries — the
 * envelope never substitutes a figure, it only stamps identity and order.
 */
export interface StreamEnvelope {
  schema_version: number;
  event_id: string;
  seq: number;
  strategy_key: string;
  plan_key: string;
  snapshot_id: string | null;
  emitted_at: string;
  kind: StreamEventKind;
  payload: Record<string, unknown>;
  /**
   * §11.3 "publica progreso real" — what the PRODUCER can certify over its own
   * session. Optional by design: a producer that cannot measure a metric omits
   * it and the card then reports ABSENT with a reason instead of substituting a
   * client-side census silently. Numeric fields are counts over the producer's
   * session window; a metric it never measured is simply not a key.
   */
  progress?: StreamPublisherProgress;
}

export interface StreamPublisherProgress {
  routes_explored?: number;
  sizes_quoted?: number;
  quotes_exact?: number;
  quotes_incomplete?: number;
  quotes_error?: number;
  budget_exhausted?: boolean;
  budget_reason?: string;
  snapshot_id?: string | null;
}

export type StreamEventKind =
  | "plan.updated"
  | "plan.rejected"
  | "strategy.progress"
  | "strategy.cleared";

export type EnvelopeParse =
  | { ok: true; envelope: StreamEnvelope }
  | { ok: false; reason: string };

function isPlainObject(v: unknown): v is Record<string, unknown> {
  return v != null && typeof v === "object" && !Array.isArray(v);
}

/**
 * Strict-enough acceptance gate for the envelope. A malformed envelope is a
 * hard reject with a machine reason (RG-1 fail-closed): the caller must NOT
 * apply it, and must NOT fall back to guessing which strategy it belonged to.
 */
export function parseStreamEnvelope(raw: unknown): EnvelopeParse {
  if (!isPlainObject(raw)) return { ok: false, reason: "not_an_object" };
  const version = raw.schema_version;
  if (typeof version !== "number" || version !== STREAM_SCHEMA_VERSION) {
    return { ok: false, reason: `schema_version_mismatch:${String(version)}` };
  }
  const seq = raw.seq;
  if (typeof seq !== "number" || !Number.isInteger(seq) || seq < 0) {
    return { ok: false, reason: "seq_not_a_non_negative_integer" };
  }
  const eventId = raw.event_id;
  if (typeof eventId !== "string" || eventId.length === 0) {
    return { ok: false, reason: "event_id_absent" };
  }
  const strategyKey = raw.strategy_key;
  if (typeof strategyKey !== "string" || strategyKey.length === 0) {
    return { ok: false, reason: "strategy_key_absent" };
  }
  const planKey = raw.plan_key;
  if (typeof planKey !== "string" || planKey.length === 0) {
    return { ok: false, reason: "plan_key_absent" };
  }
  const kind = raw.kind;
  if (
    kind !== "plan.updated" &&
    kind !== "plan.rejected" &&
    kind !== "strategy.progress" &&
    kind !== "strategy.cleared"
  ) {
    return { ok: false, reason: `unknown_kind:${String(kind)}` };
  }
  if (!isPlainObject(raw.payload)) return { ok: false, reason: "payload_not_an_object" };
  const snapshotId = raw.snapshot_id;
  if (snapshotId != null && typeof snapshotId !== "string") {
    return { ok: false, reason: "snapshot_id_not_a_string" };
  }
  const emittedAt = raw.emitted_at;
  if (typeof emittedAt !== "string" || emittedAt.length === 0) {
    return { ok: false, reason: "emitted_at_absent" };
  }
  const progress = parsePublisherProgress(raw.progress);
  return {
    ok: true,
    envelope: {
      schema_version: version,
      event_id: eventId,
      seq,
      strategy_key: strategyKey,
      plan_key: planKey,
      snapshot_id: typeof snapshotId === "string" ? snapshotId : null,
      emitted_at: emittedAt,
      kind,
      payload: raw.payload,
      ...(progress ? { progress } : {}),
    },
  };
}

/**
 * Loose acceptance of the producer's progress group: a count that is not a
 * finite non-negative integer is DROPPED (not coerced to 0 — R10), so a
 * malformed metric degrades to "absent with reason" instead of a false zero.
 */
export function parsePublisherProgress(raw: unknown): StreamPublisherProgress | null {
  if (!isPlainObject(raw)) return null;
  const out: StreamPublisherProgress = {};
  const count = (v: unknown): number | undefined =>
    typeof v === "number" && Number.isInteger(v) && v >= 0 ? v : undefined;
  const routes = count(raw.routes_explored);
  const sizes = count(raw.sizes_quoted);
  const exact = count(raw.quotes_exact);
  const partial = count(raw.quotes_incomplete);
  const error = count(raw.quotes_error);
  if (routes !== undefined) out.routes_explored = routes;
  if (sizes !== undefined) out.sizes_quoted = sizes;
  if (exact !== undefined) out.quotes_exact = exact;
  if (partial !== undefined) out.quotes_incomplete = partial;
  if (error !== undefined) out.quotes_error = error;
  if (typeof raw.budget_exhausted === "boolean") out.budget_exhausted = raw.budget_exhausted;
  if (typeof raw.budget_reason === "string" && raw.budget_reason.length > 0) {
    out.budget_reason = raw.budget_reason;
  }
  if (typeof raw.snapshot_id === "string") out.snapshot_id = raw.snapshot_id;
  return Object.keys(out).length > 0 ? out : null;
}

export type SeqVerdict = "accept" | "duplicate" | "stale_rejected" | "first";

export interface SeqState {
  seq: number | null;
  event_id: string | null;
}

/**
 * Idempotency + ordering for the ENVELOPE path (producer `seq`).
 *   · no prior state                 → `first`
 *   · same event_id                  → `duplicate` (replay of the SAME event)
 *   · seq <= last seen               → `stale_rejected` (older OR re-ordered)
 *   · seq >  last seen               → `accept`
 * A gap (`next > last + 1`) is NOT an error here — events for other plans of
 * the same strategy share the counter, and the caller's resync (REST snapshot)
 * closes any hole. Returning `accept` keeps the stream honest about what it
 * has, instead of stalling on a lost event.
 */
export function sequenceVerdict(prev: SeqState, next: SeqState): SeqVerdict {
  if (prev.seq === null) return "first";
  if (next.event_id !== null && prev.event_id !== null && next.event_id === prev.event_id) {
    return "duplicate";
  }
  if (next.seq === null) return "stale_rejected";
  if (next.seq <= prev.seq) return "stale_rejected";
  return "accept";
}

// =============================================================================
// Ledger — the visible diagnostic of what the stream actually did
// =============================================================================

/**
 * Rolling counters that make §11.3's "publica progreso real" and §11.4's
 * "idempotencia" OBSERVABLE instead of asserted: how many applications landed,
 * how many were dropped as stale, how many were replays. Every counter starts
 * at 0 because 0 is its true computed value (nothing observed yet) — this is
 * NOT the `None ≠ Some(0)` case: these are measurements of the stream itself,
 * and the state that would be a lie ("absent" vs "zero") is carried separately
 * by `SearchProgressCell` in `search-state.ts`.
 */
export interface StreamLedgerCounters {
  accepted: number;
  duplicates: number;
  stale_rejected: number;
  incomparable: number;
}

export const EMPTY_LEDGER_COUNTERS: Readonly<StreamLedgerCounters> = {
  accepted: 0,
  duplicates: 0,
  stale_rejected: 0,
  incomparable: 0,
};

export function countOutcome(
  counters: StreamLedgerCounters,
  outcome: UpsertOutcome,
): StreamLedgerCounters {
  switch (outcome) {
    case "accepted_new":
    case "accepted_update":
      return { ...counters, accepted: counters.accepted + 1 };
    case "accepted_incomparable":
      return {
        ...counters,
        accepted: counters.accepted + 1,
        incomparable: counters.incomparable + 1,
      };
    case "duplicate":
      return { ...counters, duplicates: counters.duplicates + 1 };
    case "stale_rejected":
      return { ...counters, stale_rejected: counters.stale_rejected + 1 };
  }
}
