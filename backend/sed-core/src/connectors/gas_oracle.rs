//! Gas Oracle — Real gas price from eth_gasPrice + EIP-1559.
//!
//! Reads gas prices from REAL RPC endpoints. Never hardcoded.

use super::error::ConnectorError;

/// Real-time gas price data from the network.
#[derive(Debug, Clone)]
pub struct GasPrice {
    /// Base fee (wei) from latest block
    pub base_fee_wei: u64,
    /// Suggested max priority fee (wei)
    pub max_priority_fee_wei: u64,
    /// Total suggested gas price (wei)
    pub suggested_gas_price_wei: u64,
    /// Block number at which gas was sampled
    pub block_number: u64,
}

impl GasPrice {
    /// Convert gas price to gwei for human-readable metrics.
    pub fn base_fee_gwei(&self) -> f64 {
        self.base_fee_wei as f64 / 1e9
    }

    /// Estimate execution cost in ETH for a given gas limit.
    pub fn estimated_cost_eth(&self, gas_limit: u64) -> f64 {
        (self.suggested_gas_price_wei as f64 * gas_limit as f64) / 1e18
    }
}

/// Gas oracle — reads real gas prices via eth_gasPrice.
pub struct GasOracle {
    rpc_url: String,
}

impl GasOracle {
    pub fn new(rpc_url: String) -> Result<Self, ConnectorError> {
        if rpc_url.is_empty() {
            return Err(ConnectorError::ConfigMissing {
                parameter: "ARBX_RPC_HTTP_URL".into(),
            });
        }
        Ok(Self { rpc_url })
    }

    pub fn from_env() -> Result<Self, ConnectorError> {
        let url = shared_rs::config::require_env("ARBX_RPC_HTTP_URL").map_err(|e| {
            ConnectorError::ConfigMissing {
                parameter: format!("ARBX_RPC_HTTP_URL: {e}"),
            }
        })?;
        Self::new(url)
    }

    /// Read current gas price via eth_gasPrice + eth_feeHistory.
    /// Capital Exposure: 0.
    pub async fn current_gas_price(&self) -> Result<GasPrice, ConnectorError> {
        // Alloy wiring deferred to integration sprint.
        Err(ConnectorError::ContractCallFailed {
            contract: "eth".into(),
            method: "eth_gasPrice".into(),
            reason: "alloy provider not yet wired — structural scaffold".into(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gas_price_gwei_conversion() {
        let gp = GasPrice {
            base_fee_wei: 20_000_000_000,
            max_priority_fee_wei: 2_000_000_000,
            suggested_gas_price_wei: 22_000_000_000,
            block_number: 18000000,
        };
        assert!((gp.base_fee_gwei() - 20.0).abs() < 1e-9);
    }

    #[test]
    fn estimated_cost_scales_with_gas_limit() {
        let gp = GasPrice {
            base_fee_wei: 20_000_000_000,
            max_priority_fee_wei: 2_000_000_000,
            suggested_gas_price_wei: 22_000_000_000,
            block_number: 18000000,
        };
        let cost = gp.estimated_cost_eth(200_000);
        assert!(cost > 0.0);
        assert!(cost < 1.0); // ~0.0044 ETH at 22 gwei
    }
}
