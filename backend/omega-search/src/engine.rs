//! The autonomous search engine: generations, invalidation, fair share, budget,
//! and the honest output contract.
//!
//! PROMPT §7 pseudocode is implemented literally:
//!
//! ```text
//! al recibir un cambio relevante:
//!     construir contexto coherente y determinar estrategias afectadas
//!     invalidar evidencia y candidatos dependientes del estado anterior
//!     explorar alternativas con presupuesto y reparto justo
//!     cotizar y optimizar tamaños/financiación
//!     publicar resultados parciales y mejor alternativa vigente
//!     al agotar presupuesto:
//!         publicar alcance, mejor resultado, brecha y razón de parada
//!         continuar con eventos nuevos o trabajo pendiente aún vigente
//! ```
//!
//! ## Generation discipline
//!
//! [`CandidateLedger::open_generation`] starts a new generation bound to a new
//! snapshot and **invalidates the whole current set**. A candidate whose
//! generation is not the current one is refused by [`CandidateLedger::admit`].
//! The historical maximum lives in its own field and is a **record**, never a
//! statement about the present: [`SearchOutcome`] reports `best_computed` from
//! the current generation only, so a stale positive number cannot masquerade as
//! the vigente alternative.
//!
//! ## Fair share
//!
//! Families are hop-count families by default ([`family_of_hop_count`]). A
//! 2-hop DEX cross-arb family is orders of magnitude more numerous than a 7-hop
//! family, so without a scheduler the long routes would never be evaluated.
//! [`FairShareQueue`] is a deficit round-robin: each family accrues a quantum
//! proportional to its weight, so a family with one pending item is served in
//! the first round no matter how many items its neighbour has.
//!
//! ## Minimum vs objective
//!
//! [`HardMinimum`] (execution floor) and [`SearchTarget`] (search objective) are
//! **different types** so they cannot be passed for one another. A net that is
//! positive but below the hard minimum is reported as a positive computed
//! diagnosis and is *not* authorized: PROMPT §7 — *"Si el mínimo vigente es USD
//! 50, un neto USD 0,20 puede verse positivo sin declararse objetivo cumplido ni
//! autorizar ejecución."*

use crate::cycles::{
    enumerate_bounded, Cycle, EnumerateLimits, PruneAccounting, TruncationReason,
};
use crate::eval::{evaluate_cycle, CycleEval, EvalError, ExternalCosts};
use crate::graph::{EdgeId, GraphPolicy, MultiGraph, PoolIndex};
use crate::units::Raw;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

/// Identity of the state generation a candidate belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GenerationId(pub u64);

/// Identity of the coherent context a generation is bound to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SnapshotId(pub u64);

/// Strategy family used for fair sharing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FamilyId(pub u32);

impl FamilyId {
    pub const fn new(v: u32) -> Self {
        FamilyId(v)
    }
}

/// Default family mapping: one family per cycle length.
///
/// This is a policy function, not a hardcoded taxonomy: callers that group
/// strategies differently can ignore it and key the queue themselves.
pub fn family_of_hop_count(hops: usize) -> FamilyId {
    FamilyId(hops as u32)
}

/// How a candidate entered the search.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CandidateOrigin {
    /// The route the card was originally bound to — kept as a diagnosis even
    /// when it loses.
    Seed,
    /// Produced by bounded enumeration.
    Enumerated,
    /// Proposed by the exact marginal-product model (`-log(rate)` in integer
    /// form) — a candidate generator, **never** proof of finite-size benefit.
    MarginalProduct,
}

/// The outcome of trying to evaluate a candidate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CandidateState {
    Evaluated(CycleEval),
    /// A named failure. Absence of a number is never rendered as `net = 0`.
    NotEvaluated(EvalError),
}

/// A candidate bound to the generation and snapshot that produced it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScoredCandidate {
    pub cycle: Cycle,
    pub state: CandidateState,
    pub generation: GenerationId,
    pub snapshot: SnapshotId,
    pub family: FamilyId,
    pub origin: CandidateOrigin,
    pub hops: usize,
    /// Pool indices touched (cached: invalidation must not need the graph).
    pub pools: BTreeSet<PoolIndex>,
    /// Pool ids touched (for event-driven invalidation by pool).
    pub pool_ids: Vec<u64>,
}

impl ScoredCandidate {
    /// `Some(net)` when a number was computed — including a real `0` and a real
    /// loss. `None` when the candidate could not be evaluated.
    pub fn net_raw(&self) -> Option<Raw> {
        match &self.state {
            CandidateState::Evaluated(e) => Some(e.net_raw),
            CandidateState::NotEvaluated(_) => None,
        }
    }

    pub fn is_evaluated(&self) -> bool {
        matches!(self.state, CandidateState::Evaluated(_))
    }

    pub fn not_evaluated_reason(&self) -> Option<&EvalError> {
        match &self.state {
            CandidateState::Evaluated(_) => None,
            CandidateState::NotEvaluated(e) => Some(e),
        }
    }

    /// Max-first ordering key: `(net, −hops)`. Only evaluated candidates rank.
    fn rank_key(&self) -> (i128, i64) {
        match self.net_raw() {
            Some(n) => (n.get(), -(self.hops as i64)),
            None => (i128::MIN, -(self.hops as i64)),
        }
    }

