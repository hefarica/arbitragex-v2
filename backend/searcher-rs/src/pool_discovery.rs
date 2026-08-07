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
use uuid::Uuid;

use crate::impact_index::{ImpactIndex, PoolRef};
use crate::route_intent::{DetectionSource, RouteIntent};
use shared_rs::rpc_failover::HttpRpcPool;

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
        function factory() external view returns (address);
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

#[allow(clippy::type_complexity)]
pub struct PoolDiscoveryService {
    chain_id: u64,
    db: Option<PgPool>,
    redis: redis::aio::ConnectionManager,
    impact_index: Arc<RwLock<ImpactIndex>>,
    rpc_pool: Option<Arc<HttpRpcPool>>,
    reserves_cache: Arc<crate::engines::triangular_engine::ReservesCache>,
    factories: RwLock<Option<Vec<(Uuid, Address, crate::route_intent::ProtocolType, String)>>>,
}

/// Outcome of a single Block-2 enumeration attempt — fail-honest reasons, never
/// fabricated. Surfaced to the enumeration worker for per-tick metrics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnumOutcome {
    /// Pool persisted. `activated` = token-safe → is_active=true + added to impact_index.
    Persisted { activated: bool },
    /// The pool's on-chain factory is not in the seeded `factories` table — skipped.
    FactoryUnresolved,
    /// On-chain hydration/verification failed (token mismatch, meta, or RPC).
    HydrationFailed,
    /// No RPC pool configured — cannot resolve the factory on-chain.
    NoRpc,
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

    /// Shared RPC pool accessor for read-only enumerators (e.g. the Alchemy
    /// on-chain pool source). Returns `None` when no RPC is configured (the
    /// enumerator then fails honest — it never fabricates pools without RPC).
    pub fn rpc_pool(&self) -> Option<Arc<HttpRpcPool>> {
        self.rpc_pool.clone()
    }

    async fn get_factories(
        &self,
    ) -> Vec<(Uuid, Address, crate::route_intent::ProtocolType, String)> {
        let mut cache = self.factories.write().await;
        if let Some(ref facts) = *cache {
            return facts.clone();
        }

        let mut facts = Vec::new();
        if let Some(ref db) = self.db {
            let q = "SELECT f.id, f.address, d.protocol_type, d.name FROM factories f JOIN dexes d ON f.dex_id = d.id WHERE f.chain_id = $1 AND f.is_active = TRUE";
            if let Ok(rows) = sqlx::query_as::<_, (Uuid, String, String, String)>(q)
                .bind(self.chain_id as i64)
                .fetch_all(db)
                .await
            {
                for (f_id, addr_str, proto_str, name) in rows {
                    if let Ok(addr) = addr_str.parse::<Address>() {
                        let proto = match proto_str.as_str() {
                            "UNISWAP_V2" | "V2" | "uniswap_v2" | "UniswapV2" => {
                                crate::route_intent::ProtocolType::V2
                            }
                            "UNISWAP_V3" | "V3" | "uniswap_v3" | "UniswapV3" => {
                                crate::route_intent::ProtocolType::V3
                            }
                            _ => continue,
                        };
                        facts.push((f_id, addr, proto, name));
                    }
                }
            }
        }

        if facts.is_empty() {
            warn!("discovery_failed:no_factories_configured");
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

            let mut discovered_pools: Vec<(
                Uuid,
                alloy::primitives::Address,
                crate::route_intent::ProtocolType,
                String,
                Option<u32>,
            )> = Vec::new();

            for (f_id, factory_addr, proto, dex_name) in &factories {
                match proto {
                    crate::route_intent::ProtocolType::V2 => {
                        let f_addr_alloy =
                            alloy::primitives::Address::from_slice(factory_addr.as_bytes());
                        let discovered: Result<alloy::primitives::Address, _> = rpc
                            .with_retry(|provider| {
                                let t_a = token_a;
                                let t_b = token_b;
                                async move {
                                    let call = IUniswapV2Factory::getPairCall {
                                        tokenA: t_a,
                                        tokenB: t_b,
                                    };
                                    use alloy::rpc::types::TransactionRequest;
                                    use alloy_sol_types::SolCall;
                                    let req = TransactionRequest::default()
                                        .to(f_addr_alloy)
                                        .input(call.abi_encode().into());

                                    let result = provider
                                        .call(req)
                                        .await
                                        .map_err(|e| anyhow::anyhow!("rpc error: {}", e))?;
                                    let decoded_return =
                                        IUniswapV2Factory::getPairCall::abi_decode_returns(&result)
                                            .map_err(|e| anyhow::anyhow!("decode error: {}", e))?;
                                    if decoded_return.is_zero() {
                                        anyhow::bail!("discovery_no_pool_found");
                                    }
                                    Ok(decoded_return)
                                }
                            })
                            .await;
                        if let Ok(pool_addr) = discovered {
                            discovered_pools.push((
                                *f_id,
                                pool_addr,
                                *proto,
                                dex_name.clone(),
                                Some(30),
                            ));
                        }
                    }
                    crate::route_intent::ProtocolType::V3 => {
                        let f_addr_alloy =
                            alloy::primitives::Address::from_slice(factory_addr.as_bytes());
                        let fees: [u32; 4] = [100, 500, 3000, 10000];
                        for fee in fees {
                            let discovered: Result<alloy::primitives::Address, _> = rpc
                                .with_retry(|provider| {
                                    let t_a = token_a;
                                    let t_b = token_b;
                                    async move {
                                        let fee_u24 = alloy::primitives::Uint::<24, 1>::from(fee);
                                        let call = IUniswapV3Factory::getPoolCall {
                                            tokenA: t_a,
                                            tokenB: t_b,
                                            fee: fee_u24,
                                        };
                                        use alloy::rpc::types::TransactionRequest;
                                        use alloy_sol_types::SolCall;
                                        let req = TransactionRequest::default()
                                            .to(f_addr_alloy)
                                            .input(call.abi_encode().into());

                                        let result = provider
                                            .call(req)
                                            .await
                                            .map_err(|e| anyhow::anyhow!("rpc error: {}", e))?;
                                        let decoded_return =
                                            IUniswapV3Factory::getPoolCall::abi_decode_returns(
                                                &result,
                                            )
                                            .map_err(|e| anyhow::anyhow!("decode error: {}", e))?;
                                        if decoded_return.is_zero() {
                                            anyhow::bail!("discovery_no_pool_found");
                                        }
                                        Ok(decoded_return)
                                    }
                                })
                                .await;
                            if let Ok(pool_addr) = discovered {
                                discovered_pools.push((
                                    *f_id,
                                    pool_addr,
                                    *proto,
                                    dex_name.clone(),
                                    Some(fee),
                                ));
                            }
                        }
                    }
                    _ => {}
                }
            }

            if !discovered_pools.is_empty() {
                for (f_id, pool_addr, proto, dex_name, fee_raw) in discovered_pools {
                    let e_pool = Address::from_slice(pool_addr.as_slice());
                    let fee_bps = fee_raw.map(|f| match proto {
                        crate::route_intent::ProtocolType::V3 => f / 100,
                        _ => f,
                    });

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

                    self.record_observation(
                        leg.token_in,
                        leg.token_out,
                        intent.router,
                        intent.source_event,
                        Some(e_pool),
                    )
                    .await;
                    if let Ok(pool_ref) = self
                        .hydrate_and_persist_pool(
                            rpc.clone(),
                            pool_addr,
                            f_id,
                            proto,
                            &dex_name,
                            fee_bps,
                            token_a,
                            token_b,
                            true,
                            "reactive",
                        )
                        .await
                    {
                        let mut idx = self.impact_index.write().await;
                        idx.add_pool(pool_ref);
                        drop(idx);
                        info!(event = "pool_discovery.impact_index_refreshed");
                        discovered_any = true;
                    } else {
                        warn!("pool_discovery.hydration_failed");
                    }
                }
            } else {
                warn!(
                    event = "pool_discovery.failed",
                    chain_id, "discovery_no_pool_found"
                );
                self.record_observation(
                    leg.token_in,
                    leg.token_out,
                    intent.router,
                    intent.source_event,
                    None,
                )
                .await;
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

    #[allow(clippy::too_many_arguments)]
    async fn hydrate_and_persist_pool(
        &self,
        rpc: Arc<HttpRpcPool>,
        pool_addr: alloy::primitives::Address,
        factory_id: Uuid,
        proto: crate::route_intent::ProtocolType,
        dex_name: &str,
        fee_bps: Option<u32>,
        intent_t_a: alloy::primitives::Address,
        intent_t_b: alloy::primitives::Address,
        is_active: bool,
        enum_source: &str,
    ) -> anyhow::Result<PoolRef> {
        let (token0, token1) = rpc
            .with_retry(|provider| {
                let p_addr = pool_addr;
                async move {
                    use alloy::rpc::types::TransactionRequest;
                    use alloy_sol_types::SolCall;

                    let req0 = TransactionRequest::default()
                        .to(p_addr)
                        .input(IUniswapV2Pair::token0Call {}.abi_encode().into());
                    let res0 = provider
                        .call(req0)
                        .await
                        .map_err(|e| anyhow::anyhow!("token0 rpc error: {}", e))?;
                    let t0 = IUniswapV2Pair::token0Call::abi_decode_returns(&res0)
                        .map_err(|e| anyhow::anyhow!("token0 decode error: {}", e))?
                        .0;

                    let req1 = TransactionRequest::default()
                        .to(p_addr)
                        .input(IUniswapV2Pair::token1Call {}.abi_encode().into());
                    let res1 = provider
                        .call(req1)
                        .await
                        .map_err(|e| anyhow::anyhow!("token1 rpc error: {}", e))?;
                    let t1 = IUniswapV2Pair::token1Call::abi_decode_returns(&res1)
                        .map_err(|e| anyhow::anyhow!("token1 decode error: {}", e))?
                        .0;

                    Ok((t0, t1))
                }
            })
            .await?;

        if (token0.as_slice() != intent_t_a.as_slice()
            && token0.as_slice() != intent_t_b.as_slice())
            || (token1.as_slice() != intent_t_a.as_slice()
                && token1.as_slice() != intent_t_b.as_slice())
        {
            warn!(
                "discovery_failed:token_pair_mismatch (intent: {:?}/{:?}, pool: {:?}/{:?})",
                intent_t_a, intent_t_b, token0, token1
            );
            anyhow::bail!("token_pair_mismatch");
        }

        // Extract metadata for token0
        let (sym0, dec0) = match self
            .fetch_token_meta(
                &rpc,
                alloy::primitives::Address::from_slice(token0.as_slice()),
            )
            .await
        {
            Ok(meta) => meta,
            Err(e) => {
                warn!("token_meta_unavailable for {:?}: {}", token0, e);
                anyhow::bail!("token_meta_unavailable");
            }
        };
        let (sym1, dec1) = match self
            .fetch_token_meta(
                &rpc,
                alloy::primitives::Address::from_slice(token1.as_slice()),
            )
            .await
        {
            Ok(meta) => meta,
            Err(e) => {
                warn!("token_meta_unavailable for {:?}: {}", token1, e);
                anyhow::bail!("token_meta_unavailable");
            }
        };

        let e_t0 = Address::from_slice(token0.as_slice());
        let e_t1 = Address::from_slice(token1.as_slice());
        let e_pool = Address::from_slice(pool_addr.as_slice());

        // Upsert tokens in DB (is_active mirrors the pool's activate verdict).
        let token0_id = self
            .upsert_token_in_db(e_t0, &sym0, dec0, is_active)
            .await?;
        let token1_id = self
            .upsert_token_in_db(e_t1, &sym1, dec1, is_active)
            .await?;

        // Upsert pool in DB — `?`-propagate so a failed write yields Err (the
        // caller reports HydrationFailed and does NOT add to impact_index).
        self.upsert_pool_in_db(
            e_pool,
            factory_id,
            token0_id,
            token1_id,
            fee_bps,
            is_active,
            enum_source,
        )
        .await?;

        // Hydrate ReservesCache
        match proto {
            crate::route_intent::ProtocolType::V2 => {
                let reserves = rpc
                    .with_retry(|provider| {
                        let p_addr = pool_addr;
                        async move {
                            use alloy::rpc::types::TransactionRequest;
                            use alloy_sol_types::SolCall;
                            let req = TransactionRequest::default()
                                .to(p_addr)
                                .input(IUniswapV2Pair::getReservesCall {}.abi_encode().into());
                            let res = provider
                                .call(req)
                                .await
                                .map_err(|e| anyhow::anyhow!("getReserves rpc error: {}", e))?;
                            let decoded = IUniswapV2Pair::getReservesCall::abi_decode_returns(&res)
                                .map_err(|e| anyhow::anyhow!("getReserves decode error: {}", e))?;
                            Ok((decoded.reserve0, decoded.reserve1))
                        }
                    })
                    .await?;
                // We insert directly into the cache.
                self.reserves_cache
                    .insert(
                        e_pool,
                        ethers::types::U256::from_str_radix(&reserves.0.to_string(), 10)
                            .unwrap_or_default(),
                        ethers::types::U256::from_str_radix(&reserves.1.to_string(), 10)
                            .unwrap_or_default(),
                    )
                    .await;
            }
            crate::route_intent::ProtocolType::V3 => {
                // Read slot0 + liquidity
                let (_sqrt_price, _tick, _liq) = rpc
                    .with_retry(|provider| {
                        let p_addr = pool_addr;
                        async move {
                            use alloy::rpc::types::TransactionRequest;
                            use alloy_sol_types::SolCall;
                            let req_s0 = TransactionRequest::default()
                                .to(p_addr)
                                .input(IUniswapV3Pool::slot0Call {}.abi_encode().into());
                            let res_s0 = provider
                                .call(req_s0)
                                .await
                                .map_err(|e| anyhow::anyhow!("slot0 rpc error: {}", e))?;
                            let slot0 = IUniswapV3Pool::slot0Call::abi_decode_returns(&res_s0)
                                .map_err(|e| anyhow::anyhow!("slot0 decode error: {}", e))?;

                            let req_l = TransactionRequest::default()
                                .to(p_addr)
                                .input(IUniswapV3Pool::liquidityCall {}.abi_encode().into());
                            let res_l = provider
                                .call(req_l)
                                .await
                                .map_err(|e| anyhow::anyhow!("liquidity rpc error: {}", e))?;
                            let liq = IUniswapV3Pool::liquidityCall::abi_decode_returns(&res_l)
                                .map_err(|e| anyhow::anyhow!("liquidity decode error: {}", e))?;
                            Ok((slot0.sqrtPriceX96, slot0.tick, liq))
                        }
                    })
                    .await?;

                // We set v3 state via the cache
                // Note: the reserves cache doesn't fully support V3 slots directly yet without Phase 15 state projector,
                // but we insert what we can or wait for the V3 indexer.
                // For now, fail-honest: we just fetched it successfully to prove it exists.
            }
            _ => {}
        }

        // Update Redis pool_index
        let mut redis_conn = self.redis.clone();

        match proto {
            crate::route_intent::ProtocolType::V2 => {
                let key = format!(
                    "arbx:pool_index:{}:{}:{}",
                    self.chain_id,
                    sym0.to_lowercase(),
                    sym1.to_lowercase()
                );
                let raw_val: Option<String> = redis::cmd("GET")
                    .arg(&key)
                    .query_async(&mut redis_conn)
                    .await
                    .unwrap_or(None);
                let mut list: Vec<String> = raw_val
                    .and_then(|v| serde_json::from_str(&v).ok())
                    .unwrap_or_default();
                let addr_str = format!("0x{:x}", e_pool);
                if !list.contains(&addr_str) {
                    list.push(addr_str);
                    if let Ok(json) = serde_json::to_string(&list) {
                        let _ = redis::cmd("SET")
                            .arg(&key)
                            .arg(&json)
                            .query_async::<_, ()>(&mut redis_conn)
                            .await;
                        info!(event="pool_discovery.redis_index_updated", key=?key);
                    }
                }
            }
            crate::route_intent::ProtocolType::V3 => {
                let key = format!(
                    "arbx:pool_index_v3:{}:{}:{}",
                    self.chain_id,
                    sym0.to_lowercase(),
                    sym1.to_lowercase()
                );
                let raw_val: Option<String> = redis::cmd("GET")
                    .arg(&key)
                    .query_async(&mut redis_conn)
                    .await
                    .unwrap_or(None);

                #[derive(serde::Serialize, serde::Deserialize, PartialEq)]
                struct V3PoolInfo {
                    address: String,
                    fee_bps: u32,
                }

                let mut list: Vec<V3PoolInfo> = raw_val
                    .and_then(|v| serde_json::from_str(&v).ok())
                    .unwrap_or_default();
                let addr_str = format!("0x{:x}", e_pool);
                let fee = fee_bps.unwrap_or(30);

                if !list.iter().any(|p| p.address == addr_str) {
                    list.push(V3PoolInfo {
                        address: addr_str,
                        fee_bps: fee,
                    });
                    if let Ok(json) = serde_json::to_string(&list) {
                        let _ = redis::cmd("SET")
                            .arg(&key)
                            .arg(&json)
                            .query_async::<_, ()>(&mut redis_conn)
                            .await;
                        info!(event="pool_discovery.redis_v3_index_updated", key=?key);
                    }
                }
            }
            _ => {}
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

    async fn fetch_token_meta(
        &self,
        rpc: &Arc<HttpRpcPool>,
        token: alloy::primitives::Address,
    ) -> anyhow::Result<(String, u8)> {
        rpc.with_retry(|provider| {
            let t_addr = token;
            async move {
                use alloy::rpc::types::TransactionRequest;
                use alloy_sol_types::SolCall;
                let req_s = TransactionRequest::default()
                    .to(t_addr)
                    .input(IERC20::symbolCall {}.abi_encode().into());
                let res_s = provider
                    .call(req_s)
                    .await
                    .map_err(|e| anyhow::anyhow!("symbol rpc error: {}", e))?;
                let sym = IERC20::symbolCall::abi_decode_returns(&res_s)
                    .map_err(|e| anyhow::anyhow!("symbol decode error: {}", e))?;

                let req_d = TransactionRequest::default()
                    .to(t_addr)
                    .input(IERC20::decimalsCall {}.abi_encode().into());
                let res_d = provider
                    .call(req_d)
                    .await
                    .map_err(|e| anyhow::anyhow!("decimals rpc error: {}", e))?;
                let dec = IERC20::decimalsCall::abi_decode_returns(&res_d)
                    .map_err(|e| anyhow::anyhow!("decimals decode error: {}", e))?;

                Ok((sym, dec))
            }
        })
        .await
        .map_err(|e| anyhow::anyhow!("token meta rpc error: {}", e))
    }

    async fn upsert_token_in_db(
        &self,
        token: Address,
        symbol: &str,
        decimals: u8,
        is_active: bool,
    ) -> anyhow::Result<Uuid> {
        if let Some(ref db) = self.db {
            let t_str = format!("0x{:x}", token);
            let q_sel = "SELECT id FROM tokens WHERE chain_id = $1 AND address = $2";
            if let Ok(Some((id,))) = sqlx::query_as::<_, (Uuid,)>(q_sel)
                .bind(self.chain_id as i64)
                .bind(&t_str)
                .fetch_optional(db)
                .await
            {
                return Ok(id);
            }

            // Hardened (2026-08-03, anti-corruption): include resolved_via (column is
            // NOT NULL since 034/072; omitting it produced NULL-crashes and ghost rows).
            // Anti-symbol-contamination: if a DIFFERENT address already owns this symbol
            // in defi_registry (the operator-curated Single Source of Truth), NULL the
            // discovered symbol so it cannot poison bare-symbol lookups. This is exactly
            // how the bogus duplicate DAI (0x446480cd…, decimals=9), PEPE and SHIB rows
            // were created — a fake/scam token squatting a blue-chip symbol. The natural
            // key stays (chain_id, address); only the symbol label is neutralized.
            // ON CONFLICT no longer overwrites good symbol/decimals with possibly-stale
            // discovery data (COALESCE keeps the existing value).
            let q_ins = r#"
                INSERT INTO tokens (chain_id, address, symbol, decimals, is_active, resolved_via, created_at)
                SELECT $1, $2,
                       CASE WHEN EXISTS (
                              SELECT 1 FROM defi_registry
                              WHERE chain_id = $1 AND symbol = $3 AND is_active
                                AND address <> $2
                            )
                            THEN NULL
                            ELSE $3
                       END,
                       $4, $5, 'onchain_full', NOW()
                ON CONFLICT (chain_id, address) DO UPDATE SET
                    decimals = COALESCE(tokens.decimals, EXCLUDED.decimals),
                    is_active = tokens.is_active OR EXCLUDED.is_active,
                    last_seen_at = NOW()
                RETURNING id
            "#;
            if let Ok((id,)) = sqlx::query_as::<_, (Uuid,)>(q_ins)
                .bind(self.chain_id as i64)
                .bind(&t_str)
                .bind(symbol)
                .bind(decimals as i16)
                .bind(is_active)
                .fetch_one(db)
                .await
            {
                return Ok(id);
            }
        }
        anyhow::bail!("db_unavailable_or_insert_failed")
    }

    #[allow(clippy::too_many_arguments)]
    async fn upsert_pool_in_db(
        &self,
        pool: Address,
        factory_id: Uuid,
        token0_id: Uuid,
        token1_id: Uuid,
        fee_bps: Option<u32>,
        is_active: bool,
        enum_source: &str,
    ) -> anyhow::Result<()> {
        // No DB configured → nothing to persist (preserves the prior no-op).
        let Some(ref db) = self.db else {
            return Ok(());
        };
        let p_str = format!("0x{:x}", pool);
        // is_active is MONOTONIC on conflict (`pools.is_active OR EXCLUDED`):
        // never deactivate a pool that is already active (honors "don't break
        // existing pools"); a false→true reactivation IS allowed (an unrated
        // token becoming token-safe). enum_source keeps its original provenance.
        let q = r#"
            INSERT INTO pools (chain_id, address, factory_id, token0_id, token1_id, fee_tier, is_active, enum_source, created_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, NOW())
            ON CONFLICT (chain_id, address) DO UPDATE SET
                factory_id = EXCLUDED.factory_id,
                token0_id = EXCLUDED.token0_id,
                token1_id = EXCLUDED.token1_id,
                fee_tier = EXCLUDED.fee_tier,
                is_active = pools.is_active OR EXCLUDED.is_active,
                enum_source = COALESCE(pools.enum_source, EXCLUDED.enum_source)
        "#;
        // Propagate the error (fail-honest): a swallowed failure would let the
        // caller report the pool as persisted + inject it into the live
        // impact_index with no PG backing. On Err the caller yields
        // HydrationFailed and does NOT add the pool.
        sqlx::query(q)
            .bind(self.chain_id as i64)
            .bind(&p_str)
            .bind(factory_id)
            .bind(token0_id)
            .bind(token1_id)
            .bind(fee_bps.map(|f| f as i32))
            .bind(is_active)
            .bind(enum_source)
            .execute(db)
            .await
            .map_err(|e| {
                warn!(event = "pool_discovery.upsert_pool_failed", pool = ?p_str, error = %e);
                anyhow::anyhow!("upsert_pool_failed: {e}")
            })?;
        info!(event = "pool_discovery.pool_persisted", pool = ?p_str, is_active, enum_source);
        Ok(())
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

    /// Block 2 entrypoint: enumerate a candidate pool discovered by TVL ranking.
    ///
    /// Resolves the pool's factory ON-CHAIN (never trusts the subgraph), looks it
    /// up in the seeded `factories` cache, and — if found — re-verifies + persists
    /// the pool via the existing on-chain hydration path. `activate` controls
    /// is_active + impact_index membership: a token-safe pool is activated; an
    /// unrated one is persisted is_active=false so a future tick can reactivate it.
    ///
    /// Fail-honest: factory-not-seeded or hydration failure returns the reason and
    /// logs it — it NEVER seeds a factory blindly or fabricates a pool.
    pub async fn enumerate_and_persist_pool(
        &self,
        pool_addr: alloy::primitives::Address,
        token0_hint: alloy::primitives::Address,
        token1_hint: alloy::primitives::Address,
        fee_bps: Option<u32>,
        activate: bool,
    ) -> EnumOutcome {
        let rpc = match &self.rpc_pool {
            Some(p) => p.clone(),
            None => return EnumOutcome::NoRpc,
        };

        // 1. Resolve the factory ON-CHAIN (factory() exists on V2 pairs + V3 pools).
        let factory_addr = match self.read_pool_factory(&rpc, pool_addr).await {
            Ok(a) => a,
            Err(e) => {
                warn!(event = "poolenum.factory_read_failed", pool = ?pool_addr, error = %e);
                return EnumOutcome::HydrationFailed;
            }
        };

        // 2. Lookup factory.id (Uuid) in the seeded cache by address. NOT seeded → skip.
        //    Never seed a factory blindly (R8): seeding is a separate migration decision.
        let factories = self.get_factories().await;
        let fac_addr_e = Address::from_slice(factory_addr.as_slice());
        let matched = factories.iter().find(|(_, addr, _, _)| *addr == fac_addr_e);
        let (factory_id, proto, dex_name) = match matched {
            Some((id, _, proto, name)) => (*id, *proto, name.clone()),
            None => {
                info!(event = "poolenum.factory_unresolved", pool = ?pool_addr, factory = ?fac_addr_e);
                return EnumOutcome::FactoryUnresolved;
            }
        };

        // 3. On-chain hydrate + persist (re-verifies token0/1, upserts tokens+pool,
        //    refreshes the Redis index). is_active = activate; enum_source tracked.
        match self
            .hydrate_and_persist_pool(
                rpc.clone(),
                pool_addr,
                factory_id,
                proto,
                &dex_name,
                fee_bps,
                token0_hint,
                token1_hint,
                activate,
                "subgraph_tvl",
            )
            .await
        {
            Ok(pool_ref) => {
                // Only an activated (token-safe) pool joins the live search surface.
                if activate {
                    let mut idx = self.impact_index.write().await;
                    idx.add_pool(pool_ref);
                }
                info!(event = "poolenum.pool_persisted", pool = ?pool_addr, activated = activate);
                EnumOutcome::Persisted {
                    activated: activate,
                }
            }
            Err(e) => {
                warn!(event = "poolenum.hydration_failed", pool = ?pool_addr, error = %e);
                EnumOutcome::HydrationFailed
            }
        }
    }

    /// Read the pool/pair's `factory()` view on-chain (V2 pairs and V3 pools both
    /// expose it). Used by Block 2 enumeration to resolve provenance without
    /// trusting the subgraph. Errors on a zero/failed read (fail-honest).
    async fn read_pool_factory(
        &self,
        rpc: &Arc<HttpRpcPool>,
        pool_addr: alloy::primitives::Address,
    ) -> anyhow::Result<alloy::primitives::Address> {
        let factory = rpc
            .with_retry(|provider| {
                let p_addr = pool_addr;
                async move {
                    use alloy::rpc::types::TransactionRequest;
                    use alloy_sol_types::SolCall;
                    let req = TransactionRequest::default()
                        .to(p_addr)
                        .input(IUniswapV2Pair::factoryCall {}.abi_encode().into());
                    let res = provider
                        .call(req)
                        .await
                        .map_err(|e| anyhow::anyhow!("factory rpc error: {}", e))?;
                    let factory = IUniswapV2Pair::factoryCall::abi_decode_returns(&res)
                        .map_err(|e| anyhow::anyhow!("factory decode error: {}", e))?;
                    if factory.is_zero() {
                        anyhow::bail!("factory_zero");
                    }
                    Ok(factory)
                }
            })
            .await?;
        Ok(factory)
    }
}
