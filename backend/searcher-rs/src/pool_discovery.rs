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

    #[derive(Debug)]
    interface IUniswapV2Pair {
        function getReserves() external view returns (uint112 reserve0, uint112 reserve1, uint32 blockTimestampLast);
        function token0() external view returns (address);
        function token1() external view returns (address);
    }

    #[derive(Debug)]
    interface IUniswapV3Pool {
        function slot0() external view returns (uint160 sqrtPriceX96, int24 tick, uint16 observationIndex, uint16 observationCardinality, uint16 observationCardinalityNext, uint8 feeProtocol, bool unlocked);
        function liquidity() external view returns (uint128);
        function token0() external view returns (address);
        function token1() external view returns (address);
        function fee() external view returns (uint24);
    }

    #[derive(Debug)]
    interface IERC20 {
        function symbol() external view returns (string);
        function decimals() external view returns (uint8);
    }
}

pub struct PoolDiscoveryService {
    chain_id: u64,
    db: Option<PgPool>,
    redis: redis::aio::ConnectionManager,
    impact_index: Arc<RwLock<ImpactIndex>>,
    rpc_pool: Option<Arc<HttpRpcPool>>,
    reserves_cache: Arc<crate::engines::triangular_engine::ReservesCache>,
    factories: RwLock<Option<Vec<(Address, crate::route_intent::ProtocolType, String)>>>,
}

impl PoolDiscoveryService {
    pub fn new(
        chain_id: u64,
        db: Option<PgPool>,
        redis: redis::aio::ConnectionManager,
        impact_index: Arc<RwLock<ImpactIndex>>,
        rpc_pool: Option<Arc<HttpRpcPool>>,
        reserves_cache: Arc<crate::engines::triangular_engine::ReservesCache>,
    ) -> Self {
        Self {
            chain_id,
            db,
            redis,
            impact_index,
            rpc_pool,
            reserves_cache,
            factories: RwLock::new(None),
        }
    }

    async fn get_factories(&self) -> Vec<(Address, crate::route_intent::ProtocolType, String)> {
        let mut cache = self.factories.write().await;
        if let Some(ref facts) = *cache {
            return facts.clone();
        }
        
        let mut facts = Vec::new();
        if let Some(ref db) = self.db {
            let q = "SELECT f.address, d.protocol_type, d.name FROM factories f JOIN dexes d ON f.dex_id = d.id WHERE f.chain_id = $1";
            if let Ok(rows) = sqlx::query_as::<_, (String, String, String)>(q)
                .bind(self.chain_id as i64)
                .fetch_all(db)
                .await
            {
                for (addr_str, proto_str, name) in rows {
                    if let Ok(addr) = addr_str.parse::<Address>() {
                        let proto = match proto_str.as_str() {
                            "UNISWAP_V2" => crate::route_intent::ProtocolType::V2,
                            "UNISWAP_V3" => crate::route_intent::ProtocolType::V3,
                            _ => continue,
                        };
                        facts.push((addr, proto, name));
                    }
                }
            }
        }
        
        // Fallback for empty DB scenario
        if facts.is_empty() {
            facts.push((
                Address::from_slice(&hex::decode("5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f").unwrap()),
                crate::route_intent::ProtocolType::V2,
                "uniswap_v2_fallback".to_string(),
            ));
        }
        
        *cache = Some(facts.clone());
        facts
    }

