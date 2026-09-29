//! ALWAYS-COMPUTE economics contract (operator mandate 2026-09-27).
//!
//! > "Si da FAIL, tiene que mostrarme qué hizo y cuánto da, no solamente FAIL.
//! > La aplicación debe devolver TODOS los cálculos así no satisfagan lo
//! > esperado. COMPUTED ≠ PROFITABLE. FAIL debe significar 'se hizo el cálculo
//! > y no cumple el criterio', no 'no tengo números'. Si el resultado da -$50,
//! > la card debe decir -$50, no NO COMPUTADO."
//!
//! One complete [`EconomicsComputation`] object is built BEFORE the pass/fail
//! decision (the sizing kernel's figures exist at that point — `SizedCandidate`
//! + sheet-07 `RouteNetEconomics`) and persisted on BOTH branches:
//!
//!   * kernel sized ⇒ [`economics_from_sized`] — status `"computed"`;
//!   * kernel rejected WITH the figures it computed ⇒ the kernel returns
//!     `OptimizeOutcome::RejectedComputed` carrying the same `SizedCandidate`
//!     shape ⇒ ALSO [`economics_from_sized`] on the rejected row (this is the
//!     "$-50 card" the operator asked for);
//!   * kernel rejected with NO quote (missing reserves, no config, RPC down)
//!     ⇒ [`economics_error`] — status `"error"`, reason verbatim, NO numbers
//!     (RULE 00 / R8: never fabricate);
//!   * a producer that never reached sizing but carries real probe figures
//!     (engine gross) ⇒ [`economics_partial`] stamped at the emit boundary —
//!     status `"partial"`, every absent field null WITH its reason.
//!
//! Reversibility: the whole contract is gated by `ARBX_ALWAYS_COMPUTE_ECONOMICS`
//! (default ON). With the knob OFF the producers attach nothing and the emit
//! boundary stamps nothing — the pre-mandate wire (plus `git apply -R`).
//!
//! Mode-invariant (§34.1): the builders consume kernel figures only — identical
//! arithmetic in PAPER_SHADOW / TESTNET / LIVE_MAINNET.
//!
//! ## missing_economics counters (deliverable #3)
//!
//! Per-field atomic counters observed at the emit boundary (both branches),
//! flushed as ONE summary line per emission window (R9: no per-item info logs)
//! and mirrored to Redis `arbx:diagnostics:missing_economics` (TTL 10 min) so
//! an admin can `GET` it without log access. The api-server additionally
//! computes the authoritative per-request census over the rows it serves
//! (`diagnostics.missing_economics` on the live feed envelope) — the two
//! views bracket the producer→consumer pipe and show exactly WHERE data dies.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;

use ethers::types::U256;
use shared_rs::contracts::{ComputedLeg, EconomicsComputation};
use shared_rs::trading_config::TradingConfigState;

use crate::engines::StrategyCandidate;
use crate::size_optimizer::SizedCandidate;

/// Redis key holding the last flushed window snapshot (JSON, TTL 10 min).
pub const MISSING_ECONOMICS_REDIS_KEY: &str = "arbx:diagnostics:missing_economics";

/// One emission window = 60 s (R9: one summary line per window, never per item).
const WINDOW_MS: u64 = 60_000;

/// TTL of the Redis mirror — longer than two windows so a single failed flush
/// never leaves the key absent, short enough that a dead searcher expires it.
const MISSING_ECONOMICS_TTL_SECS: u64 = 600;

// ---------------------------------------------------------------------------
// Knob
// ---------------------------------------------------------------------------

static ALWAYS_COMPUTE: OnceLock<bool> = OnceLock::new();

use std::sync::OnceLock;

/// `ARBX_ALWAYS_COMPUTE_ECONOMICS` — default ON. Disabled by `0`, `false` or
/// `off` (case-insensitive). Parsed once; the knob is read on every emit so a
/// restart is the revert path (documented in the patch report).
pub fn always_compute_enabled() -> bool {
    *ALWAYS_COMPUTE.get_or_init(|| {
        std::env::var("ARBX_ALWAYS_COMPUTE_ECONOMICS")
            .map(|v| {
                !matches!(
                    v.trim().to_ascii_lowercase().as_str(),
                    "0" | "false" | "off"
                )
            })
            .unwrap_or(true)
    })
}

// ---------------------------------------------------------------------------
// Builders (pure — unit-testable without I/O)
// ---------------------------------------------------------------------------

