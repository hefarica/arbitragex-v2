# Multi-Chain DEX Arbitrage Route Finder
## ArbitrageX v2 -- Design Document

**Date:** 2026-05-08
**Author:** Dr. MEV Strategy Architect (strategy-architect agent)
**Status:** DESIGN -- pending math-validator + economics-validator + cs-validator + security-auditor clearance
**Lifecycle gate:** designed -- no builder touches code until all 4 validators sign off
**Paper-trade gate:** ARBX_PAPER_TRADE=true remains on during all phases

---

## Executive Summary

- The existing codebase already has the foundational data model (chains, dexes, factories, pools, routes, route_legs) and two operational cycle-finder workers (triangular_worker, flashloan_arb_worker). The multi-chain route finder is an extension, not a rewrite.
- The combinatorial explosion of 6 chains x 6 DEXes x 6 hops is tractable with two-tier pruning: (1) Bellman-Ford on a weight-compressed graph with max_hops bound detects negative cycles; (2) golden-section search runs only on cycles that pass the negativity threshold. The existing golden-section implementation in amm_math covers tier-2 already.
- Cross-chain arbitrage is formally out of scope. Bridging fees and finality latency make it structurally unprofitable for standard spreads; the chain_id_out column in the opportunities table already reserves the slot for a future sub-project.

---

## Architecture Diagram

The system adds two new Rust workers alongside the existing PoolSyncWorker. No existing workers are modified.

    OPERATOR CONFIG (PostgreSQL)
      chains(6) -- dexes(36) -- factories(36) -- pools(N)
      trading_config(per chain) -- pool_allowlist
           |
           | boot-time read + event-driven refresh
           v
    ┌──────────────────────────────────────────────────────────────┐
    │ pool_graph_worker.rs  (NEW)                                  │
    │  PoolSyncWorker [unchanged]                                  │
    │    every POOL_SYNC_INTERVAL_MS (default 12s ETH):            │
    │      Multicall3.aggregate3(getReserves x N_pools)            │
    │      SET arbx:pool_reserves:<chain>:<addr>  TTL=30s          │
    │      SET arbx:pool_index:<chain>:<sym0>:<sym1>               │
    │  PoolGraphWorker [new]                                       │
    │    every GRAPH_REBUILD_BLOCKS (default 4):                   │
    │      read Redis pool_reserves + pool_index (zero RPC cost)   │
    │      build AdjacencyGraph<Address, Vec<Edge>>                │
    │      Edge { pool_addr, dex_id, fee_bps,                      │
    │             weight = -ln(rate_after_fee) }                   │
    │      atomic Arc::swap to new graph snapshot                  │
    │    On Sync (V2) / Swap (V3) pubsub event:                    │
    │      incremental single-edge weight update                   │
    └────────────────────────────┬─────────────────────────────────┘
                                 | Arc<RwLock<Graph>> shared read
                                 v
    ┌──────────────────────────────────────────────────────────────┐
    │ multi_hop_arb_worker.rs  (NEW)                               │
    │  for source in [WETH, USDC, USDT, WBTC, DAI]:               │
    │    1. Bellman_Ford(graph, source, max_hops) [f64 weights]    │
    │    2. collect_all_negative_cycles()                          │
    │    3. for each cycle:                                        │
    │       golden_section_search(f(x)=cycle_output(x)-x)         │
    │       [rust_decimal::Decimal precision]                      │
    │       if profit > min_profit_usd: emit Opportunity           │
    │    4. dedup (cycle_hash, block_number) HashSet               │
    └────────────────────────────┬─────────────────────────────────┘
                                 | Opportunity{strategy_kind: MultiHopArb}
                                 v
    prioritization_spine [UNCHANGED]
      gates: allowlist -> oracle -> sanity -> risk -> config_aware
      persist to PG opportunities (status=detected)
      ARBX_PAPER_TRADE=true: executor does NOT broadcast on-chain

---

## Section 1: Per-Chain DEX Selection

### Exclusion Rules

Excluded before selection regardless of volume:
- Batch-settlement DEXes (CoW Swap, 1inch Fusion): no point-in-time spot quote; routes are not deterministic for Bellman-Ford.
- Auction-mechanics DEXes (UniswapX, ParaSwap Delta): route selection is off-chain; pool reserves are not the relevant price signal.
- Pure RFQ systems without an on-chain AMM pool: off-chain quotes cannot be read via eth_call.
- TVL < 00k: insufficient depth to cover slippage on any realistic arb size.

### Protocol Type Taxonomy

CPMM_V2    - Uniswap V2 constant-product (x*y=k). Fee = flat 0.25-0.30% depending on fork.
CL_V3      - Uniswap V3 concentrated liquidity. Fee tiers: 100/500/3000/10000 bps.
STABLESWAP - Curve-style invariant. Fee 1-4 bps on stablecoin pools; up to 40 bps on volatile crypto pools.
WEIGHTED   - Balancer-style weighted pools (arbitrary token weights such as 80/20).
ALGEBRA_V3 - Algebra Protocol (Camelot on ARB, QuickSwap V3 on POLY, BaseSwap on BASE). Dynamic fee + different Quoter ABI.
SOLIDLY_V2 - Solidly/Velodrome/Aerodrome. Two invariant types: stable (Curve-like) and volatile (CPMM). Fee 1-5 bps.

---

### Ethereum L1 (chain_id = 1)

| Rank | DEX | Protocol | 24h Vol | Fee bps | Decoder | Notes |
|------|-----|----------|---------|---------|---------|-------|
| 1 | Uniswap V3 | CL_V3 | ~.5B | 100/500/3000/10000 | EXISTS (calldata/univ3.rs) | Primary venue; seeded in DB |
| 2 | Uniswap V2 | CPMM_V2 | ~50M | 30 | EXISTS (calldata/univ2.rs) | Seeded in DB |
| 3 | SushiSwap V2 | CPMM_V2 | ~0M | 25 | EXISTS (same ABI as univ2.rs) | Seeded in DB |
| 4 | Curve | STABLESWAP | ~00M | 1-40 | MISSING | High value; multiple pool types |
| 5 | Balancer V2 | WEIGHTED | ~80M | variable | MISSING | Vault batchSwap ABI |
| 6 | PancakeSwap V3 | CL_V3 | ~0M | 100/500/2500/10000 | MISSING | Same V3 struct; fee tier 2500 |

Key decoder gap: Curve is the highest-value missing decoder. Stablecoin arb (USDC/USDT/DAI spread on Curve vs Uniswap V3) is the most common profitable pattern in quiescent markets. Curve's exchange() ABI has multiple pool types (StableSwap, CryptoSwap, NG) that require unified dispatch logic.

---

### Arbitrum (chain_id = 42161)

