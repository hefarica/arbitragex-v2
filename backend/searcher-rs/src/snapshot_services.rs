//! Concrete offline/in-process adapter for immutable BACKEND-owned snapshots.
//! Not an HTTP handler: never deserialize SnapshotBundle from a browser request.
//! The live searcher supplies graph state, canonical PriceBus exports, exact quote
//! cache, native operator outputs, domain solver plans and cost receipts.
//! This module contains no fake provider, signer, network endpoint or defaults.
use crate::agent_graph::{
    enumerate_cycles, quote_path_progress, token_value_usd, Edge, ExactHopQuote, SearchLimits,
    SearchReport,
};
use crate::rhai_agent_bridge::{
    canonical_hash, AgentServices, CostLine, MonetaryFlow, PolicyView, QuotedPlan,
    RequirementReceipt, TokenValuation, Usd,
};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Debug, Clone)]
pub struct CanonicalPrice {
    pub chain_id: u64,
    pub token_address: String,
    pub usd: String,
    pub revision: String,
    pub observed_at_ms: u64,
    pub valid_until_ms: u64,
    pub evidence_id: String,
    /// Identity of the producer/export, not a claim that a field name proves truth.
    pub producer: String,
}
#[derive(Debug, Clone)]
pub struct PlanSupport {
    pub costs: Vec<CostLine>,
    pub required_cost_kinds: Vec<String>,
    pub constraints: Vec<RequirementReceipt>,
    pub operator_evidence: Value,
}
#[derive(Debug, Clone)]
pub struct PreparedDomainPlan {
    pub candidate: Value,
    pub quote: QuotedPlan,
    pub support: PlanSupport,
}
#[derive(Debug, Clone)]
pub struct EncodedAndSimulated {
    pub mev_id: String,
    pub context_id: String,
    pub plan_hash: String,
    pub snapshot_id: String,
    pub price_revision: String,
    pub policy_revision: String,
    pub amount_in_raw: String,
    pub trace_hash: String,
    pub calldata_hex: String,
    pub target: String,
    pub simulation_passed: bool,
    pub canonical_risk_passed: bool,
    pub net_profit_usd: String,
    pub valid_until_ms: u64,
    /// paper overrides/testnet and LIVE production preconditions are NOT interchangeable.
    pub execution_mode: String,
    pub storage_overrides_used: bool,
}
/// Construct ONLY inside the existing backend after validating the external
/// inputs. Every source-bound field is mandatory. No Serialize/Deserialize impl.
pub struct SnapshotBundle {
    pub context_id: String,
    pub snapshot_id: String,
    pub observed_at_ms: u64,
    pub valid_until_ms: u64,
    pub policy: PolicyView,
    pub start_token: String,
    pub chain_id: u64,
    pub edges: Vec<Edge>,
    pub limits: SearchLimits,
    /// Existing SizeOptimizer proposes finite positive sizes in raw units.
    /// The searcher owns this schedule. Selecting from it is NOT a proof of a
    /// continuous/global optimum. Missing sizes is an explicit DATA_GAP.
    pub size_schedule_raw: Vec<String>,
    pub prices: BTreeMap<(u64, String), CanonicalPrice>,
    pub exact_quotes: BTreeMap<String, ExactHopQuote>,
    /// Indexed by immutable candidate plan_hash, never just strategy ID.
    pub route_support: BTreeMap<String, PlanSupport>,
    pub domain_plans: BTreeMap<String, Vec<PreparedDomainPlan>>,
    pub canonical_payloads: BTreeMap<String, EncodedAndSimulated>,
    /// Generated manifest digests explicitly admitted by the backend loader.
    pub manifest_digests: BTreeMap<String, String>,
    /// Must bound candidate x size expansion as well as graph enumeration.
    pub max_evaluations: usize,
}
/// Revision liveness guard supplied by the owner (never invented here).
pub type RevisionGuard = Arc<dyn Fn(&str, &str) -> bool + Send + Sync>;
/// Dispatch into the REAL OperatorRegistry (see native_operator_adapter).
type OperatorDispatch = Arc<dyn Fn(&Value, &Value, &Value) -> Result<Value, String> + Send + Sync>;
/// Receipt resolver backed by the canonical plan store.
type PayloadResolver = Arc<dyn Fn(&str) -> Result<EncodedAndSimulated, String> + Send + Sync>;

