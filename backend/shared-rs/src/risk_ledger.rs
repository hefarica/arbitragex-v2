//! Rolling-window risk ledger — pure circuit-breaker math (shadow / capital = 0).
//!
//! ## Why this exists
//!
//! The drawdown, revert-rate and gas-burn circuit breakers in the control-plane
//! are reported `NOT_AVAILABLE` because nothing aggregates the shadow
//! `paper_trade_runs` / `executions` history into a rolling-window verdict. This
//! module is that aggregator's **pure core**: given a slice of trade outcomes
//! and operator-supplied thresholds, it returns a [`BreakerSnapshot`]. It does
//! **no I/O**, holds no wallet, commits no capital — it only turns already-
//! recorded shadow data into the evidence the pre-execute checklist (item 6 —
//! `arbx-risk-limits-enforcement`) needs.
//!
//! The I/O shell (read `paper_trade_runs`/`executions` → publish the snapshot to
//! Redis for `routes/risk-circuit-breakers.ts`) lives outside this module so the
//! breaker math stays deterministic and unit-testable with no services.
//!
//! ## Doctrine
//!
//! - Every threshold is operator-supplied via [`BreakerThresholds`] — no
//!   hardcoded productive value (`arbx-no-hardcode-doctrine`).
//! - Below `min_samples` the verdict is `sufficient = false` and the level is
//!   `Ok` (insufficient evidence NEVER trips a breaker — it stays honest, not
//!   permissive: callers must treat "insufficient" as "do not promote to live").
//! - Drawdown is measured against an operator-supplied `nav_baseline_usd`
//!   (Quarter-Kelly NAV, CLAUDE.md §24), never a magic constant.

use serde::{Deserialize, Serialize};

/// One trade outcome in the rolling window. For shadow/paper rows
/// `realized_pnl_usd` is the simulated expected profit (proxy) until the
/// drift-tracker backfills realized P&L; `gas_cost_usd` is the simulated gas.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TradeOutcome {
    /// Unix seconds the outcome was recorded.
    pub ts_unix: i64,
    /// Profit-and-loss in USD (negative for a loss).
    pub realized_pnl_usd: f64,
    /// Whether the trade reverted on-chain (or its simulation reverted).
    pub reverted: bool,
    /// Gas spent in USD (counts toward the burn cap even when the trade reverts).
    pub gas_cost_usd: f64,
}

/// Escalating breaker action, ordered so `max` yields the most severe.
/// Mirrors the control-plane tiers (warn → pause → hard-pause → kill).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BreakerLevel {
    Ok,
    Warn,
    Pause,
    HardPause,
    Kill,
}

/// Operator-supplied drawdown escalation tiers, in percent of NAV.
/// Must be non-decreasing; `tier()` picks the highest crossed threshold.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct DrawdownTiers {
    pub warn_pct: f64,
    pub pause_pct: f64,
    pub hard_pause_pct: f64,
    pub kill_pct: f64,
}

impl DrawdownTiers {
    /// Maps a drawdown percentage to the highest tier it has crossed.
    fn level(&self, drawdown_pct: f64) -> BreakerLevel {
        if drawdown_pct >= self.kill_pct {
            BreakerLevel::Kill
        } else if drawdown_pct >= self.hard_pause_pct {
            BreakerLevel::HardPause
        } else if drawdown_pct >= self.pause_pct {
            BreakerLevel::Pause
        } else if drawdown_pct >= self.warn_pct {
            BreakerLevel::Warn
        } else {
            BreakerLevel::Ok
        }
    }
}

/// All thresholds for one breaker evaluation. Operator-supplied — nothing here
/// is defaulted to a productive value in code.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BreakerThresholds {
    /// Rolling window width in seconds (e.g. 86_400 for a daily cap).
    pub window_secs: i64,
    /// Minimum outcomes in-window before any breaker is allowed to trip.
    pub min_samples: usize,
    /// Capital base the drawdown percentage is measured against (USD). Must be > 0.
    pub nav_baseline_usd: f64,
    /// Drawdown escalation tiers (percent of NAV).
    pub drawdown: DrawdownTiers,
    /// Trip the revert-rate breaker at/above this percentage.
    pub max_revert_rate_pct: f64,
    /// Trip the gas-burn breaker when in-window gas spend reaches this USD cap.
    pub max_gas_burn_usd: f64,
}

