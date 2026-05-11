# Design Spec — Event-Driven Multi-Strategy Orchestrator

**Status:** APPROVED (operator: 2026-05-11) — ready for Phase 1+ implementation
**Owner:** searcher-rs maintainer
**Sprint:** dedicated multi-phase (estimated 4-5 sub-sprints, ~4200 LOC)
**Replaces:** the hardcoded `let strategy_kind = "dex_arb_v2v2";` at
`backend/searcher-rs/src/scanner.rs:812` and the standalone polled workers
(`triangular_worker.rs`, `flashloan_arb_worker.rs`, `liquidation_worker.rs`).

---

## 1. Problem statement (verified, not theoretical)

The operator's 2026-05-11 incident report:

- `/strategies` page shows 4 enabled strategies (`dex_arb`, `flashloan_arb`,
  `triangular`, `liquidation`) with similar thresholds (`$10-20 min_profit`,
  `2% min_roi`).
- `/opportunities` dashboard surfaces only `dex_arb` rows (250 in last 6h).
- The other three emit zero opportunities despite their workers running
  diligently (`triangular_cycles_scanned: 56-100/min`,
  `flashloan_arb_pairs_scanned: 30/min`, `liquidation_positions_scanned: 0`).

### Root causes (3 distinct architectural defects)

**Defect A — strategy classification is hardcoded.**
`scanner.rs:812` assigns `strategy_kind = "dex_arb_v2v2"` to every mempool-detected
swap, regardless of its true topology. A V2→V3 cross-DEX swap, a 3-hop
triangular, a flashloan-funded route — all get the same label. The downstream
StrategyConfigGate then evaluates them against the operator's `dex_arb` config,
even when they're structurally something else.

**Defect B — polling-based detection competes against event-driven HFT bots.**
`triangular_worker`, `flashloan_arb_worker` scan static pool state every 12s.
By the time their poll observes a triangular spread, competing event-driven
bots have already arbitrated it within 1-2 blocks. The closed-form gate
`spot_product > 1.0` (line 700 of `triangular_worker.rs`) correctly rejects
99% of polled cycles because spreads close in milliseconds.

The honest math is intact; the architecture loses the race. Detection must
trigger on the *event that creates the disequilibrium* (the pending tx),
evaluate the virtual post-tx state, and emit a candidate before the tx is
included on-chain.

**Defect C — `flashloan_arb` is conceptually mis-modelled.**
Flashloan is **not** a detection signal — it's a *capital source*. A
flashloan-funded route can be a dex_arb leg, a triangular cycle, or a
liquidation. The `flashloan_arb_worker` polls for "flashloan opportunities"
as if they were a distinct phenomenon. They aren't. Aave V3 docs (cited)
make this explicit: flash loans are atomic capital wrappers; the opportunity
itself comes from the underlying route economics.

**Defect D — `liquidation` has no position indexer.**
`liquidation_worker.tick_stats` shows `positions_scanned: 0` with
`dominant_skip: empty_watchlist`. The strategy needs a `LendingPositionIndexer`
that subscribes to Aave V3 / Compound V2 borrow/withdraw/repay events,
maintains a watchlist of accounts with `health_factor < 1.05`, and
recalculates HF on oracle updates, new blocks, and position changes. This
component doesn't exist.

---

## 2. Target architecture (Mechanism C — event-driven impact graph)

