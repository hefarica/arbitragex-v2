//! CycleEnumerator — persists the DYNAMIC route universe (`pool_cycles`) from
//! the live PG pool registry.
//!
//! RU-1 (cartridge-math-264, Task 1): the `ImpactIndex` used to derive
//! `pool_to_cycles` only from the static `MVP_CYCLES` scaffold, so pools
//! outside those hand-listed triangles could never impact a cycle
//! (`impacted_cycles == 0` on every event). This module closes that structural
//! gap: it enumerates **closed 2–3 hop cycles** over the ACTIVE pools of the
//! `pools` PG table — reusing `unique_route_finder::find_routes` (bounded DFS,
//! rotation dedup, both directions kept) — and upserts them into `pool_cycles`
//! with `ON CONFLICT DO NOTHING` (idempotent).
//!
//! ## Separation of concerns
//! - [`enumerate_cycles`] is PURE (no I/O): pool descriptors in, canonical
//!   `PoolCycleRow`s out. Deterministic: the same pool set always yields the
//!   same rows (the canonicalizer hashes only integer topology).
//! - [`enumerate_and_persist`] is the thin async layer: load pools from PG →
//!   pure enumeration → one transaction of idempotent INSERTs → `cycle_enum.done`
//!   telemetry.
//!
//! ## Honesty rules (RULE 00 / R8)
//! - The graph built here is **topology-only**: a pool enters with honest
//!   `None` magnitude fields rather than being excluded for missing reserves —
//!   excluding it would silently shrink the cycle universe. Pricing happens
//!   downstream, never here.
//! - Hitting the [`MAX_POOL_CYCLES`] cap sets `capped=true` in the telemetry:
//!   the persisted universe is incomplete and we say so, we don't imply
//!   completeness.
//!
//! ## Row representation (shared with the `pool_cycles` table + `CycleSpec`)
//! `token_path`/`pool_path` are OPEN cycles (no closing repeat): hop `i` swaps
//! `token_path[i] → token_path[(i+1) % len]` via `pool_path[i]`, in canonical
//! rotation (starts at the lexicographically smallest token address), lowercase
//! `0x`-hex. Opposite traversal directions are distinct rows.

use crate::impact_index::{load_pools_from_pg, PoolRef};
use crate::route_discovery::graph_builder::TokenGraph;
use crate::route_discovery::types::{RouteCandidate, RouteDirection, RouteEdge};
use crate::route_discovery::unique_route_finder::{find_routes, RouteFinderConfig};
use anyhow::Context;
use ethers::types::Address;
use std::collections::HashMap;
use std::time::Instant;
use tracing::{info, warn};

/// Hard cap on cycles persisted per chain per enumeration pass
/// (plan `max_cycles_active = 5000`).
pub const MAX_POOL_CYCLES: usize = 5000;

/// One enumerated cycle in persistable form: canonical open paths (no closing
/// repeat) of lowercase-hex addresses, bound directly to the `pool_cycles`
/// `TEXT[]` columns.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PoolCycleRow {
    pub token_path: Vec<String>,
    pub pool_path: Vec<String>,
}

/// Outcome of the pure enumeration pass (no I/O — unit-testable).
#[derive(Debug, Clone, Default)]
pub struct CycleEnumeration {
    pub rows: Vec<PoolCycleRow>,
    /// `true` when [`MAX_POOL_CYCLES`] stopped enumeration early ⇒ the
    /// persisted universe is **incomplete** (R8 fail-honest).
    pub capped: bool,
    /// `true` when the per-pair branching cap dropped parallel pools.
    pub pools_truncated: bool,
}

