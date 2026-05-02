//! Scanner loop — real mempool detection.
//!
//! Responsibilities:
//!   - Per enabled chain, resolve `RPC_WS_<chain_id>` from env.
//!   - If absent, stay idle with explicit state gauge + periodic warn log.
//!   - If present, open WS, subscribe to pending, dedup, decode, match patterns, persist+publish.
//!   - Honor kill-switch at every tick.
//!   - Reconnect with exponential backoff + jitter on WS errors.
//!
//! No fabricated data. No partial records. Every Opportunity that reaches DB
//! corresponds to a real pending tx observed on the wire.

use crate::{
    calldata, dedup::Dedup, patterns, persistence, publisher,
};
use ethers::types::H256;
use futures_util::StreamExt;
use rand::Rng;
use shared_rs::{
    chains::{self, RouterKind},
    config::AppConfig,
    killswitch::KillSwitchClient,
    metrics::OPPORTUNITIES_TOTAL,
    rpc_failover::{WsEndpoint, WsRpcPool},
};
use sqlx::postgres::PgPool;
use std::{sync::Arc, time::Duration};
use tracing::{debug, error, info, warn};

use crate::chain_client::WsChainClient;

pub struct ScannerHandle {
    pub chain_id: u64,
}

pub async fn run_chain(
    chain_id: u64,
    cfg: Arc<AppConfig>,
    killswitch: KillSwitchClient,
    redis: redis::aio::ConnectionManager,
    db: Option<PgPool>,
    dedup: Arc<Dedup>,
) -> anyhow::Result<ScannerHandle> {
    // RPC failover discipline (G-RPC-1): build a multi-vendor pool from env.
    // CSV format `name=url,name=url`; bare URLs accepted for back-compat.
    let pool = match WsRpcPool::from_env(chain_id) {
        Ok(Some(p)) => p,
        Ok(None) => {
            warn!(
                event = "scanner.no_rpc",
                chain_id,
                env_key = format!("RPC_WS_{chain_id}"),
                "RPC_WS not configured; scanner stays idle for this chain (no detection, no fabrication)"
            );
            tokio::spawn(async move {
                idle_chain_loop(chain_id, killswitch).await;
            });
            return Ok(ScannerHandle { chain_id });
        }
        Err(e) => {
            error!(
                event = "scanner.rpc_pool_invalid",
                chain_id,
                error = %e,
                "RPC_WS env value did not parse — scanner idle"
            );
            tokio::spawn(async move {
                idle_chain_loop(chain_id, killswitch).await;
            });
            return Ok(ScannerHandle { chain_id });
        }
    };

    // Spawn the detection loop with the full endpoint list.
    tokio::spawn(detection_loop(
        chain_id, pool.endpoints, cfg, killswitch, redis, db, dedup,
    ));
    Ok(ScannerHandle { chain_id })
}

async fn idle_chain_loop(chain_id: u64, killswitch: KillSwitchClient) {
    let mut ticker = tokio::time::interval(Duration::from_secs(60));
    loop {
        ticker.tick().await;
        let ks = killswitch.is_enabled().await;
        info!(
            event = "scanner.idle",
            chain_id,
            kill_switch = ks,
            "scanner is alive but RPC_WS_{chain_id} not set; no detection happening"
        );
    }
}

