//! Round-trip executor — Phase 4 of REVM real implementation.
//!
//! Orchestrates a full forward + backward swap simulation in REVM to compute
//! REALIZED arbitrage profit (not the spread upper-bound the spine currently
//! reports). Combines all prior phases:
//!
//!   - Phase 1 (swap_encoder V2/V3): builds router calldata
//!   - Phase 3 (erc20_storage): pre-funds the synthetic EOA via slot override
//!   - lazy_db (pre-existing): RPC-backed Database for revm
//!
//! Architecture (high level):
//!
//!   1. Pre-fund caller with `amount_in` of token_in (storage override)
//!   2. Approve forward router (allowance storage override OR approve tx)
//!   3. Encode + execute forward swap (token_in → token_out via pool A)
//!   4. Read intermediate balance via `balanceOf(caller, token_out)`
//!   5. Approve backward router for token_out
//!   6. Encode + execute backward swap (token_out → token_in via pool B)
//!   7. Read final balance via `balanceOf(caller, token_in)`
//!   8. Profit = final_in - amount_in
//!   9. Subtract gas cost in USD → net profit
//!
//! What this commit ships (~Phase 4 scope):
//!   ✅ SimulationOutcome data type (downstream consumers can rely on shape)
//!   ✅ RoundTripContext input bundle
//!   ✅ Pure helpers: decode_balance_of_return, compute_profit_usd
//!   ✅ Pure helper: build_round_trip_plan (encoders only, no execution)
//!   ⏸ execute_round_trip orchestrator — SKELETON marked TODO, validation
//!     deferred to Phase 5 (mainnet fork integration tests).
//!
//! Why ship a skeleton instead of full implementation:
//! Without a real EVM context (mainnet fork) the orchestrator cannot be
//! validated. Writing 200 LOC of unverified execution code introduces
//! more risk than value. Phase 5 will pair the orchestrator with `forge
//! test --fork-url` style tests that pin a real historic block and
//! assert the simulator returns realistic profit figures.
//!
//! See `docs/superpowers/plans/2026-05-05-revm-real-implementation.md`.

use ethers::types::{Address, Bytes, U256};
use serde::{Deserialize, Serialize};

use crate::swap_encoder::{encode_erc20_balance_of, encode_v2_swap_exact_tokens_for_tokens};

/// Output of a round-trip simulation. Either the simulation completed
/// successfully (`passed = true`) and we have realised profit + gas, OR
/// it failed and we have a diagnostic `fail_reason`.
///
/// `simulated_profit_token_in` is the raw `final_balance - initial_balance`
/// in token_in units (NOT USD), i.e. the GROSS token_in delta. Negative
/// scenarios are represented as `passed=true, simulated_profit_token_in=0`
/// because the math returns a wrapped underflow on revert; converting to
/// signed would mask the actual sim outcome.
///
/// The sim is intentionally PRICES-FREE: it carries the gross token_in delta
/// plus the gas it measured (`gas_used_total` + `gas_price_wei`), and the
/// price-aware downstream layer computes net-of-gas USD via
/// `compute_profit_usd` (which also takes token_in decimals/price + eth price).
/// The NET-of-gas-in-USD profitability decision is the DOWNSTREAM consumer's
/// responsibility — and that downstream net-USD gate MUST be enforced before
/// any LIVE broadcast.
#[derive(Debug, Clone)]
pub struct SimulationOutcome {
    pub passed: bool,
    pub simulated_profit_token_in: U256,
    pub intermediate_amount_out: Option<U256>,
    pub gas_used_total: u64,
    /// The gas price (wei) the simulator used. Carried (alongside
    /// `gas_used_total`) so the price-aware downstream layer can compute the
    /// gas cost in USD via `compute_profit_usd`. `U256::zero()` on failure /
    /// producers that do not measure gas.
    pub gas_price_wei: U256,
    pub fail_reason: Option<String>,
    /// The exact wrapped-flash broadcast calldata (outer `requestFlashLoan`
    /// 0x5107d61e wrapping inner `executeArbitrageFlashFunded` 0xdde0bf51) that
    /// the sim VALIDATED, with leg-1 encoded against the REAL forward-quoted
    /// intermediate. `Some(bytes)` ONLY on a passing wrapped-flash outcome from
    /// `sim_multistep::execute_multistep_revm`; `None` for every other
    /// producer / failure path (so the broadcast path can only ever replay
    /// calldata that the sim actually proved out).
    pub wrapped_calldata: Option<Vec<u8>>,
}

