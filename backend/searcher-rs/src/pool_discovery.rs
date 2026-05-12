//! Pool Discovery Service — Dynamically identifies unmapped pools from the mempool.
//!
//! When `Orchestrator` receives a `RouteIntent` with an unmapped token pair,
//! it dispatches the intent here. The `PoolDiscoveryService`:
//!   1. Queries the on-chain factory to find the pool address.
//!   2. If a pool exists, it records the observation in `observed_unindexed_pairs`.
//!   3. Adds the pool to the `ImpactIndex` dynamically via `add_pool`.

use alloy::providers::Provider;
use alloy_sol_types::sol;
use ethers::types::Address;
use sqlx::postgres::PgPool;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};

use shared_rs::rpc_failover::HttpRpcPool;
use crate::impact_index::{ImpactIndex, PoolRef};
use crate::route_intent::{DetectionSource, RouteIntent};

sol! {
    #[derive(Debug)]
    interface IUniswapV2Factory {
        function getPair(address tokenA, address tokenB) external view returns (address pair);
    }

    #[derive(Debug)]
    interface IUniswapV3Factory {
        function getPool(address tokenA, address tokenB, uint24 fee) external view returns (address pool);
    }
}

pub struct PoolDiscoveryService {
    chain_id: u64,
    db: Option<PgPool>,
    redis: redis::aio::ConnectionManager,
    impact_index: Arc<RwLock<ImpactIndex>>,
    rpc_pool: Option<Arc<HttpRpcPool>>,
}

impl PoolDiscoveryService {
    pub fn new(
        chain_id: u64,
        db: Option<PgPool>,
        redis: redis::aio::ConnectionManager,
        impact_index: Arc<RwLock<ImpactIndex>>,
        rpc_pool: Option<Arc<HttpRpcPool>>,
    ) -> Self {
        Self {
            chain_id,
            db,
            redis,
            impact_index,
            rpc_pool,
        }
    }

    /// Attempts to discover missing pools for a RouteIntent using real RPC.
    pub async fn discover_from_intent(&self, intent: &RouteIntent) -> anyhow::Result<()> {
        let chain_id = self.chain_id;

        for leg in &intent.legs {
            let key = crate::impact_index::TokenPairKey::canonical(leg.token_in, leg.token_out);
            let idx = self.impact_index.read().await;
            if idx.has_pools_for_pair(key) {
                continue;
            }
            drop(idx);

            info!(
                event = "pool_discovery.unmapped_pair",
                chain_id,
                tx_hash = %intent.tx_hash,
                token_in = ?leg.token_in,
                token_out = ?leg.token_out,
                "Attempting dynamic pool discovery for unmapped pair"
            );

            let rpc = match &self.rpc_pool {
                Some(p) => p,
                None => {
                    warn!("discovery_failed: no rpc pool available");
                    continue;
                }
            };

            // Heuristic Factory lookup: 
            // In a strict setting, we query a known registry. 
            // If unknown, we try common factories based on chain_id.
            // For now, if we don't have a known factory, we log discovery_no_pool_found.
            let token_a = alloy::primitives::Address::from_slice(leg.token_in.as_bytes());
            let token_b = alloy::primitives::Address::from_slice(leg.token_out.as_bytes());
            
            // Example: Uniswap V2 Mainnet Factory
            let uni_v2_factory = alloy::primitives::Address::from_slice(
                &hex::decode("5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f").unwrap()
            );

            let discovered_pool: Result<alloy::primitives::Address, shared_rs::rpc_failover::PoolError> = rpc.with_retry(|provider| {
                let factory_addr = uni_v2_factory;
                let token_a = token_a;
                let token_b = token_b;
                async move {
                    let call = IUniswapV2Factory::getPairCall {
                        tokenA: token_a,
                        tokenB: token_b,
                    };
                    use alloy_sol_types::SolCall;
                    use alloy::rpc::types::TransactionRequest;
                    let req = TransactionRequest::default().to(factory_addr).input(call.abi_encode().into());
                    
                    let result = provider.call(req).await.map_err(|e| anyhow::anyhow!("rpc error: {}", e))?;
                    let decoded_return = IUniswapV2Factory::getPairCall::abi_decode_returns(&result).map_err(|e| anyhow::anyhow!("decode error: {}", e))?;
                    let pair_addr = decoded_return;
                    if pair_addr.is_zero() {
                        anyhow::bail!("discovery_no_pool_found");
                    }
                    Ok(pair_addr)
                }
            }).await;

            match discovered_pool {
                Ok(pool_addr) => {
                    let e_pool = Address::from_slice(pool_addr.as_slice());
                    info!(
                        event = "pool_discovery.success",
                        chain_id,
                        pool_addr = ?e_pool,
                        "Discovered real on-chain pool"
                    );

                    self.record_observation(leg.token_in, leg.token_out, intent.router, intent.source_event, Some(e_pool)).await;
                    
                    let mut idx = self.impact_index.write().await;
                    idx.add_pool(PoolRef {
                        chain_id,
                        address: e_pool,
                        token0: std::cmp::min(leg.token_in, leg.token_out),
                        token1: std::cmp::max(leg.token_in, leg.token_out),
                        fee_bps: Some(30), // V2 0.3%
                        dex_name: "uniswap_v2".to_string(),
                        protocol_type: crate::route_intent::ProtocolType::V2,
                    });
                }
                Err(e) => {
                    warn!(
                        event = "pool_discovery.failed",
                        chain_id,
                        error = %e,
                        "discovery_no_pool_found"
                    );
                    self.record_observation(leg.token_in, leg.token_out, intent.router, intent.source_event, None).await;
                }
            }
        }

        Ok(())
    }

    async fn record_observation(
        &self,
        token0: Address,
        token1: Address,
        router: Address,
        source_event: DetectionSource,
        resolved_pool: Option<Address>,
    ) {
        if let Some(ref db) = self.db {
            let t0_str = format!("0x{:x}", token0);
            let t1_str = format!("0x{:x}", token1);
            let router_str = format!("0x{:x}", router);
            let source_str = source_event.as_str();
            let pool_str = resolved_pool.map(|a| format!("0x{:x}", a));
            let is_resolved = resolved_pool.is_some();

            let query = r#"
                INSERT INTO observed_unindexed_pairs 
                    (chain_id, token0_addr, token1_addr, router_addr, source_event, observation_count, is_resolved, resolved_pool_addr)
                VALUES ($1, $2, $3, $4, $5, 1, $6, $7)
                ON CONFLICT (chain_id, token0_addr, token1_addr, router_addr) 
                DO UPDATE SET 
                    observation_count = observed_unindexed_pairs.observation_count + 1,
                    last_seen_at = NOW(),
                    is_resolved = $6,
                    resolved_pool_addr = $7
            "#;

            if let Err(e) = sqlx::query(query)
                .bind(self.chain_id as i64)
                .bind(t0_str)
                .bind(t1_str)
                .bind(router_str)
                .bind(source_str)
                .bind(is_resolved)
                .bind(pool_str)
                .execute(db)
                .await
            {
                warn!("pool_discovery.db_insert_failed: {}", e);
            }
        }
    }
}
