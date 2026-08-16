//! Route intent — structured representation of a swap detected in the mempool.
//!
//! A `RouteIntent` is the normalised output of the calldata decoder. It captures
//! the intent of a pending transaction (which tokens, in what order, via which
//! router) without yet evaluating whether an arb opportunity exists.
//!
//! ## R8 invariants (fail-honest, spec §3.2)
//!
//! - `pool_hint`, `dex_hint`, `fee_bps`, `min_amount_out` MUST stay `None` when
//!   the decoder cannot extract them from calldata. NEVER fall back to a sentinel.
//! - `protocol_type` defaults to `Unknown` when the router is unrecognised.
//! - `legs.len() >= 1` always — a swap has at least one leg by definition.
//!   The constructor enforces this; callers cannot create a zero-leg intent.

use ethers::types::{Address, H256, U256};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Primary struct
// ---------------------------------------------------------------------------

/// Structured representation of a swap detected from a pending transaction.
///
/// Produced by [`crate::route_decoder::decode_to_route_intents`] and consumed
/// by the orchestrator for strategy fan-out and impact resolution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteIntent {
    /// EVM chain ID (e.g. 1 = Ethereum mainnet).
    pub chain_id: u64,
    /// Hash of the originating pending transaction.
    pub tx_hash: H256,
    /// Router contract address that received the call.
    pub router: Address,
    /// Classified kind of the router.
    pub router_kind: RouterKind,
    /// Externally-owned account that sent the transaction.
    pub sender: Address,
    /// Ordered swap legs. Invariant: `legs.len() >= 1`.
    pub legs: Vec<RouteIntentLeg>,
    /// Total input amount for the first leg.
    pub amount_in: U256,
    /// Minimum output amount acceptable to the sender (`amountOutMinimum`).
    /// `None` when the calldata does not carry this field (e.g. ETH-in selectors
    /// where amount_in comes from `tx.value`) — R8 fail-honest.
    pub min_amount_out: Option<U256>,
    /// Whether the swap specifies exact-in or exact-out semantics.
    pub exact_mode: SwapExactMode,
    /// How this intent was sourced (public mempool, private hint, etc.).
    pub source_event: DetectionSource,
    /// Position of this swap within its transaction (0-based). Populated by
    /// the Universal Router multi-swap path (0, 1, 2, ...) so same-pair swaps
    /// inside one tx keep distinct emit identities (dedup fingerprints);
    /// single-swap decode paths leave the default 0.
    #[serde(default)]
    pub intra_tx_index: u8,
}

impl RouteIntent {
    /// Constructs a `RouteIntent`, enforcing the `legs.len() >= 1` invariant.
    ///
    /// Returns `None` when `legs` is empty — callers should treat this as a
    /// decoder failure rather than a zero-leg intent.
    ///
    /// The 10-argument count is justified by the struct's flat data model
    /// (spec §3.2): each field is mandatory and there is no natural grouping
    /// that would reduce coupling without complicating the decoder call site.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        chain_id: u64,
        tx_hash: H256,
        router: Address,
        router_kind: RouterKind,
        sender: Address,
        legs: Vec<RouteIntentLeg>,
        amount_in: U256,
        min_amount_out: Option<U256>,
        exact_mode: SwapExactMode,
        source_event: DetectionSource,
    ) -> Option<Self> {
        if legs.is_empty() {
            return None;
        }
        Some(Self {
            chain_id,
            tx_hash,
            router,
            router_kind,
            sender,
            legs,
            amount_in,
            min_amount_out,
            exact_mode,
            source_event,
            intra_tx_index: 0,
        })
    }
}

// ---------------------------------------------------------------------------
// Leg
// ---------------------------------------------------------------------------

