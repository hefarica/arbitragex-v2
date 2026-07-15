//! Chain + router catalog.
//!
//! Static, configuration-independent addresses for known routers. We keep the
//! catalog in code (not TOML) on purpose: these are protocol constants that
//! change extremely rarely and have strong audit requirements. Any addition
//! must be reviewed.

use ethers::types::Address;
use std::str::FromStr;

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
            RouterKind::Sushi => "sushi",
            RouterKind::Curve => "curve",
            RouterKind::Balancer => "balancer",
            RouterKind::Unknown => "unknown",
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

// Sepolia testnet routers (chain_id: 11155111)
const UNIV3_SWAPROUTER_SEPOLIA: RouterEntry = RouterEntry {
    chain_id: 11155111,
    name: "uniswap-v3-swap-router-sepolia",
    kind: RouterKind::UniswapV3,
    address: hex20("0x3bFA4769FB12BF0E297FFB62d94D4E91C4f47A28"),
};

pub const ROUTERS_SEPOLIA: &[RouterEntry] = &[UNIV3_SWAPROUTER_SEPOLIA];

// Arbitrum Sepolia testnet routers (chain_id: 421614)
const UNIV3_SWAPROUTER_ARB_SEPOLIA: RouterEntry = RouterEntry {
    chain_id: 421614,
    name: "uniswap-v3-swap-router-arb-sepolia",
    kind: RouterKind::UniswapV3,
    address: hex20("0x101F443B4d1b059569D643917553c771E1b9663E"),
};

pub const ROUTERS_ARB_SEPOLIA: &[RouterEntry] = &[UNIV3_SWAPROUTER_ARB_SEPOLIA];

// Optimism Sepolia testnet routers (chain_id: 11155420)
const UNIV3_SWAPROUTER_OP_SEPOLIA: RouterEntry = RouterEntry {
    chain_id: 11155420,
    name: "uniswap-v3-swap-router-op-sepolia",
    kind: RouterKind::UniswapV3,
    address: hex20("0x94cC0AaC535CCDB3C01d6787D6413C739ae12bc4"),
};

pub const ROUTERS_OP_SEPOLIA: &[RouterEntry] = &[UNIV3_SWAPROUTER_OP_SEPOLIA];

/// Returns the static router catalog for a given chain.
pub fn routers_for_chain(chain_id: u64) -> &'static [RouterEntry] {
    match chain_id {
        1 => ROUTERS_MAINNET,
        11155111 => ROUTERS_SEPOLIA,
        421614 => ROUTERS_ARB_SEPOLIA,
        11155420 => ROUTERS_OP_SEPOLIA,
        _ => &[],
    }
}

// Uniswap V3 QuoterV2 + canonical Multicall3, centralized here (were hardcoded
// in triangular_worker.rs). Read-only quote infrastructure — no signer/capital.
const QUOTER_V2_MAINNET: [u8; 20] = hex20("0x61fFE014bA17989E743c5F6cB21bF9697530B21e");
const QUOTER_V2_SEPOLIA: [u8; 20] = hex20("0xEd1f6473345F45b75B8178D559b7bf91486307e2");
const QUOTER_V2_ARB_SEPOLIA: [u8; 20] = hex20("0x2779a0CC1c3e0E44D254bC76C39A63ed67Bc2a61");
const QUOTER_V2_OP_SEPOLIA: [u8; 20] = hex20("0xb27308f9F90D607463bb33eA1BeBb41C27CE5AB6");

const MULTICALL3_CANONICAL: [u8; 20] = hex20("0xcA11bde05977b3631167028862bE2a173976CA11");

/// Uniswap V3 QuoterV2 address for a chain (read-only `quoteExactInputSingle`).
/// `None` when the chain has no catalogued quoter — caller fails honest.
pub fn quoter_v2_for_chain(chain_id: u64) -> Option<[u8; 20]> {
    match chain_id {
        1 => Some(QUOTER_V2_MAINNET),
        11155111 => Some(QUOTER_V2_SEPOLIA),
        421614 => Some(QUOTER_V2_ARB_SEPOLIA),
        11155420 => Some(QUOTER_V2_OP_SEPOLIA),
        _ => None,
    }
}

/// Multicall3 address for a chain. The canonical CREATE2 deployment shares one
/// address across most EVM chains; only mainnet is seeded here for now.
pub fn multicall3_for_chain(chain_id: u64) -> Option<[u8; 20]> {
    match chain_id {
        1 => Some(MULTICALL3_CANONICAL),
        11155111 => Some(MULTICALL3_CANONICAL),
        421614 => Some(MULTICALL3_CANONICAL),
        11155420 => Some(MULTICALL3_CANONICAL),
        _ => None,
    }
}

