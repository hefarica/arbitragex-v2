//! Adaptive scoring feedback loop — Sprint BE-3.5.
//!
//! `FeedbackChannel` subscribes to the Redis pub/sub pattern
//! `arbx:scoring:updates:*` and caches the latest `AdaptiveSignal` per
//! `(strategy_kind, chain_id)`.  `ConfigAwareEvaluator` reads from this cache
//! before falling back to the SQL `StrategyScoresCache` path (BE-01 Sprint B).
//!
//! ## Failure modes (both are fail-honest)
//! - Recon goes down → no new signals arrive → spine continues using the SQL
//!   path for p_fail (unchanged behaviour from BE-01).
//! - Spine restarts → cache is empty → first signal batch repopulates it.
//!   Until then the SQL path applies for every evaluation.
//!
//! ## Cache TTL
//! Signals older than 300 seconds are considered stale and ignored by
//! `ConfigAwareEvaluator`.  300 s is chosen as 5× the typical aggregator
//! interval (60 s), giving 5 full recon cycles of headroom before the spine
//! must fall back to SQL.  A signal that is exactly 300 s old is also
//! considered stale (`elapsed >= Duration::from_secs(300)`).
//!
//! ## Pub/sub channel
//! Pattern: `arbx:scoring:updates:<chain_id>` (published by recon aggregator).
//! The subscriber uses `PSUBSCRIBE arbx:scoring:updates:*` to receive all chains.

use serde::Deserialize;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, SystemTime};
use tokio::task::JoinHandle;
use tracing::{error, info, warn};

/// Maximum age of a cached signal before it is treated as stale.
pub const SIGNAL_TTL: Duration = Duration::from_secs(300);

/// Pub/sub channel pattern that the spine subscribes to.
pub const CHANNEL_PATTERN: &str = "arbx:scoring:updates:*";

/// Scoring update published by recon's aggregator after writing `strategy_scores`.
///
/// All fields are required; a missing field in the JSON payload causes the
/// message to be silently dropped (logged at WARN level).
#[derive(Debug, Clone, Deserialize)]
pub struct AdaptiveSignal {
    pub strategy_kind: String,
    pub chain_id: u64,
    /// `revert_rate` expressed as a fraction in \[0.0, 1.0\].
    /// This maps directly to `p_fail` in `RoiCalculationParams`.
    pub revert_rate: f64,
    /// Number of executions in the aggregation window.
    pub sample_count: i64,
    /// Wall-clock time the subscriber received and stored the signal.
    /// Set internally by `FeedbackChannel`; not present in the raw payload.
    /// Defaults to `UNIX_EPOCH` on deserialisation (immediately considered stale).
    #[serde(skip, default = "SystemTime::now")]
    pub received_at: SystemTime,
}

impl AdaptiveSignal {
    /// Returns `true` when the signal was received within the TTL window.
    pub fn is_fresh(&self) -> bool {
        self.received_at.elapsed().unwrap_or(Duration::MAX) < SIGNAL_TTL
    }
}

type SignalMap = HashMap<(String, u64), AdaptiveSignal>;

/// Thread-safe, in-memory store of the latest `AdaptiveSignal` per
/// `(strategy_kind, chain_id)`.
///
/// Shared between the subscriber task (writer) and `ConfigAwareEvaluator`
/// (reader) via `Arc<FeedbackChannel>`.
#[derive(Clone)]
pub struct FeedbackChannel {
    inner: Arc<RwLock<SignalMap>>,
}

impl FeedbackChannel {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Spawn a background task that subscribes to Redis pub/sub and populates
    /// the inner map on every message.
    ///
    /// The handle is detached; the task runs until the process exits or Redis
    /// becomes permanently unavailable.  Reconnection is not yet implemented —
    /// on connection failure the task exits and the cache stops updating.
    /// The spine will continue to operate using the SQL fallback path.
    pub fn spawn_subscriber(self, redis_url: String) -> JoinHandle<()> {
        tokio::spawn(async move {
            // Open a dedicated connection for pub/sub.  Cannot share the
            // connection manager's multiplexed connection for pub/sub.
            let client = match redis::Client::open(redis_url.as_str()) {
                Ok(c) => c,
                Err(e) => {
                    error!(
                        event = "feedback.subscriber.client_error",
                        error = %e,
                        "cannot open Redis client for feedback subscriber"
                    );
                    return;
                }
            };

            let mut pubsub = match client.get_async_pubsub().await {
                Ok(p) => p,
                Err(e) => {
                    error!(
                        event = "feedback.subscriber.connect_error",
                        error = %e,
                        "cannot get async pubsub connection for feedback subscriber"
                    );
                    return;
                }
            };

            if let Err(e) = pubsub.psubscribe(CHANNEL_PATTERN).await {
                error!(
                    event = "feedback.subscriber.psubscribe_error",
                    error = %e,
                    pattern = CHANNEL_PATTERN,
                    "PSUBSCRIBE failed"
                );
                return;
            }

            info!(
                event = "feedback.subscriber.ready",
                pattern = CHANNEL_PATTERN,
                "feedback subscriber active"
            );

            let mut stream = pubsub.on_message();
            while let Some(msg) = {
                use futures_util::StreamExt;
                stream.next().await
            } {
                let payload_str: String = match msg.get_payload() {
                    Ok(s) => s,
                    Err(e) => {
                        warn!(
                            event = "feedback.subscriber.payload_error",
                            error = %e,
                            "could not decode pub/sub message payload"
                        );
                        continue;
                    }
                };

                let mut signal: AdaptiveSignal = match serde_json::from_str(&payload_str) {
                    Ok(s) => s,
                    Err(e) => {
                        warn!(
                            event = "feedback.subscriber.parse_error",
                            error = %e,
                            "could not parse AdaptiveSignal JSON"
                        );
                        continue;
                    }
                };

                // Validate the signal before storing.
                if !(0.0..=1.0).contains(&signal.revert_rate) {
                    warn!(
                        event = "feedback.subscriber.invalid_revert_rate",
                        strategy_kind = %signal.strategy_kind,
                        chain_id = signal.chain_id,
                        revert_rate = signal.revert_rate,
                        "revert_rate out of [0,1] — discarding signal"
                    );
                    continue;
                }

                signal.received_at = SystemTime::now();

                let key = (signal.strategy_kind.clone(), signal.chain_id);
                // std::sync::RwLock::write() — instant; subscriber is the only writer.
                // Poisoned lock is treated as fatal (panic propagates to task boundary).
                self.inner
                    .write()
                    .expect("feedback RwLock poisoned")
                    .insert(key, signal);
            }

            warn!(
                event = "feedback.subscriber.stream_ended",
                "pub/sub message stream ended — subscriber task exiting"
            );
        })
    }