impl SimulationOutcome {
    pub fn failed(reason: impl Into<String>) -> Self {
        Self {
            passed: false,
            simulated_profit_token_in: U256::zero(),
            intermediate_amount_out: None,
            gas_used_total: 0,
            gas_price_wei: U256::zero(),
            fail_reason: Some(reason.into()),
            wrapped_calldata: None,
        }
    }
}

/// Bundle of inputs the orchestrator needs to plan + execute a round trip.
/// All addresses are caller-supplied (typically resolved by the scanner from
/// PG pool metadata before invoking the simulator).
///
/// `Serialize`/`Deserialize` are derived so this context can be embedded in the
/// M2 carrier-B [`crate::validated_plan::ValidatedPlan`] record (persisted by
/// `opp.id` at sim-validate time and re-read by the broadcast path). ethers'
/// `Address`/`U256` provide their own serde impls; the derive is purely
/// additive and does not change the sim path's behaviour.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoundTripContext {
    pub caller: Address,
    pub token_in: Address,
    pub token_out: Address,
    pub amount_in: U256,
    /// Forward leg: token_in → token_out via this router using `forward_path`.
    pub forward_router: Address,
    pub forward_path: Vec<Address>,
    /// Backward leg: token_out → token_in via this router using `backward_path`.
    /// MUST differ from forward_router OR backward_path != reversed(forward_path)
    /// for a real arbitrage; otherwise the simulator unwinds and profit ≤ 0.
    pub backward_router: Address,
    pub backward_path: Vec<Address>,
    pub deadline: U256,
}

/// The pre-computed calldata bundle for the round trip. Produced by
/// `build_round_trip_plan` and consumed by `execute_round_trip`. The
/// backward calldata is parameterised on the intermediate amount which
/// is only known AFTER forward execution — so the plan stores only the
/// router/path needed to build it later.
#[derive(Debug, Clone)]
pub struct RoundTripPlan<'ctx> {
    pub ctx: &'ctx RoundTripContext,
    pub forward_calldata: Bytes,
    pub balance_of_caller_token_out_calldata: Bytes,
    pub balance_of_caller_token_in_calldata: Bytes,
}

/// Build the calldata bundle for the round trip. Pure — no IO, no EVM
/// execution. Encoders come from Phase 1/2 (swap_encoder).
///
/// The forward calldata uses `amount_out_minimum=0` because the simulator
/// is not enforcing slippage (it's measuring realised profit, not constraining
/// the swap). Real on-chain execution would compute amount_out_minimum from
/// quote + tolerance.
pub fn build_round_trip_plan<'a>(ctx: &'a RoundTripContext) -> RoundTripPlan<'a> {
    let forward_calldata = encode_v2_swap_exact_tokens_for_tokens(
        ctx.amount_in,
        U256::zero(), // amount_out_minimum=0 (no slippage protection in sim)
        &ctx.forward_path,
        ctx.caller,
        ctx.deadline,
    );
    RoundTripPlan {
        ctx,
        forward_calldata,
        balance_of_caller_token_out_calldata: encode_erc20_balance_of(ctx.caller),
        balance_of_caller_token_in_calldata: encode_erc20_balance_of(ctx.caller),
    }
}

/// Decode the 32-byte return data of an ERC20 `balanceOf(account)` call
/// into a U256. Returns `U256::zero()` on malformed input — caller treats
/// this as "balance unknown" and may abort the simulation.
///
/// Standard return is uint256 left-padded to 32 bytes (one EVM word).
/// Anything shorter is malformed; anything longer takes the first 32 bytes.
pub fn decode_balance_of_return(return_data: &[u8]) -> U256 {
    if return_data.len() < 32 {
        return U256::zero();
    }
    U256::from_big_endian(&return_data[..32])
}