/// A single swap hop in a route — one pool interaction.
///
/// All optional fields follow R8: they stay `None` when the decoder cannot
/// extract the value from calldata. No sentinel substitution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteIntentLeg {
    /// Input token for this hop.
    pub token_in: Address,
    /// Output token for this hop.
    pub token_out: Address,
    /// Pool address, when deterministically extractable from calldata.
    /// `None` when not available — R8 fail-honest, never a zero address.
    pub pool_hint: Option<Address>,
    /// Human-readable DEX name hint (e.g. `"uniswap-v2"`, `"sushi"`).
    /// `None` when not available — R8 fail-honest, never an empty string.
    pub dex_hint: Option<String>,
    /// Pool fee in basis points (e.g. 30 for 0.3% Uniswap V2, 500/3000/10000 for V3).
    /// `None` when the fee is not embedded in the calldata — R8 fail-honest.
    pub fee_bps: Option<u32>,
    /// Protocol family of this leg's pool.
    /// Defaults to `Unknown` when the router is unrecognised (R8 invariant).
    pub protocol_type: ProtocolType,
}

// ---------------------------------------------------------------------------
// Supporting enums
// ---------------------------------------------------------------------------

/// Exact-in vs exact-out swap semantics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SwapExactMode {
    /// The input amount is fixed; output varies (e.g. `swapExactTokensForTokens`).
    ExactIn,
    /// The output amount is fixed; input is bounded above (e.g. `swapTokensForExactTokens`).
    ExactOut,
    /// Selector not mapped to a known exact-mode — R8 fail-honest.
    Unknown,
}

/// Event source that triggered the detection of this intent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DetectionSource {
    /// Public pending transaction pool (eth_subscribe("newPendingTransactions")).
    PublicMempool,
    /// Flashbots / blocknative private tx hints (Sprint 2+ — scaffolded here
    /// for `DetectionSource` completeness; listener not yet implemented).
    FilteredMempool,
    /// MEV-Share private hint stream (Sprint 2+ — scaffolded).
    PrivateHint,
    /// New block event — triggered by `eth_subscribe("newHeads")`.
    NewBlock,
    /// Chainlink / Pyth oracle update log.
    OracleUpdate,
    /// Aave V3 / Compound V2 borrow, repay, or liquidation event.
    LendingPositionUpdate,
}

impl DetectionSource {
    /// Returns a stable, lowercase string suitable for log events and
    /// Prometheus labels. Mirrors the `detection_source_as_str` helper in
    /// `orchestrator.rs` (kept in sync — same values).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::PublicMempool => "public_mempool",
            Self::FilteredMempool => "filtered_mempool",
            Self::PrivateHint => "private_hint",
            Self::NewBlock => "new_block",
            Self::OracleUpdate => "oracle_update",
            Self::LendingPositionUpdate => "lending_position_update",
        }
    }
}

/// DEX protocol family for a swap leg.
///
/// Determines which price model applies when projecting post-swap reserves.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProtocolType {
    /// Uniswap V2 / SushiSwap constant-product AMM (x*y=k).
    V2,
    /// Uniswap V3 concentrated liquidity.
    V3,
    /// Curve Finance stableswap / cryptoswap invariants.
    Curve,
    /// Balancer weighted pools.
    Balancer,
    /// Router unrecognised — R8 invariant default.
    Unknown,
}

impl Default for ProtocolType {
    /// R8 invariant: `Unknown` is the safe default for unrecognised routers.
    fn default() -> Self {
        ProtocolType::Unknown
    }
}

/// Router kind classification — local to searcher-rs.
///
/// Distinct from `shared_rs::chains::RouterKind`: this enum carries `OneInch`
/// and is used in `RouteIntent` metadata; the shared enum is used in the router
/// catalog for address-lookup purposes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RouterKind {
    UniswapV2,
    UniswapV3,
    Sushi,
    Curve,
    Balancer,
    /// Uniswap Universal Router (multi-command dispatcher; one tx can carry
    /// several swaps — see `calldata/universal_router.rs`).
    UniversalRouter,
    /// 1inch aggregator router.
    OneInch,
    /// Router address not in the static catalog.
    Unknown,
}

