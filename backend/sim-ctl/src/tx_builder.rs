//! Tx builder — Opportunity -> probe transaction parameters.
//!
//! S4 scope: `dex_arb` with UniV2 or UniV3 routers on chain_id=1 only. Anything
//! else returns `BuildError::UnsupportedStrategy` (handled as 'not_implemented'
//! sim result, NEVER as pass).

use ethers::abi::{encode, Token};
use ethers::types::{Address, Bytes, U256};
use shared_rs::chains::{find_router, RouterKind};
use shared_rs::contracts::{Opportunity, StrategyKind};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, thiserror::Error)]
pub enum BuildError {
    #[error("unsupported strategy for S4 simulator: {0:?}")]
    UnsupportedStrategy(StrategyKind),
    #[error("chain {0} not supported in S4 (only mainnet)")]
    UnsupportedChain(u64),
    #[error("router not in catalog for chain={chain} dex={dex}")]
    UnknownRouter { chain: u64, dex: String },
    #[error("token address invalid: {0}")]
    InvalidAddress(String),
    #[error("amount invalid: {0}")]
    InvalidAmount(String),
}

pub struct ProbeTx {
    pub from: Address,
    pub to: Address,
    pub value: U256,
    pub data: Bytes,
    pub gas_cap: u64,
}

const DEFAULT_GAS_CAP: u64 = 5_000_000;
const DEADLINE_OFFSET_SECS: u64 = 120;
/// UniV3 default fee tier (0.3 %). Real production should try 500/3000/10000 and
/// pick the lowest-slippage quote — deferred to S5 quoter integration.
const DEFAULT_UNIV3_FEE: u32 = 3000;

pub fn build_probe(opp: &Opportunity, signer_from: Address) -> Result<ProbeTx, BuildError> {
    if opp.chain_id != 1 {
        return Err(BuildError::UnsupportedChain(opp.chain_id));
    }
    if !matches!(opp.strategy_kind, StrategyKind::DexArb) {
        return Err(BuildError::UnsupportedStrategy(opp.strategy_kind.clone()));
    }
    let token_in = parse_addr(&opp.token_in)?;
    let token_out = parse_addr(&opp.token_out)?;
    let amount_in = U256::from_dec_str(&opp.amount_in_wei)
        .map_err(|_| BuildError::InvalidAmount(opp.amount_in_wei.clone()))?;
    if amount_in.is_zero() {
        return Err(BuildError::InvalidAmount("zero amount_in".into()));
    }

    let router_entry = find_router_by_name(opp.chain_id, &opp.dex_a)
        .ok_or_else(|| BuildError::UnknownRouter { chain: opp.chain_id, dex: opp.dex_a.clone() })?;
    let to = Address::from(router_entry.address);

    let deadline = U256::from(now_secs() + DEADLINE_OFFSET_SECS);

    let data: Bytes = match router_entry.kind {
        RouterKind::UniswapV2 | RouterKind::Sushi => encode_v2(token_in, token_out, amount_in, signer_from, deadline),
        RouterKind::UniswapV3 => encode_v3_exact_input_single(token_in, token_out, amount_in, signer_from, deadline),
        _ => return Err(BuildError::UnknownRouter { chain: opp.chain_id, dex: opp.dex_a.clone() }),
    };

    Ok(ProbeTx {
        from: signer_from,
        to,
        value: U256::zero(), // probe assumes ERC20-in; ETH-in probes deferred to S4.1
        data,
        gas_cap: DEFAULT_GAS_CAP,
    })
}

/// Search router catalog by human-readable `dex_a` name (e.g. "uniswap-v2").
fn find_router_by_name(chain_id: u64, dex_a: &str) -> Option<&'static shared_rs::chains::RouterEntry> {
    shared_rs::chains::routers_for_chain(chain_id)
        .iter()
        .find(|r| r.name.starts_with(dex_a) || r.kind.as_str() == dex_a)
}

fn parse_addr(s: &str) -> Result<Address, BuildError> {
    let cleaned = s.trim_start_matches("0x").trim_start_matches("0X");
    if cleaned.len() != 40 || !cleaned.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(BuildError::InvalidAddress(s.to_string()));
    }
    let bytes = hex::decode(cleaned).map_err(|_| BuildError::InvalidAddress(s.to_string()))?;
    let mut arr = [0u8; 20];
    arr.copy_from_slice(&bytes);
    Ok(Address::from(arr))
}