/// Compute net USD profit from a round-trip simulation.
///
/// Inputs:
///   - `initial_in`  : token_in balance BEFORE the round trip (raw u256 wei)
///   - `final_in`    : token_in balance AFTER the round trip (raw u256 wei)
///   - `token_in_decimals`: e.g. 18 for WETH, 6 for USDC, 8 for WBTC
///   - `token_in_price_usd`: USD per 1.0 unit of token_in
///   - `gas_used_total`: sum of gas across forward + backward + balanceOf calls
///   - `gas_price_wei`: gas price the simulator used
///   - `eth_price_usd`: USD per 1 ETH (for gas cost USD conversion)
///
/// Returns net USD profit (positive = profitable, negative = loss after gas).
/// Uses lossy f64 conversion past ~15 sig figs (acceptable for display).
pub fn compute_profit_usd(
    initial_in: U256,
    final_in: U256,
    token_in_decimals: u8,
    token_in_price_usd: f64,
    gas_used_total: u64,
    gas_price_wei: U256,
    eth_price_usd: f64,
) -> f64 {
    // Profit in token_in units (signed-aware: if final < initial, net loss).
    let profit_signed_usd = if final_in >= initial_in {
        let delta_in = final_in - initial_in;
        let scale = 10f64.powi(token_in_decimals as i32);
        u256_to_f64_lossy(delta_in) / scale * token_in_price_usd
    } else {
        let delta_in = initial_in - final_in;
        let scale = 10f64.powi(token_in_decimals as i32);
        -(u256_to_f64_lossy(delta_in) / scale * token_in_price_usd)
    };

    // Gas cost in USD: gas_used × gas_price_wei → wei → ETH → USD.
    let gas_cost_wei = gas_price_wei.saturating_mul(U256::from(gas_used_total));
    let gas_cost_eth = u256_to_f64_lossy(gas_cost_wei) / 1e18;
    let gas_cost_usd = gas_cost_eth * eth_price_usd;

    profit_signed_usd - gas_cost_usd
}

fn u256_to_f64_lossy(v: U256) -> f64 {
    v.to_string().parse::<f64>().unwrap_or(0.0)
}

/// Net-USD-of-gas viability gate (O4 follow-up — the pre-live profitability
/// floor that the pure REVM sim deliberately does NOT enforce).
///
/// The wrapped-flash sim (`execute_multistep_revm`) gates SIM_SUCCESS on GROSS
/// `retained_spread` (token_in units) — it carries the gross delta plus the gas
/// it measured but, being prices-free, never subtracts gas. So a gross-positive
/// but NET-negative (gas-losing) arb can pass the sim. This helper closes that
/// hole: it converts the gross token_in spread to USD, subtracts the gas cost in
/// USD (via [`compute_profit_usd`]), and returns `true` ONLY when the result is
/// strictly positive.
///
/// Inputs:
///   - `gross_token_in`: the GROSS token_in delta the sim retained
///     (`SimulationOutcome.simulated_profit_token_in`, already `final - initial`).
///   - `decimals`: token_in decimals (18 WETH, 6 USDC, 8 WBTC).
///   - `token_in_price_usd`: USD per 1.0 unit of token_in.
///   - `gas_used`: `SimulationOutcome.gas_used_total`.
///   - `gas_price_wei`: `SimulationOutcome.gas_price_wei`.
///   - `eth_price_usd`: USD per 1 ETH (for the gas-cost USD conversion).
///
/// FAIL-CLOSED: a non-finite or non-positive `token_in_price_usd` /
/// `eth_price_usd` (price feed down / unpriced token) yields `false` — the gate
/// treats "cannot price" as "not viable" so a price-unavailable path can never
/// re-open the gas-losing-broadcast hole. The caller must NOT persist/broadcast
/// a plan for which this returns `false`.
///
/// Modeled by calling `compute_profit_usd` with `initial_in = 0` and
/// `final_in = gross_token_in`, so `final - initial = gross_token_in` and the
/// returned figure is `gross_spread_usd - gas_usd` (net of gas).
pub fn net_usd_viable(
    gross_token_in: U256,
    decimals: u8,
    token_in_price_usd: f64,
    gas_used: u64,
    gas_price_wei: U256,
    eth_price_usd: f64,
) -> bool {
    // Fail-closed on unusable prices: a missing / non-finite / non-positive
    // price means we CANNOT compute net-of-gas honestly. Treat as not viable.
    if !token_in_price_usd.is_finite()
        || token_in_price_usd <= 0.0
        || !eth_price_usd.is_finite()
        || eth_price_usd <= 0.0
    {
        return false;
    }
    let net_usd = compute_profit_usd(
        U256::zero(),
        gross_token_in,
        decimals,
        token_in_price_usd,
        gas_used,
        gas_price_wei,
        eth_price_usd,
    );
    net_usd > 0.0
}

// ─── Phase 5 SKELETON (validation deferred to fork tests) ───────────────────

