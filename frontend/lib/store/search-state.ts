// frontend/lib/store/search-state.ts
//
// SEARCH-STATE-01 (§11.2, §11.3, §11.5 of PROMPT_ARBITRAGEX_BUSQUEDA_AUTONOMA_FEES_264)
// =============================================================================
// The VISIBLE search state of one strategy, derived ONLY from figures the wire
// actually carries. Pure: no React, no store, no clock of its own (the caller
// passes `nowMs`), so every rule below is unit-testable and the card cannot
// invent a number the reducer did not see.
//
// §11.2 — what the card must show: strategy + search state, current route,
// best route in force, size, financing, gross, itemised costs, net, target and
// gap. All of it lives on `PlanFacts`.
//
// §11.3 — publish REAL progress: routes explored, sizes quoted, exact /
// incomplete quotes, budget exhausted, snapshot, age, last improvement and the
// rejection cause. Every one of those is a `ProgressCell` carrying either a
// COMPUTED value WITH its observation scope, or an ABSENT state WITH its
// machine reason. E2E-COMPUTE GUARD (R10) extended to the card: a metric with
// no producer is ABSENT — never a zero dressed as a measurement.
//
// §11.5 — keep the best NEGATIVE result while alternatives are searched, and
// keep the diagnostics visible when nothing is profitable. `best_negative` +
// `rejection_reason` + `diagnostics` are first-class fields, not fallbacks.
//
// §11.6 (and the prompt's §11 color rule) — the SIGN of a number decides its
// colour; no number is painted to promise a later state. `gap` is a DISTANCE
// and is therefore deliberately sign-neutral (see `GAP_TONE`'s consumers): a
// negative net stays red, and the gap never turns green to imply that the
// target was reached.
//
// NOTHING here recomputes economics. The gross / net / cost ladder comes from
// `buildLedger` — the same SSOT the card's ladder and the high-value notifier
// consult — so this panel can never disagree with the card about which closed
// arithmetic may be painted (CARDS-NOTIONAL-01 intact: one plan = one size).

import { buildLedger, type LedgerBasis, type LedgerView } from "../opportunity-ledger";
import type { OmniOpportunity, OpportunityStatus } from "./types";
import {
  countOutcome,
  declaredPlanSizeWei,
  observationClockMs,
  planKeyOf,
  strategyKeyOf,
  type StreamLedgerCounters,
  type StreamPublisherProgress,
  type UpsertOutcome,
} from "./stream-contract";

// =============================================================================
// Progress cell — the three states of a metric
// =============================================================================

/**
 * A metric is either COMPUTED (a real measurement, always carrying the SCOPE of
 * that measurement so a client-side census can never be mistaken for a
 * producer-side counter) or ABSENT (with the machine reason it is absent).
 * A third shape — "computed and exactly zero" — is not a separate variant:
 * `value: 0` IS that state, and it is only ever produced from a real census.
 */
export type ProgressCell =
  | { state: "COMPUTED"; value: number; scope: ObservationScope; note?: string }
  | { state: "ABSENT"; reason: string };

/**
 * Where a COMPUTED number was observed. Naming the scope on the value itself is
 * what keeps a client census from being read as a server counter (the confusion
 * the E2E-COMPUTE GUARD exists to prevent).
 */
export type ObservationScope =
  /** Counted by this client from the plans it has actually observed. */
  | "client_observed_inventory"
  /** Declared by the api-server producer over its own session window. */
  | "server_session";

/** Machine reasons for absence. Stable strings — rendered verbatim in titles. */
export const ABSENT_REASONS = {
  /** No producer exists for this metric on the opportunities wire (R10). */
  noProducer: "no_producer_on_opportunities_wire",
  /** The row carries no `economics` object at all — nothing was computed. */
  economicsNotEmitted: "economics_object_not_emitted",
  /** The producer ran but this particular figure is null, with its own reason. */
  notComputedByProducer: "not_computed_by_producer",
  /** No producer declared the notional these figures belong to. */
  noDeclaredNotional: "row_carries_no_declared_notional",
  /** No observation has arrived for this strategy yet. */
  noObservations: "no_observations_observed",
  /** A snapshot envelope has not been received on this connection yet. */
  noSnapshot: "no_snapshot_received",
  /** The producer did not configure a target for this chain. */
  targetNotConfigured: "target_not_configured",
  /** A net exists but the target does not, or vice versa: no gap is computable. */
  gapNeedsBoth: "gap_requires_computed_target_and_net",
  /** An ELAPSED-time metric with no clock to measure it against. */
  noClock: "no_caller_clock",
} as const;

