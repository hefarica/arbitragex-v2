//! token-enricher — Task 8: main loop.
//!
//! Boot flow:
//!   1. Read config from env vars (fail-fast if REQUIRED vars missing).
//!   2. Init structured tracing (JSON for VPS, pretty for dev).
//!   3. Init Prometheus metrics (force Lazy init).
//!   4. Connect PostgreSQL pool and Redis.
//!   5. Drain PEL (I1 — pending entries from a previous crashed session).
//!   6. Process PEL batch (same pipeline as live entries).
//!   7. Enter main `tokio::select!` loop:
//!      a. Redis consumer tick (live entries from `arbx:opps:detected`).
//!      b. Reconciliation tick (every 5 minutes, pulls unresolved from PG).
//!      c. Graceful shutdown on SIGTERM (Linux) or ctrl-c (cross-platform).
//!   8. Serve `/metrics` on ENRICHER_METRICS_PORT (default 9004) via a
//!      background tokio task (does not block the main loop).
//!
//! ## Environment variables
//!
//! | Variable                | Required | Default  | Description                                          |
//! |-------------------------|----------|----------|------------------------------------------------------|
//! | `DATABASE_URL`          | YES      | —        | PostgreSQL DSN                                       |
//! | `REDIS_URL`             | YES      | —        | Redis DSN (redis://<host>:<port>)                    |
//! | `ENRICHER_CHAINS`       | YES      | —        | Comma-separated chain IDs to enrich (e.g. 1,137,42161) |
//! | `RPC_HTTP_<chain_id>`   | YES*     | —        | HTTP RPC URL(s) for each enabled chain.              |
//! |                         |          |          | Format: `name=url,name=url` (multi-vendor) or bare   |
//! |                         |          |          | URL. One env var per chain. Required if that chain   |
//! |                         |          |          | appears in ENRICHER_CHAINS; missing = warn + skip.   |
//! | `ENRICHER_METRICS_PORT` | NO       | 9004     | Prometheus /metrics port                             |
//! | `RUST_LOG`              | NO       | info     | tracing-subscriber filter                            |
//! | `GITHUB_TOKEN`          | NO       | —        | Bearer token for TrustWallet CDN                     |
//!
//! ## Migration note (BREAKING — BE-02 Step 4)
//!
//! The `ENRICHER_RPC_URLS` env var has been REMOVED. The canonical per-chain
//! env pattern is now used exclusively:
//!
//!   Old: `ENRICHER_RPC_URLS=1=https://eth.g.alchemy.com/v2/KEY,137=https://poly.g.alchemy.com/v2/KEY`
//!   New: `RPC_HTTP_1=https://eth.g.alchemy.com/v2/KEY`
//!        `RPC_HTTP_137=https://poly.g.alchemy.com/v2/KEY`
//!        `ENRICHER_CHAINS=1,137`
//!
//! Operator action: update VPS .env to split ENRICHER_RPC_URLS into per-chain
//! `RPC_HTTP_<id>` variables and add `ENRICHER_CHAINS`.
//!
//! ## Invariants (I2 — dedup gate)
//!
//! `process_batch` is the SINGLE deduplication gate for resolution work.
//! Both the Redis consumer path and the reconciliation path funnel through it.
//! No address is sent to on-chain RPC without first passing `needs_resolution`.
//!
//! ## R8 fail-honest
//!
//! Every resolution failure is recorded as `resolved_via = 'failed'` with NULL
//! symbol/decimals/logo.  The enricher NEVER synthesises or guesses values.

use alloy::providers::Provider;
use alloy::rpc::types::{TransactionInput, TransactionRequest};
use alloy_primitives::{Address, Bytes};
use alloy_sol_types::SolCall;
use anyhow::{Context, Result};
use futures_util::future::join_all;
use shared_rs::rpc_failover::HttpRpcPool;
use sqlx::postgres::PgPoolOptions;
use std::{collections::HashMap, collections::HashSet, str::FromStr, sync::Arc, time::Duration};
use tracing::{error, info, warn};

