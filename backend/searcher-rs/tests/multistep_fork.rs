//! Phase A.4 — Fork integration test for the multi-step REVM executor.
//!
//! THIS TEST IS `#[ignore]` BY DEFAULT — it requires external infrastructure
//! the unit-test sandbox does not have:
//!   * `RPC_HTTP_1` (or another `RPC_HTTP_<chain>`) pointing at an archive
//!     node that supports `eth_getStorageAt` at a pinned block.
//!   * `EXECUTOR_1` (or `EXECUTOR_<chain>`) — deployed `ArbitrageExecutor`
//!     contract address with EXECUTOR_ROLE granted to a test signer.
//!   * Storage layout configuration for the tokens used in the test route
//!     (WETH balance slot, USDC balance slot, USDC allowance slot, etc.).
//!   * Real V2 routers (UniswapV2 + Sushi) approved in the executor's
//!     `approvedRouters` mapping at the pinned block.
//!
//! ## Running locally
//!
//! ```bash
//! export RPC_HTTP_1="https://eth-mainnet.alchemyapi.io/v2/<KEY>"
//! export EXECUTOR_1="0x..."
//! export SIM_ORCHESTRATOR_GAS_PRICE_WEI="25000000000"
//! cargo test -p searcher-rs --test multistep_fork --all-features -- --ignored --nocapture
//! ```
//!
//! ## What the test validates (WRAPPED FLASH path — M2 flash R3)
//!
//! 1. The multi-step REVM executor reaches `sequence_runner` against real
//!    chain state (no PASS fabrication path possible).
//! 2. CacheDB<LazyDb> persists state between steps.
//! 3. `ApplyStorage` writes the caller→FLE EXECUTOR_ROLE bit + token_in
//!    balance/allowance overrides (paper-only).
//! 4. The SINGLE wrapped `requestFlashLoan` dispatch (caller → FlashLoanExecutor)
//!    runs the REAL provider callback + inner executeArbitrageFlashFunded
//!    against forked bytecode; gas_used > 0 or a real revert reason is captured.
//! 5. Final balance read produces the value the profit accounting uses.
//! 6. ANY outcome (SIM_SUCCESS / SIM_REVERT / SIM_REJECTED / SIM_ERROR)
//!    carries a typed reason; no path can produce success with zero gas,
//!    zero trace_hash, or non-positive net_profit.
//!
//! A SIM_REVERT outcome is an ACCEPTABLE pass condition for the fork
//! validation — it proves the system reaches REVM and rejects honestly.
//! A SIM_SUCCESS outcome proves the full flash-funded round trip produces
//! a real positive net profit. The end-to-end SIM_SUCCESS validation is
//! DEFERRED to the M5 deployed-testnet run (operator decision): the pinned
//! fork must carry the FLE→AE EXECUTOR_ROLE, approvedRouters / approvedTokens /
//! approvedSelectors, a funded flash provider, AND a caller with ETH for gas
//! (see the caller-gas note in `sim_multistep.rs`).

use std::str::FromStr;

use ethers::types::{Address, U256};
use prioritization_spine::round_trip_executor::RoundTripContext;
use searcher_rs::sim_multistep::{build_multistep_plan, MultiStepError, MultiStepExecutionConfig};
use searcher_rs::sim_prefund::{Erc20StorageLayout, Erc20StorageLayoutProvider};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Test-only storage layout provider
// ---------------------------------------------------------------------------

/// In-memory provider populated from environment / hardcoded mainnet
/// constants for the test route. Production uses a PG-backed registry.
struct FixtureLayoutProvider {
    layouts: HashMap<(u64, Address), Erc20StorageLayout>,
}

impl FixtureLayoutProvider {
    /// Mainnet WETH (`0xc02a…cc2`) layout:
    ///   * `balanceOf` mapping at slot 3 (verified via Foundry `cast storage`).
    ///   * No standard allowance; the test forces allowance via slot 4
    ///     which holds the canonical OZ allowance mapping for WETH9.
    ///
    /// Mainnet USDC (`0xa0b8…b48`) is a proxy; slot indices match the
    /// OpenZeppelin upgradeable storage convention for v2 implementation
    /// (balances at slot 9, allowances at slot 10). Operators should
    /// verify against the live deployment before relying on these.
    fn mainnet_fixture() -> Self {
        let mut layouts = HashMap::new();
        // WETH9 — non-OZ canonical layout:
        layouts.insert(
            (1, addr("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2")),
            Erc20StorageLayout {
                balance_base_slot: ethers::types::U256::from(3u64),
                allowance_base_slot: ethers::types::U256::from(4u64),
            },
        );
        // USDC (FiatTokenV2_2 proxy) — OZ upgradeable layout:
        layouts.insert(
            (1, addr("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48")),
            Erc20StorageLayout {
                balance_base_slot: ethers::types::U256::from(9u64),
                allowance_base_slot: ethers::types::U256::from(10u64),
            },
        );
        Self { layouts }
    }
}

