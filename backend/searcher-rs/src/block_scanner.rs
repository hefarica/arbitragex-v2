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
use ethers::types::{Address, Filter, ValueOrArray, H256, U256};
use once_cell::sync::Lazy;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

use crate::chain_client::WsChainClient;
use crate::impact_index::ImpactIndex;
use crate::orchestrator::Orchestrator;
use crate::route_intent::{
    DetectionSource, ProtocolType, RouteIntent, RouteIntentLeg, RouterKind, SwapExactMode,
};

/// Uniswap-V2 `Swap` event signature (constant-product AMMs: Uni V2 / Sushi / forks).
const V2_SWAP_SIG: &str = "Swap(address,uint256,uint256,uint256,uint256,address)";
/// Uniswap-V3 `Swap` event signature (concentrated liquidity: Uni V3 / forks).
/// `sender` + `recipient` are INDEXED (they live in topics, NOT in `data`), so the
/// non-indexed `data` payload is `(amount0 int256, amount1 int256, sqrtPriceX96 uint160,
/// liquidity uint128, tick int24)` = 5 × 32 bytes.
const V3_SWAP_SIG: &str = "Swap(address,address,int256,int256,uint160,uint128,int24)";

/// topic0 = keccak256(signature). Derived at runtime (self-documenting, no magic hex);
/// the `topic0_hashes_match_canonical` test pins each against the canonical on-chain value.
static V2_SWAP_TOPIC0: Lazy<H256> =
    Lazy::new(|| H256::from(ethers::utils::keccak256(V2_SWAP_SIG.as_bytes())));
static V3_SWAP_TOPIC0: Lazy<H256> =
    Lazy::new(|| H256::from(ethers::utils::keccak256(V3_SWAP_SIG.as_bytes())));

/// Handles into the cartridge `HostContext` atomics so the block scanner can publish chain
/// context (block height + base fee) on every `newHeads`. Without this the atomics stay 0,
/// `get_base_fee()`/`get_block_number()` return 0, and the cartridge net-profit gate is
/// unsatisfiable (gas reads as free). Cheap `Arc<AtomicU64>` clones; `None` when the cartridge
/// runtime is disabled.
#[derive(Clone)]
pub struct GasBlockSink {
    pub block_number: Arc<std::sync::atomic::AtomicU64>,
    /// Base fee stored as gwei×1000 (milli-gwei) to match `get_base_fee()`'s 3-decimal decoding.
    pub base_fee_milligwei: Arc<std::sync::atomic::AtomicU64>,
}

