/**
 * CARDS-NOTIONAL-01 (2026-09-26) — ONE ladder, ONE notional.
 *
 * `/opportunities` paints a "Capital path (USD)" ladder per card:
 *
 *     Flash loan in → gross out → repay → costs… → Total cost → Net yield
 *
 * Every row of that ladder is only meaningful if all of them belong to the
 * SAME notional, because `net == gross − total_cost` is an IDENTITY inside one
 * computation — it is not a relation between two computations.
 *
 * ── Why the wire cannot be trusted to provide one ───────────────────────────
 *
 * The payload carries up to THREE economists, each produced at its own size:
 *
 *   1. canonical GROSS `expected_profit_usd`
 *      Producer: the DEX engine's fast filter, computed at its own probe
 *      `10^decimals(token_in)` one native unit — `engines/dex_engine.rs:251`
 *      and `:293` — and recorded next to an `amount_in_wei` that is the
 *      DECODED intent amount (`dex_engine.rs:467`), or, when the decoder lost
 *      it (`"0"`), a probe stamped at the emit boundary
 *      (`opportunity_emitter.rs:828-843`).
 *
 *   2. canonical NET `net_expected_profit_usd`
 *      Producer: the sizing kernel, at the kernel's own clamped/optimal size
 *      (`size_optimizer.rs` `NonPositive { profit_usd }` /
 *      `NonPositiveGrossUsd`), stamped onto the row by the orchestrator's
 *      rejected arm (`orchestrator.rs:1069-1072`).
 *
 *   3. the SIM triple `simulated_gross_usd` + `simulated_cost_breakdown` /
 *      `simulated_costs_total_usd` + `simulated_net_profit_usd`
 *      Producer: ONE `forwardSimulate` call
 *      (`backend/api-server/src/simulation/computeSimulatedNet.ts`) at ONE
 *      notional — `simulated_amount_in_usd`, derived from `amount_in_wei`.
 *
 * Painting (1) beside (2), or (2) beside (3)'s costs, is the defect the
 * operator photographed on production build `89d68ec8`:
 *
 *     PEPE | IN $0.00 | GROSS $1.47M | NET -$0.00 | Repay $0.0000
 *          | Total cost $73.4k
 *     DAI  | IN $1.00 | GROSS $822215.98 | Total cost $41.1k
 *
 * `Total cost` there is ≈ 5 % of the GROSS (`relayFeeUsd`, basis 1) while
 * `Net yield` is the kernel's net (basis 2, ≈ −0.00001) and `IN` is basis 3's
 * probe — three notionals in one ladder, so `net == gross − total_cost` fails
 * by six orders of magnitude on the rendered row.
 *
 * ── The rule this module enforces ───────────────────────────────────────────
 *
 * Exactly ONE basis is rendered per ladder, and only when that basis is
 * self-consistent:
 *
 *   · `"simulated"` — the SIM triple, when its gross is ATTRIBUTABLE to its own
 *     principal and its cost is PAYABLE out of that principal + gross. Only
 *     this basis may render a principal (`Flash loan in` / `Repay`) and a
 *     per-component cost ladder, because it is the only one that HAS a
 *     notional on the wire.
 *   · `"canonical"` — the searcher's own (gross, net) pair, closed by
 *     construction as `total_cost := gross − net`. It carries NO principal
 *     (the wire publishes no notional for it), so the principal rows go quiet
 *     with the reason in their `title` instead of being filled from another
 *     basis' number.
 *   · `"none"` — nothing is closed; every ladder cell goes quiet.
 *
 * R8 (fail-honest): a suppressed cell is a DASH with the machine reason in its
 * `title`, never a loud wrong number and never a fabricated 0. The canonical
 * figures are still *displayed* — in the summary grid, marked with their own
 * basis — they simply stop masquerading as one capital path.
 *
 * This module is the SSOT for BOTH surfaces (card ladder + high-value toast)
 * because a guard implemented twice is a guard that will disagree with itself.
 */

import type { OmniOpportunity, SimulatedCostBreakdown } from "@/lib/store/types";

/** Same rounding as the display: half a cent (cells render 2 decimals). */
export const LEDGER_TOLERANCE_USD = 0.005;