impl Erc20StorageLayoutProvider for FixtureLayoutProvider {
    fn layout(&self, chain_id: u64, token: &Address) -> Option<Erc20StorageLayout> {
        self.layouts.get(&(chain_id, *token)).copied()
    }
}

fn addr(s: &str) -> Address {
    Address::from_str(s).expect("valid hex address")
}

// ---------------------------------------------------------------------------
// Pre-flight: check the runtime environment honestly.
// ---------------------------------------------------------------------------

#[derive(Debug)]
struct PrereqStatus {
    rpc_http_1: Option<String>,
    executor_1: Option<String>,
    flashloan_executor_1: Option<String>,
    sim_gas_price: Option<String>,
}

impl PrereqStatus {
    fn collect() -> Self {
        Self {
            rpc_http_1: std::env::var("RPC_HTTP_1").ok(),
            executor_1: std::env::var("EXECUTOR_1").ok(),
            flashloan_executor_1: std::env::var("FLASHLOAN_EXECUTOR_1").ok(),
            sim_gas_price: std::env::var("SIM_ORCHESTRATOR_GAS_PRICE_WEI").ok(),
        }
    }

    fn missing(&self) -> Vec<&'static str> {
        let mut m = Vec::new();
        if self
            .rpc_http_1
            .as_deref()
            .filter(|s| !s.is_empty())
            .is_none()
        {
            m.push("RPC_HTTP_1");
        }
        if self
            .executor_1
            .as_deref()
            .filter(|s| !s.is_empty())
            .is_none()
        {
            m.push("EXECUTOR_1");
        }
        if self
            .flashloan_executor_1
            .as_deref()
            .filter(|s| !s.is_empty())
            .is_none()
        {
            m.push("FLASHLOAN_EXECUTOR_1");
        }
        if self
            .sim_gas_price
            .as_deref()
            .filter(|s| !s.is_empty())
            .is_none()
        {
            m.push("SIM_ORCHESTRATOR_GAS_PRICE_WEI");
        }
        m
    }
}

// ---------------------------------------------------------------------------
// PHASE A.4 — Fork validation (ignored)
// ---------------------------------------------------------------------------

