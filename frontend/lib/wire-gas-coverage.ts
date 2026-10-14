/**
 * A.6 actual-gas coverage — adapter for the TWO wire fields the panel never sees.
 *
 * WHY THIS FILE EXISTS
 * --------------------
 * The A.6 producer (`backend/api-server/src/routes/risk-circuit-breakers.ts`)
 * emits a claim and the evidence that qualifies it, on the same object:
 *
 *   paths.actual.expected     — coverage denominator (rows allowed to carry a measurement)
 *   paths.actual.denominator  — WHERE that denominator came from: "ledger" or
 *                               "all_rows_no_ledger"   (producer :146, :1121)
 *   paths.actual.excluded     — window rows the ledger classified
 *                               not_applicable/impossible (SIGNED; negative = breach)
 *                                                      (producer :149, :1122)
 *
 * The consumer's adapter (`frontend/lib/schemas.ts` —
 * `CircuitBreakerEvidenceSchema.paths.actual`, :1229-1238) declares only
 * `state, value, measured, expected, window_hours, scope, provenance, reason`.
 * Zod 3 strips undeclared keys, so BOTH qualifier fields are dropped at the
 * boundary and no component can reach them.
 *
 * Impact, precisely scoped: `actualCoverageLabel` receives `expected === 0` and
 * cannot tell "no ledger rows in the window" from "expected is 0 BECAUSE the
 * ledger excluded all 12 rows" — it renders the first string for both. The
 * second claim is false for the payload that caused it, and the field that
 * disproves it (`excluded = 12`) is unreachable. With `excluded` absent the
 * consumer also cannot explain a negative-denominator breach (`excluded < 0`).
 *
 * WHAT THIS MODULE IS
 * -------------------
 * A dependency-free, PURE adapter that (a) states the contract and (b) makes the
 * loss MEASURABLE instead of silent. It does not render and does not reimplement
 * the schema: it is the seam a consumer uses to ask "did identity survive?".
 * The durable patch that actually declares the pair on `schemas.ts` is prepared
 * in `docs/handoffs/GAP-B-GAS-COVERAGE-INTEGRATION-01.md` and is NOT applied
 * here (frontend freeze — needs explicit operator approval).
 *
 * IDENTITY RULE (the non-obvious part, encoded below)
 * ---------------------------------------------------
 * `excluded !== 0` WITHOUT `denominator` is not a survivable state: it loses the
 * provenance of a nontrivial exclusion while still asserting one. A nonzero
 * `excluded` and a MISSING `denominator` must NEVER be silently relabelled — the
 * producer's two branches make different claims (`excluded = 12` ⇒ the ledger
 * did the excluding; `excluded = 0` on the all-rows branch ⇒ nothing was).
 */

/** Emitted by the producer at :146 / :241. Absent means "not the narrower claim". */
export type CoverageDenominator = "ledger" | "all_rows_no_ledger";

/** The pair the boundary must carry for the label to be a fact. */
export interface GasCoverageFields {
  denominator: CoverageDenominator | undefined;
  /** SIGNED on purpose: negative ⇒ the window's own arithmetic contradicts itself. */
  excluded: number | undefined;
}

export type CoveragePreservation = "preserved" | "discarded" | "inconsistent";

const DENOMINATORS: readonly string[] = ["ledger", "all_rows_no_ledger"];

/**
 * Read the pair out of an already-parsed object. Pure, defensive, never throws:
 * an unparsable value is reported as `inconsistent`, never coerced into a
 * denominator. Absence stays `undefined` — absence is not a value here.
 */
export function readGasCoveragePreservation(parsed: unknown): GasCoverageFields {
  if (parsed === null || typeof parsed !== "object") {
    return { denominator: undefined, excluded: undefined };
  }
  const raw = parsed as Record<string, unknown>;
  const denominator =
    typeof raw.denominator === "string" && DENOMINATORS.includes(raw.denominator)
      ? (raw.denominator as CoverageDenominator)
      : undefined;
  const excluded = typeof raw.excluded === "number" ? raw.excluded : undefined;
  return { denominator, excluded };
}

/**
 * Classify what the boundary did to the pair.
 *
 *  · preserved    — both present; `excluded` integral. The producer's claim is
 *                   qualifiable downstream (the label can name why).
 *  · discarded    — both absent: the boundary dropped them (today's schema).
 *                   Degraded, but self-consistent: nothing false is asserted.
 *  · inconsistent — exactly ONE present and it is nontrivial, or `excluded` is
 *                   non-integral. A half-carried contract must be loud, never
 *                   laundered into the "no ledger rows" reading.
 */
