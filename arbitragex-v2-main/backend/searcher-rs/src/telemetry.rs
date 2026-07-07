//! Telemetría, Métricas y Alertas para ArbitrageX
//!
//! Implementa la exportación de logs estructurados (JSON),
//! exposición de métricas (Prometheus) y envíos de alertas a Discord/Telegram.

use tracing::{info, error, Level};
use tracing_subscriber::fmt;
use tracing_subscriber::prelude::*;

/// Configura `tracing` para escupir logs en formato JSON estructurado
pub fn init_telemetry_json() {
    let fmt_layer = fmt::layer()
        .json()
        .with_current_span(false)
        .with_span_list(true);

    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new("info"))
        .with(fmt_layer)
        .init();

    info!("[Telemetry] Structured JSON logging initialized.");
}

/// Envía una alerta crítica a un webhook externo (ej: Discord)
pub async fn send_discord_alert(webhook_url: &str, title: &str, message: &str) {
    if webhook_url.is_empty() {
        return;
    }

    let client = reqwest::Client::new();
    let payload = serde_json::json!({
        "content": null,
        "embeds": [
            {
                "title": title,
                "description": message,
                "color": 16711680 // Rojo
            }
        ]
    });

    match client.post(webhook_url).json(&payload).send().await {
        Ok(_) => info!("[Alert] Discord alert sent: {}", title),
        Err(e) => error!("[Alert] Failed to send Discord alert: {}", e),
    }
}
