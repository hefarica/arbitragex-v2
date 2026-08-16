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
    /// Uniswap Universal Router — command-dispatcher contract; a single
    /// `execute()` call can carry several swaps (V2/V3) plus permits/wraps.
    UniversalRouter,
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
            RouterKind::UniversalRouter => "universal-router",
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

/// Canonical mainnet token addresses (EIP-55) — the single definition site for
/// the whole workspace (HARDCODE-10 doctrine: no inline `0x…` literals in
/// consumers; reference these consts so any future change is one edit).
/// Well-known public contract addresses, NOT secrets; the gitleaks:allow
/// silences the generic-api-key heuristic at this one definition site only.
pub const WETH_MAINNET: &str = "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"; // gitleaks:allow
pub const USDC_MAINNET: &str = "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"; // gitleaks:allow
pub const USDT_MAINNET: &str = "0xdAC17F958D2ee523a2206206994597C13D831ec7"; // gitleaks:allow
pub const DAI_MAINNET: &str = "0x6B175474E89094C44Da98b954EedeAC495271d0F"; // gitleaks:allow
pub const WBTC_MAINNET: &str = "0x2260FAC5E5542a773Aa44fBCfeDf7C193bc2C599"; // gitleaks:allow
pub const LINK_MAINNET: &str = "0x514910771AF9Ca656af840dff83E8264EcF986CA"; // gitleaks:allow
pub const UNI_MAINNET: &str = "0x1f9840a85d5aF5bf1D1762F925BDADdC4201F984"; // gitleaks:allow
pub const AAVE_MAINNET: &str = "0x7Fc66500c84A76Ad7e9c93437bFc5Ac33E2DDaE9"; // gitleaks:allow

/// Lowercase twins of the primaries above, for consumers that match on
/// lowercased runtime strings (e.g. `format!("0x{:040x}", addr)` table
/// lookups in erc20_storage). Match-arm patterns cannot call
/// `.to_lowercase()` on a const, so the lowercase form needs its own const;
/// both forms must stay byte-identical (pinned by the test below).
pub const WETH_MAINNET_LC: &str = "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2";
pub const USDC_MAINNET_LC: &str = "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48";
pub const USDT_MAINNET_LC: &str = "0xdac17f958d2ee523a2206206994597c13d831ec7";
pub const DAI_MAINNET_LC: &str = "0x6b175474e89094c44da98b954eedeac495271d0f";
pub const WBTC_MAINNET_LC: &str = "0x2260fac5e5542a773aa44fbcfedf7c193bc2c599";
pub const LINK_MAINNET_LC: &str = "0x514910771af9ca656af840dff83e8264ecf986ca";
pub const UNI_MAINNET_LC: &str = "0x1f9840a85d5af5bf1d1762f925bdaddc4201f984";
pub const AAVE_MAINNET_LC: &str = "0x7fc66500c84a76ad7e9c93437bfc5ac33e2ddae9";

// ─── Canonical stablecoins (STABLEARR-11) ─────────────────────────────────
//
// ONE replaceable definition for the stablecoin set: scattered per-site
// enumerations (price_oracle's inline symbol match, dex/flashloan "$1.00"
// arms, storage-slot tables) must reference this array or its consts, never
// re-list addresses. Symbol set mirrors the locked trust list documented in
// `price_oracle::is_known_stablecoin` (policy 2026-05-05).

/// One entry of the canonical mainnet stablecoin set.
#[derive(Debug, Clone, Copy)]
pub struct StablecoinEntry {
    pub symbol: &'static str,
    pub address: &'static str,
}

