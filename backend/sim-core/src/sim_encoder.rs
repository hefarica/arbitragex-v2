//! Phase A.3.a — OpportunityCandidate → RoundTripContext encoder.
//!
//! Honest bridge between the abstract `OpportunityCandidate` produced by the
//! orchestrator and the typed `RoundTripContext` consumed by
//! `simulator_v2::SimulatorV2`. This module does NOT execute REVM, NOT pre-fund
//! ERC-20 balances, NOT submit anything. Its sole job is to project a
//! candidate into a deterministic, testable, fail-closed simulation input.
//!
//! ## Scope (Phase A.3.a)
//!
//! - SUPPORTED: 2-leg round-trip candidates whose `dex_adapters` resolve to V2
//!   or Sushi (V2-API-compatible). The forward and backward legs both encode
//!   via `swap_encoder::encode_v2_swap_exact_tokens_for_tokens`.
//! - UNSUPPORTED (deferred to A.3.b / A.3.c):
//!   - V3 (needs per-leg fee tier — not carried in `OpportunityCandidate`)
//!   - Curve / Balancer (no encoder in `prioritization_spine::swap_encoder` yet)
//!   - Triangular 3-leg shape (incompatible with 2-leg `RoundTripContext`)
//!   - 1-leg flashloan / liquidation shapes
//!
//! Every unsupported case rejects with a typed `SimEncoderError`. There are NO
//! silent fallbacks, NO synthesised tokens, NO hardcoded routers, NO default
//! decimals. RULE 00 + RULE 12 honoured throughout.
//!
//! ## Math safety
//!
//! `OpportunityCandidate.amount_in: f64` is converted to `U256` wei via
//! `BigDecimal` arbitrary precision arithmetic with explicit `RoundingMode::
//! Floor`. This avoids the precision hazards of `(amount * 10^d) as u128` and
//! the silent half-even bias of `BigDecimal::with_scale(0)`. See
//! `convert_amount_to_wei` for full validation chain (NaN, ±Inf, ≤0, sub-wei
//! underflow, U256 overflow, decimals > 36 all return typed errors).
//!
//! ## Router resolution
//!
//! `dex_adapters: Vec<String>` carries semantic labels ("uniswap-v2", "sushi",
//! …) produced by the engines. The encoder resolves each label to a concrete
//! router address via the canonical catalogue in
//! `shared_rs::chains::routers_for_chain`. No router address is ever
//! constructed by string literal in this module.
//!
//! ## Executor address
//!
//! Read from `EXECUTOR_<chain_id>` environment variable at the start of every
//! encoding call. NO hardcode of per-chain executor addresses. Missing,
//! invalid, or zero values all reject fail-closed.

use bigdecimal::num_traits::FromPrimitive;
use bigdecimal::{BigDecimal, RoundingMode};
use ethers::types::{Address, U256};
use prioritization_spine::round_trip_executor::RoundTripContext;
use prioritization_spine::types::OpportunityCandidate;
use shared_rs::chains::{routers_for_chain, ExecutorAddrError, RouterKind};
// `HashMap` is only used by the test-only `InMemoryTokenDecimalsProvider`.
#[cfg(test)]
use std::collections::HashMap;
use std::str::FromStr;
use thiserror::Error;
use tracing::debug;

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Typed encoder error. Every rejection path produces one of these variants;
/// `Display` carries operator-readable context. Callers map the variant to a
/// counter increment + a `SIM_REJECTED:<reason>` simulation_status string.
#[derive(Debug, Error, PartialEq)]
pub enum SimEncoderError {
    #[error("EXECUTOR_{chain_id} env var not set")]
    MissingExecutorAddress { chain_id: u64 },

    #[error("EXECUTOR_{chain_id} env var value cannot parse as address: {value}")]
    InvalidExecutorAddress { chain_id: u64, value: String },

    #[error("EXECUTOR_{chain_id} env var resolves to the zero address")]
    ZeroExecutorAddress { chain_id: u64 },

    #[error("candidate has empty token_addresses (no token_in)")]
    MissingTokenIn,

    #[error("candidate token_addresses has fewer than 2 entries (no token_out)")]
    MissingTokenOut,

    #[error("token address {token:?} cannot be parsed as a 20-byte EVM address")]
    InvalidTokenAddress { token: String },

    #[error("token {token:?} is the zero address")]
    ZeroTokenAddress { token: String },

    #[error("token_in equals token_out ({token:?}); round trip requires distinct legs")]
    SameTokenInOut { token: Address },

    #[error("decimals not configured for chain {chain_id} token {token:?}")]
    MissingTokenDecimals { chain_id: u64, token: Address },

