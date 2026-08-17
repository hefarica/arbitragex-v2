//! G-SIM-1 checklist item 4 — variance benchmark harness.
//!
//! Measures the drift between the simulator-v2 PREDICTED net profit and the
//! OBSERVED profit of the SAME wrapped-flash route one block later, across a
//! sample of REAL opportunities detected by the live scanner (replayed from
//! `opportunities.route_metadata`, never synthetic).
//!
//! THIS TEST IS `#[ignore]` BY DEFAULT — it requires external infrastructure:
//!   * `RPC_HTTP_1` (or `ALCHEMY_HTTP_URL`) — single bare mainnet RPC URL
//!     (same contract as `simulator-v2/tests/fork_mainnet.rs`).
//!   * `ARBITRAGE_EXECUTOR` — the deployed ArbitrageExecutor proxy (same env
//!     sim-ctl's real-sim path reads).
//!   * `FLASHLOAN_EXECUTOR_1` — the mainnet FlashLoanExecutor address
//!     (`resolve_flashloan_executor_address` reads it; fail-closed without it).
//!   * `GAS_PRICE_WEI` — live gas price (operator driver reads Redis
//!     `arbx:gas_price_wei:<chain>` — the same key sim-ctl's RevmBackend uses).
//!   * `VARIANCE_INPUT` — path to a JSONL export of REAL opportunities
//!     (see `scripts/gsim1_variance_export.sql`): one object per line with
//!     {opportunity_id, chain_id, detected_at_unix, token_in, token_out,
//!     dex_a, pool_addresses, token_addresses, dex_adapters, amount_in_wei}.
//!   * `VARIANCE_MIN_SAMPLES` (default 100) — the checklist minimum.
//!   * `VARIANCE_MAX_MEAN_DRIFT_PCT` (default 5.0) — the checklist threshold.
//!
//! ## Method (honest scope, paper-shadow doctrine)
//!
//! For every exported opportunity the harness runs the PRODUCTION
//! `sim_core::sim_multistep::execute_multistep_revm` path twice against real
//! mainnet state via `LazyDb`:
//!   * PREDICTED  — pinned at block B, the block whose timestamp contains
//!     `detected_at` (resolved by timestamp bisection against the RPC — the
//!     opportunities table does not persist `block_number`).
//!   * OBSERVED   — pinned at block B+1, the settled block the route would
//!     have landed in (the same semantics `recon::drift_tracker` defines for
//!     `paper_trade_runs.actual_*`).
//!
//! A pair is LABELABLE only when BOTH executions complete (`passed=true` in
//! measurement mode — `require_positive_net_profit=false`, documented: the
//! benchmark measures prediction error on every completable route, profitable
//! or not). Everything else is counted and reported as skipped; nothing is
//! imputed (RULE 00 / R8).
//!
//! NOTE on "observed on-chain execution profit": the system is paper-shadow —
//! NO broadcast ever happens (§32). The observed leg is the identical
//! wrapped-flash REVM execution against real mainnet fork state at B+1. This
//! is the closest observable ground truth the doctrine permits and is recorded
//! verbatim in the evidence detail (`method: "revm_b_vs_revm_b1_fork"`).
//!
//! ## Anti-hollow-pass contract
//!
//! Emits exactly one machine-greppable `VARIANCE_BENCH_OUTCOME=PASS|FAIL` line
//! plus one `VARIANCE_BENCH_JSON={...}` line with the full histogram. The
//! operator driver (`scripts/gsim1_variance_benchmark.sh`) POSTs
//! evidenced/failed to the readiness_evidence registry based on the marker —
//! never on the test exit status alone. Missing env PANICS (fail-honest).
//!
//! DOCTRINE: read-only. NO signing, NO broadcast, NO capital. Committed REVM
//! calls mutate only the in-memory CacheDB of this process.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use ethers::prelude::{Http, Middleware, Provider};
use ethers::types::transaction::eip2718::TypedTransaction;
use ethers::types::{BlockId, BlockNumber, TransactionRequest, H256};
use ethers::utils::keccak256;
use sim_core::sim_encoder::{
    build_round_trip_context_from_candidate, RouteEncodingConfig, TokenDecimalsProvider,
};
use sim_core::sim_multistep::{execute_multistep_revm, MultiStepExecutionConfig};
use simulator_v2::SimulatorV2;

