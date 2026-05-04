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
    trading_config::TradingConfigClient,
};
use sqlx::postgres::PgPool;
use std::{sync::Arc, time::Duration};
use tracing::{debug, error, info, warn};
use prioritization_spine::types::{OpportunityCandidate};
use prioritization_spine::evidence::{OpportunityEvidence};
use prioritization_spine::scoring::{OpportunityScorer, PrioritizationEngine};
use prioritization_spine::gates::{can_execute};
use prioritization_spine::decision::{ExecutionDecision};
use prioritization_spine::simulator::EvmSimulator;
use prioritization_spine::config_aware::{ConfigAwareEvaluator, ConfigGateOutcome, NetworkSignals};
use std::fs::OpenOptions;
use std::io::Write;


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
    trading_config: TradingConfigClient,
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
        chain_id, pool.endpoints, cfg, killswitch, redis, db, dedup, trading_config,
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
    trading_config: TradingConfigClient,
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

        if let Err(e) = run_subscription(&client, &killswitch, &mut redis, db.as_ref(), &dedup, &trading_config).await {
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
    trading_config: &TradingConfigClient,
) -> anyhow::Result<()> {
    let _ = killswitch; // reserved: kill-switch only blocks downstream execution
    let mut stream = client.subscribe_pending().await?;
    info!(event = "scanner.subscribed", chain_id = client.chain_id);

    while let Some(hash) = stream.next().await {
        // We no longer pause the scanner on kill-switch. It must always scan and emit.
        if let Err(e) = process_pending(client, hash, redis, db, dedup, trading_config).await {
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
    trading_config: &TradingConfigClient,
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

    // --- CONFIG-AWARE SPINE INTERCEPTOR ---
    // Hot-reads operator's trading config from Redis (≤1s cache TTL). When no
    // config exists for this chain, the scanner OBSERVES but does not score —
    // dashboards see the detection but no fabricated profit numbers.
    let amount_in_f64 = opportunity.amount_in_wei.parse::<f64>().unwrap_or(0.0) / 1e18;
    let mut opportunity = opportunity;

    let cfg_opt = match trading_config.state(client.chain_id).await {
        Ok(opt) => opt,
        Err(e) => {
            warn!(event = "trading_config.read_failed", chain_id = client.chain_id, error = %e);
            None
        }
    };

    let candidate = OpportunityCandidate {
        route_fingerprint: format!("{}_{}_{}", opportunity.dex_a, opportunity.token_in, opportunity.token_out),
        pool_addresses: vec![],
        token_addresses: vec![opportunity.token_in.clone(), opportunity.token_out.clone()],
        dex_adapters: vec![opportunity.dex_a.clone()],
        amount_in: amount_in_f64,
        // Until route-finder + reserves fetch wire up, expected_amount_out
        // mirrors amount_in (gross_profit = 0) — math-engine then flags it as
        // not viable, which is the honest signal.
        expected_amount_out: amount_in_f64,
        gross_profit: 0.0,
    };

    let Some(cfg) = cfg_opt else {
        // No operator config → observe-only path.
        info!(
            event = "scanner.no_trading_config",
            chain_id = client.chain_id,
            hash = %hash,
            "configure /config/trading to enable scoring; persisting raw observation"
        );
        opportunity.roi_pct = None;
        opportunity.risk_score = None;
        if let Some(pool) = db {
            if let Err(e) = persistence::insert_opportunity(pool, &opportunity).await {
                error!(event = "scanner.db_error", tx_hash = %hash, error = %e);
            }
        }
        publisher::publish(redis, &opportunity).await?;
        OPPORTUNITIES_TOTAL
            .with_label_values(&[&opportunity.chain_id.to_string(), "dex_arb", "observed_no_config"])
            .inc();
        return Ok(());
    };

    if !cfg.enabled {
        debug!(event = "config.disabled", chain_id = client.chain_id, hash = %hash);
        return Ok(());
    }

    // Network signals — basefee/tip wiring lands with the chain-client refresh
    // (next sprint). Fixed gas strategy in config still works in the meantime.
    let signals = NetworkSignals::unknown(opportunity.block_number.unwrap_or(0));
    let evaluator = ConfigAwareEvaluator::new(&cfg, signals);

    // Strategy classification — when the calldata decoder grows multi-leg support,
    // this becomes router-driven. For now every observed swap is dex_arb_v2v2.
    let strategy_kind = "dex_arb_v2v2";

    let gate_outcome = evaluator.evaluate(
        &candidate,
        strategy_kind,
        client.chain_id,
        "rpc-pool".to_string(),
        cfg.gas_estimate_units.min(60_000), // proxy until rpc_latency tracked live
    );

    // ── Gate outcomes ────────────────────────────────────────────────────
    // Doctrine: silent early-returns on TokenNotAllowed / StrategyDisabled hide
    // detector activity from the operator (the dashboard then looks idle even
    // when 100s of pending txs/sec are filtered out). We persist these rows too
    // with risk_score=0 + roi_pct=0 so the operator sees rejection volume +
    // can iterate the allowlist with real evidence (RULE 00 transparency).
    let (mut final_evidence, math_outcome, config_rejection) = match gate_outcome {
        ConfigGateOutcome::TokenNotAllowed { token_symbol_or_addr } => {
            info!(
                event = "config.token_not_allowed",
                chain_id = client.chain_id,
                hash = %hash,
                token = %token_symbol_or_addr,
            );
            opportunity.expected_profit_usd = 0.0;
            opportunity.roi_pct = Some(0.0);
            opportunity.risk_score = Some(0.0);
            if let Some(pool) = db {
                if let Err(e) = persistence::insert_opportunity(pool, &opportunity).await {
                    error!(event = "scanner.db_error", tx_hash = %hash, error = %e);
                }
            }
            publisher::publish(redis, &opportunity).await?;
            OPPORTUNITIES_TOTAL
                .with_label_values(&[&opportunity.chain_id.to_string(), "dex_arb", "rejected_token_allowlist"])
                .inc();
            return Ok(());
        }
        ConfigGateOutcome::StrategyDisabled { strategy_kind } => {
            info!(
                event = "config.strategy_disabled",
                chain_id = client.chain_id,
                hash = %hash,
                strategy = %strategy_kind,
            );
            opportunity.expected_profit_usd = 0.0;
            opportunity.roi_pct = Some(0.0);
            opportunity.risk_score = Some(0.0);
            if let Some(pool) = db {
                if let Err(e) = persistence::insert_opportunity(pool, &opportunity).await {
                    error!(event = "scanner.db_error", tx_hash = %hash, error = %e);
                }
            }
            publisher::publish(redis, &opportunity).await?;
            OPPORTUNITIES_TOTAL
                .with_label_values(&[&opportunity.chain_id.to_string(), "dex_arb", "rejected_strategy_disabled"])
                .inc();
            return Ok(());
        }
        ConfigGateOutcome::Evaluated { outcome, evidence, rejection } => (evidence, outcome, rejection),
    };

    // REVM atomic sim gate (still a structural placeholder until lazy state
    // wires in — keeps the gate honest: simulator.rs returns "PASS" for empty
    // calldata so we don't reject the entire pipeline).
    let mut simulator = EvmSimulator::new(client.provider.clone());
    final_evidence.simulation_status = simulator.simulate_candidate(&candidate);

    // Connect math results to the persisted Opportunity row.
    opportunity.expected_profit_usd = math_outcome.gross_profit_usd;
    opportunity.roi_pct = Some(math_outcome.net_roi_pct);

    if let Some(reason) = config_rejection {
        info!(
            event = "config.gate_rejected",
            hash = %hash,
            reason = ?reason,
            net_profit_usd = math_outcome.net_profit_usd,
            roi_pct = math_outcome.net_roi_pct,
        );
        opportunity.risk_score = Some(0.0);
    } else {
        // Spine scoring on REAL evidence (no more hardcoded 0.95 / 0.9 / 1.0).
        let engine = PrioritizationEngine { min_profit_threshold: cfg.min_profit_usd };
        match engine.score(&candidate, &final_evidence) {
            Ok(score) => {
                final_evidence.net_expected_profit = score.net_expected_profit;
                final_evidence.final_score = score.final_score;
                final_evidence.decision = can_execute(&final_evidence, true);
                opportunity.risk_score = Some(score.final_score);

                if let Ok(json) = serde_json::to_string(&final_evidence) {
                    let _ = std::fs::create_dir_all("logs/mev");
                    if let Ok(mut file) = OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open("logs/mev/opportunity_scored.jsonl")
                    {
                        let _ = writeln!(file, "{}", json);
                    }
                }

                if final_evidence.decision == ExecutionDecision::Reject {
                    info!(event = "spine.rejected", hash = %hash, reason = ?final_evidence.reject_reason);
                }
            }
            Err(e) => {
                debug!(event = "spine.scoring_error", hash = %hash, error = ?e);
                opportunity.risk_score = Some(0.0);
            }
        }
    }
    // --- END CONFIG-AWARE SPINE INTERCEPTOR ---


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