    /// Return a fresh signal for `(strategy_kind, chain_id)`, or `None` when:
    /// - no signal has been received yet for this key, or
    /// - the most-recent signal is older than `SIGNAL_TTL` (300 s).
    ///
    /// This is a **synchronous** read via `std::sync::RwLock`; compatible with
    /// `ConfigAwareEvaluator::evaluate()` which must remain sync on the hot path.
    /// Callers should treat `None` as "fall back to `StrategyScoresCache`".
    pub fn get(&self, strategy_kind: &str, chain_id: u64) -> Option<AdaptiveSignal> {
        let key = (strategy_kind.to_string(), chain_id);
        let guard = self.inner.read().expect("feedback RwLock poisoned");
        let signal = guard.get(&key)?;
        if signal.is_fresh() {
            Some(signal.clone())
        } else {
            None
        }
    }

    /// Direct insert used in unit tests to bypass the pub/sub transport.
    #[cfg(test)]
    pub fn insert_for_test(&self, signal: AdaptiveSignal) {
        let key = (signal.strategy_kind.clone(), signal.chain_id);
        self.inner
            .write()
            .expect("feedback RwLock poisoned")
            .insert(key, signal);
    }
}

impl Default for FeedbackChannel {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_signal(strategy_kind: &str, chain_id: u64, revert_rate: f64) -> AdaptiveSignal {
        AdaptiveSignal {
            strategy_kind: strategy_kind.to_string(),
            chain_id,
            revert_rate,
            sample_count: 50,
            received_at: SystemTime::now(),
        }
    }

    #[test]
    fn feedback_channel_stores_and_retrieves_signal() {
        let ch = FeedbackChannel::new();
        let signal = make_signal("triangular", 1, 0.12);
        ch.insert_for_test(signal);

        let retrieved = ch.get("triangular", 1);
        assert!(retrieved.is_some(), "expected a stored signal");
        let s = retrieved.unwrap();
        assert_eq!(s.strategy_kind, "triangular");
        assert_eq!(s.chain_id, 1);
        assert!((s.revert_rate - 0.12).abs() < 1e-9);
    }

    #[test]
    fn different_keys_do_not_collide() {
        let ch = FeedbackChannel::new();
        ch.insert_for_test(make_signal("triangular", 1, 0.10));
        ch.insert_for_test(make_signal("dex_arb_v2v2", 1, 0.25));
        ch.insert_for_test(make_signal("triangular", 137, 0.05));

        let r1 = ch.get("triangular", 1).unwrap();
        let r2 = ch.get("dex_arb_v2v2", 1).unwrap();
        let r3 = ch.get("triangular", 137).unwrap();

        assert!((r1.revert_rate - 0.10).abs() < 1e-9);
        assert!((r2.revert_rate - 0.25).abs() < 1e-9);
        assert!((r3.revert_rate - 0.05).abs() < 1e-9);
    }

    #[test]
    fn stale_signal_returns_none() {
        let ch = FeedbackChannel::new();
        // Simulate a signal received 301 seconds ago.
        let mut signal = make_signal("triangular", 1, 0.15);
        signal.received_at = SystemTime::now()
            .checked_sub(Duration::from_secs(301))
            .expect("time subtraction");
        ch.insert_for_test(signal);

        let result = ch.get("triangular", 1);
        assert!(result.is_none(), "stale signal must not be returned");
    }

    #[test]
    fn missing_key_returns_none() {
        let ch = FeedbackChannel::new();
        assert!(ch.get("unknown_strategy", 1).is_none());
    }

    #[test]
    fn fresh_signal_within_ttl_is_returned() {
        let ch = FeedbackChannel::new();
        // 299 seconds old — still within the 300-second TTL.
        let mut signal = make_signal("dex_arb_v2v2", 42, 0.08);
        signal.received_at = SystemTime::now()
            .checked_sub(Duration::from_secs(299))
            .expect("time subtraction");
        ch.insert_for_test(signal);

        let result = ch.get("dex_arb_v2v2", 42);
        assert!(result.is_some(), "signal at 299s must still be fresh");
    }
}
