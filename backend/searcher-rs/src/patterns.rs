//! Pattern matcher.
//!
//! S2 produces only `dex_arb` candidates from observed single-swap transactions.
//! A "candidate" here means: the observed swap is a *potential* leg of an arbitrage,
//! not an arbitrage itself. Arbitrage path computation lives in S3 (selector-api).
//!
//! Explicitly NOT implemented in S2 (logged as `undetected_kind`):
//!   - triangular
//!   - liquidation
//!   - backrun
//!   - flashloan_arb
//!
//! This keeps S2 honest: we don't fabricate profit signals; we only capture
//! structured observations.

use crate::calldata::DecodedSwap;
use chrono::Utc;
use shared_rs::contracts::{Opportunity, StrategyKind};
use uuid::Uuid;

pub struct TxContext {
    pub chain_id: u64,
    pub block_number: Option<u64>,
    #[allow(dead_code)]
    pub tx_from: [u8; 20],
    pub tx_value: ethers::types::U256,
}

pub fn build_dex_arb_candidate(ctx: &TxContext, swap: &DecodedSwap) -> Opportunity {
    let amount_in = if swap.amount_in.is_zero() {
        // ETH-denominated swaps carry amount in tx.value
        ctx.tx_value
    } else {
        swap.amount_in
    };
    let pair_symbol = format!(
        "{}…/{}…",
        short_hex(&swap.token_in.as_bytes()[..20]),
        short_hex(&swap.token_out.as_bytes()[..20]),
    );

    Opportunity {
        id: Uuid::new_v4(),
        chain_id: ctx.chain_id,
        strategy_kind: StrategyKind::DexArb,
        dex_a: swap.router.to_string(),
        dex_b: None,
        pair_symbol,
        token_in: format!("0x{}", hex::encode(swap.token_in.as_bytes())),
        token_out: format!("0x{}", hex::encode(swap.token_out.as_bytes())),
        amount_in_wei: amount_in.to_string(),
        expected_profit_usd: None, // R8 fail-honest: NULL until selector+sim computes
        roi_pct: None,
        risk_score: None,
        block_number: ctx.block_number,
        rejection_reason: None, // Populated by scanner.rs at each gate decision point.
        detected_at: Utc::now(),
        trace_id: Uuid::new_v4(),
    }
}

fn short_hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(10);
    for b in bytes.iter().take(3) {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use ethers::types::{Address, U256};

    #[test]
    fn candidate_fills_fields_from_swap() {
        let ctx = TxContext {
            chain_id: 1,
            block_number: Some(19_000_000),
            tx_from: [0xab; 20],
            tx_value: U256::zero(),
        };
        let swap = DecodedSwap {
            router: "uniswap-v2",
            token_in: Address::from_low_u64_be(0xc0c0),
            token_out: Address::from_low_u64_be(0xdada),
            amount_in: U256::from(1_000_000_000_000_000_000u128),
            min_amount_out: U256::from(950_000_000u128),
            path_len: 2,
            deadline: U256::from(0xdeadbeefu32),
            recipient: Address::from_low_u64_be(0xbebe),
            selector_hex: "0x38ed1739".into(),
        };
        let o = build_dex_arb_candidate(&ctx, &swap);
        assert_eq!(o.chain_id, 1);
        assert_eq!(o.dex_a, "uniswap-v2");
        assert_eq!(o.amount_in_wei, "1000000000000000000");
        assert_eq!(o.expected_profit_usd, None);
        assert!(o.token_in.starts_with("0x"));
        assert_eq!(o.token_in.len(), 42);
    }

    #[test]
    fn eth_for_tokens_uses_tx_value_as_amount_in() {
        let ctx = TxContext {
            chain_id: 1,
            block_number: None,
            tx_from: [0u8; 20],
            tx_value: U256::from(500_000u64),
        };
        let swap = DecodedSwap {
            router: "uniswap-v2",
            token_in: Address::from_low_u64_be(0xc0c0),
            token_out: Address::from_low_u64_be(0xdada),
            amount_in: U256::zero(),
            min_amount_out: U256::from(1u8),
            path_len: 2,
            deadline: U256::from(0u8),
            recipient: Address::from_low_u64_be(1),
            selector_hex: "0x7ff36ab5".into(),
        };
        let o = build_dex_arb_candidate(&ctx, &swap);
        assert_eq!(o.amount_in_wei, "500000");
    }

    #[test]
    fn candidate_emits_none_for_unsimulated_profit() {
        let ctx = TxContext {
            chain_id: 1,
            block_number: Some(19_000_000),
            tx_from: [0xab; 20],
            tx_value: U256::zero(),
        };
        let swap = DecodedSwap {
            router: "uniswap-v2",
            token_in: Address::from_low_u64_be(0xc0c0),
            token_out: Address::from_low_u64_be(0xdada),
            amount_in: U256::from(1_000_000_000_000_000_000u128),
            min_amount_out: U256::from(950_000_000u128),
            path_len: 2,
            deadline: U256::from(0xdeadbeefu32),
            recipient: Address::from_low_u64_be(0xbebe),
            selector_hex: "0x38ed1739".into(),
        };
        let opp = build_dex_arb_candidate(&ctx, &swap);
        assert_eq!(opp.expected_profit_usd, None,
                   "R8 fail-honest: searcher must emit NULL for profit until simulator computes");
        assert_eq!(opp.roi_pct, None);
        assert_eq!(opp.risk_score, None);
    }
}
