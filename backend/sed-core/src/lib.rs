//! Sequential Equilibrium Dispatcher (SED) — quantitative orchestration layer.
//!
//! This crate is the mathematical bone structure that sits above the existing
//! `searcher-rs` (mempool ingestion) and `relays-client` (bundle submission)
//! layers. Its purpose is to enforce the *post-resolution topology* of every
//! bundle the platform produces via Rust's type system, so that an
//! unauthorised bundle position is unrepresentable at compile time.
//!
//! ## Doctrine alignment (mev-ethics.md @ c381d5c)
//!
//! The crate's [`types::bundle_position`] module ships a typestate built on
//! [`std::marker::PhantomData`] and a [sealed trait](sealed-pattern). Only the
//! variants explicitly listed below implement
//! [`types::bundle_position::PostResolutionTopology`]:
//!
//! - [`OrthogonalEquilibrium`](types::bundle_position::OrthogonalEquilibrium)
//!   — CEX cross-venue hedge / null-covariance state.
//! - [`DiracImpulseOnly`](types::bundle_position::DiracImpulseOnly) —
//!   JIT-style liquidity impulse on the CPMM manifold.
//!
//! `Sandwich`, `Frontrun`, and "victim-specific bundle attribution" variants
//! are intentionally absent and cannot be added by external crates because the
//! `PostResolutionTopology` trait is sealed via [`types::bundle_position::private::Sealed`].
//!
//! ## Phase 1 scaffold (this commit)
//!
//! - [`types::bundle_position`] — typestate compile-time verifier.
//! - [`types::kill_switch`] — [`KillSwitchGate`](types::kill_switch::KillSwitchGate)
//!   that consults the shared kill-switch before every dispatch.
//! - [`types::infrastructure`] — [`InfrastructurePrerequisite`](types::infrastructure::InfrastructurePrerequisite)
//!   + 501-shaped [`NotImplementedResponse`](types::infrastructure::NotImplementedResponse)
//!   honoring the repo's existing fail-honest discipline.
//! - [`types::errors`] — explicit error enums (`TopologyValidationError`,
//!   `DispatchError`, `InfrastructureError`).
//!
//! ## Deferred to Phase 2+
//!
//! [`filtration`], [`eigenstate`], [`allocator`], [`hedger`] currently expose
//! stub modules. The mathematical bodies (PDMP filtration, Lanczos eigenstate
//! solver, Pontryagin OCP, orthogonal hyperplane hedge) land in dedicated
//! sprints with their own deps and tests.
//!
//! ## Why fail-loud on todo!() bodies
//!
//! Phase 1-2 callers that reach a stub path will panic via `todo!()`. This is
//! the repo's R8 fail-honest stance: panic loudly when an unimplemented path
//! is exercised rather than silently returning a fabricated value. The
//! `cargo check -p sed-core` gate validates the type structure; runtime
//! exercise of a `todo!()` path is a programming error we want surfaced.

#![deny(clippy::unwrap_used, clippy::expect_used)]

pub mod allocator;
pub mod eigenstate;
pub mod filtration;
pub mod hedger;
pub mod metrics;
pub mod persistence;
pub mod prelude;
pub mod types;

// Paper-Shadow E2E pipeline integration test (test-only).
#[cfg(test)]
mod pipeline_e2e;

// Paper-Shadow Mode connectors — real data ingestion (feature-gated).
#[cfg(feature = "paper-shadow")]
pub mod connectors;
