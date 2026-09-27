//! HOPS-EMIT-01 — discovery → emission bridge for **3..=7-hop closed cycles**.
//!
//! ## The gap this closes
//!
//! Until now the ONLY opportunity emitter was [`crate::engines::dex_engine`],
//! which enumerates **pairs of pools** — 2 hops by construction. The discovery
//! side (`unique_route_finder`, `multi_hop_search`, `route_scanner_worker`)
//! already produces closed cycles of 3..=7 hops and carries everything needed to
//! build a route plan (per-hop pool, token path, protocol family, fee tier), but
//! those cycles were only ever fanned out to the CARTRIDGE path
//! (`Orchestrator::spawn_cartridge_eval`) or to the native engines, which
//! re-derive 2-hop pairs from the impacted pools. Net effect: every persisted
//! card was 2-hop.
//!
//! This module turns an already-decoded **closed cycle intent** into a
//! `StrategyCandidate` whose `RoutePlan` has `hops` legs, so the EXISTING
//! pipeline sizes it (the N-leg cycle kernel in `size_optimizer`, which already
//! accepts 2..=7 legs) and emits it through the EXISTING tail
//! (`Orchestrator::process_candidate` → `RouteMetadata` + `attach_leg_ledger` →
//! `OpportunityEmitter`). No new wire field, no new persistence path.
//!
//! ## Hard invariants
//!
//! - **RULE 00 / R8 — never fabricate.** This module computes NO reserves, NO
//!   amounts, NO profit and NO fees. `gross_profit_usd` /
//!   `expected_profit_usd` start as `None`; the sizing kernel owns every number.
//!   A leg whose `pool_hint` is absent is carried as `pool_address: None` so the
//!   kernel rejects it with its own explicit `MissingPoolAddress` — the
//!   rejection is EMITTED as a row (established contract), never patched over.
//!   A pool missing from the reserves cache is rejected by the kernel with
//!   `MissingReservesPoolA/B` for the same reason.
//! - **Bounded.** [`admit`] claims one slot from a per-(chain, block) budget
//!   ([`MultihopEmitBudget`], cap `ARBX_MULTIHOP_EMIT_MAX_PER_TICK`, default
//!   [`DEFAULT_MAX_PER_TICK`]). No loop over an unbounded set, no per-candidate
//!   RPC. Exhaustion is honest and observable ([`BridgeSkip::CapReached`] +
//!   `dropped_in`), never a silent drop.
//! - **No double emission.** `hops < 3` is refused ([`BridgeSkip::TooFewHops`]);
//!   every candidate built here has `route_plan.legs.len() >= 3` while
//!   `dex_engine` only ever builds 2-leg plans — the two sets are disjoint by
//!   construction, so a 2-hop route stays exclusively `dex_engine`'s.
//! - **Mode-invariant (§34.1).** No mode is read here. The same gates, the same
//!   kernel and the same emitter run in PAPER_SHADOW / TESTNET / LIVE_MAINNET.
//! - **Reversible.** `ARBX_MULTIHOP_EMIT=off` (or `false`/`0`) disables the
//!   whole bridge at admission time; when off, nothing is built, nothing is
//!   sized, nothing is emitted and no existing behaviour changes.
//!
//! ## PERHOP-RESERVES-01 — budget ORDER vs. budget SIZE
//!
//! The per-(chain, block) cap above is a hard bound, and in production it is
//! saturated by ~100× (`v2.hop_cycle_bridge.cap_reached`: 31 556 refusals in
//! 20 min against a cap of 12/block). Until this patch the admitted set was
//! therefore an arbitrary **first-K slice of the finder's order** — and the
//! finder's order is a best-K selection ranked by gross cycle score, which in
//! production is 100 % V3-bearing (measured: 585 of 585 discovered 3..=7-hop
//! cycles over the `arbx:route_discovery:telemetry` channel carried at least
//! one V3 leg; zero all-V2). Every one of those cycles is unpriceable by the
//! N-leg kernel ([`KernelCapability::V3MultilegUnsupported`]) — so the scarce
//! budget was spent, in full and every block, on cycles that provably cannot
//! reach the ledger.
//!
//! [`order_for_budget`] fixes the ORDER (sizeable cycles are submitted to the
//! budget first, stable within each lane) and [`MultihopEmitBudget`] now
//! COUNTS the refusals per lane ([`MultihopEmitBudget::unpriceable_refused_in`])
//! so the deferral is explicit and observable (R8 — never silent). No row is
//! suppressed: an unpriceable cycle still occupies a slot whenever the sizeable
//! lane did not need it, so the diagnostic rows the operator already sees keep
//! flowing with their own explicit kernel reason.
//!
//! ## Which strategy label, and why
//!
//! `StrategyLabel` has no `MultiHop` variant, and this module does NOT invent
//! one. A discovered closed cycle of N hops IS the cycle family the repo already
//! labels [`StrategyLabel::TriangularArb`] — the variant documented as
//! "cycle on the same or mixed DEX family (A→B→C→A)" that maps to the persisted
//! `triangular` kind and dispatches to the N-leg cycle kernel
//! (`size_optimizer::optimize_with_reason` sends it to
//! `size_triangular_with_reason`, which validates 2..=7 legs). Reusing it keeps
//! `route_plan.strategy_kind == label.as_str()` (`triangular_arb`) and keeps the
//! row inside the operator's existing strategy configuration
//! (`enabled_strategies` / per-strategy `route_constraints`), instead of
//! inventing a strategy_kind no gate knows.

use crate::engines::dex_engine::protocol_type_to_str;
use crate::engines::StrategyCandidate;
use crate::route_discovery::types::RouteCandidate;
use crate::route_intent::{DetectionSource, ProtocolType, RouteIntent, RouteIntentLeg};
use crate::strategy_label::StrategyLabel;
use once_cell::sync::Lazy;
use prioritization_spine::route_plan::{RouteLeg, RoutePlan};
use prioritization_spine::types::OpportunityCandidate;
use shared_rs::contracts::Opportunity;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use tracing::debug;
use uuid::Uuid;

/// Lowest hop count this bridge owns. 2-hop routes belong to `dex_engine`
/// exclusively (constraint: no duplicate rows).
pub const MIN_BRIDGE_HOPS: usize = 3;

/// Highest hop count — mirrors the canonical workbook `Max_Hops` and the finder
/// clamp (`2..=7`) the discovery workers already enforce.
pub const MAX_BRIDGE_HOPS: usize = 7;

/// Reversibility knob. Absent/foreign ⇒ enabled (the operator wants 3–7-hop
/// cards); `off`/`false`/`0` ⇒ disabled (exact pre-patch behaviour).
pub const BRIDGE_ENABLED_ENV: &str = "ARBX_MULTIHOP_EMIT";

/// Per-(chain, block) emission cap knob.
pub const BRIDGE_CAP_ENV: &str = "ARBX_MULTIHOP_EMIT_MAX_PER_TICK";

/// PERHOP-RESERVES-01 ordering knob. Absent/foreign ⇒ enabled, i.e. the
/// per-block budget is offered to the cycles the sizing kernel can actually
/// price BEFORE the ones it can only refuse; `off`/`false`/`0` ⇒ the pre-patch
/// single pass in finder order (byte-identical admission sequence).
pub const PREFER_SIZEABLE_ENV: &str = "ARBX_MULTIHOP_PREFER_SIZEABLE";

/// Default cap: conservative, same order as the sibling volume guards
/// (`route_scanner_worker::DEFAULT_CANONICAL_PER_BLOCK` = 25 per block) but
/// tighter because each admitted cycle costs one sizing pass plus one emitter
/// publish (PG + Redis).
pub const DEFAULT_MAX_PER_TICK: usize = 12;

/// Detector identity persisted on every row this bridge produces. Same
/// convention as the other producers (`dex_engine`, `triangular_engine`, …) —
/// the column exists so the operator can attribute a row to its producer.
pub const DETECTOR_ID: &str = "hop_cycle_bridge";

// ---------------------------------------------------------------------------
// Reversibility knob
// ---------------------------------------------------------------------------

/// Pure parser for the toggle (testable without env mutation).
///
/// `None` (absent) ⇒ `true`: the bridge is ON by default. Only the three
/// explicit OFF spellings disable it; any other value is foreign and keeps the
/// default (the mirror of `runtime_knobs::resolve_toggle`'s "a foreign value is
/// never interpreted" rule).
pub fn bridge_enabled_from_raw(raw: Option<&str>) -> bool {
    let normalized = raw.map(|v| v.trim().to_ascii_lowercase());
    !matches!(
        normalized.as_deref(),
        Some("off") | Some("false") | Some("0")
    )
}

