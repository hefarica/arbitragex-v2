//! FASE OMEGA — Block/log backrunning scanner (`ARBX_MEMPOOL_MODE=block`).
//!
//! For the free-RPC posture (publicnode/drpc/lava) that does NOT support
//! `alchemy_pendingTransactions`, this replaces pending-tx detection with
//! **confirmed-state** detection:
//!
//!   newHeads (eth_subscribe) → per block: eth_getLogs for Uniswap V2 `Swap` events
//!   on watched pools → decode → `RouteIntent` (DetectionSource::NewBlock) →
//!   `Orchestrator::on_route_intent` (which runs the cartridge shadow eval).
//!
//! This is backrun-style: we react AFTER inclusion (no mempool front-run edge), but
//! it works on $0 RPCs and produces real intents from confirmed pool-state changes.
//!
//! Transport: everything here uses the **ethers** `Provider<Ws>` (same connection for
//! `subscribe_blocks` and `get_logs`) so all types stay ethers-native and bridge
//! cleanly into `RouteIntent`. The alloy `HttpRpcPool` is intentionally NOT used.
//!
//! Scope (iter): V2 `Swap` only (the common case). V3 `Swap` decoding is a follow-up.
//! Gated entirely behind `MempoolMode::Block`; the existing pending-tx `detection_loop`
//! is untouched and is simply not spawned in this mode.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use ethers::providers::{Middleware, Provider, StreamExt, Ws};
use ethers::types::{Address, Filter, U256};
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

use crate::chain_client::WsChainClient;
use crate::impact_index::ImpactIndex;
use crate::orchestrator::Orchestrator;
use crate::route_intent::{
    DetectionSource, ProtocolType, RouteIntent, RouteIntentLeg, RouterKind, SwapExactMode,
};

/// Uniswap-V2 `Swap` event signature. `Filter::event()` derives topic0 = keccak256(sig).
const V2_SWAP_SIG: &str = "Swap(address,uint256,uint256,uint256,uint256,address)";

/// Backrunning detection loop: subscribe to new blocks and turn confirmed V2 swaps on
/// watched pools into `RouteIntent`s fed to the orchestrator. Long-running; honors
/// `cancel`. Reconnects on WS error with exponential backoff. Never panics.
pub async fn block_detection_loop(
    chain_id: u64,
    ws_urls: Vec<String>,
    orchestrator: Option<Arc<Orchestrator>>,
    impact_index: Option<Arc<RwLock<ImpactIndex>>>,
    cancel: CancellationToken,
) {
    let (orch, idx) = match (orchestrator, impact_index) {
        (Some(o), Some(i)) => (o, i),
        _ => {
            warn!(
                event = "block_scanner.disabled",
                chain_id,
                "block mode requires an orchestrator + impact index (orch_mode must be v2/shadow); idling"
            );
            cancel.cancelled().await;
            return;
        }
    };
    if ws_urls.is_empty() {
        warn!(event = "block_scanner.no_ws", chain_id, "no WS endpoints for block mode; idling");
        cancel.cancelled().await;
        return;
    }

    info!(
        event = "block_scanner.start",
        chain_id,
        endpoints = ws_urls.len(),
        "block/log backrunning scanner starting"
    );

    let mut backoff_ms: u64 = 1_000;
    let max_backoff_ms: u64 = 30_000;
    let mut url_idx = 0usize;

    loop {
        if cancel.is_cancelled() {
            return;
        }
        let url = &ws_urls[url_idx % ws_urls.len()];
        match run_block_subscription(chain_id, url, &orch, &idx, &cancel).await {
            Ok(()) => return, // clean exit (cancelled)
            Err(e) => {
                warn!(
                    event = "block_scanner.reconnect",
                    chain_id,
                    error = %e,
                    backoff_ms,
                    next_endpoint = (url_idx + 1) % ws_urls.len(),
                    "block subscription dropped; rotating endpoint + backing off"
                );
                url_idx = url_idx.wrapping_add(1);
                tokio::time::sleep(Duration::from_millis(backoff_ms)).await;
                backoff_ms = (backoff_ms * 2).min(max_backoff_ms);
            }
        }
    }
}

/// One WS connection: subscribe to blocks, process each. Returns `Ok(())` on
/// cancellation, `Err` on disconnect (caller reconnects).
async fn run_block_subscription(
    chain_id: u64,
    url: &str,
    orch: &Arc<Orchestrator>,
    idx: &Arc<RwLock<ImpactIndex>>,
    cancel: &CancellationToken,
) -> anyhow::Result<()> {
    let client = WsChainClient::connect(chain_id, url).await?;
    let provider = client.provider.clone();
    let mut blocks = client.subscribe_blocks().await?;
    info!(event = "block_scanner.connected", chain_id, "subscribed to newHeads");

    loop {
        tokio::select! {
            _ = cancel.cancelled() => return Ok(()),
            blk = blocks.next() => {
                let Some(block) = blk else {
                    return Err(anyhow::anyhow!("newHeads stream ended"));
                };
                let Some(block_num) = block.number else { continue };
                process_block(chain_id, block_num.as_u64(), &provider, orch, idx).await;
            }
        }
    }
}

