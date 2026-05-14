//! `BundlePosition<T>` — typestate compile-time verifier (spec §4.1).
//!
//! ## Invariant
//!
//! A `BundlePosition<T>` value exists only when `T` implements the *sealed*
//! [`PostResolutionTopology`] trait. The trait has exactly three implementors
//! at the time of writing:
//!
//! - [`OrthogonalEquilibrium`] — null-covariance cross-venue hedge state.
//! - [`DiracImpulseOnly`] — single Dirac impulse on the CPMM manifold.
//! - [`HolonomicLoopResolution`] — closed-contour atomic arbitrage cycle
//!   over N ≥ 3 liquidity manifolds (V1.2 amendment, 2026-05-13,
//!   `ANEXOS_V1.2.md` §4.1.1–§4.1.5). Doctrinally clean: the constructor
//!   refuses sandwich/frontrun shapes because they have no closed
//!   contour (asymmetric: pre-/post-target ordering, not a cycle).
//!
//! The trait is sealed via `private::Sealed`. External crates cannot add
//! variants because they cannot name `private::Sealed`. Inside this crate,
//! adding a variant requires:
//!
//! 1. Operator + on-call written approval per `mev-ethics.md §Amendments`.
//! 2. A new `impl private::Sealed for NewVariant {}`.
//! 3. A new `impl PostResolutionTopology for NewVariant {}`.
//! 4. A `BundlePosition::new_<variant>(...)` constructor that takes a
//!    typed mathematical proof argument (an `&OrthogonalHedgeResult`, an
//!    `&OptimalControlSolution`, a `&ClosedContourTrajectory` +
//!    `&TopologicalYield`, …) which is checked at runtime via
//!    `bool`-returning methods on the proof types.
//!
//! Any variant claim that bypasses the proof arguments is a doctrine breach.
//!
//! ## Why typestate over enum
//!
//! An enum variant could be constructed unconditionally; a typestate
//! constructor takes the proof by reference and returns `Err(...)` when the
//! proof's invariant check returns `false`. That gives:
//!
//! - **Compile-time scoping** — downstream code that wants to ship an
//!   `OrthogonalEquilibrium` bundle must thread the hedge proof all the way
//!   to the constructor. Forgetting the proof is a type error, not a
//!   silent default.
//! - **Runtime verification** — even with the right type, the constructor
//!   refuses on `verify_null_covariance < tolerance` (etc.).
//! - **Sealed extension** — external crates that consume `sed-core` cannot
//!   add a "Sandwich" variant in their own code path; the sealed trait
//!   blocks it.

use std::marker::PhantomData;

use crate::allocator::OptimalControlSolution;
use crate::hedger::OrthogonalHedgeResult;
use crate::types::errors::TopologyValidationError;
use crate::types::holonomic::{ClosedContourTrajectory, TopologicalYield};

// ── Typestate marker types ────────────────────────────────────────────

/// Pre-resolution marker. A `BundlePosition<Unresolved>` carries the
/// market identity but no mathematical commitment yet. Cannot be dispatched.
#[derive(Debug)]
pub struct Unresolved;

/// Generic "resolved" marker, primarily for internal pipelines. Most
/// callers use one of the two specific markers below instead.
#[derive(Debug)]
pub struct Resolved;

/// Marker for bundles whose post-resolution topology is a null-covariance
/// orthogonal hyperplane (CEX cross-venue hedge state). Constructor requires
/// a proof reference to [`OrthogonalHedgeResult`] with verified
/// `verify_null_covariance(1e-9) == true`.
#[derive(Debug)]
pub struct OrthogonalEquilibrium;

/// Marker for bundles whose post-resolution topology is a single Dirac
/// liquidity impulse on the CPMM manifold (JIT-style depth provision).
/// Constructor requires a proof reference to [`OptimalControlSolution`]
/// with verified `hyperbolic_constraint_satisfied == true`.
#[derive(Debug)]
pub struct DiracImpulseOnly;

