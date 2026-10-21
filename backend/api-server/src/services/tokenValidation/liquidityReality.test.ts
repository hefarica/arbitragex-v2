/**
 * PR #1004 — `volume_as_of` provenance.
 *
 * `volume_24h_usd` is a DEX Screener snapshot. The only other timestamp in the
 * row is `validated_at`, which is when OUR validator ran; a cache hit or a
 * retry makes the two differ by hours. These tests pin that the capture time is
 * the moment the payload was parsed, and that "no snapshot" is NULL — never the
 * insert time.
 */
import { afterEach, describe, expect, it, vi } from "vitest";
import { validateLiquidityReality } from "./liquidityReality.js";

const PAIR = {
  chainId: "ethereum",
  dexId: "uniswap",
  pairAddress: "0xpair",
  priceUsd: "2.5",
  liquidity: { usd: 1_000_000 },
  volume: { h24: 5_000_000, h1: 100_000, m5: 1_000 },
};

function mockFetch(body: unknown, ok = true) {
  vi.stubGlobal("fetch", vi.fn(async () => ({
    ok,
    status: ok ? 200 : 500,
    json: async () => body,
  })) as unknown as typeof fetch);
}

afterEach(() => vi.unstubAllGlobals());

describe("validateLiquidityReality — captured_at (migration 127)", () => {
  it("stamps the snapshot instant when the payload is parsed", async () => {
    mockFetch({ pairs: [PAIR] });
    const before = Date.now();
    const r = await validateLiquidityReality(1, "0xabc");
    const after = Date.now();
    expect(r.ran).toBe(true);
    expect(r.volume_24h_usd).toBe(5_000_000);
    expect(r.captured_at).not.toBeNull();
    const t = Date.parse(r.captured_at!);
    expect(t).toBeGreaterThanOrEqual(before);
    expect(t).toBeLessThanOrEqual(after + 5);
  });

  it("keeps NULL when there is no snapshot to date", async () => {
    mockFetch({ pairs: [] });
    const r = await validateLiquidityReality(1, "0xabc");
    expect(r.no_data).toBe(true);
    expect(r.captured_at).toBeNull();
  });

  it("keeps NULL on an HTTP error — a failure has no snapshot age", async () => {
    mockFetch({}, false);
    const r = await validateLiquidityReality(1, "0xabc");
    expect(r.error).toBe(true);
    expect(r.captured_at).toBeNull();
  });
});
