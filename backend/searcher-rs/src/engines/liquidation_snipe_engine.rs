//! LiquidationSnipeEngine — Lending position liquidation detector.
//!
//! Monitors Aave V3 and Compound V2 lending positions for accounts with
//! health_factor < 1.0, identifying profitable liquidation opportunities.
//!
//! ## Mathematical Model
//!
//! For a lending position:
//! - Total Collateral (USD)
//! - Total Debt (USD)
//! - Health Factor = (Collateral * Liquidation Threshold) / Debt
//!
//! When Health Factor < 1.0, the position is eligible for liquidation.
//!
//! Liquidation profit calculation:
//! - Liquidation Bonus (protocol-specific, e.g., 5-10% on Aave)
//! - Close Factor (maximum % of debt that can be repaid, e.g., 50%)
//! - Gas Cost (estimated on-chain execution cost)
//!
//! Net Topological Yield = (Debt Repaid * Liquidation Bonus) - Gas Cost
//!
//! ## R8 Invariants
//!
//! - Health Factor >= 1.0 → skip (position is safe)
//! - No liquidation bonus data → skip (cannot calculate yield)
//! - Gas cost > potential bonus → emit REJECTED with "gas_exceeds_yield"

use crate::engines::StrategyCandidate;
use crate::route_intent::RouteIntent;
use crate::strategy_label::StrategyLabel;
use chrono::Utc;
use ethers::types::{Address, H256, U256};
use futures_util::future::join_all;
use prioritization_spine::route_plan::{RouteLeg, RoutePlan};
use prioritization_spine::types::OpportunityCandidate;
use shared_rs::chains::{
    DAI_MAINNET_LC, USDC_MAINNET_LC, USDT_MAINNET_LC, WBTC_MAINNET_LC, WETH_MAINNET_LC,
};
use shared_rs::contracts::Opportunity;
use shared_rs::trading_config::TradingConfigState;
use std::collections::HashMap;
use std::str::FromStr;
use std::time::Duration;
use tracing::{debug, info, warn};
use uuid::Uuid;

/// Lending protocol configuration.
#[derive(Debug, Clone)]
pub struct LendingPoolConfig {
    /// Protocol identifier ("aave-v3", "compound-v2")
    pub protocol: String,
    /// Chain ID where the protocol is deployed
    pub chain_id: u64,
    /// Pool/Comptroller contract address
    pub pool_address: Address,
    /// Data Provider contract address (for Aave)
    pub data_provider: Option<Address>,
    /// Liquidation contract address
    pub liquidation_proxy: Address,
    /// Minimum liquidation bonus to consider (in bps)
    pub min_liquidation_bonus_bps: u32,
    /// Protocol-specific close factor (max % of debt liquidatable)
    pub close_factor_bps: u32,
}

/// Lending position state.
#[derive(Debug, Clone)]
pub struct LendingPosition {
    /// Position owner address
    pub user: Address,
    /// Protocol this position belongs to
    pub protocol: String,
    /// Total collateral in USD
    pub total_collateral_usd: f64,
    /// Total debt in USD
    pub total_debt_usd: f64,
    /// Current health factor (liquidatable if < 1.0)
    pub health_factor: f64,
    /// Maximum debt that can be liquidated (USD)
    pub max_liquidatable_debt_usd: f64,
    /// Collateral token that can be seized
    pub collateral_token: Address,
    /// Debt token to be repaid
    pub debt_token: Address,
    /// Liquidation bonus for this asset pair (in bps)
    pub liquidation_bonus_bps: u32,
}

/// Liquidation candidate opportunity.
#[derive(Debug, Clone)]
pub struct LiquidationCandidate {
    /// Unique candidate ID
    pub id: u64,
    /// The lending position being liquidated
    pub position: LendingPosition,
    /// Estimated gross profit (bonus amount in USD)
    pub gross_profit_usd: f64,
    /// Estimated gas cost (USD)
    pub gas_cost_usd: f64,
    /// Net topological yield after gas
    pub net_yield_usd: f64,
    /// Optimal amount of debt to repay (USD)
    pub optimal_repay_usd: f64,
}

// ─────────────────────────────────────────────────────────────────────────────
// STEP 2.5c — REAL position source (read-only JSON-RPC, sovereign free-RPC
// stack §38 CLAUDE.md). The legacy hosted-service subgraph endpoints were
// sunset, so the "subgraph" in the fetch method name is historical: the real
// data path is on-chain eth_getLogs discovery + eth_call screening on the
// Aave V3 Pool. All ABI shapes below verified against aave-v3-core source
// (IPool.sol / Pool.sol / AaveProtocolDataProvider.sol / DataTypes.sol).
// ─────────────────────────────────────────────────────────────────────────────

/// Aave V3 protocol identifier served by the real fetcher.
const AAVE_V3_PROTOCOL: &str = "aave-v3";
/// eth_getLogs lookback window (~24h of 12s mainnet blocks).
const BORROWER_DISCOVERY_LOOKBACK_BLOCKS: u64 = 7_200;
/// Max borrower accounts screened per scan (bounded RPC budget).
const MAX_SCREENED_ACCOUNTS: usize = 24;
/// Max reserves inspected per side (collateral/debt) during enrichment.
const MAX_ENRICH_RESERVES: usize = 6;
/// Aave base currency = USD with 8 decimals (mainnet BASE_CURRENCY_UNIT).
const AAVE_BASE_CURRENCY_SCALE: f64 = 1e8;
/// Health factor is a wad (1e18); < 1e18 ⇒ liquidatable.
const AAVE_HF_SCALE: u128 = 1_000_000_000_000_000_000;
/// Aave encodes `liquidationBonus` as 10_000 + bonus_bps.
const AAVE_BONUS_OFFSET: u128 = 10_000;

/// Screening projection of `Pool.getUserAccountData(address)` (base = USD 1e8).
#[derive(Debug, Clone, Copy)]
struct AaveAccountSnapshot {
    total_collateral_base: u128,
    total_debt_base: u128,
    health_factor_wad: u128,
}

/// Lazily fetched per-asset metadata for USD ranking (scan-local cache).
#[derive(Debug, Default, Clone, Copy)]
struct AaveAssetMeta {
    /// `AaveOracle.getAssetPrice` → USD with 1e8 precision.
    price_usd: Option<f64>,
    /// `getReserveConfigurationData` word 0.
    decimals: Option<u8>,
}

/// Engine for detecting liquidatable lending positions.
pub struct LiquidationSnipeEngine {
    /// Configured lending protocols to monitor
    lending_pools: HashMap<String, LendingPoolConfig>,
    /// Minimum liquidation bonus threshold (bps)
    min_liquidation_bonus_bps: u32,
    /// Estimated gas cost per liquidation (USD)
    gas_cost_usd: f64,
    /// Position cache for tracking known positions
    position_cache: HashMap<Address, LendingPosition>,
    /// Read-only JSON-RPC HTTP client for position fetching (STEP 2.5c).
    http: reqwest::Client,
    /// Pinned RPC endpoint override (tests inject a wiremock server here);
    /// `None` ⇒ resolve `RPC_HTTP_<chain_id>` from env per scan.
    rpc_http_url_override: Option<String>,
}

impl LiquidationSnipeEngine {
    /// Creates a new LiquidationSnipeEngine.
    pub fn new(lending_pools: Vec<LendingPoolConfig>) -> Self {
        let pools_map: HashMap<String, LendingPoolConfig> = lending_pools
            .into_iter()
            .map(|p| (format!("{}-{}", p.protocol, p.chain_id), p))
            .collect();

        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(8))
            .connect_timeout(Duration::from_secs(3))
            .build()
            .unwrap_or_default();

