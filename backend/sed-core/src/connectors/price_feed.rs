//! Price Feed — Real spot prices from on-chain pool contracts.
//!
//! Reads prices from REAL pool reserves. Never hardcoded.

use super::error::ConnectorError;
use super::reserve_reader::{OnChainReserves, PoolType, ReserveReader};

/// A price observation from a real on-chain source.
#[derive(Debug, Clone)]
pub struct PriceObservation {
    /// Token pair (base/quote)
    pub base_token: String,
    pub quote_token: String,
    /// Spot price (quote per base)
    pub spot_price: f64,
    /// Source pool address
    pub source_pool: String,
    /// Block at which price was read
    pub block_number: u64,
    /// Source type
    pub source_type: PriceSource,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PriceSource {
    UniswapV2Reserves,
    UniswapV3Slot0,
    ChainlinkOracle,
    TwapAccumulator,
}

/// Price feed aggregator — reads real prices from multiple sources.
pub struct PriceFeed {
    #[allow(dead_code)]
    reserve_reader: ReserveReader,
}

impl PriceFeed {
    pub fn new(reserve_reader: ReserveReader) -> Self {
        Self { reserve_reader }
    }

    /// Get spot price from a V2 pool's reserves.
    /// price = reserve1 / reserve0
    pub fn spot_from_reserves(
        reserves: &OnChainReserves,
    ) -> Result<PriceObservation, ConnectorError> {
        if reserves.reserve0 <= 0.0 {
            return Err(ConnectorError::DataQuality {
                source: reserves.pool_address.clone(),
                issue: "reserve0 is non-positive, cannot compute spot price".into(),
            });
        }
        Ok(PriceObservation {
            base_token: reserves.token0.clone(),
            quote_token: reserves.token1.clone(),
            spot_price: reserves.reserve1 / reserves.reserve0,
            source_pool: reserves.pool_address.clone(),
            block_number: reserves.block_number,
            source_type: match &reserves.pool_type {
                PoolType::UniswapV2 => PriceSource::UniswapV2Reserves,
                PoolType::UniswapV3 { .. } => PriceSource::UniswapV3Slot0,
                _ => PriceSource::UniswapV2Reserves,
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spot_from_reserves_computes_correctly() {
        let reserves = OnChainReserves {
            pool_address: "0xPool".into(),
            reserve0: 100.0,
            reserve1: 200.0,
            block_number: 18000000,
            token0: "WETH".into(),
            token1: "USDC".into(),
            pool_type: PoolType::UniswapV2,
        };
        let obs = PriceFeed::spot_from_reserves(&reserves).unwrap();
        assert!((obs.spot_price - 2.0).abs() < 1e-9);
    }

    #[test]
    fn spot_rejects_zero_reserve() {
        let reserves = OnChainReserves {
            pool_address: "0xPool".into(),
            reserve0: 0.0,
            reserve1: 200.0,
            block_number: 18000000,
            token0: "WETH".into(),
            token1: "USDC".into(),
            pool_type: PoolType::UniswapV2,
        };
        assert!(PriceFeed::spot_from_reserves(&reserves).is_err());
    }
}