export function computedCell(
  value: number,
  scope: ObservationScope,
  note?: string,
): ProgressCell {
  return { state: "COMPUTED", value, scope, ...(note ? { note } : {}) };
}

export function absentCell(reason: string): ProgressCell {
  return { state: "ABSENT", reason };
}

/** The number, or null when the cell is ABSENT. Never coerces ABSENT to 0. */
export function progressNumber(cell: ProgressCell): number | null {
  return cell.state === "COMPUTED" ? cell.value : null;
}

// =============================================================================
// PlanFacts — one plan, one arithmetic
// =============================================================================

export interface PlanCostRow {
  label: string;
  /** The component's USD value, or null when this row has no figure. */
  value: number | null;
  /** Why the value is null (R8) — never rendered as a silent dash. */
  reason: string | null;
}

export interface PlanFacts {
  /** Stable identity (stream-contract) — the same three keys the store uses. */
  plan_key: string;
  strategy_key: string;
  opportunity_id: string;
  /** Exact wei this plan's figures were declared at; null = undeclared. */
  declared_size_wei: string | null;
  /** The declared notional priced in USD, or null when it cannot be priced. */
  declared_notional_usd: number | null;
  /** Which producer's closed arithmetic this plan may paint (SSOT: buildLedger). */
  basis: LedgerBasis;
  /** Why the ladder went quiet, verbatim (null when nothing was suppressed). */
  basis_reason: string | null;

  status: OpportunityStatus | null;
  rejection_reason: string | null;
  computation_status: "computed" | "partial" | "error" | null;
  computation_error_reason: string | null;
  /** Producer-declared per-field absence reasons (`not_computed_reasons`). */
  not_computed_reasons: Record<string, string>;

  gross: ProgressCell;
  total_cost: ProgressCell;
  net: ProgressCell;
  principal: ProgressCell;
  financing_fee: ProgressCell;
  costs: PlanCostRow[];

  target: ProgressCell;
  /** Distance to the target: `max(0, target − net)`. Sign-neutral by design. */
  gap: ProgressCell;
  /** The producer's own predicate. null = not computed (never inferred). */
  meets_target: boolean | null;

  /** Hops of the persisted topology (0 when the plan carries no legs). */
  hops: number;
  legs_source: "route_metadata" | "none";
  clock_ms: number | null;
  age_seconds: number | null;
}

/** Cost-row labels the ladder can paint, in the operator's reading order. */
const COST_LABEL_ORDER = [
  "Gas",
  "LP fees",
  "Decoherence (slippage)",
  "TLS fee (flash)",
  "Relay fee",
  "Capital cost",
  "Failure buffer",
  "Copied buffer",
  "Ops overhead",
];

function finiteOrNull(v: number | null | undefined): number | null {
  return typeof v === "number" && Number.isFinite(v) ? v : null;
}

/**
 * Project one row onto its plan facts. `nowMs` is supplied by the caller; when
 * it is null the age stays null (R8: an undated row has no age, not age 0).
 */