/// Canonical mainnet stablecoins — single replaceable definition (STABLEARR-11).
/// Addresses EIP-55; derived matchers below (`is_stablecoin_symbol`,
/// `is_stablecoin_address_lc`) serve exact-match arms without needing an
/// `_LC` twin per entry.
pub const STABLECOINS_MAINNET: &[StablecoinEntry] = &[
    StablecoinEntry {
        symbol: "USDC",
        address: USDC_MAINNET,
    },
    StablecoinEntry {
        symbol: "USDT",
        address: USDT_MAINNET,
    },
    StablecoinEntry {
        symbol: "DAI",
        address: DAI_MAINNET,
    },
    StablecoinEntry {
        symbol: "BUSD",
        address: "0x4Fabb145d64652a948d72533023f6E7A623C7C53", // gitleaks:allow
    },
    StablecoinEntry {
        symbol: "FRAX",
        address: "0x853d955aCEf822Db058eb8505911ED77F175b99e", // gitleaks:allow
    },
    StablecoinEntry {
        symbol: "LUSD",
        address: "0x5f98805A4E8be255a32880FDeC7F6728C6568bA0", // gitleaks:allow
    },
    StablecoinEntry {
        symbol: "USDP",
        address: "0x8E870D67F660D95d5be530380D0eC0bd388289E1", // gitleaks:allow
    },
    StablecoinEntry {
        symbol: "TUSD",
        address: "0x0000000000085d4780B73119b644AE5ecd22b376", // gitleaks:allow
    },
    StablecoinEntry {
        symbol: "GUSD",
        address: "0x056Fd409E1d7A124BD7017459dFEa2F387b6d5Cd", // gitleaks:allow
    },
    StablecoinEntry {
        symbol: "USDD",
        address: "0x4f8e5DE400DE08B164E7421B3EE387f461beCD1A", // gitleaks:allow
    },
    StablecoinEntry {
        symbol: "PYUSD",
        address: "0x6c3ea9036406852006290770BEdFcAbA0e23A0e8", // gitleaks:allow
    },
];

/// Case-insensitive symbol membership in `STABLECOINS_MAINNET` — the one
/// stablecoin definition site (STABLEARR-11). Mainnet wrapper kept for #343
/// consumers; delegates to the chain-aware core (STABLEARR-12).
pub fn is_stablecoin_symbol(sym: &str) -> bool {
    is_stablecoin_symbol_for_chain(1, sym)
}

/// Case-insensitive canonical-address membership in `STABLECOINS_MAINNET`.
/// Mainnet wrapper kept for #343 consumers; delegates to the chain-aware
/// core (STABLEARR-12).
pub fn is_stablecoin_address_lc(addr: &str) -> bool {
    is_stablecoin_address_lc_for_chain(1, addr)
}

// ─── Multichain token catalogs (STABLEARR-12) ──────────────────────────────
//
// Catalogs for every Alchemy-servable chain + the repo's testnets so future
// chain enablement is data-only (operator order). EVERY address below was
// verified on-chain 2026-08-16 via `eth_call symbol()` (0x95d89b41) +
// `decimals()` (0x313ce567) against chainId-checked public RPCs; the `symbol`
// recorded is the literal decoded on-chain value. Candidates whose decode
// mismatched or whose RPC was unreachable were EXCLUDED, not guessed
// (fail-honest, RULE 00). Notable on-chain facts captured by that sweep:
//
//   - USDC choice: Circle-ISSUED NATIVE USDC is preferred over bridged
//     USDC.e wherever it exists (Arbitrum, Optimism, Base, Polygon);
//     the bridged variants are deliberately not catalogued.
//   - Arbitrum: the classic bridged USDT contract carries NO code today
//     (eth_getCode empty on two independent RPCs) — the live Tether issuance
//     self-identifies as `USD₮0` (the ₮ is U+20AE, part of the symbol bytes).
//   - Polygon: the classic USDT contract self-identifies as `USDT0` after
//     the Tether migration; the wrapped-native contract (former WMATIC)
//     self-identifies as `WPOL`.
//   - Avalanche: Tether's symbol bytes are `USDt` (lowercase t) and the
//     bridged DAI is `DAI.e`.
//   - Arbitrum + Optimism DAI share one address (0xDA10…) — deterministic
//     deployment coincidence, verified independently on each chain.
//   - Testnets: only issuer-backed USDC (Circle test mints) exists
//     canonically on Sepolia/Arb-Sepolia/OP-Sepolia; no canonical USDT/DAI
//     deployments exist there, so those arrays are intentionally 1-entry.

