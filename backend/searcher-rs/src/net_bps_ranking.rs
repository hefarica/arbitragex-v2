//! Sheet `07_INEFFICIENCY` Net_bps contract + deterministic ranking (ARBX-0009).
//!
//! Source of truth: QuoteBase workbook #5 (`07_INEFFICIENCY`). The Candidate
//! table (rows 3-66) is an **empty template** — the deliverable is the formula
//! contract, extracted verbatim from the data row (r4) via openpyxl:
//!
//! | Col | Workbook name | Formula (r4) | Here |
//! |-----|---------------|--------------|------|
//! | C   | StartAmount   | input        | `start_amount_usd` |
//! | L/N | GrossFactor/GrossFinal | `L=D*E*…(legs)`, `N=C*L` | `gross_over_input_usd` = N−C (the sizing kernel's `gross_usd`) |
//! | O   | Gas           | input        | `gas_usd` |
//! | P   | FlashFee      | input        | `flash_fee_usd` (from `financing::ModeEval`, ARBX-0007) |
//! | Q   | BuilderTip    | input        | `builder_tip_usd` |
//! | R   | OtherCost     | input        | `other_cost_usd` (= `ops_overhead_usd_per_attempt`) |
//! | S   | NetFinal      | `N−O−P−Q−R`  | `net_final_usd()` |
//! | T   | NetProfit     | `S−C`        | `net_profit_usd()` |
//! | M   | Gross_bps     | `10000*(L−1)`| `gross_bps()` |
//! | U   | Net_bps       | `10000*T/C`  | `net_bps()` — the ranking key |
//! | V   | PASS          | `AND(T>0, U>=01_CONFIG!$B$13)` | `passes(min_net_bps)` |
//!
//! Doctrine rows 67-73 (two layers): the log prefilter (`W=Σ[−ln(r_e)]`) is a
//! cheap seed filter at DISCOVERY; this module is the EVALUATION-side truth —
//! net economics over real cost components, applied only to sized finalists.
//!
//! Fail-honest (R8): a non-positive start amount or a non-finite result means
//! the metric is NOT computable — `net_bps()`/`gross_bps()`/`passes()` return
//! `None` (never a fabricated `0.0`), and ranking places those entries LAST,
//! deterministically. No entry is ever dropped: ranking orders the whole batch
//! (PASS and FAIL rows alike — the workbook table lists both).
//!
//! Builder tip: `0.0` today — no bid path exists in the shadow/paper termini,
//! and workbook arithmetic treats an empty cost column as 0. Wire a real
//! on-chain tip when MEV-boost bidding lands (fees-on-chain doctrine).

use std::cmp::Ordering;

/// `01_CONFIG!B13` (Min_Net_bps) — cross-pinned to `CanonicalKnobs::min_net_bps`.
pub const DEFAULT_MIN_NET_BPS: f64 = 5.0;

/// The sheet-07 cost/economics columns for ONE evaluated route under ONE
/// financing mode. All USD. Built by the sizing kernel, which has every
/// component in scope at its sized sites.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RouteNetEconomics {
    /// C — capital deployed into the cycle (priced `amount_in`), flash or own.
    pub start_amount_usd: f64,
    /// N−C — gross over input: final output value minus start amount. This is
    /// the sizing engine's `gross_usd` (`profit_token_units × price`).
    pub gross_over_input_usd: f64,
    /// O — Gas (USD).
    pub gas_usd: f64,
    /// P — FlashFee (USD), from `financing` (ARBX-0007 mode pricing).
    pub flash_fee_usd: f64,
    /// Q — BuilderTip (USD). 0.0 until a bid path exists (see module docs).
    pub builder_tip_usd: f64,
    /// R — OtherCost (USD) = `ops_overhead_usd_per_attempt`.
    pub other_cost_usd: f64,
}

