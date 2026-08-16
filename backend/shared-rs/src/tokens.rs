//! Token catalog for supported EVM chains.
//!
//! Static, configuration-independent token metadata (address, decimals, symbol,
//! name) for all tokens recognised by ArbitrageX v2. We keep the catalog in code
//! (not TOML) on purpose: these are protocol constants that change extremely
//! rarely and have strong audit requirements. Any addition must be reviewed.
//!
//! # Usage
//!
//! ```rust
//! use shared_rs::tokens::{token_by_symbol, token_address_str};
//!
//! let weth = token_by_symbol(1, "WETH").expect("WETH must exist");
//! assert_eq!(weth.decimals, 18);
//!
//! let addr = token_address_str(1, "USDC").expect("USDC must exist");
//! assert!(addr.starts_with("0x"));
//! ```

use crate::chains::USDT_MAINNET_LC;

/// A single entry in the token catalog.
#[derive(Debug, Clone, Copy)]
pub struct TokenEntry {
    /// Ticker symbol (uppercase, e.g. `"WETH"`).
    pub symbol: &'static str,
    /// EVM chain identifier.
    pub chain_id: u64,
    /// 20-byte EVM address in checksum-agnostic form (raw bytes, lowercase hex source).
    pub address: [u8; 20],
    /// Token decimals as reported by the ERC-20 contract.
    pub decimals: u8,
    /// Human-readable token name.
    pub name: &'static str,
}

// ---------------------------------------------------------------------------
// Const-time hex decoder
// ---------------------------------------------------------------------------
//
// `chains.rs` defines its own private `hex20` / `hex_nibble` pair and does NOT
// export them (they are `const fn` without `pub`). Duplicating them here with
// the prefix `hex20_token` / `hex_nibble_token` avoids any name collision if
// both modules are ever inlined into the same compilation unit, and follows the
// repo convention of keeping const helpers file-local.

const fn hex20_token(s: &str) -> [u8; 20] {
    assert!(s.len() == 42);
    let b = s.as_bytes();
    assert!(b[0] == b'0' && (b[1] == b'x' || b[1] == b'X'));
    let mut out = [0u8; 20];
    let mut i = 0;
    while i < 20 {
        let hi = hex_nibble_token(b[2 + 2 * i]);
        let lo = hex_nibble_token(b[2 + 2 * i + 1]);
        out[i] = (hi << 4) | lo;
        i += 1;
    }
    out
}

const fn hex_nibble_token(c: u8) -> u8 {
    match c {
        b'0'..=b'9' => c - b'0',
        b'a'..=b'f' => c - b'a' + 10,
        b'A'..=b'F' => c - b'A' + 10,
        _ => panic!("invalid hex"),
    }
}

// ---------------------------------------------------------------------------
// Ethereum mainnet (chain_id = 1) token catalog
// ---------------------------------------------------------------------------

const WETH_MAINNET: TokenEntry = TokenEntry {
    symbol: "WETH",
    chain_id: 1,
    address: hex20_token("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"),
    decimals: 18,
    name: "Wrapped Ether",
};

const USDC_MAINNET: TokenEntry = TokenEntry {
    symbol: "USDC",
    chain_id: 1,
    address: hex20_token("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"),
    decimals: 6,
    name: "USD Coin",
};

const USDT_MAINNET: TokenEntry = TokenEntry {
    symbol: "USDT",
    chain_id: 1,
    address: hex20_token(USDT_MAINNET_LC),
    decimals: 6,
    name: "Tether USD",
};

const DAI_MAINNET: TokenEntry = TokenEntry {
    symbol: "DAI",
    chain_id: 1,
    address: hex20_token("0x6b175474e89094c44da98b954eedeac495271d0f"),
    decimals: 18,
    name: "Dai Stablecoin",
};

const WBTC_MAINNET: TokenEntry = TokenEntry {
    symbol: "WBTC",
    chain_id: 1,
    address: hex20_token("0x2260fac5e5542a773aa44fbcfedf7c193bc2c599"),
    decimals: 8,
    name: "Wrapped Bitcoin",
};

const PEPE_MAINNET: TokenEntry = TokenEntry {
    symbol: "PEPE",
    chain_id: 1,
    address: hex20_token("0x6982508145454ce325ddbe47a25d4ec3d2311933"),
    decimals: 18,
    name: "Pepe",
};

