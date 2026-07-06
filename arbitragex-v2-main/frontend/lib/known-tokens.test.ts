// Pure-logic tests for the TokenIcon cascade tiers 1+2 (known map + cache +
// address normalization). These are the synchronous, network-free units the
// `useTokenIcon` hook resolves before ever touching the API. Node env (no DOM).

import { afterEach, describe, expect, it } from "vitest";

import {
  _resetIconCacheForTests,
  getCachedIcon,
  isValidEvmAddress,
  knownSymbol,
  normalizeAddress,
  resolveKnownIcon,
  setCachedIcon,
} from "./known-tokens";

const WETH_CHECKSUM = "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2";
const WETH_LOWER = "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2";
const UNKNOWN = "0x1111111111111111111111111111111111111111";

afterEach(() => {
  _resetIconCacheForTests();
});

describe("normalizeAddress", () => {
  it("lowercases + trims a valid address", () => {
    expect(normalizeAddress(`  ${WETH_CHECKSUM}  `)).toBe(WETH_LOWER);
  });
  it("rejects malformed input", () => {
    expect(normalizeAddress("0xnotvalid")).toBeNull();
    expect(normalizeAddress("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2")).toBeNull(); // no 0x
    expect(normalizeAddress("0x123")).toBeNull(); // too short
    expect(normalizeAddress(`${WETH_LOWER}ff`)).toBeNull(); // too long
    expect(normalizeAddress(null)).toBeNull();
    expect(normalizeAddress(undefined)).toBeNull();
    expect(normalizeAddress("")).toBeNull();
  });
  it("isValidEvmAddress agrees", () => {
    expect(isValidEvmAddress(WETH_CHECKSUM)).toBe(true);
    expect(isValidEvmAddress("0xnope")).toBe(false);
  });
});

describe("resolveKnownIcon", () => {
  it("resolves a known token regardless of address case", () => {
    const r = resolveKnownIcon(1, WETH_CHECKSUM);
    expect(r).not.toBeNull();
    expect(r?.source).toBe("known");
    expect(r?.isFallback).toBe(false);
    expect(r?.iconUrl).toMatch(/^https?:\/\//);
  });
  it("returns null for an unknown token", () => {
    expect(resolveKnownIcon(1, UNKNOWN)).toBeNull();
  });
  it("returns null for an invalid address", () => {
    expect(resolveKnownIcon(1, "0xbad")).toBeNull();
  });
  it("returns null on a chain with no known set", () => {
    expect(resolveKnownIcon(999, WETH_CHECKSUM)).toBeNull();
  });
  it("knownSymbol resolves the canonical symbol", () => {
    expect(knownSymbol(1, WETH_LOWER)).toBe("WETH");
    expect(knownSymbol(1, UNKNOWN)).toBeNull();
  });
});

describe("in-memory icon cache", () => {
  it("stores and retrieves a resolution per chainId+address", () => {
    expect(getCachedIcon(1, UNKNOWN)).toBeUndefined();
    setCachedIcon(1, UNKNOWN, {
      iconUrl: "https://x/y.png",
      source: "dexscreener",
      isFallback: false,
    });
    expect(getCachedIcon(1, UNKNOWN)?.iconUrl).toBe("https://x/y.png");
    // different chain → independent
    expect(getCachedIcon(10, UNKNOWN)).toBeUndefined();
  });
  it("reset clears the cache", () => {
    setCachedIcon(1, UNKNOWN, {
      iconUrl: "https://x/y.png",
      source: "redis",
      isFallback: false,
    });
    _resetIconCacheForTests();
    expect(getCachedIcon(1, UNKNOWN)).toBeUndefined();
  });
});