async fn detection_loop(
    chain_id: u64,
    endpoints: Vec<WsEndpoint>,
    _cfg: Arc<AppConfig>,
    killswitch: KillSwitchClient,
    mut redis: redis::aio::ConnectionManager,
    db: Option<PgPool>,
    dedup: Arc<Dedup>,
) {
    let mut backoff_ms: u64 = 1000;
    let mut idx: usize = 0;
    loop {
        // The searcher-rs scanner runs continuously, even if the kill-switch is ARMED.
        // The kill-switch blocks execution downstream (relays-client), but the intelligence
        // layer always detects opportunities to populate the real-time dashboards.

        // Pick the next endpoint round-robin. With a healthy primary the index
        // resets on success below, so failures rotate through the pool.
        let endpoint = &endpoints[idx % endpoints.len()];
        let client = match WsChainClient::connect(chain_id, &endpoint.url).await {
            Ok(c) => {
                if idx != 0 {
                    info!(
                        event = "scanner.connected_via_backup",
                        chain_id,
                        provider = %endpoint.name,
                        "connected via backup WS provider after primary failures"
                    );
                }
                backoff_ms = 1000;
                idx = 0;
                c
            }
            Err(e) => {
                error!(
                    event = "scanner.connect_error",
                    chain_id,
                    provider = %endpoint.name,
                    error = %e,
                    "rotating to next WS provider"
                );
                idx = (idx + 1) % endpoints.len();
                if idx == 0 {
                    // Exhausted the ring — back off before another round.
                    sleep_with_backoff(&mut backoff_ms).await;
                }
                continue;
            }
        };

        if let Err(e) = run_subscription(&client, &killswitch, &mut redis, db.as_ref(), &dedup).await {
            error!(
                event = "scanner.subscription_error",
                chain_id,
                provider = %endpoint.name,
                error = %e
            );
            // Rotate on subscription death too — the WS connection died, try next.
            idx = (idx + 1) % endpoints.len();
            if idx == 0 {
                sleep_with_backoff(&mut backoff_ms).await;
            }
        }
    }
}

async fn sleep_with_backoff(backoff_ms: &mut u64) {
    let jitter: u64 = rand::thread_rng().gen_range(0..500);
    tokio::time::sleep(Duration::from_millis(*backoff_ms + jitter)).await;
    *backoff_ms = (*backoff_ms * 2).min(30_000);
}

async fn run_subscription(
    client: &WsChainClient,
    killswitch: &KillSwitchClient,
    redis: &mut redis::aio::ConnectionManager,
    db: Option<&PgPool>,
    dedup: &Dedup,
) -> anyhow::Result<()> {
    let mut stream = client.subscribe_pending().await?;
    info!(event = "scanner.subscribed", chain_id = client.chain_id);
    
    let mut counter = 0u64;
    while let Some(hash) = stream.next().await {
        counter = counter.wrapping_add(1);
        // Mempool sampling: Process only 1 out of every 20 transactions to respect Alchemy Pay-As-You-Go limits (330 CU/s)
        if counter % 20 != 0 {
            continue;
        }

        // We no longer pause the scanner on kill-switch. It must always scan and emit.
        if let Err(e) = process_pending(client, hash, redis, db, dedup).await {
            debug!(event = "scanner.process_err", hash = %hash, error = %e);
        }
    }
    anyhow::bail!("pending tx stream ended")
}

async fn process_pending(
    client: &WsChainClient,
    hash: H256,
    redis: &mut redis::aio::ConnectionManager,
    db: Option<&PgPool>,
    dedup: &Dedup,
) -> anyhow::Result<()> {
    if !dedup.check_and_mark(hash, redis).await {
        return Ok(());
    }
    let tx = match client.get_tx(hash).await? {
        Some(t) => t,
        None => return Ok(()), // dropped from mempool before we got it
    };
    let to = match tx.to {
        Some(a) => a,
        None => return Ok(()), // contract creation, ignore
    };
    let to_bytes: [u8; 20] = to.into();
    let router = match chains::find_router(client.chain_id, &to_bytes) {
        Some(r) => r,
        None => return Ok(()),
    };
    let decoded = match calldata::decode(&tx.input, router.kind) {
        Ok(d) => d,
        Err(reason) => {
            debug!(event = "scanner.decode_failed", reason = reason.as_str(), router = router.kind.as_str());
            return Ok(());
        }
    };
    if router.kind == RouterKind::Unknown {
        return Ok(());
    }

    let ctx = patterns::TxContext {
        chain_id: client.chain_id,
        block_number: tx.block_number.map(|n| n.as_u64()),
        tx_from: tx.from.into(),
        tx_value: tx.value,
    };
    let opportunity = patterns::build_dex_arb_candidate(&ctx, &decoded);

    // Persist + publish. Both are best-effort with their own error paths.
    if let Some(pool) = db {
        if let Err(e) = persistence::insert_opportunity(pool, &opportunity).await {
            error!(event = "scanner.db_error", tx_hash = %hash, error = %e);
        }
    }
    publisher::publish(redis, &opportunity).await?;

    OPPORTUNITIES_TOTAL
        .with_label_values(&[
            &opportunity.chain_id.to_string(),
            "dex_arb",
            "detected",
        ])
        .inc();

    Ok(())
}
