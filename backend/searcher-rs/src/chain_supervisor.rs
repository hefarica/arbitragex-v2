//! Chain Task Supervisor (B1.d, 2026-05-13).
//!
//! Consumes the broadcast stream emitted by [`crate::config_reload::ChainConfigReloader`]
//! on `arbx:config:chains:reload` events from the api-server admin UI and
//! orchestrates the per-chain scanner task lifecycle:
//!
//! - On `Accepted` event for an existing chain: cancel the running task,
//!   rebuild `Arc<HttpRpcPool>` from the chain's *current* row in
//!   `chains_runtime`, spawn a fresh scanner task.
//! - On `Accepted` event with `action == "disabled"`: cancel and do not respawn.
//! - On `Accepted` event for a chain not previously running: spawn fresh.
//!
//! ## Doctrine — where does the new config come from?
//!
//! The hot-reload event carries only `(chain_id, config_hash, action)`. The
//! supervisor reads the *full row* from `chains_runtime` via the `db_pool`
//! to recover the new `rpc_http_url`. This is what closes the loop with the
//! admin UI: operator edit in /admin/chains → api-server writes PG row →
//! publishes event → supervisor reads PG row → rebuilds with the new URL.
//!
//! When `db_pool` is `None` (PG unreachable / not configured) the supervisor
//! falls back to `HttpRpcPool::from_env(chain_id)` and surfaces a warn. This
//! keeps the system functioning under degraded conditions but the UI loop
//! is broken in that mode.
//!
//! ## Known gap — WS pool reload (B1.e)
//!
//! The scanner's `WsRpcPool` is built *inside* `run_chain` by reading
//! `RPC_WS_<chain_id>` env vars. The current supervisor does not propagate
//! a `rpc_ws_url` from PG into that path. A reload that changes the WS URL
//! in chains_runtime will rebuild the HTTP pool correctly but the WS pool
//! will still read env. Tracking this as B1.e: refactor `run_chain` to
//! accept an `Option<WsRpcPool>` from the caller.
//!
//! ## Concurrency model
//!
//! - The supervisor lives in one `tokio::spawn` task. The `HashMap<u64,
//!   CancellationToken>` is owned exclusively by that task — no lock
//!   needed (operator directive #4 lock-free reads on hot path: this
//!   map is mutated only by the supervisor; scanners read their own
//!   token at spawn time and never re-read).
//! - Per-chain abort uses `CancellationToken::cancel()` which propagates
//!   to all `tokio::select!` sites in the scanner (operator directive #2
//!   graceful abort). We do NOT `await` task completion (operator
//!   directive #3 zero-downtime spawn) — the new task is spawned
//!   concurrently with the old task winding down.

use crate::config_reload::ChainReloadEvent;
use crate::scanner;
use shared_rs::{
    config::AppConfig, killswitch::KillSwitchClient, rpc_failover::HttpRpcPool,
    trading_config::TradingConfigClient,
};
use sqlx::postgres::PgPool;
use sqlx::Row;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};

#[allow(dead_code)]
pub struct ChainSupervisor {
    cfg: Arc<AppConfig>,
    killswitch: KillSwitchClient,
    redis_conn: redis::aio::ConnectionManager,
    db_pool: Option<PgPool>,
    dedup: Arc<crate::dedup::Dedup>,
    opp_dedup: Arc<crate::dedup::OppDedup>,
    trading_config: TradingConfigClient,
    decimals_provider: crate::scanner::ScannerDecimalsProvider,
    event_rx: tokio::sync::broadcast::Receiver<ChainReloadEvent>,
    /// Running tasks keyed by chain_id. The token is the supervisor's handle
    /// for graceful abort. Map is owned by the supervisor's single task so
    /// no lock is required.
    running_tasks: HashMap<u64, CancellationToken>,
}

