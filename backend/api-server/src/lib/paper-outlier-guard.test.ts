/**
 * A-02 — Unit tests for the paper-archiver outlier guard.
 *
 * Guards: implausible sim_expected_profit_usd (token-as-USD from unsized emit
 * paths — Rhai profit_usd_hint, backrun/cex_dex placeholders) is quarantined
 * instead of contaminating the paper-history average ($10M fictitious). The
 * threshold scales with the operator's capital.
 */
import { describe, it, expect } from "vitest";
import { isOutlierProfit } from "../lib/paper-outlier-guard.js";

describe("A-02 isOutlierProfit (paper ledger outlier guard)", () => {
  it("rejects the observed $49–59M outliers on a $1k capital (token-as-USD)", () => {
    expect(isOutlierProfit(49_566_371.15, 1000)).toBe(true);
    expect(isOutlierProfit(59_784_315.65, 1000)).toBe(true);
    expect(isOutlierProfit(10_131_136.49, 1000)).toBe(true); // the reported avg
  });

  it("accepts plausible profits within 10× capital", () => {
    expect(isOutlierProfit(50, 1000)).toBe(false);
    expect(isOutlierProfit(500, 1000)).toBe(false);
    expect(isOutlierProfit(9999, 1000)).toBe(false); // just under 10×
  });

  it("rejects at exactly the threshold boundary (> mult × cap, not >=)", () => {
    expect(isOutlierProfit(10_000, 1000)).toBe(false); // == 10×, not over
    expect(isOutlierProfit(10_000.01, 1000)).toBe(true); // just over
  });

  it("rejects large negative values too (absolute value)", () => {
    expect(isOutlierProfit(-49_566_371.15, 1000)).toBe(true);
    expect(isOutlierProfit(-50, 1000)).toBe(false);
  });

  it("falls back to the capital floor when capitalUsd <= 0", () => {
    expect(isOutlierProfit(49_566_371.15, 0)).toBe(true); // floor 1000 → 10k threshold
    expect(isOutlierProfit(50, 0)).toBe(false);
    expect(isOutlierProfit(50, -5)).toBe(false); // negative capital → floor
  });

  it("respects a custom multiplier and floor", () => {
    // 5× multiplier, $2000 floor → threshold 10000
    expect(isOutlierProfit(9999, 0, 5, 2000)).toBe(false);
    expect(isOutlierProfit(10_001, 0, 5, 2000)).toBe(true);
    // capital overrides floor when > 0
    expect(isOutlierProfit(4999, 1000, 5, 2000)).toBe(false); // 5 × 1000 = 5000
    expect(isOutlierProfit(5001, 1000, 5, 2000)).toBe(true);
  });

  it("rejects non-finite values (NaN / Infinity)", () => {
    expect(isOutlierProfit(Number.NaN, 1000)).toBe(true);
    expect(isOutlierProfit(Number.POSITIVE_INFINITY, 1000)).toBe(true);
    expect(isOutlierProfit(Number.NEGATIVE_INFINITY, 1000)).toBe(true);
  });

  it("distribution sanity: a realistic window of profits all pass", () => {
    // p99 of a sane paper-shadow run on $1k capital is well under 10× cap.
    const profits = [1.2, 3.4, 8.9, 12.5, 27.8, 45.0, 91.3, 150.0, 280.0, 510.0];
    const quarantined = profits.filter((p) => isOutlierProfit(p, 1000));
    expect(quarantined).toEqual([]);
  });
});