    #[error(
        "decimals {decimals} for chain {chain_id} token {token:?} exceed the safe range (>36)"
    )]
    InvalidTokenDecimals {
        chain_id: u64,
        token: Address,
        decimals: u8,
    },

    #[error("amount_in is NaN")]
    AmountInNaN,

    #[error("amount_in is infinite")]
    AmountInInfinite,

    #[error("amount_in is non-positive ({value})")]
    AmountInNonPositive { value: f64 },

    #[error("amount_in conversion overflows U256")]
    AmountConversionOverflow,

    #[error("amount_in {amount_in} with {decimals} decimals rounds to 0 wei (sub-wei input)")]
    AmountTooSmallForDecimals { amount_in: f64, decimals: u8 },

    #[error("candidate has no dex_adapters legs")]
    MissingRouteLegs,

    #[error("unsupported route shape: {reason}")]
    UnsupportedRouteShape { reason: String },

    #[error("dex_kind {dex_kind:?} unsupported by Phase A.3.a encoder")]
    UnsupportedDexKind { dex_kind: String },

    #[error("no router registered for chain {chain_id} dex_kind {dex_kind:?}")]
    MissingRouterAddress { chain_id: u64, dex_kind: String },

    #[error("RouteEncodingConfig.deadline_seconds is zero (no source for tx deadline)")]
    MissingDeadlineConfig,

    #[error("RouteEncodingConfig.min_profit_wei is zero (no source for slippage floor)")]
    MissingMinProfitInput,
}

impl SimEncoderError {
    /// Short stable tag used as the `simulation_status` reason. Used by the
    /// scanner to emit consistent counters + structured logs.
    pub fn reason_tag(&self) -> &'static str {
        match self {
            Self::MissingExecutorAddress { .. } => "missing_executor",
            Self::InvalidExecutorAddress { .. } => "invalid_executor",
            Self::ZeroExecutorAddress { .. } => "zero_executor",
            Self::MissingTokenIn => "missing_token_in",
            Self::MissingTokenOut => "missing_token_out",
            Self::InvalidTokenAddress { .. } => "invalid_token_address",
            Self::ZeroTokenAddress { .. } => "zero_token_address",
            Self::SameTokenInOut { .. } => "same_token_in_out",
            Self::MissingTokenDecimals { .. } => "missing_decimals",
            Self::InvalidTokenDecimals { .. } => "invalid_decimals",
            Self::AmountInNaN => "amount_nan",
            Self::AmountInInfinite => "amount_infinite",
            Self::AmountInNonPositive { .. } => "amount_non_positive",
            Self::AmountConversionOverflow => "amount_overflow",
            Self::AmountTooSmallForDecimals { .. } => "amount_too_small",
            Self::MissingRouteLegs => "missing_route_legs",
            Self::UnsupportedRouteShape { .. } => "unsupported_route_shape",
            Self::UnsupportedDexKind { .. } => "unsupported_dex_kind",
            Self::MissingRouterAddress { .. } => "missing_router",
            Self::MissingDeadlineConfig => "missing_deadline_config",
            Self::MissingMinProfitInput => "missing_min_profit",
        }
    }
}

// ---------------------------------------------------------------------------
// Providers
// ---------------------------------------------------------------------------

/// Read-only provider of ERC-20 `decimals` keyed by `(chain_id, address)`.
///
/// Production implementations connect to the PG `tokens` table (populated by
/// `token-enricher` from on-chain `decimals()` calls). Tests use the
/// `InMemoryTokenDecimalsProvider` below.
///
/// Returning `None` for an unknown token is the only honest signal: the
/// encoder then rejects with `MissingTokenDecimals`. NEVER return a default
/// such as `Some(18)` from a production impl.
pub trait TokenDecimalsProvider: Send + Sync {
    fn decimals(&self, chain_id: u64, token: &Address) -> Option<u8>;
}

/// In-memory provider for unit tests. Constructed empty by default; tests
/// populate it explicitly via `with_decimals`. Production code MUST NOT use
/// this — it would force every operator to manually pre-load every token.
///
/// Gated under `#[cfg(test)]` so the type is invisible to the binary build
/// (RULE 1: no test fixtures leak into runtime). Phase A.3.b production wire
/// uses `sim_encoder_pg::PgTokenDecimalsProvider`.
#[cfg(test)]
#[derive(Debug, Default, Clone)]
pub struct InMemoryTokenDecimalsProvider {
    table: HashMap<(u64, Address), u8>,
}

#[cfg(test)]
impl InMemoryTokenDecimalsProvider {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_decimals(mut self, chain_id: u64, token: Address, decimals: u8) -> Self {
        self.table.insert((chain_id, token), decimals);
        self
    }
}

#[cfg(test)]
impl TokenDecimalsProvider for InMemoryTokenDecimalsProvider {
    fn decimals(&self, chain_id: u64, token: &Address) -> Option<u8> {
        self.table.get(&(chain_id, *token)).copied()
    }
}

// ---------------------------------------------------------------------------
// RouteEncodingConfig
// ---------------------------------------------------------------------------