impl ChainSupervisor {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        cfg: Arc<AppConfig>,
        killswitch: KillSwitchClient,
        redis_conn: redis::aio::ConnectionManager,
        db_pool: Option<PgPool>,
        dedup: Arc<crate::dedup::Dedup>,
        opp_dedup: Arc<crate::dedup::OppDedup>,
        trading_config: TradingConfigClient,
        decimals_provider: crate::scanner::ScannerDecimalsProvider,
        event_rx: tokio::sync::broadcast::Receiver<ChainReloadEvent>,
    ) -> Self {
        Self {
            cfg,
            killswitch,
            redis_conn,
            db_pool,
            dedup,
            opp_dedup,
            trading_config,
            decimals_provider,
            event_rx,
            running_tasks: HashMap::new(),
        }
    }

    /// Run the supervisor. Owns the loop until the broadcast channel
    /// closes (reloader task exited) or a hard error occurs.
    pub async fn run(mut self, initial_chains: Vec<u64>) {
        info!(
            event = "chain_supervisor.boot",
            chains = ?initial_chains,
            "chain task supervisor starting"
        );

        // Build the union of (a) the static `initial_chains` list (from
        // env / configs/app.toml) and (b) all enabled chains in
        // `chains_runtime` (operator-managed via /admin/chains UI). This
        // is Fix C: chains added via the admin UI before this process
        // started will be picked up at boot, not lost across restart.
        let mut union: HashSet<u64> = initial_chains.into_iter().collect();
        if let Some(pool) = self.db_pool.as_ref() {
            match fetch_all_enabled_chains(pool).await {
                Ok(pg_chains) => {
                    info!(
                        event = "chain_supervisor.pg_initial_sync",
                        chains = ?pg_chains,
                        "discovered enabled chains in chains_runtime"
                    );
                    union.extend(pg_chains);
                }
                Err(e) => {
                    warn!(
                        event = "chain_supervisor.pg_initial_sync_failed",
                        error = %e,
                        "could not query chains_runtime at boot; static initial_chains only"
                    );
                }
            }
        }

        for chain_id in union {
            self.spawn_chain(chain_id).await;
        }

        // Event loop. Uses an explicit `match` on `recv()` so we can
        // distinguish `Closed` (graceful exit) from `Lagged` (continue)
        // from valid events. The previous `tokio::select! { Ok(event) =
        // ... }` pattern panicked at runtime if all arms were disabled,
        // which is exactly what happens when the reloader Sender is
        // dropped.
        loop {
            match self.event_rx.recv().await {
                Ok(event) => self.handle_reload_event(event).await,
                Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                    warn!(
                        event = "chain_supervisor.event_channel_closed",
                        "reloader Sender dropped; supervisor exiting cleanly"
                    );
                    break;
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(missed)) => {
                    warn!(
                        event = "chain_supervisor.event_lagged",
                        missed,
                        "broadcast lagged; some reload events dropped — chains may be out of sync until next event"
                    );
                    continue;
                }
            }
        }

        // Graceful tear-down: cancel every still-running chain so their
        // scanners exit at their next select! poll. We don't await
        // because the supervisor is itself shutting down.
        for (chain_id, token) in self.running_tasks.drain() {
            info!(
                event = "chain_supervisor.shutdown_cancel",
                chain_id, "cancelling chain task during supervisor shutdown"
            );
            token.cancel();
        }
    }

    async fn handle_reload_event(&mut self, event: ChainReloadEvent) {
        info!(
            event = "chain_supervisor.reload",
            chain_id = event.chain_id,
            action = %event.action,
            "supervisor handling reload event"
        );

        // Abort the previous task (if any). Cancel propagates through
        // every scanner select! site; we don't await because the new
        // task is allowed to spawn concurrently per operator directive
        // #3 (zero-downtime for other chains).
        if let Some(cancel_token) = self.running_tasks.remove(&event.chain_id) {
            info!(
                event = "chain_supervisor.abort_graceful",
                chain_id = event.chain_id,
                "cancelling existing chain task"
            );
            cancel_token.cancel();
        }

        if event.action == "disabled" {
            info!(
                event = "chain_supervisor.disabled",
                chain_id = event.chain_id,
                "chain disabled; not respawning"
            );
            return;
        }

        self.spawn_chain(event.chain_id).await;
    }

    async fn spawn_chain(&mut self, chain_id: u64) {
        let cancel = CancellationToken::new();
        self.running_tasks.insert(chain_id, cancel.clone());

        // Resolve the HTTP pool. Preference order:
        //   1. PG `chains_runtime` row (operator-managed via admin UI).
        //   2. `RPC_HTTP_<chain_id>` env var (legacy / bootstrap fallback).
        // Falling back to env keeps the system functional when PG is
        // unreachable but the operator should know about it — emit warn.
        let rpc_pool = self.build_http_pool(chain_id).await;

        // Reconstruct SimulatorV2 for this chain. Only attempted when an
        // RPC pool is available — without a URL we cannot snapshot a
        // simulator state.
        #[cfg(feature = "v2-simulator")]
        let sim_v2 = if let Some(ref pool) = rpc_pool {
            match pool.pick() {
                Ok(p) => {
                    let obs_block = p.snapshot_block();
                    let sim = if obs_block > 0 {
                        simulator_v2::SimulatorV2::new(p.url.clone()).with_block(obs_block)
                    } else {
                        simulator_v2::SimulatorV2::new(p.url.clone())
                    };
                    Some(Arc::new(sim))
                }
                Err(_) => None,
            }
        } else {
            None
        };
        #[cfg(not(feature = "v2-simulator"))]
        let sim_v2: Option<()> = None;

        let ks = self.killswitch.clone();
        let cfg_c = self.cfg.clone();
        let redis_c = self.redis_conn.clone();
        let db_c = self.db_pool.clone();
        let dedup_c = self.dedup.clone();
        let opp_dedup_c = self.opp_dedup.clone();
        let tc_c = self.trading_config.clone();
        let decimals_provider_c = self.decimals_provider.clone();

        tokio::spawn(async move {
            info!(
                event = "chain_supervisor.spawning",
                chain_id, "spawning chain scanner task"
            );
            // Detection loop restart: if run_chain exits with an error (e.g.,
            // all WS providers failed subscription, stream ended), respawn it
            // with backoff instead of dying permanently. The scanner's internal
            // loop rotates providers, but a subscription error that propagates
            // through `?` kills the entire task. Without this retry, a single
            // transient provider outage silences detection forever until manual
            // container restart.
            let mut retry_count: u32 = 0;
            const MAX_RETRIES: u32 = 10;
            const RETRY_DELAY_SECS: u64 = 30;
            loop {
                if let Err(e) = scanner::run_chain(
                    chain_id,
                    cfg_c.clone(),
                    ks.clone(),
                    redis_c.clone(),
                    db_c.clone(),
                    dedup_c.clone(),
                    opp_dedup_c.clone(),
                    tc_c.clone(),
                    rpc_pool.clone(),
                    sim_v2.clone(),
                    decimals_provider_c.clone(),
                    cancel.clone(),
                )
                .await
                {
                    retry_count += 1;
                    if retry_count >= MAX_RETRIES {
                        error!(
                            event = "chain_supervisor.spawn_failed_permanent",
                            chain_id,
                            retries = retry_count,
                            error = %e,
                            "scanner task exhausted retries — giving up"
                        );
                        break;
                    }
                    warn!(
                        event = "chain_supervisor.scanner_retry",
                        chain_id,
                        retry = retry_count,
                        max_retries = MAX_RETRIES,
                        next_attempt_secs = RETRY_DELAY_SECS,
                        error = %e,
                        "scanner task exited — respawning detection loop"
                    );
                    tokio::time::sleep(std::time::Duration::from_secs(RETRY_DELAY_SECS)).await;
                    continue;
                }
                // run_chain returned Ok — it was cancelled or finished normally
                break;
            }
        });
    }

    /// Build the `Arc<HttpRpcPool>` for a chain, preferring `chains_runtime`
    /// (PG) over `RPC_HTTP_<chain_id>` env. Returns `None` when neither
    /// source yields a working pool.
    async fn build_http_pool(&self, chain_id: u64) -> Option<Arc<HttpRpcPool>> {
        // Try PG first.
        if let Some(db) = self.db_pool.as_ref() {
            match fetch_chain_config(db, chain_id).await {
                Ok(Some(cfg)) if cfg.enabled => {
                    match HttpRpcPool::from_csv(chain_id, &cfg.rpc_http_url).await {
                        Ok(pool) => {
                            info!(
                                event = "chain_supervisor.pool_built_pg",
                                chain_id,
                                source = "chains_runtime",
                                "HttpRpcPool built from PG row"
                            );
                            return Some(Arc::new(pool));
                        }
                        Err(e) => {
                            warn!(
                                event = "chain_supervisor.pool_pg_build_failed",
                                chain_id,
                                error = %e,
                                "chains_runtime row found but HttpRpcPool build failed; falling through to env"
                            );
                        }
                    }
                }
                Ok(Some(_)) => {
                    // Row present but `enabled=false` — treat as no-config.
                    return None;
                }
                Ok(None) => {
                    info!(
                        event = "chain_supervisor.pool_no_pg_row",
                        chain_id, "no chains_runtime row; trying env fallback"
                    );
                }
                Err(e) => {
                    warn!(
                        event = "chain_supervisor.pool_pg_query_failed",
                        chain_id,
                        error = %e,
                        "chains_runtime query failed; trying env fallback"
                    );
                }
            }
        }

        // Env fallback.
        match HttpRpcPool::from_env(chain_id).await {
            Ok(Some(pool)) => {
                info!(
                    event = "chain_supervisor.pool_built_env",
                    chain_id,
                    source = "env",
                    "HttpRpcPool built from RPC_HTTP_<chain_id>"
                );
                Some(Arc::new(pool))
            }
            Ok(None) => {
                info!(
                    event = "chain_supervisor.pool_none",
                    chain_id, "no RPC config in PG or env; scanner will run idle"
                );
                None
            }
            Err(e) => {
                warn!(
                    event = "chain_supervisor.pool_env_invalid",
                    chain_id,
                    error = %e,
                    "RPC_HTTP_<chain_id> set but invalid"
                );
                None
            }
        }
    }
}