impl RouteNetEconomics {
    /// Explicit R8 marker: no sizing components available (engine-rejected or
    /// hand-built). Every derived bps metric on it is `None` — an entry built
    /// from this ranks LAST, never mid-table on fabricated zeros.
    pub fn not_computable() -> Self {
        Self {
            start_amount_usd: 0.0,
            gross_over_input_usd: 0.0,
            gas_usd: 0.0,
            flash_fee_usd: 0.0,
            builder_tip_usd: 0.0,
            other_cost_usd: 0.0,
        }
    }

    /// Build from the sizing kernel's components, pricing the flash fee with
    /// the SAME policy the kernel priced (ARBX-0007): a flash-backed borrow
    /// pays the selected mode's fee; own capital pays none. The fee expression
    /// is bit-identical to the kernel's `borrow * fee_bps() / 10_000.0`, and
    /// `net_profit_usd()` uses the same components — agreement with the
    /// kernel's `net_usd` holds to float associativity (≤ ~1e-12 relative).
    pub fn from_kernel(
        start_amount_usd: f64,
        gross_over_input_usd: f64,
        gas_usd: f64,
        other_cost_usd: f64,
        borrow_usd: f64,
    ) -> Self {
        let mode = crate::financing::selected_mode(borrow_usd);
        let flash_fee_usd = borrow_usd * mode.fee_bps() / 10_000.0;
        Self {
            start_amount_usd,
            gross_over_input_usd,
            gas_usd,
            flash_fee_usd,
            builder_tip_usd: 0.0,
            other_cost_usd,
        }
    }

    /// S — NetFinal = GrossFinal − Gas − FlashFee − BuilderTip − OtherCost.
    pub fn net_final_usd(&self) -> f64 {
        let gross_final = self.start_amount_usd + self.gross_over_input_usd;
        gross_final - self.gas_usd - self.flash_fee_usd - self.builder_tip_usd - self.other_cost_usd
    }

    /// T — NetProfit = NetFinal − StartAmount. Computed in the flat form
    /// (`gross − costs`, algebraically identical to `S − C`) so it stays at
    /// ulp-level agreement with the kernel's `net_usd` — the column-wise
    /// round-trip `(start+gross) − costs − start` loses low bits to
    /// associativity (the ARBX-0007 ulp lesson).
    pub fn net_profit_usd(&self) -> f64 {
        self.gross_over_input_usd
            - self.gas_usd
            - self.flash_fee_usd
            - self.builder_tip_usd
            - self.other_cost_usd
    }

    /// M — Gross_bps = `10000*(L−1)` with `L = N/C`. `None` when the start
    /// amount is not positive or the result is not finite (R8: not computable).
    pub fn gross_bps(&self) -> Option<f64> {
        if self.start_amount_usd <= 0.0 {
            return None;
        }
        let v = 10_000.0 * (self.gross_over_input_usd / self.start_amount_usd);
        v.is_finite().then_some(v)
    }

    /// U — Net_bps = `10000*T/C`, the ranking key. `None` when not computable
    /// (R8) — such entries rank last, never mid-table on a fabricated 0.0.
    pub fn net_bps(&self) -> Option<f64> {
        if self.start_amount_usd <= 0.0 {
            return None;
        }
        let v = 10_000.0 * (self.net_profit_usd() / self.start_amount_usd);
        v.is_finite().then_some(v)
    }

    /// V — PASS = `AND(T>0, U>=Min_Net_bps)`. `None` when Net_bps is not
    /// computable (R8 — no verdict without the metric).
    pub fn passes(&self, min_net_bps: f64) -> Option<bool> {
        let u = self.net_bps()?;
        Some(self.net_profit_usd() > 0.0 && u >= min_net_bps)
    }
}

/// One rankable route: an opaque stable identity plus its sheet-07 economics.
///
/// `route_key` is caller-supplied (label + pools + intent hash at the wiring
/// site) and exists purely as the deterministic tie-break — the module itself
/// never interprets it.
#[derive(Debug, Clone, PartialEq)]
pub struct RankedRoute {
    pub route_key: String,
    pub economics: RouteNetEconomics,
}

