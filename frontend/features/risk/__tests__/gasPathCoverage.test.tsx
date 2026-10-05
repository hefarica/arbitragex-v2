/**
 * F3 regression — gas-burn deciding-path coverage wording.
 *
 * Defect reproduced here (A.5/A.6 audit, 2026-10): the wire sends
 * `paths.actual.measured = w?.withActualGas ?? 0` and `.expected = w?.rowsInWindow ?? 0`,
 * so an ABSENT ledger window and an empty one arrive as `0/0`. The row printed
 *
 *     "actual 0/0 measured (NOT_AVAILABLE)"
 *
 * which reads as a measurement of zero — the exact conflation R8 forbids
 * ("absent ≠ zero", "gas burn undefined, not $0"). The label claimed a
 * measurement where the wire carried none.
 *
 * Contract asserted below (Frontend criterion: absence, zero, partial coverage
 * and not-evaluated must be distinguishable in the DOM):
 *   · expected = 0            → coverage absent, explicitly NOT evidence of $0
 *   · measured = 0, N > 0     → "0/N rows measured" (a real, different fact)
 *   · 0 < measured < expected → partial coverage, spelled out
 *   · measured = expected > 0 → plain coverage, no "partial"
 *   · state = null            → "not evaluated" (never the ambiguous "off")
 *   · sim measurements = 0    → "no measurements" (no "0 measurement(s)" claim)
 *   · the token "measured" is never attached to an empty denominator.
 */
import React from "react";
import { describe, it, expect } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";
import { DecidingPathRow } from "../RiskCircuitPanel";
import type { CircuitBreakerEvidence } from "@/lib/schemas";

function ev(p: {
  deciding: "actual" | "sim" | "none";
  simState?: string | null;
  simMeasurements?: number;
  actualState?: string | null;
  measured: number;
  expected: number;
}): CircuitBreakerEvidence {
  return {
    source: "paper_ledger",
    detail: "test fixture detail",
    current_value: null,
    threshold: 50,
    unit: "USD",
    deciding_path: p.deciding,
    paths: {
      sim: {
        state: p.simState ?? "PASS",
        value: 1.25,
        measurements: p.simMeasurements ?? 7,
        window_rows: 7,
        window_secs: 3600,
        scope: "chain:1",
        reason: "sim reason",
      },
      actual: {
        state: p.actualState === undefined ? "PASS" : p.actualState,
        value: 2.5,
        measured: p.measured,
        expected: p.expected,
        window_hours: 24,
        scope: "chain:1",
        provenance: "actual_gas_cost_usd on paper_trade_runs",
        reason: "actual reason",
      },
    },
  } as CircuitBreakerEvidence;
}

const html = (e: CircuitBreakerEvidence) =>
  renderToStaticMarkup(<DecidingPathRow id="gas_burn_breaker" evidence={e} />);

describe("DecidingPathRow — absent is not zero (F3)", () => {
  it("renders the row under its stable testid, with sim/actual as term/definition pairs", () => {
    const h = html(ev({ deciding: "actual", measured: 5, expected: 5 }));
    expect(h).toContain('data-testid="breaker-deciding-path-gas_burn_breaker"');
    expect(h).toMatch(/<dt[^>]*>sim<\/dt>/);
    expect(h).toMatch(/<dt[^>]*>actual<\/dt>/);
  });

  it("ABSENT/EMPTY window: never claims a measurement and says the absence is not $0", () => {
    const h = html(ev({ deciding: "actual", actualState: "NOT_AVAILABLE", measured: 0, expected: 0 }));
    expect(h).not.toMatch(/0\/0\s*measured/);
    expect(h).toMatch(/coverage absent/i);
    expect(h).toMatch(/not evidence of \$0 gas/i);
    // Exact rendered label (DOM artefact for the honesty contract): the absence
    // case must read as uncertainty, never as a measured zero.
    expect(h).toContain(
      "coverage absent — no ledger rows in the window; not evidence of $0 gas (NOT_AVAILABLE)",
    );
  });

  it("ZERO OF N: stays distinguishable from absent (rows exist, none measured)", () => {
    const h = html(ev({ deciding: "actual", actualState: "NOT_AVAILABLE", measured: 0, expected: 12 }));
    expect(h).toMatch(/0\/12 rows measured/);
    expect(h).toMatch(/none carry actual gas/i);
    expect(h).not.toMatch(/coverage absent/i);
  });

  it("PARTIAL coverage is spelled out, not rounded into a clean pass", () => {
    const h = html(ev({ deciding: "actual", actualState: "WARN", measured: 3, expected: 10 }));
    expect(h).toMatch(/3\/10 rows measured/);
    expect(h).toMatch(/partial coverage/i);
  });

  it("FULL coverage renders no 'partial' noise", () => {
    const h = html(ev({ deciding: "actual", actualState: "PASS", measured: 5, expected: 5 }));
    expect(h).toMatch(/5\/5 rows measured/);
    expect(h).not.toMatch(/partial coverage/i);
  });

  it("state = null reads 'not evaluated', never the ambiguous 'off'", () => {
    const h = html(ev({ deciding: "sim", actualState: null, measured: 4, expected: 4 }));
    expect(h).toMatch(/not evaluated/i);
    expect(h).not.toMatch(/\(off\)/);
  });

  it("sim path with 0 measurements does not present them as measurements", () => {
    const h = html(ev({ deciding: "sim", simMeasurements: 0, simState: "NOT_AVAILABLE", measured: 0, expected: 0 }));
    expect(h).toMatch(/no measurements/i);
    expect(h).not.toMatch(/0 measurement/);
  });

  it("ADVERSARIAL: a negative/invalid count is surfaced, never rounded into a fabricated 0/N", () => {
    const h = html(ev({ deciding: "actual", actualState: "NOT_AVAILABLE", measured: -3, expected: 12 }));
    expect(h).toMatch(/counts unusable/i);
    expect(h).toMatch(/measured=-3/);
    expect(h).not.toMatch(/0\/12 rows measured/);
  });

  it("ADVERSARIAL: measured > expected is flagged as a contradiction, not printed as coverage", () => {
    const h = html(ev({ deciding: "actual", actualState: "PASS", measured: 9, expected: 4 }));
    expect(h).toMatch(/exceeds the window/i);
    expect(h).toMatch(/9\/4/);
  });

  it("FORWARD CONTRACT: null counts (absent window sent as null, not 0) are never rendered as zero", () => {
    const e = ev({ deciding: "actual", actualState: "NOT_AVAILABLE", measured: 0, expected: 0 });
    e.paths!.actual.measured = null as unknown as number;
    e.paths!.actual.expected = null as unknown as number;
    const h = renderToStaticMarkup(<DecidingPathRow id="gas_burn_breaker" evidence={e} />);
    expect(h).toMatch(/coverage counts absent on wire/i);
    expect(h).not.toMatch(/0\/0/);
    expect(h).not.toMatch(/counts unusable/);
  });
});
