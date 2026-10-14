/**
 * INTEGRATION — canonical fixture for the A.6 coverage wire pair (GAP-B, 2026-10).
 *
 * The consumer-side contract test
 * (`frontend/features/risk/__tests__/gasCoverageWireContract.test.tsx`) measures
 * what the adapter does to `paths.actual.denominator` and `paths.actual.excluded`
 * using hand-written payloads. This file closes the provenance gap: it DERIVES
 * those payloads from the producer's own pure function (`deriveGasCoverage`,
 * :226-256) and freezes their canonical form under a SHA-256 digest.
 *
 * Consumers:
 *   · Frontend/adapter — the payloads are not invented; they are what
 *     `deriveGasCoverage` returns for `{rowsInWindow, withActualGas, measurable,
 *     denominator}`. Copying them by hand into another package is how a fixture
 *     drifts away from its producer.
 *   · Data / Reviewer — this is the evidence-level statement that produced a
 *     specific documented claim: the two branches differ in `excluded`, so they
 *     are NOT byte-identical.
 *   · Release / CI — the digest is the tripwire: if the producer's coverage
 *     arithmetic or the emitted field set changes, THIS test fails and the
 *     consumer's fixture must be regenerated deliberately. An adapter that
 *     silently tracks a moved wire is worse than one that fails.
 *
 * WHY A DIGEST AND NOT AN INLINE OBJECT COMPARISON
 * ------------------------------------------------
 * `toMatchInlineSnapshot` would be rewritten by any `-u` run, which is exactly
 * how a "green" suite stops being evidence. A hash can only change by editing an
 * explicit hex literal in this file.
 */
import { describe, it, expect } from "vitest";
import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { __forTesting } from "./risk-circuit-breakers.js";

const { deriveGasCoverage, GAS_WINDOW_SQL_LEDGER } = __forTesting;
/** Mirror of the producer-derived digest, held in the consumer's contract test.
 *  Read from source so the two constants cannot drift apart silently: this test
 *  FAILS if the consumer's copy is edited without the producer's. */
const CONSUMER_CONTRACT_TEST = fileURLToPath(
  new URL(
    "../../../../frontend/features/risk/__tests__/gasCoverageWireContract.test.tsx",
    import.meta.url,
  ),
);

/**
 * Canonical digest of the three GAP-B fixtures, sanitized.
 *
 * Sanitization is deliberate and minimal: the producer emits `scope:
 * "chain:${ctx.chainId}"` and `window_hours` from config, so those are carried
 * as an explicit placeholder/typed value rather than a live deployment id. No
 * credential, URL, address or secret is involved in this path at all (proven by
 * the existing `regression: no fake metrics + no secrets` block in
 * risk-circuit-breakers.test.ts).
 */
const GAP_B_FIXTURE_DIGEST = "3c57a09ae7f3e78c0d676620c853e54695019b422d019c2cac7726df057d6b27";
/** Same canonicalization the consumer must use: sorted keys, no whitespace. */
function canonical(value: unknown): string {
  return JSON.stringify(value, (_key, v: unknown) =>
    v !== null && typeof v === "object" && !Array.isArray(v)
      ? Object.fromEntries(Object.entries(v as Record<string, unknown>).sort(([a], [b]) => a.localeCompare(b)))
      : v,
  );
}

function digest(value: unknown): string {
  return createHash("sha256").update(canonical(value), "utf8").digest("hex");
}

/**
 * The producer's ACTUAL path, exactly as `risk-circuit-breakers.ts:1116-1127`
 * assembles it — built from `deriveGasCoverage`, never from hand-written counts.
 * `measured`/`window_hours`/`scope`/`provenance`/`reason` are set to the
 * documented producer values for the three GAP-B scenarios.
 */
function actualPathFromCoverage(input: {
  rowsInWindow: number;
  withActualGas: number;
  measurable?: number;
  denominator?: "ledger" | "all_rows_no_ledger";
  measured: number;
  reason: string;
}) {
  const cov = deriveGasCoverage({
    rowsInWindow: input.rowsInWindow,
    withActualGas: input.withActualGas,
    measurable: input.measurable,
    denominator: input.denominator,
  });
  return {
    state: null,
    value: null,
    measured: input.measured,
    expected: cov.expected,
    denominator: cov.denominator,
    excluded: cov.excluded,
    window_hours: 24,
    scope: "chain:1",
    provenance: "sim-ctl replay via drift_tracker (not on-chain settled)",
    reason: input.reason,
  };
}