/// The total deterministic order per sheet 07's Net_bps column:
///
/// 1. `Net_bps` DESC (the workbook's ranking metric);
/// 2. non-computable entries (`None`, R8) LAST, above them nothing is fabricated;
/// 3. every tie broken by `route_key` ASC (byte order) — so the SAME input set
///    yields the SAME output order regardless of input permutation (pinned by
///    the golden-orders tests).
///
/// Exposed standalone so callers carrying a parallel payload can sort
/// `(RankedRoute, payload)` pairs with the EXACT same comparator — one source
/// of order, no drift between the two entry points.
pub fn net_bps_order(a: &RankedRoute, b: &RankedRoute) -> Ordering {
    match (a.economics.net_bps(), b.economics.net_bps()) {
        (Some(x), Some(y)) => y
            .partial_cmp(&x)
            .unwrap_or(Ordering::Equal)
            .then_with(|| a.route_key.cmp(&b.route_key)),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => a.route_key.cmp(&b.route_key),
    }
}

/// Order a batch of routes by [`net_bps_order`] (see its docs). Total, never
/// panics, and never drops an entry — PASS and FAIL rows are all ordered (the
/// workbook table lists both).
pub fn rank_by_net_bps(entries: &mut [RankedRoute]) {
    entries.sort_by(net_bps_order);
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    fn econ(
        start: f64,
        gross: f64,
        gas: f64,
        flash: f64,
        tip: f64,
        other: f64,
    ) -> RouteNetEconomics {
        RouteNetEconomics {
            start_amount_usd: start,
            gross_over_input_usd: gross,
            gas_usd: gas,
            flash_fee_usd: flash,
            builder_tip_usd: tip,
            other_cost_usd: other,
        }
    }

    fn keys(entries: &[RankedRoute]) -> Vec<&str> {
        entries.iter().map(|e| e.route_key.as_str()).collect()
    }

    /// Full column arithmetic pinned to the sheet-07 formulas, hand-computed:
    /// C=10_000, gross(N−C)=60, gas=10, flash=5, tip=0, other=2 →
    /// N=10_060, S=10_043, T=43, M=60 bps, U=43 bps.
    #[test]
    fn formula_pin_matches_sheet07_row_arithmetic() {
        let e = econ(10_000.0, 60.0, 10.0, 5.0, 0.0, 2.0);
        assert!((e.net_final_usd() - 10_043.0).abs() < 1e-9);
        assert!((e.net_profit_usd() - 43.0).abs() < 1e-9);
        assert!((e.gross_bps().unwrap() - 60.0).abs() < 1e-9);
        assert!((e.net_bps().unwrap() - 43.0).abs() < 1e-9);
    }

    /// The workbook's golden order: Net_bps desc, exact ties by route_key asc,
    /// negatives below positives, non-computable (R8) last — and NOTHING dropped.
    #[test]
    fn golden_order_net_bps_desc_ties_by_route_key() {
        // b and c tie at 40 bps; d is net-negative; e has start=0 (None).
        let mut entries = vec![
            RankedRoute {
                route_key: "a".into(),
                economics: econ(10_000.0, 60.0, 10.0, 5.0, 0.0, 2.0), // U=43
            },
            RankedRoute {
                route_key: "d".into(),
                economics: econ(5_000.0, 4.0, 10.0, 2.5, 0.0, 1.5), // T=-10 → U=-20
            },
            RankedRoute {
                route_key: "c".into(),
                economics: econ(2_000.0, 16.0, 6.0, 1.0, 0.0, 1.0), // T=8 → U=40
            },
            RankedRoute {
                route_key: "b".into(),
                economics: econ(1_000.0, 8.0, 3.0, 0.5, 0.0, 0.5), // T=4 → U=40
            },
            RankedRoute {
                route_key: "e".into(),
                economics: econ(0.0, 1.0, 0.1, 0.0, 0.0, 0.0), // start=0 → None
            },
        ];
        rank_by_net_bps(&mut entries);
        assert_eq!(keys(&entries), vec!["a", "b", "c", "d", "e"]);
    }

    /// Determinism means permutation-independence: the same SET always yields
    /// the same order (fixed permutations — no RNG in tests).
    #[test]
    fn golden_order_is_input_permutation_independent() {
        let mk = |k: &str, start: f64, gross: f64| RankedRoute {
            route_key: k.into(),
            economics: econ(start, gross, 1.0, 0.0, 0.0, 0.0),
        };
        let base = [
            mk("x", 1_000.0, 9.0), // U=80
            mk("y", 1_000.0, 4.0), // U=30
            mk("z", 2_000.0, 8.0), // U=40
        ];
        let expected = vec!["x", "z", "y"];
        for perm in [
            vec![base[0].clone(), base[1].clone(), base[2].clone()],
            vec![base[2].clone(), base[0].clone(), base[1].clone()],
            vec![base[1].clone(), base[2].clone(), base[0].clone()],
        ] {
            let mut v = perm;
            rank_by_net_bps(&mut v);
            assert_eq!(keys(&v), expected);
        }
    }

    /// V column: `AND(T>0, U>=Min_Net_bps)` — both arms exercised at their
    /// exact boundaries (>= is inclusive; a zero profit fails even at min=0).
    #[test]
    fn pass_gate_is_net_positive_and_min_net_bps() {
        // U = 10000*5/10000 = 5.0 exactly at the default min → PASS.
        assert_eq!(
            econ(10_000.0, 8.0, 3.0, 0.0, 0.0, 0.0).passes(DEFAULT_MIN_NET_BPS),
            Some(true)
        );
        // One cent of gross below → U=4.99 → FAIL.
        assert_eq!(
            econ(10_000.0, 7.99, 3.0, 0.0, 0.0, 0.0).passes(DEFAULT_MIN_NET_BPS),
            Some(false)
        );
        // T=0 with min=0: U=0 >= 0 but T>0 is false → FAIL (the AND's first arm).
        assert_eq!(
            econ(10_000.0, 5.0, 5.0, 0.0, 0.0, 0.0).passes(0.0),
            Some(false)
        );
        // Metric not computable → no verdict (R8).
        assert_eq!(econ(0.0, 5.0, 1.0, 0.0, 0.0, 0.0).passes(0.0), None);
    }

    /// R8: a non-positive start amount (or non-finite arithmetic) yields `None`
    /// for every derived bps metric — never a fabricated 0.0 — and such
    /// entries rank last, deterministically by key.
    #[test]
    fn non_computable_is_none_never_fabricated_and_ranks_last() {
        let zero_start = econ(0.0, 5.0, 1.0, 0.0, 0.0, 0.0);
        assert_eq!(zero_start.net_bps(), None);
        assert_eq!(zero_start.gross_bps(), None);
        // The explicit marker behaves identically (no fabricated metrics).
        assert_eq!(RouteNetEconomics::not_computable().net_bps(), None);
        assert_eq!(RouteNetEconomics::not_computable().passes(0.0), None);
        let neg_start = econ(-1.0, 5.0, 1.0, 0.0, 0.0, 0.0);
        assert_eq!(neg_start.net_bps(), None);
        // NaN/inf components degrade to None too (not-finite guard).
        let nan_gross = econ(10_000.0, f64::NAN, 1.0, 0.0, 0.0, 0.0);
        assert_eq!(nan_gross.net_bps(), None);
        let inf_net = econ(1e-308, f64::MAX, 0.0, 0.0, 0.0, 0.0); // T/C overflows
        assert_eq!(inf_net.net_bps(), None);

        // Two non-computable entries order by key among themselves, last.
        let mut entries = vec![
            RankedRoute {
                route_key: "real".into(),
                economics: econ(1_000.0, 4.0, 1.0, 0.0, 0.0, 0.0), // U=30
            },
            RankedRoute {
                route_key: "nan-b".into(),
                economics: econ(10_000.0, f64::NAN, 1.0, 0.0, 0.0, 0.0),
            },
            RankedRoute {
                route_key: "nan-a".into(),
                economics: econ(0.0, 1.0, 0.0, 0.0, 0.0, 0.0),
            },
        ];
        rank_by_net_bps(&mut entries);
        assert_eq!(keys(&entries), vec!["real", "nan-a", "nan-b"]);
    }

    /// `01_CONFIG!B13` pin: the default gate equals the canonical knob's
    /// default (single source of truth — never a second hardcoded 5.0).
    #[test]
    fn default_min_net_bps_pins_workbook_config() {
        assert_eq!(DEFAULT_MIN_NET_BPS, 5.0);
        assert_eq!(
            DEFAULT_MIN_NET_BPS,
            crate::canonical_knobs::CanonicalKnobs::default().min_net_bps
        );
    }

    /// The kernel adapter prices the flash fee with the ARBX-0007 policy, so
    /// its NetProfit uses the same components as the kernel's `net_usd`
    /// (`gross − gas − ops − borrow*fee/10_000`) — identical up to float
    /// associativity (the two sides subtract `fee` and `ops` in swapped
    /// order, a ≤ ~1e-12 relative difference; the 0007 ulp lesson).
    #[test]
    fn from_kernel_fee_policy_matches_financing_selected_mode() {
        // Flash-backed: borrow = start ⇒ fee = start * 5 bps (Aave policy).
        let flash = RouteNetEconomics::from_kernel(12_345.6, 100.0, 7.0, 2.0, 12_345.6);
        let fee = 12_345.6 * crate::financing::FinancingMode::AaveFlashLoan.fee_bps() / 10_000.0;
        assert_eq!(flash.flash_fee_usd, fee);
        assert_eq!(flash.builder_tip_usd, 0.0);
        let kernel_net = 100.0 - 7.0 - 2.0 - fee;
        assert!(
            (flash.net_profit_usd() - kernel_net).abs() < 1e-9,
            "flash.net_profit={} kernel_net={}",
            flash.net_profit_usd(),
            kernel_net
        );
        // Own capital: borrow = 0 ⇒ no fee (exact here: no fee subtraction).
        let own = RouteNetEconomics::from_kernel(1_000.0, 10.0, 2.0, 1.0, 0.0);
        assert_eq!(own.flash_fee_usd, 0.0);
        assert!((own.net_profit_usd() - 7.0).abs() < 1e-12);
    }

    /// The standalone comparator and `rank_by_net_bps` are one order — callers
    /// sorting `(RankedRoute, payload)` pairs via `net_bps_order` get exactly
    /// the batch function's sequence (no drift between entry points).
    #[test]
    fn exposed_comparator_matches_rank_output() {
        let mut batch = vec![
            RankedRoute {
                route_key: "k2".into(),
                economics: econ(1_000.0, 4.0, 1.0, 0.0, 0.0, 0.0), // U=30
            },
            RankedRoute {
                route_key: "k3".into(),
                economics: econ(0.0, 1.0, 0.0, 0.0, 0.0, 0.0), // None
            },
            RankedRoute {
                route_key: "k1".into(),
                economics: econ(1_000.0, 9.0, 1.0, 0.0, 0.0, 0.0), // U=80
            },
        ];
        rank_by_net_bps(&mut batch);
        let expected: Vec<&str> = keys(&batch);
        let mut pairs: Vec<(RankedRoute, u8)> = vec![
            (
                RankedRoute {
                    route_key: "k2".into(),
                    economics: econ(1_000.0, 4.0, 1.0, 0.0, 0.0, 0.0),
                },
                2,
            ),
            (
                RankedRoute {
                    route_key: "k3".into(),
                    economics: econ(0.0, 1.0, 0.0, 0.0, 0.0, 0.0),
                },
                3,
            ),
            (
                RankedRoute {
                    route_key: "k1".into(),
                    economics: econ(1_000.0, 9.0, 1.0, 0.0, 0.0, 0.0),
                },
                1,
            ),
        ];
        pairs.sort_by(|(a, _), (b, _)| net_bps_order(a, b));
        let got: Vec<&str> = pairs.iter().map(|(r, _)| r.route_key.as_str()).collect();
        assert_eq!(got, expected);
        assert_eq!(
            pairs.iter().map(|(_, p)| *p).collect::<Vec<_>>(),
            vec![1, 2, 3]
        );
    }
}
// wdac-probe rev-2 (content-hash refresh for the bins test binary)
