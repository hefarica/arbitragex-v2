//! Swap calldata encoder — Phase 1 of REVM real implementation.
//!
//! Pure ABI-encoding utility that produces calldata bytes for Uniswap V2 router
//! and ERC20 functions. No RPC, no EVM execution, no async — fully testable
//! with `ethers::abi` in unit tests. Single source of truth for "what bytes
//! do I send to the router for this swap?".
//!
//! This is the foundation for `simulator.rs` Phase 4 (round-trip executor),
//! which will pre-fund a simulated EOA, encode forward + backward swap
//! calldata via these helpers, execute in REVM, and read post-execution
//! balances to compute realised profit.
//!
//! See `docs/superpowers/plans/2026-05-05-revm-real-implementation.md`
//! for the full sprint roadmap.
//!
//! Function selectors verified against:
//!   IUniswapV2Router02 — https://etherscan.io/address/0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D
//!   IERC20             — OpenZeppelin reference

use ethers::abi::{encode, Token};
use ethers::types::{Address, Bytes, U256};

/// `swapExactTokensForTokens(uint256,uint256,address[],address,uint256)`
/// selector — first 4 bytes of `keccak256("swapExactTokensForTokens(...)")`.
const SELECTOR_SWAP_EXACT_TOKENS_FOR_TOKENS: [u8; 4] = [0x38, 0xed, 0x17, 0x39];

/// `swapExactETHForTokens(uint256,address[],address,uint256)`
/// selector — note: caller sends ETH via tx.value, not as a parameter.
const SELECTOR_SWAP_EXACT_ETH_FOR_TOKENS: [u8; 4] = [0x7f, 0xf3, 0x6a, 0xb5];

/// `approve(address,uint256)` — ERC20 approval.
const SELECTOR_ERC20_APPROVE: [u8; 4] = [0x09, 0x5e, 0xa7, 0xb3];

/// `balanceOf(address)` — ERC20 balance query.
const SELECTOR_ERC20_BALANCE_OF: [u8; 4] = [0x70, 0xa0, 0x82, 0x31];

/// `transfer(address,uint256)` — ERC20 transfer.
const SELECTOR_ERC20_TRANSFER: [u8; 4] = [0xa9, 0x05, 0x9c, 0xbb];

/// Encode `IUniswapV2Router02.swapExactTokensForTokens(amountIn, amountOutMin, path, to, deadline)`.
///
/// `path` MUST contain at least 2 addresses (token_in, token_out) and may
/// include intermediate hops for multi-leg routes. The router enforces this
/// at runtime; the encoder does not validate (callers should).
pub fn encode_v2_swap_exact_tokens_for_tokens(
    amount_in: U256,
    amount_out_min: U256,
    path: &[Address],
    to: Address,
    deadline: U256,
) -> Bytes {
    let path_tokens: Vec<Token> = path.iter().map(|a| Token::Address(*a)).collect();
    let encoded_args = encode(&[
        Token::Uint(amount_in),
        Token::Uint(amount_out_min),
        Token::Array(path_tokens),
        Token::Address(to),
        Token::Uint(deadline),
    ]);
    prepend_selector(SELECTOR_SWAP_EXACT_TOKENS_FOR_TOKENS, encoded_args).into()
}

/// Encode `IUniswapV2Router02.swapExactETHForTokens(amountOutMin, path, to, deadline)`.
/// The ETH amount comes from `tx.value`, not the calldata.
pub fn encode_v2_swap_exact_eth_for_tokens(
    amount_out_min: U256,
    path: &[Address],
    to: Address,
    deadline: U256,
) -> Bytes {
    let path_tokens: Vec<Token> = path.iter().map(|a| Token::Address(*a)).collect();
    let encoded_args = encode(&[
        Token::Uint(amount_out_min),
        Token::Array(path_tokens),
        Token::Address(to),
        Token::Uint(deadline),
    ]);
    prepend_selector(SELECTOR_SWAP_EXACT_ETH_FOR_TOKENS, encoded_args).into()
}

/// Encode `IERC20.approve(spender, amount)`.
pub fn encode_erc20_approve(spender: Address, amount: U256) -> Bytes {
    let encoded_args = encode(&[Token::Address(spender), Token::Uint(amount)]);
    prepend_selector(SELECTOR_ERC20_APPROVE, encoded_args).into()
}

/// Encode `IERC20.balanceOf(account)`.
pub fn encode_erc20_balance_of(account: Address) -> Bytes {
    let encoded_args = encode(&[Token::Address(account)]);
    prepend_selector(SELECTOR_ERC20_BALANCE_OF, encoded_args).into()
}

/// Encode `IERC20.transfer(to, amount)`.
pub fn encode_erc20_transfer(to: Address, amount: U256) -> Bytes {
    let encoded_args = encode(&[Token::Address(to), Token::Uint(amount)]);
    prepend_selector(SELECTOR_ERC20_TRANSFER, encoded_args).into()
}

