# Technical Specification — BackrunAnticipationEngine (4 Pillars)

> **Spec ID:** `SPEC-BAE-TECH-2026-05-13`
> **Status:** Draft — companion to design doc `SPEC-BAE-2026-05-13` (committed `7df940f`)
> **Doctrine anchor:** `docs/policies/mev-ethics.md` @ `c381d5c`
> **Code paths:** GATED — §Amendments of mev-ethics.md requires 7-day cooldown before coding starts.
> **Capital posture:** Live OFF · Capital $0 · A.4 BLOCKED · A.5 NO-GO (unchanged).

---

## Preamble

This document deepens the architecture of the 4 pillars introduced in the
design doc. The deliverable here is *specification* — interfaces, math,
latency budgets, error handling, test strategy — sufficient that an
implementer can code from it after operator + on-call sign-off.

The compile-time `BundlePosition` constraint is treated as cross-cutting
and lives in §5 below; every pillar's submit path routes through it.

Throughout: the engine acts on **public market state**. The trigger
transaction is observed in the mempool and treated as a public market
event. Every action that submits an EVM bundle is ordered strictly
*after* the trigger's canonical inclusion (`BackrunOnly`) or provides
liquidity only (`JitOnly`); CEX hedging is block-independent and never
touches the trigger's effective price.

---

## 1. Pillar 1 — `flow_classifier.rs`

### 1.1 Purpose

Process the mempool stream and emit per-trigger scores: toxicity
(probability the flow is informed), whale (probability the originator
is a large counterparty), and directional confidence. These are
**signals only** — they do not call any submit path. Downstream
planners (`jit_liquidity_planner`, `backrun_planner`) gate on the
signals to decide whether and how to act, always post-confirmation.

### 1.2 Interface

```rust
pub struct ToxicityScore {
    /// Continuous value in [0, 1]. Higher = more likely informed flow.
    pub value: f64,
    /// Component breakdown for diagnostics + observability.
    pub components: ToxicityComponents,
    /// Wall-clock at which the score was emitted.
    pub emitted_at: std::time::Instant,
}

pub struct ToxicityComponents {
    /// Volume-synchronized PIN, Easley-López-O'Hara 2012.
    /// Updated per-bucket (volume-clock, not time-clock).
    pub vpin: f64,
    /// Easley-Kiefer-O'Hara-Paperman 1996. EM-estimated, slower update.
    pub pin: f64,
    /// P(whale | address_taxonomy, size, history). Bayesian prior model.
    pub whale_prior: f64,
}

pub struct WhaleScore {
    pub value: f64,                    // [0, 1]
    pub rationale: WhaleRationale,
}

pub enum WhaleRationale {
    KnownInstitutionalAddress { tag: &'static str },
    HighHistoricalVolumeEoa { volume_usd_30d: f64 },
    FactoryDeployedContract { age_days: u32 },
    SizeOutlierOnly { pct_of_pool_reserve: f64 },
    NoSignalUnknownAddress,
}

#[async_trait::async_trait]
pub trait FlowClassifier: Send + Sync {
    /// Score a pending trigger tx against current pool state and
    /// rolling-window history. MUST return in P99 < 800µs (hot-path).
    async fn classify(
        &self,
        trigger: &PendingTrigger,
        pool_snapshot: &PoolSnapshot,
    ) -> Result<(ToxicityScore, WhaleScore), ClassifyError>;
}
```

### 1.3 VPIN — exact formula and update strategy

VPIN is updated on a **volume clock**, not a wall clock. Define a
*bucket* as a contiguous slice of consecutive trades that together sum
to a target volume `V_target` (e.g., 1/50 of average daily volume for
the pool).

For each bucket `b`:

```
V_b^buy  = sum of |size_i| for trades classified as buy in bucket b
V_b^sell = sum of |size_i| for trades classified as sell in bucket b
imbalance_b = |V_b^buy - V_b^sell|
total_b     = V_b^buy + V_b^sell  ( = V_target by construction)
```

`VPIN` at time `t` over a rolling window of `N` buckets:

```
VPIN_t = (1/N) * sum_{b in window} (imbalance_b / total_b)
```

Trades are classified buy/sell via **bulk-volume classification** (BVC),
which approximates the tick rule without requiring tick-by-tick
quote data:

```
V_b^buy  = V_target × CDF_normal( (P_b - P_{b-1}) / σ_P )
V_b^sell = V_target - V_b^buy
```

where `σ_P` is the rolling standard deviation of mid-price changes
across the last `M` buckets.

**Numerical guards (R8 fail-honest):**
- If `σ_P ≤ 0` → emit ToxicityScore with `vpin = None`-equivalent (component sentinel `f64::NAN`) and exclude from blended `value`.
- If insufficient history (`< N` buckets) → emit with reduced confidence; downstream gates treat low-confidence VPIN as "skip JIT" (conservative).
- VPIN clamps to `[0, 1]`. Out-of-range values indicate computation error → log + reject candidate.

### 1.4 PIN — slow baseline via EM

