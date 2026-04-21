//! Inclusion tracker — polls for tx receipt up to N blocks.

use ethers::prelude::*;
use std::time::Duration;
use tracing::debug;

pub enum InclusionOutcome {
    Included { block: u64, gas_used: U256 },
    Reverted { block: u64 },
    Dropped,
}

pub async fn wait_for_inclusion(
    provider: &Provider<Http>,
    tx_hash: H256,
    start_block: u64,
    max_blocks: u32,
    poll_interval_ms: u64,
) -> InclusionOutcome {
    let end_block = start_block + max_blocks as u64;
    loop {
        match provider.get_transaction_receipt(tx_hash).await {
            Ok(Some(r)) => {
                let block = r.block_number.map(|n| n.as_u64()).unwrap_or(0);
                let gas = r.gas_used.unwrap_or_default();
                let status = r.status.map(|s| s.as_u64()).unwrap_or(0);
                if status == 1 {
                    return InclusionOutcome::Included { block, gas_used: gas };
                } else {
                    return InclusionOutcome::Reverted { block };
                }
            }
            Ok(None) => {
                let cur = provider.get_block_number().await
                    .map(|n| n.as_u64()).unwrap_or(start_block);
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
