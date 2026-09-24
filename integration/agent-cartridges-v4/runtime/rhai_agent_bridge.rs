//! Rhai agent v4 binding boundary. All I/O is supplied by the existing backend.
//!
//! This module registers real names used by the generated scripts. It does NOT
//! change engine limits, configuration, signer, trading mode or delivery systems.
//! Adapter implementations must read trusted, immutable backend snapshots; never
//! construct an AgentServices from an untrusted browser JSON payload.
//!
//! Quote adapters own protocol transitions. Native math-engine owns operators.
//! This boundary owns exact decimal accounting, complete-result preservation,
//! and plan/revision binding. A hash proves content integrity, not RPC truth.
use rhai::{Array, Dynamic, Engine, EvalAltResult, ImmutableString, Map};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeSet,BTreeMap};
use bigdecimal::BigDecimal;
use std::sync::Arc;
use std::str::FromStr;

/// Decimal USD, arbitrary precision (already a dependency of searcher-rs).
/// Tokens stay U256 strings; no f64 conversion and no $1 stable assumption.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Usd(pub BigDecimal);
impl Usd {
    pub fn parse(s: &str) -> Result<Self, String> {
        if s.len()>512 { return Err("usd_input_too_long".into()); }
        let body=s.strip_prefix('-').unwrap_or(s);
        let parts:Vec<&str>=body.split('.').collect();
        if parts.is_empty() || parts.len()>2 || parts[0].is_empty() || !parts[0].bytes().all(|b|b.is_ascii_digit())
            || (parts.len()==2 && (parts[1].is_empty() || !parts[1].bytes().all(|b|b.is_ascii_digit()))) {
            return Err("invalid_usd_decimal".into());
        }
        BigDecimal::from_str(s).map(Self).map_err(|_|"invalid_usd_decimal".into())
    }
    pub fn zero()->Self {Self(BigDecimal::from(0))}
    pub fn is_positive(&self)->bool{self.0>BigDecimal::from(0)}
    pub fn is_negative(&self)->bool{self.0<BigDecimal::from(0)}
    pub fn checked_add(&self,rhs:&Self)->Result<Self,String>{Ok(Self(&self.0+&rhs.0))}
    pub fn checked_sub(&self,rhs:&Self)->Result<Self,String>{Ok(Self(&self.0-&rhs.0))}
    pub fn text(&self)->String{self.0.normalized().to_plain_string()}
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenValuation {
    pub chain_id:u64,pub token_address:String,pub amount_raw:String,pub token_decimals:u8,
    pub price_usd:String,pub price_revision:String,pub price_evidence_id:String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonetaryFlow {
    pub name:String,pub usd:String,pub evidence_id:String,
    /// Mandatory for sequential token-route principal/output valuations.
    pub valuation:Option<TokenValuation>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostLine {
    pub kind:String,
    /// external = subtract once; embedded = already reflected in quote/flow;
    /// not_applicable = no amount, explicit applicability evidence required.
    pub treatment:String,pub usd:Option<String>,pub reason:Option<String>,pub evidence_id:String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuotedPlan {
    pub status:String,pub context_id:String,pub plan_id:String,pub plan_hash:String,
    pub snapshot_id:String,pub price_revision:String,pub policy_revision:String,
    pub amount_in_raw:String,pub capital_usd:Option<String>,
    /// before_financing or retained_after_repayment; explicit, not inferred.
    pub profit_basis:String,
    /// atomic_quote / execution_improvement / expected_carry /
    /// guaranteed_payoff / non_atomic_expected / settlement_quote.
    pub economic_kind:String,
    pub incoming:Vec<MonetaryFlow>,pub outgoing:Vec<MonetaryFlow>,pub costs:Vec<CostLine>,
    pub required_cost_kinds:Vec<String>,
    /// Full structured legs. Quantities are DECIMAL STRINGS in token base units.
    pub legs:Vec<Value>,pub missing:Vec<String>,pub evidence:Value,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequirementReceipt {
    pub name:String,pub status:String,pub reason:Option<String>,pub evidence_id:String,
    pub snapshot_id:String,pub plan_hash:String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyView {
    pub enabled:bool,pub capital_cap_usd:String,pub min_profit_usd:Option<String>,
    pub max_gas_usd:Option<String>,pub snapshot_id:String,pub price_revision:String,
    pub policy_revision:String,pub execution_mode:String,
    pub control_state:String,
}
/// Implement with existing graph/index, SizeOptimizer, quote providers,
/// math-engine/toggles, risk engine and canonical encoder. Methods are separate
/// so no strategy substitutes a projection for a protocol quote or simulation.
/// No method is given a fabricated default implementation.
pub trait AgentServices: Send+Sync {
    fn discover(&self,ctx:&Value,spec:&Value)->Result<Value,String>;
    fn quote(&self,ctx:&Value,spec:&Value,candidate:&Value)->Result<QuotedPlan,String>;
    fn operators(&self,ctx:&Value,spec:&Value,candidate:&Value)->Result<Value,String>;
    fn policy(&self,ctx:&Value,spec:&Value)->Result<PolicyView,String>;
    fn verify_requirements(&self,ctx:&Value,spec:&Value,candidate:&Value,names:&[String])->Result<Vec<RequirementReceipt>,String>;
    /// Must compare plan+amount+snapshot+price+policy revisions to the simulated
    /// canonical plan. It must NOT encode by rebuilding intent.legs.
    fn build_payload(&self,opportunity:&Value,spec:&Value)->Result<Value,String>;
}
fn failure(code:&str)->Value{json!({"status":"DATA_GAP","candidate_eligible":false,"net_profit_usd":null,"reason":code,"repairs":[{"field":"native_binding","reason":code}]})}
fn digest(v:&Value)->String {
    fn sorted(v:&Value)->Value {match v {
        Value::Object(m)=> {let sorted:BTreeMap<_,_>=m.iter().map(|(k,v)|(k.clone(),sorted(v))).collect(); Value::Object(sorted.into_iter().collect())},
        Value::Array(a)=>Value::Array(a.iter().map(sorted).collect()),_=>v.clone()
    }}
    // Value has a total JSON serialization; no empty-byte fallback.
    format!("{:x}",Sha256::digest(sorted(v).to_string().as_bytes()))
}
fn amount_is_canonical(s:&str)->bool{!s.is_empty() && s.bytes().all(|b|b.is_ascii_digit()) && (s=="0" || !s.starts_with('0'))}
fn repairs_push(repairs:&mut Vec<Value>,field:&str,reason:&str){repairs.push(json!({"field":field,"reason":reason}));}
fn sum_flows(flows:&[MonetaryFlow])->Result<Usd,String>{
    flows.iter().try_fold(Usd::zero(),|sum,f|{
        if f.evidence_id.is_empty(){return Err(format!("missing_flow_evidence:{}",f.name));}
        let value=Usd::parse(&f.usd)?; if value.is_negative(){return Err("flow_amount_must_be_nonnegative".into());}
        sum.checked_add(&value)
    })
}
fn numeric_output(v:&Value)->bool{match v {Value::Number(n)=>n.as_f64().is_some_and(f64::is_finite),Value::Array(a)=>!a.is_empty()&&a.iter().all(numeric_output),_=>false}}
fn operator_gate(spec:&Value,evidence:&Value,repairs:&mut Vec<Value>){
    let Some(requirements)=spec.get("operator_requirements").and_then(Value::as_array) else {repairs_push(repairs,"operators","missing_manifest_roles");return};
    for requirement in requirements {
        if requirement["role"]=="N/A" {continue;}
        let Some(id)=requirement["id"].as_u64() else {repairs_push(repairs,"operators","invalid_operator_id");continue};
        let key=id.to_string();let row=&evidence["operators"][&key];
        let status=row["status"].as_str().unwrap_or("MISSING");
        let valid_computed=status=="COMPUTED" && row.get("value").is_some_and(numeric_output) && row["evidence_id"].as_str().is_some_and(|s|!s.is_empty());
        // Native exact/equivalent algorithm receipts must name the role/phase.
        let equivalent=status=="EQUIVALENT" && row["equivalent_for"].as_u64()==Some(id) && row["evidence_id"].as_str().is_some_and(|s|!s.is_empty()) && row["method"].as_str().is_some_and(|s|!s.is_empty());
        let optional_disabled=status=="DISABLED" && requirement["requirement"]=="OPTIONAL_SUPPORT" && row["reason"]=="operator_switch_off";
        if !(valid_computed||equivalent||optional_disabled){repairs_push(repairs,&format!("operators.{id}"),status);}
    }
}
/// Economic evaluation after quote. Does not run a broadcast or set Sim PASS.
pub fn economic_check(svc:&dyn AgentServices,ctx:&Value,spec:&Value,candidate:&Value,quote_value:&Value,evidence:&Value,required:&[String])->Value {
    let mut repairs=Vec::new();
    let quote:QuotedPlan=match serde_json::from_value(quote_value.clone()){Ok(q)=>q,Err(_)=>return json!({"status":"DATA_GAP","candidate_eligible":false,"net_profit_usd":null,"reason":"invalid_quote_contract","context_id":ctx["context_id"],"plan_id":candidate["plan_id"],"plan_hash":candidate["plan_hash"],"candidate":candidate,"quote":quote_value,"operators":evidence})};
    let policy=match svc.policy(ctx,spec){Ok(p)=>p,Err(e)=>{let mut f=failure(&e);f["quote"]=quote_value.clone();f["legs"]=json!(quote.legs);f["operators"]=evidence.clone();f["candidate"]=candidate.clone();return f;}};
    for (field,actual,expected) in [
        ("context_id",quote.context_id.as_str(),ctx["context_id"].as_str().unwrap_or("")),
        ("snapshot_id",quote.snapshot_id.as_str(),policy.snapshot_id.as_str()),
        ("price_revision",quote.price_revision.as_str(),policy.price_revision.as_str()),
        ("policy_revision",quote.policy_revision.as_str(),policy.policy_revision.as_str()),
        ("plan_id",quote.plan_id.as_str(),candidate["plan_id"].as_str().unwrap_or("")),
        ("plan_hash",quote.plan_hash.as_str(),candidate["plan_hash"].as_str().unwrap_or("")),
    ] {if actual.is_empty()||actual!=expected {repairs_push(&mut repairs,field,"context_revision_mismatch");}}
    if !["LIVE_MAINNET","TESTNET","PAPER_SHADOW"].contains(&policy.execution_mode.as_str()){repairs_push(&mut repairs,"execution_mode","unknown_mode");}
    if !amount_is_canonical(&quote.amount_in_raw){repairs_push(&mut repairs,"amount_in_raw","noncanonical_integer");}
    if quote.status!="COMPUTED" {repairs_push(&mut repairs,"quote","incomplete_quote");}
    if !["before_financing","retained_after_repayment"].contains(&quote.profit_basis.as_str()){repairs_push(&mut repairs,"profit_basis","unknown_profit_basis");}
    if !["atomic_quote","execution_improvement","expected_carry","guaranteed_payoff","non_atomic_expected","settlement_quote"].contains(&quote.economic_kind.as_str()){repairs_push(&mut repairs,"economic_kind","unknown_economic_kind");}
    if spec["logic"]=="carry_projection"&&quote.economic_kind!="expected_carry"{repairs_push(&mut repairs,"economic_kind","carry_must_remain_projection");}
    if spec["logic"]=="cross_domain"&&quote.economic_kind!="non_atomic_expected"{repairs_push(&mut repairs,"economic_kind","cross_domain_not_atomic");}
    if spec["logic"]=="closed_route"&&quote.economic_kind!="atomic_quote"{repairs_push(&mut repairs,"economic_kind","closed_route_kind_mismatch");}
    for m in &quote.missing {repairs_push(&mut repairs,m,"applicable_input_missing");}
    // Exact ledger consistency for sequential routes. No cross-token subtraction.
    if ["closed_route","post_state_route"].contains(&spec["logic"].as_str().unwrap_or("")) {
        if quote.legs.is_empty(){repairs_push(&mut repairs,"legs","missing_leg_ledger");}
        for (i,leg) in quote.legs.iter().enumerate(){
            for k in ["token_in","token_out","amount_in_raw","amount_out_raw","quote_id","snapshot_id"]{
                if leg[k].as_str().is_none_or(str::is_empty){repairs_push(&mut repairs,&format!("legs.{i}.{k}"),"missing_leg_field");}
            }
            if leg["snapshot_id"]!=quote.snapshot_id {repairs_push(&mut repairs,&format!("legs.{i}"),"mixed_snapshot");}
            if i==0 && leg["amount_in_raw"]!=quote.amount_in_raw {repairs_push(&mut repairs,"legs.0.amount_in_raw","amount_mismatch");}
            if i>0 && (quote.legs[i-1]["token_out"]!=leg["token_in"] || quote.legs[i-1]["amount_out_raw"]!=leg["amount_in_raw"]){repairs_push(&mut repairs,&format!("legs.{i}"),"broken_token_or_amount_continuity");}
        }
        if let (Some(first),Some(last))=(quote.legs.first(),quote.legs.last()) {
            if spec["logic"]=="closed_route" && first["token_in"]!=last["token_out"] {repairs_push(&mut repairs,"legs","not_closed");}
        }
    }
    // A positive USD cashflow cannot contradict the actual raw leg ledger.
    if ["closed_route","post_state_route"].contains(&spec["logic"].as_str().unwrap_or("")) {
        if quote.incoming.len()!=1||quote.outgoing.len()!=1 {repairs_push(&mut repairs,"cashflows","sequential_route_requires_bound_principal_and_output");}
        else if let (Some(first),Some(last))=(quote.legs.first(),quote.legs.last()) {
            for (flow,leg,side) in [(&quote.outgoing[0],first,"in"),(&quote.incoming[0],last,"out")] {
                if let Some(v)=&flow.valuation {
                    let token_key=format!("token_{side}");let amount_key=format!("amount_{side}_raw");let dec_key=format!("token_{side}_decimals");
                    let expected=crate::agent_graph::token_value_usd(&v.amount_raw,v.token_decimals,&v.price_usd);
                    let valid=leg[&token_key]==v.token_address&&leg[&amount_key]==v.amount_raw&&leg[&dec_key].as_u64()==Some(v.token_decimals as u64)
                        &&leg["chain_id"].as_u64()==Some(v.chain_id)&&v.price_revision==quote.price_revision&&!v.price_evidence_id.is_empty()
                        &&expected.ok().and_then(|n|Usd::parse(&n).ok())==Usd::parse(&flow.usd).ok();
                    if !valid{repairs_push(&mut repairs,&format!("cashflows.{}",flow.name),"valuation_not_bound_to_ledger");}
                }else{repairs_push(&mut repairs,&format!("cashflows.{}",flow.name),"missing_token_valuation_provenance");}
            }
            if let (Some(a),Some(b))=(&quote.outgoing[0].valuation,&quote.incoming[0].valuation){
                if a.chain_id==b.chain_id&&a.token_address==b.token_address&&(a.token_decimals!=b.token_decimals||Usd::parse(&a.price_usd).ok()!=Usd::parse(&b.price_usd).ok()) {repairs_push(&mut repairs,"cashflows","same_asset_has_conflicting_price_or_decimals");}
            }
            if quote.capital_usd.as_deref().and_then(|n|Usd::parse(n).ok())!=Usd::parse(&quote.outgoing[0].usd).ok(){repairs_push(&mut repairs,"capital_usd","capital_differs_from_ledger_input");}
        }
    }
    let gross=if quote.incoming.is_empty()||quote.outgoing.is_empty(){repairs_push(&mut repairs,"cashflows","missing_incoming_or_outgoing_flow");None}else{match(sum_flows(&quote.incoming),sum_flows(&quote.outgoing)){
        (Ok(i),Ok(o))=>match i.checked_sub(&o){Ok(g)=>Some(g),Err(e)=>{repairs_push(&mut repairs,"gross_profit_usd",&e);None}},
        _=>{repairs_push(&mut repairs,"cashflows","invalid_or_unproven_flow");None}
    }};
    let mut seen=BTreeSet::new();let mut external=Usd::zero();let mut gas=Usd::zero();let mut costs_complete=true;
    if quote.required_cost_kinds.is_empty(){costs_complete=false;repairs_push(&mut repairs,"required_cost_kinds","empty_cost_contract");}
    for line in &quote.costs {
        if quote.profit_basis=="retained_after_repayment"&&line.kind=="financing"&&line.treatment=="external"{costs_complete=false;repairs_push(&mut repairs,"costs.financing","already_in_retained_spread");}
        if quote.economic_kind=="atomic_quote"&&line.kind=="execution_fees"&&line.treatment=="external" {costs_complete=false;repairs_push(&mut repairs,"costs.execution_fees","quote_already_includes_swap_fees");}
        if !seen.insert(line.kind.clone()){repairs_push(&mut repairs,"costs","duplicate_cost_kind");costs_complete=false;}
        if line.evidence_id.is_empty(){repairs_push(&mut repairs,&format!("costs.{}",line.kind),"missing_cost_evidence");costs_complete=false;}
        match line.treatment.as_str(){
            "external"|"embedded"=>match line.usd.as_deref().and_then(|x|Usd::parse(x).ok()){
                Some(v) if !v.is_negative()=>{if line.treatment=="external"{match external.checked_add(&v){Ok(x)=>external=x,Err(e)=>{costs_complete=false;repairs_push(&mut repairs,"costs",&e);}}if line.kind=="gas"{gas=v;}}},
                _=>{costs_complete=false;repairs_push(&mut repairs,&format!("costs.{}",line.kind),"missing_or_invalid_cost");}
            },
            "not_applicable"=>{if line.usd.is_some()||line.reason.as_deref().is_none_or(str::is_empty){costs_complete=false;repairs_push(&mut repairs,"costs","invalid_not_applicable");}},
            _=>{costs_complete=false;repairs_push(&mut repairs,"costs","unknown_cost_treatment");}
        }
    }
    for name in &quote.required_cost_kinds {if !seen.contains(name){costs_complete=false;repairs_push(&mut repairs,&format!("costs.{name}"),"required_cost_missing");}}
    if quote.economic_kind=="atomic_quote"{for name in ["gas","financing","execution_fees"]{if !seen.contains(name){costs_complete=false;repairs_push(&mut repairs,&format!("costs.{name}"),"mandatory_route_cost_missing");}}}
    let net=if costs_complete{gross.as_ref().and_then(|g|g.checked_sub(&external).ok())}else{None};
    let mut policy_ok=policy.enabled && policy.control_state=="ON";
    let capital=quote.capital_usd.as_deref().and_then(|s|Usd::parse(s).ok());
    let cap=Usd::parse(&policy.capital_cap_usd).ok();
    if !matches!((capital,cap),(Some(a),Some(b)) if a.is_positive() && b.is_positive() && a<=b){policy_ok=false;repairs_push(&mut repairs,"capital_usd","capital_missing_or_cap_exceeded");}
    if let Some(t)=policy.min_profit_usd.as_deref(){match Usd::parse(t){Ok(t)=>{if net.as_ref().is_none_or(|n|n<=&t){policy_ok=false;}},Err(e)=>repairs_push(&mut repairs,"min_profit_usd",&e)}}
    if let Some(t)=policy.max_gas_usd.as_deref(){match Usd::parse(t){Ok(t)=>{if gas>t{policy_ok=false;}},Err(e)=>repairs_push(&mut repairs,"max_gas_usd",&e)}}
    // Verify domain-specific constraints using host receipts, never user flags.
    match svc.verify_requirements(ctx,spec,candidate,required){
        Err(e)=>repairs_push(&mut repairs,"detector_requirements",&e),
        Ok(receipts)=>{
            for name in required {
                let matching:Vec<_>=receipts.iter().filter(|r|&r.name==name).collect();
                if matching.len()!=1 {repairs_push(&mut repairs,name,"missing_or_duplicate_constraint_receipt");continue;}
                let r=matching[0];
                if r.status!="PASS"||r.evidence_id.is_empty()||r.snapshot_id!=quote.snapshot_id||r.plan_hash!=quote.plan_hash{repairs_push(&mut repairs,name,r.reason.as_deref().unwrap_or("constraint_not_verified"));}
            }
        }
    }
    operator_gate(spec,evidence,&mut repairs);
    if evidence["snapshot_id"]!=quote.snapshot_id||evidence["plan_hash"]!=quote.plan_hash {repairs_push(&mut repairs,"operators","operator_context_mismatch");}
    if spec["conflicts"].as_array().is_some_and(|a|a.iter().any(|v|v=="NON_CYCLIC_NAME_VS_CLOSED_CYCLE_SPECIFIC_NOTE")){repairs_push(&mut repairs,"strategy_identity","source_conflict_requires_resolution");}
    let observe=spec["logic"]=="observe_only";
    // Improvement-only signals need a separate executable settlement proposal.
    let comparison_only=quote.economic_kind=="execution_improvement";
    let eligible=!observe&&!comparison_only&&repairs.is_empty()&&policy_ok&&net.as_ref().is_some_and(Usd::is_positive);
    json!({"contract_version":"arbx.cartridge.agent/4","status":if eligible{"CANDIDATE"}else if observe{"OBSERVE_ONLY"}else if !repairs.is_empty(){"DATA_GAP"}else if comparison_only{"IMPROVEMENT_ONLY"}else{"REJECTED"},
      "candidate_eligible":eligible,"approved_for_execution":false,"mev_id":spec["mev_id"],"detector_id":spec["detector_id"],
      "context_id":quote.context_id,"snapshot_id":quote.snapshot_id,"price_revision":quote.price_revision,"policy_revision":quote.policy_revision,
      "plan_id":quote.plan_id,"plan_hash":quote.plan_hash,"amount_in_raw":quote.amount_in_raw,"capital_usd":quote.capital_usd,
      "gross_profit_usd":gross.as_ref().map(Usd::text),"net_profit_usd":net.as_ref().map(Usd::text),"external_cost_usd":if costs_complete{Some(external.text())}else{None},
      "profit_basis":quote.profit_basis,"economic_kind":quote.economic_kind,"execution_mode":policy.execution_mode,
      "legs":quote.legs,"costs":quote.costs,"quote_evidence":quote.evidence,"operators":evidence,"repairs":repairs,
      "reason":if eligible{"economic_proposal_requires_canonical_validation"}else if observe{"source_observe_only"}else if comparison_only{"execution_improvement_is_not_settled_profit"}else if !policy_ok{"policy_rejected"}else if !repairs.is_empty(){"applicable_data_or_constraint_gap"}else{"non_positive_net"},
      "simulation":{"status":"NOT_RUN_BY_CARTRIDGE","passed":null,"reason":"canonical_simulator_owns_validation"}})
}
fn to_json(d:&Dynamic)->Result<Value,String>{
    if d.is_unit(){return Ok(Value::Null)}
    if let Ok(v)=d.as_bool(){return Ok(json!(v))}
    if let Ok(v)=d.as_int(){return Ok(json!(v))}
    if let Ok(v)=d.as_float(){return if v.is_finite(){Ok(json!(v))}else{Err("non_finite_dynamic".into())}}
    if let Some(v)=d.clone().try_cast::<ImmutableString>(){return Ok(json!(v.as_str()))}
    if let Some(v)=d.clone().try_cast::<Array>(){return v.iter().map(to_json).collect::<Result<Vec<_>,_>>().map(Value::Array)}
    if let Some(v)=d.clone().try_cast::<Map>(){let mut m=serde_json::Map::new();for(k,v)in v{m.insert(k.to_string(),to_json(&v)?);}return Ok(Value::Object(m))}
    Err("unsupported_dynamic_type".into())
}
fn from_json(v:Value)->Dynamic{
    match v {
        Value::Null=>Dynamic::UNIT, Value::Bool(x)=>x.into(),Value::String(x)=>x.into(),
        Value::Number(n)=>if let Some(i)=n.as_i64(){i.into()}else if let Some(f)=n.as_f64(){f.into()}else{Dynamic::UNIT},
        Value::Array(a)=>a.into_iter().map(from_json).collect::<Array>().into(),
        Value::Object(m)=>m.into_iter().map(|(k,v)|(k.into(),from_json(v))).collect::<Map>().into(),
    }
}
fn err(e:String)->Box<EvalAltResult>{e.into()}
fn cv(d:Dynamic)->Result<Value,Box<EvalAltResult>>{to_json(&d).map_err(err)}
/// Register only; does not mutate any engine limits or repository config.
pub fn register(engine:&mut Engine,svc:Arc<dyn AgentServices>){
    let s=svc.clone();engine.register_fn("agent_v4_discover",move|ctx:Dynamic,spec:Dynamic|->Result<Dynamic,Box<EvalAltResult>>{let(c,m)=(cv(ctx)?,cv(spec)?);Ok(from_json(s.discover(&c,&m).unwrap_or_else(|e|failure(&e))))});
    let s=svc.clone();engine.register_fn("agent_v4_quote",move|ctx:Dynamic,spec:Dynamic,candidate:Dynamic|->Result<Dynamic,Box<EvalAltResult>>{let(c,m,p)=(cv(ctx)?,cv(spec)?,cv(candidate)?);Ok(from_json(match s.quote(&c,&m,&p){Ok(q)=>serde_json::to_value(q).map_err(|_|err("quote_encode_failed".into()))?,Err(e)=>failure(&e)}))});
    let s=svc.clone();engine.register_fn("agent_v4_operators",move|ctx:Dynamic,spec:Dynamic,candidate:Dynamic|->Result<Dynamic,Box<EvalAltResult>>{let(c,m,p)=(cv(ctx)?,cv(spec)?,cv(candidate)?);Ok(from_json(s.operators(&c,&m,&p).unwrap_or_else(|e|failure(&e))))});
    let s=svc.clone();engine.register_fn("agent_v4_economic_check",move|ctx:Dynamic,spec:Dynamic,candidate:Dynamic,quote:Dynamic,evidence:Dynamic,requirements:Dynamic|->Result<Dynamic,Box<EvalAltResult>>{
        let(c,m,p,q,e,r)=(cv(ctx)?,cv(spec)?,cv(candidate)?,cv(quote)?,cv(evidence)?,cv(requirements)?);
        let names:Vec<String>=serde_json::from_value(r).map_err(|_|err("invalid_requirement_names".into()))?;
        Ok(from_json(economic_check(s.as_ref(),&c,&m,&p,&q,&e,&names)))
    });
    engine.register_fn("agent_v4_money_compare",|a:&str,b:&str|->Result<i64,Box<EvalAltResult>>{let a=Usd::parse(a).map_err(err)?;let b=Usd::parse(b).map_err(err)?;Ok(if a<b{-1}else if a>b{1}else{0})});
    engine.register_fn("agent_v4_seal",|ctx:Dynamic,spec:Dynamic,best:Dynamic,observations:Dynamic,discovery:Dynamic|->Result<Dynamic,Box<EvalAltResult>>{
        let(c,m,b,o,d)=(cv(ctx)?,cv(spec)?,cv(best)?,cv(observations)?,cv(discovery)?);
        let mut result=if b.is_object(){b}else{json!({"status":if d["status"]!="READY"{d["status"].clone()}else{json!("NO_CANDIDATE")},"net_profit_usd":null,"gross_profit_usd":null,"reason":if d["reason"].is_string(){d["reason"].clone()}else{json!("no_computed_candidate")}})};
        result["contract_version"]=json!("arbx.cartridge.agent/4");result["mev_id"]=m["mev_id"].clone();result["detector_id"]=m["detector_id"].clone();result["context_id"]=c["context_id"].clone();
        result["is_opportunity"]=json!(result["candidate_eligible"]==true);
        // Deliberately no legacy estimated_profit f64: runner v3 must not silently
        // convert absent values to zero or reconstruct the route from intent.
        result["approved_for_execution"]=json!(false);result["estimated_profit"]=Value::Null;result["confidence"]=Value::Null;
        result["numeric_contract"]=json!({"money":"USD decimal strings","token_amounts":"base-unit integer strings","missing":"null plus explicit diagnostic","compatibility":"REQUIRES_V4_RESULT_ADAPTER"});
        result["observations"]=o;result["discovery"]=d;result["manifest_digest"]=m["source_digest"].clone();
        result["result_digest"]=json!(digest(&result));Ok(from_json(result))
    });
    engine.register_fn("agent_v4_build_payload",move|opportunity:Dynamic,spec:Dynamic|->Result<Dynamic,Box<EvalAltResult>>{
        let(o,m)=(cv(opportunity)?,cv(spec)?);
        if o["contract_version"]!="arbx.cartridge.agent/4"||o["mev_id"]!=m["mev_id"]||o["manifest_digest"]!=m["source_digest"]{return Ok(from_json(failure("proposal_manifest_mismatch")));}
        Ok(from_json(svc.build_payload(&o,&m).unwrap_or_else(|e|json!({"status":"NEEDS_CANONICAL_ENCODER","reason":e,"approved_for_execution":false,"plan_hash":o["plan_hash"]}))))
    });
}

/// Helper for backend implementers: canonical content binding (SHA256, not a
/// claim of on-chain provenance). JSON object keys are recursively sorted.
pub fn canonical_hash(v:&Value)->String{digest(v)}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]fn exact_money_round_trip(){for s in ["0","-0.5","2.000000000000000001","99999999999999999999"]{assert_eq!(Usd::parse(s).unwrap().text(),s);}}
    #[test]fn money_rejects_invalid(){for s in ["","NaN","1e3"," 1","+1","1..2"]{assert!(Usd::parse(s).is_err(),"{s}");}}
    #[test]fn loss_is_preserved(){assert_eq!(Usd::parse("99.48").unwrap().checked_sub(&Usd::parse("100").unwrap()).unwrap().text(),"-0.52");}
}
