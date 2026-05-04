//! Redis cache layout for pool reserves and token metadata.
//!
//! Keys:
//!   arbx:pool_reserves:<chain_id>:<pool_addr_lower>  → JSON ReservesEntry
//!   arbx:pool_index:<chain_id>:<sym0>:<sym1>          → JSON Vec<String> (pool addrs lower)
//!                                                       sym0 < sym1 lexicographically
//!   arbx:tokens:<chain_id>:<addr_lower>               → JSON TokenMeta
//!
//! TTLs:
//!   pool_reserves: 30s (re-set every 5s by PoolSyncWorker; readers tolerate up to 10s lag)
//!   pool_index   : no expiry (operator-managed via SQL); refreshed at PoolSyncWorker boot
//!   tokens       : no expiry (rarely changes; refreshed at PoolSyncWorker boot)
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

pub fn key_pool_reserves(chain_id: u64, pool_addr_lower: &str) -> String {
    format!("arbx:pool_reserves:{}:{}", chain_id, pool_addr_lower)
}

pub fn key_pool_index(chain_id: u64, sym_a: &str, sym_b: &str) -> String {
    let (lo, hi) = if sym_a < sym_b { (sym_a, sym_b) } else { (sym_b, sym_a) };
    format!("arbx:pool_index:{}:{}:{}", chain_id, lo, hi)
}

pub fn key_token(chain_id: u64, addr_lower: &str) -> String {
    format!("arbx:tokens:{}:{}", chain_id, addr_lower)
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
            blk: 18_500_000,
            ts: 1_714_857_600,
        };
        let json = serde_json::to_string(&entry).unwrap();
        let back: ReservesEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(back.r0, entry.r0);
        assert_eq!(back.blk, entry.blk);
    }
}
