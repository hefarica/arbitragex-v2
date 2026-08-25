//! Financing-mode dimension for sized routes (ARBX-0007).
//!
//! Doctrine (`skills/arbitragex-ultra/SUPER_SKILL.md` "Financing = dimensión
//! de ruta"): the financing mode changes WHICH routes are viable, not how many
//! are discovered. Every sized route is evaluated against the canonical mode
//! set (`canonical_knobs::FINANCING_MODES`) so the funnel can answer
//! "what survives under each funding mode" — the parallel evaluation is the
//! dimension; the selected mode prices the net gate.
//!
//! Fee basis (RICH doctrine — "fees on-chain, never hardcode"):
//! - Aave V3 flash loan fee is 5 bps TODAY (2026-08) and is governance-
//!   controlled. `AAVE_FLASH_LOAN_FEE_BPS` pins today's value in exactly one
//!   place; the follow-up replaces it with an on-chain read (`Pool
//!   .FLASHLOAN_PREMIUM_TOTAL()`), not a second constant.
//! - Balancer V2 vault flash loans are 0-fee by design (protocol subsidy);
//!   availability per asset is a runtime precondition (executor-side venue
//!   check), not modeled here.
//! - Uniswap-V2 `flashSwap` charges the pool's swap fee (30 bps typical tier)
//!   on the borrowed amount.
//! - `OwnCapital` pays no financing fee — the capital opportunity cost is
//!   handled by the Kelly/cap machinery, not here.
//!
//! Selection policy: `selected_mode` keeps today's implicit behavior EXACT —
//! a flash-backed route (`base_strategy.is_some()`) prices the legacy 5 bps
//! Aave mode; a non-flash route prices `OwnCapital` (fee 0). Consuming the
//! `ARBX_KNOB_SELECTED_FINANCING` knob (declared + validated by
//! `canonical_knobs`, currently declarative/observability per XLS-CANON-01)
//! is a deliberate follow-up: promoting it to hot-path selection changes
//! net-gate economics and needs its own AC, not a silent flip. (Revision 2,
//! 2026-08-24: wired into both size_optimizer kernel sites + the orchestrator
//! rejection suffix. Rev 3: ulp-level fee pin in the policy test.)

use crate::canonical_knobs::FINANCING_MODES;

/// Aave V3 flash-loan premium, basis points of the borrowed amount.
/// TODAY's value (governance-controlled) — see module docs for the on-chain
/// read follow-up.
pub const AAVE_FLASH_LOAN_FEE_BPS: f64 = 5.0;
/// Balancer V2 vault flash-loan fee — 0 bps by design.
pub const BALANCER_FLASH_LOAN_FEE_BPS: f64 = 0.0;
/// Uniswap-V2 flashSwap fee — the typical 30 bps swap tier on the borrow.
pub const V2_FLASH_SWAP_FEE_BPS: f64 = 30.0;

/// The canonical financing modes (lowercase route-dimension tokens; the
/// canonical workbook tokens are the UPPERCASE forms in
/// `canonical_knobs::FINANCING_MODES`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FinancingMode {
    /// Fund the route with the engine's own balance — no financing fee.
    OwnCapital,
    /// Temporal Liquidity Superposition via Aave V3 (5 bps today).
    AaveFlashLoan,
    /// Temporal Liquidity Superposition via the Balancer V2 vault (0 bps).
    BalancerFlashLoan,
    /// V2 pair flash swap — the pool's swap fee on the borrowed side.
    V2FlashSwap,
}

impl FinancingMode {
    /// Stable lowercase token for rejection labels and log fields.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::OwnCapital => "own_capital",
            Self::AaveFlashLoan => "aave_fl",
            Self::BalancerFlashLoan => "balancer_fl",
            Self::V2FlashSwap => "v2_flash_swap",
        }
    }

    /// Parse a canonical workbook knob token (`OWN_CAPITAL`, `AAVE_FL`, …).
    /// Case-insensitive; `None` for anything outside the canonical set.
    pub fn from_token(token: &str) -> Option<Self> {
        match token.trim().to_ascii_uppercase().as_str() {
            "OWN_CAPITAL" => Some(Self::OwnCapital),
            "AAVE_FL" => Some(Self::AaveFlashLoan),
            "BALANCER_FL" => Some(Self::BalancerFlashLoan),
            "V2_FLASH_SWAP" => Some(Self::V2FlashSwap),
            _ => None,
        }
    }

    /// Financing fee for this mode, in basis points of the borrowed amount.
    pub fn fee_bps(&self) -> f64 {
        match self {
            Self::OwnCapital => 0.0,
            Self::AaveFlashLoan => AAVE_FLASH_LOAN_FEE_BPS,
            Self::BalancerFlashLoan => BALANCER_FLASH_LOAN_FEE_BPS,
            Self::V2FlashSwap => V2_FLASH_SWAP_FEE_BPS,
        }
    }
}