export function classifyGasCoveragePreservation(parsed: unknown): CoveragePreservation {
  const { denominator, excluded } = readGasCoveragePreservation(parsed);
  const hasDenominator = denominator !== undefined;
  const hasExcluded = excluded !== undefined;

  if (hasExcluded && !Number.isInteger(excluded)) return "inconsistent";
  if (hasDenominator && hasExcluded) return "preserved";
  if (!hasDenominator && !hasExcluded) return "discarded";
  // Half the pair survived. A nonzero exclusion with no provenance is the
  // dangerous half: it is the measurement without its identity.
  if (hasExcluded && excluded !== 0) return "inconsistent";
  return "inconsistent";
}

/**
 * What a consumer may honestly assert once it has classified the pair.
 * `identity: "unavailable"` is the R8-correct answer when provenance was lost —
 * it is NOT the same as asserting the conservative denominator.
 */
export function coverageIdentity(parsed: unknown): {
  preservation: CoveragePreservation;
  identity: CoverageDenominator | "unavailable";
  /** The only reading the caller may render, given what survived. */
  reading: "exclusion_counted" | "exclusion_none" | "provenance_lost";
} {
  const preservation = classifyGasCoveragePreservation(parsed);
  const { denominator, excluded } = readGasCoveragePreservation(parsed);

  if (preservation !== "preserved") {
    return { preservation, identity: "unavailable", reading: "provenance_lost" };
  }
  return {
    preservation,
    identity: denominator as CoverageDenominator,
    reading: excluded === 0 ? "exclusion_none" : "exclusion_counted",
  };
}

/**
 * Telemetry — the counter the integration owes its consumers. Counting is the
 * contract: a boundary that drops the pair must be visible, not inferred from a
 * wrong sentence in a panel.
 */
export interface CoverageTelemetry {
  seen: number;
  preserved: number;
  discarded: number;
  inconsistent: number;
}

export function emptyCoverageTelemetry(): CoverageTelemetry {
  return { seen: 0, preserved: 0, discarded: 0, inconsistent: 0 };
}

/** Fold one parsed breaker (or any parsed object) into the counter. Pure enough
 *  to unit test: returns a NEW object, never mutates the argument. */
export function tallyCoveragePreservation(
  telemetry: CoverageTelemetry,
  parsed: unknown,
): CoverageTelemetry {
  const preservation = classifyGasCoveragePreservation(parsed);
  return {
    seen: telemetry.seen + 1,
    preserved: telemetry.preserved + (preservation === "preserved" ? 1 : 0),
    discarded: telemetry.discarded + (preservation === "discarded" ? 1 : 0),
    inconsistent: telemetry.inconsistent + (preservation === "inconsistent" ? 1 : 0),
  };
}

/** Walk the `breakers[]` of a parsed A.6 response and tally every actual path
 *  that carries the pair or claims to. Only shape-aware, never permissive: a
 *  breaker without `evidence.paths.actual` is skipped, not counted as discarded
 *  (skipping is honest; counting it would manufacture a failure). */
export function tallyResponseCoveragePreservation(
  telemetry: CoverageTelemetry,
  parsedResponse: unknown,
): CoverageTelemetry {
  const breakers =
    parsedResponse !== null &&
    typeof parsedResponse === "object" &&
    Array.isArray((parsedResponse as Record<string, unknown>).breakers)
      ? ((parsedResponse as Record<string, unknown>).breakers as unknown[])
      : [];
  let acc = telemetry;
  for (const breaker of breakers) {
    const actual =
      breaker !== null && typeof breaker === "object"
        ? ((breaker as Record<string, unknown>).evidence as Record<string, unknown> | undefined)
            ?.paths
        : undefined;
    const actualPath =
      actual !== null && typeof actual === "object"
        ? (actual as Record<string, unknown>).actual
        : undefined;
    if (actualPath === undefined || actualPath === null) continue;
    acc = tallyCoveragePreservation(acc, actualPath);
  }
  return acc;
}

/**
 * One-line health summary for logs/OTel: a boundary that drops 100% of a field
 * is a defect statement, not a metric to average away.
 */
export function coverageTelemetryLine(telemetry: CoverageTelemetry): string {
  return (
    `gas_coverage_wire seen=${telemetry.seen} preserved=${telemetry.preserved} ` +
    `discarded=${telemetry.discarded} inconsistent=${telemetry.inconsistent}`
  );
}
