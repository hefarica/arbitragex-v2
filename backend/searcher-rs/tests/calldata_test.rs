//! Calldata decoder fixtures.
//!
//! Fixtures are hand-crafted calldata bytes matching the exact ABI of the target
//! function. They are NOT random copy-paste from etherscan — they are constructed
//! deterministically so the assertions below are stable and independent of any
//! live chain state.
//!
//! Every fixture exercises a specific selector + layout; together they cover the
//! core selectors shipped in S2 (UniV2 x3 + UniV3 x2).

use ethers::abi::{encode, Token};
use ethers::types::{Address, U256};

// ----- Fixture builders -----

fn pad_selector_and_body(selector: [u8; 4], tokens: &[Token]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&selector);
    out.extend_from_slice(&encode(tokens));
    out
}

// ----- UniV2 -----

#[test]
fn decode_univ2_swap_exact_tokens_for_tokens() {
    use searcher_rs::calldata::{decode, DecodedSwap};
    use shared_rs::chains::RouterKind;

    let token_in = Address::from_low_u64_be(0xAAAA);
    let token_out = Address::from_low_u64_be(0xBBBB);
    let recipient = Address::from_low_u64_be(0xCCCC);

    let input = pad_selector_and_body(
        [0x38, 0xed, 0x17, 0x39],
        &[
            Token::Uint(U256::from(1_000_000u64)),
            Token::Uint(U256::from(900_000u64)),
            Token::Array(vec![Token::Address(token_in), Token::Address(token_out)]),
            Token::Address(recipient),
            Token::Uint(U256::from(0xdeadbeefu32)),
        ],
    );
    let out: DecodedSwap = decode(&input, RouterKind::UniswapV2).expect("decode");
    assert_eq!(out.router, "uniswap-v2");
    assert_eq!(out.token_in, token_in);
    assert_eq!(out.token_out, token_out);
    assert_eq!(out.amount_in, U256::from(1_000_000u64));
    assert_eq!(out.min_amount_out, U256::from(900_000u64));
    assert_eq!(out.path_len, 2);
    assert_eq!(out.recipient, recipient);
    assert_eq!(out.selector_hex, "0x38ed1739");
}

#[test]
fn decode_univ2_swap_tokens_for_exact_tokens() {
    use searcher_rs::calldata::decode;
    use shared_rs::chains::RouterKind;

    let token_in = Address::from_low_u64_be(0x1111);
    let token_mid = Address::from_low_u64_be(0x2222);
    let token_out = Address::from_low_u64_be(0x3333);
    let recipient = Address::from_low_u64_be(0x4444);

    let input = pad_selector_and_body(
        [0x88, 0x03, 0xdb, 0xee],
        &[
            Token::Uint(U256::from(123u64)),     // amountOut
            Token::Uint(U256::from(456u64)),     // amountInMax
            Token::Array(vec![
                Token::Address(token_in),
                Token::Address(token_mid),
                Token::Address(token_out),
            ]),
            Token::Address(recipient),
            Token::Uint(U256::from(777u32)),
        ],
    );
    let out = decode(&input, RouterKind::UniswapV2).expect("decode");
    assert_eq!(out.token_in, token_in);
    assert_eq!(out.token_out, token_out); // last in path
    assert_eq!(out.path_len, 3);
    assert_eq!(out.amount_in, U256::from(456u64));
    assert_eq!(out.min_amount_out, U256::from(123u64));
}

#[test]
fn decode_univ2_swap_exact_eth_for_tokens_amount_in_zero() {
    use searcher_rs::calldata::decode;
    use shared_rs::chains::RouterKind;

    let weth = Address::from_low_u64_be(0xEEEE);
    let usdc = Address::from_low_u64_be(0xFFFF);
    let recipient = Address::from_low_u64_be(0xABCD);

    let input = pad_selector_and_body(
        [0x7f, 0xf3, 0x6a, 0xb5],
        &[
            Token::Uint(U256::from(50u64)),      // amountOutMin
            Token::Array(vec![Token::Address(weth), Token::Address(usdc)]),
            Token::Address(recipient),
            Token::Uint(U256::from(0u32)),
        ],
    );
    let out = decode(&input, RouterKind::UniswapV2).expect("decode");
    // For ETH-in variants, amount_in stays zero (caller uses tx.value).
    assert_eq!(out.amount_in, U256::zero());
    assert_eq!(out.min_amount_out, U256::from(50u64));
    assert_eq!(out.token_in, weth);
    assert_eq!(out.token_out, usdc);
}