/// A single breaker's evaluated state.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BreakerMetric {
    /// The measured value (percent for drawdown/revert-rate, USD for gas-burn).
    pub value: f64,
    /// Escalation level the value maps to (always `Ok` when `!sufficient`).
    pub level: BreakerLevel,
    /// Number of in-window samples backing the value.
    pub samples: usize,
    /// `false` when `samples < min_samples` (NOT_AVAILABLE → insufficient).
    pub sufficient: bool,
}

/// The aggregated verdict for one `(chain, window)` evaluation.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BreakerSnapshot {
    pub computed_at_unix: i64,
    pub window_samples: usize,
    /// Current equity = `nav_baseline_usd + Σ realized_pnl_usd` over the window.
    pub equity_usd: f64,
    pub drawdown_pct: BreakerMetric,
    pub revert_rate_pct: BreakerMetric,
    pub gas_burn_usd: BreakerMetric,
    /// Most severe level across all breakers (the overall action to take).
    pub overall: BreakerLevel,
    /// `true` while any breaker lacks `min_samples` — callers must NOT promote
    /// a strategy to live execution on insufficient evidence.
    pub insufficient_evidence: bool,
}

/// Computes the rolling-window breaker snapshot.
///
/// Pure and deterministic: filters `outcomes` to the window
/// `[now_unix - window_secs, now_unix]`, then evaluates each breaker. Below
/// `min_samples` every breaker is `sufficient = false`, `level = Ok`, and
/// `insufficient_evidence = true`.
pub fn compute_breakers(
    now_unix: i64,
    outcomes: &[TradeOutcome],
    th: &BreakerThresholds,
) -> BreakerSnapshot {
    // 1. Window filter, chronologically ordered for the equity walk.
    let window_start = now_unix.saturating_sub(th.window_secs.max(0));
    let mut win: Vec<&TradeOutcome> = outcomes
        .iter()
        .filter(|o| o.ts_unix >= window_start && o.ts_unix <= now_unix)
        .collect();
    win.sort_by_key(|o| o.ts_unix);

    let n = win.len();
    let sufficient = n >= th.min_samples;

    // 2. Equity walk → max peak-to-trough drawdown (percent of peak equity).
    //    equity starts at the NAV baseline; peak tracks the running maximum.
    let nav = if th.nav_baseline_usd > 0.0 {
        th.nav_baseline_usd
    } else {
        // Guard: a non-positive NAV would make drawdown% undefined. Fail honest
        // by reporting 0% drawdown and leaving the gate to min_samples/overall.
        f64::NAN
    };
    let mut equity = th.nav_baseline_usd;
    let mut peak = th.nav_baseline_usd;
    let mut max_drawdown_pct = 0.0_f64;
    let mut gas_sum = 0.0_f64;
    let mut revert_count = 0usize;

    for o in &win {
        equity += o.realized_pnl_usd;
        gas_sum += o.gas_cost_usd.max(0.0);
        if o.reverted {
            revert_count += 1;
        }
        if equity > peak {
            peak = equity;
        }
        if nav.is_finite() && peak > 0.0 {
            let dd = (peak - equity) / peak * 100.0;
            if dd > max_drawdown_pct {
                max_drawdown_pct = dd;
            }
        }
    }

    let revert_rate_pct = if n > 0 {
        (revert_count as f64) / (n as f64) * 100.0
    } else {
        0.0
    };

    // 3. Per-breaker levels (only when there is enough evidence to act).
    let drawdown_level = if sufficient {
        th.drawdown.level(max_drawdown_pct)
    } else {
        BreakerLevel::Ok
    };
    let revert_level = if sufficient && revert_rate_pct >= th.max_revert_rate_pct {
        BreakerLevel::Pause
    } else {
        BreakerLevel::Ok
    };
    // Gas burn is a hard cumulative cap; it can trip even without min_samples
    // ONLY if explicitly sufficient — keep symmetric with the other breakers so
    // a single fat-finger sim row never hard-pauses on cold start.
    let gas_level = if sufficient && gas_sum >= th.max_gas_burn_usd {
        BreakerLevel::Pause
    } else {
        BreakerLevel::Ok
    };

    let overall = drawdown_level.max(revert_level).max(gas_level);

    BreakerSnapshot {
        computed_at_unix: now_unix,
        window_samples: n,
        equity_usd: equity,
        drawdown_pct: BreakerMetric {
            value: max_drawdown_pct,
            level: drawdown_level,
            samples: n,
            sufficient,
        },
        revert_rate_pct: BreakerMetric {
            value: revert_rate_pct,
            level: revert_level,
            samples: n,
            sufficient,
        },
        gas_burn_usd: BreakerMetric {
            value: gas_sum,
            level: gas_level,
            samples: n,
            sufficient,
        },
        overall,
        insufficient_evidence: !sufficient,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn tiers() -> DrawdownTiers {
        DrawdownTiers {
            warn_pct: 10.0,
            pause_pct: 20.0,
            hard_pause_pct: 30.0,
            kill_pct: 40.0,
        }
    }

    fn th(min_samples: usize) -> BreakerThresholds {
        BreakerThresholds {
            window_secs: 86_400,
            min_samples,
            nav_baseline_usd: 1_000.0,
            drawdown: tiers(),
            max_revert_rate_pct: 30.0,
            max_gas_burn_usd: 100.0,
        }
    }

    fn out(ts: i64, pnl: f64, reverted: bool, gas: f64) -> TradeOutcome {
        TradeOutcome {
            ts_unix: ts,
            realized_pnl_usd: pnl,
            reverted,
            gas_cost_usd: gas,
        }
    }

    #[test]
    fn empty_input_is_insufficient_and_ok() {
        let s = compute_breakers(1_000, &[], &th(5));
        assert_eq!(s.window_samples, 0);
        assert!(s.insufficient_evidence);
        assert!(!s.drawdown_pct.sufficient);
        assert_eq!(s.overall, BreakerLevel::Ok);
        // equity equals the untouched NAV baseline.
        assert_eq!(s.equity_usd, 1_000.0);
    }

    #[test]
    fn below_min_samples_never_trips() {
        // A catastrophic-looking single row must NOT trip on cold start.
        let outcomes = [out(900, -900.0, true, 500.0)];
        let s = compute_breakers(1_000, &outcomes, &th(5));
        assert!(s.insufficient_evidence);
        assert_eq!(s.overall, BreakerLevel::Ok);
        assert!(!s.gas_burn_usd.sufficient);
    }

    #[test]
    fn monotonic_up_equity_has_zero_drawdown() {
        let outcomes = [
            out(100, 10.0, false, 1.0),
            out(200, 20.0, false, 1.0),
            out(300, 30.0, false, 1.0),
        ];
        let s = compute_breakers(1_000, &outcomes, &th(3));
        assert!(s.drawdown_pct.sufficient);
        assert!((s.drawdown_pct.value - 0.0).abs() < 1e-9);
        assert_eq!(s.drawdown_pct.level, BreakerLevel::Ok);
        assert_eq!(s.equity_usd, 1_060.0);
    }

    #[test]
    fn peak_then_drop_computes_drawdown_tier() {
        // NAV 1000. Equity walk: 1000 → +200=1200 (peak) → -300=900.
        // Drawdown from peak 1200 to 900 = 300/1200 = 25% → Pause tier.
        let outcomes = [
            out(100, 200.0, false, 1.0),
            out(200, -300.0, false, 1.0),
        ];
        let s = compute_breakers(1_000, &outcomes, &th(2));
        assert!((s.drawdown_pct.value - 25.0).abs() < 1e-9);
        assert_eq!(s.drawdown_pct.level, BreakerLevel::Pause);
        assert_eq!(s.overall, BreakerLevel::Pause);
    }

    #[test]
    fn drawdown_kill_tier_at_40pct() {
        // 1000 → +0 (peak 1000) → -400 = 600. dd = 400/1000 = 40% → Kill.
        let outcomes = [out(100, 0.0, false, 0.0), out(200, -400.0, false, 0.0)];
        let s = compute_breakers(1_000, &outcomes, &th(2));
        assert!((s.drawdown_pct.value - 40.0).abs() < 1e-9);
        assert_eq!(s.drawdown_pct.level, BreakerLevel::Kill);
        assert_eq!(s.overall, BreakerLevel::Kill);
    }

    #[test]
    fn revert_rate_trips_pause_at_threshold() {
        // 2 of 5 reverted = 40% >= 30% → Pause.
        let outcomes = [
            out(100, 1.0, true, 1.0),
            out(150, 1.0, true, 1.0),
            out(200, 1.0, false, 1.0),
            out(250, 1.0, false, 1.0),
            out(300, 1.0, false, 1.0),
        ];
        let s = compute_breakers(1_000, &outcomes, &th(5));
        assert!((s.revert_rate_pct.value - 40.0).abs() < 1e-9);
        assert_eq!(s.revert_rate_pct.level, BreakerLevel::Pause);
    }

    #[test]
    fn revert_rate_below_threshold_is_ok() {
        // 1 of 5 = 20% < 30%.
        let outcomes = [
            out(100, 1.0, true, 1.0),
            out(150, 1.0, false, 1.0),
            out(200, 1.0, false, 1.0),
            out(250, 1.0, false, 1.0),
            out(300, 1.0, false, 1.0),
        ];
        let s = compute_breakers(1_000, &outcomes, &th(5));
        assert!((s.revert_rate_pct.value - 20.0).abs() < 1e-9);
        assert_eq!(s.revert_rate_pct.level, BreakerLevel::Ok);
    }

    #[test]
    fn gas_burn_trips_at_cap() {
        // Σ gas = 60 + 50 = 110 >= 100 → Pause.
        let outcomes = [
            out(100, 1.0, false, 60.0),
            out(200, 1.0, false, 50.0),
            out(300, 1.0, false, 0.0),
        ];
        let s = compute_breakers(1_000, &outcomes, &th(3));
        assert!((s.gas_burn_usd.value - 110.0).abs() < 1e-9);
        assert_eq!(s.gas_burn_usd.level, BreakerLevel::Pause);
    }

    #[test]
    fn window_filter_excludes_old_and_future_samples() {
        // window_secs 86_400; now 1_000_000. In-window only ts >= 913_600.
        let outcomes = [
            out(100, -500.0, true, 999.0),       // far past — excluded
            out(913_600, -100.0, false, 10.0),   // exactly at window start — included
            out(1_000_000, -50.0, false, 5.0),   // now — included
            out(2_000_000, -999.0, true, 999.0), // future — excluded
        ];
        let s = compute_breakers(1_000_000, &outcomes, &th(2));
        assert_eq!(s.window_samples, 2);
        assert!((s.gas_burn_usd.value - 15.0).abs() < 1e-9);
        assert_eq!(s.revert_rate_pct.value, 0.0);
    }

    #[test]
    fn overall_is_max_escalation_across_breakers() {
        // Drawdown Warn (15%), revert Pause (50%), gas Ok → overall Pause.
        // 1000 → +0 (peak 1000) → -150 = 850. dd = 150/1000 = 15% → Warn.
        let outcomes = [
            out(100, 0.0, true, 1.0),
            out(200, -150.0, true, 1.0),
            out(300, 0.0, false, 1.0),
            out(400, 0.0, false, 1.0),
        ];
        let s = compute_breakers(1_000, &outcomes, &th(4));
        assert_eq!(s.drawdown_pct.level, BreakerLevel::Warn);
        assert_eq!(s.revert_rate_pct.level, BreakerLevel::Pause); // 2/4 = 50%
        assert_eq!(s.overall, BreakerLevel::Pause);
    }

    #[test]
    fn non_positive_nav_does_not_panic() {
        let mut t = th(1);
        t.nav_baseline_usd = 0.0;
        let outcomes = [out(100, -10.0, false, 1.0)];
        let s = compute_breakers(1_000, &outcomes, &t);
        // Drawdown undefined w/o NAV → reported 0, no panic, gate falls to overall.
        assert_eq!(s.drawdown_pct.value, 0.0);
        assert_eq!(s.overall, BreakerLevel::Ok);
    }

    #[test]
    fn level_ordering_is_monotonic() {
        assert!(BreakerLevel::Ok < BreakerLevel::Warn);
        assert!(BreakerLevel::Warn < BreakerLevel::Pause);
        assert!(BreakerLevel::Pause < BreakerLevel::HardPause);
        assert!(BreakerLevel::HardPause < BreakerLevel::Kill);
    }
}