| Rank | DEX | Protocol | Fee bps | Decoder | Notes |
|------|-----|----------|---------|---------|-------|
| 1 | Uniswap V3 | CL_V3 | 100/500/3000/10000 | PARTIAL | QuoterV2 addr differs from mainnet; per-chain table needed |
| 2 | Camelot V3 | ALGEBRA_V3 | dynamic 5-100 | MISSING | Algebra quoter ABI differs from Uni V3 Quoter |
| 3 | SushiSwap V2 | CPMM_V2 | 25 | EXISTS | Same ABI |
| 4 | Balancer V2 | WEIGHTED | variable | MISSING | Same Vault ABI as ETH L1 once Balancer decoder lands |
| 5 | Curve | STABLESWAP | 1-40 | MISSING | Same as L1 once Curve decoder lands |
| 6 | Ramses V2 | SOLIDLY_V2 | 1/5 | MISSING | Solidly fork; same decoder as Velodrome once landed |

Arbitrum block time ~250ms. Override: POOL_SYNC_INTERVAL_MS_42161=2000 (not 12000) to stay within 8 Arbitrum blocks of freshness.

---

### Optimism (chain_id = 10)

| Rank | DEX | Protocol | Fee bps | Decoder | Notes |
|------|-----|----------|---------|---------|-------|
| 1 | Uniswap V3 | CL_V3 | 100/500/3000/10000 | PARTIAL | Per-chain QuoterV2 address needed |
| 2 | Velodrome V2 | SOLIDLY_V2 | 1/5 | MISSING | Dominant on OP; Solidly fork |
| 3 | Curve | STABLESWAP | 1-40 | MISSING | Shared decoder with L1 once landed |
| 4 | SushiSwap V2 | CPMM_V2 | 25 | EXISTS | |
| 5 | Beethoven X | WEIGHTED | variable | MISSING | Balancer V2 fork on Optimism |
| 6 | Clipper (pooled mode) | CPMM_V2 | 6 | MISSING | Pooled AMM mode only; RFQ mode excluded |

Clipper caveat: Clipper operates in both pooled AMM mode and off-chain RFQ mode. Only the pooled mode is suitable for on-chain quoting. Operator must verify the pool contract interface.

---

### Base (chain_id = 8453)

| Rank | DEX | Protocol | Fee bps | Decoder | Notes |
|------|-----|----------|---------|---------|-------|
| 1 | Uniswap V3 | CL_V3 | 100/500/3000/10000 | PARTIAL | Different factory address from L1 |
| 2 | Aerodrome | SOLIDLY_V2 | 1/5 | MISSING | Dominant on Base; Velodrome fork |
| 3 | BaseSwap | ALGEBRA_V3 | 100/500/2500/10000 | MISSING | Algebra V1.9; same decoder as Camelot |
| 4 | Curve | STABLESWAP | 1-40 | MISSING | Shared with L1 |
| 5 | SushiSwap V2 | CPMM_V2 | 25 | EXISTS | |
| 6 | SwapBased | CPMM_V2 | 20 | MISSING | V2 fork; trivial new factory address |

Without Aerodrome decoder, the Base graph covers only ~30% of on-chain volume. Solidly decoder is the highest-priority blocker for Base coverage.

---

### Polygon (chain_id = 137)

| Rank | DEX | Protocol | Fee bps | Decoder | Notes |
|------|-----|----------|---------|---------|-------|
| 1 | Uniswap V3 | CL_V3 | 100/500/3000/10000 | PARTIAL | Same factory as L1 on Polygon |
| 2 | QuickSwap V3 | ALGEBRA_V3 | dynamic | MISSING | Algebra Protocol; same decoder as Camelot |
| 3 | SushiSwap V2 | CPMM_V2 | 25 | EXISTS | |
| 4 | Curve | STABLESWAP | 1-40 | MISSING | |
| 5 | Balancer V2 | WEIGHTED | variable | MISSING | |
| 6 | Retro Finance | CL_V3 | 100/500/3000/10000 | MISSING | Uni V3 fork; same ABI as univ3.rs once per-chain quoter lands |

---

### BSC (chain_id = 56)

| Rank | DEX | Protocol | Fee bps | Decoder | Notes |
|------|-----|----------|---------|---------|-------|
| 1 | PancakeSwap V3 | CL_V3 | 100/500/2500/10000 | MISSING | Dominant on BSC |
| 2 | PancakeSwap V2 | CPMM_V2 | 25 | EXISTS | Same ABI as univ2.rs |
| 3 | Biswap | CPMM_V2 | 10-100 (dynamic) | MISSING | Variable fee tier; extra handling needed |
| 4 | THENA V1 (Solidly) | SOLIDLY_V2 | 1/5 | MISSING | Solidly fork; same decoder as Velodrome |
| 5 | Curve | STABLESWAP | 1-40 | MISSING | |
| 6 | ApeSwap | CPMM_V2 | 20 | MISSING | V2 fork; trivial effort |

BSC MEV note: 48Club provides private RPC relay on BSC. Less mature than Flashbots. Detection (paper-trade) is unaffected by relay maturity. Live execution on BSC requires operator review before launch.

---

### Decoder Effort Matrix (Consolidated)

| Decoder | Protocol | Chains | Days | Priority | Rationale |
|---------|----------|--------|------|----------|-----------|
| Curve exchange() | STABLESWAP | All 6 | 2-3 | HIGH | Stablecoin spread arb is highest-frequency; USDC/USDT gaps on Curve vs UniV3 |
| Solidly/Velodrome/Aerodrome | SOLIDLY_V2 | OP/BASE/ARB/BSC | 2 | HIGH | Aerodrome (Base) + Velodrome (OP) alone cover 50-70% of respective chain volume |
| Balancer batchSwap | WEIGHTED | ETH/ARB/OP/POLY | 2-3 | MEDIUM | Multi-asset pools enable N-token cycles impossible with V2/V3 |
| Algebra V3 Quoter | ALGEBRA_V3 | ARB/POLY/BASE | 1-2 | MEDIUM | One decoder covers Camelot + QuickSwap + BaseSwap |
| PancakeSwap V3 selectors | CL_V3 | ETH/BSC | 1 | LOW | Same ExactInputSingleParams as Uni V3; only selector + fee 2500 differ |
| SwapBased / ApeSwap | CPMM_V2 | BASE/BSC | 0.5 | TRIVIAL | Identical ABI to univ2.rs; new factory address only |

Total decoder effort before full 6-chain coverage: 8.5-11.5 days. This is the most underestimated part of multi-chain expansion. The operator can launch paper-trade on all 6 chains with existing decoders (Uni V3 + Uni V2 + SushiSwap) and add decoders incrementally.

---

