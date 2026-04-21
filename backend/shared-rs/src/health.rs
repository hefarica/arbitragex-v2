//! `/health` + `/metrics` router, mountable by any Axum app.

use axum::{routing::get, Json, Router};
use serde::Serialize;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone)]
pub struct ServiceInfo {
    pub name: &'static str,
    pub version: &'static str,
    pub started_at_unix: u64,
}

impl ServiceInfo {
    pub fn new(name: &'static str, version: &'static str) -> Self {
        let started_at_unix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        Self { name, version, started_at_unix }
    }
}

#[derive(Debug, Serialize)]
struct HealthBody {
    ok: bool,
    service: &'static str,
    version: &'static str,
    uptime_s: u64,
}

pub fn build_health_router(info: ServiceInfo) -> Router {
    Router::new()
        .route("/health", get(move || {
            let info = info.clone();
            async move {
                let now = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
                let uptime = now.saturating_sub(info.started_at_unix);
                Json(HealthBody {
                    ok: true,
                    service: info.name,
                    version: info.version,
                    uptime_s: uptime,
                })
            }
        }))
        .route("/metrics", get(crate::metrics::metrics_handler))
}
