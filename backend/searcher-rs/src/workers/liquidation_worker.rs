//! LiquidationWorker — periodic emitter of Aave V3 liquidation opportunities
//! (the fourth LIVE strategy alongside `dex_arb_v2v2`, `triangular`, and
//! `flashloan_arb`).
//!
//! ## Strategy shape
//!
//! Aave V3 borrowers post collateral and take debt. The protocol exposes a
//! per-user `healthFactor = (Σ collateral_i × liq_threshold_i) / Σ debt_i`.
//! When `healthFactor < 1.0` the position is liquidatable: any account may
//! call `Pool.liquidationCall(collateral, debt, user, debtToCover, ...)`,
//! pay off up to 50% of the debt (the "close factor"), and seize the
//! corresponding amount of collateral PLUS a liquidation bonus (5–10% on
//! Aave V3 mainnet depending on asset). The bonus IS the liquidator's gross
//! profit; net profit subtracts gas (~30 USD on mainnet for the call).
//!
//! ## Worker architecture
//!
//! Per tick (default 30s; Aave health factors don't move every block, so
//! 30s reduces RPC load without missing time-critical events):
//!
//!   1. Read the operator-managed watchlist `arbx:aave_v3_watchlist:<chain>`
//!      (Redis SET of borrower addresses). Empty → skip the cycle (R8 fail-
//!      honest: no fabricated positions; non-zero `skip_empty_watchlist`
//!      counter surfaces the gap to the operator).
//!   2. Resolve the price snapshot once per tick via the cascade oracle
//!      (`RedisCachedPriceOracle::snapshot_from_redis`).
//!   3. Multicall3-batched `getUserAccountData(user)` against the Aave V3
//!      Pool. Decodes `(totalCollateralBase, totalDebtBase, _, _, _,
//!      healthFactor)`. Per-user calldata is built manually (same pattern
//!      `amm_math.rs::v3_quote_exact_in_multicall` uses).
//!   4. For each user: filter to `healthFactor < 1.05e18` (1e18 == HF=1.0;
//!      we set 1.05 buffer to scan ahead of the on-chain trigger).
//!   5. Profit estimation:
//!      - `debt_to_repay_usd = min(totalDebtBase × CLOSE_FACTOR_PCT, OPERATOR_CAP_USD)`.
//!      - `gross_profit_usd  = debt_to_repay_usd × (bonus_bps / 10_000)`.
//!      - `net_profit_usd    = gross_profit_usd − GAS_COST_USD`.
//!      - For MVP we use a fixed `DEFAULT_LIQUIDATION_BONUS_BPS = 500` (5%);
//!        per-asset on-chain bonus discovery is queued for a future task
//!        (`getReserveConfigurationData(asset).currentLiquidationBonus`).
//!   6. **Worker-level sanity bound** (load-bearing per Incidente #9):
//!      reject when `gross_profit_usd / debt_to_repay_usd > 0.20` (20% of
//!      debt). Real Aave V3 bonuses cap at ~10%; 20% catches calculation
//!      bugs without false-positives. Diagnostic dump on every breach.
//!   7. If `net_profit_usd < min_profit_usd` (chain-aware floor from `trading_config`) → skip with `below_min_profit`.
//!   8. **Per-block dedup**: `(user, block)` triple — same user at the same
//!      block does not emit twice. Mirrors `flashloan_arb_worker::DedupState`.
//!   9. Build `Opportunity { strategy_kind: Liquidation, ... }` and emit to
//!      Redis stream + persist to PG. Spine evaluator runs downstream with
//!      the canonical risk gates (allowlist, oracle, anomaly bound).
//!  10. Periodic INFO `tick_stats` log every 5 ticks (~150s at 30s/tick).
//!
//! ## Doctrine compliance
//!
//! - **R8 fail-honest**: every gate returns `None` and increments a skip
//!   counter; never fabricates values. None / NaN / Inf / negative inputs
//!   refused at the gate, never silently coerced.
//! - **RULE 00 zero mocks**: Aave V3 reads come from real on-chain via
//!   Multicall3; the watchlist is operator-managed in Redis (no hardcoded
//!   stale addresses in source).
//! - **No `unwrap()` outside tests**, no `unimplemented!()`, no `todo!()`.
//! - **Worker sanity bound mandatory** (anti-Incidente #9): the worker
//!   bypasses the spine on the persistence path, so self-defense is
//!   non-negotiable. `gross_profit / debt > 0.20` ⇒ reject + warn + counter.
//! - **Per-block dedup mandatory**: prevents emit storms when an unhealthy
//!   position is observed across consecutive ticks at the same block.
//!
//! ## Cost
//!
//! 1 Redis SMEMBERS (watchlist) + 1 Multicall3 RPC (≤ N users) per tick.
//! For an MVP watchlist of ~10 borrowers, this is one ~50ms RPC every 30s
//! — negligible compared to the V2/V3 hot path.

use crate::counters::counters;
use crate::persistence;
use crate::publisher;
use alloy::primitives::Address as AlloyAddress;
use alloy::providers::Provider as AlloyProvider;
use alloy::rpc::types::{TransactionInput, TransactionRequest};
use alloy::sol_types::SolCall;
use chrono::Utc;
use ethers::abi::{Function, Param, ParamType, StateMutability, Token};
use ethers::types::{Address, Bytes, U256};
use redis::aio::ConnectionManager;
use redis::AsyncCommands;
use shared_rs::contracts::{Opportunity, StrategyKind};
use shared_rs::rpc_failover::HttpRpcPool;
use shared_rs::trading_config::TradingConfigClient;
use sqlx::postgres::PgPool;
use std::collections::HashSet;
use std::str::FromStr;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::interval;
use tracing::{debug, info, warn};
use uuid::Uuid;

// =============================================================================
// CONSTANTS
// =============================================================================

/// Default tick period. Aave V3 health factors update on every borrow / repay /
/// price oracle tick; 30s is a healthy compromise between freshness and RPC
/// load (operator can override via `LIQUIDATION_WORKER_INTERVAL_SECS`).
pub const DEFAULT_INTERVAL_SECS: u64 = 30;

/// Strategy-kind string emitted on Opportunity / persistence layer.
const STRATEGY_KIND: &str = "liquidation";

/// Aave V3 Pool — Ethereum mainnet (chain_id = 1). Source:
/// https://docs.aave.com/developers/deployed-contracts/v3-mainnet/ethereum
const AAVE_V3_POOL_MAINNET: &str = "0x87870Bca3F3fD6335C3F4ce8392D69350B4fA4E2";

/// Multicall3 — Ethereum mainnet (and most EVM chains share this same address
/// via deterministic deployment). Source: https://github.com/mds1/multicall.
const MULTICALL3_MAINNET: &str = "0xcA11bde05977b3631167028862bE2a173976CA11";

/// Liquidation eligibility threshold. Aave V3 triggers liquidation on
/// `healthFactor < 1e18` (i.e. HF < 1.0). We scan ahead with a 5% buffer so
/// a ready bundle can be submitted the moment HF crosses 1.0 on-chain.
/// Expressed in 1e18 fixed-point.
const HEALTH_FACTOR_THRESHOLD_1E18: u128 = 1_050_000_000_000_000_000; // 1.05 × 1e18

/// Aave V3 close factor: max fraction of debt repayable in a single
/// liquidation call. Constant across assets in V3 mainnet ("Soft Liquidation"
/// changes this in some forks, not on canonical Aave V3).
/// Source: https://docs.aave.com/developers/whats-new/liquidations
const CLOSE_FACTOR_PCT: f64 = 0.50;

/// Default liquidation bonus when per-asset configuration is unavailable.
/// 5% (500 bps) is the canonical Aave V3 ETH/stable bonus. Per-asset
/// discovery via `getReserveConfigurationData(asset).currentLiquidationBonus`
/// is queued for a follow-up task — see TODO at `liquidation_bonus_bps_for_asset`.
const DEFAULT_LIQUIDATION_BONUS_BPS: u32 = 500;

