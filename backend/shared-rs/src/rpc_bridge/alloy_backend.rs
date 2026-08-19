//! rpc_bridge::alloy_backend — FASE 1 of the ethers→alloy dual-track plan.
//!
//! [`AlloyReader`] implements the neutral [`RpcReader`] contract on top of
//! alloy 1.x (`RootProvider<Ethereum>`), the same provider shape already
//! proven in `rpc_failover.rs`. It is the SECOND path (path B): it only runs
//! when the Redis toggle selects `alloy` (or `shadow`), never by default.
//!
//! All reads are plain `eth_call`s whose calldata is encoded/decoded with
//! `alloy_sol_types` via the `sol!` interfaces below — no abigen, no runtime
//! ABI parsing.

use crate::rpc_bridge::traits::{BridgeError, RpcReader, V2Reserves, V3Slot0};
use alloy::network::Ethereum;
use alloy::primitives::{Address, Bytes, U256};
use alloy::providers::{Provider, ProviderBuilder, RootProvider};
use alloy::rpc::types::{TransactionInput, TransactionRequest};
use std::time::Duration;

/// Client-level HTTP timeout. The reqwest client enforces this inside the
/// transport (see the note in `rpc_failover.rs`: a wedged `eth_call` cannot
/// be preempted from outside), and every read below additionally wraps the
/// future in `tokio::time::timeout` so the caller is bounded either way.
pub const CALL_TIMEOUT_MS: u64 = 10_000;

/// Minimal Uniswap interfaces needed by the reader, encoded via `sol!`.
mod contracts {
    alloy::sol_types::sol! {
        interface IUniswapV2Pair {
            function getReserves() external view returns (
                uint112 reserve0,
                uint112 reserve1,
                uint32 blockTimestampLast
            );
        }

        interface IUniswapV3Pool {
            function slot0() external view returns (
                uint160 sqrtPriceX96,
                int24 tick,
                uint16 observationIndex,
                uint16 observationCardinality,
                uint16 feeProtocol,
                bool unlocked
            );
            function liquidity() external view returns (uint128);
        }
    }
}

/// alloy-based implementation of the neutral [`RpcReader`] contract.
pub struct AlloyReader {
    provider: RootProvider<Ethereum>,
}

impl AlloyReader {
    /// Build a reader for an HTTP RPC endpoint. No network I/O happens here;
    /// an unparseable URL fails honestly with [`BridgeError::Invalid`].
    pub fn new(url: &str) -> Result<Self, BridgeError> {
        let parsed = url
            .parse::<reqwest::Url>()
            .map_err(|e| BridgeError::Invalid(format!("invalid RPC url {url:?}: {e}")))?;
        // Pre-build the reqwest client so the request timeout is enforced at
        // the transport level (fallible, unlike the `with_reqwest` closure
        // which forces an infallible client construction).
        let client = reqwest::Client::builder()
            .timeout(Duration::from_millis(CALL_TIMEOUT_MS))
            .build()
            .map_err(|e| BridgeError::Rpc(format!("reqwest client build failed: {e}")))?;
        let provider = ProviderBuilder::new()
            .disable_recommended_fillers()
            .connect_reqwest(client, parsed);
        Ok(Self { provider })
    }

    /// Raw `eth_call` of `calldata` against `to`, bounded by the call timeout.
    async fn call(&self, to: Address, calldata: Vec<u8>) -> Result<Bytes, BridgeError> {
        let tx = TransactionRequest::default()
            .to(to)
            .input(TransactionInput::new(calldata.into()));
        match tokio::time::timeout(
            Duration::from_millis(CALL_TIMEOUT_MS),
            self.provider.call(tx),
        )
        .await
        {
            Ok(Ok(bytes)) => Ok(bytes),
            Ok(Err(e)) => Err(BridgeError::Rpc(format!("eth_call to {to} failed: {e}"))),
            Err(_elapsed) => Err(BridgeError::Timeout(CALL_TIMEOUT_MS)),
        }
    }
}