    pub fn touches_pool(&self, pool_id: u64) -> bool {
        self.pool_ids.contains(&pool_id)
    }
}

/// Execution floor. A candidate below it is **not authorized**, whatever its
/// sign. Deliberately a different type from [`SearchTarget`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HardMinimum(pub Raw);

/// Search objective. Reaching it is a different question from being positive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SearchTarget(pub Raw);

/// Current set + historical record, with generation-bounded admission.
#[derive(Debug, Clone)]
pub struct CandidateLedger {
    generation: GenerationId,
    snapshot: SnapshotId,
    next_generation: u64,
    current: Vec<ScoredCandidate>,
    /// Best net ever admitted. A RECORD: never presented as the current best.
    historical_max: Option<ScoredCandidate>,
    invalidated: u64,
    refused_stale: u64,
    duplicates_skipped: u64,
}

impl CandidateLedger {
    /// Open the first generation for `snapshot`.
    pub fn new(snapshot: SnapshotId) -> Self {
        CandidateLedger {
            generation: GenerationId(1),
            snapshot,
            next_generation: 2,
            current: Vec::new(),
            historical_max: None,
            invalidated: 0,
            refused_stale: 0,
            duplicates_skipped: 0,
        }
    }

    /// Start a new generation bound to `snapshot`.
    ///
    /// Every current candidate is invalidated: a positive result from the
    /// previous state does **not** stay vigente. `historical_max` is preserved
    /// because it is a record, not a claim.
    pub fn open_generation(&mut self, snapshot: SnapshotId) -> GenerationId {
        self.invalidated += self.current.len() as u64;
        self.current.clear();
        self.snapshot = snapshot;
        self.generation = GenerationId(self.next_generation);
        self.next_generation += 1;
        self.generation
    }

    pub fn generation(&self) -> GenerationId {
        self.generation
    }

    pub fn snapshot(&self) -> SnapshotId {
        self.snapshot
    }

    /// Admit a candidate. `false` when it belongs to another generation — the
    /// stale-admission path, counted rather than silently dropped.
    ///
    /// A cycle already present in the current set is **deduplicated** rather
    /// than admitted twice: the same physical route discovered with different
    /// provenance is one route, not two alternatives. Provenance of the first
    /// admission wins (the seed keeps `CandidateOrigin::Seed`).
    pub fn admit(&mut self, candidate: ScoredCandidate) -> bool {
        if candidate.generation != self.generation || candidate.snapshot != self.snapshot {
            self.refused_stale += 1;
            return false;
        }
        if self
            .current
            .iter()
            .any(|c| c.cycle.edges == candidate.cycle.edges)
        {
            self.duplicates_skipped += 1;
            return false;
        }
        if let Some(net) = candidate.net_raw() {
            let better = match &self.historical_max {
                None => true,
                Some(h) => match h.net_raw() {
                    Some(hn) => net > hn,
                    None => true,
                },
            };
            if better {
                self.historical_max = Some(candidate.clone());
            }
        }
        self.current.push(candidate);
        true
    }

    pub fn current(&self) -> &[ScoredCandidate] {
        &self.current
    }

    /// Best **currently vigente** candidate (current generation only).
    pub fn current_best(&self) -> Option<&ScoredCandidate> {
        self.ranked_current().into_iter().next()
    }

    /// Current candidates, best first, deterministic.
    pub fn ranked_current(&self) -> Vec<&ScoredCandidate> {
        let mut evaluated: Vec<&ScoredCandidate> =
            self.current.iter().filter(|c| c.is_evaluated()).collect();
        evaluated.sort_by(|a, b| {
            b.rank_key()
                .cmp(&a.rank_key())
                .then_with(|| a.cycle.edges.cmp(&b.cycle.edges))
        });
        evaluated
    }

    /// The historical record. NOT the vigente alternative.
    pub fn historical_max(&self) -> Option<&ScoredCandidate> {
        self.historical_max.as_ref()
    }

    /// Invalidate every current candidate that touches `pool_id`.
    ///
    /// Returns how many were invalidated. The historical record is untouched:
    /// invalidating evidence does not erase the fact that it existed.
    pub fn invalidate_pool(&mut self, pool_id: u64) -> usize {
        let before = self.current.len();
        self.current.retain(|c| !c.touches_pool(pool_id));
        let removed = before - self.current.len();
        self.invalidated += removed as u64;
        removed
    }

    pub fn invalidated_count(&self) -> u64 {
        self.invalidated
    }

    pub fn refused_stale_count(&self) -> u64 {
        self.refused_stale
    }

    /// Duplicate cycles refused by admission (deduplication at the ledger).
    pub fn duplicates_skipped(&self) -> u64 {
        self.duplicates_skipped
    }
}

/// One unit of pending evaluation work.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkItem {
    pub cycle: Cycle,
    pub family: FamilyId,
    pub origin: CandidateOrigin,
}

/// Deficit round-robin across strategy families.
#[derive(Debug, Clone)]
pub struct FairShareQueue {
    quanta: BTreeMap<FamilyId, u64>,
    order: Vec<FamilyId>,
    deficit: BTreeMap<FamilyId, i64>,
    pending: BTreeMap<FamilyId, VecDeque<WorkItem>>,
}