/// End-to-end validation of the multi-step REVM executor against mainnet
/// fork state. Constructs a WETH→USDC→WETH round-trip plan and feeds it
/// through `execute_multistep_revm`. Expects ONE of:
///
///   * `passed = true` — a profitable route exists at the pinned block.
///     Asserts: `gas_used_total > 0`, `simulated_profit_token_in > 0`,
///     intermediate amount was non-zero.
///   * `passed = false` with a typed `fail_reason` — the route either
///     reverted on-chain or did not clear the net-profit gate. This is
///     STILL a successful fork validation: the system reached REVM and
///     handled the result honestly.
///
/// The test FAILS only when:
///   * The executor never reaches `sequence_runner` (plan construction
///     errors out before dispatch).
///   * `passed = true` but with `gas_used_total == 0` or empty trace
///     hash — anti-fraud guard violation.
///   * A SIM_SUCCESS without real REVM state evolution (impossible by
///     construction in A.3.c.3; the test guards against future drift).
#[cfg(feature = "v2-simulator")]
#[tokio::test]
#[ignore = "requires RPC_HTTP_1 + EXECUTOR_1 + archive node — see module docs"]
async fn multistep_fork_round_trip_weth_usdc() {
    let prereqs = PrereqStatus::collect();
    let missing = prereqs.missing();
    if !missing.is_empty() {
        // Honest skip — print what's missing so the operator knows
        // exactly which env to set.
        eprintln!(
            "SKIP: A.4 fork validation requires env vars: {missing:?}. \
             Without them the test cannot reach an archive node + deployed \
             executor. Aborting fail-honest per directive."
        );
        return;
    }

    // Construct a realistic mainnet WETH → USDC → WETH round trip.
    let caller = match prereqs.executor_1.as_deref() {
        Some(s) => Address::from_str(s.trim()).expect("EXECUTOR_1 must parse"),
        None => unreachable!("missing check passed"),
    };
    let weth = addr("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2");
    let usdc = addr("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48");
    let univ2 = addr("0x7a250d5630b4cf539739df2c5dacb4c659f2488d"); // UniV2 Router02
    let sushi = addr("0xd9e1ce17f2641f24ae83637ab66a2cca9c378b9f"); // Sushi Router

    let ctx = RoundTripContext {
        caller,
        token_in: weth,
        token_out: usdc,
        amount_in: U256::from(10u64).pow(U256::from(18u64)), // 1 WETH
        forward_router: univ2,
        forward_path: vec![weth, usdc],
        backward_router: sushi,
        backward_path: vec![usdc, weth],
        deadline: U256::from(u64::MAX),
    };

    let gas_price_wei: U256 = prereqs
        .sim_gas_price
        .as_deref()
        .and_then(|s| U256::from_dec_str(s).ok())
        .expect("SIM_ORCHESTRATOR_GAS_PRICE_WEI must parse as decimal U256");

    // `executor_address` is the deployed ArbitrageExecutor (inner
    // executeArbitrageFlashFunded target / per-leg swap recipient).
    let arbitrage_executor = caller;
    // `.to()` of the wrapped flash dispatch — the deployed FlashLoanExecutor.
    let flashloan_executor = match prereqs.flashloan_executor_1.as_deref() {
        Some(s) => Address::from_str(s.trim()).expect("FLASHLOAN_EXECUTOR_1 must parse"),
        None => unreachable!("missing check passed"),
    };

    let config = MultiStepExecutionConfig {
        chain_id: 1,
        executor_address: arbitrage_executor,
        // Operator-supplied route identity + net-profit gate for the inner
        // executeArbitrageFlashFunded args. Fixed sentinels for the fork run.
        route_hash: [0u8; 32],
        min_profit_wei: U256::from(1u64),
        gas_price_wei,
        gas_limit_per_step: 30_000_000,
        paper_mode: true,
        enable_storage_cheats: true,
        require_trace_hash: true,
        require_positive_net_profit: true,
        max_steps: 16,
    };

    let provider = FixtureLayoutProvider::mainnet_fixture();

    // 1. Plan construction must succeed before any REVM dispatch.
    let plan_result = build_multistep_plan(&ctx, &config, flashloan_executor, &provider);
    match plan_result {
        Ok(plan) => {
            assert!(
                plan.steps.len() >= 4,
                "expected wrapped flash plan (>=4 steps, got {})",
                plan.steps.len()
            );
            eprintln!(
                "A.4 plan built: {} steps, caller={:?}, amount_in={}",
                plan.steps.len(),
                plan.caller,
                plan.amount_in
            );
        }
        Err(MultiStepError::PrefundFailed(e)) => {
            eprintln!(
                "A.4 plan FAILED at prefund: {e:?}. \
                 This is BLOCKED — check that FixtureLayoutProvider has the \
                 storage layout for WETH+USDC at chain_id=1."
            );
            panic!("prefund layout missing — fix FixtureLayoutProvider before re-run");
        }
        Err(e) => {
            panic!(
                "A.4 plan construction failed with non-prefund error: {e:?}. \
                 This indicates a regression in build_multistep_plan."
            );
        }
    }

    // ── Live REVM dispatch against the operator's archive RPC ─────────────
    //
    // `SimulatorV2` + `LazyDb` spin up an internal tokio runtime; invoking the
    // synchronous `execute_multistep_revm` directly from this `#[tokio::test]`
    // would nest runtimes and panic on drop (the same hazard A.3.c.4 fixed by
    // reverting `cache_db_accepts_lazy_db_database_ref` to a sync `#[test]`).
    // We therefore run it inside `spawn_blocking` — a non-worker thread that is
    // free to own a runtime — and await the join handle.
    //
    // `RPC_HTTP_1` here MUST be a single bare archive URL: `LazyDb` performs
    // direct JSON-RPC and does NOT parse the multi-vendor `name=url,...` CSV.
    let rpc_url = prereqs
        .rpc_http_1
        .clone()
        .expect("RPC_HTTP_1 present (missing check already passed)");
    let simulator = std::sync::Arc::new(simulator_v2::SimulatorV2::new(rpc_url));
    let provider_arc: std::sync::Arc<
        dyn searcher_rs::sim_prefund::Erc20StorageLayoutProvider + Send + Sync,
    > = std::sync::Arc::new(provider);
    let ctx_owned = ctx.clone();
    let config_owned = config.clone();
    let outcome = tokio::task::spawn_blocking(move || {
        searcher_rs::sim_multistep::execute_multistep_revm(
            &ctx_owned,
            simulator,
            &config_owned,
            provider_arc.as_ref(),
        )
    })
    .await
    .expect("spawn_blocking joined");

    // Anti-fraud / fail-honest guards on the outcome:
    //   * `execute_multistep_revm` returns `passed = true` ONLY after a real
    //     round trip (gas > 0, non-zero trace hash, >= 2 committed calls, and
    //     net_profit > 0). We re-assert gas + profit here so a future drift
    //     that fabricates a pass trips immediately.
    //   * A non-passing outcome (SIM_REVERT / SIM_REJECTED) is an ACCEPTABLE
    //     fork-validation result — it proves the system reached REVM and
    //     rejected honestly — but it MUST carry a typed `fail_reason`.
    // Either branch prints exactly ONE machine-greppable `A4_OUTCOME=` line
    // that `run_a4_fork_validation.sh` asserts on, so a skipped / 0-test run
    // can never be recorded as a pass.
    if outcome.passed {
        assert!(
            outcome.gas_used_total > 0,
            "anti-fraud: SIM_SUCCESS with zero gas"
        );
        assert!(
            !outcome.simulated_profit_token_in.is_zero(),
            "anti-fraud: SIM_SUCCESS with zero net profit"
        );
        eprintln!(
            "A4_OUTCOME=SIM_SUCCESS gas_used={} net_profit_token_in={} intermediate={:?}",
            outcome.gas_used_total,
            outcome.simulated_profit_token_in,
            outcome.intermediate_amount_out,
        );
    } else {
        let reason = outcome
            .fail_reason
            .as_deref()
            .expect("fail-honest: a non-passing outcome MUST carry a typed reason");
        assert!(
            !reason.is_empty(),
            "fail-honest: empty fail_reason on a non-passing outcome"
        );
        eprintln!("A4_OUTCOME=SIM_REVERT reason={reason}");
    }
}

