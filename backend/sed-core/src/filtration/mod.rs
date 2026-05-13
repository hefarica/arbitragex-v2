//! `filtration` — stochastic wave filtration (spec §3.1).
//!
//! **Phase 1 status**: stub. The PDMP filtration kernels (Markov jump
//! process, Poisson random measure, CDC calculator) land in Phase 2 with
//! their own `nalgebra` + `rand_distr` dependencies and dedicated test
//! suites. The module exists in the public tree so the spec's `mod`
//! declaration in `lib.rs` resolves and so future commits land changes
//! against a stable path.