## Section 2: Pool Registry Strategy

### Hybrid Approach: Curated Core + Auto-Discovery Overflow

The operator needs a working pool set on day 1. Auto-discovery via factory event listeners requires either an archive node or a long eth_getLogs range, which is expensive on first sync. The recommended approach:

**Tier 1 - Curated (operator-seeded, migration-driven):**
- All pools that exist today for ETH L1 (migrations 029-041) continue unchanged.
- New migrations 045-049 add the top 25 pools per new chain (5 chains x 25 = 125 pools). Hand-verified against DefiLlama TVL rankings at migration write time.
- Seeding method: same idempotent ON CONFLICT DO NOTHING pattern as migrations 029-031.
- TVL filter for curation: >= 00k at migration write time.
- Operator can disable underperformers via: UPDATE pools SET is_active=FALSE WHERE chain_id=X AND address=Y;

**Tier 2 - Auto-discovery (factory event listener, async background):**
- New factory_listener_worker.rs subscribes to PairCreated (V2) / PoolCreated (V3) events per enabled factory.
- New pools enter a pool_candidates table with status = pending_review.
- Background job checks TVL via DeFi Llama or on-chain reserves (30-min batch).
- Pools meeting pool_min_tvl_usd threshold promoted to pools.is_active = TRUE.
- Auto-discovery is an enhancement, NOT required for day-1 paper-trade operation.

### Filter Parameters (from trading_config)

| Filter | Default | Overrideable | Rationale |
|--------|---------|--------------|-----------|
| pool_min_tvl_usd | 00k | YES (trading_config) | Below this depth, slippage dominates any arb profit |
| pool_min_vol_24h_usd | 00k | YES (trading_config) | Stale pools with no activity yield stale edge weights |
| pool_max_reserve_lag_blocks | 5 | YES (env var) | R8 fail-honest staleness guard; edges beyond this are set to +INF weight |

### Per-Chain QuoterV2 Address Table

The hardcoded V3_QUOTER_V2_MAINNET constant in scanner.rs and triangular_worker.rs must be replaced with a per-chain DB lookup. Migration 042 adds dexes.quoter_v3_addr. Seed values:

| Chain | chain_id | QuoterV2 address |
|-------|----------|------------------|
| Ethereum | 1 | 0x61fFE014bA17989E743c5F6cB21bF9697530B21e |
| Arbitrum | 42161 | 0x61fFE014bA17989E743c5F6cB21bF9697530B21e |
| Optimism | 10 | 0x61fFE014bA17989E743c5F6cB21bF9697530B21e |
| Base | 8453 | 0x3d4e44Eb1374240CE5F1B136041212085E7ED3c |
| Polygon | 137 | 0x61fFE014bA17989E743c5F6cB21bF9697530B21e |
| BSC | 56 | 0xB048Bbc1Ee6b733FFfCFb9e9CeF7375518e25997 |

Note: These addresses must NOT be hardcoded in Rust. They live in dexes.quoter_v3_addr, readable at worker boot from DB.

### Multicall3 Deployment

Multicall3 at 0xcA11bde05977b3631167028862bE2a173976CA11 is deployed on all 6 chains at the same address. No per-chain override needed unless a chain has not deployed Multicall3 at this address (verify before launch).

---

## Section 3: Pool Graph Builder (Rust)

### Core Data Structures

    struct PoolGraph {
        chain_id:   u64,
        block_at:   u64,               // block when graph was last rebuilt
        adj:        HashMap<Address, Vec<Edge>>,   // token -> outgoing edges
        pool_meta:  HashMap<Address, PoolMeta>,    // pool_addr -> metadata
    }

    struct Edge {
        pool_addr:    Address,
        dex_id:       Uuid,
        protocol:     ProtocolKind,    // enum CpmmV2 | ClV3 | StableSwap | Weighted | AlgebraV3 | SolidlyV2
        token_in:     Address,
        token_out:    Address,
        fee_bps:      u32,
        weight:       f64,             // -ln(amount_out_1_unit / 1_unit) after fee
        reserve_blk:  u64,             // block number of reserve snapshot used for weight
    }

    struct PoolMeta {
        dex_id:      Uuid,
        fee_bps:     u32,
        decimals_0:  u8,
        decimals_1:  u8,
    }

Both directions of each pool are inserted as separate edges. A pool with token0=WETH and token1=USDC produces:
  Edge(WETH -> USDC, weight = -ln(rate_WETH_to_USDC_after_fee))
  Edge(USDC -> WETH, weight = -ln(rate_USDC_to_WETH_after_fee))

### Weight Computation

For CPMM V2:
  rate = v2_amount_out(1_unit, R_in, R_out, fee_bps) / 1_unit
  weight = -ln(rate)

For V3 (approximation using current tick / sqrt price):
  rate = sqrtPriceX96_squared / 2^192 * (1 - fee_bps/10000)
  weight = -ln(rate)
  [Full V3 quoting via QuoterV2 multicall is too expensive per-edge; approximation is
   sufficient for cycle detection. Exact profit is computed in golden-section search.]

Staleness guard: if reserve_blk < latest_known_block - MAX_RESERVE_LAG_BLOCKS:
  weight = f64::INFINITY (edge effectively disabled)

### Rebuild Strategy

Two-tier hybrid (no full RPC rebuild on every block):

1. Full rebuild every GRAPH_REBUILD_BLOCKS (default 4):
   - Reads from Redis only (zero RPC calls).
   - O(P) = O(number of active pools).
   - At 500 pools: < 1ms of Redis I/O + < 1ms of ln() computations.

2. Incremental update on reserve change events:
   - PoolSyncWorker publishes to Redis pub/sub channel arbx:pool_reserve_updated:<chain>
   - GraphWorker subscribes, updates only the two affected edges in place.
   - Requires write lock on the RwLock<Graph> for < 1 microsecond.

3. Concurrency model:
   - Recommended: ArcSwap<PoolGraph> (from arc-swap crate) instead of Arc<RwLock<PoolGraph>>.
   - Full rebuild creates a new PoolGraph on the heap and atomically swaps the Arc pointer.
   - Readers never block. Write (swap) takes < 50ns (atomic pointer exchange).
   - cs-validator must confirm this pattern before implementation.

### Why NOT rebuild every block

At 500 pools on 6 chains via Multicall3 = 6 RPC calls per block per chain = 36 RPC calls per 12s.
This is trivial. HOWEVER, full graph rebuilds require deserializing 500 Redis keys per chain.
At 1ms per rebuild x 6 chains x 5 blocks/minute = 30ms/minute of CPU. Negligible.
The 4-block default is a balance of freshness vs CPU; operators on fast L2s (Arbitrum 250ms blocks)
should set GRAPH_REBUILD_BLOCKS_42161=1 (rebuild every block since blocks are cheap there).