use token_enricher::{
    consumer::EnricherConsumer,
    metrics::{
        init_metrics, serve_metrics, BATCHES_TOTAL, ENRICHER_UP, PENDING_UNRESOLVED,
        RPC_CALLS_TOTAL, TOKENS_RESOLVED_TOTAL,
    },
    multicall::{build_calls_for, pair_results, IMulticall3, MULTICALL3_ADDRESS},
    persistence::{needs_resolution, upsert_token, ResolvedToken},
    reconciliation::find_unresolved_tokens,
    trustwallet::TrustWalletClient,
};

/// How many addresses to resolve per multicall batch.
const BATCH_SIZE: usize = 50;
/// How many unresolved tokens to pull per reconciliation tick.
const RECON_LIMIT: i64 = 200;
/// Reconciliation tick interval.
const RECON_INTERVAL_SECS: u64 = 300; // 5 minutes

// ---------------------------------------------------------------------------
// Config from env
// ---------------------------------------------------------------------------

fn require_env(key: &str) -> Result<String> {
    std::env::var(key).with_context(|| format!("required env var {key} is not set"))
}

/// Parse `ENRICHER_CHAINS` — format: `1,137,42161`
///
/// R8 fail-honest: if the var is absent or empty we return Err so the service
/// exits cleanly with a clear log rather than silently enriching nothing.
fn parse_enricher_chains(raw: &str) -> Result<Vec<u64>> {
    let mut chains = Vec::new();
    for segment in raw.split(',') {
        let segment = segment.trim();
        if segment.is_empty() {
            continue;
        }
        let chain: u64 = segment
            .parse()
            .with_context(|| format!("ENRICHER_CHAINS value is not a valid u64: {segment:?}"))?;
        chains.push(chain);
    }
    if chains.is_empty() {
        anyhow::bail!(
            "ENRICHER_CHAINS is set but contains no valid chain IDs. \
             Example: ENRICHER_CHAINS=1,137,42161"
        );
    }
    Ok(chains)
}

// ---------------------------------------------------------------------------
// Pool construction (replaces parse_rpc_urls + build_providers)
// ---------------------------------------------------------------------------

/// Build one `HttpRpcPool` per chain from `RPC_HTTP_<chain_id>` env vars.
///
/// Chains listed in `enabled_chains` that have no `RPC_HTTP_<chain_id>` set
/// are logged as warnings and skipped (not fatal — the operator may want to
/// enable a chain incrementally). If NO chain produces a valid pool, we return
/// an error so the service fails on boot rather than silently doing nothing.
///
/// Each pool spawns a background health-loop task (detached via `spawn_health_loop`).
/// The loop fires every 15s and drives EWMA latency, drift detection, and the
/// per-provider circuit breaker (5 errors / 60s → Open for 30s).
async fn build_pools(enabled_chains: &[u64]) -> Result<HashMap<u64, Arc<HttpRpcPool>>> {
    let mut pools = HashMap::new();
    for &chain_id in enabled_chains {
        match HttpRpcPool::from_env(chain_id).await {
            Ok(Some(pool)) => {
                let pool = Arc::new(pool);
                // Detach the health-loop task. The JoinHandle is intentionally
                // dropped — the task runs for the process lifetime and we rely
                // on process exit to stop it.  Aborting on drop is fine here
                // because the health loop holds no persistent resources.
                let _health = pool.spawn_health_loop();
                pools.insert(chain_id, pool);
            }
            Ok(None) => {
                warn!(
                    event = "token_enricher.rpc_pool_absent",
                    chain_id,
                    env_key = format!("RPC_HTTP_{chain_id}"),
                    "env var not set for this chain — skipping; add RPC_HTTP_{chain_id} to .env to enable enrichment",
                );
            }
            Err(e) => {
                error!(
                    event = "token_enricher.rpc_pool_invalid",
                    chain_id,
                    error = %e,
                    "RPC_HTTP_{chain_id} is set but all providers failed validation — skipping chain",
                );
            }
        }
    }
    if pools.is_empty() {
        anyhow::bail!(
            "No valid RPC pools could be built for any chain in ENRICHER_CHAINS. \
             Set RPC_HTTP_<chain_id> for at least one chain."
        );
    }
    info!(
        event = "token_enricher.pools_built",
        chain_count = pools.len(),
        chains = ?pools.keys().collect::<Vec<_>>(),
    );
    Ok(pools)
}