/// Finds a router entry by address (case-insensitive, byte-exact).
pub fn find_router(chain_id: u64, addr: &[u8; 20]) -> Option<&'static RouterEntry> {
    routers_for_chain(chain_id)
        .iter()
        .find(|r| &r.address == addr)
}

/// Returns the average block time in seconds for a given chain.
///
/// Used for capital opportunity-cost computation:
///   `capital_cost_usd = amount_in_usd × (rate_annual / 100) × (block_time_s / 31_536_000)`.
///
/// Values are conservative best-estimates (round up, not down) so
/// capital cost is never under-counted:
///
/// | Chain           | chain_id | Typical block time | Source                             |
/// |-----------------|----------|--------------------|------------------------------------|
/// | Ethereum        | 1        | 12.0 s             | PoS slot time (12s)                |
/// | Sepolia         | 11155111 | 12.0 s             | PoS slot time (12s)                |
/// | BSC             | 56       | 3.0 s              | BNB Chain PoSA ~3s                 |
/// | Polygon         | 137      | 2.0 s              | Bor PoS ~2s                        |
/// | Base            | 8453     | 2.0 s              | OP-Stack (2s slots)                |
/// | Arbitrum        | 42161    | 0.5 s              | Nitro sub-second; 0.5s safety buf  |
/// | Arbitrum Sepolia| 421614   | 0.5 s              | Nitro sub-second; 0.5s safety buf  |
/// | Optimism        | 10       | 2.0 s              | OP-Stack (2s slots)                |
/// | Optimism Sepolia| 11155420 | 2.0 s              | OP-Stack (2s slots)                |
/// | unknown         | _        | 12.0 s             | ETH-equivalent fallback            |
///
/// BE-3.7 refinement (2026-05-08):
///   Arbitrum adjusted from 0.25s to 0.5s. Real Nitro block times are
///   ~0.25s under normal load but can spike to ~1s under congestion.
///   0.5s is the pragmatic safety margin: still 24× cheaper than the
///   pre-fix ETH 12s fallback, but won't under-count capital cost on
///   congested epochs. Capital cost formula is linear in block_time_s;
///   over-counting by 2× is a ~$0.001 penalty on $10k notional, negligible.
///
/// Sprint H1 follow-up: replaced the hardcoded `ETH_BLOCK_TIME_S = 12.0`
/// constant in `prioritization-spine/config_aware.rs`.
pub fn block_time_s_for_chain(chain_id: u64) -> f64 {
    match chain_id {
        1 => 12.0,        // Ethereum mainnet (PoS 12s slots)
        11155111 => 12.0, // Sepolia (PoS 12s slots)
        56 => 3.0,        // BNB Smart Chain (PoSA ~3s)
        137 => 2.0,       // Polygon PoS (~2s)
        8453 => 2.0,      // Base (OP-Stack 2s slots)
        42161 => 0.5,     // Arbitrum Nitro: ~0.25s real; 0.5s safety buffer (BE-3.7)
        421614 => 0.5,    // Arbitrum Sepolia (BE-3.7)
        10 => 2.0,        // Optimism (OP-Stack 2s slots)
        11155420 => 2.0,  // Optimism Sepolia (OP-Stack 2s slots)
        _ => 12.0,        // unknown → ETH-equivalent (conservative)
    }
}

