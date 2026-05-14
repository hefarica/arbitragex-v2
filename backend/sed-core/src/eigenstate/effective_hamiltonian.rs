//! `effective_hamiltonian` — market-state Hamiltonian constructor (spec §3.2.1).
//!
//! ## Physical Model
//!
//! The effective Hamiltonian H captures the energetic landscape of the
//! liquidity manifold network. Each diagonal element H_ii represents the
//! self-energy of manifold i (depth × volatility), and each off-diagonal
//! element H_ij represents the coupling strength between manifolds i and j
//! (cross-pool correlation, routing connectivity).
//!
//! The eigenvalues of H correspond to the energy levels of the system's
//! normal modes — the collective oscillation patterns of liquidity across
//! the network. The ground state (smallest eigenvalue) represents the
//! equilibrium configuration; excited states represent transient
//! topologically exploitable discrepancies.
//!
//! ## CDC Perturbation
//!
//! The State Divergence Coefficient (CDC) from Phase 2 enters as a
//! perturbation parameter that shifts the diagonal elements:
//!
//! ```text
//!     H'_ii = H_ii + α · CDC_value
//! ```
//!
//! This models the effect of regime change (recent drift diverging from
//! long-term baseline) on the manifold's self-energy. Higher CDC →
//! larger perturbation → eigenstate mixing → faster transition away
//! from the current topology.

use nalgebra::DMatrix;

/// The effective Hamiltonian of a liquidity manifold network.
///
/// Constructed as a real symmetric matrix so that eigenvalues are
/// guaranteed real and the eigendecomposition is numerically stable
/// (no complex arithmetic needed — the Hamiltonian of a conservative
/// system is Hermitian, and for real-valued observables it reduces
/// to symmetric).
#[derive(Debug, Clone)]
pub struct EffectiveHamiltonian {
    /// The N×N symmetric Hamiltonian matrix.
    matrix: DMatrix<f64>,
    /// Number of manifolds (dimension of the Hilbert space).
    dimension: usize,
    /// CDC perturbation strength applied to the diagonal.
    cdc_perturbation: f64,
}

/// Error conditions for Hamiltonian construction.
#[derive(Debug, Clone, thiserror::Error)]
pub enum HamiltonianError {
    #[error("Hamiltonian dimension must be ≥ 2, got {0}")]
    InsufficientDimension(usize),

    #[error("self_energies length ({0}) ≠ dimension ({1})")]
    SelfEnergyDimensionMismatch(usize, usize),

    #[error("couplings length ({0}) ≠ expected n*(n-1)/2 ({1})")]
    CouplingCountMismatch(usize, usize),

    #[error("non-finite value detected in Hamiltonian element")]
    NonFiniteElement,
}

impl EffectiveHamiltonian {
    /// Construct the Hamiltonian from self-energies (diagonal) and
    /// inter-manifold couplings (upper triangle, row-major).
    ///
    /// # Arguments
    ///
    /// * `self_energies` — N diagonal elements H_ii. Each represents the
    ///   self-energy of manifold i: typically `depth_i × σ_i` (TVL × vol).
    /// * `couplings` — N*(N-1)/2 off-diagonal elements in row-major upper
    ///   triangle order: (0,1), (0,2), ..., (0,N-1), (1,2), ..., (N-2,N-1).
    ///   Each represents the coupling strength between two manifolds.
    /// * `cdc_perturbation` — scalar α multiplied by the CDC value and
    ///   added to every diagonal element. Pass `0.0` for the unperturbed
    ///   Hamiltonian.
    ///
    /// The matrix is symmetrised: H_ij = H_ji = coupling[(i,j)].
    pub fn new(
        self_energies: &[f64],
        couplings: &[f64],
        cdc_perturbation: f64,
    ) -> Result<Self, HamiltonianError> {
        let n = self_energies.len();
        if n < 2 {
            return Err(HamiltonianError::InsufficientDimension(n));
        }

        let expected_couplings = n * (n - 1) / 2;
        if couplings.len() != expected_couplings {
            return Err(HamiltonianError::CouplingCountMismatch(
                couplings.len(),
                expected_couplings,
            ));
        }

        // Validate all inputs are finite.
        for &e in self_energies {
            if !e.is_finite() {
                return Err(HamiltonianError::NonFiniteElement);
            }
        }
        for &c in couplings {
            if !c.is_finite() {
                return Err(HamiltonianError::NonFiniteElement);
            }
        }
        if !cdc_perturbation.is_finite() {
            return Err(HamiltonianError::NonFiniteElement);
        }

        let mut matrix = DMatrix::zeros(n, n);

        // Diagonal: self-energy + CDC perturbation.
        for i in 0..n {
            matrix[(i, i)] = self_energies[i] + cdc_perturbation;
        }

        // Off-diagonal: symmetric couplings.
        let mut idx = 0;
        for i in 0..n {
            for j in (i + 1)..n {
                matrix[(i, j)] = couplings[idx];
                matrix[(j, i)] = couplings[idx];
                idx += 1;
            }
        }

        Ok(Self {
            matrix,
            dimension: n,
            cdc_perturbation,
        })
    }

