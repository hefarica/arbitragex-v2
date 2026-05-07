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
    let decoded = IERC20::symbolCall::abi_decode_returns(returndata)
        .context("decode symbol() returndata")?;
    Ok(decoded)
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
