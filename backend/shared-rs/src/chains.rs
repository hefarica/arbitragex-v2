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
}