pub struct SnapshotServices {
    data: Arc<SnapshotBundle>,
    /// Must consult the backend's live control/revision guard. The closure is
    /// supplied by the owner; this module does not invent switch states.
    active_revision: RevisionGuard,
    discovery: Mutex<Option<SearchReport>>,
    operator_dispatch: Option<OperatorDispatch>,
    payload_resolver: Option<PayloadResolver>,
}
fn now_ms() -> Result<u64, String> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .map_err(|_| "clock_before_epoch".into())
}
fn required<'a>(v: &'a Value, k: &str) -> Result<&'a str, String> {
    v[k].as_str()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| format!("missing_{k}"))
}
fn valid_hex(s: &str, bytes: Option<usize>) -> bool {
    let Some(h) = s.strip_prefix("0x") else {
        return false;
    };
    !h.is_empty()
        && h.len() % 2 == 0
        && h.bytes().all(|c| c.is_ascii_hexdigit())
        && bytes.is_none_or(|b| h.len() == b * 2)
}
impl SnapshotServices {
    pub fn new(data: Arc<SnapshotBundle>, active_revision: RevisionGuard) -> Result<Self, String> {
        if data.context_id.is_empty()
            || data.snapshot_id.is_empty()
            || data.chain_id == 0
            || data.max_evaluations == 0
            || data.valid_until_ms < data.observed_at_ms
        {
            return Err("invalid_snapshot_metadata".into());
        }
        if data.policy.snapshot_id != data.snapshot_id {
            return Err("snapshot_policy_mismatch".into());
        }
        if data.edges.iter().any(|e| {
            e.chain_id != data.chain_id
                || e.snapshot_id != data.snapshot_id
                || e.block_hash.is_empty()
        }) {
            return Err("graph_not_bound_to_snapshot".into());
        }
        Ok(Self {
            data,
            active_revision,
            discovery: Mutex::new(None),
            operator_dispatch: None,
            payload_resolver: None,
        })
    }
    /// Attach the actual registry dispatcher once at construction. The caller
    /// may use native_operator_adapter::evaluate_declared with is_disabled from
    /// the existing operator_toggles module; switches are never written here.
    pub fn with_operator_dispatch(mut self, dispatch: OperatorDispatch) -> Self {
        self.operator_dispatch = Some(dispatch);
        self
    }
    /// Receipts can arrive AFTER strategy selection without mutating the
    /// immutable quote snapshot. Resolve from the existing canonical plan store.
    pub fn with_payload_resolver(mut self, resolve: PayloadResolver) -> Self {
        self.payload_resolver = Some(resolve);
        self
    }
    fn check(&self, ctx: &Value, spec: &Value) -> Result<(), String> {
        let now = now_ms()?;
        if now < self.data.observed_at_ms || now > self.data.valid_until_ms {
            return Err("snapshot_stale_or_clock_skew".into());
        }
        if !(self.active_revision)(
            &self.data.policy.policy_revision,
            &self.data.policy.price_revision,
        ) {
            return Err("backend_revision_or_control_changed".into());
        }
        if required(ctx, "context_id")? != self.data.context_id
            || required(ctx, "snapshot_id")? != self.data.snapshot_id
        {
            return Err("untrusted_context_mismatch".into());
        }
        let id = required(spec, "mev_id")?;
        let digest = required(spec, "source_digest")?;
        if self.data.manifest_digests.get(id).map(String::as_str) != Some(digest) {
            return Err("manifest_not_admitted_by_backend".into());
        }
        Ok(())
    }
    fn graph_paths(&self) -> Result<SearchReport, String> {
        let mut cache = self
            .discovery
            .lock()
            .map_err(|_| "discovery_cache_poisoned")?;
        if let Some(report) = cache.as_ref() {
            return Ok(report.clone());
        }
        let report = enumerate_cycles(&self.data.edges, &self.data.start_token, &self.data.limits)?;
        *cache = Some(report.clone());
        Ok(report)
    }
    fn edges_for(&self, c: &Value) -> Result<Vec<Edge>, String> {
        let ids = c["edge_ids"].as_array().ok_or("missing_edge_ids")?;
        let mut edges = Vec::new();
        for id in ids {
            let id = id.as_str().ok_or("invalid_edge_id")?;
            let e = self
                .data
                .edges
                .iter()
                .find(|e| e.edge_id == id)
                .ok_or("unknown_edge_id")?;
            edges.push(e.clone());
        }
        Ok(edges)
    }
    fn candidate_body(&self, spec: &Value, ids: &Value, amount: &str) -> Value {
        json!({"mev_id":spec["mev_id"],"detector_id":spec["detector_id"],"context_id":self.data.context_id,
            "snapshot_id":self.data.snapshot_id,"price_revision":self.data.policy.price_revision,"policy_revision":self.data.policy.policy_revision,
            "edge_ids":ids,"amount_in_raw":amount})
    }
    fn check_candidate(&self, spec: &Value, c: &Value) -> Result<(), String> {
        if c["mev_id"] != spec["mev_id"]
            || c["detector_id"] != spec["detector_id"]
            || c["context_id"] != self.data.context_id
            || c["snapshot_id"] != self.data.snapshot_id
        {
            return Err("candidate_identity_mismatch".into());
        }
        if c["price_revision"] != self.data.policy.price_revision
            || c["policy_revision"] != self.data.policy.policy_revision
        {
            return Err("candidate_revision_mismatch".into());
        }
        if c["edge_ids"].is_array() {
            let amount = required(c, "amount_in_raw")?;
            let body = self.candidate_body(spec, &c["edge_ids"], amount);
            if required(c, "plan_hash")? != canonical_hash(&body) {
                return Err("candidate_hash_mismatch".into());
            }
            if !self.data.size_schedule_raw.iter().any(|a| a == amount) {
                return Err("size_not_in_native_schedule".into());
            }
            let n = c["edge_ids"].as_array().map(Vec::len).unwrap_or(0);
            if !spec["allowed_search_hops"]
                .as_array()
                .is_some_and(|h| h.iter().any(|v| v.as_u64() == Some(n as u64)))
            {
                return Err("strategy_hop_mask_rejected".into());
            }
        }
        Ok(())
    }
    fn domain<'a>(&'a self, spec: &Value, c: &Value) -> Result<&'a PreparedDomainPlan, String> {
        self.data
            .domain_plans
            .get(required(spec, "mev_id")?)
            .and_then(|ps| ps.iter().find(|p| p.candidate == *c))
            .ok_or_else(|| "native_domain_plan_missing_or_changed".into())
    }
    fn support<'a>(&'a self, spec: &Value, c: &Value) -> Result<&'a PlanSupport, String> {
        if c["edge_ids"].is_array() {
            self.data
                .route_support
                .get(required(c, "plan_hash")?)
                .ok_or_else(|| "native_costs_operators_or_constraints_missing".into())
        } else {
            Ok(&self.domain(spec, c)?.support)
        }
    }
    fn price(&self, token: &str) -> Result<&CanonicalPrice, String> {
        let p = self
            .data
            .prices
            .get(&(self.data.chain_id, token.to_owned()))
            .ok_or("canonical_price_missing")?;
        let now = now_ms()?;
        if p.producer != "PriceBus"
            || p.evidence_id.is_empty()
            || p.revision != self.data.policy.price_revision
            || p.token_address != token
            || p.chain_id != self.data.chain_id
        {
            return Err("canonical_price_provenance_mismatch".into());
        }
        if now < p.observed_at_ms || now > p.valid_until_ms || p.valid_until_ms < p.observed_at_ms {
            return Err("canonical_price_stale_or_future".into());
        }
        if !Usd::parse(&p.usd)?.is_positive() {
            return Err("canonical_price_nonpositive".into());
        }
        Ok(p)
    }
}
impl AgentServices for SnapshotServices {
    fn discover(&self, ctx: &Value, spec: &Value) -> Result<Value, String> {
        self.check(ctx, spec)?;
        if spec["logic"] == "observe_only" {
            return Ok(
                json!({"status":"OBSERVE_ONLY","reason":"source_observe_only","candidates":[],"snapshot_id":self.data.snapshot_id}),
            );
        }
        if ["closed_route", "post_state_route"].contains(&spec["logic"].as_str().unwrap_or("")) {
            if self.data.size_schedule_raw.is_empty() {
                return Err("native_size_schedule_missing".into());
            }
            let report = self.graph_paths()?;
            let mut candidates = Vec::new();
            let mut handoffs = Vec::new();
            let mut truncated = report.truncated;
            let allowed = spec["allowed_search_hops"]
                .as_array()
                .ok_or("missing_hop_contract")?;
            'outer: for ids in &report.paths {
                if !allowed.iter().any(|h| h.as_u64() == Some(ids.len() as u64)) {
                    handoffs.push(json!({"edge_ids":ids,"hops":ids.len(),"reason":"outside_strategy_mask","origin_mev_id":spec["mev_id"]}));
                    continue;
                }
                for size in &self.data.size_schedule_raw {
                    if candidates.len() >= self.data.max_evaluations {
                        truncated = true;
                        break 'outer;
                    }
                    let mut c = self.candidate_body(spec, &json!(ids), size);
                    let hash = canonical_hash(&c);
                    c["plan_id"] = json!(hash);
                    c["plan_hash"] = json!(hash);
                    candidates.push(c);
                }
            }
            Ok(
                json!({"status":"READY","candidates":candidates,"handoffs":handoffs,"truncated":truncated,
                "expansions":report.expansions,"graph_stopping_reason":report.stopping_reason,"selection_scope":"bounded_paths_x_native_size_schedule",
                "global_optimum_proven":false,"reason":if truncated{"search_budget_reached"}else{"search_completed_within_declared_scope"}}),
            )
        } else {
            let list = self
                .data
                .domain_plans
                .get(required(spec, "mev_id")?)
                .ok_or("native_domain_solver_required")?;
            let mut cs = Vec::new();
            for p in list.iter().take(self.data.max_evaluations) {
                self.check_candidate(spec, &p.candidate)?;
                cs.push(p.candidate.clone());
            }
            Ok(
                json!({"status":"READY","candidates":cs,"truncated":list.len()>self.data.max_evaluations,"selection_scope":"native_domain_solver_plans","global_optimum_proven":false}),
            )
        }
    }
    fn quote(&self, ctx: &Value, spec: &Value, c: &Value) -> Result<QuotedPlan, String> {
        self.check(ctx, spec)?;
        self.check_candidate(spec, c)?;
        if !c["edge_ids"].is_array() {
            return Ok(self.domain(spec, c)?.quote.clone());
        }
        let edges = self.edges_for(c)?;
        let amount = required(c, "amount_in_raw")?;
        let mut missing = Vec::new();
        let progress = quote_path_progress(&edges, amount, &self.data.exact_quotes);
        let route_complete = progress.complete;
        if let Some(reason) = progress.reason {
            missing.push(format!("hop_{:?}:{reason}", progress.failed_hop));
        }
        let mut ledger = progress.legs;
        // Price every completed leg in the SAME canonical price revision.
        // Missing intermediate valuation is a repair, not a decorative zero.
        for leg in &mut ledger {
            for side in ["in", "out"] {
                let token = required(leg, &format!("token_{side}"))?.to_owned();
                let raw = required(leg, &format!("amount_{side}_raw"))?.to_owned();
                let dec = leg[format!("token_{side}_decimals")]
                    .as_u64()
                    .and_then(|v| u8::try_from(v).ok())
                    .ok_or("invalid_leg_decimals")?;
                let k = format!("amount_{side}_usd");
                match self.price(&token).and_then(|p| {
                    token_value_usd(&raw, dec, &p.usd)
                        .map(|v| (v, p.evidence_id.clone(), p.usd.clone()))
                }) {
                    Ok((v, e, p)) => {
                        leg[&k] = json!(v);
                        leg[format!("price_{side}_usd")] = json!(p);
                        leg[format!("price_{side}_evidence")] = json!(e);
                        leg["price_revision"] = json!(self.data.policy.price_revision);
                    }
                    Err(e) => {
                        leg[&k] = Value::Null;
                        leg[format!("{k}_reason")] = json!(e);
                        missing.push(format!("{}:{k}:{e}", leg["edge_id"]));
                    }
                }
            }
        }
        let mut incoming = Vec::new();
        let mut outgoing = Vec::new();
        let mut capital = None;
        if let Some(first) = edges.first() {
            match self.price(&first.token_in).and_then(|p| {
                token_value_usd(amount, first.token_in_decimals, &p.usd)
                    .map(|v| (v, p.evidence_id.clone()))
            }) {
                Ok((v, e)) => {
                    capital = Some(v.clone());
                    let p = self.price(&first.token_in)?;
                    outgoing.push(MonetaryFlow {
                        name: "principal_in".into(),
                        usd: v,
                        evidence_id: e,
                        valuation: Some(TokenValuation {
                            chain_id: first.chain_id,
                            token_address: first.token_in.clone(),
                            amount_raw: amount.into(),
                            token_decimals: first.token_in_decimals,
                            price_usd: p.usd.clone(),
                            price_revision: p.revision.clone(),
                            price_evidence_id: p.evidence_id.clone(),
                        }),
                    });
                }
                Err(e) => missing.push(e),
            }
        }
        if route_complete {
            if let (Some(last), Some(leg)) = (edges.last(), ledger.last()) {
                match self.price(&last.token_out).and_then(|p| {
                    token_value_usd(
                        required(leg, "amount_out_raw")?,
                        last.token_out_decimals,
                        &p.usd,
                    )
                    .map(|v| (v, p.evidence_id.clone()))
                }) {
                    Ok((v, e)) => {
                        let p = self.price(&last.token_out)?;
                        incoming.push(MonetaryFlow {
                            name: "route_out".into(),
                            usd: v,
                            evidence_id: e,
                            valuation: Some(TokenValuation {
                                chain_id: last.chain_id,
                                token_address: last.token_out.clone(),
                                amount_raw: required(leg, "amount_out_raw")?.into(),
                                token_decimals: last.token_out_decimals,
                                price_usd: p.usd.clone(),
                                price_revision: p.revision.clone(),
                                price_evidence_id: p.evidence_id.clone(),
                            }),
                        });
                    }
                    Err(e) => missing.push(e),
                }
            }
        }
        let (costs, required_costs) = match self.support(spec, c) {
            Ok(s) => (s.costs.clone(), s.required_cost_kinds.clone()),
            Err(e) => {
                missing.push(e);
                (
                    Vec::new(),
                    vec!["gas".into(), "financing".into(), "execution_fees".into()],
                )
            }
        };
        Ok(QuotedPlan {
            status: if missing.is_empty() {
                "COMPUTED"
            } else {
                "DATA_GAP"
            }
            .into(),
            context_id: self.data.context_id.clone(),
            plan_id: required(c, "plan_id")?.into(),
            plan_hash: required(c, "plan_hash")?.into(),
            snapshot_id: self.data.snapshot_id.clone(),
            price_revision: self.data.policy.price_revision.clone(),
            policy_revision: self.data.policy.policy_revision.clone(),
            amount_in_raw: amount.into(),
            capital_usd: capital,
            profit_basis: "before_financing".into(),
            economic_kind: "atomic_quote".into(),
            incoming,
            outgoing,
            costs,
            required_cost_kinds: required_costs,
            legs: ledger,
            missing,
            evidence: json!({"producer":"SnapshotServices","market_reality":"depends_on_verified_backend_ingestion","quote_cache":"native_protocol_only","continuous_size_optimum":false}),
        })
    }
    fn operators(&self, ctx: &Value, spec: &Value, c: &Value) -> Result<Value, String> {
        self.check(ctx, spec)?;
        self.check_candidate(spec, c)?;
        let Some(dispatch) = &self.operator_dispatch else {
            return Ok(self.support(spec, c)?.operator_evidence.clone());
        };
        let mut fresh = dispatch(ctx, spec, c)?;
        // A disabled primary may be fulfilled only by an actual native
        // equivalent receipt already tied to this plan/snapshot. Preserve the
        // switch state separately, never pretend that the disabled op ran.
        if let Ok(support) = self.support(spec, c) {
            let cached = &support.operator_evidence;
            if cached["snapshot_id"] == self.data.snapshot_id
                && cached["plan_hash"] == c["plan_hash"]
            {
                if let Some(roles) = spec["operator_requirements"].as_array() {
                    for role in roles {
                        if let Some(id) = role["id"].as_u64() {
                            let key = id.to_string();
                            if role["requirement"] == "PRIMARY_REQUIRED_OR_EQUIVALENT"
                                && fresh["operators"][&key]["status"] == "DISABLED"
                                && cached["operators"][&key]["status"] == "EQUIVALENT"
                            {
                                let disabled = fresh["operators"][&key].clone();
                                fresh["operators"][&key] = cached["operators"][&key].clone();
                                fresh["operators"][&key]["disabled_operator_trace"] = disabled;
                            }
                        }
                    }
                }
            }
        }
        Ok(fresh)
    }
    fn policy(&self, ctx: &Value, spec: &Value) -> Result<PolicyView, String> {
        self.check(ctx, spec)?;
        Ok(self.data.policy.clone())
    }
    fn verify_requirements(
        &self,
        ctx: &Value,
        spec: &Value,
        c: &Value,
        _names: &[String],
    ) -> Result<Vec<RequirementReceipt>, String> {
        self.check(ctx, spec)?;
        self.check_candidate(spec, c)?;
        Ok(self.support(spec, c)?.constraints.clone())
    }
    fn build_payload(&self, o: &Value, spec: &Value) -> Result<Value, String> {
        self.check(
            &json!({"context_id":o["context_id"],"snapshot_id":o["snapshot_id"]}),
            spec,
        )?;
        if o["candidate_eligible"] != true {
            return Err("economic_proposal_not_eligible".into());
        }
        let key = required(o, "plan_hash")?;
        let p = match &self.payload_resolver {
            Some(resolve) => resolve(key)?,
            None => self
                .data
                .canonical_payloads
                .get(key)
                .cloned()
                .ok_or("canonical_simulation_and_encoding_missing")?,
        };
        for (k, a) in [
            ("mev_id", &p.mev_id),
            ("context_id", &p.context_id),
            ("plan_hash", &p.plan_hash),
            ("snapshot_id", &p.snapshot_id),
            ("price_revision", &p.price_revision),
            ("policy_revision", &p.policy_revision),
            ("amount_in_raw", &p.amount_in_raw),
            ("execution_mode", &p.execution_mode),
        ] {
            if required(o, k)? != a {
                return Err(format!("simulated_payload_mismatch:{k}"));
            }
        }
        if !p.simulation_passed
            || !p.canonical_risk_passed
            || !Usd::parse(&p.net_profit_usd)?.is_positive()
            || p.valid_until_ms < now_ms()?
            || !valid_hex(&p.trace_hash, Some(32))
            || p.trace_hash == format!("0x{}", "0".repeat(64))
            || !valid_hex(&p.calldata_hex, None)
            || !valid_hex(&p.target, Some(20))
            || p.target == format!("0x{}", "0".repeat(40))
            || p.calldata_hex.len() < 10
        {
            return Err("canonical_simulation_or_payload_invalid".into());
        }
        let final_net = Usd::parse(&p.net_profit_usd)?;
        if let Some(minimum) = self.data.policy.min_profit_usd.as_deref() {
            if final_net <= Usd::parse(minimum)? {
                return Err("simulated_net_below_current_policy".into());
            }
        }
        if p.execution_mode == "LIVE_MAINNET" && p.storage_overrides_used {
            return Err("paper_override_cannot_authorize_live".into());
        }
        Ok(
            json!({"status":"CANONICAL_PLAN_VALIDATED","mev_id":p.mev_id,"context_id":p.context_id,"plan_hash":p.plan_hash,"snapshot_id":p.snapshot_id,"price_revision":p.price_revision,"policy_revision":p.policy_revision,"amount_in_raw":p.amount_in_raw,"execution_mode":p.execution_mode,"calldata":p.calldata_hex,"target_contract":p.target,"simulation_trace_hash":p.trace_hash,"simulated_net_profit_usd":p.net_profit_usd,"approved_for_execution":false,"reason":"existing_signer_and_live_authorization_gates_remain_authoritative"}),
        )
    }
}