/**
 * CANONICAL SET — the coverage pair only, keyed by scenario.
 *
 * `state`, `value` and `reason` are overwritten by the evaluator AFTER this
 * object is assembled (:1116-1187), so they are not part of the adapter's
 * contract and are deliberately excluded: a fixture frozen over fields the
 * producer rewrites would fail for the wrong reason. What the boundary must
 * carry, verbatim and unclamped, is exactly these pairs.
 */
export const GAP_B_FIXTURE_PAIRS = {
  /** Migration 126 applied; all 12 window rows not_applicable/impossible. */
  ledger_all_excluded: pairOf(
    actualPathFromCoverage({
      rowsInWindow: 12,
      withActualGas: 0,
      measurable: 0,
      denominator: "ledger",
      measured: 0,
      reason: "12 runs in the window but none has actual_gas_cost_usd recorded yet",
    }),
  ),
  /** No migration 126: every window row is the denominator (pre-126 behaviour). */
  all_rows_no_ledger: pairOf(
    actualPathFromCoverage({
      rowsInWindow: 12,
      withActualGas: 0,
      measurable: undefined,
      denominator: undefined,
      measured: 0,
      reason: "12 runs in the window but none has actual_gas_cost_usd recorded yet",
    }),
  ),
  /**
   * REACHABLE breach shape: 42 rows carry `actual_gas_cost_usd` while the ledger
   * allows 12 of them to. The producer flags the contradiction through the
   * NUMERATOR (`measured > expected`), and `excluded` stays NON-NEGATIVE — 12 - 42
   * is clamped to the subset by `Math.max(0, measurable)` (producer :243).
   */
  ledger_numerator_breach: pairOf(
    actualPathFromCoverage({
      rowsInWindow: 12,
      withActualGas: 42,
      measurable: 12,
      denominator: "ledger",
      measured: 42,
      reason: "coverage ledger contradicts itself",
    }),
  ),
  /**
   * DEFENSIVE branch — NOT emitted by the current SQL, and that is the point.
   * `excluded < 0` requires `measurable > rows_in_window` (producer :244), but
   * `measurable` is a strict FILTER SUBSET of `COUNT(*)` in
   * GAS_WINDOW_SQL_LEDGER, so the subset relation is a SQL invariant, not a
   * runtime possibility. The value is kept in the fixture so the consumer's
   * contract test covers the branch the schema must NOT clamp.
   */
  ledger_inconsistent_aggregate: pairOf(
    actualPathFromCoverage({
      rowsInWindow: 12,
      withActualGas: 42,
      measurable: 42,
      denominator: "ledger",
      measured: 42,
      reason: "coverage ledger contradicts itself",
    }),
  ),
} as const;

/** The four fields the adapter owns. Everything else is evaluator-owned. */
function pairOf(path: ReturnType<typeof actualPathFromCoverage>) {
  return {
    expected: path.expected,
    denominator: path.denominator,
    excluded: path.excluded,
    measured: path.measured,
  };
}

export const GAP_B_FIXTURES = {
  ledger_all_excluded: actualPathFromCoverage({
    rowsInWindow: 12,
    withActualGas: 0,
    measurable: 0,
    denominator: "ledger",
    measured: 0,
    reason: "12 runs in the window but none has actual_gas_cost_usd recorded yet",
  }),
  all_rows_no_ledger: actualPathFromCoverage({
    rowsInWindow: 12,
    withActualGas: 0,
    measurable: undefined,
    denominator: undefined,
    measured: 0,
    reason: "12 runs in the window but none has actual_gas_cost_usd recorded yet",
  }),
  ledger_numerator_breach: actualPathFromCoverage({
    rowsInWindow: 12,
    withActualGas: 42,
    measurable: 12,
    denominator: "ledger",
    measured: 42,
    reason: "coverage ledger contradicts itself",
  }),
  ledger_inconsistent_aggregate: actualPathFromCoverage({
    rowsInWindow: 12,
    withActualGas: 42,
    measurable: 42,
    denominator: "ledger",
    measured: 42,
    reason: "coverage ledger contradicts itself",
  }),
} as const;

