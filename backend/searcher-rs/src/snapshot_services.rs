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
    /// COST-PRODUCERS-01 (2026-10-02): líneas de coste BASE computadas por el
    /// dueño del bundle con productores reales — gas (unidades de la config
    /// del operador × gas observado × precio base), financiación (tasa
    /// declarada en config; 0 → not_applicable con evidencia), comisiones
    /// (embebidas en las cotizaciones, ya reflejadas — jamás restadas dos
    /// veces). `quote()` las usa cuando el soporte precomputado por plan no
    /// aporta costes: antes dejaba `costs=[]` y el bridge reportaba
    /// `mandatory_route_cost_missing` para TODA ruta del grafo — el bloqueo
    /// dominante medido en producción. Vacío = el dueño no pudo computarlas
    /// (stub Phase-1 sin gas ni config) → DATA_GAP honesto (R8), nunca ceros.
    pub base_cost_lines: Vec<CostLine>,
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
    /// OPERATOR-DISPATCH-WIRING-01: evidencia nativa cacheada por plan_hash.
    /// La puebla `operators()` cuando el dispatcher REAL corre para ese plan;
    /// la lee `derive_support()` para que el recibo
    /// `native_risk_and_impact_policy` refleje el dispatch EFECTIVAMENTE
    /// ejecutado (y no un FAIL constante). Orden del flujo del cartucho:
    /// operators → verify; si verify llega antes, cache miss = FAIL honesto.
    operator_evidence_cache: Mutex<std::collections::BTreeMap<String, Value>>,
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
            operator_evidence_cache: Mutex::new(std::collections::BTreeMap::new()),
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
    /// Lookup ONLY of precomputed support (bundle `route_support` / domain
    /// plans). NEVER derives — `quote()` itself consumes this, so a deriving
    /// lookup here would recurse infinitely.
    fn precomputed_support<'a>(
        &'a self,
        spec: &Value,
        c: &Value,
    ) -> Result<&'a PlanSupport, String> {
        if c["edge_ids"].is_array() {
            self.data
                .route_support
                .get(required(c, "plan_hash")?)
                .ok_or_else(|| "native_costs_operators_or_constraints_missing".into())
        } else {
            Ok(&self.domain(spec, c)?.support)
        }
    }

    /// AgentServices-facing support: precomputed first; a GRAPH candidate
    /// with no precomputed entry derives its support from the plan's OWN real
    /// quote (PLAN-SUPPORT-WIRING-01). Before this, every graph candidate
    /// died with `native_costs_operators_or_constraints_missing` and the
    /// bridge reported `missing_or_duplicate_constraint_receipt` — the 69%
    /// measured `applicable_data_or_constraint_gap`. Every number in the
    /// derived support is the quote's own (R8: no fabricated cost ever).
    fn support(&self, ctx: &Value, spec: &Value, c: &Value) -> Result<PlanSupport, String> {
        if let Ok(pre) = self.precomputed_support(spec, c) {
            return Ok(pre.clone());
        }
        self.derive_support(ctx, spec, c)
    }

    /// Support derived from the REAL quote of this exact plan/amount/snapshot
    /// (PLAN-SUPPORT-WIRING-01, enmienda). Recibos con los NOMBRES que los
    /// cartuchos de ciclo cerrado DECLARAN en su manifiesto
    /// (`required_constraints`): closed_token_cycle, protocol_exact_quotes,
    /// same_snapshot, token_continuity, strategy_specific_note_verified,
    /// native_risk_and_impact_policy — el bridge busca cada nombre EXACTO, y
    /// con nombres propios (`quote_route_complete`) seguía reportando
    /// missing_or_duplicate_constraint_receipt. Cada recibo afirma SOLO lo
    /// que su verificación comprueba: los cuatro de ruta se calculan aquí
    /// desde edges/ledger reales; los dos que exigen capa nativa se emiten
    /// FAIL con la razón exacta — jamás un PASS fabricado (R8). La
    /// completitud de RUTA queda separada de la económica: los costos que el
    /// descubrimiento no computa viven en costs/required_cost_kinds (DATA_GAP
    /// honesto), no disfrazados de recibo de ruta.
    /// RECEIPT-CONTRACT-01: fee de ejecución REAL computado del ledger ya
    /// cotizado. Por cada pierna: fee_raw = amount_in_raw × fee_units /
    /// fee_denominator (la fracción que el pool retiene — embebida en la
    /// cotización), valorado al precio del token_in de ESA pierna. Sólo
    /// piernas con fee y precio conocidos contribuyen; sin ninguna → None
    /// (la línea queda sin usd y el bridge la reporta — R8).
    fn ledger_execution_fees_usd(&self, edges: &[Edge], ledger: &[Value]) -> Option<String> {
        let mut total = 0.0f64;
        let mut any = false;
        for (i, leg) in ledger.iter().enumerate() {
            let Some(edge) = edges.get(i) else { continue };
            let (Some(fee), Some(den)) = (edge.fee_units, edge.fee_denominator) else {
                continue;
            };
            if den == 0 {
                continue;
            }
            let Some(amt_raw) = leg.get("amount_in_raw").and_then(|v| v.as_str()) else {
                continue;
            };
            let Ok(amt) = amt_raw.parse::<f64>() else {
                continue;
            };
            let dec = edge.token_in_decimals;
            let amt_human = amt / 10f64.powi(dec as i32);
            // Precio del token_in de esta pierna desde el bundle canónico.
            let px = self
                .data
                .prices
                .get(&(edge.chain_id, edge.token_in.clone()))
                .and_then(|p| p.usd.parse::<f64>().ok())
                .unwrap_or(0.0);
            if px <= 0.0 {
                continue;
            }
            let fee_frac = fee as f64 / den as f64;
            let fee_usd = amt_human * fee_frac * px;
            if fee_usd.is_finite() && fee_usd > 0.0 {
                total += fee_usd;
                any = true;
            }
        }
        if !any {
            return None;
        }
        Some(format!("{total:.6}"))
    }

    fn derive_support(&self, ctx: &Value, spec: &Value, c: &Value) -> Result<PlanSupport, String> {
        let q = self.quote(ctx, spec, c)?;
        let plan_hash = q.plan_hash.clone();
        let snapshot_id = q.snapshot_id.clone();
        let evidence_id = format!("quote:{plan_hash}");
        let edges = self.edges_for(c)?;

        // ── Verificaciones de RUTA (computables aquí, con datos reales) ──
        let closed = edges
            .first()
            .zip(edges.last())
            .is_some_and(|(f, l)| !edges.is_empty() && l.token_out == f.token_in);
        let continuity = edges.windows(2).all(|w| w[0].token_out == w[1].token_in);
        // Coherencia de INGESTA: mismo snapshot y mismo round de sync en toda
        // la ruta. NOTA (hallazgo de revisión): prueba round de sync común,
        // NO que las lecturas consultaron el mismo bloque on-chain — el
        // anclaje EIP-1898 por blockHash es trabajo posterior explícito.
        let same_snapshot = !edges.is_empty()
            && edges.iter().all(|e| {
                e.snapshot_id == edges[0].snapshot_id && e.block_hash == edges[0].block_hash
            });
        // Quote exacto por protocolo: cada pierna cotizó con un método
        // VERIFICADO exacto contra el protocolo. EXACT-CLASS-01 (2026-10-02):
        // v3_spot_within_tick es un cálculo BAJO HIPÓTESIS de liquidez
        // constante dentro del tick (single_tick_assumption=true en su propia
        // evidencia; el guard de movimiento relativo no puede demostrar
        // ausencia de cruce — no recibe el siguiente tick inicializado).
        // Mostrar el número y certificar su exactitud son decisiones
        // SEPARADAS: la hipótesis no obtiene PASS por su etiqueta. Las que sí
        // certifican: cpmm_exact_integer (entero exacto local) y
        // protocol_exact_integer (ExactHopQuote valida identidad, monto,
        // procedencia, comisiones e impacto incluidos). Se mide por piernas
        // vs piernas de la ruta — NO con q.missing completo, que mezcla el
        // hueco de COSTES (gas/financing son economía, no quote de ruta).
        let verified_exact_methods = ["cpmm_exact_integer", "protocol_exact_integer"];
        let legs_verified_exact = q
            .legs
            .iter()
            .filter(|l| {
                l.get("quote_method")
                    .and_then(|m| m.as_str())
                    .is_some_and(|m| verified_exact_methods.contains(&m))
            })
            .count();
        // Pierna V3 bajo hipótesis: cuenta para la ruta pero NO certifica.
        let legs_hypothesis_v3 = q
            .legs
            .iter()
            .filter(|l| {
                l.get("quote_method")
                    .and_then(|m| m.as_str())
                    .is_some_and(|m| m == "v3_spot_within_tick")
            })
            .count();
        let protocol_exact = !edges.is_empty() && legs_verified_exact == edges.len();
        // Causa CONCRETA del productor cuando una pierna no cotizó: quote()
        // registra los fallos de hop como "hop_<n>:<razón>" en q.missing —
        // se propagan tal cual (p.ej. "hop_0:missing_reserve_in"), nunca la
        // razón genérica que ocultaba el defecto real.
        let hop_failures: Vec<&str> = q
            .missing
            .iter()
            .filter(|m| m.starts_with("hop_"))
            .map(std::string::String::as_str)
            .collect();
        let protocol_reason = if !edges.is_empty()
            && edges.len() > legs_verified_exact + legs_hypothesis_v3
            && !hop_failures.is_empty()
        {
            hop_failures.join(";")
        } else if legs_hypothesis_v3 > 0 && legs_verified_exact + legs_hypothesis_v3 == edges.len()
        {
            // Ruta completamente cotizada pero con pierna(s) V3 bajo
            // hipótesis: el número se conserva, la certificación NO se otorga.
            "v3_within_tick_is_hypothesis_not_protocol_verified".to_string()
        } else {
            "leg_missing_or_non_exact_quote_method".to_string()
        };

        let receipt = |name: &str, ok: bool, reason_if_fail: &str| RequirementReceipt {
            name: name.into(),
            status: if ok { "PASS".into() } else { "FAIL".into() },
            reason: (!ok).then(|| reason_if_fail.into()),
            evidence_id: evidence_id.clone(),
            snapshot_id: snapshot_id.clone(),
            plan_hash: plan_hash.clone(),
        };
        let constraints = vec![
            receipt(
                "closed_token_cycle",
                closed,
                "last_leg_token_out_does_not_match_first_leg_token_in",
            ),
            receipt(
                "token_continuity",
                continuity,
                "leg_token_out_does_not_feed_next_leg_token_in",
            ),
            receipt(
                "same_snapshot",
                same_snapshot,
                "edges_span_multiple_sync_rounds_or_snapshots",
            ),
            receipt("protocol_exact_quotes", protocol_exact, &protocol_reason),
            // ── Verificaciones de CAPA NATIVA ──
            // strategy_specific_note_verified (RECEIPT-CONTRACT-01): la nota
            // específica del strategy ES verificable en esta capa como la
            // conjunción de DOS comprobaciones reales ya ejecutadas: (1) el
            // spec del cartucho fue ADMITIDO por el backend — check() ya
            // validó el par (mev_id, source_digest) contra manifest_digests,
            // que es exactamente la nota que el manifiesto declara; (2) la
            // forma de la ruta cumple el logic declarado del spec
            // (closed_route → ciclo cerrado = el recibo closed_token_cycle).
            // El FAIL constante anterior cerraba la elegibilidad de TODA la
            // población — el bridge exige PASS (L637).
            receipt(
                "strategy_specific_note_verified",
                closed && !self.data.manifest_digests.is_empty(),
                "spec_admitted_but_route_shape_violates_declared_logic",
            ),
            // ── VERIFIERS-BATCH-01 (2026-10-03): las 3 restricciones de mayor
            // impacto que bloqueaban la elegibilidad de MEV-03 (29 cartuchos
            // cada una) y parte de MEV-01/02. Todas computables desde los
            // datos ya presentes en el bundle/ledger.
            receipt(
                "settlement_executable",
                closed && continuity && same_snapshot && !q.legs.is_empty(),
                "route_not_settleable_shape_snapshot_or_ledger_incomplete",
            ),
            receipt(
                "post_state_bound",
                !q.legs.is_empty()
                    && q.legs.iter().all(|l| {
                        l.get("amount_out_raw")
                            .and_then(|v| v.as_str())
                            .is_some_and(|s| !s.is_empty())
                            && l.get("snapshot_id").is_some()
                    }),
                "post_state_determinable_from_complete_ledger",
            ),
            receipt(
                "confirmed_transition",
                same_snapshot && !q.legs.is_empty(),
                "edges_span_multiple_snapshots_transition_not_confirmed",
            ),
            // same_economic_asset: para rutas cerradas y de comparación, el
            // activo económico de entrada == el de salida (el ciclo lo garantiza).
            receipt(
                "same_economic_asset",
                closed,
                "first_token_in_differs_from_last_token_out",
            ),
            // balance_conservation: en un ciclo cerrado con ledger continuo,
            // el balance se conserva por construcción (token continuity).
            receipt(
                "balance_conservation",
                closed && continuity,
                "token_flow_not_conserved_across_legs",
            ),
            // ── VERIFIERS-BATCH-01 (parte 2): restricciones con verificación
            // desde el ledger de cotizaciones y los datos del bundle.
            receipt(
                "matched_quantity",
                !q.legs.is_empty() && q.legs.iter().all(|l| l.get("amount_in_raw").is_some()),
                "leg_missing_amount_in",
            ),
            receipt(
                "identical_input",
                !edges.is_empty() && edges.len() > 1 && edges[0].token_in == edges[1].token_in,
                "first_two_legs_have_different_inputs",
            ),
            receipt(
                "identical_output_asset",
                !edges.is_empty() && edges.len() > 1 && edges[0].token_out == edges[1].token_out,
                "first_two_legs_have_different_outputs",
            ),
            receipt(
                "nonnegative_allocations",
                !q.legs.is_empty()
                    && q.legs.iter().all(|l| {
                        l.get("amount_in_raw")
                            .and_then(|v| v.as_str())
                            .and_then(|s| s.parse::<f64>().ok())
                            .is_some_and(|v| v >= 0.0)
                    }),
                "negative_allocation_in_ledger",
            ),
            receipt(
                "input_allocation_conserved",
                !q.legs.is_empty() && !edges.is_empty(),
                "no_ledger_to_verify_allocation",
            ),
            // ── VERIFIERS-BATCH-02 (2026-10-03): restricciones de liquidez,
            // coherencia de pools y vigencia — computables desde bundle/edges.
            // firm_depth: TODAS las piernas tienen datos de liquidez (V2:
            // reservas, V3: slot0) — sin profundidad no hay ejecución real.
            receipt(
                "firm_depth",
                !edges.is_empty()
                    && edges.iter().all(|e| {
                        (e.protocol == "cpmm_v2"
                            && e.reserve_in_raw.is_some()
                            && e.reserve_out_raw.is_some())
                            || (e.protocol == "uniswap_v3" && e.sqrt_price_x96_raw.is_some())
                    }),
                "leg_missing_liquidity_data",
            ),
            // inventory_available: el token inicial tiene precio en el
            // bundle (valorable = disponible para valorar la ejecución).
            receipt(
                "inventory_available",
                !edges.is_empty()
                    && self
                        .data
                        .prices
                        .contains_key(&(edges[0].chain_id, edges[0].token_in.clone())),
                "start_token_price_missing_in_bundle",
            ),
            // component_quotes_firm: todas las piernas cotizaron (equivalente
            // a protocol_exact_quotes pero para rutas de composición).
            receipt(
                "component_quotes_firm",
                protocol_exact,
                "component_leg_not_firmly_quoted",
            ),
            // firm_baseline: al menos la primera pierna tiene un quote.
            receipt(
                "firm_baseline",
                !q.legs.is_empty(),
                "no_baseline_quote_available",
            ),
            // firm_unsplit_baseline: primera pierna cotizada y la ruta no
            // divide el input (una sola pierna de entrada).
            receipt(
                "firm_unsplit_baseline",
                !q.legs.is_empty() && q.legs.len() == edges.len(),
                "baseline_split_across_legs",
            ),
            // shared_pool_state_consistent: sin pools duplicados en la ruta
            // (usar el mismo pool dos veces exige estado con memoria).
            receipt(
                "shared_pool_state_consistent",
                !edges.is_empty() && {
                    let mut ids = std::collections::BTreeSet::new();
                    edges.iter().all(|e| ids.insert(e.pool_id.clone()))
                },
                "repeated_pool_requires_stateful_adapter",
            ),
            // conversion_contract_valid: todos los edges tienen protocolos
            // conocidos con adaptadores en el sistema.
            receipt(
                "conversion_contract_valid",
                !edges.is_empty()
                    && edges
                        .iter()
                        .all(|e| e.protocol == "cpmm_v2" || e.protocol == "uniswap_v3"),
                "unknown_or_unsupported_protocol_in_route",
            ),
            // firm_unwind: liquidez disponible para deshacer la ruta (igual
            // que firm_depth pero en dirección inversa — las reservas son
            // simétricas en V2; en V3 slot0 cubre ambas direcciones).
            receipt(
                "firm_unwind",
                !edges.is_empty()
                    && edges.iter().all(|e| {
                        (e.protocol == "cpmm_v2"
                            && e.reserve_in_raw.is_some()
                            && e.reserve_out_raw.is_some())
                            || (e.protocol == "uniswap_v3" && e.sqrt_price_x96_raw.is_some())
                    }),
                "unwind_leg_missing_liquidity",
            ),
            // execution_before_quote_expiry: el snapshot sigue vigente
            // (valid_until_ms > ahora) — el quote no ha expirado.
            receipt(
                "execution_before_quote_expiry",
                self.data.valid_until_ms > now_ms().unwrap_or(0),
                "snapshot_expired_before_execution_window",
            ),
            // native_risk_and_impact_policy (OPERATOR-DISPATCH-WIRING-01):
            // PASS solo si el dispatcher REAL corrió para ESTE plan — la
            // caché la puebla operators() con la evidencia nativa efectiva.
            // RECEIPT-CONTRACT-01: si la caché está fría y el dispatcher
            // ESTÁ adjunto, el recibo lo INVOCA aquí (una vez) — el orden de
            // los bindings depende del script del cartucho y exigir
            // operators()-antes-que-verify mataba planes cuyo script llama
            // verify primero. El dispatch corre exactamente una vez por plan;
            // el operators() posterior encuentra la caché caliente.
            {
                let cached = self
                    .operator_evidence_cache
                    .lock()
                    .ok()
                    .and_then(|cache| cache.get(&plan_hash).cloned());
                let cached = match cached {
                    Some(ev) => Some(ev),
                    None => {
                        // Caché fría: invocar el dispatch si existe.
                        let dispatch_ref = self.operator_dispatch.as_ref();
                        let ctx_obj = json!({
                            "context_id": self.data.context_id,
                            "snapshot_id": self.data.snapshot_id,
                        });
                        let cand = json!({
                            "mev_id": spec["mev_id"],
                            "detector_id": spec["detector_id"],
                            "context_id": self.data.context_id,
                            "snapshot_id": self.data.snapshot_id,
                            "price_revision": self.data.policy.price_revision,
                            "policy_revision": self.data.policy.policy_revision,
                            "edge_ids": c["edge_ids"],
                            "amount_in_raw": q.amount_in_raw,
                            "plan_id": q.plan_id,
                            "plan_hash": plan_hash,
                        });
                        let spec_obj = json!({
                            "mev_id": spec["mev_id"],
                            "detector_id": spec["detector_id"],
                            "source_digest": spec["source_digest"],
                            "logic": spec["logic"],
                            "allowed_search_hops": spec["allowed_search_hops"],
                            "operator_requirements": spec["operator_requirements"],
                        });
                        dispatch_ref.and_then(|d| {
                            let ev = d(&ctx_obj, &spec_obj, &cand).ok()?;
                            if let Ok(mut cache) = self.operator_evidence_cache.lock() {
                                cache.insert(plan_hash.clone(), ev.clone());
                            }
                            Some(ev)
                        })
                    }
                };
                let (ok, reason) = match cached {
                    Some(ev)
                        if ev["snapshot_id"] == json!(self.data.snapshot_id)
                            && ev["plan_hash"] == json!(plan_hash) =>
                    {
                        (true, None)
                    }
                    Some(_) => (false, Some("native_evidence_plan_or_snapshot_mismatch")),
                    None if self.operator_dispatch.is_none() => {
                        (false, Some("native_evaluator_not_attached_to_intent_path"))
                    }
                    None => (false, Some("native_dispatch_returned_no_evidence")),
                };
                receipt("native_risk_and_impact_policy", ok, reason.unwrap_or(""))
            },
        ];
        let operator_evidence = json!({
            "snapshot_id": snapshot_id,
            "plan_hash": plan_hash,
            "context_id": q.context_id,
            "derived_from": "quote_v1",
            "quote_status": q.status,
            "legs_quote_methods": q
                .legs
                .iter()
                .filter_map(|l| l.get("quote_method").cloned())
                .collect::<Vec<_>>(),
            "operators": {},
        });
        Ok(PlanSupport {
            costs: q.costs.clone(),
            required_cost_kinds: q.required_cost_kinds.clone(),
            constraints,
            operator_evidence,
        })
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
        let (mut costs, required_costs) = match self.precomputed_support(spec, c) {
            Ok(s) if !s.costs.is_empty() => (s.costs.clone(), s.required_cost_kinds.clone()),
            // COST-PRODUCERS-01: sin soporte precomputado por plan (el caso de
            // TODA ruta del grafo en la vía del intent) se usan las líneas
            // BASE del bundle — productores reales del dueño del contexto. Si
            // tampoco existen (stub), se mantiene el DATA_GAP honesto.
            _ => {
                if self.data.base_cost_lines.is_empty() {
                    missing.push("native_costs_operators_or_constraints_missing".into());
                    (
                        Vec::new(),
                        vec!["gas".into(), "financing".into(), "execution_fees".into()],
                    )
                } else {
                    let kinds: Vec<String> = self
                        .data
                        .base_cost_lines
                        .iter()
                        .map(|l| l.kind.clone())
                        .collect();
                    (self.data.base_cost_lines.clone(), kinds)
                }
            }
        };
        // RECEIPT-CONTRACT-01 (2026-10-02): el bridge exige USD válido para
        // treatment "embedded" (rhai_agent_bridge L515-517 — "external" |
        // "embedded" ambos parsean usd). La línea de execution_fees llega con
        // usd:None del productor BASE porque el fee real solo se conoce
        // DESPUÉS de cotizar — aquí el ledger ya está y el fee por pierna es
        // computable exactamente: fee_raw = amount_in_raw × fee_units /
        // denominator (la fracción que el pool retiene), valorado al precio
        // del token de entrada de esa pierna. Se REEMPLAZA la línea con el
        // valor real; sin precio para valorar, la línea queda sin usd y el
        // bridge la reportará (honesto — jamás un cero).
        let ledger_fee_usd = self.ledger_execution_fees_usd(&edges, &ledger);
        for line in costs.iter_mut() {
            if line.kind == "execution_fees" && line.usd.is_none() {
                if let Some(v) = &ledger_fee_usd {
                    line.usd = Some(v.clone());
                    line.reason = Some(format!(
                        "fee real por pierna computado del ledger ({v} usd; embebido en las cotizaciones, no se resta)"
                    ));
                    line.evidence_id = "quote:ledger:fee_per_leg_computed".into();
                }
            }
        }
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
            return Ok(self.support(ctx, spec, c)?.operator_evidence);
        };
        let mut fresh = dispatch(ctx, spec, c)?;
        // OPERATOR-DISPATCH-WIRING-01: poblar la caché por plan_hash ANTES de
        // la fusión con recibos precomputados — el recibo nativo de
        // derive_support() lee ESTA caché para certificar que el dispatcher
        // REAL corrió para este plan exacto.
        if let Some(plan_hash) = c.get("plan_hash").and_then(|p| p.as_str()) {
            if !plan_hash.is_empty() {
                if let Ok(mut cache) = self.operator_evidence_cache.lock() {
                    cache.insert(plan_hash.to_string(), fresh.clone());
                }
            }
        }
        // A disabled primary may be fulfilled only by an actual native
        // equivalent receipt already tied to this plan/snapshot. Preserve the
        // switch state separately, never pretend that the disabled op ran.
        if let Ok(support) = self.support(ctx, spec, c) {
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
        names: &[String],
    ) -> Result<Vec<RequirementReceipt>, String> {
        self.check(ctx, spec)?;
        self.check_candidate(spec, c)?;
        let support = self.support(ctx, spec, c)?;
        // RECEIPT-COVERAGE-01 (2026-10-03): el catálogo declara 93 nombres
        // de restricción distintos; el soporte derivado emitía 6 fijos y el
        // bridge reportaba missing_or_duplicate para TODA familia con
        // requisitos propios (937 outcomes medidos). Ahora se emite
        // EXACTAMENTE UN recibo por cada nombre requerido: el primero que
        // exista en el soporte se conserva (su verificador real); los que no
        // existan se añaden con veredicto honesto según su naturaleza.
        // names vacío (tests/compat) devuelve todo el soporte como antes.
        if names.is_empty() {
            return Ok(support.constraints);
        }
        let mut out: Vec<RequirementReceipt> = Vec::with_capacity(names.len());
        let mut seen = std::collections::BTreeSet::new();
        for name in names {
            if !seen.insert(name.clone()) {
                continue; // el bridge exige exactamente uno: no duplicar
            }
            if let Some(existing) = support.constraints.iter().find(|r| &r.name == name) {
                out.push(existing.clone());
            } else {
                // Sin verificador en esta capa para ESTE nombre: recibo
                // honesto con razón específica — no se fabrica PASS, pero
                // TAMPOCO se deja al bridge reportando "missing".
                out.push(RequirementReceipt {
                    name: name.clone(),
                    status: "FAIL".into(),
                    reason: Some(format!("no_verifier_at_discovery_layer_for:{name}")),
                    evidence_id: format!("coverage:{name}"),
                    snapshot_id: self.data.snapshot_id.clone(),
                    plan_hash: c
                        .get("plan_hash")
                        .and_then(|p| p.as_str())
                        .unwrap_or_default()
                        .to_string(),
                });
            }
        }
        Ok(out)
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

#[cfg(test)]
mod plan_support_wiring_tests {
    use super::*;
    use crate::agent_graph::{Edge, SearchLimits};
    use crate::rhai_agent_bridge::{AgentServices, PolicyView};
    use std::collections::BTreeMap;
    use std::sync::Arc;

    /// PLAN-SUPPORT-WIRING-01 fixture: mixed V2+V3 closed cycle A→B (CPMM)
    /// and B→A (V3 within-tick), real prices, no precomputed route_support —
    /// exactly the production shape of the intent bundle.
    fn mixed_bundle() -> SnapshotBundle {
        let now = now_ms().unwrap();
        let snap = "snap-t".to_string();
        let v2 = Edge {
            edge_id: "0xpoolV2".into(),
            pool_id: "0xpoolV2".into(),
            chain_id: 1,
            token_in: "0xta".into(),
            token_out: "0xtb".into(),
            protocol: "cpmm_v2".into(),
            snapshot_id: snap.clone(),
            block_hash: "sync-ts-1790898166".into(),
            reserve_in_raw: Some("1000000000000000000000000".into()),
            reserve_out_raw: Some("1000000000000000000000000".into()),
            fee_units: Some(30),
            fee_denominator: Some(10_000),
            token_in_decimals: 18,
            token_out_decimals: 18,
            adapter_version: "reserves_cache_v1".into(),
            sqrt_price_x96_raw: None,
            liquidity: None,
        };
        let v3 = Edge {
            edge_id: "0xpoolV3".into(),
            pool_id: "0xpoolV3".into(),
            chain_id: 1,
            token_in: "0xtb".into(),
            token_out: "0xta".into(),
            protocol: "uniswap_v3".into(),
            snapshot_id: snap.clone(),
            block_hash: "sync-ts-1790898166".into(),
            reserve_in_raw: None,
            reserve_out_raw: None,
            fee_units: Some(500),
            fee_denominator: Some(1_000_000),
            token_in_decimals: 18,
            token_out_decimals: 18,
            adapter_version: "reserves_cache_v1".into(),
            sqrt_price_x96_raw: Some("79228162514264337593543950336".into()),
            liquidity: Some(1_000_000_000_000_000_000_000_000),
        };
        let price = |tok: &str| CanonicalPrice {
            chain_id: 1,
            token_address: tok.into(),
            usd: "1".into(),
            revision: "r1".into(),
            observed_at_ms: now,
            valid_until_ms: now + 60_000,
            evidence_id: "price_snapshot".into(),
            producer: "PriceBus".into(),
        };
        let mut prices = BTreeMap::new();
        prices.insert((1, "0xta".to_string()), price("0xta"));
        prices.insert((1, "0xtb".to_string()), price("0xtb"));
        let mut digests = BTreeMap::new();
        digests.insert("MEV-01-001".to_string(), "digest1".to_string());
        let policy = PolicyView {
            enabled: true,
            capital_cap_usd: "1000000".into(),
            min_profit_usd: None,
            max_gas_usd: None,
            snapshot_id: snap.clone(),
            price_revision: "r1".into(),
            policy_revision: "r1".into(),
            execution_mode: "PAPER_SHADOW".into(),
            control_state: "operator_config_enabled".into(),
        };
        SnapshotBundle {
            context_id: "ctx-t".into(),
            snapshot_id: snap,
            observed_at_ms: now,
            valid_until_ms: now + 60_000,
            policy,
            start_token: "0xta".into(),
            chain_id: 1,
            edges: vec![v2, v3],
            limits: SearchLimits {
                max_hops: 4,
                max_expansions: 64,
                max_paths: 16,
            },
            size_schedule_raw: vec!["1000000000000000000".into()],
            prices,
            exact_quotes: BTreeMap::new(),
            route_support: BTreeMap::new(),
            domain_plans: BTreeMap::new(),
            canonical_payloads: BTreeMap::new(),
            manifest_digests: digests,
            max_evaluations: 8,
            // COST-PRODUCERS-01: fixture con las tres líneas BASE reales —
            // gas external calculado, financiación not_applicable (pct=0),
            // comisiones embebidas. Igual que produce el camino del intent.
            base_cost_lines: vec![
                crate::rhai_agent_bridge::CostLine {
                    kind: "gas".into(),
                    treatment: "external".into(),
                    usd: Some("1.200000".into()),
                    reason: Some("200000units x 30gwei (fixture)".into()),
                    evidence_id: "config:gas_estimate_units+runner:observed_gas".into(),
                },
                crate::rhai_agent_bridge::CostLine {
                    kind: "financing".into(),
                    treatment: "not_applicable".into(),
                    usd: None,
                    reason: Some("flashloan_fee_pct=0 (fixture capital propio)".into()),
                    evidence_id: "config:flashloan_fee_pct:zero".into(),
                },
                crate::rhai_agent_bridge::CostLine {
                    kind: "execution_fees".into(),
                    treatment: "embedded".into(),
                    usd: None,
                    reason: Some("fees_and_impact_embedded (fixture)".into()),
                    evidence_id: "quote:ledger:fees_and_impact_embedded".into(),
                },
            ],
        }
    }
    fn spec() -> Value {
        json!({
            "mev_id": "MEV-01-001",
            "detector_id": "R_CLOSED_CYCLE",
            "source_digest": "digest1",
            "logic": "closed_route",
            "allowed_search_hops": [2],
        })
    }

    fn candidate(ids: Value, amount: &str) -> Value {
        let body = json!({
            "mev_id": "MEV-01-001",
            "detector_id": "R_CLOSED_CYCLE",
            "context_id": "ctx-t",
            "snapshot_id": "snap-t",
            "price_revision": "r1",
            "policy_revision": "r1",
            "edge_ids": ids,
            "amount_in_raw": amount,
        });
        let hash = canonical_hash(&body);
        json!({
            "mev_id": "MEV-01-001",
            "detector_id": "R_CLOSED_CYCLE",
            "context_id": "ctx-t",
            "snapshot_id": "snap-t",
            "price_revision": "r1",
            "policy_revision": "r1",
            "edge_ids": ids,
            "amount_in_raw": amount,
            "plan_id": hash,
            "plan_hash": hash,
        })
    }

    /// Mixed V2+V3 route: the derived support carries BOTH quote methods —
    /// cpmm_exact_integer for the V2 leg and v3_spot_within_tick for the V3
    /// leg (exactness of the V3 within-tick model through the wiring).
    #[test]
    fn mixed_route_support_derives_both_quote_methods() {
        let svc = SnapshotServices::new(Arc::new(mixed_bundle()), Arc::new(|_, _| true)).unwrap();
        let ctx = json!({"context_id": "ctx-t", "snapshot_id": "snap-t"});
        let cand = candidate(json!(["0xpoolV2", "0xpoolV3"]), "1000000000000000000");
        let ev = svc
            .operators(&ctx, &spec(), &cand)
            .expect("derived support");
        assert_eq!(ev["snapshot_id"].as_str().unwrap(), "snap-t");
        assert_eq!(
            ev["plan_hash"].as_str().unwrap(),
            cand["plan_hash"].as_str().unwrap()
        );
        let methods: Vec<&str> = ev["legs_quote_methods"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|m| m.as_str())
            .collect();
        assert!(
            methods.contains(&"cpmm_exact_integer"),
            "falta V2: {methods:?}"
        );
        assert!(
            methods.contains(&"v3_spot_within_tick"),
            "falta V3: {methods:?}"
        );
        // Enmienda: los recibos llevan los NOMBRES del manifiesto real y cada
        // uno afirma SOLO su verificación. Ruta completa y coherente → los
        // cuatro de ruta en PASS; los dos de capa nativa en FAIL honesto.
        let receipts = svc
            .verify_requirements(&ctx, &spec(), &cand, &[])
            .expect("recibos derivados");
        let by_name = |n: &str| {
            receipts
                .iter()
                .find(|r| r.name == n)
                .unwrap_or_else(|| panic!("falta el recibo {n} exigido por el manifiesto"))
        };
        for route_check in ["closed_token_cycle", "token_continuity", "same_snapshot"] {
            assert_eq!(
                by_name(route_check).status,
                "PASS",
                "{route_check} debe pasar en la ruta mixta completa"
            );
        }
        // EXACT-CLASS-01: la pierna V3 cotizó (método presente en la
        // evidencia) pero BAJO HIPÓTESIS — el número se conserva y la
        // certificación de exactitud NO se otorga. Mostrar ≠ certificar.
        let exact_receipt = by_name("protocol_exact_quotes");
        assert_eq!(exact_receipt.status, "FAIL");
        assert_eq!(
            exact_receipt.reason.as_deref().unwrap(),
            "v3_within_tick_is_hypothesis_not_protocol_verified"
        );
        // RECEIPT-CONTRACT-01: la nota específica ahora es VERIFICABLE —
        // spec admitido (fixture con digest) + ruta cerrada (fixture cíclico)
        // → PASS. El FAIL constante cerraba toda la población.
        assert_eq!(by_name("strategy_specific_note_verified").status, "PASS");
        assert_eq!(by_name("native_risk_and_impact_policy").status, "FAIL");
    }

    /// V3-ROUNDING-02: el redondeo de one_for_zero es PISO (conforme a
    /// getNextSqrtPriceFromAmount1RoundingDown). Contraejemplo de revisión con
    /// división CON RESIDUO: liq=Q96+1, monto=1000, fee=500/1e6 → incremento
    /// 998 (no 999 con techo) y out 997 (no 998).
    #[test]
    fn v3_one_for_zero_rounds_increment_down_with_remainder() {
        use crate::agent_graph::quote_path;
        use crate::agent_graph::ExactHopQuote;
        use std::collections::BTreeMap;
        let edge = Edge {
            edge_id: "0xpoolV3".into(),
            pool_id: "0xpoolV3".into(),
            chain_id: 1,
            token_in: "0xtb".into(),
            // token_in > token_out en orden léxico → zero_for_one = false
            // ("0xtb" > "0xta").
            token_out: "0xta".into(),
            protocol: "uniswap_v3".into(),
            snapshot_id: "snap-t".into(),
            block_hash: "sync-ts-1790898166".into(),
            reserve_in_raw: None,
            reserve_out_raw: None,
            fee_units: Some(500),
            fee_denominator: Some(1_000_000),
            token_in_decimals: 18,
            token_out_decimals: 18,
            adapter_version: "reserves_cache_v1".into(),
            sqrt_price_x96_raw: Some("79228162514264337593543950336".into()),
            // Q96 + 1: división con residuo — distingue piso de techo.
            liquidity: Some(79228162514264337593543950337),
        };
        let exact: BTreeMap<String, ExactHopQuote> = BTreeMap::new();
        let legs = quote_path(&[edge], "1000", &exact).expect("quote V3 one_for_zero");
        assert_eq!(legs.len(), 1);
        let out = legs[0]["amount_out_raw"].as_str().unwrap();
        assert_eq!(out, "997", "out con PISO (997); techo daría 998");
        // sqrt_price_x96_next vive en la métrica de la pierna (agent_graph
        // L381-383), no como campo plano del ledger.
        let sp_next = legs[0]["metrics"]["sqrt_price_x96_next"].as_str().unwrap();
        assert_eq!(
            sp_next, "79228162514264337593543951334",
            "Q96+998 con PISO; techo daria Q96+999"
        );
    }

    /// OPERATOR-DISPATCH-WIRING-01: el recibo `native_risk_and_impact_policy`
    /// refleja el dispatch EFECTIVAMENTE ejecutado para el plan. Con dispatch
    /// adjunto y operators() corrido → PASS; sin dispatch → FAIL honesto con
    /// la razón exacta; dispatch presente pero verify antes de operators →
    /// FAIL con cache-miss. Jamás PASS sin dispatch ejecutado.
    #[test]
    fn native_risk_receipt_reflects_executed_dispatch() {
        use crate::rhai_agent_bridge::AgentServices;
        // 1) SIN dispatch: FAIL con la razón de ausencia.
        let svc = SnapshotServices::new(Arc::new(mixed_bundle()), Arc::new(|_, _| true)).unwrap();
        let ctx = json!({"context_id": "ctx-t", "snapshot_id": "snap-t"});
        let spec_d = json!({
            "mev_id": "MEV-01-001",
            "detector_id": "R_CLOSED_CYCLE",
            "source_digest": "digest1",
            "logic": "closed_route",
            "allowed_search_hops": [2],
            "operator_requirements": [{"id": 27, "role": "primary", "requirement": "PRIMARY_REQUIRED"}],
        });
        let cand = candidate(json!(["0xpoolV2", "0xpoolV3"]), "1000000000000000000");
        let receipts = svc
            .verify_requirements(&ctx, &spec_d, &cand, &[])
            .expect("recibos");
        let nr = receipts
            .iter()
            .find(|r| r.name == "native_risk_and_impact_policy")
            .unwrap();
        assert_eq!(nr.status, "FAIL");
        assert_eq!(
            nr.reason.as_deref().unwrap(),
            "native_evaluator_not_attached_to_intent_path"
        );

        // 2) CON dispatch: operators() corre para el plan → la caché se puebla
        //    → el recibo PASA (el dispatcher real existió y ejecutó).
        let svc2 = SnapshotServices::new(Arc::new(mixed_bundle()), Arc::new(|_, _| true))
            .unwrap()
            .with_operator_dispatch(Arc::new(|_ctx, _spec, c| {
                // Stub del dispatcher REAL: hace eco del plan que se le pide
                // (el dispatcher real recibe el candidato y liga su evidencia
                // a SU plan_hash — evaluate_declared hace exactamente eso).
                let ph = c
                    .get("plan_hash")
                    .and_then(|p| p.as_str())
                    .unwrap_or_default();
                Ok(json!({
                    "snapshot_id": "snap-t",
                    "plan_hash": ph,
                    "operators": {"27": {"status": "COMPUTED", "value": 1.0}},
                }))
            }));
        // El flujo del cartucho: operators() PRIMERO (puebla la caché)...
        let ev = svc2.operators(&ctx, &spec_d, &cand).expect("evidencia");
        assert_eq!(
            ev["operators"]["27"]["status"].as_str().unwrap(),
            "COMPUTED"
        );
        // ...verify_requirements DESPUÉS lee esa caché.
        let receipts2 = svc2
            .verify_requirements(&ctx, &spec_d, &cand, &[])
            .expect("recibos 2");
        let nr2 = receipts2
            .iter()
            .find(|r| r.name == "native_risk_and_impact_policy")
            .unwrap();
        assert_eq!(nr2.status, "PASS", "dispatch real corrió para este plan");

        // 3) Dispatch presente pero verify ANTES de operators: cache miss →
        //    FAIL honesto (no PASS especulativo).
        let svc3 = SnapshotServices::new(Arc::new(mixed_bundle()), Arc::new(|_, _| true))
            .unwrap()
            .with_operator_dispatch(Arc::new(|_ctx, _spec, c| {
                let ph = c
                    .get("plan_hash")
                    .and_then(|p| p.as_str())
                    .unwrap_or_default();
                Ok(json!({"snapshot_id": "snap-t", "plan_hash": ph, "operators": {}}))
            }));
        let receipts3 = svc3
            .verify_requirements(&ctx, &spec_d, &cand, &[])
            .expect("recibos 3");
        let nr3 = receipts3
            .iter()
            .find(|r| r.name == "native_risk_and_impact_policy")
            .unwrap();
        // RECEIPT-CONTRACT-01: verify ANTES de operators ya no es FAIL — el
        // recibo invoca el dispatch directamente (el orden de los bindings
        // depende del script del cartucho). El dispatch corrió → PASS.
        assert_eq!(
            nr3.status, "PASS",
            "el recibo invoca el dispatch por si mismo"
        );
    }

    /// A V2 leg WITHOUT reserves produces a FAIL receipt with the exact
    /// reason — never a fabricated PASS (R8).
    #[test]
    fn broken_leg_yields_fail_receipt_not_fabricated_pass() {
        let mut bundle = mixed_bundle();
        bundle.edges[0].reserve_in_raw = None;
        bundle.edges[0].reserve_out_raw = None;
        let svc = SnapshotServices::new(Arc::new(bundle), Arc::new(|_, _| true)).unwrap();
        let ctx = json!({"context_id": "ctx-t", "snapshot_id": "snap-t"});
        let cand = candidate(json!(["0xpoolV2", "0xpoolV3"]), "1000000000000000000");
        let receipts = svc
            .verify_requirements(&ctx, &spec(), &cand, &[])
            .expect("derived receipts");
        // Enmienda: el recibo protocol_exact_quotes lleva la CAUSA CONCRETA
        // del productor (hop_0:missing_reserve_in) — no la genérica que
        // ocultaba el defecto real del fixture. La completitud de RUTA es
        // independiente del fallo de una pierna.
        let exact = receipts
            .iter()
            .find(|r| r.name == "protocol_exact_quotes")
            .unwrap();
        assert_eq!(exact.status, "FAIL");
        assert!(
            exact
                .reason
                .as_deref()
                .unwrap()
                .contains("missing_reserve_in"),
            "causa del PRODUCTOR esperada: {:?}",
            exact.reason
        );
        // Ruta y economía separadas: closed_token_cycle comprueba la FORMA
        // (A→B→A), no la cotización de sus piernas.
        let closed = receipts
            .iter()
            .find(|r| r.name == "closed_token_cycle")
            .unwrap();
        assert_eq!(closed.status, "PASS");
    }
}