const SHIB_MAINNET: TokenEntry = TokenEntry {
    symbol: "SHIB",
    chain_id: 1,
    address: hex20_token("0x95ad61b0a150d79219dcf64e1e6cc01f0b64c4ce"),
    decimals: 18,
    name: "Shiba Inu",
};

const MKR_MAINNET: TokenEntry = TokenEntry {
    symbol: "MKR",
    chain_id: 1,
    address: hex20_token("0x9f8f72aa9304c8b593d555f12ef6589cc3a579a2"),
    decimals: 18,
    name: "Maker",
};

const COMP_MAINNET: TokenEntry = TokenEntry {
    symbol: "COMP",
    chain_id: 1,
    address: hex20_token("0xc00e94cb662c3520282e6f5717214004a7f26888"),
    decimals: 18,
    name: "Compound",
};

/// All catalogued tokens on Ethereum mainnet (chain_id = 1).
pub const TOKENS_MAINNET: &[TokenEntry] = &[
    WETH_MAINNET,
    USDC_MAINNET,
    USDT_MAINNET,
    DAI_MAINNET,
    WBTC_MAINNET,
    PEPE_MAINNET,
    SHIB_MAINNET,
    MKR_MAINNET,
    COMP_MAINNET,
];

// ---------------------------------------------------------------------------
// Public helpers
// ---------------------------------------------------------------------------

/// Returns the static token catalog for a given chain.
pub fn tokens_for_chain(chain_id: u64) -> &'static [TokenEntry] {
    match chain_id {
        1 => TOKENS_MAINNET,
        _ => &[],
    }
}

/// Looks up a token by chain and ticker symbol (case-sensitive, uppercase convention).
///
/// Returns `None` if the symbol is unknown on that chain.
pub fn token_by_symbol(chain_id: u64, symbol: &str) -> Option<&'static TokenEntry> {
    tokens_for_chain(chain_id)
        .iter()
        .find(|t| t.symbol == symbol)
}

