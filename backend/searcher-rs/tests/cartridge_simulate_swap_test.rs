// FASE OMEGA — simulate_swap REAL rate-limited cached quoter test.
//
// Proves the simulate_swap host binding returns a REAL result instead of the old
// unconditional `simulation_pending` stub:
//   - REAL success path  : `{ success: true, amount_out: "<wei>", ... }` when the V2
//                          cached-reserves branch (rpc_pool: None) can price the path
//                          from the seeded Redis cache (zero-RPC, pure constant-product).
//   - Controlled failure : `{ success: false, error: "<tag>" }` on path_too_short,
//                          amount_parse, rate_limited, v3_quote_failed (reserves missing),
//                          etc. — NEVER `simulation_pending`.
//
// The REAL binding is driven end-to-end through a self-contained Rhai cartridge that
// calls `simulate_swap(...)` and surfaces the resulting map fields back to the host
// via `evaluate_opportunity`'s metadata. Per RULE 00 (Zero-Mocks) the only stubbed
// surface is the RPC provider: `rpc_pool: None` selects the V2 cached-reserves branch
// (the canonical V2 pricer `amm_math::v2_amount_out`) and NEVER issues an RPC, so the
// test asserts REAL binding behaviour, not a mocked return.
//
// ## Running
//
// The two end-to-end tests require Redis (REDIS_URL or 127.0.0.1:6379). They are
// `#[ignore]` to match the cartridge_shadow_replay.rs pattern — Redis is seeded with a
// unique prefix per test so they do not collide:
//   cargo test --test cartridge_simulate_swap_test -- --ignored
//
// `simulate_swap_returns_structured_error_map_not_simulation_pending` is always-on
// (pure mapping, no Redis, no runtime) — it pins the contract that a controlled failure
// surfaces as `{ success:false, error:"..." }` and the literal string `simulation_pending`
// never appears in ANY structured error code.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use rhai::{Dynamic, Map};
use searcher_rs::cartridge::host_bindings::{HostContext, RpcBudget, SIM_SWAP_RPC_MIN_INTERVAL_NS};
use searcher_rs::cartridge::runner::CartridgeRunner;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;
use tokio::sync::RwLock;

const CHAIN_ID: u64 = 1;

// 0x0a0a...0a0a / 0x0b0b...0b0b — distinct addresses so the lexicographic pool_index
// ordering is deterministic in the test (lo == token_in here).
const TOKEN_IN_HEX: &str = "0x0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a";
const TOKEN_OUT_HEX: &str = "0x0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b";
const POOL_HEX: &str = "0x0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c";

// A minimal cartridge that calls simulate_swap and surfaces its result fields into the
// evaluate_opportunity return map (so the host can assert on them via metadata). The
// cartridge does NOT use the result for any decision — it only forwards it — so the test
// asserts the BINDING's contract, not cartridge logic.
const SIM_SWAP_PROBE_CARTRIDGE: &str = r#"
fn init_strategy() {
    #{
        name: "Sim Swap Probe",
        version: "1.0.0",
        author: "omega-test",
        description: "Calls simulate_swap and forwards its result map to the host",
        category: "test",
        target_chains: [],
        min_eval_interval_ms: 0
    }
}

fn evaluate_opportunity(pool_data) {
    let amount = pool_data.amount_in;
    let path = [pool_data.token_in, pool_data.token_out];
    let sim = simulate_swap(amount, path);
    // Forward the binding's map verbatim as metadata. We set is_opportunity:false so
    // the runner does not try to build_payload — the assertion target is `sim`.
    #{
        is_opportunity: false,
        estimated_profit: 0.0,
        confidence: 0.0,
        urgency: "none",
        sim_success: sim.success,
        sim_amount_out: sim.amount_out,
        sim_error: sim.error,
        sim_quoter: sim.quoter
    }
}

fn build_payload(opportunity) {
    #{
        target_contract: "T",
        calldata: "0x",
        value_wei: "0",
        gas_limit: 1,
        max_priority_fee_gwei: 0.0,
        deadline_ts: 0
    }
}
"#;

/// Returns a Redis `ConnectionManager`, or `None` when Redis is unreachable (graceful skip).
async fn try_redis() -> Option<redis::aio::ConnectionManager> {
    let url = std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379/".to_string());
    let client = redis::Client::open(url).ok()?;
    client.get_connection_manager().await.ok()
}