export function planFactsOf(opp: OmniOpportunity, nowMs: number | null = null): PlanFacts {
  const ledger: LedgerView = buildLedger(opp);
  const e = opp.economics;
  const reasons = e?.not_computed_reasons ?? {};

  // ── Gross / costs / net / principal — the ladder's own closed arithmetic ───
  const gross = finiteOrNull(ledger.gross_usd);
  const totalCost = finiteOrNull(ledger.total_cost_usd);
  const net = finiteOrNull(ledger.net_usd);
  const principal = finiteOrNull(ledger.principal_usd);

  const basisNote = ledger.basis === "none" ? (ledger.reason ?? undefined) : undefined;

  const costValueByLabel = new Map(ledger.cost_rows.map((r) => [r.label, r.value]));
  // On the canonical basis the searcher's own decomposition fills the cells —
  // the SAME arithmetic the ladder paints on that basis (mirrors the card).
  if (ledger.basis === "canonical" && e && e.computation_status !== "error") {
    const fill: Array<[string, number | null]> = [
      ["Gas", finiteOrNull(e.gas_usd)],
      ["LP fees", finiteOrNull(e.dex_fees_usd)],
      ["Decoherence (slippage)", finiteOrNull(e.slippage_usd)],
      ["TLS fee (flash)", finiteOrNull(e.flash_fee_usd)],
      ["Relay fee", finiteOrNull(e.bribe_usd)],
      ["Ops overhead", finiteOrNull(e.other_costs_usd)],
    ];
    for (const [label, v] of fill) if (v != null) costValueByLabel.set(label, v);
  }
  const costs: PlanCostRow[] = COST_LABEL_ORDER.map((label) => {
    const v = finiteOrNull(costValueByLabel.get(label));
    if (v != null) return { label, value: v, reason: null };
    return {
      label,
      value: null,
      // The producer explains its own nulls; when it says nothing we say the
      // honest thing instead of guessing "zero".
      reason:
        label === "TLS fee (flash)"
          ? reasons["flash_fee_usd"] ?? ABSENT_REASONS.notComputedByProducer
          : ABSENT_REASONS.notComputedByProducer,
    };
  });

  const grossCell: ProgressCell =
    gross != null
      ? computedCell(gross, "server_session", ledger.basis)
      : absentCell(
          e == null
            ? ABSENT_REASONS.economicsNotEmitted
            : ledger.reason ?? ABSENT_REASONS.notComputedByProducer,
        );
  const netCell: ProgressCell =
    net != null
      ? computedCell(net, "server_session", ledger.basis)
      : absentCell(
          e == null
            ? ABSENT_REASONS.economicsNotEmitted
            : ledger.reason ?? ABSENT_REASONS.notComputedByProducer,
        );
  const costCell: ProgressCell =
    totalCost != null
      ? computedCell(totalCost, "server_session", ledger.basis)
      : absentCell(
          e == null
            ? ABSENT_REASONS.economicsNotEmitted
            : ledger.reason ?? ABSENT_REASONS.notComputedByProducer,
        );
  const principalCell: ProgressCell =
    principal != null
      ? computedCell(principal, "server_session", ledger.basis)
      : absentCell(
          ledger.notional == null
            ? ABSENT_REASONS.noDeclaredNotional
            : ledger.reason ?? ABSENT_REASONS.notComputedByProducer,
        );

  // ── Financing — the fee the plan pays for its capital, on the plan's basis ─
  const financingFee =
    ledger.basis === "canonical"
      ? finiteOrNull(e?.flash_fee_usd)
      : finiteOrNull(opp.simulated_cost_breakdown?.flashloan_fee_usd);
  const financingCell: ProgressCell =
    financingFee != null
      ? computedCell(financingFee, "server_session", ledger.basis)
      : absentCell(
          // A NULL with a producer reason is the producer's own declaration —
          // preserve it verbatim rather than flattening it to "absent".
          reasons["flash_fee_usd"] ?? ABSENT_REASONS.noProducer,
        );

  // ── Target and gap ───────────────────────────────────────────────────────
  const target = finiteOrNull(e?.target_net_usd);
  const targetCell: ProgressCell =
    target != null
      ? computedCell(target, "server_session", "economics.target_net_usd")
      : absentCell(reasons["target_net_usd"] ?? ABSENT_REASONS.targetNotConfigured);

  // The gap is a DISTANCE to the objective, so it is derived from the two
  // figures that must BOTH be computed — never from one of them alone, and
  // never from the producer's signed `target_delta_usd` (which is `net −
  // target`, i.e. the same relation with the opposite sign; deriving from it
  // would double-count the sign convention).
  const gapCell: ProgressCell =
    target != null && net != null
      ? computedCell(Math.max(0, target - net), "server_session", "max(0, target - net)")
      : absentCell(
          target == null
            ? reasons["target_net_usd"] ?? ABSENT_REASONS.targetNotConfigured
            : ABSENT_REASONS.gapNeedsBoth,
        );

  const clockMs = observationClockMs(opp);
  const rm = opp.route_metadata;
  const hops = rm?.dex_adapters?.length ?? 0;

  return {
    plan_key: planKeyOf(opp),
    strategy_key: strategyKeyOf(opp),
    opportunity_id: opp.id,
    declared_size_wei: declaredPlanSizeWei(opp),
    declared_notional_usd: finiteOrNull(ledger.notional?.usd),
    basis: ledger.basis,
    basis_reason: ledger.reason,
    status: opp.status,
    rejection_reason: opp.rejection_reason,
    computation_status: e?.computation_status ?? null,
    computation_error_reason: e?.error_reason ?? null,
    not_computed_reasons: reasons,
    gross: grossCell,
    total_cost: costCell,
    net: netCell,
    principal: principalCell,
    financing_fee: financingCell,
    costs,
    target: targetCell,
    gap: gapCell,
    meets_target: typeof e?.meets_target === "boolean" ? e.meets_target : null,
    hops,
    legs_source: rm == null ? "none" : "route_metadata",
    clock_ms: clockMs,
    age_seconds:
      clockMs == null || nowMs == null ? null : Math.max(0, Math.floor((nowMs - clockMs) / 1000)),
  };
}

