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

/// `exactInputSingle((address,address,uint24,address,uint256,uint256,uint256,uint160))`
/// V3 SwapRouter (v1) at 0xE592427A0AEce92De3Edee1F18E0157C05861564.
const SELECTOR_V3_EXACT_INPUT_SINGLE: [u8; 4] = [0x41, 0x4b, 0xf3, 0x89];

/// `exactInput((bytes,address,uint256,uint256,uint256))`
/// V3 SwapRouter multi-hop with packed `path` bytes.
const SELECTOR_V3_EXACT_INPUT: [u8; 4] = [0xc0, 0x4b, 0x8d, 0x59];

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

// ─── Uniswap V3 SwapRouter (Phase 2 of #6) ────────────────────────────────

/// Parameters for `ISwapRouter.exactInputSingle`. Field order MUST match the
/// Solidity struct so the ABI tuple encoding lines up byte-for-byte.
///
/// `fee` is the V3 pool fee tier in raw units (NOT bps): 100 (0.01%), 500
/// (0.05%), 3000 (0.30%), 10000 (1.00%). Encoded as uint24 in calldata.
///
/// `sqrt_price_limit_x96` may be `U256::zero()` for "no price limit" (router
/// will swap up to whatever the pool offers). Encoded as uint160.
pub struct V3ExactInputSingleParams {
    pub token_in: Address,
    pub token_out: Address,
    pub fee: u32,
    pub recipient: Address,
    pub deadline: U256,
    pub amount_in: U256,
    pub amount_out_minimum: U256,
    pub sqrt_price_limit_x96: U256,
}

/// Encode `ISwapRouter.exactInputSingle(params)` calldata. Uses ABI tuple
/// encoding (Token::Tuple) so the struct is laid out as a single static head
/// followed by no dynamic data.
pub fn encode_v3_exact_input_single(params: &V3ExactInputSingleParams) -> Bytes {
    let tuple = Token::Tuple(vec![
        Token::Address(params.token_in),
        Token::Address(params.token_out),
        Token::Uint(U256::from(params.fee)),
        Token::Address(params.recipient),
        Token::Uint(params.deadline),
        Token::Uint(params.amount_in),
        Token::Uint(params.amount_out_minimum),
        Token::Uint(params.sqrt_price_limit_x96),
    ]);
    let encoded = encode(&[tuple]);
    prepend_selector(SELECTOR_V3_EXACT_INPUT_SINGLE, encoded).into()
}

/// Encode the V3 multi-hop `path` bytes blob: `token0 || fee0 || token1 || fee1 || ... || tokenN`.
///
/// `tokens.len() MUST equal fees.len() + 1` (N+1 tokens, N fee segments).
/// Each token is 20 bytes, each fee is 3 bytes (uint24, big-endian).
/// Total length: `20*(N+1) + 3*N` bytes.
///
/// Returns `None` when the relationship is violated (defensive — V3 router
/// would revert on a malformed path anyway, but caught here for clearer errors).
pub fn encode_v3_path(tokens: &[Address], fees: &[u32]) -> Option<Vec<u8>> {
    if tokens.len() != fees.len() + 1 || tokens.is_empty() {
        return None;
    }
    let mut out = Vec::with_capacity(tokens.len() * 20 + fees.len() * 3);
    for (i, token) in tokens.iter().enumerate() {
        out.extend_from_slice(token.as_bytes());
        if i < fees.len() {
            // uint24 big-endian = 3 lower bytes of u32 BE representation.
            let fee_be = fees[i].to_be_bytes();
            out.extend_from_slice(&fee_be[1..]); // skip the high byte
        }
    }
    Some(out)
}

/// Parameters for `ISwapRouter.exactInput`. The `path` field is a packed
/// bytes blob (see `encode_v3_path`).
pub struct V3ExactInputParams {
    pub path: Vec<u8>,
    pub recipient: Address,
    pub deadline: U256,
    pub amount_in: U256,
    pub amount_out_minimum: U256,
}

