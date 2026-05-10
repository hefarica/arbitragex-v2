//! Redis cache layout for pool reserves and token metadata.
//!
//! Keys:
//!   arbx:pool_reserves:<chain_id>:<pool_addr_lower>     → JSON ReservesEntry          (V2)
//!   arbx:pool_index:<chain_id>:<sym0>:<sym1>             → JSON Vec<String>            (V2 — pool addrs lower)
//!   arbx:pool_index_v3:<chain_id>:<sym0>:<sym1>          → JSON Vec<V3PoolInfo>        (V3 — addr+fee_bps)
//!   arbx:tokens:<chain_id>:<addr_lower>                  → JSON TokenMeta
//!   arbx:v3_quote:<chain_id>:<pool_lower>:<amount_in>    → string amount_out (TTL 5s)  (scanner cache)
//!   arbx:v3_slot0:<chain_id>:<pool_addr_lower>           → JSON V3Slot0Entry           (V3 — slot0+liquidity)
//!                                                          sym0 < sym1 lexicographically in pool_index*
//!
//! TTLs:
//!   pool_reserves : 30s (re-set every 5s by PoolSyncWorker; readers tolerate up to 10s lag)
//!   pool_index    : no expiry (operator-managed via SQL); refreshed at PoolSyncWorker boot
//!   pool_index_v3 : no expiry (operator-managed via SQL); refreshed at PoolSyncWorker boot
//!   tokens        : no expiry (rarely changes; refreshed at PoolSyncWorker boot)
//!   v3_quote      : 5s (aligned with PoolSyncWorker tick — same staleness window as V2 reserves)
//!   v3_slot0      : 30s (re-set every tick by PoolSyncWorker; matches V2 reserves TTL)
//!
//! Doctrine: every Redis read returns Option (cache miss is normal at boot, scanner
//! tolerates None by leaving gross_profit=0 and emitting `event=scanner.no_reserves_yet`).

use redis::aio::ConnectionManager;
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReservesEntry {
    /// reserve0 as decimal string (uint112 fits in u128 but we use string for forward-compat)
    pub r0: String,
    pub r1: String,
    /// Lowercase 0x-prefixed address of token0 in the pool. Required for the
    /// scanner to determine swap orientation (which reserve is `in` vs `out`)
    /// without computing both directions and applying a magnitude heuristic.
    /// Optional with `serde(default)` for backward compat with legacy cache
    /// entries — when None, the scanner falls back to the dual-orientation
    /// heuristic. PoolSyncWorker populates this on every tick.
    #[serde(default)]
    pub token0_addr: Option<String>,
    /// block number at which the reserves were observed
    pub blk: u64,
    /// unix epoch seconds
    pub ts: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenMeta {
    pub symbol: String,
    pub decimals: u8,
    pub is_stablecoin: bool,
}

/// V3 pool descriptor — address + fee tier. Stored under
/// `arbx:pool_index_v3:<chain>:<sym0>:<sym1>` as `Vec<V3PoolInfo>`.
/// Unlike V2 (which only needs an address; reserves are fetched separately),
/// V3 quoting goes through the on-chain QuoterV2 and the fee tier is part
/// of the call signature, so it must travel with the address.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct V3PoolInfo {
    /// Pool address, lowercase hex with 0x prefix.
    pub pool_addr: String,
    /// V3 fee tier in basis points: 100 (0.01%), 500 (0.05%), 3000 (0.30%), 10000 (1.00%).
    pub fee_bps: u32,
}

pub fn key_pool_reserves(chain_id: u64, pool_addr_lower: &str) -> String {
    format!("arbx:pool_reserves:{}:{}", chain_id, pool_addr_lower)
}

pub fn key_pool_index(chain_id: u64, sym_a: &str, sym_b: &str) -> String {
    let (lo, hi) = if sym_a < sym_b { (sym_a, sym_b) } else { (sym_b, sym_a) };
    format!("arbx:pool_index:{}:{}:{}", chain_id, lo, hi)
}

pub fn key_pool_index_v3(chain_id: u64, sym_a: &str, sym_b: &str) -> String {
    let (lo, hi) = if sym_a < sym_b { (sym_a, sym_b) } else { (sym_b, sym_a) };
    format!("arbx:pool_index_v3:{}:{}:{}", chain_id, lo, hi)
}

