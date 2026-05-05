//! Scanner loop — real mempool detection.
//!
//! Responsibilities:
//!   - Per enabled chain, resolve `RPC_WS_<chain_id>` from env.
//!   - If absent, stay idle with explicit state gauge + periodic warn log.
//!   - If present, open WS, subscribe to pending, dedup, decode, match patterns, persist+publish.
//!   - Honor kill-switch at every tick.
//!   - Reconnect with exponential backoff + jitter on WS errors.
//!
//! No fabricated data. No partial records. Every Opportunity that reaches DB
//! corresponds to a real pending tx observed on the wire.

use crate::{
    calldata, dedup::Dedup, patterns, persistence, publisher,
};
use crate::amm_math;
use crate::counters::counters;
use crate::reserves;
use std::sync::atomic::Ordering;
use ethers::providers::{Http, Provider};
use ethers::types::{Address, H256};
use futures_util::StreamExt;
use rand::Rng;
use std::str::FromStr;
use shared_rs::{
    chains::{self, RouterKind},
    config::AppConfig,
    killswitch::KillSwitchClient,
    metrics::OPPORTUNITIES_TOTAL,
    rpc_failover::{WsEndpoint, WsRpcPool},
    trading_config::TradingConfigClient,
};
use sqlx::postgres::PgPool;
use std::{sync::Arc, time::Duration};
use tracing::{debug, error, info, warn};
use prioritization_spine::types::{OpportunityCandidate};
use prioritization_spine::evidence::{OpportunityEvidence};
use prioritization_spine::scoring::{OpportunityScorer, PrioritizationEngine};
use prioritization_spine::gates::{can_execute};
use prioritization_spine::decision::{ExecutionDecision};
use prioritization_spine::simulator::EvmSimulator;
use prioritization_spine::config_aware::{ConfigAwareEvaluator, ConfigGateOutcome, NetworkSignals};
use std::fs::OpenOptions;
use std::io::Write;


use crate::chain_client::WsChainClient;

/// Mainnet QuoterV2 + Multicall3 addresses. V3 enrichment is mainnet-only for
/// now; multi-chain V3 lands in a future sub-project (each chain needs its own
/// per-chain lookup table). For other chains, `v3_provider` stays None and the
/// scanner falls through to V2-only enrichment.
const V3_QUOTER_V2_MAINNET: &str = "0x61fFE014bA17989E743c5F6cB21bF9697530B21e";
const V3_MULTICALL3_ADDR: &str = "0xcA11bde05977b3631167028862bE2a173976CA11";
const V3_QUOTE_CACHE_TTL_SECS: u64 = 5;

pub struct ScannerHandle {
    pub chain_id: u64,
}

pub async fn run_chain(
    chain_id: u64,
    cfg: Arc<AppConfig>,
    killswitch: KillSwitchClient,
    redis: redis::aio::ConnectionManager,
    db: Option<PgPool>,
    dedup: Arc<Dedup>,
    trading_config: TradingConfigClient,
    rpc_http_url: Option<String>,
) -> anyhow::Result<ScannerHandle> {
    // RPC failover discipline (G-RPC-1): build a multi-vendor pool from env.
    // CSV format `name=url,name=url`; bare URLs accepted for back-compat.
    let pool = match WsRpcPool::from_env(chain_id) {
        Ok(Some(p)) => p,
        Ok(None) => {
            warn!(
                event = "scanner.no_rpc",
                chain_id,
                env_key = format!("RPC_WS_{chain_id}"),
                "RPC_WS not configured; scanner stays idle for this chain (no detection, no fabrication)"
            );
            tokio::spawn(async move {
                idle_chain_loop(chain_id, killswitch).await;
            });
            return Ok(ScannerHandle { chain_id });
        }
        Err(e) => {
            error!(
                event = "scanner.rpc_pool_invalid",
                chain_id,
                error = %e,
                "RPC_WS env value did not parse — scanner idle"
            );
            tokio::spawn(async move {
                idle_chain_loop(chain_id, killswitch).await;
            });
            return Ok(ScannerHandle { chain_id });
        }
    };

    // Build the optional HTTP provider for V3 quoting (Multicall3 + QuoterV2).
    // V3 is mainnet-only for now; other chains fall through to V2-only enrichment.
    let v3_provider: Option<Arc<Provider<Http>>> = if chain_id == 1 {
        rpc_http_url.as_ref().and_then(|url| {
            match Provider::<Http>::try_from(url.clone()) {
                Ok(p) => {
                    info!(event = "scanner.v3_provider_ready", chain_id);
                    Some(Arc::new(p))
                }
                Err(e) => {
                    warn!(event = "scanner.v3_provider_init_failed", chain_id, error = %e);
                    None
                }
            }
        })
    } else {
        None
    };
    if v3_provider.is_none() {
        info!(
            event = "scanner.v3_disabled",
            chain_id,
            reason = if chain_id != 1 { "non-mainnet" } else { "no_rpc_http_url" },
        );
    }

    // Spawn the detection loop with the full endpoint list.
    tokio::spawn(detection_loop(
        chain_id, pool.endpoints, cfg, killswitch, redis, db, dedup, trading_config, v3_provider,
    ));
    Ok(ScannerHandle { chain_id })
}