impl FairShareQueue {
    /// Build from `(family, weight)` pairs. A weight of `0` is rejected: a
    /// family with no quantum could never run, which is starvation by config.
    pub fn new(weights: &[(FamilyId, u64)]) -> Self {
        let mut quanta = BTreeMap::new();
        let mut order = Vec::new();
        for (family, weight) in weights {
            if *weight == 0 {
                continue;
            }
            if quanta.insert(*family, *weight).is_none() {
                order.push(*family);
            }
        }
        order.sort();
        let deficit = order.iter().map(|f| (*f, 0i64)).collect();
        let pending = order.iter().map(|f| (*f, VecDeque::new())).collect();
        FairShareQueue {
            quanta,
            order,
            deficit,
            pending,
        }
    }

    pub fn knows(&self, family: FamilyId) -> bool {
        self.quanta.contains_key(&family)
    }

    /// Enqueue. `false` when the family has no configured quantum, so the
    /// caller learns about the unconfigured family instead of losing the work.
    pub fn push(&mut self, item: WorkItem) -> bool {
        match self.pending.get_mut(&item.family) {
            Some(queue) => {
                queue.push_back(item);
                true
            }
            None => false,
        }
    }

    pub fn pending_len(&self) -> usize {
        self.pending.values().map(|q| q.len()).sum()
    }

    pub fn pending_of(&self, family: FamilyId) -> usize {
        self.pending.get(&family).map(|q| q.len()).unwrap_or(0)
    }

    /// Serve up to `budget` items, one quantum per family per round.
    ///
    /// Unserved items **stay queued** — the work remains vigente for the next
    /// budget round, which is what lets the system continue after a stop.
    pub fn drain_with_budget(&mut self, budget: u64) -> Vec<WorkItem> {
        let mut served: Vec<WorkItem> = Vec::new();
        let mut spent: u64 = 0;
        if budget == 0 || self.order.is_empty() {
            return served;
        }
        loop {
            let mut progressed = false;
            for family in self.order.clone() {
                if spent >= budget {
                    return served;
                }
                let quantum = *self.quanta.get(&family).unwrap_or(&0) as i64;
                let deficit = self.deficit.entry(family).or_insert(0);
                *deficit += quantum;
                loop {
                    if spent >= budget {
                        return served;
                    }
                    if *self.deficit.get(&family).unwrap_or(&0) <= 0 {
                        break;
                    }
                    let next = match self.pending.get_mut(&family) {
                        Some(queue) => queue.pop_front(),
                        None => None,
                    };
                    match next {
                        None => break,
                        Some(item) => {
                            served.push(item);
                            spent += 1;
                            progressed = true;
                            *self.deficit.entry(family).or_insert(0) -= 1;
                        }
                    }
                }
            }
            if !progressed {
                return served;
            }
        }
    }
}

/// Exact integer form of the marginal `-log(rate)` model.
///
/// A marginal rate for one edge, expressed as an exact rational. The adapter
/// supplies these from the same snapshot as the reserves; the engine never
/// derives them from a float.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MarginalRate {
    pub num: i128,
    pub den: i128,
}

/// Verdict of the marginal model for one cycle. `MissingRate` is a third state,
/// distinct from "below one".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarginalVerdict {
    AboveOne,
    AtOrBelowOne,
    MissingRate(EdgeId),
    Overflow,
    DegenerateRate(EdgeId),
}

/// Counters for the marginal generator, reported so its coverage is visible.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MarginalAccounting {
    pub above_one: usize,
    pub at_or_below_one: usize,
    pub missing_rate: usize,
    pub overflow: usize,
    pub degenerate_rate: usize,
}

fn gcd(mut a: i128, mut b: i128) -> i128 {
    a = a.abs();
    b = b.abs();
    while b != 0 {
        let t = a % b;
        a = b;
        b = t;
    }
    if a == 0 {
        1
    } else {
        a
    }
}

/// Evaluate `∏ rate_i · (1 − fee_i) > 1` exactly, with cross-cancellation.
///
/// This is the `-log(rate_after_fee)` model expressed without floats: the log
/// of a product is positive exactly when the product exceeds one. It **only
/// generates and labels candidates**; PROMPT §6 is explicit that it does not
/// prove benefit at finite size, and [`SearchOutcome`] never uses it as a
/// verdict — the verdict is always the integer evaluation at an explicit size.
pub fn marginal_product_verdict(
    g: &MultiGraph,
    cycle: &Cycle,
    rates: &BTreeMap<EdgeId, MarginalRate>,
) -> MarginalVerdict {
    // Reduced fraction accumulator: the model's product so far.
    let mut acc_num: i128 = 1;
    let mut acc_den: i128 = 1;
    for edge_id in &cycle.edges {
        let edge = g.edge(*edge_id);
        let rate = match rates.get(edge_id) {
            Some(r) => *r,
            None => return MarginalVerdict::MissingRate(*edge_id),
        };
        if rate.den == 0 || rate.num < 0 || rate.den < 0 {
            return MarginalVerdict::DegenerateRate(*edge_id);
        }
        if rate.num == 0 {
            return MarginalVerdict::AtOrBelowOne;
        }
        // Two factors per hop: the marginal price rate and the fee retention.
        let factors = [
            (rate.num, rate.den),
            (
                crate::units::BPS_DENOM - edge.fee_bps.get() as i128,
                crate::units::BPS_DENOM,
            ),
        ];
        for (n_raw, d_raw) in factors {
            if n_raw == 0 {
                return MarginalVerdict::AtOrBelowOne;
            }
            // Standard cross-cancellation keeps both sides small enough that a
            // 7-hop product does not overflow.
            let mut n = n_raw / gcd(n_raw, d_raw);
            let mut d = d_raw / gcd(n_raw, d_raw);
            let g1 = gcd(n, acc_den);
            n /= g1;
            acc_den /= g1;
            let g2 = gcd(d, acc_num);
            d /= g2;
            acc_num /= g2;
            acc_num = match acc_num.checked_mul(n) {
                Some(v) => v,
                None => return MarginalVerdict::Overflow,
            };
            acc_den = match acc_den.checked_mul(d) {
                Some(v) => v,
                None => return MarginalVerdict::Overflow,
            };
        }
    }
    if acc_den == 0 {
        return MarginalVerdict::Overflow;
    }
    if acc_num > acc_den {
        MarginalVerdict::AboveOne
    } else {
        MarginalVerdict::AtOrBelowOne
    }
}