PIN is a slower, block-level estimate computed off the hot path. Maintain
a count of buy / sell ticks over a longer window (1024 blocks). Estimate
parameters `(α, δ, μ, ε_b, ε_s)` via the Easley-Kiefer-O'Hara-Paperman
1996 maximum likelihood model, solved by EM. Re-estimate every 256 blocks.

`PIN = (α × μ) / (α × μ + ε_b + ε_s)`

This provides a less reactive baseline than VPIN; the classifier blends
them when both are available.

### 1.5 Bayesian whale prior

Maintain a Redis-cached address taxonomy:

```
arbx:flow:address:<addr> → {
    class: "factory" | "eoa_high_volume" | "eoa_low_volume" | "known_inst" | "unknown",
    volume_usd_30d: f64,
    historical_win_rate: f64,
    first_seen_block: u64,
    last_seen_block: u64,
}
```

Compute whale prior as:

```
P(whale | address, size, history) = P(features | whale) × P(whale) / P(features)
```

Where:
- `P(whale)` base rate = 0.05 (small prior — most addresses are not whales).
- `P(features | whale)` from historical labeled data (bootstrapped from EigenPhi traces if available, else from operator manual labels).
- `P(features)` from total observation density.

Update online via simple Bayesian moving average; full EM not needed (the rate of new whale addresses is low).

### 1.6 Architecture and latency budget

Pipeline:

```
WebSocket mempool subscription (scanner.rs existing path)
        │
        ▼
tokio::mpsc::channel (bounded, capacity 1024, single-producer single-consumer)
        │
        ▼
classifier task (spawn_blocking? NO — pure compute, async fine)
        │
        ▼ (single allocation on emit: ToxicityScore + WhaleScore)
        │
mpsc to backrun_planner + jit_liquidity_planner consumers
```

**Latency targets (P99):**
- WS receive → channel push: 200µs (existing, scanner.rs)
- channel push → classifier wake: 50µs (tokio task scheduling)
- VPIN bucket update: 30µs (rolling-buffer O(1))
- Whale prior lookup (Redis pipeline): 400µs (warm conn)
- Score emit (allocate + send): 50µs
- **Total budget: < 800µs P99**

Hot-path discipline:
- No allocations except for the emitted score struct (ring buffer for VPIN buckets is preallocated, `Vec<f64>` with capacity ≥ N).
- No `await` on operations >100µs without a corresponding timeout.
- Redis whale lookup uses `tokio::time::timeout(400µs, ...)` — on miss, falls back to no-signal whale rationale, the score is emitted without that component.

### 1.7 Doctrinal compliance

- **Signal only**: `FlowClassifier::classify` returns scores, never actions. There is no `submit_bundle` path reachable from this module.
- **Public-state only**: All inputs are public mempool data + public on-chain history.
- **No identifying retail counterparties**: The whale prior treats *behavior* (volume, deployment pattern), not identity. No retail-flow-specific signal.

### 1.8 Tests

Unit:
- VPIN deterministic on synthetic trade sequences (analytic ground truth).
- Bulk-volume classification matches tick-rule on a labeled fixture.
- Whale prior is monotonic in `volume_usd_30d` (higher volume → higher whale prob, ceteris paribus).
- Numerical guards: NaN, Inf, zero σ, empty window all return well-defined fallback.

Integration:
- 24h replay of historical mempool from EigenPhi traces; assert that addresses *labeled* as whales by EigenPhi receive `WhaleScore.value > 0.6` in ≥80% of cases.

Property (proptest):
- VPIN always in `[0, 1]` across 10^6 random inputs.
- ToxicityScore.value is monotonic increasing in component sum.

---

## 2. Pillar 2 — `impact_forecaster.rs`

### 2.1 Purpose

Given a pending trigger transaction observed in the mempool, fork
current EVM state and simulate the trigger's execution to predict
post-impact pool reserves with sub-tick precision. The output is the
basis for `jit_liquidity_planner` tick-range selection and
`backrun_planner` graph state.

**Crucial discipline**: this forecast is used only for *decisions*
(whether/where to act). The actual post-state used for backrun
execution is read from the *canonical* chain after the trigger
confirms — never from the forecast. Forecasting drives planning;
real state drives execution.

### 2.2 Interface