/// Native wrapped-token const per chain (WETH9/WETH/WPOL/WAVAX/WBNB), EIP-55.
/// Same discipline as the mainnet primaries above: well-known public contract
/// addresses, NOT secrets; `gitleaks:allow` at this definition site only.
pub const WETH_ARBITRUM: &str = "0x82aF49447D8a07e3bd95BD0d56f35241523fBab1"; // gitleaks:allow
pub const WETH_OPTIMISM: &str = "0x4200000000000000000000000000000000000006"; // gitleaks:allow
pub const WETH_BASE: &str = "0x4200000000000000000000000000000000000006"; // gitleaks:allow
/// Former WMATIC contract; symbol() now decodes `WPOL` (POL migration rename).
pub const WPOL_POLYGON: &str = "0x0d500B1d8E8eF31E21C99d1Db9A6444d3ADf1270"; // gitleaks:allow
pub const WAVAX_AVALANCHE: &str = "0xB31f66AA3C1e785363F0875A1B74E27b85FD66c7"; // gitleaks:allow
pub const WBNB_BNB: &str = "0xbb4CdB9CBd36B01bD1cBaEBF2De08d9173bc095c"; // gitleaks:allow
pub const WETH_SEPOLIA: &str = "0xfFf9976782d46CC05630D1f6eBAb18b2324d6B14"; // gitleaks:allow
/// Per Arbitrum's official useful-addresses doc; the other widely-cited
/// candidate sharing this prefix carries no code on 421614 (verified dead).
pub const WETH_ARB_SEPOLIA: &str = "0x980B62Da83eFf3D4576C647993b0c1D7faf17c73"; // gitleaks:allow
/// OP-Stack WETH predeploy — same canonical address on every OP-Stack chain.
pub const WETH_OP_SEPOLIA: &str = "0x4200000000000000000000000000000000000006"; // gitleaks:allow

/// Arbitrum — native Circle USDC preferred over bridged USDC.e; live Tether
/// issuance self-identifies `USD₮0` (₮ = U+20AE, byte-exact as decoded).
pub const STABLECOINS_ARBITRUM: &[StablecoinEntry] = &[
    StablecoinEntry {
        symbol: "USDC",
        address: "0xaf88d065e77c8cC2239327C5EDb3A432268e5831", // gitleaks:allow
    },
    StablecoinEntry {
        symbol: "USD₮0",
        address: "0xFd086bC7CD5C481DCC9C85ebE478A1C0b69FCbb9", // gitleaks:allow
    },
    StablecoinEntry {
        symbol: "DAI",
        address: "0xDA10009cBd5D07dd0CeCc66161FC93D7c9000da1", // gitleaks:allow
    },
];

/// Optimism — native Circle USDC preferred over bridged USDC.e.
pub const STABLECOINS_OPTIMISM: &[StablecoinEntry] = &[
    StablecoinEntry {
        symbol: "USDC",
        address: "0x0b2C639c533813f4Aa9D7837CAf62653d097Ff85", // gitleaks:allow
    },
    StablecoinEntry {
        symbol: "USDT",
        address: "0x94b008aA00579c1307B0EF2c499aD98a8ce58e58", // gitleaks:allow
    },
    StablecoinEntry {
        symbol: "DAI",
        address: "0xDA10009cBd5D07dd0CeCc66161FC93D7c9000da1", // gitleaks:allow
    },
];

/// Base — no canonical USDT exists natively; honest 2-entry set.
pub const STABLECOINS_BASE: &[StablecoinEntry] = &[
    StablecoinEntry {
        symbol: "USDC",
        address: "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913", // gitleaks:allow
    },
    StablecoinEntry {
        symbol: "DAI",
        address: "0x50c5725949A6F0c72E6C4a641F24049A917DB0Cb", // gitleaks:allow
    },
];

/// Polygon — native Circle USDC preferred over bridged USDC.e; classic USDT
/// contract self-identifies `USDT0` after the Tether migration.
pub const STABLECOINS_POLYGON: &[StablecoinEntry] = &[
    StablecoinEntry {
        symbol: "USDC",
        address: "0x3c499c542cEF5E3811e1192ce70d8cC03d5c3359", // gitleaks:allow
    },
    StablecoinEntry {
        symbol: "USDT0",
        address: "0xc2132D05D31c914a87C6611C10748AEb04B58e8F", // gitleaks:allow
    },
    StablecoinEntry {
        symbol: "DAI",
        address: "0x8f3Cf7ad23Cd3CaDbD9735AFf958023239c6A063", // gitleaks:allow
    },
];

/// Avalanche — Tether symbol bytes are `USDt`; bridged Dai is `DAI.e`.
pub const STABLECOINS_AVALANCHE: &[StablecoinEntry] = &[
    StablecoinEntry {
        symbol: "USDC",
        address: "0xB97EF9Ef8734C71904D8002F8b6Bc66Dd9c48a6E", // gitleaks:allow
    },
    StablecoinEntry {
        symbol: "USDt",
        address: "0x9702230A8Ea53601f5cD2dc00fDBc13d4dF4A8c7", // gitleaks:allow
    },
    StablecoinEntry {
        symbol: "DAI.e",
        address: "0xd586E7F844cEa2F87f50152665BCbc2C279D8d70", // gitleaks:allow
    },
];

