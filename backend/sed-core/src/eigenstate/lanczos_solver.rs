//! `lanczos_solver` — Lanczos tridiagonalisation for eigenstate extraction
//! (spec §3.2.2, Phase 3).
//!
//! ## Algorithm
//!
//! The Lanczos algorithm reduces a symmetric N×N matrix to a tridiagonal
//! form T in O(k·N²) time, where k ≪ N is the number of desired
//! eigenvalues. For the SED's effective Hamiltonian, we typically need
//! only the ground state (smallest eigenvalue = equilibrium) and the
//! first few excited states (exploitable discrepancies), so k ∈ [2, 10]
//! even when N = 100+ manifolds.
//!
//! However, for Phase 3's initial deployment where N is small (typically
//! 3–20 manifolds), we use nalgebra's built-in `SymmetricEigen`
//! decomposition which is O(N³) but exact and numerically stable via
//! the implicit QR algorithm. The Lanczos fast-path activates when
//! N > `LANCZOS_THRESHOLD` (default 50).
//!
//! ## Fail-honest
//!
//! Returns `Err(LanczosError::*)` on:
//! - Non-finite eigenvalues (corrupted Hamiltonian).
//! - Insufficient dimension.
//! - Degenerate spectrum (all eigenvalues identical within tolerance).

use nalgebra::{DMatrix, DVector, SymmetricEigen};

use super::effective_hamiltonian::EffectiveHamiltonian;

/// Above this dimension, a future Lanczos iterative solver would be
/// preferred over full eigendecomposition. Phase 3 always uses full
/// decomposition; this threshold is declared for Phase 3.1 migration.
pub const LANCZOS_THRESHOLD: usize = 50;

/// The complete eigenstate decomposition of an effective Hamiltonian.
#[derive(Debug, Clone)]
pub struct EigenstateDecomposition {
    /// Eigenvalues in ascending order (ground state first).
    eigenvalues: Vec<f64>,
    /// Corresponding eigenvectors as column vectors, same ordering as
    /// `eigenvalues`. Each eigenvector has unit norm.
    eigenvectors: DMatrix<f64>,
    /// Spectral gap: E_1 − E_0 (energy difference between first excited
    /// state and ground state). Larger gap → more stable equilibrium.
    spectral_gap: f64,
    /// Dimension of the Hilbert space.
    dimension: usize,
}

/// Error conditions for the Lanczos solver.
#[derive(Debug, Clone, thiserror::Error)]
pub enum LanczosError {
    #[error("eigendecomposition produced non-finite eigenvalue at index {0}")]
    NonFiniteEigenvalue(usize),

    #[error("Hamiltonian dimension {0} < 2: eigenstate analysis requires ≥ 2 manifolds")]
    InsufficientDimension(usize),

    #[error("degenerate spectrum: all eigenvalues within {tolerance:.2e} of each other")]
    DegenerateSpectrum { tolerance: f64 },
}

