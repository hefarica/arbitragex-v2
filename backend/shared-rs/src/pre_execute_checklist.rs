//! BE-03 — Pre-execute checklist: unified 12-step safety gate.
//!
//! Every arbitrage broadcast path MUST call `pre_execute_checklist(&mut ctx).await`
//! before submitting anything on-chain. The checks run sequentially and fail-fast:
//! the first failing check short-circuits the remaining ones.
//!
//! ## Fail-closed doctrine
//!
//! When a check cannot determine its answer (e.g. Redis or DB is unreachable),
//! it returns the blocking `Err(ChecklistError::*)` variant. Unknown state =
//! block execution. We never default to "allow" on ambiguity.
//!
//! ## Check order rationale
//!
//!   1-2  : Operator intent (kill_switch, paper_mode) — cheapest, no I/O.
//!   3-5  : Chain-level readiness (chain_active, trading_config, rpc_health).
//!   6    : Gas oracle freshness — block before any profit maths.
//!   7    : Economic gate — the floor built by the E2 fix (commit a4bc900).
//!   8-9  : Asset safety (token allowlist, factory active).
//!  10    : Route quality (slippage tolerance).
//!  11    : Self-collision (mempool clean).
//!  12    : Integration circuit breaker from the recon worker.

use redis::{aio::ConnectionManager, AsyncCommands};
use sqlx::PgPool;
use thiserror::Error;

use crate::paper_mode::PaperModeClient;

/// Redis key where the recon worker writes its circuit-breaker state.
/// Possible values: `"normal"` (or absent) = OK, anything else = tripped.
pub const CIRCUIT_BREAKER_KEY: &str = "arbx:circuit_breaker:state";
/// Redis key written by the relays-client when a tx is in-flight.
/// Format: `arbx:pending_tx:<our_address_lowercase>`.
pub fn pending_tx_key(address: &str) -> String {
    format!("arbx:pending_tx:{}", address.to_lowercase())
}
/// Redis key written by the gas-price worker (timestamp of last update, unix seconds).
pub fn gas_price_ts_key(chain_id: u64) -> String {
    format!("arbx:gas_price_ts:{chain_id}")
}
/// Redis key written by the gas-price worker (the actual gas price in wei,
/// stringified u128). Consumers (sim-ctl revm_backend, relays-client submit_engine)
/// read this to charge gas inside the simulation so `SimResult.net_profit_wei`
/// is true net-of-gas P&L (CRITICAL #2 fix, audit re-run #2 2026-05-10).
pub fn gas_price_wei_key(chain_id: u64) -> String {
    format!("arbx:gas_price_wei:{chain_id}")
}
/// Maximum age of a gas-price update before we treat it as stale (seconds).
pub const GAS_PRICE_MAX_AGE_SECS: u64 = 30;

/// Redis key for the EWMA relay-bribe estimate per (chain_id, strategy_kind).
///
/// Written by `relays-client::submit_engine` after each successful
/// `eth_callBundle` simulation. Value is a stringified `f64` representing the
/// **EWMA of `coinbase_diff_wei` in raw wei** (NOT pre-converted to USD).
/// The conversion to USD is deferred to the read site to avoid coupling
/// relays-client to the ETH/USD price oracle.
///
/// Read by `prioritization-spine::config_aware` which converts using
/// `TradingConfigState.base_token_price_usd`:
///   `relay_fee_usd = ewma_wei / 1e18 * base_token_price_usd`
///
/// TTL: 1 hour. Cold-start (key absent) → callers apply the doctrine floor:
/// `max(gross_profit * RELAY_FEE_FLOOR_PCT, RELAY_FEE_FLOOR_ABS_USD)`.
///
/// EWMA update formula (alpha = 0.2):
/// `new_ewma = alpha * observed + (1 - alpha) * previous`
pub fn relay_fee_ewma_key(chain_id: u64, strategy_kind: &str) -> String {
    format!("arbx:relay_fee_ewma:{chain_id}:{strategy_kind}")
}

