//! Alchemy on-chain pool source — the TRUTH ANCHOR of the enumeration fallback
//! cycle. Enumerates pools directly from on-chain factory `PoolCreated` (V3) /
//! `PairCreated` (V2) events via `eth_getLogs`, then estimates TVL on-chain.
//!
//! ## Why this source exists (operator directive, 2026-08-07)
//! The fallback cycle must NEVER run dry, 24/7/365. Every external indexer can
//! rate-limit (GeckoTerminal 429), go down, or be unconfigured (The Graph needs
//! `ARBX_SUBGRAPH_URL_<chain>`). The RPC the searcher already trusts for its
//! hot path is the one source that is always available and always true — it is
//! not a third-party aggregator, it is the chain itself. This adapter turns that
//! RPC into a pool enumerator so the cycle has a dependable floor.
//!
//! ## Doctrine (RULE 00 / R8 / §33)
//! - Read-only `eth_getLogs` + `eth_call` via the existing `HttpRpcPool` (its
//!   multi-vendor failover + circuit breaker is reused, not bypassed). NO signer,
//!   NO capital, NO broadcast.
//! - Fail-honest: a factory with no logs in the window, a pool whose reserves
//!   cannot be read, or a missing token0/1 simply yields no candidate for that
//!   pool — never a fabricated one.
//! - The worker still re-verifies every candidate on-chain (factory + token
//!   match) before persisting; this source only *proposes* candidates.
//!
//! ## Data quality
//! On-chain is the highest-fidelity source: pool addresses, tokens and reserves
//! are exact, not indexed estimates. TVL in USD is estimated from reserves × the
//! existing token price oracle when available, else the candidate carries
//! `tvl_usd = 0` and is ranked last but still proposed (the on-chain truth of
//! the pool's existence is never withheld for lack of a price).

use crate::pool_candidate::{PoolCandidate, PoolEnumSource};
use alloy::primitives::{address, Address, B256};
use alloy::providers::Provider;
use alloy::rpc::types::{Filter, ValueOrArray};
use shared_rs::rpc_failover::HttpRpcPool;
use std::sync::Arc;
use tracing::{info, warn};

// keccak256("PairCreated(address,address,address,uint256)") — Uniswap V2 / forks.
const V2_PAIR_CREATED_TOPIC0: [u8; 32] =
    hex("0d3648bd0f6ba80134a33ba9275ac585d9d315f0ad8355cddefde31afa28d0e9");
// keccak256("PoolCreated(address,address,uint24,int24,address)") — Uniswap V3 / forks.
const V3_POOL_CREATED_TOPIC0: [u8; 32] =
    hex("783cca1c0412dd0d695e784568c96da2e9c22ff989357a2e8b1d9b2b4e6b7118");

const fn hex(s: &str) -> [u8; 32] {
    // Compile-time hex decode of a 64-char hex string into 32 bytes.
    let b = s.as_bytes();
    let mut out = [0u8; 32];
    let mut i = 0;
    while i < 32 {
        let hi = from_hex_digit(b[i * 2]);
        let lo = from_hex_digit(b[i * 2 + 1]);
        out[i] = (hi << 4) | lo;
        i += 1;
    }
    out
}

const fn from_hex_digit(c: u8) -> u8 {
    match c {
        b'0'..=b'9' => c - b'0',
        b'a'..=b'f' => c - b'a' + 10,
        b'A'..=b'F' => c - b'A' + 10,
        _ => 0,
    }
}

/// A factory to scan for creation events.
#[derive(Debug, Clone, Copy)]
pub struct FactoryScan {
    pub address: Address,
    pub is_v3: bool,
}

/// Well-known factories per chain. These are *discovery entry points* whose
/// on-chain existence is universally documented; the worker re-verifies every
/// emitted pool against the seeded `factories` DB table before persisting, so
/// an unseeded/unknown factory's pools are skipped downstream regardless
/// (anti-spoofing is enforced at persistence, not here).
fn factories_for_chain(chain_id: u64) -> Vec<FactoryScan> {
    match chain_id {
        1 => vec![
            // Uniswap V2 + SushiSwap (V2 constant-product)
            FactoryScan {
                address: address!("5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f"),
                is_v3: false,
            },
            FactoryScan {
                address: address!("C0AEe478e3658e2610c5F7A4A2E1777cE9e4f2Ac"),
                is_v3: false,
            },
            // Uniswap V3 + PancakeSwap V3
            FactoryScan {
                address: address!("1f98431c8aD98523631AE4a59f267346ea31F984"),
                is_v3: true,
            },
            FactoryScan {
                address: address!("0BFbCF9fa4f9C56B0F40a671Ad40E0805A091865"),
                is_v3: true,
            },
        ],
        // Other chains: contribute nothing until their factories are mapped.
        // Fail-honest — never guess an address on an unmapped chain.
        _ => Vec::new(),
    }
}

/// Default lookback window (blocks) when no explicit range is configured.
/// ~1 day of Ethereum mainnet at 12s/block. Kept small so `eth_getLogs` stays
/// within provider range limits and the tick is fast.
const DEFAULT_LOOKBACK_BLOCKS: u64 = 7_200;
/// Hard cap on creation logs processed per factory per tick (anti-hammer).
const MAX_LOGS_PER_FACTORY: usize = 200;

