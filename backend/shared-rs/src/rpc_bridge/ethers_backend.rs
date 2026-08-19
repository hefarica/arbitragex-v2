//! rpc_bridge::ethers_backend — FASE 2 of the ethers→alloy dual-track plan.
//!
//! [`EthersReader`] implements the neutral [`RpcReader`] contract by wrapping
//! the ethers-rs 2.0 provider + contract calls that already run in production.
//! It is path A (the DEFAULT): the bridge is pure indirection, no behavior
//! change, so wiring it in front of existing code cannot alter results.
//!
//! The neutral contract speaks alloy primitives (`Address`/`U256`/`Bytes`);
//! this adapter converts at its edge (raw 20/32-byte buffers, zero-copy
//! `bytes::Bytes` — both crates share one `bytes` version in the lock).

use crate::rpc_bridge::traits::{BridgeError, RpcReader, V2Reserves, V3Slot0};
use alloy::primitives::{Address, Bytes as AlloyBytes, U256 as AlloyU256};
use ethers::contract::{BaseContract, Contract};
use ethers::providers::{Http, Middleware, Provider};
use ethers::types::{
    transaction::eip2718::TypedTransaction, Address as EthersAddress, Bytes as EthersBytes,
    TransactionRequest as EthersTxRequest, U256 as EthersU256,
};
use std::sync::Arc;
use std::time::Duration;

/// Outer bound for every read. ethers' `Http` transport uses a default
/// reqwest client we cannot reconfigure from this crate (it would require
/// building ethers' own reqwest 0.11 client), so the timeout is enforced by
/// wrapping each future — the caller is always bounded even if the socket
/// wedges underneath.
pub const CALL_TIMEOUT_MS: u64 = 10_000;

/// Minimal IUniswapV2Pair ABI (getReserves only).
const PAIR_ABI_JSON: &str = r#"
[
  {
    "type": "function",
    "name": "getReserves",
    "stateMutability": "view",
    "inputs": [],
    "outputs": [
      { "internalType": "uint112", "name": "reserve0", "type": "uint112" },
      { "internalType": "uint112", "name": "reserve1", "type": "uint112" },
      { "internalType": "uint32", "name": "blockTimestampLast", "type": "uint32" }
    ]
  }
]
"#;

/// Minimal IUniswapV3Pool ABI (slot0 + liquidity only).
const POOL_ABI_JSON: &str = r#"
[
  {
    "type": "function",
    "name": "slot0",
    "stateMutability": "view",
    "inputs": [],
    "outputs": [
      { "internalType": "uint160", "name": "sqrtPriceX96", "type": "uint160" },
      { "internalType": "int24", "name": "tick", "type": "int24" },
      { "internalType": "uint16", "name": "observationIndex", "type": "uint16" },
      { "internalType": "uint16", "name": "observationCardinality", "type": "uint16" },
      { "internalType": "uint16", "name": "feeProtocol", "type": "uint16" },
      { "internalType": "bool", "name": "unlocked", "type": "bool" }
    ]
  },
  {
    "type": "function",
    "name": "liquidity",
    "stateMutability": "view",
    "inputs": [],
    "outputs": [
      { "internalType": "uint128", "name": "liquidity", "type": "uint128" }
    ]
  }
]
"#;

fn parse_abi(json: &str, label: &str) -> Result<BaseContract, BridgeError> {
    let abi: ethers::abi::Contract = serde_json::from_str(json)
        .map_err(|e| BridgeError::Invalid(format!("embedded {label} ABI is invalid: {e}")))?;
    Ok(BaseContract::from(abi))
}

// ---------- edge conversions (alloy <-> ethers primitives) ----------

fn to_ethers_address(a: &Address) -> EthersAddress {
    EthersAddress::from_slice(a.as_slice())
}

fn u256_to_alloy(v: &EthersU256) -> AlloyU256 {
    let mut buf = [0u8; 32];
    v.to_big_endian(&mut buf);
    AlloyU256::from_be_bytes(buf)
}

fn u256_to_u64(v: &ethers::types::U256) -> Result<u64, BridgeError> {
    v.as_u64()
        .ok_or_else(|| BridgeError::Invalid("numeric result exceeds u64".to_string()))
}

/// ethers-based implementation of the neutral [`RpcReader`] contract.
pub struct EthersReader {
    provider: Arc<Provider<Http>>,
    pair_abi: BaseContract,
    pool_abi: BaseContract,
}

impl EthersReader {
    /// Build a reader for an HTTP RPC endpoint. No network I/O happens here;
    /// an unparseable URL (or a corrupt embedded ABI) fails honestly.
    pub fn new(url: &str) -> Result<Self, BridgeError> {
        let parsed = url
            .parse::<reqwest::Url>()
            .map_err(|e| BridgeError::Invalid(format!("invalid RPC url {url:?}: {e}")))?;
        let provider: Provider<Http> = Provider::new(Http::new(parsed));
        Ok(Self {
            provider: Arc::new(provider),
            pair_abi: parse_abi(PAIR_ABI_JSON, "IUniswapV2Pair")?,
            pool_abi: parse_abi(POOL_ABI_JSON, "IUniswapV3Pool")?,
        })
    }