```
Event Intake
   ├── PublicMempool (existing WS subscription)
   ├── FilteredMempool (flashbots/blocknative private hints — Sprint 2+)
   ├── MevShareHint (Sprint 2+)
   ├── NewBlock (existing block subscription)
   ├── OracleUpdate (Chainlink/Pyth log filters)
   └── LendingPositionUpdate (Aave/Compound events)
                       │
                       ▼
       ┌───────────────────────────────────┐
       │ route_decoder::decode_to_         │
       │ route_intents(tx, decoded, ...)   │
       │   → Vec<RouteIntent> (1-N)        │
       └─────────────────┬─────────────────┘
                         │
                         ▼
       ┌───────────────────────────────────┐
       │ Orchestrator::on_route_intent()   │
       └─────────────────┬─────────────────┘
                         │
                         ▼
       ┌───────────────────────────────────┐
       │ ImpactIndex::resolve(intent)      │
       │   → ImpactSet:                    │
       │     - impacted_pools              │
       │     - impacted_pairs              │
       │     - impacted_cycles             │
       │     - impacted_lending_positions  │
       └─────────────────┬─────────────────┘
                         │
                         ▼
       ┌───────────────────────────────────┐
       │ StrategyFanout (parallel):        │
       │   dex_engine.from_pairs(...)      │
       │   triangular_engine.from_         │
       │     cycles(...)                   │
       │   liquidation_engine.from_        │
       │     lending(...)                  │
       └─────────────────┬─────────────────┘
                         │
                         ▼  Vec<StrategyCandidate>
       ┌───────────────────────────────────┐
       │ flashloan_engine.wrap_            │
       │   profitable_routes(candidates)   │
       │   → adds flashloan-wrapped        │
       │     variants of net-positive ones │
       └─────────────────┬─────────────────┘
                         │
                         ▼
       ┌───────────────────────────────────┐
       │ state_projector::apply_pending_   │
       │   tx_delta(intent, cycle)         │
       │   → VirtualState                  │
       └─────────────────┬─────────────────┘
                         │
                         ▼
       ┌───────────────────────────────────┐
       │ size_optimizer::optimize(         │
       │   candidate, virtual_state)       │
       └─────────────────┬─────────────────┘
                         │
                         ▼
       ┌───────────────────────────────────┐
       │ ConfigAwareEvaluator::evaluate_   │
       │   with_route_plan(...)            │
       │   (existing, unchanged signature) │
       └─────────────────┬─────────────────┘
                         │
                         ▼
       ┌───────────────────────────────────┐
       │ PrioritizationSpine::score(...)   │
       │   (existing, unchanged)           │
       └─────────────────┬─────────────────┘
                         │
                         ▼
       ┌───────────────────────────────────┐
       │ OppDedup + persistence::insert    │
       │   + publisher::publish            │
       │   (existing helpers, single emit  │
       │   point — no duplicate I/O)       │
       └───────────────────────────────────┘
```

### Polling workers' new role

`triangular_worker`, `flashloan_arb_worker`, `liquidation_worker` are NOT
deleted. They become **audit-only**: they continue scanning at low cadence
(60s instead of 12s), log when they would have emitted, but do not write
to PG or publish to Redis stream. Their counters serve as a sanity check
that the event-driven engines aren't missing systematic opportunities.

Phase 12 makes this audit-only mode explicit via a worker-level env flag
`ARBX_LEGACY_WORKERS_AUDIT_ONLY=true` (default true post-refactor).

---

## 3. Type contracts (NEW MODULES)

### 3.1 `backend/searcher-rs/src/strategy_kind.rs` (Phase 1)

Local-to-searcher enum that classifies the *detected route shape*. Different
from `shared_rs::contracts::StrategyKind` (which is the persisted enum with
only 5 variants — see §6 mapping rules).

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StrategyKind {
    /// Two-leg arb where both pools are Uniswap V2-style (constant product).
    DexArbV2V2,
    /// Two-leg arb: V2 input → V3 output.
    DexArbV2V3,
    /// Two-leg arb: V3 input → V2 output.
    DexArbV3V2,
    /// Two-leg arb where both pools are Uniswap V3-style (concentrated liquidity).
    DexArbV3V3,
    /// Three-leg cycle on the same or mixed DEX family (A→B→C→A).
    TriangularArb,
    /// Any base route wrapped in a flashloan capital source. The wrapped
    /// `base_strategy` is preserved in `RouteIntent.source_event` metadata.
    FlashloanArb,
    /// Aave V3 or Compound V2 liquidation — repay debt, claim collateral
    /// at the protocol-defined bonus.
    Liquidation,
}

impl StrategyKind {
    pub fn as_str(&self) -> &'static str { /* match arm per variant */ }
    pub fn from_str_strict(s: &str) -> Option<Self> { /* inverse */ }
}

