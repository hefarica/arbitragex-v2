import { describe, expect, it } from "vitest";
import { deriveHopCount, deriveLegs, deriveLegLedger, mapToOmniOpportunity, parseRouteMetadata } from "../types";

// Synthetic unit fixtures, never injected into the live feed.
function fixture(hops: number) {
  const tokens = Array.from({ length: hops }, (_, i) => `0x${(i + 1).toString(16).padStart(40, "0")}`);
  tokens.push(tokens[0]!);
  const principal = 2n ** 200n;
  return {
    id: "unit-only", chain_id: 1, strategy_kind: "dex_arb", status: "detected",
    detected_at: "2026-09-14T00:00:00Z", block_number: 1,
    token_in: tokens[0], token_out: tokens[1], amount_in_wei: principal.toString(),
    route_metadata: {
      token_addresses: tokens, dex_adapters: Array.from({length:hops},()=>"uniswap_v2"),
      pool_addresses: Array.from({length:hops},(_,i)=>`pool-${i}`),
      leg_amounts_in: Array.from({length:hops},(_,i)=>(principal + BigInt(i)).toString()),
      leg_amounts_out: Array.from({length:hops},(_,i)=>(principal + BigInt(i+1)).toString()),
      leg_zero_for_one: Array.from({length:hops},()=>true),
    },
  };
}

describe("HOPS-PROVENANCE — 2/3/4/5-hop view-model fidelity", () => {
  it.each([2,3,4,5])("preserves all %i swaps, h+1 tokens, ordered pools and exact large wei", hops => {
    const raw=fixture(hops), opp=mapToOmniOpportunity(raw);
    expect(deriveHopCount(opp.route_metadata)).toBe(hops);
    const legs=deriveLegs(opp);
    expect(legs).toHaveLength(hops);
    for(let i=0;i<hops;i++) {
      expect(legs[i]!.token_in).toBe(raw.route_metadata.token_addresses[i]);
      expect(legs[i]!.token_out).toBe(raw.route_metadata.token_addresses[i+1]);
      expect(legs[i]!.pool).toBe(`pool-${i}`);
      expect(legs[i]!.synthetic).toBeUndefined();
    }
    expect(deriveLegLedger(opp)![hops-1]!.cycle_delta_wei).toBe(String(hops));
  });
  it("quarantines an invalid pool position instead of shifting the third pool into it", () => {
    const raw=fixture(3);
    const opp=mapToOmniOpportunity({...raw, route_metadata:{...raw.route_metadata, pool_addresses:["pool-0",null,"pool-2"]}});
    expect(opp.route_metadata).toBeNull();
    expect(opp.hop_count).toBeNull();
    expect(opp.semantic_violations).toContain("route_metadata_invalid");
    expect(deriveLegs(opp).every(l=>l.synthetic)).toBe(true);
    expect(deriveLegLedger(opp)).toBeNull();
  });
  it("does not manufacture a shorter route by filtering an invalid middle token", () => {
    const raw=fixture(3);
    expect(parseRouteMetadata({...raw.route_metadata, token_addresses:["a",null,"b","a"]})).toBeNull();
  });
  it("does not compress malformed adapters into a different hop count", () => {
    const raw=fixture(3);
    expect(parseRouteMetadata({...raw.route_metadata, dex_adapters:["v2",null,"v2"]})).toBeNull();
  });
  it("discards a malformed per-leg ledger instead of independently compacting its arrays", () => {
    const raw=fixture(2);
    const opp=mapToOmniOpportunity({...raw, route_metadata:{...raw.route_metadata, leg_amounts_in:["10",null,"20"]}});
    expect(deriveLegLedger(opp)).toBeNull();
  });
});