pub fn key_token(chain_id: u64, addr_lower: &str) -> String {
    format!("arbx:tokens:{}:{}", chain_id, addr_lower)
}

pub fn key_v3_quote(chain_id: u64, pool_addr_lower: &str, amount_in_dec: &str) -> String {
    format!("arbx:v3_quote:{}:{}:{}", chain_id, pool_addr_lower, amount_in_dec)
}

pub fn key_v3_slot0(chain_id: u64, pool_addr_lower: &str) -> String {
    format!("arbx:v3_slot0:{}:{}", chain_id, pool_addr_lower)
}

/// V3 slot0 snapshot written by `pool_sync_worker` on every tick.
///
/// Fields match what `prioritization-spine::config_aware::ConfigAwareEvaluator`
/// expects: `sqrt_price_x96` and `liquidity` as decimal strings (uint160 and
/// uint128 respectively exceed u64 max, so string is the only lossless encoding
/// in JSON). The `ts` field is unix epoch seconds for staleness checks.
///
/// The `token0_to_token1` direction flag is NOT stored here — it depends on the
/// caller's token ordering context and is resolved by the scanner when it calls
/// `ConfigAwareEvaluator::with_v3_slot0`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct V3Slot0Entry {
    /// sqrtPriceX96 from slot0() as decimal string. uint160 → fits in u128 for
    /// practical ETH prices (overflow would require price ratio > 2^32, i.e.
    /// ~4 billion x), so u128 parse is safe for all live pools.
    pub sqrt_price_x96: String,
    /// Active liquidity at current tick from liquidity(). uint128 decimal string.
    pub liquidity: String,
    /// Unix epoch seconds when this snapshot was observed.
    pub ts: u64,
}

pub async fn set_v3_slot0(
    redis: &mut ConnectionManager,
    chain_id: u64,
    pool_addr_lower: &str,
    entry: &V3Slot0Entry,
    ttl_secs: u64,
) -> redis::RedisResult<()> {
    let json = serde_json::to_string(entry).map_err(|e| {
        redis::RedisError::from((redis::ErrorKind::TypeError, "serde", e.to_string()))
    })?;
    let _: () = redis
        .set_ex(key_v3_slot0(chain_id, pool_addr_lower), json, ttl_secs)
        .await?;
    Ok(())
}

/// Read V3 slot0 snapshot from Redis. Staged here for the scanner to wire in
/// a follow-up sprint (CODE-5 consumer side). The writer (`set_v3_slot0`) is
/// the primary deliverable of CODE-5; this reader ships with it so the API is
/// symmetric and reviewable now.
#[allow(dead_code)]
pub async fn get_v3_slot0(
    redis: &mut ConnectionManager,
    chain_id: u64,
    pool_addr_lower: &str,
) -> redis::RedisResult<Option<V3Slot0Entry>> {
    let raw: Option<String> = redis.get(key_v3_slot0(chain_id, pool_addr_lower)).await?;
    Ok(raw.and_then(|s| serde_json::from_str(&s).ok()))
}

pub async fn set_reserves(
    redis: &mut ConnectionManager,
    chain_id: u64,
    pool_addr_lower: &str,
    entry: &ReservesEntry,
    ttl_secs: u64,
) -> redis::RedisResult<()> {
    let json = serde_json::to_string(entry).map_err(|e| {
        redis::RedisError::from((redis::ErrorKind::TypeError, "serde", e.to_string()))
    })?;
    let _: () = redis
        .set_ex(key_pool_reserves(chain_id, pool_addr_lower), json, ttl_secs)
        .await?;
    Ok(())
}

pub async fn get_reserves(
    redis: &mut ConnectionManager,
    chain_id: u64,
    pool_addr_lower: &str,
) -> redis::RedisResult<Option<ReservesEntry>> {
    let raw: Option<String> = redis.get(key_pool_reserves(chain_id, pool_addr_lower)).await?;
    Ok(raw.and_then(|s| serde_json::from_str(&s).ok()))
}

pub async fn set_pool_index(
    redis: &mut ConnectionManager,
    chain_id: u64,
    sym_a: &str,
    sym_b: &str,
    pool_addrs_lower: &[String],
) -> redis::RedisResult<()> {
    let json = serde_json::to_string(pool_addrs_lower).map_err(|e| {
        redis::RedisError::from((redis::ErrorKind::TypeError, "serde", e.to_string()))
    })?;
    let _: () = redis
        .set(key_pool_index(chain_id, sym_a, sym_b), json)
        .await?;
    Ok(())
}

