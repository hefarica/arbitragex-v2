//! sim-core — the shared REAL simulation path.
//!
//! This crate holds the wrapped-flash multi-step REVM orchestrator
//! (`sim_multistep`) and the ERC-20 / OZ-AccessControl storage-override
//! helpers it depends on (`sim_prefund`), extracted VERBATIM from
//! `searcher-rs` (G-SIM-1 PR-B1, Option B).
//!
//! ## Why a dedicated crate
//!
//! `execute_multistep_revm` is the REAL sim the searcher runs before a
//! wrapped-flash broadcast. It was previously a private module of the
//! `searcher-rs` binary crate. Extracting it here lets a second consumer
//! (`sim-ctl`) run the SAME sim — the SAME encoder, the SAME fail-closed
//! guards, the SAME `wrapped_calldata` — instead of growing a second,
//! divergent encoder. `searcher-rs` re-exports these modules so every
//! existing call site (`crate::sim_multistep::*`, `searcher_rs::sim_multistep::*`,
//! `crate::sim_prefund::*`) keeps compiling unchanged.
//!
//! This is a mechanical MOVE: no simulation logic changed. The only edits are
//! module relocation + visibility/import wiring. Observer-only — no signer,
//! no broadcast; the paper-mode / fail-closed invariants documented on the
//! modules below are preserved byte-for-byte.

// Mirror the searcher-rs lib lints so the moved modules keep the SAME
// hot-path-panic surfacing they had in their original home.
#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used))]

// Phase A.3.c.2 — ERC-20 / AccessControl storage-override computation. Pure
// helpers + provider trait + slot computation. Does NOT mutate REVM state; the
// multi-step orchestrator consumes the returned overrides and applies them.
pub mod sim_prefund;

// Phase A.3.c.3 — Multi-step REVM orchestrator (wrapped-flash path). Combines
// sim_prefund's role override with RoundTripContext to build + drive the
// deterministic wrapped-flash plan through REVM (feature `v2-simulator`).
pub mod sim_multistep;

// Phase A.3.a — OpportunityCandidate → RoundTripContext encoder. The SAME
// route-encoding logic searcher-rs uses, extracted here (G-SIM-1 PR-B2a) so a
// second consumer (`sim-ctl`) can build the SAME RoundTripContext instead of a
// divergent encoder. Pure + fail-closed; no REVM dispatch, no signer, no
// broadcast. searcher-rs re-exports this module so every existing
// `crate::sim_encoder::*` / `searcher_rs::sim_encoder::*` call site keeps
// compiling unchanged.
pub mod sim_encoder;

// Monte Carlo Validator — Validación estadística de convergencia para topologías
// de reintentos con DLQ forzado. Modelo: retry como retraso puro del dado de absorción.
pub mod monte_carlo_validator;