/// Why the search stopped. Published with scope, best result and gap.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopReason {
    /// Every cycle in the authorized space was evaluated within budget.
    ScopeExhausted,
    /// The evaluation budget ran out with work still pending and vigente.
    EvalBudgetExhausted { pending: usize },
    /// Enumeration itself was truncated.
    EnumerationTruncated(TruncationReason),
    /// The authorized hop mask is empty: nothing is authorized to search.
    EmptyAuthorizedScope,
    /// No cycle exists under the policy.
    NoCandidateInScope,
}

impl StopReason {
    pub const fn as_str(self) -> &'static str {
        match self {
            StopReason::ScopeExhausted => "scope_exhausted",
            StopReason::EvalBudgetExhausted { .. } => "eval_budget_exhausted",
            StopReason::EnumerationTruncated(_) => "enumeration_truncated",
            StopReason::EmptyAuthorizedScope => "empty_authorized_scope",
            StopReason::NoCandidateInScope => "no_candidate_in_scope",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StopReport {
    pub reason: StopReason,
    pub budget_total: u64,
    pub budget_used: u64,
    pub enumeration_truncated: Option<TruncationReason>,
    /// Work still vigente for the next budget round.
    pub pending_work: usize,
}

/// What was actually evaluated. Published with every outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchScope {
    pub generation: GenerationId,
    pub snapshot: SnapshotId,
    pub authorized_hops: Vec<u8>,
    pub tokens: usize,
    pub pools_kept: usize,
    pub pools_rejected: usize,
    pub edges: usize,
    pub cycles_enumerated: usize,
    pub cycles_evaluated: usize,
    pub cycles_not_evaluated: usize,
    pub sizes_tried: usize,
    /// Families auto-added because `family_weights` did not cover them.
    pub auto_added_families: Vec<FamilyId>,
}

/// The single honest label this engine is allowed to attach to a result.
pub const BEST_FOUND_LABEL: &str = "best_found_in_evaluated_scope";

/// The output contract of one search turn (PROMPT §6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchOutcome {
    pub scope: SearchScope,
    /// Always [`BEST_FOUND_LABEL`]: never "global optimum".
    pub label: &'static str,
    /// The route the card was bound to, kept even when it loses.
    pub seed_diagnosis: Option<ScoredCandidate>,
    /// Best number actually computed in this generation (may be negative).
    pub best_computed: Option<ScoredCandidate>,
    /// Best candidate at or above the **execution minimum**.
    pub best_feasible: Option<ScoredCandidate>,
    /// Best candidate at or above the **search objective**.
    pub best_over_target: Option<ScoredCandidate>,
    /// Diverse alternatives (pool-disjoint greedy), best first.
    pub diverse_alternatives: Vec<ScoredCandidate>,
    /// `Some(true)` when every evaluated candidate is negative, `Some(false)`
    /// when at least one is non-negative, `None` when nothing was evaluated.
    pub every_candidate_negative: Option<bool>,
    /// `max(0, target − best_computed_net)`. `None` when no net was computed.
    pub gap_to_target: Option<Raw>,
    pub min_profit: HardMinimum,
    pub target: SearchTarget,
    pub stop: StopReport,
    pub prunes: PruneAccounting,
    pub marginal: MarginalAccounting,
    /// Candidates refused for belonging to an older generation.
    pub refused_stale: u64,
    pub invalidated: u64,
    /// Duplicate cycles (same route, different provenance) collapsed to one.
    pub duplicates_skipped: u64,
}

impl SearchOutcome {
    /// `true` when the search found nothing at or above the objective.
    ///
    /// This is a **normal** result, not an error: PROMPT §6 — *"No elimines un
    /// neto negativo porque aún no haya ganador."*
    pub fn is_no_opportunity(&self) -> bool {
        self.best_over_target.is_none()
    }

    /// `true` when a candidate is authorized for the execution minimum.
    pub fn has_executable_candidate(&self) -> bool {
        self.best_feasible.is_some()
    }

    pub fn best_computed_net(&self) -> Option<Raw> {
        self.best_computed.as_ref().and_then(|c| c.net_raw())
    }
}