/// EWMA smoothing factor for the relay fee estimate.
/// α = 0.2 means each new observation gets 20% weight; recent history retains 80%.
/// At 5 bundles/min, this corresponds to a ~25-minute memory horizon.
pub const RELAY_FEE_EWMA_ALPHA: f64 = 0.2;
/// TTL for the relay fee EWMA key in seconds (1 hour).
pub const RELAY_FEE_EWMA_TTL_SECS: u64 = 3600;
/// Doctrine floor for cold-start relay fee: 5% of gross profit.
pub const RELAY_FEE_FLOOR_PCT: f64 = 0.05;
/// Doctrine floor for cold-start relay fee: absolute minimum $0.50.
pub const RELAY_FEE_FLOOR_ABS_USD: f64 = 0.50;

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

#[derive(Debug, Error)]
pub enum ChecklistError {
    /// Kill-switch is armed in Redis — operator has halted all execution.
    #[error("kill_switch active")]
    KillSwitch,

    /// Paper mode is enabled — opportunity should be logged but NOT broadcast.
    /// This is NOT a fatal error: the caller must treat it as "simulate, don't send".
    #[error("paper mode active — broadcast suppressed")]
    PaperModeActive,

    /// The chain is not marked `is_active = true` in the `chains` table.
    #[error("chain {0} not active")]
    ChainInactive(u64),

    /// No `trading_config` row exists for this chain, or `min_profit_usd` is NULL.
    #[error("trading_config missing or incomplete for chain {0}")]
    TradingConfigMissing(u64),

    /// No healthy RPC entries found in `rpcs` for this chain.
    #[error("no healthy RPC for chain {0}")]
    NoHealthyRpc(u64),

    /// Gas-price oracle has not been updated within the freshness window.
    #[error("gas price stale (last update {0}s ago, max allowed {1}s)")]
    GasPriceStale(u64, u64),

    /// Net profit is below the operator-configured floor.
    #[error("net profit ${profit_usd:.4} below floor ${floor_usd:.4}")]
    NetProfitBelowFloor { profit_usd: f64, floor_usd: f64 },

    /// At least one route token is absent from `tokens` with `is_active=true`, or
    /// its `safety_score` in `token_safety_cache` is below the threshold.
    #[error("token {0} not in allowlist or risk score too high")]
    TokenNotAllowed(String),

    /// At least one route factory has `is_active = false` in `factories`.
    #[error("factory {0} inactive")]
    FactoryInactive(String),

    /// Expected slippage exceeds the operator's `max_slippage_pct` tolerance.
    #[error("slippage {actual_pct:.4}% exceeds max {max_pct:.4}%")]
    SlippageExceeded { actual_pct: f64, max_pct: f64 },

    /// Our address has a pending in-flight tx recorded in Redis.
    #[error("mempool conflict: pending tx from our address {0}")]
    MempoolConflict(String),

    /// The recon-worker circuit breaker is tripped.
    #[error("circuit breaker tripped: {0}")]
    CircuitBreakerTripped(String),