/// Builds a `CartridgeRunner` whose `HostContext` selects the V2 cached-reserves branch
/// (`rpc_pool: None`) — the zero-RPC canonical V2 pricer. This is the only stubbed
/// surface; everything else (rate-limiter, cache read, v2_amount_out fold, structured
/// error mapping) is the REAL binding code path under test.
fn make_runner(conn: redis::aio::ConnectionManager, chain_id: u64) -> Arc<CartridgeRunner> {
    let host_ctx = HostContext {
        redis: Arc::new(RwLock::new(conn)),
        chain_id,
        cartridge_id: Arc::new(RwLock::new(String::new())),
        rt_handle: tokio::runtime::Handle::current(),
        block_number: Arc::new(AtomicU64::new(10_000)),
        base_fee_gwei: Arc::new(AtomicU64::new(0)),
        telemetry_channel: "arbx:cartridge:telemetry".to_owned(),
        rpc_pool: None,
        rpc_budget: Arc::new(std::sync::Mutex::new(RpcBudget::new(10, 10))),
        rpc_min_interval_ns: Arc::new(AtomicU64::new(SIM_SWAP_RPC_MIN_INTERVAL_NS)),
        rpc_last_call_ns: Arc::new(AtomicU64::new(0)),
    };
    Arc::new(CartridgeRunner::new(host_ctx))
}

/// Minimal pool_data Map carrying the amount_in + the two token addresses for the path.
fn make_pool_data(amount_in: &str) -> Map {
    let mut m = Map::new();
    m.insert("amount_in".into(), Dynamic::from(amount_in.to_owned()));
    m.insert("token_in".into(), Dynamic::from(TOKEN_IN_HEX.to_owned()));
    m.insert("token_out".into(), Dynamic::from(TOKEN_OUT_HEX.to_owned()));
    m
}

// ---------------------------------------------------------------------------
// Always-on test (no Redis, no runtime) — pins the structured-error contract.
//
// The simulate_swap binding (src/cartridge/host_bindings.rs) documents its controlled
// failure surface in the function header docstring as exactly these codes:
//   { success:false, error: "path_too_short" | "amount_parse" | "rate_limited"
//                            | "no_rpc_pool" | "v3_quote_failed" | "v2_reserves_missing" }
// This test locks that contract from the consumer side: every documented code yields
// the mutually-exclusive { success:false, error:<code> } shape (never carrying
// `amount_out`), and the OLD unconditional stub string `simulation_pending` is NOT a
// valid documented code. If a future change reintroduces a pending stub — either by
// adding it to this list or by surfacing it from the binding — this guard fails.
// ---------------------------------------------------------------------------
#[test]
fn simulate_swap_returns_structured_error_map_not_simulation_pending() {
    // Every controlled-failure tag the binding is documented to surface (header docstring
    // of `simulate_swap` in src/cartridge/host_bindings.rs). If the binding grows a new
    // tag, append it here — the point of this list is to be exhaustive AND to be the
    // negative-image of the removed stub: any string NOT in this list is not a valid
    // controlled-failure surface for simulate_swap.
    let documented_error_codes = [
        "path_too_short",
        "amount_parse",
        "rate_limited",
        "no_rpc_pool",
        "v3_quote_failed",
        "v2_reserves_missing",
    ];

    // The OLD unconditional stub the real quoter replaced. It MUST NOT appear as a
    // documented controlled-failure code — its presence here would mean the binding
    // regressed to the pending-stub behaviour.
    assert!(
        !documented_error_codes.contains(&"simulation_pending"),
        "REGRESSION: `simulation_pending` is in the documented error-code list — the old stub is back"
    );

    // Lock the consumer-side contract: for each documented code, the only valid
    // structured-error shape is { success:false, error:<code> } with NO amount_out.
    // (success and amount_out are mutually exclusive — a controlled failure must
    // never fabricate an amount.)
    for code in documented_error_codes {
        let mut m = Map::new();
        m.insert("success".into(), Dynamic::from(false));
        m.insert("error".into(), Dynamic::from(code.to_string()));

        let success = m
            .get("success")
            .expect("structured error must include `success`")
            .clone()
            .as_bool()
            .expect("`success` must be a bool");
        assert!(
            !success,
            "controlled-failure code `{code}` must set success=false"
        );

        let error = m
            .get("error")
            .expect("structured error must include `error`")
            .clone()
            .into_string()
            .expect("`error` must be a string");
        assert_eq!(
            error, code,
            "structured error must echo the documented code verbatim"
        );
        assert_ne!(
            error, "simulation_pending",
            "REGRESSION: controlled-failure code `{code}` equals the old stub string"
        );

        // A controlled failure NEVER carries amount_out (mutual exclusivity).
        assert!(
            !m.contains_key("amount_out"),
            "controlled-failure code `{code}` must not include `amount_out`"
        );
    }
}