/// Per-call encoder configuration. Supplied by the caller (scanner) so the
/// encoder stays pure: same inputs → same `RoundTripContext`. Two values are
/// MANDATORY (deadline + min profit) and validated up front.
#[derive(Debug, Clone)]
pub struct RouteEncodingConfig {
    /// Seconds added to `now_unix_ts` to build `RoundTripContext.deadline`.
    /// MUST be > 0; a zero value rejects with `MissingDeadlineConfig`.
    pub deadline_seconds: u64,
    /// Current UNIX timestamp; the encoder does NOT call `SystemTime::now()`
    /// internally — that would make it non-pure and untestable.
    pub now_unix_ts: u64,
    /// Minimum profit (in `token_in` wei) the simulation gate enforces. Zero
    /// rejects with `MissingMinProfitInput`. Honest sourcing is the caller's
    /// responsibility (e.g., spine `net_expected_profit` × confidence).
    pub min_profit_wei: U256,
}

impl RouteEncodingConfig {
    pub fn validate(&self) -> Result<(), SimEncoderError> {
        if self.deadline_seconds == 0 {
            return Err(SimEncoderError::MissingDeadlineConfig);
        }
        if self.min_profit_wei.is_zero() {
            return Err(SimEncoderError::MissingMinProfitInput);
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Look up `EXECUTOR_<chain_id>` env var and parse to a non-zero `Address`.
/// NO hardcoded fallback addresses; NO test/dummy defaults. Every chain that
/// participates in the simulator wire must export this env var explicitly.
///
/// The resolution logic lives in `shared_rs::chains::resolve_executor_address`
/// so that `relays-client` and other crates resolve the SAME executor address
/// from the SAME source. This is a thin wrapper that maps the shared
/// `ExecutorAddrError` back to the local `SimEncoderError` variants so the
/// scanner sees byte-identical behaviour (same variants, same `reason_tag()`).
pub fn parse_executor_address(chain_id: u64) -> Result<Address, SimEncoderError> {
    shared_rs::chains::resolve_executor_address(chain_id).map_err(|e| match e {
        ExecutorAddrError::Missing { chain_id } => {
            SimEncoderError::MissingExecutorAddress { chain_id }
        }
        ExecutorAddrError::Invalid { chain_id, value } => {
            SimEncoderError::InvalidExecutorAddress { chain_id, value }
        }
        ExecutorAddrError::Zero { chain_id } => SimEncoderError::ZeroExecutorAddress { chain_id },
    })
}

/// Parse a 20-byte EVM address from a string. Validates non-zero.
fn parse_token_address(s: &str) -> Result<Address, SimEncoderError> {
    let addr = Address::from_str(s.trim()).map_err(|_| SimEncoderError::InvalidTokenAddress {
        token: s.to_string(),
    })?;
    if addr == Address::zero() {
        return Err(SimEncoderError::ZeroTokenAddress {
            token: s.to_string(),
        });
    }
    Ok(addr)
}

/// Convert `amount_in: f64 + decimals` to `U256` wei. See math-validator
/// report 2026-05-12 (agentId a18def8c…) for the full proof of correctness.
///
/// Pipeline:
///   1. Reject NaN / Inf / ≤0 at the f64 boundary.
///   2. Reject decimals > 36 (no real ERC-20 exceeds that).
///   3. `BigDecimal::from_f64(amount_in)` — arbitrary precision representation.
///   4. Multiply by `10^decimals` via `BigDecimal` (no f64 precision loss
///      past 2^53 because the multiplication happens in num-bigint).
///   5. `with_scale_round(0, RoundingMode::Floor)` — strict truncation
///      (NOT half-even, which would deviate ~0.5 ULP from the floor value).
///   6. `to_string` → `U256::from_dec_str` for overflow detection.
///   7. Reject `U256::zero()` output (sub-wei input would silently round to
///      zero; an on-chain swap with amount=0 reverts — better to reject up
///      front with a precise error).
pub fn convert_amount_to_wei(amount_in: f64, decimals: u8) -> Result<U256, SimEncoderError> {
    if amount_in.is_nan() {
        return Err(SimEncoderError::AmountInNaN);
    }
    if amount_in.is_infinite() {
        return Err(SimEncoderError::AmountInInfinite);
    }
    if amount_in <= 0.0 {
        return Err(SimEncoderError::AmountInNonPositive { value: amount_in });
    }
    if decimals > 36 {
        return Err(SimEncoderError::InvalidTokenDecimals {
            chain_id: 0,
            token: Address::zero(),
            decimals,
        });
    }
    let amount_bd = BigDecimal::from_f64(amount_in).ok_or(SimEncoderError::AmountInNaN)?;
    let mut scale = BigDecimal::from(1u64);
    for _ in 0..decimals {
        scale *= BigDecimal::from(10u64);
    }
    // Floor rounding is explicit (math-validator finding 2026-05-12). The
    // default `with_scale(0)` uses HalfEven, which silently rounds 5-wei
    // boundaries inconsistently with the documented "truncate" contract.
    let wei_decimal = (amount_bd * scale).with_scale_round(0, RoundingMode::Floor);
    let wei_string = wei_decimal.to_string();
    // After Floor on a positive bd, neither '-' nor '.' can appear at scale=0.
    // These asserts catch bigdecimal API drift in future upgrades.
    debug_assert!(
        !wei_string.contains('-'),
        "unexpected negative wei string: {wei_string}"
    );
    debug_assert!(
        !wei_string.contains('.'),
        "unexpected fractional wei string: {wei_string}"
    );
    let wei =
        U256::from_dec_str(&wei_string).map_err(|_| SimEncoderError::AmountConversionOverflow)?;
    if wei.is_zero() {
        return Err(SimEncoderError::AmountTooSmallForDecimals {
            amount_in,
            decimals,
        });
    }
    Ok(wei)
}

/// Map a `dex_adapters` semantic label to `shared_rs::chains::RouterKind`.
///
/// Phase A.3.a supports V2-style routers only (UniswapV2 + Sushi). V3 requires
/// per-leg fee tier (not carried in `OpportunityCandidate`); Curve / Balancer
/// have no encoder in `prioritization_spine::swap_encoder` yet. Every other
/// label rejects with `UnsupportedDexKind`.
///
/// The match is spelling-proof: producers emit the same DEX under different
/// spellings — scanner fixtures use kebab ("uniswap-v2", "sushi") while
/// cartridge `dex_adapters` carry PascalCase ("UniswapV2", "SushiSwap").
/// Labels normalize to lowercase alphanumerics (same approach as
/// `tx_builder::find_router_by_name`), so casing/spacing never rejects an
/// encodable route. The error keeps the ORIGINAL label for observability.
pub fn parse_dex_kind(label: &str) -> Result<RouterKind, SimEncoderError> {
    let normalized: String = label
        .chars()
        .filter(|c| !matches!(c, '-' | '_' | ' '))
        .flat_map(|c| c.to_lowercase())
        .collect();
    match normalized.as_str() {
        "uniswapv2" => Ok(RouterKind::UniswapV2),
        "sushi" | "sushiswap" => Ok(RouterKind::Sushi),
        _ => Err(SimEncoderError::UnsupportedDexKind {
            dex_kind: label.to_string(),
        }),
    }
}

/// Resolve a router address from the static catalogue. Returns
/// `MissingRouterAddress` when the catalogue has no entry for
/// `(chain_id, kind)`. NO hardcode fallback.
pub fn resolve_router_address(chain_id: u64, kind: RouterKind) -> Result<Address, SimEncoderError> {
    for entry in routers_for_chain(chain_id) {
        if entry.kind == kind {
            return Ok(Address::from(entry.address));
        }
    }
    Err(SimEncoderError::MissingRouterAddress {
        chain_id,
        dex_kind: kind.as_str().to_string(),
    })
}

// ---------------------------------------------------------------------------
// Main entry point
// ---------------------------------------------------------------------------

/// Build a `RoundTripContext` from a 2-leg V2-class candidate.
///
/// Returns `Err(SimEncoderError::…)` on any incomplete or unsupported input.
/// On success, the returned context is a fully-typed input to
/// `simulator_v2::SimulatorV2::simulate` (which lands in Phase A.3.b).
///
/// ## Validation order
///
/// 1. `RouteEncodingConfig` invariants (deadline + min_profit).
/// 2. Candidate has exactly 2 `dex_adapters` legs (round-trip shape).
/// 3. `token_addresses` has at least 2 entries (token_in + token_out).
/// 4. Tokens parse, are non-zero, and distinct.
/// 5. Decimals available for token_in (used to scale `amount_in`).
/// 6. `amount_in` converts safely to U256 wei.
/// 7. Both dex_kinds supported (V2 / Sushi).
/// 8. Both routers resolve in the static catalogue.
///
/// Each step short-circuits with a typed error. The function is pure: same
/// inputs always produce the same output (or the same error).
pub fn build_round_trip_context_from_candidate(
    candidate: &OpportunityCandidate,
    chain_id: u64,
    executor: Address,
    decimals_provider: &dyn TokenDecimalsProvider,
    config: &RouteEncodingConfig,
) -> Result<RoundTripContext, SimEncoderError> {
    config.validate()?;

    // ── Route shape ────────────────────────────────────────────────────────
    if candidate.dex_adapters.is_empty() {
        return Err(SimEncoderError::MissingRouteLegs);
    }
    if candidate.dex_adapters.len() != 2 {
        return Err(SimEncoderError::UnsupportedRouteShape {
            reason: format!(
                "A.3.a supports 2-leg round trip only; candidate has {} legs",
                candidate.dex_adapters.len()
            ),
        });
    }

    // ── Tokens ─────────────────────────────────────────────────────────────
    if candidate.token_addresses.is_empty() {
        return Err(SimEncoderError::MissingTokenIn);
    }
    if candidate.token_addresses.len() < 2 {
        return Err(SimEncoderError::MissingTokenOut);
    }
    let token_in = parse_token_address(&candidate.token_addresses[0])?;
    let token_out = parse_token_address(&candidate.token_addresses[1])?;
    if token_in == token_out {
        return Err(SimEncoderError::SameTokenInOut { token: token_in });
    }

    // ── Decimals ───────────────────────────────────────────────────────────
    let decimals_in = decimals_provider.decimals(chain_id, &token_in).ok_or(
        SimEncoderError::MissingTokenDecimals {
            chain_id,
            token: token_in,
        },
    )?;
    if decimals_in > 36 {
        return Err(SimEncoderError::InvalidTokenDecimals {
            chain_id,
            token: token_in,
            decimals: decimals_in,
        });
    }

    // ── Amount ─────────────────────────────────────────────────────────────
    let amount_in_wei = convert_amount_to_wei(candidate.amount_in, decimals_in)?;

    // ── Routers ────────────────────────────────────────────────────────────
    let forward_kind = parse_dex_kind(&candidate.dex_adapters[0])?;
    let backward_kind = parse_dex_kind(&candidate.dex_adapters[1])?;
    let forward_router = resolve_router_address(chain_id, forward_kind)?;
    let backward_router = resolve_router_address(chain_id, backward_kind)?;

    // ── Paths ──────────────────────────────────────────────────────────────
    let forward_path = vec![token_in, token_out];
    let backward_path = vec![token_out, token_in];

    // ── Deadline ───────────────────────────────────────────────────────────
    let deadline = U256::from(config.now_unix_ts.saturating_add(config.deadline_seconds));

    debug!(
        event = "sim_encoder.success",
        chain_id,
        route_fingerprint = %candidate.route_fingerprint,
        token_in = ?token_in,
        token_out = ?token_out,
        amount_in_raw = candidate.amount_in,
        amount_in_wei = %amount_in_wei,
        decimals = decimals_in,
        forward_router = ?forward_router,
        backward_router = ?backward_router,
        legs = candidate.dex_adapters.len(),
        "RoundTripContext built"
    );

    Ok(RoundTripContext {
        caller: executor,
        token_in,
        token_out,
        amount_in: amount_in_wei,
        forward_router,
        forward_path,
        backward_router,
        backward_path,
        deadline,
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    // Real mainnet token addresses. NOT used as defaults anywhere in the
    // encoder runtime; they only exist here so unit tests can build realistic
    // candidates without needing live PG. The encoder still consults the
    // `decimals_provider` for the value — tests pre-populate it explicitly.
    const WETH: &str = "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2";
    const USDC: &str = "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48";

    fn addr(s: &str) -> Address {
        Address::from_str(s).unwrap()
    }

    fn valid_candidate_v2() -> OpportunityCandidate {
        OpportunityCandidate {
            route_fingerprint: "test_v2_round_trip".into(),
            pool_addresses: vec![
                "0x0d4a11d5eeaac28ec3f61d100daf4d40471f1852".into(),
                "0xb4e16d0168e52d35cacd2c6185b44281ec28c9dc".into(),
            ],
            token_addresses: vec![WETH.into(), USDC.into()],
            dex_adapters: vec!["uniswap-v2".into(), "sushi".into()],
            amount_in: 1.5,
            expected_amount_out: 1.51,
            gross_profit: 0.01,
        }
    }

    fn provider_with_weth_18() -> InMemoryTokenDecimalsProvider {
        InMemoryTokenDecimalsProvider::new()
            .with_decimals(1, addr(WETH), 18)
            .with_decimals(1, addr(USDC), 6)
    }

    fn valid_config() -> RouteEncodingConfig {
        RouteEncodingConfig {
            deadline_seconds: 60,
            now_unix_ts: 1_700_000_000,
            min_profit_wei: U256::from(1_000u64),
        }
    }

    fn dummy_executor() -> Address {
        // Test-only sentinel. NOT used as a fallback anywhere in non-test code.
        addr("0x1234567890123456789012345678901234567890")
    }

    // ── Test 1 — valid 2-leg V2 round trip builds context ───────────────────

    #[test]
    fn valid_candidate_v2_builds_round_trip_context() {
        let c = valid_candidate_v2();
        let p = provider_with_weth_18();
        let ctx =
            build_round_trip_context_from_candidate(&c, 1, dummy_executor(), &p, &valid_config())
                .unwrap();
        assert_eq!(ctx.token_in, addr(WETH));
        assert_eq!(ctx.token_out, addr(USDC));
        // 1.5 WETH (18 decimals) → 1.5 × 10^18 = 1500000000000000000
        assert_eq!(
            ctx.amount_in,
            U256::from_dec_str("1500000000000000000").unwrap()
        );
        assert_eq!(ctx.forward_path, vec![addr(WETH), addr(USDC)]);
        assert_eq!(ctx.backward_path, vec![addr(USDC), addr(WETH)]);
        // deadline = 1_700_000_000 + 60
        assert_eq!(ctx.deadline, U256::from(1_700_000_060u64));
    }

    // ── Test 2 — V3 candidate rejected (UnsupportedDexKind in A.3.a) ────────

    #[test]
    fn valid_candidate_v3_rejected_unsupported_in_a3a() {
        let mut c = valid_candidate_v2();
        c.dex_adapters = vec!["uniswap-v3".into(), "uniswap-v3".into()];
        let err = build_round_trip_context_from_candidate(
            &c,
            1,
            dummy_executor(),
            &provider_with_weth_18(),
            &valid_config(),
        )
        .unwrap_err();
        assert!(matches!(err, SimEncoderError::UnsupportedDexKind { .. }));
        assert_eq!(err.reason_tag(), "unsupported_dex_kind");
    }

    // ── Test 2b — parse_dex_kind is spelling-proof across producers ─────────
    //
    // SIMENCODER-LABEL-NORM-01: cartridge dex_adapters arrive PascalCase
    // ("UniswapV2", "SushiSwap") while the encoder only matched kebab, so
    // every encodable 2-leg candidate died as b2c_encode_failed even after
    // the B2C consumer (#484) routed it here. Same DEX, different spelling.

    #[test]
    fn parse_dex_kind_accepts_pascalcase_and_spelling_variants() {
        assert_eq!(parse_dex_kind("uniswap-v2"), Ok(RouterKind::UniswapV2));
        assert_eq!(parse_dex_kind("uniswapv2"), Ok(RouterKind::UniswapV2));
        assert_eq!(parse_dex_kind("UniswapV2"), Ok(RouterKind::UniswapV2));
        assert_eq!(parse_dex_kind("sushi"), Ok(RouterKind::Sushi));
        assert_eq!(parse_dex_kind("sushiswap"), Ok(RouterKind::Sushi));
        assert_eq!(parse_dex_kind("SushiSwap"), Ok(RouterKind::Sushi));
        assert_eq!(parse_dex_kind(" Sushi_Swap "), Ok(RouterKind::Sushi));
    }

    #[test]
    fn parse_dex_kind_fails_closed_with_original_label() {
        for unsupported in ["uniswap-v3", "UniswapV3", "PancakeSwapV3", "curve", ""] {
            let err = parse_dex_kind(unsupported).unwrap_err();
            match err {
                SimEncoderError::UnsupportedDexKind { dex_kind } => {
                    // Original label preserved for observability.
                    assert_eq!(dex_kind, unsupported);
                }
                other => panic!("expected UnsupportedDexKind, got {other:?}"),
            }
        }
    }

    // ── Test 2c — real feed shape (PascalCase adapters) builds context ──────

    #[test]
    fn pascalcase_feed_candidate_builds_round_trip_context() {
        let mut c = valid_candidate_v2();
        c.dex_adapters = vec!["UniswapV2".into(), "SushiSwap".into()];
        let ctx = build_round_trip_context_from_candidate(
            &c,
            1,
            dummy_executor(),
            &provider_with_weth_18(),
            &valid_config(),
        )
        .unwrap();
        assert_eq!(ctx.token_in, addr(WETH));
        assert_eq!(ctx.token_out, addr(USDC));
    }

    // ── Test 3 — Multihop (3 legs) rejected as unsupported shape ────────────

    #[test]
    fn multihop_candidate_rejected_as_unsupported_shape() {
        let mut c = valid_candidate_v2();
        c.dex_adapters = vec!["uniswap-v2".into(); 3];
        let err = build_round_trip_context_from_candidate(
            &c,
            1,
            dummy_executor(),
            &provider_with_weth_18(),
            &valid_config(),
        )
        .unwrap_err();
        assert!(matches!(err, SimEncoderError::UnsupportedRouteShape { .. }));
        assert_eq!(err.reason_tag(), "unsupported_route_shape");
    }

    // ── Test 4 — Missing executor env var rejected ─────────────────────────

    #[test]
    fn missing_executor_rejected() {
        std::env::remove_var("EXECUTOR_9999");
        let err = parse_executor_address(9999).unwrap_err();
        assert!(matches!(
            err,
            SimEncoderError::MissingExecutorAddress { chain_id: 9999 }
        ));
        assert_eq!(err.reason_tag(), "missing_executor");
    }

    // ── Test 5 — Invalid executor env var rejected ─────────────────────────

    #[test]
    fn invalid_executor_rejected() {
        std::env::set_var("EXECUTOR_9998", "not_an_address");
        let err = parse_executor_address(9998).unwrap_err();
        assert!(matches!(
            err,
            SimEncoderError::InvalidExecutorAddress { .. }
        ));
        std::env::remove_var("EXECUTOR_9998");
    }

    // ── Test 6 — Zero executor env var rejected ────────────────────────────

    #[test]
    fn zero_executor_rejected() {
        std::env::set_var(
            "EXECUTOR_9997",
            "0x0000000000000000000000000000000000000000",
        );
        let err = parse_executor_address(9997).unwrap_err();
        assert!(matches!(
            err,
            SimEncoderError::ZeroExecutorAddress { chain_id: 9997 }
        ));
        std::env::remove_var("EXECUTOR_9997");
    }

    // ── Test 7 — Empty token_addresses rejected ────────────────────────────

    #[test]
    fn missing_token_in_rejected() {
        let mut c = valid_candidate_v2();
        c.token_addresses.clear();
        let err = build_round_trip_context_from_candidate(
            &c,
            1,
            dummy_executor(),
            &provider_with_weth_18(),
            &valid_config(),
        )
        .unwrap_err();
        assert!(matches!(err, SimEncoderError::MissingTokenIn));
    }

    // ── Test 8 — Single-token candidate rejected as missing token_out ──────

    #[test]
    fn missing_token_out_rejected() {
        let mut c = valid_candidate_v2();
        c.token_addresses = vec![WETH.into()];
        let err = build_round_trip_context_from_candidate(
            &c,
            1,
            dummy_executor(),
            &provider_with_weth_18(),
            &valid_config(),
        )
        .unwrap_err();
        assert!(matches!(err, SimEncoderError::MissingTokenOut));
    }

    // ── Test 9 — Missing decimals rejected ─────────────────────────────────

    #[test]
    fn missing_decimals_rejected() {
        let c = valid_candidate_v2();
        let p = InMemoryTokenDecimalsProvider::new(); // empty
        let err =
            build_round_trip_context_from_candidate(&c, 1, dummy_executor(), &p, &valid_config())
                .unwrap_err();
        assert!(matches!(err, SimEncoderError::MissingTokenDecimals { .. }));
        assert_eq!(err.reason_tag(), "missing_decimals");
    }

    // ── Test 10 — decimals > 36 rejected ───────────────────────────────────

    #[test]
    fn invalid_decimals_rejected() {
        let c = valid_candidate_v2();
        let p = InMemoryTokenDecimalsProvider::new().with_decimals(1, addr(WETH), 50);
        let err =
            build_round_trip_context_from_candidate(&c, 1, dummy_executor(), &p, &valid_config())
                .unwrap_err();
        assert!(matches!(err, SimEncoderError::InvalidTokenDecimals { .. }));
    }

    // ── Test 11 — NaN amount_in rejected ───────────────────────────────────

    #[test]
    fn amount_nan_rejected() {
        let err = convert_amount_to_wei(f64::NAN, 18).unwrap_err();
        assert_eq!(err, SimEncoderError::AmountInNaN);
    }

    // ── Test 12 — Inf amount_in rejected ───────────────────────────────────

    #[test]
    fn amount_infinite_rejected() {
        let err = convert_amount_to_wei(f64::INFINITY, 18).unwrap_err();
        assert_eq!(err, SimEncoderError::AmountInInfinite);
        let err = convert_amount_to_wei(f64::NEG_INFINITY, 18).unwrap_err();
        assert_eq!(err, SimEncoderError::AmountInInfinite);
    }

    // ── Test 13 — Zero amount_in rejected ──────────────────────────────────

    #[test]
    fn amount_zero_rejected() {
        let err = convert_amount_to_wei(0.0, 18).unwrap_err();
        assert!(matches!(err, SimEncoderError::AmountInNonPositive { .. }));
    }

    // ── Test 14 — Negative amount_in rejected ──────────────────────────────

    #[test]
    fn amount_negative_rejected() {
        let err = convert_amount_to_wei(-1.0, 18).unwrap_err();
        assert!(matches!(err, SimEncoderError::AmountInNonPositive { .. }));
    }

    // ── Test 15 — Overflow amount_in rejected ──────────────────────────────

    #[test]
    fn amount_overflow_rejected() {
        // f64::MAX is ~1.8e308. Multiplied by 10^18 it exceeds U256::MAX
        // (~1.16e77). The conversion must reject with AmountConversionOverflow.
        let err = convert_amount_to_wei(f64::MAX, 18).unwrap_err();
        assert_eq!(err, SimEncoderError::AmountConversionOverflow);
    }

    // ── Test 16 — Sub-wei input rejected (would round to 0) ─────────────────

    #[test]
    fn amount_sub_wei_rejected_for_decimals() {
        // 1e-10 ETH with 18 decimals = 1e8 wei, OK.
        let ok = convert_amount_to_wei(1e-10, 18).unwrap();
        assert_eq!(ok, U256::from(100_000_000u64));
        // 1e-10 USDC with 6 decimals = 1e-4 wei → floor to 0 → reject.
        let err = convert_amount_to_wei(1e-10, 6).unwrap_err();
        assert!(matches!(
            err,
            SimEncoderError::AmountTooSmallForDecimals { .. }
        ));
    }

    // ── Test 17 — Unsupported dex_kind rejected ────────────────────────────

    #[test]
    fn unsupported_dex_rejected() {
        let mut c = valid_candidate_v2();
        c.dex_adapters = vec!["curve".into(), "balancer".into()];
        let err = build_round_trip_context_from_candidate(
            &c,
            1,
            dummy_executor(),
            &provider_with_weth_18(),
            &valid_config(),
        )
        .unwrap_err();
        assert!(matches!(err, SimEncoderError::UnsupportedDexKind { .. }));
    }

    // ── Test 18 — Empty route legs rejected ────────────────────────────────

    #[test]
    fn empty_route_rejected() {
        let mut c = valid_candidate_v2();
        c.dex_adapters.clear();
        let err = build_round_trip_context_from_candidate(
            &c,
            1,
            dummy_executor(),
            &provider_with_weth_18(),
            &valid_config(),
        )
        .unwrap_err();
        assert!(matches!(err, SimEncoderError::MissingRouteLegs));
    }

    // ── Test 19 — Missing min_profit config rejected ───────────────────────

    #[test]
    fn min_profit_missing_rejected() {
        let c = valid_candidate_v2();
        let cfg = RouteEncodingConfig {
            deadline_seconds: 60,
            now_unix_ts: 1_700_000_000,
            min_profit_wei: U256::zero(),
        };
        let err = build_round_trip_context_from_candidate(
            &c,
            1,
            dummy_executor(),
            &provider_with_weth_18(),
            &cfg,
        )
        .unwrap_err();
        assert!(matches!(err, SimEncoderError::MissingMinProfitInput));
    }

    // ── Test 20 — Missing deadline config rejected ─────────────────────────

    #[test]
    fn deadline_missing_rejected() {
        let c = valid_candidate_v2();
        let cfg = RouteEncodingConfig {
            deadline_seconds: 0,
            now_unix_ts: 1_700_000_000,
            min_profit_wei: U256::from(1_000u64),
        };
        let err = build_round_trip_context_from_candidate(
            &c,
            1,
            dummy_executor(),
            &provider_with_weth_18(),
            &cfg,
        )
        .unwrap_err();
        assert!(matches!(err, SimEncoderError::MissingDeadlineConfig));
    }

    // ── Test 21 — Same token in/out rejected ───────────────────────────────

    #[test]
    fn same_token_in_out_rejected() {
        let mut c = valid_candidate_v2();
        c.token_addresses = vec![WETH.into(), WETH.into()];
        let p = InMemoryTokenDecimalsProvider::new().with_decimals(1, addr(WETH), 18);
        let err =
            build_round_trip_context_from_candidate(&c, 1, dummy_executor(), &p, &valid_config())
                .unwrap_err();
        assert!(matches!(err, SimEncoderError::SameTokenInOut { .. }));
    }

    // ── Test 22 — Invalid token address rejected ───────────────────────────

    #[test]
    fn invalid_token_address_rejected() {
        let mut c = valid_candidate_v2();
        c.token_addresses = vec!["not_an_address".into(), USDC.into()];
        let err = build_round_trip_context_from_candidate(
            &c,
            1,
            dummy_executor(),
            &provider_with_weth_18(),
            &valid_config(),
        )
        .unwrap_err();
        assert!(matches!(err, SimEncoderError::InvalidTokenAddress { .. }));
    }

    // ── Test 23 — Zero token address rejected ──────────────────────────────

    #[test]
    fn zero_token_address_rejected() {
        let mut c = valid_candidate_v2();
        c.token_addresses = vec![
            "0x0000000000000000000000000000000000000000".into(),
            USDC.into(),
        ];
        let err = build_round_trip_context_from_candidate(
            &c,
            1,
            dummy_executor(),
            &provider_with_weth_18(),
            &valid_config(),
        )
        .unwrap_err();
        assert!(matches!(err, SimEncoderError::ZeroTokenAddress { .. }));
    }

    // ── Test 24 — 1.0 WETH with 18 decimals → 10^18 wei (precision check) ──

    #[test]
    fn convert_one_eth_to_wei_exact() {
        let wei = convert_amount_to_wei(1.0, 18).unwrap();
        assert_eq!(wei, U256::from_dec_str("1000000000000000000").unwrap());
    }

    // ── Test 25 — 1.5 USDC with 6 decimals → 1_500_000 wei ─────────────────

    #[test]
    fn convert_one_point_five_usdc_to_wei() {
        let wei = convert_amount_to_wei(1.5, 6).unwrap();
        assert_eq!(wei, U256::from(1_500_000u64));
    }

    // ── Test 26 — Missing router for unconfigured chain rejected ───────────

    #[test]
    fn missing_router_for_unconfigured_chain() {
        // chain_id 99999 is not in `routers_for_chain`. The encoder must
        // reject with MissingRouterAddress rather than fabricating a router.
        let c = valid_candidate_v2();
        let p = InMemoryTokenDecimalsProvider::new().with_decimals(99999, addr(WETH), 18);
        let err = build_round_trip_context_from_candidate(
            &c,
            99999,
            dummy_executor(),
            &p,
            &valid_config(),
        )
        .unwrap_err();
        assert!(matches!(err, SimEncoderError::MissingRouterAddress { .. }));
    }
}