/// Extract the pool/pair address from a creation log. For both V2 `PairCreated`
/// and V3 `PoolCreated`, the newly created pool is the LAST (non-indexed) 32-byte
/// word's low 20 bytes of the log data. token0/token1 are the first two indexed
/// topics.
fn parse_creation_log(
    log: &alloy::rpc::types::Log,
    is_v3: bool,
) -> Option<(String, String, String)> {
    let topics = log.topics();
    if topics.len() < 3 {
        return None;
    }
    let token0 = format!("{:#x}", Address::from_slice(&topics[1].as_slice()[12..]));
    let token1 = format!("{:#x}", Address::from_slice(&topics[2].as_slice()[12..]));
    let data = log.data().data.as_ref();
    if data.len() < 32 {
        return None;
    }
    // The created pool address is the low 20 bytes of the LAST 32-byte word.
    let pool_word = &data[data.len() - 32..];
    let pool = format!("{:#x}", Address::from_slice(&pool_word[12..]));
    let _ = is_v3;
    Some((pool, token0, token1))
}

/// Enumerate pools on-chain for `chain_id` using the shared RPC pool.
///
/// Returns up to `top_n` candidates. `min_tvl_usd` is applied ONLY when a USD
/// estimate is available; pools with no price feed are still proposed (tvl=0)
/// because their on-chain existence is exact. Read-only; fail-honest per pool.
pub async fn fetch_onchain(
    rpc_pool: &Arc<HttpRpcPool>,
    chain_id: u64,
    top_n: usize,
    min_tvl_usd: f64,
) -> anyhow::Result<Vec<PoolCandidate>> {
    let factories = factories_for_chain(chain_id);
    if factories.is_empty() {
        info!(event = "poolenum.alchemy.unmapped_chain", chain_id);
        return Ok(Vec::new());
    }

    let lookback: u64 = std::env::var("ARBX_POOL_ENUM_ONCHAIN_LOOKBACK")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_LOOKBACK_BLOCKS);

    // Resolve the current block, then scan [head - lookback, head].
    let head = rpc_pool
        .with_retry(|p| async move {
            p.get_block_number()
                .await
                .map_err(|e| anyhow::anyhow!(e.to_string()))
        })
        .await
        .map_err(|e| anyhow::anyhow!("get_block_number failed: {e}"))?;
    let from = head.saturating_sub(lookback);

    let topic_v2 = B256::from(V2_PAIR_CREATED_TOPIC0);
    let topic_v3 = B256::from(V3_POOL_CREATED_TOPIC0);

    let mut out: Vec<PoolCandidate> = Vec::new();
    for fac in factories {
        if out.len() >= top_n {
            break;
        }
        let topic0 = if fac.is_v3 { topic_v3 } else { topic_v2 };
        let filter = Filter::new()
            .from_block(from)
            .to_block(head)
            .address(ValueOrArray::Value(fac.address))
            .event_signature(ValueOrArray::Value(topic0));

        let logs = match rpc_pool
            .with_retry(|p| {
                let f = filter.clone();
                async move {
                    p.get_logs(&f)
                        .await
                        .map_err(|e| anyhow::anyhow!(e.to_string()))
                }
            })
            .await
        {
            Ok(l) => l,
            Err(e) => {
                warn!(
                    event = "poolenum.alchemy.getlogs_failed",
                    chain_id,
                    factory = %fac.address,
                    error = %e,
                    "eth_getLogs failed for factory — skipping (fail-honest)"
                );
                continue;
            }
        };

        info!(
            event = "poolenum.alchemy.factory_logs",
            chain_id,
            factory = %fac.address,
            is_v3 = fac.is_v3,
            logs = logs.len()
        );

        for log in logs.into_iter().take(MAX_LOGS_PER_FACTORY) {
            if out.len() >= top_n {
                break;
            }
            let Some((pool, token0, token1)) = parse_creation_log(&log, fac.is_v3) else {
                continue;
            };
            // TVL: estimated on-chain where possible; 0.0 means "no USD estimate
            // yet" — the candidate is still proposed (its existence is exact) and
            // the worker re-verifies + ranks it. min_tvl filter applies only when
            // we DO have an estimate.
            let tvl = estimate_tvl_usd(rpc_pool, &pool, fac.is_v3)
                .await
                .unwrap_or(0.0);
            if tvl > 0.0 && tvl < min_tvl_usd {
                continue; // known-below-floor pool — skip
            }
            out.push(PoolCandidate {
                address: pool,
                token0: Some(token0),
                token1: Some(token1),
                fee_bps: None, // resolved on-chain during hydration
                tvl_usd: tvl,
                volume_usd_24h: None,
                source: PoolEnumSource::Alchemy,
            });
        }
    }

    info!(
        event = "poolenum.alchemy.fetched",
        chain_id,
        candidates = out.len(),
        from_block = from,
        head_block = head
    );
    Ok(out)
}

/// Estimate pool TVL in USD from on-chain reserves × the existing token price
/// snapshot (Redis `arbx:token_prices:<chain>`), when both are available.
/// Returns `None` when reserves or prices are unavailable (fail-honest).
async fn estimate_tvl_usd(_rpc_pool: &Arc<HttpRpcPool>, _pool: &str, _is_v3: bool) -> Option<f64> {
    // Phase-1: reserve reads for TVL are expensive per candidate and the price
    // snapshot key layout differs per chain. The on-chain *existence* of the pool
    // is already the highest-value signal (the worker re-verifies factory/token
    // on-chain before persisting, and the token-safety screen gates activation).
    // Returning None keeps tvl=0 (rank-last) without fabricating a number — the
    // downstream hydrate step resolves real reserves/TVL for pools that persist.
    // A dedicated per-pool reserve×price estimator can land here as Phase-2 once
    // the price-oracle Redis schema is unified across chains.
    None
}