    /// Attempts to discover missing pools for a RouteIntent using real RPC.
    pub async fn discover_from_intent(&self, intent: &RouteIntent) -> anyhow::Result<bool> {
        let chain_id = self.chain_id;
        let mut discovered_any = false;

        let factories = self.get_factories().await;

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

            let token_a = alloy::primitives::Address::from_slice(leg.token_in.as_bytes());
            let token_b = alloy::primitives::Address::from_slice(leg.token_out.as_bytes());
            
            let mut found_pool: Option<(alloy::primitives::Address, crate::route_intent::ProtocolType, String, Option<u32>)> = None;

            for (factory_addr, proto, dex_name) in &factories {
                if found_pool.is_some() {
                    break;
                }

                match proto {
                    crate::route_intent::ProtocolType::V2 => {
                        let f_addr = *factory_addr;
                        let f_addr_alloy = alloy::primitives::Address::from_slice(factory_addr.as_bytes());
                        let discovered: Result<alloy::primitives::Address, _> = rpc.with_retry(|provider| {
                            let t_a = token_a;
                            let t_b = token_b;
                            async move {
                                let call = IUniswapV2Factory::getPairCall { tokenA: t_a, tokenB: t_b };
                                use alloy_sol_types::SolCall;
                                use alloy::rpc::types::TransactionRequest;
                                let req = TransactionRequest::default().to(f_addr_alloy).input(call.abi_encode().into());
                                
                                let result = provider.call(req).await.map_err(|e| anyhow::anyhow!("rpc error: {}", e))?;
                                let decoded_return = IUniswapV2Factory::getPairCall::abi_decode_returns(&result).map_err(|e| anyhow::anyhow!("decode error: {}", e))?;
                                if decoded_return.is_zero() {
                                    anyhow::bail!("discovery_no_pool_found");
                                }
                                Ok(decoded_return)
                            }
                        }).await;
                        if let Ok(pool_addr) = discovered {
                            found_pool = Some((pool_addr, *proto, dex_name.clone(), Some(30)));
                        }
                    }
                    crate::route_intent::ProtocolType::V3 => {
                        let f_addr_alloy = alloy::primitives::Address::from_slice(factory_addr.as_bytes());
                        let fees: [u32; 4] = [100, 500, 3000, 10000];
                        for fee in fees {
                            let discovered: Result<alloy::primitives::Address, _> = rpc.with_retry(|provider| {
                                let t_a = token_a;
                                let t_b = token_b;
                                async move {
                                    let fee_u24 = alloy::primitives::Uint::<24, 1>::from(fee);
                                    let call = IUniswapV3Factory::getPoolCall { tokenA: t_a, tokenB: t_b, fee: fee_u24 };
                                    use alloy_sol_types::SolCall;
                                    use alloy::rpc::types::TransactionRequest;
                                    let req = TransactionRequest::default().to(f_addr_alloy).input(call.abi_encode().into());
                                    
                                    let result = provider.call(req).await.map_err(|e| anyhow::anyhow!("rpc error: {}", e))?;
                                    let decoded_return = IUniswapV3Factory::getPoolCall::abi_decode_returns(&result).map_err(|e| anyhow::anyhow!("decode error: {}", e))?;
                                    if decoded_return.is_zero() {
                                        anyhow::bail!("discovery_no_pool_found");
                                    }
                                    Ok(decoded_return)
                                }
                            }).await;
                            if let Ok(pool_addr) = discovered {
                                found_pool = Some((pool_addr, *proto, dex_name.clone(), Some(fee)));
                                break;
                            }
                        }
                    }
                    _ => {}
                }
            }

            match found_pool {
                Some((pool_addr, proto, dex_name, fee_bps)) => {
                    let e_pool = Address::from_slice(pool_addr.as_slice());
                    info!(
                        event = match proto {
                            crate::route_intent::ProtocolType::V2 => "pool_discovery.v2.result",
                            crate::route_intent::ProtocolType::V3 => "pool_discovery.v3.result",
                            _ => "pool_discovery.unknown.result"
                        },
                        chain_id,
                        pool_addr = ?e_pool,
                        protocol = ?proto,
                        "status" = "inserted",
                        "Discovered real on-chain pool"
                    );

                    self.record_observation(leg.token_in, leg.token_out, intent.router, intent.source_event, Some(e_pool)).await;
                    
                    if let Ok(pool_ref) = self.hydrate_and_persist_pool(rpc.clone(), pool_addr, proto, &dex_name, fee_bps).await {
                        let mut idx = self.impact_index.write().await;
                        idx.add_pool(pool_ref);
                        drop(idx);
                        info!("pool_discovery.impact_index_refreshed");
                        discovered_any = true;
                    } else {
                        warn!("pool_discovery.hydration_failed");
                    }
                }
                None => {
                    warn!(
                        event = "pool_discovery.failed",
                        chain_id,
                        "discovery_no_pool_found"
                    );
                    self.record_observation(leg.token_in, leg.token_out, intent.router, intent.source_event, None).await;
                }
            }
        }

        Ok(discovered_any)
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

