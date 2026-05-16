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

#[cfg(test)]
mod tests {
    // OMEGA-S5/OMEGA-100 (2026-05-16): Tests legitimately panic on setup
    // failure (ephemeral key parsing, env-restore, header signing). Calling
    // .expect() inside a #[test] is the idiomatic way to surface a setup bug
    // as a test failure — production code paths in this module already use
    // Result<_, RelayError> and are gated by `clippy::expect_used` at the
    // crate level. Justification: doctrina-compliant scope-limited allowance.
    #![allow(clippy::expect_used, clippy::unwrap_used)]

    use super::*;
    use ethers::signers::LocalWallet;

    // Ephemeral test key (deterministic, well-known test vector). MUST NEVER
    // be reused for any real account. Anvil test account #0 well-known
    // private key.
    const EPHEMERAL_TEST_KEY: &str =
        "ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";

    fn ephemeral_signer(chain_id: u64) -> Signer {
        let wallet: LocalWallet = EPHEMERAL_TEST_KEY
            .parse()
            .expect("ephemeral test key parses");
        let wallet = wallet.with_chain_id(chain_id);
        let address = wallet.address();
        Signer { wallet, address, chain_id }
    }

    /// OMEGA-8/M4 Fase 4: from_env returns Ok(None) when the env var is
    /// missing or empty. The /execute endpoint then responds 501 NotImplemented
    /// and the consumer doesn't spawn — this is the relays-client secure-boot
    /// shape with no signer present.
    #[test]
    fn from_env_empty_returns_none() {
        // Save+clear FLASHBOTS_SIGNER_KEY for the duration of the test.
        let prev = std::env::var("FLASHBOTS_SIGNER_KEY").ok();
        std::env::remove_var("FLASHBOTS_SIGNER_KEY");
        let res = Signer::from_env(1).expect("env load must not panic");
        assert!(res.is_none(), "empty env must yield None");
        if let Some(v) = prev {
            std::env::set_var("FLASHBOTS_SIGNER_KEY", v);
        }
    }

    /// OMEGA-8/M4 Fase 4: from_env rejects malformed keys with an Err rather
    /// than crashing the boot path. RULE 8 (no silent errors).
    #[test]
    fn from_env_invalid_key_returns_err() {
        let prev = std::env::var("FLASHBOTS_SIGNER_KEY").ok();
        std::env::set_var("FLASHBOTS_SIGNER_KEY", "0xNOT_A_REAL_KEY_AT_ALL");
        let res = Signer::from_env(1);
        assert!(res.is_err(), "malformed key must surface as Err, not panic");
        match prev {
            Some(v) => std::env::set_var("FLASHBOTS_SIGNER_KEY", v),
            None => std::env::remove_var("FLASHBOTS_SIGNER_KEY"),
        }
    }

    /// OMEGA-8/M4 Fase 4: flashbots_auth_header produces the canonical
    /// `<address>:<sig>` format (Flashbots X-Flashbots-Signature contract).
    /// Verifies the header structure and that the private key never appears
    /// in the resulting header (RULE 13 + Ghost Protocol).
    #[tokio::test]
    async fn flashbots_auth_header_format_is_address_colon_sig() {
        let signer = ephemeral_signer(1);
        let body = b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"eth_sendBundle\"}";
        let header = signer
            .flashbots_auth_header(body)
            .await
            .expect("auth header signing");

        // Expected format: "0x<40-char-address>:0x<sig-hex>"
        let parts: Vec<&str> = header.split(':').collect();
        assert_eq!(
            parts.len(),
            2,
            "auth header must be exactly <addr>:<sig>, got {header:?}"
        );
        assert!(parts[0].starts_with("0x"), "address part must be hex-prefixed");
        assert!(parts[1].starts_with("0x"), "signature part must be hex-prefixed");
        assert_eq!(parts[0].len(), 42, "address must be 40-hex + 0x");

        // CRITICAL: the private key must NEVER leak into the header.
        assert!(
            !header.contains(EPHEMERAL_TEST_KEY),
            "private key must NEVER appear in auth header"
        );
        // The header is deterministic per (body, signer); two calls must match.
        let header2 = signer
            .flashbots_auth_header(body)
            .await
            .expect("second header");
        assert_eq!(header, header2, "auth header must be deterministic");
    }

    /// OMEGA-8/M4 Fase 4: distinct bodies produce distinct signatures (so an
    /// attacker can't replay a captured header against a different bundle).
    #[tokio::test]
    async fn flashbots_auth_header_differs_per_body() {
        let signer = ephemeral_signer(1);
        let body_a = b"bundle-a";
        let body_b = b"bundle-b";
        let header_a = signer.flashbots_auth_header(body_a).await.expect("a");
        let header_b = signer.flashbots_auth_header(body_b).await.expect("b");
        assert_ne!(
            header_a, header_b,
            "different bodies must produce different signatures"
        );
    }

    /// OMEGA-8/M4 Fase 4: Debug impl must NEVER print the wallet/private key.
    /// Defensive against accidental tracing::error!({signer:?}) regressions.
    #[test]
    fn debug_impl_does_not_leak_wallet_or_key() {
        let signer = ephemeral_signer(1);
        let dbg = format!("{:?}", signer);
        assert!(
            !dbg.contains(EPHEMERAL_TEST_KEY),
            "Debug must not leak private key"
        );
        // The wallet's internal hex is also off-limits; we should only see
        // address + chain_id.
        assert!(dbg.contains("address"), "Debug must include address");
        assert!(dbg.contains("chain_id"), "Debug must include chain_id");
        assert!(
            !dbg.to_lowercase().contains("wallet"),
            "Debug must not expose the underlying Wallet"
        );
    }

    /// OMEGA-8/M4 Fase 4: chain_id is properly threaded into the signer so
    /// transactions can't be replayed across chains (EIP-155).
    #[test]
    fn signer_carries_chain_id() {
        let signer = ephemeral_signer(137);
        assert_eq!(signer.chain_id, 137);
        let signer_eth = ephemeral_signer(1);
        assert_eq!(signer_eth.chain_id, 1);
        // Both wallets resolve to the same address (since the key is identical)
        // but the chain_id is what guards replay.
        assert_eq!(signer.address, signer_eth.address);
    }
}
