use alloy_primitives::Address;
use shared_rs::chains::WETH_MAINNET;
use std::str::FromStr;
use token_enricher::trustwallet::{chain_path, checksum_url_for};

#[test]
fn chain_path_known_chains() {
    assert_eq!(chain_path(1), Some("ethereum"));
    assert_eq!(chain_path(42161), Some("arbitrum"));
    assert_eq!(chain_path(10), Some("optimism"));
    assert_eq!(chain_path(8453), Some("base"));
    assert_eq!(chain_path(137), Some("polygon"));
    assert_eq!(chain_path(56), Some("smartchain"));
    assert_eq!(chain_path(99999), None);
}

#[test]
fn weth_url_uses_eip55_checksum() {
    // WETH is 0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2 in EIP-55 checksum.
    let weth = Address::from_str(WETH_MAINNET).unwrap();
    let url = checksum_url_for(1, weth).unwrap();
    assert!(
        url.contains(WETH_MAINNET),
        "URL must use EIP-55 checksum case, got: {url}"
    );
    assert!(url.starts_with(
        "https://raw.githubusercontent.com/trustwallet/assets/master/blockchains/ethereum/assets/"
    ));
    assert!(url.ends_with("/logo.png"));
}

#[test]
fn unsupported_chain_returns_none() {
    let addr = Address::ZERO;
    assert!(checksum_url_for(99999, addr).is_none());
}