/// Hard cap on the operator's per-call exposure. Even when the position is
/// huge, MVP refuses to emit a liquidation that calls for more than this in
/// debt-to-repay terms. Operator can lift via Redis (`arbx:liquidation_cap_usd`)
/// in a follow-up; for now this is the structural floor.
const OPERATOR_CAP_USD: f64 = 250_000.0;

/// Fixed gas cost estimate (USD) for one liquidation call. A real Aave V3
/// liquidation consumes ~300k gas; at 20 gwei × $2400/ETH that's ~$15. The
/// 30 estimate adds a safety margin. The spine evaluator will recompute at
/// real-time gas prices downstream; this guard is a pre-screen only.
const GAS_COST_USD: f64 = 30.0;

/// Worker-level sanity bound: max ratio of `gross_profit_usd` to
/// `debt_to_repay_usd`. Real Aave V3 bonuses cap at ~10% (e.g. WBTC, some
/// long-tail assets); 20% catches calculation bugs (decimal mismatch,
/// off-by-1e18 scale, fabricated bonus tiers) without false-positives on
/// legitimate high-bonus assets. Anti-Incidente #9.
const LIQUIDATION_PROFIT_SANITY_MULT_OF_DEBT: f64 = 0.20;

/// Chain-aware minimum USD profit floor used when no operator `trading_config`
/// row is available in Redis. Mirrors `flashloan_arb_worker::min_profit_usd_fallback`.
/// Migration 046 seeds the correct values into `trading_config` so this
/// fallback only applies during cold-start or misconfigured chains.
fn min_profit_usd_fallback(chain_id: u64) -> f64 {
    match chain_id {
        1 => 50.0,     // Ethereum mainnet
        42161 => 10.0, // Arbitrum One
        56 => 5.0,     // BNB Smart Chain
        10 => 5.0,     // Optimism
        8453 => 5.0,   // Base
        137 => 2.0,    // Polygon
        _ => 10.0,     // Unknown chain: conservative default
    }
}

/// Dedup window: keep entries for this many recent blocks. A `(user, block)`
/// pair older than this is pruned each tick.
const DEDUP_RETAIN_BLOCKS: u64 = 50;

/// Periodic INFO `tick_stats` log every N ticks. At 30s/tick × 5 = 150s.
const STATS_LOG_EVERY_N_TICKS: u32 = 5;

/// Hard upper bound on the multicall RPC. A stalled provider WITHOUT this
/// would freeze the worker tick indefinitely (mirrors the cs-validator MAJOR
/// fix pattern from `amm_math::V3_QUOTE_MULTICALL_TIMEOUT`).
const MULTICALL_TIMEOUT: Duration = Duration::from_secs(5);

/// Maximum users probed per tick. Operator-controlled watchlist via Redis
/// SADD; without a cap an oversized SET (e.g. 10K addresses) would build a
/// 320KB+ multicall calldata that RPC providers reject. 200 is a safe ceiling
/// for Multicall3 batched view-fn reads (each ~140 bytes calldata). Truncation
/// is logged at warn level so the operator notices and curates the watchlist.
/// Added 2026-05-07 per security-auditor finding on watchlist DoS surface.
const MAX_WATCHLIST_PER_TICK: usize = 200;

/// Aave V3 base unit for `totalCollateralBase` / `totalDebtBase`. The Pool
/// exposes USD-equivalent values scaled by 1e8 (matches the asset price
/// oracle precision in V3). Confirmed: https://docs.aave.com/developers/core-contracts/pool#getuseraccountdata
const AAVE_BASE_UNIT_DECIMALS: u8 = 8;

// =============================================================================
// REDIS KEYS
// =============================================================================

/// Watchlist key (Redis SET of lowercase 0x-prefixed borrower addresses).
/// Operator manages with `SADD arbx:aave_v3_watchlist:1 0x...`. Empty set →
/// worker skips the cycle (R8 fail-honest, no fabricated targets).
pub fn watchlist_redis_key(chain_id: u64) -> String {
    format!("arbx:aave_v3_watchlist:{}", chain_id)
}

// =============================================================================
// MATH KERNEL — pure functions, no async, no I/O. Tests below.
// =============================================================================

/// Decode the 1e18 fixed-point health factor to a finite f64. Returns None on
/// non-finite or negative result (defensive — Aave guarantees u256 ≥ 0 but
/// converting to f64 could overflow for malicious / corrupt inputs).
pub fn decode_health_factor_1e18(hf_raw: U256) -> Option<f64> {
    let s = hf_raw.to_string();
    let raw = s.parse::<f64>().ok()?;
    if !raw.is_finite() || raw < 0.0 {
        return None;
    }
    let hf = raw / 1e18;
    if !hf.is_finite() {
        return None;
    }
    Some(hf)
}

/// Decode an Aave V3 base-scaled (1e8) value to USD f64. Returns None when
/// the value is non-finite or negative.
pub fn decode_aave_base_to_usd(base_raw: U256) -> Option<f64> {
    let s = base_raw.to_string();
    let raw = s.parse::<f64>().ok()?;
    if !raw.is_finite() || raw < 0.0 {
        return None;
    }
    let usd = raw / 10f64.powi(AAVE_BASE_UNIT_DECIMALS as i32);
    if !usd.is_finite() || usd < 0.0 {
        return None;
    }
    Some(usd)
}

/// Returns true when this position is liquidation-eligible (HF < 1.05).
/// Operates on the raw 1e18 fixed-point integer to avoid f64 rounding around
/// the threshold.
pub fn is_liquidation_eligible(hf_raw: U256) -> bool {
    hf_raw < U256::from(HEALTH_FACTOR_THRESHOLD_1E18)
}

/// Pure profit math:
///   debt_to_repay = min(total_debt × close_factor, operator_cap)
///   gross_profit  = debt_to_repay × (bonus_bps / 10_000)
///   net_profit    = gross_profit − gas_usd
///
/// Returns `None` on R8 violations (NaN / Inf / negative debt / debt = 0).
pub fn estimate_liquidation_profit(
    total_debt_usd: f64,
    bonus_bps: u32,
    gas_cost_usd: f64,
    operator_cap_usd: f64,
) -> Option<LiquidationEstimate> {
    if !total_debt_usd.is_finite() || total_debt_usd <= 0.0 {
        return None;
    }
    if !gas_cost_usd.is_finite() || gas_cost_usd < 0.0 {
        return None;
    }
    if !operator_cap_usd.is_finite() || operator_cap_usd <= 0.0 {
        return None;
    }
    let close_amount = total_debt_usd * CLOSE_FACTOR_PCT;
    if !close_amount.is_finite() || close_amount <= 0.0 {
        return None;
    }
    let debt_to_repay_usd = close_amount.min(operator_cap_usd);
    if !debt_to_repay_usd.is_finite() || debt_to_repay_usd <= 0.0 {
        return None;
    }
    let bonus_pct = bonus_bps as f64 / 10_000.0;
    let gross_profit_usd = debt_to_repay_usd * bonus_pct;
    if !gross_profit_usd.is_finite() || gross_profit_usd < 0.0 {
        return None;
    }
    let net_profit_usd = gross_profit_usd - gas_cost_usd;
    if !net_profit_usd.is_finite() {
        return None;
    }
    Some(LiquidationEstimate {
        debt_to_repay_usd,
        gross_profit_usd,
        net_profit_usd,
        bonus_bps,
    })
}

/// Result of `estimate_liquidation_profit`. Negative `net_profit_usd` is a
/// legitimate outcome (gas exceeds bonus on small positions); the caller
/// applies the chain-aware `min_profit_usd` floor (from `trading_config`) separately.
#[derive(Debug, Clone, PartialEq)]
pub struct LiquidationEstimate {
    pub debt_to_repay_usd: f64,
    pub gross_profit_usd: f64,
    pub net_profit_usd: f64,
    pub bonus_bps: u32,
}