```rust
pub struct DirectionalForecast {
    pub trigger_tx_hash: B256,
    pub chain_id: u64,
    pub pool_address: Address,
    pub pool_kind: PoolKind,
    pub direction: PriceDirection,
    pub state_after: PoolStateAfter,
    pub confidence: f64,            // [0, 1] derived from VPIN + jitter
    pub uncertainty: ForecastUncertainty,
    pub forecast_emitted_at: std::time::Instant,
}

pub enum PoolStateAfter {
    UniV2 {
        reserve0_after: U256,
        reserve1_after: U256,
        k_invariant_after: U256,   // == before, by V2 invariant; included for sanity
    },
    UniV3 {
        sqrt_price_x96_after: U256,
        tick_after: i32,
        liquidity_after: u128,
        fee_growth_inside_x128_after: BTreeMap<TickRange, U256>,
        crossed_ticks: Vec<i32>,    // ticks that the swap crossed
    },
}

pub struct ForecastUncertainty {
    pub tick_p95_ci: i32,           // ±N ticks at 95% confidence
    pub reserve_p95_ci_bps: u32,    // ±X bps on reserves
    pub gas_price_jitter_pct: f64,  // input perturbation that produced the CI
}

#[async_trait::async_trait]
pub trait ImpactForecaster: Send + Sync {
    /// Fork state, inject trigger, predict post-state. Internally calls
    /// simulator-v2 (existing, A.2-A.4). P95 latency < 15ms (revm bound).
    async fn forecast(
        &self,
        trigger: &PendingTrigger,
        pool: &PoolDescriptor,
    ) -> Result<DirectionalForecast, ForecastError>;
}
```

### 2.3 Algorithm — V2 path

V2 CPMM with constant 0.3% fee (configurable per DEX):

```
amount_in_after_fee = amount_in × (10000 - fee_bps) / 10000

reserve_out_after = reserve_out × reserve_in / (reserve_in + amount_in_after_fee)
reserve_in_after  = reserve_in  + amount_in_after_fee

amount_out = reserve_out - reserve_out_after
```

`PoolStateAfter::UniV2` populated directly. `k_invariant_after` computed
and asserted equal to pre-trade `k` (V2 invariant). If not equal (e.g.,
fee mechanism custom), surface `ForecastError::InvariantViolation`.

Confidence: V2 deterministic — `confidence = 1.0` if pool descriptor
matches actual on-chain code hash; lower if descriptor is stale.

### 2.4 Algorithm — V3 path

V3 requires tick-by-tick traversal because liquidity is concentrated:

```
1. Start at current sqrt_price_x96, tick_current, liquidity_at_tick.
2. While amount_in remaining > 0:
     a. Determine next initialized tick in the swap direction.
     b. Compute amount_to_consume_at_next_tick:
        delta_amount_in = (sqrt_price_next - sqrt_price_current)
                          × liquidity_at_tick / 2^96
     c. If amount_in > delta_amount_in:
          - cross the tick (update liquidity by tickInfo.liquidityNet)
          - sqrt_price ← sqrt_price_next
          - amount_in -= delta_amount_in
        else:
          - partial fill within tick:
            sqrt_price_after = sqrt_price_current +
              (amount_in × 2^96) / liquidity_at_tick
          - break.
3. Emit PoolStateAfter::UniV3.
```

Use **integer-exact** math (U256, sqrt_price_x96 in Q64.96) to match
Uniswap's `SwapMath::computeSwapStep` 1:1. Floating-point is forbidden
in this kernel — sub-tick precision requires integer math.

Reference implementation: copy semantics from `uniswap-v3-core/contracts/libraries/SwapMath.sol` ported to Rust via `alloy-primitives::U256`.

### 2.5 Forking strategy

Use `simulator-v2`'s existing fork primitive (built A.2-A.4):

```rust
let fork = simulator_v2.fork_at_canonical_head(chain_id).await?;
fork.inject_pending_tx(&trigger.raw_tx_bytes)?;
let result = fork.execute_one()?;
let post_state = fork.read_pool_state(pool.address)?;
```

The fork is **owned** by this single forecast call (no reuse across
forecasts — each call gets a fresh fork from current head). This costs
~5-10ms per forecast but eliminates state contamination between
concurrent forecasts.

If the trigger reverts in fork → `ForecastError::TriggerReverts` (the
trigger is unlikely to land on canonical; downstream skips both backrun
and JIT for this trigger).

### 2.6 Uncertainty quantification

Re-simulate with perturbations on inputs to estimate confidence intervals:

- **Gas price ±10%**: triggers in queue may consume slightly different gas, shifting block position.
- **Block delay ±1 block**: pool state may shift between forecast and execution.

Repeat the V2/V3 kernel N=5 times (cheap because fork is in-memory).
Compute P95 CI on `tick_after` and `reserve_after`. Wide CI lowers
`confidence` proportionally.

### 2.7 Latency budget

P95 < 15ms per forecast:
- Fork from canonical head: 5ms (revm with pre-warm state cache)
- Inject + execute trigger: 3ms (single tx, no batching)
- Read post-state: 1ms
- Uncertainty quantification (5 perturbations): 5ms (in-memory clones)
- Result emit: 1ms

This is too slow for in-block reaction, but is well within the
trigger's window between mempool observation and inclusion (typical
mempool dwell time 200ms-2s for non-instant trades).

### 2.8 Doctrinal compliance

- **Decision-time only**: The forecast is consumed by planners to decide *whether* to act and *where* (tick range, arb path). The actual EVM action ordered against the trigger uses real post-confirmation state, never the forecast.
- **No pre-trigger tx**: This module emits a `DirectionalForecast` struct. It has no `submit_bundle` method. The struct is consumed by JIT planner (which can submit a *mint* tx ordered before trigger but with mint-only effect — see §3.7) or backrun planner (post-confirmation only).