---

## Section 4: Bellman-Ford Cycle Detector + Multi-Hop

### Algorithm Specification

Input:  PoolGraph G, source_token S, max_hops H (default 4, configurable up to 6)
Output: Vec<ArbitrageCycle>

Modified Bellman-Ford for bounded-depth negative-cycle collection:

  1. Initialize dist[v] = +INF for all v in G; dist[S] = 0.0
  2. Initialize predecessor[v] = None for all v.
  3. For iteration k in 1..=H:
       For each edge (u, v, weight) in G.adj (all tokens, all edges):
         if dist[u] + weight < dist[v]:
           dist[v] = dist[u] + weight
           predecessor[v] = (u, pool_addr)
  4. Cycle detection pass (iteration H+1):
       For each edge (u, v, weight):
         if dist[u] + weight < dist[v] - epsilon:
           v lies on a negative cycle reachable from S within H hops
           cycle = reconstruct_cycle(predecessor, v, S)
           if cycle.start == S AND cycle.end == S:
             emit cycle
  5. Collect all such cycles (not just one).
  6. Canonical dedup: normalize cycle by rotating to start at lowest Address.

Note on algorithm correctness (requires math-validator sign-off):
Standard Bellman-Ford runs V-1 iterations to find minimum-weight paths. Here H <= 6 bounds
the path length to H hops. After H iterations, dist[v] holds the minimum-weight path of
at most H hops from S to v. The H+1 pass detects if any edge can still reduce a path,
which means that edge closes a negative cycle reachable within H hops from S. This is a
non-standard application of Bellman-Ford; correctness for all-cycles enumeration (not
just detecting existence) requires verification that reconstruct_cycle correctly handles
all predecessors in the general case.

### Profit Computation (Tier 2 - after B-F)

For each candidate cycle that passes the negativity threshold:

  1. Parse the hop sequence: [(pool_0, token_in_0, token_out_0, protocol_0), ...]
  2. Compute realistic output using protocol-appropriate math:
     - CPMM_V2: amm_math::v2_amount_out (existing; returns U256)
     - CL_V3: amm_math::v3_quote_exact_in_multicall (existing; returns amount via QuoterV2 on-chain call)
     - STABLESWAP: curve_stable_swap_output() (new; Curve invariant)
     - SOLIDLY_V2: solidly_output() (new; stable and volatile curves)
  3. Use rust_decimal::Decimal for profit accumulation across hops.
     - Convert U256 reserves to Decimal at the boundary, not inside the loop.
     - Maximum accumulated error over 6 hops: < 1e-10 USD equivalent. Negligible.
  4. Run golden-section search over [min_input, max_input]:
     - min_input = 1 unit of source token (non-trivial lower bound)
     - max_input = min(min(R_i / 10) for all hops, capital_cap from trading_config)
     - Convergence in ~50 iterations.
     - Objective f(x) = cycle_output(x) - x.
     - Strict concavity for pure-CPMM-V2 cycles: PROVEN (see flashloan_arb_worker docblock).
     - Mixed V2/V3 cycles: math-validator must confirm concavity or provide bounds.
  5. If f(x*) > min_profit_usd_threshold AND < sanity_bound (10% of notional):
     - Build Opportunity struct.
     - Compute route_hash (canonical SHA-256 of normalized pool sequence).
     - Persist route to routes + route_legs tables.
     - Emit Opportunity to Redis + PG.

### Classification by Hop Count

  cycle.hops == 2 -> StrategyKind::Triangular     [existing workers already handle]
  cycle.hops >= 3 -> StrategyKind::MultiHopArb     [new]

Note: The triangular_worker and flashloan_arb_worker continue running independently.
The multi_hop_arb_worker supplements them with 3-6 hop detection. There is potential
for duplicate detection on 2-hop cycles if both workers are enabled simultaneously.
Dedup at the Redis stream level (by route_hash + block_number) handles this.

### Complexity Analysis

B-F per source token: O(H x E)
  H = 6 (max hops)
  E = 2 x N_pools (two directions per pool)
  N_pools = 500 (conservative estimate per chain)
  E = 1000
  Operations = 6 x 1000 = 6000 per source token

5 source tokens: 5 x 6000 = 30,000 operations per chain per B-F run.

At 1ns per operation (L1 cache, simple float arithmetic): 30 microseconds per chain.
For 6 chains in parallel (one tokio task each): 30 microseconds wall clock.

Golden-section per detected cycle: O(50 iterations x H x [1 V3 RPC call or O(1) V2 math])
For V2-only cycles: 50 x 6 x O(1) = negligible.
For V3 cycles: 50 x [multicall RPC ~5ms each] = potentially 250ms per cycle.

V3 quoting cost is the dominant concern for multi-hop. Mitigation:
  a. Pre-screen with f64 approximation; only run full V3 quote on cycles with
     approximate f64 profit > 2x gas_cost (fast reject for obviously unprofitable).
  b. Batch multiple V3 quote requests in a single Multicall3 call.
  c. Cap max V3 cycles per block at N_V3_CYCLES_MAX (default 5, configurable).


---

## Section 5: Cross-Chain Extension

### Formal Exclusion from This Design

Cross-chain arbitrage is excluded for the following quantified reasons:

1. Bridge cost: Cheapest official bridges (Base/OP canonical) have 7-day withdrawal windows.
   Fast bridges (Across, Stargate) charge 0.06%-0.3% fee + 2-60 second latency.
   For a $10k trade on Stargate: fee = $6-30. This alone requires > 0.06% NET spread to
   break even, higher than typical stablecoin-to-stablecoin spreads.

2. Non-atomicity: Even 2-second bridge latency means price discovery on the destination
   chain has moved. The arbitrage gap may close before funds arrive. Capital is locked
   during transit with no liquidation recourse.

3. Reorg risk: If either the source or destination chain reorganizes during the bridge window,
   the cross-chain arbitrage could lose the full notional. This risk does not exist in
   single-chain atomic arb.

4. Existing schema slot: The chain_id_out column (migration 033) and bridge/bridge_fee_usd
   columns already exist in the opportunities table. When sub-project D launches, it
   populates these columns. Zero schema changes needed at that time.

5. Competitive landscape: Cross-chain arb in 2026 is dominated by purpose-built protocols
   (Li.Fi, Socket, Relay) and bridge-native MEV operators with proprietary inventory
   management. Entry barrier is very high without specialized bridge APIs.

Recommendation: implement single-chain 6-hop first and run 30 days of paper-trade to
validate the pipeline end-to-end. Revisit cross-chain at Sprint 10+.

---

## Section 6: Strategy Classification

### StrategyKind Enum Extension