#[async_trait::async_trait]
impl RpcReader for AlloyReader {
    async fn get_block_number(&self) -> Result<u64, BridgeError> {
        match tokio::time::timeout(
            Duration::from_millis(CALL_TIMEOUT_MS),
            self.provider.get_block_number(),
        )
        .await
        {
            Ok(Ok(n)) => Ok(n),
            Ok(Err(e)) => Err(BridgeError::Rpc(format!("eth_blockNumber failed: {e}"))),
            Err(_elapsed) => Err(BridgeError::Timeout(CALL_TIMEOUT_MS)),
        }
    }

    async fn get_chain_id(&self) -> Result<u64, BridgeError> {
        match tokio::time::timeout(
            Duration::from_millis(CALL_TIMEOUT_MS),
            self.provider.get_chain_id(),
        )
        .await
        {
            Ok(Ok(id)) => Ok(id),
            Ok(Err(e)) => Err(BridgeError::Rpc(format!("eth_chainId failed: {e}"))),
            Err(_elapsed) => Err(BridgeError::Timeout(CALL_TIMEOUT_MS)),
        }
    }

    async fn get_reserves_v2(&self, pool: Address) -> Result<V2Reserves, BridgeError> {
        use contracts::IUniswapV2Pair;

        let calldata = IUniswapV2Pair::getReservesCall {}.abi_encode();
        let ret = self.call(pool, calldata).await?;
        let decoded = IUniswapV2Pair::getReservesCall::abi_decode_returns(&ret)
            .map_err(|e| BridgeError::Invalid(format!("getReserves decode failed: {e}")))?;
        Ok(V2Reserves {
            reserve0: U256::from_uint(decoded.reserve0),
            reserve1: U256::from_uint(decoded.reserve1),
        })
    }

    async fn get_slot0_v3(&self, pool: Address) -> Result<V3Slot0, BridgeError> {
        use contracts::IUniswapV3Pool;

        // slot0() does not carry liquidity; the neutral struct bundles both,
        // so issue both view calls.
        let slot0 = self
            .call(pool, IUniswapV3Pool::slot0Call {}.abi_encode())
            .await?;
        let liquidity = self
            .call(pool, IUniswapV3Pool::liquidityCall {}.abi_encode())
            .await?;

        let slot0 = IUniswapV3Pool::slot0Call::abi_decode_returns(&slot0)
            .map_err(|e| BridgeError::Invalid(format!("slot0 decode failed: {e}")))?;
        let liquidity = IUniswapV3Pool::liquidityCall::abi_decode_returns(&liquidity)
            .map_err(|e| BridgeError::Invalid(format!("liquidity decode failed: {e}")))?;
        Ok(V3Slot0 {
            sqrt_price_x96: U256::from_uint(slot0.sqrtPriceX96),
            liquidity,
            tick: i32::try_from(slot0.tick)
                .map_err(|e| BridgeError::Invalid(format!("tick out of i32 range: {e}")))?,
        })
    }