/// One exported row (source of truth: `opportunities` + `route_metadata`).
#[derive(Debug, serde::Deserialize)]
struct ExportRow {
    opportunity_id: String,
    chain_id: u64,
    detected_at_unix: u64,
    token_in: String,
    token_out: String,
    dex_a: String,
    pool_addresses: Vec<String>,
    token_addresses: Vec<String>,
    dex_adapters: Vec<String>,
    amount_in_wei: String,
}

/// Per-run histogram of every skip reason (R8: the summary IS the evidence).
#[derive(Debug, Default)]
struct Histogram {
    attempted: usize,
    dedup: usize,
    unsupported_adapter: usize,
    bad_shape: usize,
    stale_timestamp: usize,
    decimals_failed: usize,
    encode_failed: usize,
    pred_failed: usize,
    obs_failed: usize,
    zero_predicted: usize,
    labeled: usize,
}

/// route_metadata carries display names; the encoder accepts semantic labels
/// (`parse_dex_kind`: "uniswap-v2" | "sushi"). V3 legs are honestly
/// unsupported by the A.3.a encoder (needs per-leg fee tier) → None.
fn adapter_to_semantic(label: &str) -> Option<&'static str> {
    match label.trim() {
        "UniswapV2" | "uniswap-v2" | "uniswapv2" => Some("uniswap-v2"),
        "SushiSwap" | "sushi" | "sushiswap" => Some("sushi"),
        _ => None,
    }
}

/// Deterministic route_hash from the route topology (same scheme as
/// sim-ctl's `route_hash_from_fingerprint`; fingerprint format mirrors
/// scanner.rs `{dex_a}_{token_in}_{token_out}`).
fn route_hash(row: &ExportRow, forward: &str, backward: &str) -> [u8; 32] {
    let fingerprint = format!("{}|{}|{}|{}", row.dex_a, forward, backward, row.token_out);
    keccak256(fingerprint.as_bytes())
}

/// U256-ish (wei string) → f64 whole tokens.
fn wei_to_f64_tokens(wei: &str, decimals: u8) -> Option<f64> {
    let raw: f64 = wei.parse().ok()?;
    Some(raw / 10f64.powi(i32::from(decimals)))
}

/// Profit wei (U256) → f64 for ratio math.
fn u256_to_f64(v: &ethers::types::U256) -> f64 {
    if v.bits() <= 128 {
        v.as_u128() as f64
    } else {
        v.to_string().parse().unwrap_or(f64::MAX)
    }
}

/// Decimals provider backed by the harness's RPC-resolved map (real on-chain
/// `decimals()` calls — the exported route_metadata decimals map is empty).
#[derive(Clone, Default)]
struct RpcDecimalsProvider {
    chain_id: u64,
    map: HashMap<String, u8>,
}

impl TokenDecimalsProvider for RpcDecimalsProvider {
    fn decimals(&self, chain_id: u64, token: &ethers::types::Address) -> Option<u8> {
        if chain_id != self.chain_id {
            return None;
        }
        self.map.get(&format!("{token:?}").to_lowercase()).copied()
    }
}

/// eth_call with bounded retry (public RPCs throttle; honest error, no
/// fabrication on repeated failure).
async fn call_with_retry(
    provider: &Provider<Http>,
    req: &TypedTransaction,
) -> Result<ethers::types::Bytes, String> {
    let mut last = String::from("no attempt");
    for attempt in 0..4u32 {
        if attempt > 0 {
            tokio::time::sleep(Duration::from_millis(400 * u64::from(attempt))).await;
        }
        match provider.call(req, None).await {
            Ok(b) => return Ok(b),
            Err(e) => last = e.to_string(),
        }
    }
    Err(last)
}