/// Cost-component reasons reused by every builder so the wire vocabulary is
/// stable for consumers (documented, never ad-hoc strings per site).
pub mod reasons {
    pub const DEX_FEES_INCLUDED_IN_OUT: &str = "included_in_amount_out_post_fee";
    pub const SLIPPAGE_PRICED_BY_CURVE: &str = "priced_by_amm_curve";
    pub const SIM_BLOCK_IS_SIM_CTL: &str = "revm_simulation_is_sim_ctl_scope";
    pub const NO_TARGET_CONFIGURED: &str = "no_simulation_target_configured";
    pub const NO_TRADING_CONFIG: &str = "no_trading_config";
    pub const NOT_PRICED_AT_EMIT: &str = "amount_in_usd_not_priced_at_emit_boundary";
    pub const COSTS_NOT_COMPUTED_PRE_SIZING: &str = "cost_components_not_computed_pre_sizing";
    pub const AMOUNT_OUT_NOT_EXPOSED: &str = "cycle_output_not_exposed_by_kernel";
}

/// Build the COMPLETE object from the kernel's sized figures (used on BOTH the
/// accepted row and the `RejectedComputed` row — same figures, same closure).
///
/// Closure: `total_cost_usd` sums exactly the four present components and
/// `net_profit_usd` is the kernel's own net; the two agree to float
/// associativity (≤ ~1e-12 relative, the ARBX-0007 ulp lesson) — far inside
/// the display tolerance the frontend ledger applies ($0.005).
pub fn economics_from_sized(
    sized: &SizedCandidate,
    cfg: Option<&TradingConfigState>,
) -> EconomicsComputation {
    let mut not_computed = std::collections::BTreeMap::new();
    let econ = sized.net_economics;

    // ── Amounts ────────────────────────────────────────────────────────────
    // RouteNetEconomics documents a non-positive start amount as "metric NOT
    // computable" (its own net_bps contract) — mirror that here: the sized
    // amount was not exposed by the rejecting evaluation, so amount_in stays
    // absent WITH its reason instead of a fabricated 0 (R8).
    let amount_known = econ.map(|e| e.start_amount_usd > 0.0).unwrap_or(false);
    let amount_in_wei = if amount_known {
        Some(sized.optimal_amount_in.to_string())
    } else {
        not_computed.insert(
            "amount_in_wei".to_owned(),
            "sized_amount_not_exposed_by_rejecting_path".to_owned(),
        );
        None
    };
    let amount_out_wei = sized
        .leg_amounts_out
        .as_ref()
        .and_then(|outs| outs.last().cloned());
    if amount_out_wei.is_none() {
        not_computed.insert(
            "amount_out_wei".to_owned(),
            reasons::AMOUNT_OUT_NOT_EXPOSED.to_owned(),
        );
    }

    let gross = sized.gross_profit_usd;
    let net = Some(sized.estimated_net_profit_usd);

    // ── Cost components (sheet-07 columns; see RouteNetEconomics docs) ─────
    let (amount_in_usd, gas_usd, flash_fee_usd, bribe_usd, other_costs_usd) = match econ {
        Some(e) if amount_known => (
            Some(e.start_amount_usd),
            Some(e.gas_usd),
            Some(e.flash_fee_usd),
            Some(e.builder_tip_usd), // 0.0 is its TRUE computed value today
            Some(e.other_cost_usd),
        ),
        // start ≤ 0: the cost components exist but the PRINCIPAL they were
        // priced against was never exposed — components stay absent (R8).
        Some(e) => {
            let r = "principal_not_exposed_by_rejecting_path";
            for f in [
                "amount_in_usd",
                "gas_usd",
                "flash_fee_usd",
                "bribe_usd",
                "other_costs_usd",
            ] {
                not_computed.insert(f.to_owned(), r.to_owned());
            }
            (None, None, None, None, None)
        }
        None => {
            for f in [
                "amount_in_usd",
                "gas_usd",
                "flash_fee_usd",
                "bribe_usd",
                "other_costs_usd",
            ] {
                not_computed.insert(f.to_owned(), "net_economics_absent".to_owned());
            }
            (None, None, None, None, None)
        }
    };
    // dex_fees / slippage are never separate lines on the kernel path — the
    // quote already walks the AMM curve and returns post-fee amounts.
    not_computed.insert(
        "dex_fees_usd".to_owned(),
        reasons::DEX_FEES_INCLUDED_IN_OUT.to_owned(),
    );
    not_computed.insert(
        "slippage_usd".to_owned(),
        reasons::SLIPPAGE_PRICED_BY_CURVE.to_owned(),
    );

    let total_cost_usd = [gas_usd, flash_fee_usd, bribe_usd, other_costs_usd]
        .into_iter()
        .flatten()
        .reduce(|a, b| a + b);

    // out = in + gross is an identity of the kernel's arithmetic (out − in ==
    // gross over input); absent only when the principal was never exposed.
    let amount_out_usd = amount_in_usd.map(|a| a + gross);
    if amount_out_usd.is_none() {
        not_computed.insert(
            "amount_out_usd".to_owned(),
            "requires_amount_in_usd".to_owned(),
        );
    }

    // ── ROI — null ONLY when the division is impossible (R8) ───────────────
    let roi_pct = match amount_in_usd {
        Some(a) if a > 0.0 => net.map(|n| n / a * 100.0),
        _ => {
            not_computed.insert(
                "roi_pct".to_owned(),
                "amount_in_usd_not_positive".to_owned(),
            );
            None
        }
    };

    // ── Target (operator floor; same predicate the card renders) ───────────
    let (target_net_usd, target_delta_usd, meets_target) =
        match cfg.and_then(|c| c.simulation_target_profit_usd) {
            Some(t) if t > 0.0 => (Some(t), net.map(|n| n - t), net.map(|n| n >= t)),
            _ => {
                let r = match cfg {
                    None => reasons::NO_TRADING_CONFIG,
                    Some(_) => reasons::NO_TARGET_CONFIGURED,
                };
                not_computed.insert("target_net_usd".to_owned(), r.to_owned());
                (None, None, None)
            }
        };

    // ── Blocks ─────────────────────────────────────────────────────────────
    let quote_block = sized.candidate.opportunity.block_number;
    not_computed.insert(
        "simulation_block".to_owned(),
        reasons::SIM_BLOCK_IS_SIM_CTL.to_owned(),
    );

    // ── Per-leg ledger (exact wei, aligned with the plan) ──────────────────
    let legs = computed_legs(sized);

    EconomicsComputation {
        computation_status: "computed".to_owned(),
        error_reason: None,
        amount_in_wei,
        amount_out_wei,
        amount_in_usd,
        amount_out_usd,
        gross_profit_usd: Some(gross),
        gas_usd,
        dex_fees_usd: None,
        flash_fee_usd,
        bribe_usd,
        slippage_usd: None,
        other_costs_usd,
        total_cost_usd,
        net_profit_usd: net,
        roi_pct,
        target_net_usd,
        target_delta_usd,
        meets_target,
        quote_block,
        simulation_block: None,
        legs,
        not_computed_reasons: not_computed,
    }
}

