/**
 * paper-trade-archiver tests — pure-function assertions (no Redis, no PG).
 *
 * LATLED-01: `detectionToLedgerMs` computes the detection→ledger latency that
 * populates `paper_trade_runs.execution_time_ms`. It must mirror relays-client
 * `detection_to_ledger_ms` (persistence.rs) exactly: elapsed ms, clock-skew
 * clamped to 0, i32 saturation for the INTEGER column.
 *
 * ARBX-R-0001: `archiverRejectionSkip` is the paper-side twin of relays-client
 * `SubmitEngine::rejection_refusal` — the d9 6h JOIN showed 434/434
 * paper_trade_runs rows were REJECTED opps (this archiver inserted them
 * because it only gated on sim-profit presence). The predicate must treat
 * ANY non-null rejection_reason as never-a-trade.
 */
import { describe, it, expect } from "vitest";
import { detectionToLedgerMs, archiverRejectionSkip } from "./paper-trade-archiver.js";

const NOW = Date.parse("2026-08-23T12:00:00.000Z");

describe("detectionToLedgerMs (LATLED-01)", () => {
  it("measures elapsed ms from detected_at to now", () => {
    expect(detectionToLedgerMs("2026-08-23T11:59:58.500Z", NOW)).toBe(1500);
  });

  it("accepts offset-qualified ISO timestamps (zod schema allows offsets)", () => {
    // 12:00Z == 14:00+02:00 → same instant, 0 ms elapsed.
    expect(detectionToLedgerMs("2026-08-23T14:00:00.000+02:00", NOW)).toBe(0);
  });

  it("clock skew (detected_at in the future) clamps to 0 — never negative (R8)", () => {
    expect(detectionToLedgerMs("2026-08-23T12:00:05.000Z", NOW)).toBe(0);
  });

  it("saturates at i32 MAX for the INTEGER column (parity with Rust clamp)", () => {
    const ancient = "2016-08-23T12:00:00.000Z"; // 10 years back ≈ 3.15e11 ms
    expect(detectionToLedgerMs(ancient, NOW)).toBe(2_147_483_647);
  });

  it("defends NaN from unparseable timestamps to 0 (impossible post-zod, defended anyway)", () => {
    expect(detectionToLedgerMs("not-a-date", NOW)).toBe(0);
  });
});

describe("archiverRejectionSkip (ARBX-R-0001)", () => {
  it("a rejected opportunity is never a paper trade — reason returned for the skip log", () => {
    // The exact rejection from the flood: SizeOptimizer verdict copied verbatim,
    // never relabeled.
    expect(archiverRejectionSkip({ rejection_reason: "NegativeNetProfit:gas_floor_breach" })).toBe(
      "NegativeNetProfit:gas_floor_breach",
    );
  });

  it("a viable opportunity (rejection_reason null) proceeds to the insert path", () => {
    expect(archiverRejectionSkip({ rejection_reason: null })).toBe(null);
  });
});
