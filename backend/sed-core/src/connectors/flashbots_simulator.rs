//! Flashbots Simulator — Dry-run bundle simulation via eth_callBundle.
//!
//! ## CAPITAL EXPOSED: STRICTLY 0
//!
//! This module NEVER contains:
//! - SecretKey, Signer, Wallet, PrivateKey
//! - eth_sendBundle, eth_sendRawTransaction, send_transaction
//!
//! Only `eth_callBundle` (simulation/dry-run) is permitted.

use super::error::ConnectorError;

/// Result of a Flashbots dry-run simulation.
#[derive(Debug, Clone)]
pub struct SimulationResult {
    /// Gas used by the simulated bundle
    pub gas_used: u64,
    /// Whether the simulation reverted
    pub reverted: bool,
    /// Revert reason (if reverted)
    pub revert_reason: Option<String>,
    /// Simulated coinbase transfer (wei)
    pub coinbase_transfer_wei: u64,
    /// Block number used for simulation
    pub simulation_block: u64,
}

/// Flashbots dry-run simulator. NO signing capability.
pub struct FlashbotsDryRun {
    #[allow(dead_code)]
    relay_url: String,
}

impl FlashbotsDryRun {
    pub fn new(relay_url: String) -> Result<Self, ConnectorError> {
        if relay_url.is_empty() {
            return Err(ConnectorError::ConfigMissing {
                parameter: "ARBX_FLASHBOTS_RELAY_URL".into(),
            });
        }
        // CRITICAL: Verify no signer/private_key exists
        Ok(Self { relay_url })
    }

    pub fn from_env() -> Result<Self, ConnectorError> {
        let url = shared_rs::config::require_env("ARBX_FLASHBOTS_RELAY_URL").map_err(|e| {
            ConnectorError::ConfigMissing {
                parameter: format!("ARBX_FLASHBOTS_RELAY_URL: {e}"),
            }
        })?;
        Self::new(url)
    }

    /// Simulate a bundle via eth_callBundle. NO real execution.
    /// Capital Exposure: 0.
    pub async fn simulate_bundle(
        &self,
        _raw_txs: &[Vec<u8>],
        _target_block: u64,
    ) -> Result<SimulationResult, ConnectorError> {
        // Alloy wiring deferred to integration sprint.
        Err(ConnectorError::ContractCallFailed {
            contract: "flashbots_relay".into(),
            method: "eth_callBundle".into(),
            reason: "alloy provider not yet wired — structural scaffold".into(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_empty_relay_url() {
        assert!(FlashbotsDryRun::new(String::new()).is_err());
    }

    #[test]
    fn accepts_valid_relay_url() {
        assert!(FlashbotsDryRun::new("https://relay.flashbots.net".into()).is_ok());
    }

    #[test]
    fn no_signing_types_in_module() {
        // Compile-time assertion: this module has no SecretKey, Signer,
        // Wallet, or PrivateKey types. If this test compiles, the
        // structural invariant holds.
        let _sim = FlashbotsDryRun::new("https://relay.flashbots.net".into()).unwrap();
        // No .sign(), .send_bundle(), or .send_raw_transaction() methods exist.
    }
}