/// Returns the number of blocks to treat as a reorg safety buffer when
/// computing capital risk windows for a given chain.
///
/// This is NOT the standard confirmation depth for exchange deposits — it is
/// the MEV-specific window during which a reorg could invalidate an already
/// included opportunity, affecting PnL accounting. Used to size the hold
/// time in future drift-tracker and risk-management work.
///
/// L2 chains (Arbitrum, Optimism, Base) have zero L2-level reorg risk after
/// the sequencer orders the tx: they inherit finality from L1 checkpointing,
/// but individual blocks are not reorganised at the L2 layer. We therefore
/// return 0 for OP-Stack and Arbitrum chains — their risk window is the L1
/// finality delay, which is out of scope for this helper.
///
/// Values:
///
/// | Chain     | chain_id | Reorg buffer | Rationale                            |
/// |-----------|----------|--------------|--------------------------------------|
/// | Ethereum  | 1        | 12           | ~2-3 min; 12 blocks is industry std  |
/// | BSC       | 56       | 15           | Known deep reorg history (PoSA)      |
/// | Polygon   | 137      | 256          | Documented reorg incidents >100 blk  |
/// | Base      | 8453     | 0            | OP-Stack: no L2 reorgs               |
/// | Arbitrum  | 42161    | 0            | Nitro sequencer: no L2 reorgs        |
/// | Optimism  | 10       | 0            | OP-Stack: no L2 reorgs               |
/// | unknown   | _        | 12           | ETH-equivalent (conservative)        |
///
/// NOTE: This helper is not yet wired into capital-cost or risk-management
/// scoring. It is provided here for future use (drift-tracker, position
/// sizing) and is covered by unit tests to prevent silent regression.
pub fn reorg_buffer_blocks_for_chain(chain_id: u64) -> u32 {
    match chain_id {
        1 => 12,        // Ethereum: ~2.4 min finality window
        11155111 => 12, // Sepolia: ETH-equivalent
        56 => 15,       // BSC: PoSA; documented reorg incidents
        137 => 256,     // Polygon: repeated deep-reorg history
        8453 => 0,      // Base: OP-Stack sequencer, no L2 reorgs
        42161 => 0,     // Arbitrum Nitro: sequencer, no L2 reorgs
        421614 => 0,    // Arbitrum Sepolia: sequencer, no L2 reorgs
        10 => 0,        // Optimism: OP-Stack sequencer, no L2 reorgs
        11155420 => 0,  // Optimism Sepolia: OP-Stack sequencer, no L2 reorgs
        _ => 12,        // unknown → ETH-equivalent (conservative)
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

// ---------------------------------------------------------------------------
// Executor address resolution
// ---------------------------------------------------------------------------

/// Typed error for `resolve_executor_address`. Mirrors the three executor
/// rejection paths so callers can map each variant to a stable, fail-closed
/// reason. No silent fallbacks: a missing, unparsable, or zero env value each
/// produces a distinct error variant.
#[derive(Debug, thiserror::Error, PartialEq)]
pub enum ExecutorAddrError {
    #[error("EXECUTOR_{chain_id} env var not set")]
    Missing { chain_id: u64 },

    #[error("EXECUTOR_{chain_id} env var value cannot parse as address: {value}")]
    Invalid { chain_id: u64, value: String },

    #[error("EXECUTOR_{chain_id} env var resolves to the zero address")]
    Zero { chain_id: u64 },
}

/// Resolve the deployed `ArbitrageExecutor` proxy address for a chain from the
/// `EXECUTOR_<chain_id>` environment variable, parsed to a non-zero `Address`.
///
/// NO hardcoded fallback addresses; NO test/dummy defaults. Every chain that
/// participates in the executor wire must export this env var explicitly.
/// Missing, invalid, or zero values all reject fail-closed with a typed
/// `ExecutorAddrError` variant.
pub fn resolve_executor_address(chain_id: u64) -> Result<Address, ExecutorAddrError> {
    let key = format!("EXECUTOR_{chain_id}");
    let raw = match std::env::var(&key) {
        Ok(v) if !v.trim().is_empty() => v,
        _ => return Err(ExecutorAddrError::Missing { chain_id }),
    };
    let value = raw.trim();
    let addr = Address::from_str(value).map_err(|_| ExecutorAddrError::Invalid {
        chain_id,
        value: value.to_string(),
    })?;
    if addr == Address::zero() {
        return Err(ExecutorAddrError::Zero { chain_id });
    }
    Ok(addr)
}

/// Resolve the deployed `FlashLoanExecutor` proxy address for a chain from the
/// `FLASHLOAN_EXECUTOR_<chain_id>` environment variable, parsed to a non-zero
/// `Address`. This is the `.to()` target of the M2 flash-funded
/// `requestFlashLoan` transaction (sim R3 + broadcast R4).
///
/// NO hardcoded fallback addresses; NO test/dummy defaults. Every chain that
/// participates in the flash-loan wire must export this env var explicitly.
/// Missing, invalid, or zero values all reject fail-closed with a typed
/// `ExecutorAddrError` variant (reused: its variants carry only `chain_id` /
/// `value`, so they are env-key-agnostic and apply identically here).
pub fn resolve_flashloan_executor_address(chain_id: u64) -> Result<Address, ExecutorAddrError> {
    let key = format!("FLASHLOAN_EXECUTOR_{chain_id}");
    let raw = match std::env::var(&key) {
        Ok(v) if !v.trim().is_empty() => v,
        _ => return Err(ExecutorAddrError::Missing { chain_id }),
    };
    let value = raw.trim();
    let addr = Address::from_str(value).map_err(|_| ExecutorAddrError::Invalid {
        chain_id,
        value: value.to_string(),
    })?;
    if addr == Address::zero() {
        return Err(ExecutorAddrError::Zero { chain_id });
    }
    Ok(addr)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn univ2_router_mainnet_address_bytes() {
        let expected: [u8; 20] = [
            0x7a, 0x25, 0x0d, 0x56, 0x30, 0xb4, 0xcf, 0x53, 0x97, 0x39, 0xdf, 0x2c, 0x5d, 0xac,
            0xb4, 0xc6, 0x59, 0xf2, 0x48, 0x8d,
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
    ///
    /// BE-3.7: Arbitrum updated from 0.25s to 0.5s (congestion safety buffer).
    #[test]
    fn block_time_known_chains() {
        let cases: &[(u64, f64)] = &[
            (1, 12.0),        // Ethereum
            (11155111, 12.0), // Sepolia
            (56, 3.0),        // BSC
            (137, 2.0),       // Polygon
            (8453, 2.0),      // Base
            (42161, 0.5),     // Arbitrum (BE-3.7: 0.5s safety buffer; was 0.25s)
            (421614, 0.5),    // Arbitrum Sepolia
            (10, 2.0),        // Optimism
            (11155420, 2.0),  // Optimism Sepolia
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
    /// (ETH 12s vs ARB 0.25s-0.5s range) cannot be re-introduced silently.
    /// BE-3.7: value is 0.5s (safety buffer), still well under 1s.
    #[test]
    fn arbitrum_block_time_is_sub_second() {
        let arb_time = block_time_s_for_chain(42161);
        assert!(
            arb_time < 1.0,
            "Arbitrum block time must be <1s (was 12s before fix): got {arb_time}s",
        );
    }

    // ---- reorg_buffer_blocks_for_chain tests ----------------------------------

    /// L2 chains (Arbitrum, Optimism, Base) must have zero reorg buffer —
    /// their sequencers do not reorganise at the L2 layer.
    #[test]
    fn l2_chains_have_zero_reorg_buffer() {
        let l2_chains: &[u64] = &[42161, 10, 8453, 421614, 11155420]; // Arbitrum, Optimism, Base + testnets
        for &chain_id in l2_chains {
            let buf = reorg_buffer_blocks_for_chain(chain_id);
            assert_eq!(
                buf, 0,
                "L2 chain_id={chain_id} must have reorg_buffer=0 (sequencer guarantees), got {buf}",
            );
        }
    }

    /// Testnets must have router catalogs.
    #[test]
    fn testnet_routers_exist() {
        assert!(
            !routers_for_chain(11155111).is_empty(),
            "Sepolia must have routers"
        );
        assert!(
            !routers_for_chain(421614).is_empty(),
            "Arbitrum Sepolia must have routers"
        );
        assert!(
            !routers_for_chain(11155420).is_empty(),
            "Optimism Sepolia must have routers"
        );
    }

    /// Testnets must have QuoterV2 for price quotes.
    #[test]
    fn testnet_quoter_v2_exists() {
        assert!(
            quoter_v2_for_chain(11155111).is_some(),
            "Sepolia must have QuoterV2"
        );
        assert!(
            quoter_v2_for_chain(421614).is_some(),
            "Arbitrum Sepolia must have QuoterV2"
        );
        assert!(
            quoter_v2_for_chain(11155420).is_some(),
            "Optimism Sepolia must have QuoterV2"
        );
    }

    /// Testnets must have Multicall3 for batching.
    #[test]
    fn testnet_multicall3_exists() {
        assert!(
            multicall3_for_chain(11155111).is_some(),
            "Sepolia must have Multicall3"
        );
        assert!(
            multicall3_for_chain(421614).is_some(),
            "Arbitrum Sepolia must have Multicall3"
        );
        assert!(
            multicall3_for_chain(11155420).is_some(),
            "Optimism Sepolia must have Multicall3"
        );
    }

    /// Ethereum must have a non-zero reorg buffer (12 blocks = ~2.4 min).
    #[test]
    fn ethereum_reorg_buffer_is_twelve() {
        assert_eq!(reorg_buffer_blocks_for_chain(1), 12);
    }

    /// Polygon must have the highest reorg buffer (256) due to documented
    /// deep-reorg incidents.
    #[test]
    fn polygon_reorg_buffer_is_highest() {
        let polygon = reorg_buffer_blocks_for_chain(137);
        let eth = reorg_buffer_blocks_for_chain(1);
        assert!(
            polygon > eth,
            "Polygon reorg buffer ({polygon}) must exceed Ethereum ({eth})",
        );
        assert_eq!(polygon, 256);
    }

    /// Unknown chain IDs fall back to ETH-equivalent (12), not 0.
    /// A zero reorg buffer for an unknown chain would silently under-count risk.
    #[test]
    fn reorg_buffer_unknown_chain_falls_back_to_eth() {
        let unknown_ids: &[u64] = &[999, 0, u64::MAX, 31337];
        for &chain_id in unknown_ids {
            let buf = reorg_buffer_blocks_for_chain(chain_id);
            assert_eq!(
                buf, 12,
                "unknown chain_id={chain_id} should fall back to reorg_buffer=12, got {buf}",
            );
        }
    }

    // ---- resolve_executor_address fail-closed matrix --------------------------

    /// Unset `EXECUTOR_<chain_id>` rejects with `Missing` (no hardcoded default).
    #[test]
    fn executor_missing_rejected() {
        std::env::remove_var("EXECUTOR_8999");
        let err = resolve_executor_address(8999).unwrap_err();
        assert_eq!(err, ExecutorAddrError::Missing { chain_id: 8999 });
    }

    /// A non-address env value rejects with `Invalid` carrying the raw value.
    #[test]
    fn executor_invalid_rejected() {
        std::env::set_var("EXECUTOR_8998", "not_an_address");
        let err = resolve_executor_address(8998).unwrap_err();
        assert!(matches!(
            err,
            ExecutorAddrError::Invalid { chain_id: 8998, .. }
        ));
        std::env::remove_var("EXECUTOR_8998");
    }

    /// The zero address rejects with `Zero` (never a valid executor).
    #[test]
    fn executor_zero_rejected() {
        std::env::set_var(
            "EXECUTOR_8997",
            "0x0000000000000000000000000000000000000000",
        );
        let err = resolve_executor_address(8997).unwrap_err();
        assert_eq!(err, ExecutorAddrError::Zero { chain_id: 8997 });
        std::env::remove_var("EXECUTOR_8997");
    }

    /// A valid non-zero address parses and round-trips.
    #[test]
    fn executor_valid_resolves() {
        std::env::set_var(
            "EXECUTOR_8996",
            "0x1234567890123456789012345678901234567890",
        );
        let addr = resolve_executor_address(8996).unwrap();
        assert_eq!(
            addr,
            Address::from_str("0x1234567890123456789012345678901234567890").unwrap()
        );
        std::env::remove_var("EXECUTOR_8996");
    }

    // ---- resolve_flashloan_executor_address fail-closed matrix -----------------
    // Distinct chain_ids (8995..8992) from the executor matrix above so the
    // FLASHLOAN_EXECUTOR_<chain_id> env keys never race a parallel test.

    /// Unset `FLASHLOAN_EXECUTOR_<chain_id>` rejects with `Missing` (no default).
    #[test]
    fn flashloan_executor_missing_rejected() {
        std::env::remove_var("FLASHLOAN_EXECUTOR_8995");
        let err = resolve_flashloan_executor_address(8995).unwrap_err();
        assert_eq!(err, ExecutorAddrError::Missing { chain_id: 8995 });
    }

    /// A non-address env value rejects with `Invalid` carrying the raw value.
    #[test]
    fn flashloan_executor_invalid_rejected() {
        std::env::set_var("FLASHLOAN_EXECUTOR_8994", "not_an_address");
        let err = resolve_flashloan_executor_address(8994).unwrap_err();
        assert!(matches!(
            err,
            ExecutorAddrError::Invalid { chain_id: 8994, .. }
        ));
        std::env::remove_var("FLASHLOAN_EXECUTOR_8994");
    }

    /// The zero address rejects with `Zero` (never a valid executor).
    #[test]
    fn flashloan_executor_zero_rejected() {
        std::env::set_var(
            "FLASHLOAN_EXECUTOR_8993",
            "0x0000000000000000000000000000000000000000",
        );
        let err = resolve_flashloan_executor_address(8993).unwrap_err();
        assert_eq!(err, ExecutorAddrError::Zero { chain_id: 8993 });
        std::env::remove_var("FLASHLOAN_EXECUTOR_8993");
    }

    /// A valid non-zero address parses and round-trips.
    #[test]
    fn flashloan_executor_valid_resolves() {
        std::env::set_var(
            "FLASHLOAN_EXECUTOR_8992",
            "0x1234567890123456789012345678901234567890",
        );
        let addr = resolve_flashloan_executor_address(8992).unwrap();
        assert_eq!(
            addr,
            Address::from_str("0x1234567890123456789012345678901234567890").unwrap()
        );
        std::env::remove_var("FLASHLOAN_EXECUTOR_8992");
    }
}