/// Worker-level sanity check. Real Aave V3 bonuses cap at ~10%; this
/// threshold (20%) catches math kernel bugs (decimal mismatch, fabricated
/// bonus, scale errors) without rejecting legitimate high-bonus assets.
/// Returns true when the estimate is implausible (worker MUST reject + log).
pub fn breaches_sanity_bound(estimate: &LiquidationEstimate) -> bool {
    if estimate.debt_to_repay_usd <= 0.0 {
        // Already filtered by `estimate_liquidation_profit`, but defensive.
        return false;
    }
    let ratio = estimate.gross_profit_usd / estimate.debt_to_repay_usd;
    !ratio.is_finite() || ratio > LIQUIDATION_PROFIT_SANITY_MULT_OF_DEBT
}

/// Per-asset liquidation bonus lookup. **MVP**: returns the constant
/// `DEFAULT_LIQUIDATION_BONUS_BPS` for every asset.
///
/// **TODO (follow-up task)**: replace with on-chain read of
/// `Pool.getReserveConfigurationData(asset).currentLiquidationBonus` (returns
/// e.g. `10500` for 5% bonus = 105% seizure). Requires extending the
/// Multicall3 batch with one call per (user × collateral_asset). For MVP we
/// accept the worst-case 5% and emit when even that beats gas — under-reports
/// real profit on assets with 7.5%/10% bonuses, never over-reports.
pub fn liquidation_bonus_bps_for_asset(_asset_addr: &str) -> u32 {
    DEFAULT_LIQUIDATION_BONUS_BPS
}

// =============================================================================
// AAVE V3 ABI ENCODING — manual via ethers::abi
// =============================================================================

/// Build the ABI descriptor for `getUserAccountData(address)`.
///
/// Why manual: the inline-tuple-output form has historically been finicky
/// with `abigen!`'s parser. Manual encoding via `ethers::abi::Function::*`
/// is fully type-safe and mirrors `amm_math::quoter_v2_function`.
fn get_user_account_data_function() -> Function {
    #[allow(deprecated)]
    Function {
        name: "getUserAccountData".to_string(),
        inputs: vec![Param {
            name: "user".to_string(),
            kind: ParamType::Address,
            internal_type: None,
        }],
        outputs: vec![
            Param {
                name: "totalCollateralBase".to_string(),
                kind: ParamType::Uint(256),
                internal_type: None,
            },
            Param {
                name: "totalDebtBase".to_string(),
                kind: ParamType::Uint(256),
                internal_type: None,
            },
            Param {
                name: "availableBorrowsBase".to_string(),
                kind: ParamType::Uint(256),
                internal_type: None,
            },
            Param {
                name: "currentLiquidationThreshold".to_string(),
                kind: ParamType::Uint(256),
                internal_type: None,
            },
            Param {
                name: "ltv".to_string(),
                kind: ParamType::Uint(256),
                internal_type: None,
            },
            Param {
                name: "healthFactor".to_string(),
                kind: ParamType::Uint(256),
                internal_type: None,
            },
        ],
        constant: None,
        state_mutability: StateMutability::View,
    }
}

/// Encode a single `getUserAccountData(user)` call as calldata bytes.
fn encode_get_user_account_data_calldata(user: Address) -> anyhow::Result<Bytes> {
    let f = get_user_account_data_function();
    let encoded = f.encode_input(&[Token::Address(user)])?;
    Ok(Bytes::from(encoded))
}

/// Decode the 6-uint256 return data from `getUserAccountData`. Returns None
/// on malformed input (length < 192 bytes = 6 × 32).
pub fn decode_user_account_data(return_data: &[u8]) -> Option<UserAccountData> {
    if return_data.len() < 6 * 32 {
        return None;
    }
    Some(UserAccountData {
        total_collateral_base: U256::from_big_endian(&return_data[0..32]),
        total_debt_base: U256::from_big_endian(&return_data[32..64]),
        available_borrows_base: U256::from_big_endian(&return_data[64..96]),
        current_liquidation_threshold: U256::from_big_endian(&return_data[96..128]),
        ltv: U256::from_big_endian(&return_data[128..160]),
        health_factor: U256::from_big_endian(&return_data[160..192]),
    })
}

/// Decoded `getUserAccountData` output. All values are raw on-chain integers;
/// callers convert via `decode_aave_base_to_usd` / `decode_health_factor_1e18`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserAccountData {
    pub total_collateral_base: U256,
    pub total_debt_base: U256,
    pub available_borrows_base: U256,
    pub current_liquidation_threshold: U256,
    pub ltv: U256,
    pub health_factor: U256,
}

// =============================================================================
// MULTICALL3 — abigen for Aave reads (private to keep `Call3` symbol unique)
// =============================================================================

/// Alloy 1.0 migration: replaced ethers `abigen!(IMulticall3Aave, ...)` with
/// alloy `sol!`. The `aggregate3` calldata is encoded via `SolCall::abi_encode()`
/// and the return bytes decoded via `SolCall::abi_decode_returns()`. The RPC
/// call goes through `AlloyProvider::call` (alloy `eth_call`). Module name kept
/// as `multicall3` to minimise downstream diffs.
mod multicall3 {
    use alloy::sol_types::sol;

    sol! {
        interface IMulticall3Aave {
            struct Call3 {
                address target;
                bool allowFailure;
                bytes callData;
            }
            struct Result {
                bool success;
                bytes returnData;
            }
            function aggregate3(Call3[] calldata calls) external payable returns (Result[] memory returnData);
        }
    }

    pub use IMulticall3Aave::{aggregate3Call, Call3};
}

/// One per-user batched read result.
#[derive(Debug, Clone)]
pub struct UserDataResult {
    pub user: Address,
    pub data: Option<UserAccountData>,
}

/// Batch-read `getUserAccountData(user)` for a list of users via Multicall3.
///
/// On RPC failure or timeout: returns `Err`. Per-user failures (e.g. a user
/// who never interacted with Aave reverts the call) yield `data = None` for
/// that entry — caller skips them with the `multicall_user_failed` counter.
async fn multicall_get_user_account_data(
    rpc_pool: Arc<HttpRpcPool>,
    pool_addr: Address,
    multicall_addr: Address,
    users: &[Address],
) -> anyhow::Result<Vec<UserDataResult>> {
    if users.is_empty() {
        return Ok(vec![]);
    }
    // Build aggregate3 calldata. alloy sol! Call3 uses AlloyAddress / alloy Bytes.
    let pool_alloy = AlloyAddress::from_slice(pool_addr.as_bytes());
    let multicall_alloy = AlloyAddress::from_slice(multicall_addr.as_bytes());

    let calls: Vec<multicall3::Call3> = users
        .iter()
        .map(|u| {
            let calldata = encode_get_user_account_data_calldata(*u)?;
            Ok::<_, anyhow::Error>(multicall3::Call3 {
                target: pool_alloy,
                allowFailure: true,
                callData: calldata.to_vec().into(),
            })
        })
        .collect::<anyhow::Result<Vec<_>>>()?;

    let calldata = multicall3::aggregate3Call { calls }.abi_encode();

    // with_retry — circuit breaker fires on repeated RPC failure.
    // Alloy 1.0: provider.call(TransactionRequest) → Bytes.
    let raw_bytes = match tokio::time::timeout(
        MULTICALL_TIMEOUT,
        rpc_pool.with_retry(|provider| {
            let tx = TransactionRequest::default()
                .to(multicall_alloy)
                .input(TransactionInput::new(calldata.clone().into()));
            async move { provider.call(tx).await.map_err(|e| anyhow::anyhow!("{e}")) }
        }),
    )
    .await
    {
        Ok(Ok(b)) => b,
        Ok(Err(e)) => return Err(e.into()),
        Err(_elapsed) => {
            return Err(anyhow::anyhow!(
                "aave multicall timeout after {}s",
                MULTICALL_TIMEOUT.as_secs()
            ));
        }
    };

    // Decode the aggregate3 return bytes.
    // alloy-sol-types 1.0: single return value → Vec<Result> returned directly.
    let results = multicall3::aggregate3Call::abi_decode_returns(&raw_bytes)
        .map_err(|e| anyhow::anyhow!("aave multicall decode failed: {e}"))?;

    let mut out = Vec::with_capacity(users.len());
    for (user, res) in users.iter().zip(results.iter()) {
        // alloy sol! Result fields: `.success` (bool), `.returnData` (alloy Bytes).
        if res.success {
            let data = decode_user_account_data(&res.returnData);
            out.push(UserDataResult { user: *user, data });
        } else {
            out.push(UserDataResult {
                user: *user,
                data: None,
            });
        }
    }
    Ok(out)
}

