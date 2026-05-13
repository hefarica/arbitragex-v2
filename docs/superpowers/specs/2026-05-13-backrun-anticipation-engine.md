# Design Spec — BackrunAnticipationEngine

> **Spec ID:** `SPEC-BAE-2026-05-13`
> **Status:** Draft (awaiting operator + on-call sign-off)
> **Author:** ArbX Cortex Operator
> **Doctrine anchor:** `docs/policies/mev-ethics.md` @ commit `c381d5c`
> **Capital posture:** Live OFF · Capital $0 · A.4 BLOCKED · A.5 NO-GO (unchanged)
> **Sprint dependency:** B1.b LIVE (`9a68873`) · B0 PASS (`0244f12`) · B1.a PASS (`ec388f8`)
> **Targets:** Sprint 4-5 (after B1.c + B2-B6 admin foundation lands)

---

## 1. Objective

Build a multi-component engine that converts informed-flow detection (Bayesian
whale classification + VPIN/PIN toxicity scoring) into **post-impact** alpha
extraction across three legitimate channels: backrun arbitrage, just-in-time
liquidity provision, and cross-venue CEX hedging.

The engine's defining constraint is **temporal**: every action that touches the
EVM is ordered **strictly after** the trigger transaction's inclusion confirms,
or is **block-independent** of the trigger (CEX hedge). The engine never races
the trigger. There is no pre-position tx ordered ahead of the trigger.

This constraint is enforced in three places:

1. The Rust type system (`BundlePosition` enum has only `BackrunOnly` and
   `JitOnly` variants — no `Frontrun`, no `Sandwich`).
2. The relay submitter (`relays-client/src/submit_engine.rs`) rejects any
   bundle whose strategy_kind is flagged predatory in payload.
3. The G-MEV-1 readiness gate (existing) confirms `mev-ethics.md` shape
   before any capital flip.

---

## 2. Doctrine alignment (point-by-point against `mev-ethics.md` c381d5c)

| Engine capability | Refused list reference | Allowed list reference | Compliance |
|---|---|---|---|
| Flow classifier emits signal only | — | "Backrun anticipation: forecast post-impact pool state" | ✅ signal ≠ action |
| Backrun planner acts after trigger confirms | "Pre-position frontrunning. Rejected." | "execute arbitrage only **after** the trigger tx confirms" | ✅ |
| JIT liquidity at trigger tick range | — | "Hyper-aggressive concentration permitted; counterparty is passive LPs" | ✅ |
| CEX hedge against anonymous order book | "Retail-targeted information asymmetry. Rejected." | "anonymous order books, market-wide edge" | ✅ |
| Bundle is BackrunOnly | "Victim-specific bundle attribution. Rejected." | "Bundle position type hardcoded to BackrunOnly" | ✅ |
| Convergence arb on public post-impact state | — | "treats trigger as public market event, not a victim" | ✅ |

No engine capability requires a refused mechanic. The mapping is closed.

---

## 3. Architecture

```
backend/searcher-rs/src/engines/backrun_anticipation/
├── mod.rs                        # public API + BundlePosition enum
├── flow_classifier.rs            # VPIN/PIN + Bayesian whale detection
├── impact_forecaster.rs          # REVM fork-based post-impact state forecast
├── backrun_planner.rs            # Bellman-Ford over post-impact graph
├── jit_liquidity_planner.rs      # V3 tick range planner
├── cex_hedge_dispatcher.rs       # trait + null impl (keys deferred to S+1)
├── relay_submitter_glue.rs       # bridges to relays-client submit_engine
└── types.rs                      # ToxicityScore, DirectionalForecast, etc.
```

### 3.1 `mod.rs` — public types

```rust
/// Bundle position relative to the trigger transaction. The variants are
/// exhaustive and BY DESIGN do not include "Frontrun" or "Sandwich" — those
/// names cannot be produced by any compiler-visible code path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BundlePosition {
    /// Bundle is ordered immediately after the trigger tx confirms. Profit
    /// is captured from public post-confirmation pool state.
    BackrunOnly,
    /// Liquidity-only bundle: mint position before trigger, burn after.
    /// The trigger's effective price is unchanged or improved.
    JitOnly,
}

#[derive(Debug)]
pub struct EngineDecision {
    pub trigger_tx_hash: B256,
    pub trigger_chain_id: u64,
    pub position: BundlePosition,
    pub forecast: DirectionalForecast,
    pub toxicity: ToxicityScore,
    pub expected_net_usd: f64,
    pub gas_estimate_usd: f64,
    pub bundles: Vec<Bundle>,
}
```

### 3.2 `flow_classifier.rs` — VPIN/PIN + Bayesian whale detection