/// Zip the kernel's exact per-leg wei with the plan's token path. Empty when
/// the kernel had no per-leg math (triangular final-amount-only, Kelly
/// re-bound without re-quote) — all-or-nothing, never a partial ledger (R8).
fn computed_legs(sized: &SizedCandidate) -> Vec<ComputedLeg> {
    let (Some(ins), Some(outs)) = (
        sized.leg_amounts_in.as_ref(),
        sized.leg_amounts_out.as_ref(),
    ) else {
        return Vec::new();
    };
    if ins.len() != outs.len() || ins.is_empty() {
        return Vec::new();
    }
    let plan_legs = &sized.candidate.route_plan.legs;
    if plan_legs.len() != ins.len() {
        return Vec::new();
    }
    plan_legs
        .iter()
        .zip(ins.iter().zip(outs.iter()))
        .map(|(leg, (i, o))| ComputedLeg {
            token_in: leg.token_in.clone(),
            token_out: leg.token_out.clone(),
            amount_in_wei: i.clone(),
            amount_out_wei: o.clone(),
        })
        .collect()
}

/// ALWAYS-COMPUTE-03 (2026-09-27): the CLOSED `computed` object for a REJECTED
/// row whose gross was already MEASURED by its producer (`Some(gross)`, even a
/// measured `0.0`) but which never reached the sizing kernel.
///
/// MEASURED DEFECT this closes (production PG, 30 min): the engine's
/// `spread_zero_equilibrium` early-exit publishes a measured gross
/// (`Some(0.0)` — both legs quoted the same amount, so the cycle gross is
/// exactly zero), yet its economics object was still produced by the
/// emit boundary's nothing-computed arm (`economics_error` ⇒
/// `computation_status: "error"`, every figure null). The card therefore read
/// "not computed" on a row whose arithmetic WAS computed — the inversion the
/// operator's mandate removes (`Some(0.0)` = computed and exactly zero;
/// `None` = not computed).
///
/// Reuse, not re-derivation — three existing sources, one closure:
///   * cost components: the SAME config oracle the sizing kernel reads at its
///     own reject arms (`TradingConfigState::gas_cost_usd()` /
///     `ops_overhead_usd_per_attempt` — `size_optimizer`'s call sites), never a
///     literal gas price or gas-unit table;
///   * sheet-07 arithmetic: [`crate::net_bps_ranking::RouteNetEconomics::from_kernel`],
///     the kernel's own constructor (flash fee via `financing::selected_mode`);
///   * wire object: [`economics_from_sized`], so `total_cost_usd` (Σ present
///     components), `net_profit_usd` (gross − costs) and `roi_pct`
///     (`net / amount_in × 100`) are the accepted row's exact closure.
///
/// R8: the caller passes only figures it MEASURED (`gross_profit_usd`,
/// `amount_in_wei`) plus the principal it PRICED from the live source
/// (`amount_in_usd`). Nothing is invented, and a component this path cannot
/// know (per-leg wei — the reject came from the probe, not the leg-by-leg
/// kernel) stays absent WITH its reason. `None` when the principal is not a
/// usable positive finite figure: without the capital there is no closed
/// arithmetic to claim, so the caller keeps the row's honest diagnostic.
pub fn economics_from_measured_gross(
    candidate: &StrategyCandidate,
    amount_in_wei: U256,
    amount_in_usd: f64,
    gross_profit_usd: f64,
    cfg: &TradingConfigState,
) -> Option<EconomicsComputation> {
    if !amount_in_usd.is_finite() || amount_in_usd <= 0.0 || !gross_profit_usd.is_finite() {
        return None;
    }
    let gas_usd = cfg.gas_cost_usd();
    let other_cost_usd = cfg.ops_overhead_usd_per_attempt;
    // Financing: an engine-level candidate carries no flash wrapper yet
    // (`StrategyCandidate::base_strategy` is `None` on every engine rejection),
    // so the borrow is zero and the fee is zero — exactly the policy the kernel
    // prices for a non-flash candidate (`financing::selected_mode(0.0)` ⇒
    // `OwnCapital`, fee 0 bps).
    let borrow_usd = if candidate.base_strategy.is_some() {
        amount_in_usd
    } else {
        0.0
    };
    let net_economics = crate::net_bps_ranking::RouteNetEconomics::from_kernel(
        amount_in_usd,
        gross_profit_usd,
        gas_usd,
        other_cost_usd,
        borrow_usd,
    );
    let net_profit_usd = net_economics.net_profit_usd();
    let sized = SizedCandidate {
        candidate: candidate.clone(),
        optimal_amount_in: amount_in_wei,
        gross_profit_usd,
        estimated_net_profit_usd: net_profit_usd,
        net_negative: net_profit_usd <= 0.0,
        net_economics: Some(net_economics),
        // No per-leg chain on this path (R8 — never a repeated probe amount
        // dressed as a ledger).
        leg_amounts_in: None,
        leg_amounts_out: None,
    };
    Some(economics_from_sized(&sized, Some(cfg)))
}

