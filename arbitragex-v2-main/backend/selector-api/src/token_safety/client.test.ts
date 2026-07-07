import { describe, it, expect } from "vitest";
import { scoreInternal } from "./internal_heuristic.js";
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
  it("normal address → 50 neutral", () => {
    const r = scoreInternal(1, "0xC02aaa39b223FE8D0A0e5C4F27eAD9083C756Cc2", 3600, 86400);
    expect(r.safety_score).toBe(50);
    expect(r.source).toBe("internal");
    expect(r.token_address).toBe("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2");
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