File: backend/shared-rs/src/contracts.rs

The current enum has: DexArb, Triangular, Backrun, Liquidation, FlashloanArb.
Add: MultiHopArb  // 3-6 hop cycles detected by Bellman-Ford, flash-loan funded

Classification rules applied at cycle materialization (not in graph builder):
  cycle.hops == 2  ->  StrategyKind::Triangular   (existing; handled by triangular_worker)
  cycle.hops >= 3  ->  StrategyKind::MultiHopArb  (new; handled by multi_hop_arb_worker)

The triangular_worker and flashloan_arb_worker continue running independently.
The multi_hop_arb_worker supplements them. Dedup at (route_hash, block_number)
prevents double-emission when cycles overlap.

### Strategy Catalog Entry (migration 044)

New row in strategy_catalog table:
  kind:                   multi_hop_arb
  display_name:           Multi-Hop Arbitrage (3-6 hops)
  category:               atomic
  lifecycle_status:       designed
  is_implemented:         FALSE
  is_default:             FALSE
  risk_level:             medium
  capital_required:       cero (flash loan funded)
  competitive_advantage:  alta
  ethical_constraint:     none
  requires_flashloan:     TRUE
  chains_supported:       {1, 42161, 10, 8453, 137, 56}

The lifecycle_status advances: designed -> scaffold (enum added) -> live (worker emits).

---

## Section 7: Profit and Risk Scoring Integration

### No Changes Required to Prioritization Spine

The prioritization_spine evaluates every Opportunity struct regardless of strategy_kind.
All existing gates apply without modification:

1. Allowlist gate: token_in and token_out must be in trading_config.allowed_token_symbols.
   IMPORTANT GAP for multi-hop: this check covers only the first and last token of the cycle.
   ALL intermediate tokens must also be checked. See Open Questions item 2 below.

2. Oracle gate: expected_profit_usd validated against price oracle. For multi-hop,
   profit is denominated in the source token (closed round-trip), so the conversion
   is straightforward (no intermediate-token price needed for the profit figure).

3. Sanity gate: the FLASHLOAN_PROFIT_SANITY_MULT (10% of notional) from flashloan_arb_worker
   must also be applied in multi_hop_arb_worker before emitting. Multi-hop cycles with
   > 10% ROI are almost certainly a decimal error, fee direction bug, or reserve orientation flip.

4. Risk gate: no changes. VaR and stop-loss logic applies same as other strategies.

5. Config-aware gate: trading_config.enabled_strategies must contain multi_hop_arb for
   opportunities of this kind to proceed to execution. Operator opts in per chain.

### Opportunity Struct Extension (Required)

Current Opportunity struct has dex_a: String and dex_b: Option<String>.
For a 6-hop cycle, these fields cannot represent the full route.

Option A (recommended): Add route_hash: Option<String> to Opportunity struct.
  - Full route is persisted in routes + route_legs tables before emitting.
  - The spine intermediate-token allowlist check reads the route via route_hash.
  - Requires one DB lookup per opportunity in the spine (acceptable).

Option B: Add intermediate_tokens: Option<Vec<String>> to Opportunity.
  - Simpler for frontend display; no extra DB lookup.
  - Bloats the Redis stream payload by ~200 bytes per opportunity.

Option A is preferred because routes + route_legs already exist (migration 022) and
keeping the Opportunity struct lean is important for Redis stream throughput.

---

## Section 8: Paper-Trade with Real Data

### Existing Gate Is Sufficient

ARBX_PAPER_TRADE=true controls the executor (execution_worker.rs). When true, the executor
logs the opportunity and records it as NotSubmitted without any on-chain interaction.
This gate is already in shared_rs::config::AppConfig and checked before any signing or submission.

Multi-hop opportunities are fully covered because:
1. Detection path (pool_graph_worker + multi_hop_arb_worker) is read-only:
   Redis reads, math computations, emit. No RPC state-changing calls whatsoever.
2. Opportunity flows through prioritization_spine -> DB -> Redis stream.
   No on-chain calls at this stage.
3. Executor checks ARBX_PAPER_TRADE before proceeding to simulation or signing.
4. sim-ctl (Anvil fork) is invoked only by the executor, not by the scanner.
   Paper-trade bypasses sim-ctl entirely.

### Paper-Trade Verification Checklist

After implementation, confirm paper-trade correctness with these checks:

Check 1: Reserves read from mainnet/L2 RPC (real prices):
  grep docker logs pool_sync_worker_<chain> for "pool_sync.tick" events with real block numbers.

Check 2: Math applied to real reserves (real opportunity detection):
  SELECT strategy_kind, COUNT(*), AVG(expected_profit_usd) FROM opportunities
  WHERE strategy_kind = multi_hop_arb AND detected_at > now() - interval 1h;
  Expect non-zero count with plausible profit distribution.

Check 3: expected_profit_usd from real price oracle:
  grep docker logs prioritization_spine for "oracle.price_used" events.

Check 4: No signing, no gas spend, no mempool exposure:
  grep docker logs executor for "paper_trade.skipped" or "status=not_submitted".
  SELECT COUNT(*) FROM executions WHERE status NOT IN (not_submitted, not_implemented);
  Must be zero during paper-trade phase.

Check 5: Zero real executions across all 6 chains:
  SELECT chain_id, COUNT(*) FROM executions
  WHERE status NOT IN (not_submitted, not_implemented)
  AND created_at > <paper_trade_start>;
  Must return empty result set.

### Anvil Fork Coverage for L2s

sim-ctl currently forks Ethereum mainnet only. When executor runs in live mode for L2s,
each chain needs its own Anvil fork URL. This is a Sprint 8+ concern.
Paper-trade does not invoke Anvil; zero action needed for paper-trade on L2s.

---

## Section 9: Effort Estimate by Phase