// =============================================================================
// Best-plan selection — positive, real zero and negative are three states
// =============================================================================

export interface BestPlans {
  /** Best plan with net > 0 (maximised). */
  best_positive: PlanFacts | null;
  /** Best plan with net EXACTLY 0 — a computed zero, not an absence. */
  best_zero: PlanFacts | null;
  /**
   * Best plan with net < 0, i.e. the loss CLOSEST to zero (§11.5: keep the best
   * negative result while alternatives are searched). Not "the worst one".
   */
  best_negative: PlanFacts | null;
}

/** Net of a plan, or null when ABSENT (never coerced to 0). */
export function planNet(plan: PlanFacts): number | null {
  return progressNumber(plan.net);
}

export function selectBestPlans(plans: readonly PlanFacts[]): BestPlans {
  let best_positive: PlanFacts | null = null;
  let best_zero: PlanFacts | null = null;
  let best_negative: PlanFacts | null = null;
  for (const p of plans) {
    const n = planNet(p);
    if (n == null) continue;
    if (n > 0) {
      if (best_positive == null || n > (planNet(best_positive) as number)) best_positive = p;
    } else if (n === 0) {
      if (best_zero == null) best_zero = p;
    } else {
      if (best_negative == null || n > (planNet(best_negative) as number)) best_negative = p;
    }
  }
  return { best_positive, best_zero, best_negative };
}

// =============================================================================
// Progress — real numbers with their scope, or ABSENT with the reason
// =============================================================================

/**
 * Progress a PRODUCER declared for this strategy (the api-server envelope).
 * Re-exported from the stream contract so the wire shape has exactly one
 * definition: a producer that cannot measure something omits it, and the card
 * then reports ABSENT instead of substituting a client census silently.
 */
export type PublisherProgress = StreamPublisherProgress;

export interface StrategyProgress {
  routes_explored: ProgressCell;
  sizes_quoted: ProgressCell;
  quotes_exact: ProgressCell;
  quotes_incomplete: ProgressCell;
  quotes_error: ProgressCell;
  /** §11.3 "presupuesto agotado" — ABSENT until a producer declares it. */
  budget: ProgressCell;
  /** §11.3 "última mejora" — the clock of the last accepted improvement. */
  last_improvement: ProgressCell;
  /** §11.3 snapshot + antigüedad. */
  snapshot_age_ms: ProgressCell;
}

// =============================================================================
// Strategy search state
// =============================================================================

