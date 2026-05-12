// M11 allow: test modules use .unwrap()/.expect() for readability; the
// production code is clippy-clean.
//! ImpactIndex — converts a `RouteIntent` into the minimal set of
//! (pools, cycles, lending positions) that the strategy engines must evaluate.
//!
//! ## Design (Phase 5 — spec §3.4)
//!
//! Instead of "poll everything every 12 seconds", the orchestrator asks the
//! `ImpactIndex`: *"a swap just changed liquidity on the token pair (X, Y) —
//! which triangular cycles, DEX pools, and lending positions are affected?"*
//!
//! The index is built at boot from:
//!   1. `pool_to_cycles`: derived from `MVP_CYCLES` in `triangular_worker` (scaffold)
//!      and optionally extended from a `pool_cycles` PG table (future sprint).
//!   2. `token_pair_to_pools`: derived from the `pools` PG table at boot; refreshed
//!      when `pool_sync_worker` calls `add_pool`.
//!   3. `token_to_lending_positions`: empty until `LendingPositionIndexer` ships
//!      (Phase 11 of the spec). R8 fail-honest — nothing is fabricated.
//!   4. `router_to_protocol`: static config from `configs/router_kinds.json` plus
//!      runtime detection (future).
//!   5. `selector_to_decoder`: compile-time constant table.
//!
//! ## R8 invariants
//!
//! - `resolve()` on empty legs → `ImpactSet::default()` (empty), never an error.
//! - Pair not in registry → that leg contributes nothing; no fabrication.
//! - `token_to_lending_positions` being empty produces no positions in the output.
//! - Cycles and pools are deduped; the same cycle touched by two legs appears once.

use crate::route_intent::{ProtocolType, RouteIntent, RouteIntentLeg, RouterKind};
use anyhow::Context;
use ethers::types::Address;
use redis::aio::ConnectionManager;
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::collections::{HashMap, HashSet};
use std::str::FromStr;
use tracing::{info, warn};

// ---------------------------------------------------------------------------
// Supporting types
// ---------------------------------------------------------------------------

/// Stable identifier for a triangular cycle.
///
/// For the MVP `MVP_CYCLES` constant, the ID is the 0-based index into that
/// slice. When a future `pool_cycles` PG table is introduced, the ID will be
/// the PG row's stable integer primary key.
pub type CycleId = u64;

/// Normalised sorted pair key: `token_lo < token_hi` by address bytes.
///
/// Both `(A, B)` and `(B, A)` produce the same key so lookups are
/// orientation-independent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TokenPairKey {
    pub token_lo: Address,
    pub token_hi: Address,
}

impl TokenPairKey {
    /// Constructs a canonical (sorted) key from two token addresses.
    ///
    /// The address with lower byte-value becomes `token_lo`. Callers need not
    /// pre-sort — both orderings of `(a, b)` produce the same key.
    pub fn canonical(a: Address, b: Address) -> Self {
        if a <= b {
            Self {
                token_lo: a,
                token_hi: b,
            }
        } else {
            Self {
                token_lo: b,
                token_hi: a,
            }
        }
    }
}

/// Lightweight descriptor of a pool stored in the index.
///
/// Mirrors the columns the index needs from the `pools` PG table plus the
/// derived fields (dex_name, protocol_type) from `factories` / `dexes`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolRef {
    pub chain_id: u64,
    pub address: Address,
    pub dex_name: String,
    pub protocol_type: ProtocolType,
    pub token0: Address,
    pub token1: Address,
    pub fee_bps: Option<u32>,
}

/// Reference to a lending position known to the indexer.
///
/// Populated only when `LendingPositionIndexer` has been seeded (Phase 11).
/// Until then, `token_to_lending_positions` is empty and `impacted_lending_positions`
/// will always be empty (R8 fail-honest).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserPositionRef {
    pub protocol: LendingProtocol,
    pub user: Address,
    pub last_checked_block: u64,
}

/// Supported lending protocol families.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LendingProtocol {
    AaveV3,
    CompoundV2,
}

/// Fast-dispatch hint for the calldata decoder.
///
/// Stored under 4-byte function selectors so the orchestrator can determine
/// which decoder to invoke without re-scanning the selector table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecoderKind {
    UniswapV2,
    UniswapV3,
    Multicall,
    Unknown,
}