    async fn hydrate_and_persist_pool(
        &self,
        rpc: Arc<HttpRpcPool>,
        pool_addr: alloy::primitives::Address,
        proto: crate::route_intent::ProtocolType,
        dex_name: &str,
        fee_bps: Option<u32>,
    ) -> anyhow::Result<PoolRef> {
        let (token0, token1) = rpc.with_retry(|provider| {
            let p_addr = pool_addr;
            async move {
                use alloy_sol_types::SolCall;
                use alloy::rpc::types::TransactionRequest;
                
                let req0 = TransactionRequest::default().to(p_addr).input(IUniswapV2Pair::token0Call {}.abi_encode().into());
                let res0 = provider.call(req0).await.map_err(|e| anyhow::anyhow!("token0 rpc error: {}", e))?;
                let t0 = IUniswapV2Pair::token0Call::abi_decode_returns(&res0).map_err(|e| anyhow::anyhow!("token0 decode error: {}", e))?.0;

                let req1 = TransactionRequest::default().to(p_addr).input(IUniswapV2Pair::token1Call {}.abi_encode().into());
                let res1 = provider.call(req1).await.map_err(|e| anyhow::anyhow!("token1 rpc error: {}", e))?;
                let t1 = IUniswapV2Pair::token1Call::abi_decode_returns(&res1).map_err(|e| anyhow::anyhow!("token1 decode error: {}", e))?.0;
                
                Ok((t0, t1))
            }
        }).await?;

        // Extract metadata for token0
        let (sym0, dec0) = self.fetch_token_meta(&rpc, alloy::primitives::Address::from_slice(token0.as_slice())).await.unwrap_or_else(|_| ("UNKNOWN".to_string(), 18));
        let (sym1, dec1) = self.fetch_token_meta(&rpc, alloy::primitives::Address::from_slice(token1.as_slice())).await.unwrap_or_else(|_| ("UNKNOWN".to_string(), 18));

        let e_t0 = Address::from_slice(token0.as_slice());
        let e_t1 = Address::from_slice(token1.as_slice());
        let e_pool = Address::from_slice(pool_addr.as_slice());

        // Upsert tokens in DB
        self.upsert_token_in_db(e_t0, &sym0, dec0).await;
        self.upsert_token_in_db(e_t1, &sym1, dec1).await;

        // Upsert pool in DB
        self.upsert_pool_in_db(e_pool, e_t0, e_t1, fee_bps).await;

        // Hydrate ReservesCache
        match proto {
            crate::route_intent::ProtocolType::V2 => {
                let reserves = rpc.with_retry(|provider| {
                    let p_addr = pool_addr;
                    async move {
                        use alloy_sol_types::SolCall;
                        use alloy::rpc::types::TransactionRequest;
                        let req = TransactionRequest::default().to(p_addr).input(IUniswapV2Pair::getReservesCall {}.abi_encode().into());
                        let res = provider.call(req).await.map_err(|e| anyhow::anyhow!("getReserves rpc error: {}", e))?;
                        let decoded = IUniswapV2Pair::getReservesCall::abi_decode_returns(&res).map_err(|e| anyhow::anyhow!("getReserves decode error: {}", e))?;
                        Ok((decoded.reserve0, decoded.reserve1))
                    }
                }).await?;
                // We insert directly into the cache.
                self.reserves_cache.insert(
                    e_pool,
                    ethers::types::U256::from_str_radix(&reserves.0.to_string(), 10).unwrap_or_default(),
                    ethers::types::U256::from_str_radix(&reserves.1.to_string(), 10).unwrap_or_default()
                ).await;
            }
            crate::route_intent::ProtocolType::V3 => {
                // Read slot0 + liquidity
                let (sqrt_price, tick, liq) = rpc.with_retry(|provider| {
                    let p_addr = pool_addr;
                    async move {
                        use alloy_sol_types::SolCall;
                        use alloy::rpc::types::TransactionRequest;
                        let req_s0 = TransactionRequest::default().to(p_addr).input(IUniswapV3Pool::slot0Call {}.abi_encode().into());
                        let res_s0 = provider.call(req_s0).await.map_err(|e| anyhow::anyhow!("slot0 rpc error: {}", e))?;
                        let slot0 = IUniswapV3Pool::slot0Call::abi_decode_returns(&res_s0).map_err(|e| anyhow::anyhow!("slot0 decode error: {}", e))?;

                        let req_l = TransactionRequest::default().to(p_addr).input(IUniswapV3Pool::liquidityCall {}.abi_encode().into());
                        let res_l = provider.call(req_l).await.map_err(|e| anyhow::anyhow!("liquidity rpc error: {}", e))?;
                        let liq = IUniswapV3Pool::liquidityCall::abi_decode_returns(&res_l).map_err(|e| anyhow::anyhow!("liquidity decode error: {}", e))?;
                        Ok((slot0.sqrtPriceX96, slot0.tick, liq))
                    }
                }).await?;
                
                // We set v3 state via the cache
                // Note: the reserves cache doesn't fully support V3 slots directly yet without Phase 15 state projector,
                // but we insert what we can or wait for the V3 indexer.
                // For now, fail-honest: we just fetched it successfully to prove it exists.
            }
            _ => {}
        }

        // Update Redis pool_index
        let mut redis_conn = self.redis.clone();
        let key = format!("arbx:pool_index:{}:{}:{}", self.chain_id, sym0.to_lowercase(), sym1.to_lowercase());
        let val = format!("0x{:x}", e_pool);
        if let Err(e) = redis::cmd("SADD").arg(&key).arg(&val).query_async::<_, ()>(&mut redis_conn).await {
            warn!("Failed to SADD to redis {}: {}", key, e);
        } else {
            info!("pool_discovery.redis_index_updated");
        }

        Ok(PoolRef {
            chain_id: self.chain_id,
            address: e_pool,
            token0: std::cmp::min(e_t0, e_t1),
            token1: std::cmp::max(e_t0, e_t1),
            fee_bps,
            dex_name: dex_name.to_string(),
            protocol_type: proto,
        })
    }