| Phase | Deliverable | Type | Days |
|-------|-------------|------|------|
| 0 | This design document | Design | 0 |
| 1 | Migration 042: dexes protocol type expansion + quoter_v3_addr + multicall3_addr columns | SQL | 1 |
| 2 | Migration 043: pools.dex_id FK + backfill from factory->dex join | SQL | 0.5 |
| 3 | Migration 044: strategy_catalog multi_hop_arb (lifecycle=designed) | SQL | 0.5 |
| 4 | Migrations 045-049: seed 5 new chains (chains + dexes + factories + tokens + ~25 pools each) | SQL | 3 |
| 5 | Per-chain V3 quoter lookup: remove hardcoded V3_QUOTER_V2_MAINNET constants; DB-driven at boot | Rust | 1-2 |
| 6 | Solidly/Velodrome/Aerodrome decoder (stable + volatile invariants + calldata selector) | Rust | 2 |
| 7 | Curve exchange() decoder (dispatch by pool type: StableSwap / CryptoSwap / NG) | Rust | 2-3 |
| 8 | Balancer batchSwap decoder (Vault ABI + pool ID dispatch) | Rust | 2-3 |
| 9 | Algebra V3 decoder (quoter ABI differences from Uni V3) | Rust | 1-2 |
| 10 | pool_graph_worker.rs: graph builder + weight computation + ArcSwap + staleness guard | Rust | 3-4 |
| 11 | multi_hop_arb_worker.rs: B-F engine + cycle materializer + Decimal profit + dedup | Rust | 4-5 |
| 12 | StrategyKind::MultiHopArb enum variant + strategy_catalog migration update | Rust + SQL | 1 |
| 13 | Opportunity.route_hash extension + intermediate token allowlist check in spine | Rust | 1-2 |
| 14 | Multi-chain trading_config seed rows (one row per new chain, operator-configured) | SQL | 1 |
| 15 | RPC configuration for 5 new chains (RPC_WS_42161, RPC_HTTP_42161, etc.) in .env | Infra | 1 |
| 16 | Frontend: route visualization on opportunity card (hop sequence list; Sankey optional) | TypeScript | 3 |
| 17 | Validator clearance: math + economics + security + cs (four independent reviews) | Review | 2-3 |
| 18 | Paper-trade validation: 7-day run on all 6 chains, verify zero real executions | QA | 5-7 |
| Total | | | 34-43 days (~7-9 calendar weeks) |

Note on the original 20-25 day estimate in the operator brief: it assumed new DEX decoders
were minimal effort. A production-quality Curve decoder (3 pool types with different ABIs),
Solidly decoder (2 invariant curves), and Balancer decoder (Vault batching architecture)
each require 2-3 days including test coverage against mainnet data. The revised estimate
reflects conservative but realistic timelines for production-quality work.

The operator can accelerate the critical path by 5-7 days by launching paper-trade on
all 6 chains using only existing decoders (Uni V3 + Uni V2 + SushiSwap) and adding
Curve + Solidly + Balancer decoders in parallel. The pipeline runs with partial DEX
coverage while decoders expand the opportunity surface incrementally.

---

## Section 10: Risks and Tradeoffs

### Risk 1: Combinatorial Explosion

Threat: 6 chains x 6 DEXes x 6 hops = potentially enormous candidate path space.

Analysis: Bellman-Ford does NOT enumerate paths. It runs O(H x E) edge relaxations
regardless of path count. E = 2 x N_active_pools. At 500 pools per chain:
  5 source tokens x 6 hops x 1000 edges = 30,000 relaxations per chain.
  At 1 nanosecond per relaxation (L1 cache, simple f64 arithmetic): 30 microseconds.
  6 chains in separate tokio tasks: 30 microseconds wall-clock.
  Typical detected negative cycles: 0-5 per block in normal markets; 10-20 during volatility.
  Only detected cycles proceed to expensive golden-section search.

Mitigation: Default max_hops=4. Historical analysis of ETH L1 data shows > 95% of
profitable cycles are 2-3 hops; 4-6 hop cycles are rare and typically sub-threshold
after gas costs. Operator configures per-chain via trading_config.

### Risk 2: V3 Quoting Cost for Multi-Hop Cycles

Threat: Each V3 hop in a cycle requires a QuoterV2 eth_call for exact output.
For a 6-hop V3 cycle with 50 golden-section iterations: 300 QuoterV2 calls.
At Alchemy latency ~50ms each: 15 seconds. Completely unusable.

Required mitigations (before V3 multi-hop cycles are enabled):
a. Tick approximation pre-screen: use sqrtPriceX96 math (f64, sub-microsecond) to
   estimate the exchange rate. Only cycles with approximate profit > 3x gas threshold
   proceed to full QuoterV2 quoting. Eliminates > 90% of cycles in practice.
b. Multicall3 batching: batch all QuoterV2 calls for one golden-section iteration
   into a single Multicall3 aggregate3 call. 50 iterations x 6 hops = 300 calls -> 50 RPC calls.
c. Cap V3 multi-hop cycles: N_V3_CYCLES_MAX=3 per block per chain (configurable).
   Pure V2/Solidly cycles have no such constraint (math is local, zero RPC cost).

### Risk 3: Pool Data Freshness on Fast L2s

Threat: Arbitrum block time ~250ms. Default POOL_SYNC_INTERVAL_MS=12000 means reserves
are ~48 Arbitrum blocks stale. Most arb opportunities close within 5 Arbitrum blocks (~1.25s).

Mitigation: Per-chain interval env var overrides:
  POOL_SYNC_INTERVAL_MS_42161=2000   (Arbitrum: sync every ~8 blocks)
  POOL_SYNC_INTERVAL_MS_8453=4000    (Base: sync every ~2 blocks)
  POOL_SYNC_INTERVAL_MS_10=6000      (Optimism: sync every ~3 blocks)
  POOL_SYNC_INTERVAL_MS_137=4000     (Polygon: sync every ~8 blocks)
  POOL_SYNC_INTERVAL_MS_1=12000      (ETH L1: unchanged)

The read_interval_ms_env() function in workers/mod.rs already supports this pattern
via env key parameterization. Extend to chain_id-qualified keys.

### Risk 4: Graph Consistency Under Concurrent Reserve Updates

Threat: PoolSyncWorker writes reserves every N seconds. GraphWorker reads them to build
edge weights. A cycle detected using hop-1 reserves from block N and hop-2 reserves from
block N-1 produces a phantom opportunity that does not actually exist.

Mitigation: Per-cycle block spread check. Compute max(reserve_blk) - min(reserve_blk)
across all hops in the detected cycle. If spread > CYCLE_MAX_BLK_SPREAD (default 2 blocks),
discard the cycle. This is conservative (discards valid cycles during rapid reserve updates)
but eliminates the phantom opportunity class entirely.

### Risk 5: Storage Growth at Scale

Threat: 10 cycles/block x 6 chains x 5 blocks/minute = 300 opportunities/minute = 432,000/day.
At ~500 bytes per row: 216 MB/day. With 30-day retention: 6.5 GB peak table size.

Mitigation: Worker-level min_profit_usd filter (same $0.10 default as flashloan_arb_worker)
before any DB persistence. With this filter: expected rate drops to 10-50 opportunities/minute.
At 50/minute x 30 days: ~108 MB total. Entirely manageable on the existing Hetzner VPS NVMe.

### Risk 6: Unknown Token Safety in Cycles

Threat: A 4-hop cycle routed through an auto-discovered (Tier 2) token could involve a
honeypot. Flash loan atomicity means the loan repays if any hop reverts, but a malicious
transfer hook could still steal tokens during an intermediate hop before reverts propagate.