/// Backrunning detection loop: subscribe to new blocks and turn confirmed V2 swaps on
/// watched pools into `RouteIntent`s fed to the orchestrator. Long-running; honors
/// `cancel`. Reconnects on WS error with exponential backoff. Never panics.
pub async fn block_detection_loop(
    chain_id: u64,
    ws_urls: Vec<String>,
    orchestrator: Option<Arc<Orchestrator>>,
    impact_index: Option<Arc<RwLock<ImpactIndex>>>,
    gas_sink: Option<GasBlockSink>,
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
        match run_block_subscription(chain_id, url, &orch, &idx, gas_sink.as_ref(), &cancel).await {
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
    gas_sink: Option<&GasBlockSink>,
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

                // Publish chain context to the cartridge HostContext atomics on EVERY head, so
                // get_block_number()/get_base_fee() return real values. Without this the cartridge
                // net-profit gate is unsatisfiable (gas reads as 0). Ordering::Release pairs with
                // the cartridge's Relaxed loads — the values are advisory, not a lock.
                if let Some(sink) = gas_sink {
                    sink.block_number
                        .store(block_num.as_u64(), std::sync::atomic::Ordering::Release);
                    if let Some(base_fee_wei) = block.base_fee_per_gas {
                        // get_base_fee() decodes the atomic as gwei×1000 (milli-gwei); wei / 1e6
                        // = milli-gwei. Saturating: pre-EIP-1559 chains have no base fee → skipped.
                        let milligwei = (base_fee_wei / U256::from(1_000_000u64)).as_u64();
                        sink.base_fee_milligwei
                            .store(milligwei, std::sync::atomic::Ordering::Release);
                    }
                }

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
    // Snapshot watched V2 + V3 pools (clone out, release the read lock quickly). Both AMM
    // families are decoded; the per-log topic0 selects the correct decoder below.
    let watched: HashMap<Address, (Address, Address)> = {
        let guard = idx.read().await;
        guard
            .all_pools()
            .into_iter()
            .filter(|p| matches!(p.protocol_type, ProtocolType::V2 | ProtocolType::V3))
            .map(|p| (p.address, (p.token0, p.token1)))
            .collect()
    };
    if watched.is_empty() {
        debug!(event = "block_scanner.no_watched_pools", chain_id, block_num);
        return;
    }

    let mut v2_intents = 0u32;
    let mut v3_intents = 0u32;
    let mut watched_swaps = 0u32;

    // Scan the PREVIOUS block (N-1): free RPCs' getLogs node can lag behind the newHeads
    // announcement, returning -32602 "block range extends beyond current head block". N-1
    // is guaranteed available. The 1-block lag is fine for confirmed-state backrunning.
    //
    // Also: many free RPCs BLOCK address-filtered eth_getLogs ("blocked parameter:
    // params.0.address"), so we query topic-only — every V2 AND V3 Swap in the block —
    // and filter to watched pools client-side. The single-block range bounds the response.
    let scan_block = block_num.saturating_sub(1);
    let filter = Filter::new()
        .from_block(scan_block)
        .to_block(scan_block)
        .topic0(ValueOrArray::Array(vec![
            Some(*V2_SWAP_TOPIC0),
            Some(*V3_SWAP_TOPIC0),
        ]));

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
        let Some(&(token0, token1)) = watched.get(&pool) else {
            continue; // swap on a pool we don't watch
        };
        watched_swaps += 1;

        // Select the decoder + protocol tag from the log's OWN topic0. The topic-filtered
        // getLogs only ever returns V2/V3 Swaps, but we dispatch EXPLICITLY (no silent V2
        // fallback): a log carrying neither topic0 (a misbehaving RPC) is skipped, never
        // mis-decoded. Driving off the observed event also keeps `protocol_type` consistent
        // with the actual log.
        let topic0 = log.topics.first().copied();
        let is_v3 = topic0 == Some(*V3_SWAP_TOPIC0);
        let (decoded, proto, router) = if is_v3 {
            (
                decode_v3_swap(log.data.as_ref(), token0, token1),
                ProtocolType::V3,
                RouterKind::UniswapV3,
            )
        } else if topic0 == Some(*V2_SWAP_TOPIC0) {
            (
                decode_v2_swap(log.data.as_ref(), token0, token1),
                ProtocolType::V2,
                RouterKind::UniswapV2,
            )
        } else {
            continue; // neither swap topic — fail-honest skip, do not guess a decoder
        };
        let Some((token_in, token_out, amount_in)) = decoded else {
            continue;
        };

        let tx_hash = log.transaction_hash.unwrap_or_default();
        let intent = match RouteIntent::new(
            chain_id,
            tx_hash,
            pool, // router slot carries the source pool for backrun intents
            router,
            Address::zero(),
            vec![RouteIntentLeg {
                token_in,
                token_out,
                pool_hint: Some(pool),
                dex_hint: None,
                fee_bps: None,
                protocol_type: proto,
            }],
            amount_in,
            None,
            SwapExactMode::ExactIn,
            DetectionSource::NewBlock,
        ) {
            Some(i) => i,
            None => continue,
        };

        // Validation probe (info, low volume): make each newly-assimilated V3 intent
        // explicit in production so the V3 path is auditable end-to-end. Demote to debug
        // once V3 flow is confirmed (mirrors the earlier `cartridge.shadow_enter` probe).
        if is_v3 {
            info!(
                event = "block_scanner.v3_intent",
                chain_id,
                block_num,
                protocol = "v3",
                pool = %format!("{:#x}", pool),
                tx_hash = %tx_hash,
                token_in = %format!("{:#x}", token_in),
                token_out = %format!("{:#x}", token_out),
                amount_in = %amount_in,
                "Uniswap V3 swap decoded into a backrun route intent"
            );
            v3_intents += 1;
        } else {
            v2_intents += 1;
        }

        // Isolate the orchestrator call in its own task: a panic on a single intent must
        // NOT kill the block scanner loop. The cartridge shadow eval is itself spawned early
        // inside on_route_intent, so it still runs even if a later stage panics.
        let orch_task = orch.clone();
        tokio::spawn(async move {
            if let Err(e) = orch_task.on_route_intent(intent).await {
                warn!(event = "block_scanner.on_route_intent_err", error = %e);
            }
        });
    }

    if watched_swaps > 0 {
        info!(
            event = "block_scanner.block_processed",
            chain_id,
            block_num,
            watched_pools = watched.len(),
            total_swaps,
            watched_swaps,
            intents = v2_intents + v3_intents,
            v2_intents,
            v3_intents,
            "confirmed V2/V3 swaps on watched pools decoded into route intents"
        );
    } else {
        debug!(
            event = "block_scanner.block_quiet",
            chain_id,
            block_num,
            watched_pools = watched.len(),
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

/// Decode a Uniswap-V3 `Swap` event's non-indexed `data`
/// `(amount0 int256, amount1 int256, sqrtPriceX96 uint160, liquidity uint128, tick int24)`
/// into the observed `(token_in, token_out, amount_in)`.
///
/// V3 sign convention: `amount0` / `amount1` are the SIGNED deltas of the POOL's token
/// balances. A POSITIVE amount means that token flowed INTO the pool (the trader sold it) →
/// it is the `token_in`; the negative side is what the pool paid out (`token_out`). In a
/// normal swap exactly one side is positive.
///
/// Overflow-safe by construction: the sign is read straight from the two's-complement MSB
/// and we take the magnitude of the POSITIVE side only — whose raw big-endian `U256` already
/// equals its absolute value. We NEVER negate, so the `i256::MIN.abs()` overflow panic is
/// structurally impossible. Returns `None` for malformed (< 64 bytes) or degenerate (no
/// positive side) data.
fn decode_v3_swap(data: &[u8], token0: Address, token1: Address) -> Option<(Address, Address, U256)> {
    if data.len() < 64 {
        return None;
    }
    // int256, big-endian two's complement: the most-significant byte's top bit is the sign.
    let raw0 = U256::from_big_endian(&data[0..32]);
    let raw1 = U256::from_big_endian(&data[32..64]);
    let neg0 = data[0] & 0x80 != 0;
    let neg1 = data[32] & 0x80 != 0;
    // "positive" = entered the pool. A canonical V3 swap has EXACTLY one positive side; the
    // positive int256's raw big-endian U256 already equals its magnitude (no negation ⇒ no
    // overflow). `pos0`/`pos1` exclude zero so a one-sided swap routes correctly.
    let pos0 = !neg0 && !raw0.is_zero();
    let pos1 = !neg1 && !raw1.is_zero();

    match (pos0, pos1) {
        // token0 entered the pool → token0 is token_in, amount_in = |amount0|.
        (true, false) => Some((token0, token1, raw0)),
        // token1 entered the pool → token1 is token_in.
        (false, true) => Some((token1, token0, raw1)),
        // Ambiguous (both sides positive — not a well-formed V3 swap) or degenerate (neither
        // positive). Fail-honest: do NOT guess a direction (RULE 00 / R8 — None = no computado).
        _ => None,
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

    // ---- V3 ----------------------------------------------------------------

    /// Encode a signed int256 (given as `i128`) into a 32-byte big-endian two's-complement
    /// word — exactly how an on-chain `int256` amount appears in a V3 Swap log's `data`.
    fn enc_i256(v: i128) -> [u8; 32] {
        let u = if v < 0 {
            // 2^256 - |v|  (wrapping subtraction from zero == two's complement).
            U256::zero().overflowing_sub(U256::from((-v) as u128)).0
        } else {
            U256::from(v as u128)
        };
        let mut buf = [0u8; 32];
        u.to_big_endian(&mut buf);
        buf
    }

    /// Build a 160-byte V3 Swap `data` payload (amount0, amount1, sqrtPriceX96, liquidity, tick).
    fn v3_data(amount0: i128, amount1: i128) -> Vec<u8> {
        let mut out = Vec::with_capacity(160);
        out.extend_from_slice(&enc_i256(amount0));
        out.extend_from_slice(&enc_i256(amount1));
        out.extend_from_slice(&[0u8; 32]); // sqrtPriceX96 (unused by decoder)
        out.extend_from_slice(&[0u8; 32]); // liquidity     (unused)
        out.extend_from_slice(&[0u8; 32]); // tick          (unused)
        out
    }

    #[test]
    fn decode_v3_swap_token0_in_when_amount0_positive() {
        let t0 = Address::from_low_u64_be(0xA);
        let t1 = Address::from_low_u64_be(0xB);
        // amount0 = +1000 (token0 flowed INTO pool), amount1 = -990 (token1 OUT).
        let data = v3_data(1000, -990);
        let (ti, to, amt) = decode_v3_swap(&data, t0, t1).expect("decodes");
        assert_eq!(ti, t0, "positive amount0 ⇒ token0 is token_in");
        assert_eq!(to, t1);
        assert_eq!(amt, U256::from(1000u64), "amount_in is |positive side|");
    }

    #[test]
    fn decode_v3_swap_token1_in_when_amount0_negative() {
        let t0 = Address::from_low_u64_be(0xA);
        let t1 = Address::from_low_u64_be(0xB);
        // amount0 = -500 (token0 OUT), amount1 = +480 (token1 INTO pool).
        let data = v3_data(-500, 480);
        let (ti, to, amt) = decode_v3_swap(&data, t0, t1).expect("decodes");
        assert_eq!(ti, t1, "positive amount1 ⇒ token1 is token_in");
        assert_eq!(to, t0);
        assert_eq!(amt, U256::from(480u64));
    }

    #[test]
    fn decode_v3_swap_large_negative_does_not_overflow() {
        let t0 = Address::from_low_u64_be(0xA);
        let t1 = Address::from_low_u64_be(0xB);
        // A realistically huge negative amount0 (~1e30) must NOT panic (we never negate it);
        // the positive amount1 is correctly selected as token_in.
        let data = v3_data(-1_000_000_000_000_000_000_000_000_000_000i128, 7_777);
        let (ti, to, amt) = decode_v3_swap(&data, t0, t1).expect("decodes");
        assert_eq!(ti, t1);
        assert_eq!(to, t0);
        assert_eq!(amt, U256::from(7_777u64));
    }

    #[test]
    fn decode_v3_swap_rejects_short_and_zero() {
        let t0 = Address::from_low_u64_be(0xA);
        let t1 = Address::from_low_u64_be(0xB);
        assert!(decode_v3_swap(&[0u8; 32], t0, t1).is_none()); // too short (<64)
        assert!(decode_v3_swap(&v3_data(0, 0), t0, t1).is_none()); // no positive side
    }

    #[test]
    fn decode_v3_swap_rejects_ambiguous_both_positive() {
        let t0 = Address::from_low_u64_be(0xA);
        let t1 = Address::from_low_u64_be(0xB);
        // Both sides positive cannot occur in a well-formed V3 swap → fail-honest (no guess).
        assert!(decode_v3_swap(&v3_data(1000, 2000), t0, t1).is_none());
    }

    #[test]
    fn topic0_hashes_match_canonical_onchain_values() {
        // Independently-known canonical topic0 of each Swap event. If a signature string
        // had a typo, keccak256(sig) would diverge from these and this test would fail.
        assert_eq!(
            format!("{:#x}", *V2_SWAP_TOPIC0),
            "0xd78ad95fa46c994b6551d0da85fc275fe613ce37657fb8d5e3d130840159d822",
            "Uniswap V2 Swap topic0"
        );
        assert_eq!(
            format!("{:#x}", *V3_SWAP_TOPIC0),
            "0xc42079f94a6350d7e6235f29174924f928cc2ac818eb64fed8004e115fbcca67",
            "Uniswap V3 Swap topic0"
        );
    }
}