/// The emit-boundary fallback for rows whose producer never attached an
/// object but which DO carry real figures (engine probe gross, spine net).
/// Status `"partial"`: what exists is surfaced; everything else is null WITH
/// its reason. Never called when the knob is OFF.
pub fn economics_partial(
    gross: Option<f64>,
    net: Option<f64>,
    roi_pct: Option<f64>,
    rejection_reason: Option<&str>,
) -> EconomicsComputation {
    let mut not_computed = std::collections::BTreeMap::new();
    if gross.is_none() {
        not_computed.insert(
            "gross_profit_usd".to_owned(),
            "not_computed_by_producer".to_owned(),
        );
    }
    if net.is_none() {
        not_computed.insert(
            "net_profit_usd".to_owned(),
            "not_computed_by_producer".to_owned(),
        );
    }
    if roi_pct.is_none() {
        not_computed.insert("roi_pct".to_owned(), "not_computed_by_producer".to_owned());
    }
    for f in [
        "amount_in_usd",
        "amount_out_usd",
        "gas_usd",
        "flash_fee_usd",
        "other_costs_usd",
        "total_cost_usd",
        "target_net_usd",
        "quote_block",
    ] {
        not_computed.insert(
            f.to_owned(),
            reasons::COSTS_NOT_COMPUTED_PRE_SIZING.to_owned(),
        );
    }
    not_computed.insert(
        "amount_in_usd".to_owned(),
        reasons::NOT_PRICED_AT_EMIT.to_owned(),
    );
    not_computed.insert("legs".to_owned(), "no_kernel_ledger".to_owned());
    EconomicsComputation {
        computation_status: "partial".to_owned(),
        error_reason: rejection_reason.map(str::to_owned),
        amount_in_wei: None,
        amount_out_wei: None,
        amount_in_usd: None,
        amount_out_usd: None,
        gross_profit_usd: gross,
        gas_usd: None,
        dex_fees_usd: None,
        flash_fee_usd: None,
        bribe_usd: None,
        slippage_usd: None,
        other_costs_usd: None,
        total_cost_usd: None,
        net_profit_usd: net,
        roi_pct,
        target_net_usd: None,
        target_delta_usd: None,
        meets_target: None,
        quote_block: None,
        simulation_block: None,
        legs: Vec::new(),
        not_computed_reasons: not_computed,
    }
}

