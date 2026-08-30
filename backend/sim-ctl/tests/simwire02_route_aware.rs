//! SIMWIRE-E2E-01 — route-aware consumer path NEVER returns the empty-calldata
//! defect (live PG + live mainnet REVM).
//!
//! THIS TEST IS `#[ignore]` BY DEFAULT — it requires external infrastructure:
//!   * `DATABASE_URL`  — live Postgres with the migrations applied (the CI
//!     integration-tests job provides one; it seeds + cleans its own fixture
//!     rows). Outside CI: point at the dev DB.
//!   * `REVM_RPC_URL`  — an Ethereum mainnet RPC (same var the consumer's
//!     `B2cCtx` simulator uses). READ-ONLY usage: LazyDb fetches state via
//!     eth_getBalance/eth_getCode/eth_getStorageAt. No signer, no broadcast.
//!   * `REDIS_URL`     — optional; when reachable, gas_price_wei is read from
//!     the live key (`gas_price_wei_key(1)`), exactly like the consumer.
//!     Otherwise `GAS_PRICE_WEI_TEST` (wei, decimal) supplies it — a test
//!     INPUT, never persisted production data.
//!   * `ARBITRAGE_EXECUTOR` — optional; when unset, WETH9 stands in as the
//!     executor, so the verdict will be an execution-family failure. The
//!     INVARIANT under test is the ENCODING stage: the route-aware pipeline
//!     must produce real calldata topology, which the legacy RevmBackend
//!     cannot (`route_encoding_not_available` with `Vec::new()` calldata).
//!
//! ## Running
//! ```bash
//! export REVM_RPC_URL="https://eth-mainnet.g.alchemy.com/v2/<KEY>"
//! export DATABASE_URL="postgres://postgres:postgres@localhost:5432/arbitragex"
//! export GAS_PRICE_WEI_TEST="20000000000"   # 20 gwei (when no live Redis)
//! cargo test -p sim-ctl --test simwire02_route_aware -- --ignored --nocapture
//! ```
//!
//! The test exercises the REAL composition the stream consumer uses after
//! SIMWIRE-02 (`#[path]` includes of the production modules — no rewrites):
//! `route_lookup::fetch_candidate_inputs` → completeness gates →
//! `sim_runner::run_real_simulation` (the SAME encoder as the searcher) —
//! and asserts the outcome is an honest economic/market/typed-gap verdict,
//! NEVER the empty-calldata family.

#[path = "../src/route_lookup.rs"]
mod route_lookup;
#[path = "../src/sim_runner.rs"]
mod sim_runner;

use ethers::types::U256;
use sim_runner::RealSimEnvConfig;
use sqlx::postgres::PgPool;

