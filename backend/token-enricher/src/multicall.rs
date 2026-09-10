use alloy_primitives::Address;
use alloy_sol_types::{sol, SolCall};
use anyhow::{Context, Result};

pub const MULTICALL3_ADDRESS: &str = "0xcA11bde05977b3631167028862bE2a173976CA11";

sol! {
    interface IERC20 {
        function symbol() external view returns (string);
        function decimals() external view returns (uint8);
    }

    interface IMulticall3 {
        struct Call3 { address target; bool allowFailure; bytes callData; }
        struct Result { bool success; bytes returnData; }
        function aggregate3(Call3[] calldata calls) external payable returns (Result[] memory);
    }
}

#[derive(Debug, Clone)]
pub struct ResolvedTokenData {
    pub symbol: Option<String>,
    pub decimals: Option<u8>,
}

pub fn build_calls_for(addresses: &[Address]) -> Vec<IMulticall3::Call3> {
    let symbol_selector = IERC20::symbolCall {}.abi_encode();
    let decimals_selector = IERC20::decimalsCall {}.abi_encode();
    addresses
        .iter()
        .flat_map(|addr| {
            vec![
                IMulticall3::Call3 {
                    target: *addr,
                    allowFailure: true,
                    callData: symbol_selector.clone().into(),
                },
                IMulticall3::Call3 {
                    target: *addr,
                    allowFailure: true,
                    callData: decimals_selector.clone().into(),
                },
            ]
        })
        .collect()
}

pub fn decode_symbol_result(returndata: &[u8]) -> Result<String> {
    if returndata.is_empty() {
        anyhow::bail!("empty returndata for symbol()");
    }
    if let Ok(decoded) = IERC20::symbolCall::abi_decode_returns(returndata) {
        return Ok(decoded);
    }
    // BR-03 (2026-09-07): pre-standard EIP-20 tokens (MKR, REP, and other
    // 2015-2017 era contracts) return a raw right-padded bytes32 word from
    // symbol() instead of an ABI string. The string-only decode above left
    // those tokens symbol=None → permanently meta-less in the registry →
    // permanently unpriceable (every price tier keys the hash field by
    // symbol). Fall back to reading the word directly; fail-honest (R8) on
    // anything that is not printable ASCII — no invented symbols.
    if returndata.len() == 32 {
        if let Some(symbol) = symbol_from_bytes32_word(returndata) {
            return Ok(symbol);
        }
    }
    anyhow::bail!("decode symbol() returndata")
}

/// BR-03 (2026-09-07): decode a right-padded bytes32 `symbol()` word. Returns
/// `None` unless every significant byte is printable ASCII (0x21..=0x7E) —
/// garbage words stay undecoded rather than fabricating a symbol (RULE 00).
fn symbol_from_bytes32_word(word: &[u8]) -> Option<String> {
    debug_assert_eq!(word.len(), 32);
    let end = word.iter().rposition(|&b| b != 0).map_or(0, |i| i + 1);
    let trimmed = &word[..end];
    if trimmed.is_empty() {
        return None;
    }
    if !trimmed.iter().all(|&b| (0x21..=0x7e).contains(&b)) {
        return None;
    }
    std::str::from_utf8(trimmed).ok().map(|s| s.to_string())
}

pub fn decode_decimals_result(returndata: &[u8]) -> Result<u8> {
    if returndata.is_empty() {
        anyhow::bail!("empty returndata for decimals()");
    }
    let decoded = IERC20::decimalsCall::abi_decode_returns(returndata)
        .context("decode decimals() returndata")?;
    Ok(decoded)
}

pub fn pair_results(results: Vec<IMulticall3::Result>, count: usize) -> Vec<ResolvedTokenData> {
    let mut out = Vec::with_capacity(count);
    for chunk in results.chunks(2) {
        let symbol = if chunk[0].success {
            decode_symbol_result(&chunk[0].returnData).ok()
        } else {
            None
        };
        let decimals = if chunk.len() > 1 && chunk[1].success {
            decode_decimals_result(&chunk[1].returnData).ok()
        } else {
            None
        };
        out.push(ResolvedTokenData { symbol, decimals });
    }
    out
}