fn now_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// UniV2 Router02: swapExactTokensForTokens(uint256,uint256,address[],address,uint256)
/// Selector: 0x38ed1739
fn encode_v2(token_in: Address, token_out: Address, amount_in: U256, to: Address, deadline: U256) -> Bytes {
    let selector: [u8; 4] = [0x38, 0xed, 0x17, 0x39];
    let tokens = vec![
        Token::Uint(amount_in),
        Token::Uint(U256::from(1u8)), // amountOutMin = 1 (simulation only)
        Token::Array(vec![Token::Address(token_in), Token::Address(token_out)]),
        Token::Address(to),
        Token::Uint(deadline),
    ];
    let mut buf = selector.to_vec();
    buf.extend(encode(&tokens));
    Bytes::from(buf)
}

/// UniV3 SwapRouter: exactInputSingle(ExactInputSingleParams)
/// Selector: 0x414bf389
fn encode_v3_exact_input_single(token_in: Address, token_out: Address, amount_in: U256, to: Address, deadline: U256) -> Bytes {
    let selector: [u8; 4] = [0x41, 0x4b, 0xf3, 0x89];
    let params = Token::Tuple(vec![
        Token::Address(token_in),
        Token::Address(token_out),
        Token::Uint(U256::from(DEFAULT_UNIV3_FEE)),
        Token::Address(to),
        Token::Uint(deadline),
        Token::Uint(amount_in),
        Token::Uint(U256::zero()), // amountOutMinimum = 0
        Token::Uint(U256::zero()), // sqrtPriceLimitX96 = 0
    ]);
    let mut buf = selector.to_vec();
    buf.extend(encode(&[params]));
    Bytes::from(buf)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use uuid::Uuid;

    fn opp(kind: StrategyKind, chain_id: u64, dex_a: &str) -> Opportunity {
        Opportunity {
            id: Uuid::new_v4(),
            chain_id,
            strategy_kind: kind,
            dex_a: dex_a.into(),
            dex_b: None,
            pair_symbol: "WETH/USDC".into(),
            token_in: "0xC02aaa39b223FE8D0A0e5C4F27eAD9083C756Cc2".into(),
            token_out: "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48".into(),
            amount_in_wei: "1000000000000000000".into(),
            expected_profit_usd: 10.0,
            roi_pct: None, risk_score: None, block_number: None,
            detected_at: Utc::now(),
            trace_id: Uuid::new_v4(),
        }
    }

    #[test]
    fn v2_dex_arb_builds() {
        let signer: Address = [0xab; 20].into();
        let o = opp(StrategyKind::DexArb, 1, "uniswap-v2");
        let tx = build_probe(&o, signer).expect("build v2");
        assert_eq!(tx.data.as_ref()[0..4], [0x38, 0xed, 0x17, 0x39]);
        assert_eq!(tx.value, U256::zero());
        assert_eq!(tx.from, signer);
    }

    #[test]
    fn v3_dex_arb_builds() {
        let signer: Address = [0xcd; 20].into();
        let o = opp(StrategyKind::DexArb, 1, "uniswap-v3");
        let tx = build_probe(&o, signer).expect("build v3");
        assert_eq!(tx.data.as_ref()[0..4], [0x41, 0x4b, 0xf3, 0x89]);
    }

    #[test]
    fn non_dex_arb_rejected() {
        let signer: Address = [0; 20].into();
        for kind in [StrategyKind::Triangular, StrategyKind::Backrun, StrategyKind::Liquidation, StrategyKind::FlashloanArb] {
            let o = opp(kind.clone(), 1, "uniswap-v2");
            assert!(matches!(build_probe(&o, signer), Err(BuildError::UnsupportedStrategy(_))));
        }
    }

    #[test]
    fn non_mainnet_rejected() {
        let o = opp(StrategyKind::DexArb, 137, "uniswap-v2");
        assert!(matches!(build_probe(&o, [0; 20].into()), Err(BuildError::UnsupportedChain(137))));
    }

    #[test]
    fn unknown_dex_rejected() {
        let o = opp(StrategyKind::DexArb, 1, "some-unknown-dex");
        assert!(matches!(build_probe(&o, [0; 20].into()), Err(BuildError::UnknownRouter { .. })));
    }
}
