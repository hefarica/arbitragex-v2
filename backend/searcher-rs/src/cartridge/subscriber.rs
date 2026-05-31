//! CartridgeSubscriber — Redis PubSub listener for cartridge injection events.
//!
//! This module implements the hot-reload pipeline:
//!
//! ```text
//! UI (Strategy Forge) → API Server → Postgres + Redis PUBLISH
//!                                          ↓
//!                              CartridgeSubscriber (this module)
//!                                          ↓
//!                              CartridgeRunner.load_cartridge()
//!                                          ↓
//!                              Active in HashMap (zero downtime)
//! ```
//!
//! ## Redis Channel Contract
//!
//! Channel: `arbx:cartridge:injection`
//!
//! Payload (JSON):
//! ```json
//! {
//!   "cartridge_id": "uuid-v4",
//!   "event_type": "inject" | "update" | "remove" | "pause" | "resume",
//!   "content_hash": "sha256-hex",
//!   "chain_id": 1,
//!   "timestamp": "2026-05-31T12:00:00Z",
//!   "actor": "operator@arbitragex.io"
//! }
//! ```
//!
//! When `event_type` is "inject" or "update", the subscriber fetches the
//! script body from Postgres via the `script_source_key` Redis key:
//!   `arbx:cartridge:source:<cartridge_id>` → full .rhai source string
//!
//! ## Deduplication
//!
//! The subscriber tracks `content_hash` per cartridge_id. If a reload event
//! arrives with the same hash as the currently loaded version, it is skipped
//! (no recompilation). This prevents thundering-herd reloads when multiple
//! nodes receive the same PubSub event.

use crate::cartridge::runner::CartridgeRunner;
use crate::cartridge::types::CartridgeInjectionEvent;
use futures_util::StreamExt;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};

/// Redis PubSub channel for cartridge injection events.
pub const CARTRIDGE_INJECTION_CHANNEL: &str = "arbx:cartridge:injection";

/// Redis key prefix for cartridge source code storage.
pub const CARTRIDGE_SOURCE_KEY_PREFIX: &str = "arbx:cartridge:source:";

/// The subscriber that listens for cartridge injection events and hot-loads them.
pub struct CartridgeSubscriber {
    /// Redis URL for PubSub connection.
    redis_url: String,
    /// Reference to the CartridgeRunner for loading/unloading.
    runner: Arc<CartridgeRunner>,
    /// Cancellation token for graceful shutdown.
    cancel: CancellationToken,
}

impl CartridgeSubscriber {
    /// Creates a new subscriber.
    pub fn new(
        redis_url: String,
        runner: Arc<CartridgeRunner>,
        cancel: CancellationToken,
    ) -> Self {
        Self {
            redis_url,
            runner,
            cancel,
        }
    }

    /// Starts the subscription loop. This is a long-running task that should
    /// be spawned with `tokio::spawn`.
    ///
    /// The loop:
    /// 1. Connects to Redis PubSub on `arbx:cartridge:injection`.
    /// 2. For each message, deserializes the `CartridgeInjectionEvent`.
    /// 3. Based on `event_type`, performs the appropriate action.
    /// 4. On disconnect, reconnects with exponential backoff.
    pub async fn run(&self) {
        info!(
            event = "cartridge_subscriber.start",
            channel = CARTRIDGE_INJECTION_CHANNEL,
            "starting cartridge injection subscriber"
        );

        let mut backoff_ms: u64 = 100;
        let max_backoff_ms: u64 = 30_000;

        loop {
            if self.cancel.is_cancelled() {
                info!(event = "cartridge_subscriber.shutdown", "graceful shutdown");
                return;
            }

            match self.subscribe_loop().await {
                Ok(()) => {
                    // Clean exit (cancellation)
                    return;
                }
                Err(e) => {
                    warn!(
                        event = "cartridge_subscriber.reconnect",
                        error = %e,
                        backoff_ms,
                        "subscriber disconnected, reconnecting"
                    );
                    tokio::time::sleep(tokio::time::Duration::from_millis(backoff_ms)).await;
                    backoff_ms = (backoff_ms * 2).min(max_backoff_ms);
                }
            }
        }
    }

    /// Inner subscription loop. Returns `Err` on disconnect.
    async fn subscribe_loop(&self) -> Result<(), anyhow::Error> {
        let client = redis::Client::open(self.redis_url.clone())?;
        let mut pubsub = client.get_async_pubsub().await?;
        pubsub.subscribe(CARTRIDGE_INJECTION_CHANNEL).await?;

        // Also subscribe to per-chain channels for targeted reloads
        pubsub.psubscribe("arbx:cartridge:injection:*").await?;

        info!(
            event = "cartridge_subscriber.connected",
            "subscribed to cartridge injection channel"
        );

        let mut stream = pubsub.on_message();

        loop {
            tokio::select! {
                _ = self.cancel.cancelled() => {
                    return Ok(());
                }
                msg = stream.next() => {
                    match msg {
                        Some(msg) => {
                            let payload: String = msg.get_payload()?;
                            self.handle_event(&payload).await;
                        }
                        None => {
                            // Stream ended — connection lost
                            return Err(anyhow::anyhow!("pubsub stream ended"));
                        }
                    }
                }
            }
        }
    }