### 2.9 Tests

Unit:
- V2 kernel: 50 historical mainnet swaps, assert `reserve_out_after` matches actual outcome within 0.01 bps (V2 is deterministic).
- V3 kernel: 50 historical mainnet swaps with tick crossings, assert `tick_after` matches actual within 0 ticks (integer-exact required).
- Revert detection: inject a tx that reverts on-chain; assert `ForecastError::TriggerReverts`.

Integration:
- Fork-test against Anvil mainnet fork for 100 large swaps; assert P95 latency < 15ms and median absolute tick error ≤ 1 tick.

---

## 3. Pillar 3 — `jit_liquidity_planner.rs`

### 3.1 Purpose

Provide concentrated V3 liquidity in the tick range the trigger swap
will cross. Capture a share of the swap fee proportional to the JIT
position's depth contribution at the active tick. Mint pre-trigger,
burn post-trigger.

### 3.2 Doctrinal mechanics statement

Per mev-ethics.md §Strategies ALLOWED (line 54-58):

> "Just-in-time (JIT) liquidity: provide concentrated depth in the tick
> range the trigger swap will cross. Captures fee share without altering
> the swapper's effective price. Hyper-aggressive concentration is
> permitted because the counterparty is **passive LPs choosing passive
> exposure under V3 design intent**, not the swapper."

Critical property: **adding depth to a tick range never worsens the
trigger's effective fill**. Mathematically, larger total `L` at the
active tick yields lower per-unit slippage. Property is automatic from
V3 swap math; this module merely chooses size and range.

### 3.3 Interface

```rust
pub struct JitPlan {
    pub trigger_tx_hash: B256,
    pub pool_address: Address,
    pub mint_params: V3MintParams,
    pub burn_params: V3BurnParams,
    pub expected_fee_capture_usd: f64,
    pub expected_capture_share: f64,       // [0, 1]
    pub gas_estimate_mint_usd: f64,
    pub gas_estimate_burn_usd: f64,
    pub net_pnl_estimate_usd: f64,
    pub kelly_capped_size: bool,           // true if Kelly cap binds
}

pub struct V3MintParams {
    pub token0: Address,
    pub token1: Address,
    pub fee_tier: u32,
    pub tick_lower: i32,
    pub tick_upper: i32,
    pub amount0_desired: U256,
    pub amount1_desired: U256,
    pub amount0_min: U256,                 // slippage guard
    pub amount1_min: U256,
    pub recipient: Address,                // operator's V3 NFT position manager
    pub deadline: u64,
}

#[async_trait::async_trait]
pub trait JitLiquidityPlanner: Send + Sync {
    async fn plan(
        &self,
        forecast: &DirectionalForecast,
        toxicity: &ToxicityScore,
        nav_wei: U256,
        cfg: &TradingConfigState,
    ) -> Result<Option<JitPlan>, PlanError>;
}
```

### 3.4 Decision gate — when to JIT, when to skip

JIT plans are emitted only when ALL of:

1. **Toxicity gate**: `toxicity.value < cfg.jit_toxicity_skip_threshold` (default 0.65). Informed flow leaves no fee profit — VPIN-informed selection wipes JIT P&L via adverse selection. This is mathematical survival, not ethics. Math: expected loss from adverse selection on informed flow exceeds fee revenue when VPIN > ~0.6.
2. **Size gate**: `forecast.amount_in_usd > cfg.jit_min_swap_size_usd` (default $50k). Smaller swaps yield insufficient fee to cover gas.
3. **Single-range gate**: `forecast.state_after.crossed_ticks.len() <= 1` for V3, or always-true for V2. Multi-tick crossings split the JIT position's effective depth across ranges.
4. **Confidence gate**: `forecast.confidence > cfg.jit_min_forecast_confidence` (default 0.7). Wide forecast CI = wrong tick range = wasted gas.

If any gate fails → emit `Ok(None)`. Caller (engine orchestrator) logs the skip reason via Prometheus counter.

### 3.5 Tick range selection (V3)

Target the tick range the trigger will cross with a buffer:

```
tick_lower = min(tick_before, tick_after) - cfg.jit_tick_buffer
tick_upper = max(tick_before, tick_after) + cfg.jit_tick_buffer
```

`cfg.jit_tick_buffer` defaults to 1 (single-tick precision). Operator
can increase to 5-10 for higher revert tolerance at cost of diluted
fee share.

### 3.6 Sizing — Kelly-bounded fee capture

Compute capture share given target liquidity `L_jit`:

```
L_existing_at_active_tick = pool.liquidity_at(tick_active)
capture_share = L_jit / (L_jit + L_existing_at_active_tick)

expected_fee_usd = forecast.amount_in_usd × fee_tier_bps / 10000
                   × capture_share

# Adjust for known JIT competition (Wintermute, Tokka, etc.)
# Empirical observation 2026-Q1: top-3 JITers split ~70-90% of fees.
jit_competition_factor = cfg.jit_competition_dilution   # default 0.6
realistic_fee_capture = expected_fee_usd × jit_competition_factor
```

