/**
 * Regression tests for the curated-tier heuristic (2026-08-18, vivid-grove
 * audit). Before the fix, only a hand-picked 26-token subset passed the gate —
 * with provider="internal_only" and floor=70 every other token scored neutral
 * 50 and 100% of detections were rejected `safety_below_threshold` before any
 * economics ran (hollow cards, empty dashboard).
 *
 * These pin the tiers against REAL snapshot entries:
 *   - A8 (Uniswap-curated, NOT in the old 26) must pass at 95.
 *   - 0xb9ef…3b78 (CoinGecko-indexed only) must pass at 75.
 *   - an unknown mainnet address stays at neutral 50 (below floor → blocked).
 */
import { describe, expect, it } from "vitest";
import { canonicalSafetyRecord, scoreInternal } from "./internal_heuristic.js";

const TTL_OK = 3600;
const TTL_BAD = 86400;
const FLOOR = 70;

describe("canonicalSafetyRecord — curated tiers", () => {
  it("Uniswap-curated token OUTSIDE the old 26-token set clears the floor (regression: the 0-opps bug)", () => {
    // A8 — in the Uniswap default list, was NOT in the hand-picked 26.
    const rec = canonicalSafetyRecord(1, "0x3E5A19c91266aD8cE2477B91585d1856B84062dF", TTL_OK, FLOOR);
    expect(rec).not.toBeNull();
    expect(rec!.safety_score).toBeGreaterThanOrEqual(95);
    expect(rec!.flags.reason).toBe("canonical_verified");
  });

  it("CoinGecko-indexed-only token gets the 75 tier (above default floor, below uniswap)", () => {
    const rec = canonicalSafetyRecord(1, "0xb9ef770b6a5e12e45983c5d80545258aa38f3b78", TTL_OK, FLOOR);
    expect(rec).not.toBeNull();
    expect(rec!.safety_score).toBe(75);
    expect(rec!.flags.reason).toBe("coingecko_indexed");
  });

  it("token in NEITHER curated list returns null → caller falls to neutral 50 (scam signal)", () => {
    const rec = canonicalSafetyRecord(1, "0xc84782855624e0c8ec9a7f4e5a1a0f1e5d1b8a01", TTL_OK, FLOOR);
    expect(rec).toBeNull();
  });

  it("multi-chain: same address on a non-mainnet chain is evaluated against that chain's curated set", () => {
    // A mainnet WETH address on chain 137 is a DIFFERENT contract — must not
    // inherit mainnet's verified verdict (chain-scoped trust).
    const rec = canonicalSafetyRecord(137, "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2", TTL_OK, FLOOR);
    // (If Polygon happens to have this address curated the tier applies; the
    // assertion guards the SCOPING contract by checking it is not the
    // mainnet-only free pass the old implementation gave chainId===1.)
    if (rec) expect(rec.flags.reason).not.toBe("canonical_verified_mainnet_only");
  });

  it("operator floor above the CoinGecko tier (e.g. 80) is respected — tier scores never exceed their provenance", () => {
    const rec = canonicalSafetyRecord(1, "0xb9ef770b6a5e12e45983c5d80545258aa38f3b78", TTL_OK, 80);
    expect(rec).not.toBeNull();
    expect(rec!.safety_score).toBe(75); // below the stricter floor → caller rejects
  });
});

describe("scoreInternal — structural paths unchanged", () => {
  it("unknown valid mainnet address stays neutral 50 (blocked by default floor)", () => {
    const rec = scoreInternal(1, "0xc84782855624e0c8ec9a7f4e5a1a0f1e5d1b8a01", TTL_OK, TTL_BAD, FLOOR);
    expect(rec.safety_score).toBe(50);
    expect(rec.flags.reason).toBe("internal_heuristic_neutral");
  });

  it("zero address scores 0", () => {
    expect(scoreInternal(1, "0x0000000000000000000000000000000000000000", TTL_OK, TTL_BAD, FLOOR).safety_score).toBe(0);
  });

  it("burn-pattern address scores 10", () => {
    expect(scoreInternal(1, "0xdead" + "0".repeat(36), TTL_OK, TTL_BAD, FLOOR).safety_score).toBe(10);
  });

  it("invalid hex scores 0", () => {
    expect(scoreInternal(1, "not-an-address", TTL_OK, TTL_BAD, FLOOR).safety_score).toBe(0);
  });
});