/// BNB Smart Chain — Binance-pegged set (all 18 decimals on this chain).
pub const STABLECOINS_BNB: &[StablecoinEntry] = &[
    StablecoinEntry {
        symbol: "USDC",
        address: "0x8AC76a51cc950d9822D68b83fE1Ad97B32Cd580d", // gitleaks:allow
    },
    StablecoinEntry {
        symbol: "USDT",
        address: "0x55d398326f99059fF775485246999027B3197955", // gitleaks:allow
    },
    StablecoinEntry {
        symbol: "DAI",
        address: "0x1AF3F329e8BE154074D8769D1FFa4eE058B1DBc3", // gitleaks:allow
    },
];

/// Sepolia — only issuer-backed canonical stable is Circle's test USDC.
pub const STABLECOINS_SEPOLIA: &[StablecoinEntry] = &[StablecoinEntry {
    symbol: "USDC",
    address: "0x1c7D4B196Cb0C7B01d743Fbc6116a902379C7238", // gitleaks:allow
}];

/// Arbitrum Sepolia — only issuer-backed canonical stable is Circle's test
/// USDC.
pub const STABLECOINS_ARB_SEPOLIA: &[StablecoinEntry] = &[StablecoinEntry {
    symbol: "USDC",
    address: "0x75faf114eafb1BDbe2F0316DF893fd58CE46AA4d", // gitleaks:allow
}];

/// Optimism Sepolia — only issuer-backed canonical stable is Circle's test
/// USDC.
pub const STABLECOINS_OP_SEPOLIA: &[StablecoinEntry] = &[StablecoinEntry {
    symbol: "USDC",
    address: "0x5fd84259d66Cd46123540766Be93DFE6D43130D7", // gitleaks:allow
}];

/// Returns the static stablecoin catalog for a given chain — mirrors
/// `routers_for_chain`. Unknown chains get an empty slice (fail-honest:
/// no stablecoin is claimed for a chain that was never verified).
pub fn stablecoins_for_chain(chain_id: u64) -> &'static [StablecoinEntry] {
    match chain_id {
        1 => STABLECOINS_MAINNET,
        42161 => STABLECOINS_ARBITRUM,
        10 => STABLECOINS_OPTIMISM,
        8453 => STABLECOINS_BASE,
        137 => STABLECOINS_POLYGON,
        43114 => STABLECOINS_AVALANCHE,
        56 => STABLECOINS_BNB,
        11155111 => STABLECOINS_SEPOLIA,
        421614 => STABLECOINS_ARB_SEPOLIA,
        11155420 => STABLECOINS_OP_SEPOLIA,
        _ => &[],
    }
}

/// Case-insensitive symbol membership in a chain's stablecoin catalog
/// (chain-aware core; the mainnet-signature `is_stablecoin_symbol` above
/// delegates here with chain 1).
pub fn is_stablecoin_symbol_for_chain(chain_id: u64, sym: &str) -> bool {
    stablecoins_for_chain(chain_id)
        .iter()
        .any(|e| e.symbol.eq_ignore_ascii_case(sym))
}

