//! Chain + router catalog.
//!
//! Static, configuration-independent addresses for known routers. We keep the
//! catalog in code (not TOML) on purpose: these are protocol constants that
//! change extremely rarely and have strong audit requirements. Any addition
//! must be reviewed.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RouterKind {
    UniswapV2,
    UniswapV3,
    Sushi,
    Curve,
    Balancer,
    Unknown,
}

impl RouterKind {
    pub const fn as_str(&self) -> &'static str {
        match self {
            RouterKind::UniswapV2 => "uniswap-v2",
            RouterKind::UniswapV3 => "uniswap-v3",
            RouterKind::Sushi     => "sushi",
            RouterKind::Curve     => "curve",
            RouterKind::Balancer  => "balancer",
            RouterKind::Unknown   => "unknown",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct RouterEntry {
    pub chain_id: u64,
    pub name: &'static str,
    pub kind: RouterKind,
    /// 20-byte EVM address in checksum-agnostic form (lowercase hex).
    pub address: [u8; 20],
}

// Helper: constant-time hex decode for static init.
const fn hex20(s: &str) -> [u8; 20] {
    assert!(s.len() == 42);
    let b = s.as_bytes();
    assert!(b[0] == b'0' && (b[1] == b'x' || b[1] == b'X'));
    let mut out = [0u8; 20];
    let mut i = 0;
    while i < 20 {
        let hi = hex_nibble(b[2 + 2 * i]);
        let lo = hex_nibble(b[2 + 2 * i + 1]);
        out[i] = (hi << 4) | lo;
        i += 1;
    }
    out
}
const fn hex_nibble(c: u8) -> u8 {
    match c {
        b'0'..=b'9' => c - b'0',
        b'a'..=b'f' => c - b'a' + 10,
        b'A'..=b'F' => c - b'A' + 10,
        _ => panic!("invalid hex"),
    }
}

// Ethereum mainnet routers.
const UNIV2_ROUTER_MAINNET: RouterEntry = RouterEntry {
    chain_id: 1,
    name: "uniswap-v2-router-02",
    kind: RouterKind::UniswapV2,
    address: hex20("0x7a250d5630b4cf539739df2c5dacb4c659f2488d"),
};
const UNIV3_SWAPROUTER_MAINNET: RouterEntry = RouterEntry {
    chain_id: 1,
    name: "uniswap-v3-swap-router",
    kind: RouterKind::UniswapV3,
    address: hex20("0xe592427a0aece92de3edee1f18e0157c05861564"),
};
const UNIV3_SWAPROUTER02_MAINNET: RouterEntry = RouterEntry {
    chain_id: 1,
    name: "uniswap-v3-swap-router-02",
    kind: RouterKind::UniswapV3,
    address: hex20("0x68b3465833fb72a70ecdf485e0e4c7bd8665fc45"),
};
const SUSHI_ROUTER_MAINNET: RouterEntry = RouterEntry {
    chain_id: 1,
    name: "sushi-router",
    kind: RouterKind::Sushi,
    address: hex20("0xd9e1ce17f2641f24ae83637ab66a2cca9c378b9f"),
};

pub const ROUTERS_MAINNET: &[RouterEntry] = &[
    UNIV2_ROUTER_MAINNET,
    UNIV3_SWAPROUTER_MAINNET,
    UNIV3_SWAPROUTER02_MAINNET,
    SUSHI_ROUTER_MAINNET,
];

/// Returns the static router catalog for a given chain.
pub fn routers_for_chain(chain_id: u64) -> &'static [RouterEntry] {
    match chain_id {
        1 => ROUTERS_MAINNET,
        _ => &[],
    }
}

/// Finds a router entry by address (case-insensitive, byte-exact).
pub fn find_router(chain_id: u64, addr: &[u8; 20]) -> Option<&'static RouterEntry> {
    routers_for_chain(chain_id).iter().find(|r| &r.address == addr)
}

