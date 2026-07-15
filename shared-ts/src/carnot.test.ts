import { describe, it, expect } from "vitest";
import type { PermittedCycle } from "./carnot";

describe("carnot types", () => {
  it("permits a minimal cycle", () => {
    const cycle: PermittedCycle = {
      id: "cycle-1",
      chain_id: 1,
      detected_at: new Date().toISOString(),
      eta: 0.12,
      work_extracted_usd: 5.0,
      heat_in_usd: 100.0,
      heat_out_usd: 95.0,
      gradient: {
        token_in: "WETH",
        token_out: "USDC",
        potential_delta_usd: 5.0,
        venue_in: "uniswap_v3",
        venue_out: "binance",
      },
      dissipation: {
        gas_usd: 2.0,
        fee_bps: 30,
        latency_ms: 120,
        decoherence_usd: 0.5,
      },
      status: "detected",
    };
    expect(cycle.eta).toBeGreaterThan(0);
  });
});
