//! Dedup for pending tx hashes.
//!
//! Two tiers:
//!   - L1: in-memory LRU per-chain (fast path).
//!   - L2: Redis SETNX with TTL so multi-instance does not double-insert.

use ethers::types::H256;
use lru::LruCache;
use redis::aio::ConnectionManager;
use redis::AsyncCommands;
use std::num::NonZeroUsize;
use std::sync::Mutex;
use std::time::Duration;

pub struct Dedup {
    lru: Mutex<LruCache<H256, ()>>,
    redis_ttl: Duration,
}

impl Dedup {
    pub fn new(capacity: usize, redis_ttl: Duration) -> Self {
        let cap = NonZeroUsize::new(capacity).unwrap_or(NonZeroUsize::new(1).unwrap());
        Self { lru: Mutex::new(LruCache::new(cap)), redis_ttl }
    }

    /// Returns true if the hash is fresh (not seen). false if duplicate.
    /// Call ORDER: check LRU first; if fresh, check Redis SETNX; if fresh there too, accept.
    pub async fn check_and_mark(
        &self,
        hash: H256,
        redis: &mut ConnectionManager,
    ) -> bool {
        // L1 LRU
        {
            let mut lru = self.lru.lock().expect("lru mutex");
            if lru.contains(&hash) {
                return false;
            }
            lru.put(hash, ());
        }
        // L2 Redis SETNX
        let key = format!("arbx:dedup:pendingtx:{:#x}", hash);
        let set: Result<Option<String>, _> = redis
            .set_options(
                &key,
                "1",
                redis::SetOptions::default()
                    .conditional_set(redis::ExistenceCheck::NX)
                    .with_expiration(redis::SetExpiry::EX(self.redis_ttl.as_secs() as usize)),
            )
            .await;
        matches!(set, Ok(Some(_)))
    }
}