/// The flash-backed subset of modes (those that price a borrow).
pub const FLASH_MODES: [FinancingMode; 3] = [
    FinancingMode::AaveFlashLoan,
    FinancingMode::BalancerFlashLoan,
    FinancingMode::V2FlashSwap,
];

/// One mode's evaluation of the same sized route: identical gross/gas/ops,
/// differing only in the financing fee.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ModeEval {
    pub mode: FinancingMode,
    /// Financing fee for this mode, USD (`borrow_usd × fee_bps / 10_000`).
    pub fee_usd: f64,
    /// Net under this mode: `gross − gas − ops − fee_usd`.
    pub net_usd: f64,
}

impl ModeEval {
    /// Viable iff this mode's net is strictly positive.
    pub fn is_viable(&self) -> bool {
        self.net_usd > 0.0
    }
}

/// Evaluate the route under every applicable financing mode (the doctrine's
/// parallel evaluation). `borrow_usd ≤ 0` means the route needs no external
/// funding — flash modes would only add a fee to identical gross, so the
/// honest evaluation set is exactly `[OwnCapital]` (never a fabricated
/// "cheaper flash" alternative on a zero borrow).
pub fn evaluate_modes(
    gross_usd: f64,
    gas_usd: f64,
    ops_overhead_usd: f64,
    borrow_usd: f64,
) -> Vec<ModeEval> {
    if borrow_usd <= 0.0 {
        return vec![ModeEval {
            mode: FinancingMode::OwnCapital,
            fee_usd: 0.0,
            net_usd: gross_usd - gas_usd - ops_overhead_usd,
        }];
    }
    FLASH_MODES
        .iter()
        .map(|&mode| {
            let fee_usd = borrow_usd * mode.fee_bps() / 10_000.0;
            ModeEval {
                mode,
                fee_usd,
                net_usd: gross_usd - gas_usd - ops_overhead_usd - fee_usd,
            }
        })
        .collect()
}