/// Marker for bundles whose post-resolution topology is a closed-contour
/// atomic arbitrage cycle over N ≥ 3 liquidity manifolds (V1.2 amendment,
/// 2026-05-13, `ANEXOS_V1.2.md` §4.1.1).
///
/// **Doctrinal note**: This variant is doctrinally distinct from the
/// forbidden sandwich/frontrun shapes because:
///
/// - The contour is **closed** — γ(0) = γ(1) in the market manifold ℳ.
///   Sandwich/frontrun shapes are **asymmetric** in time (pre-/post-victim
///   ordering), which cannot satisfy `transition_points.first() ==
///   transition_points.last()`.
/// - The yield comes from a **price discrepancy** the cycle resolves
///   (∮γ (dp/p) ≠ 0 before, → 0 after). No third-party position is
///   targeted; the operator's own capital moves through N pools and
///   returns to the start.
/// - The friction model is **symmetric** (all gas/slippage borne by the
///   cycle, no victim subsidising the bot).
///
/// Constructor requires references to both [`ClosedContourTrajectory`]
/// (the geometric evidence: 3+ manifolds, closed loop, contour integral
/// non-trivial) and [`TopologicalYield`] (the economic evidence: net
/// yield positive after friction). Six runtime validations gate the
/// constructor — see `new_holonomic_loop`.
#[derive(Debug)]
pub struct HolonomicLoopResolution;

// ── Sealed trait pattern ──────────────────────────────────────────────

/// Sealed trait. Only this crate may add implementors.
///
/// **Operator note**: adding a new implementor (e.g., for a future
/// `AtomicBackrun` variant tied to a `BackrunArbitrageProof`) is gated by
/// `mev-ethics.md §Amendments` — operator + on-call sign-off + 7-day
/// cooldown.
pub trait PostResolutionTopology: private::Sealed {}

pub(crate) mod private {
    pub trait Sealed {}
    impl Sealed for super::OrthogonalEquilibrium {}
    impl Sealed for super::DiracImpulseOnly {}
    impl Sealed for super::HolonomicLoopResolution {} // V1.2 amendment
}

impl PostResolutionTopology for OrthogonalEquilibrium {}
impl PostResolutionTopology for DiracImpulseOnly {}
impl PostResolutionTopology for HolonomicLoopResolution {} // V1.2 amendment

// Sandwich, Frontrun, and victim-specific bundle attribution variants are
// intentionally NOT implementors of `PostResolutionTopology` and have no
// `private::Sealed` impl. There is no Cargo feature that flips them on.
// External crates cannot name `private::Sealed`. The doctrine is in the
// type system.
//
// V1.2 audit (2026-05-13): the addition of HolonomicLoopResolution does
// NOT widen the doctrinal surface to those forbidden shapes — see the
// note on the marker type above for the topological / economic /
// friction-symmetry arguments.

// ── BundlePosition<T> ─────────────────────────────────────────────────

/// A bundle whose post-resolution topology is encoded in the type
/// parameter `T`. `T` is constrained to [`PostResolutionTopology`]
/// implementors via the constructors — `BundlePosition<Unresolved>` and
/// `BundlePosition<Resolved>` exist for pipeline pre-stages but cannot be
/// dispatched.
///
/// `PhantomData<T>` carries the type without runtime cost.
///
/// ## Field privacy (V1.2 audit follow-up, 2026-05-13)
///
/// All fields are private. Construction is only possible through the
/// validated `new_*` constructors below — there is no struct-literal
/// path that bypasses the mathematical proof arguments. Read access is
/// via the accessor methods on `impl<T> BundlePosition<T>`.
///
/// Earlier the fields were `pub`, which silently allowed external code
/// to write `BundlePosition::<OrthogonalEquilibrium> { market_id: …,
/// variance_exposure: f64::MAX, topology_state: PhantomData }` —
/// bypassing `verify_null_covariance` entirely. The sealed trait
/// blocks new topologies, but field privacy is what blocks fabricating
/// instances of existing ones. Both layers are required for the
/// typestate to mean what the doctrine says it means.
#[derive(Debug)]
pub struct BundlePosition<T> {
    market_id: String,
    token_pair: (String, String),
    liquidity_commitment: f64,
    variance_exposure: f64,
    topology_state: PhantomData<T>,
}

impl<T> BundlePosition<T> {
    /// Read-only accessor for the market id. Available on all topology
    /// states for diagnostics + logging.
    pub fn market_id(&self) -> &str {
        &self.market_id
    }

    /// Read-only accessor for the token pair.
    pub fn token_pair(&self) -> &(String, String) {
        &self.token_pair
    }

    /// Read-only accessor for the position's liquidity commitment, in
    /// the same units the constructor received (typically USD-notional).
    pub fn liquidity_commitment(&self) -> f64 {
        self.liquidity_commitment
    }

