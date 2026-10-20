//! G-PRICE-3 — Chainlink event-driven price subscriber.
//!
//! Instead of polling `latestRoundData()` every 30s (price_worker), this
//! subscribes to the `AnswerUpdated` event on each Chainlink aggregator via
//! WebSocket. When Chainlink posts a new round on-chain, the price flows to
//! Redis + WS push in <2s instead of waiting up to 30s for the next poll.
//!
//! The price_worker stays as the reconciliation fallback (its 30s tick
//! catches any missed events). This subscriber is the hot path.
//!
//! ## Architecture
//! ```
//! Chainlink aggregator (on-chain AnswerUpdated event)
//!   → WS subscribe_logs (ethers Provider<Ws>)
//!   → decode current answer from event data
//!   → HSET arbx:token_prices:<chain> <SYMBOL> <price>
//!   → PUBLISH arbx:prices:updated:<chain>
//!   → api-server WS bridge → frontend cards re-price
//! ```
//!
//! ## Doctrine
//! - Read-only: subscribe to logs, never send transactions.
//! - R8 fail-honest: if the WS drops or a decode fails, log + skip; the
//!   price_worker's 30s reconciliation catches up.
//! - RULE 00: oracle addresses come from the `price_oracles` PG table
//!   (operator-seeded), never hardcoded.

use ethers::providers::{Middleware, Provider, StreamExt, Ws};
use ethers::types::{Filter, Log, ValueOrArray, H256, U256};
use redis::aio::ConnectionManager;
use std::collections::HashMap;
use std::str::FromStr;
use std::time::Duration;
use tracing::{debug, info, warn};

/// keccak256("AnswerUpdated(int256,uint256,uint256)")
const ANSWER_UPDATED_TOPIC0: H256 = H256([
    0x05, 0x59, 0x88, 0x4f, 0xd3, 0xa4, 0x60, 0xdb, 0x30, 0x73, 0xb7, 0xfc, 0x89, 0x6c, 0xc7, 0x79,
    0xb2, 0x41, 0xf5, 0xa5, 0xae, 0xde, 0x75, 0x6e, 0x57, 0xc9, 0x48, 0xbe, 0x0e, 0x3d, 0x00, 0x00,
]);

/// Feed metadata loaded from PG at boot.
struct ChainlinkFeed {
    token_address: String,
    oracle_address: String,
    decimals: i32,
    /// Resolved after boot from Redis token meta.
    symbol: String,
}

/// The subscriber: connects via WS, listens for AnswerUpdated on all
/// configured aggregator addresses, writes prices to Redis on every event.
pub struct ChainlinkSubscriber {
    chain_id: u64,
    rpc_ws_url: String,
    feeds: Vec<ChainlinkFeed>,
    redis: ConnectionManager,
}

impl ChainlinkSubscriber {
    /// Load enabled Chainlink feeds from PG, resolve their symbols from
    /// Redis token meta, then spawn the WS subscription loop.
    pub async fn spawn(
        chain_id: u64,
        rpc_ws_url: String,
        db: sqlx::postgres::PgPool,
        redis: ConnectionManager,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Load feeds from the operator-seeded price_oracles table.
        let rows = sqlx::query_as::<_, (String, String, i32)>(
            "SELECT token_address, oracle_address, decimals FROM price_oracles \
             WHERE chain_id = $1 AND kind = 'chainlink' AND enabled = TRUE",
        )
        .bind(chain_id as i32)
        .fetch_all(&db)
        .await
        .map_err(|e| {
            warn!(event = "chainlink_sub.db_failed", chain_id, error = %e, "cannot load Chainlink feeds");
            e
        })?;

        if rows.is_empty() {
            info!(
                event = "chainlink_sub.no_feeds",
                chain_id,
                "no enabled Chainlink oracles — subscriber not started (price_worker poll is the only source)"
            );
            return Ok(());
        }

        // Resolve symbols from Redis token meta (same source as price_worker).
        let mut redis = redis;
        let mut feeds = Vec::new();
        for (token_addr, oracle_addr_str, decimals) in rows {
            let addr_lower = token_addr.to_ascii_lowercase();
            let meta_key = format!("arbx:tokens:{}:{}", chain_id, addr_lower);
            let meta_raw: Option<String> = redis::cmd("GET")
                .arg(&meta_key)
                .query_async(&mut redis)
                .await
                .unwrap_or(None);
            let symbol = meta_raw
                .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
                .and_then(|v| {
                    v.get("symbol")
                        .and_then(|s| s.as_str())
                        .map(|s| s.to_ascii_uppercase())
                })
                .unwrap_or_default();
            if symbol.is_empty() {
                debug!(
                    event = "chainlink_sub.symbol_unresolved",
                    chain_id, token = %addr_lower,
                    "token meta missing — feed included but price writes will be skipped until meta arrives"
                );
            }
            feeds.push(ChainlinkFeed {
                token_address: addr_lower,
                oracle_address: oracle_addr_str,
                decimals,
                symbol,
            });
        }

        let subscriber = Self {
            chain_id,
            rpc_ws_url,
            feeds,
            redis,
        };

        info!(
            event = "chainlink_sub.started",
            chain_id = subscriber.chain_id,
            feeds = subscriber.feeds.len(),
            rpc = %subscriber.rpc_ws_url,
            "Chainlink event subscriber starting (G-PRICE-3 — event-driven, replaces 30s poll for Tier-0)"
        );

        tokio::spawn(subscriber.run());
        Ok(())
    }