/// Case-insensitive canonical-address membership in a chain's stablecoin
/// catalog (chain-aware core; the mainnet-signature `is_stablecoin_address_lc`
/// above delegates here with chain 1).
pub fn is_stablecoin_address_lc_for_chain(chain_id: u64, addr: &str) -> bool {
    stablecoins_for_chain(chain_id)
        .iter()
        .any(|e| e.address.eq_ignore_ascii_case(addr))
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

// Universal Router (command-dispatcher entrypoint). One execute() tx can
// carry several swaps, so volume here is meaningful for the searcher.
// Addresses per the official Uniswap deployments docs / deploy-addresses repo:
//   v2     = current preferred entrypoint
//   v2.1.1 = latest revision (Uniswap swapping API)
//   v1.2   = legacy, still used by older integrators
const UNIVERSAL_ROUTER_V2_MAINNET: RouterEntry = RouterEntry {
    chain_id: 1,
    name: "universal-router-v2",
    kind: RouterKind::UniversalRouter,
    address: hex20("0x66a9893cc07d91d95644aedd05d03f95e1dba8af"),
};
const UNIVERSAL_ROUTER_V2_1_1_MAINNET: RouterEntry = RouterEntry {
    chain_id: 1,
    name: "universal-router-v2-1-1",
    kind: RouterKind::UniversalRouter,
    address: hex20("0x4c82d1fbfe28c977cbb58d8c7ff8fcf9f70a2cca"),
};
const UNIVERSAL_ROUTER_V1_2_MAINNET: RouterEntry = RouterEntry {
    chain_id: 1,
    name: "universal-router-v1-2",
    kind: RouterKind::UniversalRouter,
    address: hex20("0x3fc91a3afd70395cd496c647d5a6cc9d4b2b7fad"),
};

pub const ROUTERS_MAINNET: &[RouterEntry] = &[
    UNIV2_ROUTER_MAINNET,
    UNIV3_SWAPROUTER_MAINNET,
    UNIV3_SWAPROUTER02_MAINNET,
    SUSHI_ROUTER_MAINNET,
    UNIVERSAL_ROUTER_V2_MAINNET,
    UNIVERSAL_ROUTER_V2_1_1_MAINNET,
    UNIVERSAL_ROUTER_V1_2_MAINNET,
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

    /// Catalog primaries must be valid EIP-55 checksums (a mistyped case in
    /// the const would silently diverge from the canonical deployment), and
    /// every `_LC` twin must be the byte-identical lowercase of its primary
    /// (match-arm consumers in erc20_storage depend on exact equality).
    #[test]
    fn token_consts_are_eip55_and_lc_twins_match() {
        for (primary, lc) in [
            (WETH_MAINNET, WETH_MAINNET_LC),
            (USDC_MAINNET, USDC_MAINNET_LC),
            (USDT_MAINNET, USDT_MAINNET_LC),
            (DAI_MAINNET, DAI_MAINNET_LC),
            (WBTC_MAINNET, WBTC_MAINNET_LC),
            (LINK_MAINNET, LINK_MAINNET_LC),
            (UNI_MAINNET, UNI_MAINNET_LC),
            (AAVE_MAINNET, AAVE_MAINNET_LC),
        ] {
            let addr = Address::from_str(primary).expect("primary parses as address");
            assert_eq!(
                &ethers::utils::to_checksum(&addr, None),
                primary,
                "primary must be the EIP-55 checksum form"
            );
            assert_eq!(
                primary.to_lowercase(),
                lc,
                "_LC twin must be the lowercase of its primary"
            );
        }
    }

    /// STABLEARR-11: the stablecoin array is the single definition site —
    /// every entry's address must be a valid EIP-55 checksum, symbols must be
    /// unique, and the three majors (USDT/USDC/DAI) must be present so an
    /// accidental truncation of the array fails loudly.
    #[test]
    fn stablecoins_mainnet_array_pinned() {
        let mut seen = std::collections::HashSet::new();
        for e in STABLECOINS_MAINNET {
            let addr = Address::from_str(e.address).expect("stablecoin address parses");
            assert_eq!(
                &ethers::utils::to_checksum(&addr, None),
                e.address,
                "{} address must be stored in EIP-55 checksum form",
                e.symbol
            );
            assert!(seen.insert(e.symbol), "duplicate symbol {}", e.symbol);
        }
        for sym in ["USDT", "USDC", "DAI"] {
            assert!(
                STABLECOINS_MAINNET.iter().any(|e| e.symbol == sym),
                "{sym} must stay in STABLECOINS_MAINNET"
            );
        }
        // Derived matchers iterate the same array and are case-insensitive.
        assert!(is_stablecoin_symbol("usdt"));
        assert!(is_stablecoin_symbol("PYUSD"));
        assert!(!is_stablecoin_symbol("USDE"));
        assert!(is_stablecoin_address_lc(USDT_MAINNET_LC));
        assert!(!is_stablecoin_address_lc(WBTC_MAINNET_LC));
    }

    /// STABLEARR-12: every multichain wrapped-token const must be the valid
    /// EIP-55 checksum form (a mistyped case would silently diverge from the
    /// RPC-verified deployment), and the three OP-Stack chains must share the
    /// canonical predeploy address.
    #[test]
    fn wrapped_token_consts_are_eip55() {
        for c in [
            WETH_ARBITRUM,
            WETH_OPTIMISM,
            WETH_BASE,
            WPOL_POLYGON,
            WAVAX_AVALANCHE,
            WBNB_BNB,
            WETH_SEPOLIA,
            WETH_ARB_SEPOLIA,
            WETH_OP_SEPOLIA,
        ] {
            let addr = Address::from_str(c).expect("wrapped-token const parses as address");
            assert_eq!(
                &ethers::utils::to_checksum(&addr, None),
                c,
                "wrapped-token const must be stored in EIP-55 checksum form"
            );
        }
        // OP-Stack WETH predeploy is one canonical address across OP chains.
        assert_eq!(WETH_OPTIMISM, "0x4200000000000000000000000000000000000006");
        assert_eq!(WETH_BASE, WETH_OPTIMISM);
        assert_eq!(WETH_OP_SEPOLIA, WETH_OPTIMISM);
    }

    /// STABLEARR-12: per-chain stablecoin catalogs pinned against the table
    /// verified on-chain 2026-08-16 (eth_call symbol()/decimals() via
    /// chainId-checked public RPCs). Any address/symbol drift from that
    /// verified table must fail loudly here. Symbols are the LITERAL decoded
    /// on-chain values (note USD₮0 on Arbitrum, USDT0 on Polygon, USDt on
    /// Avalanche). A new chain addition must update BOTH the catalog above
    /// and this table.
    #[test]
    fn multichain_stablecoin_catalogs_pinned() {
        let verified: &[(u64, &[(&str, &str)])] = &[
            (
                42161,
                &[
                    ("USDC", "0xaf88d065e77c8cC2239327C5EDb3A432268e5831"),
                    ("USD₮0", "0xFd086bC7CD5C481DCC9C85ebE478A1C0b69FCbb9"),
                    ("DAI", "0xDA10009cBd5D07dd0CeCc66161FC93D7c9000da1"),
                ],
            ),
            (
                10,
                &[
                    ("USDC", "0x0b2C639c533813f4Aa9D7837CAf62653d097Ff85"),
                    ("USDT", "0x94b008aA00579c1307B0EF2c499aD98a8ce58e58"),
                    ("DAI", "0xDA10009cBd5D07dd0CeCc66161FC93D7c9000da1"),
                ],
            ),
            (
                8453,
                &[
                    ("USDC", "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913"),
                    ("DAI", "0x50c5725949A6F0c72E6C4a641F24049A917DB0Cb"),
                ],
            ),
            (
                137,
                &[
                    ("USDC", "0x3c499c542cEF5E3811e1192ce70d8cC03d5c3359"),
                    ("USDT0", "0xc2132D05D31c914a87C6611C10748AEb04B58e8F"),
                    ("DAI", "0x8f3Cf7ad23Cd3CaDbD9735AFf958023239c6A063"),
                ],
            ),
            (
                43114,
                &[
                    ("USDC", "0xB97EF9Ef8734C71904D8002F8b6Bc66Dd9c48a6E"),
                    ("USDt", "0x9702230A8Ea53601f5cD2dc00fDBc13d4dF4A8c7"),
                    ("DAI.e", "0xd586E7F844cEa2F87f50152665BCbc2C279D8d70"),
                ],
            ),
            (
                56,
                &[
                    ("USDC", "0x8AC76a51cc950d9822D68b83fE1Ad97B32Cd580d"),
                    ("USDT", "0x55d398326f99059fF775485246999027B3197955"),
                    ("DAI", "0x1AF3F329e8BE154074D8769D1FFa4eE058B1DBc3"),
                ],
            ),
            (
                11155111,
                &[("USDC", "0x1c7D4B196Cb0C7B01d743Fbc6116a902379C7238")],
            ),
            (
                421614,
                &[("USDC", "0x75faf114eafb1BDbe2F0316DF893fd58CE46AA4d")],
            ),
            (
                11155420,
                &[("USDC", "0x5fd84259d66Cd46123540766Be93DFE6D43130D7")],
            ),
        ];
        for &(chain_id, expected) in verified {
            let got = stablecoins_for_chain(chain_id);
            assert_eq!(
                got.len(),
                expected.len(),
                "chain {chain_id}: catalog size drifted from the verified table"
            );
            let mut seen = std::collections::HashSet::new();
            for (e, (sym, addr)) in got.iter().zip(expected) {
                assert_eq!(e.symbol, *sym, "chain {chain_id}: symbol drifted");
                assert_eq!(e.address, *addr, "chain {chain_id}: address drifted");
                let parsed = Address::from_str(e.address)
                    .unwrap_or_else(|_| panic!("chain {chain_id}: address parses"));
                assert_eq!(
                    &ethers::utils::to_checksum(&parsed, None),
                    e.address,
                    "chain {chain_id}: {} must be stored in EIP-55 form",
                    e.symbol
                );
                assert!(seen.insert(e.symbol), "chain {chain_id}: duplicate symbol");
            }
        }
    }

    /// STABLEARR-12: fail-honest dispatch — unknown chains get an empty
    /// catalog and match nothing, and catalogs are chain-isolated (a mainnet
    /// stable address must not match on another chain).
    #[test]
    fn stablecoin_dispatch_is_chain_aware_and_fail_honest() {
        assert!(stablecoins_for_chain(999).is_empty());
        assert!(!stablecoins_for_chain(999)
            .iter()
            .any(|e| e.symbol == "USDC"));
        assert!(!is_stablecoin_symbol_for_chain(999, "USDC"));
        assert!(!is_stablecoin_address_lc_for_chain(999, USDC_MAINNET_LC));

        // Mainnet wrapper still behaves exactly as #343 (chain 1 core).
        assert!(is_stablecoin_symbol("usdt"));
        assert!(is_stablecoin_address_lc(USDC_MAINNET_LC));

        // Chain-aware cores: verified symbols match per chain (recorded
        // casing variants included)…
        assert!(is_stablecoin_symbol_for_chain(42161, "USD₮0"));
        assert!(is_stablecoin_symbol_for_chain(137, "usdt0"));
        assert!(is_stablecoin_symbol_for_chain(43114, "USDT")); // case-insensitive vs USDt
        assert!(is_stablecoin_address_lc_for_chain(
            42161,
            "0xaf88d065e77c8cc2239327c5edb3a432268e5831"
        ));

        // …and catalogs do NOT leak across chains: mainnet USDC address is
        // not an Arbitrum/Polygon stable, and vice versa.
        assert!(!is_stablecoin_address_lc_for_chain(42161, USDC_MAINNET_LC));
        assert!(!is_stablecoin_address_lc(
            "0xaf88d065e77c8cc2239327c5edb3a432268e5831"
        ));
    }

    #[test]
    fn univ2_router_mainnet_address_bytes() {
        let expected: [u8; 20] = [
            0x7a, 0x25, 0x0d, 0x56, 0x30, 0xb4, 0xcf, 0x53, 0x97, 0x39, 0xdf, 0x2c, 0x5d, 0xac,
            0xb4, 0xc6, 0x59, 0xf2, 0x48, 0x8d,
        ];
        assert_eq!(UNIV2_ROUTER_MAINNET.address, expected);
    }

    #[test]
    fn universal_router_mainnet_address_bytes() {
        // Byte-exact pins for the three catalogued UR deployments
        // (0x66a9...a8af, 0x4c82...2cca, 0x3fc9...7fad).
        let v2: [u8; 20] = [
            0x66, 0xa9, 0x89, 0x3c, 0xc0, 0x7d, 0x91, 0xd9, 0x56, 0x44, 0xae, 0xdd, 0x05, 0xd0,
            0x3f, 0x95, 0xe1, 0xdb, 0xa8, 0xaf,
        ];
        let v2_1_1: [u8; 20] = [
            0x4c, 0x82, 0xd1, 0xfb, 0xfe, 0x28, 0xc9, 0x77, 0xcb, 0xb5, 0x8d, 0x8c, 0x7f, 0xf8,
            0xfc, 0xf9, 0xf7, 0x0a, 0x2c, 0xca,
        ];
        let v1_2: [u8; 20] = [
            0x3f, 0xc9, 0x1a, 0x3a, 0xfd, 0x70, 0x39, 0x5c, 0xd4, 0x96, 0xc6, 0x47, 0xd5, 0xa6,
            0xcc, 0x9d, 0x4b, 0x2b, 0x7f, 0xad,
        ];
        assert_eq!(UNIVERSAL_ROUTER_V2_MAINNET.address, v2);
        assert_eq!(UNIVERSAL_ROUTER_V2_1_1_MAINNET.address, v2_1_1);
        assert_eq!(UNIVERSAL_ROUTER_V1_2_MAINNET.address, v1_2);
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
        assert_eq!(RouterKind::UniversalRouter.as_str(), "universal-router");
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