/**
 * Twin of the SEARCHER'S OWN gross-vs-principal sanity gate — not a new
 * threshold invented on the display layer.
 *
 *   `backend/searcher-rs/src/engines/triangular_engine.rs:69`
 *       const SANITY_PROFIT_MULT_OF_CAP: f64 = 5.0;
 *   `backend/searcher-rs/src/workers/triangular_worker.rs:1533` and `:1971`
 *       (same 5×, with the `profit_cap_ratio > SANITY_PROFIT_MULT_OF_CAP`
 *       rejection and its `threshold` log field)
 *   `backend/searcher-rs/src/counters.rs:149` documents the shared bound.
 *
 * The searcher already refuses to call a cycle real when its gross exceeds 5×
 * the capital it was sized against. A row that reaches the dashboard with
 * `gross > 5 × principal` therefore has a gross computed at a DIFFERENT
 * notional than the principal printed beside it — measured on the live feed:
 * `gross $822,215.98` against `principal $1.00` (ratio 822,216×), and
 * `gross $1.47M` against a PEPE probe worth `$0.0000045`.
 */
export const GROSS_OVER_PRINCIPAL_SANITY_MULT = 5.0;

export type LedgerBasis = "simulated" | "canonical" | "none";

export interface LedgerCostRow {
  label: string;
  value: number;
}

export interface LedgerView {
  /** Which producer's arithmetic this ladder shows. `"none"` ⇒ all cells quiet. */
  basis: LedgerBasis;
  /** Gross of the rendered basis — the ONLY gross that closes this ladder. */
  gross_usd: number | null;
  /** Σ of the cost rows of the rendered basis. */
  total_cost_usd: number | null;
  /** Net of the rendered basis. */
  net_usd: number | null;
  /**
   * Principal (the notional the basis was computed at), or null when the
   * rendered basis carries none. Null ⇒ the principal rows MUST go quiet.
   */
  principal_usd: number | null;
  /** Per-component cost rows. Non-empty only on the `"simulated"` basis. */
  cost_rows: LedgerCostRow[];
  /**
   * True when at least one wire figure the ladder used to paint was suppressed
   * because it belongs to another notional. `reason` carries the machine cause.
   */
  quiet: boolean;
  /** Machine reason, rendered verbatim in every suppressed cell's `title`. */
  reason: string | null;
}

const DASH_REASON_PREFIX = "CARDS-NOTIONAL-01";

/** Coerce a wire number, `null` for anything non-finite (R8: never NaN/Inf). */
function num(v: number | null | undefined): number | null {
  return typeof v === "number" && Number.isFinite(v) ? v : null;
}

function quiet(reason: string): LedgerView {
  return {
    basis: "none",
    gross_usd: null,
    total_cost_usd: null,
    net_usd: null,
    principal_usd: null,
    cost_rows: [],
    quiet: true,
    reason: `${DASH_REASON_PREFIX}: ${reason}`,
  };
}

/**
 * Is `grossUsd` attributable to `principalUsd`?
 *
 * True when the gross is within the searcher's own
 * `SANITY_PROFIT_MULT_OF_CAP` (5×) of the principal that would have produced it
 * — i.e. the two figures can be, and by the searcher's own doctrine must be,
 * two cells of one arithmetic. False means they were measured at different
 * sizes and MUST NOT be published side by side as if they were one notional.
 *
 * `null` gross is attributable by absence (nothing is being claimed).
 * `null`/non-positive principal is NOT attributable (no notional to attribute
 * to) — the caller decides whether that is a dash or an omission.
 */
export function grossIsAttributableToPrincipal(
  grossUsd: number | null | undefined,
  principalUsd: number | null | undefined,
): boolean {
  const gross = num(grossUsd);
  if (gross == null) return true;
  const principal = num(principalUsd);
  if (principal == null || principal <= 0) return false;
  return Math.abs(gross) <= principal * GROSS_OVER_PRINCIPAL_SANITY_MULT;
}

/**
 * The ladder's cost rows, in kernel order, as the single source of truth for
 * BOTH the label list and the sum.
 *
 * `"Copied buffer"` is last on purpose: the card used to omit it entirely, and
 * at the live `p_copied_max = 0.5` that is HALF the gross — the single largest
 * term in the `"Total cost"` the operator read.
 */
export const LEDGER_COST_ROW_LABELS = [
  "Gas",
  "LP fees",
  "Decoherence (slippage)",
  "TLS fee (flash)",
  "Relay fee",
  "Capital cost",
  "Failure buffer",
  "Ops overhead",
  "Copied buffer",
] as const;

export type LedgerCostRowLabel = (typeof LEDGER_COST_ROW_LABELS)[number];

