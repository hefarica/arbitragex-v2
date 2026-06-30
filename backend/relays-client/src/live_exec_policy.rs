//! M1 (2026-06-28): default-deny + testnet-only live-execution barrier.
//!
//! `relays-client` is the ONLY binary in the system that can sign and broadcast
//! transactions. Unlike `searcher-rs` — which hard-panics at boot if ANY
//! capital-bearing key is present (observer-only in ALL modes) — `relays-client`
//! was gated only by SOFT flags (`paper_mode` default-true, `FLASHBOTS_SIGNER_KEY`
//! presence, `ARBX_SIMULATOR_V2_READY`). That meant a single env mistake — signer
//! key set + `arbx:papermode=false` + `ARBX_SIMULATOR_V2_READY=true` — could
//! broadcast REAL mainnet transactions.
//!
//! This module makes that accident impossible:
//!   - DEFAULT-DENY: live execution requires `ARBX_LIVE_EXEC_ENABLED=true`.
//!   - MAINNET PHYSICALLY REFUSED: `chain_id == 1` is rejected UNCONDITIONALLY in
//!     the current testnet-first phase, even if it appears in the allowlist.
//!   - TESTNET-ONLY: only chains in `ARBX_LIVE_EXEC_CHAINS` (default: Sepolia
//!     `11155111`) may be broadcast to.
//!   - FAIL-CLOSED: unset/unparseable config falls back to the safe default.
//!
//! The policy is asserted at boot (fail-fast) AND on every `build_and_sign` call
//! (the un-bypassable runtime barrier — which also catches `paper_mode` being
//! flipped to false AFTER boot). This is a SAFETY barrier, NOT a live activation:
//! with defaults it denies everything.

/// Ethereum mainnet — broadcast here is physically refused in this phase.
pub const MAINNET_CHAIN_ID: u64 = 1;

/// Chains permitted for live execution in the testnet-first phase (Sepolia).
pub const DEFAULT_LIVE_CHAINS: &[u64] = &[11_155_111];

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum LiveExecDenied {
    #[error("live execution is disabled (ARBX_LIVE_EXEC_ENABLED != 'true') — default-deny")]
    NotEnabled,
    #[error(
        "mainnet (chain_id=1) live broadcast is physically refused in the testnet-first phase"
    )]
    MainnetRefused,
    #[error("chain_id {got} is not in the live-execution allowlist {allowed:?}")]
    ChainNotAllowed { got: u64, allowed: Vec<u64> },
}

#[derive(Debug, Clone)]
pub struct LiveExecPolicy {
    pub enabled: bool,
    pub allowed_chains: Vec<u64>,
}

impl LiveExecPolicy {
    /// Resolve from process env, fail-closed to the safe default
    /// (disabled, Sepolia-only).
    pub fn from_env() -> Self {
        Self::from_raw(
            std::env::var("ARBX_LIVE_EXEC_ENABLED").ok().as_deref(),
            std::env::var("ARBX_LIVE_EXEC_CHAINS").ok().as_deref(),
        )
    }

    /// Pure constructor (testable without touching process env).
    /// `enabled` is true ONLY for the exact string "true" — any other value
    /// (missing, "false", "1", "TRUE", typo) leaves the barrier armed.
    pub fn from_raw(enabled: Option<&str>, chains: Option<&str>) -> Self {
        let enabled = matches!(enabled, Some("true"));
        let allowed_chains = chains
            .map(|s| {
                s.split(',')
                    .filter_map(|x| x.trim().parse::<u64>().ok())
                    .collect::<Vec<_>>()
            })
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| DEFAULT_LIVE_CHAINS.to_vec());
        Self {
            enabled,
            allowed_chains,
        }
    }

    /// The single physical barrier every broadcast path MUST pass. Fail-closed.
    pub fn assert_broadcast_allowed(&self, chain_id: u64) -> Result<(), LiveExecDenied> {
        if !self.enabled {
            return Err(LiveExecDenied::NotEnabled);
        }
        // Mainnet is refused UNCONDITIONALLY — even if 1 is in the allowlist
        // (belt-and-suspenders against a misconfigured ARBX_LIVE_EXEC_CHAINS).
        if chain_id == MAINNET_CHAIN_ID {
            return Err(LiveExecDenied::MainnetRefused);
        }
        if !self.allowed_chains.contains(&chain_id) {
            return Err(LiveExecDenied::ChainNotAllowed {
                got: chain_id,
                allowed: self.allowed_chains.clone(),
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SEPOLIA: u64 = 11_155_111;

    #[test]
    fn default_is_deny_everything() {
        // No env → disabled → every chain denied (including the testnet).
        let p = LiveExecPolicy::from_raw(None, None);
        assert!(!p.enabled);
        assert_eq!(
            p.assert_broadcast_allowed(SEPOLIA),
            Err(LiveExecDenied::NotEnabled)
        );
        assert_eq!(
            p.assert_broadcast_allowed(MAINNET_CHAIN_ID),
            Err(LiveExecDenied::NotEnabled)
        );
    }

    #[test]
    fn enabled_allows_sepolia_only() {
        let p = LiveExecPolicy::from_raw(Some("true"), None); // default chains = Sepolia
        assert!(p.assert_broadcast_allowed(SEPOLIA).is_ok());
        assert_eq!(
            p.assert_broadcast_allowed(MAINNET_CHAIN_ID),
            Err(LiveExecDenied::MainnetRefused)
        );
        assert_eq!(
            p.assert_broadcast_allowed(137),
            Err(LiveExecDenied::ChainNotAllowed {
                got: 137,
                allowed: vec![SEPOLIA],
            })
        );
    }

    #[test]
    fn mainnet_refused_even_if_explicitly_allowlisted() {
        // Dangerous misconfig: operator enables live AND lists mainnet.
        let p = LiveExecPolicy::from_raw(Some("true"), Some("1,11155111"));
        assert_eq!(
            p.assert_broadcast_allowed(MAINNET_CHAIN_ID),
            Err(LiveExecDenied::MainnetRefused)
        );
        // ...but the testnet in the same list still works.
        assert!(p.assert_broadcast_allowed(SEPOLIA).is_ok());
    }

    #[test]
    fn dangerous_env_triple_cannot_reach_mainnet() {
        // Regression for the exact safety gap: even with live enabled and mainnet
        // explicitly requested, the last physical barrier refuses mainnet.
        let p = LiveExecPolicy::from_raw(Some("true"), Some("1"));
        assert_eq!(
            p.assert_broadcast_allowed(1),
            Err(LiveExecDenied::MainnetRefused)
        );
    }

    #[test]
    fn non_true_values_are_disabled() {
        for v in ["", "false", "1", "TRUE", "yes", "True", " true "] {
            assert!(
                !LiveExecPolicy::from_raw(Some(v), None).enabled,
                "value {v:?} must NOT enable live execution"
            );
        }
    }

    #[test]
    fn unparseable_chains_fall_back_to_default() {
        let p = LiveExecPolicy::from_raw(Some("true"), Some("garbage,,"));
        assert_eq!(p.allowed_chains, DEFAULT_LIVE_CHAINS.to_vec());
    }
}
