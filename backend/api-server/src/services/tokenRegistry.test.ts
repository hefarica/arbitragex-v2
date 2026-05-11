/**
 * Unit tests for the curated-token verification registry.
 *
 * Pin behavior against the committed snapshot of the Uniswap Labs Default
 * Token List. If the snapshot is regenerated, the version-specific
 * assertions (token counts) will need updating — that's intentional, the
 * test surfaces a deliberate input change.
 */

import { describe, expect, it, beforeEach } from "vitest";
import {
  _resetRegistryForTests,
  lookupRegistry,
  registryStats,
  verifyToken,
} from "./tokenRegistry.js";

// Canonical mainnet addresses (lowercase) — from the committed snapshot.
const WETH_MAINNET = "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2";
const USDC_MAINNET = "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48";
// The "69420" memecoin contract surfaced by the live feed on 2026-05-10.
// Deliberately chosen as a known absent-from-registry example.
const SCAM_69420 = "0xd162e93c1552f5b4c2fb9ce9890614411c3e36df";

describe("tokenRegistry", () => {
  beforeEach(() => _resetRegistryForTests());

  it("loads the snapshot and indexes by (chain_id, lowercase address)", () => {
    const s = registryStats();
    expect(s.loaded).toBe(true);
    expect(s.load_error).toBeNull();
    // Pin: the snapshot shipped in 2026-05 has ≥ 1k tokens and ≥ 300 on chain 1.
    expect(s.total_tokens).toBeGreaterThan(1000);
    expect(s.by_chain[1]).toBeGreaterThan(300);
  });

  it("looks up WETH mainnet by lowercase address", () => {
    const e = lookupRegistry(1, WETH_MAINNET);
    expect(e).not.toBeNull();
    expect(e!.symbol).toBe("WETH");
    expect(e!.name).toBe("Wrapped Ether");
    expect(e!.decimals).toBe(18);
    expect(e!.logo_uri).toMatch(/^https:\/\//);
  });

  it("address matching is case-insensitive", () => {
    const lower = lookupRegistry(1, WETH_MAINNET);
    const upper = lookupRegistry(1, WETH_MAINNET.toUpperCase());
    const mixed = lookupRegistry(1, "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2");
    expect(lower).not.toBeNull();
    expect(upper).toEqual(lower);
    expect(mixed).toEqual(lower);
  });

  it("returns null for chain-mismatched lookups (right address, wrong chain)", () => {
    // WETH mainnet address on chain 42161 — not what L1 WETH is on Arbitrum.
    expect(lookupRegistry(42161, WETH_MAINNET)).toBeNull();
  });
});

describe("verifyToken", () => {
  beforeEach(() => _resetRegistryForTests());

  it("verifies WETH on chain 1 when symbol matches", () => {
    const r = verifyToken(1, WETH_MAINNET, "WETH");
    expect(r.verified).toBe(true);
    expect(r.registry_symbol).toBe("WETH");
    expect(r.registry_name).toBe("Wrapped Ether");
    expect(r.notes).toContain("in-registry");
    expect(r.notes).not.toContain("symbol-mismatch");
  });

  it("verifies USDC on chain 1 with mixed-case symbol", () => {
    const r = verifyToken(1, USDC_MAINNET, "usdc");
    expect(r.verified).toBe(true);
    expect(r.registry_symbol).toBe("USDC");
  });

  it("flags symbol-mismatch when address in registry but on-chain symbol differs", () => {
    // Imagine WETH's address but contract claims to be "ETHER" instead.
    const r = verifyToken(1, WETH_MAINNET, "ETHER");
    expect(r.verified).toBe(false);
    expect(r.notes).toContain("in-registry");
    expect(r.notes).toContain("symbol-mismatch");
    expect(r.registry_symbol).toBe("WETH");  // honest: tell operator what we expected
  });

  it("does NOT flag mismatch when on-chain symbol is missing (graceful)", () => {
    // Curated address, but resolver couldn't fetch symbol — trust registry.
    const r = verifyToken(1, WETH_MAINNET, null);
    expect(r.verified).toBe(true);
    expect(r.notes).toContain("in-registry");
    expect(r.notes).not.toContain("symbol-mismatch");
  });

  it("rejects unknown addresses (the 69420 memecoin)", () => {
    const r = verifyToken(1, SCAM_69420, "69420");
    expect(r.verified).toBe(false);
    expect(r.registry_symbol).toBeNull();
    expect(r.registry_name).toBeNull();
    expect(r.notes).toContain("address-not-in-registry");
  });

  it("rejects unknown contracts that claim to be USDC (impersonation defense)", () => {
    const fakeAddr = "0x" + "1".repeat(40);
    const r = verifyToken(1, fakeAddr, "USDC");
    expect(r.verified).toBe(false);
    expect(r.notes).toContain("address-not-in-registry");
    // Critical: the impersonator's claimed "USDC" does NOT make it verified.
  });
});