export type SearchPhase =
  /** No observation of this strategy has arrived yet. */
  | "no_observations"
  /** Fresh observations exist and none reaches the target. */
  | "searching"
  /** A plan's producer-declared predicate says the target is met. */
  | "target_met"
  /** Observations exist but every one is older than the vigency window. */
  | "search_stalled";

/**
 * Lifecycle stages §11.2 requires the card to DISTINGUISH instead of implying.
 * Each stage is REACHED only when the wire says so, and each names the field
 * that said it — the card never promotes a positive number into "approved", nor
 * an approved simulation into "submitted".
 */
export interface LifecycleStage {
  key: "number_positive" | "target_met" | "sim_approved" | "submitted" | "included" | "settled" | "reconciled";
  label: string;
  reached: boolean;
  /** The wire field this verdict came from. Always present, even when not reached. */
  source: string;
}

/** Statuses that mean the canonical lifecycle has moved past simulation. */
const SIM_APPROVED_STATUSES: ReadonlySet<OpportunityStatus> = new Set<OpportunityStatus>([
  "simulated",
  "scored",
  "executing",
  "executed",
  "reconciled",
]);

/**
 * The canonical lifecycle ORDER, as the schema itself defines it
 * (`OpportunityStatus`). A stage is REACHED either because the wire declares it
 * exactly, or because it declares a LATER stage of the same total order — and
 * the `source` string says which of the two happened, so an implied stage is
 * never presented as an observed one. `rejected`/`failed` sit OUTSIDE the
 * ladder (index -1): a rejected row claims no stage.
 */
const STATUS_ORDER: Partial<Record<OpportunityStatus, number>> = {
  detected: 0,
  validated: 1,
  simulated: 2,
  scored: 3,
  executing: 4,
  executed: 5,
  reconciled: 6,
};

function stageSource(observed: OpportunityStatus | null, stage: OpportunityStatus): string {
  const at = observed == null ? -1 : STATUS_ORDER[observed] ?? -1;
  const need = STATUS_ORDER[stage] ?? Number.POSITIVE_INFINITY;
  // NOTE on wording: this string ends up in a `title` attribute, and the card's
  // own suites count occurrences of the literal "ciclo " to pin the per-hop
  // cycle-Δ cell ("exactly once"). A tooltip here must therefore never contain
  // that substring, or an unrelated assertion on the card's ledger starts
  // counting this panel's chips. Hence "orden canónico".
  if (at < 0) return `status=${String(observed)} (fuera del orden canónico)`;
  return at === need ? `status=${observed}` : `status=${observed} ≥ ${stage}`;
}

export function lifecycleOf(plan: PlanFacts | null): LifecycleStage[] {
  const n = plan ? planNet(plan) : null;
  const status = plan?.status ?? null;
  const at = status == null ? -1 : STATUS_ORDER[status] ?? -1;
  const reached = (stage: OpportunityStatus): boolean => {
    const need = STATUS_ORDER[stage];
    return need != null && at >= need;
  };
  const simApproved =
    (status != null && SIM_APPROVED_STATUSES.has(status)) ||
    plan?.computation_status === "computed";
  return [
    {
      key: "number_positive",
      label: "net > 0",
      reached: n != null && n > 0,
      source: plan?.net.state === "COMPUTED" ? `net (${plan.net.note ?? "ladder"})` : "net",
    },
    {
      key: "target_met",
      label: "objetivo",
      reached: plan?.meets_target === true,
      source: "economics.meets_target",
    },
    {
      key: "sim_approved",
      label: "sim aprobada",
      reached: simApproved,
      source:
        plan?.computation_status === "computed"
          ? "economics.computation_status=computed"
          : "status",
    },
    {
      key: "submitted",
      label: "envío",
      reached: reached("executing"),
      source: stageSource(status, "executing"),
    },
    {
      key: "included",
      label: "inclusión",
      reached: reached("executed"),
      source: stageSource(status, "executed"),
    },
    {
      key: "settled",
      label: "liquidación",
      reached: reached("executed"),
      source: stageSource(status, "executed"),
    },
    {
      key: "reconciled",
      label: "conciliación",
      reached: reached("reconciled"),
      source: stageSource(status, "reconciled"),
    },
  ];
}