async fn idle_chain_loop(chain_id: u64, killswitch: KillSwitchClient) {
    let mut ticker = tokio::time::interval(Duration::from_secs(60));
    loop {
        ticker.tick().await;
        let ks = killswitch.is_enabled().await;
        info!(
            event = "scanner.idle",
            chain_id,
            kill_switch = ks,
            "scanner is alive but RPC_WS_{chain_id} not set; no detection happening"
        );
    }
}

async fn detection_loop(
    chain_id: u64,
    endpoints: Vec<WsEndpoint>,
    _cfg: Arc<AppConfig>,
    killswitch: KillSwitchClient,
    mut redis: redis::aio::ConnectionManager,
    db: Option<PgPool>,
    dedup: Arc<Dedup>,
    trading_config: TradingConfigClient,
    v3_provider: Option<Arc<Provider<Http>>>,
) {
    let mut backoff_ms: u64 = 1000;
    let mut idx: usize = 0;
    loop {
        // The searcher-rs scanner runs continuously, even if the kill-switch is ARMED.
        // The kill-switch blocks execution downstream (relays-client), but the intelligence
        // layer always detects opportunities to populate the real-time dashboards.

        // Pick the next endpoint round-robin. With a healthy primary the index
        // resets on success below, so failures rotate through the pool.
        let endpoint = &endpoints[idx % endpoints.len()];
        let client = match WsChainClient::connect(chain_id, &endpoint.url).await {
            Ok(c) => {
                if idx != 0 {
                    info!(
                        event = "scanner.connected_via_backup",
                        chain_id,
                        provider = %endpoint.name,
                        "connected via backup WS provider after primary failures"
                    );
                }
                backoff_ms = 1000;
                idx = 0;
                c
            }
            Err(e) => {
                error!(
                    event = "scanner.connect_error",
                    chain_id,
                    provider = %endpoint.name,
                    error = %e,
                    "rotating to next WS provider"
                );
                idx = (idx + 1) % endpoints.len();
                if idx == 0 {
                    // Exhausted the ring — back off before another round.
                    sleep_with_backoff(&mut backoff_ms).await;
                }
                continue;
            }
        };

        if let Err(e) = run_subscription(&client, &killswitch, &mut redis, db.as_ref(), &dedup, &trading_config, v3_provider.as_ref()).await {
            error!(
                event = "scanner.subscription_error",
                chain_id,
                provider = %endpoint.name,
                error = %e
            );
            // Rotate on subscription death too — the WS connection died, try next.
            idx = (idx + 1) % endpoints.len();
            if idx == 0 {
                sleep_with_backoff(&mut backoff_ms).await;
            }
        }
    }
}

async fn sleep_with_backoff(backoff_ms: &mut u64) {
    let jitter: u64 = rand::thread_rng().gen_range(0..500);
    tokio::time::sleep(Duration::from_millis(*backoff_ms + jitter)).await;
    *backoff_ms = (*backoff_ms * 2).min(30_000);
}

