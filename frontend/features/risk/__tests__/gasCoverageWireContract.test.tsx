/**
 * INTEGRATION — A.6 coverage denominator: does the pair survive the boundary?
 *
 * This is the CONTRACT test for the producer→adapter seam, not a UI test. It
 * takes payloads that are field-for-field the producer's ACTUAL path
 * (`backend/api-server/src/routes/risk-circuit-breakers.ts:1116-1127`, exercised
 * against the pure producer `deriveGasCoverage` at :226-256 by
 * `backend/api-server/src/routes/risk-circuit-breakers.test.ts`), pushes each
 * through the REAL `CircuitBreakerSchema` in `@/lib/schemas`, and measures what
 * the adapter did to `denominator` and `excluded`.
 *
 * WHAT IT PROVES TODAY (the defect, measured — not asserted from a summary):
 *   · the real schema PARSES both branches (so declaring the pair breaks nobody);
 *   · both qualifier fields are DROPPED, so `expected === 0` arrives with no way
 *     to tell "no ledger rows" from "all rows excluded by the ledger";
 *   · `excluded = -30` (breach) is also dropped, so the contradiction it names is
 *     unreachable downstream.
 *
 * WHY THE PRESERVED BRANCH IS A LOCAL FREEDOM, NOT A FORK
 * ------------------------------------------------------
 * `preservedSchema` below is the IDENTICAL schema with the two fields declared.
 * It exists to prove the target behaviour is reachable and that no strict
 * validator is needed (the pair includes a NEGATIVE integer) — NOT as a second
 * source of truth. When `schemas.ts` declares the pair, this file's expectation
 * flips from `discarded` to `preserved` and `preservedSchema` is deleted.
 * `frontend/lib/schemas.ts` is under the frontend freeze; the durable patch is
 * prepared in docs/handoffs/GAP-B-GAS-COVERAGE-INTEGRATION-01.md.
 */
import { describe, it, expect } from "vitest";
import { z } from "zod";
import { CircuitBreakerSchema } from "@/lib/schemas";
import { actualCoverageLabel } from "../RiskCircuitPanel";
import {
  classifyGasCoveragePreservation,
  coverageIdentity,
  emptyCoverageTelemetry,
  readGasCoveragePreservation,
  tallyResponseCoveragePreservation,
  coverageTelemetryLine,
} from "@/lib/wire-gas-coverage";

/**
 * Canonical digest of the four GAP-B payloads, as DERIVED by the producer's own
 * `deriveGasCoverage` in
 * `backend/api-server/src/routes/risk-circuit-breakers.gapb-fixture.test.ts`.
 * That test reads THIS file and fails if the two constants diverge, so the two
 * packages cannot start asserting different wires.
 */
export const GAP_B_FIXTURE_DIGEST =
  "3c57a09ae7f3e78c0d676620c853e54695019b422d019c2cac7726df057d6b27";

/** Producer-shaped breaker envelope (evidence/current_value/types included so
 *  the parse succeeds for reasons other than the pair under test). */
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

/** Migration 126 applied; the ledger classified every one of the 12 window rows
 *  not_applicable/impossible. `excluded = 12` is the only field that says WHY
 *  `expected` is 0. Producer: deriveGasCoverage({rowsInWindow:12, withActualGas:0,
 *  measurable:0, denominator:"ledger"}) ⇒ {expected:0, excluded:12, breach:false}. */
