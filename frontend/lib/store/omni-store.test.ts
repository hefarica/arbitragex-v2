import { describe, it, expect } from "vitest";
import { routeKey, mergeOpportunitySnapshots } from "./omni-store";
import type { OmniOpportunity } from "./types";

/**
 * Locks the contract of the snapshot-merge that replaced the old
 * clear+re-add poll path (the root cause of the "same trades flash as new
 * every 5s" UX bug — see systematic-debugging Phase 1).
 *
 * Against the OLD behaviour (clearOpportunities + forEach addOpportunity) every
 * merge_* test below would FAIL: duplicates would accumulate, detected_at would
 * reset each cycle, and an empty fetch would wipe the feed.
 */

function makeOpp(
  over: Partial<OmniOpportunity> &
    Pick<OmniOpportunity, "id" | "token_in" | "token_out" | "dex_a" | "detected_at">,
): OmniOpportunity {
  return {
    chain_id: 1,
    strategy_kind: "dex_arb",
    trace_id: "",
    dex_b: null,
    pair_symbol: null,
    amount_in_wei: "0",
    token_in_info: null,
    token_out_info: null,
    chain_base_token_symbol: null,
    expected_profit_usd: null,
    net_expected_profit_usd: null,
    roi_pct: null,
    risk_score: null,
    status: "detected",
    rejection_reason: null,
    paper_status: null,
    block_number: null,
    chain_id_out: null,
    bridge: null,
    bridge_fee_usd: null,
    chains_used: [],
    dexes_used: [],
    simulated_net_profit_usd: null,
    simulated_amount_in_usd: null,
    simulated_roi_pct: null,
    simulated_cost_breakdown: null,
    simulated_target: null,
    simulated_at: null,
    simulated_notes: null,
    confidence_score_bps: null,
    gas_used: null,
    // PR 4: full trade math (null until scanner wiring PR 4b).
    buy_price_usd: null,
    sell_price_usd: null,
    amount_out_wei: null,
    amount_in_token: null,
    amount_out_token: null,
    amount_in_usd: null,
    amount_out_usd: null,
    start_value_usd: null,
    end_value_usd: null,
    net_roi_pct: null,
    total_fees_usd: null,
    pool_buy: null,
    pool_sell: null,
    ...over,
  };
}

describe("routeKey", () => {
  it("is stable across address-case variations (API may emit either casing)", () => {
    const a = makeOpp({ id: "1", token_in: "0xABCDEF", token_out: "0x123456", dex_a: "Uniswap", detected_at: "2026-01-01T00:00:00Z" });
    const b = makeOpp({ id: "2", token_in: "0xabcdef", token_out: "0x123456", dex_a: "uniswap", detected_at: "2026-01-01T00:00:00Z" });
    expect(routeKey(a)).toBe(routeKey(b));
  });

  it("differs per chain (same tokens/dex, different chain_id)", () => {
    const a = makeOpp({ id: "1", chain_id: 1, token_in: "0xabc", token_out: "0xdef", dex_a: "u", detected_at: "2026-01-01T00:00:00Z" });
    const b = makeOpp({ id: "2", chain_id: 137, token_in: "0xabc", token_out: "0xdef", dex_a: "u", detected_at: "2026-01-01T00:00:00Z" });
    expect(routeKey(a)).not.toBe(routeKey(b));
  });

  it("differs per strategy_kind (dex_arb vs flashloan_arb on the same pair)", () => {
    const a = makeOpp({ id: "1", strategy_kind: "dex_arb", token_in: "0xabc", token_out: "0xdef", dex_a: "u", detected_at: "2026-01-01T00:00:00Z" });
    const b = makeOpp({ id: "2", strategy_kind: "flashloan_arb", token_in: "0xabc", token_out: "0xdef", dex_a: "u", detected_at: "2026-01-01T00:00:00Z" });
    expect(routeKey(a)).not.toBe(routeKey(b));
  });
});