#[test]
fn decode_univ2_rejects_short_input() {
    use searcher_rs::calldata::{decode, DecodeFailReason};
    use shared_rs::chains::RouterKind;

    let err = decode(&[0x38, 0xed, 0x17], RouterKind::UniswapV2).unwrap_err();
    assert!(matches!(err, DecodeFailReason::ShortInput));
}

#[test]
fn decode_univ2_rejects_unknown_selector() {
    use searcher_rs::calldata::{decode, DecodeFailReason};
    use shared_rs::chains::RouterKind;

    let input = vec![0xde, 0xad, 0xbe, 0xef, 0, 0, 0, 0];
    let err = decode(&input, RouterKind::UniswapV2).unwrap_err();
    assert!(matches!(err, DecodeFailReason::UnsupportedSelector));
}

// ----- UniV3 -----

#[test]
fn decode_univ3_exact_input_single() {
    use searcher_rs::calldata::decode;
    use shared_rs::chains::RouterKind;

    let token_in = Address::from_low_u64_be(0xAAA1);
    let token_out = Address::from_low_u64_be(0xBBB1);
    let recipient = Address::from_low_u64_be(0xCCC1);

    // exactInputSingle(ExactInputSingleParams)
    let input = pad_selector_and_body(
        [0x41, 0x4b, 0xf3, 0x89],
        &[Token::Tuple(vec![
            Token::Address(token_in),
            Token::Address(token_out),
            Token::Uint(U256::from(3000u32)),     // fee
            Token::Address(recipient),
            Token::Uint(U256::from(0xdeadu32)),   // deadline
            Token::Uint(U256::from(1_000u64)),    // amountIn
            Token::Uint(U256::from(950u64)),      // amountOutMinimum
            Token::Uint(U256::zero()),            // sqrtPriceLimitX96
        ])],
    );

    let out = decode(&input, RouterKind::UniswapV3).expect("decode");
    assert_eq!(out.router, "uniswap-v3");
    assert_eq!(out.token_in, token_in);
    assert_eq!(out.token_out, token_out);
    assert_eq!(out.amount_in, U256::from(1_000u64));
    assert_eq!(out.min_amount_out, U256::from(950u64));
    assert_eq!(out.path_len, 2);
    assert_eq!(out.recipient, recipient);
    assert_eq!(out.selector_hex, "0x414bf389");
}

#[test]
fn decode_univ3_exact_input_multi_hop() {
    use searcher_rs::calldata::decode;
    use shared_rs::chains::RouterKind;

    // path = tokenA(20) | fee(3) | tokenB(20) | fee(3) | tokenC(20) => 66 bytes, hops=2
    let mut path = Vec::new();
    path.extend_from_slice(&[0xaa; 20]);
    path.extend_from_slice(&[0x00, 0x0b, 0xb8]); // fee 3000
    path.extend_from_slice(&[0xbb; 20]);
    path.extend_from_slice(&[0x00, 0x0b, 0xb8]);
    path.extend_from_slice(&[0xcc; 20]);
    assert_eq!(path.len(), 66);

    let recipient = Address::from_low_u64_be(0xDEF1);
    let input = pad_selector_and_body(
        [0xc0, 0x4b, 0x8d, 0x59],
        &[Token::Tuple(vec![
            Token::Bytes(path.clone()),
            Token::Address(recipient),
            Token::Uint(U256::from(11u32)),
            Token::Uint(U256::from(1_000_000u64)),
            Token::Uint(U256::from(999_000u64)),
        ])],
    );
    let out = decode(&input, RouterKind::UniswapV3).expect("decode");
    assert_eq!(out.router, "uniswap-v3");
    assert_eq!(out.token_in.as_bytes(), &[0xaa; 20]);
    assert_eq!(out.token_out.as_bytes(), &[0xcc; 20]);
    assert_eq!(out.path_len, 3);
    assert_eq!(out.amount_in, U256::from(1_000_000u64));
    assert_eq!(out.min_amount_out, U256::from(999_000u64));
}