/// Build a **topology-only** token graph from pool descriptors.
///
/// Unlike [`crate::route_discovery::graph_builder::build_graph`] this needs NO
/// reserves/metadata from Redis: cycle enumeration is a structural question, so
/// each pool contributes its two directed edges with honest `None` magnitude
/// fields instead of being rejected for missing reserves — excluding it would
/// silently shrink the cycle universe (RULE 00 / R8). Pools with an invalid
/// shape (equal/zero tokens) are skipped, mirroring `build_edges_for_pool`.
/// Non-V2/V3 protocols stay in the graph; the finder's `RouteKind::classify`
/// gate (single source of truth for the priceable universe) drops any cycle
/// that traverses one.
fn topology_graph(pools: &[PoolRef]) -> TokenGraph {
    let mut edges: Vec<RouteEdge> = Vec::with_capacity(pools.len().saturating_mul(2));
    for pool in pools {
        if pool.token0 == pool.token1 || pool.token0.is_zero() || pool.token1.is_zero() {
            continue; // invalid_pool_shape — same guard as build_edges_for_pool
        }
        let forward = RouteEdge {
            chain_id: pool.chain_id,
            pool: pool.address,
            token_in: pool.token0,
            token_out: pool.token1,
            token0: pool.token0,
            token1: pool.token1,
            protocol: pool.protocol_type,
            fee_bps: pool.fee_bps,
            liquidity_hint: None,
            log_weight: None,
            freshness_ts: 0,
            blk: 0,
            hot_token: false,
            direction: RouteDirection::ZeroForOne,
        };
        let reverse = RouteEdge {
            token_in: pool.token1,
            token_out: pool.token0,
            direction: RouteDirection::OneForZero,
            ..forward.clone()
        };
        edges.push(forward);
        edges.push(reverse);
    }

    let mut adjacency: HashMap<Address, Vec<usize>> = HashMap::new();
    for (i, e) in edges.iter().enumerate() {
        adjacency.entry(e.token_in).or_default().push(i);
    }

    TokenGraph {
        edges,
        adjacency,
        dense: None,
    }
}

/// Convert one canonical `RouteCandidate` into its persistable row form.
fn candidate_row(candidate: &RouteCandidate) -> PoolCycleRow {
    PoolCycleRow {
        token_path: candidate.tokens.iter().map(|t| format!("{t:#x}")).collect(),
        pool_path: candidate.pools.iter().map(|p| format!("{p:#x}")).collect(),
    }
}

/// Enumerate closed 2–3 hop cycles (both directions kept, rotations deduped)
/// over the given pool set. Pure and deterministic: same pools in ⇒ same rows
/// out, in a stable canonical form — which is what makes the PG upsert
/// idempotent (`ON CONFLICT DO NOTHING` hits on every re-run over an unchanged
/// pool graph).
pub fn enumerate_cycles(pools: &[PoolRef], chain_id: u64, max_cycles: usize) -> CycleEnumeration {
    let graph = topology_graph(pools);
    let cfg = RouteFinderConfig {
        max_depth: 3,
        max_routes_per_tick: max_cycles,
        mode: "cycle_enum".to_string(),
        ..RouteFinderConfig::default()
    };
    let outcome = find_routes(&graph, chain_id, &cfg);
    // Canonical output order: `find_routes` starts its DFS from
    // `graph.adjacency.keys()` (HashMap iteration order, fresh `RandomState`
    // per graph instance), so two identical calls can emit the same row SET
    // in a different sequence. Sorting restores the documented contract
    // ("same pools in ⇒ same rows out, row-for-row").
    let mut rows: Vec<PoolCycleRow> = outcome.routes.iter().map(candidate_row).collect();
    rows.sort();
    CycleEnumeration {
        rows,
        capped: outcome.capped,
        pools_truncated: outcome.pools_truncated,
    }
}