    async fn eth_call(&self, to: Address, data: Bytes) -> Result<Bytes, BridgeError> {
        self.call(to, data.to_vec()).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// getReserves() = 0x0902f1ac (canonical UniswapV2 selector).
    #[test]
    fn get_reserves_selector_is_canonical() {
        assert_eq!(
            contracts::IUniswapV2Pair::getReservesCall::SELECTOR,
            [0x09, 0x02, 0xf1, 0xac]
        );
    }

    /// slot0() = 0x3850c7bd, liquidity() = 0x1a686502 (canonical V3 selectors).
    #[test]
    fn v3_selectors_are_canonical() {
        assert_eq!(
            contracts::IUniswapV3Pool::slot0Call::SELECTOR,
            [0x38, 0x50, 0xc7, 0xbd]
        );
        assert_eq!(
            contracts::IUniswapV3Pool::liquidityCall::SELECTOR,
            [0x1a, 0x68, 0x65, 0x02]
        );
    }

    #[test]
    fn get_reserves_calldata_is_bare_selector() {
        // No arguments: encoded calldata is exactly the 4-byte selector.
        let calldata = contracts::IUniswapV2Pair::getReservesCall {}.abi_encode();
        assert_eq!(calldata, vec![0x09, 0x02, 0xf1, 0xac]);
    }

    /// ABI-decode a hand-built getReserves() return blob and verify the
    /// neutral mapping (offline — exercises decode + U112→U256 widening).
    #[test]
    fn decodes_get_reserves_return_data() {
        let mut data = Vec::with_capacity(96);
        data.extend_from_slice(&U256::from(1_000_000_000_000_000_000u128).to_be_bytes::<32>());
        data.extend_from_slice(&U256::from(2_500_000_000_000_000_000u128).to_be_bytes::<32>());
        data.extend_from_slice(&U256::from(1_700_000_000u64).to_be_bytes::<32>());

        let decoded = contracts::IUniswapV2Pair::getReservesCall::abi_decode_returns(&data)
            .expect("valid 3-word return blob must decode");
        assert_eq!(
            U256::from_uint(decoded.reserve0),
            U256::from(1_000_000_000_000_000_000u128)
        );
        assert_eq!(
            U256::from_uint(decoded.reserve1),
            U256::from(2_500_000_000_000_000_000u128)
        );
        assert_eq!(decoded.blockTimestampLast, 1_700_000_000);
    }

    /// ABI-decode hand-built slot0() + liquidity() returns, including a
    /// NEGATIVE tick, and verify the neutral mapping (I24→i32 sign handling).
    #[test]
    fn decodes_slot0_return_data_with_negative_tick() {
        let mut slot0 = Vec::with_capacity(192);
        slot0.extend_from_slice(&U256::from(79228162514264337593543950336u128).to_be_bytes::<32>()); // 2^96
        slot0.extend_from_slice(&(U256::MAX - U256::from(4u64)).to_be_bytes::<32>()); // -5 two's complement
        slot0.extend_from_slice(&U256::from(1u8).to_be_bytes::<32>()); // observationIndex
        slot0.extend_from_slice(&U256::from(3u8).to_be_bytes::<32>()); // observationCardinality
        slot0.extend_from_slice(&U256::ZERO.to_be_bytes::<32>()); // feeProtocol
        slot0.extend_from_slice(&U256::from(1u8).to_be_bytes::<32>()); // unlocked = true

        let decoded = contracts::IUniswapV3Pool::slot0Call::abi_decode_returns(&slot0)
            .expect("valid 6-word slot0 blob must decode");
        assert_eq!(
            U256::from_uint(decoded.sqrtPriceX96),
            U256::from(2u128.pow(96))
        );
        assert_eq!(
            i32::try_from(decoded.tick).expect("int24 always fits i32"),
            -5
        );

        let liquidity_word = U256::from(12_345_678_912_345_678_912u128).to_be_bytes::<32>();
        let liquidity =
            contracts::IUniswapV3Pool::liquidityCall::abi_decode_returns(&liquidity_word)
                .expect("valid liquidity blob must decode");
        assert_eq!(liquidity, 12_345_678_912_345_678_912u128);
    }

    #[test]
    fn rejects_short_return_data() {
        // Fail-honest: a truncated blob must error, never fabricate zeros.
        let err = contracts::IUniswapV2Pair::getReservesCall::abi_decode_returns(&[0u8; 64])
            .expect_err("truncated return blob must fail decode");
        assert!(!err.to_string().is_empty());
    }

    #[test]
    fn new_rejects_invalid_url() {
        let err = AlloyReader::new("not a url").expect_err("invalid url must be rejected");
        assert!(matches!(err, BridgeError::Invalid(_)), "got {err:?}");
    }

    #[test]
    fn new_accepts_valid_url_without_network() {
        let _reader =
            AlloyReader::new("http://127.0.0.1:8545").expect("valid url must construct a reader");
    }

    /// Connection-refused against a local port: must fail honestly (any
    /// variant) instead of hanging past the call timeout.
    #[tokio::test]
    async fn get_block_number_fails_fast_on_unreachable_endpoint() {
        let reader = AlloyReader::new("http://127.0.0.1:1").expect("construct reader");
        let res = reader.get_block_number().await;
        assert!(res.is_err(), "unreachable endpoint must not succeed");
    }
}