const LEDGER_ALL_EXCLUDED = {
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

/** No migration 126: conservative denominator = every window row. Same
 *  `expected`-shaped label, OPPOSITE meaning. */
const ALL_ROWS_0_OF_12 = {
  ...LEDGER_ALL_EXCLUDED,
  denominator: "all_rows_no_ledger",
  excluded: 0,
  expected: 12,
};

/** REACHABLE breach: 42 rows carry actual gas while the ledger allows 12 to.
 *  Producer: deriveGasCoverage({rowsInWindow:12, withActualGas:42, measurable:12,
 *  denominator:"ledger"}) ⇒ {expected:12, excluded:0, breach:true}. The
 *  contradiction is carried by the NUMERATOR (measured > expected), NOT by a
 *  negative `excluded` — `Math.max(0, measurable)` clamps it to the subset. */
const LEDGER_NUMERATOR_BREACH = {
  ...LEDGER_ALL_EXCLUDED,
  measured: 42,
  expected: 12,
  excluded: 0,
  reason: "coverage ledger contradicts itself",
};

/** DEFENSIVE branch: `excluded < 0` needs `measurable > rows_in_window`. The
 *  ledger query computes `measurable` as a COUNT(*) FILTER over the same rows it
 *  counts as `rows_in_window`, so the current SQL never emits this — it is the
 *  branch a schema must not CLAMP (a strict `.min(0)` would turn a contradiction
 *  into a valid-looking payload). Held so the contract covers it explicitly. */
const LEDGER_INCONSISTENT_AGGREGATE = {
  ...LEDGER_ALL_EXCLUDED,
  measured: 42,
  expected: 42,
  excluded: -30,
  reason: "coverage ledger contradicts itself",
};

/** The target adapter: same schema, pair declared. Optional + nullable-free
 *  exactly as the producer emits it; NO `.nonnegative()` / `.min(0)` — the
 *  breach branch legitimately sends a negative integer. */
const preservedSchema = CircuitBreakerSchema.extend({
  evidence: CircuitBreakerSchema.shape.evidence.extend({
    paths: z
      .object({
        sim: z.object({
          state: z.string().nullable(),
          value: z.number().nullable(),
          measurements: z.number().int().nonnegative(),
          window_rows: z.number().int().nonnegative(),
          window_secs: z.number().nullable(),
          scope: z.string(),
          reason: z.string(),
        }),
        actual: z.object({
          state: z.string().nullable(),
          value: z.number().nullable(),
          measured: z.number().int().nonnegative(),
          expected: z.number().int().nonnegative(),
          denominator: z.enum(["ledger", "all_rows_no_ledger"]),
          excluded: z.number().int(),
          window_hours: z.number().nullable(),
          scope: z.string(),
          provenance: z.string(),
          reason: z.string(),
        }),
      })
      .optional(),
  }),
});

function parseWith(schema: typeof CircuitBreakerSchema, actual: Record<string, unknown>) {
  const r = schema.safeParse(wireBreaker(actual));
  if (!r.success) {
    throw new Error(`wire rejected: ${JSON.stringify(r.error.issues)}`);
  }
  return (r.data as { evidence: { paths?: { actual?: unknown } } }).evidence.paths?.actual;
}

/** Producer-mirroring canonicalization: sorted keys, no whitespace. */
function canonical(value: unknown): string {
  return JSON.stringify(value, (_key, v: unknown) =>
    v !== null && typeof v === "object" && !Array.isArray(v)
      ? Object.fromEntries(
          Object.entries(v as Record<string, unknown>).sort(([a], [b]) => a.localeCompare(b)),
        )
      : v,
  );
}

/** The four fields the adapter owns — the canonical set the digest freezes. */
function pairOf(actual: Record<string, unknown>) {
  return {
    expected: actual.expected,
    denominator: actual.denominator,
    excluded: actual.excluded,
    measured: actual.measured,
  };
}

async function digestOf(value: unknown): Promise<string> {
  const bytes = new TextEncoder().encode(canonical(value));
  const hash = await crypto.subtle.digest("SHA-256", bytes);
  return Array.from(new Uint8Array(hash))
    .map((b) => b.toString(16).padStart(2, "0"))
    .join("");
}

describe("A.6 wire contract — the pair the producer emits", () => {
  it("the payloads are the PRODUCER-DERIVED canonical set (digest identity)", async () => {
    // The four payloads are not invented here: the producer test derives them
    // through `deriveGasCoverage` and freezes this digest over the PAIR ONLY
    // (expected/denominator/excluded/measured). `state`/`value`/`reason` are
    // rewritten by the evaluator after assembly, so they are excluded on both
    // sides — freezing them would fail for reasons the adapter does not own.
    const actual = await digestOf({
      ledger_all_excluded: pairOf(LEDGER_ALL_EXCLUDED),
      all_rows_no_ledger: pairOf(ALL_ROWS_0_OF_12),
      ledger_numerator_breach: pairOf(LEDGER_NUMERATOR_BREACH),
      ledger_inconsistent_aggregate: pairOf(LEDGER_INCONSISTENT_AGGREGATE),
    });
    expect(actual).toBe(GAP_B_FIXTURE_DIGEST);
  });

  it("the schema under test is the SHIPPED one (no local fork of @/lib/schemas)", () => {
    // If a patch ever lands, this test file must be updated consciously — the
    // pair appearing in the shipped schema is the trigger, not a silent drift.
    const parsed = parseWith(CircuitBreakerSchema, LEDGER_ALL_EXCLUDED) as Record<string, unknown>;
    expect(Object.prototype.hasOwnProperty.call(parsed, "denominator")).toBe(false);
    expect(Object.prototype.hasOwnProperty.call(parsed, "excluded")).toBe(false);
    // Provenance of the measurement itself IS carried — so the loss is specific.
    expect(parsed.provenance).toBe("sim-ctl replay via drift_tracker (not on-chain settled)");
    expect(parsed.expected).toBe(0);
  });

  it("both branches parse: no consumer is broken by declaring the pair", () => {
    for (const payload of [
      LEDGER_ALL_EXCLUDED,
      ALL_ROWS_0_OF_12,
      LEDGER_NUMERATOR_BREACH,
      LEDGER_INCONSISTENT_AGGREGATE,
    ]) {
      expect(parseWith(CircuitBreakerSchema, payload)).toBeDefined();
      expect(parseWith(preservedSchema, payload)).toBeDefined();
    }
  });

  it("TODAY: the adapter DROPS the pair — the qualifier of the zero is unreachable", () => {
    const ledger = parseWith(CircuitBreakerSchema, LEDGER_ALL_EXCLUDED);
    const allRows = parseWith(CircuitBreakerSchema, ALL_ROWS_0_OF_12);

    expect(classifyGasCoveragePreservation(ledger)).toBe("discarded");
    expect(classifyGasCoveragePreservation(allRows)).toBe("discarded");

    // The load-bearing consequence: two payloads that make OPPOSITE claims are
    // indistinguishable on the qualifier fields, and only `expected` separates
    // them — a field the producer itself labels "conservative default", not a
    // fact about the ledger.
    const l = readGasCoveragePreservation(ledger);
    const a = readGasCoveragePreservation(allRows);
    expect(l).toEqual({ denominator: undefined, excluded: undefined });
    expect(a).toEqual({ denominator: undefined, excluded: undefined });
    expect((ledger as { measured: number }).measured).toBe((allRows as { measured: number }).measured);
    expect((ledger as { expected: number }).expected).toBe(0);
    expect((allRows as { expected: number }).expected).toBe(12);

    // And the SENTENCE the panel renders: the all-rows branch is told the window
    // has no ledger rows at all — a claim its own `excluded`/`expected` refute.
    expect(actualCoverageLabel(0, (allRows as { expected: number }).expected, "NOT_AVAILABLE")).toContain(
      "12 rows measured",
    );
    expect(actualCoverageLabel(0, (ledger as { expected: number }).expected, "NOT_AVAILABLE")).toContain(
      "coverage absent",
    );
  });

  it("the signed (breach) exclusion is dropped too — signedness is not the blocker", () => {
    const inconsistent = parseWith(CircuitBreakerSchema, LEDGER_INCONSISTENT_AGGREGATE);
    expect(classifyGasCoveragePreservation(inconsistent)).toBe("discarded");
    // Declaring it is safe: a NEGATIVE integer must parse (no .min(0)/.nonnegative()).
    // A clamp here would turn a self-contradicting aggregate into a valid payload.
    const preserved = parseWith(preservedSchema, LEDGER_INCONSISTENT_AGGREGATE) as Record<string, unknown>;
    expect(preserved.excluded).toBe(-30);
    expect(preserved.denominator).toBe("ledger");

    // REACHABILITY: the breach the producer actually emits is the NUMERATOR one
    // (measured 42 > expected 12) and it arrives with excluded = 0. So the
    // contract needs no negative value to be exercised — but must not forbid one.
    const reachable = parseWith(preservedSchema, LEDGER_NUMERATOR_BREACH) as Record<string, unknown>;
    expect(reachable.excluded).toBe(0);
    expect((reachable as { measured: number }).measured).toBeGreaterThan(
      (reachable as { expected: number }).expected,
    );
  });

  it("TARGET: with the pair declared, identity and reading both survive", () => {
    const ledger = parseWith(preservedSchema, LEDGER_ALL_EXCLUDED);
    const allRows = parseWith(preservedSchema, ALL_ROWS_0_OF_12);
    const inconsistent = parseWith(preservedSchema, LEDGER_INCONSISTENT_AGGREGATE);

    expect(coverageIdentity(ledger)).toEqual({
      preservation: "preserved",
      identity: "ledger",
      reading: "exclusion_counted",
    });
    expect(coverageIdentity(allRows)).toEqual({
      preservation: "preserved",
      identity: "all_rows_no_ledger",
      reading: "exclusion_none",
    });
    // The contradiction stays visible instead of being clamped into a PASS.
    expect(coverageIdentity(inconsistent)).toEqual({
      preservation: "preserved",
      identity: "ledger",
      reading: "exclusion_counted",
    });
    expect((inconsistent as { excluded: number }).excluded).toBeLessThan(0);
  });

  it("half-carried is INCONSISTENT, never laundered into the conservative reading", () => {
    // A nonzero exclusion without its provenance is the dangerous half.
    expect(classifyGasCoveragePreservation({ expected: 0, excluded: 12 })).toBe("inconsistent");
    expect(coverageIdentity({ expected: 0, excluded: 12 }).reading).toBe("provenance_lost");
    // Provenance without the count is equally unqualified.
    expect(classifyGasCoveragePreservation({ expected: 12, denominator: "ledger" })).toBe(
      "inconsistent",
    );
    // A discarded pair is degraded but says nothing false.
    expect(coverageIdentity({ expected: 0 }).preservation).toBe("discarded");
    expect(coverageIdentity({ expected: 0 }).identity).toBe("unavailable");
  });

  it("telemetry counts the loss per response instead of averaging it away", () => {
    const response = {
      breakers: [wireBreaker(LEDGER_ALL_EXCLUDED), wireBreaker(LEDGER_INCONSISTENT_AGGREGATE)],
    };
    const shipped = tallyResponseCoveragePreservation(emptyCoverageTelemetry(), {
      // the breakers as the SHIPPED schema sees them
      breakers: response.breakers.map((b) => ({
        evidence: {
          paths: {
            actual: parseWith(CircuitBreakerSchema, b.evidence.paths.actual as Record<string, unknown>),
          },
        },
      })),
    });
    expect(shipped).toEqual({ seen: 2, preserved: 0, discarded: 2, inconsistent: 0 });
    expect(coverageTelemetryLine(shipped)).toContain("discarded=2");

    const target = tallyResponseCoveragePreservation(emptyCoverageTelemetry(), {
      breakers: response.breakers.map((b) => ({
        evidence: {
          paths: {
            actual: parseWith(preservedSchema, b.evidence.paths.actual as Record<string, unknown>),
          },
        },
      })),
    });
    expect(target).toEqual({ seen: 2, preserved: 2, discarded: 0, inconsistent: 0 });
  });

  it("a breaker with no actual path is SKIPPED, not counted as a failure", () => {
    const telemetry = tallyResponseCoveragePreservation(emptyCoverageTelemetry(), {
      breakers: [{ evidence: {} }, null, { evidence: { paths: { sim: {} } } }],
    });
    expect(telemetry).toEqual({ seen: 0, preserved: 0, discarded: 0, inconsistent: 0 });
  });
});
