import { describe, it, expect } from "vitest";
import { scoreInternal, canonicalSafetyRecord } from "./internal_heuristic.js";
import { normalizeAddress } from "./cache.js";

describe("scoreInternal", () => {
  it("invalid address → 0", () => {
    const r = scoreInternal(1, "not-an-address", 3600, 86400);
    expect(r.safety_score).toBe(0);
    expect(r.flags).toMatchObject({ reason: "invalid_address_format" });
    expect(r.source).toBe("internal");
  });
  it("zero address → 0", () => {
    const r = scoreInternal(1, "0x0000000000000000000000000000000000000000", 3600, 86400);
    expect(r.safety_score).toBe(0);
  });
  it("suspicious prefix → 10", () => {
    const r = scoreInternal(1, "0xdeadbeef000000000000000000000000000000aa", 3600, 86400);
    expect(r.safety_score).toBe(10);
  });
  it("non-canonical normal address → 50 neutral (below floor)", () => {
    // A structurally-valid address that is NOT a known blue-chip: stays neutral
    // 50 and is correctly rejected by the default floor (70). Lowercasing still
    // applies. (WETH was moved out of this path — see canonical suite below.)
    const r = scoreInternal(1, "0x1234567890abcdef1234567890abcdef12345678", 3600, 86400);
    expect(r.safety_score).toBe(50);
    expect(r.safety_score).toBeLessThan(70);
    expect(r.source).toBe("internal");
    expect(r.token_address).toBe("0x1234567890abcdef1234567890abcdef12345678");
    expect(r.flags).toMatchObject({ reason: "internal_heuristic_neutral" });
  });
  it("canonical mainnet USDC → clears floor, reason canonical_verified", () => {
    const r = scoreInternal(1, "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48", 3600, 86400, 70);
    expect(r.safety_score).toBe(95);
    expect(r.safety_score).toBeGreaterThanOrEqual(70);
    expect(r.flags).toMatchObject({ reason: "canonical_verified" });
    expect(r.source).toBe("internal");
    expect(r.token_address).toBe("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48");
  });
  it("canonical mainnet WETH → clears floor", () => {
    const r = scoreInternal(1, "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2", 3600, 86400, 70);
    expect(r.safety_score).toBe(95);
    expect(r.flags).toMatchObject({ reason: "canonical_verified" });
  });
  it("canonical clears ANY floor (max(95, threshold)) — recurrence guard", () => {
    // If an operator raises the floor to 98, canonical must still clear it.
    // Without max(95,floor) the 0-opportunities bug would silently return.
    const r = scoreInternal(1, "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48", 3600, 86400, 98);
    expect(r.safety_score).toBeGreaterThanOrEqual(98);
  });
  it("canonical scoping is mainnet-only (chain 137 → still neutral 50)", () => {
    // Same hex address on a non-mainnet chain must NOT be trusted.
    const r = scoreInternal(137, "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48", 3600, 86400, 70);
    expect(r.safety_score).toBe(50);
    expect(r.flags).toMatchObject({ reason: "internal_heuristic_neutral" });
  });
});

describe("canonicalSafetyRecord (pre-cache path)", () => {
  // checkToken calls this BEFORE the cache read; its null-contract is what lets
  // unknown tokens fall through to the cache/heuristic. Lock it down.
  it("canonical mainnet USDC → record clearing floor", () => {
    const r = canonicalSafetyRecord(1, "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48", 3600, 70);
    expect(r).not.toBeNull();
    expect(r!.safety_score).toBeGreaterThanOrEqual(70);
    expect(r!.flags).toMatchObject({ reason: "canonical_verified" });
    expect(r!.source).toBe("internal");
  });
  it("non-mainnet chain → null (do not trust same address off-mainnet)", () => {
    expect(canonicalSafetyRecord(137, "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48", 3600, 70)).toBeNull();
  });
  it("unknown address → null (falls through to cache/heuristic)", () => {
    expect(canonicalSafetyRecord(1, "0x1234567890abcdef1234567890abcdef12345678", 3600, 70)).toBeNull();
  });
  it("invalid address → null (does not throw)", () => {
    expect(canonicalSafetyRecord(1, "not-an-address", 3600, 70)).toBeNull();
  });
});

describe("normalizeAddress", () => {
  it("lowercases valid addresses", () => {
    expect(normalizeAddress("0xDeaDbeEf00000000000000000000000000000001"))
      .toBe("0xdeadbeef00000000000000000000000000000001");
  });
  it("throws on invalid format", () => {
    expect(() => normalizeAddress("not-valid")).toThrow();
    expect(() => normalizeAddress("0x123")).toThrow();
  });
});