/// ERC-20 `decimals()` static call (selector 0x313ce567).
async fn token_decimals(
    provider: &Provider<Http>,
    token: &ethers::types::Address,
) -> Result<u8, String> {
    let req: TypedTransaction = TransactionRequest::new()
        .to(*token)
        .data(ethers::types::Bytes::from_static(&[0x31, 0x3c, 0xe5, 0x67]))
        .into();
    let out = call_with_retry(provider, &req).await?;
    let last = out.last().ok_or("empty decimals() response")?;
    Ok(*last)
}

/// Block timestamp with a local cache (bisection revisits blocks).
async fn block_ts(
    provider: &Provider<Http>,
    cache: &mut HashMap<u64, u64>,
    block: u64,
) -> Result<u64, String> {
    if let Some(ts) = cache.get(&block) {
        return Ok(*ts);
    }
    for attempt in 0..4u32 {
        if attempt > 0 {
            tokio::time::sleep(Duration::from_millis(400 * u64::from(attempt))).await;
        }
        if let Ok(Some(b)) = provider
            .get_block(BlockId::Number(BlockNumber::Number(block.into())))
            .await
        {
            let ts = b.timestamp.as_u64();
            cache.insert(block, ts);
            return Ok(ts);
        }
    }
    Err(format!(
        "eth_getBlockByNumber({block}) failed after retries"
    ))
}