const COST_FIELD_BY_LABEL: Record<
  LedgerCostRowLabel,
  keyof SimulatedCostBreakdown
> = {
  Gas: "gas_usd",
  "LP fees": "lp_fees_usd",
  "Decoherence (slippage)": "slippage_usd",
  "TLS fee (flash)": "flashloan_fee_usd",
  "Relay fee": "relay_fee_usd",
  "Capital cost": "capital_cost_usd",
  "Failure buffer": "failure_buffer_usd",
  "Ops overhead": "ops_overhead_usd",
  "Copied buffer": "copied_buffer_usd",
};

/**
 * Rebuild the SIM's per-component ladder from the wire breakdown, INCLUDING
 * `copied_buffer_usd` — the component the rendered "Total cost" used to drop.
 */
function simulatedCostRows(opp: OmniOpportunity): LedgerCostRow[] {
  const cb = opp.simulated_cost_breakdown;
  if (cb == null) return [];
  const rows: LedgerCostRow[] = [];
  for (const label of LEDGER_COST_ROW_LABELS) {
    const value = num(cb[COST_FIELD_BY_LABEL[label]]);
    if (value != null) rows.push({ label, value });
  }
  return rows;
}

/** Real cost rows emitted by the Rust searcher economics object.
 * DEX fees/slippage remain absent as separate numbers when the producer says
 * they are already embedded in amount_out; absence here is NOT rewritten to 0.
 */
function canonicalCostRows(opp: OmniOpportunity): LedgerCostRow[] {
  const e = opp.economics;
  if (e == null || e.computation_status !== "computed") return [];
  const rows: LedgerCostRow[] = [];
  const add = (label: LedgerCostRowLabel, value: number | null | undefined) => {
    const v = num(value);
    if (v != null) rows.push({ label, value: v });
  };
  add("Gas", e.gas_usd);
  add("LP fees", e.dex_fees_usd);
  add("Decoherence (slippage)", e.slippage_usd);
  add("TLS fee (flash)", e.flash_fee_usd);
  add("Relay fee", e.bribe_usd);
  add("Ops overhead", e.other_costs_usd);
  return rows;
}

/**
 * Decide, per row, which closed arithmetic the ladder may paint.
 *
 * Pure: reads only `opp`, performs no I/O and never mutates its input. Both the
 * card and the notifier call it, so the rendered ladder and the announced
 * figure can never disagree.
 */
