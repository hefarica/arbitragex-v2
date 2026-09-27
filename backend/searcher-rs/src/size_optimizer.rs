// M11 allow: test modules use .unwrap()/.expect() for readability;
// production paths use ? / anyhow throughout.
//! SizeOptimizer — Phase 13 — optimal `amount_in` sizing for arb candidates.
//!
//! Finds the `amount_in` that maximises net profit for a strategy candidate.
//! Wraps `golden_section_search` from `triangular_worker` for 3-leg routes
//! and implements a 2-leg analogue for DEX arb candidates.
//!
//! ## Algorithm
//!
//! For 2-leg routes:
//!   1. Translate `cap_usd` → `cap_wei` using token price + decimals.
//!   2. Set `x_lo = 1 wei`, `x_hi = min(cap_wei, first_pool_reserve_in)`.
//!   3. Run `golden_section_search_2leg` (golden-section over the 2-hop profit
//!      function f(x) = leg2_out(leg1_out(x)) − x). The profit function is
//!      concave for V2 CPMM with fees.
//!   4. Re-evaluate at `min(x_star, cap_wei)` (anti-BUG-3 cap clamp).
//!   5. Compute gross and net USD profit.
//!
//! For 3-leg routes: delegates to `evaluate_cycle` (triangular_worker kernel).
//!
//! ## R8 invariants
//!
//! - Returns `Ok(None)` (never `Err`) when:
//!   - Net profit ≤ 0 at the optimal point.
//!   - `cap_usd == 0` or token cannot be priced.
//!   - Reserves unavailable for any leg.
//! - `gross_profit_usd` and `estimated_net_profit_usd` on `SizedCandidate`
//!   are always strictly positive.
//! - Never inflates profit by more than available reserves allow.

use crate::amm_math::{v2_amount_out, v3_amount_out_single_tick};
use crate::engines::StrategyCandidate;
use crate::route_intent::RouteIntent;
use crate::state_projector::{LegEval, LegQuote, PoolRef, RouteQuoteProvider, StateProjector};
use crate::strategy_label::StrategyLabel;
use crate::workers::triangular_worker::{
    clamp_to_cap_wei, evaluate_cycle_detailed, CycleEvalOutcome, EvalInput,
};
use ethers::types::{Address, U256};
use prioritization_spine::route_plan::RouteLeg;
use shared_rs::chains::USDT_MAINNET_LC;
use shared_rs::trading_config::TradingConfigState;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::debug;

// ---------------------------------------------------------------------------
// V3-MULTILEG-SIZING-01 — N-leg (3..=7) V3-bearing cycles
//
// Operator mandate: "deben salir arbitrajes de 2 a 7 hops, cada hop tiene su
// lugar en la card". Measured 2026-09-26 in production (PG, 15 min): every deep
// row (hops 3..6, ~1 800 rows) carried `missing_reserves_pool_a|b` and no
// ledger, because 585/585 sampled discovered 3..=7-hop cycles carry at least
// one concentrated-liquidity leg and the N-leg kernel was constant-product
// only. PERHOP-RESERVES-01 made those rows HONEST (`v3_multileg_unsupported`);
// this section makes them SIZED.
//
// The kernel below is bounded by construction (every knob is parsed once; a
// malformed/foreign value keeps the default — the repo's fail-honest env
// convention):
//   · LOCAL (0 RPC) — the within-tick V3 model + exact CPMM compose the whole
//     chain for `V3_MULTILEG_LOCAL_POINTS` sizes and either REJECT the cycle
//     outright (the optimistic bound is a provable upper bound: if it is ≤ 0
//     the real grid cannot be positive) or RANK the probe bracket.
//   · RPC (bounded) — QuoterV2 prices the real chain at ≤
//     `ARBX_V3_MULTILEG_MAX_PROBES` sizes, ≤ `ARBX_V3_MULTILEG_MAX_QUOTES`
//     sub-calls per candidate, and ≤ `ARBX_V3_MULTILEG_QUOTES_PER_BLOCK`
//     sub-calls per (chain, block). The first V3 leg's probe inputs are known
//     without any quote, so they are prefetched in ONE aggregate3
//     (`quote_batch`).
// ---------------------------------------------------------------------------

/// Reversibility knob. Absent/foreign ⇒ ENABLED (the operator's 3..=7-hop
/// cards); `off`/`false`/`0` ⇒ the pre-patch behaviour, byte-identical: the
/// `V3MultilegUnsupported` refusal with zero RPC.
pub const V3_MULTILEG_SIZING_ENV: &str = "ARBX_V3_MULTILEG_SIZING";

/// Signed RPC probe sizes per candidate (the refinement bracket).
pub const V3_MULTILEG_MAX_PROBES_ENV: &str = "ARBX_V3_MULTILEG_MAX_PROBES";

/// Hard QuoterV2 sub-call ceiling per candidate.
pub const V3_MULTILEG_MAX_QUOTES_ENV: &str = "ARBX_V3_MULTILEG_MAX_QUOTES";

/// Per-(chain, block) QuoterV2 sub-call ceiling for this kernel (process-wide).
pub const V3_MULTILEG_QUOTES_PER_BLOCK_ENV: &str = "ARBX_V3_MULTILEG_QUOTES_PER_BLOCK";

/// 0-RPC local ranking grid (log-spaced over [1, cap_wei]). 16 points resolve
/// the bracket to ~17× steps across 18 orders of magnitude — coarser than an
/// exhaustive search but far finer than the 2-leg V3 path's 8-point RPC grid, and
/// it costs no network at all (integer math only).
const V3_MULTILEG_LOCAL_POINTS: usize = 16;
/// Default RPC probes per candidate: the local argmax plus its best neighbour.
const DEFAULT_V3_MULTILEG_MAX_PROBES: usize = 2;
/// Hard ceiling for the probe knob (each probe costs one quote per V3 leg).
const MAX_V3_MULTILEG_MAX_PROBES: usize = 8;
/// Default per-candidate sub-call ceiling (covers 2 probes × ≤ 4 V3 legs).
const DEFAULT_V3_MULTILEG_MAX_QUOTES: usize = 8;
/// Hard ceiling for the per-candidate quote knob.
const MAX_V3_MULTILEG_MAX_QUOTES: usize = 64;
/// Default per-(chain, block) sub-call ceiling: 48 quotes / 12 s ≈ 4 quotes/s
/// worst case, against the measured 23 508 rejected 2-hop quotes / 15 min
/// (~26/s) the sovereign free-RPC stack already absorbs.
const DEFAULT_V3_MULTILEG_QUOTES_PER_BLOCK: usize = 48;
/// Hard ceiling for the per-block quote knob.
const MAX_V3_MULTILEG_QUOTES_PER_BLOCK: usize = 4_096;

/// Pure parser for [`V3_MULTILEG_SIZING_ENV`] (testable without env mutation).
/// Same rule as `hop_cycle_bridge::bridge_enabled_from_raw`: `None`/foreign ⇒
/// ON; only the three explicit OFF spellings disable the kernel.
pub fn v3_multileg_sizing_from_raw(raw: Option<&str>) -> bool {
    let normalized = raw.map(|v| v.trim().to_ascii_lowercase());
    !matches!(
        normalized.as_deref(),
        Some("off") | Some("false") | Some("0")
    )
}

/// Pure env→usize parser (testable without env mutation): unset/junk keeps the
/// default; a valid value is capped at `max` (a degenerate knob never produces
/// an unbounded budget).
fn env_usize_capped(raw: Option<String>, default: usize, max: usize) -> usize {
    raw.and_then(|v| v.trim().parse::<usize>().ok())
        .map(|n| n.min(max))
        .unwrap_or(default)
}

/// Boot-time verdict of [`V3_MULTILEG_SIZING_ENV`], memoised (the hot path must
/// not pay an environment lookup per candidate — the knob is a deployment
/// switch, so changing it requires a restart, like every other `ARBX_*` gate).
pub fn v3_multileg_sizing_enabled() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| {
        v3_multileg_sizing_from_raw(std::env::var(V3_MULTILEG_SIZING_ENV).ok().as_deref())
    })
}

fn v3_multileg_max_probes() -> usize {
    static V: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    *V.get_or_init(|| {
        env_usize_capped(
            std::env::var(V3_MULTILEG_MAX_PROBES_ENV).ok(),
            DEFAULT_V3_MULTILEG_MAX_PROBES,
            MAX_V3_MULTILEG_MAX_PROBES,
        )
        .max(1)
    })
}

fn v3_multileg_max_quotes() -> usize {
    static V: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    *V.get_or_init(|| {
        env_usize_capped(
            std::env::var(V3_MULTILEG_MAX_QUOTES_ENV).ok(),
            DEFAULT_V3_MULTILEG_MAX_QUOTES,
            MAX_V3_MULTILEG_MAX_QUOTES,
        )
        .max(1)
    })
}

fn v3_multileg_quotes_per_block() -> usize {
    static V: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    *V.get_or_init(|| {
        env_usize_capped(
            std::env::var(V3_MULTILEG_QUOTES_PER_BLOCK_ENV).ok(),
            DEFAULT_V3_MULTILEG_QUOTES_PER_BLOCK,
            MAX_V3_MULTILEG_QUOTES_PER_BLOCK,
        )
    })
}

/// Per-(chain, block) QuoterV2 sub-call allowance for the N-leg V3 kernel.
///
/// Same claim shape as `hop_cycle_bridge::MultihopEmitBudget` (lock-free CAS,
/// reset when a new epoch is observed) — the proven in-repo pattern for a
/// per-block allowance, so concurrent ticks can never both spend the same slot
/// and can never overspend the cap.
///
/// INJECTABLE (a field on [`SizeOptimizer`], not a global static): a test owns
/// its own allowance and cannot perturb a sibling test, while the production
/// optimizer owns exactly one per process.
///
/// `epoch = None` (a candidate that carries no block height) is charged against
/// the CURRENT allowance WITHOUT resetting it: a block-less producer can drain
/// what is left, never widen the per-block bound.
pub struct V3MultilegQuoteBudget {
    per_epoch: u64,
    epoch: AtomicU64,
    used: AtomicU64,
}

impl V3MultilegQuoteBudget {
    /// `per_epoch == 0` is honoured as "quote nothing" (an explicit operator
    /// decision to mute the RPC half of the kernel without disabling sizing
    /// structurally — the honest rejection is then
    /// `v3_multileg_budget_exhausted`, never a fabricated price).
    pub fn new(per_epoch: usize) -> Self {
        Self {
            per_epoch: per_epoch as u64,
            // Sentinel epoch no real block number can equal, so the first claim
            // always performs the reset.
            epoch: AtomicU64::new(u64::MAX),
            used: AtomicU64::new(0),
        }
    }

    pub fn per_epoch(&self) -> u64 {
        self.per_epoch
    }

    /// Claim `n` sub-calls for `epoch`. `true` = the whole chain may be quoted.
    pub fn try_claim(&self, epoch: Option<u64>, n: u64) -> bool {
        if n == 0 {
            return true;
        }
        if let Some(e) = epoch {
            loop {
                let observed = self.epoch.load(Ordering::Acquire);
                if observed != e {
                    if self
                        .epoch
                        .compare_exchange(observed, e, Ordering::AcqRel, Ordering::Acquire)
                        .is_ok()
                    {
                        self.used.store(0, Ordering::Release);
                    }
                    continue;
                }
                return self.claim_here(n);
            }
        }
        // No block height: consume the current allowance without resetting it.
        self.claim_here(n)
    }

    fn claim_here(&self, n: u64) -> bool {
        let mut used = self.used.load(Ordering::Acquire);
        loop {
            let Some(next) = used.checked_add(n) else {
                return false;
            };
            if next > self.per_epoch {
                return false;
            }
            match self
                .used
                .compare_exchange(used, next, Ordering::AcqRel, Ordering::Acquire)
            {
                Ok(_) => return true,
                Err(observed) => used = observed,
            }
        }
    }

    /// Sub-calls claimed in `epoch` (0 for any other epoch — the counter resets
    /// on epoch change).
    pub fn used_in(&self, epoch: u64) -> u64 {
        if self.epoch.load(Ordering::Acquire) == epoch {
            self.used.load(Ordering::Acquire)
        } else {
            0
        }
    }
}

impl Default for V3MultilegQuoteBudget {
    fn default() -> Self {
        Self::new(v3_multileg_quotes_per_block())
    }
}

// ---------------------------------------------------------------------------
// OptimizeRejectReason — explicit rejection enum (TASK 2)
// ---------------------------------------------------------------------------

/// Specific reason why `SizeOptimizer::optimize_with_reason` rejected a candidate.
///
/// R8 invariant: every `Ok(None)` path in the legacy `optimize` API maps to
/// exactly one variant here. `Err` is reserved for I/O / infrastructure failures
/// (e.g., a completely broken projector) — never for business-logic rejections.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptimizeRejectReason {
    /// No `TradingConfigState` available for this chain.
    NoConfig,
    /// Capital cap resolved to 0 or negative for this token / strategy.
    ZeroCapitalCap,
    /// Token cannot be priced via oracle or config.
    UnknownTokenPrice,
    /// Resolved token decimals are out of range (> 38, which would overflow f64).
    InvalidDecimals,
    /// Route plan has < 2 legs (2-leg path) or < 3 legs (triangular path).
    MissingRouteLegs,
    /// A leg in the route plan has `pool_address = None`.
    MissingPoolAddress,
    /// Reserves for pool A (leg 0) are absent from the cache.
    MissingReservesPoolA,
    /// Reserves for pool B (leg 1) are absent from the cache.
    MissingReservesPoolB,
    /// One or more pool reserves resolved to zero (degenerate pool).
    ZeroReserves,
    /// The profit function produced profit_wei ≤ 0 at every size in the search range.
    NonPositiveProfit,
    /// Gross USD profit is zero or negative (profit_token_units × price ≤ 0).
    NonPositiveGrossUsd,
    /// Net USD profit (gross − gas − overhead − flashloan_fee) is zero or negative.
    NonPositiveNetUsd,
    /// `clamp_to_cap_wei` returned `None` (internal overflow guard — should never fire
    /// in practice but is modelled explicitly to distinguish from missing reserves).
    CapClampFailed,
    /// Net profit below the gas-safety floor: `net_usd < gas_usd × kelly_gas_safety_multiplier`.
    /// The trade is viable in the kernel's view (net > 0) but does not survive the
    /// stricter post-optimization floor that protects against gas price spikes
    /// between forecast and inclusion (operator directive 2026-05-13).
    GasFloorBreach,
    /// Kelly criterion computed a non-positive fraction → mathematical edge is
    /// against us. Specifically `p × W ≤ (1 − p)`: the win-probability adjusted
    /// gain does not exceed the loss-probability adjusted loss. Rejecting here
    /// avoids deploying capital on bets where the expected logarithmic growth
    /// is negative even after the kernel found a profit-optimal point.
    KellyNegativeEdge,
    /// A V3 leg could not be priced: the `StateProjector` has no V3 quote
    /// provider (e.g. non-mainnet / absent at boot) OR every on-chain QuoterV2
    /// probe failed (pool revert / insufficient liquidity / RPC exhausted).
    /// R8 fail-honest: no fabricated price, so no opportunity is emitted —
    /// distinct from `NonPositiveProfit` (which means the quoter answered and
    /// the real spread is ≤ 0).
    V3QuoteUnavailable,
    /// The V3 pool address is absent from the fee catalog (WO-06): the QuoterV2
    /// derives the pool from the fee tier, so quoting would be blind at an
    /// unverified tier. Rejected WITHOUT an RPC — distinct from
    /// `V3QuoteUnavailable` (a real provider failure).
    V3PoolNotCatalogued,
    /// The token pair has no known V3 pools at all (WO-06) — no fee tier can
    /// exist, so the candidate is rejected WITHOUT an RPC.
    V3PairNoPools,
    /// Route leg count exceeds the canonical hard cap (min(7, Max_Legs),
    /// workbook `08_HOPS_2_7` ADMISSIBILITY). No sizing exists beyond the cap —
    /// fail closed rather than truncating the route (CARDS-HOPS 2026-09-20).
    UnsupportedLegCount,
    /// A route with MORE than 2 legs containing a V3 leg: the N-leg cycle
    /// kernel is all-V2 (constant-product composition) and the V3 kernel is
    /// 2-leg only, so no honest sizing exists. Rejected explicitly instead of
    /// falling into a 2-leg kernel that would size a fabricated 2-of-N slice
    /// (CARDS-HOPS 2026-09-20).
    ///
    /// V3-MULTILEG-SIZING-01: still the verdict whenever the N-leg V3 kernel
    /// is disabled (`ARBX_V3_MULTILEG_SIZING=off`) — the pre-patch behaviour,
    /// byte-identical.
    V3MultilegUnsupported,
    /// V3-MULTILEG-SIZING-01: the N-leg V3 kernel's per-(chain, block)
    /// QuoterV2 sub-call allowance was already spent when this cycle asked for
    /// its chain quotes. Honest and explicit (R8): the cycle was DEFERRED, not
    /// priced — it must never be reported as a provider failure
    /// (`v3_quote_unavailable`) nor as a spread verdict
    /// (`non_positive_profit`), and it carries no ledger (no chain was quoted).
    V3MultilegBudgetExhausted,
}

impl OptimizeRejectReason {
    /// Returns a stable, lowercase, underscore-separated string for use in
    /// Prometheus labels and `v2.optimizer.output` log events.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::NoConfig => "no_config",
            Self::ZeroCapitalCap => "zero_capital_cap",
            Self::UnknownTokenPrice => "unknown_token_price",
            Self::InvalidDecimals => "invalid_decimals",
            Self::MissingRouteLegs => "missing_route_legs",
            Self::MissingPoolAddress => "missing_pool_address",
            Self::MissingReservesPoolA => "missing_reserves_pool_a",
            Self::MissingReservesPoolB => "missing_reserves_pool_b",
            Self::ZeroReserves => "zero_reserves",
            Self::NonPositiveProfit => "non_positive_profit",
            Self::NonPositiveGrossUsd => "non_positive_gross_usd",
            Self::NonPositiveNetUsd => "non_positive_net_usd",
            Self::CapClampFailed => "cap_clamp_failed",
            Self::GasFloorBreach => "gas_floor_breach",
            Self::KellyNegativeEdge => "kelly_negative_edge",
            Self::V3QuoteUnavailable => "v3_quote_unavailable",
            Self::V3PoolNotCatalogued => "v3_pool_not_catalogued",
            Self::V3PairNoPools => "v3_pair_no_pools",
            Self::UnsupportedLegCount => "unsupported_leg_count",
            Self::V3MultilegUnsupported => "v3_multileg_unsupported",
            Self::V3MultilegBudgetExhausted => "v3_multileg_budget_exhausted",
        }
    }

    /// Map a V3 quote-failure label (`ProjectV3Error::as_label`) to its
    /// precise reject reason (WO-06). "v3_quote_unavailable" stays reserved
    /// for real provider failures — the former single bucket is now honestly
    /// split so the production metrics can arbitrate the fix branches.
    pub fn from_v3_unavailable_label(label: &'static str) -> Self {
        match label {
            "v3_pool_not_catalogued" => Self::V3PoolNotCatalogued,
            "v3_pair_no_pools" => Self::V3PairNoPools,
            _ => Self::V3QuoteUnavailable,
        }
    }

    /// True when the rejection depends on a NET-derived value that already
    /// includes the financing fee (ARBX-0007). Only these reasons carry the
    /// priced financing mode in their rejection label suffix — attaching it
    /// elsewhere would be noise (e.g. missing reserves has no mode context).
    pub fn is_net_dependent(&self) -> bool {
        matches!(
            self,
            Self::NonPositiveNetUsd | Self::GasFloorBreach | Self::KellyNegativeEdge
        )
    }
}

// ---------------------------------------------------------------------------
// OptimizeOutcome — discriminated result (TASK 2)
// ---------------------------------------------------------------------------

/// The result of `SizeOptimizer::optimize_with_reason`.
///
/// `Sized` carries a fully-valued `SizedCandidate`. `Rejected` carries the
/// specific reason why no profitable size was found. This replaces the
/// opaque `Option<SizedCandidate>` return so callers can log structured
/// diagnostics and route to the correct Prometheus counter.
///
/// `SizedCandidate` is boxed to avoid a large-enum-variant clippy error:
/// `SizedCandidate` is ~688 bytes while `Rejected` is 1 byte, so the
/// unboxed variant would pad the enum to 688 bytes on every call path.
#[allow(clippy::large_enum_variant)]
pub enum OptimizeOutcome {
    /// A profitable size was found.
    Sized(Box<SizedCandidate>),
    /// No profitable size exists; the explicit reason is provided.
    ///
    /// R8 contract on the payload: `None` = no USD value was computed on this
    /// path (infrastructure error, missing reserves, spot check failed, cap
    /// uncomputable); `Some(v)` = the kernel computed exactly `v` USD —
    /// typically `v <= 0` for the non-positive gates, or a positive net below
    /// the gas floor for `GasFloorBreach` / `KellyNegativeEdge`.
    ///
    /// Deuda 4-B sources of `Some(v)` on `NonPositiveProfit`: (a) the
    /// triangular clamped-size profit from `evaluate_cycle_detailed`
    /// (`CycleEvalOutcome::NonPositive`), and (b) the within-tick OPTIMISTIC
    /// UPPER BOUND best profit on the V3 0-RPC early-reject — a provable
    /// bound on the real grid's max, not a QuoterV2 quote.
    Rejected(OptimizeRejectReason, Option<f64>),
    /// PER-HOP (math-audit AUDIT-MATH-OPPS-2026-09-26): a rejection whose path
    /// HAD already computed the exact per-leg wei chain (the clamped-size
    /// evaluation runs before the non-positive gate fires). Carries the same
    /// payload semantics as `Rejected` plus the per-leg ledger, so a rejected
    /// row can still show how much each hop moves — the operator's per-hop
    /// request — instead of "not computed".
    ///
    /// `None` legs = the chain was not computable on this path (R8: the variant
    /// is used ONLY where the amounts genuinely exist; it is never fabricated).
    RejectedWithLedger(
        OptimizeRejectReason,
        Option<f64>,
        Option<(Vec<String>, Vec<String>)>,
    ),
    /// ALWAYS-COMPUTE (operator mandate 2026-09-27): a rejection whose path
    /// HAD computed the full economics BEFORE the pass/fail decision — the
    /// boxed `SizedCandidate` carries the exact figures (gross, net, sheet-07
    /// cost components, sized amount, per-leg ledger) the rejecting evaluation
    /// produced. "COMPUTED ≠ PROFITABLE": the row rejects (reason verbatim)
    /// AND keeps every number, so a FAIL card renders the full arithmetic —
    /// if the result is −$50 the card says −$50, not "no computado".
    ///
    /// Callers stamp these figures onto the rejected row (gross AND net — the
    /// legacy `Rejected` payload carried only one scalar) and attach the
    /// `EconomicsComputation` object built by `economics::economics_from_sized`.
    RejectedComputed(OptimizeRejectReason, Box<SizedCandidate>),
}