pub async fn get_pools_for_pair(
    redis: &mut ConnectionManager,
    chain_id: u64,
    sym_a: &str,
    sym_b: &str,
) -> redis::RedisResult<Vec<String>> {
    let raw: Option<String> = redis.get(key_pool_index(chain_id, sym_a, sym_b)).await?;
    Ok(raw.and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default())
}

pub async fn set_pool_index_v3(
    redis: &mut ConnectionManager,
    chain_id: u64,
    sym_a: &str,
    sym_b: &str,
    pools: &[V3PoolInfo],
) -> redis::RedisResult<()> {
    let json = serde_json::to_string(pools).map_err(|e| {
        redis::RedisError::from((redis::ErrorKind::TypeError, "serde", e.to_string()))
    })?;
    let _: () = redis
        .set(key_pool_index_v3(chain_id, sym_a, sym_b), json)
        .await?;
    Ok(())
}

pub async fn get_pools_for_pair_v3(
    redis: &mut ConnectionManager,
    chain_id: u64,
    sym_a: &str,
    sym_b: &str,
) -> redis::RedisResult<Vec<V3PoolInfo>> {
    let raw: Option<String> = redis.get(key_pool_index_v3(chain_id, sym_a, sym_b)).await?;
    Ok(raw.and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default())
}

/// Cache a V3 quote result. Key is keyed by (chain, pool, amount_in) so two
/// candidates with the same trade size against the same pool reuse the quote
/// for up to 5s — that aligns with the PoolSyncWorker tick on V2, so V3 quote
/// staleness matches V2 reserves staleness.
pub async fn set_v3_quote(
    redis: &mut ConnectionManager,
    chain_id: u64,
    pool_addr_lower: &str,
    amount_in_dec: &str,
    amount_out_dec: &str,
    ttl_secs: u64,
) -> redis::RedisResult<()> {
    let _: () = redis
        .set_ex(
            key_v3_quote(chain_id, pool_addr_lower, amount_in_dec),
            amount_out_dec,
            ttl_secs,
        )
        .await?;
    Ok(())
}

pub async fn get_v3_quote(
    redis: &mut ConnectionManager,
    chain_id: u64,
    pool_addr_lower: &str,
    amount_in_dec: &str,
) -> redis::RedisResult<Option<String>> {
    redis.get(key_v3_quote(chain_id, pool_addr_lower, amount_in_dec)).await
}

pub async fn set_token_meta(
    redis: &mut ConnectionManager,
    chain_id: u64,
    addr_lower: &str,
    meta: &TokenMeta,
) -> redis::RedisResult<()> {
    let json = serde_json::to_string(meta).map_err(|e| {
        redis::RedisError::from((redis::ErrorKind::TypeError, "serde", e.to_string()))
    })?;
    let _: () = redis.set(key_token(chain_id, addr_lower), json).await?;
    Ok(())
}