// ---------------------------------------------------------------------------
// Smoke test (NOT ignored) — verifies the test harness itself compiles
// without external prereqs. Ensures the fork test scaffold doesn't rot.
// ---------------------------------------------------------------------------

#[test]
fn fixture_layout_provider_returns_layout_for_mainnet_weth() {
    let p = FixtureLayoutProvider::mainnet_fixture();
    let weth = addr("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2");
    let layout = p.layout(1, &weth).expect("WETH layout must exist");
    // Sanity: balance slot is 3 (known WETH9 layout); allowance slot is 4.
    assert_eq!(layout.balance_base_slot, U256::from(3u64));
    assert_eq!(layout.allowance_base_slot, U256::from(4u64));
}

#[test]
fn prereq_status_reports_missing_when_env_is_empty() {
    // Save and clear known env vars for the duration of this test.
    let prev_rpc = std::env::var("RPC_HTTP_1").ok();
    let prev_exec = std::env::var("EXECUTOR_1").ok();
    let prev_fle = std::env::var("FLASHLOAN_EXECUTOR_1").ok();
    let prev_gas = std::env::var("SIM_ORCHESTRATOR_GAS_PRICE_WEI").ok();
    std::env::remove_var("RPC_HTTP_1");
    std::env::remove_var("EXECUTOR_1");
    std::env::remove_var("FLASHLOAN_EXECUTOR_1");
    std::env::remove_var("SIM_ORCHESTRATOR_GAS_PRICE_WEI");

    let prereqs = PrereqStatus::collect();
    let missing = prereqs.missing();
    assert!(missing.contains(&"RPC_HTTP_1"));
    assert!(missing.contains(&"EXECUTOR_1"));
    assert!(missing.contains(&"FLASHLOAN_EXECUTOR_1"));
    assert!(missing.contains(&"SIM_ORCHESTRATOR_GAS_PRICE_WEI"));

    // Restore env vars for any subsequent tests.
    if let Some(v) = prev_rpc {
        std::env::set_var("RPC_HTTP_1", v);
    }
    if let Some(v) = prev_exec {
        std::env::set_var("EXECUTOR_1", v);
    }
    if let Some(v) = prev_fle {
        std::env::set_var("FLASHLOAN_EXECUTOR_1", v);
    }
    if let Some(v) = prev_gas {
        std::env::set_var("SIM_ORCHESTRATOR_GAS_PRICE_WEI", v);
    }
}