/// Execute the round trip in REVM. Validation deferred to Phase 5
/// (mainnet fork integration tests) — until those land, this function
/// is intentionally NOT integrated into `simulator::simulate_candidate`.
/// The stub there continues to return "PASS" so production behaviour is
/// unchanged by this commit.
///
/// Phase 5 will:
///   - Pin a known historic block where an arbitrage existed
///   - Build a RoundTripContext for that opportunity
///   - Call execute_round_trip(...)
///   - Assert SimulationOutcome.simulated_profit_token_in matches the
///     historical realised profit ±5%
///   - Wire `simulator::simulate_candidate` to use this path
///
/// The signature is finalised here (consumers in config_aware can depend
/// on the SimulationOutcome shape). Implementation follows.
pub fn execute_round_trip(_plan: &RoundTripPlan) -> SimulationOutcome {
    // TODO Phase 5: implement using LazyRpcDatabase + revm EVM.
    // See module docs for the 9-step orchestration.
    SimulationOutcome::failed("execute_round_trip pending Phase 5 implementation")
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared_rs::chains::{USDC_MAINNET, WETH_MAINNET};
    use std::str::FromStr;

    fn addr(hex: &str) -> Address {
        Address::from_str(hex).expect("valid hex address")
    }

    fn weth() -> Address {
        addr(WETH_MAINNET)
    }
    fn usdc() -> Address {
        addr(USDC_MAINNET)
    }

    #[test]
    fn decode_balance_of_return_standard_word() {
        // U256(123_456) left-padded to 32 bytes.
        let mut bytes = [0u8; 32];
        let val = U256::from(123_456u64);
        val.to_big_endian(&mut bytes);
        assert_eq!(decode_balance_of_return(&bytes), val);
    }

    #[test]
    fn decode_balance_of_return_malformed_returns_zero() {
        // <32 bytes is malformed.
        assert_eq!(decode_balance_of_return(&[]), U256::zero());
        assert_eq!(decode_balance_of_return(&[0u8; 31]), U256::zero());
    }

    #[test]
    fn decode_balance_of_return_extra_bytes_uses_first_word() {
        // Return data >32 bytes (some routers pad weirdly) — take first word.
        let mut bytes = [0u8; 64];
        let val = U256::from(999_888_777u64);
        val.to_big_endian(&mut bytes[..32]);
        // bytes[32..64] is garbage; should be ignored
        for b in &mut bytes[32..] {
            *b = 0xff;
        }
        assert_eq!(decode_balance_of_return(&bytes), val);
    }

    #[test]
    fn compute_profit_usd_positive_outcome() {
        // Started with 1 ETH, ended with 1.005 ETH → +0.005 ETH * $2500 = $12.50
        // Gas: 200_000 gas × 25 gwei × $2500/ETH = $12.50
        // Net: $12.50 - $12.50 = $0
        let initial = U256::from(10_u64).pow(U256::from(18));
        let final_ = initial + U256::from(5_000_000_000_000_000u64); // +0.005 ETH
        let gas_price = U256::from(25_000_000_000u64); // 25 gwei
        let net = compute_profit_usd(initial, final_, 18, 2500.0, 200_000, gas_price, 2500.0);
        assert!(
            (net - 0.0).abs() < 0.5,
            "net should be ~$0 (gross $12.5 - gas $12.5), got {net}"
        );
    }

    #[test]
    fn compute_profit_usd_negative_outcome() {
        // Lost 0.001 ETH on the round trip.
        let initial = U256::from(10_u64).pow(U256::from(18));
        let final_ = initial - U256::from(1_000_000_000_000_000u64); // -0.001 ETH
        let gas_price = U256::from(20_000_000_000u64);
        let net = compute_profit_usd(initial, final_, 18, 2500.0, 100_000, gas_price, 2500.0);
        // Profit USD = -0.001 * 2500 = -$2.50
        // Gas USD = 100_000 * 20e9 / 1e18 * 2500 = $5.00
        // Net = -2.50 - 5.00 = -$7.50
        assert!((net - (-7.5)).abs() < 0.1, "expected ~-$7.50, got {net}");
    }

    #[test]
    fn compute_profit_usd_handles_usdc_decimals() {
        // USDC has 6 decimals. 1000 USDC = 1_000_000_000 raw units.
        // Round trip leaves +10 USDC = 10_000_000 raw units = $10.00
        // Gas $1.00
        // Net $9.00
        let initial = U256::from(1_000_000_000u64);
        let final_ = U256::from(1_010_000_000u64);
        let gas_price = U256::from(20_000_000_000u64);
        // 100_000 gas * 20 gwei = 2_000_000_000_000_000 wei = 0.002 ETH * $500 = $1
        let net = compute_profit_usd(initial, final_, 6, 1.0, 100_000, gas_price, 500.0);
        assert!(
            (net - 9.0).abs() < 0.1,
            "expected ~$9 net for USDC trade, got {net}"
        );
    }

    // ── net_usd_viable gate (O4 follow-up) ──────────────────────────────────

    #[test]
    fn net_usd_viable_weth_gross_positive_low_gas_passes() {
        // +0.01 WETH gross spread @ $2500 = $25.00 gross.
        // Gas: 200_000 × 25 gwei × $2500/ETH = $12.50. Net = +$12.50 → viable.
        let gross = U256::from(10_000_000_000_000_000u64); // 0.01 WETH
        let gas_price = U256::from(25_000_000_000u64); // 25 gwei
        assert!(net_usd_viable(
            gross, 18, 2500.0, 200_000, gas_price, 2500.0
        ));
    }

    #[test]
    fn net_usd_viable_weth_gross_positive_but_gas_exceeds_spread_rejects() {
        // +0.001 WETH gross spread @ $2500 = $2.50 gross.
        // Gas: 200_000 × 25 gwei × $2500/ETH = $12.50. Net = -$10.00 → NOT viable.
        let gross = U256::from(1_000_000_000_000_000u64); // 0.001 WETH
        let gas_price = U256::from(25_000_000_000u64);
        assert!(!net_usd_viable(
            gross, 18, 2500.0, 200_000, gas_price, 2500.0
        ));
    }

    #[test]
    fn net_usd_viable_usdc_six_decimals_gross_positive_passes() {
        // USDC has 6 decimals. +50 USDC gross = 50_000_000 raw units @ $1 = $50.
        // Gas: 100_000 × 20 gwei × $2500/ETH = 0.002 ETH × $2500 = $5.00.
        // Net = +$45 → viable. (Guards the units-conflation case: a 6-decimal
        // token must NOT be scaled by 1e18.)
        let gross = U256::from(50_000_000u64); // 50 USDC
        let gas_price = U256::from(20_000_000_000u64);
        assert!(net_usd_viable(gross, 6, 1.0, 100_000, gas_price, 2500.0));
    }

    #[test]
    fn net_usd_viable_usdc_tiny_spread_rejected_by_gas() {
        // +1 USDC gross = $1.00. Gas $5.00 → net -$4.00 → NOT viable.
        let gross = U256::from(1_000_000u64); // 1 USDC
        let gas_price = U256::from(20_000_000_000u64);
        assert!(!net_usd_viable(gross, 6, 1.0, 100_000, gas_price, 2500.0));
    }

    #[test]
    fn net_usd_viable_wbtc_eight_decimals_gross_positive_passes() {
        // WBTC has 8 decimals @ ~$60_000. +0.001 WBTC = 100_000 raw units = $60.
        // Gas: 200_000 × 25 gwei × $2500/ETH = $12.50. Net = +$47.50 → viable.
        // Guards the high-value/8-decimal conflation case.
        let gross = U256::from(100_000u64); // 0.001 WBTC
        let gas_price = U256::from(25_000_000_000u64);
        assert!(net_usd_viable(
            gross, 8, 60_000.0, 200_000, gas_price, 2500.0
        ));
    }

    #[test]
    fn net_usd_viable_cheap_token_huge_units_small_usd_rejected() {
        // Cheap token: 18 decimals, $0.0001 each. A "big" 1000-token gross delta
        // is only $0.10 gross. Gas $12.50 → net -$12.40 → NOT viable.
        // Guards against a large raw-unit delta masquerading as profit.
        let gross = U256::from(10u64).pow(U256::from(18u64)) * U256::from(1000u64); // 1000 tokens
        let gas_price = U256::from(25_000_000_000u64);
        assert!(!net_usd_viable(
            gross, 18, 0.0001, 200_000, gas_price, 2500.0
        ));
    }

    #[test]
    fn net_usd_viable_zero_token_price_fails_closed() {
        // Price unavailable (0.0) → cannot compute net-of-gas → fail-closed.
        let gross = U256::from(10_000_000_000_000_000u64); // 0.01 WETH (gross-positive)
        let gas_price = U256::from(25_000_000_000u64);
        assert!(!net_usd_viable(gross, 18, 0.0, 200_000, gas_price, 2500.0));
    }

    #[test]
    fn net_usd_viable_zero_eth_price_fails_closed() {
        // ETH price unavailable → gas cost cannot be priced → fail-closed.
        let gross = U256::from(10_000_000_000_000_000u64);
        let gas_price = U256::from(25_000_000_000u64);
        assert!(!net_usd_viable(gross, 18, 2500.0, 200_000, gas_price, 0.0));
    }

    #[test]
    fn net_usd_viable_nan_or_inf_price_fails_closed() {
        let gross = U256::from(10_000_000_000_000_000u64);
        let gas_price = U256::from(25_000_000_000u64);
        assert!(!net_usd_viable(
            gross,
            18,
            f64::NAN,
            200_000,
            gas_price,
            2500.0
        ));
        assert!(!net_usd_viable(
            gross,
            18,
            2500.0,
            200_000,
            gas_price,
            f64::INFINITY
        ));
        assert!(!net_usd_viable(
            gross, 18, -2500.0, 200_000, gas_price, 2500.0
        ));
    }

    #[test]
    fn net_usd_viable_zero_gross_spread_rejected() {
        // Zero gross spread with any gas → net = -gas < 0 → NOT viable.
        let gas_price = U256::from(25_000_000_000u64);
        assert!(!net_usd_viable(
            U256::zero(),
            18,
            2500.0,
            200_000,
            gas_price,
            2500.0
        ));
    }

    #[test]
    fn net_usd_viable_exactly_breakeven_rejected() {
        // Gross USD == gas USD → net == 0 → strictly NOT viable (> 0 required).
        // +0.005 WETH @ $2500 = $12.50; gas 200_000 × 25 gwei × $2500 = $12.50.
        let gross = U256::from(5_000_000_000_000_000u64); // 0.005 WETH
        let gas_price = U256::from(25_000_000_000u64);
        assert!(!net_usd_viable(
            gross, 18, 2500.0, 200_000, gas_price, 2500.0
        ));
    }

    #[test]
    fn build_round_trip_plan_produces_valid_calldata() {
        let ctx = RoundTripContext {
            caller: addr("0x1111111111111111111111111111111111111111"),
            token_in: weth(),
            token_out: usdc(),
            amount_in: U256::from(10_u64).pow(U256::from(18)),
            forward_router: addr("0x7a250d5630b4cf539739df2c5dacb4c659f2488d"),
            forward_path: vec![weth(), usdc()],
            backward_router: addr("0x68b3465833fb72A70ecDF485E0e4C7bD8665Fc45"),
            backward_path: vec![usdc(), weth()],
            deadline: U256::from(1_700_000_000u64),
        };
        let plan = build_round_trip_plan(&ctx);

        // Forward calldata starts with V2 swapExactTokensForTokens selector
        assert_eq!(&plan.forward_calldata[..4], &[0x38, 0xed, 0x17, 0x39]);
        // BalanceOf calls start with the standard selector
        assert_eq!(
            &plan.balance_of_caller_token_out_calldata[..4],
            &[0x70, 0xa0, 0x82, 0x31]
        );
        assert_eq!(
            &plan.balance_of_caller_token_in_calldata[..4],
            &[0x70, 0xa0, 0x82, 0x31]
        );
    }

    #[test]
    fn simulation_outcome_failed_constructor() {
        let outcome = SimulationOutcome::failed("router reverted");
        assert!(!outcome.passed);
        assert_eq!(outcome.simulated_profit_token_in, U256::zero());
        assert_eq!(outcome.fail_reason.as_deref(), Some("router reverted"));
    }

    #[test]
    fn execute_round_trip_skeleton_returns_pending_marker() {
        // Until Phase 5 implements the orchestrator, calling it returns
        // a "pending" outcome — NEVER a fake "PASS". This protects against
        // accidental wiring into config_aware before validation.
        let ctx = RoundTripContext {
            caller: addr("0x1111111111111111111111111111111111111111"),
            token_in: weth(),
            token_out: usdc(),
            amount_in: U256::from(10_u64).pow(U256::from(18)),
            forward_router: addr("0x7a250d5630b4cf539739df2c5dacb4c659f2488d"),
            forward_path: vec![weth(), usdc()],
            backward_router: addr("0x68b3465833fb72A70ecDF485E0e4C7bD8665Fc45"),
            backward_path: vec![usdc(), weth()],
            deadline: U256::from(1_700_000_000u64),
        };
        let plan = build_round_trip_plan(&ctx);
        let outcome = execute_round_trip(&plan);
        assert!(!outcome.passed);
        assert!(outcome.fail_reason.unwrap().contains("Phase 5"));
    }
}