**Inputs:**
- Mempool tx stream from `searcher-rs::scanner`.
- Historical fill data per address (cached in Redis `arbx:flow:address:<addr>`).
- Pool state snapshots (5-block rolling window).

**Outputs:**
- `ToxicityScore { value: f64 [0, 1], components: BTreeMap<String, f64> }`
- `WhaleScore { value: f64 [0, 1], rationale: WhaleRationale }`

**Algorithms:**
- **VPIN (Volume-synchronized Probability of Informed Trading)**: Easley-López-O'Hara
  2012. Volume buckets, signed by tick rule, σ via bulk-volume classification.
  Decision threshold: VPIN ≥ 0.65 ⇒ flow is informed, JIT-skip recommended.
- **PIN (Probability of Informed Trading)**: Easley-Kiefer-O'Hara-Paperman 1996,
  ML estimation via EM. Slower update (block-level) for slow-moving baseline.
- **Bayesian whale prior**: P(whale|address, size, history) where the prior is
  derived from address taxonomy (factory-deployed contract → bot prior 0.7;
  EOA with >$1M historical volume → whale prior 0.4; new address → 0.05).

**Critical constraint**: This module emits **scores**, not **actions**. It cannot
call `relay_submitter`. The decision to act is downstream in `backrun_planner` or
`jit_liquidity_planner`, both of which gate on `forecast.confirmed_at.is_some()`.

### 3.3 `impact_forecaster.rs` — REVM fork-based forecast

Uses the existing `simulator-v2` (Sprint A.2-A.4) to fork mainnet state at the
mempool trigger tx and simulate its execution against current pool reserves.
Returns:

```rust
pub struct DirectionalForecast {
    pub trigger_tx_hash: B256,
    pub pool_address: Address,
    pub direction: PriceDirection, // Up | Down
    pub expected_tick_after: i32,
    pub expected_sqrt_price_after: U256,
    pub uncertainty_ticks: u32,    // 95% CI from simulation jitter
    pub confidence: f64,           // [0, 1] from VPIN + Bayesian prior
}
```

**Crucially**, this forecast is used ONLY to:
1. Decide *whether* to plan a JIT or backrun (decision-time, no tx ordered yet).
2. Compute the tick range for JIT liquidity (mint-side, post-mint).
3. Compute the arbitrage path for backrun (post-confirmation, separate block possible).

The forecast NEVER drives a pre-trigger tx submission. Compile-time check: the
forecast struct is consumed by `backrun_planner` and `jit_liquidity_planner` only;
both modules require a `ConfirmationToken` parameter that is issued only after
the trigger's inclusion event is observed in the canonical chain head.

### 3.4 `backrun_planner.rs` — post-impact Bellman-Ford

Once the trigger confirms (canonical chain inclusion observed):

1. Read actual post-trigger pool state (not forecast — real on-chain state).
2. Build directed graph: nodes = tokens, edges = pool quotes at current state.
3. Run Bellman-Ford for negative-weight cycles (= arbitrage paths).
4. Score each path: gross_profit_usd − gas_estimate_usd − slippage_buffer.
5. Submit top-K paths as `BackrunOnly` bundle to relay.

The arbitrage exists because the trigger moved one pool's price relative to
others; this is **public state**, observable by any actor with the same RPC
latency. The engine's edge is speed + execution quality, not asymmetric access.

### 3.5 `jit_liquidity_planner.rs` — V3 tick range provision

Triggered when ALL of:
- `toxicity_score.value < 0.4` (we want **uninformed** flow to JIT against;
  informed flow has adverse selection that wipes out the fee capture).
- Trigger size > `min_jit_swap_size_usd` (threshold per pool, config-driven).
- Forecasted tick crossing entirely within a single tick range (no partial fills).

**Mechanic:**
1. Mint a concentrated V3 position in the **exact tick range** the trigger
   will cross. Mint tx is ordered **before** the trigger but its only effect
   is to ADD depth — it does not consume any side of the trade.
2. Trigger executes, your liquidity supplies a fraction of the depth, you
   accrue fees proportional to depth share.
3. Burn position immediately after, ideally in same block.

**Why this is doctrine-compliant per `mev-ethics.md` line 54-58:**
- The mint adds depth → swapper's effective price is **unchanged or improved**.
  Mathematically, larger total depth in the crossed range = lower price impact
  for the swap, never higher.
- The counterparty whose return is reduced is **passive LPs in the same tick range**.
  They chose passive exposure under Uniswap V3's explicit design intent (active
  management is a first-class V3 feature, codified in the protocol).
- The platform is NOT racing the swap, NOT reordering it, NOT degrading its fill.