/// Boot-time verdict of [`BRIDGE_ENABLED_ENV`], memoised so the hot path pays no
/// environment lookup per intent (the knob is a deployment switch — changing it
/// requires a restart, exactly like every other `ARBX_*` boot gate).
static ENABLED: Lazy<bool> =
    Lazy::new(|| bridge_enabled_from_raw(std::env::var(BRIDGE_ENABLED_ENV).ok().as_deref()));

/// Live verdict of [`BRIDGE_ENABLED_ENV`] (resolved once at first use).
pub fn bridge_enabled() -> bool {
    *ENABLED
}

/// Pure parser for [`PREFER_SIZEABLE_ENV`] (testable without env mutation).
///
/// `None` (absent) ⇒ `true`: sizeable cycles are offered the budget first. Only
/// the three explicit OFF spellings restore the pre-patch finder-order pass;
/// any other value is foreign and keeps the default — the same rule
/// [`bridge_enabled_from_raw`] applies.
pub fn prefer_sizeable_from_raw(raw: Option<&str>) -> bool {
    let normalized = raw.map(|v| v.trim().to_ascii_lowercase());
    !matches!(
        normalized.as_deref(),
        Some("off") | Some("false") | Some("0")
    )
}

/// Boot-time verdict of [`PREFER_SIZEABLE_ENV`], memoised (per-item env lookups
/// would put a `std::env` call on the tick path).
static PREFER_SIZEABLE: Lazy<bool> =
    Lazy::new(|| prefer_sizeable_from_raw(std::env::var(PREFER_SIZEABLE_ENV).ok().as_deref()));

/// Live verdict of [`PREFER_SIZEABLE_ENV`] (resolved once at first use).
pub fn prefer_sizeable() -> bool {
    *PREFER_SIZEABLE
}

fn cap_from_env() -> usize {
    std::env::var(BRIDGE_CAP_ENV)
        .ok()
        .and_then(|v| v.trim().parse::<usize>().ok())
        .unwrap_or(DEFAULT_MAX_PER_TICK)
}

// ---------------------------------------------------------------------------
// PERHOP-RESERVES-01 — kernel capability (which lane a cycle belongs to)
// ---------------------------------------------------------------------------

/// Which slot of the per-(chain, block) budget a discovered cycle competes for.
///
/// Derived locally from the intent's own legs (no I/O, no RPC, no reserves), so
/// classifying a cycle never costs a Redis or chain round-trip in the hot path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmitLane {
    /// Every leg is a constant-product pool: the N-leg cycle kernel *can* price
    /// this cycle from the in-memory reserves cache (and rejects honestly with
    /// `missing_reserves_pool_*` when a pool is genuinely uncached).
    Sizeable,
    /// At least one leg is concentrated-liquidity: no N-leg V3 kernel exists, so
    /// the cycle can only ever be refused. It must not outrank a sizeable cycle
    /// for the scarce budget.
    Unpriceable,
}

/// The verdict [`crate::size_optimizer::SizeOptimizer`] will reach for this
/// cycle's geometry, decided from the intent alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KernelCapability {
    /// All-V2 (constant-product) legs — `size_triangular_with_reason` owns it.
    V2CycleSizeable,
    /// Contains a V3 (concentrated-liquidity) leg — the optimizer refuses with
    /// its own `v3_multileg_unsupported` reason. A V3 pool has no
    /// `getReserves()`, hence no `arbx:pool_reserves:<chain>:<pool>` key, so the
    /// V2 cache-miss reason would name the wrong cause.
    V3MultilegUnsupported,
}

impl KernelCapability {
    /// The budget lane this capability competes for.
    pub fn lane(self) -> EmitLane {
        match self {
            Self::V2CycleSizeable => EmitLane::Sizeable,
            Self::V3MultilegUnsupported => EmitLane::Unpriceable,
        }
    }

    /// Stable lowercase token for logs/telemetry.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::V2CycleSizeable => "v2_cycle_sizeable",
            Self::V3MultilegUnsupported => "v3_multileg_unsupported",
        }
    }
}

/// The single source of truth for "is this leg's protocol a V3 family?".
///
/// Mirrors `size_optimizer::leg_is_v3` exactly (it tests the leg's
/// `protocol_type` string with a case-insensitive `contains("v3")`), routed
/// through the ONE protocol-name table
/// [`protocol_type_to_str`] so the bridge and the kernel can never disagree.
pub fn protocol_is_v3(pt: ProtocolType) -> bool {
    protocol_type_to_str(pt).to_ascii_lowercase().contains("v3")
}

/// Classify a cycle from its per-leg protocol families.
pub fn kernel_capability_of_protocols<'a>(
    protocols: impl IntoIterator<Item = &'a ProtocolType>,
) -> KernelCapability {
    if protocols.into_iter().any(|p| protocol_is_v3(*p)) {
        KernelCapability::V3MultilegUnsupported
    } else {
        KernelCapability::V2CycleSizeable
    }
}

/// Classify an admitted intent (its legs are what `bridge_leg` renders onto the
/// `RoutePlan`, so this is the same predicate the kernel will evaluate).
pub fn kernel_capability(intent: &RouteIntent) -> KernelCapability {
    kernel_capability_of_protocols(intent.legs.iter().map(|l| &l.protocol_type))
}

/// Order one tick's discovered cycles so the per-(chain, block) budget is
/// offered to the cycles the kernel can actually price FIRST.
///
/// Stable within each lane: the finder's own order is preserved inside
/// [`EmitLane::Sizeable`] and inside [`EmitLane::Unpriceable`], so the change is
/// a pure lane partition — it never re-ranks within a lane and never drops an
/// item. `prefer_sizeable == false` returns the input order verbatim (the
/// pre-patch admission sequence).
///
/// Bounded by construction: one pass, one `Vec` of `len` references (the tick's
/// route set is already capped by `max_routes_per_tick`).
pub fn order_for_budget(routes: &[RouteCandidate], prefer_sizeable: bool) -> Vec<&RouteCandidate> {
    if !prefer_sizeable {
        return routes.iter().collect();
    }
    let mut ordered: Vec<&RouteCandidate> = Vec::with_capacity(routes.len());
    ordered.extend(routes.iter().filter(|c| {
        kernel_capability_of_protocols(c.protocols.iter()) == KernelCapability::V2CycleSizeable
    }));
    ordered.extend(routes.iter().filter(|c| {
        kernel_capability_of_protocols(c.protocols.iter())
            == KernelCapability::V3MultilegUnsupported
    }));
    ordered
}

// ---------------------------------------------------------------------------
// Budget
// ---------------------------------------------------------------------------

/// Bounded per-(chain, block) allowance for admitted cycles.
///
/// Same shape as [`crate::route_discovery::triangular_adapter::BackfillBudget`]
/// (lock-free CAS claims, reset when a new epoch is observed), which is the
/// proven in-repo pattern for a per-block allowance: concurrent ticks can never
/// both spend the same slot and can never overspend the cap.
///
/// `dropped` makes truncation OBSERVABLE (R8 — the cap must never be a silent
/// drop): the caller increments it once per refused admission and the count is
/// reported in the bridge's structured log line for the epoch.
///
/// `epoch == 0` (block height unavailable — the tick's block fetch failed) never
/// resets the allowance: after `per_epoch` admissions the bridge stays muted
/// until a real block height is seen again. Deliberate fail-honest degradation,
/// identical to `BackfillBudget`'s documented `current_block == 0` behaviour —
/// a broken RPC path must not license unbounded emission.
#[derive(Debug)]
pub struct MultihopEmitBudget {
    per_epoch: usize,
    epoch: AtomicU64,
    used: AtomicU64,
    dropped: AtomicU64,
    /// PERHOP-RESERVES-01 lane accounting: slots spent by cycles the kernel can
    /// price, slots spent by cycles it can only refuse, and — the figure R8
    /// demands be visible — how many unpriceable cycles were refused because the
    /// budget was already spent (i.e. deferred behind the sizeable lane).
    sizeable_used: AtomicU64,
    unpriceable_used: AtomicU64,
    unpriceable_refused: AtomicU64,
    /// LOGFLOOD-02 (R9): epoch whose single aggregate cap-hit line has already
    /// been emitted. `u64::MAX` = none yet.
    cap_reported_epoch: AtomicU64,
}

impl MultihopEmitBudget {
    /// `per_epoch == 0` is honoured as "emit nothing" (an explicit operator
    /// decision to mute the bridge without disabling it structurally).
    pub fn new(per_epoch: usize) -> Self {
        Self {
            per_epoch,
            // Sentinel epoch that no real block number can equal, so the first
            // claim always performs the reset.
            epoch: AtomicU64::new(u64::MAX),
            used: AtomicU64::new(0),
            dropped: AtomicU64::new(0),
            sizeable_used: AtomicU64::new(0),
            unpriceable_used: AtomicU64::new(0),
            unpriceable_refused: AtomicU64::new(0),
            cap_reported_epoch: AtomicU64::new(u64::MAX),
        }
    }