/// Returns the average block time in seconds for a given chain.
///
/// Used for capital opportunity-cost computation:
///   `capital_cost_usd = amount_in_usd × (rate_annual / 100) × (block_time_s / 31_536_000)`.
///
/// Values are conservative best-estimates (round up, not down) so
/// capital cost is never under-counted:
///
/// | Chain     | chain_id | Typical block time | Source                   |
/// |-----------|----------|-------------------|--------------------------|
/// | Ethereum  | 1        | 12.0 s            | PoS slot time (12s)      |
/// | BSC       | 56       | 3.0 s             | BNB Chain consensus      |
/// | Polygon   | 137      | 2.0 s             | Bor PoS                  |
/// | Base      | 8453     | 2.0 s             | OP-Stack (2s slots)      |
/// | Arbitrum  | 42161    | 0.25 s            | Nitro: sub-second real;  |
/// |           |          |                   | conservative 250ms used  |
/// | Optimism  | 10       | 2.0 s             | OP-Stack (2s slots)      |
/// | unknown   | _        | 12.0 s            | ETH-equivalent fallback  |
///
/// Sprint H1 follow-up: replaces the hardcoded `ETH_BLOCK_TIME_S = 12.0`
/// constant in `prioritization-spine/config_aware.rs`. ARB (42161) was
/// previously 6× over-costed at 12s vs real 0.25s; this corrects it.
pub fn block_time_s_for_chain(chain_id: u64) -> f64 {
    match chain_id {
        1     => 12.0,  // Ethereum mainnet (PoS 12s slots)
        56    => 3.0,   // BNB Smart Chain
        137   => 2.0,   // Polygon PoS
        8453  => 2.0,   // Base (OP-Stack)
        42161 => 0.25,  // Arbitrum Nitro (sub-second; 250ms conservative)
        10    => 2.0,   // Optimism (OP-Stack)
        _     => 12.0,  // unknown → ETH-equivalent (conservative)
    }
}

/// Returns the list of router addresses (lowercase 0x-prefixed hex) for use as
/// upstream `toAddress` filter on Alchemy `alchemy_pendingTransactions`
/// subscriptions. Subscribing with this list bounds CU consumption to txs
/// targeting our decoders; everything else is dropped at the relay level.
pub fn router_addresses_hex_for_chain(chain_id: u64) -> Vec<String> {
    routers_for_chain(chain_id)
        .iter()
        .map(|r| {
            let mut s = String::with_capacity(42);
            s.push_str("0x");
            for b in r.address.iter() {
                s.push_str(&format!("{:02x}", b));
            }
            s
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn univ2_router_mainnet_address_bytes() {
        let expected: [u8; 20] = [
            0x7a, 0x25, 0x0d, 0x56, 0x30, 0xb4, 0xcf, 0x53, 0x97, 0x39,
            0xdf, 0x2c, 0x5d, 0xac, 0xb4, 0xc6, 0x59, 0xf2, 0x48, 0x8d,
        ];
        assert_eq!(UNIV2_ROUTER_MAINNET.address, expected);
    }

    #[test]
    fn find_router_hits_and_misses() {
        let univ2 = UNIV2_ROUTER_MAINNET.address;
        assert!(find_router(1, &univ2).is_some());
        let zero = [0u8; 20];
        assert!(find_router(1, &zero).is_none());
        assert!(find_router(137, &univ2).is_none());
    }

    #[test]
    fn router_kind_str_is_stable() {
        assert_eq!(RouterKind::UniswapV2.as_str(), "uniswap-v2");
        assert_eq!(RouterKind::UniswapV3.as_str(), "uniswap-v3");
        assert_eq!(RouterKind::Unknown.as_str(), "unknown");
    }

    // ---- block_time_s_for_chain fixture matrix --------------------------------

    /// Matrix test: each known chain_id maps to the documented value.
    /// Chain additions must update BOTH the function body and this test.
    #[test]
    fn block_time_known_chains() {
        let cases: &[(u64, f64)] = &[
            (1,     12.0),  // Ethereum
            (56,    3.0),   // BSC
            (137,   2.0),   // Polygon
            (8453,  2.0),   // Base
            (42161, 0.25),  // Arbitrum
            (10,    2.0),   // Optimism
        ];
        for &(chain_id, expected) in cases {
            let got = block_time_s_for_chain(chain_id);
            assert!(
                (got - expected).abs() < 1e-9,
                "chain_id={chain_id}: expected block_time={expected}s, got {got}s",
            );
        }
    }

    /// Unknown chain IDs fall back to ETH-equivalent (12s), not 0.0.
    /// A zero block time would divide by zero in capital_cost_usd arithmetic.
    #[test]
    fn block_time_unknown_chain_falls_back_to_eth() {
        let unknown_ids: &[u64] = &[999, 0, u64::MAX, 31337];
        for &chain_id in unknown_ids {
            let got = block_time_s_for_chain(chain_id);
            assert!(
                (got - 12.0).abs() < 1e-9,
                "unknown chain_id={chain_id} should fall back to 12.0s, got {got}s",
            );
        }
    }

    /// Arbitrum block time is ≤1s — ensures the 6× over-cost regression
    /// (ETH 12s vs ARB 0.25s) cannot be re-introduced silently.
    #[test]
    fn arbitrum_block_time_is_sub_second() {
        let arb_time = block_time_s_for_chain(42161);
        assert!(
            arb_time < 1.0,
            "Arbitrum block time must be <1s (was 12s before fix): got {arb_time}s",
        );
    }
}
