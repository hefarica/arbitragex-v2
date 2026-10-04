import { describe, it, expect } from "vitest";
import {
  toXRayProps,
  resolveConfidence,
  resolveTokenSafety,
  resolveSimVerdict,
  resolveDecoherence,
  resolveTlsAmount,
  CARD_FIELD_REASON,
  type HomeOpportunitySource,
} from "../home-opportunity";

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
    expect(value.legs.state).toBe("computed");
    expect(value.legs.text).toBe(`${h} legs`);
    expect(value.route.text.split(" → ")).toHaveLength(h);
    // R8: no ROI recorded ⇒ a declared reason, never a mute "—".
    expect(value.yield.state).toBe("not_computed");
    expect(value.yield.text).not.toContain("—");
  });
  it.each([null,{}, {token_addresses:["A","B","A"],dex_adapters:["v2","v2"],pool_addresses:["p0",""]}])(
    "missing or incomplete topology never falls back to venue count",(metadata)=>{
      const opp=row(2);opp.route_metadata=metadata;
      const props=toXRayProps(opp);
      expect(props.legs.state).toBe("not_computed");
      expect(props.legs.text).toContain(CARD_FIELD_REASON.routeTopologyUnresolved);
      expect(props.route.state).toBe("not_computed");
    });
  it.each([0,-0.5,2.5])("uses the recorded ROI %s, not the profit's sign",(roi)=>{
    const opp={...row(2),roi_pct:roi,net_expected_profit_usd:-6};
    const props=toXRayProps(opp);
    expect(props.yield.text).toBe(`${roi>=0?"+":""}${roi.toFixed(2)}%`);
    expect(props.yield.state).toBe(roi===0?"zero":"computed");
  });
  it.each([null,NaN,Infinity])("absent/non-finite ROI stays unknown (%s)",(roi)=>{
    const props=toXRayProps({...row(2),roi_pct:roi});
    expect(props.yield.state).toBe("not_computed");
    expect(props.yield.text).toContain(CARD_FIELD_REASON.roiNull);
  });
  it("a terminal failure cannot inherit a successful-looking simulation verdict",()=>{
    const props=toXRayProps({...row(2),status:"failed",rejection_reason:"build_error",sim_classification:"success"});
    expect(props.simVerdict.text).toBe("failed");
    expect(props.simVerdict.state).toBe("computed");
  });
});

// ── A8-CONF-01: the five card fields, R8/R10 enforced structurally ───────────
// The card previously rendered four different KINDS of empty with one glyph:
// a computed null, a field with no producer anywhere, a fabricated "pendiente"
// default, and a hardcoded null for a value the API WAS shipping. These tests
// pin each field's verdict so none of those can come back silently.
describe("A8-CONF-01 — card fields resolve to a value or an explicit reason",()=>{
  it("confidence: a real scored posterior renders the value WITH its provenance",()=>{
    const f=resolveConfidence({confidence_score_bps:8700,posterior_prob:0.87,
      confidence_state:"computed",confidence_source:"scored_opportunities.posterior_prob"});
    expect(f.state).toBe("computed");
    expect(f.text).toBe("87% conf");
    expect(f).toHaveProperty("source","scored_opportunities.posterior_prob");
  });

  it("confidence: posterior EXACTLY 0 is CERO REAL — distinct state, not an absence",()=>{
    const f=resolveConfidence({confidence_score_bps:0,posterior_prob:0,confidence_state:"computed"});
    expect(f.state).toBe("zero");
    expect(f.text).toBe("0% conf");
  });

  it("confidence: a sub-basis-point posterior is COMPUTED, never rounded into a false 0",()=>{
    // Live prod value 2026-10-03: scored_opportunities.posterior_prob=2.600356865160073e-05.
    const f=resolveConfidence({confidence_score_bps:0,posterior_prob:2.600356865160073e-5,
      confidence_state:"computed"});
    expect(f.state).toBe("computed");
    expect(f.text).toBe("<0.01% conf");
    expect(f.text).not.toBe("0% conf");
  });

  it("confidence: no scored row → NO COMPUTADO with the API's own reason (R10)",()=>{
    const f=resolveConfidence({confidence_score_bps:null,confidence_state:"not_computed",
      confidence_reason:CARD_FIELD_REASON.confidenceNotComputedFallback});
    expect(f.state).toBe("not_computed");
    if (f.state==="not_computed") expect(f.reason).toBe("no_scored_row_for_opportunity");
    expect(f.text).toContain("no_scored_row_for_opportunity");
  });

  it("confidence: absent from the payload entirely reports THAT fact, it does not guess",()=>{
    const f=resolveConfidence({});
    expect(f.state).toBe("not_computed");
    if (f.state==="not_computed") expect(f.reason).toBe(CARD_FIELD_REASON.confidenceAbsentOnWire);
  });

  it("TOKEN SAFETY: the API's real 0-100 validation scores are wired, not discarded",()=>{
    // Shape measured live on /api/opportunities/live (2026-10-03): both legs
    // carried validation {status:"VERIFIED", score:75}.
    const f=resolveTokenSafety({
      ...row(2),
      token_in_info:{validation:{status:"VERIFIED",score:75}},
      token_out_info:{validation:{status:"VERIFIED",score:75}},
    } as HomeOpportunitySource);
    expect(f.state).toBe("computed");
    expect(f.text).toBe("A 75 VERIFIED · B 75 VERIFIED");
  });

  it("TOKEN SAFETY: validation not written back yet is declared pending, not 'safe'",()=>{
    const f=resolveTokenSafety(row(2));
    expect(f.state).toBe("not_computed");
    if (f.state==="not_computed") expect(f.reason).toBe("validation_pending_first_sighting");
  });

  it("TLS AMOUNT has NO producer — declared with its reason instead of a mute dash",()=>{
    const f=resolveTlsAmount();
    expect(f.state).toBe("not_computed");
    expect(f.text).toContain(CARD_FIELD_REASON.tlsAmountNoProducer);
    expect(f.text).not.toBe("—");
  });

  it("SIM VERDICT is never fabricated: absent verdict declares the missing producer",()=>{
    const f=resolveSimVerdict({status:"detected",rejection_reason:null});
    expect(f.state).toBe("not_computed");
    expect(f.text).toContain("no_producer__sim_classification_not_emitted");
    expect(f.text).not.toContain("pendiente");
  });

  it("DECOHERENCIA reads the producer's OWN not-computed reason when it ships one",()=>{
    const f=resolveDecoherence({
      ...row(2),
      economics:{computation_status:"computed",not_computed_reasons:{slippage_usd:"priced_by_amm_curve"}},
    } as HomeOpportunitySource);
    expect(f.state).toBe("not_computed");
    if (f.state==="not_computed") expect(f.reason).toBe("priced_by_amm_curve");
  });

  it("DECOHERENCIA renders a real slippage when the producer computed one",()=>{
    const f=resolveDecoherence({
      ...row(2), economics:{slippage_usd:0.42},
    } as HomeOpportunitySource);
    expect(f.state).toBe("computed");
    expect(f.text).toBe("slippage $0.42");
  });

  it("DECOHERENCIA does NOT relabel ROI as slippage (the pre-fix mislabel)",()=>{
    const props=toXRayProps({...row(2),roi_pct:2.5});
    expect(props.decoherence.state).toBe("not_computed");
    expect(props.decoherence.text).not.toContain("convergence");
    expect(props.yield.text).toBe("+2.50%");
  });
});
