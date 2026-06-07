//! MEV Ethics Gate — integration test suite.
//!
//! Exercises the CANONICAL gate (`searcher_rs::mev_gate::apply_mev_gate`) — the
//! single source of truth in `src/mev_gate.rs`. There is NO local mock here: the
//! test imports the real decision tree so a doctrine change in one place cannot
//! drift from the other. (The previous version re-implemented the gate inline and
//! rotted into a non-compiling botched merge — see arbx-mev-ethics-gate / commit
//! 9317c07. Fixed forward 2026-06-07 to import the real API.)
//!
//! Gate decision tree (src/mev_gate.rs):
//!   #1 depends_on_specific_pending_tx == true  -> PROHIBITED (sandwich/frontrun/JIT)
//!   #2 worsens_user_outcome          == true   -> PROHIBITED (value extraction)
//!   #3 protocol_explicitly_permits   == true   -> PERMITTED  (liquidation/MEV-Share)
//!      else                                     -> PROHIBITED (gray zone default)

use searcher_rs::mev_gate::{apply_mev_gate, GateVerdict};

const REASON_PENDING_TX: &str =
    "Strategy depends on specific pending user transaction (sandwich/frontrun/JIT)";
const REASON_WORSENS_USER: &str = "Strategy extracts value from user outcome";
const REASON_NO_CONSENT: &str = "Strategy not authorized by protocol";
const REASON_PERMITTED: &str =
    "Protocol explicitly permits this mechanism (liquidation bonus / MEV-Share)";

/// Decision #1 short-circuits: any strategy that depends on a SPECIFIC pending user
/// tx is PROHIBITED. JIT-V3 (mint tight LP ahead of an observed swap) is the canonical
/// case — it reads the mempool to target one user's transaction.
#[test]
fn jit_v3_prohibited_depends_on_pending_tx() {
    let verdict = apply_mev_gate(/*depends*/ true, /*worsens*/ false, /*permits*/ false);
    assert_eq!(
        verdict,
        GateVerdict::Prohibited { reason: REASON_PENDING_TX.to_string() },
    );
}

/// Sandwich targets a specific user swap → fails Decision #1 first (before #2),
/// regardless of the (also-true) user-harm flag.
#[test]
fn sandwich_prohibited_depends_on_pending_tx() {
    let verdict = apply_mev_gate(/*depends*/ true, /*worsens*/ true, /*permits*/ false);
    assert_eq!(
        verdict,
        GateVerdict::Prohibited { reason: REASON_PENDING_TX.to_string() },
    );
}

/// Frontrunning reads a pending user tx and orders before it → Decision #1.
#[test]
fn frontrun_prohibited_depends_on_pending_tx() {
    let verdict = apply_mev_gate(/*depends*/ true, /*worsens*/ true, /*permits*/ true);
    assert_eq!(
        verdict,
        GateVerdict::Prohibited { reason: REASON_PENDING_TX.to_string() },
    );
}

/// Decision #2: a strategy that is NOT mempool-specific but still worsens the user's
/// outcome is PROHIBITED — even if some protocol would "permit" it, value extraction
/// from a user fails before the consent check.
#[test]
fn worsens_user_outcome_prohibited_before_consent() {
    let verdict = apply_mev_gate(/*depends*/ false, /*worsens*/ true, /*permits*/ true);
    assert_eq!(
        verdict,
        GateVerdict::Prohibited { reason: REASON_WORSENS_USER.to_string() },
    );
}

/// Decision #3 (NO branch): cross-DEX arbitrage is mempool-independent and harms no
/// specific user, but absent explicit protocol consent it defaults to PROHIBITED.
#[test]
fn cross_dex_arb_gray_zone_prohibited_without_consent() {
    let verdict = apply_mev_gate(/*depends*/ false, /*worsens*/ false, /*permits*/ false);
    assert_eq!(
        verdict,
        GateVerdict::Prohibited { reason: REASON_NO_CONSENT.to_string() },
    );
}

/// Decision #3 (YES branch): a residual backrun / liquidation on a protocol that
/// explicitly permits the mechanism (Aave liquidation bonus, MEV-Share) is PERMITTED.
#[test]
fn liquidation_backrun_permitted_with_protocol_consent() {
    let verdict = apply_mev_gate(/*depends*/ false, /*worsens*/ false, /*permits*/ true);
    assert_eq!(
        verdict,
        GateVerdict::Permitted { reason: REASON_PERMITTED.to_string() },
    );
}