// ---------------------------------------------------------------------------
// Core resolution pipeline
// ---------------------------------------------------------------------------

/// Deduplicate and resolve a batch of `(chain_id, address)` pairs.
///
/// THIS IS THE SINGLE DEDUP GATE (I2).  No RPC call is issued unless
/// `needs_resolution` returns `true`.  Both the consumer path and the
/// reconciliation path MUST funnel through this function.
///
/// Steps:
/// 1. `needs_resolution` filter → group per chain.
/// 2. Build multicall calldata per chain.
/// 3. `eth_call` via `HttpRpcPool::with_retry` — EWMA-ranked failover + circuit
///    breaker activated (replaces single-provider `DynProvider::call`).
/// 4. `tw.verify` in parallel per address via `join_all` (Defect #2).
/// 5. Classify `resolved_via` and `upsert_token` sequentially.
async fn process_batch(
    pool: &sqlx::PgPool,
    rpc_pools: &HashMap<u64, Arc<HttpRpcPool>>,
    tw: &TrustWalletClient,
    pairs: &[(u64, Address)],
    source: &str, // "consumer" | "reconciliation" | "pel"
) -> Result<()> {
    if pairs.is_empty() {
        return Ok(());
    }

    BATCHES_TOTAL.with_label_values(&[source]).inc();

    // --- Step 1: dedup gate ---
    let mut per_chain: HashMap<u64, Vec<Address>> = HashMap::new();
    for &(chain_id, addr) in pairs {
        match needs_resolution(pool, chain_id, addr).await {
            Ok(true) => {
                per_chain.entry(chain_id).or_default().push(addr);
            }
            Ok(false) => {} // already resolved, skip
            Err(e) => {
                warn!(
                    event = "enricher.needs_resolution_err",
                    chain = chain_id,
                    addr = %addr,
                    err = %e
                );
                // Fail-honest: skip this address, don't resolve with stale data.
            }
        }
    }

    if per_chain.is_empty() {
        return Ok(());
    }

    let multicall3_addr: Address =
        Address::from_str(MULTICALL3_ADDRESS).context("parse MULTICALL3_ADDRESS")?;

    for (chain_id, addrs) in &per_chain {
        // Look up the HttpRpcPool for this chain (replaces providers.get).
        let rpc_pool = match rpc_pools.get(chain_id) {
            Some(p) => p,
            None => {
                warn!(event = "enricher.no_provider", chain = chain_id);
                for &addr in addrs {
                    let _ = upsert_token(
                        pool,
                        *chain_id,
                        addr,
                        ResolvedToken {
                            symbol: None,
                            decimals: None,
                            logo_url: None,
                            resolved_via: "failed",
                        },
                    )
                    .await;
                }
                continue;
            }
        };

        // --- Step 2: build multicall calldata ---
        let calls = build_calls_for(addrs);
        let calldata = IMulticall3::aggregate3Call { calls }.abi_encode();
        // Build once; the closure below may call the provider twice (retry path),
        // so `tx` must be cloneable. `TransactionRequest` derives Clone in alloy 1.x.
        let tx = TransactionRequest::default()
            .to(multicall3_addr)
            .input(TransactionInput::new(calldata.into()));

        // --- Step 3: eth_call via HttpRpcPool::with_retry ---
        // with_retry selects the best provider (lowest EWMA latency among Healthy),
        // retries on a different provider on first failure, reports outcomes to the
        // circuit breaker. The closure must return anyhow::Result so the error
        // bridges into the pool's failure-counting path.
        let raw_bytes = match rpc_pool
            .with_retry(|provider| {
                let tx = tx.clone();
                async move {
                    let result: anyhow::Result<Bytes> =
                        provider.call(tx).await.map_err(anyhow::Error::from);
                    result
                }
            })
            .await
        {
            Ok(b) => {
                RPC_CALLS_TOTAL
                    .with_label_values(&[&chain_id.to_string(), "ok"])
                    .inc();
                b
            }
            Err(e) => {
                RPC_CALLS_TOTAL
                    .with_label_values(&[&chain_id.to_string(), "err"])
                    .inc();
                error!(
                    event = "enricher.multicall_rpc_err",
                    chain = chain_id,
                    err = %e
                );
                for &addr in addrs {
                    let _ = upsert_token(
                        pool,
                        *chain_id,
                        addr,
                        ResolvedToken {
                            symbol: None,
                            decimals: None,
                            logo_url: None,
                            resolved_via: "failed",
                        },
                    )
                    .await;
                }
                continue;
            }
        };

        // --- Decode aggregate3 return (alloy 1.x: 1-arg, no validate bool — Defect #1) ---
        // In alloy 1.x, `aggregate3Call::abi_decode_returns` returns the tuple
        // element directly: `Vec<IMulticall3::Result>` (single unnamed return).
        let mc_results: Vec<IMulticall3::Result> =
            match IMulticall3::aggregate3Call::abi_decode_returns(&raw_bytes) {
                Ok(r) => r,
                Err(e) => {
                    error!(
                        event = "enricher.multicall_decode_err",
                        chain = chain_id,
                        err = %e
                    );
                    for &addr in addrs {
                        let _ = upsert_token(
                            pool,
                            *chain_id,
                            addr,
                            ResolvedToken {
                                symbol: None,
                                decimals: None,
                                logo_url: None,
                                resolved_via: "failed",
                            },
                        )
                        .await;
                    }
                    continue;
                }
            };

        let resolved_data = pair_results(mc_results, addrs.len());

        // --- Step 4: parallel TrustWallet HEAD verifications (Defect #2) ---
        let logo_futures: Vec<_> = addrs
            .iter()
            .map(|addr| tw.verify(*chain_id, *addr))
            .collect();
        let logos: Vec<Option<String>> = join_all(logo_futures)
            .await
            .into_iter()
            .map(|res| res.ok().flatten())
            .collect();

        // --- Step 5: classify + upsert ---
        for ((addr, r), logo) in addrs
            .iter()
            .zip(resolved_data.into_iter())
            .zip(logos.into_iter())
        {
            let resolved_via = match (r.symbol.is_some(), r.decimals.is_some(), logo.is_some()) {
                (true, true, _) => "onchain_full",
                (true, false, _) | (false, true, _) => "onchain_partial",
                (false, false, true) => "trustwallet_only",
                (false, false, false) => "failed",
            };

            TOKENS_RESOLVED_TOTAL
                .with_label_values(&[&chain_id.to_string(), resolved_via])
                .inc();

            if let Err(e) = upsert_token(
                pool,
                *chain_id,
                *addr,
                ResolvedToken {
                    symbol: r.symbol,
                    decimals: r.decimals,
                    logo_url: logo,
                    resolved_via,
                },
            )
            .await
            {
                error!(
                    event = "enricher.upsert_err",
                    chain = chain_id,
                    addr = %addr,
                    err = %e
                );
            }
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Convert a flat list of `(chain_id, token_in_lc, token_out_lc)` triples into
/// a deduplicated `Vec<(u64, Address)>` for `process_batch`.
fn triples_to_pairs(triples: Vec<(u64, String, String)>) -> Vec<(u64, Address)> {
    let mut seen: HashSet<(u64, Address)> = HashSet::new();
    let mut out = Vec::new();
    for (chain, ti, to) in triples {
        for addr_str in [&ti, &to] {
            if let Ok(addr) = Address::from_str(addr_str) {
                let key = (chain, addr);
                if seen.insert(key) {
                    out.push(key);
                }
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() -> Result<()> {
    // --- 1. Config ---
    let database_url = require_env("DATABASE_URL")?;
    let redis_url = require_env("REDIS_URL")?;
    let enricher_chains_raw = require_env("ENRICHER_CHAINS")?;
    let github_token = std::env::var("GITHUB_TOKEN").ok().filter(|s| !s.is_empty());
    let metrics_port: u16 = std::env::var("ENRICHER_METRICS_PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(9004);

    // --- 2. Tracing ---
    let filter = std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into());
    tracing_subscriber::fmt()
        .json()
        .with_env_filter(tracing_subscriber::EnvFilter::new(&filter))
        .init();

    info!(event = "enricher.starting", metrics_port);

    // --- 3. Metrics ---
    init_metrics();

    // --- 4. PostgreSQL ---
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .acquire_timeout(Duration::from_secs(10))
        .connect(&database_url)
        .await
        .context("PgPoolOptions::connect")?;
    info!(event = "enricher.db_connected");

    // --- 4b. Parse enabled chains + build HttpRpcPool per chain ---
    // Replaces the former parse_rpc_urls + build_providers pair.
    // Each pool uses canonical env var RPC_HTTP_<chain_id>, supports
    // multi-vendor CSV format, and runs a background health-check loop.
    let enabled_chains = parse_enricher_chains(&enricher_chains_raw)?;
    let rpc_pools = build_pools(&enabled_chains).await?;

    // --- 4c. TrustWallet client ---
    let tw = TrustWalletClient::new(github_token).context("TrustWalletClient::new")?;

    // --- 4d. Redis consumer ---
    let mut consumer = EnricherConsumer::connect(&redis_url).await?;

    // --- Metrics HTTP server (background task) ---
    tokio::spawn(serve_metrics(metrics_port));

    // --- 5. Drain PEL (I1 — crash recovery before main loop) ---
    // ACK-after-process: drain_pel returns (triples, ids).  We call
    // process_batch first, then ack_batch regardless of outcome — same
    // contract as the live consumer arm.  If the process crashes between
    // drain_pel return and ack_batch, the PEL entries remain unACKed and
    // will be re-fetched on the next startup.
    match consumer.drain_pel(BATCH_SIZE).await {
        Err(e) => {
            warn!(event = "enricher.drain_pel_failed", err = %e);
        }
        Ok((triples, ids)) if triples.is_empty() && ids.is_empty() => {
            info!(event = "enricher.pel_empty");
        }
        Ok((triples, ids)) => {
            info!(event = "enricher.processing_pel", count = triples.len());
            let pairs = triples_to_pairs(triples);
            if let Err(e) = process_batch(&pool, &rpc_pools, &tw, &pairs, "pel").await {
                error!(event = "enricher.pel_batch_err", err = %e);
            }
            // ACK after process (success or error) — at-most-once with
            // reconciliation tick as durability backstop.
            if let Err(e) = consumer.ack_batch(ids).await {
                error!(event = "enricher.pel_ack_err", err = %e);
            }
        }
    }

    // --- 6. Graceful shutdown signal setup ---

    // ctrl_c is a one-shot future; pin so it can be polled across loop iterations.
    let ctrl_c = tokio::signal::ctrl_c();
    tokio::pin!(ctrl_c);

    // SIGTERM: Linux VPS deploy target.  On non-Unix targets we use a future
    // that never resolves (`std::future::pending`) so the select! compiles
    // everywhere without #[cfg] inside the macro (tokio::select! does not
    // support attribute tokens on arms).
    #[cfg(unix)]
    let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        .context("register SIGTERM handler")?;

    // --- 7. Main select loop ---
    let mut recon_tick = tokio::time::interval(Duration::from_secs(RECON_INTERVAL_SECS));
    // Skip the first immediate tick so reconciliation doesn't fire at t=0.
    recon_tick.tick().await;

    info!(event = "enricher.main_loop_started");

    loop {
        // Build a SIGTERM future that either resolves (Unix) or is pending
        // (non-Unix) — constructed fresh each iteration so it doesn't require
        // pinning across the loop.
        #[cfg(unix)]
        let sigterm_fut = sigterm.recv();
        #[cfg(not(unix))]
        let sigterm_fut = std::future::pending::<Option<()>>();

        tokio::select! {
            biased;

            // Shutdown: SIGTERM (resolves on Unix, never on non-Unix)
            _ = sigterm_fut => {
                info!(event = "enricher.sigterm_received");
                break;
            }

            // Shutdown: ctrl-c (cross-platform)
            result = &mut ctrl_c => {
                match result {
                    Ok(()) => info!(event = "enricher.sigint_received"),
                    Err(e) => error!(event = "enricher.ctrlc_err", err = %e),
                }
                break;
            }

            // Reconciliation tick: pull unresolved tokens from PG every 5 min.
            _ = recon_tick.tick() => {
                match find_unresolved_tokens(&pool, RECON_LIMIT).await {
                    Err(e) => {
                        error!(event = "enricher.recon_query_err", err = %e);
                    }
                    Ok(rows) if rows.is_empty() => {
                        info!(event = "enricher.recon_tick_idle");
                    }
                    Ok(rows) => {
                        info!(event = "enricher.recon_tick", count = rows.len());

                        // I3 guard: reject malformed chain_id <= 0 before u64 cast.
                        let pairs: Vec<(u64, Address)> = rows
                            .into_iter()
                            .filter_map(|(chain, addr_str)| {
                                if chain <= 0 {
                                    warn!(event = "enricher.skip_invalid_chain_id", chain);
                                    return None;
                                }
                                let chain_u64 = chain as u64;
                                match Address::from_str(&addr_str) {
                                    Ok(addr) => Some((chain_u64, addr)),
                                    Err(e) => {
                                        warn!(
                                            event = "enricher.bad_address",
                                            addr = %addr_str,
                                            err = %e
                                        );
                                        None
                                    }
                                }
                            })
                            .collect();

                        // Update pending gauge per chain.
                        // ISSUE-3 fix: reset all known-chain labels to 0 first so
                        // stale gauges ("200 pending") don't persist after the backlog clears.
                        // M4 hardening: also reset any unsupported chain present in the
                        // current batch (e.g. 59144 Linea, 324 zkSync) so their gauge
                        // label doesn't remain non-zero forever after the backlog clears.
                        {
                            let mut per_chain_cnt: HashMap<u64, i64> = HashMap::new();
                            for &(chain, _) in &pairs {
                                *per_chain_cnt.entry(chain).or_default() += 1;
                            }
                            // Belt-and-suspenders: reset the hardcoded supported-chain list
                            // (handles chains that had entries in a previous iteration but
                            // have none in the current batch).
                            for known_chain in [1u64, 10, 56, 137, 8453, 42161] {
                                PENDING_UNRESOLVED
                                    .with_label_values(&[&known_chain.to_string()])
                                    .set(0);
                            }
                            // Reset every chain seen in THIS batch — covers unsupported
                            // chains before we write their actual counts below.
                            for chain in per_chain_cnt.keys() {
                                PENDING_UNRESOLVED
                                    .with_label_values(&[&chain.to_string()])
                                    .set(0);
                            }
                            // Write actual counts.
                            for (chain, cnt) in &per_chain_cnt {
                                PENDING_UNRESOLVED
                                    .with_label_values(&[&chain.to_string()])
                                    .set(*cnt);
                            }
                        }

                        if let Err(e) =
                            process_batch(&pool, &rpc_pools, &tw, &pairs, "reconciliation").await
                        {
                            error!(event = "enricher.recon_batch_err", err = %e);
                        }
                    }
                }
            }

            // Consumer arm: one XREADGROUP read (BLOCK 2000ms).
            // ACK-after-process: parse returns (triples, ids). We call
            // process_batch first, then ack_batch regardless of outcome.
            // This ensures entries survive a crash between read and process.
            batch_result = consumer.read_one_batch() => {
                match batch_result {
                    Err(e) => {
                        // Log and continue — transient Redis errors should not crash the loop.
                        error!(event = "enricher.consumer_read_err", err = %e);
                    }
                    Ok((triples, _ids)) if triples.is_empty() => {
                        // BLOCK timeout with no messages — normal idle path.
                        // ids is also empty; nothing to ACK.
                    }
                    Ok((triples, ids)) => {
                        let pairs = triples_to_pairs(triples);
                        if let Err(e) =
                            process_batch(&pool, &rpc_pools, &tw, &pairs, "consumer").await
                        {
                            error!(event = "enricher.consumer_batch_err", err = %e);
                        }
                        // ACK after process (success or error) — at-most-once with
                        // reconciliation tick as durability backstop.
                        if let Err(e) = consumer.ack_batch(ids).await {
                            error!(event = "enricher.consumer_ack_err", err = %e);
                        }
                    }
                }
            }
        }
    }

    // ISSUE-4 fix: signal to Prometheus that the service is no longer running.
    ENRICHER_UP.set(0);
    info!(event = "enricher.shutdown");
    Ok(())
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // --- parse_enricher_chains ---

    #[test]
    fn parse_enricher_chains_valid() {
        let chains = parse_enricher_chains("1,137,42161").unwrap();
        assert_eq!(chains, vec![1u64, 137, 42161]);
    }

    #[test]
    fn parse_enricher_chains_single() {
        let chains = parse_enricher_chains("1").unwrap();
        assert_eq!(chains, vec![1u64]);
    }

    #[test]
    fn parse_enricher_chains_with_spaces() {
        let chains = parse_enricher_chains(" 1 , 137 ").unwrap();
        assert_eq!(chains, vec![1u64, 137]);
    }

    #[test]
    fn parse_enricher_chains_empty_fails() {
        assert!(parse_enricher_chains("").is_err());
        assert!(parse_enricher_chains("   ").is_err());
    }

    #[test]
    fn parse_enricher_chains_bad_value_fails() {
        assert!(parse_enricher_chains("1,notanumber").is_err());
    }

    #[test]
    fn parse_enricher_chains_negative_fails() {
        // u64 parse rejects negative strings
        assert!(parse_enricher_chains("-1").is_err());
    }

    // --- triples_to_pairs ---

    #[test]
    fn triples_to_pairs_deduplicates() {
        let triples = vec![
            (
                1u64,
                "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2".to_string(),
                "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48".to_string(),
            ),
            // duplicate of first token_in
            (
                1u64,
                "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2".to_string(),
                "0xdac17f958d2ee523a2206206994597c13d831ec7".to_string(),
            ),
        ];
        let pairs = triples_to_pairs(triples);
        // 3 unique addresses across 2 triples.
        assert_eq!(pairs.len(), 3);
    }

    #[test]
    fn triples_to_pairs_skips_bad_addresses() {
        let triples = vec![(
            1u64,
            "not-an-address".to_string(),
            "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2".to_string(),
        )];
        let pairs = triples_to_pairs(triples);
        // Only the valid address survives.
        assert_eq!(pairs.len(), 1);
    }
}