/// The mode whose fee prices the net gate. Preserves today's implicit policy
/// exactly: flash-backed route → Aave 5 bps (the value the hardcoded
/// `borrow_usd * 0.0005` charged), otherwise → `OwnCapital` (fee 0).
pub fn selected_mode(borrow_usd: f64) -> FinancingMode {
    if borrow_usd <= 0.0 {
        FinancingMode::OwnCapital
    } else {
        FinancingMode::AaveFlashLoan
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The module's tokens must stay 1:1 with the canonical workbook surface
    /// (XLS-ENUM-01) — a drifted token would orphan the knob wiring.
    #[test]
    fn tokens_match_canonical_knobs_financing_modes() {
        let parsed: Vec<Option<FinancingMode>> = FINANCING_MODES
            .iter()
            .map(|t| FinancingMode::from_token(t))
            .collect();
        assert!(
            parsed.iter().all(Option::is_some),
            "every canonical token must parse: {FINANCING_MODES:?}"
        );
        let strs: Vec<&str> = parsed.iter().map(|m| m.unwrap().as_str()).collect();
        assert_eq!(strs.len(), 4);
        assert_eq!(
            strs.iter().collect::<std::collections::HashSet<_>>().len(),
            4,
            "as_str tokens must be distinct"
        );
    }

    #[test]
    fn from_token_is_case_insensitive_and_rejects_unknown() {
        assert_eq!(
            FinancingMode::from_token("aave_fl"),
            Some(FinancingMode::AaveFlashLoan)
        );
        assert_eq!(
            FinancingMode::from_token(" Balancer_FL "),
            Some(FinancingMode::BalancerFlashLoan)
        );
        assert_eq!(FinancingMode::from_token("MARGIN"), None);
        assert_eq!(FinancingMode::from_token(""), None);
    }

    /// Doctrine: "Aave = 5bps HOY, gobernable" — pinned so any governance
    /// change is a one-constant edit (plus the on-chain read follow-up).
    #[test]
    fn fee_bps_pins_todays_protocol_values() {
        assert_eq!(FinancingMode::OwnCapital.fee_bps(), 0.0);
        assert_eq!(FinancingMode::AaveFlashLoan.fee_bps(), 5.0);
        assert_eq!(FinancingMode::BalancerFlashLoan.fee_bps(), 0.0);
        assert_eq!(FinancingMode::V2FlashSwap.fee_bps(), 30.0);
    }

    #[test]
    fn evaluate_modes_zero_borrow_is_own_capital_only() {
        let evals = evaluate_modes(10.0, 2.0, 1.0, 0.0);
        assert_eq!(evals.len(), 1);
        assert_eq!(evals[0].mode, FinancingMode::OwnCapital);
        assert_eq!(evals[0].fee_usd, 0.0);
        assert!((evals[0].net_usd - 7.0).abs() < 1e-12);
        assert!(evals[0].is_viable());
    }

    /// The doctrine's parallel evaluation: a borrowed route is priced under
    /// all three flash modes with a strictly non-increasing net in fee order.
    #[test]
    fn evaluate_modes_flash_borrow_prices_all_three_in_fee_order() {
        let evals = evaluate_modes(10.0, 2.0, 1.0, 1_000.0);
        assert_eq!(evals.len(), 3);
        // Balancer 0 bps → fee 0, net 7
        assert_eq!(evals[0].mode, FinancingMode::AaveFlashLoan);
        assert!((evals[0].fee_usd - 0.5).abs() < 1e-12, "1000 × 5bps = 0.5");
        assert!((evals[0].net_usd - 6.5).abs() < 1e-12);
        assert_eq!(evals[1].mode, FinancingMode::BalancerFlashLoan);
        assert_eq!(evals[1].fee_usd, 0.0);
        assert!((evals[1].net_usd - 7.0).abs() < 1e-12);
        assert_eq!(evals[2].mode, FinancingMode::V2FlashSwap);
        assert!((evals[2].fee_usd - 3.0).abs() < 1e-12, "1000 × 30bps = 3.0");
        assert!((evals[2].net_usd - 4.0).abs() < 1e-12);
        // Fee order ⇒ net order (BALANCER ≥ AAVE ≥ V2_SWAP).
        assert!(evals[1].net_usd >= evals[0].net_usd);
        assert!(evals[0].net_usd >= evals[2].net_usd);
        assert!(evals.iter().all(ModeEval::is_viable));
    }

    /// The funnel claim: a route unviable under the selected mode can be
    /// viable under a cheaper one — the dimension, not a curiosity.
    #[test]
    fn viability_flips_across_modes_on_marginal_routes() {
        // gross 10, gas 2, ops 1 → own net 7; borrow 1000: Aave fee 0.5 (net
        // 6.5 viable), V2 swap fee 3.0 (net 4.0 viable)… make gross marginal:
        let evals = evaluate_modes(5.0, 2.0, 1.0, 1000.0);
        // nets: Aave 1.5, Balancer 2.0, V2 swap −1.0
        assert!(evals[0].is_viable());
        assert!(evals[1].is_viable());
        assert!(
            !evals[2].is_viable(),
            "V2 swap fee must sink a marginal route"
        );
    }

    /// Pins the NO-economics-change policy: whatever the knob future holds,
    /// today's selection is the legacy implicit mode (Aave 5 bps on a flash
    /// route, OwnCapital otherwise).
    #[test]
    fn selected_mode_preserves_legacy_policy() {
        assert_eq!(selected_mode(0.0), FinancingMode::OwnCapital);
        assert_eq!(selected_mode(-1.0), FinancingMode::OwnCapital);
        assert_eq!(selected_mode(1_000.0), FinancingMode::AaveFlashLoan);
        // The selected mode's fee must match the previously hardcoded math
        // (borrow × 0.0005) within 1 ulp: `× 5.0 / 10_000.0` rounds at the
        // division where `× 0.0005` rounds at the product (0.0005 is not
        // exactly representable) — the fee term differs by at most one ulp,
        // never the economic decision.
        let borrow = 12_345.6;
        let wired = borrow * selected_mode(borrow).fee_bps() / 10_000.0;
        let legacy = borrow * 0.0005;
        assert!((wired - legacy).abs() <= legacy.abs() * f64::EPSILON);
        assert_eq!(
            wired.to_bits().max(legacy.to_bits()) - wired.to_bits().min(legacy.to_bits()),
            1
        );
    }
}