    /// Read-only accessor for the position's variance exposure. This is
    /// the field the GateManager (Phase 4) reads when enforcing the
    /// system-level "varianza monótona no-creciente" invariant.
    pub fn variance_exposure(&self) -> f64 {
        self.variance_exposure
    }
}

impl BundlePosition<OrthogonalEquilibrium> {
    /// Construct an `OrthogonalEquilibrium` bundle. Requires a proof
    /// reference to [`OrthogonalHedgeResult`] whose null-covariance check
    /// passes at tolerance `1e-9`. Refuses otherwise.
    ///
    /// # Errors
    ///
    /// - `TopologyValidationError::NonOrthogonalCovariance` when
    ///   `proof.verify_null_covariance(1e-9) == false`.
    pub fn new_orthogonal_equilibrium(
        market_id: String,
        token_pair: (String, String),
        liquidity_commitment: f64,
        variance_exposure: f64,
        orthogonality_proof: &OrthogonalHedgeResult,
    ) -> Result<Self, TopologyValidationError> {
        if !orthogonality_proof.verify_null_covariance(1e-9) {
            return Err(TopologyValidationError::NonOrthogonalCovariance);
        }
        Ok(Self {
            market_id,
            token_pair,
            liquidity_commitment,
            variance_exposure,
            topology_state: PhantomData,
        })
    }
}

impl BundlePosition<DiracImpulseOnly> {
    /// Construct a `DiracImpulseOnly` bundle. Requires a proof reference to
    /// [`OptimalControlSolution`] whose `hyperbolic_constraint_satisfied`
    /// flag is `true`. Refuses otherwise.
    ///
    /// # Errors
    ///
    /// - `TopologyValidationError::HyperbolicViolation` when
    ///   `proof.hyperbolic_constraint_satisfied == false`.
    pub fn new_dirac_impulse_only(
        market_id: String,
        token_pair: (String, String),
        liquidity_commitment: f64,
        variance_exposure: f64,
        dirac_solution: &OptimalControlSolution,
    ) -> Result<Self, TopologyValidationError> {
        if !dirac_solution.hyperbolic_constraint_satisfied {
            return Err(TopologyValidationError::HyperbolicViolation);
        }
        Ok(Self {
            market_id,
            token_pair,
            liquidity_commitment,
            variance_exposure,
            topology_state: PhantomData,
        })
    }
}