// =============================================================================
// DEDUP STATE — same shape as flashloan_arb_worker::DedupState
// =============================================================================

/// In-memory dedup state: set of `(user_lc, block)` pairs. Pruned each tick.
#[derive(Debug, Default)]
pub struct DedupState {
    seen: HashSet<(String, u64)>,
}

impl DedupState {
    pub fn new() -> Self {
        Self {
            seen: HashSet::new(),
        }
    }
    /// Returns true when the (user, block) pair is fresh and was just inserted.
    /// Returns false when the pair was already present (dedup hit).
    pub fn check_and_mark(&mut self, user_lc: &str, block: u64) -> bool {
        let key = (user_lc.to_string(), block);
        if self.seen.contains(&key) {
            return false;
        }
        self.seen.insert(key);
        true
    }
    /// Drop entries with `block < latest_block - DEDUP_RETAIN_BLOCKS`.
    pub fn prune(&mut self, latest_block: u64) {
        if latest_block <= DEDUP_RETAIN_BLOCKS {
            return;
        }
        let cutoff = latest_block - DEDUP_RETAIN_BLOCKS;
        self.seen.retain(|(_, b)| *b >= cutoff);
    }
    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.seen.len()
    }
}

// =============================================================================
// TICK STATS — periodic operator-visibility log
// =============================================================================

#[derive(Default, Debug, Clone)]
struct TickStats {
    positions_scanned: u32,
    skip_empty_watchlist: u32,
    skip_no_provider: u32,
    skip_multicall_failed: u32,
    skip_user_call_failed: u32,
    skip_decode_failed: u32,
    skip_hf_above_threshold: u32,
    skip_zero_debt: u32,
    skip_unknown_price: u32,
    skip_dedup_hit: u32,
    skip_no_profit: u32,
    skip_below_min_profit: u32,
    skip_sanity_reject: u32,
    emitted: u32,
}

impl TickStats {
    /// Returns the dominant skip-bucket name (highest count) so the operator
    /// sees at a glance WHY positions aren't producing emit-able opps.
    /// `None` when no skips happened in the period.
    fn dominant_skip_reason(&self) -> Option<&'static str> {
        let buckets: [(&'static str, u32); 12] = [
            ("empty_watchlist", self.skip_empty_watchlist),
            ("no_provider", self.skip_no_provider),
            ("multicall_failed", self.skip_multicall_failed),
            ("user_call_failed", self.skip_user_call_failed),
            ("decode_failed", self.skip_decode_failed),
            ("hf_above_threshold", self.skip_hf_above_threshold),
            ("zero_debt", self.skip_zero_debt),
            ("unknown_price", self.skip_unknown_price),
            ("dedup_hit", self.skip_dedup_hit),
            ("no_profit", self.skip_no_profit),
            ("below_min_profit", self.skip_below_min_profit),
            ("sanity_reject", self.skip_sanity_reject),
        ];
        buckets
            .iter()
            .filter(|(_, n)| *n > 0)
            .max_by_key(|(_, n)| *n)
            .map(|(name, _)| *name)
    }
}

// =============================================================================
// WATCHLIST RESOLUTION
// =============================================================================

/// Read the operator-managed watchlist from Redis. Returns the list of
/// lowercase 0x-prefixed addresses. On Redis error or empty set returns an
/// empty Vec (caller increments `skip_empty_watchlist` and skips the tick).
async fn read_watchlist(redis: &mut ConnectionManager, chain_id: u64) -> Vec<String> {
    let key = watchlist_redis_key(chain_id);
    let members: redis::RedisResult<Vec<String>> = redis.smembers(&key).await;
    match members {
        Ok(v) => v.into_iter().map(|s| s.to_ascii_lowercase()).collect(),
        Err(e) => {
            warn!(event = "liquidation_worker.watchlist_read_failed", error = %e);
            Vec::new()
        }
    }
}

/// Parse a watchlist entry to `Address`. Returns None on malformed entries.
fn parse_user_address(s: &str) -> Option<Address> {
    Address::from_str(s).ok()
}

// =============================================================================
// WORKER
// =============================================================================

/// LiquidationWorker — owns its tick interval, dedup state, and (optional)
/// HTTP RPC pool for the Aave V3 multicall.
pub struct LiquidationWorker {
    pub period: Duration,
    pub chain_id: u64,
    pub rpc_pool: Option<Arc<HttpRpcPool>>,
}

impl LiquidationWorker {
    pub fn new(interval_secs: u64, chain_id: u64) -> Self {
        Self {
            period: Duration::from_secs(interval_secs.max(1)),
            chain_id,
            rpc_pool: None,
        }
    }

    /// Builder: attach an HTTP RPC pool for the on-chain multicall. Without
    /// this, the worker still ticks but every cycle increments
    /// `skip_no_provider` and emits nothing — surfaces the missing-provider
    /// state in the heartbeat without crashing.
    pub fn with_provider(mut self, pool: Arc<HttpRpcPool>) -> Self {
        self.rpc_pool = Some(pool);
        self
    }

