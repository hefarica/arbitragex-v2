import { describe, expect, it } from "vitest";

import {
  DefiChainRowSchema,
  DefiChainsResponseSchema,
  DefiPoolRowSchema,
  DefiPoolsResponseSchema,
} from "@/lib/schemas";

/**
 * FASE0 contract lock (A-03). These fixtures are the REAL payloads served by the
 * live edge (verified 2026-08-09). The schemas MUST accept them and MUST reject
 * the pre-fix drift (dex/active + rpc_url) — so any future serializer drift
 * explodes here in CI instead of rendering lies on /pools or /chains.
 */

const REAL_POOL = {
  id: "5616cefc-05da-4804-8be9-a9c7659be4da",
  chain_id: 1,
  address: "0x1445f32d1a74872ba41f3d8cf4022e9996120b31",
  dex_id: "0a5a4a3a-dc87-40b6-9eb7-a375d445f1d4",
  dex_name: "PancakeSwap V3",
  protocol_type: "UNISWAP_V3",
  fee_tier: null,
  token0_symbol: "USDC",
  token0_address: "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48",
  token1_symbol: "WETH",
  token1_address: "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
  is_active: true,
};

const REAL_CHAIN = {
  id: "6d74796d-0499-4c9b-b783-5f91bae1b8a8",
  chain_id: 1,
  name: "ethereum",
  native_currency: "ETH",
  explorer_url: "https://etherscan.io",
  is_active: true,
  created_at: "2026-05-04T05:21:13.510Z",
  updated_at: "2026-05-04T05:21:13.510Z",
};

describe("FASE0 contract — DefiPoolRowSchema (aligned to live /api/pools)", () => {
  it("accepts the real edge payload (dex_name, is_active)", () => {
    const r = DefiPoolRowSchema.safeParse(REAL_POOL);
    expect(r.success).toBe(true);
    if (r.success) {
      expect(r.data.dex_name).toBe("PancakeSwap V3");
      expect(r.data.is_active).toBe(true);
    }
  });

  it("rejects the pre-A-03 drift payload (dex/active instead of dex_name/is_active)", () => {
    // This is what USED to silently pass (optional + passthrough) and render
    // 61 live pools as DISABLED with DEX "—". It must now FAIL.
    const drifted = {
      address: "0xabc",
      token0_symbol: "USDC",
      token1_symbol: "WETH",
      dex: "Uniswap",
      active: true,
    };
    expect(DefiPoolRowSchema.safeParse(drifted).success).toBe(false);
  });

  it("wrapped response validates the live envelope", () => {
    const r = DefiPoolsResponseSchema.safeParse({ success: true, data: [REAL_POOL] });
    expect(r.success).toBe(true);
  });
});

describe("FASE0 contract — DefiChainRowSchema (aligned to live /api/chains)", () => {
  it("accepts the real edge payload (no rpc_url served)", () => {
    const r = DefiChainRowSchema.safeParse(REAL_CHAIN);
    expect(r.success).toBe(true);
    if (r.success) expect(r.data.is_active).toBe(true);
  });

  it("rejects a chain missing the required is_active flag", () => {
    const noActive = { ...REAL_CHAIN } as Partial<typeof REAL_CHAIN>;
    delete noActive.is_active;
    expect(DefiChainRowSchema.safeParse(noActive).success).toBe(false);
  });

  it("wrapped response validates the live envelope", () => {
    const r = DefiChainsResponseSchema.safeParse({ success: true, data: [REAL_CHAIN] });
    expect(r.success).toBe(true);
  });
});