export interface StrategySearchState {
  strategy_key: string;
  phase: SearchPhase;
  phase_reason: string;
  /** The most recent observation — the route being searched now. */
  current: PlanFacts | null;
  /** Every observed plan, newest first. NEVER truncated here (§11.5). */
  plans: PlanFacts[];
  /** Total observed inventory, so a display cap can declare what it hid. */
  inventory_size: number;
  best: BestPlans;
  lifecycle: LifecycleStage[];
  progress: StrategyProgress;
  /** Stream-level diagnostics: what the sequencing contract applied or dropped. */
  counters: StreamLedgerCounters;
  /** Non-null while the strategy has at least one plan with a rejection cause. */
  diagnostics: Array<{ plan_key: string; reason: string }>;
  last_improvement_ms: number | null;
}

export interface DeriveOptions {
  /** Producer-declared progress (absent ⇒ every producer-only metric is ABSENT). */
  publisher?: PublisherProgress | null;
  /** Sequencing counters for this strategy (stream-contract). */
  counters?: StreamLedgerCounters;
  /** Clock (ms) of the last accepted improvement, or null if none observed. */
  lastImprovementMs?: number | null;
  /** ms epoch at which the last snapshot envelope was received, or null. */
  snapshotReceivedAtMs?: number | null;
  /** Freshness budget; a plan older than this is not "searching" evidence. */
  vigencyMs?: number;
  /**
   * Caller clock (ms epoch). OPTIONAL: omitted → ages stay ABSENT (R8), which
   * is the correct state for a server render with no clock. Callers that have a
   * clock (the mounted card) pass it; a caller that does not must not be forced
   * to invent one to satisfy the type.
   */
  nowMs?: number | null;
}

export const DEFAULT_VIGENCY_MS = 5 * 60_000;

/**
 * Fold an inventory of rows into the strategy's visible search state.
 *
 * The inventory is the STORE's list (already sequenced + deduplicated); this
 * function only classifies and counts it. It never drops a plan, so a display
 * cap cannot silently hide an active strategy (§11.5).
 */