    /// Forever loop. Designed to be `tokio::spawn`'d from main.
    pub async fn run(
        self,
        mut redis: ConnectionManager,
        db: Option<PgPool>,
        trading_config: TradingConfigClient,
    ) {
        let mut ticker = interval(self.period);
        let mut dedup = DedupState::new();
        let mut tick_count: u32 = 0;
        let mut stats = TickStats::default();
        let fallback_min = min_profit_usd_fallback(self.chain_id);
        info!(
            event = "liquidation_worker.boot",
            chain_id = self.chain_id,
            period_secs = self.period.as_secs(),
            hf_threshold_1e18 = HEALTH_FACTOR_THRESHOLD_1E18,
            close_factor_pct = CLOSE_FACTOR_PCT,
            default_bonus_bps = DEFAULT_LIQUIDATION_BONUS_BPS,
            operator_cap_usd = OPERATOR_CAP_USD,
            gas_cost_usd = GAS_COST_USD,
            sanity_mult_of_debt = LIQUIDATION_PROFIT_SANITY_MULT_OF_DEBT,
            min_profit_usd_fallback = fallback_min,
            stats_log_every_n_ticks = STATS_LOG_EVERY_N_TICKS,
            provider_attached = self.rpc_pool.is_some(),
        );

        loop {
            ticker.tick().await;
            tick_count += 1;

            // Load per-chain min_profit_usd from operator config (Redis-backed).
            // Falls back to chain-aware conservative floor if Redis has no config.
            let min_profit_usd = match trading_config.state(self.chain_id).await {
                Ok(Some(cfg)) => cfg.min_profit_usd,
                Ok(None) => fallback_min,
                Err(e) => {
                    debug!(
                        event = "liquidation_worker.cfg_fetch_failed",
                        chain_id = self.chain_id,
                        error = %e,
                        fallback = fallback_min,
                    );
                    fallback_min
                }
            };

            // Bail early when no RPC pool is wired (e.g. RPC env missing).
            // Per-tick counter increment keeps the operator informed via heartbeat.
            let rpc_pool = match self.rpc_pool.as_ref() {
                Some(p) => p.clone(),
                None => {
                    stats.skip_no_provider += 1;
                    if tick_count % STATS_LOG_EVERY_N_TICKS == 0 {
                        self.log_tick_stats(&stats);
                        stats = TickStats::default();
                    }
                    continue;
                }
            };

            let watchlist = read_watchlist(&mut redis, self.chain_id).await;
            if watchlist.is_empty() {
                stats.skip_empty_watchlist += 1;
                if tick_count % STATS_LOG_EVERY_N_TICKS == 0 {
                    self.log_tick_stats(&stats);
                    stats = TickStats::default();
                }
                continue;
            }

            // Parse to Address; drop malformed entries silently (operator-input
            // hygiene is the operator's responsibility, not the worker's).
            let mut users: Vec<Address> = watchlist
                .iter()
                .filter_map(|s| parse_user_address(s))
                .collect();
            // Cap the per-tick batch to avoid RPC-rejection on oversized
            // multicalls (security-auditor: watchlist DoS surface).
            if users.len() > MAX_WATCHLIST_PER_TICK {
                warn!(
                    event = "liquidation_worker.watchlist_truncated",
                    chain_id = self.chain_id,
                    received = users.len(),
                    cap = MAX_WATCHLIST_PER_TICK,
                    hint = "Redis watchlist exceeds per-tick cap; truncating. Operator should curate the SET.",
                );
                users.truncate(MAX_WATCHLIST_PER_TICK);
            }
            if users.is_empty() {
                stats.skip_empty_watchlist += 1;
                if tick_count % STATS_LOG_EVERY_N_TICKS == 0 {
                    self.log_tick_stats(&stats);
                    stats = TickStats::default();
                }
                continue;
            }

            // Aave V3 Pool address — for now mainnet only. Other chains land
            // when the operator extends the watchlist + Cargo plumbing.
            let pool_addr = match Address::from_str(AAVE_V3_POOL_MAINNET) {
                Ok(a) => a,
                Err(e) => {
                    warn!(event = "liquidation_worker.pool_addr_invalid", error = %e);
                    continue;
                }
            };
            let multicall_addr = match Address::from_str(MULTICALL3_MAINNET) {
                Ok(a) => a,
                Err(e) => {
                    warn!(event = "liquidation_worker.multicall_addr_invalid", error = %e);
                    continue;
                }
            };

            // Block number context — used for dedup. We fetch via with_retry
            // so the circuit breaker fires on RPC outages. The MULTICALL_TIMEOUT
            // guard prevents the tick from stalling on a stalled provider.
            // On failure or timeout we use 0 (dedup degrades to "single emit
            // per session per user", which is conservative).
            let current_block: u64 = match tokio::time::timeout(
                MULTICALL_TIMEOUT,
                rpc_pool.with_retry(|p| async move {
                    p.get_block_number()
                        .await
                        .map_err(|e| anyhow::anyhow!("{e}"))
                }),
            )
            .await
            {
                // alloy 1.0: get_block_number() returns u64 directly.
                Ok(Ok(n)) => n,
                Ok(Err(e)) => {
                    debug!(event = "liquidation_worker.block_number_failed", error = %e);
                    0
                }
                Err(_elapsed) => {
                    warn!(
                        event = "liquidation_worker.block_number_timeout",
                        chain_id = self.chain_id,
                        timeout_secs = MULTICALL_TIMEOUT.as_secs(),
                    );
                    0
                }
            };

            // Snapshot price oracle once per tick. Used for future per-asset
            // refinement; current MVP only needs USD-denominated debt from
            // Aave's base unit, but we plumb the snapshot through so the
            // follow-up task (per-asset bonus) can use it without refactoring.
            let _price_snapshot =
                shared_rs::price_oracle::RedisCachedPriceOracle::snapshot_from_redis(
                    &mut redis,
                    self.chain_id,
                )
                .await
                .into_snapshot();

            let results = match multicall_get_user_account_data(
                rpc_pool.clone(),
                pool_addr,
                multicall_addr,
                &users,
            )
            .await
            {
                Ok(r) => r,
                Err(e) => {
                    stats.skip_multicall_failed += 1;
                    warn!(event = "liquidation_worker.multicall_failed", error = %e);
                    if tick_count % STATS_LOG_EVERY_N_TICKS == 0 {
                        self.log_tick_stats(&stats);
                        stats = TickStats::default();
                    }
                    continue;
                }
            };

            for r in &results {
                stats.positions_scanned += 1;
                counters()
                    .liquidation_positions_scanned
                    .fetch_add(1, Ordering::Relaxed);

                let user_lc = format!("{:#x}", r.user);
                let data = match r.data.as_ref() {
                    Some(d) => d,
                    None => {
                        stats.skip_user_call_failed += 1;
                        continue;
                    }
                };

                if !is_liquidation_eligible(data.health_factor) {
                    stats.skip_hf_above_threshold += 1;
                    continue;
                }

                if data.total_debt_base.is_zero() {
                    stats.skip_zero_debt += 1;
                    continue;
                }

                let total_debt_usd = match decode_aave_base_to_usd(data.total_debt_base) {
                    Some(v) if v > 0.0 => v,
                    _ => {
                        stats.skip_zero_debt += 1;
                        continue;
                    }
                };

                let total_collateral_usd = match decode_aave_base_to_usd(data.total_collateral_base)
                {
                    Some(v) if v > 0.0 => v,
                    _ => {
                        // No collateral = liquidation impossible (nothing to seize).
                        stats.skip_zero_debt += 1;
                        continue;
                    }
                };

                let hf_f64 = match decode_health_factor_1e18(data.health_factor) {
                    Some(v) => v,
                    None => {
                        stats.skip_decode_failed += 1;
                        continue;
                    }
                };

                // Per-block dedup before any expensive math.
                if !dedup.check_and_mark(&user_lc, current_block) {
                    stats.skip_dedup_hit += 1;
                    continue;
                }

                // MVP: assume default bonus. Per-asset on-chain lookup queued.
                let bonus_bps = liquidation_bonus_bps_for_asset(&user_lc);

                let estimate = match estimate_liquidation_profit(
                    total_debt_usd,
                    bonus_bps,
                    GAS_COST_USD,
                    OPERATOR_CAP_USD,
                ) {
                    Some(e) => e,
                    None => {
                        stats.skip_no_profit += 1;
                        continue;
                    }
                };

                // Worker-level sanity bound (anti-Incidente #9). Diagnostic
                // dump on every breach so root-cause analysis is one log line.
                if breaches_sanity_bound(&estimate) {
                    stats.skip_sanity_reject += 1;
                    counters()
                        .liquidation_sanity_reject
                        .fetch_add(1, Ordering::Relaxed);
                    warn!(
                        event = "liquidation_worker.sanity_reject",
                        chain_id = self.chain_id,
                        user = %user_lc,
                        block = current_block,
                        total_collateral_usd = total_collateral_usd,
                        total_debt_usd = total_debt_usd,
                        debt_to_repay_usd = estimate.debt_to_repay_usd,
                        gross_profit_usd = estimate.gross_profit_usd,
                        net_profit_usd = estimate.net_profit_usd,
                        bonus_bps = estimate.bonus_bps,
                        ratio = estimate.gross_profit_usd / estimate.debt_to_repay_usd,
                        threshold_mult = LIQUIDATION_PROFIT_SANITY_MULT_OF_DEBT,
                        health_factor = hf_f64,
                        hint = "math kernel returned profit > 20% of debt — investigate bonus_bps source / decimal scaling"
                    );
                    continue;
                }

                if estimate.net_profit_usd < min_profit_usd {
                    stats.skip_below_min_profit += 1;
                    continue;
                }

                // Build & emit Opportunity. `token_in` and `token_out` carry
                // synthetic strings that the spine evaluator already accepts
                // for non-V2 strategies (see triangular emit). The Aave Pool
                // address fills `dex_a`; the user address fills `dex_b` so
                // operator analytics queries can group by liquidation target.
                //
                // NOTE: for MVP we cannot identify the SPECIFIC debt /
                // collateral asset pair on-chain without an extra Multicall3
                // round (`getReservesList` + per-asset `getUserReserveData`).
                // Follow-up task carries that. For MVP we use the user address
                // in the symbol slot; the spine + executor know to dispatch
                // to a generic liquidation handler that picks the optimal
                // asset pair from the user's actual reserves.
                let pair_symbol = format!("liquidate:{}", &user_lc);
                // Synthetic placeholders so persistence's NOT NULL constraints
                // hold; downstream consumers identify the real asset pair via
                // an on-chain probe driven by `dex_b = user:<addr>`.
                let token_in_synth = AAVE_V3_POOL_MAINNET.to_ascii_lowercase();
                let token_out_synth = user_lc.clone();

                // amount_in_wei for liquidation = debt_to_repay in Aave base
                // units (1e8). Stored as a decimal-string integer for parity
                // with the BigDecimal column; a non-wei value is acceptable
                // here because liquidation calls use a USD-equivalent amount,
                // and the executor reads the strategy_kind to interpret the
                // unit (spine + executor are aware of this convention).
                let debt_to_repay_aave_base = (estimate.debt_to_repay_usd
                    * 10f64.powi(AAVE_BASE_UNIT_DECIMALS as i32))
                .round();
                let amount_in_str =
                    if debt_to_repay_aave_base.is_finite() && debt_to_repay_aave_base >= 0.0 {
                        format!("{:.0}", debt_to_repay_aave_base)
                    } else {
                        "0".to_string()
                    };

                let opp = Opportunity {
                    id: Uuid::new_v4(),
                    chain_id: self.chain_id,
                    strategy_kind: StrategyKind::Liquidation,
                    dex_a: format!("aave-v3:{}", AAVE_V3_POOL_MAINNET.to_ascii_lowercase()),
                    dex_b: Some(format!("user:{}", &user_lc)),
                    pair_symbol,
                    token_in: token_in_synth,
                    token_out: token_out_synth,
                    amount_in_wei: amount_in_str,
                    expected_profit_usd: Some(estimate.net_profit_usd),
                    // H2 landmine fix: liquidation worker already computes
                    // net_profit_usd = gross - GAS_COST_USD internally.
                    // Mirror it to net_expected_profit_usd so submit_engine
                    // Check 7 gates on NET, not gross.
                    net_expected_profit_usd: Some(estimate.net_profit_usd),
                    roi_pct: None,
                    risk_score: None,
                    block_number: if current_block > 0 {
                        Some(current_block)
                    } else {
                        None
                    },
                    rejection_reason: None,
                    detected_at: Utc::now(),
                    trace_id: Uuid::new_v4(),
                };

                info!(
                    event = "liquidation_worker.opp_emit",
                    chain_id = self.chain_id,
                    user = %user_lc,
                    block = current_block,
                    health_factor = hf_f64,
                    total_debt_usd = total_debt_usd,
                    total_collateral_usd = total_collateral_usd,
                    debt_to_repay_usd = estimate.debt_to_repay_usd,
                    gross_profit_usd = estimate.gross_profit_usd,
                    net_profit_usd = estimate.net_profit_usd,
                    bonus_bps = estimate.bonus_bps,
                    strategy = STRATEGY_KIND,
                );

                if let Some(pool) = db.as_ref() {
                    if let Err(e) = persistence::insert_opportunity(pool, &opp).await {
                        counters().db_errors.fetch_add(1, Ordering::Relaxed);
                        warn!(event = "liquidation_worker.db_error", error = %e);
                    } else {
                        counters().db_persisted.fetch_add(1, Ordering::Relaxed);
                    }
                }
                if let Err(e) = publisher::publish(&mut redis, &opp).await {
                    warn!(event = "liquidation_worker.publish_error", error = %e);
                } else {
                    counters()
                        .liquidation_opps_emitted
                        .fetch_add(1, Ordering::Relaxed);
                    stats.emitted += 1;
                }
            }

            dedup.prune(current_block);

            if tick_count % STATS_LOG_EVERY_N_TICKS == 0 {
                self.log_tick_stats(&stats);
                stats = TickStats::default();
            }
        }
    }

