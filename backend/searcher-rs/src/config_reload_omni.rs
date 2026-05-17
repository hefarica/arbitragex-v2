//! =========================================================================
//! OMEGA config_reload_omni — multi-channel Arc-swap reload + runtime ack
//! =========================================================================
//!
//! Extends `config_reload.rs` (which only handles `arbx:config:chains:reload`)
//! to cover the full 11-channel canonical fabric:
//!
//!   arbx:config:rpcs:reload         arbx:config:rpcs:<id>:reload
//!   arbx:config:dexes:reload        arbx:config:dexes:<id>:reload
//!   arbx:config:tokens:reload       arbx:config:tokens:<id>:reload
//!   arbx:config:pools:reload        arbx:config:pools:<id>:reload
//!   arbx:config:wallets:reload      arbx:config:wallets:<id>:reload
//!   arbx:config:strategies:reload   arbx:config:strategies:<id>:reload
//!   arbx:config:risk:reload
//!   arbx:config:capital:reload
//!   arbx:config:contracts:reload    arbx:config:contracts:<id>:reload
//!   arbx:config:relays:reload       arbx:config:relays:<id>:reload
//!   arbx:config:agents:reload
//!
//! Each event triggers per-resource Arc-swap with O(1) hand-off (no lock
//! contention on the hot path) and emits a runtime_ack POST to the
//! api-server so the frontend can render the WAITING_RUNTIME_ACK → VERIFIED
//! state transition.
//!
//! Doctrine:
//!   • Zero-Mocks   — every code path performs real Redis + HTTP I/O.
//!   • Ghost Protocol — no outbound telemetry; ack stays inside cluster.
//!   • Lexicón Absoluto — Topological Yield / Holonomic Resolution.

use std::sync::Arc;
use std::time::Instant;

use anyhow::{Context, Result};
use arc_swap::ArcSwap;
use redis::AsyncCommands;
use reqwest::Client as HttpClient;
use serde::{Deserialize, Serialize};
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};

/// Reload event mirror of `backend/api-server/src/lib/registry-engine.ts`.
#[derive(Debug, Clone, Deserialize)]
pub struct ReloadEvent {
    pub event_id: String,
    pub idempotency_key: String,
    pub resource: String,
    pub chain_id: Option<i64>,
    pub action: String,
    pub entity_id: String,
    pub config_hash_before: Option<String>,
    pub config_hash_after: String,
    pub actor: String,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize)]
struct RuntimeAck {
    event_id: String,
    resource: String,
    chain_id: Option<i64>,
    idempotency_key: String,
    config_hash_before: Option<String>,
    config_hash_after: String,
    worker_id: String,
    layer: &'static str,
    status: &'static str,
    latency_ms: Option<u64>,
    error: Option<String>,
}

/// Trait every per-resource catalog must implement to receive hot-reloads.
pub trait ReloadableCatalog: Send + Sync + 'static {
    /// Resource label (must match `ReloadEvent::resource`).
    fn resource(&self) -> &'static str;
    /// Apply the event to the in-memory state.  Returns the new SHA-256.
    fn apply(&self, evt: &ReloadEvent) -> Result<String>;
}

/// Generic Arc-swap wrapper that publishes acks back to the api-server.
pub struct OmniReloadCoordinator {
    pub worker_id: String,
    pub api_base: String,
    pub http: HttpClient,
    pub catalogs: Arc<ArcSwap<Vec<Arc<dyn ReloadableCatalog>>>>,
}

impl OmniReloadCoordinator {
    pub fn new(worker_id: String, api_base: String) -> Self {
        Self {
            worker_id,
            api_base,
            http: HttpClient::new(),
            catalogs: Arc::new(ArcSwap::from_pointee(Vec::new())),
        }
    }

    pub fn register(&self, catalog: Arc<dyn ReloadableCatalog>) {
        let mut next = (**self.catalogs.load()).clone();
        next.push(catalog);
        self.catalogs.store(Arc::new(next));
    }