/// Enumerate 2–3 hop cycles from the ACTIVE pools of the `pools` PG table
/// (same join shape as the `ImpactIndex` boot loader) and upsert them into
/// `pool_cycles` for `chain_id`.
///
/// Idempotent: every row lands with `ON CONFLICT DO NOTHING` on the table's
/// UNIQUE key, so re-running over an unchanged pool graph inserts nothing.
/// Returns the number of NEW cycles inserted (`0` on an idempotent re-run).
/// Telemetry: `cycle_enum.done` with `count`, `capped`, `elapsed_ms` (plus a
/// `cycle_enum.capped` warn when the cap truncated the universe).
pub async fn enumerate_and_persist(pool: &sqlx::PgPool, chain_id: u64) -> anyhow::Result<usize> {
    let started = Instant::now();

    let pools = load_pools_from_pg(pool, chain_id)
        .await
        .context("cycle_enum: load active pools from PG")?;

    let enumeration = enumerate_cycles(&pools, chain_id, MAX_POOL_CYCLES);
    let attempted = enumeration.rows.len();

    let mut tx = pool.begin().await.context("cycle_enum: begin tx")?;

    let mut inserted: usize = 0;
    for row in &enumeration.rows {
        let res = sqlx::query(
            r#"
            INSERT INTO pool_cycles (chain_id, token_path, pool_path, direction)
            VALUES ($1, $2, $3, 1)
            ON CONFLICT DO NOTHING
            "#,
        )
        .bind(chain_id as i32)
        .bind(&row.token_path)
        .bind(&row.pool_path)
        .execute(&mut *tx)
        .await
        .context("cycle_enum: insert pool_cycle row")?;
        inserted += res.rows_affected() as usize;
    }

    tx.commit().await.context("cycle_enum: commit tx")?;

    info!(
        event = "cycle_enum.done",
        chain_id,
        pools = pools.len(),
        attempted,
        count = inserted,
        capped = enumeration.capped,
        pools_truncated = enumeration.pools_truncated,
        elapsed_ms = started.elapsed().as_millis() as u64,
        "pool_cycles upserted from the live pool graph"
    );

    if enumeration.capped {
        warn!(
            event = "cycle_enum.capped",
            chain_id,
            cap = MAX_POOL_CYCLES,
            "cycle enumeration truncated at the cap; the persisted universe is incomplete (R8 fail-honest)"
        );
    }

    Ok(inserted)
}

