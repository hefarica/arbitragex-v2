import { describe, expect, it } from "vitest";
import { routeGroupKeyOf } from "../route-key";
import type { OmniOpportunity } from "../types";

/**
 * Pins the bit-exact format of the shared route group key so the api-server
 * LIVE_QUERY GROUP BY twin (concat_ws + COALESCE, see route-key.ts doc) can
 * never drift from the client side. If this test changes on purpose, the SQL
 * twin must change in the same commit (merge-authority condition 2026-09-20).
 */
function baseOpp(overrides: Partial<OmniOpportunity> = {}): OmniOpportunity {
  return {
    id: "uuid-1",
    chain_id: 1,
    strategy_kind: "dex_arb",
    detected_at: "2026-09-20T12:00:00Z",
    trace_id: "t1",
    cartridge_id: null,
    hop_count: 2,
    candidate_id: null,
    route_id: null,
    pair_id: null,
    detector_id: null,
    pipeline_latency_ms: null,
    quote_token: null,
    quote_version: null,
    graph_version: null,
    config_version: null,
    strategy_version: null,
    gate_results: null,
    data_quality: null,
    dex_a: "uniswap_v2",
    dex_b: "sushiswap",
    pair_symbol: "WETH/USDC",
    token_in: "0xA",
    token_out: "0xB",
    amount_in_wei: "1000",
    token_in_info: null,
    token_out_info: null,
    chain_base_token_symbol: null,
    leg_symbols: null,
    expected_profit_usd: 1.5,
    net_expected_profit_usd: 1.0,
    roi_pct: 0.5,
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
    route_metadata: null,
    simulated_net_profit_usd: null,
    simulated_amount_in_usd: null,
    simulated_roi_pct: null,
    ...overrides,
  } as OmniOpportunity;
}

describe("routeGroupKeyOf", () => {
  it("emits the exact pinned format for a full row", () => {
    expect(routeGroupKeyOf(baseOpp())).toBe(
      "1||dex_arb|0xA|0xB|uniswap_v2|sushiswap",
    );
  });

  it("renders null sides as empty strings (SQL COALESCE twin)", () => {
    const key = routeGroupKeyOf(
      baseOpp({ chain_id_out: null, dex_b: null, strategy_kind: null }),
    );
    expect(key).toBe("1|||0xA|0xB|uniswap_v2|");
  });

  it("keeps cross-chain rows distinct from same-chain ones", () => {
    const same = routeGroupKeyOf(baseOpp({ chain_id_out: null }));
    const cross = routeGroupKeyOf(baseOpp({ chain_id_out: 137 }));
    expect(same).not.toBe(cross);
    expect(cross).toBe("1|137|dex_arb|0xA|0xB|uniswap_v2|sushiswap");
  });

  it("same identity with different ids/timestamps shares one key", () => {
    const a = baseOpp({ id: "uuid-1", detected_at: "2026-09-20T10:00:00Z" });
    const b = baseOpp({ id: "uuid-2", detected_at: "2026-09-20T13:00:00Z" });
    expect(routeGroupKeyOf(a)).toBe(routeGroupKeyOf(b));
  });
});
