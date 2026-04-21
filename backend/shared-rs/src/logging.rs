//! Structured JSON logging via `tracing`.
//!
//! Every line has: timestamp, level, service, trace_id (when present), target, message.

use tracing_subscriber::{prelude::*, EnvFilter};

pub fn init_tracing(service: &str, log_level: &str) -> anyhow::Result<()> {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(log_level));

    let fmt_layer = tracing_subscriber::fmt::layer()
        .json()
        .with_current_span(true)
        .with_span_list(false)
        .with_target(true)
        .with_file(false)
        .with_line_number(false)
        .with_thread_ids(false)
        .with_thread_names(false);

    tracing_subscriber::registry()
        .with(filter)
        .with(fmt_layer)
        .try_init()
        .map_err(|e| anyhow::anyhow!("failed to init tracing for {service}: {e}"))?;

    tracing::info!(service = %service, event = "service.boot", result = "tracing_initialized");
    Ok(())
}