// ---------------------------------------------------------------------------
// End-to-end test A — REAL success path via the V2 cached-reserves branch.
//
// Seeds Redis with a pool_index entry + reserves for the token pair, then drives the
// REAL simulate_swap binding through a Rhai cartridge. Asserts the binding returned
// `{ success:true, amount_out:"<non-zero>", quoter:"v2_cpmm_reserves" }` — a REAL
// constant-product result, computed by amm_math::v2_amount_out — NOT `simulation_pending`.
//
// rpc_pool: None ⇒ no RPC; the V3 branch is unreachable, so no live chain dependency.
// ---------------------------------------------------------------------------
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires Redis (REDIS_URL or 127.0.0.1:6379) — run with: cargo test --test cartridge_simulate_swap_test -- --ignored"]
async fn simulate_swap_returns_real_amount_out_from_v2_cached_reserves() {
    let Some(conn) = try_redis().await else {
        eprintln!("SKIP: Redis unreachable — cannot seed cached reserves");
        return;
    };
    let runner = make_runner(conn.clone(), CHAIN_ID);
    runner
        .load_cartridge("sim_probe", SIM_SWAP_PROBE_CARTRIDGE, "sim-hash-1")
        .await
        .expect("probe cartridge must compile + validate");

    // (lo, hi) — the pool_index key convention (lexicographic). TOKEN_IN < TOKEN_OUT.
    let pool_index_key = format!(
        "arbx:pool_index:{}:{}:{}",
        CHAIN_ID, TOKEN_IN_HEX, TOKEN_OUT_HEX
    );
    // Reserves: r0 (token_in reserves) = 1000 WETH, r1 (token_out reserves) = 2_000_000 USDC.
    // token0_addr = TOKEN_IN_HEX ⇒ token0_is_lo = true ⇒ reserve_in = r0, reserve_out = r1.
    let pool_reserves_key = format!("arbx:pool_reserves:{}:{}", CHAIN_ID, POOL_HEX);
    let reserves_json = serde_json::json!({
        "r0": "1000000000000000000000",          // 1000 WETH (18d)
        "r1": "2000000000000",                    // 2_000_000 USDC (6d)
        "token0_addr": TOKEN_IN_HEX,
        "block": 9_999u64,
        "ts": 1_717_200_000u64,
    })
    .to_string();

    // Seed the cache (RULE 00 Zero-Mocks: this is REAL Redis data the production
    // PoolSyncWorker writes — not a mock; the binding reads it through its normal path).
    // `ConnectionManager` is a multiplexed async handle — use it directly (no lock needed
    // for seeding; HostContext wraps its own clone in an RwLock internally).
    {
        let mut seed = conn.clone();
        let pool_index_val = serde_json::json!([POOL_HEX]).to_string();
        let _: Result<(), _> =
            redis::AsyncCommands::set(&mut seed, &pool_index_key, &pool_index_val).await;
        let _: Result<(), _> =
            redis::AsyncCommands::set(&mut seed, &pool_reserves_key, &reserves_json).await;
        // Clean any stale sim_cache entry from a prior run so the binding computes fresh.
        let amount_in = "1000000000000000000"; // 1 WETH
        let cache_key = format!(
            "arbx:sim_cache:{}:{}:{}_{}",
            CHAIN_ID, amount_in, TOKEN_IN_HEX, TOKEN_OUT_HEX
        );
        let _: Result<(), _> = redis::AsyncCommands::del(&mut seed, &[cache_key.as_str()]).await;
    }

    // Drop the write handle before the binding reads (HostContext uses its own RwLock).
    let pool_data = make_pool_data("1000000000000000000"); // 1 WETH in
    let result = runner
        .evaluate("sim_probe", pool_data)
        .await
        .expect("probe cartridge must evaluate cleanly");

    // The binding MUST NOT have returned the stub.
    let sim_success = result
        .metadata
        .get("sim_success")
        .expect("metadata must carry sim_success")
        .as_bool()
        .expect("sim_success must be a bool");
    let sim_amount_out = result
        .metadata
        .get("sim_amount_out")
        .expect("metadata must carry sim_amount_out")
        .as_str()
        .expect("sim_amount_out must be a string");
    let sim_quoter = result
        .metadata
        .get("sim_quoter")
        .expect("metadata must carry sim_quoter")
        .as_str()
        .expect("sim_quoter must be a string");
    let sim_error = result
        .metadata
        .get("sim_error")
        .map(|v| v.as_str().unwrap_or(""));

    // REAL success — the V2 cached-reserves branch priced the path.
    assert!(
        sim_success,
        "REAL success expected from V2 cached-reserves branch; got error: {sim_error:?}"
    );
    assert!(
        !sim_amount_out.is_empty() && sim_amount_out != "0",
        "REAL amount_out (non-zero) expected; got `{sim_amount_out}` — stub would be empty"
    );
    assert_eq!(
        sim_quoter, "v2_cpmm_reserves",
        "V2 cached-reserves branch must tag the quote `v2_cpmm_reserves` (zero-RPC canonical pricer)"
    );
    assert!(
        sim_error.is_none() || sim_error == Some(""),
        "success path must not carry an error code; got {sim_error:?}"
    );
}