// ---------------------------------------------------------------------------
// Tests — synthetic graphs (fixtures are TEST-ONLY; runtime has zero mocks)
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::impact_index::ImpactIndex;
    use crate::route_intent::{
        DetectionSource, ProtocolType, RouteIntent, RouteIntentLeg, RouterKind, SwapExactMode,
    };
    use ethers::types::{Address, H256, U256};
    use std::collections::HashSet;

    fn addr(n: u64) -> Address {
        Address::from_low_u64_be(n)
    }

    /// Fixture tokens with a KNOWN address ordering (WETH < USDC < DAI < PEPE
    /// < USDT) so canonical rotations are predictable in assertions.
    const WETH: u64 = 1;
    const USDC: u64 = 2;
    const DAI: u64 = 3;
    const PEPE: u64 = 4;
    const USDT: u64 = 5;

    /// Fixture pools (plan's 4-token/6-pool synthetic graph):
    /// WETH-USDC, USDC-DAI, DAI-WETH, WETH-PEPE, PEPE-USDC, WETH-USDT.
    /// USDT is a leaf (no cycle can close through it).
    fn synthetic_pools() -> Vec<PoolRef> {
        let mk = |id: u64, t0: u64, t1: u64| PoolRef {
            chain_id: 1,
            address: addr(id),
            dex_name: "uniswap-v2".to_string(),
            protocol_type: ProtocolType::V2,
            token0: addr(t0),
            token1: addr(t1),
            fee_bps: Some(30),
        };
        vec![
            mk(0x11, WETH, USDC), // WETH-USDC
            mk(0x22, USDC, DAI),  // USDC-DAI
            mk(0x33, DAI, WETH),  // DAI-WETH
            mk(0x44, WETH, PEPE), // WETH-PEPE
            mk(0x55, PEPE, USDC), // PEPE-USDC
            mk(0x66, WETH, USDT), // WETH-USDT (leaf)
        ]
    }

    fn hex(n: u64) -> String {
        format!("{:#x}", addr(n))
    }

    /// The four EXACT cycles the plan requires over the synthetic graph:
    /// triangles {WETH,USDC,DAI} and {WETH,PEPE,USDC}, each in BOTH traversal
    /// directions, each in canonical rotation (starts at WETH = smallest).
    fn expected_rows() -> Vec<PoolCycleRow> {
        let row = |tokens: &[u64], pools: &[u64]| PoolCycleRow {
            token_path: tokens.iter().map(|t| hex(*t)).collect(),
            pool_path: pools.iter().map(|p| hex(*p)).collect(),
        };
        vec![
            // {WETH,USDC,DAI} forward: WETH→USDC→DAI→WETH
            row(&[WETH, USDC, DAI], &[0x11, 0x22, 0x33]),
            // {WETH,USDC,DAI} reverse: WETH→DAI→USDC→WETH
            row(&[WETH, DAI, USDC], &[0x33, 0x22, 0x11]),
            // {WETH,PEPE,USDC} forward: WETH→PEPE→USDC→WETH
            row(&[WETH, PEPE, USDC], &[0x44, 0x55, 0x11]),
            // {WETH,PEPE,USDC} reverse: WETH→USDC→PEPE→WETH
            row(&[WETH, USDC, PEPE], &[0x11, 0x55, 0x44]),
        ]
    }

    #[test]
    fn enumerates_exact_triangles_both_directions() {
        let out = enumerate_cycles(&synthetic_pools(), 1, MAX_POOL_CYCLES);
        assert_eq!(out.rows.len(), 4, "exactly the 2 triangles x 2 directions");
        let mut expected = expected_rows();
        let mut actual = out.rows.clone();
        expected.sort();
        actual.sort();
        assert_eq!(actual, expected, "exact canonical token/pool paths");
        assert!(!out.capped, "4 << 5000 cap → complete universe");
        assert!(!out.pools_truncated, "1 pool per pair → no per-pair drop");
        // The USDT leaf never appears in any cycle.
        for r in &out.rows {
            assert!(
                !r.token_path.contains(&hex(USDT)),
                "leaf token must not appear in any cycle"
            );
        }
    }

    #[test]
    fn rotation_dedup_no_duplicate_rows() {
        // Same physical cycle reached from every start token must collapse to
        // ONE row per direction (canonicalizer rotation dedup).
        let out = enumerate_cycles(&synthetic_pools(), 1, MAX_POOL_CYCLES);
        let mut seen: HashSet<(Vec<String>, Vec<String>)> = HashSet::new();
        for r in &out.rows {
            assert!(
                seen.insert((r.token_path.clone(), r.pool_path.clone())),
                "duplicate (token_path, pool_path) row: {:?}",
                r
            );
        }
        assert_eq!(seen.len(), out.rows.len());
    }

    #[test]
    fn enumeration_is_deterministic_idempotent_input() {
        // Purity: identical input ⇒ identical output (row-for-row). This is the
        // property that makes the PG upsert idempotent — on a re-run over an
        // unchanged pool graph every INSERT hits the UNIQUE conflict and
        // DO NOTHING fires (asserted structurally here; the DB layer is a thin
        // ON CONFLICT DO NOTHING upsert).
        let a = enumerate_cycles(&synthetic_pools(), 1, MAX_POOL_CYCLES);
        let b = enumerate_cycles(&synthetic_pools(), 1, MAX_POOL_CYCLES);
        assert_eq!(a.rows, b.rows);
        assert_eq!(a.capped, b.capped);
        // Input order must not matter either (dedup is content-based).
        let mut shuffled = synthetic_pools();
        shuffled.reverse();
        let c = enumerate_cycles(&shuffled, 1, MAX_POOL_CYCLES);
        let mut rows_a = a.rows.clone();
        let mut rows_c = c.rows.clone();
        rows_a.sort();
        rows_c.sort();
        assert_eq!(
            rows_a, rows_c,
            "pool input order must not change the universe"
        );
    }

    #[test]
    fn cap_truncates_and_flags_capped() {
        let out = enumerate_cycles(&synthetic_pools(), 1, 2);
        assert_eq!(out.rows.len(), 2, "hard cap respected");
        assert!(out.capped, "cap hit → universe flagged incomplete (R8)");
    }

    #[test]
    fn single_pool_pair_yields_no_cycle() {
        let pools = vec![PoolRef {
            chain_id: 1,
            address: addr(0x11),
            dex_name: "uniswap-v2".to_string(),
            protocol_type: ProtocolType::V2,
            token0: addr(WETH),
            token1: addr(USDC),
            fee_bps: Some(30),
        }];
        let out = enumerate_cycles(&pools, 1, MAX_POOL_CYCLES);
        assert!(out.rows.is_empty(), "one pool cannot close a cycle");
    }

    // ── REGRESSION (RU-1): evento sobre pool de un ciclo descubierto ⇒
    //    impacted_cycles > 0. Pre-fix this was structurally 0 because
    //    pool_to_cycles came only from the static MVP_CYCLES scaffold.
    #[test]
    fn event_on_discovered_cycle_pool_impacts_cycles() {
        let pools = synthetic_pools();
        let out = enumerate_cycles(&pools, 1, MAX_POOL_CYCLES);
        assert_eq!(out.rows.len(), 4);

        // Simulate the read path: PG assigned ids 101..=104 (row order sorted).
        let mut sorted = out.rows.clone();
        sorted.sort();
        let rows: Vec<(String, String, String)> = sorted
            .iter()
            .enumerate()
            .map(|(i, r)| {
                (
                    (101 + i as u64).to_string(),
                    r.token_path.join(","),
                    r.pool_path.join(","),
                )
            })
            .collect();
        let universe = crate::impact_index::load_pool_to_cycles(&rows, 1);
        assert_eq!(universe.specs.len(), 4);

        let mut idx = ImpactIndex::empty();
        for p in &pools {
            idx.add_pool(p.clone());
        }
        idx.load_pool_cycles(universe);

        // A swap event on the WETH/USDC pair touches pool 0x11, which belongs
        // to BOTH discovered triangles → both cycle ids must be impacted.
        let leg = RouteIntentLeg {
            token_in: addr(WETH),
            token_out: addr(USDC),
            pool_hint: None,
            dex_hint: None,
            fee_bps: None,
            protocol_type: ProtocolType::V2,
        };
        let intent = RouteIntent::new(
            1,
            H256::zero(),
            Address::zero(),
            RouterKind::UniswapV2,
            Address::zero(),
            vec![leg],
            U256::from(1_000u64),
            None,
            SwapExactMode::ExactIn,
            DetectionSource::PublicMempool,
        )
        .expect("valid intent");

        let set = idx.resolve(&intent);
        assert!(
            !set.impacted_cycles.is_empty(),
            "RU-1 regression: event on a discovered-cycle pool must impact cycles"
        );
        assert_eq!(
            set.impacted_cycles.len(),
            4,
            "both triangles x both direction rows share pool 0x11"
        );
        // Every impacted id resolves to a real spec (registry contract).
        for cid in &set.impacted_cycles {
            assert!(
                idx.cycle_spec(*cid).is_some(),
                "impacted cycle {cid} must resolve to a CycleSpec"
            );
        }

        // And a pool OUTSIDE every cycle impacts none (fail-honest bound).
        let outside_leg = RouteIntentLeg {
            token_in: addr(WETH),
            token_out: addr(USDT),
            pool_hint: None,
            dex_hint: None,
            fee_bps: None,
            protocol_type: ProtocolType::V2,
        };
        let outside_intent = RouteIntent::new(
            1,
            H256::zero(),
            Address::zero(),
            RouterKind::UniswapV2,
            Address::zero(),
            vec![outside_leg],
            U256::from(1_000u64),
            None,
            SwapExactMode::ExactIn,
            DetectionSource::PublicMempool,
        )
        .expect("valid intent");
        let outside_set = idx.resolve(&outside_intent);
        assert!(
            outside_set.impacted_cycles.is_empty(),
            "leaf pair is in no cycle — no fabricated impact"
        );
    }
}
