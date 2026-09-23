//! Lossless v4 result reader. This replaces no existing parser by itself.
//! Call only on the v4 branch after the existing engine evaluates the script.
//! Do NOT convert a v4 proposal into the v3 f64 estimated_profit contract.
use crate::rhai_agent_bridge::Usd;
use serde::{Deserialize,Serialize};
use serde_json::Value;

#[derive(Debug,Clone,Serialize,Deserialize)]
pub struct ProposalV4 {
    pub contract_version:String,pub mev_id:String,pub detector_id:String,
    pub status:String,pub is_opportunity:bool,pub approved_for_execution:bool,
    pub context_id:String,pub manifest_digest:String,pub result_digest:String,
    pub gross_profit_usd:Option<String>,pub net_profit_usd:Option<String>,
    #[serde(default)]pub candidate_eligible:bool,
    #[serde(default)]pub plan_id:Option<String>,#[serde(default)]pub plan_hash:Option<String>,
    #[serde(default)]pub snapshot_id:Option<String>,#[serde(default)]pub price_revision:Option<String>,
    #[serde(default)]pub policy_revision:Option<String>,#[serde(default)]pub amount_in_raw:Option<String>,
    #[serde(default)]pub economic_kind:Option<String>,#[serde(default)]pub legs:Vec<Value>,
    #[serde(default)]pub observations:Vec<Value>,
    /// Preserve all extra evidence, diagnostics, cost lines and revisions for API/PG.
    #[serde(flatten)]pub extra:serde_json::Map<String,Value>,
}
impl ProposalV4 {
    pub fn parse(value:Value)->Result<Self,String>{
        let p:Self=serde_json::from_value(value).map_err(|e|format!("invalid_v4_result:{e}"))?;
        if p.contract_version!="arbx.cartridge.agent/4"||p.mev_id.is_empty()||p.manifest_digest.is_empty()||p.context_id.is_empty(){return Err("invalid_v4_identity".into());}
        if p.approved_for_execution{return Err("cartridge_cannot_self_authorize_execution".into());}
        for amount in [&p.gross_profit_usd,&p.net_profit_usd].into_iter().flatten(){Usd::parse(amount)?;}
        if p.candidate_eligible {
            if p.status!="CANDIDATE"||!p.is_opportunity||p.net_profit_usd.as_deref().and_then(|n|Usd::parse(n).ok()).is_none_or(|n|!n.is_positive()){return Err("candidate_economics_inconsistent".into());}
            for f in [&p.plan_id,&p.plan_hash,&p.snapshot_id,&p.price_revision,&p.policy_revision,&p.amount_in_raw,&p.economic_kind]{if f.as_deref().is_none_or(str::is_empty){return Err("candidate_missing_plan_binding".into());}}
            if p.legs.is_empty(){return Err("candidate_missing_action_ledger".into());}
        }
        Ok(p)
    }
    /// Used before consuming a canonical execution plan from the trusted store.
    /// The plan store must also validate amount, token direction, price/policy
    /// revision, permissions and simulation. No path comes from the old intent.
    pub fn plan_lookup_key(&self)->Result<(&str,&str,&str),String>{
        if !self.candidate_eligible{return Err("not_an_economic_candidate".into());}
        Ok((self.plan_hash.as_deref().ok_or("missing_plan_hash")?,self.snapshot_id.as_deref().ok_or("missing_snapshot")?,self.amount_in_raw.as_deref().ok_or("missing_amount")?))
    }
}
