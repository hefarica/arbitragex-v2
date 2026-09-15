import { describe, it, expect } from "vitest";
import { toXRayProps, type HomeOpportunitySource } from "../home-opportunity";

// Constructed display fixtures. No RPC, transactions or production injection.
function row(hops: number): HomeOpportunitySource {
  const tokens=Array.from({length:hops},(_,i)=>`0x${String(i+1).padStart(40,"0")}`);
  tokens.push(tokens[0]!);
  return {id:"lab-home",chain_id:1,strategy_kind:"flashloan_arb",dex_a:"v2:p0",dex_b:"v2:p1",
    token_in:tokens[0]!,token_out:tokens[0]!,amount_in_wei:"42",pair_symbol:"A/A",
    expected_profit_usd:7,net_expected_profit_usd:6,roi_pct:null,status:"detected",
    rejection_reason:null,trace_id:"lab-trace",detected_at:"2026-09-14T00:00:00Z",risk_score:null,block_number:null,
    dexes_used:["v2:p0","v2:p1","uniswap_v2_router"],
    route_metadata:{token_addresses:tokens,pool_addresses:Array.from({length:hops},(_,i)=>`pool-${i}`),
      dex_adapters:Array(hops).fill("uniswap_v2_router")}};
}

describe("home projects route topology, not venue cardinality",()=>{
  it.each([2,3,4,5])("preserves exactly %s swaps even when all use the same adapter",(h)=>{
    const value=toXRayProps(row(h));
    expect(value.legs).toBe(h);
    expect(value.route.split(" → ")).toHaveLength(h);
    expect(value.yield).toBe("—");
  });
  it.each([null,{}, {token_addresses:["A","B","A"],dex_adapters:["v2","v2"],pool_addresses:["p0",""]}])(
    "missing or incomplete topology never falls back to venue count",(metadata)=>{
      const opp=row(2);opp.route_metadata=metadata;
      expect(toXRayProps(opp).legs).toBeNull();
    });
  it.each([0,-0.5,2.5])("uses the recorded ROI %s, not the profit's sign",(roi)=>{
    const opp={...row(2),roi_pct:roi,net_expected_profit_usd:-6};
    expect(toXRayProps(opp).yield).toBe(`${roi>=0?"+":""}${roi.toFixed(2)}%`);
  });
  it.each([null,NaN,Infinity])("absent/non-finite ROI stays unknown (%s)",(roi)=>{
    expect(toXRayProps({...row(2),roi_pct:roi}).yield).toBe("—");
  });
  it("a terminal failure cannot inherit a successful-looking simulation verdict",()=>{
    const props=toXRayProps({...row(2),status:"failed",rejection_reason:"build_error",sim_classification:"success"});
    expect(props.simVerdict).toBe("failed");
    expect(props.safetyA).toBeNull();expect(props.safetyB).toBeNull();
  });
});