Sizing decision:

```
max_capital_from_pool_depth = L_existing × 9.0  # cap at 9× existing depth
                                                # (>10× saturates capture, wastes gas)
kelly_fraction = fractional_kelly(
    win_prob: forecast.confidence × (1 - toxicity.value),  // composite
    gain_on_win: (realistic_fee_capture - gas_mint_usd - gas_burn_usd) / capital,
    loss_on_loss: (gas_mint_usd + gas_burn_usd) / capital,
    kelly_multiplier: cfg.jit_kelly_multiplier,           // 0.25 quarter-Kelly
    max_per_trade_fraction: cfg.max_per_trade_fraction,   // 0.02
);

L_target = min(L_kelly_capped, max_capital_from_pool_depth)
```

This uses the existing `kelly_sizing::fractional_kelly` primitive (deduplicates with `size_optimizer.rs` once Kelly is properly wired there per fix-1).

### 3.7 Mint / burn ordering

```
Block N (or pre-trigger in N+1):
  - mint_tx submitted to relay as JitOnly bundle position
  - effect: ADDS depth to tick_lower..tick_upper
  - the swapper's effective price is unchanged or improved (math invariant)

Block N+1 (same block as trigger if possible):
  - trigger lands, JIT position earns fee proportional to capture_share
  - burn_tx submitted as backrun bundle (same block via Flashbots bundle)
  - principal + fees withdrawn
```

If mint lands but trigger reverts or doesn't land:
- Burn the position next block, recover principal, eat gas as loss.
- Track in `arbx_bae_jit_orphan_total{chain, pool}` for tuning.

### 3.8 Doctrinal compliance — depth-only proof

Per V3 swap math, increasing `liquidity_at_tick` strictly decreases
per-unit price impact for swaps in that range. Concretely:

```
For swap of size Δx through tick with liquidity L:
  Δsqrt_price = Δx × 2^96 / L
  effective_price_impact = Δsqrt_price / sqrt_price

  L_new = L + L_jit  →  Δsqrt_price_new = Δx × 2^96 / (L + L_jit) < Δsqrt_price
```

The swapper's fill is monotonically better in `L_jit`. This is the
mathematical proof underlying the doctrinal allowance. Documented here
for audit reference.

### 3.9 Tests

Unit:
- Tick range computation matches the design (buffer applied symmetrically).
- Capture share monotone increasing in `L_jit`.
- Toxicity gate skips at threshold (boundary test).
- Kelly cap binds when full Kelly exceeds `max_per_trade_fraction`.

Integration:
- Fork-test: replay 50 historical large swaps, assert that for each,
  JIT P&L computed matches simulated mint-execute-burn within 5%.

Property:
- For all valid forecasts: `JitPlan.net_pnl_estimate_usd ≥ -gas_total`
  (worst case is wasted gas, never larger loss — by mint-only mechanic).

### 3.10 Latency budget

JIT planning runs after `flow_classifier` (800µs) and `impact_forecaster`
(15ms). The planner itself:

- Liquidity-at-tick lookup: 100µs (cached snapshot)
- Capture math: 50µs (pure compute)
- Kelly computation: 30µs (existing primitive)
- Mint tx encoding: 200µs
- **Total P95: < 500µs**

End-to-end mempool → JIT mint submitted: P95 < 20ms (dominated by forecaster).

---

## 4. Pillar 4 — `cex_hedge_dispatcher.rs`

### 4.1 Purpose

Maintain inventory delta-neutrality by hedging directional DEX
exposure on CEX spot markets. The information edge is *cross-venue*
and *anonymous-aggregate*: we sell into the deepest order book that
will absorb our hedge without leaving footprint. Counterparty on CEX
side is the aggregate order book, never a specific identifiable user.

### 4.2 Doctrinal constraint statement

Per mev-ethics.md §Strategies ALLOWED line 59-60:

> "Cross-venue CEX hedging on anonymous order books, where information
> edge is market-wide, not against a specific identifiable counterparty."

Per §Strategies REFUSED line 42-45:

> "Retail-targeted information asymmetry. Pre-running known retail
> flows (ETF rebalance windows, NFT mint sniping by privileged
> ordering). Rejected."

This module operates on CEX **spot** order books only. It does NOT:
- Pre-run ETF rebalance windows.
- Pre-run derivatives funding flips.
- Target specific known retail counterparties.
- Use any order type (IOC, post-only) that creates a measurable
  information signal in the order book.

### 4.3 Interface