fn prepend_selector(selector: [u8; 4], args: Vec<u8>) -> Vec<u8> {
    let mut out = Vec::with_capacity(4 + args.len());
    out.extend_from_slice(&selector);
    out.extend(args);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    fn addr(hex: &str) -> Address {
        Address::from_str(hex).expect("valid hex address")
    }

    /// Selector verified against Etherscan disassembly of router 0x7a250...
    /// First 4 bytes of swapExactTokensForTokens calldata.
    #[test]
    fn swap_exact_tokens_for_tokens_starts_with_known_selector() {
        let calldata = encode_v2_swap_exact_tokens_for_tokens(
            U256::from(1_000_000u64),
            U256::from(900_000u64),
            &[
                addr("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"), // WETH
                addr("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"), // USDC
            ],
            addr("0x1111111111111111111111111111111111111111"),
            U256::from(1_700_000_000u64),
        );
        assert_eq!(&calldata[..4], &[0x38, 0xed, 0x17, 0x39]);
    }

    #[test]
    fn swap_exact_eth_for_tokens_starts_with_known_selector() {
        let calldata = encode_v2_swap_exact_eth_for_tokens(
            U256::from(900_000u64),
            &[
                addr("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"),
                addr("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"),
            ],
            addr("0x1111111111111111111111111111111111111111"),
            U256::from(1_700_000_000u64),
        );
        assert_eq!(&calldata[..4], &[0x7f, 0xf3, 0x6a, 0xb5]);
    }

    #[test]
    fn erc20_approve_selector_and_args_roundtrip() {
        let spender = addr("0x7a250d5630b4cf539739df2c5dacb4c659f2488d"); // V2 router
        let calldata = encode_erc20_approve(spender, U256::MAX);
        assert_eq!(&calldata[..4], &[0x09, 0x5e, 0xa7, 0xb3]);
        // address (left-padded to 32 bytes) + uint256 = 64 bytes after selector
        assert_eq!(calldata.len(), 4 + 32 + 32);
        // Last 20 bytes of the address slot must equal the spender bytes
        assert_eq!(&calldata[16..36], spender.as_bytes());
    }

    #[test]
    fn erc20_balance_of_selector_and_single_arg() {
        let account = addr("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2");
        let calldata = encode_erc20_balance_of(account);
        assert_eq!(&calldata[..4], &[0x70, 0xa0, 0x82, 0x31]);
        assert_eq!(calldata.len(), 4 + 32); // selector + 1 address slot
        assert_eq!(&calldata[16..36], account.as_bytes());
    }

    #[test]
    fn erc20_transfer_selector_and_args() {
        let to = addr("0xdead000000000000000000000000000000000000");
        let amount = U256::from(1_000_000_000_000_000_000u128); // 1 ether
        let calldata = encode_erc20_transfer(to, amount);
        assert_eq!(&calldata[..4], &[0xa9, 0x05, 0x9c, 0xbb]);
        assert_eq!(calldata.len(), 4 + 32 + 32);
        assert_eq!(&calldata[16..36], to.as_bytes());
    }

    /// Multi-hop V2 path (WETH → USDC → DAI) — the router supports any path
    /// length; encoder must round-trip the full array.
    #[test]
    fn swap_supports_multi_hop_path() {
        let path = vec![
            addr("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"), // WETH
            addr("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"), // USDC
            addr("0x6b175474e89094c44da98b954eedeac495271d0f"), // DAI
        ];
        let calldata = encode_v2_swap_exact_tokens_for_tokens(
            U256::from(10_u64).pow(U256::from(18)),
            U256::from(0u64),
            &path,
            addr("0x1111111111111111111111111111111111111111"),
            U256::from(1_700_000_000u64),
        );
        // 5 static args × 32 = 160 (amount_in, amount_out_min, path_offset, to, deadline)
        // + dynamic part: array length (32) + 3 path entries × 32 = 128
        // + 4-byte selector = 4 + 160 + 128 = 292
        assert_eq!(calldata.len(), 292);
        assert_eq!(&calldata[..4], &[0x38, 0xed, 0x17, 0x39]);
    }

    /// Edge case — zero amounts encode without panicking.
    #[test]
    fn zero_amounts_encode_cleanly() {
        let calldata = encode_v2_swap_exact_tokens_for_tokens(
            U256::zero(),
            U256::zero(),
            &[
                addr("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"),
                addr("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"),
            ],
            Address::zero(),
            U256::zero(),
        );
        assert_eq!(&calldata[..4], &[0x38, 0xed, 0x17, 0x39]);
        // Should still be well-formed bytes, not panicked.
        assert!(calldata.len() > 4);
    }
}