    /// LOGFLOOD-02 — `true` exactly once per epoch: the caller may emit that
    /// epoch's single aggregate cap-hit line. Every other refusal of the same
    /// epoch stays a per-item `debug!`.
    ///
    /// This is what the callers always MEANT ("one warn ... not a per-item noise
    /// line (R9)") but not what they did: at the measured refusal rate the
    /// per-cycle `warn!` was ~20 % of the container's log volume and collapsed
    /// the 50 MB window to under 10 minutes (2026-09-27: 23 920 `cap_reached`
    /// lines in 9m47s of retained logs), destroying the forensic window R9
    /// exists to protect. The truncation is still never hidden: the aggregate
    /// line carries the epoch's totals, the per-item line survives at `debug`,
    /// and `dropped_in` remains the counted truth (R8).
    ///
    /// Out-of-order epochs cannot re-report: the claim is strictly monotonic in
    /// the epoch id, so a late-arriving older block never earns a second line.
    pub fn claim_cap_report(&self, epoch: u64) -> bool {
        let mut seen = self.cap_reported_epoch.load(Ordering::Acquire);
        loop {
            if seen != u64::MAX && epoch <= seen {
                return false;
            }
            match self.cap_reported_epoch.compare_exchange_weak(
                seen,
                epoch,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => return true,
                Err(actual) => seen = actual,
            }
        }
    }

    pub fn per_epoch(&self) -> usize {
        self.per_epoch
    }

    /// Claim one slot for `epoch`. `true` = admitted (caller may build + emit).
    pub fn claim(&self, epoch: u64) -> bool {
        self.claim_lane(epoch, EmitLane::Sizeable)
    }

    /// PERHOP-RESERVES-01 — claim one slot for `epoch` **in `lane`**.
    ///
    /// The hard cap (`per_epoch`) is unchanged and shared: the lane only decides
    /// which counter observes the spend. Ordering is the caller's job
    /// ([`order_for_budget`]); the budget's job is to make the outcome countable.
    pub fn claim_lane(&self, epoch: u64, lane: EmitLane) -> bool {
        loop {
            let observed = self.epoch.load(Ordering::Acquire);
            if observed != epoch {
                // New epoch ⇒ reset the allowance, then fall through to claim.
                // A concurrent winner of this CAS performs the same reset.
                if self
                    .epoch
                    .compare_exchange(observed, epoch, Ordering::AcqRel, Ordering::Acquire)
                    .is_ok()
                {
                    self.used.store(0, Ordering::Release);
                    self.dropped.store(0, Ordering::Release);
                    self.sizeable_used.store(0, Ordering::Release);
                    self.unpriceable_used.store(0, Ordering::Release);
                    self.unpriceable_refused.store(0, Ordering::Release);
                }
                continue;
            }
            let used = self.used.load(Ordering::Acquire);
            if used >= self.per_epoch as u64 {
                return false;
            }
            if self
                .used
                .compare_exchange(used, used + 1, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                match lane {
                    EmitLane::Sizeable => self.sizeable_used.fetch_add(1, Ordering::Relaxed),
                    EmitLane::Unpriceable => self.unpriceable_used.fetch_add(1, Ordering::Relaxed),
                };
                return true;
            }
            // Lost the race against a concurrent claim → retry.
        }
    }

    /// Record one refused admission (cap reached) for `epoch` — the observable
    /// half of the truncation contract.
    pub fn note_dropped(&self, epoch: u64) {
        self.note_refused(epoch, EmitLane::Sizeable)
    }

    /// PERHOP-RESERVES-01 — record one refused admission for `epoch` in `lane`.
    ///
    /// An [`EmitLane::Unpriceable`] refusal additionally bumps
    /// [`Self::unpriceable_refused_in`]: with [`order_for_budget`] having
    /// already offered the budget to every sizeable cycle this epoch, such a
    /// refusal means the cap was spent on priceable work — the deferred count
    /// the operator needs in order to see *why* unpriceable cycles stop
    /// appearing. Never silent (R8).
    pub fn note_refused(&self, epoch: u64, lane: EmitLane) {
        // Only count drops belonging to the CURRENT epoch: a late drop from a
        // previous block must not inflate this block's truncation figure.
        if self.epoch.load(Ordering::Acquire) == epoch {
            self.dropped.fetch_add(1, Ordering::Relaxed);
            if lane == EmitLane::Unpriceable {
                self.unpriceable_refused.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    /// Slots spent in `epoch` (0 for any other epoch — the counter is reset on
    /// epoch change).
    pub fn used_in(&self, epoch: u64) -> u64 {
        if self.epoch.load(Ordering::Acquire) == epoch {
            self.used.load(Ordering::Acquire)
        } else {
            0
        }
    }

    /// Admissions refused in `epoch` (truncation, observable).
    pub fn dropped_in(&self, epoch: u64) -> u64 {
        if self.epoch.load(Ordering::Acquire) == epoch {
            self.dropped.load(Ordering::Acquire)
        } else {
            0
        }
    }

    /// Slots in `epoch` spent on cycles the sizing kernel can price.
    pub fn sizeable_used_in(&self, epoch: u64) -> u64 {
        if self.epoch.load(Ordering::Acquire) == epoch {
            self.sizeable_used.load(Ordering::Acquire)
        } else {
            0
        }
    }

    /// Slots in `epoch` spent on cycles the sizing kernel can only refuse.
    pub fn unpriceable_used_in(&self, epoch: u64) -> u64 {
        if self.epoch.load(Ordering::Acquire) == epoch {
            self.unpriceable_used.load(Ordering::Acquire)
        } else {
            0
        }
    }

    /// PERHOP-RESERVES-01 — unpriceable cycles refused in `epoch` because the
    /// budget had already been offered to (and spent by) priceable work.
    pub fn unpriceable_refused_in(&self, epoch: u64) -> u64 {
        if self.epoch.load(Ordering::Acquire) == epoch {
            self.unpriceable_refused.load(Ordering::Acquire)
        } else {
            0
        }
    }
}

/// Per-chain budget registry (one budget per chain ⇒ one epoch == one block
/// height per chain). Mirrors `counters::chain_counters`.
static BUDGETS: Lazy<RwLock<HashMap<u64, Arc<MultihopEmitBudget>>>> =
    Lazy::new(|| RwLock::new(HashMap::new()));

/// The shared budget of `chain_id`, created from [`BRIDGE_CAP_ENV`] on first use.
pub fn budget_for(chain_id: u64) -> Arc<MultihopEmitBudget> {
    if let Ok(g) = BUDGETS.read() {
        if let Some(b) = g.get(&chain_id) {
            return b.clone();
        }
    }
    let mut g = BUDGETS.write().unwrap_or_else(|e| e.into_inner());
    g.entry(chain_id)
        .or_insert_with(|| Arc::new(MultihopEmitBudget::new(cap_from_env())))
        .clone()
}

// ---------------------------------------------------------------------------
// Admission
// ---------------------------------------------------------------------------

/// Why a discovered cycle did NOT become a candidate. Every variant is an
/// honest stop with a stable telemetry token; none of them fabricates a row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BridgeSkip {
    /// `ARBX_MULTIHOP_EMIT` resolves OFF.
    Disabled,
    /// `legs < MIN_BRIDGE_HOPS` — 2-hop routes are `dex_engine`'s.
    TooFewHops,
    /// `legs > MAX_BRIDGE_HOPS` — outside the canonical 2..=7 envelope.
    TooManyHops,
    /// Not a new-block observation: no honest per-tick (per-block) bound exists,
    /// so the cap cannot be enforced — fail closed rather than emit unbounded.
    NotBlockSourced,
    /// The leg geometry is broken (`RouteIntent::token_path()` is `None`).
    DiscontinuousPath,
    /// The path does not close back on its first token (an open swap route is
    /// not a cycle).
    OpenCycle,
    /// The per-(chain, block) cap is spent.
    CapReached,
}

impl BridgeSkip {
    /// Stable lowercase token for logs/telemetry.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::TooFewHops => "too_few_hops",
            Self::TooManyHops => "too_many_hops",
            Self::NotBlockSourced => "not_block_sourced",
            Self::DiscontinuousPath => "discontinuous_path",
            Self::OpenCycle => "open_cycle",
            Self::CapReached => "cap_reached",
        }
    }
}