/// Smallest block whose timestamp >= target, searched inside [lo, hi]
/// (timestamps are monotonic). Assumes ts(lo) < target (caller guarantees via
/// the window construction).
async fn block_for_ts(
    provider: &Provider<Http>,
    cache: &mut HashMap<u64, u64>,
    target: u64,
    lo: u64,
    hi: u64,
) -> Result<u64, String> {
    let (mut lo, mut hi) = (lo, hi);
    if lo >= hi {
        return Ok(hi);
    }
    while lo < hi {
        let mid = lo + (hi - lo) / 2;
        if block_ts(provider, cache, mid).await? < target {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }
    Ok(lo)
}

fn env_required(key: &str) -> String {
    match std::env::var(key) {
        Ok(v) if !v.trim().is_empty() => v.trim().to_owned(),
        _ => panic!(
            "variance benchmark requires {key} (see the module docs of this test); \
             refusing to run with a hollow sample"
        ),
    }
}

/// Run the production multi-step REVM sim pinned at `block`; returns the
/// outcome (never panics on sim failure — failure is data).
fn sim_at_block(
    rpc: &str,
    block: u64,
    ctx: &prioritization_spine::round_trip_executor::RoundTripContext,
    cfg: &MultiStepExecutionConfig,
) -> prioritization_spine::round_trip_executor::SimulationOutcome {
    let simulator = Arc::new(SimulatorV2::new(rpc).with_block(block));
    execute_multistep_revm(ctx, simulator, cfg)
}

// ---------------------------------------------------------------------------
// Benchmark (ignored)
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
#[ignore = "requires RPC_HTTP_1 + ARBITRAGE_EXECUTOR + FLASHLOAN_EXECUTOR_1 + GAS_PRICE_WEI + VARIANCE_INPUT (JSONL of real opportunities) — see module docs"]
async fn variance_benchmark_predicted_vs_settled_block() {
    let rpc = match std::env::var("RPC_HTTP_1") {
        Ok(v) if !v.trim().is_empty() => v.trim().to_owned(),
        _ => match std::env::var("ALCHEMY_HTTP_URL") {
            Ok(v) if !v.trim().is_empty() => v.trim().to_owned(),
            _ => panic!("set RPC_HTTP_1 (or ALCHEMY_HTTP_URL) to a single bare mainnet RPC URL"),
        },
    };
    let executor = ethers::types::Address::from_str(&env_required("ARBITRAGE_EXECUTOR"))
        .expect("ARBITRAGE_EXECUTOR must be a 0x-prefixed address");
    // execute_multistep_revm resolves this internally; fail EARLY + explicit
    // here so the operator sees the missing var before minutes of RPC work.
    env_required("FLASHLOAN_EXECUTOR_1");
    let gas_price_wei = ethers::types::U256::from_dec_str(&env_required("GAS_PRICE_WEI"))
        .expect("GAS_PRICE_WEI must be a decimal wei value");
    let input_path = env_required("VARIANCE_INPUT");
    let min_samples: usize = std::env::var("VARIANCE_MIN_SAMPLES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(100);
    let max_mean_drift: f64 = std::env::var("VARIANCE_MAX_MEAN_DRIFT_PCT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(5.0);

    let raw = std::fs::read_to_string(&input_path)
        .unwrap_or_else(|e| panic!("VARIANCE_INPUT {input_path:?} unreadable: {e}"));
    let mut rows: Vec<ExportRow> = raw
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            serde_json::from_str(l)
                .unwrap_or_else(|e| panic!("malformed JSONL line in {input_path}: {e}\nline: {l}"))
        })
        .collect();
    // Freshest first; dedup identical route topologies (a single pool pair
    // spamming the scanner must not dominate the sample).
    rows.sort_by(|a, b| b.detected_at_unix.cmp(&a.detected_at_unix));
    let mut seen_routes: std::collections::HashSet<String> = std::collections::HashSet::new();

    let provider =
        Provider::<Http>::try_from(rpc.as_str()).expect("RPC URL must parse as an HTTP endpoint");
    let tip = provider
        .get_block_number()
        .await
        .expect("eth_blockNumber against the provided RPC failed")
        .as_u64();
    // Detection window: 3h of blocks (12s target) with variance margin.
    let window_lo = tip.saturating_sub(1_100);
    let mut ts_cache: HashMap<u64, u64> = HashMap::new();
    let mut decimals_cache: RpcDecimalsProvider = RpcDecimalsProvider {
        chain_id: 1,
        map: HashMap::new(),
    };

    let mut hist = Histogram {
        attempted: rows.len(),
        ..Default::default()
    };
    let mut drifts_abs: Vec<f64> = Vec::new();
    let mut first_pred_block: Option<u64> = None;
    let mut last_pred_block: Option<u64> = None;

    let now_unix_ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    'rows: for row in &rows {
        let route_key = format!(
            "{}|{}|{}|{:?}",
            row.dex_a, row.token_in, row.token_out, row.pool_addresses
        );
        if !seen_routes.insert(route_key) {
            hist.dedup += 1;
            continue;
        }
        if row.chain_id != 1 {
            hist.unsupported_adapter += 1; // non-mainnet export rows: out of scope
            continue;
        }
        if row.dex_adapters.len() != 2 || row.token_addresses.len() != 3 {
            hist.bad_shape += 1;
            continue;
        }
        let forward = match adapter_to_semantic(&row.dex_adapters[0]) {
            Some(s) => s,
            None => {
                hist.unsupported_adapter += 1;
                continue;
            }
        };
        let backward = match adapter_to_semantic(&row.dex_adapters[1]) {
            Some(s) => s,
            None => {
                hist.unsupported_adapter += 1;
                continue;
            }
        };

        // Resolve block B by timestamp bisection inside the freshness window.
        let block_b = match block_for_ts(
            &provider,
            &mut ts_cache,
            row.detected_at_unix,
            window_lo,
            tip,
        )
        .await
        {
            Ok(b) => b,
            Err(e) => {
                eprintln!("skip {} block-resolve: {e}", row.opportunity_id);
                hist.stale_timestamp += 1;
                continue;
            }
        };
        if block_b <= window_lo || block_b + 1 > tip {
            // Detection older than the window (or settled block not yet mined).
            hist.stale_timestamp += 1;
            continue;
        }

        // Real on-chain decimals for every token on the path.
        for addr in &row.token_addresses {
            let key = addr.to_lowercase();
            if decimals_cache.map.contains_key(&key) {
                continue;
            }
            let parsed = ethers::types::Address::from_str(addr).ok();
            let dec = match parsed {
                Some(a) => token_decimals(&provider, &a).await,
                None => Err(format!("unparseable token address {addr}")),
            };
            match dec {
                Ok(d) => {
                    decimals_cache.map.insert(key, d);
                }
                Err(e) => {
                    eprintln!("skip {} decimals({addr}): {e}", row.opportunity_id);
                    hist.decimals_failed += 1;
                    continue 'rows;
                }
            }
        }
        let dec_in = *decimals_cache
            .map
            .get(&row.token_addresses[0].to_lowercase())
            .expect("token_in decimals resolved above");

        let amount_in = match wei_to_f64_tokens(&row.amount_in_wei, dec_in) {
            Some(a) if a.is_finite() && a > 0.0 => a,
            _ => {
                hist.bad_shape += 1;
                continue;
            }
        };

        // Build the SAME candidate the production path encodes (7 fields,
        // mirroring sim-ctl sim_runner::to_spine_candidate).
        let spine_candidate = prioritization_spine::types::OpportunityCandidate {
            route_fingerprint: format!("{}_{}_{}", row.dex_a, row.token_in, row.token_out),
            pool_addresses: row.pool_addresses.clone(),
            token_addresses: row.token_addresses.clone(),
            dex_adapters: vec![forward.to_string(), backward.to_string()],
            amount_in,
            // The multistep path re-quotes the real intermediate on-chain
            // (read_amounts_out); these advisory fields are not the measured
            // signal — the measured profits come from the REVM executions.
            expected_amount_out: amount_in,
            gross_profit: 0.0,
        };
        let encode_config = RouteEncodingConfig {
            deadline_seconds: 300,
            now_unix_ts,
            min_profit_wei: ethers::types::U256::one(),
        };
        let ctx = match build_round_trip_context_from_candidate(
            &spine_candidate,
            row.chain_id,
            executor,
            &decimals_cache,
            &encode_config,
        ) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("skip {} encode: {e}", row.opportunity_id);
                hist.encode_failed += 1;
                continue;
            }
        };

        let exec_cfg = MultiStepExecutionConfig {
            chain_id: row.chain_id,
            executor_address: executor,
            route_hash: route_hash(row, forward, backward),
            min_profit_wei: ethers::types::U256::one(),
            gas_price_wei,
            gas_limit_per_step: 500_000,
            paper_mode: true,
            enable_storage_cheats: true,
            require_trace_hash: true,
            // Measurement mode: label completable routes regardless of
            // profitability — the benchmark measures prediction error, not
            // market viability (documented in the evidence detail).
            require_positive_net_profit: false,
            max_steps: 8,
        };

        let pred = sim_at_block(&rpc, block_b, &ctx, &exec_cfg);
        if !pred.passed {
            hist.pred_failed += 1;
            continue;
        }
        let obs = sim_at_block(&rpc, block_b + 1, &ctx, &exec_cfg);
        if !obs.passed {
            hist.obs_failed += 1;
            continue;
        }

        let pred_f = u256_to_f64(&pred.simulated_profit_token_in);
        let obs_f = u256_to_f64(&obs.simulated_profit_token_in);
        if pred_f <= 0.0 {
            // Zero predicted profit → ratio undefined → unlabeled (honest).
            hist.zero_predicted += 1;
            continue;
        }
        if first_pred_block.is_none() {
            first_pred_block = Some(block_b);
        }
        last_pred_block = Some(block_b);
        drifts_abs.push(((obs_f - pred_f) / pred_f * 100.0).abs());
        hist.labeled += 1;
    }

    drifts_abs.sort_by(|a, b| a.partial_cmp(b).expect("finite drifts"));
    let mean_abs = drifts_abs.iter().sum::<f64>() / drifts_abs.len().max(1) as f64;
    let p95 = drifts_abs
        .get((drifts_abs.len().saturating_sub(1)) * 95 / 100)
        .copied()
        .unwrap_or(f64::NAN);
    let max = drifts_abs.last().copied().unwrap_or(f64::NAN);

    let pass = hist.labeled >= min_samples && mean_abs < max_mean_drift;
    let outcome = if pass { "PASS" } else { "FAIL" };

    let stats = serde_json::json!({
        "method": "revm_b_vs_revm_b1_fork",
        "simulator": "simulator-v2 multi-step REVM (sim_core::sim_multistep::execute_multistep_revm)",
        "rpc": rpc,
        "tip_block": tip,
        "block_window": [first_pred_block, last_pred_block],
        "samples_labeled": hist.labeled,
        "min_samples_required": min_samples,
        "mean_abs_drift_pct": (mean_abs * 10_000.0).round() / 10_000.0,
        "p95_abs_drift_pct": (p95 * 10_000.0).round() / 10_000.0,
        "max_abs_drift_pct": (max * 10_000.0).round() / 10_000.0,
        "threshold_mean_pct": max_mean_drift,
        "skips": {
            "attempted": hist.attempted,
            "dedup": hist.dedup,
            "unsupported_adapter": hist.unsupported_adapter,
            "bad_shape": hist.bad_shape,
            "stale_timestamp": hist.stale_timestamp,
            "decimals_failed": hist.decimals_failed,
            "encode_failed": hist.encode_failed,
            "pred_failed": hist.pred_failed,
            "obs_failed": hist.obs_failed,
            "zero_predicted": hist.zero_predicted,
        },
    });

    println!(
        "VARIANCE_BENCH_OUTCOME={outcome} samples={} mean_abs_drift_pct={:.4} p95={:.4} max={:.4} (min_samples={min_samples}, threshold={max_mean_drift}%)",
        hist.labeled, mean_abs, p95, max
    );
    println!("VARIANCE_BENCH_JSON={stats}");
}

