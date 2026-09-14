import { describe, expect, it } from "vitest";
import { scoringCountLimit, readScoringSummary, type ScoringReader } from "./scoring-summary.js";

describe("bounded scoring observation contract (unit inputs, not live evidence)", () => {
  it("never caps below the configured calibration threshold", () => {
    expect(scoringCountLimit(100)).toBe(1000);
    expect(scoringCountLimit(2000)).toBe(2000);
    expect(scoringCountLimit(2000.1)).toBe(2001);
  });
  for (const invalid of [-1, NaN, Infinity, Number.MAX_SAFE_INTEGER]) {
    it(`rejects a non-representable threshold ${invalid}`, () => {
      expect(() => scoringCountLimit(invalid)).toThrow("scoring_threshold_invalid");
    });
  }
  for (const total of [-1, "nonsense", 1002]) {
    it(`does not turn invalid SQL count ${total} into zero`, async () => {
      const db = { query: async () => ({ rows: [{ total, first: null, last: null }] }) } as unknown as ScoringReader;
      await expect(readScoringSummary(db, 100)).rejects.toThrow("scoring_summary_count_invalid");
    });
  }
  it("does not interpret missing bounds as an empty ledger", async () => {
    const db = { query: async () => ({ rows: [{ total: "1", first: null, last: null }] }) } as unknown as ScoringReader;
    await expect(readScoringSummary(db, 100)).rejects.toThrow("scoring_summary_bounds_invalid");
  });
});