export function deriveStrategySearchState(
  strategyKey: string,
  rows: readonly OmniOpportunity[],
  options: DeriveOptions = {},
): StrategySearchState {
  const {
    publisher = null,
    counters,
    lastImprovementMs = null,
    snapshotReceivedAtMs = null,
    vigencyMs = DEFAULT_VIGENCY_MS,
    nowMs = null,
  } = options;

  const plans = rows
    .map((r) => planFactsOf(r, nowMs))
    .sort((a, b) => (b.clock_ms ?? -Infinity) - (a.clock_ms ?? -Infinity));

  const current = plans[0] ?? null;
  const best = selectBestPlans(plans);

  // ── Phase — from the observations themselves, never from a promise ────────
  let phase: SearchPhase;
  let phaseReason: string;
  if (plans.length === 0) {
    phase = "no_observations";
    phaseReason = ABSENT_REASONS.noObservations;
  } else if (plans.some((p) => p.meets_target === true)) {
    phase = "target_met";
    phaseReason = "economics.meets_target=true";
  } else if (
    nowMs != null &&
    plans.every((p) => p.clock_ms != null && nowMs - p.clock_ms > vigencyMs)
  ) {
    phase = "search_stalled";
    phaseReason = `every observed plan is older than ${vigencyMs} ms`;
  } else {
    phase = "searching";
    phaseReason =
      best.best_positive != null
        ? "positive net observed, target not met"
        : "no positive net observed yet — best negative retained";
  }

  // ── Progress cells ───────────────────────────────────────────────────────
  const routesObserved = new Set(plans.map((p) => p.plan_key.split("#")[1] ?? p.plan_key)).size;
  const sizesObserved = new Set(
    plans.map((p) => p.declared_size_wei).filter((s): s is string => s != null),
  ).size;
  const withEconomics = plans.filter((p) => p.computation_status != null).length;

  const routesExplored: ProgressCell =
    publisher?.routes_explored != null
      ? computedCell(publisher.routes_explored, "server_session")
      : plans.length === 0
        ? absentCell(ABSENT_REASONS.noObservations)
        : computedCell(routesObserved, "client_observed_inventory", "distinct plan routes seen");

  const sizesQuoted: ProgressCell =
    publisher?.sizes_quoted != null
      ? computedCell(publisher.sizes_quoted, "server_session")
      : sizesObserved === 0
        ? absentCell(ABSENT_REASONS.noDeclaredNotional)
        : computedCell(sizesObserved, "client_observed_inventory", "distinct declared notions");

  const quotesCell = (
    producerValue: number | undefined,
    status: "computed" | "partial" | "error",
  ): ProgressCell => {
    if (producerValue != null) return computedCell(producerValue, "server_session");
    // A census of zero is only honest when the wire actually carries the
    // object being counted; with no `economics` anywhere there is nothing to
    // classify and the metric is ABSENT (R10: absence is never a zero).
    if (withEconomics === 0) return absentCell(ABSENT_REASONS.economicsNotEmitted);
    return computedCell(plans.filter((p) => p.computation_status === status).length, "client_observed_inventory");
  };

  const budget: ProgressCell =
    publisher?.budget_exhausted != null
      ? computedCell(
          publisher.budget_exhausted ? 1 : 0,
          "server_session",
          publisher.budget_reason ?? undefined,
        )
      : absentCell(ABSENT_REASONS.noProducer);

  // ── Elapsed-time metrics need a CLOCK, and the clock is supplied by the
  //    caller. With no clock these are ABSENT — never 0. Painting a 0 age would
  //    claim "received just now" for a snapshot that may be minutes old, which
  //    is the false-zero failure this whole module is built to avoid (the
  //    server render is exactly the no-clock case: the card's first paint must
  //    not claim a freshness it has not measured).
  const lastImprovement: ProgressCell =
    lastImprovementMs == null
      ? absentCell(
          publisher == null ? ABSENT_REASONS.noObservations : ABSENT_REASONS.noProducer,
        )
      : nowMs == null
        ? absentCell(ABSENT_REASONS.noClock)
        : computedCell(
            Math.max(0, nowMs - lastImprovementMs),
            "client_observed_inventory",
            "ms since the last accepted improvement",
          );

  const snapshotAge: ProgressCell =
    snapshotReceivedAtMs == null
      ? absentCell(ABSENT_REASONS.noSnapshot)
      : nowMs == null
        ? absentCell(ABSENT_REASONS.noClock)
        : computedCell(
            Math.max(0, nowMs - snapshotReceivedAtMs),
            "client_observed_inventory",
            "ms since the last snapshot envelope",
          );

  const diagnostics = plans
    .filter((p) => p.rejection_reason != null || p.computation_error_reason != null)
    .map((p) => ({
      plan_key: p.plan_key,
      reason: p.rejection_reason ?? p.computation_error_reason ?? "unknown",
    }));

  return {
    strategy_key: strategyKey,
    phase,
    phase_reason: phaseReason,
    current,
    plans,
    inventory_size: plans.length,
    best,
    lifecycle: lifecycleOf(current),
    progress: {
      routes_explored: routesExplored,
      sizes_quoted: sizesQuoted,
      quotes_exact: quotesCell(publisher?.quotes_exact, "computed"),
      quotes_incomplete: quotesCell(publisher?.quotes_incomplete, "partial"),
      quotes_error: quotesCell(publisher?.quotes_error, "error"),
      budget,
      last_improvement: lastImprovement,
      snapshot_age_ms: snapshotAge,
    },
    counters:
      counters ?? { accepted: 0, duplicates: 0, stale_rejected: 0, incomparable: 0 },
    diagnostics,
    last_improvement_ms: lastImprovementMs,
  };
}

// =============================================================================
// Improvement tracking — §11.3 "última mejora", observed, never animated
// =============================================================================

export interface ImprovementState {
  bestNetSeen: number | null;
  bestNetPlanKey: string | null;
  lastImprovementMs: number | null;
}