/// Encode `ISwapRouter.exactInput(params)` calldata for multi-hop V3 swaps.
/// `params.path` should come from `encode_v3_path`.
pub fn encode_v3_exact_input(params: &V3ExactInputParams) -> Bytes {
    let tuple = Token::Tuple(vec![
        Token::Bytes(params.path.clone()),
        Token::Address(params.recipient),
        Token::Uint(params.deadline),
        Token::Uint(params.amount_in),
        Token::Uint(params.amount_out_minimum),
    ]);
    let encoded = encode(&[tuple]);
    prepend_selector(SELECTOR_V3_EXACT_INPUT, encoded).into()
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

    // ─── Uniswap V3 tests (Phase 2 of #6) ─────────────────────────────────

    fn weth() -> Address { addr("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2") }
    fn usdc() -> Address { addr("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48") }
    fn dai() -> Address { addr("0x6b175474e89094c44da98b954eedeac495271d0f") }

    /// Selector verified against Etherscan disassembly of SwapRouter
    /// 0xE592427A0AEce92De3Edee1F18E0157C05861564.
    #[test]
    fn v3_exact_input_single_starts_with_known_selector() {
        let params = V3ExactInputSingleParams {
            token_in: weth(),
            token_out: usdc(),
            fee: 500,
            recipient: addr("0x1111111111111111111111111111111111111111"),
            deadline: U256::from(1_700_000_000u64),
            amount_in: U256::from(10_u64).pow(U256::from(18)),
            amount_out_minimum: U256::from(2_000_000_000u64),
            sqrt_price_limit_x96: U256::zero(),
        };
        let calldata = encode_v3_exact_input_single(&params);
        assert_eq!(&calldata[..4], &[0x41, 0x4b, 0xf3, 0x89]);
        // 8 fields × 32 bytes = 256 + 4 selector = 260
        assert_eq!(calldata.len(), 260);
    }

    /// Selector verified against ISwapRouter ABI.
    #[test]
    fn v3_exact_input_starts_with_known_selector() {
        let path = encode_v3_path(&[weth(), usdc()], &[500]).unwrap();
        let params = V3ExactInputParams {
            path,
            recipient: addr("0x1111111111111111111111111111111111111111"),
            deadline: U256::from(1_700_000_000u64),
            amount_in: U256::from(10_u64).pow(U256::from(18)),
            amount_out_minimum: U256::from(0u64),
        };
        let calldata = encode_v3_exact_input(&params);
        assert_eq!(&calldata[..4], &[0xc0, 0x4b, 0x8d, 0x59]);
    }

    /// Single-hop V3 path: WETH → USDC at 500 fee tier.
    /// Layout: 20 (WETH) + 3 (500) + 20 (USDC) = 43 bytes.
    #[test]
    fn v3_path_single_hop_length_and_layout() {
        let path = encode_v3_path(&[weth(), usdc()], &[500]).unwrap();
        assert_eq!(path.len(), 43);
        assert_eq!(&path[..20], weth().as_bytes());
        // Fee 500 = 0x0001F4 in 3 bytes BE.
        assert_eq!(&path[20..23], &[0x00, 0x01, 0xf4]);
        assert_eq!(&path[23..43], usdc().as_bytes());
    }

    /// Multi-hop V3 path: WETH → USDC → DAI with fees 500/100.
    /// Layout: 20 + 3 + 20 + 3 + 20 = 66 bytes.
    #[test]
    fn v3_path_multi_hop_length_and_fees() {
        let path = encode_v3_path(&[weth(), usdc(), dai()], &[500, 100]).unwrap();
        assert_eq!(path.len(), 66);
        // Fee 100 = 0x000064 in 3 bytes BE.
        assert_eq!(&path[43..46], &[0x00, 0x00, 0x64]);
        assert_eq!(&path[46..66], dai().as_bytes());
    }

    /// Defensive — path with mismatched tokens/fees count returns None.
    #[test]
    fn v3_path_mismatched_lengths_return_none() {
        // 2 tokens but 0 fees: needs 1 fee segment
        assert!(encode_v3_path(&[weth(), usdc()], &[]).is_none());
        // 2 tokens but 2 fees: needs 1 fee segment
        assert!(encode_v3_path(&[weth(), usdc()], &[500, 100]).is_none());
        // empty input
        assert!(encode_v3_path(&[], &[]).is_none());
    }

    /// All 4 standard V3 fee tiers encode correctly.
    #[test]
    fn v3_path_all_standard_fee_tiers() {
        for fee in [100u32, 500, 3000, 10000] {
            let path = encode_v3_path(&[weth(), usdc()], &[fee]).unwrap();
            let fee_bytes_in_path = &path[20..23];
            let recovered =
                ((fee_bytes_in_path[0] as u32) << 16)
              | ((fee_bytes_in_path[1] as u32) << 8)
              |  (fee_bytes_in_path[2] as u32);
            assert_eq!(recovered, fee, "fee tier {fee} round-trip failed");
        }
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