/// Input seed for building the initial `pool_to_cycles` map from `MVP_CYCLES`.
///
/// Each entry represents one pool address that participates in a given cycle.
/// Multiple seeds with the same `pool_address` but different `cycle_id`s are
/// valid — a pool can participate in more than one cycle.
#[derive(Debug, Clone)]
pub struct MvpCycleSeed {
    /// Zero-based index into `MVP_CYCLES` (used as the stable `CycleId`).
    pub cycle_id: CycleId,
    /// Lowercase 0x-prefixed pool address.
    pub pool_address: Address,
}

/// The resolved set of impacted entities for a given `RouteIntent`.
///
/// This is what the orchestrator fans out to individual strategy engines.
/// All inner collections are deduplicated — a cycle or pool touched by
/// multiple legs of the same intent appears exactly once.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ImpactSet {
    /// All token pairs whose pools were impacted by at least one leg.
    pub impacted_pairs: Vec<TokenPairKey>,
    /// All pools matching the impacted pairs.
    pub impacted_pools: Vec<PoolRef>,
    /// Triangular cycles that include at least one impacted pool.
    pub impacted_cycles: Vec<CycleId>,
    /// Lending positions where at least one leg token is collateral or debt.
    /// Empty until `LendingPositionIndexer` ships (Phase 11 / R8 honest).
    pub impacted_lending_positions: Vec<UserPositionRef>,
    /// Union of protocol types across all impacted pools.
    pub impacted_protocols: HashSet<ProtocolType>,
}

// ---------------------------------------------------------------------------
// ImpactIndex
// ---------------------------------------------------------------------------

/// Central in-memory registry used by the orchestrator to convert a
/// `RouteIntent` into its `ImpactSet` in O(L × P) time where L = number of
/// legs and P = average number of pools per token pair.
pub struct ImpactIndex {
    /// pool_address (lowercase) → cycle IDs that contain this pool.
    /// Populated from `MVP_CYCLES` at boot and optionally from `pool_cycles`
    /// PG table in future sprints.
    pool_to_cycles: HashMap<Address, Vec<CycleId>>,

    /// Sorted (token_lo, token_hi) → pools holding that pair.
    /// Loaded from the `pools` PG table at boot and updated incrementally by
    /// `add_pool` when `pool_sync_worker` discovers new pools.
    token_pair_to_pools: HashMap<TokenPairKey, Vec<PoolRef>>,

    /// token_address → lending positions where it is collateral or debt.
    /// Empty until `LendingPositionIndexer` ships (Phase 11). R8: never
    /// fabricated — an empty map produces no positions in `ImpactSet`.
    token_to_lending_positions: HashMap<Address, Vec<UserPositionRef>>,

    /// router_address → router family classification.
    /// Used by the decoder for fast dispatch.
    #[allow(dead_code)]
    router_to_protocol: HashMap<Address, RouterKind>,

    /// 4-byte function selector → decoder kind.
    /// Compile-time constant populated in `new_with_selectors`.
    #[allow(dead_code)]
    selector_to_decoder: HashMap<[u8; 4], DecoderKind>,
}

impl ImpactIndex {
    // -----------------------------------------------------------------------
    // Constructors
    // -----------------------------------------------------------------------

    /// Constructs an empty index.
    ///
    /// Useful for unit tests and as the base before loaders are applied.
    pub fn empty() -> Self {
        Self {
            pool_to_cycles: HashMap::new(),
            token_pair_to_pools: HashMap::new(),
            token_to_lending_positions: HashMap::new(),
            router_to_protocol: HashMap::new(),
            selector_to_decoder: Self::build_selector_table(),
        }
    }