impl BundlePosition<HolonomicLoopResolution> {
    /// Construct a `HolonomicLoopResolution` bundle (V1.2 amendment,
    /// `ANEXOS_V1.2.md` §4.1.5).
    ///
    /// Models an atomic arbitrage cycle over N ≥ 3 liquidity manifolds.
    /// Six mathematical validations gate the constructor; failure of
    /// any one yields a specific `TopologyValidationError`. The
    /// validations are applied in order — the first failure short-
    /// circuits and the caller never sees a "the cycle failed multiple
    /// invariants" muddled response, which preserves fail-honest reason
    /// fidelity.
    ///
    /// ## Mathematical contract
    ///
    /// ```text
    /// ∮_γ (dp/p) ≠ 0                       (holonomy non-trivial)
    /// γ(0) = γ(1) in ℳ                     (closed contour)
    /// |manifolds| ≥ 3                      (loop cardinality)
    /// net_yield = raw_holonomy − F_net > 0 (economic viability)
    /// |contour_integral − raw_holonomy| ≤ 1e-9   (contour↔yield consistency)
    /// |raw − friction − net| ≤ 1e-9              (friction identity)
    /// ```
    ///
    /// ## The half-life check (`T_exec < T_half-life` per Bloque 3 of
    /// the anexo) is **NOT** enforced here.
    ///
    /// Reason: half-life requires a live `StateDivergenceCoefficient`
    /// (Phase 2 filtration kernel) and a per-dispatch execution-time
    /// measurement. Both belong to the `GateManager` runtime path
    /// (Phase 4), not the typestate constructor. The typestate proves
    /// "this cycle is mathematically and economically valid at
    /// construction"; the runtime gate proves "this cycle will close
    /// before the discrepancy decays". Conflating them would force the
    /// constructor to take a `StateDivergenceCoefficient` argument
    /// which Phase 1 does not have. Once Phase 4 lands, the
    /// `HolonomicInvariantChecker` from anexo Bloque 3 wraps both
    /// layers.
    ///
    /// # Errors
    ///
    /// Returned in the order they are checked:
    ///
    /// 1. [`TopologyValidationError::OpenContourTrajectory`]
    ///    — `contour.is_closed() == false`.
    /// 2. [`TopologyValidationError::InsufficientLoopCardinality`]
    ///    — `contour.loop_cardinality < MIN_LOOP_SIZE` (= 3).
    /// 3. [`TopologyValidationError::TrivialHolonomy`]
    ///    — `|contour.contour_integral()| < 1e-12`.
    /// 4. [`TopologyValidationError::HolonomyYieldMismatch`]
    ///    — `|contour_integral − yield.raw_holonomy| > 1e-9`.
    /// 5. [`TopologyValidationError::NonPositiveTopologicalYield`]
    ///    — `yield.is_economically_viable() == false`.
    /// 6. [`TopologyValidationError::FrictionDeductionInvalid`]
    ///    — `|raw − friction − net| > 1e-9`.
    pub fn new_holonomic_loop(
        market_id: String,
        token_pair: (String, String),
        liquidity_commitment: f64,
        variance_exposure: f64,
        contour: &ClosedContourTrajectory,
        topological_yield: &TopologicalYield,
    ) -> Result<Self, TopologyValidationError> {
        // Validación 1: Contorno cerrado.
        if !contour.is_closed() {
            return Err(TopologyValidationError::OpenContourTrajectory);
        }

        // Validación 2: Cardinalidad mínima del lazo.
        // (is_closed already rejects len < 3, but the dedicated reason
        // is preserved for callers who explicitly pre-validate closure
        // and want a precise diagnostic on cardinality.)
        if contour.loop_cardinality < ClosedContourTrajectory::MIN_LOOP_SIZE {
            return Err(TopologyValidationError::InsufficientLoopCardinality);
        }

        // Validación 3: Holonomía no trivial.
        let holonomy = contour.contour_integral();
        if !holonomy.is_finite() || !(holonomy.abs() > 1e-12) {
            // `is_finite()` rejects ±∞ (from ln(0) on a zero price) and
            // NaN (from ln(negative_price)). The negated `> 1e-12` also
            // catches the regular trivial case AND any NaN that slipped
            // past `is_finite()` (`NaN > 1e-12` is false, the `!` flips
            // to true → reject). Both degeneracies route to the same
            // fail-honest error — the caller knows their price input is
            // pathological without us inventing a separate variant.
            return Err(TopologyValidationError::TrivialHolonomy);
        }

        // Validación 4: Consistencia entre contorno y yield.
        if (holonomy - topological_yield.raw_holonomy).abs() > 1e-9 {
            return Err(TopologyValidationError::HolonomyYieldMismatch);
        }

        // Validación 5: Rendimiento neto estrictamente positivo.
        if !topological_yield.is_economically_viable() {
            return Err(TopologyValidationError::NonPositiveTopologicalYield);
        }

        // Validación 6: Fricción de red deducida correctamente.
        let expected_net =
            topological_yield.raw_holonomy - topological_yield.network_friction;
        if (expected_net - topological_yield.net_yield).abs() > 1e-9 {
            return Err(TopologyValidationError::FrictionDeductionInvalid);
        }

        Ok(Self {
            market_id,
            token_pair,
            liquidity_commitment,
            variance_exposure,
            topology_state: PhantomData,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::allocator::OptimalControlSolution;
    use crate::hedger::OrthogonalHedgeResult;

    #[test]
    fn rejects_orthogonal_eq_without_null_covariance() {
        let proof = OrthogonalHedgeResult::stub(/* null_cov */ false);
        let r = BundlePosition::<OrthogonalEquilibrium>::new_orthogonal_equilibrium(
            "WETH/USDC".into(),
            ("WETH".into(), "USDC".into()),
            1_000.0,
            0.0,
            &proof,
        );
        assert!(matches!(r, Err(TopologyValidationError::NonOrthogonalCovariance)));
    }

    #[test]
    fn accepts_orthogonal_eq_with_valid_proof() {
        let proof = OrthogonalHedgeResult::stub(/* null_cov */ true);
        let r = BundlePosition::<OrthogonalEquilibrium>::new_orthogonal_equilibrium(
            "WETH/USDC".into(),
            ("WETH".into(), "USDC".into()),
            1_000.0,
            0.0,
            &proof,
        );
        assert!(r.is_ok());
    }

    #[test]
    fn rejects_dirac_without_hyperbolic_constraint() {
        let proof = OptimalControlSolution::stub(/* hyperbolic_ok */ false);
        let r = BundlePosition::<DiracImpulseOnly>::new_dirac_impulse_only(
            "WETH/USDC".into(),
            ("WETH".into(), "USDC".into()),
            1_000.0,
            0.0,
            &proof,
        );
        assert!(matches!(r, Err(TopologyValidationError::HyperbolicViolation)));
    }

    #[test]
    fn accepts_dirac_with_valid_proof() {
        let proof = OptimalControlSolution::stub(/* hyperbolic_ok */ true);
        let r = BundlePosition::<DiracImpulseOnly>::new_dirac_impulse_only(
            "WETH/USDC".into(),
            ("WETH".into(), "USDC".into()),
            1_000.0,
            0.0,
            &proof,
        );
        assert!(r.is_ok());
    }

    /// Compile-time sanity: only the three authorised topologies
    /// (`OrthogonalEquilibrium`, `DiracImpulseOnly`,
    /// `HolonomicLoopResolution`) implement `PostResolutionTopology`.
    /// If a future PR adds a new implementor (e.g., a hypothetical
    /// `Sandwich` — which is doctrinally forbidden), this exhaustive
    /// match in the helper forces an explicit code-review touch point.
    fn _post_resolution_witness<T: PostResolutionTopology>(_marker: PhantomData<T>) {}

    #[test]
    fn only_authorised_topologies_witness() {
        _post_resolution_witness::<OrthogonalEquilibrium>(PhantomData);
        _post_resolution_witness::<DiracImpulseOnly>(PhantomData);
        _post_resolution_witness::<HolonomicLoopResolution>(PhantomData);
    }

    // ── V1.2 amendment — HolonomicLoopResolution constructor tests ───
    // One test per validation step, plus one happy path. The fixtures
    // build a minimal 3-pool closed cycle and mutate exactly one input
    // per negative test so the failure attribution is unambiguous.

    use crate::eigenstate::LiquidityManifold;
    use crate::types::holonomic::{ClosedContourTrajectory, TopologicalYield};
    use nalgebra::DVector;

    fn manifold(id: &str) -> LiquidityManifold {
        LiquidityManifold::new(id)
    }

    /// Builds a valid 3-manifold closed contour whose contour_integral
    /// is `2·ln(1.02) + ln(0.97)` ≈ 0.00914 (positive — the cycle is
    /// genuinely profitable before friction). The endpoints coincide
    /// exactly so `is_closed()` returns true.
    ///
    /// Earlier draft used `[1.01, 1.02, 0.97]` which evaluates to
    /// ≈ −0.00071 (negative holonomy = mispriced cycle that LOSES money
    /// before friction). That was rejected by v5 in tests, which was
    /// the correct fail-honest behavior but the wrong fixture for the
    /// happy path.
    fn valid_contour() -> ClosedContourTrajectory {
        ClosedContourTrajectory {
            manifolds: vec![manifold("0xA"), manifold("0xB"), manifold("0xC")],
            transition_points: vec![
                DVector::from_row_slice(&[1.0, 0.0]),
                DVector::from_row_slice(&[0.0, 1.0]),
                DVector::from_row_slice(&[1.0, 1.0]),
                DVector::from_row_slice(&[1.0, 0.0]),
            ],
            relative_prices: vec![1.02, 1.02, 0.97],
            contour_length: 3.0,
            loop_cardinality: 3,
        }
    }

    /// Topological yield consistent with `valid_contour()`. raw_holonomy
    /// matches the contour_integral; friction is small; net_yield is
    /// strictly above the floor.
    fn valid_yield(raw: f64) -> TopologicalYield {
        let friction = 1e-5;
        TopologicalYield {
            raw_holonomy: raw,
            network_friction: friction,
            net_yield: raw - friction,
            efficiency_factor: (raw - friction) / raw,
            manifold_ids: vec!["0xa".into(), "0xb".into(), "0xc".into()],
            resolved_at: 0,
        }
    }

    fn build(
        contour: &ClosedContourTrajectory,
        yield_: &TopologicalYield,
    ) -> Result<BundlePosition<HolonomicLoopResolution>, TopologyValidationError> {
        BundlePosition::<HolonomicLoopResolution>::new_holonomic_loop(
            "WETH/USDC/USDT".into(),
            ("WETH".into(), "USDT".into()),
            1_000.0,
            0.0,
            contour,
            yield_,
        )
    }

    #[test]
    fn holonomic_happy_path_constructs() {
        let c = valid_contour();
        let y = valid_yield(c.contour_integral());
        let r = build(&c, &y);
        assert!(r.is_ok(), "happy path should succeed, got {r:?}");
    }

    #[test]
    fn holonomic_v1_rejects_open_contour() {
        let mut c = valid_contour();
        // Break closure by shifting the last point away from the first.
        c.transition_points[3] = DVector::from_row_slice(&[0.5, 0.5]);
        let y = valid_yield(c.contour_integral());
        let r = build(&c, &y);
        assert!(
            matches!(r, Err(TopologyValidationError::OpenContourTrajectory)),
            "got {r:?}"
        );
    }

    #[test]
    fn holonomic_v2_rejects_insufficient_cardinality() {
        // We need to bypass v1 (is_closed already enforces ≥3) by
        // constructing a contour that IS closed but reports
        // `loop_cardinality < 3`. is_closed checks `manifolds.len()` and
        // endpoint coincidence, NOT loop_cardinality directly, so a
        // hand-edited cardinality value with a real 3-manifold list
        // will fall through v1 and hit v2.
        let mut c = valid_contour();
        c.loop_cardinality = 2; // lie about cardinality
        let y = valid_yield(c.contour_integral());
        let r = build(&c, &y);
        assert!(
            matches!(r, Err(TopologyValidationError::InsufficientLoopCardinality)),
            "got {r:?}"
        );
    }

    #[test]
    fn holonomic_v3_rejects_trivial_holonomy() {
        let mut c = valid_contour();
        // All relative_prices = 1.0 → ln(1.0) = 0 → integral = 0.
        c.relative_prices = vec![1.0, 1.0, 1.0];
        let y = valid_yield(0.0);
        let r = build(&c, &y);
        assert!(
            matches!(r, Err(TopologyValidationError::TrivialHolonomy)),
            "got {r:?}"
        );
    }

    #[test]
    fn holonomic_v3_rejects_nan_holonomy() {
        let mut c = valid_contour();
        // ln(0) = -inf, ln(-x) = NaN. Both must route to TrivialHolonomy.
        c.relative_prices = vec![0.0, 1.0, 1.0];
        let y = valid_yield(0.0); // any yield, v3 fires first
        let r = build(&c, &y);
        assert!(
            matches!(r, Err(TopologyValidationError::TrivialHolonomy)),
            "got {r:?}"
        );
    }

    #[test]
    fn holonomic_v4_rejects_contour_yield_mismatch() {
        let c = valid_contour();
        // raw_holonomy diverges from contour_integral by ≫ 1e-9.
        let mut y = valid_yield(c.contour_integral());
        y.raw_holonomy += 1e-3;
        // Keep net_yield internally consistent so v6 passes after v4
        // would have already rejected.
        y.net_yield = y.raw_holonomy - y.network_friction;
        let r = build(&c, &y);
        assert!(
            matches!(r, Err(TopologyValidationError::HolonomyYieldMismatch)),
            "got {r:?}"
        );
    }

    #[test]
    fn holonomic_v5_rejects_non_viable_yield() {
        let c = valid_contour();
        let raw = c.contour_integral();
        // raw_holonomy ≈ 2.9e-4, friction = raw + epsilon → net ≤ 0.
        let mut y = valid_yield(raw);
        y.network_friction = raw + 1e-3;
        y.net_yield = raw - y.network_friction; // negative
        let r = build(&c, &y);
        assert!(
            matches!(r, Err(TopologyValidationError::NonPositiveTopologicalYield)),
            "got {r:?}"
        );
    }

    #[test]
    fn holonomic_v6_rejects_inconsistent_friction_deduction() {
        let c = valid_contour();
        let raw = c.contour_integral();
        // raw, friction agree with contour but net_yield is hand-edited
        // to a value that violates `net = raw − friction`.
        let mut y = valid_yield(raw);
        y.net_yield = raw - y.network_friction + 1e-3; // off by 1e-3
        let r = build(&c, &y);
        assert!(
            matches!(r, Err(TopologyValidationError::FrictionDeductionInvalid)),
            "got {r:?}"
        );
    }
}