/// Canonical mainnet fixtures (public constants, not operator data).
const WETH: &str = "0xC02AAA39b223FE8D0A0e5C4F27eAD9083C756Cc2"; // 18 decimals
const USDC: &str = "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"; // 6 decimals
/// UniswapV2 USDC/WETH 0.3% pair — real pool with real reserves on mainnet.
const USDC_WETH_V2_POOL: &str = "0xB4e16d0168e52d35CaCD2c6185b44281Ec28C9Dc";

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs live PG + REVM_RPC_URL (see module docs)"]
async fn stream_b2c_pipeline_is_route_aware_not_empty_calldata() {
    let rpc_url = std::env::var("REVM_RPC_URL").unwrap_or_default();
    if rpc_url.is_empty() {
        eprintln!("SKIP: REVM_RPC_URL not set — route-aware E2E needs a mainnet RPC");
        return;
    }
    let db_url = std::env::var("DATABASE_URL").unwrap_or_default();
    if db_url.is_empty() {
        eprintln!("SKIP: DATABASE_URL not set — route-aware E2E needs live PG with migrations");
        return;
    }

    // RealSimEnvConfig::from_env requires ARBITRAGE_EXECUTOR; a stand-in
    // executor is acceptable because the invariant is the ENCODING stage.
    if std::env::var("ARBITRAGE_EXECUTOR")
        .unwrap_or_default()
        .is_empty()
    {
        std::env::set_var("ARBITRAGE_EXECUTOR", WETH);
        eprintln!("NOTE: ARBITRAGE_EXECUTOR unset — standing in WETH9; expect an execution-family verdict");
    }
    let env_config = RealSimEnvConfig::from_env().expect("RealSimEnvConfig");

    let pool: PgPool =
        shared_rs::db_pool::options_with_timeouts(&shared_rs::db_pool::PoolConfig::from_env(2))
            .connect(&db_url)
            .await
            .expect("PG connect");

    // ---- Fixture: tokens (decimals resolution) + opportunity (route) ----
    sqlx::query(
        r#"
        INSERT INTO tokens (chain_id, address, symbol, decimals)
        VALUES (1, $1, 'WETH', 18), (1, $2, 'USDC', 6)
        ON CONFLICT (chain_id, address) DO NOTHING
        "#,
    )
    .bind(WETH.to_lowercase())
    .bind(USDC.to_lowercase())
    .execute(&pool)
    .await
    .expect("seed tokens");

    let route_metadata = serde_json::json!({
        "pool_addresses": [USDC_WETH_V2_POOL, USDC_WETH_V2_POOL],
        "token_addresses": [WETH, USDC, WETH],
        "dex_adapters": ["uniswap-v2", "uniswap-v2"],
        "decimals": {"map": {}}
    });
    let opp_id: uuid::Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO opportunities (
            chain_id, strategy_kind, dex_a, token_in, token_out,
            amount_in_wei, status, trace_id, route_metadata
        ) VALUES (1, 'dex_arb', 'uniswap-v2', $1, $2, $3, 'validated', gen_random_uuid(), $4)
        RETURNING id
        "#,
    )
    .bind(WETH)
    .bind(USDC)
    .bind(1_000_000_000_000_000_000i64) // 1 WETH — i64 binds as INT8, PG coerces to NUMERIC(78,0)
    .bind(sqlx::types::Json(&route_metadata)) // encodes as JSONB
    .fetch_one(&pool)
    .await
    .expect("seed opportunity");

    // Cleanup no matter how the assertions land.
    struct DropRow<'a>(&'a PgPool, uuid::Uuid);
    impl Drop for DropRow<'_> {
        fn drop(&mut self) {
            let pool = self.0.clone();
            let id = self.1;
            tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(async move {
                    let _ = sqlx::query("DELETE FROM opportunities WHERE id = $1")
                        .bind(id)
                        .execute(&pool)
                        .await;
                });
            });
        }
    }
    let _guard = DropRow(&pool, opp_id);

    // ---- 1) Canonical inputs (the consumer's exact source) ----
    let inputs = route_lookup::fetch_candidate_inputs(&pool, opp_id)
        .await
        .expect("fetch_candidate_inputs")
        .expect("fixture row must have populated route_metadata");
    assert_eq!(inputs.chain_id, 1);
    assert_eq!(inputs.route_metadata.token_addresses.len(), 3);
    assert!(
        inputs
            .resolved_decimals
            .validate_complete(&inputs.route_metadata.token_addresses)
            .is_ok(),
        "tokens-table decimals must resolve for the whole route"
    );

    // ---- 2) The consumer's candidate construction (same as simulate_b2c) ----
    let token_addresses = &inputs.route_metadata.token_addresses;
    let amount_in_wei: u128 = inputs.amount_in_wei.trim().parse().expect("wei parse");
    let decimals_in = inputs
        .resolved_decimals
        .get(&token_addresses[0])
        .expect("decimals_in");
    let amount_in = amount_in_wei as f64 / 10f64.powi(i32::from(decimals_in));
    assert!(amount_in > 0.0);
    let candidate = shared_rs::candidates::OpportunityCandidate {
        opportunity_id: opp_id,
        chain_id: inputs.chain_id as u64,
        token_addresses: token_addresses.clone(),
        pool_addresses: inputs.route_metadata.pool_addresses.clone(),
        dex_adapters: inputs.route_metadata.dex_adapters.clone(),
        amount_in,
        expected_amount_out: 0.0,
        gross_profit: 0.0,
        decimals: inputs.resolved_decimals.clone(),
        block_number: inputs.block_number.filter(|b| *b >= 0).map(|b| b as u64),
        route_fingerprint: format!("{}_{}_{}", inputs.dex_a, inputs.token_in, inputs.token_out),
    };

    // ---- 3) Gas: live Redis key first, documented test input otherwise ----
    let gas_price_wei = live_gas_price_or_test_input().await;

    // ---- 4) The REAL route-aware simulation ----
    let simulator = std::sync::Arc::new(simulator_v2::SimulatorV2::new(rpc_url));
    let outcome =
        sim_runner::run_real_simulation(candidate, simulator, &env_config, gas_price_wei).await;
    eprintln!(
        "SIMWIRE-E2E-01 outcome: passed={} fail_reason={:?} gas_used={} profit_token_in={} wrapped_calldata={}",
        outcome.passed,
        outcome.fail_reason,
        outcome.gas_used_total,
        outcome.simulated_profit_token_in,
        outcome.wrapped_calldata.as_ref().map(|b| b.len()).unwrap_or(0)
    );

    // ---- 5) THE INVARIANT: never the empty-calldata defect family ----
    let fr = outcome.fail_reason.clone().unwrap_or_default();
    assert!(
        !fr.starts_with("route_encoding_not_available"),
        "route-aware pipeline MUST NOT produce the legacy empty-calldata defect (got: {fr})"
    );
    assert!(
        !fr.starts_with("multistep_empty_calldata"),
        "encoder produced empty calldata for a complete 2-leg route (got: {fr})"
    );
    // Encoding happened ⇒ a fail verdict must be an honest execution/
    // economic/market/typed-gap reason, all of which are allowed here.
    if outcome.passed {
        assert!(
            outcome
                .wrapped_calldata
                .as_ref()
                .is_some_and(|b| !b.is_empty()),
            "a passing outcome must carry wrapped_calldata"
        );
    }
}

/// Gas price exactly like the consumer: live Redis key
/// (`gas_price_wei_key(chain)`, written by gas_oracle_worker), else the
/// documented `GAS_PRICE_WEI_TEST` input. Fails closed — never 0-by-default.
async fn live_gas_price_or_test_input() -> U256 {
    if let Ok(url) = std::env::var("REDIS_URL") {
        if let Ok(client) = redis::Client::open(url) {
            if let Ok(mut conn) = client.get_connection_manager().await {
                let key = shared_rs::pre_execute_checklist::gas_price_wei_key(1);
                let val: Option<String> = {
                    use redis::AsyncCommands;
                    conn.get(&key).await.ok().flatten()
                };
                if let Some(v) = val {
                    if let Ok(g) = v.parse::<U256>() {
                        if g > U256::zero() {
                            return g;
                        }
                    }
                }
            }
        }
    }
    std::env::var("GAS_PRICE_WEI_TEST")
        .ok()
        .and_then(|s| s.parse::<U256>().ok())
        .filter(|g| *g > U256::zero())
        .expect("no live gas in Redis and GAS_PRICE_WEI_TEST unset — refusing to fabricate gas_price_wei=0")
}