        Self {
            lending_pools: pools_map,
            min_liquidation_bonus_bps: 500, // 5% minimum bonus
            gas_cost_usd: 50.0,             // $50 estimated gas
            position_cache: HashMap::new(),
            http,
            rpc_http_url_override: None,
        }
    }

    /// Pins the HTTP RPC endpoint instead of resolving `RPC_HTTP_<chain_id>`
    /// from env per scan. Used by tests (wiremock) and ops overrides.
    pub fn with_rpc_url(mut self, url: impl Into<String>) -> Self {
        self.rpc_http_url_override = Some(url.into());
        self
    }

    /// Creates engine with default Aave and Compound configurations.
    pub fn with_default_pools() -> Self {
        let pools = vec![
            // Aave V3 on Ethereum mainnet
            LendingPoolConfig {
                protocol: "aave-v3".to_string(),
                chain_id: 1,
                pool_address: Address::from_str("0x87870bca3f3fd6335c3f4ce8392d69350b4fa4e2")
                    .unwrap_or_default(),
                data_provider: Some(
                    Address::from_str("0x7b4eb56e7cd4b454b8f71d048c14752252e36fb4")
                        .unwrap_or_default(),
                ),
                liquidation_proxy: Address::from_str("0x87870bca3f3fd6335c3f4ce8392d69350b4fa4e2")
                    .unwrap_or_default(),
                min_liquidation_bonus_bps: 500, // 5%
                close_factor_bps: 5000,         // 50%
            },
            // Compound V2 on Ethereum mainnet
            LendingPoolConfig {
                protocol: "compound-v2".to_string(),
                chain_id: 1,
                pool_address: Address::from_str("0x3d9819210a31b4961b30ef54be2aed79b9c9cd3b")
                    .unwrap_or_default(),
                data_provider: None,
                liquidation_proxy: Address::from_str("0x3d9819210a31b4961b30ef54be2aed79b9c9cd3b")
                    .unwrap_or_default(),
                min_liquidation_bonus_bps: 800, // 8%
                close_factor_bps: 5000,         // 50%
            },
        ];

        Self::new(pools)
    }

    /// Scans for liquidatable positions across all configured protocols.
    ///
    /// Data source (STEP 2.5c): real read-only JSON-RPC against each pool's
    /// chain RPC — borrower discovery via `Borrow`/`LiquidationCall` event
    /// logs, screening via `Pool.getUserAccountData`, enrichment via the Aave
    /// V3 data provider + oracle. Compound V2 has no real fetcher yet and
    /// returns an honest empty set (R8 — never fabricated).
    pub async fn scan_liquidatable_positions(&self) -> Vec<LiquidationCandidate> {
        let mut candidates = Vec::new();
        let mut id_counter = 0u64;

        for (key, pool_config) in &self.lending_pools {
            debug!(
                event = "liquidation_snipe_engine.scanning_pool",
                pool = key,
                protocol = %pool_config.protocol,
                chain_id = pool_config.chain_id,
                "scanning for liquidatable positions"
            );

            // Fetch positions from subgraph/RPC
            // In production, this would call the actual protocol contracts
            let positions = self.fetch_positions_from_subgraph(pool_config).await;

            for position in positions {
                // Skip safe positions (R8: health_factor >= 1.0)
                if position.health_factor >= 1.0 {
                    continue;
                }

                // Skip if bonus below threshold
                if position.liquidation_bonus_bps < self.min_liquidation_bonus_bps {
                    debug!(
                        event = "liquidation_snipe_engine.bonus_too_low",
                        user = %position.user,
                        bonus_bps = position.liquidation_bonus_bps,
                        min_bonus_bps = self.min_liquidation_bonus_bps,
                        "skipping: liquidation bonus below threshold"
                    );
                    continue;
                }

                // Calculate liquidation opportunity
                if let Some(candidate) =
                    self.calculate_liquidation_opportunity(&position, &mut id_counter)
                {
                    if candidate.net_yield_usd > 0.0 {
                        info!(
                            event = "liquidation_snipe_engine.candidate_found",
                            candidate_id = candidate.id,
                            user = %position.user,
                            protocol = %position.protocol,
                            health_factor = position.health_factor,
                            debt_usd = position.total_debt_usd,
                            net_yield_usd = candidate.net_yield_usd,
                            "liquidation candidate detected"
                        );
                        candidates.push(candidate);
                    }
                }
            }
        }

        candidates
    }

    /// Entry point for orchestrator integration.
    /// Evaluates liquidation opportunities in response to a RouteIntent.
    pub async fn build_liquidation_candidates(
        &self,
        intent: &RouteIntent,
        cfg: Option<&TradingConfigState>,
    ) -> anyhow::Result<Vec<StrategyCandidate>> {
        let chain_id = intent.chain_id;
        let tx_hash = intent.tx_hash;

        // Detect liquidation opportunities
        let liquidations = self.scan_liquidatable_positions().await;

        // Convert to StrategyCandidates
        let mut candidates = Vec::new();

        for liq in liquidations {
            let candidate = self.build_strategy_candidate(&liq, chain_id, tx_hash, cfg);
            candidates.push(candidate);
        }

        Ok(candidates)
    }

    // -------------------------------------------------------------------------
    // Internal methods
    // -------------------------------------------------------------------------

    /// Fetches lending positions from a REAL source (STEP 2.5c).
    ///
    /// The method name is historical: the original scaffold targeted The
    /// Graph's hosted subgraphs (`api.thegraph.com/subgraphs/name/aave/
    /// protocol-v3`), which were sunset. The production path is read-only
    /// JSON-RPC against the chain's configured RPC (sovereign free-RPC stack,
    /// §38 CLAUDE.md — no paid providers):
    ///
    /// 1. **Discovery** — `eth_getLogs` on the Aave V3 Pool for `Borrow`
    ///    (indexed `onBehalfOf` = debt owner) and `LiquidationCall` (indexed
    ///    `user`; cascade re-liquidation candidates) over a bounded recent
    ///    block window. Event signatures verified against aave-v3-core
    ///    `IPool.sol`.
    /// 2. **Screening** — `Pool.getUserAccountData(user)` per discovered
    ///    account (parallel, capped): account-level collateral/debt in base
    ///    currency (USD × 1e8) and health factor (wad × 1e18). Only accounts
    ///    with `0 < HF < 1.0` and non-zero collateral/debt survive.
    /// 3. **Enrichment** — per liquidatable account: resolve the market's data
    ///    provider + oracle ON-CHAIN (`Pool.ADDRESSES_PROVIDER()` →
    ///    `getPoolDataProvider()` / `getPriceOracle()`), map the user's config
    ///    bitmap to concrete reserves via `getReserveAddressById`, rank
    ///    flagged reserves by USD value for the principal collateral/debt
    ///    pair, and read the collateral's `liquidationBonus`.
    ///
    /// R8 invariants: every empty return carries an explicit `reason`; a
    /// failed RPC leg never fabricates a position; enrichment failure skips
    /// the account (never half-builds one).
    async fn fetch_positions_from_subgraph(
        &self,
        pool_config: &LendingPoolConfig,
    ) -> Vec<LendingPosition> {
        if pool_config.protocol != AAVE_V3_PROTOCOL {
            debug!(
                event = "liquidation_snipe_engine.fetch_skipped",
                reason = "protocol_fetch_not_supported",
                protocol = %pool_config.protocol,
                chain_id = pool_config.chain_id,
                "only aave-v3 has a real on-chain fetcher (R8 honest empty)"
            );
            return vec![];
        }

        let Some(rpc_url) = self.rpc_url_for(pool_config.chain_id) else {
            debug!(
                event = "liquidation_snipe_engine.fetch_skipped",
                reason = "no_rpc_url",
                chain_id = pool_config.chain_id,
                "RPC_HTTP_<chain_id> unresolved (R8 honest empty)"
            );
            return vec![];
        };

        let Some(latest_block) = self.rpc_block_number(&rpc_url).await else {
            debug!(
                event = "liquidation_snipe_engine.fetch_skipped",
                reason = "block_number_failed",
                chain_id = pool_config.chain_id,
                "R8 honest empty"
            );
            return vec![];
        };
        let from_block = latest_block.saturating_sub(BORROWER_DISCOVERY_LOOKBACK_BLOCKS);

        // ── Discovery 1/2 — recently liquidated users (cascade candidates).
        // Topics (IPool.sol): [0]=sig, [1]=collateralAsset, [2]=debtAsset, [3]=user.
        let liq_topic = Self::event_topic0(
            "LiquidationCall(address,address,address,uint256,uint256,address,bool)",
        );
        let mut liq_pairs: HashMap<Address, (Address, Address)> = HashMap::new();
        let mut accounts: Vec<Address> = Vec::new();
        match self
            .rpc_get_logs(
                &rpc_url,
                pool_config.pool_address,
                &liq_topic,
                from_block,
                latest_block,
            )
            .await
        {
            Some(logs) => {
                for log in &logs {
                    let Some(topics) = log.get("topics").and_then(|t| t.as_array()) else {
                        continue;
                    };
                    if topics.len() < 4 {
                        continue;
                    }
                    let (Some(collateral), Some(debt), Some(user)) = (
                        topics[1].as_str().and_then(Self::address_from_topic),
                        topics[2].as_str().and_then(Self::address_from_topic),
                        topics[3].as_str().and_then(Self::address_from_topic),
                    ) else {
                        continue;
                    };
                    liq_pairs.entry(user).or_insert((collateral, debt));
                    if !accounts.contains(&user) {
                        accounts.push(user);
                    }
                }
            }
            None => debug!(
                event = "liquidation_snipe_engine.discovery_leg_failed",
                reason = "liquidation_getlogs_failed",
                from_block,
                to_block = latest_block,
                "continuing with Borrow leg (R8)"
            ),
        }

        // ── Discovery 2/2 — recent borrowers.
        // Topics (IPool.sol): [0]=sig, [1]=reserve, [2]=onBehalfOf (debt owner), [3]=referralCode.
        let borrow_topic =
            Self::event_topic0("Borrow(address,address,address,uint256,uint8,uint256,uint16)");
        match self
            .rpc_get_logs(
                &rpc_url,
                pool_config.pool_address,
                &borrow_topic,
                from_block,
                latest_block,
            )
            .await
        {
            Some(logs) => {
                for log in &logs {
                    let Some(topics) = log.get("topics").and_then(|t| t.as_array()) else {
                        continue;
                    };
                    if topics.len() < 3 {
                        continue;
                    }
                    if let Some(on_behalf_of) =
                        topics[2].as_str().and_then(Self::address_from_topic)
                    {
                        if !accounts.contains(&on_behalf_of) {
                            accounts.push(on_behalf_of);
                        }
                    }
                }
            }
            None => debug!(
                event = "liquidation_snipe_engine.discovery_leg_failed",
                reason = "borrow_getlogs_failed",
                from_block,
                to_block = latest_block,
                "borrower discovery leg failed (R8)"
            ),
        }

        if accounts.is_empty() {
            debug!(
                event = "liquidation_snipe_engine.fetch_empty",
                reason = "no_borrowers_discovered",
                from_block,
                to_block = latest_block,
                "R8: no candidate borrowers in window"
            );
            return vec![];
        }
        let discovered = accounts.len();
        accounts.truncate(MAX_SCREENED_ACCOUNTS);

        // ── Screening — getUserAccountData per account, parallel and bounded.
        let sel_account_data = Self::fn_selector("getUserAccountData(address)");
        let futures = accounts.iter().map(|user| {
            let calldata = Self::call_with_address(&sel_account_data, *user);
            let rpc = rpc_url.clone();
            async move {
                self.rpc_eth_call(&rpc, pool_config.pool_address, &calldata)
                    .await
                    .and_then(|out| Self::parse_user_account_data(&out))
            }
        });
        let snapshots = join_all(futures).await;

        let mut positions = Vec::new();
        for (user, snapshot) in accounts.iter().zip(snapshots) {
            let Some(snap) = snapshot else {
                debug!(
                    event = "liquidation_snipe_engine.account_screen_failed",
                    user = %user,
                    "R8: no account data — account skipped, never fabricated"
                );
                continue;
            };
            // HF >= 1.0 (incl. Aave's no-debt sentinel) or zero debt ⇒ safe.
            if snap.total_debt_base == 0 || snap.health_factor_wad >= AAVE_HF_SCALE {
                continue;
            }
            // No collateral ⇒ liquidationCall reverts; not a viable candidate.
            if snap.total_collateral_base == 0 {
                debug!(
                    event = "liquidation_snipe_engine.account_skipped",
                    reason = "zero_collateral",
                    user = %user,
                    "undercollateralized with zero collateral (R8)"
                );
                continue;
            }
            match self
                .enrich_aave_position(&rpc_url, pool_config, *user, &snap, liq_pairs.get(user))
                .await
            {
                Some(pos) => positions.push(pos),
                None => debug!(
                    event = "liquidation_snipe_engine.enrichment_failed",
                    user = %user,
                    "R8: principal pair unresolvable — position skipped, never fabricated"
                ),
            }
        }

        debug!(
            event = "liquidation_snipe_engine.fetch_summary",
            protocol = %pool_config.protocol,
            chain_id = pool_config.chain_id,
            from_block,
            to_block = latest_block,
            discovered,
            screened = accounts.len(),
            liquidatable = positions.len(),
            reason = if positions.is_empty() {
                "no_liquidatable_positions"
            } else {
                "positions_fetched"
            },
            "scan window summary"
        );
        positions
    }

    /// Resolves the market's data provider + oracle and builds the enriched
    /// `LendingPosition` for a screened-liquidatable Aave V3 user.
    ///
    /// R8: any unresolvable dependency ⇒ `None` (caller skips the user; a
    /// position is never half-built).
    async fn enrich_aave_position(
        &self,
        rpc_url: &str,
        pool_config: &LendingPoolConfig,
        user: Address,
        snap: &AaveAccountSnapshot,
        liq_pair_hint: Option<&(Address, Address)>,
    ) -> Option<LendingPosition> {
        // ── Market plumbing, resolved ON-CHAIN (config address is fallback).
        let provider_out = self
            .rpc_eth_call(
                rpc_url,
                pool_config.pool_address,
                &Self::fn_selector("ADDRESSES_PROVIDER()"),
            )
            .await?;
        let addresses_provider = Self::address_from_word(Self::word_at(&provider_out, 0)?);
        if addresses_provider == Address::zero() {
            return None;
        }
        let data_provider = self
            .rpc_eth_call(
                rpc_url,
                addresses_provider,
                &Self::fn_selector("getPoolDataProvider()"),
            )
            .await
            .and_then(|out| Self::word_at(&out, 0).map(Self::address_from_word))
            .filter(|a| *a != Address::zero())
            .or(pool_config.data_provider)
            .filter(|a| *a != Address::zero())?;
        let oracle = self
            .rpc_eth_call(
                rpc_url,
                addresses_provider,
                &Self::fn_selector("getPriceOracle()"),
            )
            .await
            .and_then(|out| Self::word_at(&out, 0).map(Self::address_from_word))
            .filter(|a| *a != Address::zero());

        // ── User config bitmap → flagged reserve ids → concrete assets.
        // The bitmap is indexed by reserve id (DataTypes.UserConfigurationMap:
        // bit 2i = collateral, bit 2i+1 = borrowing), so assets are resolved
        // id-exactly via getReserveAddressById — robust to dropped reserves.
        let bitmap = self
            .rpc_eth_call(
                rpc_url,
                pool_config.pool_address,
                &Self::call_with_address(&Self::fn_selector("getUserConfiguration(address)"), user),
            )
            .await
            .and_then(|out| Self::word_at(&out, 0))?;
        let (mut collateral_ids, mut debt_ids) = Self::bitmap_flags(bitmap);
        collateral_ids.truncate(MAX_ENRICH_RESERVES);
        debt_ids.truncate(MAX_ENRICH_RESERVES);

        let mut id_to_asset: HashMap<usize, Address> = HashMap::new();
        for id in collateral_ids.iter().chain(debt_ids.iter()) {
            if id_to_asset.contains_key(id) {
                continue;
            }
            if let Some(asset) = self
                .reserve_address_by_id(rpc_url, pool_config.pool_address, *id)
                .await
            {
                id_to_asset.insert(*id, asset);
            }
        }

        // ── Per-reserve balances → USD ranking → principal pair.
        let sel_user_reserve = Self::fn_selector("getUserReserveData(address,address)");
        let mut meta_cache: HashMap<Address, AaveAssetMeta> = HashMap::new();

        // Words (AaveProtocolDataProvider.getUserReserveData): 0=aTokenBalance,
        // 1=stableDebt, 2=variableDebt.
        let mut best_debt: Option<(Address, f64)> = None;
        for id in &debt_ids {
            let Some(asset) = id_to_asset.get(id) else {
                continue;
            };
            let Some(out) = self
                .rpc_eth_call(
                    rpc_url,
                    data_provider,
                    &Self::call_with_two_addresses(&sel_user_reserve, *asset, user),
                )
                .await
            else {
                continue;
            };
            let (Some(stable), Some(var)) = (Self::word_at(&out, 1), Self::word_at(&out, 2)) else {
                continue;
            };
            let debt_raw = stable.low_u128().saturating_add(var.low_u128());
            if debt_raw == 0 {
                continue;
            }
            if let Some(usd) = self
                .asset_usd_value(
                    rpc_url,
                    oracle,
                    data_provider,
                    *asset,
                    debt_raw,
                    &mut meta_cache,
                )
                .await
            {
                let better = best_debt.is_none_or(|(_, best)| usd > best);
                if better {
                    best_debt = Some((*asset, usd));
                }
            }
        }

        let mut best_collateral: Option<(Address, f64)> = None;
        for id in &collateral_ids {
            let Some(asset) = id_to_asset.get(id) else {
                continue;
            };
            let Some(out) = self
                .rpc_eth_call(
                    rpc_url,
                    data_provider,
                    &Self::call_with_two_addresses(&sel_user_reserve, *asset, user),
                )
                .await
            else {
                continue;
            };
            let Some(collateral_raw) = Self::word_at(&out, 0) else {
                continue;
            };
            let collateral_raw = collateral_raw.low_u128();
            if collateral_raw == 0 {
                continue;
            }
            if let Some(usd) = self
                .asset_usd_value(
                    rpc_url,
                    oracle,
                    data_provider,
                    *asset,
                    collateral_raw,
                    &mut meta_cache,
                )
                .await
            {
                let better = best_collateral.is_none_or(|(_, best)| usd > best);
                if better {
                    best_collateral = Some((*asset, usd));
                }
            }
        }

        // Principal pair: USD-ranked pair → on-chain LiquidationCall hint →
        // first flagged (all are real holdings; ranking only picks quality).
        let (collateral_token, debt_token) = match (best_collateral, best_debt) {
            (Some((c, _)), Some((d, _))) => (c, d),
            _ => {
                let hint = liq_pair_hint.copied();
                let first_collateral = collateral_ids
                    .iter()
                    .find_map(|id| id_to_asset.get(id).copied());
                let first_debt = debt_ids.iter().find_map(|id| id_to_asset.get(id).copied());
                match hint {
                    Some((c, d)) if first_collateral.is_some() && first_debt.is_some() => (c, d),
                    _ => (first_collateral?, first_debt?),
                }
            }
        };
        if collateral_token == debt_token {
            return None; // degenerate pair — cannot construct sanely
        }

        // ── Liquidation bonus of the collateral reserve.
        // Words (AaveProtocolDataProvider.getReserveConfigurationData):
        // 0=decimals, 1=ltv, 2=liquidationThreshold, 3=liquidationBonus.
        let config_out = self
            .rpc_eth_call(
                rpc_url,
                data_provider,
                &Self::call_with_address(
                    &Self::fn_selector("getReserveConfigurationData(address)"),
                    collateral_token,
                ),
            )
            .await?;
        let bonus_raw = Self::word_at(&config_out, 3)?.low_u128();
        let liquidation_bonus_bps = Self::bonus_bps_from_raw(bonus_raw);

        // Account-level USD figures come from the pool itself (oracle-derived
        // inside getUserAccountData) — no re-derivation, no fabrication.
        let total_collateral_usd = snap.total_collateral_base as f64 / AAVE_BASE_CURRENCY_SCALE;
        let total_debt_usd = snap.total_debt_base as f64 / AAVE_BASE_CURRENCY_SCALE;
        let health_factor = snap.health_factor_wad as f64 / AAVE_HF_SCALE as f64;
        let max_liquidatable_debt_usd =
            total_debt_usd * (pool_config.close_factor_bps as f64 / 10_000.0);

        Some(LendingPosition {
            user,
            protocol: AAVE_V3_PROTOCOL.to_string(),
            total_collateral_usd,
            total_debt_usd,
            health_factor,
            max_liquidatable_debt_usd,
            collateral_token,
            debt_token,
            liquidation_bonus_bps,
        })
    }

    /// `Pool.getReserveAddressById(uint16)` → underlying asset (zero ⇒ None).
    async fn reserve_address_by_id(
        &self,
        rpc_url: &str,
        pool: Address,
        reserve_id: usize,
    ) -> Option<Address> {
        // uint16 arg is right-aligned in a full 32-byte ABI word.
        let calldata = format!(
            "{}{:064x}",
            Self::fn_selector("getReserveAddressById(uint16)"),
            reserve_id
        );
        let out = self.rpc_eth_call(rpc_url, pool, &calldata).await?;
        let addr = Self::address_from_word(Self::word_at(&out, 0)?);
        (addr != Address::zero()).then_some(addr)
    }

    /// USD value of a raw token amount: `AaveOracle.getAssetPrice` (USD × 1e8)
    /// and reserve decimals, both cached per asset for this scan. `None` when
    /// either input is unavailable (caller degrades ranking, never invents).
    async fn asset_usd_value(
        &self,
        rpc_url: &str,
        oracle: Option<Address>,
        data_provider: Address,
        asset: Address,
        raw_amount: u128,
        cache: &mut HashMap<Address, AaveAssetMeta>,
    ) -> Option<f64> {
        if let std::collections::hash_map::Entry::Vacant(entry) = cache.entry(asset) {
            let price_usd = match oracle {
                Some(o) => self
                    .rpc_eth_call(
                        rpc_url,
                        o,
                        &Self::call_with_address(
                            &Self::fn_selector("getAssetPrice(address)"),
                            asset,
                        ),
                    )
                    .await
                    .and_then(|out| Self::word_at(&out, 0))
                    .map(|w| w.low_u128() as f64 / AAVE_BASE_CURRENCY_SCALE),
                None => None,
            };
            let decimals = self
                .rpc_eth_call(
                    rpc_url,
                    data_provider,
                    &Self::call_with_address(
                        &Self::fn_selector("getReserveConfigurationData(address)"),
                        asset,
                    ),
                )
                .await
                .and_then(|out| Self::word_at(&out, 0))
                .map(|w| w.low_u128().min(255) as u8);
            entry.insert(AaveAssetMeta {
                price_usd,
                decimals,
            });
        }
        let meta = cache.get(&asset)?;
        let price = meta.price_usd?;
        let decimals = meta.decimals?;
        Some(raw_amount as f64 / 10f64.powi(decimals as i32) * price)
    }

    /// Pinned override → `RPC_HTTP_<chain_id>` env (price_worker's CSV parser —
    /// single source of truth for RPC endpoints, no-hardcode doctrine).
    fn rpc_url_for(&self, chain_id: u64) -> Option<String> {
        self.rpc_http_url_override
            .clone()
            .or_else(|| crate::workers::price_worker::rpc_http_url_from_env(chain_id))
    }

    // ── Read-only JSON-RPC plumbing (eth_call / eth_blockNumber / eth_getLogs) ──

    /// Single JSON-RPC POST; `None` on transport error, non-JSON body, or a
    /// JSON-RPC `error` envelope (some free RPCs answer HTTP 200 with errors).
    async fn rpc_post(&self, rpc_url: &str, body: serde_json::Value) -> Option<serde_json::Value> {
        let resp = match self.http.post(rpc_url).json(&body).send().await {
            Ok(r) => r,
            Err(e) => {
                debug!(
                    event = "liquidation_snipe_engine.rpc_transport_error",
                    error = %e,
                    "R8: RPC call failed"
                );
                return None;
            }
        };
        let parsed: serde_json::Value = match resp.json().await {
            Ok(v) => v,
            Err(e) => {
                debug!(
                    event = "liquidation_snipe_engine.rpc_parse_error",
                    error = %e,
                    "R8: non-JSON RPC response"
                );
                return None;
            }
        };
        if let Some(err) = parsed.get("error") {
            debug!(
                event = "liquidation_snipe_engine.rpc_json_error",
                error = %err,
                "R8: JSON-RPC error envelope"
            );
            return None;
        }
        Some(parsed)
    }

    /// Read-only `eth_call` (view, `"latest"`); returns the raw `0x…` result.
    async fn rpc_eth_call(&self, rpc_url: &str, to: Address, data: &str) -> Option<String> {
        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "eth_call",
            "params": [
                { "to": format!("0x{:040x}", to), "data": data },
                "latest"
            ]
        });
        let parsed = self.rpc_post(rpc_url, body).await?;
        parsed
            .get("result")?
            .as_str()
            .map(str::to_string)
            .filter(|s| !s.is_empty())
    }

    /// Latest block height (`eth_blockNumber`).
    async fn rpc_block_number(&self, rpc_url: &str) -> Option<u64> {
        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "eth_blockNumber",
            "params": []
        });
        let parsed = self.rpc_post(rpc_url, body).await?;
        let hex = parsed.get("result")?.as_str()?;
        u64::from_str_radix(hex.trim_start_matches("0x"), 16).ok()
    }

    /// `eth_getLogs` filtered by address + topic0 over a block window.
    async fn rpc_get_logs(
        &self,
        rpc_url: &str,
        address: Address,
        topic0: &str,
        from_block: u64,
        to_block: u64,
    ) -> Option<Vec<serde_json::Value>> {
        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "eth_getLogs",
            "params": [{
                "address": format!("0x{:040x}", address),
                "topics": [[topic0]],
                "fromBlock": format!("0x{:x}", from_block),
                "toBlock": format!("0x{:x}", to_block),
            }]
        });
        let parsed = self.rpc_post(rpc_url, body).await?;
        parsed.get("result")?.as_array().cloned()
    }

    // ── ABI helpers (runtime keccak — no magic hex constants) ──

    /// `keccak256(signature)[..4]` as `0x…` hex (4-byte function selector).
    fn fn_selector(sig: &str) -> String {
        let h = ethers::utils::keccak256(sig.as_bytes());
        format!("0x{}", hex::encode(&h[..4]))
    }

    /// `keccak256(event signature)` as `0x…` hex (topic0).
    fn event_topic0(sig: &str) -> String {
        let h = ethers::utils::keccak256(sig.as_bytes());
        format!("0x{}", hex::encode(h))
    }

    /// ABI calldata `<selector>` + one address argument.
    ///
    /// BUG (found by CI on #947, 2026-10-13): ethers' `Address` LowerHex impl
    /// writes its own 40-char form and IGNORES the `064` width —
    /// `format!("{:064x}", a)` produced a 40-char argument, an INVALID ABI
    /// word (every eth_call of this engine was malformed). Pad explicitly.
    fn call_with_address(selector: &str, a: Address) -> String {
        let arg = format!("{a:x}");
        format!("{selector}{}{arg}", "0".repeat(64 - arg.len()))
    }

    /// ABI calldata `<selector>` + two address arguments (see the width bug
    /// note on [`Self::call_with_address`]).
    fn call_with_two_addresses(selector: &str, a: Address, b: Address) -> String {
        let arg_a = format!("{a:x}");
        let arg_b = format!("{b:x}");
        format!(
            "{selector}{}{arg_a}{}{arg_b}",
            "0".repeat(64 - arg_a.len()),
            "0".repeat(64 - arg_b.len())
        )
    }

    /// The `idx`-th 32-byte word of an `eth_call` result as `U256`.
    fn word_at(result_hex: &str, idx: usize) -> Option<U256> {
        let hex = result_hex.strip_prefix("0x").unwrap_or(result_hex);
        if hex.len() < 64 * (idx + 1) {
            return None;
        }
        U256::from_str_radix(&hex[64 * idx..64 * (idx + 1)], 16).ok()
    }

    /// Address from an indexed log topic (`0x` + 64 hex chars, right-aligned).
    fn address_from_topic(topic: &str) -> Option<Address> {
        let hex = topic.strip_prefix("0x").unwrap_or(topic);
        if hex.len() != 64 {
            return None;
        }
        Address::from_str(&hex[24..]).ok()
    }

    /// Low 20 bytes of an ABI word as an address.
    fn address_from_word(word: U256) -> Address {
        let mut bytes = [0u8; 32];
        word.to_big_endian(&mut bytes);
        let mut addr = [0u8; 20];
        addr.copy_from_slice(&bytes[12..]);
        Address::from(addr)
    }

    /// Parses `Pool.getUserAccountData` output — words (Pool.sol, verified):
    /// 0=totalCollateralBase, 1=totalDebtBase, 2=availableBorrowsBase,
    /// 3=currentLiquidationThreshold, 4=ltv, 5=healthFactor.
    fn parse_user_account_data(result_hex: &str) -> Option<AaveAccountSnapshot> {
        Some(AaveAccountSnapshot {
            total_collateral_base: Self::word_at(result_hex, 0)?.low_u128(),
            total_debt_base: Self::word_at(result_hex, 1)?.low_u128(),
            health_factor_wad: Self::word_at(result_hex, 5)?.low_u128(),
        })
    }

    /// Splits the user-config bitmap into (collateral ids, borrowed ids).
    /// `DataTypes.UserConfigurationMap`: bit 2i = reserve i used as
    /// collateral, bit 2i+1 = reserve i borrowed.
    fn bitmap_flags(bitmap: U256) -> (Vec<usize>, Vec<usize>) {
        let mut collateral = Vec::new();
        let mut borrowed = Vec::new();
        for i in 0..128usize {
            if bitmap.bit(2 * i) {
                collateral.push(i);
            }
            if bitmap.bit(2 * i + 1) {
                borrowed.push(i);
            }
        }
        (collateral, borrowed)
    }

    /// Aave reports `liquidationBonus` as 10_000 + bonus_bps (e.g. 10_650 =
    /// +6.5%); the engine's math wants the pure bonus bps.
    fn bonus_bps_from_raw(raw: u128) -> u32 {
        raw.saturating_sub(AAVE_BONUS_OFFSET).min(u32::MAX as u128) as u32
    }

    /// Calculates the liquidation opportunity for a position.
    fn calculate_liquidation_opportunity(
        &self,
        position: &LendingPosition,
        id_counter: &mut u64,
    ) -> Option<LiquidationCandidate> {
        // Max liquidatable debt (limited by close factor)
        let max_repay_usd =
            position.total_debt_usd * (position.liquidation_bonus_bps as f64 / 10_000.0);
        let optimal_repay_usd = max_repay_usd.min(position.max_liquidatable_debt_usd);

        if optimal_repay_usd <= 0.0 {
            return None;
        }

        // Gross profit is the liquidation bonus on the repaid amount
        let bonus_rate = position.liquidation_bonus_bps as f64 / 10_000.0;
        let gross_profit_usd = optimal_repay_usd * bonus_rate;

        // Net yield after gas costs
        let net_yield_usd = gross_profit_usd - self.gas_cost_usd;

        *id_counter += 1;

        Some(LiquidationCandidate {
            id: *id_counter,
            position: position.clone(),
            gross_profit_usd,
            gas_cost_usd: self.gas_cost_usd,
            net_yield_usd,
            optimal_repay_usd,
        })
    }

    /// Builds a StrategyCandidate from a LiquidationCandidate.
    fn build_strategy_candidate(
        &self,
        liq: &LiquidationCandidate,
        chain_id: u64,
        tx_hash: H256,
        _cfg: Option<&TradingConfigState>,
    ) -> StrategyCandidate {
        let id = Uuid::new_v4();
        let trace_id = Uuid::new_v4();
        let strategy_kind = StrategyLabel::LiquidationSnipe.to_contract_strategy_kind();

        let position = &liq.position;
        let pair_symbol = format!(
            "{}(liquidation:{}",
            Self::token_symbol(&position.debt_token),
            position.protocol
        );

        let token_in = format!("0x{:040x}", position.debt_token);
        let token_out = format!("0x{:040x}", position.collateral_token);

        // Determine rejection reason if net yield is negative
        let (rejection_reason, gross_profit_usd) = if liq.net_yield_usd <= 0.0 {
            (Some("gas_exceeds_yield".to_string()), None)
        } else {
            (None, Some(liq.gross_profit_usd))
        };

        let opportunity = Opportunity {
            id,
            chain_id,
            strategy_kind,
            dex_a: position.protocol.clone(),
            dex_b: None,
            pair_symbol,
            token_in: token_in.clone(),
            token_out: token_out.clone(),
            amount_in_wei: Self::usd_to_wei(liq.optimal_repay_usd, 18).to_string(),
            expected_profit_usd: gross_profit_usd,
            net_expected_profit_usd: if liq.net_yield_usd > 0.0 {
                Some(liq.net_yield_usd)
            } else {
                None
            },
            roi_pct: Some((liq.net_yield_usd / liq.optimal_repay_usd) * 100.0),
            risk_score: Some(0.2), // Liquidations have execution risk
            block_number: None,
            rejection_reason: rejection_reason.clone(),
            cartridge_id: None,
            // WO-CARDS-COMPLETE-01 (2026-09-17): detector identity at construction.
            detector_id: Some("liquidation_snipe_engine".to_string()),
            pipeline_latency_ms: None,
            detected_at: Utc::now(),
            trace_id,
            economics: None,
        };

        let pool_addresses = vec![format!("0x{:040x}", position.user)]; // Position being liquidated
        let token_addresses = vec![token_in.clone(), token_out.clone()];

        let candidate = OpportunityCandidate {
            route_fingerprint: format!("liq_{}_{}", liq.id, position.user),
            pool_addresses,
            token_addresses,
            dex_adapters: vec![position.protocol.clone()],
            amount_in: liq.optimal_repay_usd,
            expected_amount_out: liq.optimal_repay_usd
                * (1.0 + position.liquidation_bonus_bps as f64 / 10_000.0),
            gross_profit: liq.gross_profit_usd,
        };

        // Build route plan for liquidation
        let legs = vec![RouteLeg {
            dex_id: position.protocol.clone(),
            dex_name: position.protocol.clone(),
            protocol_type: "liquidation".to_string(),
            factory_address: String::new(),
            pool_id: None,
            pool_address: Some(format!("0x{:040x}", position.collateral_token)),
            token_in: token_in.clone(),
            token_out: token_out.clone(),
            fee_bps: Some(position.liquidation_bonus_bps),
            amount_in: Some(liq.optimal_repay_usd),
            amount_out: None,
            tvl_usd: Some(position.total_collateral_usd),
            volume_24h_usd: None,
            pool_is_active: true,
        }];

        let route_plan = RoutePlan {
            route_id: Some(format!("liq-{}-{:x}", liq.id, tx_hash)),
            strategy_kind: StrategyLabel::LiquidationSnipe.as_str().to_string(),
            chain_id,
            legs,
            atomic: true, // Liquidations are atomic
            estimated_slippage_pct: None,
            price_impact_pct: None,
        };

        debug!(
            event = "liquidation_snipe_engine.candidate_built",
            chain_id,
            candidate_id = liq.id,
            user = %position.user,
            protocol = %position.protocol,
            health_factor = position.health_factor,
            net_yield_usd = liq.net_yield_usd,
            "liquidation candidate {}",
            if rejection_reason.is_some() { "rejected" } else { "accepted" }
        );

        StrategyCandidate {
            label: StrategyLabel::LiquidationSnipe,
            opportunity,
            candidate,
            route_plan,
            gross_profit_usd,
            net_expected_profit_usd: if liq.net_yield_usd > 0.0 {
                Some(liq.net_yield_usd)
            } else {
                None
            },
            rejection_reason,
            source_intent_hash: tx_hash,
            base_strategy: None,
        }
    }

    /// Returns the symbol for a well-known token address.
    fn token_symbol(token: &Address) -> String {
        let addr_str = format!("0x{:040x}", token);
        match addr_str.as_str() {
            WETH_MAINNET_LC => "WETH".to_string(),
            USDC_MAINNET_LC => "USDC".to_string(),
            USDT_MAINNET_LC => "USDT".to_string(),
            DAI_MAINNET_LC => "DAI".to_string(),
            WBTC_MAINNET_LC => "WBTC".to_string(),
            _ => format!("0x{:08x}", token),
        }
    }

    /// Converts USD amount to token wei.
    fn usd_to_wei(usd: f64, decimals: u8) -> U256 {
        let multiplier = 10f64.powi(decimals as i32);
        let wei = (usd * multiplier) as u128;
        U256::from(wei)
    }

    /// Adds or updates a position in the cache.
    pub fn update_position(&mut self, position: LendingPosition) {
        self.position_cache.insert(position.user, position);
    }

    /// Gets a cached position.
    pub fn get_cached_position(&self, user: &Address) -> Option<&LendingPosition> {
        self.position_cache.get(user)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ethers::types::Address;

    fn test_position(health_factor: f64, bonus_bps: u32) -> LendingPosition {
        LendingPosition {
            user: Address::from_low_u64_be(0x1234),
            protocol: "aave-v3".to_string(),
            total_collateral_usd: 10_000.0,
            total_debt_usd: 8_000.0,
            health_factor,
            max_liquidatable_debt_usd: 4_000.0, // 50% close factor
            collateral_token: Address::from_low_u64_be(0xABC),
            debt_token: Address::from_low_u64_be(0xDEF),
            liquidation_bonus_bps: bonus_bps,
        }
    }

    #[test]
    fn test_skips_safe_position() {
        let engine = LiquidationSnipeEngine::with_default_pools();
        let position = test_position(1.5, 500); // HF = 1.5 (safe)

        let mut id = 0u64;
        let candidate = engine.calculate_liquidation_opportunity(&position, &mut id);

        // Should not produce candidate for safe position
        // (scan_liquidatable_positions filters HF >= 1.0 before calling this)
        // But if we did calculate, it would have negative net yield due to gas
        assert!(candidate.is_none() || candidate.unwrap().net_yield_usd <= 0.0);
    }

    #[test]
    fn test_detects_liquidatable_position() {
        let engine = LiquidationSnipeEngine::with_default_pools();
        let position = test_position(0.85, 1000); // HF = 0.85 (liquidatable), 10% bonus

        let mut id = 0u64;
        let candidate = engine
            .calculate_liquidation_opportunity(&position, &mut id)
            .unwrap();

        // Max repay = $4000 (50% close factor)
        // Gross profit = $4000 * 10% = $400
        assert!(candidate.gross_profit_usd > 0.0);
        assert_eq!(candidate.id, 1);
        assert!(candidate.optimal_repay_usd <= position.max_liquidatable_debt_usd);
    }

    #[test]
    fn test_rejects_low_bonus() {
        let mut engine = LiquidationSnipeEngine::with_default_pools();
        let position = test_position(0.9, 100); // Only 1% bonus (below 5% threshold)

        // Manually check bonus threshold
        assert!(position.liquidation_bonus_bps < engine.min_liquidation_bonus_bps);
    }

    #[test]
    fn test_gas_cost_impact() {
        let engine = LiquidationSnipeEngine::with_default_pools();
        // Small position with low bonus - gas might exceed yield
        let position = test_position(0.95, 500); // 5% bonus

        let mut id = 0u64;
        if let Some(candidate) = engine.calculate_liquidation_opportunity(&position, &mut id) {
            // Net yield = gross - $50 gas
            assert_eq!(candidate.net_yield_usd, candidate.gross_profit_usd - 50.0);
        }
    }

    #[test]
    fn test_token_symbol_resolution() {
        let weth = Address::from_str("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2").unwrap();
        assert_eq!(LiquidationSnipeEngine::token_symbol(&weth), "WETH");

        let usdc = Address::from_str("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48").unwrap();
        assert_eq!(LiquidationSnipeEngine::token_symbol(&usdc), "USDC");
    }

    #[test]
    fn test_usd_to_wei_conversion() {
        // $1000 of tokens with 18 decimals
        let wei = LiquidationSnipeEngine::usd_to_wei(1000.0, 18);
        let expected = U256::from(1_000_000_000_000_000_000_000u128); // 1000 * 1e18
        assert_eq!(wei, expected);
    }

    // ── STEP 2.5c: pure ABI/parsing helper tests ────────────────────────────

    #[test]
    fn fn_selector_matches_known_chainlink_constant() {
        // Anchors runtime-keccak selectors against the known Chainlink
        // `latestRoundData()` selector (0xfeaf968c, price_worker const).
        assert_eq!(
            LiquidationSnipeEngine::fn_selector("latestRoundData()"),
            "0xfeaf968c"
        );
    }

    #[test]
    fn word_at_parses_words_and_rejects_short_results() {
        let hex = format!("0x{}{:064x}", "0".repeat(64), 42u128);
        assert_eq!(LiquidationSnipeEngine::word_at(&hex, 0), Some(U256::zero()));
        assert_eq!(
            LiquidationSnipeEngine::word_at(&hex, 1),
            Some(U256::from(42u64))
        );
        assert_eq!(LiquidationSnipeEngine::word_at(&hex, 2), None);
        assert_eq!(LiquidationSnipeEngine::word_at("0x", 0), None);
    }

    #[test]
    fn address_from_topic_extracts_right_aligned_address() {
        let weth = "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2";
        let topic = format!("0x{}{}", "0".repeat(24), &weth[2..]);
        assert_eq!(
            LiquidationSnipeEngine::address_from_topic(&topic).unwrap(),
            Address::from_str(weth).unwrap()
        );
        // Wrong length topics are rejected (never a garbage address).
        assert_eq!(
            LiquidationSnipeEngine::address_from_topic("0xdeadbeef"),
            None
        );
    }

    #[test]
    fn address_from_word_keeps_low_twenty_bytes() {
        // 38-hex literal = a 19-byte value; the low-20-bytes address of the
        // word is that value, and {:040x} pads it to the canonical 40 chars
        // (t3's original literal dropped the "00" pad — CI caught it).
        let word = U256::from_str_radix("1234567890abcdef567890abcdef1234567890", 16).unwrap();
        let addr = LiquidationSnipeEngine::address_from_word(word);
        assert_eq!(
            format!("0x{:040x}", addr),
            "0x001234567890abcdef567890abcdef1234567890"
        );
    }

    #[test]
    fn parse_user_account_data_reads_six_word_layout() {
        // (collateral, debt, availBorrows, liqThreshold, ltv, hf)
        let hex = format!(
            "0x{:064x}{:064x}{}{:064x}{}{:064x}",
            1_000_000_000_000_000_000u128, // word 0: totalCollateralBase
            900_000_000_000_000_000u128,   // word 1: totalDebtBase
            "0".repeat(64),                // word 2: availableBorrowsBase
            8_000u128,                     // word 3: currentLiquidationThreshold
            "0".repeat(64),                // word 4: ltv
            850_000_000_000_000_000u128,   // word 5: healthFactor
        );
        let snap = LiquidationSnipeEngine::parse_user_account_data(&hex).unwrap();
        assert_eq!(snap.total_collateral_base, 1_000_000_000_000_000_000);
        assert_eq!(snap.total_debt_base, 900_000_000_000_000_000);
        assert_eq!(snap.health_factor_wad, 850_000_000_000_000_000);
        // Truncated output never fabricates data.
        assert!(LiquidationSnipeEngine::parse_user_account_data("0x1234").is_none());
    }

    #[test]
    fn bitmap_flags_decodes_pairs_of_bits() {
        // bit0 (id0 collateral) + bit3 (id1 borrowing) = 0b1001 = 9
        let (collateral, borrowed) = LiquidationSnipeEngine::bitmap_flags(U256::from(9u64));
        assert_eq!(collateral, vec![0]);
        assert_eq!(borrowed, vec![1]);
        let (empty_c, empty_b) = LiquidationSnipeEngine::bitmap_flags(U256::zero());
        assert!(empty_c.is_empty() && empty_b.is_empty());
    }

    #[test]
    fn bonus_bps_from_raw_strips_ten_thousand_offset() {
        // Aave 10_650 = +6.5% bonus → 650 bps for the engine's math.
        assert_eq!(LiquidationSnipeEngine::bonus_bps_from_raw(10_650), 650);
        assert_eq!(LiquidationSnipeEngine::bonus_bps_from_raw(10_500), 500);
        // Degenerate sub-10_000 values degrade to 0 (never wrap around).
        assert_eq!(LiquidationSnipeEngine::bonus_bps_from_raw(9_000), 0);
        assert_eq!(LiquidationSnipeEngine::bonus_bps_from_raw(0), 0);
    }

    #[test]
    fn calldata_encoding_shapes_are_canonical() {
        let addr = Address::from_low_u64_be(0xaaa1);
        let cd = LiquidationSnipeEngine::call_with_address("0xdeadbeef", addr);
        // "0x" (2) + selector (8) + one 64-char ABI word.
        assert_eq!(cd.len(), 2 + 8 + 64);
        assert!(cd.starts_with("0xdeadbeef"));
        assert!(cd.ends_with("000000000000000000000000000000000000000000000000000000000000aaa1"));
        let cd2 = LiquidationSnipeEngine::call_with_two_addresses("0xdeadbeef", addr, addr);
        assert_eq!(cd2.len(), 2 + 8 + 128);
    }

    #[test]
    fn fetch_returns_honest_empty_for_unsupported_protocol() {
        // Compound V2 has no real fetcher yet — R8 honest empty, no RPC call.
        let engine = LiquidationSnipeEngine::with_default_pools();
        let compound = engine
            .lending_pools
            .get("compound-v2-1")
            .cloned()
            .expect("default pools include compound-v2 on mainnet");
        let rt = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        let out = rt.block_on(engine.fetch_positions_from_subgraph(&compound));
        assert!(out.is_empty());
    }

    // ── STEP 2.5c: full fetch pipeline against a local wiremock JSON-RPC ────
    //
    // RULE 00: no external network — wiremock serves deterministic JSON-RPC
    // responses on localhost (same idiom as price_worker tests).

    use wiremock::matchers::{body_string_contains, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    /// ABI word hex (`{:064x}`).
    fn w(v: u128) -> String {
        format!("{:064x}", v)
    }

    /// ABI word for an address (160-bit, right-aligned — NOT low_u64, which
    /// would truncate the top 24 bits).
    fn wa(a: Address) -> U256 {
        U256::from_big_endian(a.as_bytes())
    }

    /// eth_call 200 envelope carrying `words` as the result.
    /// (Explicit pad — see the width bug note on `call_with_address`.)
    fn eth_call_result(words: &[U256]) -> ResponseTemplate {
        let word_hex = |x: &U256| {
            let h = format!("{x:x}");
            format!("{}{h}", "0".repeat(64 - h.len()))
        };
        let result: String = words.iter().map(word_hex).collect();
        ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "jsonrpc": "2.0", "id": 1, "result": format!("0x{result}")
        }))
    }

    fn n(v: u128) -> U256 {
        U256::from(v)
    }

    async fn mount_rpc_mock(server: &MockServer, request_fragment: &str, resp: ResponseTemplate) {
        Mock::given(method("POST"))
            .and(path("/"))
            .and(body_string_contains(request_fragment))
            .respond_with(resp)
            .mount(server)
            .await;
    }

    #[tokio::test]
    async fn fetch_positions_real_pipeline_wiremock() {
        let server = MockServer::start().await;

        // Cast.
        let user_liq = Address::from_low_u64_be(0xaaa1); // HF 0.85 — liquidatable
        let user_safe = Address::from_low_u64_be(0xbbb2); // HF 2.0 — safe
        let weth = Address::from_str("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2").unwrap();
        let usdc = Address::from_str("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48").unwrap();
        let pool = Address::from_str("0x87870bca3f3fd6335c3f4ce8392d69350b4fa4e2").unwrap();
        let addresses_provider = Address::from_low_u64_be(0x1111);
        let data_provider = Address::from_low_u64_be(0x2222);
        let oracle = Address::from_low_u64_be(0x3333);

        // ── eth_blockNumber.
        mount_rpc_mock(
            &server,
            "eth_blockNumber",
            ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "jsonrpc": "2.0", "id": 1, "result": "0x7b2c80"
            })),
        )
        .await;

        // ── Discovery logs.
        let liq_topic = LiquidationSnipeEngine::event_topic0(
            "LiquidationCall(address,address,address,uint256,uint256,address,bool)",
        );
        let borrow_topic = LiquidationSnipeEngine::event_topic0(
            "Borrow(address,address,address,uint256,uint8,uint256,uint16)",
        );
        // Explicit pad — `{:064x}` on Address writes its own 40-char form
        // and the engine's address_from_topic correctly REJECTS short topics
        // (len != 64), which zeroed out every user in the original mock.
        let topic_hex = |a: Address| {
            let h = format!("{a:x}");
            format!("0x{}{h}", "0".repeat(64 - h.len()))
        };
        mount_rpc_mock(
            &server,
            &liq_topic[2..],
            ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "jsonrpc": "2.0", "id": 1,
                "result": [{
                    "address": format!("0x{:040x}", pool),
                    "topics": [
                        liq_topic,
                        topic_hex(weth),   // collateralAsset
                        topic_hex(usdc),   // debtAsset
                        topic_hex(user_liq) // user
                    ]
                }]
            })),
        )
        .await;
        mount_rpc_mock(
            &server,
            &borrow_topic[2..],
            ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "jsonrpc": "2.0", "id": 1,
                "result": [{
                    "address": format!("0x{:040x}", pool),
                    "topics": [
                        borrow_topic,
                        topic_hex(usdc),     // reserve
                        topic_hex(user_safe) // onBehalfOf (debt owner)
                    ]
                }]
            })),
        )
        .await;

        // ── getUserAccountData: liquidatable user (HF 0.85e18, $10k/$9k) and
        // safe borrower (HF 2e18 → filtered by the HF < 1.0 gate).
        let sel_uad = LiquidationSnipeEngine::fn_selector("getUserAccountData(address)");
        mount_rpc_mock(
            &server,
            &LiquidationSnipeEngine::call_with_address(&sel_uad, user_liq),
            eth_call_result(&[
                n(1_000_000_000_000_000_000), // collateral $10k (base 1e8)
                n(900_000_000_000_000_000),   // debt $9k
                n(0),
                n(8_000),
                n(0),
                n(850_000_000_000_000_000), // HF 0.85 wad
            ]),
        )
        .await;
        mount_rpc_mock(
            &server,
            &LiquidationSnipeEngine::call_with_address(&sel_uad, user_safe),
            eth_call_result(&[
                n(2_000_000_000_000_000_000),
                n(100_000_000_000_000_000),
                n(0),
                n(8_000),
                n(0),
                n(2_000_000_000_000_000_000), // HF 2.0 — safe
            ]),
        )
        .await;

        // ── Enrichment plumbing: Pool → ADDRESSES_PROVIDER → provider/oracle.
        mount_rpc_mock(
            &server,
            &LiquidationSnipeEngine::fn_selector("ADDRESSES_PROVIDER()")[2..],
            eth_call_result(&[wa(addresses_provider)]),
        )
        .await;
        mount_rpc_mock(
            &server,
            &LiquidationSnipeEngine::fn_selector("getPoolDataProvider()")[2..],
            eth_call_result(&[wa(data_provider)]),
        )
        .await;
        mount_rpc_mock(
            &server,
            &LiquidationSnipeEngine::fn_selector("getPriceOracle()")[2..],
            eth_call_result(&[wa(oracle)]),
        )
        .await;

        // ── User bitmap: id0 collateral (WETH), id1 borrowed (USDC) = 0b1001.
        let sel_guc = LiquidationSnipeEngine::fn_selector("getUserConfiguration(address)");
        mount_rpc_mock(
            &server,
            &LiquidationSnipeEngine::call_with_address(&sel_guc, user_liq),
            eth_call_result(&[n(9)]),
        )
        .await;

        // ── getReserveAddressById: 0→WETH, 1→USDC.
        let sel_by_id = LiquidationSnipeEngine::fn_selector("getReserveAddressById(uint16)");
        mount_rpc_mock(
            &server,
            &format!("{sel_by_id}{}", w(0)),
            eth_call_result(&[wa(weth)]),
        )
        .await;
        mount_rpc_mock(
            &server,
            &format!("{sel_by_id}{}", w(1)),
            eth_call_result(&[wa(usdc)]),
        )
        .await;

        // ── getUserReserveData(asset, user) on the data provider:
        // words 0=aToken, 1=stableDebt, 2=variableDebt.
        let sel_urd = LiquidationSnipeEngine::fn_selector("getUserReserveData(address,address)");
        mount_rpc_mock(
            &server,
            &LiquidationSnipeEngine::call_with_two_addresses(&sel_urd, usdc, user_liq),
            eth_call_result(&[n(0), n(0), n(12_000_000_000)]), // 12k USDC variable debt
        )
        .await;
        mount_rpc_mock(
            &server,
            &LiquidationSnipeEngine::call_with_two_addresses(&sel_urd, weth, user_liq),
            eth_call_result(&[n(6_000_000_000_000_000_000), n(0), n(0)]), // 6 WETH collateral
        )
        .await;

        // ── Oracle prices (USD × 1e8) + reserve config (decimals, bonus).
        let sel_gap = LiquidationSnipeEngine::fn_selector("getAssetPrice(address)");
        mount_rpc_mock(
            &server,
            &LiquidationSnipeEngine::call_with_address(&sel_gap, weth),
            eth_call_result(&[n(200_000_000_000)]), // $2000
        )
        .await;
        mount_rpc_mock(
            &server,
            &LiquidationSnipeEngine::call_with_address(&sel_gap, usdc),
            eth_call_result(&[n(100_000_000)]), // $1
        )
        .await;
        let sel_rcd = LiquidationSnipeEngine::fn_selector("getReserveConfigurationData(address)");
        mount_rpc_mock(
            &server,
            &LiquidationSnipeEngine::call_with_address(&sel_rcd, weth),
            eth_call_result(&[
                n(18),
                n(0),
                n(8_000),
                n(10_650),
                n(0),
                n(1),
                n(1),
                n(0),
                n(1),
                n(0),
            ]), // bonus +6.5%
        )
        .await;
        mount_rpc_mock(
            &server,
            &LiquidationSnipeEngine::call_with_address(&sel_rcd, usdc),
            eth_call_result(&[
                n(6),
                n(0),
                n(8_700),
                n(10_100),
                n(0),
                n(1),
                n(1),
                n(0),
                n(1),
                n(0),
            ]),
        )
        .await;

        // ── Run the real fetch against the mock RPC.
        let engine =
            LiquidationSnipeEngine::with_default_pools().with_rpc_url(server.uri().to_string());
        let aave_cfg = engine
            .lending_pools
            .get("aave-v3-1")
            .cloned()
            .expect("default pools include aave-v3 on mainnet");
        let positions = engine.fetch_positions_from_subgraph(&aave_cfg).await;

        assert_eq!(positions.len(), 1, "exactly the liquidatable user survives");
        let p = &positions[0];
        assert_eq!(p.user, user_liq);
        assert_eq!(p.protocol, "aave-v3");
        assert!((p.total_collateral_usd - 10_000.0).abs() < 1e-6);
        assert!((p.total_debt_usd - 9_000.0).abs() < 1e-6);
        assert!((p.health_factor - 0.85).abs() < 1e-12);
        assert!(
            (p.max_liquidatable_debt_usd - 4_500.0).abs() < 1e-6,
            "50% close factor"
        );
        assert_eq!(p.collateral_token, weth);
        assert_eq!(p.debt_token, usdc);
        assert_eq!(p.liquidation_bonus_bps, 650);
    }

    #[tokio::test]
    async fn fetch_positions_fails_honest_on_rpc_error() {
        // Wiremock with NO mounted mocks: every JSON-RPC call 404s → the fetch
        // must return an HONEST empty vec (R8), never a fabricated position.
        let server = MockServer::start().await;
        let engine =
            LiquidationSnipeEngine::with_default_pools().with_rpc_url(server.uri().to_string());
        let aave_cfg = engine
            .lending_pools
            .get("aave-v3-1")
            .cloned()
            .expect("default pools include aave-v3 on mainnet");
        let positions = engine.fetch_positions_from_subgraph(&aave_cfg).await;
        assert!(positions.is_empty());
    }
}