    /// Loads from PG + Redis + an optional set of MVP cycle symbol triples.
    ///
    /// Returns `Err` only on catastrophic load failure (e.g., PG pool
    /// completely unavailable when `pool` is `Some`). Partial loads succeed:
    /// missing lending positions (indexer not yet shipped) keep the map empty
    /// (R8 fail-honest). Callers should always treat a successful return as
    /// "index is ready; some maps may be empty".
    ///
    /// ### Load sources
    /// 1. `mvp_cycles` (caller-supplied, typically `triangular_worker::MVP_CYCLES`)
    ///    → Redis pool index `arbx:pool_index:<chain>:<sym0>:<sym1>` → `pool_to_cycles`.
    ///    Decoupled from `workers` crate so this module compiles as a library target.
    /// 2. PG `pools` table → `token_pair_to_pools`.
    /// 3. `token_to_lending_positions` → empty (indexer not yet shipped).
    pub async fn from_registry(
        pool: Option<&PgPool>,
        redis: &mut ConnectionManager,
        chain_id: u64,
        mvp_cycles: &[(&str, &str, &str)],
    ) -> anyhow::Result<Self> {
        let mut idx = Self::empty();

        // ── Step 1: load pools from PG ────────────────────────────────────
        if let Some(pg) = pool {
            match load_pools_from_pg(pg, chain_id).await {
                Ok(pool_refs) => {
                    let count = pool_refs.len();
                    for pr in pool_refs {
                        idx.add_pool(pr);
                    }
                    info!(
                        event = "impact_index.pools_loaded",
                        chain_id, count, "loaded pool registry from PG"
                    );
                }
                Err(e) => {
                    // Non-fatal: pool index empty, resolver will yield nothing.
                    warn!(
                        event = "impact_index.pg_load_failed",
                        chain_id,
                        error = %e,
                        "failed to load pools from PG; token_pair_to_pools will be empty"
                    );
                }
            }
        }

        // ── Step 2: seed MVP_CYCLES from Redis pool index ─────────────────
        let seeds = build_mvp_seeds_from_redis(redis, chain_id, mvp_cycles).await;
        let seed_count = seeds.len();
        idx.seed_cycles_from_mvp(&seeds);
        info!(
            event = "impact_index.cycles_seeded",
            chain_id,
            cycle_count = seed_count,
            "seeded pool_to_cycles from MVP_CYCLES"
        );

        // ── Step 3: lending positions ─────────────────────────────────────
        // LendingPositionIndexer not yet shipped (Phase 11). Map stays empty.
        // R8 fail-honest: no fabrication.

        Ok(idx)
    }

    // -----------------------------------------------------------------------
    // Incremental update
    // -----------------------------------------------------------------------

    /// Adds a single pool to the index, updating both `token_pair_to_pools`
    /// and (when applicable) `pool_to_cycles`.
    ///
    /// Called by `pool_sync_worker` when it discovers a new pool on-chain.
    pub fn add_pool(&mut self, pool: PoolRef) {
        let key = TokenPairKey::canonical(pool.token0, pool.token1);
        self.token_pair_to_pools.entry(key).or_default().push(pool);
    }

    /// Checks if the index has any pools for the given pair key.
    pub fn has_pools_for_pair(&self, key: TokenPairKey) -> bool {
        self.token_pair_to_pools.contains_key(&key)
    }

    /// Seeds `pool_to_cycles` from the `MVP_CYCLES` constant.
    ///
    /// Each `MvpCycleSeed` maps one pool address to the cycle it belongs to.
    /// Multiple seeds for the same pool address (different cycle_id values)
    /// accumulate correctly.
    ///
    /// This is a TEMPORARY scaffold — replace with reading from a `pool_cycles`
    /// PG table when that is added in a future sprint.
    pub fn seed_cycles_from_mvp(&mut self, seeds: &[MvpCycleSeed]) {
        for seed in seeds {
            self.pool_to_cycles
                .entry(seed.pool_address)
                .or_default()
                .push(seed.cycle_id);
        }
    }

    // -----------------------------------------------------------------------
    // Resolution
    // -----------------------------------------------------------------------

    /// Converts a `RouteIntent` into its `ImpactSet`.
    ///
    /// ### R8 invariants
    /// - Empty legs → `ImpactSet::default()` (never an error).
    /// - Unknown pair → that leg contributes nothing (no fabrication).
    /// - Empty `token_to_lending_positions` → no lending positions in output.
    /// - Dedup: a cycle or pool that appears in multiple legs is included once.
    pub fn resolve(&self, intent: &RouteIntent) -> ImpactSet {
        if intent.legs.is_empty() {
            return ImpactSet::default();
        }

        let mut acc = ImpactSet::default();
        let mut seen_pools: HashSet<Address> = HashSet::new();
        let mut seen_cycles: HashSet<CycleId> = HashSet::new();
        let mut seen_pairs: HashSet<TokenPairKey> = HashSet::new();

        for leg in &intent.legs {
            self.resolve_leg(
                leg,
                &mut acc,
                &mut seen_pools,
                &mut seen_cycles,
                &mut seen_pairs,
            );
        }

        acc
    }