    async fn fetch_token_meta(&self, rpc: &Arc<HttpRpcPool>, token: alloy::primitives::Address) -> anyhow::Result<(String, u8)> {
        rpc.with_retry(|provider| {
            let t_addr = token;
            async move {
                use alloy_sol_types::SolCall;
                use alloy::rpc::types::TransactionRequest;
                let req_s = TransactionRequest::default().to(t_addr).input(IERC20::symbolCall {}.abi_encode().into());
                let res_s = provider.call(req_s).await.map_err(|e| anyhow::anyhow!("symbol rpc error: {}", e))?;
                let sym = IERC20::symbolCall::abi_decode_returns(&res_s).map_err(|e| anyhow::anyhow!("symbol decode error: {}", e))?;

                let req_d = TransactionRequest::default().to(t_addr).input(IERC20::decimalsCall {}.abi_encode().into());
                let res_d = provider.call(req_d).await.map_err(|e| anyhow::anyhow!("decimals rpc error: {}", e))?;
                let dec = IERC20::decimalsCall::abi_decode_returns(&res_d).map_err(|e| anyhow::anyhow!("decimals decode error: {}", e))?;
                
                Ok((sym, dec))
            }
        }).await.map_err(|e| anyhow::anyhow!("token meta rpc error: {}", e))
    }

    async fn upsert_token_in_db(&self, token: Address, symbol: &str, decimals: u8) {
        if let Some(ref db) = self.db {
            let t_str = format!("0x{:x}", token);
            let q = r#"
                INSERT INTO tokens (chain_id, address, symbol, decimals, resolved_via, resolved_at, last_seen_at)
                VALUES ($1, $2, $3, $4, 'onchain_full', NOW(), NOW())
                ON CONFLICT (chain_id, address) DO UPDATE SET 
                    last_seen_at = NOW(),
                    symbol = EXCLUDED.symbol,
                    decimals = EXCLUDED.decimals
            "#;
            if let Err(e) = sqlx::query(q)
                .bind(self.chain_id as i64)
                .bind(t_str)
                .bind(symbol)
                .bind(decimals as i16)
                .execute(db)
                .await
            {
                warn!("pool_discovery.upsert_token_failed: {}", e);
            }
        }
    }

    async fn upsert_pool_in_db(&self, pool: Address, _token0: Address, _token1: Address, _fee_bps: Option<u32>) {
        if let Some(ref db) = self.db {
            let p_str = format!("0x{:x}", pool);
            // We do a simplified insert just to have the pool. Factory id is missing here but address is recorded.
            let q = r#"
                INSERT INTO pools (chain_id, address, is_active, created_at)
                VALUES ($1, $2, TRUE, NOW())
                ON CONFLICT (chain_id, address) DO NOTHING
            "#;
            if let Err(e) = sqlx::query(q)
                .bind(self.chain_id as i64)
                .bind(p_str)
                .execute(db)
                .await
            {
                warn!("pool_discovery.upsert_pool_failed: {}", e);
            } else {
                info!("pool_discovery.pool_persisted");
            }
        }
    }

    pub async fn record_opportunity_observation(
        &self,
        intent: &crate::route_intent::RouteIntent,
        rejection_reason: &str,
        estimated_profit_usd: Option<f64>,
        estimated_gas_usd: Option<f64>,
    ) {
        if let Some(ref db) = self.db {
            let tx_hash = format!("0x{:x}", intent.tx_hash);
            let q = r#"
                INSERT INTO opportunity_observations (
                    chain_id, strategy_kind, tx_hash, discovered_pool_addrs, 
                    estimated_profit_usd, estimated_gas_usd, is_viable, rejection_reason
                )
                VALUES ($1, $2, $3, '[]', $4, $5, FALSE, $6)
            "#;
            if let Err(e) = sqlx::query(q)
                .bind(self.chain_id as i64)
                .bind("discovery")
                .bind(&tx_hash)
                .bind(estimated_profit_usd)
                .bind(estimated_gas_usd)
                .bind(rejection_reason)
                .execute(db)
                .await
            {
                tracing::debug!("Failed to insert opportunity observation: {}", e);
            }
        }
    }
}
