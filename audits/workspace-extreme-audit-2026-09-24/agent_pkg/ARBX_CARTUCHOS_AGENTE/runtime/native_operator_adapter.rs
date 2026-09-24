//! Integration module for the EXISTING searcher-rs workspace only.
//! Uses the current math-engine registry; does not reimplement operators, set
//! weights, alter enabled states or add a 32nd role to the source 31-column map.
//! Native defaults must be excluded by input admission BEFORE dispatch. A
//! numerical result by itself does not prove the financial validity of an op.
use math_engine::operators::{MarketState,OperatorRegistry};
use crate::rhai_agent_bridge::canonical_hash;
use serde_json::{json,Value};
use std::panic::{catch_unwind,AssertUnwindSafe};

/// The backend supplies this admission hook from field lineage/type/unit checks.
/// It must validate ALL inputs consumed by the operator, including any that the
/// legacy implementation would otherwise fill with defaults. Return a receipt
/// identifier on success; an empty ID or absent input is a data error.
pub trait OperatorInputAdmission {
    fn validate(&self,id:u8,state:&MarketState,snapshot_id:&str,plan_hash:&str)->Result<String,String>;
}

pub fn evaluate_declared(
    registry:&OperatorRegistry,state:&MarketState,spec:&Value,
    snapshot_id:&str,plan_hash:&str,admission:&dyn OperatorInputAdmission,
    is_disabled:impl Fn(u8)->bool,
)->Result<Value,String>{
    if snapshot_id.is_empty()||plan_hash.is_empty(){return Err("operator_context_missing".into());}
    let roles=spec["operator_requirements"].as_array().ok_or("missing_declared_operator_roles")?;
    let mut outputs=serde_json::Map::new();
    for r in roles {
        let id=r["id"].as_u64().filter(|i|(1..=31).contains(i)).ok_or("invalid_source_operator_id")? as u8;
        let key=id.to_string();
        if outputs.contains_key(&key){return Err("duplicate_source_operator_id".into());}
        if r["role"]=="N/A"{outputs.insert(key,json!({"status":"NOT_APPLICABLE","reason":"source_matrix_role_na","role":r["role"]}));continue;}
        if is_disabled(id){outputs.insert(key,json!({"status":"DISABLED","reason":"operator_switch_off","role":r["role"],"requirement":r["requirement"]}));continue;}
        let input_receipt=match admission.validate(id,state,snapshot_id,plan_hash){
            Ok(id)if !id.is_empty()=>id,
            Ok(_)=>{outputs.insert(key,json!({"status":"DATA_GAP","reason":"empty_input_admission_receipt"}));continue;},
            Err(e)=>{outputs.insert(key,json!({"status":"DATA_GAP","reason":e}));continue;}
        };
        let dispatched=catch_unwind(AssertUnwindSafe(||registry.dispatch(id,state)));
        let out=match dispatched{
            Ok(Some(o))=>o,
            Ok(None)=>{outputs.insert(key,json!({"status":"MISSING","reason":"operator_not_registered"}));continue;},
            Err(_)=>{outputs.insert(key,json!({"status":"ERROR","reason":"native_operator_panicked"}));continue;}
        };
        let scalar=out.scalar_value.filter(|v|v.is_finite());
        let vector=out.vector_result.filter(|v|!v.is_empty()&&v.iter().all(|x|x.is_finite()));
        let matrix=out.matrix_result.filter(|m|!m.is_empty()&&m.iter().all(|v|!v.is_empty()&&v.iter().all(|x|x.is_finite())));
        let value=scalar.map(|v|json!(v)).or_else(||vector.as_ref().map(|v|json!(v))).or_else(||matrix.as_ref().map(|v|json!(v)));
        let mut receipt=json!({"operator_id":id,"operator_name":out.operator_name,"snapshot_id":snapshot_id,"plan_hash":plan_hash,
            "input_receipt":input_receipt,"phase":r["phase"],"role":r["role"],"scalar":scalar,"vector":vector,"matrix":matrix,
            "metadata":out.metadata,"weight":null,"calibration_state":"UNCALIBRATED"});
        if let Some(v)=value{receipt["status"]=json!("COMPUTED");receipt["value"]=v;}
        else{receipt["status"]=json!("DATA_GAP");receipt["reason"]=json!("native_operator_returned_no_finite_value");}
        receipt["evidence_id"]=json!(canonical_hash(&receipt));outputs.insert(key,receipt);
    }
    Ok(json!({"snapshot_id":snapshot_id,"plan_hash":plan_hash,"operators":outputs,"source_operator_count":31,
        "runtime_operator_32":"unchanged_unassigned_by_these_sources","weighted_confidence":null,
        "reason":"source_weights_uncalibrated_no_fabricated_confidence"}))
}