// ---------------------------------------------------------------------------
// Smoke tests (NOT ignored) — pure helpers, no RPC/env access, so the binary
// stays honest (>= 1 executed test) in RPC-less CI runs.
// ---------------------------------------------------------------------------

#[test]
fn adapter_mapping_is_truthful_about_supported_dexes() {
    assert_eq!(adapter_to_semantic("UniswapV2"), Some("uniswap-v2"));
    assert_eq!(adapter_to_semantic("SushiSwap"), Some("sushi"));
    assert_eq!(adapter_to_semantic("uniswap-v2"), Some("uniswap-v2"));
    // V3 legs need a per-leg fee tier the A.3.a encoder does not carry —
    // honestly unsupported, never silently coerced to a V2 router.
    assert_eq!(adapter_to_semantic("UniswapV3"), None);
    assert_eq!(adapter_to_semantic("Curve"), None);
}

#[test]
fn wei_to_tokens_converts_with_decimals() {
    assert_eq!(wei_to_f64_tokens("1000000", 6), Some(1.0));
    assert_eq!(wei_to_f64_tokens("1000000000000000000", 18), Some(1.0));
    assert_eq!(wei_to_f64_tokens("0", 18), Some(0.0));
    assert!(wei_to_f64_tokens("not-a-number", 18).is_none());
}

#[test]
fn route_hash_is_deterministic() {
    let row = ExportRow {
        opportunity_id: "x".into(),
        chain_id: 1,
        detected_at_unix: 0,
        token_in: "0xa".into(),
        token_out: "0xc".into(),
        dex_a: "UniswapV2".into(),
        pool_addresses: vec!["0xp".into()],
        token_addresses: vec!["0xa".into(), "0xb".into(), "0xc".into()],
        dex_adapters: vec!["UniswapV2".into(), "SushiSwap".into()],
        amount_in_wei: "1".into(),
    };
    let h1 = route_hash(&row, "uniswap-v2", "sushi");
    let h2 = route_hash(&row, "uniswap-v2", "sushi");
    assert_eq!(h1, h2);
    assert_ne!(h1, [0u8; 32]);
    assert_ne!(H256::from(h1), H256::zero());
}