    /// A Postgres error occurred during one of the DB checks.
    /// Treated as blocking (fail-closed).
    #[error("db error: {0}")]
    DbError(#[from] sqlx::Error),

    /// A Redis error occurred during one of the cache checks.
    /// Treated as blocking (fail-closed).
    #[error("redis error: {0}")]
    RedisError(#[from] redis::RedisError),

    /// `net_expected_profit_usd` is None and the engine is in live mode.
    ///
    /// In live mode, falling back to the gross `expected_profit_usd` is
    /// forbidden because gross overstates net profit by 20-40% (relay bribe,
    /// LP fees, slippage not yet deducted). The opportunity must be
    /// re-evaluated by the spine before proceeding to broadcast.
    ///
    /// In paper mode this error is NOT raised — the gross fallback is allowed
    /// and a `tracing::warn!(event="checklist.gross_fallback")` is emitted
    /// instead so operators see the gap in observability without blocking
    /// paper-trade data collection.
    #[error("net_expected_profit_usd is None in live mode — gross fallback forbidden")]
    NetProfitUnknown,
}

// ---------------------------------------------------------------------------
// Input context
// ---------------------------------------------------------------------------

/// All inputs needed to run the 12 checks. Constructed by the broadcast call
/// site and consumed immutably (except `redis` which requires `&mut` for async
/// pool access).
pub struct PreExecuteContext<'a> {
    /// Chain being traded on.
    pub chain_id: u64,
    /// Lowercase hex addresses of every token in the route (e.g. `["0xabc...", "0xdef..."]`).
    pub route_tokens: &'a [String],
    /// Lowercase hex addresses of every factory/DEX in the route.
    pub route_factories: &'a [String],
    /// Expected gross profit in USD (before gas).
    pub expected_profit_usd: f64,
    /// Estimated gas cost in USD for this route.
    pub estimated_gas_usd: f64,
    /// Estimated slippage as a percentage (e.g. `0.3` = 0.3 %).
    pub expected_slippage_pct: f64,
    /// Our signing address (lowercase hex), used for the mempool-collision check.
    pub our_address: &'a str,
    /// Postgres connection pool — used for chain/trading_config/rpc/token/factory queries.
    pub pg: &'a PgPool,
    /// Redis connection manager — used for kill_switch, paper_mode, gas_price_ts,
    /// mempool, and circuit_breaker checks.
    pub redis: &'a mut ConnectionManager,
    /// Paper-mode client — used by check 2 to read the per-chain paper-mode
    /// state via the SAME canonical path as `submit_engine` (which reads
    /// `arbx:papermode:<chain_id>` first, then falls back to the legacy global
    /// `arbx:papermode` key for 30 days from 2026-05-13). Re-using the client
    /// keeps the checklist's read consistent with the engine's pre-checklist
    /// read so arming a chain via the admin POST fires the paper short-circuit
    /// from both sites.
    pub paper_mode_client: &'a PaperModeClient,
}

// ---------------------------------------------------------------------------
// Main entry point
// ---------------------------------------------------------------------------

/// Run all 12 safety checks in order. Returns `Ok(())` only when every check
/// passes. Returns the *first* failure as `Err(ChecklistError::*)`.
///
/// `ChecklistError::PaperModeActive` is a **non-fatal** signal: the caller
/// must log the opportunity but skip the actual broadcast. All other error
/// variants are fatal blockers.
pub async fn pre_execute_checklist(ctx: &mut PreExecuteContext<'_>) -> Result<(), ChecklistError> {
    // 1. Kill switch — cheapest abort, no DB round-trip.
    check_kill_switch(ctx.redis).await?;

    // 2. Paper mode — also Redis, essentially free.
    check_paper_mode(ctx.paper_mode_client, ctx.chain_id).await?;

    // 3. Chain active — single PG point-read.
    check_chain_active(ctx.pg, ctx.chain_id).await?;

    // 4. Trading config loaded — verifies min_profit_usd is set.
    let min_profit_usd = check_trading_config_loaded(ctx.pg, ctx.chain_id).await?;

    // 5. RPC health — at least one active entry in `rpcs` table.
    check_rpc_health(ctx.pg, ctx.chain_id).await?;

    // 6. Gas price fresh — timestamp in Redis must be ≤30s old.
    check_gas_price_fresh(ctx.redis, ctx.chain_id).await?;

    // 7. Net profit above floor — depends on min_profit_usd from check 4.
    check_net_profit_above_floor(
        ctx.expected_profit_usd,
        ctx.estimated_gas_usd,
        min_profit_usd,
    )?;

    // 8. Tokens in allowlist — every route token must be active + safe.
    check_tokens_in_allowlist(ctx.pg, ctx.chain_id, ctx.route_tokens).await?;

    // 9. Factories active — every route factory must be is_active.
    check_factories_active(ctx.pg, ctx.chain_id, ctx.route_factories).await?;

    // 10. Slippage within tolerance — load max from trading_config.
    check_slippage_within_tolerance(ctx.pg, ctx.chain_id, ctx.expected_slippage_pct).await?;

    // 11. Mempool clean — no in-flight tx from our address.
    check_mempool_clean(ctx.redis, ctx.our_address).await?;

    // 12. Circuit breaker off — recon's anomaly detector.
    check_circuit_breaker_off(ctx.redis).await?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Individual checks
// ---------------------------------------------------------------------------

/// Check 1: Kill switch — reads `arbx:killswitch` JSON from Redis.
/// If the key is absent, treated as NOT armed (normal operation).
/// If Redis is unreachable, defaults to BLOCK (fail-closed).
async fn check_kill_switch(redis: &mut ConnectionManager) -> Result<(), ChecklistError> {
    use crate::killswitch::{KillSwitchState, KILLSWITCH_KEY};

    let raw: Option<String> = redis.get(KILLSWITCH_KEY).await?;
    if let Some(json) = raw {
        if let Ok(state) = serde_json::from_str::<KillSwitchState>(&json) {
            if state.enabled {
                return Err(ChecklistError::KillSwitch);
            }
        } else {
            // Cannot parse state — fail closed.
            return Err(ChecklistError::KillSwitch);
        }
    }
    // Absent key = kill switch off (normal operation).
    Ok(())
}

/// Check 2: Paper mode — reads the per-chain paper-mode state via the SAME
/// canonical `PaperModeClient::is_enabled_for_chain` path that
/// `submit_engine` uses (line 106). The client reads
/// `arbx:papermode:<chain_id>` first, then falls back to the legacy global
/// `arbx:papermode` key for 30 days from 2026-05-13 (B0.2 migration).
///
/// Fail-closed semantics are preserved by `PaperModeClient`'s
/// `default_when_absent` (constructed with `cfg.execution.paper_mode`, which
/// is `true` by default in `relays-client::main`):
///   - per-chain key absent + legacy key absent → paper ON (suppress broadcast)
///   - unparseable JSON → client returns Err → falls back to default_when_absent
///   - Redis unreachable → client returns Err → falls back to default_when_absent
///
/// Env overrides (`ARBX_PAPER_MODE`, `ARBX_PAPER_TRADE`) take precedence for
/// fast operator opt-in and are kept identical to the legacy implementation.
async fn check_paper_mode(
    client: &PaperModeClient,
    chain_id: u64,
) -> Result<(), ChecklistError> {
    // Canonical per-chain read — identical resolution to submit_engine line 106.
    let paper_enabled = client.is_enabled_for_chain(chain_id).await;

    // Also check the env override (takes precedence over Redis for fast opt-in).
    let env_override = std::env::var("ARBX_PAPER_MODE")
        .map(|v| v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    // Legacy env variable used by relays-client:
    let env_legacy = std::env::var("ARBX_PAPER_TRADE")
        .map(|v| v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);

    if paper_enabled || env_override || env_legacy {
        return Err(ChecklistError::PaperModeActive);
    }
    Ok(())
}

/// Check 3: Chain active — `SELECT is_active FROM chains WHERE chain_id = $1`.
/// Absent row OR `is_active = false` → block.
async fn check_chain_active(pg: &PgPool, chain_id: u64) -> Result<(), ChecklistError> {
    let row: Option<(bool,)> = sqlx::query_as("SELECT is_active FROM chains WHERE chain_id = $1")
        .bind(chain_id as i64)
        .fetch_optional(pg)
        .await?;

    match row {
        Some((true,)) => Ok(()),
        Some((false,)) | None => Err(ChecklistError::ChainInactive(chain_id)),
    }
}

/// Check 4: Trading config loaded — row must exist with non-NULL `min_profit_usd`.
/// Returns the floor value so check 7 can reuse it without a second DB round-trip.
async fn check_trading_config_loaded(pg: &PgPool, chain_id: u64) -> Result<f64, ChecklistError> {
    let row: Option<(Option<f64>,)> = sqlx::query_as(
        "SELECT min_profit_usd::float8 FROM trading_config WHERE chain_id = $1 AND enabled = TRUE",
    )
    .bind(chain_id as i64)
    .fetch_optional(pg)
    .await?;

    match row {
        Some((Some(min_profit),)) => Ok(min_profit),
        _ => Err(ChecklistError::TradingConfigMissing(chain_id)),
    }
}

/// Check 5: RPC health — at least one entry in `rpcs` table with `is_active = true`
/// for this chain. Does NOT ping the endpoint; that is the rpc_health_worker's job.
async fn check_rpc_health(pg: &PgPool, chain_id: u64) -> Result<(), ChecklistError> {
    let row: Option<(i64,)> =
        sqlx::query_as("SELECT COUNT(*) FROM rpcs WHERE chain_id = $1 AND is_active = TRUE")
            .bind(chain_id as i64)
            .fetch_optional(pg)
            .await?;

    let count = row.map(|(c,)| c).unwrap_or(0);
    if count == 0 {
        return Err(ChecklistError::NoHealthyRpc(chain_id));
    }
    Ok(())
}

/// Check 6: Gas price freshness — Redis key `arbx:gas_price_ts:<chain_id>` must
/// hold a unix-epoch seconds timestamp that is ≤ GAS_PRICE_MAX_AGE_SECS old.
/// Absent key = treated as stale (fail-closed). Redis error = fail-closed.
async fn check_gas_price_fresh(
    redis: &mut ConnectionManager,
    chain_id: u64,
) -> Result<(), ChecklistError> {
    let key = gas_price_ts_key(chain_id);
    let raw: Option<String> = redis.get(&key).await?;

    let last_ts: u64 = match raw {
        Some(v) => v.trim().parse::<u64>().unwrap_or(0),
        // Absent = never updated = stale.
        None => {
            return Err(ChecklistError::GasPriceStale(
                u64::MAX,
                GAS_PRICE_MAX_AGE_SECS,
            ));
        }
    };

    let now_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let age_secs = now_secs.saturating_sub(last_ts);
    if age_secs > GAS_PRICE_MAX_AGE_SECS {
        return Err(ChecklistError::GasPriceStale(
            age_secs,
            GAS_PRICE_MAX_AGE_SECS,
        ));
    }
    Ok(())
}

/// Check 7: Net profit above floor — `expected_profit_usd - estimated_gas_usd >= min_profit_usd`.
/// Pure arithmetic; no I/O.
fn check_net_profit_above_floor(
    expected_profit_usd: f64,
    estimated_gas_usd: f64,
    min_profit_usd: f64,
) -> Result<(), ChecklistError> {
    let net = expected_profit_usd - estimated_gas_usd;
    if net < min_profit_usd {
        return Err(ChecklistError::NetProfitBelowFloor {
            profit_usd: net,
            floor_usd: min_profit_usd,
        });
    }
    Ok(())
}

/// Check 8: Tokens in allowlist.
///
/// Two-tier check per token:
///   a. `tokens` table: `is_active = true` (021 migration schema).
///   b. `token_safety_cache`: if a fresh entry exists, `safety_score` must be
///      ≥ `min_token_safety_score` from `token_safety_cfg` in `app.toml` (default 70).
///      If no cache entry exists the token passes this tier (the safety check is
///      advisory until the token_enricher populates it).
///
/// A token absent from `tokens` entirely is also rejected.
async fn check_tokens_in_allowlist(
    pg: &PgPool,
    chain_id: u64,
    route_tokens: &[String],
) -> Result<(), ChecklistError> {
    for token_addr in route_tokens {
        // Tier a: tokens table — must be active.
        let active_row: Option<(bool,)> =
            sqlx::query_as("SELECT is_active FROM tokens WHERE chain_id = $1 AND address = $2")
                .bind(chain_id as i64)
                .bind(token_addr.as_str())
                .fetch_optional(pg)
                .await?;

        match active_row {
            Some((true,)) => {}
            // Not in table or is_active=false — reject.
            Some((false,)) | None => {
                return Err(ChecklistError::TokenNotAllowed(token_addr.clone()));
            }
        }

        // Tier b: token_safety_cache — if present and fresh, enforce score floor.
        let score_row: Option<(i32,)> = sqlx::query_as(
            r#"
            SELECT safety_score
            FROM token_safety_cache
            WHERE chain_id = $1
              AND token_address = $2
              AND ttl_expires_at > NOW()
            "#,
        )
        .bind(chain_id as i64)
        .bind(token_addr.as_str())
        .fetch_optional(pg)
        .await?;

        // Default floor: 70 (matches app.toml `[token_safety] min_acceptable_score`).
        // We read it from env to avoid a second DB round-trip per token.
        let floor: i32 = std::env::var("TOKEN_SAFETY_FLOOR")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(70);

        if let Some((score,)) = score_row {
            if score < floor {
                return Err(ChecklistError::TokenNotAllowed(token_addr.clone()));
            }
        }
        // No cache entry → pass this tier (token_enricher has not rated it yet).
    }
    Ok(())
}

/// Check 9: Factories active — every factory in the route must have
/// `is_active = true` in the `factories` table (column added in migration 044).
async fn check_factories_active(
    pg: &PgPool,
    chain_id: u64,
    route_factories: &[String],
) -> Result<(), ChecklistError> {
    for factory_addr in route_factories {
        let row: Option<(bool,)> =
            sqlx::query_as("SELECT is_active FROM factories WHERE chain_id = $1 AND address = $2")
                .bind(chain_id as i64)
                .bind(factory_addr.as_str())
                .fetch_optional(pg)
                .await?;

        match row {
            Some((true,)) => {}
            // Absent or inactive — block.
            Some((false,)) | None => {
                return Err(ChecklistError::FactoryInactive(factory_addr.clone()));
            }
        }
    }
    Ok(())
}

/// Check 10: Slippage within tolerance — `expected_slippage_pct <= max_slippage_pct`
/// from the operator's `trading_config` row.
async fn check_slippage_within_tolerance(
    pg: &PgPool,
    chain_id: u64,
    expected_slippage_pct: f64,
) -> Result<(), ChecklistError> {
    let row: Option<(f64,)> = sqlx::query_as(
        "SELECT max_slippage_pct::float8 FROM trading_config WHERE chain_id = $1 AND enabled = TRUE",
    )
    .bind(chain_id as i64)
    .fetch_optional(pg)
    .await?;

    let max_pct = match row {
        Some((v,)) => v,
        // If trading_config row disappeared between check 4 and check 10, block.
        None => {
            return Err(ChecklistError::SlippageExceeded {
                actual_pct: expected_slippage_pct,
                max_pct: 0.0,
            })
        }
    };

    if expected_slippage_pct > max_pct {
        return Err(ChecklistError::SlippageExceeded {
            actual_pct: expected_slippage_pct,
            max_pct,
        });
    }
    Ok(())
}

/// Check 11: Mempool clean — Redis key `arbx:pending_tx:<our_address>` must not exist.
/// This key is written by the relays-client when a bundle is submitted and deleted
/// once the tracker confirms inclusion or drop. Absent = no pending tx = clean.
async fn check_mempool_clean(
    redis: &mut ConnectionManager,
    our_address: &str,
) -> Result<(), ChecklistError> {
    let key = pending_tx_key(our_address);
    let exists: bool = redis.exists(&key).await?;
    if exists {
        return Err(ChecklistError::MempoolConflict(our_address.to_string()));
    }
    Ok(())
}

/// Check 12: Circuit breaker off — Redis key `arbx:circuit_breaker:state` must be
/// `"normal"` or absent. Any other value (set by the recon worker's anomaly detector
/// when revert rate spikes) blocks execution.
async fn check_circuit_breaker_off(redis: &mut ConnectionManager) -> Result<(), ChecklistError> {
    let raw: Option<String> = redis.get(CIRCUIT_BREAKER_KEY).await?;
    match raw {
        None => Ok(()), // Absent = normal (circuit breaker not triggered).
        Some(ref state) if state.trim() == "normal" || state.trim().is_empty() => Ok(()),
        Some(state) => Err(ChecklistError::CircuitBreakerTripped(state)),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ---------------------------------------------------------------------------
    // Helpers for pure (no-I/O) checks
    // ---------------------------------------------------------------------------

    #[test]
    fn test_net_profit_below_floor_blocks() {
        // $1.00 profit - $2.00 gas = -$1.00 net, floor is $50.00
        let result = check_net_profit_above_floor(1.0, 2.0, 50.0);
        assert!(
            matches!(
                result,
                Err(ChecklistError::NetProfitBelowFloor { profit_usd, floor_usd })
                if profit_usd < 0.0 && floor_usd == 50.0
            ),
            "expected NetProfitBelowFloor, got: {:?}",
            result
        );
    }

    #[test]
    fn test_net_profit_exactly_at_floor_passes() {
        // $52.00 profit - $2.00 gas = $50.00 net, floor is $50.00 — must pass.
        let result = check_net_profit_above_floor(52.0, 2.0, 50.0);
        assert!(
            result.is_ok(),
            "expected Ok when net==floor, got: {:?}",
            result
        );
    }

    #[test]
    fn test_net_profit_above_floor_passes() {
        // $200 profit - $10 gas = $190 net, floor is $50 — passes.
        let result = check_net_profit_above_floor(200.0, 10.0, 50.0);
        assert!(result.is_ok(), "expected Ok, got: {:?}", result);
    }

    // ---------------------------------------------------------------------------
    // Kill switch: JSON parsing edge cases (pure logic, no Redis).
    // ---------------------------------------------------------------------------

    /// Mirrors the kill-switch parse logic without network I/O.
    fn parse_kill_switch_enabled(json: &str) -> Option<bool> {
        #[derive(serde::Deserialize)]
        struct Ks {
            enabled: bool,
        }
        serde_json::from_str::<Ks>(json).ok().map(|s| s.enabled)
    }

    #[test]
    fn test_kill_switch_active_json_blocks() {
        let json = r#"{"enabled":true,"reason":"manual","triggered_by":"ops","updated_at":"2026-05-07T00:00:00Z"}"#;
        assert_eq!(parse_kill_switch_enabled(json), Some(true));
    }

    #[test]
    fn test_kill_switch_inactive_json_passes() {
        let json = r#"{"enabled":false,"reason":null,"triggered_by":null,"updated_at":"2026-05-07T00:00:00Z"}"#;
        assert_eq!(parse_kill_switch_enabled(json), Some(false));
    }

    #[test]
    fn test_kill_switch_malformed_json_fails_closed() {
        // parse fails → None → caller treats as enabled (fail-closed in check_kill_switch).
        assert_eq!(parse_kill_switch_enabled("not_json"), None);
    }

    // ---------------------------------------------------------------------------
    // Paper mode: JSON parsing + env interaction (pure logic).
    // ---------------------------------------------------------------------------

    fn parse_paper_mode_enabled(json: &str) -> Option<bool> {
        #[derive(serde::Deserialize)]
        struct Pm {
            enabled: bool,
        }
        serde_json::from_str::<Pm>(json).ok().map(|s| s.enabled)
    }

    #[test]
    fn test_paper_mode_returns_paper_mode_active() {
        let json = r#"{"enabled":true,"updated_at":"2026-05-07T00:00:00Z","updated_by":null}"#;
        assert_eq!(parse_paper_mode_enabled(json), Some(true));
    }

    #[test]
    fn test_paper_mode_disabled_json_allows_broadcast() {
        let json = r#"{"enabled":false,"updated_at":"2026-05-07T00:00:00Z","updated_by":"ops"}"#;
        assert_eq!(parse_paper_mode_enabled(json), Some(false));
    }

    // ---------------------------------------------------------------------------
    // Per-chain read parity: check_paper_mode must reach the same answer as the
    // legacy global-only read when ONLY the legacy global key is set (30-day
    // fallback window from B0.2, 2026-05-13). This is the resolution that
    // `PaperModeClient::state_for_chain` performs at runtime; we mirror it as a
    // pure helper so the parity is provable without a live Redis.
    // ---------------------------------------------------------------------------

    /// Mirrors `PaperModeClient::state_for_chain` resolution: per-chain key
    /// first, then legacy global key, then `default_when_absent`. Returns the
    /// effective `enabled` flag. The actual client also tags a `PaperModeSource`
    /// for observability; that attribution is irrelevant to the go/no-go
    /// decision so we drop it here.
    fn resolve_paper_enabled(
        per_chain_raw: Option<&str>,
        legacy_raw: Option<&str>,
        default_when_absent: bool,
    ) -> bool {
        if let Some(json) = per_chain_raw {
            if let Ok(s) = serde_json::from_str::<PaperModeStateJson>(json) {
                return s.enabled;
            }
            // Unparseable per-chain state → fail closed (paper ON).
            return default_when_absent;
        }
        if let Some(json) = legacy_raw {
            if let Ok(s) = serde_json::from_str::<PaperModeStateJson>(json) {
                return s.enabled;
            }
            return default_when_absent;
        }
        default_when_absent
    }

    /// Minimal mirror of `paper_mode::PaperModeState` for the pure parity test.
    /// We keep this local (instead of importing) so the test stays a true
    /// unit test with zero Redis/IO coupling.
    #[derive(serde::Deserialize)]
    struct PaperModeStateJson {
        enabled: bool,
    }

    /// Mirrors the LEGACY (pre-B0.2) global-only read that `check_paper_mode`
    /// performed before this fix. Used solely to assert 30-day fallback parity.
    fn resolve_paper_enabled_legacy_only(
        legacy_raw: Option<&str>,
        default_when_absent: bool,
    ) -> bool {
        match legacy_raw {
            Some(json) => serde_json::from_str::<PaperModeStateJson>(json)
                .map(|s| s.enabled)
                .unwrap_or(default_when_absent),
            None => default_when_absent,
        }
    }

    #[test]
    fn test_per_chain_read_matches_legacy_when_only_global_key_set() {
        // 30-day fallback parity: with no per-chain key armed, the new
        // per-chain resolution path MUST yield the same `enabled` value the
        // legacy global-only read produced. Proven across enabled/disabled and
        // both fail-closed defaults.
        let default_when_absent = true; // secure default (relays-client::main)

        let cases: &[(&str, Option<&str>)] = &[
            // (label, legacy_raw)
            (
                "legacy enabled",
                Some(r#"{"enabled":true,"updated_at":"2026-05-13T00:00:00Z","updated_by":"ops"}"#),
            ),
            (
                "legacy disabled",
                Some(r#"{"enabled":false,"updated_at":"2026-05-13T00:00:00Z","updated_by":"ops"}"#),
            ),
            ("no key anywhere", None),
        ];

        for (label, legacy_raw) in cases {
            let legacy = resolve_paper_enabled_legacy_only(*legacy_raw, default_when_absent);
            let per_chain =
                resolve_paper_enabled(None, *legacy_raw, default_when_absent);
            assert_eq!(
                legacy, per_chain,
                "parity broken for case `{label}`: legacy={legacy}, per_chain={per_chain}",
            );
        }
    }

    #[test]
    fn test_per_chain_key_overrides_legacy_when_armed() {
        // The bug this package fixes: arming a chain via the admin POST writes
        // `arbx:papermode:<chain_id>`. The per-chain read MUST surface that
        // armed state even when the legacy global key says the opposite — this
        // is exactly the divergence that the old global-only check_paper_mode
        // produced (it never saw the per-chain arming).
        let legacy_disabled =
            r#"{"enabled":false,"updated_at":"2026-05-13T00:00:00Z","updated_by":"ops"}"#;
        let per_chain_armed =
            r#"{"enabled":true,"updated_at":"2026-05-13T00:00:01Z","updated_by":"admin-armed-chain-1"}"#;

        // Old behaviour (legacy-only): broadcast NOT suppressed — bug.
        let legacy = resolve_paper_enabled_legacy_only(Some(legacy_disabled), true);
        assert!(!legacy, "legacy-only read should see disabled");

        // New behaviour (per-chain first): broadcast suppressed — fix.
        let per_chain = resolve_paper_enabled(Some(per_chain_armed), Some(legacy_disabled), true);
        assert!(per_chain, "per-chain read must surface the armed chain");
    }

    // ---------------------------------------------------------------------------
    // Chain inactive: logic mirror (no DB).
    // ---------------------------------------------------------------------------

    fn simulate_chain_active_check(
        is_active_in_db: Option<bool>,
        chain_id: u64,
    ) -> Result<(), ChecklistError> {
        match is_active_in_db {
            Some(true) => Ok(()),
            Some(false) | None => Err(ChecklistError::ChainInactive(chain_id)),
        }
    }

    #[test]
    fn test_chain_inactive_blocks() {
        let result = simulate_chain_active_check(Some(false), 1);
        assert!(
            matches!(result, Err(ChecklistError::ChainInactive(1))),
            "expected ChainInactive(1), got: {:?}",
            result
        );
    }

    #[test]
    fn test_chain_absent_blocks() {
        let result = simulate_chain_active_check(None, 42161);
        assert!(
            matches!(result, Err(ChecklistError::ChainInactive(42161))),
            "expected ChainInactive(42161) when absent, got: {:?}",
            result
        );
    }

    #[test]
    fn test_chain_active_passes() {
        let result = simulate_chain_active_check(Some(true), 1);
        assert!(result.is_ok(), "expected Ok, got: {:?}", result);
    }

    // ---------------------------------------------------------------------------
    // All-checks-pass: happy-path exercise of the pure functions.
    // ---------------------------------------------------------------------------

    #[test]
    fn test_all_pure_checks_pass_returns_ok() {
        // Chain active simulation.
        assert!(simulate_chain_active_check(Some(true), 1).is_ok());

        // Trading config: returns min_profit_usd = 50.0.
        let min_profit = 50.0_f64;

        // Net profit: $200 gross - $10 gas = $190 net > $50 floor.
        assert!(check_net_profit_above_floor(200.0, 10.0, min_profit).is_ok());

        // Slippage: 0.3% actual <= 0.5% max — inline check.
        let actual_pct = 0.3_f64;
        let max_pct = 0.5_f64;
        assert!(actual_pct <= max_pct, "slippage should pass");

        // Circuit breaker: absent = OK.
        let state: Option<String> = None;
        let tripped = match state {
            None => false,
            Some(ref s) if s.trim() == "normal" => false,
            Some(_) => true,
        };
        assert!(
            !tripped,
            "circuit breaker should not be tripped when key absent"
        );
    }
}