/// Search tuning. Every field is explicit input — no market constant is baked
/// into the engine.
#[derive(Debug, Clone)]
pub struct EngineConfig {
    pub limits: EnumerateLimits,
    /// Maximum number of candidate evaluations per turn.
    pub eval_budget: u64,
    /// Multiscale size grid for the initial exploration.
    pub sizes: Vec<Raw>,
    /// Local bisection rounds around the best bracket. This is **refinement**,
    /// not an optimality proof: no unimodality is assumed.
    pub refine_rounds: u32,
    pub costs: ExternalCosts,
    /// Fair-share weights per family. Families absent here are not searched.
    pub family_weights: Vec<(FamilyId, u64)>,
    pub diverse_k: usize,
    pub min_profit: HardMinimum,
    pub target: SearchTarget,
    /// Exact marginal rates per edge, used **only** to tag candidates.
    pub marginal_rates: BTreeMap<EdgeId, MarginalRate>,
}

impl EngineConfig {
    /// Uniform weights across the hop families present in the authorized mask.
    pub fn uniform_family_weights(policy: &GraphPolicy) -> Vec<(FamilyId, u64)> {
        policy
            .hop_lengths
            .lengths()
            .into_iter()
            .map(|h| (family_of_hop_count(h as usize), 1u64))
            .collect()
    }
}

/// One shared, reused evaluation state.
#[derive(Debug, Clone)]
pub struct SearchEngine {
    graph: MultiGraph,
    policy: GraphPolicy,
    cfg: EngineConfig,
    /// Families that were missing from `family_weights` but are authorized by
    /// the hop mask. They were added with weight `1` instead of being silently
    /// dropped, and their ids are published in the scope.
    auto_added_families: Vec<FamilyId>,
}

impl SearchEngine {
    /// Build an engine, widening `family_weights` to cover every authorized hop
    /// family.
    ///
    /// A family with no configured weight would otherwise never be evaluated —
    /// work lost by configuration, which is exactly the kind of silent hole the
    /// prompt forbids. Missing families are added with weight `1` and reported
    /// through [`SearchScope::auto_added_families`].
    pub fn new(graph: MultiGraph, policy: GraphPolicy, mut cfg: EngineConfig) -> Self {
        let mut auto_added_families: Vec<FamilyId> = Vec::new();
        for hops in policy.hop_lengths.lengths() {
            let family = family_of_hop_count(hops as usize);
            if !cfg.family_weights.iter().any(|(f, w)| *f == family && *w > 0) {
                cfg.family_weights.push((family, 1));
                auto_added_families.push(family);
            }
        }
        auto_added_families.sort();
        SearchEngine {
            graph,
            policy,
            cfg,
            auto_added_families,
        }
    }

    pub fn auto_added_families(&self) -> &[FamilyId] {
        &self.auto_added_families
    }

    pub fn graph(&self) -> &MultiGraph {
        &self.graph
    }

    pub fn policy(&self) -> &GraphPolicy {
        &self.policy
    }

    pub fn config(&self) -> &EngineConfig {
        &self.cfg
    }

    /// Evaluate one cycle across the multiscale grid plus local refinement.
    ///
    /// Returns the best `CycleEval` found and the first named failure, so a
    /// partially-evaluable candidate is never reported as fully uncomputable.
    fn eval_sizes(&self, cycle: &Cycle) -> (Option<CycleEval>, Option<EvalError>, usize) {
        let mut best: Option<CycleEval> = None;
        let mut first_error: Option<EvalError> = None;
        let mut index_of_best: Option<usize> = None;

        for (i, size) in self.cfg.sizes.iter().enumerate() {
            match evaluate_cycle(&self.graph, cycle, *size, &self.cfg.costs) {
                Ok(evaluated) => {
                    let improves = match &best {
                        None => true,
                        Some(current) => evaluated.net_raw > current.net_raw,
                    };
                    if improves {
                        best = Some(evaluated);
                        index_of_best = Some(i);
                    }
                }
                Err(err) => {
                    if first_error.is_none() {
                        first_error = Some(err);
                    }
                }
            }
        }

        if let (Some(current), Some(center)) = (best.clone(), index_of_best) {
            if self.cfg.refine_rounds > 0 && self.cfg.sizes.len() >= 2 {
                let lo = if center > 0 {
                    self.cfg.sizes[center - 1]
                } else {
                    self.cfg.sizes[0]
                };
                let hi = if center + 1 < self.cfg.sizes.len() {
                    self.cfg.sizes[center + 1]
                } else {
                    self.cfg.sizes[self.cfg.sizes.len() - 1]
                };
                let mut best_refined = current;
                let (mut lo_v, mut hi_v) = (lo.get(), hi.get());
                for _ in 0..self.cfg.refine_rounds {
                    if hi_v - lo_v <= 1 {
                        break;
                    }
                    let mid = Raw::new(lo_v + (hi_v - lo_v) / 2);
                    match evaluate_cycle(&self.graph, cycle, mid, &self.cfg.costs) {
                        Ok(evaluated) => {
                            if evaluated.net_raw > best_refined.net_raw {
                                best_refined = evaluated;
                                lo_v = mid.get();
                            } else {
                                hi_v = mid.get();
                            }
                        }
                        Err(err) => {
                            if first_error.is_none() {
                                first_error = Some(err);
                            }
                            break;
                        }
                    }
                }
                best = Some(best_refined);
            }
        }

        let tried = self.cfg.sizes.len();
        let state_error = if self.cfg.sizes.is_empty() {
            Some(EvalError::NoSizeDomain)
        } else {
            None
        };
        (best, state_error.or(first_error), tried)
    }

