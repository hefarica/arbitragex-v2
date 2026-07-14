use ethers::types::U256;
use std::sync::Arc;
use ethers_providers::{Http, Middleware, Provider};
use ethers_core::types::BlockNumber;

pub struct GasOracle { provider: Arc<Provider<Http>> }

#[derive(Clone)]
pub struct GasEstimate {
    pub max_fee_per_gas: U256,
    pub max_priority_fee_per_gas: U256,
    pub base_fee: U256,
}

#[derive(Debug)]
pub enum GasOracleError { ProviderError(String), NoLatestBlock, NoBaseFee }

impl GasOracle {
    pub fn new(provider: Arc<Provider<Http>>) -> Self { Self { provider } }

    pub async fn estimate(&self) -> Result<GasEstimate, GasOracleError> {
        let block = self.provider.get_block(BlockNumber::Latest).await
            .map_err(|e| GasOracleError::ProviderError(e.to_string()))?
            .ok_or(GasOracleError::NoLatestBlock)?;
        let base_fee = block.base_fee_per_gas.ok_or(GasOracleError::NoBaseFee)?;
        let priority_fee = U256::from(2_000_000_000u64);
        Ok(GasEstimate { max_fee_per_gas: base_fee * 2 + priority_fee, max_priority_fee_per_gas: priority_fee, base_fee })
    }
}