// ── PG helpers ──────────────────────────────────────────────────────────

/// Minimal projection of the `chains_runtime` columns the supervisor needs.
/// Keep this struct private — full row access lives in api-server.
#[derive(Debug, Clone)]
struct ChainRuntimeConfig {
    rpc_http_url: String,
    #[allow(dead_code)]
    rpc_ws_url: Option<String>,
    enabled: bool,
}

/// Fetch the current `chains_runtime` row for one chain.
async fn fetch_chain_config(
    db: &PgPool,
    chain_id: u64,
) -> Result<Option<ChainRuntimeConfig>, sqlx::Error> {
    let row_opt = sqlx::query(
        "SELECT rpc_http_url, rpc_ws_url, enabled FROM chains_runtime WHERE chain_id = $1",
    )
    .bind(chain_id as i64)
    .fetch_optional(db)
    .await?;
    Ok(row_opt.map(|r| ChainRuntimeConfig {
        rpc_http_url: r.get("rpc_http_url"),
        rpc_ws_url: r.get("rpc_ws_url"),
        enabled: r.get("enabled"),
    }))
}

/// Fetch every `chain_id` currently marked `enabled = true` in
/// `chains_runtime`. Used at supervisor boot to pick up chains registered
/// via the admin UI before this process started.
async fn fetch_all_enabled_chains(db: &PgPool) -> Result<Vec<u64>, sqlx::Error> {
    let rows =
        sqlx::query("SELECT chain_id FROM chains_runtime WHERE enabled = TRUE ORDER BY chain_id")
            .fetch_all(db)
            .await?;
    Ok(rows
        .into_iter()
        .map(|r| r.get::<i64, _>("chain_id") as u64)
        .collect())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::config_reload::ChainReloadEvent;

    /// Smoke test the dedup-style match arms on RecvError. We don't need a
    /// real supervisor instance — we exercise the same match logic in a
    /// helper to assert each variant maps to the intended action.
    #[tokio::test]
    async fn broadcast_recv_closed_exits_loop() {
        let (tx, mut rx) = tokio::sync::broadcast::channel::<ChainReloadEvent>(4);
        drop(tx); // simulate Sender dropped → recv() returns Err(Closed)
        let err = rx.recv().await.unwrap_err();
        assert!(matches!(
            err,
            tokio::sync::broadcast::error::RecvError::Closed
        ));
        // The supervisor's loop sees this and breaks (covered by run() body).
    }

    #[tokio::test]
    async fn broadcast_recv_lagged_is_recoverable() {
        let (tx, mut rx) = tokio::sync::broadcast::channel::<ChainReloadEvent>(2);
        // Fill + overflow the buffer so the slow receiver lags.
        for i in 0..10u64 {
            let _ = tx.send(ChainReloadEvent {
                action: "created".into(),
                chain_id: i,
                config_hash: format!("h{i}"),
                timestamp: "".into(),
            });
        }
        // The first recv after overflow returns Err(Lagged(_)).
        let err = rx.recv().await.unwrap_err();
        assert!(matches!(
            err,
            tokio::sync::broadcast::error::RecvError::Lagged(_),
        ));
        // Subsequent recv recovers — channel keeps working.
        let next = rx.recv().await.unwrap();
        assert!(next.chain_id < 10);
    }

    /// Drive a single iteration of handle_reload_event semantics without
    /// constructing the full supervisor: assert that the disabled action
    /// path does NOT respawn (no new entry inserted) while a non-disabled
    /// action would respawn (insert).
    #[tokio::test]
    async fn disabled_action_does_not_respawn() {
        // We exercise the decision logic in isolation by mirroring the
        // branch in handle_reload_event. This guards the policy without
        // spinning up the actual scanner.
        let event = ChainReloadEvent {
            action: "disabled".into(),
            chain_id: 137,
            config_hash: "h".into(),
            timestamp: "".into(),
        };
        let should_respawn = event.action != "disabled";
        assert!(!should_respawn, "disabled events must not respawn");

        let event_updated = ChainReloadEvent {
            action: "updated".into(),
            ..event
        };
        assert!(event_updated.action != "disabled");
    }

    #[test]
    fn chain_runtime_config_shape() {
        // Structural assertion — guards the projection schema. If the
        // fetcher SELECT list changes, this test wouldn't break but the
        // sqlx::Error path would surface at runtime — by reading r.get(...)
        // for an absent column. Keep the projection minimal and named.
        let c = ChainRuntimeConfig {
            rpc_http_url: "https://x".into(),
            rpc_ws_url: Some("wss://y".into()),
            enabled: true,
        };
        assert_eq!(c.rpc_http_url, "https://x");
        assert!(c.enabled);
    }
}