describe("GAP-B canonical wire fixture (producer-derived)", () => {
  it("the fixture is DERIVED from deriveGasCoverage, not hand-written", () => {
    expect(GAP_B_FIXTURES.ledger_all_excluded).toMatchObject({
      expected: 0,
      excluded: 12,
      denominator: "ledger",
    });
    expect(GAP_B_FIXTURES.all_rows_no_ledger).toMatchObject({
      expected: 12,
      excluded: 0,
      denominator: "all_rows_no_ledger",
    });
    expect(GAP_B_FIXTURES.ledger_numerator_breach).toMatchObject({
      expected: 12,
      excluded: 0,
      denominator: "ledger",
      measured: 42,
    });
    expect(GAP_B_FIXTURES.ledger_inconsistent_aggregate).toMatchObject({
      expected: 42,
      excluded: -30,
      denominator: "ledger",
    });
    // The producer emits the pair as ordinary JSON numbers/strings — nothing
    // exotic for an adapter to carry (a string-typed `excluded` would fail here).
    expect(typeof GAP_B_FIXTURES.ledger_inconsistent_aggregate.excluded).toBe("number");
    expect(typeof GAP_B_FIXTURES.ledger_all_excluded.denominator).toBe("string");
  });

  it("REACHABILITY: measurable is a SQL subset of the window, so excluded < 0 is defensive", () => {
    // This is the claim that decides whether the schema's `excluded` needs a
    // clamp. `excluded = rowsInWindow - expected` and `expected = max(0,
    // measurable)` (producer :243-244), so a NEGATIVE excluded requires
    // measurable > rows_in_window. The ledger query computes both with the same
    // COUNT(*) population, so the subset relation cannot be violated by the
    // query itself — it would take an inconsistent aggregate (a different
    // window, a join mismatch) to produce one.
    const sql = GAS_WINDOW_SQL_LEDGER;
    expect(sql).toContain("COUNT(*)::int AS rows_in_window");
    expect(sql).toContain(")::int AS measurable");
    // The FILTER counts a subset of the same rows — never a separate source.
    expect(sql).toMatch(/COUNT\(\*\)\s+FILTER/);
    expect(sql).not.toMatch(/measurable[\s\S]*FROM\s+\w+\s+JOIN/i);
  });

  it("the pair is the ONLY difference between the two zero-coverage branches", () => {
    // Corrects the report that claimed the branches render byte-identically:
    // they differ in exactly these fields, so a consumer that drops them is the
    // reason the two claims collapse — not the payload. The set is COMPUTED so a
    // newly divergent field surfaces instead of hiding behind a hardcoded list.
    const a = { ...GAP_B_FIXTURES.ledger_all_excluded } as Record<string, unknown>;
    const b = { ...GAP_B_FIXTURES.all_rows_no_ledger } as Record<string, unknown>;
    const differing = Object.keys(a)
      .filter((k) => canonical(a[k]) !== canonical(b[k]))
      .sort();
    expect(differing).toEqual(["denominator", "excluded", "expected"]);
    // Both branches are the SAME counts otherwise: measured = 0 on both, so a
    // consumer that only reads `measured`/`expected` sees 0/0 vs 0/12.
    expect(a.measured).toBe(0);
    expect(b.measured).toBe(0);
  });

  it("canonical digest is frozen (regenerating the consumer fixture is a conscious act)", () => {
    // Over the PAIR ONLY: `state`/`value`/`reason` are rewritten by the evaluator
    // after assembly, so freezing them would fail for reasons the adapter does
    // not own. See GAP_B_FIXTURE_PAIRS.
    const actual = digest(GAP_B_FIXTURE_PAIRS);
    // On failure the diff carries the new digest: replace the constant in this
    // file AND the mirrored constant in the consumer test in ONE commit. The
    // canonical form is emitted alongside so the change is auditable, not guessed.
    expect(actual, `canonical=${canonical(GAP_B_FIXTURE_PAIRS)}`).toBe(GAP_B_FIXTURE_DIGEST);
    expect(actual).toMatch(/^[0-9a-f]{64}$/);
  });

  it("the mirrored digest in the consumer contract test is IDENTICAL", () => {
    // One definition of the fixture, two packages. A copy edited in isolation is
    // how the two sides start asserting different wires.
    const source = readFileSync(CONSUMER_CONTRACT_TEST, "utf8");
    const match = source.match(
      /GAP_B_FIXTURE_DIGEST\s*=\s*\n?\s*"([0-9a-f]{64}|PLACEHOLDER)"/,
    );
    expect(match, "consumer test must declare GAP_B_FIXTURE_DIGEST").not.toBeNull();
    expect(match![1]).toBe(GAP_B_FIXTURE_DIGEST);
  });
});