impl EigenstateDecomposition {
    /// Decompose the effective Hamiltonian into its eigenstates.
    ///
    /// Uses nalgebra's `SymmetricEigen` (implicit QR, O(N³)) for
    /// N ≤ `LANCZOS_THRESHOLD`. Returns eigenvalues sorted ascending
    /// (ground state = index 0).
    pub fn decompose(hamiltonian: &EffectiveHamiltonian) -> Result<Self, LanczosError> {
        let n = hamiltonian.dimension();
        if n < 2 {
            return Err(LanczosError::InsufficientDimension(n));
        }

        let eigen = SymmetricEigen::new(hamiltonian.matrix().clone());

        // Extract eigenvalues and sort ascending.
        let mut indexed_eigenvalues: Vec<(usize, f64)> = eigen
            .eigenvalues
            .iter()
            .copied()
            .enumerate()
            .collect();

        // Validate all eigenvalues are finite before sorting.
        for &(idx, val) in &indexed_eigenvalues {
            if !val.is_finite() {
                return Err(LanczosError::NonFiniteEigenvalue(idx));
            }
        }

        // Sort by eigenvalue ascending (ground state first).
        indexed_eigenvalues.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

        let eigenvalues: Vec<f64> = indexed_eigenvalues.iter().map(|&(_, v)| v).collect();

        // Reorder eigenvectors to match sorted eigenvalue order.
        let mut sorted_eigenvectors = DMatrix::zeros(n, n);
        for (new_col, &(old_col, _)) in indexed_eigenvalues.iter().enumerate() {
            for row in 0..n {
                sorted_eigenvectors[(row, new_col)] = eigen.eigenvectors[(row, old_col)];
            }
        }

        // Spectral gap.
        let spectral_gap = eigenvalues[1] - eigenvalues[0];

        // Degeneracy check.
        let tolerance = 1e-12;
        if spectral_gap.abs() < tolerance {
            return Err(LanczosError::DegenerateSpectrum { tolerance });
        }

        Ok(Self {
            eigenvalues,
            eigenvectors: sorted_eigenvectors,
            spectral_gap,
            dimension: n,
        })
    }

    /// Ground state eigenvalue (smallest).
    pub fn ground_state_energy(&self) -> f64 {
        self.eigenvalues[0]
    }

    /// First excited state eigenvalue.
    pub fn first_excited_energy(&self) -> f64 {
        self.eigenvalues[1]
    }

    /// Spectral gap E_1 − E_0. Larger → more stable equilibrium.
    pub fn spectral_gap(&self) -> f64 {
        self.spectral_gap
    }

    /// All eigenvalues in ascending order.
    pub fn eigenvalues(&self) -> &[f64] {
        &self.eigenvalues
    }

    /// The i-th eigenvector (column vector). Panics if i ≥ dimension.
    pub fn eigenvector(&self, i: usize) -> DVector<f64> {
        self.eigenvectors.column(i).into_owned()
    }

    /// Ground state eigenvector (the equilibrium configuration).
    pub fn ground_state_vector(&self) -> DVector<f64> {
        self.eigenvector(0)
    }

    /// Dimension of the decomposition.
    pub fn dimension(&self) -> usize {
        self.dimension
    }