    /// Per-leg resolution helper.
    fn resolve_leg(
        &self,
        leg: &RouteIntentLeg,
        acc: &mut ImpactSet,
        seen_pools: &mut HashSet<Address>,
        seen_cycles: &mut HashSet<CycleId>,
        seen_pairs: &mut HashSet<TokenPairKey>,
    ) {
        let pair_key = TokenPairKey::canonical(leg.token_in, leg.token_out);

        // ── Token pair ────────────────────────────────────────────────────
        if seen_pairs.insert(pair_key) {
            acc.impacted_pairs.push(pair_key);
        }

        // ── Pools for this pair ───────────────────────────────────────────
        if let Some(pools) = self.token_pair_to_pools.get(&pair_key) {
            for pool_ref in pools {
                if seen_pools.insert(pool_ref.address) {
                    // Collect cycles containing this pool.
                    if let Some(cycle_ids) = self.pool_to_cycles.get(&pool_ref.address) {
                        for &cid in cycle_ids {
                            if seen_cycles.insert(cid) {
                                acc.impacted_cycles.push(cid);
                            }
                        }
                    }
                    acc.impacted_protocols.insert(pool_ref.protocol_type);
                    acc.impacted_pools.push(pool_ref.clone());
                }
            }
        }

        // ── Lending positions for either token ────────────────────────────
        // R8: `token_to_lending_positions` is empty until Phase 11.
        // This loop produces nothing — no fabrication.
        for token in [leg.token_in, leg.token_out] {
            if let Some(positions) = self.token_to_lending_positions.get(&token) {
                for pos in positions {
                    acc.impacted_lending_positions.push(pos.clone());
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    // Private helpers
    // -----------------------------------------------------------------------

    /// Compile-time constant selector → DecoderKind table.
    ///
    /// These are the 4-byte selectors of the Uniswap V2/V3 swap functions
    /// recognised by the existing `calldata` module.
    fn build_selector_table() -> HashMap<[u8; 4], DecoderKind> {
        // V2 selectors
        let v2_selectors: &[[u8; 4]] = &[
            [0x38, 0xed, 0x17, 0x39], // swapExactTokensForTokens
            [0x8a, 0x04, 0x16, 0x75], // swapTokensForExactTokens
            [0x7f, 0xf3, 0x64, 0xd8], // swapExactETHForTokens
            [0x4a, 0x25, 0xd9, 0x4a], // swapTokensForExactETH
            [0x18, 0xcb, 0xaf, 0xe5], // swapExactTokensForETH
            [0xfb, 0x3b, 0xdb, 0x41], // swapETHForExactTokens
            // Fee-on-transfer variants
            [0x5c, 0x11, 0xd7, 0x95], // swapExactTokensForTokensSupportingFeeOnTransferTokens
            [0xb6, 0xf9, 0xde, 0x95], // swapExactETHForTokensSupportingFeeOnTransferTokens
            [0x79, 0x1a, 0xc9, 0x47], // swapExactTokensForETHSupportingFeeOnTransferTokens
        ];
        // V3 selectors
        let v3_selectors: &[[u8; 4]] = &[
            [0x41, 0x4b, 0xf3, 0x89], // exactInputSingle
            [0xc0, 0x4b, 0x8d, 0x59], // exactInput
            [0xdb, 0x3e, 0x21, 0x98], // exactOutputSingle
            [0x09, 0xb8, 0x13, 0x46], // exactOutput
        ];
        // Multicall selectors (Uniswap V3 router multicall)
        let multicall_selectors: &[[u8; 4]] = &[
            [0xac, 0x96, 0x50, 0xd8], // multicall(bytes[])
            [0x5a, 0xe4, 0x01, 0xdc], // multicall(uint256,bytes[])
        ];

        let mut map = HashMap::new();
        for sel in v2_selectors {
            map.insert(*sel, DecoderKind::UniswapV2);
        }
        for sel in v3_selectors {
            map.insert(*sel, DecoderKind::UniswapV3);
        }
        for sel in multicall_selectors {
            map.insert(*sel, DecoderKind::Multicall);
        }
        map
    }
}

// ---------------------------------------------------------------------------
// PG loader
// ---------------------------------------------------------------------------

/// Loads pool descriptors from the PG `pools` table for a given chain.
///
/// The query joins `pools` → `factories` → `dexes` to recover `dex_name` and
/// `protocol_type`, and joins `tokens` twice to recover `token0`/`token1`
/// addresses. Returns an empty Vec (not Err) when the table is empty.
async fn load_pools_from_pg(pg: &PgPool, chain_id: u64) -> anyhow::Result<Vec<PoolRef>> {
    // Use sqlx::query (not query!) to avoid offline data file requirement.
    // The join structure: pools → factories → dexes for dex_name/protocol_type,
    // pools → tokens (x2) for token addresses.
    let rows = sqlx::query(
        r#"
        SELECT
            p.chain_id,
            p.address,
            d.name          AS dex_name,
            d.protocol_type AS protocol_type,
            t0.address      AS token0_addr,
            t1.address      AS token1_addr,
            p.fee_tier
        FROM pools p
        JOIN factories f  ON f.id = p.factory_id
        JOIN dexes    d  ON d.id = f.dex_id
        JOIN tokens   t0 ON t0.id = p.token0_id
        JOIN tokens   t1 ON t1.id = p.token1_id
        WHERE p.chain_id = $1
          AND p.is_active = TRUE
        "#,
    )
    .bind(chain_id as i64)
    .fetch_all(pg)
    .await
    .context("load pools from PG for ImpactIndex")?;

    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        use sqlx::Row;

        let chain_id_pg: i64 = row.try_get("chain_id").unwrap_or(chain_id as i64);
        let address_str: &str = match row.try_get("address") {
            Ok(s) => s,
            Err(e) => {
                warn!(event = "impact_index.pg_row_skip", error = %e, "skipping pool row: missing address");
                continue;
            }
        };
        let address = match Address::from_str(address_str) {
            Ok(a) => a,
            Err(e) => {
                warn!(event = "impact_index.pg_addr_parse", addr = address_str, error = %e, "skipping pool: bad address");
                continue;
            }
        };

        let token0_str: &str = match row.try_get("token0_addr") {
            Ok(s) => s,
            Err(e) => {
                warn!(event = "impact_index.pg_row_skip", addr = address_str, error = %e, "skipping pool: missing token0");
                continue;
            }
        };
        let token1_str: &str = match row.try_get("token1_addr") {
            Ok(s) => s,
            Err(e) => {
                warn!(event = "impact_index.pg_row_skip", addr = address_str, error = %e, "skipping pool: missing token1");
                continue;
            }
        };
        let token0 = match Address::from_str(token0_str) {
            Ok(a) => a,
            Err(e) => {
                warn!(event = "impact_index.pg_addr_parse", addr = token0_str, error = %e, "skipping pool: bad token0");
                continue;
            }
        };
        let token1 = match Address::from_str(token1_str) {
            Ok(a) => a,
            Err(e) => {
                warn!(event = "impact_index.pg_addr_parse", addr = token1_str, error = %e, "skipping pool: bad token1");
                continue;
            }
        };

        let dex_name: String = row.try_get("dex_name").unwrap_or_default();
        let protocol_type_str: String = row.try_get("protocol_type").unwrap_or_default();
        let protocol_type = parse_protocol_type(&protocol_type_str);
        let fee_tier: Option<i32> = row.try_get("fee_tier").ok();

        out.push(PoolRef {
            chain_id: chain_id_pg as u64,
            address,
            dex_name,
            protocol_type,
            token0,
            token1,
            fee_bps: fee_tier.map(|f| f as u32),
        });
    }

    Ok(out)
}

/// Maps the `dexes.protocol_type` string from PG to the `ProtocolType` enum.
fn parse_protocol_type(s: &str) -> ProtocolType {
    match s.to_ascii_uppercase().as_str() {
        "UNISWAP_V2" | "V2" => ProtocolType::V2,
        "UNISWAP_V3" | "V3" => ProtocolType::V3,
        "CURVE" => ProtocolType::Curve,
        "BALANCER" => ProtocolType::Balancer,
        _ => ProtocolType::Unknown,
    }
}

// ---------------------------------------------------------------------------
// Redis-based MVP cycle seed builder
// ---------------------------------------------------------------------------

/// Resolves caller-supplied cycle symbol triples into pool addresses via Redis
/// `arbx:pool_index:<chain>:<sym0>:<sym1>` and produces one `MvpCycleSeed`
/// per (cycle_id, pool_address) pair.
///
/// Accepts `mvp_cycles` as a parameter (rather than importing from `workers`)
/// so this module compiles as a library target without depending on the binary.
///
/// Pools that cannot be resolved from Redis (cache miss at boot) are silently
/// skipped — R8 fail-honest. The caller logs the total seeds loaded.
async fn build_mvp_seeds_from_redis(
    redis: &mut ConnectionManager,
    chain_id: u64,
    mvp_cycles: &[(&str, &str, &str)],
) -> Vec<MvpCycleSeed> {
    let mut seeds = Vec::new();

    for (cycle_idx, &(sym_a, sym_b, sym_c)) in mvp_cycles.iter().enumerate() {
        let cycle_id = cycle_idx as CycleId;

        // Resolve each of the three edges: (a,b), (b,c), (a,c).
        // Each edge can resolve to zero or more pool addresses.
        let edges: [(&str, &str); 3] = [(sym_a, sym_b), (sym_b, sym_c), (sym_a, sym_c)];
        for (s0, s1) in edges {
            let (lo, hi) = if s0 <= s1 { (s0, s1) } else { (s1, s0) };
            let key = format!("arbx:pool_index:{}:{}:{}", chain_id, lo, hi);
            let raw: Result<Option<String>, _> = redis.get(&key).await;
            if let Ok(Some(json)) = raw {
                if let Ok(addrs) = serde_json::from_str::<Vec<String>>(&json) {
                    for addr_str in addrs {
                        if let Ok(address) = Address::from_str(&addr_str) {
                            seeds.push(MvpCycleSeed {
                                cycle_id,
                                pool_address: address,
                            });
                        }
                    }
                }
            }
        }
    }

    seeds
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::route_intent::{
        DetectionSource, RouteIntent, RouteIntentLeg, RouterKind, SwapExactMode,
    };
    use ethers::types::{Address, H256, U256};

    // ── Address helpers ──────────────────────────────────────────────────────

    fn addr(n: u64) -> Address {
        Address::from_low_u64_be(n)
    }

    // ── RouteIntent helpers ──────────────────────────────────────────────────

    fn make_leg(token_in: Address, token_out: Address) -> RouteIntentLeg {
        RouteIntentLeg {
            token_in,
            token_out,
            pool_hint: None,
            dex_hint: None,
            fee_bps: None,
            protocol_type: ProtocolType::V2,
        }
    }

    fn make_intent(legs: Vec<RouteIntentLeg>) -> RouteIntent {
        RouteIntent::new(
            1,
            H256::zero(),
            Address::zero(),
            RouterKind::UniswapV2,
            Address::zero(),
            legs,
            U256::from(1_000u64),
            None,
            SwapExactMode::ExactIn,
            DetectionSource::PublicMempool,
        )
        .expect("valid intent")
    }

    // ── Pool + cycle helpers ─────────────────────────────────────────────────

    fn make_pool(chain_id: u64, address: Address, token0: Address, token1: Address) -> PoolRef {
        PoolRef {
            chain_id,
            address,
            dex_name: "uniswap-v2".to_string(),
            protocol_type: ProtocolType::V2,
            token0,
            token1,
            fee_bps: Some(30),
        }
    }

    // ── impact_index::tests::empty_intent_returns_empty ─────────────────────

    /// An intent with 0 legs must produce an empty ImpactSet (not a panic).
    ///
    /// Note: `RouteIntent::new` enforces the `legs >= 1` invariant and returns
    /// None on empty input. We test `resolve` with a manually crafted intent
    /// that has an empty legs vec to exercise the `resolve` guard directly.
    #[test]
    fn empty_intent_returns_empty() {
        let idx = ImpactIndex::empty();
        // Construct an intent directly (bypassing the constructor invariant) to
        // exercise the resolve() guard.
        let intent = RouteIntent {
            chain_id: 1,
            tx_hash: H256::zero(),
            router: Address::zero(),
            router_kind: RouterKind::Unknown,
            sender: Address::zero(),
            legs: vec![],
            amount_in: U256::zero(),
            min_amount_out: None,
            exact_mode: SwapExactMode::Unknown,
            source_event: DetectionSource::PublicMempool,
        };
        let set = idx.resolve(&intent);
        assert!(set.impacted_pairs.is_empty());
        assert!(set.impacted_pools.is_empty());
        assert!(set.impacted_cycles.is_empty());
        assert!(set.impacted_lending_positions.is_empty());
        assert!(set.impacted_protocols.is_empty());
    }

    // ── impact_index::tests::unknown_pair_returns_empty ──────────────────────

    /// A pair not in the registry contributes nothing to the ImpactSet.
    #[test]
    fn unknown_pair_returns_empty() {
        let idx = ImpactIndex::empty();
        let intent = make_intent(vec![make_leg(addr(0xA), addr(0xB))]);
        let set = idx.resolve(&intent);
        assert!(set.impacted_pools.is_empty());
        assert!(set.impacted_cycles.is_empty());
        // pair key IS captured even when no pools are registered for it
        assert_eq!(set.impacted_pairs.len(), 1);
    }

    // ── impact_index::tests::single_pool_returns_only_that_pool ─────────────

    /// When a pair has exactly one pool and no cycles, resolve returns that pool only.
    #[test]
    fn single_pool_returns_only_that_pool() {
        let tok_a = addr(0x1);
        let tok_b = addr(0x2);
        let pool_addr = addr(0xFF);

        let mut idx = ImpactIndex::empty();
        idx.add_pool(make_pool(1, pool_addr, tok_a, tok_b));

        let intent = make_intent(vec![make_leg(tok_a, tok_b)]);
        let set = idx.resolve(&intent);

        assert_eq!(set.impacted_pools.len(), 1);
        assert_eq!(set.impacted_pools[0].address, pool_addr);
        assert!(set.impacted_cycles.is_empty());
        assert_eq!(set.impacted_pairs.len(), 1);
    }

    // ── impact_index::tests::pool_returns_only_its_cycles ───────────────────

    /// A pool that belongs to exactly 2 cycles returns those 2 cycle IDs.
    #[test]
    fn pool_returns_only_its_cycles() {
        let tok_a = addr(0x1);
        let tok_b = addr(0x2);
        let pool_addr = addr(0xAA);

        let mut idx = ImpactIndex::empty();
        idx.add_pool(make_pool(1, pool_addr, tok_a, tok_b));
        idx.seed_cycles_from_mvp(&[
            MvpCycleSeed {
                cycle_id: 7,
                pool_address: pool_addr,
            },
            MvpCycleSeed {
                cycle_id: 13,
                pool_address: pool_addr,
            },
        ]);

        let intent = make_intent(vec![make_leg(tok_a, tok_b)]);
        let set = idx.resolve(&intent);

        assert_eq!(set.impacted_pools.len(), 1);
        let mut cycles = set.impacted_cycles.clone();
        cycles.sort_unstable();
        assert_eq!(cycles, vec![7u64, 13u64]);
    }

    // ── impact_index::tests::triangular_excludes_unimpacted_cycles ──────────

    /// Register 3 cycles; only cycle 1 touches the intent's pair.
    /// resolve() must return only cycle 1.
    #[test]
    fn triangular_excludes_unimpacted_cycles() {
        let tok_a = addr(0x10);
        let tok_b = addr(0x20);
        let tok_c = addr(0x30); // different pair — not in the intent

        let pool_ab = addr(0xAB);
        let pool_bc = addr(0xBC);

        let mut idx = ImpactIndex::empty();
        // pool_ab is the relevant pool (pair A-B)
        idx.add_pool(make_pool(1, pool_ab, tok_a, tok_b));
        // pool_bc is irrelevant for this intent (pair B-C)
        idx.add_pool(make_pool(1, pool_bc, tok_b, tok_c));

        // cycle 0: uses pool_ab → will be touched
        idx.seed_cycles_from_mvp(&[MvpCycleSeed {
            cycle_id: 0,
            pool_address: pool_ab,
        }]);
        // cycle 1: uses pool_bc → not touched by intent (A→B)
        idx.seed_cycles_from_mvp(&[MvpCycleSeed {
            cycle_id: 1,
            pool_address: pool_bc,
        }]);
        // cycle 2: uses a completely unrelated pool
        idx.seed_cycles_from_mvp(&[MvpCycleSeed {
            cycle_id: 2,
            pool_address: addr(0xFF),
        }]);

        let intent = make_intent(vec![make_leg(tok_a, tok_b)]);
        let set = idx.resolve(&intent);

        assert_eq!(set.impacted_cycles, vec![0u64]);
    }

    // ── impact_index::tests::same_cycle_via_two_legs_deduplicated ───────────

    /// A 2-leg intent where both legs touch the same cycle returns that cycle once.
    #[test]
    fn same_cycle_via_two_legs_deduplicated() {
        let tok_a = addr(0x1);
        let tok_b = addr(0x2);
        let tok_c = addr(0x3);

        let pool_ab = addr(0xAB);
        let pool_bc = addr(0xBC);
        let cycle_id = 42u64;

        let mut idx = ImpactIndex::empty();
        idx.add_pool(make_pool(1, pool_ab, tok_a, tok_b));
        idx.add_pool(make_pool(1, pool_bc, tok_b, tok_c));

        // Both pools belong to cycle 42
        idx.seed_cycles_from_mvp(&[
            MvpCycleSeed {
                cycle_id,
                pool_address: pool_ab,
            },
            MvpCycleSeed {
                cycle_id,
                pool_address: pool_bc,
            },
        ]);

        let intent = make_intent(vec![make_leg(tok_a, tok_b), make_leg(tok_b, tok_c)]);
        let set = idx.resolve(&intent);

        // cycle 42 appears exactly once despite both legs touching it
        assert_eq!(set.impacted_cycles.len(), 1);
        assert_eq!(set.impacted_cycles[0], cycle_id);

        // both pools are included
        assert_eq!(set.impacted_pools.len(), 2);
    }

    // ── impact_index::tests::lending_positions_only_if_indexed ──────────────

    /// Empty `token_to_lending_positions` produces no positions in the output.
    /// R8 invariant: Phase 11 not yet shipped.
    #[test]
    fn lending_positions_only_if_indexed() {
        let tok_a = addr(0x1);
        let tok_b = addr(0x2);

        let idx = ImpactIndex::empty();
        // No token_to_lending_positions seeded.

        let intent = make_intent(vec![make_leg(tok_a, tok_b)]);
        let set = idx.resolve(&intent);

        assert!(
            set.impacted_lending_positions.is_empty(),
            "no positions fabricated when indexer not seeded"
        );
    }

    // ── impact_index::tests::token_pair_key_canonical_sort ──────────────────

    /// `TokenPairKey::canonical(a, b)` and `canonical(b, a)` produce the same key.
    #[test]
    fn token_pair_key_canonical_sort() {
        let a = addr(0x100);
        let b = addr(0x200);

        let k_ab = TokenPairKey::canonical(a, b);
        let k_ba = TokenPairKey::canonical(b, a);

        assert_eq!(k_ab, k_ba, "canonical must be order-independent");
        assert!(
            k_ab.token_lo <= k_ab.token_hi,
            "token_lo must be the smaller address"
        );
    }

    // ── impact_index::tests::add_pool_updates_index ──────────────────────────

    /// After `add_pool`, `resolve` can find the new pool.
    #[test]
    fn add_pool_updates_index() {
        let tok_a = addr(0xA0);
        let tok_b = addr(0xB0);
        let pool_addr = addr(0xC0);

        let mut idx = ImpactIndex::empty();
        // Pool not yet registered — resolve returns nothing.
        let intent = make_intent(vec![make_leg(tok_a, tok_b)]);
        let set_before = idx.resolve(&intent);
        assert!(set_before.impacted_pools.is_empty());

        // Add the pool.
        idx.add_pool(make_pool(1, pool_addr, tok_a, tok_b));

        // Now resolve finds it.
        let set_after = idx.resolve(&intent);
        assert_eq!(set_after.impacted_pools.len(), 1);
        assert_eq!(set_after.impacted_pools[0].address, pool_addr);
    }

    // ── parse_protocol_type mapping pin ────────────────────────────────────

    #[test]
    fn parse_protocol_type_mapping() {
        assert_eq!(parse_protocol_type("UNISWAP_V2"), ProtocolType::V2);
        assert_eq!(parse_protocol_type("V2"), ProtocolType::V2);
        assert_eq!(parse_protocol_type("UNISWAP_V3"), ProtocolType::V3);
        assert_eq!(parse_protocol_type("V3"), ProtocolType::V3);
        assert_eq!(parse_protocol_type("CURVE"), ProtocolType::Curve);
        assert_eq!(parse_protocol_type("BALANCER"), ProtocolType::Balancer);
        assert_eq!(
            parse_protocol_type("UNKNOWN_PROTOCOL"),
            ProtocolType::Unknown
        );
        assert_eq!(parse_protocol_type(""), ProtocolType::Unknown);
    }

    // ── TokenPairKey serde round-trip ────────────────────────────────────────

    #[test]
    fn token_pair_key_serde_roundtrip() {
        let key = TokenPairKey::canonical(addr(0x100), addr(0x200));
        let json = serde_json::to_string(&key).expect("serialize");
        let back: TokenPairKey = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(key, back);
    }

    // ── same pool not duplicated when touched by two intents ─────────────────

    #[test]
    fn same_pool_not_duplicated_in_single_intent() {
        // Two legs with the same pair (edge case: could happen if a swap routes
        // through the same pool twice — degenerate but must not produce duplicates).
        let tok_a = addr(0xA);
        let tok_b = addr(0xB);
        let pool_addr = addr(0xAB);

        let mut idx = ImpactIndex::empty();
        idx.add_pool(make_pool(1, pool_addr, tok_a, tok_b));

        // Two legs with the same pair.
        let intent = make_intent(vec![
            make_leg(tok_a, tok_b),
            make_leg(tok_a, tok_b), // repeated
        ]);
        let set = idx.resolve(&intent);

        // The pool must appear exactly once despite two legs matching it.
        assert_eq!(
            set.impacted_pools.len(),
            1,
            "pool must not be duplicated for same-pair multi-leg intent"
        );
    }
}
