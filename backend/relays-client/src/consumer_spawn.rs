//! Consumer spawn policy — paper path must not require a capital signer.
//!
//! ## Why this exists
//!
//! Historically `relays-client` only spawned the `arbx:opps:simulated` consumer
//! when `signer + rpc_pool + db` were all present. On a paper-only VPS
//! (`FLASHBOTS_SIGNER_KEY` unset) that skipped the consumer entirely, so
//! simulated opportunities never reached `paper_trade_runs` and the paper path
//! went stale while `arbx:opps:detected` kept filling.
//!
//! Paper mode records runs via `SubmitEngine` without signing or broadcasting
//! (checklist `PaperModeActive` → `insert_paper_trade_run`). Live / broadcast
//! still fail-closed inside the engine when no signer is configured.
//!
//! ## Rule
//!
//! ```text
//! spawn ⇔ has_db ∧ has_rpc_pool ∧ (has_signer ∨ paper_mode)
//! ```

/// Returns true when the Redis Streams consumer on `arbx:opps:simulated`
/// should be spawned at boot.
///
/// * `has_db` — Postgres pool available (required for paper_trade_runs + persist).
/// * `has_rpc_pool` — HTTP RPC pool configured (kept for parity with live path
///   boot wiring; paper short-circuit does not call RPC).
/// * `has_signer` — `FLASHBOTS_SIGNER_KEY` loaded.
/// * `paper_mode` — paper/shadow armed (env and/or Redis papermode at boot).
pub fn should_spawn_consumer(
    has_db: bool,
    has_rpc_pool: bool,
    has_signer: bool,
    paper_mode: bool,
) -> bool {
    if !has_db || !has_rpc_pool {
        return false;
    }
    has_signer || paper_mode
}

/// Whether `/execute` may invoke the engine without a loaded signer.
///
/// Paper mode is allowed (engine short-circuits to paper_trade_runs).
/// Live without signer stays 501 / not-submitted at the handler or engine.
pub fn allow_execute_without_signer(paper_mode: bool) -> bool {
    paper_mode
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn live_requires_signer_db_rpc() {
        assert!(should_spawn_consumer(true, true, true, false));
        assert!(!should_spawn_consumer(true, true, false, false));
        assert!(!should_spawn_consumer(false, true, true, false));
        assert!(!should_spawn_consumer(true, false, true, false));
    }

    #[test]
    fn paper_spawns_without_signer_when_db_and_rpc_present() {
        assert!(should_spawn_consumer(true, true, false, true));
        assert!(should_spawn_consumer(true, true, true, true));
    }

    #[test]
    fn paper_still_needs_db_and_rpc() {
        assert!(!should_spawn_consumer(false, true, false, true));
        assert!(!should_spawn_consumer(true, false, false, true));
        assert!(!should_spawn_consumer(false, false, false, true));
    }

    #[test]
    fn execute_without_signer_only_in_paper() {
        assert!(allow_execute_without_signer(true));
        assert!(!allow_execute_without_signer(false));
    }
}