    async fn run(self) {
        loop {
            match self.connect_and_listen().await {
                Ok(()) => {
                    info!(
                        event = "chainlink_sub.stream_ended",
                        chain_id = self.chain_id,
                        "WS log stream ended normally — reconnecting"
                    );
                }
                Err(e) => {
                    warn!(
                        event = "chainlink_sub.stream_error",
                        chain_id = self.chain_id,
                        error = %e,
                        "WS subscription error — retrying in 5s (price_worker poll covers the gap)"
                    );
                }
            }
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
    }

    async fn connect_and_listen(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let provider = Provider::<Ws>::connect(&self.rpc_ws_url).await?;

        // Build the filter: AnswerUpdated events from any of our aggregator addresses.
        let oracle_addresses: Vec<ethers::types::Address> = self
            .feeds
            .iter()
            .filter_map(|f| ethers::types::Address::from_str(&f.oracle_address).ok())
            .collect::<Vec<_>>()
            .into_iter()
            .map(|a| a)
            .collect();

        let filter = Filter::new()
            .address(oracle_addresses)
            .topic0(ValueOrArray::Value(ANSWER_UPDATED_TOPIC0));

        let mut stream = provider.subscribe_logs(&filter).await?;
        info!(
            event = "chainlink_sub.subscribed",
            chain_id = self.chain_id,
            oracles = self.feeds.len(),
            "subscribed to AnswerUpdated on all Chainlink aggregators"
        );

        // Oracle address → feed lookup for O(1) event routing.
        let mut feed_by_oracle: HashMap<String, &ChainlinkFeed> = HashMap::new();
        for feed in &self.feeds {
            feed_by_oracle.insert(feed.oracle_address.to_ascii_lowercase(), feed);
        }

        let mut redis = self.redis.clone();

        while let Some(log) = stream.next().await {
            self.handle_log(&log, &feed_by_oracle, &mut redis).await;
        }

        Ok(())
    }

    async fn handle_log(
        &self,
        log: &Log,
        feed_by_oracle: &HashMap<String, &ChainlinkFeed>,
        redis: &mut ConnectionManager,
    ) {
        let oracle_addr = format!("{:?}", log.address).to_ascii_lowercase();
        let feed = match feed_by_oracle.get(&oracle_addr) {
            Some(f) => *f,
            None => return, // not one of our feeds
        };

        if feed.symbol.is_empty() {
            debug!(
                event = "chainlink_sub.skip_no_symbol",
                chain_id = self.chain_id,
                oracle = %oracle_addr,
                "AnswerUpdated received but symbol unresolved — price_worker will catch it"
            );
            return;
        }

        // AnswerUpdated(int256 indexed current, uint256 indexed roundId, uint256 updatedAt)
        // data layout: 32-byte current answer (offset to data since it's indexed → in data section)
        // For non-indexed data, it's in the data field starting at byte 0.
        // Actually: `current` IS indexed (appears in topics[1]), `roundId` indexed (topics[2]),
        // `updatedAt` NOT indexed (in data).
        // But we read `current` from topics[1] as the answer value.
        let answer = match log.topics.get(1) {
            Some(t) => U256::from_big_endian(t.as_bytes()),
            None => {
                debug!(
                    event = "chainlink_sub.no_answer_topic",
                    "missing topics[1] in AnswerUpdated"
                );
                return;
            }
        };

        // Convert to f64 with the feed's decimals.
        let answer_f64 = u256_to_f64(&answer);
        let price = answer_f64 / 10f64.powi(feed.decimals.max(0));
        if !price.is_finite() || price <= 0.0 {
            return;
        }

        // Write to Redis: HSET + PUBLISH (atomic pipeline, same as price_worker).
        let prices_key = format!("arbx:token_prices:{}", self.chain_id);
        let channel = format!("arbx:prices:updated:{}", self.chain_id);

        let payload = serde_json::json!({
            "source": "chainlink_ws",
            "count": 1,
            "token": feed.symbol,
        });

        let _: Result<(), _> = redis::pipe()
            .atomic()
            .hset(&prices_key, &feed.symbol, price)
            .expire(&prices_key, 60)
            .publish(&channel, payload.to_string())
            .query_async(redis)
            .await;

        debug!(
            event = "chainlink_sub.price_updated",
            chain_id = self.chain_id,
            token = %feed.symbol,
            price = price,
            oracle = %oracle_addr,
            "AnswerUpdated → Redis + PUBLISH (G-PRICE-3 event-driven)"
        );
    }
}

fn u256_to_f64(v: &U256) -> f64 {
    if v.bits() <= 128 {
        v.as_u128() as f64
    } else {
        v.to_string().parse().unwrap_or(f64::MAX)
    }
}
