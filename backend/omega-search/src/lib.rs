//! # omega-search — autonomous bounded search engine
//!
//! Implementation of the *free* sectors of `PROMPT_ARBITRAGEX_BUSQUEDA_AUTONOMA_FEES_264.md`
//! §6 ("Motor de búsqueda y optimización real") and §7 ("Búsqueda autónoma continua,
//! sin bucles ciegos"), plus their §13/§14 obligations (performance budgets,
//! exhaustive-oracle tests).
//!
//! ## What this crate is
//!
//! A deterministic, allocation-light search kernel over a **directed multigraph**
//! whose nodes are tokens and whose edges are `(pool, direction, version)`. It
//! enumerates closed cycles under an explicit policy (allowlists, liquidity
//! floors, venue availability, hop masks, authorized hop range), evaluates them
//! with **exact integer arithmetic at finite size**, and publishes an honest
//! outcome contract: best computed diagnosis, best feasible candidate, best
//! candidate over target, diverse alternatives, target gap, evaluated scope and
//! stop reason.
//!
//! ## Non-negotiable rules encoded here
//!
//! * **No `f64` for money.** Every token amount is [`units::Raw`] — an integer
//!   count of minimal units — and every conversion is an explicit checked
//!   rational operation that returns `None` on overflow instead of wrapping.
//! * **Marginal `-log(rate)` weights only *generate* candidates.** They never
//!   prove benefit at finite size. [`engine::marginal_product_generates`] is
//!   exposed as a *generator*; the verdict always comes from
//!   [`eval::evaluate_cycle`]/[`eval::PoolLedger`] at an explicit size.
//! * **No Dijkstra, no Johnson reweighting.** Those require preconditions that
//!   do not hold for directed graphs with fixed costs and impact; the engine
//!   uses bounded DFS + exact evaluation instead.
//! * **Absence, real zero and loss are three distinct states.** Rejected edges
//!   carry a machine-readable reason (`zero_reserve` ≠ `reserve_below_floor` ≠
//!   `venue_unavailable`); a computed `0` is `Raw::ZERO`, never "missing".
//! * **A stale positive result does not stay current because it was positive.**
//!   [`engine::CandidateLedger`] binds every candidate to the generation and
//!   snapshot that produced it and keeps the historical max in a *separate*
//!   slot.
//! * **The label is `best_found_in_evaluated_scope`.** Never "global optimum".
//!
//! ## Integration (PENDING — reported, not silently done)
//!
//! This crate is standalone so it cannot collide with the 16 live `fix/*`
//! branches. Wiring it into the searcher requires exactly two edits, both
//! deliberately left undone because one of them is in a prohibited hot file:
//!
//! 1. `backend/searcher-rs/Cargo.toml` — add `omega-search = { path = "../omega-search" }`.
//! 2. `backend/searcher-rs/src/lib.rs` — add `pub mod omega_search;`
//!    **PROHIBITED ZONE**: `lib.rs` is one of the 11 files the live branches are
//!    editing, so this edit is intentionally NOT made here.
//!
//! The adapter that maps `route_discovery::types::RouteEdge` /
//! `RouteCandidate` onto [`graph::PoolSpec`] / [`cycles::Cycle`] and back is the
//! remaining bridge work; see the task report for the exact call sites.

pub mod cycles;
pub mod engine;
pub mod eval;
pub mod graph;
pub mod units;

pub use cycles::{
    canonicalize, enumerate_bounded, enumerate_exhaustive, prune_rules, Cycle, EnumerateLimits,
    PruneAccounting, TruncationReason,
};
pub use engine::{
    family_of_hop_count, marginal_product_verdict, CandidateLedger, CandidateOrigin, CandidateState,
    EngineConfig, FairShareQueue, FamilyId, GenerationId, HardMinimum, MarginalAccounting,
    MarginalRate, MarginalVerdict, ScoredCandidate, SearchEngine, SearchOutcome, SearchScope,
    SearchTarget, SnapshotId, StopReason, StopReport, WorkItem, BEST_FOUND_LABEL,
};
pub use eval::{evaluate_cycle, evaluate_split_joint, CycleEval, EvalError, ExternalCosts, PoolLedger};
pub use graph::{
    Edge, EdgeId, GraphPolicy, HopLengthMask, MultiGraph, PoolIndex, PoolReject, PoolSpec,
    ProtocolVersion, RejectedPool, TokenId,
};
pub use units::{Bps, Raw};