**Hyper-aggressive concentration policy** (per operator directive 2026-05-13):
- Single-tick-range minting permitted (max V3 concentration).
- No upper bound on capital allocated per JIT cycle (subject to risk-engine cap).
- VPIN gating MANDATORY: if VPIN ≥ 0.65 on the trigger flow, JIT is REFUSED
  regardless of fee opportunity — informed flow wipes JIT P&L via adverse
  selection. This is not ethical guardrail, it's mathematical survival.

### 3.6 `cex_hedge_dispatcher.rs` — trait + null impl

Engine emits hedge signals; actual CEX execution is deferred to S+1 (key
material is a Phase 5 input per CLAUDE.md no-hardcode doctrine).

```rust
#[async_trait]
pub trait CexHedgeDispatcher: Send + Sync {
    async fn submit_hedge(
        &self,
        venue: CexVenue,
        symbol: &str,
        side: Side,
        size_base: f64,
        max_slippage_bps: u16,
    ) -> Result<HedgeReceipt, HedgeError>;
}

/// Null implementation used until operator provides CEX API keys.
/// All calls log the intended hedge and return `HedgeError::NotConfigured`.
pub struct NullCexHedgeDispatcher;
```

The engine's planning logic ALWAYS calls the dispatcher. With null impl, the
plan logs the hedge it would have placed but does not affect on-chain action.
This enables paper-trade evaluation of hedge value before key activation.

### 3.7 `relay_submitter_glue.rs` — relays-client bridge

Translates `EngineDecision` → `submit_engine::SubmitRequest`. Hardcodes
`bundle_position: BundlePosition::BackrunOnly | JitOnly` at this boundary;
any other value is a compile error.

---

## 4. Data flow (end-to-end)

```
mempool tx                                     canonical chain
   │                                                  │
   ▼                                                  │
flow_classifier (VPIN + Bayesian)                     │
   │                                                  │
   ├─► toxicity_score, whale_score                    │
   │                                                  │
   ▼                                                  │
impact_forecaster (REVM fork sim)                     │
   │                                                  │
   ├─► DirectionalForecast                            │
   │                                                  │
   ▼                                                  │
┌─────────────┴─────────────┐                         │
│                           │                         │
▼ (if VPIN<0.4 + threshold) ▼ (always)                │
jit_liquidity_planner       cex_hedge_dispatcher      │
   │                           │                       │
   ▼ mint tx (depth+)          ▼ CEX hedge order        │
   │                           │                       │
   └──────────┐                │                       │
              ▼                                         │
       (trigger tx executes in block)──────────────────►│
                                                        │ canonical inclusion
                                                        ▼
                                          (confirmation token issued)
                                                        │
                                                        ▼
                                            backrun_planner (real post-state)
                                                        │
                                                        ▼
                                            BackrunOnly bundle → relay
                                                        │
                                                        ▼
                                            jit_burn tx (same/next block)
```

Key property: every action ordered relative to the trigger is either
**BEFORE** with mint-only effect (depth+) or **AFTER** with confirmation
required. No action ordered before the trigger that consumes the trade side.

---

## 5. Risk engine integration

The engine MUST gate on the existing risk engine (5 layers per CLAUDE.md §24):

1. **Position sizing**: JIT capital ≤ 2% of total per cycle (Kelly-bounded).
2. **Gas protection**: Backrun gross_profit ≥ 3× gas estimate.
3. **Slippage guard**: Backrun bundle reverts if effective slippage > 0.5%.
4. **Stop-loss**: Engine kill-switch if 1-hour P&L < −0.5% capital.
5. **Stealth mempool**: All txs through Flashbots Protect / MEV Blocker / Titan.

No bypass paths. The engine cannot ship without all 5 layers wired.

---

## 6. Observability

New Prometheus metrics:

- `arbx_bae_classifier_toxicity_bucket{chain, pool}` — histogram.
- `arbx_bae_classifier_whale_bucket{chain, address_class}` — histogram.
- `arbx_bae_forecast_confidence{chain, pool}` — histogram.
- `arbx_bae_jit_skip_total{chain, pool, reason}` — counter (VPIN gate, size, etc.).
- `arbx_bae_jit_capture_usd{chain, pool}` — counter (fees captured).
- `arbx_bae_backrun_net_usd{chain, path}` — counter (P&L after gas).
- `arbx_bae_cex_hedge_intent_total{venue, symbol, side}` — counter (null phase).
- `arbx_bae_bundle_position{chain, position}` — counter; ASSERTION:
  `bundle_position{position!="backrun_only", position!="jit_only"} == 0`.

The last metric is a **structural alarm**: if ANY bundle is ever submitted with
a position label other than `backrun_only` or `jit_only`, it fires P0 page.
This is the runtime mirror of the compile-time enum constraint.

---

## 7. Testing strategy