pub async fn get_token_meta(
    redis: &mut ConnectionManager,
    chain_id: u64,
    addr_lower: &str,
) -> redis::RedisResult<Option<TokenMeta>> {
    let raw: Option<String> = redis.get(key_token(chain_id, addr_lower)).await?;
    Ok(raw.and_then(|s| serde_json::from_str(&s).ok()))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)] // M11: test module — panics are acceptable
    use super::*;

    #[test]
    fn pool_index_key_sorts_symbols() {
        assert_eq!(key_pool_index(1, "WETH", "USDC"), "arbx:pool_index:1:USDC:WETH");
        assert_eq!(key_pool_index(1, "USDC", "WETH"), "arbx:pool_index:1:USDC:WETH");
    }

    #[test]
    fn reserves_key_lowercase_addr() {
        assert_eq!(
            key_pool_reserves(1, "0xb4e16d01"),
            "arbx:pool_reserves:1:0xb4e16d01"
        );
    }

    #[test]
    fn reserves_entry_serde_roundtrip() {
        let entry = ReservesEntry {
            r0: "12345".into(),
            r1: "67890".into(),
            token0_addr: Some("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2".into()),
            blk: 18_500_000,
            ts: 1_714_857_600,
        };
        let json = serde_json::to_string(&entry).unwrap();
        let back: ReservesEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(back.r0, entry.r0);
        assert_eq!(back.blk, entry.blk);
        assert_eq!(back.token0_addr, entry.token0_addr);
    }

    #[test]
    fn reserves_entry_legacy_json_without_token0_addr_deserialises() {
        // Backward compat: cache entries written before the structural fix
        // do NOT have token0_addr. They must still deserialise (as None)
        // so the scanner can fall back to dual-orientation heuristic.
        let legacy_json = r#"{"r0":"100","r1":"200","blk":1,"ts":2}"#;
        let entry: ReservesEntry = serde_json::from_str(legacy_json).unwrap();
        assert_eq!(entry.r0, "100");
        assert_eq!(entry.token0_addr, None);
    }

    #[test]
    fn pool_index_v3_key_sorts_symbols() {
        // Same sort invariant as V2 — readers query without knowing the seed order.
        assert_eq!(key_pool_index_v3(1, "WETH", "USDC"), "arbx:pool_index_v3:1:USDC:WETH");
        assert_eq!(key_pool_index_v3(1, "USDC", "WETH"), "arbx:pool_index_v3:1:USDC:WETH");
    }

    #[test]
    fn pool_index_v3_disjoint_from_v2() {
        // V2 and V3 caches must NOT share a key — scanner reads both independently.
        let v2 = key_pool_index(1, "USDC", "WETH");
        let v3 = key_pool_index_v3(1, "USDC", "WETH");
        assert_ne!(v2, v3);
    }

    #[test]
    fn v3_pool_info_serde_roundtrip() {
        let info = V3PoolInfo {
            pool_addr: "0x88e6a0c2ddd26feeb64f039a2c41296fcb3f5640".into(),
            fee_bps: 500,
        };
        let json = serde_json::to_string(&info).unwrap();
        let back: V3PoolInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(back, info);
    }

    #[test]
    fn v3_pool_info_array_serde_roundtrip() {
        // The pool index stores Vec<V3PoolInfo> — the same pair can have multiple
        // fee tiers on V3 (0.05% and 0.30% for WETH/USDC are common).
        let arr = vec![
            V3PoolInfo { pool_addr: "0xaaa".into(), fee_bps: 500 },
            V3PoolInfo { pool_addr: "0xbbb".into(), fee_bps: 3000 },
        ];
        let json = serde_json::to_string(&arr).unwrap();
        let back: Vec<V3PoolInfo> = serde_json::from_str(&json).unwrap();
        assert_eq!(back, arr);
    }

    #[test]
    fn v3_quote_key_layout() {
        let key = key_v3_quote(1, "0xpool", "1000000000000000000");
        assert_eq!(key, "arbx:v3_quote:1:0xpool:1000000000000000000");
    }

    #[test]
    fn v3_slot0_key_layout() {
        let key = key_v3_slot0(1, "0x88e6a0c2ddd26feeb64f039a2c41296fcb3f5640");
        assert_eq!(key, "arbx:v3_slot0:1:0x88e6a0c2ddd26feeb64f039a2c41296fcb3f5640");
    }

    #[test]
    fn v3_slot0_entry_serde_roundtrip() {
        // sqrtPriceX96 for USDC/WETH ~1800 USD:
        //   √(1800 × 10^12) × 2^96 ≈ 1.682e33  (fits in u128, < 2^112)
        let entry = V3Slot0Entry {
            sqrt_price_x96: "1682363847015700853685856186817536".to_string(),
            liquidity: "12345678901234567890".to_string(),
            ts: 1_714_857_600,
        };
        let json = serde_json::to_string(&entry).unwrap();
        let back: V3Slot0Entry = serde_json::from_str(&json).unwrap();
        assert_eq!(back.sqrt_price_x96, entry.sqrt_price_x96);
        assert_eq!(back.liquidity, entry.liquidity);
        assert_eq!(back.ts, entry.ts);
    }

    #[test]
    fn v3_slot0_key_disjoint_from_pool_reserves() {
        // v3_slot0 and pool_reserves must NOT share a key — spine reads both
        // independently for different DEX types.
        let slot0_key = key_v3_slot0(1, "0xpool");
        let reserves_key = key_pool_reserves(1, "0xpool");
        assert_ne!(slot0_key, reserves_key);
    }
}