export function buildLedger(opp: OmniOpportunity): LedgerView {
  const canonicalGross = num(opp.expected_profit_usd);
  const canonicalNet = num(opp.net_expected_profit_usd);
  const simGross = num(opp.simulated_gross_usd);
  const simNet = num(opp.simulated_net_profit_usd);
  const simCostsTotal = num(opp.simulated_costs_total_usd);
  const principal = num(opp.simulated_amount_in_usd);

  // ── Basis 1: Rust searcher economics (REAL/LIVE SSOT) ────────────────────
  // #711 now persists one computation object at the sized notional on both
  // PASS and rejected-but-computed paths. It is therefore the first and only
  // source allowed to define a REAL/LIVE trading card.
  const e = opp.economics;
  if (e?.computation_status === "computed") {
    const ePrincipal = num(e.amount_in_usd);
    const eGross = num(e.gross_profit_usd);
    const eTotal = num(e.total_cost_usd);
    const eNet = num(e.net_profit_usd);
    if (ePrincipal != null && eGross != null && eTotal != null && eNet != null) {
      const rows = canonicalCostRows(opp);
      const rowSum = rows.reduce((a, r) => a + r.value, 0);
      const closed = Math.abs(eNet - (eGross - eTotal)) <= LEDGER_TOLERANCE_USD;
      const componentsClose = Math.abs(rowSum - eTotal) <= LEDGER_TOLERANCE_USD;
      if (
        ePrincipal > 0 &&
        eTotal >= 0 &&
        closed &&
        componentsClose &&
        grossIsAttributableToPrincipal(eGross, ePrincipal)
      ) {
        return {
          basis: "canonical",
          gross_usd: eGross,
          total_cost_usd: eTotal,
          net_usd: eNet,
          principal_usd: ePrincipal,
          cost_rows: rows,
          quiet: false,
          reason: null,
        };
      }
    }
  }

  // ── Basis 2: legacy TS SIM triple ────────────────────────────────────────
  // Kept only for legacy/detail compatibility. isRealLiveEconomicCard() below
  // explicitly rejects this basis, so it can never qualify a live trading card.
  // Preferred because it is the only triple the wire PROVES to be one
  // computation at one notional, and the only one that carries that notional.
  let simReject: string | null = null;
  if (simGross != null && simNet != null && simCostsTotal != null) {
    if (simCostsTotal < 0) {
      simReject = "simulated total cost is negative — not a closed ladder";
    } else if (
      Math.abs(simNet - (simGross - simCostsTotal)) > LEDGER_TOLERANCE_USD
    ) {
      simReject = "simulated net != gross - total cost";
    } else if (principal == null || principal <= 0) {
      simReject = "simulated principal not computed (amount_in_wei unpriced)";
    } else if (!grossIsAttributableToPrincipal(simGross, principal)) {
      // The observed break: a gross thousands of times the principal printed
      // beside it cannot be that principal's return.
      simReject =
        `simulated gross (${simGross}) exceeds ${GROSS_OVER_PRINCIPAL_SANITY_MULT}x ` +
        `its own principal (${principal}) — gross and notional belong to different sizes`;
    } else if (simCostsTotal > principal + simGross) {
      simReject = "simulated total cost exceeds principal + gross — not payable";
    }
    if (simReject == null) {
      const cost_rows = simulatedCostRows(opp);
      // The rows must reconstruct the total, or the visible ladder is not the
      // closed one (a component missing from the breakdown object).
      const rowSum = cost_rows.reduce((a, r) => a + r.value, 0);
      if (Math.abs(rowSum - simCostsTotal) <= LEDGER_TOLERANCE_USD) {
        return {
          basis: "simulated",
          gross_usd: simGross,
          total_cost_usd: simCostsTotal,
          net_usd: simNet,
          principal_usd: principal,
          cost_rows,
          quiet: false,
          reason: null,
        };
      }
      simReject = "simulated cost components do not sum to the simulated total";
    }
  } else if (simGross != null || simNet != null || simCostsTotal != null) {
    simReject = "simulated ladder incomplete (gross/total/net not all on the wire)";
  }

  // ── Basis 2: the searcher's own pair, closed by construction ──────────────
  // `net_expected_profit_usd` is DOCUMENTED on the wire as `gross − costs`
  // (`opportunities-live.ts` row mapper), so `total_cost := gross − net` is the
  // wire's own relation — a derivation, not an invention. No principal is
  // rendered: the wire publishes no notional for this pair, and borrowing one
  // from another basis is the defect.
  if (canonicalGross != null && canonicalNet != null) {
    const total = canonicalGross - canonicalNet;
    if (total >= 0) {
      return {
        basis: "canonical",
        gross_usd: canonicalGross,
        total_cost_usd: total,
        net_usd: canonicalNet,
        principal_usd: null,
        cost_rows: [],
        quiet: false,
        reason:
          `${DASH_REASON_PREFIX} (published): searcher's own (gross, net) pair ` +
          `— principal not rendered (no notional published for it)` +
          (simReject != null ? `; simulated ladder suppressed: ${simReject}` : ""),
      };
    }
  }

  // ── OPERATOR ORDER 2026-09-27 (verbatim): "QUITA EL MALDITO RENDER QUE ESCONDE
  // LOS NUMEROS." ─────────────────────────────────────────────────────────────
  // This tail used to silence the whole capital path whenever no basis closed.
  // Nothing that exists on the wire may be withheld: publish every figure that
  // is present, labelled with the reason the ladder does not close. Only a row
  // with NO figure at all still goes quiet (R8: absence is a state).
  const anyFigure = [
    canonicalGross, canonicalNet, simGross, simNet, simCostsTotal, principal,
  ].some((v) => v != null);
  if (anyFigure) {
    const rows = simulatedCostRows(opp);
    const derivedTotal =
      simCostsTotal ??
      (canonicalGross != null && canonicalNet != null ? canonicalGross - canonicalNet : null);
    return {
      basis: simGross != null || simNet != null ? "simulated" : "canonical",
      gross_usd: canonicalGross ?? simGross,
      total_cost_usd: derivedTotal,
      net_usd: canonicalNet ?? simNet,
      principal_usd: principal,
      cost_rows: rows,
      quiet: false,
      reason:
        `${DASH_REASON_PREFIX} (published, ladder not closed): ` +
        `${simReject ?? "no closed triple on the wire"}`,
    };
  }
  return quiet(
    simReject ??
      "no closed (gross, net, cost) triple on the wire for this row",
  );
}

