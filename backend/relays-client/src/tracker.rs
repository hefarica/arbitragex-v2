//! Inclusion tracker — polls for tx receipt up to N blocks.
//!
//! Alloy 1.0 (fully migrated):
//!   - `Provider<Http>` → `AlloyHttpProvider` (alloy `RootProvider<Http<reqwest::Client>>`).
//!   - `get_transaction_receipt(B256)` — caller converts from any [u8;32] source.
//!   - Receipt `block_number` is `Option<u64>` in alloy (no `.as_u64()`).
//!   - Receipt `gas_used` is `u128` in alloy (was `Option<U256>` in ethers).
//!   - Receipt status: `receipt.status()` returns `bool` (true = success).
//!   - `get_block_number()` returns `u64` in alloy (no `.as_u64()`).

use alloy::primitives::B256;
use alloy::providers::Provider as AlloyProvider;
use shared_rs::rpc_failover::AlloyHttpProvider;
use std::time::Duration;
use tracing::debug;

pub enum InclusionOutcome {
    /// `gas_used` is `u64` — alloy 1.0 `TransactionReceipt.gas_used` type.
    Included { block: u64, gas_used: u64 },
    Reverted { block: u64 },
    Dropped,
}

pub async fn wait_for_inclusion(
    provider: &AlloyHttpProvider,
    tx_hash: B256,
    start_block: u64,
    max_blocks: u32,
    poll_interval_ms: u64,
) -> InclusionOutcome {
    let end_block = start_block + max_blocks as u64;

    loop {
        match provider.get_transaction_receipt(tx_hash).await {
            Ok(Some(r)) => {
                // alloy 1.0: block_number is Option<u64>, gas_used is u128.
                let block = r.block_number.unwrap_or(0);
                let gas = r.gas_used;
                // alloy 1.0: receipt.status() returns bool (true = success / 1).
                if r.status() {
                    return InclusionOutcome::Included { block, gas_used: gas };
                } else {
                    return InclusionOutcome::Reverted { block };
                }
            }
            Ok(None) => {
                // alloy 1.0: get_block_number() returns u64 directly.
                let cur = provider.get_block_number().await.unwrap_or(start_block);
                if cur >= end_block {
                    debug!(event = "tracker.timeout", start = start_block, end = end_block, current = cur);
                    return InclusionOutcome::Dropped;
                }
            }
            Err(e) => {
                debug!(event = "tracker.receipt_err", error = %e);
            }
        }
        tokio::time::sleep(Duration::from_millis(poll_interval_ms)).await;
    }
}
