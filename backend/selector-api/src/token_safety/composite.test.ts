/**
 * composite.test.ts — FASE 2 anti-rug scorer unit tests.
 *
 * Locks the 11-signal composite contract: hard gates force DROP, the positive
 * accumulation reaches SAFE when the strong signals are present, and the parser
 * is defensive (no fabrication on malformed GoPlus fields).
 */
import { describe, it, expect } from "vitest";
import {
  computeComposite,
  parseGoPlusFlags,
  type CompositeInput,
} from "./composite.js";

// Helper: a baseline SAFE-ish input (all strong signals, no penalties).
function safeBaseline(): CompositeInput {
  return {
    is_honeypot: false,
    fake_token: false,
    cannot_sell_all: false,
    is_open_source: true,
    holder_count: 500,
    top_holder_pct: 10,
    liquidity_locked_days: 90,
    ownership_safe: true,
    hidden_owner: false,
    buy_tax_pct: 2,
    sell_tax_pct: 3,
    transfer_pausable: false,
    slippage_modifiable: false,
    token_age_days: 30,
    tvl_usd: 250_000,
  };
}
// Helper: a baseline with everything missing/null (enrichment-pending).
function bareBaseline(): CompositeInput {
  return {
    is_honeypot: false, fake_token: false, cannot_sell_all: false,
    is_open_source: false, holder_count: 0, top_holder_pct: null,
    liquidity_locked_days: null, ownership_safe: false, hidden_owner: false,
    buy_tax_pct: null, sell_tax_pct: null, transfer_pausable: false,
    slippage_modifiable: false, token_age_days: null, tvl_usd: null,
  };
}

describe("computeComposite — hard gates (DROP, score 0)", () => {
  it("honeypot → score 0, DROP, hard_gate='honeypot'", () => {
    const r = computeComposite({ ...safeBaseline(), is_honeypot: true });
    expect(r.score).toBe(0);
    expect(r.classification).toBe("DROP");
    expect(r.hard_gate).toBe("honeypot");
  });

  it("fake_token → score 0, DROP, hard_gate='fake_token'", () => {
    const r = computeComposite({ ...safeBaseline(), fake_token: true });
    expect(r.score).toBe(0);
    expect(r.classification).toBe("DROP");
    expect(r.hard_gate).toBe("fake_token");
  });

  it("cannot_sell_all → score 0, DROP (honeypot-adjacent)", () => {
    const r = computeComposite({ ...safeBaseline(), cannot_sell_all: true });
    expect(r.score).toBe(0);
    expect(r.classification).toBe("DROP");
    expect(r.hard_gate).toBe("cannot_sell_all");
  });

  it("buy_tax > 20% → DROP, hard_gate='tax_gt_20_pct'", () => {
    const r = computeComposite({ ...safeBaseline(), buy_tax_pct: 25 });
    expect(r.classification).toBe("DROP");
    expect(r.hard_gate).toBe("tax_gt_20_pct");
  });

  it("sell_tax > 20% → DROP", () => {
    const r = computeComposite({ ...safeBaseline(), sell_tax_pct: 30 });
    expect(r.classification).toBe("DROP");
    expect(r.hard_gate).toBe("tax_gt_20_pct");
  });

  it("TVL < $10K (known) → DROP, hard_gate='tvl_lt_10k'", () => {
    const r = computeComposite({ ...safeBaseline(), tvl_usd: 5000 });
    expect(r.classification).toBe("DROP");
    expect(r.hard_gate).toBe("tvl_lt_10k");
  });

  it("token_age < 1 day (known) → DROP, hard_gate='age_lt_1d'", () => {
    const r = computeComposite({ ...safeBaseline(), token_age_days: 0.5 });
    expect(r.classification).toBe("DROP");
    expect(r.hard_gate).toBe("age_lt_1d");
  });
});

