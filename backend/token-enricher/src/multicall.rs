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
    // BR-03: recognize the exact legacy shape before ABI string decoding.
    // This also prevents a permissive decoder accepting an all-zero word
    // as an empty dynamic string. Never invent a symbol from malformed data.
    if returndata.len() == 32 {
        return symbol_from_bytes32_word(returndata);
    }
    let decoded =
        IERC20::symbolCall::abi_decode_returns(returndata).context("decode symbol() returndata")?;
    Ok(decoded)
}

fn symbol_from_bytes32_word(word: &[u8]) -> Result<String> {
    anyhow::ensure!(word.len() == 32, "legacy symbol() must be one bytes32 word");
    let end = word.iter().position(|byte| *byte == 0).unwrap_or(word.len());
    anyhow::ensure!(end > 0, "empty legacy symbol()");
    anyhow::ensure!(
        word[end..].iter().all(|byte| *byte == 0),
        "legacy symbol() has nonzero bytes after its terminator"
    );
    anyhow::ensure!(
        word[..end].iter().all(|byte| (0x21..=0x7e).contains(byte)),
        "legacy symbol() contains non-printable ASCII"
    );
    String::from_utf8(word[..end].to_vec()).context("decode legacy symbol() ASCII")
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

#[cfg(test)]
mod br03_symbol_regression_tests {
    use super::decode_symbol_result;

    fn bytes32(symbol: &[u8]) -> [u8; 32] {
        assert!(symbol.len() <= 32);
        let mut word = [0u8; 32];
        word[..symbol.len()].copy_from_slice(symbol);
        word
    }

    #[test]
    fn accepts_legacy_mkr_and_rep() {
        for symbol in ["MKR", "REP"] {
            assert_eq!(
                decode_symbol_result(&bytes32(symbol.as_bytes())).unwrap(),
                symbol
            );
        }
    }

    #[test]
    fn keeps_standard_dynamic_string_decoding() {
        // ABI return: offset=32, length=3, then one right-padded data word.
        let mut data = vec![0u8; 96];
        data[31] = 32;
        data[63] = 3;
        data[64..67].copy_from_slice(b"ABC");
        assert_eq!(decode_symbol_result(&data).unwrap(), "ABC");
    }

    #[test]
    fn accepts_full_length_printable_word() {
        assert_eq!(decode_symbol_result(&[b'A'; 32]).unwrap(), "A".repeat(32));
    }

    #[test]
    fn rejects_empty_and_zero_word() {
        assert!(decode_symbol_result(&[]).is_err());
        assert!(decode_symbol_result(&[0u8; 32]).is_err());
    }

    #[test]
    fn rejects_nonzero_trailing_padding() {
        let mut word = bytes32(b"MKR");
        word[12] = b'X';
        assert!(decode_symbol_result(&word).is_err());
    }

    #[test]
    fn rejects_space_control_and_non_ascii() {
        for bad in [0x20, 0x1f, 0x7f, 0x80, 0xff] {
            assert!(decode_symbol_result(&bytes32(&[bad])).is_err());
        }
    }

    #[test]
    fn rejects_wrong_lengths_and_integer_shaped_word() {
        assert!(decode_symbol_result(&[b'A'; 31]).is_err());
        assert!(decode_symbol_result(&[b'A'; 33]).is_err());
        let mut integer = [0u8; 32];
        integer[31] = 1;
        assert!(decode_symbol_result(&integer).is_err());
    }
}