/// The technical-failure object: NO numbers, only the reason (RULE 00 — a
/// missing quote is the one honest absence; gate 2 of the mandate).
pub fn economics_error(reason: &str) -> EconomicsComputation {
    EconomicsComputation {
        computation_status: "error".to_owned(),
        error_reason: Some(reason.to_owned()),
        amount_in_wei: None,
        amount_out_wei: None,
        amount_in_usd: None,
        amount_out_usd: None,
        gross_profit_usd: None,
        gas_usd: None,
        dex_fees_usd: None,
        flash_fee_usd: None,
        bribe_usd: None,
        slippage_usd: None,
        other_costs_usd: None,
        total_cost_usd: None,
        net_profit_usd: None,
        roi_pct: None,
        target_net_usd: None,
        target_delta_usd: None,
        meets_target: None,
        quote_block: None,
        simulation_block: None,
        legs: Vec::new(),
        not_computed_reasons: std::collections::BTreeMap::new(),
    }
}

/// Stamp the computation object at the emit boundary (single publish gate,
/// both branches). Idempotent for producers that already attached one — only
/// `quote_block` is backfilled from the row's own block evidence.
pub fn stamp_on_emit(opp: &mut shared_rs::contracts::Opportunity) {
    if !always_compute_enabled() {
        return;
    }
    match opp.economics.as_mut() {
        Some(e) => {
            if e.quote_block.is_none() {
                e.quote_block = opp.block_number;
            }
        }
        None => {
            let has_figures =
                opp.expected_profit_usd.is_some() || opp.net_expected_profit_usd.is_some();
            let mut obj = if has_figures {
                economics_partial(
                    opp.expected_profit_usd,
                    opp.net_expected_profit_usd,
                    opp.roi_pct,
                    opp.rejection_reason.as_deref(),
                )
            } else {
                economics_error(
                    opp.rejection_reason
                        .as_deref()
                        .unwrap_or("no_economics_computed"),
                )
            };
            // The partial object inherits the row's own block evidence (the
            // figures were computed against it); the error object stays
            // block-less — no quote existed (R8).
            if has_figures && obj.quote_block.is_none() {
                obj.quote_block = opp.block_number;
            }
            opp.economics = Some(obj);
        }
    }
}

// ---------------------------------------------------------------------------
// missing_economics counters
// ---------------------------------------------------------------------------

/// The operator's exact field census (deliverable #3). Counted at the emit
/// boundary. `required_amount_usd` is owned by the api-server's inverse
/// sizing — the searcher counts it only when NO economics object exists at
/// all (nothing anywhere), and the api-server census is the authoritative
/// per-field view for it.
#[derive(Default)]
pub struct MissingEconomicsCounters {
    pub amount_in_usd: AtomicU64,
    pub gross_usd: AtomicU64,
    pub costs_usd: AtomicU64,
    pub net_usd: AtomicU64,
    pub roi_pct: AtomicU64,
    pub target_usd: AtomicU64,
    pub achieved_usd: AtomicU64,
    pub required_amount_usd: AtomicU64,
    pub ledger: AtomicU64,
    pub emitted_total: AtomicU64,
}