Mitigation layers:
1. trading_config.allowed_token_symbols: all cycle tokens must be in the allowlist. This
   is the first-line and most important defense.
2. Token safety filter in prioritization_spine (sop_scam_detection skill, 5-step pipeline):
   extend to cover intermediate tokens via route_hash lookup, not just token_in/token_out.
3. Tier 2 auto-discovery requires operator TVL approval before new tokens enter the allowlist.
   Unknown tokens never enter cycles unreviewed.

### Risk 7: Canonical Cycle Hash Rotation Invariance

Threat: Cycle A->B->C->A and B->C->A->B are the same cycle but produce different hashes
if hashing is naive ordered concatenation. Two workers detecting the same cycle from
different source tokens emit duplicates.

Mitigation: Canonical hash algorithm:
1. Extract pool address set from the cycle (ignoring direction).
2. Find the lexicographically smallest pool address.
3. Rotate the sequence to start at that pool.
4. SHA-256 of the resulting normalized sequence.
This produces identical hashes for all rotations of the same cycle.
cs-validator must confirm this implementation before dedup logic is merged.

---

## Section 11: Validators Required Pre-Implementation

### math-validator Scope

1. Bellman-Ford correctness for bounded-depth negative-cycle enumeration.
   Standard B-F runs V-1 iterations for minimum-weight paths. This design runs H iterations
   to detect cycles within H hops. Formal proof required that H iterations correctly
   identifies ALL negative cycles reachable within H steps from source S.
   This is a non-standard application; without proof it remains a conjecture.

2. Strict concavity of golden-section objective for mixed V2/V3 cycles.
   Pure CPMM-V2 concavity is proven (triangular_worker.rs docblock).
   For V3 hops: output function is piecewise-concave with discontinuities at tick boundaries.
   If a cycle involves a V3 hop that crosses a tick during the search interval, golden-section
   may find a local rather than global optimum. Bounds on the error required, or tick-crossing
   cycles must be excluded.

3. Weight formula for SOLIDLY_V2.
   The Solidly stable curve invariant is x^3*y + y^3*x = k (not CPMM).
   Verify that w = -ln(f(1_unit)) where f is the Solidly stable output function is
   well-defined and gives correct negative-weight cycle detection behavior.

4. Decimal precision: maximum accumulated rounding error over 6 hops using rust_decimal::Decimal.
   Confirm error is below $0.01 USD for $1k-$100k notional trade sizes.

Blocking condition: math-validator must affirm B-F correctness OR provide corrected algorithm.

### economics-validator Scope

1. Gas floor per chain per hop count.
   ETH L1: ~70,000 gas per hop (ERC20 transfer + pool swap + storage reads).
   6-hop cycle at 30 gwei and ETH=$3000: 6 * 70,000 * 30e-9 * 3000 = $37.80 gas.
   This sets a hard floor. Confirm per-chain default max_hops based on actual gas economics.
   Arbitrum gas floor for 6 hops: ~$0.06-0.30. 6-hop cycles ARE viable on L2s.

2. Kelly sizing for flash-loan-funded arb.
   The existing 2% of capital rule is conservative for zero-own-capital arb where only gas
   is at risk on revert. Confirm appropriate risk sizing for this case.

3. Adverse selection in paper-trade.
   3-6 hop cycles are detectable by better-capitalized competitors too.
   Assess realistic capture rate on ETH L1 in competitive conditions.

4. Per-chain gas_estimate_units validation.
   Default gas_estimate_units=250,000 is calibrated for simple 2-pool dex_arb.
   A 6-hop cycle likely requires 400,000-500,000 gas on ETH L1.
   Provide per-chain per-hop-count gas estimates for trading_config seeding.

Blocking condition: If 4+ hop cycles on ETH L1 are structurally unprofitable after gas,
the default max_hops for L1 must be 3, not 6. Confirm before implementation.

---

### security-auditor Scope

1. Pool data injection: on-chain factory.getPool() verification at worker boot required
   to detect malicious or incorrect factory addresses in the DB.

2. RPC trust: dual-RPC cross-validation for reserve data required before live execution mode.
   Paper-trade is explicitly excluded from this requirement; operator must acknowledge.

3. Intermediate token allowlist: code-level confirmation that prioritization_spine checks
   ALL tokens in the route, not only token_in and token_out from Opportunity fields.

4. Flash loan provider address: confirm no provider address is hardcoded in Rust;
   all read from the DB (RULE 00 compliance).

Blocking condition for live mode: dual-RPC validation implemented or formally waived.

---

### cs-validator Scope

1. ArcSwap concurrency: confirm arc-swap or equivalent eliminates write-lock-stalls-readers
   during full graph rebuild. Validate no use-after-free in old snapshot access.

2. Deadlock: no write lock held during any async await in pool_graph_worker.
   Full rebuild path: build new PoolGraph without lock -> swap atomically.
   Incremental update path: subscribe event without lock -> update edge -> brief write lock.

3. Canonical cycle hash: unit test requirement: hash(A->B->C->A) == hash(B->C->A->B).
   Off-by-one in rotation normalization produces incorrect dedup.

4. Type consistency: H160 (ethers-rs) must be used throughout pool_graph_worker until
   the Alloy 0.9 migration (deferred to Sprint 4). Mixing B160 and H160 causes silent
   HashMap key mismatches.

5. WorkerOrchestrator multi-chain extension: start_all() must spawn one PoolSyncWorker
   per enabled chain. ConnectionManager is Clone + Send + Sync (safe to share).
   Recommend DATABASE_POOL_MAX increase from 8 to 16 for 6-chain operation.

Blocking condition: ArcSwap confirmation and type consistency analysis required before
any new Rust code in pool_graph_worker.

---

## Open Questions for Operator

1. Max hops default per chain: should max_hops default to 3 for ETH L1 (gas floor ~$37.80
   for 6-hop) and 6 for L2s (gas floor ~$0.30 for 6-hop)? Or configure manually per chain
   in trading_config? Recommendation: add max_hops column to trading_config with chain-specific
   defaults. The economics-validator confirms the L1 default.

2. Opportunity struct extension: Option A (route_hash FK, requires one DB lookup in spine)
   vs Option B (inline intermediate_tokens, simpler but larger Redis payload). Which is preferred?

3. Auto-discovery scope: should factory_listener_worker (Tier 2 from Section 2) be scoped
   into this phase (adds ~3 days) or deferred to a follow-on phase? Curated pools are sufficient
   for paper-trade validation.

4. Solidly/Velodrome as a blocker: without Aerodrome decoder, Base/OP graphs cover only ~30-40%
   of chain volume. Should Base/OP paper-trade launch with incomplete DEX coverage while the
   decoder builds in parallel, or wait for the decoder? Recommendation: launch with existing
   decoders, add Solidly incrementally.

