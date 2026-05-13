//! `filtration` — stochastic wave filtration (spec §3.1, Phase 2).
//!
//! ## Status
//!
//! - **Default profile**: empty stub. The math kernels are gated behind
//!   the `filtration` Cargo feature so downstream crates that only
//!   consume the V1.1/V1.2 typestate (no math) stay lean.
//! - **`--features filtration`** (Phase 2, 2026-05-13): two submodules
//!   compile and become accessible at this path:
//!
//!   - [`markov_jump`] — `PdmpEstimator` for online recursive
//!     estimation of `{μ, σ, λ}` of a piecewise-deterministic Markov
//!     jump-diffusion process (Cont-Tankov SDE). Welford's algorithm
//!     for mean + variance; z-score classifier for jumps.
//!   - [`cdc_calculator`] — `CdcCalculator` + `StateDivergenceCoefficient`
//!     for the half-life dual of the V1.2 holonomic invariant
//!     (`ANEXOS_V1.2.md` Bloque 3 §invariant 5).
//!
//! ## Deferred to Phase 2.1+
//!
//! - **Synthetic-data backtesting harness** that uses `rand_distr` to
//!   generate known jump-diffusion processes and asserts the estimator
//!   converges within tolerance. The `rand_distr` dep is already wired
//!   under the `filtration` feature; the harness will live in a future
//!   `filtration::synth` submodule with its own `#[cfg(test)]` surface.
//! - **Multi-asset filtration** — current estimator is univariate.
//!   Phase 2.1 extends to a `DMatrix<f64>` correlation-aware variant
//!   for the cross-venue hedge path.
//! - **Poisson random measure** (anexo §3.1 reference) — required for
//!   the full stochastic optimal control problem in `allocator`; lands
//!   when Phase 4 unblocks.

#[cfg(feature = "filtration")]
pub mod cdc_calculator;

#[cfg(feature = "filtration")]
pub mod markov_jump;

#[cfg(feature = "filtration")]
pub use cdc_calculator::{CdcCalculator, StateDivergenceCoefficient};

#[cfg(feature = "filtration")]
pub use markov_jump::{PdmpEstimator, ProcessStats};