    /// Bound a read future by [`CALL_TIMEOUT_MS`], mapping elapsed to
    /// [`BridgeError::Timeout`].
    async fn bounded<F, T>(fut: F) -> Result<T, BridgeError>
    where
        F: std::future::Future<Output = Result<T, BridgeError>>,
    {
        match tokio::time::timeout(Duration::from_millis(CALL_TIMEOUT_MS), fut).await {
            Ok(res) => res,
            Err(_elapsed) => Err(BridgeError::Timeout(CALL_TIMEOUT_MS)),
        }
    }

    async fn call_contract<D>(
        &self,
        abi: &BaseContract,
        label: &str,
        pool: Address,
    ) -> Result<D, BridgeError>
    where
        D: ethers::abi::Detokenize + Send + Sync,
    {
        let contract = Contract::new(to_ethers_address(&pool), abi.clone(), self.provider.clone());
        let call = contract
            .method::<(), D>(label, ())
            .map_err(|e| BridgeError::Invalid(format!("{label} not in ABI: {e}")))?;
        Self::bounded(async {
            call.call()
                .await
                .map_err(|e| BridgeError::Rpc(format!("{label} call to {pool} failed: {e}")))
        })
        .await
    }
}

#[async_trait::async_trait]
impl RpcReader for EthersReader {
    async fn get_block_number(&self) -> Result<u64, BridgeError> {
        Self::bounded(async {
            let n = self
                .provider
                .get_block_number()
                .await
                .map_err(|e| BridgeError::Rpc(format!("eth_blockNumber failed: {e}")))?;
            // ethers' U64::as_u64 is total (u64-wide type).
            Ok(n.as_u64())
        })
        .await
    }

    async fn get_chain_id(&self) -> Result<u64, BridgeError> {
        Self::bounded(async {
            let id = self
                .provider
                .get_chainid()
                .await
                .map_err(|e| BridgeError::Rpc(format!("eth_chainId failed: {e}")))?;
            u256_to_u64(&id)
        })
        .await
    }

    async fn get_reserves_v2(&self, pool: Address) -> Result<V2Reserves, BridgeError> {
        let (r0, r1, _block_timestamp_last) = self
            .call_contract::<(EthersU256, EthersU256, u32)>(&self.pair_abi, "getReserves", pool)
            .await?;
        Ok(V2Reserves {
            reserve0: u256_to_alloy(&r0),
            reserve1: u256_to_alloy(&r1),
        })
    }

    async fn get_slot0_v3(&self, pool: Address) -> Result<V3Slot0, BridgeError> {
        // slot0() does not carry liquidity; the neutral struct bundles both.
        let (sqrt_price_x96, tick, _obs_idx, _obs_card, _fee_proto, _unlocked) = self
            .call_contract::<(EthersU256, i64, u16, u16, u16, bool)>(&self.pool_abi, "slot0", pool)
            .await?;
        let liquidity = self
            .call_contract::<u128>(&self.pool_abi, "liquidity", pool)
            .await?;
        Ok(V3Slot0 {
            sqrt_price_x96: u256_to_alloy(&sqrt_price_x96),
            liquidity,
            // int24 always fits i32; the clamp below stays honest if it ever
            // does not (fail instead of silently truncating).
            tick: i32::try_from(tick)
                .map_err(|e| BridgeError::Invalid(format!("tick out of i32 range: {e}")))?,
        })
    }