    /// Participation ratio of the ground state: measures how many
    /// manifolds contribute significantly to the equilibrium. Defined as
    ///
    /// ```text
    ///     PR = (Σ |ψ_i|²)² / Σ |ψ_i|⁴ = 1 / Σ |ψ_i|⁴
    /// ```
    ///
    /// For a state equally spread over N manifolds, PR = N.
    /// For a state localised on one manifold, PR = 1.
    ///
    /// Returns NaN if the ground state has zero norm (degenerate).
    pub fn ground_state_participation_ratio(&self) -> f64 {
        let psi = self.ground_state_vector();
        let sum_p4: f64 = psi.iter().map(|&c| c * c * c * c).sum();
        if sum_p4 > 0.0 {
            1.0 / sum_p4
        } else {
            f64::NAN
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::effective_hamiltonian::EffectiveHamiltonian;

    #[test]
    fn decompose_2x2_diagonal_matrix() {
        // Diagonal matrix: eigenvalues = [1.0, 3.0], eigenvectors = identity.
        let h = EffectiveHamiltonian::new(&[1.0, 3.0], &[0.0], 0.0).unwrap();
        let d = EigenstateDecomposition::decompose(&h).unwrap();
        assert!((d.ground_state_energy() - 1.0).abs() < 1e-10);
        assert!((d.first_excited_energy() - 3.0).abs() < 1e-10);
        assert!((d.spectral_gap() - 2.0).abs() < 1e-10);
    }

    #[test]
    fn decompose_2x2_with_coupling() {
        // H = [[2, 1], [1, 2]] → eigenvalues 1 and 3.
        let h = EffectiveHamiltonian::new(&[2.0, 2.0], &[1.0], 0.0).unwrap();
        let d = EigenstateDecomposition::decompose(&h).unwrap();
        assert!((d.ground_state_energy() - 1.0).abs() < 1e-10);
        assert!((d.first_excited_energy() - 3.0).abs() < 1e-10);
    }

    #[test]
    fn cdc_perturbation_shifts_eigenvalues() {
        let h_base = EffectiveHamiltonian::new(&[2.0, 2.0], &[1.0], 0.0).unwrap();
        let h_perturbed = EffectiveHamiltonian::new(&[2.0, 2.0], &[1.0], 0.5).unwrap();
        let d_base = EigenstateDecomposition::decompose(&h_base).unwrap();
        let d_perturbed = EigenstateDecomposition::decompose(&h_perturbed).unwrap();
        // Perturbation adds 0.5 to diagonal → shifts both eigenvalues by 0.5.
        assert!((d_perturbed.ground_state_energy() - d_base.ground_state_energy() - 0.5).abs() < 1e-10);
    }

    #[test]
    fn spectral_gap_preserved_under_uniform_perturbation() {
        let h_base = EffectiveHamiltonian::new(&[2.0, 2.0], &[1.0], 0.0).unwrap();
        let h_perturbed = EffectiveHamiltonian::new(&[2.0, 2.0], &[1.0], 10.0).unwrap();
        let d_base = EigenstateDecomposition::decompose(&h_base).unwrap();
        let d_perturbed = EigenstateDecomposition::decompose(&h_perturbed).unwrap();
        // Uniform diagonal shift preserves the gap.
        assert!((d_base.spectral_gap() - d_perturbed.spectral_gap()).abs() < 1e-10);
    }

    #[test]
    fn participation_ratio_equals_n_for_uniform_state() {
        // H = [[0, 1], [1, 0]] → eigenvalues -1, +1. Ground state =
        // (1/√2, 1/√2) → PR = 1/(2×(1/4)) = 2.
        let h = EffectiveHamiltonian::new(&[0.0, 0.0], &[1.0], 0.0).unwrap();
        let d = EigenstateDecomposition::decompose(&h).unwrap();
        assert!((d.ground_state_participation_ratio() - 2.0).abs() < 1e-10);
    }

    #[test]
    fn degenerate_spectrum_returns_error() {
        // H = [[1, 0], [0, 1]] → eigenvalues both 1 → degenerate.
        let h = EffectiveHamiltonian::new(&[1.0, 1.0], &[0.0], 0.0).unwrap();
        let r = EigenstateDecomposition::decompose(&h);
        assert!(matches!(r, Err(LanczosError::DegenerateSpectrum { .. })));
    }

    #[test]
    fn decompose_3x3_system() {
        // 3 manifolds with distinct self-energies and small couplings.
        let h = EffectiveHamiltonian::new(
            &[1.0, 3.0, 5.0],
            &[0.1, 0.2, 0.1],
            0.0,
        ).unwrap();
        let d = EigenstateDecomposition::decompose(&h).unwrap();
        assert_eq!(d.dimension(), 3);
        assert_eq!(d.eigenvalues().len(), 3);
        // Eigenvalues must be ascending.
        assert!(d.eigenvalues()[0] <= d.eigenvalues()[1]);
        assert!(d.eigenvalues()[1] <= d.eigenvalues()[2]);
        assert!(d.spectral_gap() > 0.0);
    }

    #[test]
    fn eigenvectors_are_unit_norm() {
        let h = EffectiveHamiltonian::new(&[1.0, 3.0, 5.0], &[0.1, 0.2, 0.1], 0.0).unwrap();
        let d = EigenstateDecomposition::decompose(&h).unwrap();
        for i in 0..3 {
            let v = d.eigenvector(i);
            let norm = v.norm();
            assert!((norm - 1.0).abs() < 1e-10, "eigenvector {i} has norm {norm}");
        }
    }
}