```rust
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum CexVenue {
    BinanceSpot,
    OkxSpot,
    BybitSpot,
    CoinbaseProSpot,
}

pub struct HedgeIntent {
    pub venue: CexVenue,
    pub symbol: String,            // e.g., "ETHUSDT"
    pub side: Side,                // Buy | Sell
    pub size_base: f64,            // amount of base asset
    pub max_slippage_bps: u16,     // hard limit
    pub time_in_force: TimeInForce,
}

pub struct HedgeReceipt {
    pub venue: CexVenue,
    pub order_id: String,
    pub filled_base: f64,
    pub avg_fill_price: f64,
    pub filled_at: std::time::SystemTime,
}

#[async_trait::async_trait]
pub trait CexHedgeDispatcher: Send + Sync {
    async fn submit_hedge(&self, intent: HedgeIntent) -> Result<HedgeReceipt, HedgeError>;
    async fn inventory(&self, venue: CexVenue, asset: &str) -> Result<f64, HedgeError>;
    async fn book_depth(&self, venue: CexVenue, symbol: &str, side: Side, levels: u32) -> Result<Vec<(f64, f64)>, HedgeError>;
}

pub struct NullCexHedgeDispatcher;
// Logs the intent it would have placed; returns HedgeError::NotConfigured.
// MVP ships with NullCexHedgeDispatcher; real impl deferred to S+1 keys phase.
```

### 4.4 Cointegration model — Engle-Granger

The mathematical core: identify CEX and DEX price series that are
cointegrated (linear combination is stationary even though levels are
non-stationary). Trade the spread mean-reversion.

For pair `(P_cex, P_dex)`:

1. **Regress** `log(P_cex_t)` on `log(P_dex_t)` over rolling 4-hour window:
   ```
   log(P_cex_t) = α + β × log(P_dex_t) + ε_t
   ```
   `β` is the cointegration coefficient (typically ~1.0 for the same
   underlying asset).

2. **Test residual stationarity** via Augmented Dickey-Fuller (ADF) on
   `ε_t`. Reject null (residuals are stationary) at p < 0.05 →
   cointegrated. Otherwise: pair is not cointegrated, do NOT trade
   the spread for this window.

3. **z-score the residual**:
   ```
   z_t = (ε_t - mean(ε)) / stddev(ε)
   ```

4. **Half-life of mean reversion** via AR(1) on residual:
   ```
   ε_t = φ × ε_{t-1} + η_t
   half_life = -ln(2) / ln(φ)         (only meaningful if φ ∈ (0, 1))
   ```
   Half-life > 30min → reject pair (too slow for our holding window).

### 4.5 Signal generation

Open hedge when:
- Pair is cointegrated (ADF p < 0.05) AND
- |z_t| > cfg.cex_dex_open_z_threshold (default 2.0) AND
- half_life < cfg.cex_dex_max_half_life_min (default 30 min)

Close hedge when:
- |z_t| < cfg.cex_dex_close_z_threshold (default 0.5) OR
- Holding > 4 × half_life OR
- Risk-engine signal (stop-loss, kill-switch)

The hedge is paired with a DEX action only when the DirectionalForecast
indicates a price move that the z_t reversion would profit from. Pure
spread arbitrage without a DEX leg is also valid (CEX–DEX statistical
arb, strategy #7 from CLAUDE.md §17 — allowed independently).

### 4.6 Inventory neutrality

Maintain per-venue per-asset inventory state. After each DEX action:

```
expected_delta_eth = forecast.direction × position_size_eth
hedge_intent.side = -sign(expected_delta_eth)
hedge_intent.size_base = |expected_delta_eth|
```

Track cumulative net delta:

```
inventory[venue][asset] += sign(side) × filled_base
```

If `|net_delta| > cfg.max_inventory_drift_usd` for any asset → emit
`HedgeAlert` to operator dashboard; rebalance via cross-venue transfer
(if permitted) or by reducing position size on next cycle.

### 4.7 Anonymous-book mechanics

To stay within the doctrinal "anonymous order book" constraint:

- **Only spot books** (no perpetuals — funding rate adversarial games).
- **Limit orders preferred** at or near top-of-book, never IOC-only or
  post-only-only patterns that reveal intent.
- **Order size ≤ 5% of top-5-level depth** to avoid being a price-moving
  participant (which would create information asymmetry).
- **Time-in-force GTC** (good-till-cancel) — execute over seconds, not
  milliseconds. The latency edge is on the DEX side, not CEX.

### 4.8 Risk engine integration

Hedges count toward the operator's per-chain capital cap. A hedge
intent is rejected if:

- Submitting it would exceed `cfg.max_cex_inventory_per_asset`.
- The CEX venue's connectivity SLO is breached (>1s p95 latency
  observed in last 5min).
- Kill-switch active.

### 4.9 Doctrinal compliance

- **Anonymous-aggregate counterparty**: orders sit on the public order
  book and match whoever's there. No targeting.
- **No retail-flow pre-run**: explicitly excluded by venue choice (spot
  only) and timing (no ETF/derivative event timing).
- **No DEX-side mechanic harm**: CEX hedge is block-independent of any
  DEX trigger; never reorders or races a DEX user's tx.

### 4.10 Tests

Unit:
- Engle-Granger regression on synthetic cointegrated series; assert
  ADF p < 0.05 and β within 5% of construction.
- Half-life computation matches AR(1) closed-form for known φ.
- z-score crosses thresholds at correct points.

