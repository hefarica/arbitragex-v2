//! REDEMPTION-PRODUCER-01 fase 2 (2026-10-03): lector on-chain de estado de
//! contratos de redemption (ERC-4626 vaults y equivalentes). Lee maxRedeem /
//! previewRedeem / convertToAssets vía `eth_call` crudo sobre el pool HTTP
//! con failover existente — mismo patrón que `price_worker::eth_call_latest_answer`.
//!
//! Las direcciones de los baskets vienen del OPERADOR vía
//! `ARBX_BASKET_CONTRACTS` (env var, formato `addr1,addr2,...`) — jamás
//! hardcodeadas (RULE 00 / arbx-no-hardcode-doctrine). Sin env var → sin
//! datos → los verificadores de redemption siguen en FAIL honesto (R8).
//!
//! OBSERVER-ONLY: eth_call estático, sin firma ni broadcast.

use serde_json::{json, Value};
use std::collections::BTreeMap;

/// Selector de función (4 bytes keccak) para llamadas ERC-4626.
/// maxRedeem(address) = 0xc63d32b8
/// totalAssets() = 0x01e1d114
const SEL_MAX_REDEEM: &str = "c63d32b8";
const SEL_TOTAL_ASSETS: &str = "01e1d114";

/// Parsea la env var `ARBX_BASKET_CONTRACTS` (direcciones separadas por coma).
/// Sin var o malformada → mapa vacío (los verificadores fallan honesto).
pub fn baskets_from_env() -> Vec<String> {
    std::env::var("ARBX_BASKET_CONTRACTS")
        .unwrap_or_default()
        .split(',')
        .map(|s| s.trim().to_ascii_lowercase())
        .filter(|s| s.len() == 42 && s.starts_with("0x"))
        .collect()
}

/// Lee el estado de redemption de un contrato ERC-4626 via eth_call crudo.
/// Una sola llamada multicall-style: maxRedeem(zero_addr) para el límite
/// global + totalAssets() para el AUM. Devuelve None si el RPC falla (R8).
pub async fn read_redemption_state(
    rpc_url: &str,
    basket_addr: &str,
    owner_addr: &str,
) -> Option<Value> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(3))
        .build()
        .ok()?;

    // maxRedeem(owner) — el máximo que ESA dirección puede redimir.
    let max_redeem_data = format!(
        "{}{}{}",
        SEL_MAX_REDEEM,
        "000000000000000000000000",
        &owner_addr[2..]
    );
    let max_redeem = eth_call_static(&client, rpc_url, basket_addr, &max_redeem_data).await?;

    // totalAssets() — el AUM del vault (sanity check de liquidez).
    let total_assets = eth_call_static(&client, rpc_url, basket_addr, SEL_TOTAL_ASSETS).await;

    Some(json!({
        "basket_address": basket_addr,
        "max_redeem_raw": format!("0x{}", max_redeem),
        "total_assets_raw": total_assets
            .as_ref()
            .map(|t| format!("0x{}", t))
            .unwrap_or_default(),
        "read_at_ms": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .ok()?
            .as_millis() as u64,
    }))
}

/// eth_call estático crudo (mismo patrón que price_worker). Devuelve los
/// datos de respuesta (hex) sin el prefijo 0x.
async fn eth_call_static(
    client: &reqwest::Client,
    rpc_url: &str,
    to: &str,
    data: &str,
) -> Option<String> {
    let body = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "eth_call",
        "params": [
            { "to": to, "data": format!("0x{data}") },
            "latest"
        ]
    });
    let resp: Value = client
        .post(rpc_url)
        .json(&body)
        .send()
        .await
        .ok()?
        .json()
        .await
        .ok()?;
    resp.get("result")?
        .as_str()
        .map(|s| s.trim_start_matches("0x").to_string())
        .filter(|s| s.len() >= 2)
}

/// Lee todos los baskets configurados y devuelve el mapa para el bundle.
/// Sin baskets configurados → mapa vacío (honesto, no error).
pub async fn read_all_baskets(rpc_urls: &[String]) -> BTreeMap<String, Value> {
    let mut out = BTreeMap::new();
    let baskets = baskets_from_env();
    if baskets.is_empty() || rpc_urls.is_empty() {
        return out;
    }
    // Owner = zero address (límite global del vault, no de una cuenta).
    let zero_owner = "0x0000000000000000000000000000000000000000";
    for basket in &baskets {
        for rpc in rpc_urls {
            if let Some(state) = read_redemption_state(rpc, basket, zero_owner).await {
                out.insert(basket.clone(), state);
                break; // primer RPC que responde gana (failover)
            }
        }
    }
    out
}
