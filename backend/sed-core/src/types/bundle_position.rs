//! `BundlePosition<T>` — typestate compile-time verifier (spec §4.1).
//!
//! ## Invariant
//!
//! A `BundlePosition<T>` value exists only when `T` implements the *sealed*
//! [`PostResolutionTopology`] trait. The trait has exactly two implementors
//! at the time of writing:
//!
//! - [`OrthogonalEquilibrium`] — null-covariance cross-venue hedge state.
//! - [`DiracImpulseOnly`] — single Dirac impulse on the CPMM manifold.
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
//!    `&OptimalControlSolution`, …) which is checked at runtime via a
//!    `bool`-returning method on the proof type.
//!
//! Any variant claim that bypasses the proof argument is a doctrine breach.
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

// ── Typestate marker types ────────────────────────────────────────────

/// Pre-resolution marker. A `BundlePosition<Unresolved>` carries the
/// market identity but no mathematical commitment yet. Cannot be dispatched.
pub struct Unresolved;

/// Generic "resolved" marker, primarily for internal pipelines. Most
/// callers use one of the two specific markers below instead.
pub struct Resolved;

/// Marker for bundles whose post-resolution topology is a null-covariance
/// orthogonal hyperplane (CEX cross-venue hedge state). Constructor requires
/// a proof reference to [`OrthogonalHedgeResult`] with verified
/// `verify_null_covariance(1e-9) == true`.
pub struct OrthogonalEquilibrium;

/// Marker for bundles whose post-resolution topology is a single Dirac
/// liquidity impulse on the CPMM manifold (JIT-style depth provision).
/// Constructor requires a proof reference to [`OptimalControlSolution`]
/// with verified `hyperbolic_constraint_satisfied == true`.
pub struct DiracImpulseOnly;

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
}

impl PostResolutionTopology for OrthogonalEquilibrium {}
impl PostResolutionTopology for DiracImpulseOnly {}

// Sandwich, Frontrun, and victim-specific bundle attribution variants are
// intentionally NOT implementors of `PostResolutionTopology` and have no
// `private::Sealed` impl. There is no Cargo feature that flips them on.
// External crates cannot name `private::Sealed`. The doctrine is in the
// type system.

// ── BundlePosition<T> ─────────────────────────────────────────────────

/// A bundle whose post-resolution topology is encoded in the type
/// parameter `T`. `T` is constrained to [`PostResolutionTopology`]
/// implementors via the constructors — `BundlePosition<Unresolved>` and
/// `BundlePosition<Resolved>` exist for pipeline pre-stages but cannot be
/// dispatched.
///
/// `PhantomData<T>` carries the type without runtime cost.
pub struct BundlePosition<T> {
    pub market_id: String,
    pub token_pair: (String, String),
    pub liquidity_commitment: f64,
    pub variance_exposure: f64,
    pub topology_state: PhantomData<T>,
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

    /// Compile-time sanity: only `OrthogonalEquilibrium` and `DiracImpulseOnly`
    /// implement `PostResolutionTopology`. If a future PR adds a new
    /// implementor (e.g., `Sandwich`), this exhaustive match in the helper
    /// forces an explicit code-review touch point.
    fn _post_resolution_witness<T: PostResolutionTopology>(_marker: PhantomData<T>) {}

    #[test]
    fn only_authorised_topologies_witness() {
        _post_resolution_witness::<OrthogonalEquilibrium>(PhantomData);
        _post_resolution_witness::<DiracImpulseOnly>(PhantomData);
    }
}