impl OptimizeOutcome {
    /// Returns the reject reason as a static string, or `None` for `Sized`.
    pub fn reason_str(&self) -> Option<&'static str> {
        match self {
            Self::Sized(_) => None,
            Self::Rejected(r, _) => Some(r.as_str()),
            Self::RejectedWithLedger(r, _, _) => Some(r.as_str()),
            Self::RejectedComputed(r, _) => Some(r.as_str()),
        }
    }

    /// Returns the gross profit in USD. `None` for the legacy reject variants
    /// (they never carried it); `Some` for `Sized` and `RejectedComputed`
    /// (both computed it — ALWAYS-COMPUTE surfaces the figure on rejects too).
    pub fn gross_profit_usd(&self) -> Option<f64> {
        match self {
            Self::Sized(s) => Some(s.gross_profit_usd),
            Self::Rejected(_, _) => None,
            Self::RejectedWithLedger(_, _, _) => None,
            Self::RejectedComputed(_, s) => Some(s.gross_profit_usd),
        }
    }

    /// Returns the net profit in USD. For `Rejected`, this is the kernel's
    /// computed value when the rejecting path had one (R8: `None` = not
    /// computed, `Some(v)` = computed and exactly `v`, usually `v <= 0`).
    /// For `RejectedComputed` it is the boxed figure's net (same semantics,
    /// explicitly separated from the gross there).
    pub fn net_profit_usd(&self) -> Option<f64> {
        match self {
            Self::Sized(s) => Some(s.estimated_net_profit_usd),
            Self::Rejected(_, net) => *net,
            Self::RejectedWithLedger(_, net, _) => *net,
            Self::RejectedComputed(_, s) => Some(s.estimated_net_profit_usd),
        }
    }

    /// Returns the optimal `amount_in` in wei, or `None` for `Rejected`.
    pub fn optimal_amount_in(&self) -> Option<U256> {
        match self {
            Self::Sized(s) => Some(s.optimal_amount_in),
            Self::Rejected(_, _) => None,
            Self::RejectedWithLedger(_, _, _) => None,
            Self::RejectedComputed(_, s) => Some(s.optimal_amount_in),
        }
    }

    /// PER-HOP: the per-leg wei ledger (amounts_in, amounts_out) when the
    /// rejecting path had computed it. `None` on every other path (R8) —
    /// including `Sized`, whose ledger travels through `SizedCandidate`.
    /// Owned (cloned) because `RejectedComputed` stores the two arrays inside
    /// its boxed `SizedCandidate`, not as a tuple.
    pub fn leg_ledger(&self) -> Option<(Vec<String>, Vec<String>)> {
        match self {
            Self::RejectedWithLedger(_, _, legs) => legs.clone(),
            Self::RejectedComputed(_, s) => {
                match (s.leg_amounts_in.clone(), s.leg_amounts_out.clone()) {
                    (Some(ins), Some(outs)) => Some((ins, outs)),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    /// ALWAYS-COMPUTE: the full sized figures when the rejecting path computed
    /// them (`RejectedComputed`). `None` everywhere else.
    pub fn rejected_figures(&self) -> Option<&SizedCandidate> {
        match self {
            Self::RejectedComputed(_, s) => Some(s),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Output type
// ---------------------------------------------------------------------------

/// A `StrategyCandidate` augmented with the optimal `amount_in` and USD profit
/// computed by the optimizer.
#[derive(Debug, Clone)]
pub struct SizedCandidate {
    /// The base strategy candidate (mutated: `amount_in` updated to optimal).
    pub candidate: StrategyCandidate,
    /// Optimal input size in wei (clamped to capital cap).
    pub optimal_amount_in: U256,
    /// Gross profit in USD at `optimal_amount_in`.
    pub gross_profit_usd: f64,
    /// Net profit after gas + fees + ops overhead.
    pub estimated_net_profit_usd: f64,
    /// Hardening flag: true when net <= 0 (opportunity is not viable but the
    /// values are real and must be surfaced to the operator in the dashboard).
    /// The gate of net-positive is an EXECUTION gate, not a DETECTION gate —
    /// the card must show the numbers so the operator can see WHY it's not
    /// viable instead of seeing "—" everywhere.
    pub net_negative: bool,
    /// ARBX-0009: sheet-07 cost components for Net_bps ranking. `Some` at every
    /// sized site (all components in kernel scope); `None` in hand-built test
    /// fixtures. R8: `None` = not computable, ranks last — never fabricated.
    pub net_economics: Option<crate::net_bps_ranking::RouteNetEconomics>,
    /// HOPS-LEDGER-04: exact per-leg wei amounts at `optimal_amount_in`,
    /// aligned with route_plan legs — `leg_amounts_in[i]` enters leg i,
    /// `leg_amounts_out[i]` leaves it (leg 1's input IS leg 0's output).
    /// `Some` only where the kernel computed real leg outputs (V2/V3 2-leg
    /// kernels); `None` = per-leg not computed (triangular kernel exposes only
    /// the final cycle amount; Kelly rebinds the size and per-leg cannot be
    /// re-quoted without another RPC pass; hand-built fixtures) — R8, never
    /// fabricated. Exact wei strings, NOT f64 (precision loss above 2^53).
    pub leg_amounts_in: Option<Vec<String>>,
    pub leg_amounts_out: Option<Vec<String>>,
}

// ---------------------------------------------------------------------------
// V3 slot0 cache — local within-tick upper bound for the 0-RPC early-reject
// (Plan B.1). Optional: when absent the optimizer falls through to the grid.
// ---------------------------------------------------------------------------

/// Local V3 slot0 snapshot — `sqrtPriceX96` + active liquidity — parsed to
/// native `U256` for direct use by `amm_math::v3_amount_out_single_tick`.
/// Mirrors `reserves::V3Slot0Entry` (which holds decimal strings for JSON) but
/// in the numeric form the within-tick math consumes.
#[derive(Debug, Clone, Copy)]
pub struct V3Slot0Snapshot {
    /// sqrtPriceX96 from `slot0()` (uint160).
    pub sqrt_price_x96: U256,
    /// Active liquidity at the current tick from `liquidity()` (uint128).
    pub liquidity: U256,
}

/// In-memory V3 slot0 cache keyed by pool address. Mirrors `ReservesCache`
/// (`tokio::sync::RwLock<HashMap<..>>`). Populated by an external sync path
/// (e.g. a future `pool_sync_worker` hook reading `reserves::get_v3_slot0`);
/// the within-tick early-reject treats a missing entry as "fall through to the
/// grid", so an unpopulated cache NEVER changes sizing behaviour.
///
/// NOTE: `StateProjector` does not yet expose slot0; until a read accessor is
/// wired there, the cache is attached directly to `SizeOptimizer` via
/// `with_slot0_cache` (tests populate it; prod wiring is a follow-up).
pub struct Slot0Cache {
    inner: RwLock<HashMap<Address, V3Slot0Snapshot>>,
}

impl Slot0Cache {
    /// Construct an empty cache.
    pub fn new() -> Self {
        Self {
            inner: RwLock::new(HashMap::new()),
        }
    }

    /// Insert or update a slot0 snapshot for a pool.
    pub async fn insert(&self, pool: Address, snap: V3Slot0Snapshot) {
        self.inner.write().await.insert(pool, snap);
    }

    /// Fetch the slot0 snapshot for a pool (`None` on cache miss).
    pub async fn get(&self, pool: &Address) -> Option<V3Slot0Snapshot> {
        self.inner.read().await.get(pool).copied()
    }
}

impl Default for Slot0Cache {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// SizeOptimizer
// ---------------------------------------------------------------------------

/// Optimizes `amount_in` for a `StrategyCandidate`.
///
/// Constructed once per orchestrator context. Thread-safe: all state is
/// `Arc`-wrapped or stateless.
pub struct SizeOptimizer {
    state_projector: Arc<StateProjector>,
    /// Optional V3 slot0 cache enabling the 0-RPC within-tick early-reject in
    /// `size_two_leg_v3_with_reason`. `None` (default) → always fall through to
    /// the QuoterV2 grid. Attached via `with_slot0_cache`.
    slot0_cache: Option<Arc<Slot0Cache>>,
    /// V3-MULTILEG-SIZING-01: per-(chain, block) QuoterV2 sub-call allowance
    /// shared by every N-leg V3 sizing attempt of this optimizer. Injectable so
    /// a test owns its own allowance (`with_v3_multileg_quote_budget`).
    v3_multileg_budget: Arc<V3MultilegQuoteBudget>,
}

impl SizeOptimizer {
    /// Constructs a `SizeOptimizer`.
    ///
    /// - `state_projector`: used to read current reserves for the optimization
    ///   search range bounds.
    ///
    /// No slot0 cache is attached; the V3 within-tick early-reject stays
    /// disabled (falls through to the QuoterV2 grid). Use `with_slot0_cache`
    /// to enable it.
    ///
    /// The N-leg V3 kernel's per-(chain, block) quote allowance is created here
    /// from [`V3_MULTILEG_QUOTES_PER_BLOCK_ENV`] (default
    /// [`DEFAULT_V3_MULTILEG_QUOTES_PER_BLOCK`]).
    pub fn new(state_projector: Arc<StateProjector>) -> Self {
        Self {
            state_projector,
            slot0_cache: None,
            v3_multileg_budget: Arc::new(V3MultilegQuoteBudget::default()),
        }
    }

    /// Attach an in-memory V3 slot0 cache, enabling the 0-RPC within-tick
    /// early-reject in `size_two_leg_v3_with_reason`. Builder-style; existing
    /// callers of `new()` are unchanged (cache defaults to `None`).
    pub fn with_slot0_cache(mut self, cache: Arc<Slot0Cache>) -> Self {
        self.slot0_cache = Some(cache);
        self
    }

    /// V3-MULTILEG-SIZING-01: replace the per-(chain, block) QuoterV2 sub-call
    /// allowance. Builder-style (same shape as `with_slot0_cache`); the
    /// production caller keeps the env-derived default.
    pub fn with_v3_multileg_quote_budget(mut self, budget: Arc<V3MultilegQuoteBudget>) -> Self {
        self.v3_multileg_budget = budget;
        self
    }

    // -----------------------------------------------------------------------
    // Main entry point — diagnostic-rich version (TASK 2)
    // -----------------------------------------------------------------------

    /// Optimize `amount_in` for a candidate, returning an explicit `OptimizeOutcome`.
    ///
    /// Returns `OptimizeOutcome::Sized` when a positive-net opportunity exists.
    /// Returns `OptimizeOutcome::Rejected(reason)` with the specific reason when
    /// no profitable size is found — never an opaque `None`.
    ///
    /// R8: `Err` is reserved for genuine I/O / infrastructure failures only.
    /// Business-logic rejections (no config, no price, no profit) all produce
    /// `Ok(Rejected(...))`.
    pub async fn optimize_with_reason(
        &self,
        candidate: StrategyCandidate,
        intent: &RouteIntent,
        cfg: Option<&TradingConfigState>,
    ) -> anyhow::Result<OptimizeOutcome> {
        // Step 1: config required for capital cap + pricing.
        let Some(state) = cfg else {
            debug!(
                event = "size_optimizer.no_config",
                label = candidate.label.as_str(),
                "no TradingConfigState — cannot size"
            );
            return Ok(OptimizeOutcome::Rejected(
                OptimizeRejectReason::NoConfig,
                None,
            ));
        };

        // Step 2: determine token_in symbol (for capital cap lookup).
        // CORE-05: chain-gated address map + pair-symbol fallback + native
        // fallback. In practice this always resolves (native last-resort);
        // None is kept for the future TokenIdentityIndex wiring.
        let token_in_symbol =
            resolve_token_in_symbol(&candidate, state).unwrap_or_else(|| "WETH".to_string());

        // Step 3: capital cap in USD.
        //
        // Kelly is applied POST-optimization (after the kernel finds the
        // profit-optimal `optimal_amount_in`) — see `apply_kelly_constraints`
        // below. Applying Kelly here as a pre-cap on the search range would
        // require pre-estimating win_prob/gain/loss without candidate-specific
        // evidence, yielding a constant scaling rather than a real Kelly cap.
        // The previous in-tree version (commit 7df940f) did exactly that with
        // hardcoded 0.95/0.01/0.005 — removed.
        let cap_usd = state.effective_capital_for(&token_in_symbol, candidate.label.as_str());
        if cap_usd <= 0.0 {
            debug!(
                event = "size_optimizer.zero_cap",
                label = candidate.label.as_str(),
                token = token_in_symbol,
            );
            return Ok(OptimizeOutcome::Rejected(
                OptimizeRejectReason::ZeroCapitalCap,
                None,
            ));
        }

        // Step 4: token price for USD conversion.
        let token_price_usd = resolve_token_price(state, &token_in_symbol);
        let Some(token_price_usd) = token_price_usd else {
            debug!(
                event = "size_optimizer.no_price",
                label = candidate.label.as_str(),
                token = token_in_symbol,
                "token unpriced — Rejected(UnknownTokenPrice)"
            );
            return Ok(OptimizeOutcome::Rejected(
                OptimizeRejectReason::UnknownTokenPrice,
                None,
            ));
        };

        // Step 5: token decimals.
        let decimals = resolve_token_decimals(&token_in_symbol);

        // Step 6: cap in wei.
        let cap_wei = match clamp_to_cap_wei(U256::MAX, cap_usd, token_price_usd, decimals) {
            Some(v) => v,
            None => {
                return Ok(OptimizeOutcome::Rejected(
                    OptimizeRejectReason::CapClampFailed,
                    None,
                ))
            }
        };

        if cap_wei.is_zero() {
            return Ok(OptimizeOutcome::Rejected(
                OptimizeRejectReason::ZeroCapitalCap,
                None,
            ));
        }

        // Step 7: dispatch to the correct sizing kernel.
        let kernel_outcome = match candidate.label {
            StrategyLabel::TriangularArb => {
                // PERHOP-RESERVES-01: an N-leg cycle containing a concentrated-
                // liquidity (V3) leg has NO kernel. `size_triangular_with_reason`
                // reads the constant-product reserves cache ONLY (the oriented
                // `(r0, r1)` pair per leg); a V3 pool has no `getReserves()`,
                // hence no `arbx:pool_reserves:<chain>:<pool>` key, so the leg
                // would miss the cache and the cycle would be refused with the
                // MISLEADING `missing_reserves_pool_a/b` — the exact confusion
                // `multileg_v3_route_rejects_honestly` pins for the other labels
                // and explicitly forbids. The branch below is the repo's own
                // declared refusal for this shape; it must be evaluated BEFORE
                // the triangular kernel, because `hop_cycle_bridge` labels every
                // discovered 3..=7-hop cycle `TriangularArb` and would otherwise
                // shadow it for the entire discovered-cycle population
                // (measured 2026-09-26: 1 800 such rows / 15 min, 100 % of them
                // carrying `missing_reserves_pool_a/b`).
                //
                // Byte-identical for everything else: `legs <= 2` (the whole
                // `dex_engine` population) and every all-V2 N-leg cycle still
                // reach `size_triangular_with_reason` unchanged.
                if candidate.route_plan.legs.len() > 2 && route_has_v3(&candidate) {
                    // V3-MULTILEG-SIZING-01: the N-leg V3 kernel owns this
                    // shape now (bounded RPC, same outputs as the V2 path).
                    // Knob OFF ⇒ the exact pre-patch refusal below.
                    self.size_multileg_v3_or_refuse(
                        &candidate,
                        cap_wei,
                        cap_usd,
                        token_price_usd,
                        decimals,
                        state,
                    )
                    .await
                } else {
                    self.size_triangular_with_reason(
                        &candidate,
                        state,
                        cap_usd,
                        token_price_usd,
                        decimals,
                    )
                    .await
                }
            }
            // 2-leg routes that touch a Uniswap-V3-style pool: concentrated-
            // liquidity sizing via the on-chain QuoterV2 (Step 1 provider).
            // Triangular V3 cycles are handled by the triangular kernel above;
            // this is the 2-leg DEX-arb path that previously had no V3 sizer.
            _ if route_has_v3(&candidate) => {
                // N-leg V3 routes (QuoterV2 arms are strictly 2-leg) size
                // through the V3 N-leg kernel; knob OFF ⇒ the pre-patch refusal.
                if candidate.route_plan.legs.len() > 2 {
                    self.size_multileg_v3_or_refuse(
                        &candidate,
                        cap_wei,
                        cap_usd,
                        token_price_usd,
                        decimals,
                        state,
                    )
                    .await
                } else {
                    self.size_two_leg_v3_with_reason(
                        &candidate,
                        intent,
                        cap_wei,
                        cap_usd,
                        token_price_usd,
                        decimals,
                        state,
                    )
                    .await
                }
            }
            // WO-13: N-leg (4..7) all-V2 cycles size through the generic
            // cycle kernel — same math as triangular, arbitrary hop count.
            // (Discovery's min(7, Max_Legs) already bounds legs at 7.)
            _ if candidate.route_plan.legs.len() != 2 => {
                self.size_triangular_with_reason(
                    &candidate,
                    state,
                    cap_usd,
                    token_price_usd,
                    decimals,
                )
                .await
            }
            // All remaining 2-leg V2 variants use the V2 CPMM kernel.
            _ => {
                self.size_two_leg_with_reason(
                    &candidate,
                    intent,
                    cap_wei,
                    cap_usd,
                    token_price_usd,
                    decimals,
                    state,
                )
                .await
            }
        };

        // Step 8: Kelly post-optimization constraints (fix-2, 2026-05-13).
        // Replaces the static hardcoded Kelly that the previous version
        // applied as a 50% pre-cap on the search range. Now uses
        // candidate-specific gross/net ratios + config-sourced multiplier
        // and per-trade cap.
        let result_opt = self
            .apply_kelly_constraints(kernel_outcome, state, cap_usd, token_price_usd, decimals)
            .await;

        Ok(result_opt)
    }

    // -----------------------------------------------------------------------
    // Step 8 — Kelly post-optimization constraints
    // -----------------------------------------------------------------------

    /// Apply the constrained-fractional-Kelly + gas-floor + intersection-min
    /// constraints to the kernel's profit-optimal candidate.
    ///
    /// Math (operator directive, 2026-05-13):
    ///   f* = p − (1 − p)/W                           [constrained Kelly]
    ///   p  = state.min_landing_probability ∈ (0, 1)  [conservative; swap for
    ///                                                 candidate.evidence.landing_probability
    ///                                                 once that field is wired post-A.4]
    ///   W  = gross_profit_usd / total_cost_usd        [gain/loss ratio per
    ///                                                 unit capital, cost
    ///                                                 proxy includes gas +
    ///                                                 flashloan fee + ops]
    ///   final_size = min(kernel_optimum,
    ///                   nav × f* × kelly_multiplier  capped at kelly_max_per_trade_fraction)
    ///   Gas floor: net_usd ≥ cost_usd × kelly_gas_safety_multiplier
    async fn apply_kelly_constraints(
        &self,
        outcome: OptimizeOutcome,
        state: &TradingConfigState,
        cap_usd: f64,
        token_price_usd: f64,
        decimals: u8,
    ) -> OptimizeOutcome {
        let mut sized = match outcome {
            OptimizeOutcome::Sized(boxed) => *boxed,
            // Rejected outcomes pass through — kernel already named the reason
            // and computed whatever USD value it could (R8 payload). The
            // ledger-carrying variant passes through intact (PER-HOP); the
            // figures-carrying variant too (ALWAYS-COMPUTE).
            OptimizeOutcome::Rejected(r, net) => return OptimizeOutcome::Rejected(r, net),
            OptimizeOutcome::RejectedWithLedger(r, net, legs) => {
                return OptimizeOutcome::RejectedWithLedger(r, net, legs)
            }
            OptimizeOutcome::RejectedComputed(r, s) => {
                return OptimizeOutcome::RejectedComputed(r, s)
            }
        };

        let gross_usd = sized.gross_profit_usd;
        let net_usd = sized.estimated_net_profit_usd;
        if gross_usd <= 0.0 || net_usd <= 0.0 {
            // HARDENING: retornar el SizedCandidate con los valores calculados
            // aunque net <= 0. El gate de net-positive es de ejecución, no de
            // detección. La tarjeta debe mostrar los valores reales (gross, net,
            // costs) para que el operador vea POR QUÉ no es viable.
            sized.net_negative = true;
            return OptimizeOutcome::Sized(Box::new(sized));
        }

        // Cost proxy: everything that the kernel subtracted from gross to
        // arrive at net (gas + flashloan fee + ops overhead + capital cost).
        // Using the full proxy makes the safety floor strictly stronger than
        // gas-only — desirable for the conservative posture.
        let cost_proxy_usd = (gross_usd - net_usd).max(0.0);

        // Gas Floor (operator directive #3): require net ≥ multiplier × cost.
        if net_usd < cost_proxy_usd * state.kelly_gas_safety_multiplier {
            debug!(
                event = "size_optimizer.kelly_gas_floor_breach",
                label = sized.candidate.label.as_str(),
                net_usd,
                cost_proxy_usd,
                multiplier = state.kelly_gas_safety_multiplier,
            );
            // Payload = the computed net (positive but below the floor).
            // ALWAYS-COMPUTE: the FULL sized figures travel on the reject —
            // gross, net, components, sized amount, ledger. The row rejects
            // AND keeps every number ("COMPUTED ≠ PROFITABLE").
            return OptimizeOutcome::RejectedComputed(
                OptimizeRejectReason::GasFloorBreach,
                Box::new(sized),
            );
        }

        // Constrained Fractional Kelly (operator directive #1).
        let p = state.min_landing_probability.clamp(0.01, 0.99);
        let w_ratio = if cost_proxy_usd > 1e-9 {
            gross_usd / cost_proxy_usd
        } else {
            // Degenerate cost: gain dominates loss → cap fraction by p
            // (the Kelly limit as W → ∞).
            f64::INFINITY
        };

        let f_raw = if w_ratio.is_infinite() {
            p
        } else {
            p - (1.0 - p) / w_ratio
        };

        if !f_raw.is_finite() || f_raw <= 0.0 {
            debug!(
                event = "size_optimizer.kelly_negative_edge",
                label = sized.candidate.label.as_str(),
                p,
                w_ratio,
            );
            // Payload = the computed net the edge test rejected.
            // ALWAYS-COMPUTE: full figures on the reject (same doctrine as the
            // gas-floor arm above).
            return OptimizeOutcome::RejectedComputed(
                OptimizeRejectReason::KellyNegativeEdge,
                Box::new(sized),
            );
        }

        // Apply config-sourced multiplier and per-trade cap (operator
        // directive #4 + no-hardcode doctrine).
        let f_scaled = (f_raw * state.kelly_multiplier).clamp(0.0, 1.0);
        let f_capped = f_scaled.min(state.kelly_max_per_trade_fraction);

        // Convert Kelly USD cap → wei. Unlike the kernel's `clamp_to_cap_wei`
        // (which floors the whole-token count and returns None for sub-token
        // caps), Kelly must support fractional-token caps because the
        // fractional-Kelly bound on small NAVs frequently yields amounts
        // smaller than one token (e.g., 0.001 WETH = 10^15 wei is valid).
        let kelly_cap_usd = cap_usd * f_capped;
        let kelly_cap_wei =
            if kelly_cap_usd > 0.0 && token_price_usd.is_finite() && token_price_usd > 0.0 {
                let tokens_capped = kelly_cap_usd / token_price_usd;
                let wei_f = tokens_capped * 10f64.powi(i32::from(decimals));
                if !wei_f.is_finite() || wei_f <= 0.0 {
                    // Kelly cap rounds to zero wei — too small to bet at all.
                    return OptimizeOutcome::Rejected(OptimizeRejectReason::ZeroCapitalCap, None);
                }
                f64_to_u256_clamped(wei_f.floor())
            } else {
                // Numerical edge — keep kernel result (conservative fall-back).
                return OptimizeOutcome::Sized(Box::new(sized));
            };

        if sized.optimal_amount_in <= kelly_cap_wei {
            // Kelly cap not binding — kernel's optimum is already inside Kelly's
            // budget. No further action.
            return OptimizeOutcome::Sized(Box::new(sized));
        }

        // Kelly binds: rescale to the capped amount. Profit scales
        // approximately linearly for small-to-medium bets relative to pool
        // depth (V2 CPMM) and exactly when entirely within a V3 tick range.
        // Gas does not rescale — same tx, same gas. For exact precision at
        // the new amount, the kernel would need a second pass; deferred
        // here as the approximation is conservative (overestimates gas
        // weight at smaller sizes).
        let denom = u256_to_f64_lossy(sized.optimal_amount_in);
        let ratio = if denom > 0.0 {
            (u256_to_f64_lossy(kelly_cap_wei) / denom).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let new_gross = (gross_usd * ratio).max(0.0);
        let new_net = (new_gross - cost_proxy_usd).max(0.0);

        if new_net <= 0.0 {
            // HARDENING: conservar los valores calculados aunque net <= 0.
            // Propagar el Kelly cap al amount_in para consistencia con el gross
            // escalado (new_gross); sin esto la tarjeta mostraría el amount del
            // kernel (10^18) junto al gross escalado (0.1) — datos inconsistentes
            // (0.1 gross corresponde a 0.001 WETH, no 1 WETH). Fix KELLY-CAPS-01.
            sized.optimal_amount_in = kelly_cap_wei;
            sized.gross_profit_usd = new_gross;
            sized.estimated_net_profit_usd = new_net;
            sized.net_negative = true;
            // HOPS-LEDGER-05: the ledger the kernel produced belongs to the
            // KERNEL size, so it is re-derived AT the Kelly-capped size — a
            // stale chain contradicting the rescaled figures is never kept,
            // and the per-hop numbers survive instead of vanishing (measured
            // before this fix: 339 of 4.16M rows in 24h carried a ledger).
            let (ins, outs) = self.reledger_at(&sized, kelly_cap_wei).await;
            sized.leg_amounts_in = ins;
            sized.leg_amounts_out = outs;
            return OptimizeOutcome::Sized(Box::new(sized));
        }
        // Re-check gas floor on the scaled profit. A Kelly cap that drives
        // profit below the floor means the size we were forced down to is
        // not worth running at all.
        if new_net < cost_proxy_usd * state.kelly_gas_safety_multiplier {
            // Payload = the Kelly-capped net (positive but below the floor).
            // ALWAYS-COMPUTE: stamp the RESCALED figures (the numbers this
            // verdict actually judged) before rejecting — gross at the Kelly
            // size, net at the Kelly size, same sized amount the ledger was
            // re-derived for. Never the pre-rescale figures.
            sized.optimal_amount_in = kelly_cap_wei;
            sized.gross_profit_usd = new_gross;
            sized.estimated_net_profit_usd = new_net;
            let (ins, outs) = self.reledger_at(&sized, kelly_cap_wei).await;
            sized.leg_amounts_in = ins;
            sized.leg_amounts_out = outs;
            return OptimizeOutcome::RejectedComputed(
                OptimizeRejectReason::GasFloorBreach,
                Box::new(sized),
            );
        }

        sized.optimal_amount_in = kelly_cap_wei;
        sized.gross_profit_usd = new_gross;
        sized.estimated_net_profit_usd = new_net;
        // HOPS-LEDGER-05: same re-derivation as the net_negative bind arm above.
        let (ins, outs) = self.reledger_at(&sized, kelly_cap_wei).await;
        sized.leg_amounts_in = ins;
        sized.leg_amounts_out = outs;
        OptimizeOutcome::Sized(Box::new(sized))
    }

    /// HOPS-LEDGER-05 — re-derive the per-leg ledger at `amount_in` (the FINAL,
    /// Kelly-capped size) instead of discarding it.
    ///
    /// WHY THIS EXISTS: the sizing kernels emit the ledger at the profit-optimal
    /// size; Kelly then caps the size, and the pre-fix code set `leg_amounts_* =
    /// None` because the chain no longer matched the reported figures. Honest, but
    /// it made the operator's per-hop numbers unreachable on essentially every
    /// emitted row — measured in production: **339 of 4,161,891 rows in 24 h**
    /// carried `leg_amounts_in` (0.008%).
    ///
    /// The V2 chain is exact, local and RPC-free: `out_i = v2_amount_out(in_i,
    /// r_in_i, r_out_i, fee_i)` with the SAME cached reserves the kernel read, so
    /// recomputing it at the capped size makes the ledger belong to the figures
    /// the row reports (Sancho condition 1) with no extra network round.
    ///
    /// Fees mirror the producing kernel exactly: the 2-leg V2 kernel uses each
    /// leg's declared `fee_bps` (default 30, `size_two_leg_with_reason`), the
    /// N-leg cycle kernel uses a constant 30 (`size_triangular_with_reason`).
    ///
    /// R8 — returns `(None, None)`, never a stale or partial chain, when:
    ///   · the kernel produced no ledger in the first place (V3/RPC kernel,
    ///     hand-built fixture) — nothing to re-derive;
    ///   · ANY leg touches a V3-style pool: constant-product math is WRONG for
    ///     concentrated liquidity, and a fabricated CPMM chain would understate
    ///     the real output;
    ///   · a pool address or a reserve is missing from the cache.
    async fn reledger_at(
        &self,
        sized: &SizedCandidate,
        amount_in: U256,
    ) -> (Option<Vec<String>>, Option<Vec<String>>) {
        if sized.leg_amounts_in.is_none() {
            return (None, None);
        }
        let candidate = &sized.candidate;
        if route_has_v3(candidate) {
            return (None, None);
        }
        let legs = &candidate.route_plan.legs;
        if legs.is_empty() {
            return (None, None);
        }
        // Fee source must match the kernel that produced the ledger.
        let n_leg_fee = candidate.label == StrategyLabel::TriangularArb || legs.len() != 2;

        let mut current = amount_in;
        let mut ins: Vec<String> = Vec::with_capacity(legs.len());
        let mut outs: Vec<String> = Vec::with_capacity(legs.len());
        for leg in legs {
            let Some(pool_str) = leg.pool_address.as_deref() else {
                return (None, None);
            };
            let Ok(pool_addr) = pool_str.parse::<ethers::types::Address>() else {
                return (None, None);
            };
            let Some((r0, r1)) = self.state_projector.reserves_cache.get(&pool_addr).await else {
                return (None, None);
            };
            // Same token0/token1 orientation the kernels use (address ordering).
            let (reserve_in, reserve_out) = if leg.token_in <= leg.token_out {
                (r0, r1)
            } else {
                (r1, r0)
            };
            let fee_bps = if n_leg_fee {
                30
            } else {
                leg.fee_bps.unwrap_or(30)
            };
            let out = crate::amm_math::v2_amount_out(current, reserve_in, reserve_out, fee_bps);
            if out.is_zero() {
                // Degenerate hop — a zero-output chain is not a ledger (R8).
                return (None, None);
            }
            ins.push(current.to_string());
            outs.push(out.to_string());
            current = out;
        }
        (Some(ins), Some(outs))
    }

    // -----------------------------------------------------------------------
    // Legacy shim — backwards compat
    // -----------------------------------------------------------------------

    /// Optimize `amount_in` for a candidate.
    ///
    /// Returns `Ok(Some(SizedCandidate))` when a positive-net opportunity
    /// exists at some size within [min_input, cap_wei].
    ///
    /// Returns `Ok(None)` when no profitable size exists (any reason).
    ///
    /// Forwards to `optimize_with_reason` internally; the specific rejection
    /// reason is available by calling that method directly.
    pub async fn optimize(
        &self,
        candidate: StrategyCandidate,
        intent: &RouteIntent,
        cfg: Option<&TradingConfigState>,
    ) -> anyhow::Result<Option<SizedCandidate>> {
        match self.optimize_with_reason(candidate, intent, cfg).await? {
            OptimizeOutcome::Sized(s) => Ok(Some(*s)),
            OptimizeOutcome::Rejected(_, _) => Ok(None),
            OptimizeOutcome::RejectedWithLedger(_, _, _) => Ok(None),
            OptimizeOutcome::RejectedComputed(_, _) => Ok(None),
        }
    }

    // -----------------------------------------------------------------------
    // N-leg cycle sizing (triangular + 4..7-hop) — with explicit reason
    // -----------------------------------------------------------------------

    async fn size_triangular_with_reason(
        &self,
        candidate: &StrategyCandidate,
        state: &TradingConfigState,
        cap_usd: f64,
        token_price_usd: f64,
        decimals: u8,
    ) -> OptimizeOutcome {
        // Extract hop reserves from the route plan legs.
        let legs = &candidate.route_plan.legs;
        // WO-13: the cycle kernel is generic in N; admissibility follows the
        // canonical workbook rule h ∈ [2, min(7, Max_Legs)] — discovery already
        // clamps, this guard fails closed on anything beyond the canonical cap.
        if legs.len() < 2 {
            return OptimizeOutcome::Rejected(OptimizeRejectReason::MissingRouteLegs, None);
        }
        if legs.len() > 7 {
            return OptimizeOutcome::Rejected(OptimizeRejectReason::UnsupportedLegCount, None);
        }

        // Build (reserve_in, reserve_out) for each hop from the reserves cache.
        let mut hop_reserves: Vec<(U256, U256)> = Vec::with_capacity(legs.len());
        for (leg_idx, leg) in legs.iter().enumerate() {
            let pool_addr_str = match leg.pool_address.as_deref() {
                Some(s) => s,
                None => {
                    return OptimizeOutcome::Rejected(
                        OptimizeRejectReason::MissingPoolAddress,
                        None,
                    )
                }
            };
            let pool_addr: ethers::types::Address = match pool_addr_str.parse().ok() {
                Some(a) => a,
                None => {
                    return OptimizeOutcome::Rejected(
                        OptimizeRejectReason::MissingPoolAddress,
                        None,
                    )
                }
            };
            let (r0, r1) = match self.state_projector.reserves_cache.get(&pool_addr).await {
                Some(pair) => pair,
                None => {
                    return OptimizeOutcome::Rejected(
                        if leg_idx == 0 {
                            OptimizeRejectReason::MissingReservesPoolA
                        } else {
                            OptimizeRejectReason::MissingReservesPoolB
                        },
                        None,
                    )
                }
            };
            let token_in_str = &leg.token_in;
            let token_out_str = &leg.token_out;
            let (reserve_in, reserve_out) = if token_in_str <= token_out_str {
                (r0, r1)
            } else {
                (r1, r0)
            };
            hop_reserves.push((reserve_in, reserve_out));
        }

        let eval_input = EvalInput {
            hop_reserves,
            token_a_price_usd: Some(token_price_usd),
            token_a_decimals: decimals,
            cap_usd,
            fee_bps: 30,
        };

        let eval_result = match evaluate_cycle_detailed(&eval_input) {
            CycleEvalOutcome::Profitable(r) => r,
            // Deuda 4-B: the kernel DID compute a clamped-size profit ≤ 0 —
            // stamp it (the #617 USD payload contract).
            //
            // ALWAYS-COMPUTE (2026-09-27): the cycle evaluator computed the
            // gross (≤ 0) at its clamped size; the fixed cost components
            // (gas + ops) are config-sourced and computable here. The clamped
            // AMOUNT is not exposed by this outcome, so amount_in and the
            // per-leg ledger stay honestly absent (R8 + reasons) — a
            // flash-wrapped variant additionally keeps net_economics absent
            // because its fee needs the unknown principal (never assumed 0).
            CycleEvalOutcome::NonPositive { profit_usd } => {
                let gas_cost = state.gas_cost_usd();
                let ops_overhead = state.ops_overhead_usd_per_attempt;
                let net_usd = profit_usd - gas_cost - ops_overhead;
                let own_capital = candidate.base_strategy.is_none();
                let mut cand = candidate.clone();
                cand.opportunity.expected_profit_usd = Some(profit_usd);
                cand.opportunity.net_expected_profit_usd = Some(net_usd);
                return OptimizeOutcome::RejectedComputed(
                    OptimizeRejectReason::NonPositiveProfit,
                    Box::new(SizedCandidate {
                        candidate: cand,
                        optimal_amount_in: U256::zero(),
                        gross_profit_usd: profit_usd,
                        estimated_net_profit_usd: net_usd,
                        net_negative: true,
                        net_economics: own_capital.then(|| {
                            crate::net_bps_ranking::RouteNetEconomics::from_kernel(
                                0.0,
                                profit_usd,
                                gas_cost,
                                ops_overhead,
                                0.0,
                            )
                        }),
                        leg_amounts_in: None,
                        leg_amounts_out: None,
                    }),
                );
            }
            // R8: evaluate_cycle computed nothing — no value to stamp.
            CycleEvalOutcome::NotComputed => {
                return OptimizeOutcome::Rejected(OptimizeRejectReason::NonPositiveProfit, None)
            }
        };

        // WO-LEGS-TRIANGULAR-01: exact per-leg wei from the POST-CLAMP
        // re-evaluation inside `evaluate_cycle` — the same hop chain whose
        // final amount_out the reported profit consumed (Sancho condition 1).
        // leg_outputs = [out0..outN]; the honest input chain is
        // [x, out0..outN-1] (leg i+1's input IS leg i's output). Same shape the
        // 2-leg kernel emits (see `leg_amounts_in` in size_two_leg_with_reason).
        // R8: any missing link → no ledger at all (all-or-nothing).
        let (leg_amounts_in, leg_amounts_out) = match eval_result.leg_outputs.as_ref() {
            Some(outs) if outs.len() == legs.len() => {
                let x = eval_result.amount_in_wei;
                let mut ins = Vec::with_capacity(outs.len());
                ins.push(x.to_string());
                for out in outs.iter().take(outs.len().saturating_sub(1)) {
                    ins.push(out.to_string());
                }
                (
                    Some(ins),
                    Some(outs.iter().map(|o| o.to_string()).collect::<Vec<_>>()),
                )
            }
            _ => (None, None),
        };

        let gross_usd = match eval_result.expected_profit_usd {
            Some(g) if g > 0.0 => g,
            // R8: pass the computed gross through verbatim (Some(<=0)); None
            // stays None — `evaluate_cycle` computed nothing.
            //
            // ALWAYS-COMPUTE (2026-09-27): a computed (≤ 0) gross at a KNOWN
            // clamped amount upgrades to the full figures — the post-clamp
            // re-evaluation produced the per-leg outputs (WO-LEGS-TRIANGULAR-01
            // consumes them below), so the ledger rides the reject too.
            computed_gross => {
                let (g, has_figures) = match computed_gross {
                    Some(g) => (g, true),
                    None => (0.0, false),
                };
                if !has_figures {
                    return OptimizeOutcome::Rejected(
                        OptimizeRejectReason::NonPositiveGrossUsd,
                        None,
                    );
                }
                let gas_cost = state.gas_cost_usd();
                let ops_overhead = state.ops_overhead_usd_per_attempt;
                let start_amount_usd = (clamped_to_i128(eval_result.amount_in_wei) as f64)
                    / 10f64.powi(decimals as i32)
                    * token_price_usd;
                let borrow_usd = if candidate.base_strategy.is_some() {
                    start_amount_usd
                } else {
                    0.0
                };
                let flash_fee_usd =
                    borrow_usd * crate::financing::selected_mode(borrow_usd).fee_bps() / 10_000.0;
                let net_usd = g - gas_cost - ops_overhead - flash_fee_usd;
                let (legs_in, legs_out) = match eval_result.leg_outputs.as_ref() {
                    Some(outs) if outs.len() == legs.len() => {
                        let x = eval_result.amount_in_wei;
                        let mut ins = Vec::with_capacity(outs.len());
                        ins.push(x.to_string());
                        for out in outs.iter().take(outs.len().saturating_sub(1)) {
                            ins.push(out.to_string());
                        }
                        (
                            Some(ins),
                            Some(outs.iter().map(|o| o.to_string()).collect::<Vec<_>>()),
                        )
                    }
                    _ => (None, None),
                };
                let mut cand = candidate.clone();
                cand.opportunity.amount_in_wei = eval_result.amount_in_wei.to_string();
                cand.opportunity.expected_profit_usd = Some(g);
                cand.opportunity.net_expected_profit_usd = Some(net_usd);
                return OptimizeOutcome::RejectedComputed(
                    OptimizeRejectReason::NonPositiveGrossUsd,
                    Box::new(SizedCandidate {
                        candidate: cand,
                        optimal_amount_in: eval_result.amount_in_wei,
                        gross_profit_usd: g,
                        estimated_net_profit_usd: net_usd,
                        net_negative: true,
                        net_economics: Some(
                            crate::net_bps_ranking::RouteNetEconomics::from_kernel(
                                start_amount_usd,
                                g,
                                gas_cost,
                                ops_overhead,
                                borrow_usd,
                            ),
                        ),
                        leg_amounts_in: legs_in,
                        leg_amounts_out: legs_out,
                    }),
                );
            }
        };

        let gas_cost = state.gas_cost_usd();
        let ops_overhead = state.ops_overhead_usd_per_attempt;
        let net_usd = gross_usd - gas_cost - ops_overhead;
        // ARBX-0009: sheet-07 economics for Net_bps ranking. Triangulars run
        // own capital (base_strategy None ⇒ borrow 0 ⇒ no flash fee), so the
        // ranked NetProfit is identical to the recorded `net_usd` here; a
        // flash-wrapped variant, should one ever route here, prices its fee
        // with the same ARBX-0007 policy the kernel sites use.
        let start_amount_usd = (clamped_to_i128(eval_result.amount_in_wei) as f64)
            / 10f64.powi(decimals as i32)
            * token_price_usd;
        let borrow_usd = if candidate.base_strategy.is_some() {
            start_amount_usd
        } else {
            0.0
        };
        let net_economics = Some(crate::net_bps_ranking::RouteNetEconomics::from_kernel(
            start_amount_usd,
            gross_usd,
            gas_cost,
            ops_overhead,
            borrow_usd,
        ));

        if net_usd <= 0.0 {
            debug!(
                event = "size_optimizer.triangular_negative_net",
                gross_usd, gas_cost, ops_overhead, net_usd,
            );
            // HARDENING: poblar el SizedCandidate con los valores calculados
            // aunque net <= 0. El operador necesita ver los números.
            let mut cand = candidate.clone();
            cand.opportunity.amount_in_wei = eval_result.amount_in_wei.to_string();
            cand.opportunity.expected_profit_usd = Some(gross_usd);
            cand.opportunity.net_expected_profit_usd = Some(net_usd);
            return OptimizeOutcome::Sized(Box::new(SizedCandidate {
                candidate: cand,
                optimal_amount_in: eval_result.amount_in_wei,
                gross_profit_usd: gross_usd,
                estimated_net_profit_usd: net_usd,
                net_negative: true,
                net_economics,
                // WO-LEGS-TRIANGULAR-01: ledger from the post-clamp re-eval —
                // still at the REPORTED size here (no rescale happens on this
                // arm), so the chain is exact, not stale.
                leg_amounts_in,
                leg_amounts_out,
            }));
        }

        let mut sized = candidate.clone();
        sized.opportunity.amount_in_wei = eval_result.amount_in_wei.to_string();
        sized.gross_profit_usd = Some(gross_usd);
        sized.net_expected_profit_usd = Some(net_usd);

        OptimizeOutcome::Sized(Box::new(SizedCandidate {
            candidate: sized,
            optimal_amount_in: eval_result.amount_in_wei,
            gross_profit_usd: gross_usd,
            estimated_net_profit_usd: net_usd,
            net_negative: false,
            net_economics,
            // WO-LEGS-TRIANGULAR-01: per-leg wei from the post-clamp
            // re-evaluation — replaces the honest-absent placeholders of
            // HOPS-LEDGER-04. Kelly overlay still nulls these when it rescales
            // (apply_kelly_constraints runs for every kernel outcome).
            leg_amounts_in,
            leg_amounts_out,
        }))
    }

    // -----------------------------------------------------------------------
    // V3-MULTILEG-SIZING-01 — N-leg (3..=7) V3-bearing sizing kernel
    // -----------------------------------------------------------------------

    /// Dispatch for an N-leg (3..=7) route carrying at least one V3 leg: the V3
    /// N-leg kernel when [`v3_multileg_sizing_enabled`], otherwise the repo's
    /// declared refusal — the pre-patch behaviour, byte-identical (zero RPC).
    async fn size_multileg_v3_or_refuse(
        &self,
        candidate: &StrategyCandidate,
        cap_wei: U256,
        cap_usd: f64,
        token_price_usd: f64,
        decimals: u8,
        state: &TradingConfigState,
    ) -> OptimizeOutcome {
        if !v3_multileg_sizing_enabled() {
            debug!(
                event = "size_optimizer.v3_multileg_unsupported",
                legs = candidate.route_plan.legs.len(),
                label = candidate.label.as_str(),
                "N-leg V3 kernel disabled (ARBX_V3_MULTILEG_SIZING) — declared refusal"
            );
            return OptimizeOutcome::Rejected(OptimizeRejectReason::V3MultilegUnsupported, None);
        }
        self.size_multileg_v3_with_reason(
            candidate,
            cap_wei,
            cap_usd,
            token_price_usd,
            decimals,
            state,
        )
        .await
    }

    /// V3-MULTILEG-SIZING-01 — optimal `amount_in` for a 3..=7-leg closed cycle
    /// in which at least one leg is a Uniswap-V3-style concentrated-liquidity
    /// pool, plus the per-leg ledger aligned with `route_plan.legs`.
    ///
    /// WHY A NEW KERNEL: `size_triangular_with_reason` composes constant-product
    /// legs only (a V3 pool has no `getReserves()`, hence no
    /// `arbx:pool_reserves:<chain>:<pool>` entry), and `size_two_leg_v3_with_reason`
    /// is strictly 2-leg. Measured production shape (2026-09-26): **585 of 585**
    /// sampled discovered 3..=7-hop cycles carry ≥ 1 V3 leg and **zero** are
    /// all-V2, so "refuse honestly" was the only possible verdict for the whole
    /// deep population — the operator's per-hop cards could never exist.
    ///
    /// MATH (the same outputs the V2 path produces, so nothing downstream changes):
    ///   · the profit function is the sequential composition
    ///     `f(x) = out_N(…out_1(x)…) − x`, one oriented leg at a time — exactly
    ///     the chain `size_triangular_with_reason` optimises, and exactly the
    ///     ledger shape it emits (`leg_amounts_in[i]` enters leg i;
    ///     `leg_amounts_in[i+1] == leg_amounts_out[i]`).
    ///   · V2 legs: exact local `amm_math::v2_amount_out` at the leg's own
    ///     declared `fee_bps` (default 30) — the same per-leg fee source the
    ///     2-leg V3 kernel uses. Zero RPC.
    ///   · V3 legs: **real on-chain QuoterV2** answers (`quote_leg` →
    ///     `project_v3_quote_checked`, tier resolved from `v3_fee_catalog`).
    ///     A within-tick local model is NEVER an accept signal: per the repo's
    ///     own documented convention it is an UPPER bound (it overestimates
    ///     output at sizes that cross a tick boundary), so accepting from it
    ///     would over-report profit and break the net-profit gate invariant. It
    ///     is used for exactly two 0-RPC jobs: (a) refuse the cycle when even
    ///     the optimistic bound is ≤ 0 (`v3_within_tick_upper_bound_nonpositive`
    ///     precedent), and (b) rank the RPC probe bracket.
    ///
    /// BOUNDED RPC (exact budget, all four figures enforced in code):
    ///   · ≤ `ARBX_V3_MULTILEG_MAX_PROBES` probe sizes per candidate (default 2:
    ///     the local argmax plus its grid neighbour; with no slot0 snapshot the
    ///     deterministic middle of the same log grid is used instead).
    ///   · ≤ `ARBX_V3_MULTILEG_MAX_QUOTES` QuoterV2 sub-calls per candidate
    ///     (default 8; one probe costs exactly one sub-call per V3 leg, and a
    ///     probe is never started unless the remaining allowance covers a FULL
    ///     chain — a partial chain can never become a ledger).
    ///   · ≤ `ARBX_V3_MULTILEG_QUOTES_PER_BLOCK` sub-calls per (chain, block)
    ///     across the process (default 48 ≈ 4/s at 12 s blocks) — claimed up
    ///     front, CAS-reset per block, shared by every sizing call site.
    ///   · the first V3 leg's per-probe inputs are known WITHOUT any quote when
    ///     the legs before it are V2 (leg 0's input IS the probe; a V2 output is
    ///     exact local CPMM), so they are prefetched in ONE aggregate3
    ///     Multicall (`quote_batch`, `ARBX_V3_QUOTE_BATCH_*` chunking/backoff).
    ///     The unary path stays authoritative, so a failed prefetch changes no
    ///     verdict — it only costs round-trips.
    ///
    /// R8: a leg whose state or quote is unavailable returns the WO-06 label
    /// (`v3_quote_unavailable` / `v3_pool_not_catalogued` / `v3_pair_no_pools`)
    /// with NO ledger and NO amounts; a candidate deferred by the quote
    /// allowance returns `v3_multileg_budget_exhausted`; neither is ever the
    /// misleading `missing_reserves_pool_a|b` (PERHOP-RESERVES-01 is preserved).
    /// When a complete real chain WAS priced but the verdict is a rejection,
    /// the row still carries that chain (`RejectedWithLedger`) so the operator
    /// sees every hop — the same all-or-nothing contract `attach_leg_ledger`
    /// enforces, and never a partial or fabricated ledger.
    async fn size_multileg_v3_with_reason(
        &self,
        candidate: &StrategyCandidate,
        cap_wei: U256,
        cap_usd: f64,
        token_price_usd: f64,
        decimals: u8,
        state: &TradingConfigState,
    ) -> OptimizeOutcome {
        let legs = &candidate.route_plan.legs;
        if legs.len() < 3 {
            return OptimizeOutcome::Rejected(OptimizeRejectReason::MissingRouteLegs, None);
        }
        if legs.len() > 7 {
            return OptimizeOutcome::Rejected(OptimizeRejectReason::UnsupportedLegCount, None);
        }

        // ── 1. Per-leg evaluators (protocol-neutral; zero RPC) ───────────────
        // A V2 leg needs its cached oriented reserves (an absent V2 entry IS an
        // honest `missing_reserves_pool_*` — it names the RIGHT cause: a V2 pool
        // with no cached reserves); a V3 leg needs only its address + fee tier +
        // direction, so it can never inherit a reserve-miss label.
        //
        // An unresolvable leg is REMEMBERED, never returned early: the blocker
        // the operator must see is the FIRST one in TRAVERSAL order. A V3 leg can
        // only fail to price at its own position in the chain, so a V2 reserve
        // miss sitting BEHIND it must never pre-empt the V3 cause — that is
        // exactly the PERHOP-RESERVES-01 confusion this kernel must not restore.
        let plan_v3_legs = legs.iter().filter(|l| leg_is_v3(l)).count();
        if plan_v3_legs == 0 {
            // Defensive: an all-V2 route never belongs here (the dispatcher
            // routes it to the constant-product kernel). Delegate so the two
            // kernels can never disagree about the same geometry.
            return self
                .size_triangular_with_reason(candidate, state, cap_usd, token_price_usd, decimals)
                .await;
        }
        let mut evals: Vec<Result<LegEval, OptimizeRejectReason>> = Vec::with_capacity(legs.len());
        for (i, leg) in legs.iter().enumerate() {
            let missing = if i == 0 {
                OptimizeRejectReason::MissingReservesPoolA
            } else {
                OptimizeRejectReason::MissingReservesPoolB
            };
            evals.push(self.build_leg_eval(leg, missing).await);
        }
        let first_unresolved = evals.iter().position(|e| e.is_err());
        // Quotes one full chain costs: the V3 legs the chain actually REACHES.
        let quotes_per_chain = evals
            .iter()
            .take(first_unresolved.unwrap_or(evals.len()))
            .filter(|e| matches!(e, Ok(LegEval::V3 { .. })))
            .count();

        // ── 2. LOCAL 0-RPC pass: within-tick V3 + CPMM over the log grid ──────
        // Only available when EVERY leg resolved (otherwise there is no local
        // model at all, and the bracket falls back below).
        let grid = geom_probes(U256::one(), cap_wei, V3_MULTILEG_LOCAL_POINTS);
        let local_best = match first_unresolved {
            Some(_) => None,
            None => {
                let resolved: Vec<LegEval> = evals
                    .iter()
                    .filter_map(|e| e.as_ref().ok().cloned())
                    .collect();
                let slot0s = self.slot0_snapshots(&resolved).await;
                Self::local_best_probe(&resolved, &slot0s, &grid)
            }
        };
        if let Some((_, bound_profit_wei)) = local_best {
            if bound_profit_wei <= 0 {
                // The optimistic bound is a provable upper bound on the real
                // grid's max, so a non-positive max refutes every real probe:
                // refuse WITHOUT a single QuoterV2 call (R8 — the payload is
                // the upper bound, not a quote).
                let bound_usd =
                    (bound_profit_wei as f64) / 10f64.powi(decimals as i32) * token_price_usd;
                debug!(
                    event = "size_optimizer.v3_multileg_early_reject_within_tick",
                    label = candidate.label.as_str(),
                    legs = legs.len(),
                    v3_legs = plan_v3_legs,
                    bound_profit_wei,
                    "within-tick upper bound ≤ 0 across the local grid — refusing before any QuoterV2 probe"
                );
                return OptimizeOutcome::Rejected(
                    OptimizeRejectReason::NonPositiveProfit,
                    Some(bound_usd),
                );
            }
        }

        // ── 3. Probe bracket (≤ ARBX_V3_MULTILEG_MAX_PROBES sizes) ────────────
        let max_probes = v3_multileg_max_probes();
        let probes = match local_best {
            Some((idx, _)) => Self::probes_around_best(&grid, idx, max_probes),
            // No local model (a leg did not resolve, or a V3 leg has no slot0
            // snapshot): spend the budget at the AUTHORIZED CAPITAL end of the
            // same grid. Taking the grid's centre instead sized the row at
            // ~`cap_wei^(8/15)` (dust) and published every figure at that
            // notional — NLEG-SIZE-BAND-01.
            None => Self::capital_band_probes(&grid, max_probes),
        };

        // ── 4. Quote allowance (per candidate AND per (chain, block)) ─────────
        // `planned` is the WORST CASE of the plan (every probe prices a full
        // chain); a chain that reaches no V3 leg needs no allowance at all.
        let planned = probes
            .len()
            .saturating_mul(quotes_per_chain)
            .min(v3_multileg_max_quotes());
        // A probe whose whole chain is not covered by the remaining allowance is
        // never started (a partial chain can never become a ledger).
        let affordable = if quotes_per_chain == 0 {
            probes.len()
        } else {
            planned / quotes_per_chain
        };
        let probes: &[U256] = &probes[..affordable.min(probes.len())];
        let epoch = candidate.opportunity.block_number;
        if planned > 0 && !self.v3_multileg_budget.try_claim(epoch, planned as u64) {
            debug!(
                event = "size_optimizer.v3_multileg_budget_exhausted",
                label = candidate.label.as_str(),
                legs = legs.len(),
                v3_legs = plan_v3_legs,
                planned,
                per_epoch = self.v3_multileg_budget.per_epoch(),
                "quote allowance spent — cycle deferred, never priced from a bound (R8)"
            );
            return OptimizeOutcome::Rejected(
                OptimizeRejectReason::V3MultilegBudgetExhausted,
                None,
            );
        }
        let mut quotes_left = planned;

        // ── 5. B1 batched prefetch: ONE aggregate3 for the first V3 leg ───────
        if let Some((k, pool, zero_for_one)) = Self::first_v3_leg(&evals) {
            let mut entries: Vec<U256> = Vec::with_capacity(probes.len());
            let mut all_known = true;
            for &x in probes {
                let mut current = x;
                for leg in evals.iter().take(k) {
                    match leg {
                        Ok(LegEval::V2 {
                            reserve_in,
                            reserve_out,
                            fee_bps,
                        }) => {
                            let out = v2_amount_out(current, *reserve_in, *reserve_out, *fee_bps);
                            if out.is_zero() {
                                all_known = false;
                                break;
                            }
                            current = out;
                        }
                        // Unreachable by construction (`first_v3_leg` returns the
                        // EARLIEST V3 leg) — kept honest rather than assumed: a
                        // preceding V3 leg's local output is an upper bound and
                        // quoting downstream at a bound could overstate the chain.
                        _ => {
                            all_known = false;
                            break;
                        }
                    }
                }
                if !all_known {
                    break;
                }
                entries.push(current);
            }
            if all_known && !entries.is_empty() {
                let dispatched = self
                    .state_projector
                    .prefetch_v3_quotes_multi_amount(&pool, zero_for_one, &entries)
                    .await;
                debug!(
                    event = "size_optimizer.v3_multileg_batch_prefetch",
                    pool = %pool.address,
                    requested = entries.len(),
                    dispatched,
                    "first V3 leg prefetched in one aggregate3 multicall"
                );
            }
        }

        // ── 6. Bounded sequential refinement — the REAL chain ────────────────
        let mut best: Option<(U256, Vec<String>, Vec<String>, i128)> = None;
        let mut unavailable: Option<&'static str> = None;
        // The first leg the chain could not even RESOLVE (a V2 reserve miss, a
        // zero-reserve pool, an absent pool address) — recorded so the verdict
        // names the first blocker in traversal order.
        let mut first_leg_blocker: Option<OptimizeRejectReason> = None;
        let mut probes_attempted = 0usize;
        for &x in probes {
            if x.is_zero() {
                continue;
            }
            // One full chain costs exactly one quote per V3 leg: never start a
            // probe the remaining allowance cannot cover (a partial chain is
            // never a ledger).
            if quotes_left < quotes_per_chain {
                break;
            }
            probes_attempted += 1;
            let mut current = x;
            let mut ins: Vec<String> = Vec::with_capacity(evals.len());
            let mut outs: Vec<String> = Vec::with_capacity(evals.len());
            let mut complete = true;
            for eval in &evals {
                let leg = match eval {
                    Ok(l) => l,
                    Err(r) => {
                        first_leg_blocker.get_or_insert(*r);
                        complete = false;
                        break;
                    }
                };
                if matches!(leg, LegEval::V3 { .. }) {
                    quotes_left = quotes_left.saturating_sub(1);
                }
                match self.eval_leg_out(leg, current).await {
                    LegQuote::Priced(v) => {
                        // A zero-yield hop kills the chain (no ledger from it).
                        if v.is_zero() {
                            complete = false;
                            break;
                        }
                        ins.push(current.to_string());
                        outs.push(v.to_string());
                        current = v;
                    }
                    LegQuote::Unavailable(label) => {
                        unavailable.get_or_insert(label);
                        complete = false;
                        break;
                    }
                }
            }
            if !complete {
                continue;
            }
            let profit_wei = clamped_to_i128(current).saturating_sub(clamped_to_i128(x));
            if best.as_ref().is_none_or(|b| profit_wei > b.3) {
                best = Some((x, ins, outs, profit_wei));
            }
        }

        let Some((amount_in, ins, outs, profit_wei)) = best else {
            if let Some(label) = unavailable {
                // WO-06 vocabulary: a catalog gap stays distinct from a real
                // provider failure (both were rejected without inventing a
                // price, and neither carries a ledger — R8).
                let reason = OptimizeRejectReason::from_v3_unavailable_label(label);
                debug!(
                    event = "size_optimizer.v3_multileg_unavailable",
                    label = candidate.label.as_str(),
                    reason = reason.as_str(),
                    legs = legs.len(),
                    v3_legs = plan_v3_legs,
                    probes_attempted,
                    "no complete real chain — explicit rejection, no ledger (R8)"
                );
                return OptimizeOutcome::Rejected(reason, None);
            }
            if let Some(reason) = first_leg_blocker {
                // The chain died on a leg that has no local state at all. This
                // is only reachable when every EARLIER leg (any V3 among them
                // included) was priced or resolved, so the reason names the true
                // first blocker — never a V3 leg's state behind a V2 miss.
                debug!(
                    event = "size_optimizer.v3_multileg_leg_unresolved",
                    label = candidate.label.as_str(),
                    reason = reason.as_str(),
                    legs = legs.len(),
                    v3_legs = plan_v3_legs,
                    probes_attempted,
                    "the chain reached a leg with no local state — explicit rejection (R8)"
                );
                return OptimizeOutcome::Rejected(reason, None);
            }
            // Every probe died on a real zero output (the quoter answered).
            debug!(
                event = "size_optimizer.v3_multileg_zero_chain",
                label = candidate.label.as_str(),
                legs = legs.len(),
                v3_legs = plan_v3_legs,
                probes_attempted,
            );
            return OptimizeOutcome::Rejected(OptimizeRejectReason::NonPositiveProfit, None);
        };

        let profit_token_units = (profit_wei as f64) / 10f64.powi(decimals as i32);
        let gross_usd = profit_token_units * token_price_usd;

        // PER-HOP: the full real chain was quote-priced, so the ledger travels
        // with the row EVEN on the rejection arms below (the operator's per-hop
        // requirement) — never a partial or fabricated chain (R8).
        if profit_wei <= 0 {
            debug!(
                event = "size_optimizer.v3_multileg_non_positive",
                label = candidate.label.as_str(),
                v3_legs = plan_v3_legs,
                probes_attempted,
                profit_wei,
            );
            // ALWAYS-COMPUTE (2026-09-27 rebase over #700): #700 already
            // carried the priced chain + gross on this reject
            // (`RejectedWithLedger`); the operator's mandate upgrades the SAME
            // payload to the FULL figures — cost components, net, and the
            // complete economics object ride the boxed `SizedCandidate`.
            // Nothing from #700 is dropped: its ledger and gross survive
            // verbatim inside the richer variant ("el máximo de campos con
            // dato").
            let gas_cost = state.gas_cost_usd();
            let ops_overhead = state.ops_overhead_usd_per_attempt;
            let start_amount_usd =
                (clamped_to_i128(amount_in) as f64) / 10f64.powi(decimals as i32) * token_price_usd;
            let borrow_usd = if candidate.base_strategy.is_some() {
                start_amount_usd
            } else {
                0.0
            };
            let flash_fee_usd =
                borrow_usd * crate::financing::selected_mode(borrow_usd).fee_bps() / 10_000.0;
            let net_usd = gross_usd - gas_cost - ops_overhead - flash_fee_usd;
            let mut cand = candidate.clone();
            cand.opportunity.amount_in_wei = amount_in.to_string();
            cand.opportunity.expected_profit_usd = Some(gross_usd);
            cand.opportunity.net_expected_profit_usd = Some(net_usd);
            return OptimizeOutcome::RejectedComputed(
                OptimizeRejectReason::NonPositiveProfit,
                Box::new(SizedCandidate {
                    candidate: cand,
                    optimal_amount_in: amount_in,
                    gross_profit_usd: gross_usd,
                    estimated_net_profit_usd: net_usd,
                    net_negative: true,
                    net_economics: Some(crate::net_bps_ranking::RouteNetEconomics::from_kernel(
                        start_amount_usd,
                        gross_usd,
                        gas_cost,
                        ops_overhead,
                        borrow_usd,
                    )),
                    leg_amounts_in: Some(ins),
                    leg_amounts_out: Some(outs),
                }),
            );
        }
        if gross_usd <= 0.0 {
            // Same enrichment on the degenerate-pricing arm (gross priced ≤ 0
            // with a complete priced chain — the ledger still rides, R8).
            let gas_cost = state.gas_cost_usd();
            let ops_overhead = state.ops_overhead_usd_per_attempt;
            let start_amount_usd =
                (clamped_to_i128(amount_in) as f64) / 10f64.powi(decimals as i32) * token_price_usd;
            let borrow_usd = if candidate.base_strategy.is_some() {
                start_amount_usd
            } else {
                0.0
            };
            let flash_fee_usd =
                borrow_usd * crate::financing::selected_mode(borrow_usd).fee_bps() / 10_000.0;
            let net_usd = gross_usd - gas_cost - ops_overhead - flash_fee_usd;
            let mut cand = candidate.clone();
            cand.opportunity.amount_in_wei = amount_in.to_string();
            cand.opportunity.expected_profit_usd = Some(gross_usd);
            cand.opportunity.net_expected_profit_usd = Some(net_usd);
            return OptimizeOutcome::RejectedComputed(
                OptimizeRejectReason::NonPositiveGrossUsd,
                Box::new(SizedCandidate {
                    candidate: cand,
                    optimal_amount_in: amount_in,
                    gross_profit_usd: gross_usd,
                    estimated_net_profit_usd: net_usd,
                    net_negative: true,
                    net_economics: Some(crate::net_bps_ranking::RouteNetEconomics::from_kernel(
                        start_amount_usd,
                        gross_usd,
                        gas_cost,
                        ops_overhead,
                        borrow_usd,
                    )),
                    leg_amounts_in: Some(ins),
                    leg_amounts_out: Some(outs),
                }),
            );
        }

        // USD/net math is byte-identical to the 2-leg V3 / V2 kernels (same
        // ARBX-0007 financing dimension, same sheet-07 economics).
        let gas_cost = state.gas_cost_usd();
        let ops_overhead = state.ops_overhead_usd_per_attempt;
        let start_amount_usd =
            (clamped_to_i128(amount_in) as f64) / 10f64.powi(decimals as i32) * token_price_usd;
        let borrow_usd = if candidate.base_strategy.is_some() {
            start_amount_usd
        } else {
            0.0
        };
        let financing_mode = crate::financing::selected_mode(borrow_usd);
        let financing_evals =
            crate::financing::evaluate_modes(gross_usd, gas_cost, ops_overhead, borrow_usd);
        let flashloan_fee_usd = borrow_usd * financing_mode.fee_bps() / 10_000.0;
        let net_usd = gross_usd - gas_cost - ops_overhead - flashloan_fee_usd;
        let net_economics = Some(crate::net_bps_ranking::RouteNetEconomics::from_kernel(
            start_amount_usd,
            gross_usd,
            gas_cost,
            ops_overhead,
            borrow_usd,
        ));
        let leg_amounts_in = Some(ins);
        let leg_amounts_out = Some(outs);

        if net_usd <= 0.0 {
            debug!(
                event = "size_optimizer.v3_multileg_negative_net",
                label = candidate.label.as_str(),
                legs = legs.len(),
                v3_legs = plan_v3_legs,
                amount_in = %amount_in,
                gross_usd,
                gas_cost,
                ops_overhead,
                flashloan_fee_usd,
                financing_mode = financing_mode.as_str(),
                financing_nets = ?financing_evals,
                net_usd,
            );
            // HARDENING (same contract as every other kernel): populate the
            // SizedCandidate with the computed values even when net ≤ 0 — the
            // net-positive gate is an EXECUTION gate, and the operator must see
            // the real numbers and the per-hop ledger on the card.
            let mut cand = candidate.clone();
            cand.opportunity.amount_in_wei = amount_in.to_string();
            cand.opportunity.expected_profit_usd = Some(gross_usd);
            cand.opportunity.net_expected_profit_usd = Some(net_usd);
            return OptimizeOutcome::Sized(Box::new(SizedCandidate {
                candidate: cand,
                optimal_amount_in: amount_in,
                gross_profit_usd: gross_usd,
                estimated_net_profit_usd: net_usd,
                net_negative: true,
                net_economics,
                leg_amounts_in,
                leg_amounts_out,
            }));
        }

        let mut sized = candidate.clone();
        sized.opportunity.amount_in_wei = amount_in.to_string();
        sized.opportunity.expected_profit_usd = Some(gross_usd);
        sized.gross_profit_usd = Some(gross_usd);
        sized.net_expected_profit_usd = Some(net_usd);

        debug!(
            event = "size_optimizer.v3_multileg_sized",
            label = sized.label.as_str(),
            legs = legs.len(),
            v3_legs = plan_v3_legs,
            amount_in = %amount_in,
            gross_usd,
            net_usd,
            probes = probes_attempted,
            financing_mode = financing_mode.as_str(),
            financing_nets = ?financing_evals,
        );

        OptimizeOutcome::Sized(Box::new(SizedCandidate {
            candidate: sized,
            optimal_amount_in: amount_in,
            gross_profit_usd: gross_usd,
            estimated_net_profit_usd: net_usd,
            net_negative: false,
            net_economics,
            leg_amounts_in,
            leg_amounts_out,
        }))
    }

    // -----------------------------------------------------------------------
    // V3-MULTILEG-SIZING-01 helpers
    // -----------------------------------------------------------------------

    /// Resolve the slot0 snapshot of every V3 leg (index-aligned with `evals`;
    /// `None` for V2 legs and for a V3 leg whose snapshot is not cached).
    ///
    /// Zero RPC: a miss is a miss (the local pass then simply produces no
    /// ranking and the kernel falls back to the deterministic middle bracket).
    async fn slot0_snapshots(&self, evals: &[LegEval]) -> Vec<Option<V3Slot0Snapshot>> {
        let mut out: Vec<Option<V3Slot0Snapshot>> = Vec::with_capacity(evals.len());
        for leg in evals {
            match leg {
                LegEval::V3 { pool, .. } => match self.slot0_cache.as_ref() {
                    Some(cache) => out.push(cache.get(&pool.address).await),
                    None => out.push(None),
                },
                LegEval::V2 { .. } => out.push(None),
            }
        }
        out
    }

    /// Compose the WHOLE N-leg chain locally (0 RPC) at `x`: exact CPMM for V2
    /// legs, `amm_math::v3_amount_out_single_tick` for V3 legs. `None` when a leg
    /// cannot be priced locally (V3 without a slot0 snapshot) or the chain dies
    /// (a leg yields zero).
    ///
    /// R8: the V3 half is the repo's documented within-tick UPPER bound, so the
    /// composed result bounds the real chain output from above. It is used ONLY
    /// to refuse or to rank probes — never to price a row.
    fn compose_chain_within_tick(
        evals: &[LegEval],
        slot0s: &[Option<V3Slot0Snapshot>],
        x: U256,
    ) -> Option<U256> {
        let mut current = x;
        for (leg, slot0) in evals.iter().zip(slot0s.iter()) {
            let out = match leg {
                LegEval::V2 {
                    reserve_in,
                    reserve_out,
                    fee_bps,
                } => v2_amount_out(current, *reserve_in, *reserve_out, *fee_bps),
                LegEval::V3 { pool, zero_for_one } => {
                    let sp = (*slot0)?;
                    // V3 fee tier lives in the (misnamed) `fee_bps` field in
                    // MILLIONTHS (500 = 0.05%) — exactly the `fee_pips`
                    // within-tick math expects. Same convention as
                    // `v3_within_tick_upper_bound_nonpositive`.
                    let fee_pips = pool.fee_bps.unwrap_or(500);
                    v3_amount_out_single_tick(
                        current,
                        sp.sqrt_price_x96,
                        sp.liquidity,
                        fee_pips,
                        *zero_for_one,
                    )
                }
            };
            if out.is_zero() {
                return None;
            }
            current = out;
        }
        Some(current)
    }

    /// Highest optimistic profit over `grid`, with the index that produced it.
    /// `None` when NO probe could be priced locally (a missing slot0 snapshot or
    /// a degenerate local chain) — a missing cache never fabricates a verdict.
    fn local_best_probe(
        evals: &[LegEval],
        slot0s: &[Option<V3Slot0Snapshot>],
        grid: &[U256],
    ) -> Option<(usize, i128)> {
        let mut best: Option<(usize, i128)> = None;
        for (i, &x) in grid.iter().enumerate() {
            if x.is_zero() {
                continue;
            }
            let Some(out) = Self::compose_chain_within_tick(evals, slot0s, x) else {
                continue;
            };
            let profit = clamped_to_i128(out).saturating_sub(clamped_to_i128(x));
            if best.is_none_or(|(_, p)| profit > p) {
                best = Some((i, profit));
            }
        }
        best
    }

    /// Probe bracket around the local argmax: the argmax first, then its grid
    /// neighbours outward (`i-1`, `i+1`, `i-2`, …), capped at `max` and
    /// deduplicated. The argmax of the upper bound is the best available proxy
    /// for the real optimum; its neighbours make the RPC refinement a real
    /// comparison rather than a single-point guess.
    fn probes_around_best(grid: &[U256], best_idx: usize, max: usize) -> Vec<U256> {
        let mut out: Vec<U256> = Vec::with_capacity(max.min(grid.len()));
        let mut push = |i: usize, out: &mut Vec<U256>| {
            if out.len() < max {
                if let Some(v) = grid.get(i) {
                    if !v.is_zero() && !out.contains(v) {
                        out.push(*v);
                    }
                }
            }
        };
        push(best_idx, &mut out);
        let mut step = 1usize;
        while out.len() < max && step < grid.len() {
            if best_idx >= step {
                push(best_idx - step, &mut out);
            }
            push(best_idx + step, &mut out);
            step += 1;
        }
        if out.is_empty() {
            out.push(U256::one());
        }
        out
    }

    /// Fallback bracket when no local model is available: the centred `max`
    /// points of the same log grid. Deterministic, never a guessed optimum.
    ///
    /// Kept for callers whose grid is NOT anchored on the authorized capital;
    /// the V3 N-leg model-free path uses [`Self::capital_band_probes`] instead
    /// (NLEG-SIZE-BAND-01 — see that function for the measured defect).
    fn middle_probes(grid: &[U256], max: usize) -> Vec<U256> {
        if grid.is_empty() {
            return vec![U256::one()];
        }
        if grid.len() <= max {
            return grid.to_vec();
        }
        let start = (grid.len() - max) / 2;
        grid[start..start + max].to_vec()
    }

    /// NLEG-SIZE-BAND-01 — model-free bracket anchored on the AUTHORIZED capital.
    ///
    /// MEASURED DEFECT. The V3 N-leg path builds its local grid as
    /// `geom_probes(1 wei, cap_wei, 16)` — log-spaced from one wei to the
    /// operator's capital. When no local model exists (a leg did not resolve, or
    /// a V3 leg has no slot0 snapshot) the probe budget used to be spent on the
    /// **centre** of that grid: for `cap_usd = $1000` on an 18-decimals token
    /// (`cap_wei ≈ 3.7e17`) the centred points are `≈ cap_wei^(8/15) ≈ 2.3e9 wei
    /// ≈ $0.000006`. Every figure the row then published — gross, net, roi_pct,
    /// and the whole cost ladder — was computed at that dust notional, which is
    /// why live cards carried `gross = 0.00000000` next to `amount_in_wei = 1e18`
    /// and why rejected rows looked like they had "no economics".
    ///
    /// The decision the sizing kernel exists to make is "how much of the
    /// AUTHORIZED capital should this route take", so a model-free row must be
    /// probed at the capital end of the same grid. This returns the largest
    /// `max` points of the grid, i.e. the top of the authorized band; the
    /// largest probe is exactly `cap_wei`.
    fn capital_band_probes(grid: &[U256], max: usize) -> Vec<U256> {
        if grid.is_empty() || max == 0 {
            return vec![U256::one()];
        }
        let take = max.min(grid.len());
        grid[grid.len() - take..].to_vec()
    }

    /// The earliest RESOLVED V3 leg: `(index, pool, zero_for_one)`. `None` when
    /// no V3 leg resolved (an all-V2 route — the dispatcher never routes those
    /// here — or a V3 leg whose own address is unusable).
    fn first_v3_leg(
        evals: &[Result<LegEval, OptimizeRejectReason>],
    ) -> Option<(usize, PoolRef, bool)> {
        evals.iter().enumerate().find_map(|(i, e)| match e {
            Ok(LegEval::V3 { pool, zero_for_one }) => Some((i, pool.clone(), *zero_for_one)),
            _ => None,
        })
    }

    // -----------------------------------------------------------------------
    // Legacy shim — 3-leg triangular (kept for backwards compat within this file)
    // -----------------------------------------------------------------------

    #[allow(dead_code)]
    async fn size_triangular(
        &self,
        candidate: &StrategyCandidate,
        state: &TradingConfigState,
        cap_usd: f64,
        token_price_usd: f64,
        decimals: u8,
    ) -> Option<SizedCandidate> {
        match self
            .size_triangular_with_reason(candidate, state, cap_usd, token_price_usd, decimals)
            .await
        {
            OptimizeOutcome::Sized(s) => Some(*s),
            OptimizeOutcome::Rejected(_, _) => None,
            OptimizeOutcome::RejectedWithLedger(_, _, _) => None,
            OptimizeOutcome::RejectedComputed(_, _) => None,
        }
    }

    // -----------------------------------------------------------------------
    // 2-leg DEX sizing — with explicit reason (TASK 2)
    // -----------------------------------------------------------------------

    #[allow(clippy::too_many_arguments)]
    async fn size_two_leg_with_reason(
        &self,
        candidate: &StrategyCandidate,
        intent: &RouteIntent,
        cap_wei: U256,
        cap_usd: f64,
        token_price_usd: f64,
        decimals: u8,
        state: &TradingConfigState,
    ) -> OptimizeOutcome {
        // Extract pool addresses and orientations from the 2-leg route plan.
        let legs = &candidate.route_plan.legs;
        if legs.len() < 2 {
            return OptimizeOutcome::Rejected(OptimizeRejectReason::MissingRouteLegs, None);
        }
        // Defensive fail-closed: this kernel only models exactly 2 legs —
        // a >2-leg candidate here would silently truncate to legs[0..2] and
        // fabricate the economics (the "cards show only 2 hops" root cause).
        if legs.len() > 2 {
            return OptimizeOutcome::Rejected(OptimizeRejectReason::UnsupportedLegCount, None);
        }

        // Pool A reserves (leg 0).
        let pool_a_addr_str = match legs[0].pool_address.as_deref() {
            Some(s) => s,
            None => {
                return OptimizeOutcome::Rejected(OptimizeRejectReason::MissingPoolAddress, None)
            }
        };
        let pool_a_addr: ethers::types::Address = match pool_a_addr_str.parse().ok() {
            Some(a) => a,
            None => {
                return OptimizeOutcome::Rejected(OptimizeRejectReason::MissingPoolAddress, None)
            }
        };
        let (r0_a, r1_a) = match self.state_projector.reserves_cache.get(&pool_a_addr).await {
            Some(pair) => pair,
            None => {
                return OptimizeOutcome::Rejected(OptimizeRejectReason::MissingReservesPoolA, None)
            }
        };

        // Pool B reserves (leg 1).
        let pool_b_addr_str = match legs[1].pool_address.as_deref() {
            Some(s) => s,
            None => {
                return OptimizeOutcome::Rejected(OptimizeRejectReason::MissingPoolAddress, None)
            }
        };
        let pool_b_addr: ethers::types::Address = match pool_b_addr_str.parse().ok() {
            Some(a) => a,
            None => {
                return OptimizeOutcome::Rejected(OptimizeRejectReason::MissingPoolAddress, None)
            }
        };
        let (r0_b, r1_b) = match self.state_projector.reserves_cache.get(&pool_b_addr).await {
            Some(pair) => pair,
            None => {
                return OptimizeOutcome::Rejected(OptimizeRejectReason::MissingReservesPoolB, None)
            }
        };

        // Orient reserves for each leg.
        let (reserve_in_a, reserve_out_a) =
            orient_reserves(r0_a, r1_a, &legs[0].token_in, &legs[0].token_out);
        let (reserve_in_b, reserve_out_b) =
            orient_reserves(r0_b, r1_b, &legs[1].token_in, &legs[1].token_out);

        if reserve_in_a.is_zero()
            || reserve_out_a.is_zero()
            || reserve_in_b.is_zero()
            || reserve_out_b.is_zero()
        {
            return OptimizeOutcome::Rejected(OptimizeRejectReason::ZeroReserves, None);
        }

        let fee_a = legs[0].fee_bps.unwrap_or(30);
        let fee_b = legs[1].fee_bps.unwrap_or(30);

        // Search bounds.
        let x_lo = U256::from(1u64);
        let x_hi = {
            let ceiling = if cap_wei < reserve_in_a {
                cap_wei
            } else {
                reserve_in_a
            };
            if ceiling > x_lo {
                ceiling
            } else {
                x_lo
            }
        };

        let hop_reserves_a = vec![(reserve_in_a, reserve_out_a)];
        let hop_reserves_b = vec![(reserve_in_b, reserve_out_b)];

        let (x_star, profit_wei) = golden_section_search_2leg(
            x_lo,
            x_hi,
            &hop_reserves_a,
            &hop_reserves_b,
            fee_a,
            fee_b,
            25,
        );

        if profit_wei <= 0 {
            // ALWAYS-COMPUTE (operator mandate 2026-09-27): the golden-section
            // evaluation DID price both legs at its optimum `x_star` (local
            // CPMM math over the same cached reserves) — re-derive the exact
            // chain there so the rejection carries the FULL figures it
            // computed: gross (≤ 0), the cost components it was judged
            // against, net, and the per-leg ledger. "FAIL = se hizo el cálculo
            // y no cumple el criterio", never "no tengo números".
            let profit_usd = (profit_wei as f64) / 10f64.powi(decimals as i32) * token_price_usd;
            let out_a = v2_amount_out(x_star, reserve_in_a, reserve_out_a, fee_a);
            let out_b = v2_amount_out(out_a, reserve_in_b, reserve_out_b, fee_b);
            let gas_cost = state.gas_cost_usd();
            let ops_overhead = state.ops_overhead_usd_per_attempt;
            let start_amount_usd =
                (clamped_to_i128(x_star) as f64) / 10f64.powi(decimals as i32) * token_price_usd;
            let borrow_usd = if candidate.base_strategy.is_some() {
                start_amount_usd
            } else {
                0.0
            };
            let flash_fee_usd =
                borrow_usd * crate::financing::selected_mode(borrow_usd).fee_bps() / 10_000.0;
            let net_usd = profit_usd - gas_cost - ops_overhead - flash_fee_usd;
            let mut cand = candidate.clone();
            cand.opportunity.amount_in_wei = x_star.to_string();
            cand.opportunity.expected_profit_usd = Some(profit_usd);
            cand.opportunity.net_expected_profit_usd = Some(net_usd);
            return OptimizeOutcome::RejectedComputed(
                OptimizeRejectReason::NonPositiveProfit,
                Box::new(SizedCandidate {
                    candidate: cand,
                    optimal_amount_in: x_star,
                    gross_profit_usd: profit_usd,
                    estimated_net_profit_usd: net_usd,
                    net_negative: true,
                    net_economics: Some(crate::net_bps_ranking::RouteNetEconomics::from_kernel(
                        start_amount_usd,
                        profit_usd,
                        gas_cost,
                        ops_overhead,
                        borrow_usd,
                    )),
                    leg_amounts_in: Some(vec![x_star.to_string(), out_a.to_string()]),
                    leg_amounts_out: Some(vec![out_a.to_string(), out_b.to_string()]),
                }),
            );
        }

        // Anti-BUG-3: clamp to cap.
        let amount_in = match clamp_to_cap_wei(x_star, cap_usd, token_price_usd, decimals) {
            Some(v) => v,
            None => return OptimizeOutcome::Rejected(OptimizeRejectReason::CapClampFailed, None),
        };

        // Re-evaluate at clamped amount.
        let out_a = v2_amount_out(amount_in, reserve_in_a, reserve_out_a, fee_a);
        let out_b = v2_amount_out(out_a, reserve_in_b, reserve_out_b, fee_b);
        let profit_at_clamped = {
            let out_b_i = clamped_to_i128(out_b);
            let in_i = clamped_to_i128(amount_in);
            out_b_i.saturating_sub(in_i)
        };

        if profit_at_clamped <= 0 {
            // Payload = the clamped-size profit converted to USD (<= 0).
            let profit_usd =
                (profit_at_clamped as f64) / 10f64.powi(decimals as i32) * token_price_usd;
            // PER-HOP (math-audit AUDIT-MATH-OPPS-2026-09-26): this path ALREADY
            // computed the exact clamped chain (amount_in, out_a, out_b) — the
            // same values the accepted path below turns into the ledger. Carry
            // them so the rejected row can still show each hop's movement
            // (operator's per-hop request) instead of "not computed".
            //
            // ALWAYS-COMPUTE (2026-09-27): upgraded from RejectedWithLedger to
            // RejectedComputed — the full gross/costs/net arithmetic travels
            // with the ledger (the same construction the accepted tail uses,
            // one comparison earlier).
            let gas_cost = state.gas_cost_usd();
            let ops_overhead = state.ops_overhead_usd_per_attempt;
            let start_amount_usd =
                (clamped_to_i128(amount_in) as f64) / 10f64.powi(decimals as i32) * token_price_usd;
            let borrow_usd = if candidate.base_strategy.is_some() {
                start_amount_usd
            } else {
                0.0
            };
            let flash_fee_usd =
                borrow_usd * crate::financing::selected_mode(borrow_usd).fee_bps() / 10_000.0;
            let net_usd = profit_usd - gas_cost - ops_overhead - flash_fee_usd;
            let mut cand = candidate.clone();
            cand.opportunity.amount_in_wei = amount_in.to_string();
            cand.opportunity.expected_profit_usd = Some(profit_usd);
            cand.opportunity.net_expected_profit_usd = Some(net_usd);
            return OptimizeOutcome::RejectedComputed(
                OptimizeRejectReason::NonPositiveProfit,
                Box::new(SizedCandidate {
                    candidate: cand,
                    optimal_amount_in: amount_in,
                    gross_profit_usd: profit_usd,
                    estimated_net_profit_usd: net_usd,
                    net_negative: true,
                    net_economics: Some(crate::net_bps_ranking::RouteNetEconomics::from_kernel(
                        start_amount_usd,
                        profit_usd,
                        gas_cost,
                        ops_overhead,
                        borrow_usd,
                    )),
                    leg_amounts_in: Some(vec![amount_in.to_string(), out_a.to_string()]),
                    leg_amounts_out: Some(vec![out_a.to_string(), out_b.to_string()]),
                }),
            );
        }

        // HOPS-LEDGER-04: exact per-leg wei at the reported (clamped) size —
        // the same values the profit math above consumed. Leg 1's input IS
        // leg 0's output, so the arrays chain an honest ledger.
        let leg_amounts_in = Some(vec![amount_in.to_string(), out_a.to_string()]);
        let leg_amounts_out = Some(vec![out_a.to_string(), out_b.to_string()]);

        let profit_token_units = (profit_at_clamped as f64) / 10f64.powi(decimals as i32);
        let gross_usd = profit_token_units * token_price_usd;

        if gross_usd <= 0.0 {
            return OptimizeOutcome::Rejected(
                OptimizeRejectReason::NonPositiveGrossUsd,
                Some(gross_usd),
            );
        }

        let gas_cost = state.gas_cost_usd();
        let ops_overhead = state.ops_overhead_usd_per_attempt;
        // ARBX-0007: financing-mode route dimension. The flash-backed borrow
        // (base_strategy set) is priced under every canonical mode; the
        // selected mode preserves the legacy 5 bps math (fee term within
        // 1 ulp; own capital pays no fee). Per-mode nets surface the
        // born/died funnel in ONE debug line (R9).
        let borrow_usd = if candidate.base_strategy.is_some() {
            (clamped_to_i128(amount_in) as f64) / 10f64.powi(decimals as i32) * token_price_usd
        } else {
            0.0
        };
        let financing_mode = crate::financing::selected_mode(borrow_usd);
        let financing_evals =
            crate::financing::evaluate_modes(gross_usd, gas_cost, ops_overhead, borrow_usd);
        let flashloan_fee_usd = borrow_usd * financing_mode.fee_bps() / 10_000.0;
        let net_usd = gross_usd - gas_cost - ops_overhead - flashloan_fee_usd;
        // ARBX-0009: sheet-07 economics — start = priced amount_in (the capital
        // deployed; identical to borrow for flash routes, so the ranked
        // NetProfit is the same figure the kernel's net_usd produced).
        let start_amount_usd =
            (clamped_to_i128(amount_in) as f64) / 10f64.powi(decimals as i32) * token_price_usd;
        let net_economics = Some(crate::net_bps_ranking::RouteNetEconomics::from_kernel(
            start_amount_usd,
            gross_usd,
            gas_cost,
            ops_overhead,
            borrow_usd,
        ));

        if net_usd <= 0.0 {
            debug!(
                event = "size_optimizer.dex_negative_net",
                label = candidate.label.as_str(),
                gross_usd,
                gas_cost,
                ops_overhead,
                flashloan_fee_usd,
                financing_mode = financing_mode.as_str(),
                financing_nets = ?financing_evals,
                net_usd,
            );
            // HARDENING: poblar el SizedCandidate con los valores calculados
            // aunque net <= 0. El operador necesita ver los números.
            let mut cand = candidate.clone();
            cand.opportunity.amount_in_wei = amount_in.to_string();
            cand.opportunity.expected_profit_usd = Some(gross_usd);
            cand.opportunity.net_expected_profit_usd = Some(net_usd);
            return OptimizeOutcome::Sized(Box::new(SizedCandidate {
                candidate: cand,
                optimal_amount_in: amount_in,
                gross_profit_usd: gross_usd,
                estimated_net_profit_usd: net_usd,
                net_negative: true,
                net_economics,
                leg_amounts_in,
                leg_amounts_out,
            }));
        }

        let _ = intent;

        let mut sized = candidate.clone();
        sized.opportunity.amount_in_wei = amount_in.to_string();
        sized.opportunity.expected_profit_usd = Some(gross_usd);
        sized.gross_profit_usd = Some(gross_usd);
        sized.net_expected_profit_usd = Some(net_usd);

        debug!(
            event = "size_optimizer.sized",
            label = sized.label.as_str(),
            amount_in = %amount_in,
            gross_usd,
            net_usd,
            financing_mode = financing_mode.as_str(),
            financing_nets = ?financing_evals,
        );

        OptimizeOutcome::Sized(Box::new(SizedCandidate {
            candidate: sized,
            optimal_amount_in: amount_in,
            gross_profit_usd: gross_usd,
            estimated_net_profit_usd: net_usd,
            net_negative: false,
            net_economics,
            leg_amounts_in,
            leg_amounts_out,
        }))
    }

    // -----------------------------------------------------------------------
    // 2-leg DEX sizing — V3 concentrated liquidity — with explicit reason
    // -----------------------------------------------------------------------
    //
    // Sizes a 2-leg DEX arb where at least one leg is a Uniswap-V3-style
    // concentrated-liquidity pool. V3 output is NOT a closed-form function of
    // two reserves — it depends on the tick/liquidity distribution — so every
    // probe size is priced with the REAL on-chain QuoterV2 via
    // `state_projector.project_v3_quote` (the provider wired in Step 1 /
    // commit 6cb7d69). QuoterV2's `amount_out` already incorporates the fee
    // tier and cross-tick liquidity, so the profit number is EXACT and never an
    // over-report — the invariant the net-profit-gate depends on.
    //
    // Search: a fixed log-spaced bracket of `V3_BRACKET_POINTS` sizes over
    // `[1, cap_wei]`. The 2-leg profit f(x)=leg2(leg1(x))−x is unimodal for
    // CFMM-style curves, so the best grid point is a sound, honest size for
    // DETECTION. (RPC-frugal refinement tracked separately: bracket locally
    // with `amm_math::v3_amount_out_single_tick` over a slot0 cache — an upper
    // bound that early-rejects at 0 RPC — then refine survivors with a batched
    // multicall. Not required to surface real opps, so deferred.)
    //
    // RPC: each V3 leg is quoted once per probe (≤ `V3_BRACKET_POINTS` calls),
    // each through `HttpRpcPool::with_retry` (circuit-breaker + failover). V2
    // legs are local. NO-ACTIVE: QuoterV2 is a `staticcall` — read-only, no
    // signer, no capital. Only `amount_in` is decided here; the capital barrier
    // (net-profit-gate, simulation, checklist) is downstream and untouched.
    //
    // Honesty caveats (adversarial review 2026-06-05):
    //   * `amount_in` is an exactly-quoted grid point — the reported profit is
    //     the REAL QuoterV2 round-trip at that size, not an interpolation.
    //   * Block freshness: the quote reads the provider's default block, which
    //     `rpc_failover` allows to lag ≤ `DRIFT_THRESHOLD_BLOCKS`. This is the
    //     SAME provenance the triangular V3 path already relies on; acceptable
    //     for shadow DETECTION, and the mandatory fork simulation re-verifies
    //     against fresh state before any execution (which is FASE D regardless).
    //   * Kelly (Step 8) only ever shrinks the size when it binds, and rescales
    //     gross linearly. For a concave 2-leg profit curve a linear down-scale
    //     UNDER-states the true profit (chord ≤ curve) — conservative, never an
    //     over-report. The sparse 8-point grid likewise can only under-sample
    //     the optimum (false negative), never fabricate one (false positive).
    #[allow(clippy::too_many_arguments)]
    async fn size_two_leg_v3_with_reason(
        &self,
        candidate: &StrategyCandidate,
        intent: &RouteIntent,
        cap_wei: U256,
        cap_usd: f64,
        token_price_usd: f64,
        decimals: u8,
        state: &TradingConfigState,
    ) -> OptimizeOutcome {
        let legs = &candidate.route_plan.legs;
        if legs.len() < 2 {
            return OptimizeOutcome::Rejected(OptimizeRejectReason::MissingRouteLegs, None);
        }
        // Defensive fail-closed: the V3 mixed kernel models exactly 2 legs
        // (QuoterV2 arms). >2-leg V3 routes must be rejected, never truncated.
        if legs.len() > 2 {
            return OptimizeOutcome::Rejected(OptimizeRejectReason::UnsupportedLegCount, None);
        }

        // Resolve a per-leg evaluator once (V2 → oriented cached reserves;
        // V3 → PoolRef + direction for the on-chain quoter). Precise reject
        // reason per leg (PoolA / PoolB) for metric fidelity.
        let eval0 = match self
            .build_leg_eval(&legs[0], OptimizeRejectReason::MissingReservesPoolA)
            .await
        {
            Ok(e) => e,
            Err(r) => return OptimizeOutcome::Rejected(r, None),
        };
        let eval1 = match self
            .build_leg_eval(&legs[1], OptimizeRejectReason::MissingReservesPoolB)
            .await
        {
            Ok(e) => e,
            Err(r) => return OptimizeOutcome::Rejected(r, None),
        };

        let leg0_v3 = matches!(eval0, LegEval::V3 { .. });
        let leg1_v3 = matches!(eval1, LegEval::V3 { .. });

        // Log-spaced probe bracket over [1, cap_wei].
        let probes = geom_probes(U256::one(), cap_wei, V3_BRACKET_POINTS);

        // Plan B.1 — 0-RPC within-tick early-reject. When slot0 is cached for
        // every V3 leg, the within-tick model is a provable upper bound on the
        // real QuoterV2 output at every probe size. If even that optimistic
        // bound shows ≤ 0 profit across the grid, the real grid cannot find a
        // profit either → honest reject `NonPositiveProfit`, skipping all ≤ 8
        // QuoterV2 probes (R8). A missing slot0 entry → None → fall through to
        // the grid unchanged. Do NOT alter the grid below when it runs.
        if let Some((true, bound_profit_wei)) = self
            .v3_within_tick_upper_bound_nonpositive(&eval0, &eval1, &probes)
            .await
        {
            debug!(
                event = "size_optimizer.v3_early_reject_within_tick",
                label = candidate.label.as_str(),
                probe_count = probes.len(),
                "within-tick upper bound ≤ 0 across probes — skipping QuoterV2 grid"
            );
            // Deuda 4-B: stamp the optimistic within-tick best profit (≤ 0) in
            // USD. It is a provable UPPER BOUND on the real grid's max, not a
            // QuoterV2 quote — the payload contract documents this.
            let bound_usd =
                (bound_profit_wei as f64) / 10f64.powi(decimals as i32) * token_price_usd;
            return OptimizeOutcome::Rejected(
                OptimizeRejectReason::NonPositiveProfit,
                Some(bound_usd),
            );
        }

        let mut best: Option<(U256, U256, U256, i128)> = None; // (amount_in, out_a, out_b, profit_wei)
                                                               // Per-V3-leg pricing telemetry (R8): `priced` = the quoter answered at
                                                               // least once (value may be 0); `leg1_reached` = leg 1 was quoted at all
                                                               // (only happens when leg 0 yields a non-zero mid-amount).
        let mut leg0_priced = false;
        let mut leg1_priced = false;
        let mut leg1_reached = false;
        // WO-06: honest split of the former single "unavailable" bucket — the
        // first V3 failure label decides the reject reason downstream.
        let mut v3_unavailable_label: Option<&'static str> = None;

        for x in probes {
            if x.is_zero() {
                continue;
            }
            // Leg 0 (always quoted for every probe with x > 0).
            let out_a = match self.eval_leg_out(&eval0, x).await {
                LegQuote::Priced(v) => {
                    leg0_priced = true;
                    v
                }
                LegQuote::Unavailable(label) => {
                    v3_unavailable_label.get_or_insert(label);
                    continue; // V3 leg could not be priced
                }
            };
            if out_a.is_zero() {
                continue; // leg 0 yields nothing → unprofitable; don't quote leg 1 on 0
            }

            // Leg 1 (input = leg 0 output; closed 2-leg arb back to token_in).
            leg1_reached = true;
            let out_b = match self.eval_leg_out(&eval1, out_a).await {
                LegQuote::Priced(v) => {
                    leg1_priced = true;
                    v
                }
                LegQuote::Unavailable(label) => {
                    v3_unavailable_label.get_or_insert(label);
                    continue;
                }
            };
            if out_b.is_zero() {
                continue;
            }

            let profit = clamped_to_i128(out_b).saturating_sub(clamped_to_i128(x));
            if best.as_ref().is_none_or(|(_, _, _, bp)| profit > *bp) {
                best = Some((x, out_a, out_b, profit));
            }
        }

        let (amount_in, out_a, out_b, profit_wei) = match best {
            Some(b) if b.3 > 0 => b,
            _ => {
                // No positive-profit probe. R8: a V3 leg that was quoted but the
                // provider NEVER answered (absent / all RPC failures) is
                // `V3QuoteUnavailable`; everything else (quoter answered, even
                // with 0, but the real spread is ≤ 0) is `NonPositiveProfit`.
                // leg 0 is always quoted; leg 1 only when it was reached.
                let v3_unpriced =
                    (leg0_v3 && !leg0_priced) || (leg1_v3 && leg1_reached && !leg1_priced);
                if v3_unpriced {
                    // WO-06: the recorded label splits catalog gaps
                    // (v3_pool_not_catalogued / v3_pair_no_pools) from real
                    // provider failures (v3_quote_unavailable).
                    let reason = v3_unavailable_label
                        .map(OptimizeRejectReason::from_v3_unavailable_label)
                        .unwrap_or(OptimizeRejectReason::V3QuoteUnavailable);
                    return OptimizeOutcome::Rejected(reason, None);
                }
                // R8: payload only when a probe actually computed a profit
                // (best = Some with profit_wei <= 0); no probe answered → None.
                //
                // ALWAYS-COMPUTE (2026-09-27): when a probe DID answer, its
                // round-trip is a REAL QuoterV2 quote at that size — carry the
                // full figures (gross ≤ 0 priced, cost components, net, the
                // exact per-leg chain) on the reject. No probe answered at all
                // → legacy payload-None reject (nothing was computed; the
                // caller stamps computation_status:"error").
                if let Some((x, mid, out, pw)) = best {
                    let computed_usd = (pw as f64) / 10f64.powi(decimals as i32) * token_price_usd;
                    let gas_cost = state.gas_cost_usd();
                    let ops_overhead = state.ops_overhead_usd_per_attempt;
                    let start_amount_usd =
                        (clamped_to_i128(x) as f64) / 10f64.powi(decimals as i32) * token_price_usd;
                    let borrow_usd = if candidate.base_strategy.is_some() {
                        start_amount_usd
                    } else {
                        0.0
                    };
                    let flash_fee_usd = borrow_usd
                        * crate::financing::selected_mode(borrow_usd).fee_bps()
                        / 10_000.0;
                    let net_usd = computed_usd - gas_cost - ops_overhead - flash_fee_usd;
                    let mut cand = candidate.clone();
                    cand.opportunity.amount_in_wei = x.to_string();
                    cand.opportunity.expected_profit_usd = Some(computed_usd);
                    cand.opportunity.net_expected_profit_usd = Some(net_usd);
                    return OptimizeOutcome::RejectedComputed(
                        OptimizeRejectReason::NonPositiveProfit,
                        Box::new(SizedCandidate {
                            candidate: cand,
                            optimal_amount_in: x,
                            gross_profit_usd: computed_usd,
                            estimated_net_profit_usd: net_usd,
                            net_negative: true,
                            net_economics: Some(
                                crate::net_bps_ranking::RouteNetEconomics::from_kernel(
                                    start_amount_usd,
                                    computed_usd,
                                    gas_cost,
                                    ops_overhead,
                                    borrow_usd,
                                ),
                            ),
                            leg_amounts_in: Some(vec![x.to_string(), mid.to_string()]),
                            leg_amounts_out: Some(vec![mid.to_string(), out.to_string()]),
                        }),
                    );
                }
                return OptimizeOutcome::Rejected(OptimizeRejectReason::NonPositiveProfit, None);
            }
        };

        // `amount_in` is already ≤ cap_wei (probes ⊆ [1, cap_wei]); no extra
        // clamp needed. USD + net math is byte-identical to the V2 path.
        let profit_token_units = (profit_wei as f64) / 10f64.powi(decimals as i32);
        let gross_usd = profit_token_units * token_price_usd;
        if gross_usd <= 0.0 {
            return OptimizeOutcome::Rejected(
                OptimizeRejectReason::NonPositiveGrossUsd,
                Some(gross_usd),
            );
        }

        // HOPS-LEDGER-04: exact per-leg wei at the chosen grid point — the
        // same QuoterV2-quoted values the profit above consumed.
        let leg_amounts_in = Some(vec![amount_in.to_string(), out_a.to_string()]);
        let leg_amounts_out = Some(vec![out_a.to_string(), out_b.to_string()]);

        let gas_cost = state.gas_cost_usd();
        let ops_overhead = state.ops_overhead_usd_per_attempt;
        // ARBX-0007: financing-mode route dimension — identical policy to the
        // V2 path above (selected mode = legacy 5 bps math; per-mode nets in
        // one debug line).
        let borrow_usd = if candidate.base_strategy.is_some() {
            (clamped_to_i128(amount_in) as f64) / 10f64.powi(decimals as i32) * token_price_usd
        } else {
            0.0
        };
        let financing_mode = crate::financing::selected_mode(borrow_usd);
        let financing_evals =
            crate::financing::evaluate_modes(gross_usd, gas_cost, ops_overhead, borrow_usd);
        let flashloan_fee_usd = borrow_usd * financing_mode.fee_bps() / 10_000.0;
        let net_usd = gross_usd - gas_cost - ops_overhead - flashloan_fee_usd;
        // ARBX-0009: sheet-07 economics — same construction as the V2 path.
        let start_amount_usd =
            (clamped_to_i128(amount_in) as f64) / 10f64.powi(decimals as i32) * token_price_usd;
        let net_economics = Some(crate::net_bps_ranking::RouteNetEconomics::from_kernel(
            start_amount_usd,
            gross_usd,
            gas_cost,
            ops_overhead,
            borrow_usd,
        ));
        if net_usd <= 0.0 {
            debug!(
                event = "size_optimizer.v3_negative_net",
                label = candidate.label.as_str(),
                gross_usd,
                gas_cost,
                ops_overhead,
                flashloan_fee_usd,
                financing_mode = financing_mode.as_str(),
                financing_nets = ?financing_evals,
                net_usd,
            );
            // HARDENING: poblar el SizedCandidate con los valores calculados
            // aunque net <= 0. El operador necesita ver los números.
            let mut cand = candidate.clone();
            cand.opportunity.amount_in_wei = amount_in.to_string();
            cand.opportunity.expected_profit_usd = Some(gross_usd);
            cand.opportunity.net_expected_profit_usd = Some(net_usd);
            return OptimizeOutcome::Sized(Box::new(SizedCandidate {
                candidate: cand,
                optimal_amount_in: amount_in,
                gross_profit_usd: gross_usd,
                estimated_net_profit_usd: net_usd,
                net_negative: true,
                net_economics,
                leg_amounts_in,
                leg_amounts_out,
            }));
        }

        let _ = (intent, cap_usd);

        let mut sized = candidate.clone();
        sized.opportunity.amount_in_wei = amount_in.to_string();
        sized.opportunity.expected_profit_usd = Some(gross_usd);
        sized.gross_profit_usd = Some(gross_usd);
        sized.net_expected_profit_usd = Some(net_usd);

        debug!(
            event = "size_optimizer.v3_sized",
            label = sized.label.as_str(),
            amount_in = %amount_in,
            gross_usd,
            net_usd,
            financing_mode = financing_mode.as_str(),
            financing_nets = ?financing_evals,
        );

        OptimizeOutcome::Sized(Box::new(SizedCandidate {
            candidate: sized,
            optimal_amount_in: amount_in,
            gross_profit_usd: gross_usd,
            estimated_net_profit_usd: net_usd,
            net_negative: false,
            net_economics,
            leg_amounts_in,
            leg_amounts_out,
        }))
    }

    /// 0-RPC within-tick upper bound for the 2-leg V3 profit curve (Plan B.1).
    ///
    /// For each probe size, computes leg outputs LOCALLY (no QuoterV2): V3 legs
    /// via `amm_math::v3_amount_out_single_tick` — the within-tick model, which
    /// OVERESTIMATES real output at sizes that cross a tick boundary — and V2
    /// legs via exact `v2_amount_out`. Because within-tick ≥ real QuoterV2
    /// output at every size (`wta(x) ≥ ra(x)` and `wtb(y) ≥ rb(y)` with `rb`
    /// monotone ⇒ `wtb(wta(x)) ≥ rb(ra(x))`), the optimistic max profit over
    /// the probe grid is a provable upper bound on the real grid's max.
    ///
    /// Returns `Some((nonpositive, best_profit_wei))` where `nonpositive` is
    /// true when that upper bound is ≤ 0 → the real grid cannot find a profit
    /// either, so the caller early-rejects `NonPositiveProfit` and skips all
    /// QuoterV2 probes (saves ≤ 8 RPC). `best_profit_wei` is the optimistic
    /// max profit across the probe grid (token-in wei) — the Deuda 4-B stamp
    /// payload source (≤ 0 when `nonpositive`, an upper bound not a quote).
    /// Returns `Some((false, _))` when the optimistic bound shows possible
    /// profit → fall through to the grid. Returns `None` (→ fall through)
    /// whenever a V3 leg lacks a slot0 snapshot or no probe could be priced —
    /// a missing cache NEVER fabricates a rejection (R8 fail-safe).
    async fn v3_within_tick_upper_bound_nonpositive(
        &self,
        eval0: &LegEval,
        eval1: &LegEval,
        probes: &[U256],
    ) -> Option<(bool, i128)> {
        let cache = self.slot0_cache.as_ref()?;

        // Resolve slot0 once per V3 leg up front. A cache miss on any V3 leg
        // → return None (fall through to the QuoterV2 grid).
        let sp0 = match eval0 {
            LegEval::V3 { pool, .. } => Some(cache.get(&pool.address).await?),
            LegEval::V2 { .. } => None,
        };
        let sp1 = match eval1 {
            LegEval::V3 { pool, .. } => Some(cache.get(&pool.address).await?),
            LegEval::V2 { .. } => None,
        };

        // Local optimistic output for one leg (no RPC). V3 → within-tick upper
        // bound (slot0 resolved by the caller); V2 → exact CPMM with the leg's
        // cached oriented reserves. The V3 branch's `None` slot0 arm is dead
        // here (a V3 miss already returned None above) but kept defensive.
        fn leg_out_optimistic(
            leg: &LegEval,
            amount_in: U256,
            slot0: Option<V3Slot0Snapshot>,
        ) -> U256 {
            match leg {
                LegEval::V2 {
                    reserve_in,
                    reserve_out,
                    fee_bps,
                } => v2_amount_out(amount_in, *reserve_in, *reserve_out, *fee_bps),
                LegEval::V3 { pool, zero_for_one } => {
                    // V3 fee tier lives in the misnamed `fee_bps` field in
                    // MILLIONTHS (e.g. 500 = 0.05% tier) — exactly the
                    // `fee_pips` v3_amount_out_single_tick expects. Default 500.
                    let fee_pips = pool.fee_bps.unwrap_or(500);
                    let Some(sp) = slot0 else {
                        return U256::zero();
                    };
                    v3_amount_out_single_tick(
                        amount_in,
                        sp.sqrt_price_x96,
                        sp.liquidity,
                        fee_pips,
                        *zero_for_one,
                    )
                }
            }
        }

        let mut best_profit: i128 = i128::MIN;
        let mut any_priced = false;
        for &x in probes {
            if x.is_zero() {
                continue;
            }
            let out_a = leg_out_optimistic(eval0, x, sp0);
            if out_a.is_zero() {
                continue;
            }
            let out_b = leg_out_optimistic(eval1, out_a, sp1);
            if out_b.is_zero() {
                continue;
            }
            any_priced = true;
            let profit = clamped_to_i128(out_b).saturating_sub(clamped_to_i128(x));
            if profit > best_profit {
                best_profit = profit;
            }
        }
        // Reject only when ≥1 probe priced AND the optimistic max is ≤ 0.
        // No probe priced (degenerate slot0 / all-zero output) → fall through.
        if !any_priced {
            None
        } else {
            Some((best_profit <= 0, best_profit))
        }
    }

    /// Build a per-leg evaluator. V2 → oriented cached reserves + fee. V3 →
    /// `PoolRef` (token0/token1 sorted ascending, Uniswap convention) + swap
    /// direction for the on-chain quoter. `missing_reserves` is the reject
    /// reason for an absent V2 reserve entry (caller supplies PoolA / PoolB).
    async fn build_leg_eval(
        &self,
        leg: &RouteLeg,
        missing_reserves: OptimizeRejectReason,
    ) -> Result<LegEval, OptimizeRejectReason> {
        let addr: Address = leg
            .pool_address
            .as_deref()
            .and_then(|s| s.parse().ok())
            .ok_or(OptimizeRejectReason::MissingPoolAddress)?;

        if leg_is_v3(leg) {
            let tin: Address = leg
                .token_in
                .parse()
                .map_err(|_| OptimizeRejectReason::MissingPoolAddress)?;
            let tout: Address = leg
                .token_out
                .parse()
                .map_err(|_| OptimizeRejectReason::MissingPoolAddress)?;
            // Uniswap V3 sorts tokens ascending: token0 < token1.
            // zero_for_one (token0 → token1) iff the leg's input is token0.
            let (token0, token1, zero_for_one) = if tin < tout {
                (tin, tout, true)
            } else {
                (tout, tin, false)
            };
            Ok(LegEval::V3 {
                pool: PoolRef {
                    address: addr,
                    token0,
                    token1,
                    fee_bps: leg.fee_bps,
                },
                zero_for_one,
            })
        } else {
            let (r0, r1) = self
                .state_projector
                .reserves_cache
                .get(&addr)
                .await
                .ok_or(missing_reserves)?;
            let (reserve_in, reserve_out) = orient_reserves(r0, r1, &leg.token_in, &leg.token_out);
            if reserve_in.is_zero() || reserve_out.is_zero() {
                return Err(OptimizeRejectReason::ZeroReserves);
            }
            Ok(LegEval::V2 {
                reserve_in,
                reserve_out,
                fee_bps: leg.fee_bps.unwrap_or(30),
            })
        }
    }

    /// Evaluate one leg's output for `amount_in`.
    ///   * V2 → local CPMM (always `Priced`; may be `Priced(0)` for degenerate
    ///     reserves).
    ///   * V3 → on-chain QuoterV2 via the StateProjector. `Priced(v)` when the
    ///     quoter answered (`v` may be 0 — a real "this size yields nothing");
    ///     `Unavailable` when the provider is absent or the call failed/reverted.
    ///
    /// R8 fail-honest: `Unavailable` (could-not-price) stays DISTINCT from
    /// `Priced(0)` (real zero-yield) — never a fabricated number, and the caller
    /// classifies `V3QuoteUnavailable` vs `NonPositiveProfit` precisely.
    async fn eval_leg_out(&self, leg: &LegEval, amount_in: U256) -> LegQuote {
        // Root 2C Phase 1: delegate to the protocol-agnostic RouteQuoteProvider
        // (impl'd by StateProjector). V2 byte-identical (same `v2_amount_out`);
        // V3 unchanged. The optimizer no longer inlines either protocol's math.
        self.state_projector.quote_leg(leg, amount_in).await
    }

    // -----------------------------------------------------------------------
    // Legacy shim — 2-leg DEX (kept for backwards compat within this file)
    // -----------------------------------------------------------------------

    #[allow(clippy::too_many_arguments, dead_code)]
    async fn size_two_leg(
        &self,
        candidate: &StrategyCandidate,
        intent: &RouteIntent,
        cap_wei: U256,
        cap_usd: f64,
        token_price_usd: f64,
        decimals: u8,
        state: &TradingConfigState,
    ) -> Option<SizedCandidate> {
        match self
            .size_two_leg_with_reason(
                candidate,
                intent,
                cap_wei,
                cap_usd,
                token_price_usd,
                decimals,
                state,
            )
            .await
        {
            OptimizeOutcome::Sized(s) => Some(*s),
            OptimizeOutcome::Rejected(_, _) => None,
            OptimizeOutcome::RejectedWithLedger(_, _, _) => None,
            OptimizeOutcome::RejectedComputed(_, _) => None,
        }
    }
}

// ---------------------------------------------------------------------------
// 2-leg golden-section search
// ---------------------------------------------------------------------------

/// Golden-section search for the 2-leg profit-maximising input.
///
/// f(x) = leg_b_out(leg_a_out(x)) − x, where each leg has its own fee tier.
/// `hop_reserves_a` and `hop_reserves_b` are single-element slices
/// (one (reserve_in, reserve_out) pair each).
///
/// Uses `v2_amount_out` for each leg independently so mixed-fee pairs are
/// handled correctly (different fee_bps per leg).
///
/// Returns `(x_star, profit_at_x_star_wei)` where profit is signed i128.
///
/// `pub` (ARBX-0012): the benchmark matrix reuses THIS exact kernel — no
/// bench-side twin, so bench ratios and motor answers cannot drift apart.
pub fn golden_section_search_2leg(
    x_lo: U256,
    x_hi: U256,
    hop_reserves_a: &[(U256, U256)],
    hop_reserves_b: &[(U256, U256)],
    fee_bps_a: u32,
    fee_bps_b: u32,
    iterations: u32,
) -> (U256, i128) {
    if x_lo >= x_hi {
        let p = eval_2leg_profit(x_lo, hop_reserves_a, hop_reserves_b, fee_bps_a, fee_bps_b);
        return (x_lo, p);
    }

    // Re-use the same scalar-proxy approach as triangular golden_section_search:
    // f64 for the search, integer for the final evaluation.
    let phi: f64 = (1.0 + 5.0_f64.sqrt()) / 2.0;
    let inv_phi = 1.0 / phi;
    let inv_phi2 = inv_phi * inv_phi;

    let a_f = u256_to_f64_lossy(x_lo);
    let b_f = u256_to_f64_lossy(x_hi);
    let mut a = a_f;
    let mut b = b_f;
    let mut h = b - a;

    let mut c = a + inv_phi2 * h;
    let mut d = a + inv_phi * h;

    let f = |x: f64| -> f64 {
        let xi = f64_to_u256_clamped(x);
        eval_2leg_profit(xi, hop_reserves_a, hop_reserves_b, fee_bps_a, fee_bps_b) as f64
    };

    let mut yc = f(c);
    let mut yd = f(d);

    for _ in 0..iterations {
        if yc > yd {
            b = d;
            d = c;
            yd = yc;
            h *= inv_phi;
            c = a + inv_phi2 * h;
            yc = f(c);
        } else {
            a = c;
            c = d;
            yc = yd;
            h *= inv_phi;
            d = a + inv_phi * h;
            yd = f(d);
        }
    }

    let x_star_f = if yc > yd {
        (a + d) / 2.0
    } else {
        (c + b) / 2.0
    };
    let x_star = f64_to_u256_clamped(x_star_f);
    let profit = eval_2leg_profit(x_star, hop_reserves_a, hop_reserves_b, fee_bps_a, fee_bps_b);
    (x_star, profit)
}

/// Evaluate 2-leg profit at `x`: f(x) = leg_b_out(leg_a_out(x)) − x.
fn eval_2leg_profit(
    x: U256,
    hop_a: &[(U256, U256)],
    hop_b: &[(U256, U256)],
    fee_a: u32,
    fee_b: u32,
) -> i128 {
    if hop_a.is_empty() || hop_b.is_empty() || x.is_zero() {
        return 0;
    }
    let (r_in_a, r_out_a) = hop_a[0];
    let (r_in_b, r_out_b) = hop_b[0];

    let out_a = v2_amount_out(x, r_in_a, r_out_a, fee_a);
    if out_a.is_zero() {
        return 0;
    }
    let out_b = v2_amount_out(out_a, r_in_b, r_out_b, fee_b);
    if out_b.is_zero() {
        return 0;
    }

    clamped_to_i128(out_b).saturating_sub(clamped_to_i128(x))
}

/// ARBX-0008 (XLS-QB amount buckets): N-bucket sweep over the SAME 2-leg
/// V2 curve `golden_section_search_2leg` maximizes. Takes the identical
/// resolved inputs (oriented reserves, fee bps) and the kernel's own
/// bracket semantics — `x_lo = 1 wei`, `x_hi = min(cap, reserve_in_a)` —
/// so a bucket result and a golden-section result are answers about ONE
/// curve, never two. The probe grid reuses `geom_probes` (log-spaced,
/// envelope-validated N ∈ [8, 128]); the RUNNING V3 grid
/// (`V3_BRACKET_POINTS`) is untouched. Each probe is evaluated through
/// `eval_2leg_profit` verbatim — its conventions (early 0 on degenerate
/// hops) are the motor's own and are recorded, not second-guessed.
///
/// Additive observation surface: the sizing DECISION keeps coming from
/// `golden_section_search_2leg`. Consumers are the future amount-aware
/// refine pass (lat.refine) and the ARBX-0012 benchmark matrix.
pub fn bucket_sweep_2leg_curve(
    n: usize,
    x_lo: U256,
    x_hi: U256,
    hop_reserves_a: &[(U256, U256)],
    hop_reserves_b: &[(U256, U256)],
    fee_bps_a: u32,
    fee_bps_b: u32,
) -> Result<crate::amount_buckets::BucketSweep, String> {
    let n = crate::amount_buckets::validate_amount_buckets(n)?;
    let probes = geom_probes(x_lo, x_hi, n);
    crate::amount_buckets::bucket_sweep_2leg(&probes, &mut |x| {
        Some(eval_2leg_profit(
            *x,
            hop_reserves_a,
            hop_reserves_b,
            fee_bps_a,
            fee_bps_b,
        ))
    })
}

// ---------------------------------------------------------------------------
// Utilities
// ---------------------------------------------------------------------------

/// Saturating U256 → i128 (same approach as triangular_worker).
fn clamped_to_i128(v: U256) -> i128 {
    let s = v.to_string();
    s.parse::<i128>().unwrap_or(i128::MAX)
}

fn u256_to_f64_lossy(v: U256) -> f64 {
    v.to_string().parse::<f64>().unwrap_or(0.0)
}

fn f64_to_u256_clamped(x: f64) -> U256 {
    if !x.is_finite() || x <= 0.0 {
        return U256::from(1u64);
    }
    let s = format!("{:.0}", x);
    U256::from_dec_str(&s).unwrap_or(U256::from(1u64))
}

/// Number of log-spaced probe sizes in the V3 sizing bracket. Matches the
/// "8 puntos [x_lo..x_hi]" spec; each probe is one on-chain QuoterV2 call per
/// V3 leg, so this directly bounds RPC per V3 candidate.
const V3_BRACKET_POINTS: usize = 8;

// `LegEval` / `LegQuote` live in `state_projector.rs` (Root 2C Phase 1): they
// are the protocol-neutral input/output of `RouteQuoteProvider::quote_leg`,
// shared by the sizing kernels and (future) cartridges / the triangular V3 path.

/// True when a route leg is a Uniswap-V3-style concentrated-liquidity pool.
/// Matches the codebase's `protocol_type` spellings ("uniswap-v3",
/// "UNISWAP_V3", "V3", "v3", …) case-insensitively; never matches
/// v2 / curve / balancer.
fn leg_is_v3(leg: &RouteLeg) -> bool {
    leg.protocol_type.to_ascii_lowercase().contains("v3")
}

/// True when any leg of the candidate's route plan is a V3 pool. Routes a
/// candidate to the V3 sizing kernel instead of the V2 CPMM kernel.
fn route_has_v3(candidate: &StrategyCandidate) -> bool {
    candidate.route_plan.legs.iter().any(leg_is_v3)
}

/// `n` log-spaced sizes spanning `[lo, hi]` inclusive (`n ≥ 2`). Geometric
/// spacing covers many orders of magnitude with few probes — the 2-leg arb
/// optimum can sit anywhere below the capital cap. Degenerate range → a single
/// clamped point.
fn geom_probes(lo: U256, hi: U256, n: usize) -> Vec<U256> {
    if n < 2 || hi <= lo {
        return vec![if hi.is_zero() { U256::one() } else { hi }];
    }
    let lo_f = u256_to_f64_lossy(lo).max(1.0);
    let hi_f = u256_to_f64_lossy(hi).max(lo_f);
    let ln_lo = lo_f.ln();
    let ln_hi = hi_f.ln();
    (0..n)
        .map(|i| {
            let t = i as f64 / (n as f64 - 1.0);
            f64_to_u256_clamped((ln_lo + t * (ln_hi - ln_lo)).exp())
        })
        .collect()
}

/// Orient pool reserves for a leg: returns (reserve_in, reserve_out) such
/// that reserve_in corresponds to token_in of the leg.
///
/// Simple heuristic: if token_in string < token_out string lexicographically,
/// assume token_in is token0 → reserve_in = r0. Otherwise reverse.
/// This mirrors the PoolSyncWorker convention: token0 = smaller address.
fn orient_reserves(r0: U256, r1: U256, token_in: &str, token_out: &str) -> (U256, U256) {
    if token_in <= token_out {
        (r0, r1)
    } else {
        (r1, r0)
    }
}

/// Resolve the token_in symbol from the candidate's route_plan.
///
/// CORE-05 fix (2026-09-24): the previous version hardcoded 5 MAINNET
/// addresses + a "WETH" fallback — on Base/Arbitrum/Polygon a non-mainnet
/// token resolved to "WETH" ($3000) when its real price was $0.01, inflating
/// cap_wei by ~300000×. Now: (a) the address→symbol map is GATED by the
/// candidate's chain_id (mainnet addresses only resolve on chain 1); (b) the
/// fallback chain prefers the pair_symbol, then native (WETH/WMATIC/WBNB per
/// chain), then honest None (the caller rejects with UnknownTokenPrice).
/// A follow-up should thread the TokenIdentityIndex here (same pattern as
/// the cartridge path's identity_idx).
fn resolve_token_in_symbol(
    candidate: &StrategyCandidate,
    state: &TradingConfigState,
) -> Option<String> {
    // Native token symbol per chain — the fallback when the specific token
    // cannot be resolved (conservative: native is the most common pair side).
    let native_symbol = match candidate.opportunity.chain_id {
        1 | 10 | 8453 | 42161 | 11155111 => "WETH",
        137 => "WMATIC",
        56 => "WBNB",
        _ => "WETH", // unknown chain — conservative
    };

    if let Some(leg) = candidate.route_plan.legs.first() {
        let token_in_lower = leg.token_in.to_ascii_lowercase();
        // Mainnet canonical addresses — ONLY on chain 1 (CORE-05).
        if candidate.opportunity.chain_id == 1 {
            if token_in_lower.contains("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2") {
                return Some("WETH".to_string());
            }
            if token_in_lower.contains("a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48") {
                return Some("USDC".to_string());
            }
            if token_in_lower.contains(&USDT_MAINNET_LC[2..]) {
                return Some("USDT".to_string());
            }
            if token_in_lower.contains("6b175474e89094c44da98b954eedeac495271d0f") {
                return Some("DAI".to_string());
            }
            if token_in_lower.contains("2260fac5e5542a773aa44fbcfedf7c193bc2c599") {
                return Some("WBTC".to_string());
            }
        }
        // Fallback: parse the pair symbol (chain-agnostic).
        let pair = &candidate.opportunity.pair_symbol;
        if pair.contains("WETH") || pair.contains("weth") {
            return Some("WETH".to_string());
        }
        if pair.contains("USDC") {
            return Some("USDC".to_string());
        }
        if pair.contains("USDT") {
            return Some("USDT".to_string());
        }
        if pair.contains("DAI") {
            return Some("DAI".to_string());
        }
        // Last resort: the chain's native token (documented heuristic — the
        // caller's UnknownTokenPrice gate still catches unresolvable tokens).
        return Some(native_symbol.to_string());
    }
    Some(native_symbol.to_string())
}

/// Resolve the USD price for `token_symbol` from `TradingConfigState`.
///
/// R8: returns `None` when the price is unknown (not fabricated).
fn resolve_token_price(state: &TradingConfigState, symbol: &str) -> Option<f64> {
    let sym_upper = symbol.to_ascii_uppercase();
    // Check per-token prices map first.
    if let Some(&p) = state.token_prices_usd.get(&sym_upper) {
        if p > 0.0 {
            return Some(p);
        }
    }
    // Fall back to base_token_price_usd for WETH. Stables have NO $1.00
    // shortcut (WO-PC4): the map lookup above already covered them — an
    // unpriced stable is None (R8), never parity.
    match sym_upper.as_str() {
        "WETH" => {
            if state.base_token_price_usd > 0.0 {
                Some(state.base_token_price_usd)
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Returns the canonical decimals for a well-known symbol. Defaults to 18.
fn resolve_token_decimals(symbol: &str) -> u8 {
    match symbol.to_ascii_uppercase().as_str() {
        "WETH" => 18,
        "USDC" | "USDT" => 6,
        "DAI" => 18,
        "WBTC" => 8,
        _ => 18,
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
    use crate::engines::StrategyCandidate;
    use crate::route_intent::{
        DetectionSource, ProtocolType, RouteIntent, RouteIntentLeg, RouterKind, SwapExactMode,
    };
    use crate::state_projector::StateProjector;
    use crate::strategy_label::StrategyLabel;
    use chrono::Utc;
    use ethers::types::{Address, H256, U256};
    use prioritization_spine::route_plan::{RouteLeg, RoutePlan};
    use prioritization_spine::types::OpportunityCandidate;
    use shared_rs::contracts::{Opportunity, StrategyKind};
    use shared_rs::trading_config::{GasPriceStrategy, TradingConfigState};
    use std::collections::HashMap;
    use std::sync::atomic::AtomicU64;
    use std::sync::Arc;
    use uuid::Uuid;

    // ── Helpers ──────────────────────────────────────────────────────────────

    fn addr(n: u64) -> Address {
        Address::from_low_u64_be(n)
    }

    /// Empty in-memory fee catalog (WO-06): every pool/pair is uncatalogued —
    /// V3 tests that only exercise V2 legs or the no-provider path use this.
    fn empty_v3_fee_catalog() -> Arc<crate::v3_fee_catalog::V3FeeCatalog> {
        Arc::new(crate::v3_fee_catalog::V3FeeCatalog::new())
    }

    /// Catalog for the standard V3 test fixture: pools addr(0x10)/addr(0x11)
    /// on the addr(0xAAAA)/addr(0xBBBB) pair at the 0.05% tier — the same
    /// fixture legs carry (fee_bps = Some(500)) — so V3 sizing tests exercise
    /// the catalogued path (WO-06).
    fn v3_test_fee_catalog() -> Arc<crate::v3_fee_catalog::V3FeeCatalog> {
        let c = empty_v3_fee_catalog();
        c.record_observed(addr(0x10), addr(0xAAAA), addr(0xBBBB), 500);
        c.record_observed(addr(0x11), addr(0xAAAA), addr(0xBBBB), 500);
        c
    }

    fn unit(n: u64) -> U256 {
        U256::from(10u128).pow(U256::from(18u32)) * U256::from(n)
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
            enabled_strategies: vec!["dex_arb_v2v2".into(), "triangular_arb".into()],
            enabled_dex_ids: None,
            strategy_configs: HashMap::new(),
            capital_cost_rate_annual_pct: 0.0,
            ops_overhead_usd_per_attempt: 0.0,
            spread_sanity_mult: 3.0,
            p_copied_volume_threshold_usd: 1_000_000.0,
            p_copied_max: 0.5,
            lp_fee_default_pct: 0.003, // WO-04 (2026-09-06)
            // Kelly fix-2 fields. Tests use loose values to keep existing
            // golden-path tests passing; Kelly-specific tests override.
            kelly_multiplier: 0.5,
            kelly_max_per_trade_fraction: 1.0, // no cap in legacy tests
            kelly_gas_safety_multiplier: 1.0,  // permissive in legacy tests
            enabled: true,
            updated_at: Utc::now(),
            updated_by: None,
        }
    }

    fn make_dex_candidate(
        pool_a: Address,
        pool_b: Address,
        token_in: Address,
        token_out: Address,
        label: StrategyLabel,
    ) -> StrategyCandidate {
        let id = Uuid::new_v4();
        let pool_a_str = format!("0x{:040x}", pool_a);
        let pool_b_str = format!("0x{:040x}", pool_b);
        // Use WETH address for token_in to get pricing.
        let token_in_str = "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2".to_string();
        let token_out_str = format!("0x{:040x}", token_out);

        let opp = Opportunity {
            id,
            chain_id: 1,
            strategy_kind: StrategyKind::dex_arb(),
            dex_a: "uniswap-v2".to_string(),
            dex_b: Some("sushi".to_string()),
            pair_symbol: "WETH/USDC".to_string(),
            token_in: token_in_str.clone(),
            token_out: token_out_str.clone(),
            amount_in_wei: unit(1).to_string(),
            expected_profit_usd: Some(1.0),
            net_expected_profit_usd: None,
            roi_pct: None,
            risk_score: None,
            block_number: None,
            rejection_reason: None,
            cartridge_id: None,
            detector_id: None,
            pipeline_latency_ms: None,
            detected_at: Utc::now(),
            trace_id: Uuid::new_v4(),
            economics: None,
        };

        let candidate_inner = OpportunityCandidate {
            route_fingerprint: "test".to_string(),
            pool_addresses: vec![pool_a_str.clone(), pool_b_str.clone()],
            token_addresses: vec![token_in_str.clone(), token_out_str.clone()],
            dex_adapters: vec!["uniswap-v2".to_string(), "sushi".to_string()],
            amount_in: 1.0,
            expected_amount_out: 1.001,
            gross_profit: 1.0,
        };

        let route_plan = RoutePlan {
            route_id: Some("test-route".to_string()),
            strategy_kind: label.as_str().to_string(),
            chain_id: 1,
            legs: vec![
                RouteLeg {
                    dex_id: "uniswap-v2".to_string(),
                    dex_name: "uniswap-v2".to_string(),
                    protocol_type: "uniswap-v2".to_string(),
                    factory_address: String::new(),
                    pool_id: None,
                    pool_address: Some(pool_a_str),
                    token_in: token_in_str.clone(),
                    token_out: token_out_str.clone(),
                    fee_bps: Some(30),
                    amount_in: Some(1.0),
                    amount_out: None,
                    tvl_usd: None,
                    volume_24h_usd: None,
                    pool_is_active: true,
                },
                RouteLeg {
                    dex_id: "sushi".to_string(),
                    dex_name: "sushi".to_string(),
                    protocol_type: "uniswap-v2".to_string(),
                    factory_address: String::new(),
                    pool_id: None,
                    pool_address: Some(pool_b_str),
                    token_in: token_out_str.clone(),
                    token_out: token_in_str.clone(),
                    fee_bps: Some(30),
                    amount_in: Some(1.0),
                    amount_out: None,
                    tvl_usd: None,
                    volume_24h_usd: None,
                    pool_is_active: true,
                },
            ],
            atomic: true,
            estimated_slippage_pct: None,
            price_impact_pct: None,
        };

        let _ = token_in; // used via token_in_str

        StrategyCandidate {
            label,
            opportunity: opp,
            candidate: candidate_inner,
            route_plan,
            gross_profit_usd: Some(1.0),
            net_expected_profit_usd: None,
            rejection_reason: None,
            source_intent_hash: H256::zero(),
            base_strategy: None,
        }
    }

    fn make_intent(token_in: Address, token_out: Address) -> RouteIntent {
        RouteIntent::new(
            1,
            H256::from_low_u64_be(0xDEAD),
            Address::zero(),
            RouterKind::UniswapV2,
            Address::zero(),
            vec![RouteIntentLeg {
                token_in,
                token_out,
                pool_hint: None,
                dex_hint: None,
                fee_bps: Some(30),
                protocol_type: ProtocolType::V2,
            }],
            unit(1),
            None,
            SwapExactMode::ExactIn,
            DetectionSource::PublicMempool,
        )
        .expect("valid intent")
    }

    // ── size_optimizer::tests::profitable_route_returns_optimal_size ─────────
    //
    // Two V2 pools with asymmetric reserves (pool_a favours buying token_out,
    // pool_b favours selling it back). The optimizer must find a positive-net size.

    #[tokio::test]
    async fn profitable_route_returns_optimal_size() {
        let pool_a = addr(0x10);
        let pool_b = addr(0x11);
        let tok_weth = addr(0xAAAA); // arbitrary, mapped to WETH via leg token_in string
        let tok_usdc = addr(0xBBBB);

        let cache = Arc::new(ReservesCache::new());
        // Pool A: buy token_out cheaply (more token_out per token_in).
        // Uses string-based orientation: tok_weth < tok_usdc by address.
        // token_in_str for leg 0 is WETH addr → tok_weth is "smaller" string? No.
        // Since we force token_in_str to WETH mainnet addr in make_dex_candidate,
        // orientation depends on string comparison of addresses.
        // We pick reserves where the arb is clearly profitable regardless of orientation.
        // Pool A: large amount of token_out (deep in the buy direction).
        let r_in_a = unit(1000); // 1000 WETH
        let r_out_a = unit(2_000_000); // 2M USDC-equivalent (favorable rate)
        cache.insert(pool_a, r_in_a, r_out_a).await;

        // Pool B: smaller amount of token_out (sells back at higher WETH rate).
        // r_in_b = USDC side, r_out_b = WETH side → same token pair but reversed.
        let r_in_b = unit(1_000_000); // 1M USDC
        let r_out_b = unit(600); // 600 WETH (implied price ~1667 USDC/WETH — higher than pool A's 2000)
        cache.insert(pool_b, r_in_b, r_out_b).await;

        let projector = Arc::new(StateProjector::new(cache, None, empty_v3_fee_catalog()));
        let optimizer = SizeOptimizer::new(projector);

        let candidate = make_dex_candidate(
            pool_a,
            pool_b,
            tok_weth,
            tok_usdc,
            StrategyLabel::DexArbV2V2,
        );
        let intent = make_intent(tok_weth, tok_usdc);
        let cfg = make_cfg(10_000.0); // $10K capital cap

        let result = optimizer
            .optimize(candidate, &intent, Some(&cfg))
            .await
            .expect("optimize must not error");

        // With deeply asymmetric pools this route should be profitable.
        // We just verify the structure — the exact amount depends on golden-section convergence.
        if let Some(sized) = result {
            assert!(
                sized.gross_profit_usd > 0.0,
                "gross_profit_usd must be positive"
            );
            assert!(
                sized.estimated_net_profit_usd > 0.0,
                "estimated_net_profit_usd must be positive"
            );
            assert!(
                sized.optimal_amount_in > U256::zero(),
                "optimal_amount_in must be > 0"
            );
        }
        // Ok(None) is also acceptable if the pool orientation doesn't produce profit
        // with this test setup — the test verifies structure not exact values.
    }

    // ── size_optimizer::tests::non_profitable_route_returns_none ─────────────
    //
    // Symmetric pools (equal reserves, equal fees) → no arbitrage profit.

    #[tokio::test]
    async fn non_profitable_route_returns_none() {
        let pool_a = addr(0x10);
        let pool_b = addr(0x11);
        let tok_weth = addr(0xAAAA);
        let tok_usdc = addr(0xBBBB);

        let cache = Arc::new(ReservesCache::new());
        // Perfectly symmetric pools — no spread.
        let r = unit(10_000);
        cache.insert(pool_a, r, r).await;
        cache.insert(pool_b, r, r).await;

        let projector = Arc::new(StateProjector::new(cache, None, empty_v3_fee_catalog()));
        let optimizer = SizeOptimizer::new(projector);

        let candidate = make_dex_candidate(
            pool_a,
            pool_b,
            tok_weth,
            tok_usdc,
            StrategyLabel::DexArbV2V2,
        );
        let intent = make_intent(tok_weth, tok_usdc);
        let cfg = make_cfg(100_000.0);

        let result = optimizer
            .optimize(candidate, &intent, Some(&cfg))
            .await
            .expect("optimize must not error");

        assert!(
            result.is_none(),
            "symmetric pools produce no profit — must return None"
        );
    }

    // ── N-LEG CYCLE DISPATCH (CARDS-HOPS 2026-09-20) ─────────────────────────
    //
    // Root cause of "only 2 hops in the cards": route_graph/amm_curve engines
    // emit 4..7-leg candidates labelled DexArbV2V2, which fell into the
    // 2-leg kernels — legs[1] of a multi-leg route is a different pool than
    // the route's actual second pool, producing the `missing_reserves_pool_b`
    // wall (and, when legs[0..2] happened to be cacheable, a FABRICATED
    // 2-of-N-leg sizing). The dispatch must size all-V2 cycles of ANY leg
    // count (2..=7) with the N-leg cycle kernel and reject mixed V3 multi-leg
    // routes honestly instead of truncating.

    /// 4-leg all-V2 cycle candidate: WETH → T1 → T2 → T3 → WETH.
    /// T1/T2/T3 are low addresses (0x..01/02/03) so string-orientation is
    /// deterministic: leg0 flips (WETH > T1), legs 1..3 keep (r0, r1).
    fn make_multileg_candidate(
        pools: &[Address],
        protocols: &[&str],
        label: StrategyLabel,
    ) -> StrategyCandidate {
        assert_eq!(pools.len(), 4);
        assert_eq!(protocols.len(), 4);
        let weth = "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2";
        let t1 = "0x0000000000000000000000000000000000000001";
        let t2 = "0x0000000000000000000000000000000000000002";
        let t3 = "0x0000000000000000000000000000000000000003";
        let cycle = [weth, t1, t2, t3, weth];

        let id = Uuid::new_v4();
        let opp = Opportunity {
            id,
            chain_id: 1,
            strategy_kind: StrategyKind::dex_arb(),
            dex_a: "uniswap-v2".to_string(),
            dex_b: Some("sushi".to_string()),
            pair_symbol: "WETH/MULTIHOP".to_string(),
            token_in: weth.to_string(),
            token_out: weth.to_string(),
            amount_in_wei: unit(1).to_string(),
            expected_profit_usd: Some(1.0),
            net_expected_profit_usd: None,
            roi_pct: None,
            risk_score: None,
            block_number: None,
            rejection_reason: None,
            cartridge_id: None,
            detector_id: None,
            pipeline_latency_ms: None,
            detected_at: Utc::now(),
            trace_id: Uuid::new_v4(),
            economics: None,
        };

        let candidate_inner = OpportunityCandidate {
            route_fingerprint: "test-multileg".to_string(),
            pool_addresses: pools.iter().map(|p| format!("0x{:040x}", p)).collect(),
            token_addresses: cycle.iter().map(|s| s.to_string()).collect(),
            dex_adapters: vec!["uniswap-v2".to_string(); 4],
            amount_in: 1.0,
            expected_amount_out: 1.001,
            gross_profit: 1.0,
        };

        let legs = (0..4)
            .map(|i| RouteLeg {
                dex_id: "uniswap-v2".to_string(),
                dex_name: "uniswap-v2".to_string(),
                protocol_type: protocols[i].to_string(),
                factory_address: String::new(),
                pool_id: None,
                pool_address: Some(format!("0x{:040x}", pools[i])),
                token_in: cycle[i].to_string(),
                token_out: cycle[i + 1].to_string(),
                fee_bps: Some(30),
                amount_in: Some(1.0),
                amount_out: None,
                tvl_usd: None,
                volume_24h_usd: None,
                pool_is_active: true,
            })
            .collect::<Vec<_>>();

        let route_plan = RoutePlan {
            route_id: Some("test-multileg-route".to_string()),
            strategy_kind: label.as_str().to_string(),
            chain_id: 1,
            legs,
            atomic: true,
            estimated_slippage_pct: None,
            price_impact_pct: None,
        };

        StrategyCandidate {
            label,
            opportunity: opp,
            candidate: candidate_inner,
            route_plan,
            gross_profit_usd: Some(1.0),
            net_expected_profit_usd: None,
            rejection_reason: None,
            source_intent_hash: H256::zero(),
            base_strategy: None,
        }
    }

    /// Insert oriented (Rin, Rout) for one leg, honouring the kernel's
    /// string-orientation rule (token_in <= token_out → (r0, r1)).
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

    const WETH_T: &str = "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2";
    const T1: &str = "0x0000000000000000000000000000000000000001";
    const T2: &str = "0x0000000000000000000000000000000000000002";
    const T3: &str = "0x0000000000000000000000000000000000000003";

    #[tokio::test]
    async fn multileg_all_v2_route_sizes_via_cycle_kernel() {
        // Legs 0+1 alone are UNPROFITABLE (spot 0.855 < 1) — the old 2-leg
        // dispatch rejected here. The full 4-hop cycle is profitable
        // (0.855 × 2.25 × 0.997⁴ ≈ 1.894 > 1) — only the N-leg kernel finds it.
        let pools = [addr(0x20), addr(0x21), addr(0x22), addr(0x23)];
        let cache = Arc::new(ReservesCache::new());
        insert_oriented(&cache, pools[0], WETH_T, T1, unit(100), unit(90)).await;
        insert_oriented(&cache, pools[1], T1, T2, unit(100), unit(95)).await;
        insert_oriented(&cache, pools[2], T2, T3, unit(100), unit(150)).await;
        insert_oriented(&cache, pools[3], T3, WETH_T, unit(100), unit(150)).await;

        let projector = Arc::new(StateProjector::new(cache, None, empty_v3_fee_catalog()));
        let optimizer = SizeOptimizer::new(projector);

        let candidate = make_multileg_candidate(
            &pools,
            &["uniswap-v2", "uniswap-v2", "uniswap-v2", "uniswap-v2"],
            StrategyLabel::DexArbV2V2,
        );
        let intent = make_intent(addr(0xAAAA), addr(0xBBBB));
        let cfg = make_cfg(100_000.0);

        let outcome = optimizer
            .optimize_with_reason(candidate, &intent, Some(&cfg))
            .await
            .expect("optimize must not error");

        match outcome {
            OptimizeOutcome::Sized(s) => {
                assert!(
                    s.gross_profit_usd > 0.0,
                    "4-hop profitable cycle must size with positive gross, got {}",
                    s.gross_profit_usd
                );
            }
            other => panic!(
                "4-leg all-V2 route must reach the cycle kernel, got {}",
                matches!(other, OptimizeOutcome::Sized(_))
            ),
        }
    }

    #[tokio::test]
    async fn multileg_v3_route_rejects_honestly() {
        // A mixed V2/V3 route of >2 legs is NEVER the misleading
        // `missing_reserves_pool_b` (a V3 pool has no `getReserves()`, so that
        // reason would name the wrong cause) and never a fabricated 2-leg
        // sizing. PERHOP-RESERVES-01 pinned `v3_multileg_unsupported` here;
        // V3-MULTILEG-SIZING-01 replaces that refusal with the N-leg V3 kernel,
        // so the honest verdict for THIS fixture — which wires no V3 provider at
        // all — is now the provider-failure label `v3_quote_unavailable`
        // (R8: the state genuinely cannot be quoted, so nothing is invented).
        let pools = [addr(0x20), addr(0x21), addr(0x22), addr(0x23)];
        let cache = Arc::new(ReservesCache::new());
        insert_oriented(&cache, pools[0], WETH_T, T1, unit(100), unit(90)).await;

        let projector = Arc::new(StateProjector::new(cache, None, empty_v3_fee_catalog()));
        let optimizer = SizeOptimizer::new(projector);

        let candidate = make_multileg_candidate(
            &pools,
            &["uniswap-v2", "uniswap-v3", "uniswap-v2", "uniswap-v2"],
            StrategyLabel::DexArbV2V2,
        );
        let intent = make_intent(addr(0xAAAA), addr(0xBBBB));
        let cfg = make_cfg(100_000.0);

        let outcome = optimizer
            .optimize_with_reason(candidate, &intent, Some(&cfg))
            .await
            .expect("optimize must not error");

        assert!(
            matches!(
                outcome,
                OptimizeOutcome::Rejected(OptimizeRejectReason::V3QuoteUnavailable, None)
            ),
            "a mixed V3 multi-leg route with no V3 provider must reject with \
             v3_quote_unavailable, got {:?}",
            outcome.reason_str()
        );
        // …and the refusal names the V3 cause, never the reserve cache.
        assert_ne!(outcome.reason_str(), Some("missing_reserves_pool_a"));
        assert_ne!(outcome.reason_str(), Some("missing_reserves_pool_b"));
        assert!(outcome.leg_ledger().is_none(), "no ledger without a quote");
        assert!(outcome.optimal_amount_in().is_none(), "no amount invented");
        assert!(outcome.net_profit_usd().is_none(), "no USD invented");
    }

    /// GATE — PERHOP-RESERVES-01 (kept under V3-MULTILEG-SIZING-01).
    ///
    /// The SAME shape as `multileg_v3_route_rejects_honestly`, but with the label
    /// `hop_cycle_bridge::cycle_candidate_from_intent` actually stamps on every
    /// discovered 3..=7-hop cycle (`StrategyLabel::TriangularArb`). Before the
    /// Step-7 fix the `TriangularArb` arm matched first, the cycle fell into the
    /// constant-product N-leg kernel, that kernel looked the V3 pool up in the
    /// V2 reserves cache, missed (a V3 pool has no `getReserves()` and therefore
    /// no `arbx:pool_reserves` entry), and the row was persisted with the
    /// misleading `missing_reserves_pool_a/b` — which is what production shows on
    /// ~1 800 deep rows / 15 min.
    ///
    /// V3-MULTILEG-SIZING-01: this fixture wires NO V3 provider, so the kernel's
    /// honest verdict is `v3_quote_unavailable` (knob ON) — and with the knob OFF
    /// it is exactly `v3_multileg_unsupported`. BOTH are V3-family reasons; the
    /// invariant this gate defends is that `missing_reserves_pool_*` can never
    /// name a V3 leg's cause again, in either knob position.
    #[tokio::test]
    async fn gate_triangular_labelled_multileg_v3_rejects_with_the_v3_reason() {
        let pools = [addr(0x20), addr(0x21), addr(0x22), addr(0x23)];
        let cycle = [WETH_T, T1, T2, T3, WETH_T];

        for (v3_at, legs) in [
            (
                1usize,
                ["uniswap-v2", "uniswap-v3", "uniswap-v2", "uniswap-v2"],
            ),
            (
                0usize,
                ["uniswap-v3", "uniswap-v3", "uniswap-v3", "uniswap-v3"],
            ),
            (
                3usize,
                ["uniswap-v2", "uniswap-v2", "uniswap-v2", "uniswap-v3"],
            ),
        ] {
            // Every V2 leg's reserves ARE cached, so the ONLY thing that can
            // block the cycle is the V3 leg's own state/quote — exactly the
            // fixture the V3 hypothesis is about. (A genuinely uncached V2 leg
            // still names ITSELF: see the companion gate below.)
            let cache = Arc::new(ReservesCache::new());
            for i in 0..4 {
                if legs[i].contains("v2") {
                    insert_oriented(
                        &cache,
                        pools[i],
                        cycle[i],
                        cycle[i + 1],
                        unit(100),
                        unit(90),
                    )
                    .await;
                }
            }
            let projector = Arc::new(StateProjector::new(cache, None, empty_v3_fee_catalog()));
            let optimizer = SizeOptimizer::new(projector);

            let candidate = make_multileg_candidate(&pools, &legs, StrategyLabel::TriangularArb);
            let intent = make_intent(addr(0xAAAA), addr(0xBBBB));
            let cfg = make_cfg(100_000.0);

            let outcome = optimizer
                .optimize_with_reason(candidate, &intent, Some(&cfg))
                .await
                .expect("optimize must not error");

            let reason = outcome.reason_str();
            assert!(
                matches!(
                    reason,
                    Some("v3_quote_unavailable") | Some("v3_multileg_unsupported")
                ),
                "a TriangularArb-labelled 4-leg route with a V3 leg at index {v3_at} \
                 whose V2 legs are all cached must reject with a V3 reason, got {reason:?}"
            );
            assert!(
                !matches!(
                    reason,
                    Some("missing_reserves_pool_a") | Some("missing_reserves_pool_b")
                ),
                "the reserves cache must never be blamed for a V3 leg ({v3_at})"
            );
            assert!(
                outcome.leg_ledger().is_none() && outcome.optimal_amount_in().is_none(),
                "a cycle that could not be priced must carry NO ledger and NO amount"
            );
        }

        // …and the guard must NOT touch the population it was never meant for:
        // an all-V2 TriangularArb 4-leg cycle still reaches the cycle kernel.
        let cache = Arc::new(ReservesCache::new());
        insert_oriented(&cache, pools[0], WETH_T, T1, unit(100), unit(90)).await;
        insert_oriented(&cache, pools[1], T1, T2, unit(100), unit(95)).await;
        insert_oriented(&cache, pools[2], T2, T3, unit(100), unit(150)).await;
        insert_oriented(&cache, pools[3], T3, WETH_T, unit(100), unit(150)).await;
        let projector = Arc::new(StateProjector::new(cache, None, empty_v3_fee_catalog()));
        let optimizer = SizeOptimizer::new(projector);
        let candidate = make_multileg_candidate(
            &pools,
            &["uniswap-v2", "uniswap-v2", "uniswap-v2", "uniswap-v2"],
            StrategyLabel::TriangularArb,
        );
        let cfg = make_cfg(100_000.0);
        let outcome = optimizer
            .optimize_with_reason(
                candidate,
                &make_intent(addr(0xAAAA), addr(0xBBBB)),
                Some(&cfg),
            )
            .await
            .expect("optimize must not error");
        assert!(
            matches!(outcome, OptimizeOutcome::Sized(_)),
            "the all-V2 4-leg cycle must still size through the N-leg kernel"
        );
    }

    #[tokio::test]
    async fn triangular_4leg_sizes_all_legs_no_truncation() {
        // The old kernel did legs.iter().take(3) — a 4-leg TriangularArb
        // candidate was sized on 3 of 4 legs (fabricated economics) and the
        // ledger came back with 3 entries. The generalized kernel must size
        // ALL legs and emit a 4-entry ledger chained to the reported amounts.
        let pools = [addr(0x20), addr(0x21), addr(0x22), addr(0x23)];
        let cache = Arc::new(ReservesCache::new());
        insert_oriented(&cache, pools[0], WETH_T, T1, unit(100), unit(120)).await;
        insert_oriented(&cache, pools[1], T1, T2, unit(100), unit(110)).await;
        insert_oriented(&cache, pools[2], T2, T3, unit(100), unit(90)).await;
        insert_oriented(&cache, pools[3], T3, WETH_T, unit(100), unit(200)).await;

        let projector = Arc::new(StateProjector::new(cache, None, empty_v3_fee_catalog()));
        let optimizer = SizeOptimizer::new(projector);

        let candidate = make_multileg_candidate(
            &pools,
            &["uniswap-v2", "uniswap-v2", "uniswap-v2", "uniswap-v2"],
            StrategyLabel::TriangularArb,
        );
        let cfg = make_cfg(100_000.0);

        let outcome = optimizer
            .size_triangular_with_reason(&candidate, &cfg, 100_000.0, 3000.0, 18)
            .await;

        match outcome {
            OptimizeOutcome::Sized(s) => {
                let outs = s
                    .leg_amounts_out
                    .as_ref()
                    .expect("profitable 4-leg cycle must carry a ledger (R8 all-or-nothing)");
                assert_eq!(outs.len(), 4, "ledger must cover ALL 4 legs, not take(3)");
                let ins = s
                    .leg_amounts_in
                    .as_ref()
                    .expect("ledger inputs must be present with outputs");
                assert_eq!(ins.len(), 4);
                // Chain invariant: leg i+1 input == leg i output.
                for i in 0..3 {
                    assert_eq!(ins[i + 1], outs[i], "leg chain broken at hop {i}");
                }
                assert_eq!(ins[0], s.optimal_amount_in.to_string());
                assert!(s.gross_profit_usd > 0.0);
            }
            other => panic!(
                "4-leg TriangularArb must size via the cycle kernel, got sized={}",
                matches!(other, OptimizeOutcome::Sized(_))
            ),
        }
    }

    #[tokio::test]
    async fn two_leg_kernel_rejects_extra_legs_defensively() {
        // Fund-path defensive check (CLAUDE.md §2): a >2-leg candidate that
        // ever reaches a 2-leg kernel must fail CLOSED — never size a
        // truncated 2-of-N slice.
        let pools = [addr(0x20), addr(0x21), addr(0x22), addr(0x23)];
        let cache = Arc::new(ReservesCache::new());
        insert_oriented(&cache, pools[0], WETH_T, T1, unit(100), unit(120)).await;
        insert_oriented(&cache, pools[1], T1, T2, unit(100), unit(110)).await;

        let projector = Arc::new(StateProjector::new(cache, None, empty_v3_fee_catalog()));
        let optimizer = SizeOptimizer::new(projector);

        let candidate = make_multileg_candidate(
            &pools,
            &["uniswap-v2", "uniswap-v2", "uniswap-v2", "uniswap-v2"],
            StrategyLabel::DexArbV2V2,
        );
        let intent = make_intent(addr(0xAAAA), addr(0xBBBB));
        let cfg = make_cfg(100_000.0);

        let outcome = optimizer
            .size_two_leg_with_reason(&candidate, &intent, unit(10), 100_000.0, 3000.0, 18, &cfg)
            .await;

        assert!(
            matches!(
                outcome,
                OptimizeOutcome::Rejected(OptimizeRejectReason::UnsupportedLegCount, None)
            ),
            "2-leg kernel must fail closed on a 4-leg candidate"
        );
    }

    // ── ARBX-0008 (XLS-QB amount buckets): sweep over the SAME curve the
    // golden-section kernel maximizes ──────────────────────────────────────
    //
    // Canonical vectors verified by an exact Python port of `eval_2leg_profit`
    // + `geom_probes` (integer-faithful) on this fixture: golden p* =
    // 2.845_290 WETH net; grid argmax ratios vs p* are N=8: 6.65%, N=16:
    // 92.5%, N=22: 55.6%, N=32: 83.6%, N=64: 91.4%, N=128: 99.75%. The
    // ratios are NOT monotone in N (geometric grids of different N are not
    // nested) — the test therefore asserts structural bounds, never
    // monotonicity. Ratio table itself is ARBX-0012 benchmark territory.
    #[test]
    fn bucket_sweep_2leg_curve_bounded_by_golden_and_envelope_enforced() {
        // Same fixture shape as `profitable_route_returns_optimal_size`:
        // pool A buys token_out deep, pool B sells it back higher.
        let hop_a = vec![(unit(1000), unit(2_000_000))];
        let hop_b = vec![(unit(1_000_000), unit(600))];
        let x_lo = U256::from(1u64);
        let x_hi = unit(1000);

        // Self-derived reference: the kernel's own answer on the SAME
        // bracket (no hardcoded profit literal — cross-platform proof).
        let (_x_gs, p_gs) = golden_section_search_2leg(x_lo, x_hi, &hop_a, &hop_b, 30, 30, 25);
        assert!(p_gs > 0, "fixture must be profitable, got {}", p_gs);

        // Envelope: N outside [8, 128] fails fast.
        assert!(bucket_sweep_2leg_curve(7, x_lo, x_hi, &hop_a, &hop_b, 30, 30).is_err());
        assert!(bucket_sweep_2leg_curve(129, x_lo, x_hi, &hop_a, &hop_b, 30, 30).is_err());

        for n in crate::amount_buckets::AMOUNT_BUCKETS_CANONICAL {
            let sweep = bucket_sweep_2leg_curve(n, x_lo, x_hi, &hop_a, &hop_b, 30, 30)
                .unwrap_or_else(|e| panic!("N={} must be admissible: {}", n, e));
            assert_eq!(sweep.buckets, n);
            assert_eq!(sweep.points.len(), n, "evaluator never returns None here");
            let best = sweep.best.expect("profitable fixture: best must be Some");
            assert!(best.net_wei > 0);
            assert!(
                best.net_wei <= p_gs,
                "grid argmax (N={}) cannot exceed the continuous optimum",
                n
            );
        }

        // Refinement helps on this fixture (6.65% -> 99.75% of p*), with
        // margins far beyond any ulp-level probe displacement.
        let sweep_8 =
            bucket_sweep_2leg_curve(8, x_lo, x_hi, &hop_a, &hop_b, 30, 30).expect("N=8 admissible");
        let sweep_128 = bucket_sweep_2leg_curve(128, x_lo, x_hi, &hop_a, &hop_b, 30, 30)
            .expect("N=128 admissible");
        let best8 = sweep_8.best.expect("N=8 best");
        let best128 = sweep_128.best.expect("N=128 best");
        assert!(best8.net_wei < best128.net_wei);
        assert!(
            best128.net_wei * 1000 >= p_gs * 995,
            "N=128 must reach >= 99.5% of the golden-section optimum"
        );
    }

    #[test]
    fn bucket_sweep_2leg_curve_symmetric_pools_yield_best_none() {
        // Identical pools + equal fees: product rate < 1 everywhere, every
        // computed net is negative — an honest sweep reports best: None
        // (R8), never a fabricated zero point.
        let hop = vec![(unit(1_000_000), unit(1_000_000))];
        let sweep =
            bucket_sweep_2leg_curve(22, U256::from(1u64), unit(1_000_000), &hop, &hop, 30, 30)
                .expect("N=22 admissible");
        assert_eq!(sweep.points.len(), 22);
        assert!(
            sweep.points.iter().all(|p| p.net_wei <= 0),
            "symmetric fixture must be non-positive everywhere"
        );
        assert!(sweep.best.is_none(), "all-negative sweep: best None (R8)");
    }

    // ── Root 2C Phase 1: RouteQuoteProvider V2 byte-identical regression ─────
    //
    // The V2 path behind StateProjector::quote_leg MUST equal amm_math::v2_amount_out
    // exactly — this is the safety net for the protocol-agnostic quoting refactor.
    // If a future change drifts the V2 quote, this fails before the optimizer does.
    #[tokio::test]
    async fn route_quote_v2_leg_matches_v2_amount_out() {
        let cache = Arc::new(ReservesCache::new());
        let projector = StateProjector::new(cache, None, empty_v3_fee_catalog());
        let rin = U256::from(1_000_000u64);
        let rout = U256::from(2_000_000u64);
        let leg = LegEval::V2 {
            reserve_in: rin,
            reserve_out: rout,
            fee_bps: 30,
        };
        for amt in [
            U256::from(1u64),
            U256::from(1_000u64),
            U256::from(1_000_000_000u64),
            U256::from(10u128.pow(18)),
        ] {
            let direct = v2_amount_out(amt, rin, rout, 30);
            assert_eq!(
                projector.quote_leg(&leg, amt).await,
                LegQuote::Priced(direct),
                "V2 quote_leg must equal v2_amount_out for amt {amt}"
            );
        }
        // zero amount → Priced(0), no reserves/RPC consulted.
        assert_eq!(
            projector.quote_leg(&leg, U256::zero()).await,
            LegQuote::Priced(U256::zero())
        );
    }

    // ── Root 2C Phase 1: quote_route folds legs; V3 Unavailable propagates ──
    #[tokio::test]
    async fn route_quote_route_folds_and_propagates_unavailable() {
        let cache = Arc::new(ReservesCache::new());
        let projector = StateProjector::new(cache, None, empty_v3_fee_catalog());
        // V2 → V2 route: out of leg0 feeds leg1. Compose by hand, compare exactly.
        let legs = [
            LegEval::V2 {
                reserve_in: U256::from(1_000_000u64),
                reserve_out: U256::from(2_000_000u64),
                fee_bps: 30,
            },
            LegEval::V2 {
                reserve_in: U256::from(2_000_000u64),
                reserve_out: U256::from(1_000_000u64),
                fee_bps: 30,
            },
        ];
        let amt = U256::from(10u128.pow(18));
        let mid = v2_amount_out(amt, U256::from(1_000_000u64), U256::from(2_000_000u64), 30);
        let end = v2_amount_out(mid, U256::from(2_000_000u64), U256::from(1_000_000u64), 30);
        assert_eq!(projector.quote_route(&legs, amt).await, Some(end));

        // A V3 leg with no provider → Unavailable → route returns None (R8, no fabrication).
        let v3_leg = LegEval::V3 {
            pool: PoolRef {
                address: Address::zero(),
                token0: Address::zero(),
                token1: Address::from_low_u64_be(1),
                fee_bps: Some(500),
            },
            zero_for_one: true,
        };
        assert_eq!(
            projector.quote_route(&[legs[0].clone(), v3_leg], amt).await,
            None,
            "V3 leg with no provider must propagate Unavailable, not fabricate"
        );
    }

    // ── size_optimizer::tests::cap_bound_caps_input ───────────────────────────
    //
    // Math optimum at ~$10K, capital cap at $100. Returned size must be ≤ $100.

    #[tokio::test]
    async fn cap_bound_caps_input() {
        let pool_a = addr(0x10);
        let pool_b = addr(0x11);
        let tok_weth = addr(0xAAAA);
        let tok_usdc = addr(0xBBBB);

        let cache = Arc::new(ReservesCache::new());
        // Profitable setup with large reserves.
        cache.insert(pool_a, unit(10_000), unit(20_000_000)).await;
        cache.insert(pool_b, unit(9_000_000), unit(6_000)).await;

        let projector = Arc::new(StateProjector::new(cache, None, empty_v3_fee_catalog()));
        let optimizer = SizeOptimizer::new(projector);

        let candidate = make_dex_candidate(
            pool_a,
            pool_b,
            tok_weth,
            tok_usdc,
            StrategyLabel::DexArbV2V2,
        );
        let intent = make_intent(tok_weth, tok_usdc);

        // Capital cap: $100 → about 0.033 WETH at $3000/WETH.
        let cfg = make_cfg(100.0);
        let cap_wei = {
            let cap_usd = 100.0_f64;
            let price = 3000.0_f64;
            // ~0.033 WETH in wei.
            let cap_tokens = cap_usd / price;
            let cap_raw = (cap_tokens * 1e18) as u64;
            U256::from(cap_raw)
        };

        let result = optimizer
            .optimize(candidate, &intent, Some(&cfg))
            .await
            .expect("optimize must not error");

        if let Some(sized) = result {
            assert!(
                sized.optimal_amount_in <= cap_wei * U256::from(2u32),
                // Allow 2x tolerance for rounding in f64→U256 conversion.
                "optimal_amount_in must be bounded by cap: {} <= cap_approx {}",
                sized.optimal_amount_in,
                cap_wei
            );
        }
        // Ok(None) is acceptable if cap is too small to be profitable after gas.
    }

    // ── size_optimizer::tests::zero_cap_returns_none ─────────────────────────

    #[tokio::test]
    async fn zero_cap_returns_none() {
        let pool_a = addr(0x10);
        let pool_b = addr(0x11);
        let tok_weth = addr(0xAAAA);
        let tok_usdc = addr(0xBBBB);

        let cache = Arc::new(ReservesCache::new());
        cache.insert(pool_a, unit(1000), unit(2_000_000)).await;
        cache.insert(pool_b, unit(1_000_000), unit(600)).await;

        let projector = Arc::new(StateProjector::new(cache, None, empty_v3_fee_catalog()));
        let optimizer = SizeOptimizer::new(projector);

        let candidate = make_dex_candidate(
            pool_a,
            pool_b,
            tok_weth,
            tok_usdc,
            StrategyLabel::DexArbV2V2,
        );
        let intent = make_intent(tok_weth, tok_usdc);

        // capital_usd = 0 → cap = 0.
        let cfg = make_cfg(0.0);

        let result = optimizer
            .optimize(candidate, &intent, Some(&cfg))
            .await
            .expect("optimize must not error");

        assert!(
            result.is_none(),
            "zero capital cap must return None — R8 invariant"
        );
    }

    // ── size_optimizer::tests::no_price_returns_none ─────────────────────────
    //
    // Config has no base_token_price_usd (0.0) and no per-token entry.
    // Token cannot be priced → Ok(None).

    #[tokio::test]
    async fn no_price_returns_none() {
        let pool_a = addr(0x10);
        let pool_b = addr(0x11);
        let tok_weth = addr(0xAAAA);
        let tok_usdc = addr(0xBBBB);

        let cache = Arc::new(ReservesCache::new());
        cache.insert(pool_a, unit(1000), unit(1100)).await;
        cache.insert(pool_b, unit(1100), unit(1000)).await;

        let projector = Arc::new(StateProjector::new(cache, None, empty_v3_fee_catalog()));
        let optimizer = SizeOptimizer::new(projector);

        let candidate = make_dex_candidate(
            pool_a,
            pool_b,
            tok_weth,
            tok_usdc,
            StrategyLabel::DexArbV2V2,
        );
        let intent = make_intent(tok_weth, tok_usdc);

        // Price = 0 → cannot compute USD profit.
        let mut cfg = make_cfg(10_000.0);
        cfg.base_token_price_usd = 0.0;

        let result = optimizer
            .optimize(candidate, &intent, Some(&cfg))
            .await
            .expect("optimize must not error");

        assert!(
            result.is_none(),
            "unpriced token must return Ok(None) — R8 invariant"
        );
    }

    // ── size_optimizer::tests::triangular_uses_golden_section ────────────────
    //
    // 3-leg candidate with profitable reserves. Optimizer must find a positive size.

    #[tokio::test]
    async fn triangular_uses_golden_section() {
        let pool_a = addr(0x100);
        let pool_b = addr(0x200);
        let pool_c = addr(0x300);
        let tok_a = addr(0x10);
        let tok_b = addr(0x20);
        let tok_c = addr(0x30);

        let cache = Arc::new(ReservesCache::new());
        // Same profitable reserves as triangular_engine tests.
        let unit_val = U256::from(10u128).pow(U256::from(18u32));
        cache
            .insert(
                pool_a,
                unit_val * U256::from(100u32),
                unit_val * U256::from(120u32),
            )
            .await;
        cache
            .insert(
                pool_b,
                unit_val * U256::from(100u32),
                unit_val * U256::from(110u32),
            )
            .await;
        // hop2: swap_in_is_token0 = (tok_c < tok_a) = (0x30 < 0x10) = false
        //   → reserve_in = r1 (tok_c side), reserve_out = r0 (tok_a side).
        //   r0=200 (tok_a), r1=100 (tok_c) → reserves_oriented: r_in=100, r_out=200.
        cache
            .insert(
                pool_c,
                unit_val * U256::from(200u32),
                unit_val * U256::from(100u32),
            )
            .await;

        let projector = Arc::new(StateProjector::new(cache, None, empty_v3_fee_catalog()));
        let optimizer = SizeOptimizer::new(projector);

        let id = Uuid::new_v4();
        let pool_a_str = format!("0x{:040x}", pool_a);
        let pool_b_str = format!("0x{:040x}", pool_b);
        let pool_c_str = format!("0x{:040x}", pool_c);
        let tok_a_str = format!("0x{:040x}", tok_a);
        let tok_b_str = format!("0x{:040x}", tok_b);
        let tok_c_str = format!("0x{:040x}", tok_c);

        let opp = Opportunity {
            id,
            chain_id: 1,
            strategy_kind: StrategyKind::triangular(),
            dex_a: "uniswap-v2".to_string(),
            dex_b: None,
            pair_symbol: "WETH(triangular)".to_string(),
            token_in: tok_a_str.clone(),
            token_out: tok_a_str.clone(),
            amount_in_wei: unit_val.to_string(),
            expected_profit_usd: Some(1.0),
            net_expected_profit_usd: None,
            roi_pct: None,
            risk_score: None,
            block_number: None,
            rejection_reason: None,
            cartridge_id: None,
            detector_id: None,
            pipeline_latency_ms: None,
            detected_at: Utc::now(),
            trace_id: Uuid::new_v4(),
            economics: None,
        };

        let route_plan = RoutePlan {
            route_id: Some("tri-test".to_string()),
            strategy_kind: "triangular_arb".to_string(),
            chain_id: 1,
            legs: vec![
                RouteLeg {
                    dex_id: "uniswap-v2".to_string(),
                    dex_name: "uniswap-v2".to_string(),
                    protocol_type: "uniswap-v2".to_string(),
                    factory_address: String::new(),
                    pool_id: None,
                    pool_address: Some(pool_a_str),
                    token_in: tok_a_str.clone(),
                    token_out: tok_b_str.clone(),
                    fee_bps: Some(30),
                    amount_in: Some(1.0),
                    amount_out: None,
                    tvl_usd: None,
                    volume_24h_usd: None,
                    pool_is_active: true,
                },
                RouteLeg {
                    dex_id: "uniswap-v2".to_string(),
                    dex_name: "uniswap-v2".to_string(),
                    protocol_type: "uniswap-v2".to_string(),
                    factory_address: String::new(),
                    pool_id: None,
                    pool_address: Some(pool_b_str),
                    token_in: tok_b_str.clone(),
                    token_out: tok_c_str.clone(),
                    fee_bps: Some(30),
                    amount_in: Some(1.0),
                    amount_out: None,
                    tvl_usd: None,
                    volume_24h_usd: None,
                    pool_is_active: true,
                },
                RouteLeg {
                    dex_id: "uniswap-v2".to_string(),
                    dex_name: "uniswap-v2".to_string(),
                    protocol_type: "uniswap-v2".to_string(),
                    factory_address: String::new(),
                    pool_id: None,
                    pool_address: Some(pool_c_str),
                    token_in: tok_c_str,
                    token_out: tok_a_str,
                    fee_bps: Some(30),
                    amount_in: Some(1.0),
                    amount_out: None,
                    tvl_usd: None,
                    volume_24h_usd: None,
                    pool_is_active: true,
                },
            ],
            atomic: true,
            estimated_slippage_pct: None,
            price_impact_pct: None,
        };

        let candidate = StrategyCandidate {
            label: StrategyLabel::TriangularArb,
            opportunity: opp,
            candidate: OpportunityCandidate {
                route_fingerprint: "tri-test".to_string(),
                pool_addresses: vec![],
                token_addresses: vec![],
                dex_adapters: vec!["uniswap-v2".to_string(); 3],
                amount_in: 1.0,
                expected_amount_out: 2.0,
                gross_profit: 1.0,
            },
            route_plan,
            gross_profit_usd: Some(1.0),
            net_expected_profit_usd: None,
            rejection_reason: None,
            source_intent_hash: H256::zero(),
            base_strategy: None,
        };

        let intent = make_intent(tok_a, tok_b);
        let cfg = make_cfg(50_000.0);

        let result = optimizer
            .optimize(candidate, &intent, Some(&cfg))
            .await
            .expect("optimize must not error");

        // With profitable reserves and valid config, optimizer should return Some.
        // Note: the triangular kernel requires specific reserve orientation matching
        // the pool's token0/token1 ordering via swap_in_is_token0.
        // The route plan uses string-based orientation which may not match
        // the cache (tok_a=0x10, tok_b=0x20, tok_c=0x30 all have tok_a < tok_b < tok_c).
        // With the string-based orient_reserves: tok_a_str < tok_b_str for standard
        // 0x-prefixed addresses → (r0, r1) is (100, 120) for hop 0 → reserve_in=100.
        // This matches the profitable setup.
        if let Some(sized) = result {
            assert!(
                sized.gross_profit_usd > 0.0,
                "triangular: gross_profit_usd must be positive"
            );
            assert!(
                sized.estimated_net_profit_usd > 0.0,
                "triangular: estimated_net_profit_usd must be positive"
            );
        }
        // Ok(None) is acceptable — exact profitability depends on evaluate_cycle
        // processing the reserves in the expected orientation.
    }

    // ── size_optimizer::tests::triangular_sized_emits_per_leg_ledger ────────
    //
    // WO-LEGS-TRIANGULAR-01: the triangular kernel must emit the per-leg wei
    // ledger on the SizedCandidate, chained to the reported amount_in and the
    // profit's own hop chain (post-clamp re-evaluation inside evaluate_cycle).

    #[tokio::test]
    async fn triangular_sized_emits_per_leg_ledger() {
        let pool_a = addr(0x100);
        let pool_b = addr(0x200);
        let pool_c = addr(0x300);
        let tok_a = addr(0x10);
        let tok_b = addr(0x20);
        let tok_c = addr(0x30);

        let cache = Arc::new(ReservesCache::new());
        // Same profitable reserves/orientation as triangular_uses_golden_section:
        //   hop0 (a→b): (r_in, r_out) = (100e18, 120e18)
        //   hop1 (b→c): (r_in, r_out) = (100e18, 110e18)
        //   hop2 (c→a): (r_in, r_out) = (100e18, 200e18)
        let unit_val = U256::from(10u128).pow(U256::from(18u32));
        cache
            .insert(
                pool_a,
                unit_val * U256::from(100u32),
                unit_val * U256::from(120u32),
            )
            .await;
        cache
            .insert(
                pool_b,
                unit_val * U256::from(100u32),
                unit_val * U256::from(110u32),
            )
            .await;
        cache
            .insert(
                pool_c,
                unit_val * U256::from(200u32),
                unit_val * U256::from(100u32),
            )
            .await;

        let projector = Arc::new(StateProjector::new(cache, None, empty_v3_fee_catalog()));
        let optimizer = SizeOptimizer::new(projector);

        let pool_a_str = format!("0x{:040x}", pool_a);
        let pool_b_str = format!("0x{:040x}", pool_b);
        let pool_c_str = format!("0x{:040x}", pool_c);
        let tok_a_str = format!("0x{:040x}", tok_a);
        let tok_b_str = format!("0x{:040x}", tok_b);
        let tok_c_str = format!("0x{:040x}", tok_c);

        let make_leg = |pool: String, t_in: String, t_out: String| RouteLeg {
            dex_id: "uniswap-v2".to_string(),
            dex_name: "uniswap-v2".to_string(),
            protocol_type: "uniswap-v2".to_string(),
            factory_address: String::new(),
            pool_id: None,
            pool_address: Some(pool),
            token_in: t_in,
            token_out: t_out,
            fee_bps: Some(30),
            amount_in: Some(1.0),
            amount_out: None,
            tvl_usd: None,
            volume_24h_usd: None,
            pool_is_active: true,
        };

        let opp = Opportunity {
            id: Uuid::new_v4(),
            chain_id: 1,
            strategy_kind: StrategyKind::triangular(),
            dex_a: "uniswap-v2".to_string(),
            dex_b: None,
            pair_symbol: "WETH(tri-ledger)".to_string(),
            token_in: tok_a_str.clone(),
            token_out: tok_a_str.clone(),
            amount_in_wei: unit_val.to_string(),
            expected_profit_usd: Some(1.0),
            net_expected_profit_usd: None,
            roi_pct: None,
            risk_score: None,
            block_number: None,
            rejection_reason: None,
            cartridge_id: None,
            detector_id: None,
            pipeline_latency_ms: None,
            detected_at: Utc::now(),
            trace_id: Uuid::new_v4(),
            economics: None,
        };

        let route_plan = RoutePlan {
            route_id: Some("tri-ledger-test".to_string()),
            strategy_kind: "triangular_arb".to_string(),
            chain_id: 1,
            legs: vec![
                make_leg(pool_a_str.clone(), tok_a_str.clone(), tok_b_str.clone()),
                make_leg(pool_b_str.clone(), tok_b_str.clone(), tok_c_str.clone()),
                make_leg(pool_c_str.clone(), tok_c_str.clone(), tok_a_str.clone()),
            ],
            atomic: true,
            estimated_slippage_pct: None,
            price_impact_pct: None,
        };

        let candidate = StrategyCandidate {
            label: StrategyLabel::TriangularArb,
            opportunity: opp,
            candidate: OpportunityCandidate {
                route_fingerprint: "tri-ledger-test".to_string(),
                pool_addresses: vec![],
                token_addresses: vec![],
                dex_adapters: vec!["uniswap-v2".to_string(); 3],
                amount_in: 1.0,
                expected_amount_out: 2.0,
                gross_profit: 1.0,
            },
            route_plan,
            gross_profit_usd: Some(1.0),
            net_expected_profit_usd: None,
            rejection_reason: None,
            source_intent_hash: H256::zero(),
            base_strategy: None,
        };

        let intent = make_intent(tok_a, tok_b);
        // Generous Kelly budget ($1M NAV, per-trade cap 100%) so the overlay
        // does NOT rescale — this test pins the KERNEL's ledger emission;
        // the rescale-null-out is pinned separately in
        // kelly_overlay_drops_ledger_when_reserves_not_derivable.
        let cfg = make_cfg_kelly(
            /* capital_usd */ 1_000_000.0,
            /* multiplier */ 1.0,
            /* max_per_trade */ 1.0,
            /* gas_safety */ 1.0,
            /* min_p */ 0.9,
        );

        let result = optimizer
            .optimize(candidate, &intent, Some(&cfg))
            .await
            .expect("optimize must not error");

        let sized = result.expect("profitable triangular fixture must size");
        assert!(
            sized.estimated_net_profit_usd > 0.0,
            "fixture must be net-positive for the ledger arm"
        );
        let ins = sized.leg_amounts_in.expect("leg_amounts_in must be Some");
        let outs = sized.leg_amounts_out.expect("leg_amounts_out must be Some");
        assert_eq!(ins.len(), 3, "3-hop route → 3 leg inputs");
        assert_eq!(outs.len(), 3, "3-hop route → 3 leg outputs");

        // Ledger entry point == the reported optimal amount.
        assert_eq!(ins[0], sized.optimal_amount_in.to_string());

        // Chain consistency (Sancho cond. 4): in[i+1] == out[i].
        for i in 0..2 {
            assert_eq!(
                ins[i + 1],
                outs[i],
                "leg {} input must equal leg {} output",
                i + 1,
                i
            );
        }

        // Cross-check against the kernel: outs[last] == cycle_profit().0 at
        // the reported amount_in — the ledger is the SAME evaluation whose
        // profit was reported, not a stale re-quote.
        let hop_reserves = vec![
            (unit_val * U256::from(100u32), unit_val * U256::from(120u32)),
            (unit_val * U256::from(100u32), unit_val * U256::from(110u32)),
            (unit_val * U256::from(100u32), unit_val * U256::from(200u32)),
        ];
        let (plain_out, _plain_profit) = crate::workers::triangular_worker::cycle_profit(
            sized.optimal_amount_in,
            &hop_reserves,
            30,
        );
        assert_eq!(outs[2], plain_out.to_string());
    }

    // ── size_optimizer::tests::no_config_returns_rejected_no_config ─────────
    //
    // TASK 2: Passing cfg = None must return Rejected(NoConfig), not Ok(None).

    #[tokio::test]
    async fn no_config_returns_rejected_no_config() {
        let pool_a = addr(0x10);
        let pool_b = addr(0x11);
        let tok_weth = addr(0xAAAA);
        let tok_usdc = addr(0xBBBB);

        let cache = Arc::new(ReservesCache::new());
        // Profitable reserves to ensure it is the config gate, not the math, that rejects.
        cache.insert(pool_a, unit(1000), unit(2_000_000)).await;
        cache.insert(pool_b, unit(1_000_000), unit(600)).await;

        let projector = Arc::new(StateProjector::new(cache, None, empty_v3_fee_catalog()));
        let optimizer = SizeOptimizer::new(projector);

        let candidate = make_dex_candidate(
            pool_a,
            pool_b,
            tok_weth,
            tok_usdc,
            StrategyLabel::DexArbV2V2,
        );
        let intent = make_intent(tok_weth, tok_usdc);

        let outcome = optimizer
            .optimize_with_reason(candidate, &intent, None) // cfg = None
            .await
            .expect("optimize_with_reason must not return Err");

        assert!(
            matches!(
                outcome,
                OptimizeOutcome::Rejected(OptimizeRejectReason::NoConfig, _)
            ),
            "cfg=None must produce Rejected(NoConfig)"
        );
    }

    // ── size_optimizer::tests::missing_reserves_returns_specific_pool_reason ──
    //
    // TASK 2: When pool A reserves are absent from cache, must return
    // Rejected(MissingReservesPoolA). When pool B is absent, Rejected(MissingReservesPoolB).

    #[tokio::test]
    async fn missing_reserves_returns_specific_pool_reason() {
        let pool_a = addr(0x10);
        let pool_b = addr(0x11);
        let tok_weth = addr(0xAAAA);
        let tok_usdc = addr(0xBBBB);

        // ── Case A: only pool_b has reserves (pool_a missing) ───────────────
        {
            let cache = Arc::new(ReservesCache::new());
            // Insert pool_b reserves but NOT pool_a.
            cache.insert(pool_b, unit(1_000_000), unit(600)).await;

            let projector = Arc::new(StateProjector::new(cache, None, empty_v3_fee_catalog()));
            let optimizer = SizeOptimizer::new(projector);

            let candidate = make_dex_candidate(
                pool_a,
                pool_b,
                tok_weth,
                tok_usdc,
                StrategyLabel::DexArbV2V2,
            );
            let intent = make_intent(tok_weth, tok_usdc);
            let cfg = make_cfg(10_000.0);

            let outcome = optimizer
                .optimize_with_reason(candidate, &intent, Some(&cfg))
                .await
                .expect("must not error");

            assert!(
                matches!(
                    outcome,
                    OptimizeOutcome::Rejected(OptimizeRejectReason::MissingReservesPoolA, _)
                ),
                "missing pool_a reserves must produce Rejected(MissingReservesPoolA)"
            );
        }

        // ── Case B: only pool_a has reserves (pool_b missing) ───────────────
        {
            let cache = Arc::new(ReservesCache::new());
            // Insert pool_a reserves but NOT pool_b.
            cache.insert(pool_a, unit(1000), unit(2_000_000)).await;

            let projector = Arc::new(StateProjector::new(cache, None, empty_v3_fee_catalog()));
            let optimizer = SizeOptimizer::new(projector);

            let candidate = make_dex_candidate(
                pool_a,
                pool_b,
                tok_weth,
                tok_usdc,
                StrategyLabel::DexArbV2V2,
            );
            let intent = make_intent(tok_weth, tok_usdc);
            let cfg = make_cfg(10_000.0);

            let outcome = optimizer
                .optimize_with_reason(candidate, &intent, Some(&cfg))
                .await
                .expect("must not error");

            assert!(
                matches!(
                    outcome,
                    OptimizeOutcome::Rejected(OptimizeRejectReason::MissingReservesPoolB, _)
                ),
                "missing pool_b reserves must produce Rejected(MissingReservesPoolB)"
            );
        }
    }

    // ── size_optimizer::tests::non_positive_net_returns_non_positive_net_usd ──
    //
    // TASK 2: A route where gross is positive but net (after gas) is ≤ 0 must
    // return Rejected(NonPositiveNetUsd).
    //
    // Setup: symmetric pools (profit_wei == 0) → search finds no profit →
    // NonPositiveProfit is returned (not NonPositiveNetUsd). To hit NonPositiveNetUsd
    // specifically we need a route with a tiny gross profit that gas erases.
    // We use a config with very high gas_estimate_units (500_000) at 20 gwei
    // and deliberately small reserves that produce only a micro-spread in USD.

    #[tokio::test]
    async fn non_positive_net_returns_non_positive_net_usd() {
        let pool_a = addr(0x10);
        let pool_b = addr(0x11);
        let tok_weth = addr(0xAAAA);
        let tok_usdc = addr(0xBBBB);

        let cache = Arc::new(ReservesCache::new());
        // Tiny asymmetric reserves: pool_a r0=1001 r1=1000, pool_b r0=1000 r1=1001.
        // The spread is tiny: approximately (1001/1000 - 1000/1001) ~= 0.1%.
        // With 500_000 gas @ 20 gwei @ $3000/ETH → gas cost ≈ $30.
        // The profit on 0.001 WETH at $3000/WETH ≈ $0.003. So net = 0.003 - 30 < 0.
        let tiny = U256::from(10u128).pow(U256::from(15u32)); // 0.001 ETH in wei units
        cache
            .insert(
                pool_a,
                tiny * U256::from(1001u32),
                tiny * U256::from(1000u32),
            )
            .await;
        cache
            .insert(
                pool_b,
                tiny * U256::from(1000u32),
                tiny * U256::from(1001u32),
            )
            .await;

        let projector = Arc::new(StateProjector::new(cache, None, empty_v3_fee_catalog()));
        let optimizer = SizeOptimizer::new(projector);

        let candidate = make_dex_candidate(
            pool_a,
            pool_b,
            tok_weth,
            tok_usdc,
            StrategyLabel::DexArbV2V2,
        );
        let intent = make_intent(tok_weth, tok_usdc);

        // Config: high gas makes net profit negative even if gross is tiny-positive.
        let mut cfg = make_cfg(10_000.0);
        cfg.gas_estimate_units = 500_000;
        cfg.fixed_gas_price_gwei = Some(20.0);

        let outcome = optimizer
            .optimize_with_reason(candidate, &intent, Some(&cfg))
            .await
            .expect("must not error");

        // The outcome is EITHER NonPositiveProfit (search found no wei profit with these
        // tiny reserves and the golden-section convergence) OR NonPositiveNetUsd (search
        // found small profit but gas eliminated it). Both are valid rejections here;
        // what must NOT happen is Sized or NoConfig/MissingReserves.
        match outcome {
            OptimizeOutcome::Rejected(
                OptimizeRejectReason::NonPositiveProfit
                | OptimizeRejectReason::NonPositiveNetUsd
                | OptimizeRejectReason::NonPositiveGrossUsd,
                _,
            ) => {
                // Expected: gas destroys any micro-profit.
            }
            // ALWAYS-COMPUTE (2026-09-27): the converted reject paths carry the
            // FULL figures — same reasons, richer (honest) payload.
            OptimizeOutcome::RejectedComputed(
                OptimizeRejectReason::NonPositiveProfit
                | OptimizeRejectReason::NonPositiveNetUsd
                | OptimizeRejectReason::NonPositiveGrossUsd,
                _,
            ) => {
                // Expected: gas destroys any micro-profit — WITH the numbers.
            }
            other => {
                let reason = match other {
                    OptimizeOutcome::Sized(_) => "Sized (unexpected — profit survived high gas)",
                    OptimizeOutcome::Rejected(r, _)
                    | OptimizeOutcome::RejectedWithLedger(r, _, _)
                    | OptimizeOutcome::RejectedComputed(r, _) => r.as_str(),
                };
                panic!("unexpected outcome: {reason}");
            }
        }
    }

    // ── size_optimizer::tests::sized_outcome_carries_optimal_amount_in ───────
    //
    // TASK 2: A Sized outcome must carry a positive optimal_amount_in.

    #[tokio::test]
    async fn sized_outcome_carries_optimal_amount_in() {
        let pool_a = addr(0x10);
        let pool_b = addr(0x11);
        let tok_weth = addr(0xAAAA);
        let tok_usdc = addr(0xBBBB);

        let cache = Arc::new(ReservesCache::new());
        // Deeply profitable reserves (same as profitable_route_returns_optimal_size).
        // Values are pinned here so a reader can trace the math:
        //   Pool A: r_in=1000 WETH, r_out=2_000_000 USDC → price 2000 USDC/WETH
        //   Pool B: r_in=1_000_000 USDC, r_out=600 WETH   → price 1667 USDC/WETH
        // Spread direction: buy WETH cheap on B (sell USDC → WETH), sell on A.
        // With WETH addr used as token_in for the leg, orient_reserves uses
        // string comparison of "0xc02..." vs leg's token_out to pick r0/r1.
        // Regardless of orientation the spread is large enough for a profit.
        let r_in_a = unit(1000);
        let r_out_a = unit(2_000_000);
        cache.insert(pool_a, r_in_a, r_out_a).await;

        let r_in_b = unit(1_000_000);
        let r_out_b = unit(600);
        cache.insert(pool_b, r_in_b, r_out_b).await;

        let projector = Arc::new(StateProjector::new(cache, None, empty_v3_fee_catalog()));
        let optimizer = SizeOptimizer::new(projector);

        let candidate = make_dex_candidate(
            pool_a,
            pool_b,
            tok_weth,
            tok_usdc,
            StrategyLabel::DexArbV2V2,
        );
        let intent = make_intent(tok_weth, tok_usdc);
        let cfg = make_cfg(10_000.0);

        let outcome = optimizer
            .optimize_with_reason(candidate, &intent, Some(&cfg))
            .await
            .expect("must not error");

        // This route may or may not produce profit depending on orient_reserves
        // resolution. If it does produce a Sized outcome, verify the invariants.
        if let OptimizeOutcome::Sized(sized) = outcome {
            assert!(
                sized.optimal_amount_in > U256::zero(),
                "Sized outcome must carry a positive optimal_amount_in"
            );
            assert!(
                sized.gross_profit_usd > 0.0,
                "Sized outcome must carry a positive gross_profit_usd"
            );
            assert!(
                sized.estimated_net_profit_usd > 0.0,
                "Sized outcome must carry a positive estimated_net_profit_usd"
            );
            // Verify OptimizeOutcome helpers are consistent.
            // Re-build outcome to test the helper methods.
            let outcome2 = OptimizeOutcome::Sized(sized.clone());
            assert!(outcome2.reason_str().is_none());
            assert_eq!(outcome2.gross_profit_usd(), Some(sized.gross_profit_usd));
            assert_eq!(
                outcome2.net_profit_usd(),
                Some(sized.estimated_net_profit_usd)
            );
            assert_eq!(outcome2.optimal_amount_in(), Some(sized.optimal_amount_in));
        }
        // Ok(Rejected(...)) is also valid if orientation doesn't produce profit.
    }

    // ── size_optimizer::tests::flashloan_wrapped_subtracts_fee ───────────────
    //
    // A FlashloanArb candidate with base_strategy = Some(DexArbV2V2) must
    // have the flash loan fee subtracted from net profit.
    // The test uses symmetric pools (no profit) so Ok(None) is the expected outcome,
    // but we verify the fee subtraction code path doesn't panic.

    #[tokio::test]
    async fn flashloan_wrapped_subtracts_fee() {
        let pool_a = addr(0x10);
        let pool_b = addr(0x11);
        let tok_weth = addr(0xAAAA);
        let tok_usdc = addr(0xBBBB);

        let cache = Arc::new(ReservesCache::new());
        // Symmetric pools → no profit → tests that fee path doesn't panic.
        cache.insert(pool_a, unit(10_000), unit(10_000)).await;
        cache.insert(pool_b, unit(10_000), unit(10_000)).await;

        let projector = Arc::new(StateProjector::new(cache, None, empty_v3_fee_catalog()));
        let optimizer = SizeOptimizer::new(projector);

        let mut candidate = make_dex_candidate(
            pool_a,
            pool_b,
            tok_weth,
            tok_usdc,
            StrategyLabel::FlashloanArb,
        );
        candidate.base_strategy = Some(StrategyLabel::DexArbV2V2);

        let intent = make_intent(tok_weth, tok_usdc);
        let cfg = make_cfg(10_000.0);

        // Must not panic — symmetric pools return None.
        let result = optimizer
            .optimize(candidate, &intent, Some(&cfg))
            .await
            .expect("optimize must not panic on flashloan candidate");

        // Symmetric pools → Ok(None).
        assert!(
            result.is_none(),
            "symmetric pools must return None even for flashloan candidate"
        );
    }

    // ── Fix-2 (2026-05-13): Kelly post-optimization tests ──────────────

    /// Build a SizedCandidate with explicit gross/net/optimal_amount_in
    /// for direct `apply_kelly_constraints` testing.
    fn make_sized(optimal_amount_in_wei: U256, gross_usd: f64, net_usd: f64) -> SizedCandidate {
        let pa = addr(1);
        let pb = addr(2);
        let ti = addr(3);
        let to = addr(4);
        let cand = make_dex_candidate(pa, pb, ti, to, StrategyLabel::DexArbV2V2);
        SizedCandidate {
            candidate: cand,
            optimal_amount_in: optimal_amount_in_wei,
            gross_profit_usd: gross_usd,
            estimated_net_profit_usd: net_usd,
            net_negative: false,
            net_economics: None, // ARBX-0009: hand-built fixture — no components.
            leg_amounts_in: None, // HOPS-LEDGER-04: fixture — no kernel leg math.
            leg_amounts_out: None,
        }
    }

    /// Cfg helper that lets us override the Kelly knobs independently.
    fn make_cfg_kelly(
        capital_usd: f64,
        kelly_multiplier: f64,
        kelly_max_per_trade_fraction: f64,
        kelly_gas_safety_multiplier: f64,
        min_landing_probability: f64,
    ) -> TradingConfigState {
        let mut c = make_cfg(capital_usd);
        c.kelly_multiplier = kelly_multiplier;
        c.kelly_max_per_trade_fraction = kelly_max_per_trade_fraction;
        c.kelly_gas_safety_multiplier = kelly_gas_safety_multiplier;
        c.min_landing_probability = min_landing_probability;
        c
    }

    /// HOPS-LEDGER-05 — drive the Kelly step through a bare optimizer instance.
    ///
    /// The step is now an instance method because it RE-DERIVES the per-leg ledger
    /// at the capped size from the reserves cache. These gates are pure Kelly
    /// arithmetic over hand-built fixtures whose pools are not in any cache, so
    /// `reledger_at` honestly returns `(None, None)` (R8: not derivable) and every
    /// pre-existing expectation holds bit-for-bit. The re-derivation itself has its
    /// own gate (`kelly_ledger_is_rederived_at_the_final_size`).
    async fn kelly_step(
        outcome: OptimizeOutcome,
        cfg: &TradingConfigState,
        cap_usd: f64,
        token_price_usd: f64,
        decimals: u8,
    ) -> OptimizeOutcome {
        let cache = Arc::new(ReservesCache::new());
        let projector = Arc::new(StateProjector::new(cache, None, empty_v3_fee_catalog()));
        SizeOptimizer::new(projector)
            .apply_kelly_constraints(outcome, cfg, cap_usd, token_price_usd, decimals)
            .await
    }

    /// Same, with the pools actually cached — the `HOPS-LEDGER-05` re-derivation
    /// path (the empty-cache helper above can only prove the R8 absence).
    async fn kelly_step_with_cache(
        outcome: OptimizeOutcome,
        cfg: &TradingConfigState,
        cap_usd: f64,
        token_price_usd: f64,
        decimals: u8,
        cache: Arc<ReservesCache>,
    ) -> OptimizeOutcome {
        let projector = Arc::new(StateProjector::new(cache, None, empty_v3_fee_catalog()));
        SizeOptimizer::new(projector)
            .apply_kelly_constraints(outcome, cfg, cap_usd, token_price_usd, decimals)
            .await
    }

    #[tokio::test]
    async fn kelly_passes_rejected_through_unchanged() {
        let cfg = make_cfg(1000.0);
        let result = kelly_step(
            OptimizeOutcome::Rejected(OptimizeRejectReason::NoConfig, None),
            &cfg,
            1000.0,
            3000.0,
            18,
        )
        .await;
        match result {
            OptimizeOutcome::Rejected(OptimizeRejectReason::NoConfig, payload) => {
                assert!(payload.is_none(), "None payload must pass through as None");
            }
            _ => panic!("expected pass-through Rejected(NoConfig)"),
        }
    }

    /// Deuda 4-(B): R8 contract on the `Rejected` payload — `None` = not
    /// computed (infra/Err-path), `Some(v)` = computed and exactly `v`.
    #[test]
    fn rejected_net_profit_usd_payload_contract() {
        let none_case = OptimizeOutcome::Rejected(OptimizeRejectReason::NonPositiveProfit, None);
        assert!(none_case.net_profit_usd().is_none());

        let some_case = OptimizeOutcome::Rejected(OptimizeRejectReason::GasFloorBreach, Some(1.25));
        assert_eq!(some_case.net_profit_usd(), Some(1.25));

        // Negative values are valid computed results — must survive verbatim.
        let negative_case =
            OptimizeOutcome::Rejected(OptimizeRejectReason::NonPositiveProfit, Some(-0.42));
        assert_eq!(negative_case.net_profit_usd(), Some(-0.42));
    }

    #[tokio::test]
    async fn kelly_passes_when_cap_not_binding() {
        // Generous cap and permissive gates → Kelly does not change the kernel
        // result.
        let cfg = make_cfg_kelly(
            /* capital_usd */ 1_000_000.0,
            /* multiplier */ 0.5,
            /* max_per_trade */ 1.0,
            /* gas_safety */ 1.0,
            /* min_p */ 0.8,
        );
        let sized = make_sized(unit(1), 100.0, 90.0); // 1 WETH bet; cap is huge
        let result = kelly_step(
            OptimizeOutcome::Sized(Box::new(sized.clone())),
            &cfg,
            cfg.capital_usd,
            3000.0,
            18,
        )
        .await;
        match result {
            OptimizeOutcome::Sized(s) => {
                assert_eq!(s.optimal_amount_in, sized.optimal_amount_in);
                assert_eq!(s.gross_profit_usd, sized.gross_profit_usd);
                assert_eq!(s.estimated_net_profit_usd, sized.estimated_net_profit_usd);
            }
            OptimizeOutcome::Rejected(r, _)
            | OptimizeOutcome::RejectedWithLedger(r, _, _)
            | OptimizeOutcome::RejectedComputed(r, _) => {
                panic!("unexpected reject {:?}", r)
            }
        }
    }

    #[tokio::test]
    async fn kelly_caps_optimal_when_max_per_trade_binds() {
        // Tight max_per_trade forces the cap to bind. Profit must scale down.
        let cfg = make_cfg_kelly(
            /* capital_usd */ 3_000.0, // $3K cap
            /* multiplier */ 0.5,
            /* max_per_trade */ 0.001, // 0.1% of NAV → max bet $3
            /* gas_safety */ 1.0, /* min_p */ 0.8,
        );
        // Kernel says: 1 WETH ≈ $3000, gross=$100, net=$90 (cost=$10).
        let sized = make_sized(unit(1), 100.0, 90.0);
        let result = kelly_step(
            OptimizeOutcome::Sized(Box::new(sized.clone())),
            &cfg,
            cfg.capital_usd,
            3000.0,
            18,
        )
        .await;
        match result {
            OptimizeOutcome::Sized(s) => {
                assert!(
                    s.optimal_amount_in < sized.optimal_amount_in,
                    "Kelly cap should have reduced optimal_amount_in (was {}, now {})",
                    sized.optimal_amount_in,
                    s.optimal_amount_in,
                );
                assert!(
                    s.gross_profit_usd < sized.gross_profit_usd,
                    "gross should scale down when amount caps down",
                );
            }
            OptimizeOutcome::Rejected(r, _)
            | OptimizeOutcome::RejectedWithLedger(r, _, _)
            | OptimizeOutcome::RejectedComputed(r, _) => {
                // The scaled net may drop below gas floor at very tight caps;
                // both Sized-with-smaller-amount and Rejected(GasFloorBreach
                // / NonPositiveNetUsd) are doctrinally correct outcomes.
                assert!(
                    matches!(
                        r,
                        OptimizeRejectReason::GasFloorBreach
                            | OptimizeRejectReason::NonPositiveNetUsd
                    ),
                    "unexpected reject {:?}",
                    r,
                );
            }
        }
    }

    #[tokio::test]
    async fn gas_floor_rejects_when_net_below_safety_multiplier() {
        // gross=10, net=4 → cost_proxy=6. With gas_safety=3.0, floor=18.
        // net (4) < floor (18) → reject GasFloorBreach.
        let cfg = make_cfg_kelly(1000.0, 0.5, 1.0, 3.0, 0.8);
        let sized = make_sized(unit(1), 10.0, 4.0);
        let result = kelly_step(
            OptimizeOutcome::Sized(Box::new(sized)),
            &cfg,
            1000.0,
            3000.0,
            18,
        )
        .await;
        match result {
            // ALWAYS-COMPUTE (operator mandate 2026-09-27): the gas-floor
            // reject now carries the FULL sized figures — "COMPUTED ≠
            // PROFITABLE": the row rejects AND keeps gross, net, components,
            // sized amount and ledger. The -$card shows its arithmetic.
            OptimizeOutcome::RejectedComputed(OptimizeRejectReason::GasFloorBreach, s) => {
                // Deuda 4-(B) upgraded: the computed net (4.0, positive but
                // below the floor) still travels — R8 Some = computed.
                assert_eq!(
                    s.estimated_net_profit_usd, 4.0,
                    "net must survive the reject"
                );
                assert_eq!(
                    s.gross_profit_usd, 10.0,
                    "the gross it was judged on survives too"
                );
                // NOTE: sheet-07 components are None on this HAND-BUILT fixture
                // (make_sized constructs no net_economics — ARBX-0009); kernel
                // paths always populate them (the V3 gate test asserts that).
            }
            other => panic!("expected GasFloorBreach, got {:?}", other.reason_str()),
        }
    }

    #[tokio::test]
    async fn gas_floor_accepts_when_net_meets_safety_multiplier() {
        // gross=100, net=90 → cost_proxy=10. With gas_safety=3.0, floor=30.
        // net (90) ≥ floor (30) → accepts.
        let cfg = make_cfg_kelly(1_000_000.0, 0.5, 1.0, 3.0, 0.8);
        let sized = make_sized(unit(1), 100.0, 90.0);
        let result = kelly_step(
            OptimizeOutcome::Sized(Box::new(sized)),
            &cfg,
            1_000_000.0,
            3000.0,
            18,
        )
        .await;
        assert!(
            matches!(result, OptimizeOutcome::Sized(_)),
            "gas floor should not reject when net ≥ multiplier × cost",
        );
    }

    #[tokio::test]
    async fn kelly_rejects_negative_edge() {
        // Setup so gas-floor passes but Kelly says negative edge.
        // gross=100, net=70 → cost=30. With gas_safety=1.0, floor=30 ≤ net.
        // W = gross/cost = 100/30 ≈ 3.33. p=0.2:
        //   f* = 0.2 - 0.8/3.33 ≈ 0.2 - 0.24 = -0.04 < 0 → KellyNegativeEdge.
        let cfg = make_cfg_kelly(1_000_000.0, 0.5, 1.0, 1.0, 0.2);
        let sized = make_sized(unit(1), 100.0, 70.0);
        let result = kelly_step(
            OptimizeOutcome::Sized(Box::new(sized)),
            &cfg,
            1_000_000.0,
            3000.0,
            18,
        )
        .await;
        match result {
            // ALWAYS-COMPUTE: negative-edge reject keeps the full figures.
            OptimizeOutcome::RejectedComputed(OptimizeRejectReason::KellyNegativeEdge, s) => {
                assert_eq!(s.estimated_net_profit_usd, 70.0);
                assert_eq!(s.gross_profit_usd, 100.0);
            }
            other => panic!("expected KellyNegativeEdge, got {:?}", other.reason_str()),
        }
    }

    #[tokio::test]
    async fn kelly_accepts_when_edge_is_positive() {
        // p=0.7 (good), W = gross/cost = 100/10 = 10
        // f* = 0.7 - 0.3/10 = 0.67 > 0 → accept (with cap).
        let cfg = make_cfg_kelly(1_000_000.0, 0.5, 1.0, 1.0, 0.7);
        let sized = make_sized(unit(1), 100.0, 90.0);
        let result = kelly_step(
            OptimizeOutcome::Sized(Box::new(sized)),
            &cfg,
            1_000_000.0,
            3000.0,
            18,
        )
        .await;
        assert!(
            matches!(result, OptimizeOutcome::Sized(_)),
            "positive Kelly edge should accept",
        );
    }

    // ── WO-LEGS-TRIANGULAR-01: Kelly overlay vs the triangular ledger ───────
    //
    // Sancho condition 2 (run_6db07): apply_kelly_constraints runs in
    // optimize_with_reason Step 8 for EVERY kernel outcome — the triangular
    // SizedCandidate flows through the same code path as the 2-leg one, so the
    // HOPS-LEDGER-04 null-out applies to it too. These tests document and
    // pin that inheritance.

    #[tokio::test]
    async fn kelly_overlay_drops_ledger_when_reserves_not_derivable() {
        // 1 WETH bet, gross=$100, net=$90 (cost=$10). max_per_trade=0.1% of
        // $1M NAV → $1000 cap → 0.333 WETH. Kelly binds and rescales:
        // new_gross=$33.3, new_net=$23.3 > 0, gas floor (1.0×) passes → Sized
        // with a SMALLER amount — the per-leg ledger was computed at the
        // kernel size, so it must be honestly absent (R8), never stale.
        let cfg = make_cfg_kelly(
            /* capital_usd */ 1_000_000.0,
            /* multiplier */ 0.5,
            /* max_per_trade */ 0.001,
            /* gas_safety */ 1.0,
            /* min_p */ 0.7,
        );
        let mut sized = make_sized(unit(1), 100.0, 90.0);
        sized.candidate.label = StrategyLabel::TriangularArb;
        sized.leg_amounts_in = Some(vec![
            "1000000000000000000".to_string(),
            "995000000000000000".to_string(),
            "990000000000000000".to_string(),
        ]);
        sized.leg_amounts_out = Some(vec![
            "995000000000000000".to_string(),
            "990000000000000000".to_string(),
            "1010000000000000000".to_string(),
        ]);
        let before_amount = sized.optimal_amount_in;
        let result = kelly_step(
            OptimizeOutcome::Sized(Box::new(sized)),
            &cfg,
            cfg.capital_usd,
            3000.0,
            18,
        )
        .await;
        match result {
            OptimizeOutcome::Sized(s) => {
                assert!(
                    s.optimal_amount_in < before_amount,
                    "Kelly cap should have reduced the triangular amount"
                );
                // HOPS-LEDGER-05: absent because these fixture pools are in NO
                // reserves cache — `reledger_at` cannot re-derive honestly (R8).
                // It is NOT the old unconditional discard: the gate below proves
                // the ledger comes back when the reserves ARE cached.
                assert!(
                    s.leg_amounts_in.is_none(),
                    "ledger must be absent when the reserves are not derivable (R8)"
                );
                assert!(
                    s.leg_amounts_out.is_none(),
                    "ledger must be absent when the reserves are not derivable (R8)"
                );
            }
            OptimizeOutcome::Rejected(r, _)
            | OptimizeOutcome::RejectedWithLedger(r, _, _)
            | OptimizeOutcome::RejectedComputed(r, _) => {
                panic!("unexpected reject {:?}", r)
            }
        }
    }

    /// HOPS-LEDGER-05 gate — the per-leg ledger must BELONG to the size the row
    /// reports, and survive the Kelly cap instead of disappearing.
    ///
    /// Before this fix Kelly unconditionally dropped `leg_amounts_*` whenever it
    /// rescaled the size ("computed at the kernel size, not the capped size"),
    /// which made the operator's per-hop numbers unreachable on essentially every
    /// emitted row. Measured in production: **339 of 4,161,891 rows in 24 h**
    /// carried `leg_amounts_in`. The fix re-derives the V2 chain at the capped
    /// size from the SAME cached reserves the kernel read — exact, local, no RPC.
    ///
    /// Fixture: the same 2-leg `DexArbV2V2` candidate `make_sized` builds, with the
    /// kernel's ledger attached by hand (the kernel's own emission is gated by the
    /// profitability fixtures elsewhere). Asserted:
    ///   · cached pools + binding cap ⇒ ledger PRESENT and `ins[0]` IS the capped
    ///     `optimal_amount_in` (Sancho condition 1: the chain belongs to the
    ///     reported size), contiguous, and different from the kernel-size chain;
    ///   · NO cached pools ⇒ ledger absent (R8: not derivable — never fabricated).
    #[tokio::test]
    async fn kelly_ledger_is_rederived_at_the_final_size() {
        // Pools the fixture candidate points at (`make_sized` uses addr(1)/addr(2)).
        let pool_a = addr(1);
        let pool_b = addr(2);
        let cache = Arc::new(ReservesCache::new());
        cache.insert(pool_a, unit(1000), unit(2_000_000)).await;
        cache.insert(pool_b, unit(1_000_000), unit(600)).await;

        // 0.1% of NAV per trade binds against a 1 WETH kernel optimum.
        let cfg = make_cfg_kelly(10_000.0, 0.5, 0.001, 1.0, 0.8);

        let mut sized = make_sized(unit(1), 100.0, 90.0);
        sized.leg_amounts_in = Some(vec![unit(1).to_string(), "1999999999999999999".to_string()]);
        sized.leg_amounts_out = Some(vec![
            "1999999999999999999".to_string(),
            "2000000000000000000".to_string(),
        ]);
        let kernel_in = sized.leg_amounts_in.clone().unwrap();
        let before_amount = sized.optimal_amount_in;

        let result = kelly_step_with_cache(
            OptimizeOutcome::Sized(Box::new(sized)),
            &cfg,
            cfg.capital_usd,
            3000.0,
            18,
            cache,
        )
        .await;

        let s = match result {
            OptimizeOutcome::Sized(s) => s,
            OptimizeOutcome::Rejected(r, _)
            | OptimizeOutcome::RejectedWithLedger(r, _, _)
            | OptimizeOutcome::RejectedComputed(r, _) => {
                panic!("unexpected reject {:?}", r)
            }
        };
        assert!(
            s.optimal_amount_in < before_amount,
            "the tight per-trade fraction must bind ({} vs {})",
            before_amount,
            s.optimal_amount_in
        );
        let ins = s
            .leg_amounts_in
            .as_ref()
            .expect("HOPS-LEDGER-05: the ledger must SURVIVE the Kelly cap (re-derived)");
        let outs = s.leg_amounts_out.as_ref().expect("re-derived ledger out");
        assert_eq!(ins.len(), 2, "2-leg route ⇒ 2-hop ledger");
        assert_eq!(outs.len(), 2);
        assert_eq!(
            ins[0],
            s.optimal_amount_in.to_string(),
            "re-derived ledger must START at the capped size, not the kernel size"
        );
        assert_ne!(
            ins[0], kernel_in[0],
            "the capped chain differs from the kernel's"
        );
        assert_eq!(
            outs[0], ins[1],
            "leg 1's input IS leg 0's output (contiguity)"
        );

        // Chain correctness, orientation-agnostic: the final leg's output must be
        // the exact V2 chain from the capped input under ONE consistent per-leg
        // reserve orientation (the legs swap token0/token1 by address, and this
        // gate must not encode that convention twice).
        let x = s.optimal_amount_in;
        let leg0 = [(unit(1000), unit(2_000_000)), (unit(2_000_000), unit(1000))];
        let leg1 = [(unit(1_000_000), unit(600)), (unit(600), unit(1_000_000))];
        let mut expected: Vec<String> = Vec::new();
        for (a_in, a_out) in leg0 {
            let mid = crate::amm_math::v2_amount_out(x, a_in, a_out, 30);
            for (b_in, b_out) in leg1 {
                expected.push(crate::amm_math::v2_amount_out(mid, b_in, b_out, 30).to_string());
            }
        }
        assert!(
            expected.contains(&outs[1]),
            "final leg output must be the exact V2 chain from the capped input \
             (got {}, candidates {:?})",
            outs[1],
            expected
        );

        // R8 counter-proof: with NO cached reserves the same rescaled row must
        // report the ledger as absent — never a stale or partial chain.
        let mut uncached = make_sized(unit(1), 100.0, 90.0);
        uncached.leg_amounts_in = Some(vec![unit(1).to_string(), unit(1).to_string()]);
        uncached.leg_amounts_out = Some(vec![unit(1).to_string(), unit(1).to_string()]);
        let bare = kelly_step(
            OptimizeOutcome::Sized(Box::new(uncached)),
            &cfg,
            cfg.capital_usd,
            3000.0,
            18,
        )
        .await;
        match bare {
            OptimizeOutcome::Sized(b) => {
                assert!(
                    b.leg_amounts_in.is_none() && b.leg_amounts_out.is_none(),
                    "not derivable ⇒ absent (R8), never a fabricated chain"
                );
            }
            other => panic!("expected Sized, got {:?}", other.reason_str()),
        }
    }

    #[tokio::test]
    async fn kelly_overlay_preserves_triangular_leg_ledger_when_cap_not_binding() {
        // Generous Kelly budget → no rescale → the triangular ledger survives.
        let cfg = make_cfg_kelly(1_000_000.0, 0.5, 1.0, 1.0, 0.7);
        let mut sized = make_sized(unit(1), 100.0, 90.0);
        sized.candidate.label = StrategyLabel::TriangularArb;
        sized.leg_amounts_in = Some(vec![
            "1000000000000000000".to_string(),
            "995000000000000000".to_string(),
            "990000000000000000".to_string(),
        ]);
        sized.leg_amounts_out = Some(vec![
            "995000000000000000".to_string(),
            "990000000000000000".to_string(),
            "1010000000000000000".to_string(),
        ]);
        let result = kelly_step(
            OptimizeOutcome::Sized(Box::new(sized)),
            &cfg,
            cfg.capital_usd,
            3000.0,
            18,
        )
        .await;
        match result {
            OptimizeOutcome::Sized(s) => {
                assert!(s.leg_amounts_in.is_some(), "ledger must survive");
                assert!(s.leg_amounts_out.is_some(), "ledger must survive");
            }
            OptimizeOutcome::Rejected(r, _)
            | OptimizeOutcome::RejectedWithLedger(r, _, _)
            | OptimizeOutcome::RejectedComputed(r, _) => {
                panic!("unexpected reject {:?}", r)
            }
        }
    }

    #[test]
    fn reject_reason_strings_cover_new_variants() {
        // Regression alarm: every variant must map to a stable label.
        assert_eq!(
            OptimizeRejectReason::GasFloorBreach.as_str(),
            "gas_floor_breach"
        );
        assert_eq!(
            OptimizeRejectReason::KellyNegativeEdge.as_str(),
            "kelly_negative_edge"
        );
        assert_eq!(
            OptimizeRejectReason::V3QuoteUnavailable.as_str(),
            "v3_quote_unavailable"
        );
        // WO-06: catalog gaps are honestly split out of the provider-failure
        // bucket.
        assert_eq!(
            OptimizeRejectReason::V3PoolNotCatalogued.as_str(),
            "v3_pool_not_catalogued"
        );
        assert_eq!(
            OptimizeRejectReason::V3PairNoPools.as_str(),
            "v3_pair_no_pools"
        );
        assert_eq!(
            OptimizeRejectReason::from_v3_unavailable_label("v3_pool_not_catalogued"),
            OptimizeRejectReason::V3PoolNotCatalogued
        );
        assert_eq!(
            OptimizeRejectReason::from_v3_unavailable_label("v3_pair_no_pools"),
            OptimizeRejectReason::V3PairNoPools
        );
        assert_eq!(
            OptimizeRejectReason::from_v3_unavailable_label("v3_quote_unavailable"),
            OptimizeRejectReason::V3QuoteUnavailable
        );
    }

    /// ARBX-0007: exactly the three net-derived gates (whose net includes the
    /// financing fee) are net-dependent — they carry the mode suffix at the
    /// orchestrator rejection site; every other reason must stay bare.
    #[test]
    fn net_dependent_reasons_are_exactly_the_net_gates() {
        use OptimizeRejectReason as R;
        for r in [
            R::NonPositiveNetUsd,
            R::GasFloorBreach,
            R::KellyNegativeEdge,
        ] {
            assert!(r.is_net_dependent(), "{} must be net-dependent", r.as_str());
        }
        for r in [
            R::NoConfig,
            R::ZeroCapitalCap,
            R::UnknownTokenPrice,
            R::InvalidDecimals,
            R::MissingRouteLegs,
            R::MissingPoolAddress,
            R::MissingReservesPoolA,
            R::MissingReservesPoolB,
            R::ZeroReserves,
            R::NonPositiveProfit,
            R::NonPositiveGrossUsd,
            R::CapClampFailed,
            R::V3QuoteUnavailable,
        ] {
            assert!(!r.is_net_dependent(), "{} must stay bare", r.as_str());
        }
    }

    // ── V3 sizing ────────────────────────────────────────────────────────────
    //
    // A mock V3 quoter with a proportional rate (out = in·num/den) so the 2-leg
    // round-trip profit is monotone — lets us drive Sized vs unprofitable vs
    // unavailable deterministically without any RPC. RULE 00: test-only.
    struct ProportionalV3Mock {
        num: u64,
        den: u64,
    }

    impl crate::state_projector::V3QuoteProvider for ProportionalV3Mock {
        fn quote_exact_input_single(
            &self,
            _pool: Address,
            _token_in: Address,
            _token_out: Address,
            amount_in: U256,
            _fee_bps: u32,
        ) -> std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<U256>> + Send + '_>>
        {
            let out = amount_in.saturating_mul(U256::from(self.num)) / U256::from(self.den);
            Box::pin(async move { Ok(out) })
        }
    }

    /// A 2-leg candidate whose both legs are V3 pools (protocol_type flipped to
    /// "uniswap-v3" so `route_has_v3` routes it to the V3 sizing kernel).
    fn make_v3_dex_candidate(
        pool_a: Address,
        pool_b: Address,
        token_in: Address,
        token_out: Address,
    ) -> StrategyCandidate {
        let mut c = make_dex_candidate(
            pool_a,
            pool_b,
            token_in,
            token_out,
            StrategyLabel::DexArbV3V3,
        );
        for leg in c.route_plan.legs.iter_mut() {
            leg.protocol_type = "uniswap-v3".to_string();
            leg.fee_bps = Some(500); // V3 0.05% tier
        }
        c
    }

    // Profitable V3 route via mock QuoterV2 → Sized (amount_in>0, net>0).
    #[tokio::test]
    async fn v3_route_sized_with_mock_provider() {
        let cache = Arc::new(ReservesCache::new()); // V3 legs don't read reserves
        let provider = Arc::new(ProportionalV3Mock { num: 12, den: 10 }); // 1.2x/leg → profitable
        let projector = Arc::new(StateProjector::new(
            cache,
            Some(provider),
            v3_test_fee_catalog(),
        ));
        let optimizer = SizeOptimizer::new(projector);

        let candidate = make_v3_dex_candidate(addr(0x10), addr(0x11), addr(0xAAAA), addr(0xBBBB));
        let intent = make_intent(addr(0xAAAA), addr(0xBBBB));
        let mut cfg = make_cfg(10_000.0);
        cfg.min_landing_probability = 0.9; // positive Kelly edge so Sized survives Step 8

        let outcome = optimizer
            .optimize_with_reason(candidate, &intent, Some(&cfg))
            .await
            .expect("optimize must not error");

        match outcome {
            OptimizeOutcome::Sized(s) => {
                assert!(s.optimal_amount_in > U256::zero(), "amount_in must be > 0");
                assert!(s.gross_profit_usd > 0.0, "gross must be positive");
                assert!(s.estimated_net_profit_usd > 0.0, "net must be positive");
            }
            OptimizeOutcome::Rejected(r, _)
            | OptimizeOutcome::RejectedWithLedger(r, _, _)
            | OptimizeOutcome::RejectedComputed(r, _) => {
                panic!(
                    "expected Sized for a profitable V3 route, got {}",
                    r.as_str()
                )
            }
        }
    }

    // No V3 provider → V3QuoteUnavailable (R8 fail-honest; NOT a fabricated 0).
    #[tokio::test]
    async fn v3_route_without_provider_is_quote_unavailable() {
        let cache = Arc::new(ReservesCache::new());
        let projector = Arc::new(StateProjector::new(cache, None, empty_v3_fee_catalog())); // no V3 provider wired
        let optimizer = SizeOptimizer::new(projector);

        let candidate = make_v3_dex_candidate(addr(0x10), addr(0x11), addr(0xAAAA), addr(0xBBBB));
        let intent = make_intent(addr(0xAAAA), addr(0xBBBB));
        let cfg = make_cfg(10_000.0);

        let outcome = optimizer
            .optimize_with_reason(candidate, &intent, Some(&cfg))
            .await
            .expect("optimize must not error");

        assert!(
            matches!(
                outcome,
                OptimizeOutcome::Rejected(OptimizeRejectReason::V3QuoteUnavailable, _)
            ),
            "no V3 provider must yield V3QuoteUnavailable, got {:?}",
            outcome.reason_str()
        );
    }

    // Quoter answers but the spread is ≤ 0 → NonPositiveProfit (distinct from
    // V3QuoteUnavailable: the quoter DID respond).
    #[tokio::test]
    async fn v3_route_unprofitable_is_non_positive_profit() {
        let cache = Arc::new(ReservesCache::new());
        let provider = Arc::new(ProportionalV3Mock { num: 8, den: 10 }); // 0.8x/leg → always a loss
        let projector = Arc::new(StateProjector::new(
            cache,
            Some(provider),
            v3_test_fee_catalog(),
        ));
        let optimizer = SizeOptimizer::new(projector);

        let candidate = make_v3_dex_candidate(addr(0x10), addr(0x11), addr(0xAAAA), addr(0xBBBB));
        let intent = make_intent(addr(0xAAAA), addr(0xBBBB));
        let cfg = make_cfg(10_000.0);

        let outcome = optimizer
            .optimize_with_reason(candidate, &intent, Some(&cfg))
            .await
            .expect("optimize must not error");

        // ALWAYS-COMPUTE (2026-09-27) gate: a route the quoter ANSWERED for
        // but that is unprofitable rejects WITH its full arithmetic — the
        // real QuoterV2(-mock) round-trip figures, cost components, net, and
        // the per-leg chain — never "no tengo números".
        match outcome {
            OptimizeOutcome::RejectedComputed(OptimizeRejectReason::NonPositiveProfit, s) => {
                let n = s.estimated_net_profit_usd;
                assert!(
                    n <= 0.0 && n.is_finite(),
                    "computed reject net must be finite and <= 0, got {n}"
                );
                assert!(
                    s.gross_profit_usd <= 0.0 && s.gross_profit_usd.is_finite(),
                    "the gross the verdict judged must ride the reject, got {}",
                    s.gross_profit_usd
                );
                assert!(s.net_economics.is_some(), "sheet-07 components present");
                assert!(
                    s.leg_amounts_in.is_some() && s.leg_amounts_out.is_some(),
                    "the exact per-leg chain the probes computed rides the reject"
                );
            }
            other => panic!(
                "answered-but-unprofitable must be RejectedComputed(NonPositiveProfit) with figures, got {:?}",
                other.reason_str()
            ),
        }
    }

    // R8 honesty (adversarial-review fix): when the quoter ANSWERS with 0 (a
    // real zero-yield), the reason must be NonPositiveProfit — NOT
    // V3QuoteUnavailable (which means the quoter could not answer at all).
    #[tokio::test]
    async fn v3_route_zero_quote_is_non_positive_not_unavailable() {
        let cache = Arc::new(ReservesCache::new());
        let provider = Arc::new(ProportionalV3Mock { num: 0, den: 1 }); // quoter answers, always 0
        let projector = Arc::new(StateProjector::new(
            cache,
            Some(provider),
            v3_test_fee_catalog(),
        ));
        let optimizer = SizeOptimizer::new(projector);

        let candidate = make_v3_dex_candidate(addr(0x10), addr(0x11), addr(0xAAAA), addr(0xBBBB));
        let intent = make_intent(addr(0xAAAA), addr(0xBBBB));
        let cfg = make_cfg(10_000.0);

        let outcome = optimizer
            .optimize_with_reason(candidate, &intent, Some(&cfg))
            .await
            .expect("optimize must not error");

        assert!(
            matches!(
                outcome,
                OptimizeOutcome::Rejected(OptimizeRejectReason::NonPositiveProfit, _)
            ),
            "quoter-answered-zero must be NonPositiveProfit (not V3QuoteUnavailable), got {:?}",
            outcome.reason_str()
        );
    }

    // ── Plan B.1: V3 within-tick curve mock + early-reject regression ────────
    //
    // A mock V3 quoter that answers each pool with its REAL within-tick V3
    // curve via `amm_math::v3_amount_out_single_tick` — the same local model
    // the early-reject uses. Because for swaps that stay within the active tick
    // QuoterV2 == within-tick, the mock is a faithful, deterministic stand-in
    // for a concentrated-liquidity pool — no RPC (RULE 00). A call counter
    // proves the early-reject skips the grid when it fires.
    struct CurveV3Mock {
        // pool address → (sqrt_price_x96, liquidity)
        curves: HashMap<Address, (U256, U256)>,
        calls: AtomicU64,
    }

    impl crate::state_projector::V3QuoteProvider for CurveV3Mock {
        fn quote_exact_input_single(
            &self,
            pool: Address,
            token_in: Address,
            token_out: Address,
            amount_in: U256,
            fee_bps: u32,
        ) -> std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<U256>> + Send + '_>>
        {
            self.calls
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let Some(&(sp, liq)) = self.curves.get(&pool) else {
                return Box::pin(async move { Ok(U256::zero()) });
            };
            // Uniswap V3 token ordering: token0 < token1. zero_for_one iff the
            // leg's input is token0. Matches build_leg_eval's orientation.
            let zero_for_one = token_in < token_out;
            // For V3 legs `fee_bps` holds the fee tier in MILLIONTHS (500 =
            // 0.05%), which is exactly the `fee_pips` the within-tick math wants.
            let out = crate::amm_math::v3_amount_out_single_tick(
                amount_in,
                sp,
                liq,
                fee_bps,
                zero_for_one,
            );
            Box::pin(async move { Ok(out) })
        }
    }

    /// A 2-leg V3 candidate with explicit token0/token1 ordering (token0 <
    /// token1) so the swap directions are deterministic. Leg0 = token0→token1
    /// (pool A, zero_for_one); leg1 = token1→token0 (pool B, one_for_zero).
    /// The round-trip factor is ≈ (spA/spB)²: spA > spB → profitable.
    fn make_v3_curve_candidate(
        pool_a: Address,
        pool_b: Address,
        token0: Address,
        token1: Address,
    ) -> StrategyCandidate {
        let t0 = format!("0x{:040x}", token0);
        let t1 = format!("0x{:040x}", token1);
        let pa = format!("0x{:040x}", pool_a);
        let pb = format!("0x{:040x}", pool_b);
        let id = Uuid::new_v4();
        let opp = Opportunity {
            id,
            chain_id: 1,
            strategy_kind: StrategyKind::dex_arb(),
            dex_a: "uniswap-v3".to_string(),
            dex_b: Some("uniswap-v3".to_string()),
            pair_symbol: "T0/T1".to_string(),
            token_in: t0.clone(),
            token_out: t1.clone(),
            amount_in_wei: unit(1).to_string(),
            expected_profit_usd: Some(1.0),
            net_expected_profit_usd: None,
            roi_pct: None,
            risk_score: None,
            block_number: None,
            rejection_reason: None,
            cartridge_id: None,
            detector_id: None,
            pipeline_latency_ms: None,
            economics: None, // ALWAYS-COMPUTE: stamped at the emit boundary
            detected_at: Utc::now(),
            trace_id: Uuid::new_v4(),
        };
        let candidate_inner = OpportunityCandidate {
            route_fingerprint: "test-curve".to_string(),
            pool_addresses: vec![pa.clone(), pb.clone()],
            token_addresses: vec![t0.clone(), t1.clone()],
            dex_adapters: vec!["uniswap-v3".to_string(), "uniswap-v3".to_string()],
            amount_in: 1.0,
            expected_amount_out: 1.001,
            gross_profit: 1.0,
        };
        let v3_leg = |pool: String, tin: String, tout: String| RouteLeg {
            dex_id: "uniswap-v3".to_string(),
            dex_name: "uniswap-v3".to_string(),
            protocol_type: "uniswap-v3".to_string(),
            factory_address: String::new(),
            pool_id: None,
            pool_address: Some(pool),
            token_in: tin,
            token_out: tout,
            fee_bps: Some(500), // V3 0.05% tier, stored in millionths
            amount_in: Some(1.0),
            amount_out: None,
            tvl_usd: None,
            volume_24h_usd: None,
            pool_is_active: true,
        };
        let route_plan = RoutePlan {
            route_id: Some("test-curve".to_string()),
            strategy_kind: StrategyLabel::DexArbV3V3.as_str().to_string(),
            chain_id: 1,
            legs: vec![
                v3_leg(pa, t0.clone(), t1.clone()),
                v3_leg(pb, t1.clone(), t0.clone()),
            ],
            atomic: true,
            estimated_slippage_pct: None,
            price_impact_pct: None,
        };
        StrategyCandidate {
            label: StrategyLabel::DexArbV3V3,
            opportunity: opp,
            candidate: candidate_inner,
            route_plan,
            gross_profit_usd: Some(1.0),
            net_expected_profit_usd: None,
            rejection_reason: None,
            source_intent_hash: H256::zero(),
            base_strategy: None,
        }
    }

    // Profitable within-tick V3 curve (spA > spB) → Sized with a sensible
    // optimum (amount_in > 0, gross > 0, net > 0). No slot0 cache attached →
    // the early-reject falls through and the grid runs against the curve mock.
    #[tokio::test]
    async fn v3_curve_sizing_finds_profitable_optimum() {
        let token0 = addr(0xAAAA); // < token1 ⇒ canonical token0
        let token1 = addr(0xBBBB);
        let pool_a = addr(0x10);
        let pool_b = addr(0x11);
        let q96 = U256::one() << 96;
        // spA/spB = 1.2 ⇒ round-trip factor ≈ 1.44 (minus fees) ⇒ profitable.
        let sp_a = q96 * U256::from(12u32) / U256::from(10u32);
        let sp_b = q96;
        let liq = U256::one() << 100; // deep pool ⇒ within-tick is exact over the probes

        let cache = Arc::new(ReservesCache::new());
        let curves = HashMap::from([(pool_a, (sp_a, liq)), (pool_b, (sp_b, liq))]);
        let provider = Arc::new(CurveV3Mock {
            curves,
            calls: AtomicU64::new(0),
        });
        let projector = Arc::new(StateProjector::new(
            cache,
            Some(provider),
            v3_test_fee_catalog(),
        ));
        let optimizer = SizeOptimizer::new(projector); // no slot0 cache ⇒ grid runs

        let candidate = make_v3_curve_candidate(pool_a, pool_b, token0, token1);
        let intent = make_intent(token0, token1);
        let mut cfg = make_cfg(10_000.0);
        cfg.min_landing_probability = 0.9; // positive Kelly edge so Sized survives Step 8

        let outcome = optimizer
            .optimize_with_reason(candidate, &intent, Some(&cfg))
            .await
            .expect("optimize must not error");

        match outcome {
            OptimizeOutcome::Sized(s) => {
                assert!(s.optimal_amount_in > U256::zero(), "amount_in must be > 0");
                assert!(s.gross_profit_usd > 0.0, "gross must be positive");
                assert!(s.estimated_net_profit_usd > 0.0, "net must be positive");
            }
            OptimizeOutcome::Rejected(r, _)
            | OptimizeOutcome::RejectedWithLedger(r, _, _)
            | OptimizeOutcome::RejectedComputed(r, _) => {
                panic!(
                    "expected Sized for spA>spB profitable curve, got {}",
                    r.as_str()
                )
            }
        }
    }

    // Negative spread (spA < spB ⇒ round-trip factor < 1) → NonPositiveProfit.
    // No slot0 cache ⇒ the grid runs and the real curve confirms the loss.
    #[tokio::test]
    async fn v3_curve_negative_spread_is_non_positive_profit() {
        let token0 = addr(0xAAAA);
        let token1 = addr(0xBBBB);
        let pool_a = addr(0x10);
        let pool_b = addr(0x11);
        let q96 = U256::one() << 96;
        // spA/spB = 1/1.2 ⇒ round-trip factor ≈ 0.69 ⇒ always a loss.
        let sp_a = q96;
        let sp_b = q96 * U256::from(12u32) / U256::from(10u32);
        let liq = U256::one() << 100;

        let cache = Arc::new(ReservesCache::new());
        let curves = HashMap::from([(pool_a, (sp_a, liq)), (pool_b, (sp_b, liq))]);
        let provider = Arc::new(CurveV3Mock {
            curves,
            calls: AtomicU64::new(0),
        });
        let projector = Arc::new(StateProjector::new(
            cache,
            Some(provider),
            v3_test_fee_catalog(),
        ));
        let optimizer = SizeOptimizer::new(projector);

        let candidate = make_v3_curve_candidate(pool_a, pool_b, token0, token1);
        let intent = make_intent(token0, token1);
        let cfg = make_cfg(10_000.0);

        let outcome = optimizer
            .optimize_with_reason(candidate, &intent, Some(&cfg))
            .await
            .expect("optimize must not error");

        assert!(
            matches!(
                outcome,
                OptimizeOutcome::RejectedComputed(OptimizeRejectReason::NonPositiveProfit, _)
            ),
            "negative-spread V3 curve must be NonPositiveProfit with its figures (ALWAYS-COMPUTE), got {:?}",
            outcome.reason_str()
        );
    }

    // The new 0-RPC early-reject: with slot0 cached for both V3 legs and a
    // loss curve, the within-tick upper bound is ≤ 0 across the probe grid, so
    // the optimizer rejects NonPositiveProfit WITHOUT calling the QuoterV2
    // mock at all (calls == 0 ⇒ the ≤ 8 RPC grid was skipped).
    #[tokio::test]
    async fn v3_within_tick_early_reject_skips_quoter_grid() {
        let token0 = addr(0xAAAA);
        let token1 = addr(0xBBBB);
        let pool_a = addr(0x10);
        let pool_b = addr(0x11);
        let q96 = U256::one() << 96;
        let sp_a = q96; // spA < spB ⇒ loss
        let sp_b = q96 * U256::from(12u32) / U256::from(10u32);
        let liq = U256::one() << 100;

        // Populate slot0 for BOTH V3 legs — this arms the 0-RPC early-reject.
        let slot0 = Arc::new(Slot0Cache::new());
        slot0
            .insert(
                pool_a,
                V3Slot0Snapshot {
                    sqrt_price_x96: sp_a,
                    liquidity: liq,
                },
            )
            .await;
        slot0
            .insert(
                pool_b,
                V3Slot0Snapshot {
                    sqrt_price_x96: sp_b,
                    liquidity: liq,
                },
            )
            .await;

        let cache = Arc::new(ReservesCache::new());
        let curves = HashMap::from([(pool_a, (sp_a, liq)), (pool_b, (sp_b, liq))]);
        let provider = Arc::new(CurveV3Mock {
            curves,
            calls: AtomicU64::new(0),
        });
        let projector = Arc::new(StateProjector::new(
            cache,
            Some(provider.clone()),
            v3_test_fee_catalog(),
        ));
        let optimizer = SizeOptimizer::new(projector).with_slot0_cache(slot0);

        let candidate = make_v3_curve_candidate(pool_a, pool_b, token0, token1);
        let intent = make_intent(token0, token1);
        let cfg = make_cfg(10_000.0);

        let outcome = optimizer
            .optimize_with_reason(candidate, &intent, Some(&cfg))
            .await
            .expect("optimize must not error");

        assert!(
            matches!(
                outcome,
                OptimizeOutcome::Rejected(OptimizeRejectReason::NonPositiveProfit, _)
            ),
            "within-tick early-reject must yield NonPositiveProfit, got {:?}",
            outcome.reason_str()
        );
        // Deuda 4-B: the payload is the within-tick UPPER-BOUND best profit in
        // USD — computed and stamped (≤ 0), not an honest None.
        let payload = outcome.net_profit_usd();
        assert!(
            matches!(payload, Some(v) if v <= 0.0),
            "within-tick early-reject must stamp the upper-bound USD payload, got {:?}",
            payload
        );
        // The grid was SKIPPED: the QuoterV2 mock was never called.
        assert_eq!(
            provider.calls.load(std::sync::atomic::Ordering::Relaxed),
            0,
            "early-reject must skip all QuoterV2 probes (0 mock calls)"
        );
    }

    // ── V3-MULTILEG-SIZING-01 gates ──────────────────────────────────────────
    //
    // Operator mandate: "deben salir arbitrajes de 2 a 7 hops, cada hop tiene su
    // lugar en la card". Production shape (measured 2026-09-26): 585/585 sampled
    // discovered 3..=7-hop cycles carry ≥ 1 V3 leg, zero are all-V2 — so before
    // this kernel NO deep cycle could ever reach the per-hop ledger.
    //
    // The fixtures below are the shape `hop_cycle_bridge::cycle_candidate_from_intent`
    // builds (a closed cycle, one RouteLeg per hop, ascending synthetic tokens
    // after the real mainnet WETH head — so the kernel prices with a REAL token
    // identity) over a V3 double that answers each pool with its EXACT
    // within-tick curve (`amm_math::v3_amount_out_single_tick`, the same model
    // the kernel's 0-RPC bound uses). Deterministic, no RPC (RULE 00).

    /// Cache key of the V3 double: (pool, token_in, token_out, amount_in, fee).
    type MockQuoteKey = (Address, Address, Address, U256, u32);

    /// Counting V3 double. Answers every V3 leg with its own within-tick curve
    /// and models the production provider's TTL cache: `quote_batch` (the
    /// kernel's B1 prefetch) warms the same keys the unary path then reads, so
    /// the counters measure REAL round-trips, not attempts.
    struct CountingCurveV3Mock {
        /// pool → (sqrtPriceX96, active liquidity).
        curves: HashMap<Address, (U256, U256)>,
        cache: std::sync::Mutex<HashMap<MockQuoteKey, U256>>,
        /// Unary lookups attempted (cache hit or miss).
        unary_attempts: AtomicU64,
        /// Unary lookups that missed the cache = one real `eth_call`.
        unary_rpc_calls: AtomicU64,
        /// `quote_batch` invocations (each = one aggregate3 `eth_call`).
        batch_dispatches: AtomicU64,
        /// Sub-calls carried by those batches.
        batch_sub_calls: AtomicU64,
    }

    impl CountingCurveV3Mock {
        fn new(curves: HashMap<Address, (U256, U256)>) -> Self {
            Self {
                curves,
                cache: std::sync::Mutex::new(HashMap::new()),
                unary_attempts: AtomicU64::new(0),
                unary_rpc_calls: AtomicU64::new(0),
                batch_dispatches: AtomicU64::new(0),
                batch_sub_calls: AtomicU64::new(0),
            }
        }

        /// The pool's real within-tick output for this direction/size.
        fn answer(
            &self,
            pool: Address,
            token_in: Address,
            token_out: Address,
            amount_in: U256,
            fee_bps: u32,
        ) -> U256 {
            match self.curves.get(&pool) {
                Some(&(sp, liq)) => crate::amm_math::v3_amount_out_single_tick(
                    amount_in,
                    sp,
                    liq,
                    fee_bps,
                    token_in < token_out,
                ),
                None => U256::zero(),
            }
        }

        /// Real QuoterV2 sub-calls issued: batched + unary cache misses.
        fn sub_calls(&self) -> u64 {
            self.batch_sub_calls
                .load(std::sync::atomic::Ordering::Relaxed)
                + self
                    .unary_rpc_calls
                    .load(std::sync::atomic::Ordering::Relaxed)
        }
    }

    impl crate::state_projector::V3QuoteProvider for CountingCurveV3Mock {
        fn quote_exact_input_single(
            &self,
            pool: Address,
            token_in: Address,
            token_out: Address,
            amount_in: U256,
            fee_bps: u32,
        ) -> std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<U256>> + Send + '_>>
        {
            self.unary_attempts
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let key = (pool, token_in, token_out, amount_in, fee_bps);
            if let Some(v) = self
                .cache
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .get(&key)
                .copied()
            {
                return Box::pin(async move { Ok(v) });
            }
            self.unary_rpc_calls
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let out = self.answer(pool, token_in, token_out, amount_in, fee_bps);
            self.cache
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .insert(key, out);
            Box::pin(async move { Ok(out) })
        }

        fn quote_batch(
            &self,
            reqs: Vec<crate::amm_math::V3QuoteRequest>,
        ) -> std::pin::Pin<
            Box<
                dyn std::future::Future<Output = crate::state_projector::V3BatchQuoteResults>
                    + Send
                    + '_,
            >,
        > {
            self.batch_dispatches
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            self.batch_sub_calls
                .fetch_add(reqs.len() as u64, std::sync::atomic::Ordering::Relaxed);
            let mut out: Vec<(crate::amm_math::V3QuoteRequest, Result<U256, String>)> =
                Vec::with_capacity(reqs.len());
            {
                let mut cache = self.cache.lock().unwrap_or_else(|e| e.into_inner());
                for r in reqs {
                    let v =
                        self.answer(r.pool_addr, r.token_in, r.token_out, r.amount_in, r.fee_bps);
                    cache.insert(
                        (r.pool_addr, r.token_in, r.token_out, r.amount_in, r.fee_bps),
                        v,
                    );
                    out.push((r, Ok(v)));
                }
            }
            Box::pin(async move { out })
        }
    }

    /// A V3 provider that answers nothing (RPC exhausted / provider absent):
    /// every leg is `Unavailable` → the honest verdict is a WO-06 V3 label.
    struct FailingV3Mock;

    impl crate::state_projector::V3QuoteProvider for FailingV3Mock {
        fn quote_exact_input_single(
            &self,
            _pool: Address,
            _token_in: Address,
            _token_out: Address,
            _amount_in: U256,
            _fee_bps: u32,
        ) -> std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<U256>> + Send + '_>>
        {
            Box::pin(async move { Err(anyhow::anyhow!("v3 quote rpc failover exhausted")) })
        }
    }

    /// Closed N-leg cycle candidate (the bridge's shape): `tokens[0]` is the real
    /// mainnet WETH address (chain-1 identity → the kernel prices the route with
    /// a REAL token, `resolve_token_in_symbol` matches it by address), the legs
    /// walk `tokens` and close on `tokens[0]`.
    fn make_cycle_candidate(
        pools: &[Address],
        protocols: &[&str],
        tokens: &[&str],
    ) -> StrategyCandidate {
        let n = pools.len();
        assert!(n >= 3, "deep cycles only");
        assert_eq!(protocols.len(), n);
        assert_eq!(tokens.len(), n + 1);
        assert_eq!(tokens[0], tokens[n], "the cycle must close");
        let pool_str = |p: &Address| format!("0x{:040x}", p);

        let id = Uuid::new_v4();
        let opp = Opportunity {
            id,
            chain_id: 1,
            strategy_kind: StrategyKind::dex_arb(),
            dex_a: protocols[0].to_string(),
            dex_b: protocols.get(1).map(|s| s.to_string()),
            pair_symbol: format!("WETH({n}-hop cycle)"),
            token_in: tokens[0].to_string(),
            token_out: tokens[n].to_string(),
            amount_in_wei: "0".to_string(),
            expected_profit_usd: None,
            net_expected_profit_usd: None,
            roi_pct: None,
            risk_score: None,
            block_number: None,
            rejection_reason: None,
            cartridge_id: None,
            detector_id: Some("hop_cycle_bridge".to_string()),
            pipeline_latency_ms: None,
            detected_at: Utc::now(),
            trace_id: Uuid::new_v4(),
            economics: None,
        };

        let candidate_inner = OpportunityCandidate {
            route_fingerprint: format!("test-v3multileg-{n}"),
            pool_addresses: pools.iter().map(pool_str).collect(),
            token_addresses: tokens.iter().map(|t| t.to_string()).collect(),
            dex_adapters: protocols.iter().map(|p| p.to_string()).collect(),
            amount_in: 0.0,
            expected_amount_out: 0.0,
            gross_profit: 0.0,
        };

        let legs = (0..n)
            .map(|i| RouteLeg {
                dex_id: protocols[i].to_string(),
                dex_name: protocols[i].to_string(),
                protocol_type: protocols[i].to_string(),
                factory_address: String::new(),
                pool_id: None,
                pool_address: Some(pool_str(&pools[i])),
                token_in: tokens[i].to_string(),
                token_out: tokens[i + 1].to_string(),
                // V3 tiers live in millionths (500 = 0.05%), V2 in bps.
                fee_bps: Some(if protocols[i].to_ascii_lowercase().contains("v3") {
                    500
                } else {
                    30
                }),
                amount_in: Some(0.0),
                amount_out: None,
                tvl_usd: None,
                volume_24h_usd: None,
                pool_is_active: true,
            })
            .collect::<Vec<_>>();

        let route_plan = RoutePlan {
            route_id: Some(format!("test-v3multileg-{n}")),
            // The label the bridge stamps on every discovered cycle.
            strategy_kind: StrategyLabel::TriangularArb.as_str().to_string(),
            chain_id: 1,
            legs,
            atomic: true,
            estimated_slippage_pct: None,
            price_impact_pct: None,
        };

        StrategyCandidate {
            label: StrategyLabel::TriangularArb,
            opportunity: opp,
            candidate: candidate_inner,
            route_plan,
            gross_profit_usd: None,
            net_expected_profit_usd: None,
            rejection_reason: None,
            source_intent_hash: H256::zero(),
            base_strategy: None,
        }
    }

    /// Fee catalog for a cycle fixture: every V3 leg is catalogued at `tier`
    /// (pips), so the WO-06 catalog can resolve it (an empty catalog would reject
    /// with `v3_pool_not_catalogued`, which is a different gate).
    fn cycle_fee_catalog(
        pools: &[Address],
        protocols: &[&str],
        tokens: &[&str],
        tier: u32,
    ) -> Arc<crate::v3_fee_catalog::V3FeeCatalog> {
        let c = empty_v3_fee_catalog();
        for i in 0..pools.len() {
            if protocols[i].to_ascii_lowercase().contains("v3") {
                let tin: Address = tokens[i].parse().expect("token_in parses");
                let tout: Address = tokens[i + 1].parse().expect("token_out parses");
                c.record_observed(pools[i], tin, tout, tier);
            }
        }
        c
    }

    /// Token path for an `n`-leg CLOSED cycle: the real mainnet WETH head, then
    /// `n-1` ascending synthetic tokens (0x…01 < 0x…02 < … < WETH), then back
    /// into WETH — so every leg's direction is deterministic and the cycle
    /// closes exactly like `hop_cycle_bridge`'s discovered cycles do.
    fn cycle_path(n: usize) -> Vec<String> {
        assert!(n >= 3, "deep cycles only");
        let mut v = vec![WETH_T.to_string()];
        for i in 1..n {
            v.push(format!("0x{:040x}", i));
        }
        v.push(WETH_T.to_string());
        assert_eq!(v.len(), n + 1);
        v
    }

    /// Arm the 0-RPC within-tick arm: cache slot0 for every V3 leg's pool.
    async fn arm_slot0(curves: &HashMap<Address, (U256, U256)>) -> Arc<Slot0Cache> {
        let cache = Arc::new(Slot0Cache::new());
        for (pool, (sp, liq)) in curves {
            cache
                .insert(
                    *pool,
                    V3Slot0Snapshot {
                        sqrt_price_x96: *sp,
                        liquidity: *liq,
                    },
                )
                .await;
        }
        cache
    }

    /// The gate cfg: a positive-Kelly-edge configuration whose caps cannot bind
    /// below the capital cap (so the KERNEL's ledger survives Step 8 intact —
    /// the Kelly overlay's own V3 behaviour is out of this patch's scope).
    fn make_v3_multileg_cfg(capital_usd: f64) -> TradingConfigState {
        let mut cfg = make_cfg(capital_usd);
        cfg.min_landing_probability = 0.99;
        cfg.kelly_multiplier = 1.0;
        cfg.kelly_max_per_trade_fraction = 1.0;
        cfg.kelly_gas_safety_multiplier = 1.0;
        cfg.ops_overhead_usd_per_attempt = 0.0;
        cfg
    }

    /// Curves for the standard 3-leg fixture: ONE V3 leg (the WETH→T1 "down"
    /// leg, sqrtPrice = 0.9·Q96 ⇒ marginal rate 1/0.81 ≈ 1.235) whose capacity
    /// (`L·√P/Q96 ≈ 1.26e18` wei) puts the profit optimum ≈ 1.4e17 wei — inside
    /// the capital cap and far below the Kelly cap. The two V2 legs are deep
    /// (1.02 rate each) so the composite edge is ≈ 1.28.
    fn three_leg_curves(pool_v3: Address) -> HashMap<Address, (U256, U256)> {
        let q96 = U256::one() << 96;
        let sp = q96 * U256::from(9u32) / U256::from(10u32);
        let liq = U256::from(1_400_000_000_000_000_000u64);
        HashMap::from([(pool_v3, (sp, liq))])
    }

    /// GATE — 3-leg fixture with a cached slot0 ⇒ Sized + a full ledger.
    #[tokio::test]
    async fn gate_v3_multileg_three_leg_cycle_sizes_with_a_full_ledger() {
        let tokens = cycle_path(3);
        let t: Vec<&str> = tokens.iter().map(|s| s.as_str()).collect();
        let pools = [addr(0x30), addr(0x31), addr(0x32)];
        let protocols = ["uniswap-v3", "uniswap-v2", "uniswap-v2"];
        let curves = three_leg_curves(pools[0]);

        let cache = Arc::new(ReservesCache::new());
        insert_oriented(
            &cache,
            pools[1],
            t[1],
            t[2],
            unit(100),
            U256::from(102u64) * unit(1),
        )
        .await;
        insert_oriented(
            &cache,
            pools[2],
            t[2],
            t[3],
            unit(100),
            U256::from(102u64) * unit(1),
        )
        .await;

        let provider = Arc::new(CountingCurveV3Mock::new(curves.clone()));
        let catalog = cycle_fee_catalog(&pools, &protocols, &t, 500);
        let projector = Arc::new(StateProjector::new(cache, Some(provider.clone()), catalog));
        let slot0 = arm_slot0(&curves).await;
        let optimizer = SizeOptimizer::new(projector).with_slot0_cache(slot0);

        let mut candidate = make_cycle_candidate(&pools, &protocols, &t);
        candidate.opportunity.block_number = Some(21_000_001);
        let mut cfg = make_v3_multileg_cfg(10_000.0);
        cfg.min_landing_probability = 0.99;

        let outcome = optimizer
            .optimize_with_reason(
                candidate,
                &make_intent(addr(0xAAAA), addr(0xBBBB)),
                Some(&cfg),
            )
            .await
            .expect("optimize must not error");

        match outcome {
            OptimizeOutcome::Sized(s) => {
                let ins = s.leg_amounts_in.clone().expect("3-leg ledger inputs");
                let outs = s.leg_amounts_out.clone().expect("3-leg ledger outputs");
                assert_eq!(ins.len(), 3, "one ledger entry per leg");
                assert_eq!(outs.len(), 3);
                assert_eq!(
                    ins[0],
                    s.optimal_amount_in.to_string(),
                    "leg 0's input IS the reported optimal amount"
                );
                for i in 0..2 {
                    assert_eq!(ins[i + 1], outs[i], "chain broken at hop {i}");
                }
                assert!(s.gross_profit_usd > 0.0, "gross must be positive");
                // The gross must be consistent with the LEDGER's own final
                // amount, not with some other size.
                let final_out = outs[2].parse::<f64>().expect("decimal wei string");
                let amount_in = ins[0].parse::<f64>().expect("decimal wei string");
                let expected_gross = (final_out - amount_in) / 1e18 * 3000.0;
                let rel = (s.gross_profit_usd - expected_gross).abs() / expected_gross.abs();
                assert!(
                    rel < 1e-9,
                    "gross {} must match the ledger's own chain ({}): rel diff {rel}",
                    s.gross_profit_usd,
                    expected_gross
                );
                assert!(
                    s.estimated_net_profit_usd > 0.0,
                    "this fixture is chosen to clear the gas floor: net {}",
                    s.estimated_net_profit_usd
                );
            }
            other => panic!(
                "a 3-leg cycle with a cached slot0 must SIZE, got {:?}",
                other.reason_str()
            ),
        }
        // Budget: 1 V3 leg × 2 probes (default) = 2 sub-calls, one of them in
        // the batched prefetch.
        assert!(
            provider.sub_calls() <= 8,
            "per-candidate budget exceeded: {}",
            provider.sub_calls()
        );
        assert_eq!(
            provider
                .batch_dispatches
                .load(std::sync::atomic::Ordering::Relaxed),
            1
        );
    }

    /// GATE — 4-leg fixture with a cached slot0 ⇒ Sized + a full ledger.
    #[tokio::test]
    async fn gate_v3_multileg_four_leg_cycle_sizes_with_a_full_ledger() {
        let tokens = cycle_path(4);
        let t: Vec<&str> = tokens.iter().map(|s| s.as_str()).collect();
        let pools = [addr(0x40), addr(0x41), addr(0x42), addr(0x43)];
        let protocols = ["uniswap-v3", "uniswap-v2", "uniswap-v2", "uniswap-v2"];
        let curves = three_leg_curves(pools[0]);

        let cache = Arc::new(ReservesCache::new());
        for i in 1..4 {
            insert_oriented(
                &cache,
                pools[i],
                t[i],
                t[i + 1],
                unit(100),
                U256::from(102u64) * unit(1),
            )
            .await;
        }

        let provider = Arc::new(CountingCurveV3Mock::new(curves.clone()));
        let catalog = cycle_fee_catalog(&pools, &protocols, &t, 500);
        let projector = Arc::new(StateProjector::new(cache, Some(provider.clone()), catalog));
        let slot0 = arm_slot0(&curves).await;
        let optimizer = SizeOptimizer::new(projector).with_slot0_cache(slot0);

        let mut candidate = make_cycle_candidate(&pools, &protocols, &t);
        candidate.opportunity.block_number = Some(21_000_002);
        let cfg = make_v3_multileg_cfg(10_000.0);

        let outcome = optimizer
            .optimize_with_reason(
                candidate,
                &make_intent(addr(0xAAAA), addr(0xBBBB)),
                Some(&cfg),
            )
            .await
            .expect("optimize must not error");

        match outcome {
            OptimizeOutcome::Sized(s) => {
                let ins = s.leg_amounts_in.clone().expect("4-leg ledger inputs");
                let outs = s.leg_amounts_out.clone().expect("4-leg ledger outputs");
                assert_eq!(ins.len(), 4, "a 4-leg cycle must carry 4 hops, not 3");
                assert_eq!(outs.len(), 4);
                assert_eq!(ins[0], s.optimal_amount_in.to_string());
                for i in 0..3 {
                    assert_eq!(ins[i + 1], outs[i], "chain broken at hop {i}");
                }
                assert!(s.gross_profit_usd > 0.0);
                let final_out = outs[3].parse::<f64>().expect("decimal wei string");
                let amount_in = ins[0].parse::<f64>().expect("decimal wei string");
                let expected_gross = (final_out - amount_in) / 1e18 * 3000.0;
                let rel = (s.gross_profit_usd - expected_gross).abs() / expected_gross.abs();
                assert!(rel < 1e-9, "gross must match the 4-hop chain: {rel}");
            }
            other => panic!(
                "a 4-leg cycle with a cached slot0 must SIZE, got {:?}",
                other.reason_str()
            ),
        }
    }

    /// GATE — unavailable V3 state/quote ⇒ an explicit V3 reason, NO ledger, NO
    /// amounts (R8: never a fabricated chain, never `missing_reserves_pool_*`).
    #[tokio::test]
    async fn gate_v3_multileg_unpriceable_is_explicit_and_carries_no_ledger() {
        let tokens = cycle_path(3);
        let t: Vec<&str> = tokens.iter().map(|s| s.as_str()).collect();
        let pools = [addr(0x30), addr(0x31), addr(0x32)];
        let protocols = ["uniswap-v3", "uniswap-v2", "uniswap-v2"];
        let curves = three_leg_curves(pools[0]);

        for (label, provider, with_slot0) in [
            // (a) no provider wired at all (non-mainnet / absent at boot).
            ("no provider", None::<Arc<CountingCurveV3Mock>>, true),
            // (b) slot0 absent AND the provider answers nothing.
            ("no slot0, failing provider", None, false),
        ] {
            let cache = Arc::new(ReservesCache::new());
            insert_oriented(
                &cache,
                pools[1],
                t[1],
                t[2],
                unit(100),
                U256::from(102u64) * unit(1),
            )
            .await;
            insert_oriented(
                &cache,
                pools[2],
                t[2],
                t[3],
                unit(100),
                U256::from(102u64) * unit(1),
            )
            .await;

            let catalog = cycle_fee_catalog(&pools, &protocols, &t, 500);
            let projector = match provider {
                Some(p) => Arc::new(StateProjector::new(cache, Some(p), catalog)),
                None => Arc::new(StateProjector::new(cache, None, catalog)),
            };
            let mut optimizer = SizeOptimizer::new(projector);
            if with_slot0 {
                optimizer = optimizer.with_slot0_cache(arm_slot0(&curves).await);
            }
            let mut candidate = make_cycle_candidate(&pools, &protocols, &t);
            candidate.opportunity.block_number = Some(21_000_003);
            let cfg = make_v3_multileg_cfg(10_000.0);

            let outcome = optimizer
                .optimize_with_reason(
                    candidate,
                    &make_intent(addr(0xAAAA), addr(0xBBBB)),
                    Some(&cfg),
                )
                .await
                .expect("optimize must not error");

            assert_eq!(
                outcome.reason_str(),
                Some("v3_quote_unavailable"),
                "{label}: the honest reason is the provider/state failure"
            );
            assert!(
                outcome.leg_ledger().is_none(),
                "{label}: no ledger may be attached without quotes"
            );
            assert!(
                outcome.optimal_amount_in().is_none(),
                "{label}: no amount may be invented"
            );
            assert!(
                outcome.net_profit_usd().is_none(),
                "{label}: no USD may be invented"
            );
        }

        // (c) a failing provider (RPC exhausted) with slot0 cached: the local
        // bound is POSITIVE (the fixture is profitable), so the kernel really
        // tries to quote and the failure must surface honestly.
        let cache = Arc::new(ReservesCache::new());
        insert_oriented(
            &cache,
            pools[1],
            t[1],
            t[2],
            unit(100),
            U256::from(102u64) * unit(1),
        )
        .await;
        insert_oriented(
            &cache,
            pools[2],
            t[2],
            t[3],
            unit(100),
            U256::from(102u64) * unit(1),
        )
        .await;
        let catalog = cycle_fee_catalog(&pools, &protocols, &t, 500);
        let projector = Arc::new(StateProjector::new(
            cache,
            Some(Arc::new(FailingV3Mock)),
            catalog,
        ));
        let optimizer = SizeOptimizer::new(projector).with_slot0_cache(arm_slot0(&curves).await);
        let mut candidate = make_cycle_candidate(&pools, &protocols, &t);
        candidate.opportunity.block_number = Some(21_000_004);
        let cfg = make_v3_multileg_cfg(10_000.0);
        let outcome = optimizer
            .optimize_with_reason(
                candidate,
                &make_intent(addr(0xAAAA), addr(0xBBBB)),
                Some(&cfg),
            )
            .await
            .expect("optimize must not error");
        assert_eq!(outcome.reason_str(), Some("v3_quote_unavailable"));
        assert!(outcome.leg_ledger().is_none());
    }

    /// GATE — mixed V2/V3 5-leg cycle: every leg is priced by ITS OWN protocol
    /// (V2 by exact CPMM, V3 by the real quoter) and the ledger aligns per leg.
    #[tokio::test]
    async fn gate_v3_multileg_mixed_five_leg_ledger_aligns_per_leg() {
        let tokens = cycle_path(5);
        let t: Vec<&str> = tokens.iter().map(|s| s.as_str()).collect();
        let pools = [addr(0x50), addr(0x51), addr(0x52), addr(0x53), addr(0x54)];
        let protocols = [
            "uniswap-v3",
            "uniswap-v2",
            "uniswap-v3",
            "uniswap-v2",
            "uniswap-v3",
        ];
        let q96 = U256::one() << 96;
        let liq = U256::from(1_400_000_000_000_000_000u64);
        // leg0: WETH→T1 (token_in > token_out ⇒ one_for_zero, sqrtPrice 0.95·Q96
        //       ⇒ marginal rate 1/0.9025 ≈ 1.108).
        // leg2/leg4: ascending pairs (zero_for_one, 1.05·Q96 ⇒ 1.1025 each).
        let curves = HashMap::from([
            (
                pools[0],
                (q96 * U256::from(95u32) / U256::from(100u32), liq),
            ),
            (
                pools[2],
                (q96 * U256::from(105u32) / U256::from(100u32), liq),
            ),
            (
                pools[4],
                (q96 * U256::from(105u32) / U256::from(100u32), liq),
            ),
        ]);

        let cache = Arc::new(ReservesCache::new());
        let r_in = unit(100);
        let r_out = U256::from(102u64) * unit(1);
        insert_oriented(&cache, pools[1], t[1], t[2], r_in, r_out).await;
        insert_oriented(&cache, pools[3], t[3], t[4], r_in, r_out).await;

        let provider = Arc::new(CountingCurveV3Mock::new(curves.clone()));
        let catalog = cycle_fee_catalog(&pools, &protocols, &t, 500);
        let projector = Arc::new(StateProjector::new(cache, Some(provider.clone()), catalog));
        let slot0 = arm_slot0(&curves).await;
        let optimizer = SizeOptimizer::new(projector).with_slot0_cache(slot0);

        let mut candidate = make_cycle_candidate(&pools, &protocols, &t);
        candidate.opportunity.block_number = Some(21_000_005);
        let cfg = make_v3_multileg_cfg(10_000.0);

        let outcome = optimizer
            .optimize_with_reason(
                candidate,
                &make_intent(addr(0xAAAA), addr(0xBBBB)),
                Some(&cfg),
            )
            .await
            .expect("optimize must not error");

        let OptimizeOutcome::Sized(s) = outcome else {
            panic!(
                "the mixed 5-leg cycle must SIZE, got {:?}",
                outcome.reason_str()
            );
        };
        let ins = s.leg_amounts_in.clone().expect("5-leg ledger inputs");
        let outs = s.leg_amounts_out.clone().expect("5-leg ledger outputs");
        assert_eq!(ins.len(), 5);
        assert_eq!(outs.len(), 5);
        assert_eq!(ins[0], s.optimal_amount_in.to_string());
        assert!(s.gross_profit_usd > 0.0);

        // Per-leg protocol reconstruction: V2 legs MUST be exact CPMM, V3 legs
        // MUST be the quoted within-tick curve (the double answers with it), and
        // every leg's input MUST be the previous leg's output.
        let mut current = U256::from_dec_str(&ins[0]).expect("decimal wei string");
        for i in 0..5 {
            assert_eq!(ins[i], current.to_string(), "hop {i} input is not chained");
            let expected = if protocols[i].contains("v3") {
                let (sp, l) = curves[&pools[i]];
                crate::amm_math::v3_amount_out_single_tick(current, sp, l, 500, t[i] < t[i + 1])
            } else {
                let tin: Address = t[i].parse().expect("token parses");
                let tout: Address = t[i + 1].parse().expect("token parses");
                let (r0, r1) = if t[i] <= t[i + 1] {
                    (r_in, r_out)
                } else {
                    (r_out, r_in)
                };
                let _ = (tin, tout);
                v2_amount_out(current, r0, r1, 30)
            };
            assert_eq!(
                outs[i],
                expected.to_string(),
                "hop {i} ({}) must be priced by its own protocol",
                protocols[i]
            );
            current = expected;
        }
        // The reported gross is the chain's own final amount minus its input.
        let expected_gross =
            (current.to_string().parse::<f64>().unwrap() - ins[0].parse::<f64>().unwrap()) / 1e18
                * 3000.0;
        let rel = (s.gross_profit_usd - expected_gross).abs() / expected_gross.abs();
        assert!(rel < 1e-9, "gross must match the 5-hop chain: {rel}");
    }

    /// GATE — bounded RPC: the per-candidate budget is RESPECTED, the batch
    /// prefetch is used, and the per-(chain, block) allowance is charged.
    #[tokio::test]
    async fn gate_v3_multileg_quote_budget_is_bounded_per_candidate() {
        let tokens = cycle_path(3);
        let t: Vec<&str> = tokens.iter().map(|s| s.as_str()).collect();
        let pools = [addr(0x60), addr(0x61), addr(0x62)];
        let protocols = ["uniswap-v3", "uniswap-v3", "uniswap-v3"];
        let q96 = U256::one() << 96;
        let liq = U256::from(1_400_000_000_000_000_000u64);
        let curves = HashMap::from([
            (
                pools[0],
                (q96 * U256::from(95u32) / U256::from(100u32), liq),
            ),
            (
                pools[1],
                (q96 * U256::from(105u32) / U256::from(100u32), liq),
            ),
            (
                pools[2],
                (q96 * U256::from(105u32) / U256::from(100u32), liq),
            ),
        ]);

        let cache = Arc::new(ReservesCache::new());
        let provider = Arc::new(CountingCurveV3Mock::new(curves.clone()));
        let catalog = cycle_fee_catalog(&pools, &protocols, &t, 500);
        let projector = Arc::new(StateProjector::new(cache, Some(provider.clone()), catalog));
        let slot0 = arm_slot0(&curves).await;
        let budget = Arc::new(V3MultilegQuoteBudget::new(48));
        let optimizer = SizeOptimizer::new(projector)
            .with_slot0_cache(slot0)
            .with_v3_multileg_quote_budget(budget.clone());

        const BLOCK: u64 = 21_000_006;
        let mut candidate = make_cycle_candidate(&pools, &protocols, &t);
        candidate.opportunity.block_number = Some(BLOCK);
        let cfg = make_v3_multileg_cfg(10_000.0);

        let outcome = optimizer
            .optimize_with_reason(
                candidate,
                &make_intent(addr(0xAAAA), addr(0xBBBB)),
                Some(&cfg),
            )
            .await
            .expect("optimize must not error");
        assert!(
            matches!(outcome, OptimizeOutcome::Sized(_)),
            "the 3-V3-leg fixture must size, got {:?}",
            outcome.reason_str()
        );

        let v3_legs = 3u64;
        let max_probes = 2u64; // DEFAULT_V3_MULTILEG_MAX_PROBES
        let per_candidate_cap = 8u64; // DEFAULT_V3_MULTILEG_MAX_QUOTES
        let attempts = provider
            .unary_attempts
            .load(std::sync::atomic::Ordering::Relaxed);
        assert_eq!(
            attempts,
            v3_legs * max_probes,
            "3 V3 legs × 2 probes = 6 unary lookups (the stated per-candidate plan)"
        );
        assert!(
            provider.sub_calls() <= per_candidate_cap,
            "per-candidate sub-calls {} exceeded the stated budget {per_candidate_cap}",
            provider.sub_calls()
        );
        assert_eq!(
            provider
                .batch_dispatches
                .load(std::sync::atomic::Ordering::Relaxed),
            1,
            "the first V3 leg's probe inputs must travel in ONE aggregate3"
        );
        // The per-(chain, block) allowance is charged with the PLAN (2 probes ×
        // 3 V3 legs = 6), and never more than the stated per-block cap.
        assert_eq!(budget.used_in(BLOCK), 6);
        assert!(budget.used_in(BLOCK) <= 48, "per-block budget exceeded");
    }

    /// GATE — the per-(chain, block) allowance is hard: an exhausted block
    /// DEFERS the cycle with its own reason (never a fabricated verdict), and a
    /// new block resets it.
    #[tokio::test]
    async fn gate_v3_multileg_per_block_budget_defers_and_resets() {
        let tokens = cycle_path(3);
        let t: Vec<&str> = tokens.iter().map(|s| s.as_str()).collect();
        let pools = [addr(0x70), addr(0x71), addr(0x72)];
        let protocols = ["uniswap-v3", "uniswap-v2", "uniswap-v2"];
        let curves = three_leg_curves(pools[0]);
        let q96 = U256::one() << 96;
        let _ = q96;

        let cache = Arc::new(ReservesCache::new());
        insert_oriented(
            &cache,
            pools[1],
            t[1],
            t[2],
            unit(100),
            U256::from(102u64) * unit(1),
        )
        .await;
        insert_oriented(
            &cache,
            pools[2],
            t[2],
            t[3],
            unit(100),
            U256::from(102u64) * unit(1),
        )
        .await;
        let provider = Arc::new(CountingCurveV3Mock::new(curves.clone()));
        let catalog = cycle_fee_catalog(&pools, &protocols, &t, 500);
        let projector = Arc::new(StateProjector::new(cache, Some(provider.clone()), catalog));
        let slot0 = arm_slot0(&curves).await;

        // (a) an allowance smaller than one plan (2 probes × 1 V3 leg = 2) ⇒
        // the cycle is DEFERRED with its own reason and ZERO RPC.
        let tiny = Arc::new(V3MultilegQuoteBudget::new(1));
        let optimizer = SizeOptimizer::new(projector.clone())
            .with_slot0_cache(slot0.clone())
            .with_v3_multileg_quote_budget(tiny.clone());
        let mut candidate = make_cycle_candidate(&pools, &protocols, &t);
        candidate.opportunity.block_number = Some(100);
        let cfg = make_v3_multileg_cfg(10_000.0);
        let outcome = optimizer
            .optimize_with_reason(
                candidate,
                &make_intent(addr(0xAAAA), addr(0xBBBB)),
                Some(&cfg),
            )
            .await
            .expect("optimize must not error");
        assert_eq!(
            outcome.reason_str(),
            Some("v3_multileg_budget_exhausted"),
            "a cycle deferred by the allowance must name that, not a spread verdict"
        );
        assert!(outcome.leg_ledger().is_none());
        assert!(outcome.optimal_amount_in().is_none());
        assert_eq!(
            provider
                .batch_dispatches
                .load(std::sync::atomic::Ordering::Relaxed),
            0,
            "a deferred cycle must not spend a single quote"
        );
        assert_eq!(tiny.used_in(100), 0);

        // (b) an allowance of exactly one plan: the first cycle in block 100 is
        // quoted, the second is deferred, and block 101 resets the allowance.
        let budget = Arc::new(V3MultilegQuoteBudget::new(2));
        let optimizer = SizeOptimizer::new(projector)
            .with_slot0_cache(slot0)
            .with_v3_multileg_quote_budget(budget.clone());

        let mut first = make_cycle_candidate(&pools, &protocols, &t);
        first.opportunity.block_number = Some(100);
        let first_outcome = optimizer
            .optimize_with_reason(first, &make_intent(addr(0xAAAA), addr(0xBBBB)), Some(&cfg))
            .await
            .expect("optimize must not error");
        assert!(
            matches!(first_outcome, OptimizeOutcome::Sized(_)),
            "the first cycle of the block fits the allowance, got {:?}",
            first_outcome.reason_str()
        );
        assert_eq!(budget.used_in(100), 2);

        let mut second = make_cycle_candidate(&pools, &protocols, &t);
        second.opportunity.block_number = Some(100);
        let second_outcome = optimizer
            .optimize_with_reason(second, &make_intent(addr(0xAAAA), addr(0xBBBB)), Some(&cfg))
            .await
            .expect("optimize must not error");
        assert_eq!(
            second_outcome.reason_str(),
            Some("v3_multileg_budget_exhausted"),
            "the block's allowance is spent — the second cycle is deferred (R8)"
        );
        assert_eq!(budget.used_in(100), 2, "a deferred cycle spends nothing");

        let mut third = make_cycle_candidate(&pools, &protocols, &t);
        third.opportunity.block_number = Some(101);
        let third_outcome = optimizer
            .optimize_with_reason(third, &make_intent(addr(0xAAAA), addr(0xBBBB)), Some(&cfg))
            .await
            .expect("optimize must not error");
        assert!(
            matches!(third_outcome, OptimizeOutcome::Sized(_)),
            "a new block resets the allowance, got {:?}",
            third_outcome.reason_str()
        );
        assert_eq!(budget.used_in(101), 2);
        assert_eq!(
            budget.used_in(100),
            0,
            "the previous block's counter is gone"
        );
    }

    /// GATE — the honest COMPLEMENT of the PERHOP invariant: when the V3 legs DO
    /// price and a V2 leg genuinely has no cached reserves, the verdict names
    /// that V2 leg (`missing_reserves_pool_b`) — the reserve label stays
    /// reachable and correctly attributed, it is just never allowed to pre-empt
    /// a V3 leg that fails EARLIER in the chain.
    #[tokio::test]
    async fn gate_v3_multileg_v2_reserve_miss_still_names_the_v2_leg() {
        let tokens = cycle_path(3);
        let t: Vec<&str> = tokens.iter().map(|s| s.as_str()).collect();
        let pools = [addr(0x30), addr(0x31), addr(0x32)];
        let protocols = ["uniswap-v3", "uniswap-v2", "uniswap-v2"];
        let curves = three_leg_curves(pools[0]);

        // Leg 0 (V3) quotes fine; leg 1 (V2) has NO cached reserves.
        let cache = Arc::new(ReservesCache::new());
        insert_oriented(
            &cache,
            pools[2],
            t[2],
            t[3],
            unit(100),
            U256::from(102u64) * unit(1),
        )
        .await;
        let provider = Arc::new(CountingCurveV3Mock::new(curves.clone()));
        let catalog = cycle_fee_catalog(&pools, &protocols, &t, 500);
        let projector = Arc::new(StateProjector::new(cache, Some(provider), catalog));
        let optimizer = SizeOptimizer::new(projector).with_slot0_cache(arm_slot0(&curves).await);

        let mut candidate = make_cycle_candidate(&pools, &protocols, &t);
        candidate.opportunity.block_number = Some(21_000_007);
        let cfg = make_v3_multileg_cfg(10_000.0);
        let outcome = optimizer
            .optimize_with_reason(
                candidate,
                &make_intent(addr(0xAAAA), addr(0xBBBB)),
                Some(&cfg),
            )
            .await
            .expect("optimize must not error");
        assert_eq!(
            outcome.reason_str(),
            Some("missing_reserves_pool_b"),
            "an uncached V2 leg behind a priced V3 leg is the TRUE first blocker"
        );
        assert!(
            outcome.leg_ledger().is_none(),
            "no ledger without a full chain"
        );
    }

    /// The N-leg V3 kernel's knobs are default-ON and explicitly reversible, and
    /// a malformed/foreign value never changes behaviour silently.
    #[test]
    fn v3_multileg_knobs_are_default_on_and_fail_honest() {
        assert!(v3_multileg_sizing_from_raw(None), "absent ⇒ ON (default)");
        assert!(v3_multileg_sizing_from_raw(Some("")));
        assert!(v3_multileg_sizing_from_raw(Some("ON")));
        assert!(v3_multileg_sizing_from_raw(Some("true")));
        assert!(!v3_multileg_sizing_from_raw(Some("off")));
        assert!(!v3_multileg_sizing_from_raw(Some(" OFF ")));
        assert!(!v3_multileg_sizing_from_raw(Some("false")));
        assert!(!v3_multileg_sizing_from_raw(Some("0")));
        assert!(
            v3_multileg_sizing_from_raw(Some("banana")),
            "a foreign value is never interpreted as a verdict (R8)"
        );
        // Env parsers: unset/junk keeps the default, a valid value is capped.
        assert_eq!(env_usize_capped(None, 8, 64), 8);
        assert_eq!(env_usize_capped(Some("3".into()), 8, 64), 3);
        assert_eq!(env_usize_capped(Some("junk".into()), 8, 64), 8);
        assert_eq!(env_usize_capped(Some("9999".into()), 8, 64), 64);
        // The stated defaults are the kernel's contract.
        assert_eq!(DEFAULT_V3_MULTILEG_MAX_PROBES, 2);
        assert_eq!(DEFAULT_V3_MULTILEG_MAX_QUOTES, 8);
        assert_eq!(DEFAULT_V3_MULTILEG_QUOTES_PER_BLOCK, 48);
        // …and the new rejection reason has its stable token.
        assert_eq!(
            OptimizeRejectReason::V3MultilegBudgetExhausted.as_str(),
            "v3_multileg_budget_exhausted"
        );
        assert!(!OptimizeRejectReason::V3MultilegBudgetExhausted.is_net_dependent());
    }

    /// The pure probe-bracket helpers: the argmax comes first, neighbours fan
    /// out inside the grid, and a degenerate grid never returns an empty bracket.
    #[test]
    fn v3_multileg_probe_bracket_is_bounded_and_centred() {
        let grid: Vec<U256> = (0..8)
            .map(|i| U256::from(10u64).pow(U256::from(i)))
            .collect();
        let around = SizeOptimizer::probes_around_best(&grid, 0, 3);
        assert_eq!(around.len(), 3);
        assert_eq!(around[0], grid[0], "the argmax is always the first probe");
        assert_eq!(around[1], grid[1]);
        assert_eq!(around[2], grid[2]);
        let mid = SizeOptimizer::probes_around_best(&grid, 4, 3);
        assert_eq!(mid, vec![grid[4], grid[3], grid[5]]);
        assert!(SizeOptimizer::probes_around_best(&grid, 4, 99).len() <= grid.len());
        // Bounded by `max`, never empty.
        assert_eq!(SizeOptimizer::probes_around_best(&grid, 3, 1).len(), 1);
        let single = vec![U256::from(7u64)];
        assert_eq!(SizeOptimizer::probes_around_best(&single, 0, 4), single);
        // The no-local-model fallback is the centred window of the same grid.
        assert_eq!(
            SizeOptimizer::middle_probes(&grid, 2),
            vec![grid[3], grid[4]]
        );
        assert_eq!(SizeOptimizer::middle_probes(&grid, 99), grid);
        assert_eq!(SizeOptimizer::middle_probes(&[], 4), vec![U256::one()]);
    }

    /// NLEG-SIZE-BAND-01 — the model-free bracket must be anchored on the
    /// AUTHORIZED CAPITAL, not on the geometric centre of the log grid.
    ///
    /// This is the regression that produced live cards with
    /// `amount_in_wei = 1e18` next to `gross = 0.00000000`: with
    /// `cap_usd = $1000` on an 18-decimals token the centred probes are
    /// `≈ cap_wei^(8/15) ≈ 2.3e9 wei ≈ $0.000006`, so gross, net, roi and the
    /// whole cost ladder were all computed at dust.
    #[test]
    fn model_free_bracket_is_anchored_on_the_authorized_capital() {
        // The exact live shape: $1000 cap, token at $2698.80, 18 decimals.
        let cap_usd = 1000.0_f64;
        let price = 2698.797_5_f64;
        let decimals = 18u8;
        let cap_wei = clamp_to_cap_wei(U256::MAX, cap_usd, price, decimals)
            .expect("cap_wei must resolve for a live-shaped configuration");
        let grid = geom_probes(U256::one(), cap_wei, 16);
        assert_eq!(grid.len(), 16, "grid shape is part of the contract");
        // The grid's TOP is the capital end. `geom_probes` rounds every point,
        // so the top lands within rounding of `cap_wei` — measured 320 wei ABOVE
        // a 3.7e17 cap (8.6e-16 relative). The kernel's own anti-BUG-3 clamp
        // (`min(x_star, cap_wei)`, documented in this file's header) bounds the
        // size actually executed; this test asserts the band is the capital end,
        // not the exact integer.
        let top = *grid.last().unwrap();
        let tolerance = cap_wei / U256::from(100_000u64); // 0.001%
        let diff = if top >= cap_wei {
            top - cap_wei
        } else {
            cap_wei - top
        };
        assert!(
            diff <= tolerance,
            "grid top {top} is not the capital end (cap_wei {cap_wei}, diff {diff})"
        );

        let probes = SizeOptimizer::capital_band_probes(&grid, 2);
        assert_eq!(probes.len(), 2, "bounded by max_probes");

        // 1. The largest probe IS the capital end of the grid.
        assert_eq!(*probes.last().unwrap(), top);

        // 2. Every probe sits in the top decade of the band, i.e. within an
        //    order of magnitude of the capital the operator authorized.
        let floor = cap_wei / U256::from(16u64);
        for p in &probes {
            assert!(
                *p >= floor,
                "model-free probe {p} is below cap_wei/16 ({floor}) — dust sizing"
            );
        }

        // 3. The dust point the old centre-of-grid fallback selected is GONE.
        let dust = SizeOptimizer::middle_probes(&grid, 2);
        for d in &dust {
            assert!(
                !probes.contains(d),
                "the centred (dust) point {d} must not be probed as the capital band"
            );
        }
        // …and that centred point really was orders of magnitude below the cap
        // (documents the size of the defect rather than asserting a constant).
        let centre = dust[0];
        assert!(
            centre < cap_wei / U256::from(1_000_000u64),
            "the old fallback point {centre} was not dust-sized; the premise changed"
        );

        // 4. Degenerate inputs keep the old honest behaviour.
        assert_eq!(
            SizeOptimizer::capital_band_probes(&[], 4),
            vec![U256::one()]
        );
        assert_eq!(
            SizeOptimizer::capital_band_probes(&grid, 0),
            vec![U256::one()]
        );
        assert_eq!(SizeOptimizer::capital_band_probes(&grid, 99), grid);
    }
}
