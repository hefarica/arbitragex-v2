/**
 * GAP-B (Data, 2026-10) — consumer contract for the A.6 gas-coverage DENOMINATOR.
 *
 * WHY THIS FILE EXISTS, AND WHY PART OF IT IS `.todo`
 * --------------------------------------------------
 * The producer names WHERE its coverage denominator came from
 * (`paths.actual.denominator`, `paths.actual.excluded` —
 * backend/api-server/src/routes/risk-circuit-breakers.ts:1121-1122, added by the
 * GAP-B backend work) so a consumer can tell "no rows in the window" apart from
 * "no MEASURABLE rows in the window". Those are opposite claims and the
 * conservative one is the degraded deployment.
 *
 * MEASURED TODAY (6/6 PASS in a probe running the REAL schema against a
 * producer-shaped wire payload, rama `fix/perhop-reserves-01` @ 858b943f):
 *   10 raw keys -> 8 parsed keys. `denominator` and `excluded` DO NOT SURVIVE
 *   Zod 3.25 (`z.object` strips unknown keys by default and
 *   CircuitBreakerEvidenceSchema.paths.actual never declared them), so they are
 *   `undefined` in every component. The producer's disclosure is neutralised at
 *   the consumer edge.
 *
 * CONSEQUENCE observable in the DOM (DOM artefact, not a live page):
 *   ledger case (migration 126 applied, 12 rows all not_applicable/impossible)
 *     -> "coverage absent — no ledger rows in the window; not evidence of $0 gas"
 *   while the SAME card's Evidence block states "12 runs in the window"
 *   (risk-circuit-breakers.ts:1148-1150). The row denies the rows the card has.
 *
 * The fix is a 2-field ADDITIVE schema change owned by frontend/lib/schemas.ts
 * (prepared, NOT applied — FRONTEND FREEZE PROTOCOL requires explicit operator
 * approval). The `.todo` gates below are the promotion targets: each one is
 * asserted live the moment the schema declares the pair. Marking them `.todo`
 * rather than landing a permanently red suite is a deliberate CI-hygiene choice,
 * not a weakened assertion — no assertion here was softened to pass.
 *
 * WHAT IS *NOT* CLAIMED HERE
 * --------------------------
 * The two denominator branches were expected to render byte-identically; that
 * was FALSIFIED by measurement (`P3_BYTE_IDENTICAL=false`). They render two
 * different labels ("coverage absent…" vs "0/12 rows measured…"). The defect is
 * narrower and worse than "identical output": the ledger branch prints a claim
 * that is provably FALSE for that payload, and the consumer cannot reach
 * `excluded` to see why.
 */
import { describe, it, expect } from "vitest";
import React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { CircuitBreakerSchema } from "@/lib/schemas";
import { DecidingPathRow } from "../RiskCircuitPanel";

/** Producer-shaped ACTUAL path — field-for-field from risk-circuit-breakers.ts:1116-1127. */
function wireBreaker(actual: Record<string, unknown>) {
  return {
    id: "gas_burn_breaker",
    name: "Max gas burn (rolling window)",
    category: "gas_burn",
    state: "NOT_AVAILABLE",
    severity: "high",
    action: "none",
    evidence: {
      source: "paper_ledger",
      detail: "actual-gas coverage ledger carries no measurement yet",
      current_value: null,
      threshold: 50,
      unit: "USD per window",
      deciding_path: "actual",
      paths: {
        sim: {
          state: "PASS",
          value: 1.25,
          measurements: 7,
          window_rows: 7,
          window_secs: 3600,
          scope: "chain:1",
          reason: "sim reason",
        },
        actual,
      },
    },
    blocks: ["LIVE"],
    operator_required: false,
    last_evaluated_at: "2026-10-04T00:00:00.000Z",
    description: "gas burn breaker",
    required_action: "probe",
  };
}

/** Migration 126 applied, 12 window rows all not_applicable/impossible. */
const LEDGER_0_OF_12 = {
  state: "NOT_AVAILABLE",
  value: null,
  measured: 0,
  expected: 0,
  denominator: "ledger",
  excluded: 12,
  window_hours: 24,
  scope: "chain:1",
  provenance: "sim-ctl replay via drift_tracker (not on-chain settled)",
  reason: "12 runs in the window but none has actual_gas_cost_usd recorded yet",
};

/** No migration 126: conservative denominator = every window row. */
const ALL_ROWS_0_OF_12 = { ...LEDGER_0_OF_12, denominator: "all_rows_no_ledger", excluded: 0, expected: 12 };

function parsed(actual: Record<string, unknown>) {
  const r = CircuitBreakerSchema.safeParse(wireBreaker(actual));
  if (!r.success) throw new Error(`producer wire rejected by schema: ${JSON.stringify(r.error.issues)}`);
  return r.data.evidence.paths!.actual as unknown as Record<string, unknown>;
}

describe("A.6 actual path — the wire never rejects either denominator", () => {
  // This is the part that must hold TODAY: adding the pair to the schema must
  // not be allowed to make the schema reject a real payload.
  it("both denominator branches parse (no strict validator may be introduced)", () => {
    expect(CircuitBreakerSchema.safeParse(wireBreaker(LEDGER_0_OF_12)).success).toBe(true);
    expect(CircuitBreakerSchema.safeParse(wireBreaker(ALL_ROWS_0_OF_12)).success).toBe(true);
  });

  it("excluded is NEGATIVE on the breach branch — a .nonnegative()/.min(0) validator would 500 the panel", () => {
    // risk-circuit-breakers.ts:248 `breach = ... || excluded < 0`, emitted at :1122.
    const breach = { ...LEDGER_0_OF_12, measured: 1, expected: 10, excluded: -30, state: "NOT_AVAILABLE" };
    const r = CircuitBreakerSchema.safeParse(wireBreaker(breach));
    expect(r.success).toBe(true);
    // The value as the producer emits it (a consumer validator assuming >= 0
    // would abort the whole /risk parse — getValidated turns safeParse failure
    // into {ok:false}).
    expect(breach.excluded).toBe(-30);
  });
});

describe("A.6 actual path — GAP-B GATE (promote to `it` when schemas.ts declares the pair)", () => {
  it.todo("denominator survives parsing — the producer's provenance reaches the component");
  it.todo("excluded survives parsing — including its negative (breach) value");
});