    async fn eth_call(&self, to: Address, data: AlloyBytes) -> Result<AlloyBytes, BridgeError> {
        let tx: TypedTransaction = EthersTxRequest::new()
            .to(to_ethers_address(&to))
            .data(EthersBytes(data.0))
            .into();
        Self::bounded(async {
            let out = self
                .provider
                .call(&tx, None)
                .await
                .map_err(|e| BridgeError::Rpc(format!("eth_call to {to} failed: {e}")))?;
            Ok(AlloyBytes(out.0))
        })
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ethers::abi::FunctionExt; // selector() extension on ethabi::Function

    /// getReserves() = 0x0902f1ac (canonical UniswapV2 selector).
    #[test]
    fn pair_abi_get_reserves_selector_is_canonical() {
        let abi = parse_abi(PAIR_ABI_JSON, "IUniswapV2Pair").expect("pair ABI must parse");
        let selector = abi
            .abi()
            .function("getReserves")
            .expect("getReserves in ABI")
            .selector();
        assert_eq!(selector, [0x09, 0x02, 0xf1, 0xac]);
    }

    /// slot0() = 0x3850c7bd, liquidity() = 0x1a686502 (canonical V3 selectors).
    #[test]
    fn pool_abi_v3_selectors_are_canonical() {
        let abi = parse_abi(POOL_ABI_JSON, "IUniswapV3Pool").expect("pool ABI must parse");
        assert_eq!(
            abi.abi()
                .function("slot0")
                .expect("slot0 in ABI")
                .selector(),
            [0x38, 0x50, 0xc7, 0xbd]
        );
        assert_eq!(
            abi.abi()
                .function("liquidity")
                .expect("liquidity in ABI")
                .selector(),
            [0x1a, 0x68, 0x65, 0x02]
        );
    }

    #[test]
    fn corrupt_embedded_abi_fails_honestly() {
        let err =
            parse_abi("[{\"type\":\"function\"}]  ", "broken").expect_err("broken ABI must fail");
        assert!(matches!(err, BridgeError::Invalid(_)), "got {err:?}");
    }

    fn word(v: &EthersU256) -> [u8; 32] {
        let mut buf = [0u8; 32];
        v.to_big_endian(&mut buf);
        buf
    }

    /// Decode a hand-built getReserves() return blob through the same ABI the
    /// live path uses (offline — proves the JSON ABI + decode types).
    #[test]
    fn decodes_get_reserves_return_data() {
        let abi = parse_abi(PAIR_ABI_JSON, "IUniswapV2Pair").expect("pair ABI must parse");
        let mut data = Vec::with_capacity(96);
        data.extend_from_slice(&word(&EthersU256::from(1_000_000_000_000_000_000u128)));
        data.extend_from_slice(&word(&EthersU256::from(2_500_000_000_000_000_000u128)));
        data.extend_from_slice(&word(&EthersU256::from(1_700_000_000u64)));

        let (r0, r1, ts): (EthersU256, EthersU256, u32) = abi
            .decode_output("getReserves", data.as_slice())
            .expect("valid 3-word return blob must decode");
        assert_eq!(r0, EthersU256::from(1_000_000_000_000_000_000u128));
        assert_eq!(r1, EthersU256::from(2_500_000_000_000_000_000u128));
        assert_eq!(ts, 1_700_000_000);

        // Neutral mapping edge conversion round-trips the same bytes.
        assert_eq!(
            u256_to_alloy(&r0),
            AlloyU256::from(1_000_000_000_000_000_000u128)
        );
    }

    /// Decode hand-built slot0() + liquidity() returns through the live ABI,
    /// including a NEGATIVE tick (int24 sign extension).
    #[test]
    fn decodes_slot0_return_data_with_negative_tick() {
        let abi = parse_abi(POOL_ABI_JSON, "IUniswapV3Pool").expect("pool ABI must parse");
        let mut slot0 = Vec::with_capacity(192);
        slot0.extend_from_slice(&word(&EthersU256::from(79228162514264337593543950336u128))); // 2^96
        slot0.extend_from_slice(&word(&(EthersU256::MAX - 4u64.into()))); // -5 two's complement
        slot0.extend_from_slice(&word(&1u64.into())); // observationIndex
        slot0.extend_from_slice(&word(&3u64.into())); // observationCardinality
        slot0.extend_from_slice(&word(&0u64.into())); // feeProtocol
        slot0.extend_from_slice(&word(&1u64.into())); // unlocked = true

        let (sqrt_price_x96, tick, obs_idx, obs_card, fee_proto, unlocked): (
            EthersU256,
            i64,
            u16,
            u16,
            u16,
            bool,
        ) = abi
            .decode_output("slot0", slot0.as_slice())
            .expect("valid 6-word slot0 blob must decode");
        assert_eq!(
            sqrt_price_x96,
            EthersU256::from(79228162514264337593543950336u128)
        );
        assert_eq!(tick, -5);
        assert_eq!((obs_idx, obs_card, fee_proto, unlocked), (1, 3, 0, true));

        let liquidity: u128 = abi
            .decode_output(
                "liquidity",
                word(&EthersU256::from(12_345_678_912_345_678_912u128)).as_slice(),
            )
            .expect("valid liquidity blob must decode");
        assert_eq!(liquidity, 12_345_678_912_345_678_912u128);
    }

    #[test]
    fn address_conversion_round_trips() {
        let alloy_addr = Address::from([0x11u8; 20]);
        let ethers_addr = to_ethers_address(&alloy_addr);
        assert_eq!(ethers_addr.as_bytes(), alloy_addr.as_slice());
        let back = Address::from(*ethers_addr.as_bytes());
        assert_eq!(back, alloy_addr);
    }

    #[test]
    fn new_rejects_invalid_url() {
        let err = EthersReader::new("not a url").expect_err("invalid url must be rejected");
        assert!(matches!(err, BridgeError::Invalid(_)), "got {err:?}");
    }

    #[test]
    fn new_accepts_valid_url_without_network() {
        let _reader =
            EthersReader::new("http://127.0.0.1:8545").expect("valid url must construct a reader");
    }

    /// Connection-refused against a local port: must fail honestly (any
    /// variant) instead of hanging past the call timeout.
    #[tokio::test]
    async fn get_block_number_fails_fast_on_unreachable_endpoint() {
        let reader = EthersReader::new("http://127.0.0.1:1").expect("construct reader");
        let res = reader.get_block_number().await;
        assert!(res.is_err(), "unreachable endpoint must not succeed");
    }
}