export const EMPTY_IMPROVEMENT_STATE: Readonly<ImprovementState> = {
  bestNetSeen: null,
  bestNetPlanKey: null,
  lastImprovementMs: null,
};

/**
 * Fold one accepted observation into the improvement tracker.
 *
 * §11.3 requires the card's progress to come from EFFECTIVE SAMPLES and not
 * from animation or artificial alternation: an improvement is recorded only
 * when a plan's net is STRICTLY greater than the best net ever seen on this
 * strategy, and the stamp is that observation's own clock — never the wall
 * clock at render time.
 */
export function trackImprovement(
  state: ImprovementState,
  plan: PlanFacts,
): ImprovementState {
  const n = planNet(plan);
  if (n == null) return state;
  if (state.bestNetSeen != null && n <= state.bestNetSeen) return state;
  return {
    bestNetSeen: n,
    bestNetPlanKey: plan.plan_key,
    lastImprovementMs: plan.clock_ms,
  };
}

// =============================================================================
// Display cap — §11.5 (never a silent limit)
// =============================================================================

export interface CappedInventory<T> {
  shown: T[];
  hidden: number;
  total: number;
}

/**
 * Apply a display cap WITHOUT losing the inventory: the caller always receives
 * the hidden count so the UI can state "showing X of Y" instead of pretending
 * the list ended (the silent frontend limit §11.5 forbids).
 */
export function capInventory<T>(items: readonly T[], cap: number): CappedInventory<T> {
  if (!Number.isInteger(cap) || cap < 0) {
    return { shown: [...items], hidden: 0, total: items.length };
  }
  const shown = items.slice(0, cap);
  return { shown, hidden: items.length - shown.length, total: items.length };
}

// =============================================================================
// Stream ledger — what the sequencing contract actually did, per strategy
// =============================================================================

export interface StrategyStreamEntry {
  counters: StreamLedgerCounters;
  improvement: ImprovementState;
  /** Clock of the last ACCEPTED observation for this strategy (ms, or null). */
  lastAcceptedMs: number | null;
  /** The producer's own progress declaration, when the envelope carried one. */
  publisher: PublisherProgress | null;
}

export type StreamLedger = ReadonlyMap<string, StrategyStreamEntry>;

export const EMPTY_STRATEGY_ENTRY: Readonly<StrategyStreamEntry> = {
  counters: { accepted: 0, duplicates: 0, stale_rejected: 0, incomparable: 0 },
  improvement: EMPTY_IMPROVEMENT_STATE,
  lastAcceptedMs: null,
  publisher: null,
};

export function ledgerEntryOf(
  ledger: StreamLedger,
  strategyKey: string,
): StrategyStreamEntry {
  return ledger.get(strategyKey) ?? { ...EMPTY_STRATEGY_ENTRY };
}

/**
 * Fold one sequencing outcome into the ledger.
 *
 * A REJECTED event still moves the ledger (the rejection is a fact about the
 * stream and must be visible — §11.4), but it never touches `lastAcceptedMs`
 * nor the improvement tracker: a stale event cannot claim to be progress.
 * Returns a NEW map so a Zustand store update stays a single immutable write.
 */
export function recordObservation(
  ledger: StreamLedger,
  opp: OmniOpportunity,
  outcome: UpsertOutcome,
  publisher: PublisherProgress | null = null,
): StreamLedger {
  const key = strategyKeyOf(opp);
  const prev = ledgerEntryOf(ledger, key);
  const counters = countOutcome(prev.counters, outcome);
  const applied = outcome !== "duplicate" && outcome !== "stale_rejected";
  const next: StrategyStreamEntry = {
    counters,
    improvement: applied
      ? trackImprovement(prev.improvement, planFactsOf(opp, null))
      : prev.improvement,
    lastAcceptedMs: applied ? observationClockMs(opp) ?? prev.lastAcceptedMs : prev.lastAcceptedMs,
    publisher: publisher ?? prev.publisher,
  };
  const out = new Map(ledger);
  out.set(key, next);
  return out;
}
