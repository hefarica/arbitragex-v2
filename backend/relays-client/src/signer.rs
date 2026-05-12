//! Signer. Loads FLASHBOTS_SIGNER_KEY from env, converts to ethers Wallet.
//! The raw key string is never stored in the struct — only the Wallet.
//! The address is logged on boot, the key never is.

use anyhow::{Context, Result};
use ethers::signers::{LocalWallet, Signer as _};
use ethers::types::Address;
use ethers::utils::keccak256;

pub struct Signer {
    pub wallet: LocalWallet,
    pub address: Address,
    pub chain_id: u64,
}

impl Signer {
    /// Attempt to load from env. Returns Ok(None) if the env var is empty/unset.
    pub fn from_env(chain_id: u64) -> Result<Option<Self>> {
        let key = std::env::var("FLASHBOTS_SIGNER_KEY").unwrap_or_default();
        if key.is_empty() {
            return Ok(None);
        }
        let trimmed = key.trim_start_matches("0x");
        let wallet: LocalWallet = trimmed
            .parse::<LocalWallet>()
            .context("invalid FLASHBOTS_SIGNER_KEY (expected hex private key)")?;
        let wallet = wallet.with_chain_id(chain_id);
        let address = wallet.address();
        Ok(Some(Self {
            wallet,
            address,
            chain_id,
        }))
    }

    /// Signs an arbitrary body for Flashbots' X-Flashbots-Signature header.
    /// Format: `<address>:<eth_sign(keccak256(hex_body))>`.
    pub async fn flashbots_auth_header(&self, body: &[u8]) -> Result<String> {
        let digest = keccak256(body);
        // Convention: the message signed is the hex encoding of the digest.
        let msg = format!("0x{}", hex::encode(digest));
        let sig = self
            .wallet
            .sign_message(msg.as_bytes())
            .await
            .context("flashbots auth signature")?;
        Ok(format!(
            "0x{}:0x{}",
            hex::encode(self.address.as_bytes()),
            sig
        ))
    }
}

// Intentional: we do NOT implement Debug for Signer to avoid accidental logging.
impl std::fmt::Debug for Signer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Signer")
            .field("address", &self.address)
            .field("chain_id", &self.chain_id)
            .finish_non_exhaustive()
    }
}