/// Returns the lowercase 0x-prefixed hex address string for a token, or `None`
/// if the symbol is unknown on the given chain.
///
/// Output is always 42 characters (`"0x"` + 40 hex digits, all lowercase).
pub fn token_address_str(chain_id: u64, symbol: &str) -> Option<String> {
    token_by_symbol(chain_id, symbol).map(|t| {
        let mut s = String::with_capacity(42);
        s.push_str("0x");
        for b in t.address.iter() {
            s.push_str(&format!("{:02x}", b));
        }
        s
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // Helper: assert that a raw address slice has the expected first and last byte.
    fn assert_first_last(addr: &[u8; 20], first: u8, last: u8, label: &str) {
        assert_eq!(
            addr[0], first,
            "{label}: expected first byte 0x{first:02x}, got 0x{:02x}",
            addr[0]
        );
        assert_eq!(
            addr[19], last,
            "{label}: expected last byte 0x{last:02x}, got 0x{:02x}",
            addr[19]
        );
    }

    // ---- Per-token byte-asserting tests ------------------------------------

    /// WETH: 0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2
    ///   first = 0xc0, last = 0xc2
    #[test]
    fn weth_mainnet_address_first_last_bytes() {
        assert_first_last(&WETH_MAINNET.address, 0xc0, 0xc2, "WETH");
    }

    /// WETH full-vector sanity: spot-check bytes at known offsets.
    #[test]
    fn weth_mainnet_address_full_vector() {
        let expected: [u8; 20] = [
            0xc0, 0x2a, 0xaa, 0x39, 0xb2, 0x23, 0xfe, 0x8d, 0x0a, 0x0e, 0x5c, 0x4f, 0x27, 0xea,
            0xd9, 0x08, 0x3c, 0x75, 0x6c, 0xc2,
        ];
        assert_eq!(WETH_MAINNET.address, expected);
    }

    /// USDC: 0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48
    ///   first = 0xa0, last = 0x48
    #[test]
    fn usdc_mainnet_address_first_last_bytes() {
        assert_first_last(&USDC_MAINNET.address, 0xa0, 0x48, "USDC");
    }

    /// USDT: 0xdac17f958d2ee523a2206206994597c13d831ec7
    ///   first = 0xda, last = 0xc7
    #[test]
    fn usdt_mainnet_address_first_last_bytes() {
        assert_first_last(&USDT_MAINNET.address, 0xda, 0xc7, "USDT");
    }

    /// DAI: 0x6b175474e89094c44da98b954eedeac495271d0f
    ///   first = 0x6b, last = 0x0f
    #[test]
    fn dai_mainnet_address_first_last_bytes() {
        assert_first_last(&DAI_MAINNET.address, 0x6b, 0x0f, "DAI");
    }

    /// WBTC: 0x2260fac5e5542a773aa44fbcfedf7c193bc2c599
    ///   first = 0x22, last = 0x99
    #[test]
    fn wbtc_mainnet_address_first_last_bytes() {
        assert_first_last(&WBTC_MAINNET.address, 0x22, 0x99, "WBTC");
    }

    /// PEPE: 0x6982508145454ce325ddbe47a25d4ec3d2311933
    ///   first = 0x69, last = 0x33
    #[test]
    fn pepe_mainnet_address_first_last_bytes() {
        assert_first_last(&PEPE_MAINNET.address, 0x69, 0x33, "PEPE");
    }

    /// SHIB: 0x95ad61b0a150d79219dcf64e1e6cc01f0b64c4ce
    ///   first = 0x95, last = 0xce
    #[test]
    fn shib_mainnet_address_first_last_bytes() {
        assert_first_last(&SHIB_MAINNET.address, 0x95, 0xce, "SHIB");
    }

    /// MKR: 0x9f8f72aa9304c8b593d555f12ef6589cc3a579a2
    ///   first = 0x9f, last = 0xa2
    #[test]
    fn mkr_mainnet_address_first_last_bytes() {
        assert_first_last(&MKR_MAINNET.address, 0x9f, 0xa2, "MKR");
    }

    /// COMP: 0xc00e94cb662c3520282e6f5717214004a7f26888
    ///   first = 0xc0, last = 0x88
    #[test]
    fn comp_mainnet_address_first_last_bytes() {
        assert_first_last(&COMP_MAINNET.address, 0xc0, 0x88, "COMP");
    }

    // ---- Catalog completeness & metadata ----------------------------------

    #[test]
    fn tokens_mainnet_has_nine_entries() {
        assert_eq!(TOKENS_MAINNET.len(), 9);
    }

    #[test]
    fn all_mainnet_tokens_have_correct_chain_id() {
        for t in TOKENS_MAINNET {
            assert_eq!(t.chain_id, 1, "token {} has wrong chain_id", t.symbol);
        }
    }

    #[test]
    fn decimals_values_are_correct() {
        let cases: &[(&str, u8)] = &[
            ("WETH", 18),
            ("USDC", 6),
            ("USDT", 6),
            ("DAI", 18),
            ("WBTC", 8),
            ("PEPE", 18),
            ("SHIB", 18),
            ("MKR", 18),
            ("COMP", 18),
        ];
        for &(symbol, expected_decimals) in cases {
            let t = token_by_symbol(1, symbol)
                .unwrap_or_else(|| panic!("token {symbol} not found in catalog"));
            assert_eq!(
                t.decimals, expected_decimals,
                "{symbol}: expected decimals={expected_decimals}, got {}",
                t.decimals
            );
        }
    }

    // ---- Helper functions -------------------------------------------------

    #[test]
    fn token_by_symbol_hits_and_misses() {
        assert!(token_by_symbol(1, "WETH").is_some());
        assert!(token_by_symbol(1, "UNKNOWN_XYZ").is_none());
        // Wrong chain: no tokens catalogued for chain 137 yet.
        assert!(token_by_symbol(137, "WETH").is_none());
        // Symbol lookup is case-sensitive.
        assert!(token_by_symbol(1, "weth").is_none());
    }

    #[test]
    fn token_address_str_format() {
        let addr = token_address_str(1, "WETH").expect("WETH must exist");
        assert_eq!(addr.len(), 42, "address string must be 42 chars");
        assert!(addr.starts_with("0x"), "address must be 0x-prefixed");
        // All hex chars after 0x must be lowercase.
        assert!(
            addr[2..]
                .chars()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()),
            "address must be lowercase hex: {addr}",
        );
        assert_eq!(addr, "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2");
    }

    #[test]
    fn token_address_str_usdc() {
        let addr = token_address_str(1, "USDC").expect("USDC must exist");
        assert_eq!(addr, "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48");
    }

    #[test]
    fn token_address_str_returns_none_for_unknown() {
        assert!(token_address_str(1, "NOTATOKEN").is_none());
        assert!(token_address_str(999, "WETH").is_none());
    }

    #[test]
    fn tokens_for_chain_unknown_is_empty() {
        assert!(tokens_for_chain(999).is_empty());
        assert!(tokens_for_chain(0).is_empty());
    }
}
