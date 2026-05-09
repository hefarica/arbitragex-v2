//! PnL engine — fetches receipt, decodes Transfer/Swap logs, computes amount_out delta.
//!
//! Honesty rules:
//! - No receipt → fail_reason="receipt_unavailable", don't persist fake numbers.
//! - No decodable logs → actual_amount_out_wei=None; variance stays null.
//! - pnl_source="native_only" always in S6 (no oracle).

use chrono::Utc;
use ethers::prelude::*;
use ethers::utils::keccak256;
use shared_rs::contracts::{ExecutionResult, Opportunity, ReconReport};
use std::time::Duration;
use tokio::time::timeout;
use tracing::{debug, warn};
use uuid::Uuid;

/// ERC20 Transfer(address indexed from, address indexed to, uint256 value)
/// topic0 = keccak256("Transfer(address,address,uint256)")
fn transfer_topic() -> H256 {
    H256::from(keccak256(b"Transfer(address,address,uint256)"))
}

pub async fn compute(
    opp: &Opportunity,
    exec: &ExecutionResult,
    provider: &Provider<Http>,
    receipt_timeout_ms: u64,
) -> ReconReport {
    let trace_id = opp.trace_id;
    let now = Utc::now();
    let common = |gas_used: Option<String>, gas_price: Option<String>,
                  actual_out: Option<String>, variance_pct: Option<f64>,
                  variance_native: Option<String>, fail_reason: Option<String>| ReconReport {
        opportunity_id: opp.id,
        execution_id: None, // not tracked through stream; derive via tx_hash later
        tx_hash: exec.tx_hash.clone(),
        chain_id: opp.chain_id,
        expected_amount_out_wei: Some(opp.amount_in_wei.clone()), // best-effort proxy in S6
        actual_amount_out_wei: actual_out,
        variance_native_units: variance_native,
        variance_pct,
        expected_profit_usd: opp.expected_profit_usd.unwrap_or(0.0),
        actual_profit_usd: 0.0, // S6: no oracle, leave at 0 with pnl_source=native_only
        pnl_source: "native_only".to_string(),
        actual_gas_used_wei: gas_used,
        actual_gas_price_wei: gas_price,
        fail_reason,
        notes: None,
        created_at: now,
        trace_id,
    };

    let Some(hash) = exec.tx_hash.as_deref() else {
        return common(None, None, None, None, None, Some("no_tx_hash".into()));
    };
    let tx_hash = match hash.parse::<H256>() {
        Ok(h) => h,
        Err(_) => return common(None, None, None, None, None, Some("invalid_tx_hash".into())),
    };

    let receipt_res = timeout(
        Duration::from_millis(receipt_timeout_ms),
        provider.get_transaction_receipt(tx_hash),
    ).await;

    let receipt = match receipt_res {
        Ok(Ok(Some(r))) => r,
        Ok(Ok(None)) => return common(None, None, None, None, None, Some("receipt_not_found".into())),
        Ok(Err(e)) => {
            warn!(event = "recon.receipt_err", hash = %hash, error = %e);
            return common(None, None, None, None, None, Some(format!("rpc_error: {e}")));
        }
        Err(_) => return common(None, None, None, None, None, Some("receipt_timeout".into())),
    };

    let gas_used = receipt.gas_used.map(|g| g.to_string());
    let gas_price = receipt.effective_gas_price.map(|g| g.to_string());

    // Decode ERC20 Transfer logs targeting the tx origin (from addr) to find amount_out.
    let from_addr = match opp.token_out.parse::<Address>() {
        Ok(a) => a,
        Err(_) => return common(gas_used, gas_price, None, None, None, Some("invalid_token_out".into())),
    };
    let topic = transfer_topic();
    let mut candidate_out: Option<U256> = None;
    for log in &receipt.logs {
        if log.topics.first() != Some(&topic) { continue; }
        // log.address is the token contract; we want token_out's transfers to the signer.
        if log.address != from_addr { continue; }
        // data is the `value`
        if log.data.len() >= 32 {
            let value = U256::from_big_endian(&log.data[0..32]);
            candidate_out = Some(value);
            // Keep the last matching transfer of token_out (heuristic: final in path).
        }
    }

    let actual_out = candidate_out.map(|u| u.to_string());

    // Variance on native units: compare amount_in_wei vs actual_out as a proxy.
    // Not a real profit in USD — documented; S6.1 oracle refines.
    let (variance_pct, variance_native) = match (&actual_out, opp.amount_in_wei.parse::<u128>()) {
        (Some(out_str), Ok(in_u)) if in_u > 0 => {
            if let Ok(out_u) = out_str.parse::<u128>() {
                let diff = out_u.abs_diff(in_u);
                let pct = (diff as f64 / in_u as f64) * 100.0;
                let sign = if out_u >= in_u { diff as i128 } else { -(diff as i128) };
                (Some(pct), Some(sign.to_string()))
            } else {
                (None, None)
            }
        }
        _ => (None, None),
    };

    debug!(event = "recon.computed", tx = %hash, variance_pct = ?variance_pct);
    let _ = Uuid::nil(); // keep uuid import used even when fail paths dominate
    common(gas_used, gas_price, actual_out, variance_pct, variance_native, None)
}