/// Global registry — one per process, drained per window.
pub static MISSING: MissingEconomicsCounters = MissingEconomicsCounters {
    amount_in_usd: AtomicU64::new(0),
    gross_usd: AtomicU64::new(0),
    costs_usd: AtomicU64::new(0),
    net_usd: AtomicU64::new(0),
    roi_pct: AtomicU64::new(0),
    target_usd: AtomicU64::new(0),
    achieved_usd: AtomicU64::new(0),
    required_amount_usd: AtomicU64::new(0),
    ledger: AtomicU64::new(0),
    emitted_total: AtomicU64::new(0),
};

static LAST_FLUSH_UNIX_MS: AtomicU64 = AtomicU64::new(0);
static FLUSH_IN_FLIGHT: AtomicBool = AtomicBool::new(false);

fn unix_ms_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Observe one emitted row (both branches). Cheap: a handful of relaxed
/// atomic increments on the emit path, zero allocation.
pub fn observe_missing(opp: &shared_rs::contracts::Opportunity) {
    let e = opp.economics.as_ref();
    MISSING.emitted_total.fetch_add(1, Ordering::Relaxed);
    let missing = |b: bool| if b { 1 } else { 0 };

    MISSING.amount_in_usd.fetch_add(
        missing(e.and_then(|x| x.amount_in_usd).is_none()),
        Ordering::Relaxed,
    );
    MISSING.gross_usd.fetch_add(
        missing(opp.expected_profit_usd.is_none() && e.and_then(|x| x.gross_profit_usd).is_none()),
        Ordering::Relaxed,
    );
    MISSING.costs_usd.fetch_add(
        missing(e.and_then(|x| x.total_cost_usd).is_none()),
        Ordering::Relaxed,
    );
    MISSING.net_usd.fetch_add(
        missing(
            opp.net_expected_profit_usd.is_none() && e.and_then(|x| x.net_profit_usd).is_none(),
        ),
        Ordering::Relaxed,
    );
    MISSING.roi_pct.fetch_add(
        missing(opp.roi_pct.is_none() && e.and_then(|x| x.roi_pct).is_none()),
        Ordering::Relaxed,
    );
    MISSING.target_usd.fetch_add(
        missing(e.and_then(|x| x.target_net_usd).is_none()),
        Ordering::Relaxed,
    );
    MISSING.achieved_usd.fetch_add(
        missing(e.and_then(|x| x.net_profit_usd).is_none()),
        Ordering::Relaxed,
    );
    // required_amount_in_usd: inverse sizing is api-server scope; count the
    // total absence only (documented above).
    MISSING
        .required_amount_usd
        .fetch_add(missing(e.is_none()), Ordering::Relaxed);
    MISSING.ledger.fetch_add(
        missing(e.map(|x| x.legs.is_empty()).unwrap_or(true)),
        Ordering::Relaxed,
    );
}

/// Flush ONE summary line per emission window (R9) + mirror to Redis.
/// Called opportunistically from the emit path: whichever emission crosses
/// the 60 s boundary performs the flush — no background task lifecycle, and
/// a silent pipeline (zero emissions) correctly produces zero windows.
pub async fn maybe_flush_window(redis: &mut redis::aio::ConnectionManager) {
    let now = unix_ms_now();
    let last = LAST_FLUSH_UNIX_MS.load(Ordering::Relaxed);
    if now.saturating_sub(last) < WINDOW_MS {
        return;
    }
    // First emission of the window: arm the next window, then drain.
    if LAST_FLUSH_UNIX_MS
        .compare_exchange(last, now, Ordering::Relaxed, Ordering::Relaxed)
        .is_err()
    {
        return; // another task is flushing this window
    }
    if FLUSH_IN_FLIGHT.swap(true, Ordering::Relaxed) {
        return; // previous flush still writing (Redis slow) — skip, not pile up
    }
    let snap = snapshot_and_reset();
    // ONE info line per window (R9): the whole histogram in structured fields.
    tracing::info!(
        event = "economics.missing_economics_window",
        emitted = snap.emitted_total,
        amount_in_usd = snap.amount_in_usd,
        gross_usd = snap.gross_usd,
        costs_usd = snap.costs_usd,
        net_usd = snap.net_usd,
        roi_pct = snap.roi_pct,
        target_usd = snap.target_usd,
        achieved_usd = snap.achieved_usd,
        required_amount_usd = snap.required_amount_usd,
        ledger = snap.ledger,
        window_ms = WINDOW_MS,
        "ALWAYS-COMPUTE census: fields absent at the emit boundary this window"
    );
    // Redis mirror (best-effort, never fatal): admin-readable without logs.
    let payload = serde_json::json!({
        "flushed_at_unix_ms": now,
        "window_ms": WINDOW_MS,
        "emitted_total": snap.emitted_total,
        "fields": {
            "amount_in_usd": snap.amount_in_usd,
            "gross_usd": snap.gross_usd,
            "costs_usd": snap.costs_usd,
            "net_usd": snap.net_usd,
            "roi_pct": snap.roi_pct,
            "target_usd": snap.target_usd,
            "achieved_usd": snap.achieved_usd,
            "required_amount_usd": snap.required_amount_usd,
            "ledger": snap.ledger,
        },
    });
    if let Ok(json) = serde_json::to_string(&payload) {
        let res: redis::RedisResult<()> = redis::cmd("SET")
            .arg(MISSING_ECONOMICS_REDIS_KEY)
            .arg(&json)
            .arg("EX")
            .arg(MISSING_ECONOMICS_TTL_SECS)
            .query_async(redis)
            .await;
        if let Err(e) = res {
            tracing::debug!(
                event = "economics.missing_economics_mirror_failed",
                error = %e,
                "Redis mirror of the missing_economics window failed (non-fatal)"
            );
        }
    }
    FLUSH_IN_FLIGHT.store(false, Ordering::Relaxed);
}