impl From<shared_rs::chains::RouterKind> for RouterKind {
    /// Converts the shared router catalog kind to the local intent kind.
    /// `shared_rs::chains::RouterKind` has no `OneInch` variant — that maps to
    /// `Unknown` until the catalog is extended.
    fn from(k: shared_rs::chains::RouterKind) -> Self {
        match k {
            shared_rs::chains::RouterKind::UniswapV2 => RouterKind::UniswapV2,
            shared_rs::chains::RouterKind::UniswapV3 => RouterKind::UniswapV3,
            shared_rs::chains::RouterKind::Sushi => RouterKind::Sushi,
            shared_rs::chains::RouterKind::Curve => RouterKind::Curve,
            shared_rs::chains::RouterKind::Balancer => RouterKind::Balancer,
            shared_rs::chains::RouterKind::UniversalRouter => RouterKind::UniversalRouter,
            shared_rs::chains::RouterKind::Unknown => RouterKind::Unknown,
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use ethers::types::{Address, H256, U256};

    fn dummy_leg(token_in: Address, token_out: Address) -> RouteIntentLeg {
        RouteIntentLeg {
            token_in,
            token_out,
            pool_hint: None,
            dex_hint: None,
            fee_bps: None,
            protocol_type: ProtocolType::Unknown,
        }
    }

    fn minimal_intent(legs: Vec<RouteIntentLeg>) -> Option<RouteIntent> {
        RouteIntent::new(
            1,
            H256::zero(),
            Address::zero(),
            RouterKind::Unknown,
            Address::zero(),
            legs,
            U256::from(1_000u64),
            None,
            SwapExactMode::ExactIn,
            DetectionSource::PublicMempool,
        )
    }

    // ── spec §8: route_intent::tests::single_leg_minimal ─────────────────────

    #[test]
    fn single_leg_minimal() {
        let leg = dummy_leg(
            Address::from_low_u64_be(0xAAAA),
            Address::from_low_u64_be(0xBBBB),
        );
        let intent = minimal_intent(vec![leg]).expect("single leg must succeed");
        assert_eq!(intent.legs.len(), 1);
        assert_eq!(intent.chain_id, 1);
        assert_eq!(intent.amount_in, U256::from(1_000u64));
        assert_eq!(intent.exact_mode, SwapExactMode::ExactIn);
        assert_eq!(intent.source_event, DetectionSource::PublicMempool);
    }

    // ── spec §8: route_intent::tests::unknowns_stay_unknown (R8 invariant) ───

    #[test]
    fn unknowns_stay_unknown() {
        // When the decoder cannot extract pool/dex/fee, all optional fields are None.
        let leg = dummy_leg(Address::from_low_u64_be(0x1), Address::from_low_u64_be(0x2));
        assert!(
            leg.pool_hint.is_none(),
            "pool_hint must be None when unknown"
        );
        assert!(leg.dex_hint.is_none(), "dex_hint must be None when unknown");
        assert!(leg.fee_bps.is_none(), "fee_bps must be None when unknown");
        assert_eq!(
            leg.protocol_type,
            ProtocolType::Unknown,
            "protocol_type must default to Unknown"
        );

        // min_amount_out also stays None when not computable.
        let intent = minimal_intent(vec![leg]).expect("valid");
        assert!(
            intent.min_amount_out.is_none(),
            "min_amount_out must be None when not provided"
        );
    }

    // ── R8: zero legs produces None (not panic, not Some with 0 legs) ────────

    #[test]
    fn zero_legs_returns_none() {
        let intent = minimal_intent(vec![]);
        assert!(
            intent.is_none(),
            "RouteIntent::new with empty legs must return None, not panic"
        );
    }

    // ── R8: legs.len() >= 1 invariant holds for multi-leg ────────────────────

    #[test]
    fn multi_leg_intent_valid() {
        let legs = vec![
            dummy_leg(Address::from_low_u64_be(0xA), Address::from_low_u64_be(0xB)),
            dummy_leg(Address::from_low_u64_be(0xB), Address::from_low_u64_be(0xC)),
            dummy_leg(Address::from_low_u64_be(0xC), Address::from_low_u64_be(0xA)),
        ];
        let intent = minimal_intent(legs).expect("3-leg triangular intent must succeed");
        assert_eq!(intent.legs.len(), 3);
    }

    // ── protocol_type default == Unknown ─────────────────────────────────────

    #[test]
    fn protocol_type_default_is_unknown() {
        assert_eq!(ProtocolType::default(), ProtocolType::Unknown);
    }

    // ── RouterKind conversion from shared catalog kind ────────────────────────

    #[test]
    fn router_kind_from_shared_roundtrip() {
        use shared_rs::chains::RouterKind as Shared;
        let cases: &[(Shared, RouterKind)] = &[
            (Shared::UniswapV2, RouterKind::UniswapV2),
            (Shared::UniswapV3, RouterKind::UniswapV3),
            (Shared::Sushi, RouterKind::Sushi),
            (Shared::Curve, RouterKind::Curve),
            (Shared::Balancer, RouterKind::Balancer),
            (Shared::UniversalRouter, RouterKind::UniversalRouter),
            (Shared::Unknown, RouterKind::Unknown),
        ];
        for &(shared, expected) in cases {
            let got = RouterKind::from(shared);
            assert_eq!(
                got, expected,
                "RouterKind::from({shared:?}) must be {expected:?}"
            );
        }
    }

    // ── serde round-trips for all enums ──────────────────────────────────────

    #[test]
    fn swap_exact_mode_serde_roundtrip() {
        for mode in [
            SwapExactMode::ExactIn,
            SwapExactMode::ExactOut,
            SwapExactMode::Unknown,
        ] {
            let json = serde_json::to_string(&mode).expect("serialize");
            let back: SwapExactMode = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(back, mode);
        }
    }

    #[test]
    fn detection_source_serde_roundtrip() {
        for src in [
            DetectionSource::PublicMempool,
            DetectionSource::FilteredMempool,
            DetectionSource::PrivateHint,
            DetectionSource::NewBlock,
            DetectionSource::OracleUpdate,
            DetectionSource::LendingPositionUpdate,
        ] {
            let json = serde_json::to_string(&src).expect("serialize");
            let back: DetectionSource = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(back, src);
        }
    }

    #[test]
    fn protocol_type_serde_roundtrip() {
        for pt in [
            ProtocolType::V2,
            ProtocolType::V3,
            ProtocolType::Curve,
            ProtocolType::Balancer,
            ProtocolType::Unknown,
        ] {
            let json = serde_json::to_string(&pt).expect("serialize");
            let back: ProtocolType = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(back, pt);
        }
    }

    #[test]
    fn router_kind_serde_roundtrip() {
        for rk in [
            RouterKind::UniswapV2,
            RouterKind::UniswapV3,
            RouterKind::Sushi,
            RouterKind::Curve,
            RouterKind::Balancer,
            RouterKind::UniversalRouter,
            RouterKind::OneInch,
            RouterKind::Unknown,
        ] {
            let json = serde_json::to_string(&rk).expect("serialize");
            let back: RouterKind = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(back, rk);
        }
    }

    // ── full struct serde round-trip ──────────────────────────────────────────

    #[test]
    fn route_intent_serde_roundtrip() {
        let intent = RouteIntent::new(
            1,
            H256::from_low_u64_be(0xDEAD),
            Address::from_low_u64_be(0xCAFE),
            RouterKind::UniswapV2,
            Address::from_low_u64_be(0xBEEF),
            vec![RouteIntentLeg {
                token_in: Address::from_low_u64_be(0xA),
                token_out: Address::from_low_u64_be(0xB),
                pool_hint: None,
                dex_hint: Some("uniswap-v2".to_string()),
                fee_bps: Some(30),
                protocol_type: ProtocolType::V2,
            }],
            U256::from(1_000_000u64),
            Some(U256::from(900_000u64)),
            SwapExactMode::ExactIn,
            DetectionSource::PublicMempool,
        )
        .expect("valid intent");

        let json = serde_json::to_string(&intent).expect("serialize");
        let back: RouteIntent = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(back.chain_id, intent.chain_id);
        assert_eq!(back.tx_hash, intent.tx_hash);
        assert_eq!(back.legs.len(), 1);
        assert_eq!(back.legs[0].fee_bps, Some(30));
        assert_eq!(back.min_amount_out, Some(U256::from(900_000u64)));
    }
}