describe("mergeOpportunitySnapshots", () => {
  const T0 = "2026-01-01T00:00:00Z";
  const T1 = "2026-01-01T00:00:05Z";
  const T2 = "2026-01-01T00:00:10Z";

  it("returns the existing list unchanged when incoming is empty (a momentary empty fetch must NOT wipe the feed)", () => {
    const existing = [makeOpp({ id: "a", token_in: "0x1", token_out: "0x2", dex_a: "u", detected_at: T0 })];
    expect(mergeOpportunitySnapshots(existing, [])).toBe(existing);
  });

  it("appends a genuinely-new route and sorts newest detected_at first", () => {
    const existing = [makeOpp({ id: "old", token_in: "0x1", token_out: "0x2", dex_a: "u", detected_at: T0 })];
    const incoming = [
      makeOpp({ id: "old-fresh", token_in: "0x1", token_out: "0x2", dex_a: "u", detected_at: T0 }),
      makeOpp({ id: "new", token_in: "0x3", token_out: "0x4", dex_a: "u", detected_at: T2 }),
    ];
    const merged = mergeOpportunitySnapshots(existing, incoming);
    expect(merged).toHaveLength(2);
    expect(merged[0]?.id).toBe("new"); // T2 newest
    expect(merged[1]?.id).toBe("old"); // preserved id (route matched)
  });

  it("updates a re-detected route IN PLACE: preserves detected_at + id, refreshes metrics, NO duplicate", () => {
    const existing = [
      makeOpp({ id: "orig", token_in: "0x1", token_out: "0x2", dex_a: "u", detected_at: T0, expected_profit_usd: 10 }),
    ];
    // Same route re-detected 5s later with a NEW id + NEW profit + NEW detected_at.
    const incoming = [
      makeOpp({ id: "fresh-detector", token_in: "0x1", token_out: "0x2", dex_a: "u", detected_at: T1, expected_profit_usd: 99 }),
    ];
    const merged = mergeOpportunitySnapshots(existing, incoming);
    expect(merged).toHaveLength(1); // NOT 2 — deduped by route
    expect(merged[0]?.id).toBe("orig"); // id preserved ⇒ stable React key ⇒ no remount/flash
    expect(merged[0]?.detected_at).toBe(T0); // age preserved (continuous, no reset)
    expect(merged[0]?.expected_profit_usd).toBe(99); // metric refreshed from latest detection
  });

  it("keeps routes no longer in the snapshot (they age out visually, not silently deleted)", () => {
    const existing = [
      makeOpp({ id: "gone", token_in: "0x1", token_out: "0x2", dex_a: "u", detected_at: T0 }),
      makeOpp({ id: "stay", token_in: "0x3", token_out: "0x4", dex_a: "u", detected_at: T1 }),
    ];
    // Only the "stay" route is re-detected; "gone" is absent from the snapshot.
    const incoming = [makeOpp({ id: "stay-fresh", token_in: "0x3", token_out: "0x4", dex_a: "u", detected_at: T2 })];
    const merged = mergeOpportunitySnapshots(existing, incoming);
    expect(merged).toHaveLength(2); // stay (updated) + gone (kept)
    const ids = merged.map((m) => m.id);
    expect(ids).toContain("gone");
    expect(ids).toContain("stay"); // preserved id (route matched)
    expect(merged[0]?.id).toBe("stay"); // T1 (preserved) newer than T0
  });

  it("respects the cap (oldest beyond cap dropped after sort)", () => {
    const existing: OmniOpportunity[] = [];
    // 3 incoming routes at T0, T1, T2
    const incoming = [
      makeOpp({ id: "a", token_in: "0x1", token_out: "0x2", dex_a: "u", detected_at: T0 }),
      makeOpp({ id: "b", token_in: "0x3", token_out: "0x4", dex_a: "u", detected_at: T1 }),
      makeOpp({ id: "c", token_in: "0x5", token_out: "0x6", dex_a: "u", detected_at: T2 }),
    ];
    const merged = mergeOpportunitySnapshots(existing, incoming, 2);
    expect(merged).toHaveLength(2);
    expect(merged.map((m) => m.id)).toEqual(["c", "b"]); // newest 2 kept, oldest (a) dropped
  });

  it("does not double-count a route present in both existing and incoming", () => {
    const route = { token_in: "0x1", token_out: "0x2", dex_a: "u" };
    const existing = [makeOpp({ id: "e1", ...route, detected_at: T0 })];
    const incoming = [makeOpp({ id: "i1", ...route, detected_at: T1 })];
    const merged = mergeOpportunitySnapshots(existing, incoming);
    expect(merged).toHaveLength(1);
    expect(merged[0]?.id).toBe("e1");
    expect(merged[0]?.detected_at).toBe(T0);
  });
});