    /// Read-only access to the underlying matrix.
    pub fn matrix(&self) -> &DMatrix<f64> {
        &self.matrix
    }

    /// Dimension of the Hilbert space (number of manifolds).
    pub fn dimension(&self) -> usize {
        self.dimension
    }

    /// The CDC perturbation strength that was applied.
    pub fn cdc_perturbation(&self) -> f64 {
        self.cdc_perturbation
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_dimension_below_two() {
        let r = EffectiveHamiltonian::new(&[1.0], &[], 0.0);
        assert!(matches!(r, Err(HamiltonianError::InsufficientDimension(1))));
    }

    #[test]
    fn rejects_coupling_count_mismatch() {
        // 3 manifolds → need 3 couplings, provide 2.
        let r = EffectiveHamiltonian::new(&[1.0, 2.0, 3.0], &[0.1, 0.2], 0.0);
        assert!(matches!(r, Err(HamiltonianError::CouplingCountMismatch(2, 3))));
    }

    #[test]
    fn rejects_non_finite_self_energy() {
        let r = EffectiveHamiltonian::new(&[1.0, f64::NAN], &[0.1], 0.0);
        assert!(matches!(r, Err(HamiltonianError::NonFiniteElement)));
    }

    #[test]
    fn rejects_non_finite_coupling() {
        let r = EffectiveHamiltonian::new(&[1.0, 2.0], &[f64::INFINITY], 0.0);
        assert!(matches!(r, Err(HamiltonianError::NonFiniteElement)));
    }

    #[test]
    fn constructs_symmetric_2x2() {
        let h = EffectiveHamiltonian::new(&[1.0, 2.0], &[0.5], 0.0).unwrap();
        let m = h.matrix();
        assert_eq!(m[(0, 0)], 1.0);
        assert_eq!(m[(1, 1)], 2.0);
        assert_eq!(m[(0, 1)], 0.5);
        assert_eq!(m[(1, 0)], 0.5); // symmetric
    }

    #[test]
    fn cdc_perturbation_shifts_diagonal() {
        let h = EffectiveHamiltonian::new(&[1.0, 2.0], &[0.5], 0.1).unwrap();
        let m = h.matrix();
        assert!((m[(0, 0)] - 1.1).abs() < 1e-15);
        assert!((m[(1, 1)] - 2.1).abs() < 1e-15);
        assert!((m[(0, 1)] - 0.5).abs() < 1e-15); // off-diagonal unchanged
    }

    #[test]
    fn constructs_3x3_with_correct_coupling_order() {
        // couplings: (0,1)=0.1, (0,2)=0.2, (1,2)=0.3
        let h = EffectiveHamiltonian::new(&[1.0, 2.0, 3.0], &[0.1, 0.2, 0.3], 0.0).unwrap();
        let m = h.matrix();
        assert_eq!(m[(0, 1)], 0.1);
        assert_eq!(m[(0, 2)], 0.2);
        assert_eq!(m[(1, 2)], 0.3);
        // Symmetric:
        assert_eq!(m[(1, 0)], 0.1);
        assert_eq!(m[(2, 0)], 0.2);
        assert_eq!(m[(2, 1)], 0.3);
    }
}