    fn score_cycle(
        &self,
        cycle: &Cycle,
        generation: GenerationId,
        snapshot: SnapshotId,
        origin: CandidateOrigin,
    ) -> ScoredCandidate {
        let (best, error, _tried) = self.eval_sizes(cycle);
        let pools = cycle.pool_footprint(&self.graph);
        let pool_ids = cycle.pool_ids(&self.graph);
        let hops = cycle.hops();
        let state = match best {
            Some(evaluated) => CandidateState::Evaluated(evaluated),
            None => CandidateState::NotEvaluated(error.unwrap_or(EvalError::EmptyCycle)),
        };
        ScoredCandidate {
            cycle: cycle.clone(),
            state,
            generation,
            snapshot,
            family: family_of_hop_count(hops),
            origin,
            hops,
            pools,
            pool_ids,
        }
    }

    fn marginal_verdict_for(&self, cycle: &Cycle) -> MarginalVerdict {
        marginal_product_verdict(&self.graph, cycle, &self.cfg.marginal_rates)
    }

    /// Diverse alternatives: greedy maximisation of pool-footprint novelty.
    ///
    /// An alternative must be pool-disjoint from **every already-reported slot**
    /// (best computed / feasible / over-target) and from the alternatives chosen
    /// before it. Two "alternatives" that consume the same pool are not
    /// alternatives: acting on one consumes the liquidity of the other, which is
    /// the same trap as summing split legs independently.
    ///
    /// Deterministic: candidates are visited best-first with a stable tie-break.
    fn pick_diverse(
        &self,
        ranked: &[&ScoredCandidate],
        top_slots: &[&ScoredCandidate],
    ) -> Vec<ScoredCandidate> {
        let mut chosen: Vec<ScoredCandidate> = Vec::new();
        let mut used: BTreeSet<PoolIndex> = BTreeSet::new();
        let mut taken: Vec<&Vec<EdgeId>> = Vec::new();
        for slot in top_slots {
            used.extend(slot.pools.iter().copied());
            if !taken.iter().any(|e| **e == slot.cycle.edges) {
                taken.push(&slot.cycle.edges);
            }
        }
        for candidate in ranked {
            if chosen.len() >= self.cfg.diverse_k {
                break;
            }
            if taken.iter().any(|e| **e == candidate.cycle.edges) {
                continue;
            }
            if candidate.pools.iter().any(|p| used.contains(p)) {
                continue;
            }
            used.extend(candidate.pools.iter().copied());
            taken.push(&candidate.cycle.edges);
            chosen.push((*candidate).clone());
        }
        chosen
    }

