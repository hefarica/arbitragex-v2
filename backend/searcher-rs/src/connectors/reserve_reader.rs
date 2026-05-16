use crate::normalization::u256_converter::u256_to_f64_normalized;
use ethers::contract::abigen;
use ethers::providers::{Http, Middleware, Provider};
use ethers::types::{Address, U256};
use std::sync::Arc;

// ABI canónico de Uniswap V2 Pair — generado en build time vía ethers-rs
abigen!(
    IUniswapV2Pair,
    r#"[
        function getReserves() external view returns (uint112 reserve0, uint112 reserve1, uint32 blockTimestampLast)
        function token0() external view returns (address)
        function token1() external view returns (address)
    ]"#
);

#[derive(Debug, Clone)]
pub struct NormalizedReserves {
    pub pair_address: String,
    pub token0: String,
    pub token1: String,
    pub reserve0_normalized: f64,
    pub reserve1_normalized: f64,
    pub price_0_1: f64, // reserve1/reserve0 — precio relativo normalizado
    pub block_timestamp: u32,
    pub fetched_at: std::time::Instant,
}

pub struct ReserveReader {
    provider: Arc<Provider<Http>>,
}

impl ReserveReader {
    pub fn new(rpc_http_url: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let provider = Provider::<Http>::try_from(rpc_http_url)?;
        Ok(Self {
            provider: Arc::new(provider),
        })
    }

    pub async fn fetch_reserves(
        &self,
        pair_address: Address,
    ) -> Result<NormalizedReserves, Box<dyn std::error::Error>> {
        let start = std::time::Instant::now();

        let pair = IUniswapV2Pair::new(pair_address, self.provider.clone());

        // Llamadas RPC reales — SIN MOCK
        let reserves = pair.get_reserves().call().await?;
        let token0 = pair.token_0().call().await?;
        let token1 = pair.token_1().call().await?;

        let reserve0_norm = u256_to_f64_normalized(U256::from(reserves.0), 18)?;
        let reserve1_norm = u256_to_f64_normalized(U256::from(reserves.1), 18)?;

        let price_0_1 = if reserve0_norm > 0.0 {
            reserve1_norm / reserve0_norm
        } else {
            0.0
        };

        let latency_ms = start.elapsed().as_millis() as f64;
        tracing::debug!(pair = ?pair_address, latency_ms, "Reserve fetch completed");

        Ok(NormalizedReserves {
            pair_address: format!("{:?}", pair_address),
            token0: format!("{:?}", token0),
            token1: format!("{:?}", token1),
            reserve0_normalized: reserve0_norm,
            reserve1_normalized: reserve1_norm,
            price_0_1,
            block_timestamp: reserves.2,
            fetched_at: start,
        })
    }
}