/// Geometry gates shared by admission and by the constructor. Pure — no I/O,
/// no env, no clock.
///
/// Returns the hop count on success.
pub fn cycle_geometry(intent: &RouteIntent) -> Result<usize, BridgeSkip> {
    let hops = intent.legs.len();
    if hops < MIN_BRIDGE_HOPS {
        return Err(BridgeSkip::TooFewHops);
    }
    if hops > MAX_BRIDGE_HOPS {
        return Err(BridgeSkip::TooManyHops);
    }
    if intent.source_event != DetectionSource::NewBlock {
        return Err(BridgeSkip::NotBlockSourced);
    }
    // `token_path()` is the repo's canonical geometry validator: it refuses a
    // zero first token, a non-contiguous chain, a zero leg output and a
    // self-swap. A candidate that fails it cannot become a route plan.
    if intent.token_path().is_none() {
        return Err(BridgeSkip::DiscontinuousPath);
    }
    // …and it must CLOSE: the last hop returns to the first token. An open
    // multi-hop swap (the mempool decoder's usual shape) is not an arb cycle.
    let first_in = match intent.legs.first() {
        Some(l) => l.token_in,
        None => return Err(BridgeSkip::TooFewHops),
    };
    let last_out = match intent.legs.last() {
        Some(l) => l.token_out,
        None => return Err(BridgeSkip::TooFewHops),
    };
    if first_in != last_out {
        return Err(BridgeSkip::OpenCycle);
    }
    Ok(hops)
}

/// Admit one discovered closed cycle: reversibility knob → geometry gates →
/// per-(chain, block) budget. On success returns the candidate the caller must
/// size and emit; on refusal NOTHING is built (and, for [`BridgeSkip::CapReached`],
/// the truncation is recorded on the budget).
///
/// PERHOP-RESERVES-01: the slot is claimed in the lane its
/// [`KernelCapability`] belongs to, so the budget can report how much of the
/// block's allowance went to work the kernel can price versus work it can only
/// refuse. The ORDER in which cycles reach this function is the caller's
/// ([`order_for_budget`]); this function never drops a row on its own account —
/// a refused cycle is refused by the cap, and counted.
pub fn admit(
    intent: &RouteIntent,
    budget: &MultihopEmitBudget,
    epoch: u64,
) -> Result<StrategyCandidate, BridgeSkip> {
    if !bridge_enabled() {
        return Err(BridgeSkip::Disabled);
    }
    cycle_geometry(intent)?;
    let lane = kernel_capability(intent).lane();
    if !budget.claim_lane(epoch, lane) {
        budget.note_refused(epoch, lane);
        return Err(BridgeSkip::CapReached);
    }
    Ok(cycle_candidate_from_intent(intent))
}

/// Build the `StrategyCandidate` for an already-admitted closed cycle.
///
/// The caller MUST have validated the geometry ([`cycle_geometry`]) and claimed
/// a budget slot ([`admit`]); this function assumes both and fabricates nothing.
///
/// Unchecked preconditions (documented, not asserted): `legs.len()` in
/// `MIN_BRIDGE_HOPS..=MAX_BRIDGE_HOPS` and a closed, contiguous path.
pub fn cycle_candidate_from_intent(intent: &RouteIntent) -> StrategyCandidate {
    let hops = intent.legs.len();
    let label = StrategyLabel::TriangularArb;
    let chain_id = intent.chain_id;
    let tx_hash = intent.tx_hash;

    let legs: Vec<RouteLeg> = intent.legs.iter().map(bridge_leg).collect();

    // Token path (hops + 1 addresses) — the same adjacent chain the kernel's
    // per-leg ledger is aligned with. `token_path()` was validated at admission;
    // the empty fallback is unreachable for admitted intents and, should it ever
    // happen, refuses the ledger rather than misaligning it (R8).
    let token_addresses: Vec<String> = intent
        .token_path()
        .unwrap_or_default()
        .iter()
        .map(|t| format!("{t:#x}"))
        .collect();

    // Pool addresses stay aligned 1:1 with the legs (length == hops) — an absent
    // hint becomes the empty string, exactly like
    // `persistence::build_route_metadata_from_plan` renders an absent pool, so
    // `RouteMetadata::validate()` keeps holding. The kernel still rejects that
    // leg with its own `MissingPoolAddress`.
    let pool_addresses: Vec<String> = intent
        .legs
        .iter()
        .map(|l| l.pool_hint.map(|p| format!("{p:#x}")).unwrap_or_default())
        .collect();

    // `dex_adapters` MUST be exactly `hops` long (attach_leg_ledger is
    // all-or-nothing on that length). A discovered leg carries no venue lookup
    // (`dex_hint` is None by construction in `route_intent_dispatcher`), so the
    // honest maximum is the leg's own protocol family — real data from the
    // intent, never a guessed venue.
    let dex_adapters: Vec<String> = intent
        .legs
        .iter()
        .map(|l| {
            l.dex_hint
                .clone()
                .unwrap_or_else(|| protocol_type_to_str(l.protocol_type))
        })
        .collect();

    let token_in_str = token_addresses.first().cloned().unwrap_or_default();
    let token_out_str = token_addresses.last().cloned().unwrap_or_default();
    let pair_symbol = pair_symbol_for(&token_in_str, hops);

    let opportunity = Opportunity {
        id: Uuid::new_v4(),
        chain_id,
        strategy_kind: label.to_contract_strategy_kind(),
        // A cycle has no single second counter-venue: the first two hops are the
        // two venues the cycle trades against, reported as-is.
        dex_a: dex_adapters.first().cloned().unwrap_or_default(),
        dex_b: dex_adapters.get(1).cloned(),
        pair_symbol,
        token_in: token_in_str,
        token_out: token_out_str,
        // The intent's own amount (the discovery dispatchers build intents with
        // `amount_in = 0`). Nothing is invented here: the sizing kernel
        // OVERWRITES this with its optimal size on the sized path, and on the
        // rejected path it stays the honest "not sized" value the column
        // already carries for pre-sizing rows.
        amount_in_wei: intent.amount_in.to_string(),
        // R8: NO profit is known at this layer — the kernel computes it.
        expected_profit_usd: None,
        net_expected_profit_usd: None,
        roi_pct: None,
        risk_score: None,
        block_number: intent.observed_block(),
        rejection_reason: None,
        cartridge_id: None,
        detector_id: Some(DETECTOR_ID.to_string()),
        pipeline_latency_ms: None,
        detected_at: chrono::Utc::now(),
        trace_id: Uuid::new_v4(),
    };

    let candidate = OpportunityCandidate {
        route_fingerprint: format!("{DETECTOR_ID}:{hops}:{tx_hash:x}"),
        pool_addresses,
        token_addresses,
        dex_adapters,
        // Mirror of `cartridge_boot`'s intent-derived candidate: the raw intent
        // amount in token units (0 for discovery-built intents). The spine gate
        // guards every use of it with `> 0.0`.
        amount_in: intent.amount_in.as_u128() as f64,
        expected_amount_out: 0.0,
        gross_profit: 0.0,
    };

    let route_plan = RoutePlan {
        route_id: Some(format!("{DETECTOR_ID}-{hops}-{tx_hash:x}")),
        // The label↔plan invariant (`route_plan.strategy_kind == label.as_str()`)
        // that `dex_engine::tests::route_plan_strategy_kind_matches_label` pins.
        strategy_kind: label.as_str().to_string(),
        chain_id,
        legs,
        atomic: true,
        estimated_slippage_pct: None,
        price_impact_pct: None,
    };

    debug!(
        event = "hop_cycle_bridge.candidate_built",
        chain_id,
        tx_hash = %tx_hash,
        hops,
        strategy = label.as_str(),
        pools_with_hint = intent.legs.iter().filter(|l| l.pool_hint.is_some()).count(),
    );

    StrategyCandidate {
        label,
        opportunity,
        candidate,
        route_plan,
        // R8: nothing computed yet — the N-leg cycle kernel owns these.
        gross_profit_usd: None,
        net_expected_profit_usd: None,
        rejection_reason: None,
        source_intent_hash: tx_hash,
        base_strategy: None,
    }
}

/// Display-only pair symbol for a closed cycle. A cycle's entry and exit token
/// are the SAME token, so `dex_engine`'s `A…/B…` shape would render as `X…/X…`;
/// the hop count is what actually distinguishes these rows.
fn pair_symbol_for(first_token_lower_hex: &str, hops: usize) -> String {
    let head = if first_token_lower_hex.len() >= 8 {
        &first_token_lower_hex[2..8]
    } else {
        first_token_lower_hex
    };
    format!("{head}({hops}-hop cycle)")
}