    /// Channels this coordinator subscribes to.
    pub fn channels() -> Vec<&'static str> {
        vec![
            "arbx:config:rpcs:reload",
            "arbx:config:dexes:reload",
            "arbx:config:tokens:reload",
            "arbx:config:pools:reload",
            "arbx:config:wallets:reload",
            "arbx:config:strategies:reload",
            "arbx:config:risk:reload",
            "arbx:config:capital:reload",
            "arbx:config:contracts:reload",
            "arbx:config:relays:reload",
            "arbx:config:agents:reload",
        ]
    }

    pub async fn spawn(self: Arc<Self>, redis_url: String) -> Result<JoinHandle<()>> {
        let client = redis::Client::open(redis_url.clone())
            .context("OmniReloadCoordinator: redis client open")?;
        let mut pubsub = client
            .get_async_pubsub()
            .await
            .context("OmniReloadCoordinator: pubsub connection")?;
        for ch in Self::channels() {
            pubsub
                .subscribe(ch)
                .await
                .with_context(|| format!("subscribe {ch}"))?;
        }
        // Per-chain patterns: use psubscribe wildcard
        pubsub
            .psubscribe("arbx:config:*:*:reload")
            .await
            .context("psubscribe per-chain channels")?;

        let this = self.clone();
        let handle = tokio::spawn(async move {
            let mut stream = pubsub.on_message();
            while let Some(msg) = futures_util::StreamExt::next(&mut stream).await {
                let payload: String = match msg.get_payload() {
                    Ok(p) => p,
                    Err(e) => {
                        error!("omni_reload: bad payload: {e}");
                        continue;
                    }
                };
                let evt: ReloadEvent = match serde_json::from_str(&payload) {
                    Ok(e) => e,
                    Err(e) => {
                        error!("omni_reload: bad json: {e}");
                        continue;
                    }
                };
                this.dispatch(evt).await;
            }
        });
        Ok(handle)
    }

    async fn dispatch(&self, evt: ReloadEvent) {
        let t0 = Instant::now();
        let catalogs = self.catalogs.load();
        let target = catalogs.iter().find(|c| c.resource() == evt.resource);
        let (status, error) = match target {
            None => {
                warn!(
                    resource = %evt.resource,
                    event_id = %evt.event_id,
                    "omni_reload: no catalog registered, ack as received only"
                );
                ("received", None)
            }
            Some(cat) => match cat.apply(&evt) {
                Ok(hash) => {
                    debug!(
                        resource = %evt.resource,
                        chain_id = ?evt.chain_id,
                        hash = %hash,
                        "omni_reload: applied"
                    );
                    ("applied", None)
                }
                Err(e) => {
                    error!(
                        resource = %evt.resource,
                        event_id = %evt.event_id,
                        "omni_reload: apply failed: {e}"
                    );
                    ("failed", Some(e.to_string()))
                }
            },
        };
        let latency = t0.elapsed().as_millis() as u64;
        self.publish_ack(&evt, status, error, latency).await;
    }

    async fn publish_ack(
        &self,
        evt: &ReloadEvent,
        status: &'static str,
        error: Option<String>,
        latency_ms: u64,
    ) {
        let ack = RuntimeAck {
            event_id: evt.event_id.clone(),
            resource: evt.resource.clone(),
            chain_id: evt.chain_id,
            idempotency_key: evt.idempotency_key.clone(),
            config_hash_before: evt.config_hash_before.clone(),
            config_hash_after: evt.config_hash_after.clone(),
            worker_id: self.worker_id.clone(),
            layer: "arc_swap",
            status,
            latency_ms: Some(latency_ms),
            error,
        };
        let url = format!(
            "{}/api/system/runtime-ack",
            self.api_base.trim_end_matches('/')
        );
        match self.http.post(&url).json(&ack).send().await {
            Ok(r) if r.status().is_success() => {}
            Ok(r) => warn!("omni_reload: ack non-2xx: {}", r.status()),
            Err(e) => error!("omni_reload: ack POST failed: {e}"),
        }
        info!(
            event_id = %evt.event_id,
            resource = %evt.resource,
            status = %status,
            latency_ms = latency_ms,
            "omni_reload: ack emitted"
        );
    }
}