describe("computeComposite — classification thresholds", () => {
  it("strong signals (open-source + ownership + locked + holders + low tax) → SAFE (>=75)", () => {
    const r = computeComposite(safeBaseline());
    expect(r.classification).toBe("SAFE");
    expect(r.score).toBeGreaterThanOrEqual(75);
    expect(r.paper_shadow_only).toBe(false);
  });

  it("bare input (no signals, enrichment-pending) → DROP (<50)", () => {
    const r = computeComposite(bareBaseline());
    expect(r.classification).toBe("DROP");
    expect(r.score).toBeLessThan(50);
    // age + tvl both enrichment-pending
    expect(r.enrichment_pending).toContain("age");
    expect(r.enrichment_pending).toContain("tvl");
  });

  it("transfer_pausable → -20 penalty vs the same input without it", () => {
    const without = computeComposite(safeBaseline());
    const withPausable = computeComposite({ ...safeBaseline(), transfer_pausable: true });
    expect(withPausable.score).toBe(without.score - 20);
  });

  it("WARN (50..74) → paper_shadow_only=true (never live)", () => {
    // Partial signals: open-source only (+20) + holder_count 50 (+5) + non-slippage (+5) = 30.
    // Need to land in 50..74 — add tax_le_5 (+15) + age (+10) + tvl (+10) = 30+35=65 → WARN.
    const r = computeComposite({
      ...bareBaseline(),
      is_open_source: true,
      holder_count: 50,
      buy_tax_pct: 3,
      sell_tax_pct: 3,
      token_age_days: 10,
      tvl_usd: 60_000,
    });
    expect(r.score).toBeGreaterThanOrEqual(50);
    expect(r.score).toBeLessThan(75);
    expect(r.classification).toBe("WARN");
    expect(r.paper_shadow_only).toBe(true);
  });

  it("deterministic: same input → same score + classification (run twice)", () => {
    const r1 = computeComposite(safeBaseline());
    const r2 = computeComposite(safeBaseline());
    expect(r1.score).toBe(r2.score);
    expect(r1.classification).toBe(r2.classification);
  });

  it("score clamped to [0,100]", () => {
    // Everything bad (but no hard gate): pausable + slippage → strong penalty.
    const r = computeComposite({ ...bareBaseline(), transfer_pausable: true, slippage_modifiable: true });
    expect(r.score).toBeGreaterThanOrEqual(0);
    expect(r.score).toBeLessThanOrEqual(100);
  });
});

describe("computeComposite — sub-signals stored for cache", () => {
  it("sub_signals mirrors the input", () => {
    const input = safeBaseline();
    const r = computeComposite(input);
    expect(r.sub_signals.is_open_source).toBe(true);
    expect(r.sub_signals.holder_count).toBe(500);
    expect(r.sub_signals.buy_tax_pct).toBe(2);
    expect(r.sub_signals.token_age_days).toBe(30);
  });
});

describe("parseGoPlusFlags — defensive parsing", () => {
  it("parses booleans + holder_count + ownership", () => {
    const input = parseGoPlusFlags({
      is_honeypot: "0",
      is_open_source: "1",
      holder_count: "250",
      is_owner_address_multi_token_mode: "1",
      hidden_owner: "0",
    });
    expect(input.is_honeypot).toBe(false);
    expect(input.is_open_source).toBe(true);
    expect(input.holder_count).toBe(250);
    expect(input.ownership_safe).toBe(true);
  });

  it("parses tax as decimal ('0.05' → 5%)", () => {
    const input = parseGoPlusFlags({ buy_tax: "0.05", sell_tax: "0.03" });
    expect(input.buy_tax_pct).toBe(5);
    expect(input.sell_tax_pct).toBe(3);
  });

  it("parses tax as already-pct ('8' → 8%)", () => {
    const input = parseGoPlusFlags({ buy_tax: "8" });
    expect(input.buy_tax_pct).toBe(8);
  });

  it("parses lp_holders JSON → top_holder_pct + liquidity_locked_days", () => {
    const futureLock = Math.floor(Date.now() / 1000) + 60 * 86400; // +60d
    const input = parseGoPlusFlags({
      lp_holders: JSON.stringify([
        { address: "0xaaa", amount: "800", locked_time: futureLock },
        { address: "0xbbb", amount: "200" },
      ]),
    });
    expect(input.top_holder_pct).toBe(80); // 800/1000
    expect(input.liquidity_locked_days).toBeGreaterThan(59);
    expect(input.liquidity_locked_days).toBeLessThan(61);
  });

  it("malformed lp_holders → nulls (no fabrication)", () => {
    const input = parseGoPlusFlags({ lp_holders: "not-json" });
    expect(input.top_holder_pct).toBeNull();
    expect(input.liquidity_locked_days).toBeNull();
  });

  it("age + TVL are ALWAYS null from GoPlus alone (enrichment-pending)", () => {
    const input = parseGoPlusFlags({ token_creation_block: "19000000" });
    expect(input.token_age_days).toBeNull();
    expect(input.tvl_usd).toBeNull();
  });

  it("fake_token field is recognized in both 'fake_token' and 'is_fake_token' forms", () => {
    expect(parseGoPlusFlags({ fake_token: "1" }).fake_token).toBe(true);
    expect(parseGoPlusFlags({ is_fake_token: "1" }).fake_token).toBe(true);
  });
});
