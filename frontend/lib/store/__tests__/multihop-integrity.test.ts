import {describe, it, expect} from "vitest";
import {mapToOmniOpportunity, deriveHopCount, deriveLegs, deriveLegLedger, validateOpportunitySemantics} from "@/lib/store/types";

// Deterministic constructed unit routes; NEVER represented as market opportunities.
const address = (i: number) => `0x${i.toString(16).padStart(40, "0")}`;
function route(hops: number) {
  const tokens = Array.from({length:hops},(_,i)=>address(i+10));
  const input = (1n << 200n) + 1n;
  return {
    token_addresses:[...tokens,tokens[0]!],
    pool_addresses:tokens.map((_,i)=>address(i+100)),
    dex_adapters:tokens.map((_,i)=>`unit-dex-${i}`),
    leg_amounts_in:tokens.map((_,i)=>(input + BigInt(i)).toString()),
    leg_amounts_out:tokens.map((_,i)=>(input + BigInt(i+1)).toString()),
    leg_zero_for_one:tokens.map((_,i)=>i % 2 === 0),
  };
}
const mapped = (metadata: unknown) => mapToOmniOpportunity({
  id:"unit-route",chain_id:1,strategy_kind:"triangular",block_number:123,
  detected_at:"2026-09-14T00:00:00Z",status:"detected",token_in:address(10),token_out:address(10),
  dex_a:"unit-dex-0", route_metadata:metadata,
});

describe("2/3/4/5-hop route and ledger fidelity",()=>{
  for (const hops of [2,3,4,5]) {
    it(`${hops} swaps preserve every position and uint256 precision`,()=>{
      const raw=route(hops);const opp=mapped(raw);
      expect(opp.semantic_violations).toEqual([]);
      expect(opp.hop_count).toBe(hops);
      expect(deriveLegs(opp)).toHaveLength(hops);
      const ledger=deriveLegLedger(opp)!;expect(ledger).toHaveLength(hops);
      for(let i=0;i<hops;i++){
        expect(deriveLegs(opp)[i]).toMatchObject({token_in:raw.token_addresses[i],token_out:raw.token_addresses[i+1],pool:raw.pool_addresses[i],dex:raw.dex_adapters[i]});
        expect(ledger[i]!.amount_in_wei).toBe(raw.leg_amounts_in[i]);
        expect(ledger[i]!.amount_out_wei).toBe(raw.leg_amounts_out[i]);
        expect(ledger[i]!.cycle_delta_wei).toBe(i===hops-1?String(hops):null);
      }
    });
    it(`${hops} swaps do not silently shift a non-string token, pool or adapter`,()=>{
      for(const field of ["token_addresses","pool_addresses","dex_adapters"] as const){
        const raw=route(hops);const entries:unknown[]=[...raw[field]];entries.splice(1,0,42);
        const opp=mapped({...raw,[field]:entries});
        expect(opp.hop_count).toBeNull();
        expect(opp.semantic_violations).toContain("route_metadata_invalid");
        expect(deriveLegs(opp).every(l=>l.synthetic)).toBe(true);
      }
    });
    it(`${hops} swaps reject missing pools and amount arrays without forging a ledger`,()=>{
      const raw=route(hops);
      const missingPool=mapped({...raw,pool_addresses:raw.pool_addresses.slice(1)});
      expect(missingPool.hop_count).toBeNull();
      expect(missingPool.semantic_violations).toContain("hop_incoherent");
      expect(deriveLegLedger(missingPool)).toBeNull();
      const invalid=mapped({...raw,leg_amounts_in:[42,...raw.leg_amounts_in]});
      expect(invalid.hop_count).toBe(hops); // valid topology survives invalid money fields
      expect(invalid.semantic_violations).toContain("leg_ledger_incoherent");
      expect(deriveLegLedger(invalid)).toBeNull();
      expect(validateOpportunitySemantics(invalid)).toEqual(invalid.semantic_violations);
    });
  }
  for(const invalid of ["-1","1.5","1e18","",String(1n<<256n),"garbage"]){
    it(`refuses invalid wei ${invalid.slice(0,14)} rather than printing a sized route`,()=>{
      const raw=route(3);raw.leg_amounts_out[1]=invalid;
      const opp=mapped(raw);expect(deriveLegLedger(opp)).toBeNull();
      expect(opp.semantic_violations).toContain("leg_ledger_incoherent");
    });
  }
  it("EVM address casing does not break closure or conceal self-swaps",()=>{
    const raw=route(3);raw.token_addresses[3]=raw.token_addresses[0]!.toUpperCase();
    expect(mapped(raw).semantic_violations).toEqual([]);
    expect(deriveLegLedger(mapped(raw))![2]!.cycle_delta_wei).toBe("3");
    raw.token_addresses[1]=raw.token_addresses[0]!.toUpperCase();
    expect(mapped(raw).semantic_violations).toContain("degenerate_pair");
  });
  it("zero explicitly computed amounts are not confused with absent amounts",()=>{
    const raw=route(2);raw.leg_amounts_in=["0","0"];raw.leg_amounts_out=["0","0"];
    expect(deriveLegLedger(mapped(raw))![1]!.cycle_delta_wei).toBe("0");
  });
  it("absence remains null, never invented 2-hop topology",()=>{
    const opp=mapped(null);expect(opp.hop_count).toBeNull();expect(deriveLegLedger(opp)).toBeNull();
    expect(deriveHopCount(null)).toBeNull();
  });
});