Integration (with NullCexHedgeDispatcher):
- Replay 30 days of historical ETH/USDC mid prices across Binance and
  Uniswap V3; assert that the signal would have triggered ≥ 50 times
  with median holding ≤ 2 × half_life and positive z-score reversal in
  ≥ 70% of triggers (positive expected value).

Live (after keys arrive):
- 30-day paper-shadow before any capital allocation.

### 4.11 Latency budget

The hedge is block-independent — not on the hot path. Budget:

- Signal compute (rolling regression + z): 5ms (off hot path)
- CEX REST/WS order submit: 50-200ms (network bound, CEX-side)
- Receipt fetch: 50ms
- **Total: <300ms p95**

This is fine for a hedge that operates on minutes-to-hours
mean-reversion timescale.

---

## 5. Cross-Cutting — `BundlePosition` compile-time enforcement

### 5.1 Exhaustive enum + sealed pattern

```rust
/// Bundle position relative to the trigger transaction. The variants
/// are exhaustive AND the enum is sealed: external crates cannot add
/// variants. This is the type-system anchor of the post-impact-only
/// doctrine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]                              // soft seal — operator may add new ALLOWED variants
pub enum BundlePosition {
    /// Bundle ordered AFTER the trigger tx confirms. Profit captured from
    /// public post-confirmation pool state.
    BackrunOnly,
    /// Liquidity-only bundle: mint position pre-trigger (depth+), burn
    /// post-trigger. Swapper's effective price is unchanged or improved.
    JitOnly,
}

/// Constructor sealing: there is NO `pub const fn from_u8(_: u8) -> Self`.
/// There is NO `impl From<&str> for BundlePosition`.
/// There is NO `impl Default for BundlePosition`.
/// Variants are explicit-only.

impl BundlePosition {
    /// Validation hook called at the relay-submit boundary. The
    /// boundary code MUST call this; it is the runtime mirror of the
    /// compile-time exhaustiveness.
    pub fn validate_for_submit(self) -> Result<Self, BundleRejection> {
        match self {
            Self::BackrunOnly | Self::JitOnly => Ok(self),
            // Compiler enforces exhaustiveness; this match has no other arms.
            // If a future operator-approved variant is added, this match
            // forces an explicit decision.
        }
    }
}

pub enum BundleRejection {
    UnsupportedPositionForRelay,
    PolicyViolation { ref_doctrine: &'static str },
}
```

### 5.2 Submit-path requirement

```rust
/// Every relay submit path takes `BundlePosition` by value, not by
/// untyped `u8` or `&str`. There is no escape hatch.
pub async fn submit_bundle(
    bundle: SignedBundle,
    position: BundlePosition,                  // <-- typed at the boundary
    relay: &dyn RelaySubmitter,
) -> Result<BundleReceipt, SubmitError> {
    let _ = position.validate_for_submit()?;   // explicit ack
    relay.submit(bundle, position).await
}
```

### 5.3 Compile-time test (proves the constraint)

```rust
#[test]
fn bundle_position_only_two_variants_exist() {
    // If a future PR adds `BundlePosition::Frontrun`, this test forces
    // the addition to be explicit in code review.
    fn exhaustive_match(p: BundlePosition) -> &'static str {
        match p {
            BundlePosition::BackrunOnly => "backrun_only",
            BundlePosition::JitOnly => "jit_only",
            // Adding a new variant here without operator+on-call signoff
            // per mev-ethics §Amendments is a code-review blocker.
        }
    }
    assert_eq!(exhaustive_match(BundlePosition::BackrunOnly), "backrun_only");
    assert_eq!(exhaustive_match(BundlePosition::JitOnly), "jit_only");
}
```

The `#[non_exhaustive]` attribute makes external crates forced to add
a wildcard arm — they cannot exhaustively match without source access.
This is intentional: only the operator-owned code can claim "I handle
all variants exhaustively."

### 5.4 Runtime mirror — Prometheus assertion

```
arbx_bae_bundle_position_total{chain, position}
```

This counter exposes every bundle submitted. A Prometheus rule (also
codified in this spec):

```yaml
- alert: BackrunAnticipationEnginePolicyBreach
  expr: |
    sum by (chain, position) (rate(arbx_bae_bundle_position_total{position!~"backrun_only|jit_only"}[5m])) > 0
  for: 0s        # fire immediately
  labels:
    severity: P0
    runbook: docs/runbooks/bae-policy-breach.md
```

