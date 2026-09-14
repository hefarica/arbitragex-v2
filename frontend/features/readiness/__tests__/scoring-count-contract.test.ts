/** Constructed schema-boundary inputs, not recorded production observations. */
import { describe, expect, it } from "vitest";
import { ScoringStatusResponseSchema } from "@/lib/schemas";
import { formatScoredCount } from "../ConfidenceScoringPanel";
const legacy = {
  generated_at: "2026-09-14T00:00:00Z", source: "runtime_evidence", mode: "paper_only",
  scoring_status: "partial", scoring_enabled: false, bayesian_filter_wired: true,
  kelly_sizing_wired: true, vpin_wired: true, scoring_pipeline_wired: false,
  components: [], confidence_threshold_bps: 7000, min_expected_value_wei: null,
  scoring_version: "0.2.0-evidence", recent_scored_count: 0, last_scored_at: null,
  blocked_reasons: [], available_decisions: [], live_trading: false,
  private_relay: false, submit_enabled: false, capital_exposure_usd: 0, next_action: "test input",
};
const bounded = { ...legacy, scoring_version: "0.3.0-bounded-evidence", recent_scored_count: 1000,
  recent_scored_count_exact: false, scoring_count_limit: 1000 };
describe("scoring wire precision and presentation", () => {
  it("accepts unchanged legacy exact-count responses", () => {
    expect(ScoringStatusResponseSchema.safeParse(legacy).success).toBe(true);
    expect(formatScoredCount(42, undefined)).toBe("42");
  });
  it("accepts and visibly distinguishes a proven lower bound", () => {
    expect(ScoringStatusResponseSchema.safeParse(bounded).success).toBe(true);
    expect(formatScoredCount(1000, false)).toBe("≥ 1000");
  });
  for (const n of [0, 1, 1000]) {
    it(`accepts an exact bounded population ${n}`, () => {
      expect(ScoringStatusResponseSchema.safeParse({ ...bounded, recent_scored_count: n, recent_scored_count_exact: true }).success).toBe(true);
      expect(formatScoredCount(n, true)).toBe(String(n));
    });
  }
  for (const patch of [
    { recent_scored_count_exact: undefined }, { scoring_count_limit: undefined },
    { recent_scored_count: 999 }, { recent_scored_count: 1001 },
    { recent_scored_count_exact: "false" }, { scoring_count_limit: 0 },
  ]) {
    it(`rejects misleading bounded metadata ${JSON.stringify(patch)}`, () => {
      expect(ScoringStatusResponseSchema.safeParse({ ...bounded, ...patch }).success).toBe(false);
    });
  }
  it("a new-version response cannot omit all precision metadata", () => {
    expect(ScoringStatusResponseSchema.safeParse({ ...legacy, scoring_version: bounded.scoring_version }).success).toBe(false);
  });
});