impl std::fmt::Display for StrategyKind { /* uses as_str */ }
impl std::str::FromStr for StrategyKind { /* uses from_str_strict, errors when unknown */ }
```

**Mapping to `shared_rs::contracts::StrategyKind`** (for PG persistence,
which keeps the 5-variant taxonomy unchanged):

| Detector enum (this crate) | Persisted enum (shared_rs) | DB string |
|---|---|---|
| `DexArbV2V2` | `DexArb` | `dex_arb` |
| `DexArbV2V3` | `DexArb` | `dex_arb` |
| `DexArbV3V2` | `DexArb` | `dex_arb` |
| `DexArbV3V3` | `DexArb` | `dex_arb` |
| `TriangularArb` | `Triangular` | `triangular` |
| `FlashloanArb` | `FlashloanArb` | `flashloan_arb` |
| `Liquidation` | `Liquidation` | `liquidation` |

The DEX variant differentiation is surfaced separately via the
`RoutePlan.strategy_kind` string field — that field accepts the more
granular `dex_arb_v2v3` etc. for analytics/UI without requiring a
migration to the persisted enum.

### 3.2 `backend/searcher-rs/src/route_intent.rs` (Phase 2)

```rust
use ethers::types::{Address, H256, U256};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteIntent {
    pub chain_id: u64,
    pub tx_hash: H256,
    pub router: Address,
    pub router_kind: RouterKind,
    pub sender: Address,
    pub legs: Vec<RouteIntentLeg>,
    pub amount_in: U256,
    pub min_amount_out: Option<U256>,
    pub exact_mode: SwapExactMode,
    pub source_event: DetectionSource,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteIntentLeg {
    pub token_in: Address,
    pub token_out: Address,
    pub pool_hint: Option<Address>,
    pub dex_hint: Option<String>,
    pub fee_bps: Option<u32>,
    pub protocol_type: ProtocolType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SwapExactMode { ExactIn, ExactOut, Unknown }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DetectionSource {
    PublicMempool,
    FilteredMempool,
    PrivateHint,
    NewBlock,
    OracleUpdate,
    LendingPositionUpdate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProtocolType { V2, V3, Curve, Balancer, Unknown }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RouterKind {
    UniswapV2,
    UniswapV3,
    Sushi,
    Curve,
    Balancer,
    OneInch,
    Unknown,
}
```

**R8 invariants** (must hold for every constructor):
- `pool_hint`, `dex_hint`, `fee_bps`, `min_amount_out` MUST stay `None` when
  the decoder cannot extract them from calldata. NEVER fall back to a sentinel.
- `protocol_type` defaults to `Unknown` when the router is unrecognised.
- `legs.len() >= 1` always (a swap has at least one leg by definition).

### 3.3 `backend/searcher-rs/src/route_decoder.rs` (Phase 3)

```rust
pub fn decode_to_route_intents(
    tx: &Transaction,
    decoded: &DecodedCalldata,
    router: &RouterInfo,
    chain_id: u64,
    source: DetectionSource,
) -> anyhow::Result<Vec<RouteIntent>>;
```

**Behaviour matrix** (driven by existing `calldata/univ2.rs`,
`calldata/univ3.rs` decoders that already extract path arrays + amounts):

| Calldata pattern | Output |
|---|---|
| V2 `swapExactTokensForTokens(path=[A,B])` | 1 intent, 1 leg, ExactIn, ProtocolType::V2 |
| V2 `swapExactTokensForTokens(path=[A,B,C])` | 1 intent, 2 legs (multi-hop in same family) |
| V3 `exactInputSingle(...)` | 1 intent, 1 leg, ProtocolType::V3 |
| V3 `exactInput(path=...)` | 1 intent, N legs (V3 packed path) |
| Multicall containing 2× swaps | 2 intents OR 1 intent with combined legs (see §3.3.1) |
| Unknown router | 0 intents (caller skips), `decoded_intents_total{reason="unknown_router"}` incremented |
| Calldata fails decode | 0 intents, error logged but never propagated as crash |

#### 3.3.1 Multicall decomposition rule

When the tx is a multicall:
- If sub-calls share token_in/token_out chain (the output of call N is the
  input of call N+1), produce **one** RouteIntent with N legs.
- Otherwise (parallel/unrelated swaps in one tx, e.g. Uniswap V4 multi-
  position rebalancing), produce **N** separate RouteIntents.

### 3.4 `backend/searcher-rs/src/impact_index.rs` (Phase 4)

```rust
pub struct ImpactIndex {
    /// pool_address → cycle_ids that include this pool
    pool_to_cycles: HashMap<Address, Vec<CycleId>>,
    /// (token_a, token_b) (sorted) → pools holding this pair
    token_pair_to_pools: HashMap<TokenPairKey, Vec<PoolRef>>,
    /// token_address → lending positions where it's collateral or debt
    token_to_lending_positions: HashMap<Address, Vec<UserPositionRef>>,
    /// router_address → router family classification
    router_to_protocol: HashMap<Address, RouterKind>,
    /// 4-byte function selector → decoder kind for fast dispatch
    selector_to_decoder: HashMap<[u8; 4], DecoderKind>,
}

#[derive(Debug, Clone, Default)]
pub struct ImpactSet {
    pub impacted_pairs: Vec<TokenPairKey>,
    pub impacted_pools: Vec<PoolRef>,
    pub impacted_cycles: Vec<CycleId>,
    pub impacted_lending_positions: Vec<UserPositionRef>,
    pub impacted_protocols: HashSet<ProtocolType>,
}

impl ImpactIndex {
    pub fn resolve(&self, intent: &RouteIntent) -> ImpactSet { /* O(L × P) */ }
    pub fn from_registry(/* loaders */) -> anyhow::Result<Self>;
    pub fn refresh_pool_cycles(&mut self, ...);
}
```

**Loading sources** (no synthetic data, all real):
- `pool_to_cycles`: read from `pools` and `pool_cycles` tables at boot;
  refreshed when `pool_sync_worker` adds new pools.
- `token_pair_to_pools`: derived from `pools` table (already populated by
  `pool_sync_worker`).
- `token_to_lending_positions`: populated by the new `LendingPositionIndexer`
  (Phase 9); empty when indexer hasn't run yet.
- `router_to_protocol`: static config at `configs/router_kinds.json` plus
  on-chain detection from `factory()` calls when boot encounters an unknown.
- `selector_to_decoder`: compile-time constant table.

**`resolve()` returns `Default::default()` (empty)** when no datum in the
index matches — that's R8 honest, not an error.

### 3.5 `backend/searcher-rs/src/orchestrator.rs` (Phase 5)

```rust
pub struct OrchestratorContext {
    pub impact_index: Arc<ImpactIndex>,
    pub dex_engine: Arc<DexEngine>,
    pub triangular_engine: Arc<TriangularEngine>,
    pub flashloan_engine: Arc<FlashloanEngine>,
    pub liquidation_engine: Arc<LiquidationEngine>,
    pub state_projector: Arc<StateProjector>,
    pub size_optimizer: Arc<SizeOptimizer>,
    pub evaluator: Arc<ConfigAwareEvaluator>,
    pub prioritizer: Arc<PrioritizationSpine>,
    pub opp_dedup: Arc<OppDedup>,
    pub pool: Option<PgPool>,
    pub redis: ConnectionManager,
    pub config: Arc<TradingConfigState>,
}

pub struct Orchestrator { ctx: OrchestratorContext }

impl Orchestrator {
    pub async fn on_route_intent(&self, intent: RouteIntent) -> anyhow::Result<()>;
}
```

**`on_route_intent` flow** (sequential per intent, but multiple intents
processed concurrently by spawning tasks):

```rust
async fn on_route_intent(&self, intent: RouteIntent) -> anyhow::Result<()> {
    metrics::decoded_intents_total(&intent).inc();
    let impact = self.ctx.impact_index.resolve(&intent);
    metrics::impacted_routes_total(&intent, &impact).inc();

    // StrategyFanout — parallel where independent
    let (dex, triangular, liquidation) = tokio::try_join!(
        self.ctx.dex_engine.build_from_impacted_pairs(&intent, &impact),
        self.ctx.triangular_engine.build_from_impacted_cycles(&intent, &impact),
        self.ctx.liquidation_engine.build_from_lending_impact(&intent, &impact),
    )?;
    let mut candidates: Vec<StrategyCandidate> =
        dex.into_iter().chain(triangular).chain(liquidation).collect();

    // Flashloan wrapper — applies AFTER base candidates exist
    let flash_wrapped = self.ctx.flashloan_engine
        .wrap_profitable_routes(&candidates).await?;
    candidates.extend(flash_wrapped);

    for c in candidates {
        let projected = self.ctx.state_projector.project(&intent, &c).await?;
        let sized = match self.ctx.size_optimizer.optimize(&c, &projected).await? {
            Some(s) => s,
            None => { metrics::rejected_no_profit(&c).inc(); continue; }
        };
        let gate = self.ctx.evaluator.evaluate_with_route_plan(
            &sized.candidate, Some(&sized.route_plan),
            sized.strategy_kind.as_str(), intent.chain_id,
            "rpc-pool".to_string(), self.ctx.config.gas_estimate_units.min(60_000),
        );
        let scored = match gate { /* match arms persist/publish or reject */ };
        // OppDedup applied AFTER scoring (existing pattern preserved).
        if self.ctx.opp_dedup.check_and_record(&scored.fingerprint).await {
            metrics::dedup_hit(&scored).inc(); continue;
        }
        // SINGLE EMIT POINT
        persistence::insert_opportunity(pool, &scored.opportunity).await?;
        publisher::publish(&mut self.ctx.redis.clone(), &scored.opportunity).await?;
        metrics::opportunities_published_total(&scored).inc();
    }
    Ok(())
}
```

**R8 contract**: every `continue` increments a metric with a `reason` label.
No silent drops.

### 3.6 Engines (Phase 6-9)

Each engine has a single method:

```rust
trait StrategyEngine {
    async fn build(&self, intent: &RouteIntent, impact: &ImpactSet)
        -> anyhow::Result<Vec<StrategyCandidate>>;
}

pub struct StrategyCandidate {
    pub strategy_kind: StrategyKind,        // detector-level enum
    pub route_plan: RoutePlan,              // shared spine type
    pub opportunity_seed: OpportunitySeed,  // pre-scoring data (gross,
                                            //   amount_in, pair_symbol, ...)
    pub base_strategy: Option<StrategyKind>, // Some(...) iff wrapped by
                                             //   flashloan_engine
    pub source_intent_hash: H256,           // for tracing
}
```

#### `engines/dex_engine.rs` (Phase 6)

Migrates `scanner.rs`'s existing dex-arb path into a proper engine. Builds
2-leg `RoutePlan`s with the correct V2/V3 variant labels.

Rejection labels (each becomes a metric):
- `single_pool_no_spread` — only one pool in `impacted_pools` for the
  pair, no inter-DEX arb possible.
- `no_price_oracle` — neither token priced; R8 returns None upstream.
- `non_positive_spread` — math says no profit before threshold.

#### `engines/triangular_engine.rs` (Phase 7)

```rust
pub async fn build_from_impacted_cycles(
    &self, intent: &RouteIntent, impact: &ImpactSet,
) -> anyhow::Result<Vec<StrategyCandidate>> {
    let mut out = Vec::new();
    for cycle_id in &impact.impacted_cycles {
        let cycle = self.registry.get(cycle_id)?;
        let virtual_state = self.state_projector
            .apply_pending_tx_delta(intent, &cycle).await?;
        let spot = math::spot_product_virtual(&cycle, &virtual_state);
        if spot <= cycle.min_product_after_fees {
            self.metrics.skip_no_profit.inc();
            continue;
        }
        // size optimization happens in orchestrator (not here)
        let candidate = self.builder.build(cycle, virtual_state)?;
        out.push(candidate);
    }
    Ok(out)
}
```

Key difference from current `triangular_worker.rs`: cycles are NOT polled —
only the cycles that contain at least one of the `impact.impacted_pools` are
evaluated. The `state_projector` projects the post-tx pool reserves before
`spot_product` is computed.

#### `engines/flashloan_engine.rs` (Phase 8)

```rust
pub async fn wrap_profitable_routes(
    &self, base_candidates: &[StrategyCandidate],
) -> anyhow::Result<Vec<StrategyCandidate>> {
    let mut out = Vec::new();
    for base in base_candidates {
        // Determine if the route is amenable to flashloan funding
        let borrow_asset = base.route_plan.legs[0].token_in.clone();
        let provider = self.select_provider(&borrow_asset, base.opportunity_seed.amount_in)?;
        let fee_bps = provider.fee_bps();
        let net_after_flash = base.opportunity_seed.gross_profit_usd
            - (base.opportunity_seed.amount_in_usd * (fee_bps as f64 / 10_000.0));
        if net_after_flash <= 0.0 {
            self.metrics.rejected_negative_after_fee.inc();
            continue;
        }
        let wrapped = base.with_capital_source(CapitalSource::FlashLoan {
            provider: provider.kind(),
            fee_bps,
            max_amount: provider.max_for_asset(&borrow_asset),
        });
        // Strategy kind becomes FlashloanArb; base_strategy preserved
        let wrapped = wrapped
            .with_strategy_kind(StrategyKind::FlashloanArb)
            .with_base_strategy(base.strategy_kind);
        out.push(wrapped);
    }
    Ok(out)
}
```

Flashloan provider selection (deterministic, no fabrication):
- Aave V3 if `borrow_asset` is in the operator's Aave-enabled list AND amount
  is within Aave's per-asset liquidity ceiling.
- Balancer Vault if Aave can't supply (Balancer charges 0% but smaller
  liquidity per asset).
- dYdX Solo if Ethereum mainnet and asset is in dYdX's 3-asset list.
- Otherwise return `Err(NoProviderAvailable)` — wrapper drops that variant.

#### `engines/liquidation_engine.rs` + `lending_position_indexer.rs` (Phase 9)

The indexer maintains:
```rust
pub struct LendingPosition {
    pub protocol: LendingProtocol,
    pub user: Address,
    pub collateral_assets: Vec<Address>,
    pub debt_assets: Vec<Address>,
    pub health_factor: f64,
    pub total_collateral_usd: f64,
    pub total_debt_usd: f64,
    pub last_checked_block: u64,
}
```

Trigger surface:
- New Aave V3 `Borrow`, `Withdraw`, `Repay`, `LiquidationCall` events →
  recompute that user's HF.
- Compound V2 `Borrow`, `RedeemUnderlying`, `RepayBorrow` → same.
- Oracle (`AggregatorProxy.AnswerUpdated`) → recompute HF for every user
  whose collateral/debt includes that asset.
- New block → batch-recompute the watchlist (HF<1.05) every 12s.

Engine emits when `HF < 1.0` AND simulated net (repay + receive collateral
+ optional swap collateral to debt asset − gas − protocol bonus) > 0.

Empty watchlist behaviour: `metrics::positions_watchlist_empty.inc()` and
**no synthetic positions** — that violation of R8 is the bug we're fixing.

### 3.7 `state_projector.rs` (Phase 10)

```rust
pub struct StateProjector { reserves_cache: Arc<ReservesCache>, /* ... */ }

impl StateProjector {
    pub async fn apply_pending_tx_delta(
        &self, intent: &RouteIntent, cycle: &TriangularCycle,
    ) -> anyhow::Result<VirtualState>;

    pub async fn project_v2_post_swap(
        &self, pool: &PoolRef, swap: &RouteIntentLeg,
    ) -> Option<V2VirtualReserves>;

    pub async fn project_v3_quote(
        &self, pool: &PoolRef, amount_in: U256, zero_for_one: bool,
    ) -> Option<V3VirtualQuote>;
}
```

**No mutations** — the projector reads from `ReservesCache` (already populated
by `pool_sync_worker`) and returns a *virtual* state object. Real `ReservesCache`
remains untouched.

V2 projection math: standard `x*y=k` after applying intent.amount_in to the
correct leg's reserves with fee.

V3 projection: when fee tier known, use the existing `IQuoter` adapter from
`v3_rpc_pool` to fetch a quote. Cached briefly (200ms) for replay within the
same orchestration cycle.

### 3.8 `size_optimizer.rs` (Phase 11)

Wraps the existing `golden_section_search` from `triangular_worker.rs` and
generalises it to 2-leg dex routes + 3-leg triangular + flashloan-wrapped
variants. Outputs:

```rust
pub struct SizedCandidate {
    pub candidate: StrategyCandidate,
    pub route_plan: RoutePlan,             // legs.amount_in/out populated
    pub strategy_kind: StrategyKind,
    pub gross_profit_usd: f64,
    pub estimated_net_profit_usd: f64,     // gross − var_costs − fixed_costs
    pub optimal_amount_in: U256,
}
```

Returns `Ok(None)` when:
- The math has no positive-profit point in `[min_input, cap_amount_in]`.
- The required amount exceeds the operator's capital cap.
- The token is unpriceable (R8 — propagated from upstream).

---

## 4. Scanner integration (Phase 12)

`scanner.rs` change is surgical:

```rust
// BEFORE (lines 808-867):
let evaluator = ConfigAwareEvaluator::with_cache(&cfg, signals, snapshot_map);
let strategy_kind = "dex_arb_v2v2";
let route_leg = RouteLeg { /* hardcoded single leg */ };
let route_plan = RoutePlan { strategy_kind: strategy_kind.to_string(), legs: vec![route_leg], ... };
let gate_outcome = evaluator.evaluate_with_route_plan(&candidate, Some(&route_plan), strategy_kind, ...);
// ... 200 lines of gate-outcome match arms persisting + publishing

// AFTER:
let intents = route_decoder::decode_to_route_intents(
    &tx, &decoded, &router, client.chain_id, DetectionSource::PublicMempool,
)?;
for intent in intents {
    orchestrator.on_route_intent(intent).await?;
}
```

The 200 lines of gate-outcome match arms migrate to `orchestrator.rs`. Single
emit point preserved (persistence + publisher called exactly once per
emitted opportunity).

`Orchestrator` is constructed once at `run_chain` startup and `Arc`-cloned
into each subscription handler.

---

## 5. Metrics (Phase 13)

All Prometheus counters use `StrategyKind::as_str()` as the `strategy` label
value — never a hardcoded string.

```rust
// Pre-fanout
decoded_intents_total{chain_id, source}
impacted_routes_total{chain_id, strategy}

// Per-engine
candidates_total{chain_id, strategy}
rejected_no_profit_total{chain_id, strategy, reason}
simulation_failed_total{chain_id, strategy, reason}
opportunities_published_total{chain_id, strategy}

// Specific to flashloan
flashloan_wrapped_total{chain_id, base_strategy, provider}
flashloan_rejected_negative_after_fee_total{chain_id}

// Specific to liquidation
liquidation_watchlist_size{chain_id, protocol}
liquidation_recalc_triggered_total{chain_id, trigger}
positions_watchlist_empty_total{chain_id}
```

---

## 6. Compatibility & migration plan

### 6.1 Non-breaking surface

All these existing items remain unchanged:
- `shared_rs::contracts::StrategyKind` (5-variant persisted enum)
- `shared_rs::contracts::Opportunity` shape
- `prioritization-spine::route_plan::{RoutePlan, RouteLeg}` shape
- `ConfigAwareEvaluator::evaluate_with_route_plan` signature
- `persistence::insert_opportunity` signature
- `publisher::publish` signature
- `OppDedup` API
- `RedisCachedPriceOracle` API
- All existing `tokens`, `pools`, `opportunities` table schemas (no migration)
- Frontend `OpportunityListItem` wire shape

### 6.2 Behaviour preserved bit-for-bit during Phase 12 cutover

For mempool-detected swap that today produces a `dex_arb_v2v2` opp at
amount_in X with gross Y, after Phase 12 the same swap:
- Produces a `RouteIntent` with one leg.
- `impact_index.resolve()` returns its pair as impacted.
- `dex_engine.build_from_impacted_pairs()` walks the pair's pools, finds the
  same other-pool the legacy code would have used, builds a 2-leg RoutePlan,
  classifies V2V2/V2V3/V3V2/V3V3.
- For backward compat: when the impact set has only the source pool (no
  paired pool to arb against), the engine emits a single-leg candidate with
  `strategy_kind=DexArbV2V2` (matches legacy exactly).

This is verified by parallel-run testing in Phase 12: feed the same 100 mempool
txs through old and new pipelines, assert opportunity sets are identical
modulo ordering. CI test in `scanner_parallel_run.rs`.

### 6.3 Roll-forward, roll-back

Feature flag: `ARBX_ORCHESTRATOR_MODE=v1|v2|shadow|off`.
- `v1`: legacy (current). Default until Phase 12 lands.
- `v2`: orchestrator path only. New default after Phase 12 ships.
- `shadow`: both paths run; v2 results logged + compared but only v1 emits.
  Used during cutover for one week.
- `off`: scanner short-circuits before route classification (emergency).

### 6.4 Phase ordering (re-confirmed from user prompt)

1. **Phase 0 — Spec** (this document). ✅ committed.
2. **Phase 1** — `strategy_kind.rs` enum + tests.
3. **Phase 2** — `route_intent.rs` + tests.
4. **Phase 3** — `route_decoder.rs` (uses existing calldata decoders).
5. **Phase 4** — `impact_index.rs` (boot loader from registry + Redis).
6. **Phase 5** — `orchestrator.rs` skeleton (no engines yet — empty fanout).
7. **Phase 6** — `engines/dex_engine.rs` (migrates current path).
8. **Phase 7** — `engines/triangular_engine.rs` (event-driven).
9. **Phase 8** — `engines/flashloan_engine.rs` (wrapper).
10. **Phase 9** — `engines/liquidation_engine.rs` + `lending_position_indexer.rs`.
11. **Phase 10** — `state_projector.rs`.
12. **Phase 11** — `size_optimizer.rs`.
13. **Phase 12** — `scanner.rs` integration + shadow-mode parallel run tests.
14. **Phase 13** — Prometheus metrics finalisation.
15. **Phase 14** — End-to-end test suite + parallel-run regression suite.

Each phase commits and passes `cargo fmt`, `cargo clippy -- -D warnings`,
`cargo test --workspace` before the next phase starts.

---

## 7. Risk register

| Risk | Likelihood | Mitigation |
|---|---|---|
| `state_projector` V3 quotes hammer RPC, repeat of 2026-05-11 Validation Engine incident | High | Per-pool 200ms cache + circuit breaker + use of `v3_rpc_pool` (separate provider from mempool WS) |
| `ImpactIndex` grows unbounded as new pools registered | Medium | Bounded `LruCache` for `pool_to_cycles`, evict cold cycles (last_touched > 24h) |
| Multicall decomposition produces too many intents per tx, fan-out explosion | Medium | Bounded `MAX_INTENTS_PER_TX = 16` + metric for clipped overflow |
| `Orchestrator::on_route_intent` slower than current scanner per-tx latency | Medium | Engines run in `tokio::try_join!`; benchmark in shadow mode before cutover |
| Flashloan provider liquidity check requires on-chain reads | Low | Cached liquidity snapshots from `pool_sync_worker`, refresh 1/min |
| Aave V3 `getUserAccountData` rate-limited for large watchlists | Medium | Multicall batch in `LendingPositionIndexer`, off-peak batch refresh, per-account stale_after 12s |

---

## 8. Test plan (Phase 14)

### Unit (one file per module)
- `strategy_kind::tests::as_str_roundtrip`
- `route_intent::tests::single_leg_minimal`
- `route_intent::tests::unknowns_stay_unknown` (R8 invariant)
- `route_decoder::tests::v2_simple_swap_one_leg`
- `route_decoder::tests::v3_packed_path_multi_leg`
- `route_decoder::tests::multicall_unrelated_swaps_split`
- `route_decoder::tests::unknown_router_zero_intents`
- `impact_index::tests::pool_returns_only_its_cycles`
- `impact_index::tests::unknown_pair_returns_empty`
- `orchestrator::tests::valid_intent_fans_to_three_engines`
- `orchestrator::tests::no_profit_path_increments_metric`
- `dex_engine::tests::v2_to_v3_classifies_correctly`
- `triangular_engine::tests::polled_cycle_outside_impact_not_evaluated`
- `triangular_engine::tests::spot_product_le_one_rejects`
- `flashloan_engine::tests::wraps_only_positive_routes`
- `flashloan_engine::tests::base_strategy_preserved`
- `liquidation_engine::tests::hf_above_one_no_emit`
- `liquidation_engine::tests::hf_below_one_with_net_positive_emits`
- `state_projector::tests::projection_does_not_mutate_base`
- `size_optimizer::tests::cap_bound_returns_max_feasible`

### Integration
- `scanner_parallel_run::same_tx_produces_equivalent_opp` (shadow mode CI gate)
- `orchestrator_end_to_end::single_v2_swap_emits_dex_arb_v2v2`
- `orchestrator_end_to_end::multicall_with_v2_and_v3_legs_emits_v2v3_or_v3v2`

### Grep guards (CI)
- `! grep -rn 'let strategy_kind = "dex_arb' backend/searcher-rs/src/ --include="*.rs" | grep -v test`
- `! grep -rn '"dex_arb_v2v2"' backend/searcher-rs/src/ --include="*.rs" | grep -v -E 'tests?\.|as_str|/// |// '`

---

## 9. Out of scope (deferred)

- New event sources (`MevShareHint`, `OracleUpdate` subscriptions) —
  scaffolded in `DetectionSource` enum but their listeners are Phase 15+.
- Curve / Balancer support in `route_decoder` beyond `ProtocolType::Unknown`
  classification — Phase 16+.
- Cross-chain bridge legs in `RoutePlan` — already supported by spine, but
  the orchestrator only emits same-chain routes in Phase 12.
- `Backrun` strategy (already in `shared_rs::contracts::StrategyKind` but
  not implemented anywhere) — Phase 17+.

---

## 10. Definition of done (spec-level)

The refactor is complete when:

1. `git grep -nE 'strategy_kind = "dex_arb_v2v2"' backend/searcher-rs/src/` returns zero matches outside `as_str()` definitions, tests, and docs.
2. The orchestrator emits opportunities labelled `dex_arb_v2v2`, `dex_arb_v2v3`, `dex_arb_v3v2`, `dex_arb_v3v3`, `triangular_arb`, `flashloan_arb`, `liquidation` based on actual route topology.
3. Polled legacy workers run in audit-only mode (`ARBX_LEGACY_WORKERS_AUDIT_ONLY=true`) and log discrepancies vs orchestrator output.
4. The dashboard's `/opportunities` route surfaces non-zero counts for at least 2 strategies (assuming sufficient mempool activity + functional indexers).
5. `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` all pass on the final commit.
6. No on-chain side effects from the scanner — verified by `Bash` rg of `eth_sendRawTransaction` / `signed_tx` / `wallet.sign` in `searcher-rs/src/` returning zero matches outside `execution_worker` and tests.
7. R8 fail-honest preserved end-to-end — verified by `cargo test --test r8_invariants`.