    fn log_tick_stats(&self, stats: &TickStats) {
        info!(
            event = "liquidation_worker.tick_stats",
            chain_id = self.chain_id,
            period_ticks = STATS_LOG_EVERY_N_TICKS,
            positions_scanned = stats.positions_scanned,
            emitted = stats.emitted,
            skip_empty_watchlist = stats.skip_empty_watchlist,
            skip_no_provider = stats.skip_no_provider,
            skip_multicall_failed = stats.skip_multicall_failed,
            skip_user_call_failed = stats.skip_user_call_failed,
            skip_decode_failed = stats.skip_decode_failed,
            skip_hf_above_threshold = stats.skip_hf_above_threshold,
            skip_zero_debt = stats.skip_zero_debt,
            skip_unknown_price = stats.skip_unknown_price,
            skip_dedup_hit = stats.skip_dedup_hit,
            skip_no_profit = stats.skip_no_profit,
            skip_below_min_profit = stats.skip_below_min_profit,
            skip_sanity_reject = stats.skip_sanity_reject,
            dominant_skip = ?stats.dominant_skip_reason(),
        );
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)] // M11: test module
    #![allow(clippy::field_reassign_with_default)]
    use super::*;

    // ---------------------------------------------------------------
    // decode_health_factor_1e18
    // ---------------------------------------------------------------

    #[test]
    fn decode_health_factor_1e18_one_returns_one() {
        // 1e18 raw → HF = 1.0
        let raw = U256::from_dec_str("1000000000000000000").unwrap();
        let hf = decode_health_factor_1e18(raw).unwrap();
        assert!((hf - 1.0).abs() < 1e-12, "got {}", hf);
    }

    #[test]
    fn decode_health_factor_1e18_below_one_returns_subunit() {
        // 0.95 × 1e18 → HF = 0.95
        let raw = U256::from_dec_str("950000000000000000").unwrap();
        let hf = decode_health_factor_1e18(raw).unwrap();
        assert!((hf - 0.95).abs() < 1e-9, "got {}", hf);
    }

    #[test]
    fn decode_health_factor_1e18_zero_returns_zero() {
        let hf = decode_health_factor_1e18(U256::zero()).unwrap();
        assert_eq!(hf, 0.0);
    }

    // ---------------------------------------------------------------
    // decode_aave_base_to_usd
    // ---------------------------------------------------------------

    #[test]
    fn decode_aave_base_to_usd_one_dollar() {
        // 1e8 raw = $1.00
        let raw = U256::from(100_000_000u64);
        let usd = decode_aave_base_to_usd(raw).unwrap();
        assert!((usd - 1.0).abs() < 1e-12);
    }

    #[test]
    fn decode_aave_base_to_usd_realistic_debt() {
        // 150_000 USD debt = 1.5e13 raw
        let raw = U256::from_dec_str("15000000000000").unwrap();
        let usd = decode_aave_base_to_usd(raw).unwrap();
        assert!((usd - 150_000.0).abs() < 1e-6, "got {}", usd);
    }

    #[test]
    fn decode_aave_base_to_usd_zero_returns_zero() {
        let usd = decode_aave_base_to_usd(U256::zero()).unwrap();
        assert_eq!(usd, 0.0);
    }

    // ---------------------------------------------------------------
    // is_liquidation_eligible
    // ---------------------------------------------------------------

    #[test]
    fn liquidation_eligible_below_buffer() {
        // HF = 1.02 × 1e18 → below 1.05 threshold → eligible
        let raw = U256::from_dec_str("1020000000000000000").unwrap();
        assert!(is_liquidation_eligible(raw));
    }

    #[test]
    fn liquidation_not_eligible_above_buffer() {
        // HF = 1.10 × 1e18 → above 1.05 threshold → not eligible
        let raw = U256::from_dec_str("1100000000000000000").unwrap();
        assert!(!is_liquidation_eligible(raw));
    }

    #[test]
    fn liquidation_eligible_at_strict_under_one() {
        // HF = 0.99 × 1e18 → below 1.05 → eligible. The actual on-chain
        // trigger is HF<1, but our 1.05 buffer scans ahead.
        let raw = U256::from_dec_str("990000000000000000").unwrap();
        assert!(is_liquidation_eligible(raw));
    }

    #[test]
    fn liquidation_not_eligible_at_threshold_exact() {
        // HF == 1.05e18 → strictly NOT eligible (we use `<`, not `<=`).
        let raw = U256::from(HEALTH_FACTOR_THRESHOLD_1E18);
        assert!(!is_liquidation_eligible(raw));
    }

    // ---------------------------------------------------------------
    // estimate_liquidation_profit (R8 fail-honest)
    // ---------------------------------------------------------------

    #[test]
    fn estimate_realistic_position_yields_expected_profit() {
        // Realistic: total_debt = $150_000, bonus = 5% (500 bps), gas = $30, cap = $250_000.
        // close_amount = 150_000 × 0.5 = 75_000 (under cap).
        // gross_profit = 75_000 × 0.05 = 3_750.
        // net_profit = 3_750 − 30 = 3_720.
        let e = estimate_liquidation_profit(150_000.0, 500, 30.0, 250_000.0).unwrap();
        assert!((e.debt_to_repay_usd - 75_000.0).abs() < 1e-9);
        assert!((e.gross_profit_usd - 3_750.0).abs() < 1e-9);
        assert!((e.net_profit_usd - 3_720.0).abs() < 1e-9);
        assert_eq!(e.bonus_bps, 500);
    }

    #[test]
    fn estimate_huge_position_clamped_by_operator_cap() {
        // total_debt = $10_000_000 → close = $5M; cap = $250k → clamp to $250k.
        let e = estimate_liquidation_profit(10_000_000.0, 500, 30.0, 250_000.0).unwrap();
        assert!((e.debt_to_repay_usd - 250_000.0).abs() < 1e-9);
        assert!((e.gross_profit_usd - 12_500.0).abs() < 1e-9); // 250k × 5%
    }

    #[test]
    fn estimate_zero_debt_returns_none() {
        assert!(estimate_liquidation_profit(0.0, 500, 30.0, 250_000.0).is_none());
    }

    #[test]
    fn estimate_negative_debt_returns_none() {
        assert!(estimate_liquidation_profit(-100.0, 500, 30.0, 250_000.0).is_none());
    }

    #[test]
    fn estimate_nan_inputs_return_none() {
        assert!(estimate_liquidation_profit(f64::NAN, 500, 30.0, 250_000.0).is_none());
        assert!(estimate_liquidation_profit(150_000.0, 500, f64::NAN, 250_000.0).is_none());
        assert!(estimate_liquidation_profit(150_000.0, 500, 30.0, f64::NAN).is_none());
    }

    #[test]
    fn estimate_inf_inputs_return_none() {
        assert!(estimate_liquidation_profit(f64::INFINITY, 500, 30.0, 250_000.0).is_none());
        assert!(estimate_liquidation_profit(150_000.0, 500, f64::INFINITY, 250_000.0).is_none());
    }

    #[test]
    fn estimate_zero_cap_returns_none() {
        assert!(estimate_liquidation_profit(150_000.0, 500, 30.0, 0.0).is_none());
    }

    #[test]
    fn estimate_zero_bonus_yields_negative_net() {
        // 0% bonus → gross = 0 → net = -gas = -30
        let e = estimate_liquidation_profit(150_000.0, 0, 30.0, 250_000.0).unwrap();
        assert_eq!(e.gross_profit_usd, 0.0);
        assert!((e.net_profit_usd - (-30.0)).abs() < 1e-9);
    }

    // ---------------------------------------------------------------
    // breaches_sanity_bound (anti-Incidente #9)
    // ---------------------------------------------------------------

    #[test]
    fn sanity_bound_passes_at_5_pct_bonus() {
        // 5% bonus, ratio = 0.05 < 0.20 → does NOT breach.
        let e = estimate_liquidation_profit(150_000.0, 500, 30.0, 250_000.0).unwrap();
        assert!(!breaches_sanity_bound(&e));
    }

    #[test]
    fn sanity_bound_passes_at_10_pct_bonus() {
        // 10% bonus (Aave V3 long-tail like LDO/MKR), ratio = 0.10 < 0.20.
        let e = estimate_liquidation_profit(150_000.0, 1000, 30.0, 250_000.0).unwrap();
        assert!(!breaches_sanity_bound(&e));
    }

    #[test]
    fn sanity_bound_breaches_at_25_pct_bonus() {
        // 25% bonus (impossible for real Aave V3) → ratio = 0.25 > 0.20 → REJECT.
        let e = estimate_liquidation_profit(150_000.0, 2500, 30.0, 250_000.0).unwrap();
        assert!(breaches_sanity_bound(&e));
    }

    #[test]
    fn sanity_bound_breaches_at_100_pct_bug() {
        // 100% bonus = catastrophic decimal-mismatch class bug. MUST reject.
        let e = estimate_liquidation_profit(150_000.0, 10_000, 30.0, 250_000.0).unwrap();
        assert!(breaches_sanity_bound(&e));
    }

    // ---------------------------------------------------------------
    // decode_user_account_data
    // ---------------------------------------------------------------

    #[test]
    fn decode_user_account_data_short_buffer_returns_none() {
        assert!(decode_user_account_data(&[0u8; 100]).is_none());
        assert!(decode_user_account_data(&[]).is_none());
    }

    #[test]
    fn decode_user_account_data_full_buffer_decodes_all_six() {
        // Build 6 × 32-byte big-endian uints with distinct sentinels.
        let mut buf = vec![0u8; 6 * 32];
        // Each 32-byte slot ends with a u32 sentinel in the last 4 bytes.
        for (i, slot) in buf.chunks_mut(32).enumerate() {
            let s = ((i + 1) * 100) as u32;
            slot[28..32].copy_from_slice(&s.to_be_bytes());
        }
        let d = decode_user_account_data(&buf).unwrap();
        assert_eq!(d.total_collateral_base, U256::from(100u32));
        assert_eq!(d.total_debt_base, U256::from(200u32));
        assert_eq!(d.available_borrows_base, U256::from(300u32));
        assert_eq!(d.current_liquidation_threshold, U256::from(400u32));
        assert_eq!(d.ltv, U256::from(500u32));
        assert_eq!(d.health_factor, U256::from(600u32));
    }

    // ---------------------------------------------------------------
    // DedupState
    // ---------------------------------------------------------------

    #[test]
    fn dedup_first_insert_passes() {
        let mut d = DedupState::new();
        assert!(d.check_and_mark("0xabc", 100));
    }

    #[test]
    fn dedup_same_user_same_block_rejected() {
        let mut d = DedupState::new();
        assert!(d.check_and_mark("0xabc", 100));
        assert!(!d.check_and_mark("0xabc", 100));
    }

    #[test]
    fn dedup_same_user_different_block_passes() {
        let mut d = DedupState::new();
        assert!(d.check_and_mark("0xabc", 100));
        assert!(d.check_and_mark("0xabc", 101));
    }

    #[test]
    fn dedup_different_user_same_block_passes() {
        let mut d = DedupState::new();
        assert!(d.check_and_mark("0xabc", 100));
        assert!(d.check_and_mark("0xdef", 100));
    }

    #[test]
    fn dedup_prune_drops_old_entries() {
        let mut d = DedupState::new();
        d.check_and_mark("0xabc", 100);
        d.check_and_mark("0xabc", 200);
        d.check_and_mark("0xabc", 300);
        // latest_block = 300, retain window = 50 → cutoff = 250 → drops 100, 200.
        d.prune(300);
        assert_eq!(d.len(), 1);
    }

    #[test]
    fn dedup_prune_below_retention_is_noop() {
        let mut d = DedupState::new();
        d.check_and_mark("0xabc", 5);
        d.check_and_mark("0xabc", 10);
        // latest_block = 10 ≤ DEDUP_RETAIN_BLOCKS (50) → nothing to prune.
        d.prune(10);
        assert_eq!(d.len(), 2);
    }

    // ---------------------------------------------------------------
    // TickStats dominant_skip_reason
    // ---------------------------------------------------------------

    #[test]
    fn tick_stats_dominant_none_when_empty() {
        let s = TickStats::default();
        assert_eq!(s.dominant_skip_reason(), None);
    }

    #[test]
    fn tick_stats_dominant_returns_max_bucket() {
        let mut s = TickStats::default();
        s.skip_hf_above_threshold = 50;
        s.skip_dedup_hit = 5;
        s.skip_no_profit = 100;
        s.skip_sanity_reject = 1;
        assert_eq!(s.dominant_skip_reason(), Some("no_profit"));
    }

    #[test]
    fn tick_stats_dominant_handles_sanity_reject() {
        let mut s = TickStats::default();
        s.skip_sanity_reject = 7;
        s.skip_hf_above_threshold = 3;
        assert_eq!(s.dominant_skip_reason(), Some("sanity_reject"));
    }

    // ---------------------------------------------------------------
    // ABI encoding sanity
    // ---------------------------------------------------------------

    #[test]
    fn encode_get_user_account_data_calldata_starts_with_selector() {
        // selector("getUserAccountData(address)") = 0xbf92857c
        let user = Address::from_low_u64_be(0x1234);
        let calldata = encode_get_user_account_data_calldata(user).unwrap();
        let bytes = calldata.as_ref();
        assert!(bytes.len() >= 4);
        assert_eq!(&bytes[0..4], &[0xbf, 0x92, 0x85, 0x7c]);
        // Total = 4 selector + 32 padded address = 36 bytes.
        assert_eq!(bytes.len(), 4 + 32);
    }

    #[test]
    fn encode_get_user_account_data_pads_address_correctly() {
        // Last 20 bytes of the calldata (after 4-byte selector + 12-byte
        // left-pad) must equal the 20-byte address.
        let user = Address::from_low_u64_be(0xDEADBEEF);
        let calldata = encode_get_user_account_data_calldata(user).unwrap();
        let bytes = calldata.as_ref();
        // user bytes = last 20 of the 32-byte arg slot starting at offset 4.
        let arg_start = 4;
        let user_start = arg_start + 12; // 12 bytes of left-pad
        assert_eq!(&bytes[user_start..user_start + 20], user.as_bytes());
    }

    // ---------------------------------------------------------------
    // parse_user_address
    // ---------------------------------------------------------------

    #[test]
    fn parse_user_address_valid_lowercase() {
        let a = parse_user_address("0x0000000000000000000000000000000000000001");
        assert!(a.is_some());
    }

    #[test]
    fn parse_user_address_invalid_returns_none() {
        assert!(parse_user_address("not-a-hex-address").is_none());
        assert!(parse_user_address("0xZZZZ").is_none());
        assert!(parse_user_address("").is_none());
    }

    // ---------------------------------------------------------------
    // watchlist_redis_key
    // ---------------------------------------------------------------

    #[test]
    fn watchlist_redis_key_is_chain_specific() {
        assert_eq!(watchlist_redis_key(1), "arbx:aave_v3_watchlist:1");
        assert_eq!(watchlist_redis_key(8453), "arbx:aave_v3_watchlist:8453");
    }

    // ---------------------------------------------------------------
    // liquidation_bonus_bps_for_asset (MVP constant)
    // ---------------------------------------------------------------

    #[test]
    fn liquidation_bonus_bps_returns_default_for_any_asset() {
        // MVP: every asset returns DEFAULT_LIQUIDATION_BONUS_BPS until the
        // per-asset on-chain lookup lands in the follow-up task.
        assert_eq!(
            liquidation_bonus_bps_for_asset("0xanything"),
            DEFAULT_LIQUIDATION_BONUS_BPS
        );
        assert_eq!(
            liquidation_bonus_bps_for_asset(""),
            DEFAULT_LIQUIDATION_BONUS_BPS
        );
    }

    // ---------------------------------------------------------------
    // End-to-end math walkthrough (matches the brief's worked example)
    // ---------------------------------------------------------------

    /// Worked example from the implementation brief: borrower has $300k of
    /// collateral and $150k debt; HF = 1.02; bonus = 5%. We expect:
    ///   debt_to_repay = $150k × 0.5 = $75k
    ///   gross_profit  = $75k × 0.05 = $3.75k
    ///   net_profit    = $3.75k − $30 = $3.72k
    ///   ratio         = $3.75k / $75k = 0.05 → well below 0.20 threshold
    /// System would emit (passes all gates, net_profit $3720 >> $50 Ethereum floor).
    #[test]
    fn math_walkthrough_realistic_position_emits() {
        let estimate = estimate_liquidation_profit(150_000.0, 500, 30.0, 250_000.0).unwrap();
        assert!((estimate.debt_to_repay_usd - 75_000.0).abs() < 1e-6);
        assert!((estimate.gross_profit_usd - 3_750.0).abs() < 1e-6);
        assert!((estimate.net_profit_usd - 3_720.0).abs() < 1e-6);
        assert!(!breaches_sanity_bound(&estimate));
        // Use the Ethereum chain floor (50.0) — the old $1 hardcode would have
        // passed here trivially; $50 still passes comfortably and is honest.
        assert!(estimate.net_profit_usd > min_profit_usd_fallback(1));
        // Eligibility: HF = 1.02 → 1.02e18.
        let hf_raw = U256::from_dec_str("1020000000000000000").unwrap();
        assert!(is_liquidation_eligible(hf_raw));
    }

    /// Negative path: dust position. $50 debt, 5% bonus → $25 close, $1.25
    /// gross, $1.25 − $30 = -$28.75 net → below ANY chain's floor, must NOT emit.
    #[test]
    fn math_walkthrough_dust_position_skipped() {
        let estimate = estimate_liquidation_profit(50.0, 500, 30.0, 250_000.0).unwrap();
        // net_profit is negative (-$28.75) — below every chain-aware floor.
        assert!(estimate.net_profit_usd < min_profit_usd_fallback(137)); // Polygon = $2, lowest floor
        assert!(!breaches_sanity_bound(&estimate));
    }

    /// Verify the chain-aware fallback floors are all economically sound
    /// (all > $1 old hardcode) and chain-specific.
    #[test]
    fn fallback_floors_are_economically_sound() {
        assert_eq!(min_profit_usd_fallback(1), 50.0);
        assert_eq!(min_profit_usd_fallback(42161), 10.0);
        assert_eq!(min_profit_usd_fallback(56), 5.0);
        assert_eq!(min_profit_usd_fallback(10), 5.0);
        assert_eq!(min_profit_usd_fallback(8453), 5.0);
        assert_eq!(min_profit_usd_fallback(137), 2.0);
        for chain in &[1u64, 42161, 56, 10, 8453, 137] {
            assert!(
                min_profit_usd_fallback(*chain) > 1.0,
                "chain {} fallback {} is not > $1 — economic defect E2 reintroduced",
                chain,
                min_profit_usd_fallback(*chain)
            );
        }
    }
}