This is the runtime safety net: even if a future bug bypassed the
type system (it shouldn't, but defense in depth), the metric trips
within 5 minutes.

---

## 6. Cross-Cutting — Latency budgets summary

| Stage | P95 budget | Cumulative |
|---|---|---|
| Mempool WS receive → flow_classifier emit | 800µs | 0.8ms |
| flow_classifier emit → impact_forecaster start | 50µs (channel) | 0.85ms |
| impact_forecaster fork+sim | 15ms | ~16ms |
| forecaster emit → jit_planner emit (or skip) | 500µs | ~16.5ms |
| jit_planner emit → relay submit | 5ms (signing + RPC) | ~21.5ms |
| Trigger inclusion (chain dependent) | 200ms-2s | 220ms-2.02s |
| Post-confirm read → backrun_planner emit | 3ms | +3ms |
| backrun_planner → relay submit | 5ms | +5ms |

End-to-end from mempool observation to backrun bundle submitted:
**~225ms-2.05s P95**, dominated by trigger inclusion latency (out of
our control). Our compute budget is ~30ms — within the per-pillar
budgets above.

---

## 7. Cross-Cutting — Testing strategy summary

| Layer | What | Where |
|---|---|---|
| Unit | Each module in isolation, mocked at trait boundaries | `backend/searcher-rs/src/engines/backrun_anticipation/tests/` |
| Integration (off-chain) | 100 historical large swaps replayed; assert latency budgets, P&L, classification correctness | `backend/searcher-rs/tests/bae_replay_*.rs` |
| Fork test | Anvil mainnet fork; full pipeline including REVM | `backend/searcher-rs/tests/bae_fork_*.rs` (gated on A.4 unblock) |
| Property | proptest invariants: VPIN ∈ [0,1], capture share monotone, JIT P&L ≥ −gas_total | inline `proptest!` blocks |
| Compile-time | `BundlePosition` exhaustiveness test | `bae_types_test.rs` |
| Paper-shadow | 30-day production shadow mode comparing to atomic-arb baseline | gated on A.5 unblock |

---

## 8. Open questions for operator

1. **VPIN bucket size `V_target`**: paper recommends 1/50 of avg daily volume per pool. For multi-DEX pools, define as the *median* across the top-3 venues, or aggregate? Default: aggregate (sum across venues). Adjustable per pool in B4 (pools admin).
2. **Toxicity skip threshold for JIT (default 0.65)**: tighten in early production (0.55) to under-fit, then loosen as paper-shadow data accumulates? Default proposal: ship at 0.65, retune at first 30-day review.
3. **CEX venue priority order**: Binance (deepest spot) → OKX → Bybit → Coinbase Pro. Operator override per chain? Default: global order, adjustable in B4 admin.
4. **Hedge size cap relative to position**: 100% (full delta-neutral) or 80% (residual exposure)? 100% is doctrinally cleanest; 80% leaves some directional alpha but may breach risk-engine cap. Default: 100% full hedge.
5. **`max_capital_from_pool_depth` JIT cap multiplier (default 9× existing depth)**: at 10× you saturate capture share at ~91% (`10/11`); beyond that, gas dominates. Operator OK with 9× as ceiling?
6. **Kelly inputs (composite `win_prob`)**: forecast.confidence × (1 - toxicity.value) is the proposed composite. Alternatives: weighted geometric mean, or hard-AND (both must exceed threshold). Default: multiplicative product (current proposal).

---

## 9. Compliance gate (must be GREEN to begin coding)

Per mev-ethics.md §Amendments: a 7-day cooldown is required between
this spec's sign-off and the first matching code path. The cooldown
starts when:

- [ ] Operator written approval (sign here when ready).
- [ ] On-call written approval.
- [ ] `docs/governance/DATA-MATRIX.md §M9` entry added referencing this spec ID.

Other gates (from design doc §8):

- [⏳] `B1.c` hot-reload subscriber lands.
- [⏳] `B2-B4` admin foundation lands (chains, dexes, tokens, pools).
- [⏳] `B5` chain readiness panel.
- [⏳] `A.4` fork simulation unblocks (requires RPC archive node + EXECUTOR_1 test funds).
- [⏳] `A.5` paper-shadow ≥ 30 days.
- [⏳] `A.9` GO/NO-GO sign-off.

Until all gates are GREEN, the engine code may be written and unit-tested
but cannot submit a single live bundle. The `G-MEV-1` readiness verifier
enforces this at runtime.

---

## 10. References

- Design doc: `docs/superpowers/specs/2026-05-13-backrun-anticipation-engine.md` (`7df940f`)
- Doctrine: `docs/policies/mev-ethics.md` (`c381d5c`)
- Math primitive: `backend/searcher-rs/src/kelly_sizing.rs`
- Simulator: `backend/searcher-rs/src/simulator_v2/` (A.2-A.4)
- Easley, López de Prado, O'Hara (2012). "Flow Toxicity and Liquidity in a High Frequency World." *Review of Financial Studies* 25(5).
- Easley, Kiefer, O'Hara, Paperman (1996). "Liquidity, Information, and Infrequently Traded Stocks." *Journal of Finance* 51(4).
- Engle, Granger (1987). "Co-Integration and Error Correction." *Econometrica* 55(2).
- Uniswap V3 Whitepaper §6, Adams et al. (2021). Tick math + concentrated liquidity formulas.
- Flashbots Collective Forum, "Backrun-only OFA alpha decomposition" (2025-Q3 research thread).

---

*Spec drafted 2026-05-13. Awaiting operator + on-call sign-off + 7-day cooldown before coding starts. No code path activated by this document.*