/** Strict trading-card gate.
 * A FAIL/rejected row is welcome when the searcher actually computed it.
 * Partial/error rows remain diagnostics until their source can produce a quote.
 */
export function isRealLiveEconomicCard(opp: OmniOpportunity): boolean {
  if (opp.economics?.computation_status !== "computed") return false;
  const ledger = buildLedger(opp);
  return (
    ledger.basis === "canonical" &&
    ledger.principal_usd != null &&
    ledger.gross_usd != null &&
    ledger.total_cost_usd != null &&
    ledger.net_usd != null
  );
}

/**
 * Does this row carry a COMPUTED economics object with figures on it?
 *
 * This is what separates a rejection the searcher actually PRICED (its gross /
 * cost / net are real numbers, rejection included) from a detection whose
 * producer never produced a number at all (`error` / `partial`, e.g.
 * `v3_quote_unavailable`). Operator order 2026-09-27 (verbatim): "las mostrar
 * rechazadas las muestra SIN DATOS > 0, esas [con datos] son las que deben
 * mostrarse" — the data-carrying rejections are the ones to paint.
 */
export function hasComputedFigures(opp: OmniOpportunity): boolean {
  const e = opp.economics;
  if (e == null || e.computation_status !== "computed") return false;
  return (
    num(e.amount_in_usd) != null ||
    num(e.gross_profit_usd) != null ||
    num(e.total_cost_usd) != null ||
    num(e.net_profit_usd) != null
  );
}

/** Net for the profit filter: canonical economics first, then the wire pair,
 *  then the SIM triple — the same precedence every card cell already uses. */
export function netOf(opp: OmniOpportunity): number | null {
  const e = opp.economics;
  return (
    num(e?.net_profit_usd) ??
    num(opp.net_expected_profit_usd) ??
    num(opp.simulated_net_profit_usd)
  );
}

/** A row the pipeline itself marked as a rejection (either lifecycle status or
 *  an explicit reason — the same union the api-server's paper_status uses). */
export function isRejectedRow(opp: OmniOpportunity): boolean {
  return (
    opp.status === "rejected" ||
    opp.status === "failed" ||
    (opp.rejection_reason != null && opp.rejection_reason !== "")
  );
}

/**
 * OPERATOR ORDER 2026-09-27 (verbatim): "APLICA LA D" —
 *
 *   WHERE (rejection_reason IS NULL
 *          OR (computation_status = 'computed' AND net_amount > 0))
 *
 * A rejection is welcome on the grid when the searcher actually PRICED it and
 * the arithmetic came out POSITIVE (the `gas_floor_breach:own_capital` row with
 * net +1.7712 is the canonical case). A rejection priced NEGATIVE (e.g.
 * `non_positive_profit` at −68.08) or never priced at all (`error`:
 * `v3_quote_unavailable`) is not a gain and stays out of the default view —
 * still counted, still listed in the audit trail, one toggle away.
 *
 * Non-rejected rows are always in scope (the clause's `rejection_reason IS
 * NULL` arm); this function never invents figures — it only classifies.
 */
export function inDefaultScope(opp: OmniOpportunity): boolean {
  if (!isRejectedRow(opp)) return true;
  if (!hasComputedFigures(opp)) return false;
  const net = netOf(opp);
  return net != null && net > 0;
}

/**
 * Grid scope for the live card grid — operator orders 2026-09-27:
 *   · "agregar un toggle 'Mostrar rechazadas'"
 *   · "las mostrar rechazadas las muestra sin datos > 0, esas son las que deben
 *     mostrarse"
 *   · "APLICA LA D" (rejected rows only when computed AND net > 0)
 *
 * Buckets:
 *   `economic`  — ladder closed on a real notional: real/live trading cards.
 *   `inScope`   — the DEFAULT grid: every non-rejected row, plus the rejected
 *                 rows with computed POSITIVE economics (gate D).
 *   `withData`  — rejected rows the searcher priced but that the default scope
 *                 leaves out (negative net) — revealed by the toggle.
 *   `noData`    — detections with no computed figure at all (`error`/`partial`).
 *                 NEVER painted as economic cards, never filled with an
 *                 assumption; always DECLARED (counted, with reason, in the
 *                 audit trail).
 *
 * OFF (default) ⇒ grid === the D scope. ON ⇒ every row that carries data
 * (cards first), i.e. the pre-D view.
 *
 * Pure: no I/O, no mutation, input order preserved inside each bucket.
 */
