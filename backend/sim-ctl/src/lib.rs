// M11 (audit 2026-05-10): surface panics in hot-path crate.
#![warn(clippy::unwrap_used, clippy::expect_used)]

//! sim-ctl library target.
//!
//! Exists so the GET /capabilities handler (G-SIM-1 FASE 1) is unit-testable
//! under CI's `cargo test --workspace --lib` gate, which runs only lib-target
//! tests — a bin-only crate's inline `#[cfg(test)]` modules would never
//! execute there. The bin (`src/main.rs`) consumes this lib for the route.

pub mod capabilities;

/// SIMWIRE-02c P1-2: drain-guard authorization is PER-BACKEND, never a
/// common `fork || b2c` OR. ANVIL_URL is always set in prod compose, so
/// `fork.is_some()` is ~always true — under the OR, `SIM_BACKEND=revm` with
/// an INCOMPLETE B2c env (missing REVM_RPC_URL / ARBITRAGE_EXECUTOR /
/// REDIS_URL / FLASHLOAN_EXECUTOR_1) would spawn the consumer on the legacy
/// RevmBackend, whose calldata is empty by construction — a structural
/// drain of the validated stream into permanent rejections. Correct rule:
/// REVM selected → the B2c context MUST exist; ANVIL selected → the fork
/// MUST exist. One backend, one authorization.
pub fn backend_available_for(sim_backend: &str, fork_ready: bool, b2c_ready: bool) -> bool {
    match sim_backend {
        "revm" => b2c_ready,
        // "anvil" and "" (the legacy selector's default) authorize on the fork.
        _ => fork_ready,
    }
}

/// The B2c REVM pipeline replays against the REVM_RPC_URL fork — a MAINNET
/// fork — so FLASHLOAN_EXECUTOR_1 is the executor step 0 of
/// `execute_multistep_revm` needs for every stream candidate resolved
/// against that fork. Per-candidate chains are re-checked fail-closed in
/// the consumer (`simulate_b2c` pre-check), so a non-1 chain cannot slip
/// through on a mainnet fork.
pub const B2C_FORK_CHAIN_ID: u64 = 1;

/// SIMWIRE-02c P1-3: FlashLoanExecutor boot readiness — fail-closed via
/// `shared_rs::chains` (Missing / Invalid / Zero all refuse, no hardcoded
/// fallback). Boot withholds the B2c context when this is false, which
/// makes the drain guard refuse the consumer spawn: without an executor
/// every simulation would fail at step 0 with
/// `multistep_flashloan_executor_unresolved`.
pub fn flashloan_executor_boot_ready() -> bool {
    shared_rs::chains::resolve_flashloan_executor_address(B2C_FORK_CHAIN_ID).is_ok()
}

#[cfg(test)]
mod simwire02c_boot_tests {
    use super::{backend_available_for, flashloan_executor_boot_ready, B2C_FORK_CHAIN_ID};

    /// Serialize the env-mutating test (cargo runs lib tests in parallel
    /// threads; only this module touches FLASHLOAN_EXECUTOR_1).
    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn lock_env() -> std::sync::MutexGuard<'static, ()> {
        match ENV_LOCK.lock() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(),
        }
    }

    /// P1-2 matrix. The exact production defect was ("revm", fork=Some,
    /// b2c=None) authorizing under `fork || b2c`; every row below pins the
    /// per-backend rule instead.
    #[test]
    fn drain_guard_authorizes_per_backend_never_a_common_or() {
        // ANVIL present + B2c incomplete MUST refuse (the P1-2 defect row):
        assert!(!backend_available_for("revm", true, false));
        // REVM authorizes on ITS OWN B2c context (anvil down is irrelevant):
        assert!(backend_available_for("revm", false, true));
        assert!(backend_available_for("revm", true, true));
        assert!(!backend_available_for("revm", false, false));
        // ANVIL authorizes on ITS fork, never on a stray B2c context:
        assert!(backend_available_for("anvil", true, false));
        assert!(!backend_available_for("anvil", false, true));
        assert!(!backend_available_for("anvil", false, false));
        // Empty selector = legacy anvil default in the backend selection.
        assert!(backend_available_for("", true, false));
    }

    /// P1-3: FLASHLOAN_EXECUTOR_1 missing / unparsable / zero ⇒ boot NOT
    /// ready ⇒ b2c context withheld ⇒ drain guard refuses. A valid non-zero
    /// address is the only ready state.
    #[test]
    fn flashloan_executor_missing_means_refuse() {
        let _guard = lock_env();
        let key = format!("FLASHLOAN_EXECUTOR_{B2C_FORK_CHAIN_ID}");
        let saved = std::env::var(&key).ok();
        for bad in [
            None,
            Some("not-an-address"),
            Some("0x0000000000000000000000000000000000000000"),
        ] {
            match &bad {
                Some(v) => std::env::set_var(&key, v),
                None => std::env::remove_var(&key),
            }
            assert!(
                !flashloan_executor_boot_ready(),
                "FLASHLOAN_EXECUTOR_1={bad:?} must refuse boot readiness"
            );
        }
        std::env::set_var(&key, "0x1111111111111111111111111111111111111111");
        assert!(flashloan_executor_boot_ready());
        match saved {
            Some(v) => std::env::set_var(&key, v),
            None => std::env::remove_var(&key),
        }
    }

    /// P1-2 × P1-3 composition: the withheld-b2c path IS the refusal —
    /// `backend_available_for("revm", fork=true, b2c=false)` is the exact
    /// state boot reaches when the executor env is missing.
    #[test]
    fn missing_executor_composes_with_drain_guard_refusal() {
        let _guard = lock_env();
        let key = format!("FLASHLOAN_EXECUTOR_{B2C_FORK_CHAIN_ID}");
        let saved = std::env::var(&key).ok();
        std::env::remove_var(&key);
        // Boot behavior under a missing executor: b2c withheld → guard sees
        // b2c_ready=false even with the anvil fork up → refuse.
        assert!(!backend_available_for(
            "revm",
            true,
            flashloan_executor_boot_ready()
        ));
        if let Some(v) = saved {
            std::env::set_var(&key, v);
        }
    }
}
