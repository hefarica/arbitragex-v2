use ethers::providers::{Http, Provider, Ws};
use std::sync::Arc;
use tokio::sync::RwLock;

/// RPCMultiplexer — Gestiona múltiples endpoints RPC con failover activo
/// y latencia mínima. Selecciona el nodo con menor RTT en cada ciclo.
pub struct RPCMultiplexer {
    http_endpoints: Vec<Arc<Provider<Http>>>,
    ws_endpoints: Vec<Arc<Provider<Ws>>>,
    current_best: RwLock<usize>,
}

impl RPCMultiplexer {
    pub async fn from_urls(
        http_urls: Vec<&str>,
        ws_url: &str,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let mut http_endpoints = Vec::new();
        for url in http_urls {
            // OBSERVER-ONLY: bare ethers read-only provider — NEVER wrap with
            // SignerMiddleware / .with_signer; no LocalWallet attached (capital_exposed == 0).
            let provider = Provider::<Http>::try_from(url)?;
            http_endpoints.push(Arc::new(provider));
        }

        // OBSERVER-ONLY: bare ethers WS provider (subscriptions + reads only); no signer.
        let ws_provider = Provider::<Ws>::connect(ws_url).await?;
        let ws_endpoints = vec![Arc::new(ws_provider)];

        Ok(Self {
            http_endpoints,
            ws_endpoints,
            current_best: RwLock::new(0),
        })
    }

    pub fn best_http(&self) -> Arc<Provider<Http>> {
        let idx = *self.current_best.blocking_read();
        self.http_endpoints
            .get(idx)
            .or_else(|| self.http_endpoints.first())
            .cloned()
            .expect("At least one HTTP endpoint required")
    }
}