    /// Run one search turn against `ledger`.
    ///
    /// The caller owns the ledger so it controls when a generation turns over
    /// (`open_generation`) and which events invalidate which pools.
    pub fn run(&self, ledger: &mut CandidateLedger, seed: Option<&Cycle>) -> SearchOutcome {
        let generation = ledger.generation();
        let snapshot = ledger.snapshot();

        let mut seed_diagnosis: Option<ScoredCandidate> = None;
        if let Some(seed_cycle) = seed {
            let scored = self.score_cycle(
                seed_cycle,
                generation,
                snapshot,
                CandidateOrigin::Seed,
            );
            ledger.admit(scored.clone());
            seed_diagnosis = Some(scored);
        }

        let (cycles, prunes) = enumerate_bounded(&self.graph, &self.policy, self.cfg.limits);

        let mut marginal = MarginalAccounting::default();
        let mut queue = FairShareQueue::new(&self.cfg.family_weights);
        let mut unconfigured_family: usize = 0;
        for cycle in cycles {
            let verdict = self.marginal_verdict_for(&cycle);
            let origin = match verdict {
                MarginalVerdict::AboveOne => {
                    marginal.above_one += 1;
                    CandidateOrigin::MarginalProduct
                }
                MarginalVerdict::AtOrBelowOne => {
                    marginal.at_or_below_one += 1;
                    CandidateOrigin::Enumerated
                }
                MarginalVerdict::MissingRate(_) => {
                    marginal.missing_rate += 1;
                    CandidateOrigin::Enumerated
                }
                MarginalVerdict::Overflow => {
                    marginal.overflow += 1;
                    CandidateOrigin::Enumerated
                }
                MarginalVerdict::DegenerateRate(_) => {
                    marginal.degenerate_rate += 1;
                    CandidateOrigin::Enumerated
                }
            };
            let hops = cycle.hops();
            let pushed = queue.push(WorkItem {
                cycle,
                family: family_of_hop_count(hops),
                origin,
            });
            if !pushed {
                unconfigured_family += 1;
            }
        }

        let served = queue.drain_with_budget(self.cfg.eval_budget);
        let budget_used = served.len() as u64;
        for item in served {
            let scored = self.score_cycle(&item.cycle, generation, snapshot, item.origin);
            ledger.admit(scored);
        }

        let pending = queue.pending_len() + unconfigured_family;
        let ranked = ledger.ranked_current();

        let best_computed = ranked.first().map(|c| (*c).clone());
        let min = self.cfg.min_profit.0;
        let target = self.cfg.target.0;

        let best_feasible = ranked
            .iter()
            .find(|c| c.net_raw().map(|n| n >= min).unwrap_or(false))
            .map(|c| (*c).clone());
        let best_over_target = ranked
            .iter()
            .find(|c| c.net_raw().map(|n| n >= target).unwrap_or(false))
            .map(|c| (*c).clone());

        let mut top_slots: Vec<&ScoredCandidate> = Vec::new();
        if let Some(b) = &best_computed {
            top_slots.push(b);
        }
        if let Some(b) = &best_feasible {
            top_slots.push(b);
        }
        if let Some(b) = &best_over_target {
            top_slots.push(b);
        }
        let diverse_alternatives = self.pick_diverse(&ranked, &top_slots);

        let evaluated_count = ledger.current().iter().filter(|c| c.is_evaluated()).count();
        let every_negative = if evaluated_count == 0 {
            None
        } else {
            Some(
                ledger
                    .current()
                    .iter()
                    .filter_map(|c| c.net_raw())
                    .all(|n| n.is_negative()),
            )
        };

        let best_net = best_computed.as_ref().and_then(|c| c.net_raw());
        let gap_to_target = best_net.map(|n| {
            let raw_gap = target.checked_sub(n).unwrap_or(target);
            raw_gap.max(Raw::ZERO)
        });

        let reason = if self.policy.hop_lengths.is_empty() {
            StopReason::EmptyAuthorizedScope
        } else if let Some(trunc) = prunes.truncated {
            StopReason::EnumerationTruncated(trunc)
        } else if pending > 0 {
            StopReason::EvalBudgetExhausted { pending }
        } else if ledger.current().is_empty() {
            StopReason::NoCandidateInScope
        } else {
            StopReason::ScopeExhausted
        };

        let scope = SearchScope {
            generation,
            snapshot,
            authorized_hops: self.policy.hop_lengths.lengths(),
            tokens: self.graph.tokens().len(),
            pools_kept: self.graph.kept_pool_indices().len(),
            pools_rejected: self.graph.rejected_pool_indices().len(),
            edges: self.graph.edges().len(),
            cycles_enumerated: prunes.cycles_emitted,
            cycles_evaluated: evaluated_count,
            cycles_not_evaluated: ledger
                .current()
                .iter()
                .filter(|c| !c.is_evaluated())
                .count(),
            sizes_tried: self.cfg.sizes.len(),
            auto_added_families: self.auto_added_families.clone(),
        };

        SearchOutcome {
            scope,
            label: BEST_FOUND_LABEL,
            seed_diagnosis,
            best_computed,
            best_feasible,
            best_over_target,
            diverse_alternatives,
            every_candidate_negative: every_negative,
            gap_to_target,
            min_profit: self.cfg.min_profit,
            target: self.cfg.target,
            stop: StopReport {
                reason,
                budget_total: self.cfg.eval_budget,
                budget_used,
                enumeration_truncated: prunes.truncated,
                pending_work: pending,
            },
            prunes,
            marginal,
            refused_stale: ledger.refused_stale_count(),
            invalidated: ledger.invalidated_count(),
            duplicates_skipped: ledger.duplicates_skipped(),
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::graph::{PoolSpec, ProtocolVersion};
    use crate::units::Bps;

    fn bps(v: u32) -> Bps {
        Bps::new(v).unwrap()
    }

    fn pool(id: u64, t0: u64, t1: u64, r0: i128, r1: i128, fee: u32) -> PoolSpec {
        PoolSpec {
            pool_id: id,
            version: ProtocolVersion::V2,
            token0: t0,
            token1: t1,
            fee_bps: bps(fee),
            reserve0: Raw::new(r0),
            reserve1: Raw::new(r1),
            available: true,
        }
    }

    fn item(hops: usize, family: FamilyId) -> WorkItem {
        WorkItem {
            cycle: Cycle {
                edges: (0..hops).collect(),
            },
            family,
            origin: CandidateOrigin::Enumerated,
        }
    }

    #[test]
    fn fair_share_serves_a_small_family_in_the_first_round() {
        let big = FamilyId::new(2);
        let small = FamilyId::new(7);
        let mut queue = FairShareQueue::new(&[(big, 8), (small, 2)]);
        for _ in 0..1_000 {
            assert!(queue.push(item(2, big)));
        }
        assert!(queue.push(item(7, small)));
        let served = queue.drain_with_budget(12);
        assert_eq!(served.len(), 12);
        assert!(
            served.iter().any(|w| w.family == small),
            "the one-item family must be served inside a 12-slot budget"
        );
        // unserved work stays vigente
        assert_eq!(queue.pending_len(), 1_000 + 1 - 12);
        // family `big` served 8 (round 1) + 3 (round 2) = 11
        assert_eq!(queue.pending_of(big), 1_000 - 11);
        assert_eq!(queue.pending_of(small), 0);
    }

    #[test]
    fn unconfigured_family_is_refused_not_silently_dropped() {
        let mut queue = FairShareQueue::new(&[(FamilyId::new(2), 1)]);
        assert!(!queue.push(item(5, FamilyId::new(5))));
        assert_eq!(queue.pending_len(), 0);
    }

    #[test]
    fn ledger_separates_current_best_from_historical_record() {
        let mut ledger = CandidateLedger::new(SnapshotId(1));
        let g = MultiGraph::build(
            vec![pool(1, 1, 2, 1_000_000, 1_000_000, 0)],
            &GraphPolicy::permissive_2_to_7(),
        );
        let cycle = Cycle { edges: vec![0, 1] };
        let winner = ScoredCandidate {
            cycle: cycle.clone(),
            state: CandidateState::Evaluated(CycleEval {
                amount_in: Raw::new(100),
                amount_out: Raw::new(200),
                gross_raw: Raw::new(100),
                financing_raw: Raw::ZERO,
                unconditional_raw: Raw::ZERO,
                costs_raw: Raw::ZERO,
                net_raw: Raw::new(100),
                hops: 2,
                fees_embedded_in_quote: true,
            }),
            generation: ledger.generation(),
            snapshot: SnapshotId(1),
            family: FamilyId::new(2),
            origin: CandidateOrigin::Enumerated,
            hops: 2,
            pools: cycle.pool_footprint(&g),
            pool_ids: cycle.pool_ids(&g),
        };
        assert!(ledger.admit(winner));
        assert_eq!(ledger.current_best().unwrap().net_raw(), Some(Raw::new(100)));
        assert_eq!(ledger.historical_max().unwrap().net_raw(), Some(Raw::new(100)));

        // New generation: current is invalidated, the record survives.
        ledger.open_generation(SnapshotId(2));
        assert!(ledger.current().is_empty());
        assert_eq!(ledger.invalidated_count(), 1);
        assert_eq!(ledger.current_best(), None);
        assert_eq!(
            ledger.historical_max().unwrap().net_raw(),
            Some(Raw::new(100)),
            "the historical record is not current evidence"
        );
    }

    #[test]
    fn stale_candidate_is_refused_by_admission() {
        let mut ledger = CandidateLedger::new(SnapshotId(1));
        let old_generation = ledger.generation();
        ledger.open_generation(SnapshotId(2));
        let stale = ScoredCandidate {
            cycle: Cycle { edges: vec![0, 1] },
            state: CandidateState::NotEvaluated(EvalError::EmptyCycle),
            generation: old_generation,
            snapshot: SnapshotId(1),
            family: FamilyId::new(2),
            origin: CandidateOrigin::Enumerated,
            hops: 2,
            pools: BTreeSet::new(),
            pool_ids: Vec::new(),
        };
        assert!(!ledger.admit(stale));
        assert_eq!(ledger.refused_stale_count(), 1);
    }

    #[test]
    fn invalidate_pool_drops_only_touching_candidates() {
        let g = MultiGraph::build(
            vec![
                pool(1, 1, 2, 1_000_000, 1_000_000, 0),
                pool(2, 3, 4, 1_000_000, 1_000_000, 0),
            ],
            &GraphPolicy::permissive_2_to_7(),
        );
        let mut ledger = CandidateLedger::new(SnapshotId(1));
        let generation = ledger.generation();
        let make = |edges: Vec<EdgeId>| {
            let cycle = Cycle { edges };
            let hops = cycle.hops();
            let pools = cycle.pool_footprint(&g);
            let pool_ids = cycle.pool_ids(&g);
            ScoredCandidate {
                cycle,
                state: CandidateState::NotEvaluated(EvalError::EmptyCycle),
                generation,
                snapshot: SnapshotId(1),
                family: FamilyId::new(2),
                origin: CandidateOrigin::Enumerated,
                hops,
                pools,
                pool_ids,
            }
        };
        let a = make(vec![0, 1]);
        let b = make(vec![2, 3]);
        assert!(ledger.admit(a));
        assert!(ledger.admit(b));
        assert_eq!(ledger.current().len(), 2);
        assert_eq!(ledger.invalidate_pool(1), 1);
        assert_eq!(ledger.current().len(), 1);
    }

    #[test]
    fn marginal_verdict_is_exact_and_missing_rate_is_a_third_state() {
        // Fee-free cycle with marginal rates of exactly 1 → not above one.
        let g = MultiGraph::build(
            vec![
                pool(1, 1, 2, 1_000_000, 1_000_000, 0),
                pool(2, 1, 2, 1_000_000, 1_000_000, 0),
            ],
            &GraphPolicy::permissive_2_to_7(),
        );
        let cycle = Cycle { edges: vec![0, 3] };
        let mut rates = BTreeMap::new();
        rates.insert(0usize, MarginalRate { num: 1, den: 1 });
        rates.insert(3usize, MarginalRate { num: 1, den: 1 });
        assert_eq!(
            marginal_product_verdict(&g, &cycle, &rates),
            MarginalVerdict::AtOrBelowOne
        );

        // 1.2 × 1.0 > 1 → accepted, though finite size may still lose.
        rates.insert(3usize, MarginalRate { num: 12, den: 10 });
        assert_eq!(
            marginal_product_verdict(&g, &cycle, &rates),
            MarginalVerdict::AboveOne
        );

        rates.remove(&3usize);
        assert_eq!(
            marginal_product_verdict(&g, &cycle, &rates),
            MarginalVerdict::MissingRate(3)
        );
    }
}