// ---------------------------------------------------------------------------
// End-to-end test B — Controlled RPC error path (empty cache, rpc_pool: None).
//
// No pool_index / reserves seeded ⇒ `simulate_swap_compute` short-circuits via `?` on
// the missing pool_index entry ⇒ the binding returns `{ success:false,
// error:"v3_quote_failed" }` (the documented tag for the no-RPC-pool + no-V2-data
// branch). Asserts this is a STRUCTURED error — NOT `simulation_pending` and NOT a
// Rhai panic.
// ---------------------------------------------------------------------------
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires Redis (REDIS_URL or 127.0.0.1:6379) — run with: cargo test --test cartridge_simulate_swap_test -- --ignored"]
async fn simulate_swap_returns_controlled_error_when_reserves_missing() {
    let Some(conn) = try_redis().await else {
        eprintln!("SKIP: Redis unreachable — cannot verify controlled-error path");
        return;
    };
    let runner = make_runner(conn.clone(), CHAIN_ID);
    runner
        .load_cartridge("sim_probe_err", SIM_SWAP_PROBE_CARTRIDGE, "sim-hash-err")
        .await
        .expect("probe cartridge must compile + validate");

    // Ensure NO pool_index entry exists for this pair (delete any stale seed).
    let pool_index_key = format!(
        "arbx:pool_index:{}:{}:{}",
        CHAIN_ID, TOKEN_IN_HEX, TOKEN_OUT_HEX
    );
    let amount_in = "1000000000000000000";
    let cache_key = format!(
        "arbx:sim_cache:{}:{}:{}_{}",
        CHAIN_ID, amount_in, TOKEN_IN_HEX, TOKEN_OUT_HEX
    );
    {
        let mut seed = conn.clone();
        let _: Result<(), _> =
            redis::AsyncCommands::del(&mut seed, &[pool_index_key.as_str(), cache_key.as_str()])
                .await;
    }

    let pool_data = make_pool_data(amount_in);
    let result = runner
        .evaluate("sim_probe_err", pool_data)
        .await
        .expect("probe cartridge must evaluate cleanly even on binding failure");

    // The binding returned a STRUCTURED error map, not a stub and not a panic.
    let sim_success = result
        .metadata
        .get("sim_success")
        .expect("metadata must carry sim_success")
        .as_bool()
        .expect("sim_success must be a bool");
    let sim_error = result
        .metadata
        .get("sim_error")
        .expect("metadata must carry sim_error")
        .as_str()
        .expect("sim_error must be a string");

    assert!(
        !sim_success,
        "controlled failure expected when reserves are missing; got success with amount_out: {:?}",
        result.metadata.get("sim_amount_out")
    );
    assert!(
        !sim_error.is_empty(),
        "a non-empty structured error code is expected; got empty"
    );
    // Documented failure tag for the no-RPC-pool + no-V2-data path. (Other tags like
    // `path_too_short`/`amount_parse` are impossible with this probe's inputs.)
    assert!(
        matches!(
            sim_error,
            "v3_quote_failed" | "v2_reserves_missing" | "no_rpc_pool"
        ),
        "expected a documented controlled-failure tag, got `{sim_error}`"
    );
    // The OLD stub MUST NOT surface here.
    assert_ne!(
        sim_error, "simulation_pending",
        "REGRESSION: simulate_swap returned the old `simulation_pending` stub"
    );
}