#[derive(Debug, Default, Clone, Copy)]
struct WindowSnapshot {
    amount_in_usd: u64,
    gross_usd: u64,
    costs_usd: u64,
    net_usd: u64,
    roi_pct: u64,
    target_usd: u64,
    achieved_usd: u64,
    required_amount_usd: u64,
    ledger: u64,
    emitted_total: u64,
}

fn snapshot_and_reset() -> WindowSnapshot {
    let take = |c: &AtomicU64| c.swap(0, Ordering::Relaxed);
    WindowSnapshot {
        amount_in_usd: take(&MISSING.amount_in_usd),
        gross_usd: take(&MISSING.gross_usd),
        costs_usd: take(&MISSING.costs_usd),
        net_usd: take(&MISSING.net_usd),
        roi_pct: take(&MISSING.roi_pct),
        target_usd: take(&MISSING.target_usd),
        achieved_usd: take(&MISSING.achieved_usd),
        required_amount_usd: take(&MISSING.required_amount_usd),
        ledger: take(&MISSING.ledger),
        emitted_total: take(&MISSING.emitted_total),
    }
}

/// Test hook: force the next `maybe_flush_window` call to consider the window
/// elapsed (the real clock gate makes the flush untestable otherwise).
pub fn force_window_elapsed_for_tests() {
    LAST_FLUSH_UNIX_MS.store(0, Ordering::Relaxed);
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    // ── Knob ───────────────────────────────────────────────────────────────

    #[test]
    fn knob_defaults_on_and_parses_disablers() {
        // The OnceCell caches the FIRST read in-process; assert the parse
        // table directly instead of mutating process env (parallel tests).
        let disable = |v: &str| {
            matches!(
                v.trim().to_ascii_lowercase().as_str(),
                "0" | "false" | "off"
            )
        };
        assert!(disable("0") && disable("false") && disable(" off "));
        assert!(!disable("1") && !disable("true") && !disable("on") && !disable(""));
        // Default posture (env unset) is ON — the mandate's contract.
        assert!(std::env::var("ARBX_ALWAYS_COMPUTE_ECONOMICS")
            .map(|v| !disable(&v))
            .unwrap_or(true));
    }

    // ── error object: gate 2 — no fabricated numbers ───────────────────────

    #[test]
    fn error_object_carries_reason_and_no_numbers() {
        let e = economics_error("missing_reserves_pool_b");
        assert_eq!(e.computation_status, "error");
        assert_eq!(e.error_reason.as_deref(), Some("missing_reserves_pool_b"));
        assert!(e.gross_profit_usd.is_none());
        assert!(e.net_profit_usd.is_none());
        assert!(e.total_cost_usd.is_none());
        assert!(e.roi_pct.is_none());
        assert!(e.legs.is_empty());
    }

    // ── partial object: real figures + reasons for the rest ────────────────

    #[test]
    fn partial_object_surfaces_figures_with_reasons() {
        let e = economics_partial(Some(1.5), None, None, Some("token_not_allowed:PEPE"));
        assert_eq!(e.computation_status, "partial");
        assert_eq!(e.gross_profit_usd, Some(1.5));
        assert!(e.net_profit_usd.is_none());
        assert!(e.not_computed_reasons.contains_key("net_profit_usd"));
        assert!(e.not_computed_reasons.contains_key("total_cost_usd"));
        assert_eq!(
            e.error_reason.as_deref(),
            Some("token_not_allowed:PEPE"),
            "the reject reason rides the partial object for context"
        );
    }

    // ── stamp_on_emit: the three honest states at the publish gate ─────────

    fn row_opp(
        gross: Option<f64>,
        net: Option<f64>,
        rejection: Option<&str>,
    ) -> shared_rs::contracts::Opportunity {
        shared_rs::contracts::Opportunity {
            id: uuid::Uuid::new_v4(),
            chain_id: 1,
            strategy_kind: shared_rs::contracts::StrategyKind::dex_arb(),
            dex_a: "uniswap_v2".to_owned(),
            dex_b: None,
            pair_symbol: "WETH/USDC".to_owned(),
            token_in: "0xweth".to_owned(),
            token_out: "0xusdc".to_owned(),
            amount_in_wei: "1000000000000000000".to_owned(),
            expected_profit_usd: gross,
            net_expected_profit_usd: net,
            roi_pct: None,
            risk_score: None,
            block_number: Some(12_345_678),
            rejection_reason: rejection.map(str::to_owned),
            cartridge_id: None,
            detector_id: Some("dex_engine".to_owned()),
            pipeline_latency_ms: None,
            economics: None,
            detected_at: chrono::Utc::now(),
            trace_id: uuid::Uuid::new_v4(),
        }
    }

    #[test]
    fn stamp_partial_when_figures_exist_but_no_object() {
        let mut opp = row_opp(Some(1.5), None, Some("non_positive_profit"));
        stamp_on_emit(&mut opp);
        let e = opp.economics.as_ref().expect("object must be stamped");
        assert_eq!(e.computation_status, "partial");
        assert_eq!(e.gross_profit_usd, Some(1.5));
        // quote_block backfilled from the row's own block evidence.
        assert_eq!(e.quote_block, Some(12_345_678));
    }

    #[test]
    fn stamp_error_when_nothing_computed() {
        let mut opp = row_opp(None, None, Some("missing_reserves_pool_a"));
        stamp_on_emit(&mut opp);
        let e = opp.economics.as_ref().expect("object must be stamped");
        assert_eq!(e.computation_status, "error");
        assert_eq!(e.error_reason.as_deref(), Some("missing_reserves_pool_a"));
        assert!(e.gross_profit_usd.is_none());
    }

    #[test]
    fn stamp_never_overwrites_a_producer_object() {
        let mut opp = row_opp(Some(1.5), Some(-0.25), None);
        opp.economics = Some(economics_error("producer_attached"));
        stamp_on_emit(&mut opp);
        // The producer's object wins verbatim — only quote_block backfills.
        assert_eq!(opp.economics.as_ref().unwrap().computation_status, "error");
        assert_eq!(
            opp.economics.as_ref().unwrap().quote_block,
            Some(12_345_678)
        );
    }

    // ── observe_missing: the census counts today's production gap ──────────

    #[test]
    fn census_counts_absent_fields_on_legacy_shaped_row() {
        // Production shape TODAY (the operator's audit): economics absent,
        // gross/net/roi all null on a rejected row.
        let before = MISSING.gross_usd.load(Ordering::Relaxed);
        let opp = row_opp(None, None, Some("unknown_token_price"));
        observe_missing(&opp);
        assert_eq!(MISSING.gross_usd.load(Ordering::Relaxed), before + 1);
        assert!(MISSING.emitted_total.load(Ordering::Relaxed) >= 1);
    }

    #[test]
    fn census_does_not_count_present_fields() {
        let before = MISSING.gross_usd.load(Ordering::Relaxed);
        let mut opp = row_opp(Some(2.0), Some(-0.5), Some("gas_floor_breach"));
        let mut e = economics_partial(Some(2.0), Some(-0.5), None, None);
        e.gross_profit_usd = Some(2.0);
        e.net_profit_usd = Some(-0.5);
        e.total_cost_usd = Some(2.5);
        e.amount_in_usd = Some(1.0);
        e.roi_pct = Some(-50.0);
        e.target_net_usd = Some(5.0);
        e.legs.push(shared_rs::contracts::ComputedLeg {
            token_in: "0xa".to_owned(),
            token_out: "0xb".to_owned(),
            amount_in_wei: "1".to_owned(),
            amount_out_wei: "1".to_owned(),
        });
        opp.economics = Some(e);
        observe_missing(&opp);
        assert_eq!(MISSING.gross_usd.load(Ordering::Relaxed), before);
    }
}