// =============================================================================
// Strategy "must be profitable" HARD filter (operator decision 2026-07-05):
// an opp that is net-negative OR backend-tagged non_positive NEVER renders,
// toggle-independent. The toggle controls only OTHER-reason rejections.
// =============================================================================
describe("mergeOpportunitySnapshots — strategy non-positive hard filter", () => {
  const T = "2026-01-01T00:00:00Z";
  const route = { token_in: "0xa", token_out: "0xb", dex_a: "u" };

  it("drops an incoming opp with negative canonical net", () => {
    const merged = mergeOpportunitySnapshots(
      [],
      [makeOpp({ id: "n1", ...route, detected_at: T, net_expected_profit_usd: -5 })],
    );
    expect(merged).toHaveLength(0);
  });

  it("drops an incoming opp with negative simulated net when canonical is null (fallback priority)", () => {
    const merged = mergeOpportunitySnapshots(
      [],
      [makeOpp({ id: "n2", ...route, detected_at: T, net_expected_profit_usd: null, simulated_net_profit_usd: -2 })],
    );
    expect(merged).toHaveLength(0);
  });

  it("drops EVERY backend non_positive_* rejection tag (substring covers all ~8 variants)", () => {
    const tags = [
      "non_positive_profit",
      "non_positive_gross_usd",
      "non_positive_net_usd",
      "revm_net_profit_non_positive",
      "net_usd_non_positive",
      "net_profit_non_positive",
      "multistep_gross_spread_non_positive",
      "amount_non_positive",
    ];
    for (const reason of tags) {
      const merged = mergeOpportunitySnapshots(
        [],
        [makeOpp({ id: `tag-${reason}`, ...route, detected_at: T, status: "rejected", rejection_reason: reason, net_expected_profit_usd: null, simulated_net_profit_usd: null })],
      );
      expect(merged, `expected drop for rejection_reason="${reason}"`).toHaveLength(0);
    }
  });

  it("KEEPS an opp with net == null on BOTH fields (cold-start, R8: null ≠ zero)", () => {
    const merged = mergeOpportunitySnapshots(
      [],
      [makeOpp({ id: "c1", ...route, detected_at: T, net_expected_profit_usd: null, simulated_net_profit_usd: null })],
    );
    expect(merged).toHaveLength(1);
  });

  it("DROPS an opp with net == 0 (break-even is NOT above 0 — rule is strictly 'show > 0')", () => {
    const merged = mergeOpportunitySnapshots(
      [],
      [makeOpp({ id: "z1", ...route, detected_at: T, net_expected_profit_usd: 0 })],
    );
    expect(merged).toHaveLength(0);
  });

  it("KEEPS an opp rejected for a NON-strategy reason with positive net (toggle controls, not the hard filter)", () => {
    const merged = mergeOpportunitySnapshots(
      [],
      [makeOpp({ id: "o1", ...route, detected_at: T, status: "rejected", rejection_reason: "token_meta_unavailable", net_expected_profit_usd: 4.5 })],
    );
    expect(merged).toHaveLength(1);
  });

  it("KEEPS an opp rejected for below-min-profit-floor with positive net (floor is a separate sizing concern)", () => {
    const merged = mergeOpportunitySnapshots(
      [],
      [makeOpp({ id: "o2", ...route, detected_at: T, status: "rejected", rejection_reason: "below_min_profit", net_expected_profit_usd: 3 })],
    );
    expect(merged).toHaveLength(1);
  });

  it("flip case: existing POSITIVE route whose latest snapshot flipped NEGATIVE DISAPPEARS (not retained as stale positive)", () => {
    const existing = [makeOpp({ id: "f1", ...route, detected_at: T, net_expected_profit_usd: 5 })];
    const incoming = [makeOpp({ id: "f1-flip", ...route, detected_at: T, net_expected_profit_usd: -3 })];
    const merged = mergeOpportunitySnapshots(existing, incoming);
    expect(merged).toHaveLength(0);
  });

  it("hard filter fires for a positive existing route that a later non-positive incoming retires, AND a different positive route is preserved", () => {
    const routeB = { token_in: "0xc", token_out: "0xd", dex_a: "u" };
    const existing: OmniOpportunity[] = [];
    // First snapshot: route positive, routeB positive.
    let merged = mergeOpportunitySnapshots(existing, [
      makeOpp({ id: "a1", ...route, detected_at: T, net_expected_profit_usd: 5 }),
      makeOpp({ id: "b1", ...routeB, detected_at: T, net_expected_profit_usd: 8 }),
    ]);
    expect(merged).toHaveLength(2);
    // Second snapshot: route flipped negative (must disappear), routeB still positive.
    merged = mergeOpportunitySnapshots(merged, [
      makeOpp({ id: "a2", ...route, detected_at: T, net_expected_profit_usd: -1 }),
      makeOpp({ id: "b2", ...routeB, detected_at: T, net_expected_profit_usd: 9 }),
    ]);
    expect(merged).toHaveLength(1);
    expect(merged[0]?.token_in).toBe("0xc"); // routeB survived, route gone
  });
});