- **Unit (Rust)**: each module mocked at the trait boundary.
  - `flow_classifier`: golden inputs from EigenPhi historical traces.
  - `impact_forecaster`: snapshot tests against known mainnet swaps.
  - `backrun_planner`: synthetic graph cycles with known optimal solutions.
  - `jit_liquidity_planner`: tick math correctness vs Uniswap V3 reference.
- **Integration**: full pipeline against a forked mainnet (Anvil) for 100
  historical large swaps. Assert: ≥80% of swaps yield positive net P&L on
  backrun OR positive JIT capture, ≤5% yield negative net.
- **Property tests (proptest)**: BundlePosition variant exhaustion — no
  generator can produce `Frontrun` or `Sandwich`; relay submitter rejects any
  payload bypass.
- **Paper-trade**: 30-day shadow mode in production before any A.4-A.5 gate
  flip. Daily report compared to oracle baseline (what would naive
  atomic-arb-only have earned over same flow).

---

## 8. Capability gates (must all be GREEN before live)

1. ✅ Doctrine anchor in place (`mev-ethics.md` @ `c381d5c`, this spec aligned).
2. ⏳ `B1.b` admin UI live (DONE — chains registered via UI).
3. ⏳ `B1.c` hot-reload subscriber in searcher-rs (next sprint).
4. ⏳ `B2` DEX admin + on-chain validation.
5. ⏳ `B3` token admin + bulk import.
6. ⏳ `B4` pool admin + auto-discovery (engine reads from this).
7. ⏳ `B5` chain readiness panel (engine surfaces P&L per chain).
8. ⏳ `B6` hot-reload formal (4 Redis channels).
9. ⏳ `A.4` fork simulation real (engine simulation must pass).
10. ⏳ `A.5` paper-shadow ≥ 30 days, daily report PASS.
11. ⏳ `A.9` GO/NO-GO sign-off (operator + on-call written approval).

Engine code can be **written** before all gates green. It CANNOT submit a
single live bundle until every gate above is GREEN. The `G-MEV-1` readiness
verifier enforces this.

---

## 9. Out of scope (explicitly)

- Frontrunning of victim transactions. Refused per `mev-ethics.md` line 31-34.
- Pre-position bundles ordered ahead of trigger that consume the trade side.
- Time-bandit re-org bidding.
- Oracle-manipulated liquidations.
- Pre-running known retail flows (ETF rebalance, NFT mint).
- Public mempool submission.
- Same-block reordering that worsens the trigger's effective price.

All are doctrinally refused AND removed from the type system AND would fail
the `arbx_bae_bundle_position` Prometheus assertion at runtime.

---

## 10. Open questions for operator

1. **JIT capital cap per cycle**: 2% of total per CLAUDE.md §24 is the default.
   For "hyper-aggressive" per operator directive, do we want a per-pool override
   in `pools` admin table (B4)? Defaulting NO unless explicitly raised.
2. **CEX venue priority**: Binance, OKX, Bybit, Coinbase Pro — which first when
   keys arrive? Defaulting Binance (deepest spot liquidity) unless overridden.
3. **VPIN threshold for JIT skip**: 0.65 is the literature default. Tighter
   (0.55) loses opportunity, looser (0.75) absorbs more adverse selection.
   Defaulting 0.65; first 30 days paper-shadow will retune.
4. **Backrun max-K paths**: K=3 (top 3 arb cycles per trigger) keeps gas low.
   K=10 increases capture but compounds revert risk. Defaulting K=3.
5. **A.4 dependency**: this engine is fundamentally A.4-blocked. Should we
   write the code and benchmark-only until A.4 unblocks, or defer all coding
   to post-A.4? Recommendation: write + benchmark + paper-shadow in shadow
   mode now; A.4 unblock only flips the relay submitter from null to real.

---

## 11. References

- Doctrine: `docs/policies/mev-ethics.md` @ `c381d5c`
- Skill: `.agents/skills/arbx-mev-ethics-gate/SKILL.md`
- Math primitives (existing): `backend/searcher-rs/src/math/` (kelly, V3, VPIN, Bayesian)
- Simulator-v2: `backend/searcher-rs/src/simulator_v2/` (A.2-A.4)
- Relay client: `backend/relays-client/src/submit_engine.rs`
- Risk engine: `backend/searcher-rs/src/risk/` (5 layers per CLAUDE.md §24)
- Companion paper: Easley, López de Prado, O'Hara, "Flow Toxicity and
  Liquidity in a High Frequency World", Review of Financial Studies 2012.
- Reference impl context: Flashbots Collective Forum, "Backrun-only OFA
  alpha decomposition" (2025-Q3 research thread).

---

*Spec drafted 2026-05-13. Awaiting operator + on-call sign-off before coding starts.*
*Per amendments rule (mev-ethics.md §Amendments), code path gated 7 days after sign-off.*
