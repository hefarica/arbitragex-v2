//! Topology Vault runtime for RPC/WS hot-reload.
//!
//! This module mirrors the `api-server` Topology Vault contract published on
//! Redis channel `arbx:topology:mutation`.  It performs only server-side RPC
//! connection work: full URLs are accepted from Redis/internal HTTP fallback,
//! validated, handshaken, and then swapped into an atomic pointer.  Any logs or
//! snapshots generated here redact URL credentials and API keys.

use anyhow::{Context, Result};
use arc_swap::ArcSwapOption;
use async_trait::async_trait;
use futures_util::{future::join_all, StreamExt};
use once_cell::sync::Lazy;
use prometheus::{register_int_gauge_vec_with_registry, IntGaugeVec};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use serde::{Deserialize, Serialize};
use shared_rs::rpc_failover::{HttpRpcPool, WsEndpoint};
use std::sync::Arc;
use std::time::Duration;
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

use crate::chain_client::{is_alchemy_endpoint, WsChainClient};

pub const TOPOLOGY_MUTATION_CHANNEL: &str = "arbx:topology:mutation";

static TOPOLOGY_APPLIED_VERSION: Lazy<Result<IntGaugeVec, prometheus::Error>> = Lazy::new(|| {
    register_int_gauge_vec_with_registry!(
        "searcher_topology_applied_version_id",
        "Last topology version_id atomically applied by searcher-rs.",
        &["scope", "chain_id"],
        shared_rs::metrics::REGISTRY
    )
});

static TOPOLOGY_WS_READY: Lazy<Result<IntGaugeVec, prometheus::Error>> = Lazy::new(|| {
    register_int_gauge_vec_with_registry!(
        "searcher_topology_ws_ready_clients",
        "Number of WS clients validated before the last topology swap.",
        &["scope", "chain_id"],
        shared_rs::metrics::REGISTRY
    )
});

static TOPOLOGY_HTTP_READY: Lazy<Result<IntGaugeVec, prometheus::Error>> = Lazy::new(|| {
    register_int_gauge_vec_with_registry!(
        "searcher_topology_http_ready_clients",
        "Number of HTTP clients validated before the last topology swap.",
        &["scope", "chain_id"],
        shared_rs::metrics::REGISTRY
    )
});

