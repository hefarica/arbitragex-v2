//! rpc_bridge::traits — neutral contracts for the ethers↔alloy dual-track
//! migration (macro plan FASE 0, `docs/plans/alloy-parallel-path-macro-plan.md` §2).
//!
//! These traits are the ONLY surface services should eventually code against:
//! they mention neither ethers nor alloy. Both backends implement
//! [`RpcReader`] behind [`BackendFactory`], and the runtime toggle
//! (`arbx:rpc_backend:<service>` in Redis, per macro plan §2.3) decides which
//! implementation answers.
//!
//! Type choice: the neutral primitives are `alloy::primitives`
//! (`Address`/`U256`/`Bytes`, i.e. alloy-primitives 1.x). This is deliberate:
//! the workspace ALSO pins `alloy-primitives = "0.7"` for token-enricher, and
//! 0.7 types would NOT unify with the alloy 1.8 provider API — every backend
//! call would need lossy conversions. Using the alloy re-export keeps the
//! AlloyBackend zero-conversion; the EthersBackend converts at its edge.

use alloy::primitives::{Address, Bytes, U256};
use async_trait::async_trait;
use std::sync::Arc;

/// Neutral result of a V2 pair `getReserves()` read (Liquidity Manifold state).
#[derive(Debug, Clone, PartialEq)]
pub struct V2Reserves {
    pub reserve0: U256,
    pub reserve1: U256,
}

/// Neutral result of a V3 pool `slot0()` + `liquidity()` read.
///
/// Note: the on-chain `slot0()` itself does NOT return liquidity; backends
/// fetch `liquidity()` in the same call so the struct is self-contained.
#[derive(Debug, Clone, PartialEq)]
pub struct V3Slot0 {
    pub sqrt_price_x96: U256,
    pub liquidity: u128,
    pub tick: i32,
}

/// Backend selection (toggle value read from Redis key `arbx:rpc_backend:<service>`).
///
/// - `Ethers` → path A, the production default.
/// - `Alloy`  → path B, the new alloy path.
/// - `Shadow` → both paths run, ethers decides, differences are logged
///   (implemented in FASE 3; until then the factory routes to the alloy
///   reader so shadow traffic exercises the new path without deciding on it).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendSelection {
    Ethers,
    Alloy,
    Shadow,
}

impl BackendSelection {
    /// Parse the Redis toggle value. Unknown/missing values fall back to
    /// `Ethers` — the production path — so a bad toggle can never silently
    /// switch the hot-path to the unverified backend.
    // Returns Self with a fail-safe default, not the std FromStr Result
    // contract (same precedent as searcher-rs `pool_candidate.rs`).
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Self {
        match s {
            "alloy" => Self::Alloy,
            "shadow" => Self::Shadow,
            _ => Self::Ethers, // default = production
        }
    }
}

/// Neutral error shared by both backends.
#[derive(Debug, thiserror::Error)]
pub enum BridgeError {
    #[error("RPC error: {0}")]
    Rpc(String),
    #[error("timeout after {0}ms")]
    Timeout(u64),
    #[error("invalid response: {0}")]
    Invalid(String),
}

/// Trait 1: Reader — read-only RPC surface.
#[async_trait]
pub trait RpcReader: Send + Sync {
    async fn get_block_number(&self) -> Result<u64, BridgeError>;
    async fn get_chain_id(&self) -> Result<u64, BridgeError>;
    async fn get_reserves_v2(&self, pool: Address) -> Result<V2Reserves, BridgeError>;
    async fn get_slot0_v3(&self, pool: Address) -> Result<V3Slot0, BridgeError>;
    async fn eth_call(&self, to: Address, data: Bytes) -> Result<Bytes, BridgeError>;
}

/// Trait 2: Subscriber — WebSocket event surface (implemented in a later
/// macro-plan phase; defined here so the contract is stable from day one).
#[async_trait]
pub trait EventSubscriber: Send + Sync {
    async fn health_check(&self) -> Result<(), BridgeError>;
}

/// Trait 3: Signer — signature surface, SOLO relays-client (§34: the single
/// terminus that may ever sign; never implemented in the reader path).
#[async_trait]
pub trait TransactionSigner: Send + Sync {
    fn address(&self) -> Address;
    async fn sign_message(&self, msg: &[u8]) -> Result<Bytes, BridgeError>;
}

/// Factory producing the selected [`RpcReader`] implementation.
pub struct BackendFactory;

impl BackendFactory {
    /// Build a reader for `url` (an HTTP RPC endpoint). Fails honestly on an
    /// unparseable URL instead of fabricating a provider (RULE 00 / RULE 08).
    pub fn create_reader(
        selection: BackendSelection,
        url: &str,
    ) -> Result<Arc<dyn RpcReader>, BridgeError> {
        match selection {
            BackendSelection::Ethers => {
                Ok(Arc::new(super::ethers_backend::EthersReader::new(url)?))
            }
            BackendSelection::Alloy | BackendSelection::Shadow => {
                Ok(Arc::new(super::alloy_backend::AlloyReader::new(url)?))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backend_selection_parses_known_values() {
        assert_eq!(
            BackendSelection::from_str("ethers"),
            BackendSelection::Ethers
        );
        assert_eq!(BackendSelection::from_str("alloy"), BackendSelection::Alloy);
        assert_eq!(
            BackendSelection::from_str("shadow"),
            BackendSelection::Shadow
        );
    }

    #[test]
    fn backend_selection_defaults_to_ethers_on_unknown_value() {
        // A corrupted/missing Redis toggle must route to the PRODUCTION path.
        assert_eq!(BackendSelection::from_str(""), BackendSelection::Ethers);
        assert_eq!(
            BackendSelection::from_str("not-a-backend"),
            BackendSelection::Ethers
        );
        assert_eq!(
            BackendSelection::from_str("ETHERS"),
            BackendSelection::Ethers
        );
    }

    #[test]
    fn factory_rejects_invalid_urls_without_panicking() {
        for selection in [
            BackendSelection::Ethers,
            BackendSelection::Alloy,
            BackendSelection::Shadow,
        ] {
            let err = BackendFactory::create_reader(selection, "not a url")
                .expect_err("invalid url must fail, not fabricate a provider");
            assert!(
                matches!(err, BridgeError::Invalid(_)),
                "expected BridgeError::Invalid, got {err:?}"
            );
        }
    }

    #[test]
    fn factory_accepts_valid_urls_for_every_selection() {
        // Constructing the provider performs no network I/O; a valid URL must
        // produce a reader for every selection.
        for selection in [
            BackendSelection::Ethers,
            BackendSelection::Alloy,
            BackendSelection::Shadow,
        ] {
            let reader = BackendFactory::create_reader(selection, "http://127.0.0.1:8545")
                .expect("valid http url must construct");
            // Dyn-coercion proves the factory honors the neutral trait.
            let _dyn_reader: Arc<dyn RpcReader> = reader;
        }
    }
}