/// Fetch V2 Swap logs for watched pools at `block_num`, decode them into RouteIntents,
/// and feed the orchestrator. Errors are logged, never propagated.
async fn process_block(
    chain_id: u64,
    block_num: u64,
    provider: &Provider<Ws>,
    orch: &Arc<Orchestrator>,
    idx: &Arc<RwLock<ImpactIndex>>,
) {
    // Snapshot watched V2 pools (clone out, release the read lock quickly).
    let v2_pools: HashMap<Address, (Address, Address)> = {
        let guard = idx.read().await;
        guard
            .all_pools()
            .into_iter()
            .filter(|p| p.protocol_type == ProtocolType::V2)
            .map(|p| (p.address, (p.token0, p.token1)))
            .collect()
    };
    if v2_pools.is_empty() {
        debug!(event = "block_scanner.no_watched_pools", chain_id, block_num);
        return;
    }

    let mut intents = 0u32;
    let mut watched_swaps = 0u32;

    // Many free RPCs BLOCK address-filtered eth_getLogs ("blocked parameter:
    // params.0.address"). Query topic-only (all V2 Swaps in this single block) and
    // filter to watched pools client-side. The single-block range bounds the response.
    let filter = Filter::new()
        .from_block(block_num)
        .to_block(block_num)
        .event(V2_SWAP_SIG);

    let logs = match provider.get_logs(&filter).await {
        Ok(l) => l,
        Err(e) => {
            warn!(
                event = "block_scanner.getlogs_failed",
                chain_id,
                block_num,
                error = %e,
                "eth_getLogs failed; skipping block"
            );
            return;
        }
    };
    let total_swaps = logs.len();

    for log in logs {
        let pool = log.address;
        let Some(&(token0, token1)) = v2_pools.get(&pool) else {
            continue; // swap on a pool we don't watch
        };
        watched_swaps += 1;
        let Some((token_in, token_out, amount_in)) =
            decode_v2_swap(log.data.as_ref(), token0, token1)
        else {
            continue;
        };

        let tx_hash = log.transaction_hash.unwrap_or_default();
        let intent = match RouteIntent::new(
            chain_id,
            tx_hash,
            pool, // router slot carries the source pool for backrun intents
            RouterKind::UniswapV2,
            Address::zero(),
            vec![RouteIntentLeg {
                token_in,
                token_out,
                pool_hint: Some(pool),
                dex_hint: None,
                fee_bps: None,
                protocol_type: ProtocolType::V2,
            }],
            amount_in,
            None,
            SwapExactMode::ExactIn,
            DetectionSource::NewBlock,
        ) {
            Some(i) => i,
            None => continue,
        };

        if let Err(e) = orch.on_route_intent(intent).await {
            warn!(
                event = "block_scanner.on_route_intent_err",
                chain_id,
                block_num,
                error = %e,
            );
        }
        intents += 1;
    }

    if watched_swaps > 0 {
        info!(
            event = "block_scanner.block_processed",
            chain_id,
            block_num,
            watched_pools = v2_pools.len(),
            total_swaps,
            watched_swaps,
            intents,
            "confirmed V2 swaps on watched pools decoded into route intents"
        );
    } else {
        debug!(
            event = "block_scanner.block_quiet",
            chain_id,
            block_num,
            watched_pools = v2_pools.len(),
            total_swaps,
        );
    }
}

/// Decode a Uniswap-V2 `Swap` event's non-indexed data
/// `(amount0In, amount1In, amount0Out, amount1Out)` (4×uint256) into the observed
/// `(token_in, token_out, amount_in)`. Returns `None` for a malformed / zero swap.
fn decode_v2_swap(data: &[u8], token0: Address, token1: Address) -> Option<(Address, Address, U256)> {
    if data.len() < 128 {
        return None;
    }
    let amount0_in = U256::from_big_endian(&data[0..32]);
    let amount1_in = U256::from_big_endian(&data[32..64]);
    if !amount0_in.is_zero() {
        // token0 went IN → swap sold token0 for token1.
        Some((token0, token1, amount0_in))
    } else if !amount1_in.is_zero() {
        Some((token1, token0, amount1_in))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn enc(words: &[U256]) -> Vec<u8> {
        let mut out = Vec::new();
        for w in words {
            let mut buf = [0u8; 32];
            w.to_big_endian(&mut buf);
            out.extend_from_slice(&buf);
        }
        out
    }

    #[test]
    fn decode_v2_swap_token0_in() {
        let t0 = Address::from_low_u64_be(0xA);
        let t1 = Address::from_low_u64_be(0xB);
        // amount0In=1000, amount1In=0, amount0Out=0, amount1Out=990
        let data = enc(&[U256::from(1000u64), U256::zero(), U256::zero(), U256::from(990u64)]);
        let (ti, to, amt) = decode_v2_swap(&data, t0, t1).expect("decodes");
        assert_eq!(ti, t0);
        assert_eq!(to, t1);
        assert_eq!(amt, U256::from(1000u64));
    }

    #[test]
    fn decode_v2_swap_token1_in() {
        let t0 = Address::from_low_u64_be(0xA);
        let t1 = Address::from_low_u64_be(0xB);
        // amount0In=0, amount1In=2000
        let data = enc(&[U256::zero(), U256::from(2000u64), U256::from(1980u64), U256::zero()]);
        let (ti, to, amt) = decode_v2_swap(&data, t0, t1).expect("decodes");
        assert_eq!(ti, t1);
        assert_eq!(to, t0);
        assert_eq!(amt, U256::from(2000u64));
    }

    #[test]
    fn decode_v2_swap_rejects_short_and_zero() {
        let t0 = Address::from_low_u64_be(0xA);
        let t1 = Address::from_low_u64_be(0xB);
        assert!(decode_v2_swap(&[0u8; 64], t0, t1).is_none()); // too short
        let zero = enc(&[U256::zero(); 4]);
        assert!(decode_v2_swap(&zero, t0, t1).is_none()); // no input amount
    }
}