pub fn init_topology_metrics() {
    Lazy::force(&TOPOLOGY_APPLIED_VERSION);
    Lazy::force(&TOPOLOGY_WS_READY);
    Lazy::force(&TOPOLOGY_HTTP_READY);
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum MempoolMode {
    Auto,
    Filtered,
    Firehose,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TopologyMutationCommitted {
    pub event: String,
    pub mutation_id: String,
    pub version_id: u64,
    pub scope: String,
    pub chain_id: u64,
    pub mempool_mode: MempoolMode,
    pub rpc_http_1: String,
    pub rpc_ws_1: String,
    pub checksum: String,
    pub committed_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TopologySnapshotResponse {
    pub ok: bool,
    #[serde(default)]
    pub topology: Option<serde_json::Value>,
    #[serde(default)]
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderEndpoint {
    pub name: String,
    pub url: String,
}

pub struct TopologyClients {
    pub version_id: u64,
    pub scope: String,
    pub chain_id: u64,
    #[allow(dead_code)]
    pub mempool_mode: MempoolMode,
    #[allow(dead_code)]
    pub checksum: String,
    pub http_pool: Arc<HttpRpcPool>,
    pub ws_endpoints: Vec<WsEndpoint>,
}

impl TopologyClients {
    pub fn redacted_ws_urls(&self) -> Vec<String> {
        self.ws_endpoints
            .iter()
            .map(|e| redact_url(&e.url))
            .collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TopologyApplyDecision {
    Applied,
    IdempotentSkip,
}

#[derive(Debug, thiserror::Error)]
pub enum TopologyError {
    #[error("unexpected_event")]
    UnexpectedEvent,
    #[error("invalid_provider_name")]
    InvalidProviderName,
    #[error("invalid_url")]
    InvalidUrl,
    #[error("invalid_http_scheme")]
    InvalidHttpScheme,
    #[error("invalid_ws_scheme")]
    InvalidWsScheme,
    #[error("userinfo_forbidden")]
    UserInfoForbidden,
    #[error("proxy_forbidden")]
    ProxyForbidden,
    #[error("no_providers")]
    NoProviders,
    #[error("filtered_requires_first_ws_alchemy")]
    FilteredRequiresAlchemy,
}

#[async_trait]
pub trait TopologyClientFactory: Send + Sync {
    async fn build(&self, event: &TopologyMutationCommitted) -> Result<TopologyClients>;
}

pub struct LiveTopologyClientFactory;

#[async_trait]
impl TopologyClientFactory for LiveTopologyClientFactory {
    async fn build(&self, event: &TopologyMutationCommitted) -> Result<TopologyClients> {
        validate_mutation(event)?;
        let http_pool = Arc::new(
            HttpRpcPool::from_csv(event.chain_id, &event.rpc_http_1)
                .await
                .context("build HTTP RPC pool for topology mutation")?,
        );
        let ws = parse_provider_list(&event.rpc_ws_1, ProviderFamily::Ws)?;
        let handshakes = ws.iter().map(|endpoint| {
            let url = endpoint.url.clone();
            let chain_id = event.chain_id;
            async move { WsChainClient::connect(chain_id, &url).await.map(|_| ()) }
        });
        let results = join_all(handshakes).await;
        for res in results {
            res.context("WS topology handshake failed")?;
        }
        Ok(TopologyClients {
            version_id: event.version_id,
            scope: event.scope.clone(),
            chain_id: event.chain_id,
            mempool_mode: event.mempool_mode,
            checksum: event.checksum.clone(),
            http_pool,
            ws_endpoints: ws
                .into_iter()
                .map(|e| WsEndpoint {
                    name: e.name,
                    url: e.url,
                })
                .collect(),
        })
    }
}

#[derive(Clone)]
pub struct TopologyRuntime {
    current: Arc<ArcSwapOption<TopologyClients>>,
    notify: tokio::sync::watch::Sender<u64>,
}

impl Default for TopologyRuntime {
    fn default() -> Self {
        Self::new()
    }
}

impl TopologyRuntime {
    pub fn new() -> Self {
        let (notify, _) = tokio::sync::watch::channel(0);
        Self {
            current: Arc::new(ArcSwapOption::from(None)),
            notify,
        }
    }

    #[allow(dead_code)]
    pub fn subscribe_versions(&self) -> tokio::sync::watch::Receiver<u64> {
        self.notify.subscribe()
    }

    pub fn current(&self) -> Option<Arc<TopologyClients>> {
        self.current.load_full()
    }

    pub fn current_version(&self) -> u64 {
        self.current().map(|c| c.version_id).unwrap_or(0)
    }

    pub async fn apply_event<F>(
        &self,
        event: TopologyMutationCommitted,
        factory: &F,
    ) -> Result<TopologyApplyDecision>
    where
        F: TopologyClientFactory + ?Sized,
    {
        validate_mutation(&event)?;
        if event.version_id <= self.current_version() {
            info!(
                event = "topology.idempotent_skip",
                mutation_id = %event.mutation_id,
                incoming_version_id = event.version_id,
                current_version_id = self.current_version(),
                "topology mutation ignored because it is stale or duplicate"
            );
            return Ok(TopologyApplyDecision::IdempotentSkip);
        }

        let clients = factory.build(&event).await?;
        let applied_version = clients.version_id;
        let scope = clients.scope.clone();
        let chain_label = clients.chain_id.to_string();
        let http_ready = clients.http_pool.entries.len() as i64;
        let ws_ready = clients.ws_endpoints.len() as i64;
        let redacted_ws = clients.redacted_ws_urls();

        self.current.store(Some(Arc::new(clients)));
        let _ = self.notify.send(applied_version);

        if let Ok(metric) = &*TOPOLOGY_APPLIED_VERSION {
            metric
                .with_label_values(&[scope.as_str(), chain_label.as_str()])
                .set(applied_version as i64);
        }
        if let Ok(metric) = &*TOPOLOGY_HTTP_READY {
            metric
                .with_label_values(&[scope.as_str(), chain_label.as_str()])
                .set(http_ready);
        }
        if let Ok(metric) = &*TOPOLOGY_WS_READY {
            metric
                .with_label_values(&[scope.as_str(), chain_label.as_str()])
                .set(ws_ready);
        }

        info!(
            event = "topology.hot_swap.applied",
            mutation_id = %event.mutation_id,
            version_id = applied_version,
            scope = %scope,
            chain_id = %chain_label,
            checksum = %event.checksum,
            http_ready,
            ws_ready,
            ws_urls = ?redacted_ws,
            "topology clients swapped atomically after successful server-side handshakes"
        );
        Ok(TopologyApplyDecision::Applied)
    }
}

pub struct TopologySubscriber<F> {
    redis_url: String,
    snapshot_url: Option<String>,
    admin_token: Option<String>,
    runtime: TopologyRuntime,
    factory: Arc<F>,
    cancel: CancellationToken,
}

impl<F> TopologySubscriber<F>
where
    F: TopologyClientFactory + 'static,
{
    pub fn new(
        redis_url: String,
        snapshot_url: Option<String>,
        admin_token: Option<String>,
        runtime: TopologyRuntime,
        factory: Arc<F>,
        cancel: CancellationToken,
    ) -> Self {
        Self {
            redis_url,
            snapshot_url,
            admin_token,
            runtime,
            factory,
            cancel,
        }
    }

    pub async fn run(self) -> Result<()> {
        self.apply_durable_fallback().await;
        let client = redis::Client::open(self.redis_url.clone())
            .context("open Redis client for topology pub/sub")?;
        let mut pubsub = tokio::select! {
            _ = self.cancel.cancelled() => return Ok(()),
            res = client.get_async_pubsub() => res.context("get_async_pubsub for topology reload")?,
        };
        tokio::select! {
            _ = self.cancel.cancelled() => return Ok(()),
            res = pubsub.subscribe(TOPOLOGY_MUTATION_CHANNEL) => {
                res.context("subscribe to topology mutation channel")?;
            }
        };
        info!(
            event = "topology.subscribed",
            channel = TOPOLOGY_MUTATION_CHANNEL,
            "topology Pub/Sub subscriber ready"
        );
        let mut stream = pubsub.on_message();
        loop {
            tokio::select! {
                _ = self.cancel.cancelled() => break,
                msg = stream.next() => {
                    let Some(msg) = msg else { break; };
                    let payload: String = match msg.get_payload() {
                        Ok(payload) => payload,
                        Err(e) => {
                            warn!(event = "topology.payload_error", error = %e, "topology Redis payload is not UTF-8 string");
                            continue;
                        }
                    };
                    match serde_json::from_str::<TopologyMutationCommitted>(&payload) {
                        Ok(event) => {
                            if let Err(e) = self.runtime.apply_event(event, self.factory.as_ref()).await {
                                warn!(event = "topology.apply_failed", error = %e, "topology mutation rejected or handshake failed");
                            }
                        }
                        Err(e) => warn!(event = "topology.deserialize_failed", error = %e, "invalid topology mutation payload"),
                    }
                }
            }
        }
        Ok(())
    }

    async fn apply_durable_fallback(&self) {
        let Some(url) = &self.snapshot_url else {
            warn!(
                event = "topology.snapshot.skipped",
                reason = "TOPOLOGY_SNAPSHOT_URL_unset",
                "durable fallback URL not configured; Redis subscriber remains active"
            );
            return;
        };
        match fetch_snapshot(url, self.admin_token.as_deref()).await {
            Ok(Some(event)) => {
                if let Err(e) = self.runtime.apply_event(event, self.factory.as_ref()).await {
                    warn!(event = "topology.snapshot.apply_failed", error = %e, "durable topology snapshot rejected or unavailable");
                }
            }
            Ok(None) => info!(
                event = "topology.snapshot.empty",
                "durable topology snapshot returned empty vault"
            ),
            Err(e) => {
                warn!(event = "topology.snapshot.fetch_failed", error = %e, "durable topology snapshot fetch failed; continuing with Redis subscriber")
            }
        }
    }
}

pub async fn fetch_snapshot(
    url: &str,
    admin_token: Option<&str>,
) -> Result<Option<TopologyMutationCommitted>> {
    let mut headers = HeaderMap::new();
    if let Some(token) = admin_token {
        let value = HeaderValue::from_str(token).context("invalid topology admin token header")?;
        headers.insert(HeaderName::from_static("x-arbx-admin-token"), value);
    }
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .default_headers(headers)
        .build()
        .context("build topology snapshot HTTP client")?;
    let resp = client
        .get(url)
        .send()
        .await
        .context("GET topology snapshot")?
        .error_for_status()
        .context("topology snapshot HTTP status")?;
    let body = resp
        .json::<TopologySnapshotResponse>()
        .await
        .context("decode topology snapshot")?;
    let Some(topology) = body.topology else {
        return Ok(None);
    };
    if topology
        .get("rpc_http_1")
        .and_then(serde_json::Value::as_str)
        .is_none()
        || topology
            .get("rpc_ws_1")
            .and_then(serde_json::Value::as_str)
            .is_none()
    {
        anyhow::bail!(
            "topology snapshot is redacted or incomplete; configure an internal full-url snapshot endpoint for cold boot"
        );
    }
    let event = serde_json::from_value::<TopologyMutationCommitted>(topology)
        .context("decode full topology snapshot event")?;
    Ok(Some(event))
}

#[derive(Debug, Clone, Copy)]
enum ProviderFamily {
    Http,
    Ws,
}

pub fn validate_mutation(
    event: &TopologyMutationCommitted,
) -> std::result::Result<(), TopologyError> {
    if event.event != "topology.mutation.committed" {
        return Err(TopologyError::UnexpectedEvent);
    }
    let _ = parse_provider_list(&event.rpc_http_1, ProviderFamily::Http)?;
    let ws = parse_provider_list(&event.rpc_ws_1, ProviderFamily::Ws)?;
    if ws.is_empty() {
        return Err(TopologyError::NoProviders);
    }
    if event.mempool_mode == MempoolMode::Filtered && !is_alchemy_name_or_url(&ws[0]) {
        return Err(TopologyError::FilteredRequiresAlchemy);
    }
    Ok(())
}

fn parse_provider_list(
    input: &str,
    family: ProviderFamily,
) -> std::result::Result<Vec<ProviderEndpoint>, TopologyError> {
    let mut providers = Vec::new();
    for (index, token) in input.split(',').enumerate() {
        let trimmed = token.trim();
        if trimmed.is_empty() {
            continue;
        }
        providers.push(parse_provider_token(trimmed, family, index)?);
    }
    if providers.is_empty() {
        return Err(TopologyError::NoProviders);
    }
    Ok(providers)
}

fn parse_provider_token(
    token: &str,
    family: ProviderFamily,
    index: usize,
) -> std::result::Result<ProviderEndpoint, TopologyError> {
    let (name, raw_url) = match token.split_once('=') {
        Some((name, raw_url)) if !name.trim().is_empty() && !raw_url.trim().is_empty() => {
            (name.trim().to_string(), raw_url.trim().to_string())
        }
        Some(_) => return Err(TopologyError::InvalidProviderName),
        None => (format!("provider{}", index + 1), token.trim().to_string()),
    };
    if !is_valid_provider_name(&name) {
        return Err(TopologyError::InvalidProviderName);
    }
    let url = reqwest::Url::parse(&raw_url).map_err(|_| TopologyError::InvalidUrl)?;
    match family {
        ProviderFamily::Http if url.scheme() != "http" && url.scheme() != "https" => {
            return Err(TopologyError::InvalidHttpScheme)
        }
        ProviderFamily::Ws if url.scheme() != "ws" && url.scheme() != "wss" => {
            return Err(TopologyError::InvalidWsScheme)
        }
        _ => {}
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err(TopologyError::UserInfoForbidden);
    }
    if is_explicit_proxy_host(url.host_str().unwrap_or_default()) {
        return Err(TopologyError::ProxyForbidden);
    }
    Ok(ProviderEndpoint { name, url: raw_url })
}

fn is_valid_provider_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

fn is_alchemy_name_or_url(endpoint: &ProviderEndpoint) -> bool {
    endpoint.name.eq_ignore_ascii_case("alchemy") || is_alchemy_endpoint(&endpoint.url)
}

fn is_explicit_proxy_host(host: &str) -> bool {
    let h = host.to_ascii_lowercase();
    h.contains("proxy")
        || h.ends_with(".ngrok.app")
        || h.ends_with(".ngrok-free.app")
        || h.ends_with(".trycloudflare.com")
        || h.ends_with(".workers.dev")
        || h.ends_with(".vercel.app")
        || h.ends_with(".netlify.app")
        || h.ends_with(".onrender.com")
        || h.ends_with(".fly.dev")
        || h.ends_with(".localtunnel.me")
        || h.ends_with(".serveo.net")
}

pub fn redact_url(raw_url: &str) -> String {
    match reqwest::Url::parse(raw_url) {
        Ok(url) => {
            let suffix_source = format!("{}{}", url.path(), url.query().unwrap_or_default());
            let compact: String = suffix_source
                .chars()
                .filter(|c| c.is_ascii_alphanumeric())
                .collect();
            let suffix = if compact.len() >= 4 {
                compact[compact.len() - 4..].to_string()
            } else {
                "set".to_string()
            };
            let port = url.port().map(|p| format!(":{p}")).unwrap_or_default();
            format!(
                "{}://...{}{} /****{}",
                url.scheme(),
                url.host_str().unwrap_or("unknown"),
                port,
                suffix
            )
            .replace(" ", "")
        }
        Err(_) => "<invalid-url-redacted>".to_string(),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    struct MockFactory;

    #[async_trait]
    impl TopologyClientFactory for MockFactory {
        async fn build(&self, event: &TopologyMutationCommitted) -> Result<TopologyClients> {
            validate_mutation(event)?;
            let http = parse_provider_list(&event.rpc_http_1, ProviderFamily::Http)?;
            let ws = parse_provider_list(&event.rpc_ws_1, ProviderFamily::Ws)?;
            Ok(TopologyClients {
                version_id: event.version_id,
                scope: event.scope.clone(),
                chain_id: event.chain_id,
                mempool_mode: event.mempool_mode,
                checksum: event.checksum.clone(),
                http_pool: Arc::new(HttpRpcPool {
                    chain_id: event.chain_id,
                    entries: Vec::with_capacity(http.len()),
                }),
                ws_endpoints: ws
                    .into_iter()
                    .map(|e| WsEndpoint {
                        name: e.name,
                        url: e.url,
                    })
                    .collect(),
            })
        }
    }

    fn event(version_id: u64, ws: &str) -> TopologyMutationCommitted {
        TopologyMutationCommitted {
            event: "topology.mutation.committed".to_string(),
            mutation_id: format!("m-{version_id}"),
            version_id,
            scope: "staging".to_string(),
            chain_id: 1,
            mempool_mode: MempoolMode::Filtered,
            rpc_http_1: "alchemy=https://eth-mainnet.g.alchemy.com/v2/httpabcd".to_string(),
            rpc_ws_1: ws.to_string(),
            checksum: format!("checksum-{version_id}"),
            committed_at: "2026-05-23T00:00:00.000Z".to_string(),
        }
    }

    #[tokio::test]
    async fn applies_newer_event_and_swaps_pointer_atomically() {
        let runtime = TopologyRuntime::new();
        let factory = MockFactory;
        let first = event(100, "alchemy=wss://eth-mainnet.g.alchemy.com/v2/firstabcd");
        let second = event(101, "alchemy=wss://eth-mainnet.g.alchemy.com/v2/secondwxyz");

        assert_eq!(
            runtime.apply_event(first, &factory).await.unwrap(),
            TopologyApplyDecision::Applied
        );
        let before = runtime.current().unwrap();
        assert_eq!(before.version_id, 100);
        assert_eq!(
            before.redacted_ws_urls()[0],
            "wss://...eth-mainnet.g.alchemy.com/****abcd"
        );

        assert_eq!(
            runtime.apply_event(second, &factory).await.unwrap(),
            TopologyApplyDecision::Applied
        );
        let after = runtime.current().unwrap();
        assert_eq!(after.version_id, 101);
        assert_eq!(after.checksum, "checksum-101");
        assert_ne!(Arc::as_ptr(&before), Arc::as_ptr(&after));
    }

    #[tokio::test]
    async fn ignores_stale_or_duplicate_version_id() {
        let runtime = TopologyRuntime::new();
        let factory = MockFactory;
        assert_eq!(
            runtime
                .apply_event(
                    event(200, "alchemy=wss://eth-mainnet.g.alchemy.com/v2/newerabcd"),
                    &factory
                )
                .await
                .unwrap(),
            TopologyApplyDecision::Applied
        );
        let current = runtime.current().unwrap();
        assert_eq!(
            runtime
                .apply_event(
                    event(199, "alchemy=wss://eth-mainnet.g.alchemy.com/v2/staleabcd"),
                    &factory
                )
                .await
                .unwrap(),
            TopologyApplyDecision::IdempotentSkip
        );
        assert_eq!(
            runtime
                .apply_event(
                    event(200, "alchemy=wss://eth-mainnet.g.alchemy.com/v2/dupeabcd"),
                    &factory
                )
                .await
                .unwrap(),
            TopologyApplyDecision::IdempotentSkip
        );
        assert!(Arc::ptr_eq(&current, &runtime.current().unwrap()));
    }

    #[test]
    fn rejects_proxy_and_userinfo_urls() {
        let mut proxied = event(1, "alchemy=wss://gateway.proxy.example/ws");
        assert!(matches!(
            validate_mutation(&proxied),
            Err(TopologyError::ProxyForbidden)
        ));
        proxied.rpc_ws_1 = "alchemy=wss://user:secret@eth-mainnet.g.alchemy.com/v2/key".to_string();
        assert!(matches!(
            validate_mutation(&proxied),
            Err(TopologyError::UserInfoForbidden)
        ));
    }

    #[test]
    fn filtered_mode_requires_first_ws_alchemy() {
        let evt = event(1, "standard=wss://rpc.example.org/wsabcd");
        assert!(matches!(
            validate_mutation(&evt),
            Err(TopologyError::FilteredRequiresAlchemy)
        ));
    }

    #[test]
    fn redaction_preserves_only_safe_suffix() {
        let masked = redact_url("wss://eth-mainnet.g.alchemy.com/v2/supersecretabcd");
        assert_eq!(masked, "wss://...eth-mainnet.g.alchemy.com/****abcd");
        assert!(!masked.contains("supersecret"));
    }
}