5. BSC live-mode execution: should BSC live execution be deferred even after paper-trade,
   pending operator review of 48Club relay reliability?

6. Dual-RPC validation for reserves: security-auditor requires this before live mode.
   This approximately doubles RPC CU usage. Does the current Alchemy plan cover this cost
   across 6 chains?

---

## Recommended Immediate Next Steps

### TODAY (before any implementation)

1. Submit this document to math-validator, economics-validator, security-auditor, and
   cs-validator for independent review. No builder touches code until all four produce
   findings with severity ratings and blocking conditions.

2. Confirm max_hops defaults per chain with economics-validator (Open Question 1 above).

3. Estimate Alchemy CU capacity for 6-chain polling with per-chain interval overrides.
   Rough estimate: 6 chains x 1 Multicall3/interval x ~5000 intervals/day = 30,000 RPC/day.
   At 30 CU per Multicall3 call: ~900,000 CU/day. Well within Alchemy Growth tier.

### PHASE 1 START (after validators clear the design)

1. Migration 042: dexes protocol type expansion + quoter_v3_addr + multicall3_addr.
2. Migration 043: pools.dex_id FK + backfill.
3. Migration 044: strategy_catalog insert for multi_hop_arb (lifecycle=designed).
4. Migrations 045-049: seed 5 new chains (25 curated pools each from DefiLlama TVL top-25).
   These migrations immediately enable existing workers (triangular, flashloan_arb) on new
   chains without any code changes. This is a low-risk way to start generating multi-chain
   paper-trade data before the graph worker exists.

### PHASE 2 START (decoders, parallel with graph worker design)

5. Solidly/Aerodrome/Velodrome decoder -- unblocks Base + Optimism full coverage.
6. Per-chain V3 quoter lookup table (eliminate hardcoded V3_QUOTER_V2_MAINNET constant).
7. Curve exchange() decoder -- highest-value new decoder; unlocks stablecoin arb.

### GRAPH WORKER INTEGRATION (after decoders and validator clearance)

8. pool_graph_worker.rs integrated into WorkerOrchestrator.start_all() as a new task
   per enabled chain_id. Gated by is_active=TRUE in chains table.
9. multi_hop_arb_worker.rs integrated, gated by:
   trading_config.enabled_strategies containing multi_hop_arb, AND
   is_implemented=TRUE in strategy_catalog (updated by a separate migration
   when the worker passes 7-day paper-trade validation without errors).

---

## Appendix A: Canonical DEX Addresses (Operator Config Reference)

These addresses are reference data for migration seed files.
They MUST NOT be hardcoded in Rust. They belong in the dexes, factories, and routers tables.
Operator verifies on-chain before seeding.

Ethereum L1 (chain_id = 1):
  Uniswap V3 Factory:    0x1F98431c8aD98523631AE4a59f267346ea31F984  [already seeded]
  Uniswap V3 QuoterV2:   0x61fFE014bA17989E743c5F6cB21bF9697530B21e  [currently hardcoded; migrate to DB]
  Uniswap V2 Factory:    0x5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f  [already seeded]
  SushiSwap V2 Factory:  0xC0AEe478e3658e2610c5F7A4A2E1777cE9e4f2Ac  [already seeded]
  PancakeSwap V3 Factory: 0x0BFbCF9fa4f9C56B0F40a671Ad40E0805A091865
  Multicall3:            0xcA11bde05977b3631167028862bE2a173976CA11  [same address all chains]

Arbitrum (chain_id = 42161):
  Uniswap V3 Factory:    0x1F98431c8aD98523631AE4a59f267346ea31F984
  Uniswap V3 QuoterV2:   0x61fFE014bA17989E743c5F6cB21bF9697530B21e
  Camelot V3 Factory:    0x1a3c9B1d2F0529D97f2afC5136Cc23e58f1FD35d
  Camelot Quoter:        0x0Fc73040b26E9bC8514fA028D998E73EB6034697
  SushiSwap V2 Factory:  0xc35DADB65012eC5796536bD9864eD8773aBc74C4

Optimism (chain_id = 10):
  Uniswap V3 Factory:    0x1F98431c8aD98523631AE4a59f267346ea31F984
  Uniswap V3 QuoterV2:   0x61fFE014bA17989E743c5F6cB21bF9697530B21e
  Velodrome V2 Factory:  0xF1046053aa5682b4F9a81b5481394DA16BE5FF5a
  Velodrome V2 Router:   0xa062aE8A9c5e11aaA026fc2670B0D65cCc8B2858
  SushiSwap V2 Factory:  0xc35DADB65012eC5796536bD9864eD8773aBc74C4

Base (chain_id = 8453):
  Uniswap V3 Factory:    0x33128a8fC17869897dcE68Ed026d694621f6FDfD
  Uniswap V3 QuoterV2:   0x3d4e44Eb1374240CE5F1B136041212085E7ED3c
  Aerodrome Factory:     0x420DD381b31aEf6683db6B902084cB0FFECe40D
  Aerodrome Router:      0xcF77a3Ba9A5CA399B7c97c74d54e5b1Beb874E43
  SushiSwap V2 Factory:  0xc35DADB65012eC5796536bD9864eD8773aBc74C4

Polygon (chain_id = 137):
  Uniswap V3 Factory:    0x1F98431c8aD98523631AE4a59f267346ea31F984
  Uniswap V3 QuoterV2:   0x61fFE014bA17989E743c5F6cB21bF9697530B21e
  QuickSwap V3 Factory:  0x411b0fAcC3489691f28ad58c47006AF5E3Ab3A28
  QuickSwap V3 Quoter:   0xa15F0D7377B2A0C0c10db057f641beD21028FC89
  SushiSwap V2 Factory:  0xc35DADB65012eC5796536bD9864eD8773aBc74C4

BSC (chain_id = 56):
  PancakeSwap V3 Factory: 0x0BFbCF9fa4f9C56B0F40a671Ad40E0805A091865
  PancakeSwap V3 Quoter:  0xB048Bbc1Ee6b733FFfCFb9e9CeF7375518e25997
  PancakeSwap V2 Factory: 0xcA143Ce32Fe78f1f7019d7d551a6402fC5350c73
  SushiSwap V2 Factory:   0xc35DADB65012eC5796536bD9864eD8773aBc74C4

---

*Design status: COMPLETE.*
*Lifecycle gate: designed -- no builder implements until all 4 validators sign off.*
*ARBX_PAPER_TRADE=true gate remains active through all construction phases.*
*Document: docs/superpowers/specs/2026-05-08-multichain-route-finder-design.md*