/// One `RouteLeg` per intent leg. The pool address is `Some(...)` only when the
/// intent carried a hint (never a placeholder); everything else is either the
/// intent's own value or an honest absence (`amount_out`/`tvl_usd`/... = `None`).
fn bridge_leg(leg: &RouteIntentLeg) -> RouteLeg {
    let family = protocol_type_to_str(leg.protocol_type);
    let dex = leg.dex_hint.clone().unwrap_or_else(|| family.clone());
    RouteLeg {
        dex_id: dex.to_ascii_lowercase(),
        dex_name: dex,
        protocol_type: family,
        // The intent carries no factory identity — empty, never a sentinel.
        factory_address: String::new(),
        pool_id: None,
        pool_address: leg.pool_hint.map(|p| format!("{p:#x}")),
        token_in: format!("{:#x}", leg.token_in),
        token_out: format!("{:#x}", leg.token_out),
        // The intent's own fee tier (`None` when the producer did not know it).
        // The N-leg cycle kernel prices its own fee constant; nothing is
        // defaulted here.
        fee_bps: leg.fee_bps,
        // Not computed at this layer (the kernel emits the per-leg ledger).
        amount_in: None,
        amount_out: None,
        tvl_usd: None,
        volume_24h_usd: None,
        pool_is_active: true,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::engines::triangular_engine::ReservesCache;
    use crate::route_discovery::route_intent_dispatcher::build_intent;
    use crate::route_discovery::types::{RouteCandidate, RouteDirection, RouteKind};
    use crate::route_intent::{ProtocolType, RouterKind, SwapExactMode};
    use crate::size_optimizer::{OptimizeOutcome, OptimizeRejectReason, SizeOptimizer};
    use crate::state_projector::StateProjector;
    use crate::v3_fee_catalog::V3FeeCatalog;
    use ethers::types::{Address, H256, U256};
    use shared_rs::trading_config::{GasPriceStrategy, TradingConfigState};

    const WETH_T: &str = "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2";

    fn addr(n: u64) -> Address {
        Address::from_low_u64_be(n)
    }

    fn token(s: &str) -> Address {
        s.parse::<Address>().expect("canonical token address")
    }

    fn unit(n: u64) -> U256 {
        U256::from(10u128).pow(U256::from(18u32)) * U256::from(n)
    }

    fn empty_v3_fee_catalog() -> Arc<V3FeeCatalog> {
        Arc::new(V3FeeCatalog::new())
    }

    fn make_cfg(capital_usd: f64) -> TradingConfigState {
        TradingConfigState {
            chain_id: 1,
            capital_usd,
            base_token_symbol: "WETH".into(),
            base_token_price_usd: 3000.0,
            allowed_token_symbols: vec!["WETH".into(), "USDC".into()],
            token_prices_usd: HashMap::new(),
            simulation_capital_usd: None,
            simulation_per_token_amounts_usd: HashMap::new(),
            simulation_per_strategy_caps_usd: HashMap::new(),
            simulation_target_profit_usd: None,
            simulation_target_roi_pct: None,
            min_profit_usd: 0.01,
            min_roi_pct: 0.0,
            min_landing_probability: 0.0,
            min_liquidity_confidence: 0.0,
            max_token_risk_score: 1.0,
            gas_price_strategy: GasPriceStrategy::Fixed,
            fixed_gas_price_gwei: Some(20.0),
            gas_estimate_units: 200_000,
            max_slippage_pct: 1.0,
            failure_risk_buffer_pct: 0.001,
            flashloan_fee_pct: 0.0,
            // Deliberately permissive: these gates are NOT what this module
            // tests (an empty list would be permissive too).
            enabled_strategies: vec!["triangular_arb".into()],
            enabled_dex_ids: None,
            strategy_configs: HashMap::new(),
            capital_cost_rate_annual_pct: 0.0,
            ops_overhead_usd_per_attempt: 0.0,
            spread_sanity_mult: 3.0,
            p_copied_volume_threshold_usd: 1_000_000.0,
            p_copied_max: 0.5,
            lp_fee_default_pct: 0.003,
            kelly_multiplier: 0.5,
            kelly_max_per_trade_fraction: 1.0,
            kelly_gas_safety_multiplier: 1.0,
            enabled: true,
            updated_at: chrono::Utc::now(),
            updated_by: None,
        }
    }

    /// A closed `hops`-cycle discovered by the route finder: `WETH → T1 → … →
    /// WETH`, all V2, each leg with its own pool and 30 bps fee tier — the real
    /// `RouteCandidate` shape `unique_route_finder` emits.
    fn make_route_candidate(hops: usize) -> RouteCandidate {
        let mut tokens = vec![token(WETH_T)];
        for i in 0..hops.saturating_sub(1) {
            tokens.push(addr(0x100 + i as u64));
        }
        let pools = (0..hops).map(|i| addr(0x20 + i as u64)).collect::<Vec<_>>();
        RouteCandidate {
            chain_id: 1,
            route_hash: format!("0x{}", "ab".repeat(32)),
            route_kind: if hops == 3 {
                RouteKind::Triangular
            } else {
                RouteKind::MultiHop
            },
            tokens,
            pools,
            protocols: vec![ProtocolType::V2; hops],
            fee_tiers: vec![Some(30); hops],
            directions: vec![RouteDirection::ZeroForOne; hops],
            hops: hops as u8,
            applicable_strategies: vec![StrategyLabel::TriangularArb],
            rejected_strategies: vec![],
            mode: "shadow".to_string(),
        }
    }

    /// The production shape the discovery workers hand to the orchestrator: the
    /// canonical `build_intent` output, stamped with the tick's observed block
    /// (exactly what `route_discovery_worker`'s run loop does before dispatch).
    fn make_intent(hops: usize, block: u64) -> RouteIntent {
        let c = make_route_candidate(hops);
        let mut intent = build_intent(&c).expect("canonical build_intent must succeed");
        intent.observed_block_number = Some(block);
        intent
    }

    /// Insert oriented `(Rin, Rout)` in canonical (token0 = min address) order —
    /// the orientation the N-leg kernel reads back by string comparison.
    async fn insert_oriented(
        cache: &Arc<ReservesCache>,
        pool: Address,
        token_in: &str,
        token_out: &str,
        r_in: U256,
        r_out: U256,
    ) {
        if token_in <= token_out {
            cache.insert(pool, r_in, r_out).await;
        } else {
            cache.insert(pool, r_out, r_in).await;
        }
    }

    /// A `hops`-hop cycle that IS profitable only as a whole
    /// (0.9 × 0.95 × 1.5 × 1.5 ≈ 1.92 > 1), reserving WETH on the first leg so
    /// the kernel's USD anchor resolves to WETH. `skip_leg` omits one pool from
    /// the cache (gate b: the kernel must then reject with its own reason).
    async fn profitable_cache(hops: usize, skip_leg: Option<usize>) -> Arc<ReservesCache> {
        let tokens: Vec<String> = {
            let mut t = vec![WETH_T.to_string()];
            for i in 0..hops - 1 {
                t.push(format!("0x{:040x}", 0x100 + i));
            }
            t.push(WETH_T.to_string());
            t
        };
        let pools: Vec<Address> = (0..hops).map(|i| addr(0x20 + i as u64)).collect();
        // Per-leg (r_in, r_out): leg 0+1 are unfavourable, the tail pays for the
        // whole cycle — so only the N-leg kernel can find the profit.
        let legs: Vec<(u64, u64)> = (0..hops)
            .map(|i| match i {
                0 => (100, 90),
                1 => (100, 95),
                _ => (100, 150),
            })
            .collect();
        let cache = Arc::new(ReservesCache::new());
        for i in 0..hops {
            if skip_leg == Some(i) {
                continue;
            }
            let (rin, rout) = legs[i];
            insert_oriented(
                &cache,
                pools[i],
                &tokens[i],
                &tokens[i + 1],
                unit(rin),
                unit(rout),
            )
            .await;
        }
        cache
    }

    fn optimizer_with(cache: Arc<ReservesCache>) -> SizeOptimizer {
        let projector = Arc::new(StateProjector::new(cache, None, empty_v3_fee_catalog()));
        SizeOptimizer::new(projector)
    }

    // ── GATE (c): 2-hop cycles are NOT this path's ──────────────────────────

    #[test]
    fn gate_c_two_hop_intent_is_refused_by_geometry() {
        let two = make_intent(2, 1_000);
        assert_eq!(two.legs.len(), 2, "fixture must be a 2-hop cycle");
        assert_eq!(
            cycle_geometry(&two),
            Err(BridgeSkip::TooFewHops),
            "a 2-hop route belongs to dex_engine — this bridge must refuse it"
        );

        let budget = MultihopEmitBudget::new(4);
        let err = admit(&two, &budget, 1_000).expect_err("2-hop must not be admitted");
        assert_eq!(
            err,
            BridgeSkip::TooFewHops,
            "admission must refuse before claiming any budget slot"
        );
        assert_eq!(
            budget.used_in(1_000),
            0,
            "a refused 2-hop intent must not consume the cap"
        );
    }

    #[test]
    fn gate_c_bridged_candidates_are_structurally_distinct_from_dex_engine() {
        // dex_engine builds exactly 2 legs; every candidate this bridge builds
        // has >= 3 — the two sets cannot collide.
        for hops in MIN_BRIDGE_HOPS..=MAX_BRIDGE_HOPS {
            let intent = make_intent(hops, 1_000);
            let c = cycle_candidate_from_intent(&intent);
            assert_eq!(c.route_plan.legs.len(), hops);
            assert!(c.route_plan.legs.len() >= MIN_BRIDGE_HOPS);
            assert_eq!(c.label, StrategyLabel::TriangularArb);
            assert_eq!(c.route_plan.strategy_kind, c.label.as_str());
        }
    }

    #[test]
    fn hop_envelope_is_the_canonical_two_to_seven() {
        let too_many = make_intent(8, 1_000);
        assert_eq!(
            cycle_geometry(&too_many),
            Err(BridgeSkip::TooManyHops),
            "8 hops is outside the canonical 2..=7 envelope"
        );
    }

    #[test]
    fn open_path_is_refused() {
        // A mempool-shaped multi-hop swap: A→B→C→D, NOT closed.
        let legs = vec![
            RouteIntentLeg {
                token_in: token(WETH_T),
                token_out: addr(0x1),
                pool_hint: Some(addr(0x21)),
                dex_hint: None,
                fee_bps: Some(30),
                protocol_type: ProtocolType::V2,
            },
            RouteIntentLeg {
                token_in: addr(0x1),
                token_out: addr(0x2),
                pool_hint: Some(addr(0x22)),
                dex_hint: None,
                fee_bps: Some(30),
                protocol_type: ProtocolType::V2,
            },
            RouteIntentLeg {
                token_in: addr(0x2),
                token_out: addr(0x3),
                pool_hint: Some(addr(0x23)),
                dex_hint: None,
                fee_bps: Some(30),
                protocol_type: ProtocolType::V2,
            },
        ];
        let mut intent = RouteIntent::new(
            1,
            H256::from_low_u64_be(0xABCD),
            Address::zero(),
            RouterKind::Unknown,
            Address::zero(),
            legs,
            U256::zero(),
            None,
            SwapExactMode::ExactIn,
            DetectionSource::NewBlock,
        )
        .expect("valid intent");
        intent.observed_block_number = Some(1_000);
        assert_eq!(
            cycle_geometry(&intent),
            Err(BridgeSkip::OpenCycle),
            "an open A→B→C→D swap is not an arbitrage cycle"
        );
    }

    #[test]
    fn non_block_source_is_refused() {
        let mut intent = make_intent(4, 1_000);
        intent.source_event = DetectionSource::PublicMempool;
        assert_eq!(
            cycle_geometry(&intent),
            Err(BridgeSkip::NotBlockSourced),
            "without a block there is no honest per-tick bound — fail closed"
        );
    }

    // ── GATE (d): the per-tick cap bounds the work, truncation observable ───

    #[test]
    fn gate_d_cap_bounds_admissions_and_truncation_is_observable() {
        const CAP: usize = 3;
        const N: usize = 10;
        const EPOCH: u64 = 21_000_000;

        let budget = MultihopEmitBudget::new(CAP);
        let intent = make_intent(4, EPOCH);

        let mut admitted = 0usize;
        let mut refused = 0usize;
        for _ in 0..N {
            match admit(&intent, &budget, EPOCH) {
                Ok(_) => admitted += 1,
                Err(BridgeSkip::CapReached) => refused += 1,
                Err(other) => panic!("unexpected skip: {}", other.as_str()),
            }
        }

        assert_eq!(admitted, CAP, "exactly `cap` cycles may be processed");
        assert_eq!(refused, N - CAP, "every excess cycle must be refused");
        assert_eq!(
            budget.used_in(EPOCH),
            CAP as u64,
            "the budget must report the exact processed count"
        );
        assert_eq!(
            budget.dropped_in(EPOCH),
            (N - CAP) as u64,
            "truncation must be observable (R8: never a silent drop)"
        );

        // A new block resets the allowance (per-tick semantics).
        let next = EPOCH + 1;
        assert!(budget.claim(next), "a new epoch must reopen the allowance");
        assert_eq!(budget.dropped_in(next), 0, "drop counter is per-epoch");
    }

    #[test]
    fn zero_cap_mutes_without_structurally_disabling() {
        let budget = MultihopEmitBudget::new(0);
        let intent = make_intent(4, 7);
        let err = admit(&intent, &budget, 7).expect_err("cap 0 admits nothing");
        assert_eq!(err, BridgeSkip::CapReached);
        assert_eq!(budget.dropped_in(7), 1);
    }

    // ── PERHOP-RESERVES-01: kernel capability = the budget lane ─────────────

    /// A `hops`-cycle with one leg's protocol family overridden — the production
    /// shape (`["uniswap-v2","uniswap-v3","uniswap-v3","uniswap-v3"]`).
    fn make_mixed_route_candidate(hops: usize, v3_legs: &[usize]) -> RouteCandidate {
        let mut c = make_route_candidate(hops);
        for &i in v3_legs {
            c.protocols[i] = ProtocolType::V3;
        }
        c
    }

    fn make_mixed_intent(hops: usize, v3_legs: &[usize], block: u64) -> RouteIntent {
        let c = make_mixed_route_candidate(hops, v3_legs);
        let mut intent = build_intent(&c).expect("canonical build_intent must succeed");
        intent.observed_block_number = Some(block);
        intent
    }

    #[test]
    fn capability_mirrors_the_kernel_v3_predicate() {
        // All-V2 ⇒ the N-leg constant-product kernel owns it.
        for hops in MIN_BRIDGE_HOPS..=MAX_BRIDGE_HOPS {
            assert_eq!(
                kernel_capability(&make_intent(hops, 1)),
                KernelCapability::V2CycleSizeable,
                "{hops}-hop all-V2 cycle must be sizeable"
            );
        }
        // Any V3 leg (at any position) ⇒ V3MultilegUnsupported, the reason the
        // optimizer emits — NOT a V2 reserve miss.
        for v3_at in 0..4usize {
            let intent = make_mixed_intent(4, &[v3_at], 1);
            assert_eq!(
                kernel_capability(&intent),
                KernelCapability::V3MultilegUnsupported,
                "a V3 leg at index {v3_at} must classify the cycle as unpriceable"
            );
        }
        // The predicate is the SAME table the kernel reads (`leg_is_v3` tests the
        // rendered `protocol_type` string for "v3").
        assert!(protocol_is_v3(ProtocolType::V3));
        assert!(!protocol_is_v3(ProtocolType::V2));
        assert_eq!(KernelCapability::V2CycleSizeable.lane(), EmitLane::Sizeable);
        assert_eq!(
            KernelCapability::V3MultilegUnsupported.lane(),
            EmitLane::Unpriceable
        );
    }

    #[test]
    fn budget_lanes_are_counted_separately_and_reset_per_epoch() {
        const EPOCH: u64 = 5_000;
        let budget = MultihopEmitBudget::new(4);
        assert!(budget.claim_lane(EPOCH, EmitLane::Sizeable));
        assert!(budget.claim_lane(EPOCH, EmitLane::Unpriceable));
        assert!(budget.claim_lane(EPOCH, EmitLane::Unpriceable));
        assert_eq!(budget.sizeable_used_in(EPOCH), 1);
        assert_eq!(budget.unpriceable_used_in(EPOCH), 2);
        assert_eq!(budget.used_in(EPOCH), 3);
        // The cap is SHARED — the lane never widens it.
        assert!(budget.claim_lane(EPOCH, EmitLane::Sizeable));
        assert!(!budget.claim_lane(EPOCH, EmitLane::Unpriceable));
        budget.note_refused(EPOCH, EmitLane::Unpriceable);
        assert_eq!(budget.dropped_in(EPOCH), 1);
        assert_eq!(budget.unpriceable_refused_in(EPOCH), 1);
        // A plain sizeable refusal is NOT an ordering deferral.
        budget.note_refused(EPOCH, EmitLane::Sizeable);
        assert_eq!(budget.dropped_in(EPOCH), 2);
        assert_eq!(budget.unpriceable_refused_in(EPOCH), 1);
        // New epoch ⇒ every lane counter resets.
        let next = EPOCH + 1;
        assert!(budget.claim_lane(next, EmitLane::Unpriceable));
        assert_eq!(budget.sizeable_used_in(next), 0);
        assert_eq!(budget.unpriceable_used_in(next), 1);
        assert_eq!(budget.unpriceable_refused_in(next), 0);
        assert_eq!(budget.dropped_in(next), 0);
    }

    #[test]
    fn cap_report_is_claimed_once_per_epoch_and_per_item_lines_stay_debug() {
        // LOGFLOOD-02 (R9): the aggregate cap-hit line is emitted once per epoch,
        // no matter how many cycles the cap refuses — the volume that collapsed
        // the container's log window came from warning on EVERY refusal.
        const EPOCH: u64 = 5_100;
        let budget = MultihopEmitBudget::new(1);
        assert!(budget.claim(EPOCH));
        assert!(!budget.claim(EPOCH));
        budget.note_refused(EPOCH, EmitLane::Unpriceable);
        assert_eq!(budget.dropped_in(EPOCH), 1);

        assert!(budget.claim_cap_report(EPOCH), "first refusal of the epoch reports");
        for _ in 0..1_000 {
            budget.note_refused(EPOCH, EmitLane::Unpriceable);
            assert!(
                !budget.claim_cap_report(EPOCH),
                "every further refusal of the same epoch must stay a debug line"
            );
        }
        // The counting is untouched: the cap still bounds, the drops still count.
        assert_eq!(budget.dropped_in(EPOCH), 1_001);

        // A new epoch earns its own single aggregate line, and the previous
        // epoch cannot claim a second one.
        let next = EPOCH + 1;
        assert!(budget.claim_cap_report(next));
        assert!(!budget.claim_cap_report(next));
        assert!(!budget.claim_cap_report(EPOCH));
    }

    #[test]
    fn ordering_is_a_stable_lane_partition_and_reversible() {
        // The production shape: the finder's gross-score order puts V3-bearing
        // cycles first and the (rare) all-V2 cycle last.
        let routes = vec![
            make_mixed_route_candidate(4, &[0, 1, 2, 3]),
            make_mixed_route_candidate(4, &[1, 2, 3]),
            make_route_candidate(4),
            make_mixed_route_candidate(5, &[4]),
            make_route_candidate(3),
        ];

        let ordered = order_for_budget(&routes, true);
        assert_eq!(ordered.len(), routes.len(), "ordering never drops an item");
        let hops: Vec<u8> = ordered.iter().map(|c| c.hops).collect();
        assert_eq!(
            hops,
            vec![4, 3, 4, 4, 5],
            "sizeable cycles keep the finder's relative order and come first; \
             unpriceable cycles follow in their own finder order"
        );
        assert!(
            ordered[..2]
                .iter()
                .all(|c| kernel_capability_of_protocols(c.protocols.iter())
                    == KernelCapability::V2CycleSizeable),
            "the head of the ordered set must be the sizeable lane"
        );

        // Reversibility: the knob OFF reproduces the pre-patch sequence exactly.
        let off = order_for_budget(&routes, false);
        let off_hashes: Vec<&str> = off.iter().map(|c| c.route_hash.as_str()).collect();
        let in_hashes: Vec<&str> = routes.iter().map(|c| c.route_hash.as_str()).collect();
        assert_eq!(
            off_hashes, in_hashes,
            "ARBX_MULTIHOP_PREFER_SIZEABLE=off must restore finder order verbatim"
        );
    }

    #[test]
    fn prefer_sizeable_knob_is_default_on_and_explicitly_reversible() {
        assert!(prefer_sizeable_from_raw(None), "absent ⇒ ON (default)");
        assert!(prefer_sizeable_from_raw(Some("")));
        assert!(prefer_sizeable_from_raw(Some("true")));
        assert!(prefer_sizeable_from_raw(Some("ON")));
        assert!(!prefer_sizeable_from_raw(Some("off")));
        assert!(!prefer_sizeable_from_raw(Some(" OFF ")));
        assert!(!prefer_sizeable_from_raw(Some("false")));
        assert!(!prefer_sizeable_from_raw(Some("0")));
        assert!(
            prefer_sizeable_from_raw(Some("banana")),
            "a foreign value is never interpreted as a verdict (R8)"
        );
    }

    /// GATE (e) — PERHOP-RESERVES-01, the brief's ordering gate.
    ///
    /// With the per-block cap SATURATED, the admitted set must be the cycles the
    /// kernel can price, and the cycles deferred behind them must be COUNTED
    /// (R8 — never a silent skip). Before this patch the admitted set was the
    /// first `cap` cycles of the finder's order — which in production is 100 %
    /// V3-bearing, i.e. 100 % unpriceable, so the block's whole allowance bought
    /// nothing that could reach a ledger.
    #[tokio::test]
    async fn gate_e_saturated_cap_admits_the_sizeable_set_and_counts_the_deferral() {
        const CAP: usize = 2;
        const EPOCH: u64 = 21_000_042;
        let budget = MultihopEmitBudget::new(CAP);

        // Finder order: five unpriceable cycles, then two sizeable ones — exactly
        // the production starvation shape.
        let routes = vec![
            make_mixed_route_candidate(4, &[0, 1, 2, 3]),
            make_mixed_route_candidate(4, &[1]),
            make_mixed_route_candidate(5, &[4]),
            make_mixed_route_candidate(3, &[0]),
            make_mixed_route_candidate(6, &[3]),
            make_route_candidate(3),
            make_route_candidate(4),
        ];

        let mut admitted_hops: Vec<usize> = Vec::new();
        let mut deferred_unpriceable = 0usize;
        for c in order_for_budget(&routes, true) {
            let mut intent = build_intent(c).expect("canonical build_intent");
            intent.observed_block_number = Some(EPOCH);
            let capability = kernel_capability(&intent);
            match admit(&intent, &budget, EPOCH) {
                Ok(cand) => {
                    assert_eq!(
                        capability,
                        KernelCapability::V2CycleSizeable,
                        "only sizeable cycles may be admitted while one is waiting"
                    );
                    admitted_hops.push(cand.route_plan.legs.len());
                }
                Err(BridgeSkip::CapReached) => {
                    if capability == KernelCapability::V3MultilegUnsupported {
                        deferred_unpriceable += 1;
                    }
                }
                Err(other) => panic!("unexpected skip: {}", other.as_str()),
            }
        }

        assert_eq!(
            admitted_hops,
            vec![3, 4],
            "the saturated cap must be spent on the sizeable cycles, not on the \
             unpriceable ones that precede them in finder order"
        );
        assert_eq!(budget.used_in(EPOCH), CAP as u64);
        assert_eq!(
            budget.sizeable_used_in(EPOCH),
            CAP as u64,
            "every spent slot went to priceable work"
        );
        assert_eq!(budget.unpriceable_used_in(EPOCH), 0);
        assert_eq!(
            budget.dropped_in(EPOCH),
            5,
            "every refused cycle is still counted"
        );
        assert_eq!(
            budget.unpriceable_refused_in(EPOCH),
            deferred_unpriceable as u64,
            "the deferred-behind-sizeable count must equal the observed deferrals"
        );
        assert_eq!(
            budget.unpriceable_refused_in(EPOCH),
            5,
            "all five unpriceable cycles were deferred (observable, R8)"
        );

        // Reversibility: with the knob OFF the pre-patch first-K slice is spent
        // on the unpriceable cycles — the exact production defect.
        let legacy = MultihopEmitBudget::new(CAP);
        let mut legacy_hops: Vec<usize> = Vec::new();
        for c in order_for_budget(&routes, false) {
            let mut intent = build_intent(c).expect("canonical build_intent");
            intent.observed_block_number = Some(EPOCH);
            if let Ok(cand) = admit(&intent, &legacy, EPOCH) {
                legacy_hops.push(cand.route_plan.legs.len());
            }
        }
        assert_eq!(
            legacy_hops,
            vec![4, 4],
            "pre-patch behaviour: the cap goes to the first cycles in finder order"
        );
        assert_eq!(
            legacy.sizeable_used_in(EPOCH),
            0,
            "pre-patch, zero slots reached the sizeable lane"
        );
    }

    /// GATE (f) — the four production V3-bearing shapes must be classified
    /// unpriceable, and the one all-V2 shape sizeable.
    #[test]
    fn gate_f_production_shapes_classify_as_measured() {
        let measured: [(&[usize], KernelCapability); 5] = [
            (&[0, 1, 2, 3], KernelCapability::V3MultilegUnsupported),
            (&[1, 2, 3], KernelCapability::V3MultilegUnsupported),
            (&[2, 3], KernelCapability::V3MultilegUnsupported),
            (&[0, 1, 2, 3, 4], KernelCapability::V3MultilegUnsupported),
            (&[], KernelCapability::V2CycleSizeable),
        ];
        for (v3_legs, expected) in measured {
            let hops = if v3_legs.is_empty() {
                3
            } else {
                v3_legs.iter().max().copied().unwrap() + 1
            };
            let c = make_mixed_route_candidate(hops, v3_legs);
            assert_eq!(
                kernel_capability_of_protocols(c.protocols.iter()),
                expected,
                "hops={hops} v3_legs={v3_legs:?}"
            );
        }
    }

    // ── Reversibility knob ──────────────────────────────────────────────────

    #[test]
    fn toggle_parser_is_default_on_and_explicitly_reversible() {
        assert!(bridge_enabled_from_raw(None), "absent ⇒ ON (default)");
        assert!(bridge_enabled_from_raw(Some("")), "empty ⇒ ON (default)");
        assert!(bridge_enabled_from_raw(Some("true")));
        assert!(bridge_enabled_from_raw(Some("ON")));
        assert!(!bridge_enabled_from_raw(Some("off")));
        assert!(!bridge_enabled_from_raw(Some(" OFF ")));
        assert!(!bridge_enabled_from_raw(Some("false")));
        assert!(!bridge_enabled_from_raw(Some("0")));
        // A foreign value is never interpreted as a verdict (R8).
        assert!(bridge_enabled_from_raw(Some("banana")));
    }

    // ── GATE (a): 4-hop cycle + cached reserves ⇒ Sized with a 4-leg ledger ──

    #[tokio::test]
    async fn gate_a_four_hop_cycle_sizes_with_a_four_leg_ledger() {
        let intent = make_intent(4, 1_000);
        let budget = MultihopEmitBudget::new(4);
        let candidate = admit(&intent, &budget, 1_000).expect("4-hop cycle must be admitted");

        // The bridge computed no economics (R8).
        assert!(candidate.gross_profit_usd.is_none());
        assert!(candidate.opportunity.expected_profit_usd.is_none());
        assert!(candidate.opportunity.net_expected_profit_usd.is_none());

        let optimizer = optimizer_with(profitable_cache(4, None).await);
        let cfg = make_cfg(100_000.0);
        let outcome = optimizer
            .optimize_with_reason(candidate.clone(), &intent, Some(&cfg))
            .await
            .expect("sizing must not error");

        let sized = match outcome {
            OptimizeOutcome::Sized(s) => *s,
            OptimizeOutcome::Rejected(r, _) | OptimizeOutcome::RejectedWithLedger(r, _, _) => {
                panic!(
                    "4-hop profitable cycle must size, got Rejected({})",
                    r.as_str()
                )
            }
        };

        let ins = sized
            .leg_amounts_in
            .as_ref()
            .expect("sized row must carry leg_amounts_in");
        let outs = sized
            .leg_amounts_out
            .as_ref()
            .expect("sized row must carry leg_amounts_out");
        assert_eq!(ins.len(), 4, "the ledger must have one entry per hop");
        assert_eq!(outs.len(), 4);
        assert_eq!(
            ins[0],
            sized.optimal_amount_in.to_string(),
            "ledger[0] must be the reported optimal size"
        );
        for i in 0..3 {
            assert_eq!(
                outs[i],
                ins[i + 1],
                "the ledger chain must be contiguous at hop {i}"
            );
        }

        // …and the row that reaches the emitter carries the SAME 4-hop topology.
        assert_eq!(sized.candidate.route_plan.legs.len(), 4);
        let mut rm =
            crate::persistence::build_route_metadata_from_plan(&sized.candidate.route_plan);
        assert_eq!(rm.dex_adapters.len(), 4);
        assert_eq!(rm.token_addresses.len(), 5);
        assert!(
            rm.attach_leg_ledger(ins, outs),
            "the ledger must attach to the 4-hop RouteMetadata as-is"
        );
        assert_eq!(rm.leg_amounts_in.as_ref().map(|v| v.len()), Some(4));
        assert_eq!(rm.leg_zero_for_one.as_ref().map(|v| v.len()), Some(4));
    }

    // ── GATE (b): one pool missing from the cache ⇒ explicit rejection ──────

    #[tokio::test]
    async fn gate_b_missing_pool_reserves_rejects_with_no_fabricated_amounts() {
        let intent = make_intent(4, 1_000);
        let budget = MultihopEmitBudget::new(4);
        let candidate = admit(&intent, &budget, 1_000).expect("admitted");

        // Hop 2's pool was never synced.
        let optimizer = optimizer_with(profitable_cache(4, Some(2)).await);
        let cfg = make_cfg(100_000.0);
        let outcome = optimizer
            .optimize_with_reason(candidate.clone(), &intent, Some(&cfg))
            .await
            .expect("sizing must not error");

        match outcome {
            OptimizeOutcome::Rejected(reason, net)
            | OptimizeOutcome::RejectedWithLedger(reason, net, _) => {
                assert_eq!(
                    reason,
                    OptimizeRejectReason::MissingReservesPoolB,
                    "the kernel's own explicit reason must survive verbatim"
                );
                assert_eq!(reason.as_str(), "missing_reserves_pool_b");
                assert!(
                    net.is_none(),
                    "no economics may be stamped when the cycle could not be evaluated"
                );
            }
            OptimizeOutcome::Sized(s) => panic!(
                "a missing hop must never size — got Sized with ledger {:?}",
                s.leg_amounts_in
            ),
        }
    }

    #[tokio::test]
    async fn gate_b_missing_first_pool_rejects_as_pool_a() {
        let intent = make_intent(4, 1_000);
        let budget = MultihopEmitBudget::new(4);
        let candidate = admit(&intent, &budget, 1_000).expect("admitted");
        let optimizer = optimizer_with(profitable_cache(4, Some(0)).await);
        let cfg = make_cfg(100_000.0);
        let outcome = optimizer
            .optimize_with_reason(candidate, &intent, Some(&cfg))
            .await
            .expect("sizing must not error");
        match outcome {
            OptimizeOutcome::Rejected(reason, _)
            | OptimizeOutcome::RejectedWithLedger(reason, _, _) => {
                assert_eq!(reason, OptimizeRejectReason::MissingReservesPoolA)
            }
            OptimizeOutcome::Sized(_) => panic!("missing hop 0 must never size"),
        }
    }

    #[tokio::test]
    async fn absent_pool_hint_rejects_with_missing_pool_address() {
        // RULE 00: a leg without a pool identity is NOT patched with a
        // placeholder — the kernel's own MissingPoolAddress row is the contract.
        let mut intent = make_intent(3, 1_000);
        intent.legs[1].pool_hint = None;
        // Re-open the cycle so geometry still passes (token path unchanged).
        let budget = MultihopEmitBudget::new(4);
        let candidate = admit(&intent, &budget, 1_000).expect("admitted");
        assert_eq!(
            candidate.route_plan.legs[1].pool_address, None,
            "an absent hint must stay absent on the leg"
        );
        let optimizer = optimizer_with(profitable_cache(3, None).await);
        let cfg = make_cfg(100_000.0);
        let outcome = optimizer
            .optimize_with_reason(candidate, &intent, Some(&cfg))
            .await
            .expect("sizing must not error");
        match outcome {
            OptimizeOutcome::Rejected(reason, _)
            | OptimizeOutcome::RejectedWithLedger(reason, _, _) => {
                assert_eq!(reason, OptimizeRejectReason::MissingPoolAddress)
            }
            OptimizeOutcome::Sized(_) => panic!("a leg without a pool must never size"),
        }
    }

    // ── RoutePlan geometry the downstream tail depends on ───────────────────

    #[test]
    fn candidate_carries_the_full_closed_path_and_aligned_vectors() {
        let intent = make_intent(5, 1_000);
        let c = cycle_candidate_from_intent(&intent);
        let plan = &c.route_plan;

        assert_eq!(plan.legs.len(), 5);
        for i in 0..5 {
            assert_eq!(
                plan.legs[i].token_out,
                plan.legs[(i + 1) % 5].token_in,
                "legs must chain hop {i} → {}",
                (i + 1) % 5
            );
            assert!(plan.legs[i].pool_address.is_some());
        }
        // Length contracts the emitter's tail enforces.
        assert_eq!(c.candidate.pool_addresses.len(), 5);
        assert_eq!(c.candidate.dex_adapters.len(), 5);
        assert_eq!(c.candidate.token_addresses.len(), 6);
        assert_eq!(
            c.candidate.token_addresses[0],
            c.candidate.token_addresses[5]
        );
        assert_eq!(c.source_intent_hash, intent.tx_hash);
        assert_eq!(c.opportunity.detector_id.as_deref(), Some(DETECTOR_ID));
        assert_eq!(c.opportunity.strategy_kind.as_str(), "triangular");
    }
}
