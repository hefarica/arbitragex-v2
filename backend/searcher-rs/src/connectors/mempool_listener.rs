use crate::normalization::u256_converter::u256_to_f64_normalized;
use ethers::providers::{Middleware, Provider, StreamExt, Ws};
use ethers::types::{Transaction, U256};
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

/// mempool_listener — Conector Alloy Anti-Mock
/// Se suscribe a un nodo RPC vía WebSocket y emite transacciones pendientes
/// normalizadas para el SED Engine.
pub struct MempoolListener {
    provider: Provider<Ws>,
    tx_out: mpsc::Sender<NormalizedTx>,
    rpc_url: String,
}

#[derive(Debug, Clone)]
pub struct NormalizedTx {
    pub hash: String,
    pub from: String,
    pub to: Option<String>,
    pub value_normalized: f64,     // ← U256 → f64 (fase 1 de normalización)
    pub gas_price_normalized: f64, // ← U256 → f64
    pub max_fee_normalized: f64,   // ← U256 → f64
    pub data: Vec<u8>,
    pub detected_at: Instant,
}

impl MempoolListener {
    pub async fn new(
        rpc_ws_url: &str,
        tx_out: mpsc::Sender<NormalizedTx>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        // OBSERVER-ONLY: bare ethers WS provider (subscribe_pending_txs + get_transaction
        // only) — NEVER wrap with SignerMiddleware / .with_signer; no Wallet (capital_exposed == 0).
        let provider = Provider::<Ws>::connect(rpc_ws_url).await?;
        Ok(Self {
            provider,
            tx_out,
            rpc_url: rpc_ws_url.to_string(),
        })
    }

    pub async fn run(self) -> Result<(), Box<dyn std::error::Error>> {
        let mut stream = self.provider.subscribe_pending_txs().await?;

        tracing::info!(rpc = %self.rpc_url, "Mempool listener connected");

        while let Some(tx_hash) = stream.next().await {
            let start = Instant::now();

            // Fetch full transaction — SIN MOCK, SIN CACHE DE TX
            match self.provider.get_transaction(tx_hash).await {
                Ok(Some(tx)) => {
                    let normalized = self.normalize(tx)?;
                    let _latency_us = start.elapsed().as_micros() as f64;

                    if self.tx_out.send(normalized).await.is_err() {
                        break; // Receptor caído — salir limpio
                    }
                }
                Ok(None) => {
                    tracing::trace!("TX hash without full data");
                }
                Err(e) => {
                    tracing::error!(error = %e, "RPC error fetching transaction");
                }
            }
        }

        tracing::warn!("Mempool listener stream ended");
        Ok(())
    }

    fn normalize(&self, tx: Transaction) -> Result<NormalizedTx, Box<dyn std::error::Error>> {
        Ok(NormalizedTx {
            hash: format!("{:?}", tx.hash),
            from: format!("{:?}", tx.from),
            to: tx.to.map(|a| format!("{:?}", a)),
            value_normalized: u256_to_f64_normalized(tx.value, 18)?,
            gas_price_normalized: tx
                .gas_price
                .map(|v| u256_to_f64_normalized(v, 18))
                .transpose()?
                .unwrap_or(0.0),
            max_fee_normalized: tx
                .max_fee_per_gas
                .map(|v| u256_to_f64_normalized(v, 18))
                .transpose()?
                .unwrap_or(0.0),
            data: tx.input.to_vec(),
            detected_at: Instant::now(),
        })
    }
}