export function selectGridRows(
  filtered: OmniOpportunity[],
  showRejected: boolean,
): {
  economic: OmniOpportunity[];
  inScope: OmniOpportunity[];
  withData: OmniOpportunity[];
  hiddenByGate: OmniOpportunity[];
  noData: OmniOpportunity[];
  grid: OmniOpportunity[];
} {
  const economic: OmniOpportunity[] = [];
  const inScope: OmniOpportunity[] = [];
  const withData: OmniOpportunity[] = [];
  const hiddenByGate: OmniOpportunity[] = [];
  const noData: OmniOpportunity[] = [];
  for (const opp of filtered) {
    const isCard = isRealLiveEconomicCard(opp);
    const hasData = isCard || hasComputedFigures(opp);
    const inD = inDefaultScope(opp);
    if (isCard) economic.push(opp);
    if (hasData) withData.push(opp);
    else noData.push(opp);
    if (inD) inScope.push(opp);
    else if (hasData) hiddenByGate.push(opp);
  }
  return {
    economic,
    inScope,
    withData,
    hiddenByGate,
    noData,
    // OFF ⇒ gate D (viable + rejected with computed net > 0).
    // ON  ⇒ every row that carries data (the pre-D view); a row with NO figure
    //       is never painted on either setting — it stays declared.
    grid: showRejected ? withData : inScope,
  };
}

// ── Notifier ────────────────────────────────────────────────────────────────

export interface NotificationDecision {
  /** False ⇒ the toast must not fire for this row. */
  fire: boolean;
  /** The figure the toast would announce, in USD, or null when it must not fire. */
  amount_usd: number | null;
  /** What that figure IS. Never call a gross a "net yield". */
  label: "Net yield" | "Gross" | null;
  /** Which producer's arithmetic `amount_usd` belongs to. */
  basis: LedgerBasis;
  /** Why it did not fire (observability / tests — not rendered on the toast). */
  reason: string | null;
}

/**
 * High-value-toast gate.
 *
 * The live toast read
 * `High value opportunity — dex_arb / Net yield $822215.98 · chain 1 · …`
 * while `$822215.98` is `expected_profit_usd`, i.e. the DEX engine's fast-filter
 * GROSS at its probe — announced as a "Net yield", on a row whose own kernel net
 * was ≈ −0.00001. Two independent violations on one line: the wrong figure and
 * the wrong word for it.
 *
 * Rules:
 *   · The announced figure is the NET of the closed ladder, else its GROSS —
 *     never a field picked by name.
 *   · `null` is NOT `0`: an absent figure can never clear the threshold
 *     (`opp.expected_profit_usd ?? 0` made every uncomputed row pass a
 *     threshold of 0, R8).
 *   · A ladder rendered on the canonical basis announces its GROSS as a gross,
 *     or (when it is the closed net that is being compared) its net, always
 *     with the honest label.
 */
export function decideOpportunityNotification(
  opp: OmniOpportunity,
  thresholdUsd: number,
): NotificationDecision {
  const ledger = buildLedger(opp);

  if (ledger.basis === "none") {
    return {
      fire: false,
      amount_usd: null,
      label: null,
      basis: "none",
      reason: ledger.reason ?? "no closed ladder",
    };
  }

  // Prefer the NET (the operator's "did this make money" figure). Fall back to
  // the gross only when the ladder carries no net at all, and SAY SO.
  const netUsd = num(ledger.net_usd);
  const grossUsd = num(ledger.gross_usd);
  const amount = netUsd ?? grossUsd;
  const label: "Net yield" | "Gross" = netUsd != null ? "Net yield" : "Gross";

  if (amount == null) {
    return {
      fire: false,
      amount_usd: null,
      label: null,
      basis: ledger.basis,
      reason: "ladder carries neither a net nor a gross",
    };
  }
  const threshold = Number.isFinite(thresholdUsd) ? thresholdUsd : 0;
  if (!(amount >= threshold)) {
    return {
      fire: false,
      // R8 contract: a toast that does not fire announces NOTHING — neither a
      // figure nor a label a caller could still render.
      amount_usd: null,
      label: null,
      basis: ledger.basis,
      reason: `below threshold (${amount} < ${threshold})`,
    };
  }
  return {
    fire: true,
    amount_usd: amount,
    label,
    basis: ledger.basis,
    reason: null,
  };
}
