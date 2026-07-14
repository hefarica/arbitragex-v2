use axum::{
    extract::Json,
    http::StatusCode,
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct ConfigRequest {
    pub enabled: bool,
    pub chain_id: u64,
}

#[derive(Debug, Serialize)]
pub struct ConfigResponse {
    pub mode: String,
    pub enabled: bool,
    pub chain_id: u64,
    pub can_execute: bool,
    pub mainnet_blocked: bool,
    pub blockers: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

pub async fn get_config() -> impl IntoResponse {
    Json(ConfigResponse {
        mode: "LIVE_TESTNET".to_string(),
        enabled: true,
        chain_id: 11155111,
        can_execute: true,
        mainnet_blocked: true,
        blockers: vec![],
    })
}

pub async fn post_config(Json(req): Json<ConfigRequest>) -> impl IntoResponse {
    if req.chain_id == 1 {
        return (StatusCode::FORBIDDEN, Json(ErrorResponse { error: "MAINNET_BLOCKED".to_string() }));
    }
    if ![11155111u64, 421614, 11155420].contains(&req.chain_id) {
        return (StatusCode::BAD_REQUEST, Json(ErrorResponse { error: "UNSUPPORTED_CHAIN".to_string() }));
    }
    (StatusCode::OK, Json(ConfigResponse {
        mode: "LIVE_TESTNET".to_string(),
        enabled: req.enabled,
        chain_id: req.chain_id,
        can_execute: req.enabled,
        mainnet_blocked: true,
        blockers: vec![],
    }))
}