async fn run_subscription(
    client: &WsChainClient,
    killswitch: &KillSwitchClient,
    redis: &mut redis::aio::ConnectionManager,
    db: Option<&PgPool>,
    dedup: &Dedup,
    trading_config: &TradingConfigClient,
    v3_provider: Option<&Arc<Provider<Http>>>,
) -> anyhow::Result<()> {
    let _ = killswitch; // reserved: kill-switch only blocks downstream execution
    let mut stream = client.subscribe_pending().await?;
    info!(event = "scanner.subscribed", chain_id = client.chain_id);

    while let Some(hash) = stream.next().await {
        // We no longer pause the scanner on kill-switch. It must always scan and emit.
        if let Err(e) = process_pending(client, hash, redis, db, dedup, trading_config, v3_provider).await {
            debug!(event = "scanner.process_err", hash = %hash, error = %e);
        }
    }
    anyhow::bail!("pending tx stream ended")
}

async fn process_pending(
    client: &WsChainClient,
    hash: H256,
    redis: &mut redis::aio::ConnectionManager,
    db: Option<&PgPool>,
    dedup: &Dedup,
    trading_config: &TradingConfigClient,
    v3_provider: Option<&Arc<Provider<Http>>>,
) -> anyhow::Result<()> {
    if !dedup.check_and_mark(hash, redis).await {
        return Ok(());
    }
    counters().pending_received.fetch_add(1, Ordering::Relaxed);
    let tx = match client.get_tx(hash).await? {
        Some(t) => t,
        None => return Ok(()), // dropped from mempool before we got it
    };
    let to = match tx.to {
        Some(a) => a,
        None => return Ok(()), // contract creation, ignore
    };
    let to_bytes: [u8; 20] = to.into();
    let router = match chains::find_router(client.chain_id, &to_bytes) {
        Some(r) => r,
        None => return Ok(()),
    };
    let decoded = match calldata::decode(&tx.input, router.kind) {
        Ok(d) => d,
        Err(reason) => {
            debug!(event = "scanner.decode_failed", reason = reason.as_str(), router = router.kind.as_str());
            return Ok(());
        }
    };
    if router.kind == RouterKind::Unknown {
        return Ok(());
    }
    counters().decoded_ok.fetch_add(1, Ordering::Relaxed);

    let ctx = patterns::TxContext {
        chain_id: client.chain_id,
        block_number: tx.block_number.map(|n| n.as_u64()),
        tx_from: tx.from.into(),
        tx_value: tx.value,
    };
    let opportunity = patterns::build_dex_arb_candidate(&ctx, &decoded);

    // --- CONFIG-AWARE SPINE INTERCEPTOR ---
    // Hot-reads operator's trading config from Redis (≤1s cache TTL). When no
    // config exists for this chain, the scanner OBSERVES but does not score —
    // dashboards see the detection but no fabricated profit numbers.
    let mut opportunity = opportunity;

    let cfg_opt = match trading_config.state(client.chain_id).await {
        Ok(opt) => opt,
        Err(e) => {
            warn!(event = "trading_config.read_failed", chain_id = client.chain_id, error = %e);
            None
        }
    };

    // Sub-proyecto-1 enrichment: lookup reserves + compute V2 amount_out + spread.
    // Order of operations: token meta → pool index → per-pool reserves → spread.
    // Each lookup tolerates miss with explicit log; net effect on a cold cache is
    // gross_profit=0 (same as before this sub-project) so behaviour degrades
    // gracefully when PoolSyncWorker hasn't ticked yet.
    let token_in_lower = opportunity.token_in.to_lowercase();
    let token_out_lower = opportunity.token_out.to_lowercase();
    let amount_in_wei_u256 = ethers::types::U256::from_dec_str(&opportunity.amount_in_wei)
        .unwrap_or_else(|_| ethers::types::U256::zero());

    let meta_in = reserves::get_token_meta(redis, client.chain_id, &token_in_lower).await.ok().flatten();
    let meta_out = reserves::get_token_meta(redis, client.chain_id, &token_out_lower).await.ok().flatten();

    // BUG-1 fix (2026-05-04): use the token's actual decimals when converting
    // amount_in_wei to f64 token units. The pre-fix code divided by 1e18
    // unconditionally, collapsing 6-decimal tokens (USDT, USDC) to ~0 in
    // f64 space and producing downstream ROI in the billions of percent
    // when composed with BUG-3 (now also fixed). Defaults to 18 only when
    // the token meta is unknown — preserving prior behaviour for unmapped
    // tokens while honouring real decimals for the curated allowlist.
    let amount_in_decimals: u8 = meta_in.as_ref().map(|m| m.decimals).unwrap_or(18);
    let amount_in_f64 = amm_math::wei_str_to_token_units(
        &opportunity.amount_in_wei,
        amount_in_decimals,
    );

    let mut expected_amount_out_f64 = amount_in_f64;
    let mut gross_profit_f64 = 0.0_f64;

    if let (Some(m_in), Some(m_out)) = (&meta_in, &meta_out) {
        // Read both V2 and V3 pool indexes for this pair. The two indexes are
        // independent (V2: just addresses; V3: address + fee_bps tuple) and
        // contain disjoint pools — see reserves.rs key layout doc.
        let pools_v2 = reserves::get_pools_for_pair(redis, client.chain_id, &m_in.symbol, &m_out.symbol)
            .await
            .unwrap_or_default();
        let pools_v3 = reserves::get_pools_for_pair_v3(redis, client.chain_id, &m_in.symbol, &m_out.symbol)
            .await
            .unwrap_or_default();
        let total_pools = pools_v2.len() + pools_v3.len();

        if total_pools < 2 {
            debug!(event = "scanner.single_pool_no_spread",
                   pair = format!("{}-{}", m_in.symbol, m_out.symbol),
                   v2 = pools_v2.len(), v3 = pools_v3.len());
        } else {
            let mut outs: Vec<ethers::types::U256> = Vec::with_capacity(total_pools);

            // ── V2 path ────────────────────────────────────────────────────
            // We don't know token0/token1 orientation from Redis alone (we'd
            // need a pools.token0_id JOIN — TODO: structural fix in next PR).
            // For now we compute BOTH orientations and apply a magnitude
            // heuristic: if the two outputs differ by >1e6x, one is wrong-side
            // saturation (amount_in dominates reserve_wrong_side, output
            // asymptotes to reserve_other). The SMALLER value is the realistic
            // trade. If they differ by less than 1e6x, both are plausible
            // and we take the larger (best execution price).
            //
            // The 1e6 threshold accounts for legit decimal-asymmetric swaps
            // (USDC 6dec → WETH 18dec ratios reach ~1e8 but never 1e15 in practice
            // for blue-chip pairs).
            for pool_addr in &pools_v2 {
                let entry = match reserves::get_reserves(redis, client.chain_id, pool_addr).await.ok().flatten() {
                    Some(e) => e,
                    None => continue,
                };
                let r0 = ethers::types::U256::from_dec_str(&entry.r0).unwrap_or_else(|_| ethers::types::U256::zero());
                let r1 = ethers::types::U256::from_dec_str(&entry.r1).unwrap_or_else(|_| ethers::types::U256::zero());
                let out_a = amm_math::v2_amount_out(amount_in_wei_u256, r0, r1, 30);
                let out_b = amm_math::v2_amount_out(amount_in_wei_u256, r1, r0, 30);

                let out = if out_a.is_zero() && out_b.is_zero() {
                    continue;
                } else if out_a.is_zero() {
                    out_b
                } else if out_b.is_zero() {
                    out_a
                } else {
                    let bigger = std::cmp::max(out_a, out_b);
                    let smaller = std::cmp::min(out_a, out_b);
                    let ratio_threshold = ethers::types::U256::from(1_000_000u64);
                    if bigger > smaller.saturating_mul(ratio_threshold) {
                        smaller // wrong-orientation saturation; correct is smaller
                    } else {
                        bigger // both plausible; best price wins
                    }
                };
                outs.push(out);
            }

            // ── V3 path ────────────────────────────────────────────────────
            // QuoterV2 takes (tokenIn, tokenOut, amountIn, fee, sqrtPriceLimitX96).
            // It knows direction, so a single call per pool is enough — no
            // dual-orientation hack needed. We cache results in Redis keyed by
            // (pool, amount_in_dec) with 5s TTL — same staleness window as V2
            // reserves. Cache hits skip the RPC; cache misses go through one
            // batched Multicall3 call covering all the misses.
            let mut v3_used = 0usize;
            if !pools_v3.is_empty() && !amount_in_wei_u256.is_zero() {
                let amount_in_dec = amount_in_wei_u256.to_string();

                // Resolve quotes from cache first.
                let mut cached_outs: Vec<ethers::types::U256> = Vec::new();
                let mut to_quote: Vec<amm_math::V3QuoteRequest> = Vec::new();

                for info in &pools_v3 {
                    if let Ok(Some(cached)) = reserves::get_v3_quote(
                        redis, client.chain_id, &info.pool_addr, &amount_in_dec,
                    ).await {
                        let val = ethers::types::U256::from_dec_str(&cached)
                            .unwrap_or_else(|_| ethers::types::U256::zero());
                        cached_outs.push(val);
                        continue;
                    }
                    // Build a Quoter request for cache-miss pools.
                    let pool_a = match Address::from_str(&info.pool_addr) { Ok(a) => a, Err(_) => continue };
                    let tin_a = match Address::from_str(&token_in_lower) { Ok(a) => a, Err(_) => continue };
                    let tout_a = match Address::from_str(&token_out_lower) { Ok(a) => a, Err(_) => continue };
                    to_quote.push(amm_math::V3QuoteRequest {
                        pool_addr: pool_a,
                        token_in: tin_a,
                        token_out: tout_a,
                        amount_in: amount_in_wei_u256,
                        fee_bps: info.fee_bps,
                    });
                }

                // Add cache hits to outs.
                for v in &cached_outs {
                    if !v.is_zero() {
                        outs.push(*v);
                        v3_used += 1;
                    }
                }

                // Batch the misses through Multicall3 if a provider is available.
                if !to_quote.is_empty() {
                    if let Some(provider) = v3_provider {
                        let quoter = Address::from_str(V3_QUOTER_V2_MAINNET).unwrap();
                        let multicall = Address::from_str(V3_MULTICALL3_ADDR).unwrap();
                        match amm_math::v3_quote_exact_in_multicall(
                            provider.clone(), quoter, multicall, to_quote.clone(),
                        ).await {
                            Ok(results) => {
                                for r in &results {
                                    if r.success && !r.amount_out.is_zero() {
                                        // Cache + push.
                                        let pool_lower = format!("0x{:040x}", r.pool_addr);
                                        let amount_out_dec = r.amount_out.to_string();
                                        let _ = reserves::set_v3_quote(
                                            redis, client.chain_id, &pool_lower,
                                            &amount_in_dec, &amount_out_dec,
                                            V3_QUOTE_CACHE_TTL_SECS,
                                        ).await;
                                        outs.push(r.amount_out);
                                        v3_used += 1;
                                    }
                                }
                            }
                            Err(e) => {
                                debug!(event = "scanner.v3_quote_rpc_failed",
                                       pair = format!("{}-{}", m_in.symbol, m_out.symbol),
                                       error = %e);
                            }
                        }
                    } else {
                        debug!(event = "scanner.v3_provider_unavailable",
                               pair = format!("{}-{}", m_in.symbol, m_out.symbol),
                               pending_quotes = to_quote.len());
                    }
                }
            }

            if outs.len() >= 2 {
                outs.sort();
                let lo = outs[0];
                let hi = outs[outs.len() - 1];
                let gross_profit_token_out = hi.saturating_sub(lo);
                let decimals_out = m_out.decimals as i32;
                let scale = 10f64.powi(decimals_out);
                expected_amount_out_f64 = u256_to_f64_lossy(hi) / scale;
                let spread_token_out_f64 = u256_to_f64_lossy(gross_profit_token_out) / scale;

                // USD pricing rules: only price in USD when token_out matches the
                // operator's base token (WETH) or is a known stablecoin. Otherwise
                // we surface the spread in token_out units but leave USD=0 with a
                // pending-oracle log — Sub-proyecto futuro adds a price oracle.
                gross_profit_f64 = if let Some(cfg_ref) = cfg_opt.as_ref() {
                    if m_out.symbol.eq_ignore_ascii_case(&cfg_ref.base_token_symbol) {
                        spread_token_out_f64 * cfg_ref.base_token_price_usd
                    } else if m_out.is_stablecoin {
                        spread_token_out_f64
                    } else {
                        debug!(event = "scanner.usd_conversion_pending_oracle",
                               token_out_symbol = %m_out.symbol);
                        0.0
                    }
                } else {
                    0.0
                };

                // Distinguish V2-only enrichment from V2+V3 in the event name so
                // dashboards can chart V3 reach growth without parsing fields.
                let event_name = if v3_used > 0 {
                    counters().enriched_v3.fetch_add(1, Ordering::Relaxed);
                    "scanner.candidate_enriched_v3"
                } else {
                    counters().enriched_v2.fetch_add(1, Ordering::Relaxed);
                    "scanner.candidate_enriched"
                };
                info!(event = event_name,
                      hash = %hash,
                      pair = format!("{}-{}", m_in.symbol, m_out.symbol),
                      pool_count = total_pools,
                      v2_pools = pools_v2.len(),
                      v3_pools = pools_v3.len(),
                      v3_quotes_landed = v3_used,
                      hi = %hi, lo = %lo,
                      spread_token_out = %gross_profit_token_out,
                      gross_profit_usd = gross_profit_f64);
            }
        }
    } else {
        debug!(event = "scanner.token_meta_unknown",
               token_in = %token_in_lower, token_out = %token_out_lower);
    }

    // The downstream gate (`ConfigAwareEvaluator`) checks
    // `cfg.allowed_token_symbols` against entries in `candidate.token_addresses`.
    // It compares STRINGS — so if we pass hex addresses while the operator's
    // allowlist holds symbols ("WETH", "USDC", ...), every check fails. Pass the
    // resolved symbol when the Redis token cache knew it, otherwise fall back to
    // the hex address (which will still fail the allowlist gate, but explicitly
    // — that's the correct semantics for an unknown token).
    let token_in_for_gate = meta_in
        .as_ref()
        .map(|m| m.symbol.clone())
        .unwrap_or_else(|| opportunity.token_in.clone());
    let token_out_for_gate = meta_out
        .as_ref()
        .map(|m| m.symbol.clone())
        .unwrap_or_else(|| opportunity.token_out.clone());

    let candidate = OpportunityCandidate {
        route_fingerprint: format!("{}_{}_{}", opportunity.dex_a, opportunity.token_in, opportunity.token_out),
        pool_addresses: vec![],
        token_addresses: vec![token_in_for_gate, token_out_for_gate],
        dex_adapters: vec![opportunity.dex_a.clone()],
        amount_in: amount_in_f64,
        expected_amount_out: expected_amount_out_f64,
        gross_profit: gross_profit_f64,
    };

    let Some(cfg) = cfg_opt else {
        // No operator config → observe-only path.
        info!(
            event = "scanner.no_trading_config",
            chain_id = client.chain_id,
            hash = %hash,
            "configure /config/trading to enable scoring; persisting raw observation"
        );
        counters().gate_no_config.fetch_add(1, Ordering::Relaxed);
        opportunity.roi_pct = None;
        opportunity.risk_score = None;
        if let Some(pool) = db {
            if let Err(e) = persistence::insert_opportunity(pool, &opportunity).await {
                counters().db_errors.fetch_add(1, Ordering::Relaxed);
                error!(event = "scanner.db_error", tx_hash = %hash, error = %e);
            } else {
                counters().db_persisted.fetch_add(1, Ordering::Relaxed);
            }
        }
        publisher::publish(redis, &opportunity).await?;
        OPPORTUNITIES_TOTAL
            .with_label_values(&[&opportunity.chain_id.to_string(), "dex_arb", "observed_no_config"])
            .inc();
        return Ok(());
    };

    if !cfg.enabled {
        debug!(event = "config.disabled", chain_id = client.chain_id, hash = %hash);
        return Ok(());
    }

    // Network signals — basefee/tip wiring lands with the chain-client refresh
    // (next sprint). Fixed gas strategy in config still works in the meantime.
    let signals = NetworkSignals::unknown(opportunity.block_number.unwrap_or(0));
    let evaluator = ConfigAwareEvaluator::new(&cfg, signals);

    // Strategy classification — when the calldata decoder grows multi-leg support,
    // this becomes router-driven. For now every observed swap is dex_arb_v2v2.
    let strategy_kind = "dex_arb_v2v2";

    let gate_outcome = evaluator.evaluate(
        &candidate,
        strategy_kind,
        client.chain_id,
        "rpc-pool".to_string(),
        cfg.gas_estimate_units.min(60_000), // proxy until rpc_latency tracked live
    );

    // ── Gate outcomes ────────────────────────────────────────────────────
    // Doctrine: silent early-returns on TokenNotAllowed / StrategyDisabled hide
    // detector activity from the operator (the dashboard then looks idle even
    // when 100s of pending txs/sec are filtered out). We persist these rows too
    // with risk_score=0 + roi_pct=0 so the operator sees rejection volume +
    // can iterate the allowlist with real evidence (RULE 00 transparency).
    let (mut final_evidence, math_outcome, config_rejection) = match gate_outcome {
        ConfigGateOutcome::TokenNotAllowed { token_symbol_or_addr } => {
            info!(
                event = "config.token_not_allowed",
                chain_id = client.chain_id,
                hash = %hash,
                token = %token_symbol_or_addr,
            );
            opportunity.expected_profit_usd = 0.0;
            opportunity.roi_pct = Some(0.0);
            opportunity.risk_score = Some(0.0);
            // GAP-2 fix: persist diagnostic reason — operator filters by
            // `rejection_reason` in the dashboard to count and audit allowlist gaps.
            opportunity.rejection_reason = Some(format!("TokenNotAllowed:{token_symbol_or_addr}"));
            counters().gate_token_not_allowed.fetch_add(1, Ordering::Relaxed);
            if let Some(pool) = db {
                if let Err(e) = persistence::insert_opportunity(pool, &opportunity).await {
                    counters().db_errors.fetch_add(1, Ordering::Relaxed);
                    error!(event = "scanner.db_error", tx_hash = %hash, error = %e);
                } else {
                    counters().db_persisted.fetch_add(1, Ordering::Relaxed);
                }
            }
            publisher::publish(redis, &opportunity).await?;
            OPPORTUNITIES_TOTAL
                .with_label_values(&[&opportunity.chain_id.to_string(), "dex_arb", "rejected_token_allowlist"])
                .inc();
            return Ok(());
        }
        ConfigGateOutcome::StrategyDisabled { strategy_kind } => {
            info!(
                event = "config.strategy_disabled",
                chain_id = client.chain_id,
                hash = %hash,
                strategy = %strategy_kind,
            );
            opportunity.expected_profit_usd = 0.0;
            opportunity.roi_pct = Some(0.0);
            opportunity.risk_score = Some(0.0);
            opportunity.rejection_reason = Some(format!("StrategyDisabled:{strategy_kind}"));
            counters().gate_strategy_disabled.fetch_add(1, Ordering::Relaxed);
            if let Some(pool) = db {
                if let Err(e) = persistence::insert_opportunity(pool, &opportunity).await {
                    counters().db_errors.fetch_add(1, Ordering::Relaxed);
                    error!(event = "scanner.db_error", tx_hash = %hash, error = %e);
                } else {
                    counters().db_persisted.fetch_add(1, Ordering::Relaxed);
                }
            }
            publisher::publish(redis, &opportunity).await?;
            OPPORTUNITIES_TOTAL
                .with_label_values(&[&opportunity.chain_id.to_string(), "dex_arb", "rejected_strategy_disabled"])
                .inc();
            return Ok(());
        }
        ConfigGateOutcome::Evaluated { outcome, evidence, rejection } => (evidence, outcome, rejection),
    };

    // REVM atomic sim gate (still a structural placeholder until lazy state
    // wires in — keeps the gate honest: simulator.rs returns "PASS" for empty
    // calldata so we don't reject the entire pipeline).
    let mut simulator = EvmSimulator::new(client.provider.clone());
    final_evidence.simulation_status = simulator.simulate_candidate(&candidate);

    // Connect math results to the persisted Opportunity row.
    opportunity.expected_profit_usd = math_outcome.gross_profit_usd;
    opportunity.roi_pct = Some(math_outcome.net_roi_pct);

    if let Some(reason) = config_rejection {
        info!(
            event = "config.gate_rejected",
            hash = %hash,
            reason = ?reason,
            net_profit_usd = math_outcome.net_profit_usd,
            roi_pct = math_outcome.net_roi_pct,
        );
        opportunity.risk_score = Some(0.0);
        // GAP-2 fix: persist the spine's diagnostic rejection reason
        // (UnknownTokenPrice / AnomalousMath / NegativeNetProfit / LowLiquidity / ...)
        // — converted to debug string so the operator dashboard can group + filter.
        let reason_str = format!("{reason:?}");
        opportunity.rejection_reason = Some(reason_str.clone());
        // Heartbeat counter — split the most diagnostic reasons into
        // dedicated buckets so the operator's per-minute summary surfaces
        // BUG-2-class issues (UnknownTokenPrice) and defense-in-depth
        // hits (AnomalousMath) separately from operational risk gates.
        if reason_str.starts_with("UnknownTokenPrice") {
            counters().gate_unknown_token_price.fetch_add(1, Ordering::Relaxed);
        } else if reason_str.starts_with("AnomalousMath") {
            counters().gate_anomalous_math.fetch_add(1, Ordering::Relaxed);
        } else {
            counters().gate_other_rejected.fetch_add(1, Ordering::Relaxed);
        }
    } else {
        counters().passed_all_gates.fetch_add(1, Ordering::Relaxed);
        // Spine scoring on REAL evidence (no more hardcoded 0.95 / 0.9 / 1.0).
        let engine = PrioritizationEngine { min_profit_threshold: cfg.min_profit_usd };
        match engine.score(&candidate, &final_evidence) {
            Ok(score) => {
                final_evidence.net_expected_profit = score.net_expected_profit;
                final_evidence.final_score = score.final_score;
                final_evidence.decision = can_execute(&final_evidence, true);
                opportunity.risk_score = Some(score.final_score);

                if let Ok(json) = serde_json::to_string(&final_evidence) {
                    let _ = std::fs::create_dir_all("logs/mev");
                    if let Ok(mut file) = OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open("logs/mev/opportunity_scored.jsonl")
                    {
                        let _ = writeln!(file, "{}", json);
                    }
                }

                if final_evidence.decision == ExecutionDecision::Reject {
                    info!(event = "spine.rejected", hash = %hash, reason = ?final_evidence.reject_reason);
                }
            }
            Err(e) => {
                debug!(event = "spine.scoring_error", hash = %hash, error = ?e);
                opportunity.risk_score = Some(0.0);
            }
        }
    }
    // --- END CONFIG-AWARE SPINE INTERCEPTOR ---


    // Persist + publish. Both are best-effort with their own error paths.
    if let Some(pool) = db {
        if let Err(e) = persistence::insert_opportunity(pool, &opportunity).await {
            counters().db_errors.fetch_add(1, Ordering::Relaxed);
            error!(event = "scanner.db_error", tx_hash = %hash, error = %e);
        } else {
            counters().db_persisted.fetch_add(1, Ordering::Relaxed);
        }
    }
    publisher::publish(redis, &opportunity).await?;

    OPPORTUNITIES_TOTAL
        .with_label_values(&[
            &opportunity.chain_id.to_string(),
            "dex_arb",
            "detected",
        ])
        .inc();

    Ok(())
}

fn u256_to_f64_lossy(v: ethers::types::U256) -> f64 {
    // U256 → f64 via decimal string. Loses precision past ~15 sig figs but
    // f64 is what OpportunityCandidate uses; this is a one-way display path,
    // never re-fed into on-chain arithmetic.
    v.to_string().parse::<f64>().unwrap_or(0.0)
}