    /// Handles a single injection event from Redis PubSub.
    async fn handle_event(&self, payload: &str) {
        let event: CartridgeInjectionEvent = match serde_json::from_str(payload) {
            Ok(e) => e,
            Err(e) => {
                warn!(
                    event = "cartridge_subscriber.malformed_event",
                    error = %e,
                    payload_len = payload.len(),
                    "ignoring malformed injection event"
                );
                return;
            }
        };

        info!(
            event = "cartridge_subscriber.event_received",
            cartridge_id = %event.cartridge_id,
            event_type = %event.event_type,
            content_hash = %event.content_hash,
            actor = %event.actor,
        );

        match event.event_type.as_str() {
            "inject" | "update" => {
                self.handle_inject_or_update(&event).await;
            }
            "remove" => {
                self.runner.unload_cartridge(&event.cartridge_id).await;
            }
            "pause" => {
                if let Err(e) = self.runner.pause_cartridge(&event.cartridge_id).await {
                    warn!(
                        event = "cartridge_subscriber.pause_failed",
                        cartridge_id = %event.cartridge_id,
                        error = %e,
                    );
                }
            }
            "resume" => {
                if let Err(e) = self.runner.resume_cartridge(&event.cartridge_id).await {
                    warn!(
                        event = "cartridge_subscriber.resume_failed",
                        cartridge_id = %event.cartridge_id,
                        error = %e,
                    );
                }
            }
            other => {
                warn!(
                    event = "cartridge_subscriber.unknown_event_type",
                    event_type = %other,
                    "ignoring unknown event type"
                );
            }
        }
    }

    /// Handles inject/update: fetches source from Redis, compiles, and loads.
    async fn handle_inject_or_update(&self, event: &CartridgeInjectionEvent) {
        // Dedup check: skip if we already have this exact content hash loaded
        if self.runner.has_content_hash(&event.content_hash).await {
            info!(
                event = "cartridge_subscriber.dedup_skip",
                cartridge_id = %event.cartridge_id,
                content_hash = %event.content_hash,
                "skipping reload — same content hash already loaded"
            );
            return;
        }

        // Fetch source from Redis
        let source_key = format!("{}{}", CARTRIDGE_SOURCE_KEY_PREFIX, event.cartridge_id);
        let source = match self.fetch_source(&source_key).await {
            Some(s) => s,
            None => {
                error!(
                    event = "cartridge_subscriber.source_not_found",
                    cartridge_id = %event.cartridge_id,
                    key = %source_key,
                    "cartridge source not found in Redis"
                );
                return;
            }
        };

        // Load the cartridge
        match self
            .runner
            .load_cartridge(&event.cartridge_id, &source, &event.content_hash)
            .await
        {
            Ok(metadata) => {
                info!(
                    event = "cartridge_subscriber.loaded",
                    cartridge_id = %event.cartridge_id,
                    name = %metadata.name,
                    version = %metadata.version,
                    "cartridge hot-loaded successfully"
                );

                // Publish success acknowledgment
                self.publish_ack(&event.cartridge_id, true, None).await;
            }
            Err(e) => {
                error!(
                    event = "cartridge_subscriber.load_failed",
                    cartridge_id = %event.cartridge_id,
                    error = %e,
                    "failed to load cartridge"
                );

                // Publish failure acknowledgment
                self.publish_ack(&event.cartridge_id, false, Some(&e.to_string()))
                    .await;
            }
        }
    }

    /// Fetches cartridge source code from Redis.
    async fn fetch_source(&self, key: &str) -> Option<String> {
        let client = redis::Client::open(self.redis_url.clone()).ok()?;
        let mut conn = client.get_connection_manager().await.ok()?;
        redis::AsyncCommands::get(&mut conn, key).await.ok()?
    }

    /// Publishes a load acknowledgment to Redis for the API server/UI.
    async fn publish_ack(&self, cartridge_id: &str, success: bool, error: Option<&str>) {
        let ack = serde_json::json!({
            "cartridge_id": cartridge_id,
            "success": success,
            "error": error,
            "node": std::env::var("HOSTNAME").unwrap_or_else(|_| "unknown".to_owned()),
            "timestamp": chrono::Utc::now().to_rfc3339(),
        });

        if let Ok(client) = redis::Client::open(self.redis_url.clone()) {
            if let Ok(mut conn) = client.get_connection_manager().await {
                let _: Result<(), _> = redis::AsyncCommands::publish(
                    &mut conn,
                    "arbx:cartridge:ack",
                    ack.to_string(),
                )
                .await;
            }
        }
    }
}
