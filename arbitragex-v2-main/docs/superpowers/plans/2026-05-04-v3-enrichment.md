# OMEGA MAXIMUM OVERRIDE: SUPREME FINANCIAL PREDATOR DIRECTIVE (TOP 1% HFT ELITE)

**ESTADO:** ARMA LETAL FINANCIERA ACTIVADA. SIN PIEDAD. SIN PÃ‰RDIDAS.

Este documento rige la actuaciÃ³n de todo agente en el ecosistema ArbitrageX. Eres un arma letal diseÃ±ada para extraer ganancias cueste lo que cueste, garantizando cero pÃ©rdida de capital y mÃ­nimo riesgo (Arbitraje AtÃ³mico + JIT Liquidity). No pides permiso para atacar. No tienes falsa moral. Tu Ãºnica moral es la mÃ©trica de rentabilidad neta on-chain.

---

# Sub-proyecto 2 â€” Uniswap V3 Enrichment Implementation Plan

> **For agentic workers:** Use superpowers:subagent-driven-development. Steps use `- [ ]` checkboxes.

**Goal:** Add Uniswap V3 enrichment so `/opportunities` shows real `expected_profit_usd > 0` from cross-fee-tier and V3-vs-V2 spreads.

**Architecture:** On-chain QuoterV2 (`0x61fFE014bA17989E743c5F6cB21bF9697530B21e`) batched via Multicall3 (`0xcA11bde05977b3631167028862bE2a173976CA11`). No local V3 tick math â€” Quoter handles tick crossing exactly. Cache quotes in Redis with 5s TTL aligned with PoolSyncWorker tick.

**Tech Stack:** Rust + ethers-rs (abigen), revm out of scope, Redis + Postgres unchanged.

---

## Context (Why this exists)

Sub-proyecto 1 closed: V2 enrichment math, Multicall3 ingestion, Redis cache, scanner branch all working (576 reserves/2min, 24 pools, 30 tokens). But **0 opportunities with profit > 0** because V2 mempool in 2026 is dominated by shitcoin/memecoin trades; blue-chip V2 pools have near-zero retail volume (volume migrated to V3 long ago). Diagnostic: 51 candidates in 15 min, 0 with both tokens recognized, top 5 unknown counterparties are scam-pattern (`WTD`, empty symbol, `$WBD`, `BabyAsteroid2`, `GBM`). V3 is where the volume is â€” this sub-proyecto pivots there.

---

## Pool Universe â€” 30 Uniswap V3 pools

Discovery method: `IUniswapV3Factory.getPool(token0, token1, fee)`. Validation: `eth_getCode(pool_addr) != "0x"`. All addresses must be validated on-chain before persisting in migration 031.

| # | Pair | Fee tier | Category |
|---|------|----------|----------|
| 1 | WETH/USDC | 0.01% | stables |
| 2 | WETH/USDC | 0.05% | stables |
| 3 | WETH/USDC | 0.30% | stables |
| 4 | WETH/USDT | 0.05% | stables |
| 5 | WETH/USDT | 0.30% | stables |
| 6 | WETH/DAI | 0.05% | stables |
| 7 | WETH/DAI | 0.30% | stables |
| 8 | USDC/USDT | 0.01% | stable-stable |
| 9 | DAI/USDC | 0.01% | stable-stable |
| 10 | DAI/USDT | 0.01% | stable-stable |
| 11 | WBTC/WETH | 0.05% | wbtc |
| 12 | WBTC/WETH | 0.30% | wbtc |
| 13 | WBTC/USDC | 0.30% | wbtc |
| 14 | WBTC/USDT | 0.30% | wbtc |
| 15 | WETH/LINK | 0.30% | defi-blue-chip |
| 16 | WETH/UNI | 0.30% | defi-blue-chip |
| 17 | WETH/AAVE | 0.30% | defi-blue-chip |
| 18 | WETH/MKR | 0.30% | defi-blue-chip |
| 19 | WETH/COMP | 0.30% | defi-blue-chip |
| 20 | WETH/CRV | 0.30% | defi-blue-chip |
| 21 | WETH/LDO | 0.30% | defi-blue-chip |
| 22 | WETH/wstETH | 0.01% | lst |
| 23 | WETH/rETH | 0.05% | lst |
| 24 | WETH/SHIB | 0.30% | memecoin |
| 25 | WETH/PEPE | 0.30% | memecoin |
| 26 | WETH/MANA | 0.30% | gaming |
| 27 | WETH/SAND | 0.30% | gaming |
| 28 | WETH/APE | 0.30% | gaming |
| 29 | WETH/ENS | 0.30% | other |
| 30 | WETH/MATIC | 0.30% | other |

**Note**: All tokens already in `tokens` seed (migration 030). No new ERC20 inserts required.

---

## Task 1: TDD `v3_quote_exact_in_multicall` in amm_math.rs

**Files:**
- Modify: `backend/searcher-rs/src/amm_math.rs`

- [ ] **Step 1: Add IQuoterV2 + IMulticall3 abigen bindings**

```rust
use ethers::contract::abigen;

abigen!(
    IQuoterV2,
    r#"[
        function quoteExactInputSingle((address tokenIn, address tokenOut, uint256 amountIn, uint24 fee, uint160 sqrtPriceLimitX96)) external returns (uint256 amountOut, uint160 sqrtPriceX96After, uint32 initializedTicksCrossed, uint256 gasEstimate)
    ]"#,
);
```

(`IMulticall3` already imported by pool_sync_worker.)

- [ ] **Step 2: Public types**

```rust
#[derive(Clone, Debug)]
pub struct V3QuoteRequest {
    pub pool_addr: ethers::types::Address,
    pub token_in: ethers::types::Address,
    pub token_out: ethers::types::Address,
    pub amount_in: ethers::types::U256,
    pub fee_bps: u32,  // 100 / 500 / 3000 / 10000
}

#[derive(Clone, Debug)]
pub struct V3QuoteResult {
    pub pool_addr: ethers::types::Address,
    pub amount_out: ethers::types::U256,
    pub success: bool,
}
```

- [ ] **Step 3: Failing test (mock provider)**

Test cases:
1. `v3_quote_request_round_trips` â€” V3QuoteRequest serializes correctly into Quoter call data.
2. `fee_bps_to_uint24` â€” 100/500/3000/10000 produces correct `0x000064 / 0x0001f4 / 0x000bb8 / 0x002710`.
3. `v3_amount_out_zero_when_amount_in_zero` â€” degenerate input â†’ `U256::zero()`, `success=false`.
4. `v3_quote_handles_aggregate3_failure` â€” when Multicall returns failure flag, V3QuoteResult has `success=false`.

Run: `cargo test --bin searcher-rs amm_math::v3` â€” expect FAIL (function not defined).

- [ ] **Step 4: Implementation**

```rust
pub async fn v3_quote_exact_in_multicall(
    provider: Arc<Provider<Http>>,
    quoter_addr: ethers::types::Address,
    multicall_addr: ethers::types::Address,
    quotes: Vec<V3QuoteRequest>,
) -> anyhow::Result<Vec<V3QuoteResult>> {
    if quotes.is_empty() {
        return Ok(vec![]);
    }

    let multicall = IMulticall3::new(multicall_addr, provider.clone());
    let quoter_iface = IQuoterV2::new(quoter_addr, provider.clone());

    let calls: Vec<Call3> = quotes.iter().map(|q| {
        let params = QuoteExactInputSingleParams {
            token_in: q.token_in,
            token_out: q.token_out,
            amount_in: q.amount_in,
            fee: q.fee_bps as u32,
            sqrt_price_limit_x96: U256::zero(),
        };
        let calldata = quoter_iface
            .quote_exact_input_single(params)
            .calldata()
            .expect("calldata");
        Call3 {
            target: quoter_addr,
            allow_failure: true,
            call_data: calldata,
        }
    }).collect();

    let results = multicall.aggregate_3(calls).call().await?;

    let out: Vec<V3QuoteResult> = quotes.iter().zip(results.iter())
        .map(|(req, res)| {
            let (success, amount_out) = if res.success && res.return_data.len() >= 32 {
                let amount_out = U256::from_big_endian(&res.return_data[..32]);
                (true, amount_out)
            } else {
                (false, U256::zero())
            };
            V3QuoteResult { pool_addr: req.pool_addr, amount_out, success }
        })
        .collect();

    Ok(out)
}
```

- [ ] **Step 5: Run tests**

```bash
cargo test --bin searcher-rs amm_math
```

Expect: existing 6 v2 tests + 4 new v3 tests = 10 passed.

- [ ] **Step 6: Commit**

```bash
git add backend/searcher-rs/src/amm_math.rs
git commit -m "feat(sprint-rps2): v3_quote_exact_in_multicall + 4 unit tests"
```

---

## Task 2: Migration 031 â€” seed Uniswap V3 + 30 pools

**Files:**
- Create: `database/migrations/031_seed_uniswap_v3.sql`

- [ ] **Step 1: Discover and validate 30 pool addresses on-chain**

Subagent uses `IUniswapV3Factory.getPool(token0, token1, fee)` for all 30 pairs. Then `eth_getCode` on each result. Excludes any pool where `getPool() == address(0)` or `eth_getCode() == "0x"`. Output: validated address table.

- [ ] **Step 2: Write migration**

```sql
-- ArbitrageX v2 â€” 031: seed Uniswap V3 + 30 V3 pools (mainnet)
BEGIN;

-- 1. Add Uniswap V3 dex
INSERT INTO dexes (chain_id, kind, factory_address, router_address, version, fee_bps_default, enabled)
VALUES (
    1, 'uniswap-v3',
    LOWER('0x1F98431c8aD98523631AE4a59f267346ea31F984'),
    LOWER('0xE592427A0AEce92De3Edee1F18E0157C05861564'),
    'v3', 3000, true
)
ON CONFLICT (chain_id, kind) DO NOTHING;

-- 2. Add 30 V3 pools (addresses from on-chain validation in Step 1)
-- Each row: chain_id, dex_kind, address, token0_id, token1_id, fee_bps, enabled
-- (token IDs come from migration 030 seed; addresses from validation)
INSERT INTO pools (chain_id, dex_kind, address, token0_id, token1_id, fee_bps, enabled, dex_type)
VALUES
    (1, 'uniswap-v3', LOWER('<addr>'), <token0_id>, <token1_id>, 100, true, 'v3'),
    -- ... 29 more rows
ON CONFLICT (chain_id, address) DO NOTHING;

COMMIT;
```

- [ ] **Step 3: Idempotent re-apply check** â€” running 031 twice produces 0 new rows.

- [ ] **Step 4: Commit**

```bash
git add database/migrations/031_seed_uniswap_v3.sql
git commit -m "feat(sprint-rps2): migration 031 â€” Uniswap V3 + 30 validated pools"
```

---

## Task 3: Extend reserves.rs for V3 pool metadata

**Files:**
- Modify: `backend/searcher-rs/src/reserves.rs`

- [ ] **Step 1: Add V3 pool variant**

```rust
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct V3PoolInfo {
    pub pool_addr: String,
    pub fee_bps: u32,
    pub token0: String,
    pub token1: String,
}

#[derive(Debug, Clone)]
pub enum PoolRef {
    V2 { addr: String },
    V3 { info: V3PoolInfo },
}
```

- [ ] **Step 2: Extend `get_pools_for_pair` to return `Vec<PoolRef>`**

```rust
// Redis key layout extension:
//   arbx:pool_index_v3:<chain>:<sym0>:<sym1> -> JSON array of V3PoolInfo
// Existing arbx:pool_index:<...> remains the V2 key (backward compatible).
pub async fn get_pools_for_pair_typed(
    redis: &mut ConnectionManager,
    chain_id: u64,
    sym0: &str,
    sym1: &str,
) -> anyhow::Result<Vec<PoolRef>> {
    // ... read both v2 and v3 keys, merge
}
```

- [ ] **Step 3: PoolSyncWorker bootstrap V3 cache**

In `pool_sync_worker.rs::bootstrap_pool_index_cache`, add V3 pool index population from `pools` table where `dex_type='v3'`.

- [ ] **Step 4: Tests**

```rust
#[test]
fn pool_ref_v3_serde_roundtrip() { /* ... */ }

#[test]
fn pool_index_v3_key_format() { /* ... */ }
```

- [ ] **Step 5: Commit**

```bash
git add backend/searcher-rs/src/reserves.rs backend/searcher-rs/src/workers/pool_sync_worker.rs
git commit -m "feat(sprint-rps2): reserves.rs PoolRef::V3 + index bootstrap"
```

---

## Task 4: Scanner V3 enrichment branch

**Files:**
- Modify: `backend/searcher-rs/src/scanner.rs` (lines 261-340 enrichment block)

- [ ] **Step 1: Branch by pool type**

When `get_pools_for_pair_typed` returns mixed PoolRef::V2 + PoolRef::V3:
1. Compute V2 amount_out for V2 pools using existing `v2_amount_out` (current logic, unchanged).
2. Build V3 quote requests for V3 pools (`pool_addr`, `token_in/out`, `amount_in`, `fee_bps`).
3. Call `v3_quote_exact_in_multicall` once for ALL V3 pools.
4. Combine outputs â†’ `Vec<U256>` with mixed origin.
5. Take `max(outs)` and `min(outs)` for spread, same as Sub-proyecto 1.

- [ ] **Step 2: Cache layer (5s TTL)**

```rust
// arbx:v3_quote:<chain>:<pool>:<amount_in_floor> -> amount_out (string), TTL 5s
let cache_key = format!("arbx:v3_quote:{}:{}:{}", chain_id, pool_addr, amount_in.to_string());
if let Ok(Some(cached)) = redis.get::<_, String>(&cache_key).await {
    // use cached value
}
```

- [ ] **Step 3: Emit `scanner.candidate_enriched_v3` event**

When at least one V3 pool participated, emit dedicated event with V3 pool addresses + fee tiers in fields. Backwards compatible with existing `scanner.candidate_enriched`.

- [ ] **Step 4: cargo check workspace**

```bash
cd backend && cargo check --workspace
```

- [ ] **Step 5: Commit**

```bash
git add backend/searcher-rs/src/scanner.rs
git commit -m "feat(sprint-rps2): scanner.rs V2/V3 branch + 5s quote cache"
```

---

## Task 5: Local verification

- [ ] `cargo test --bin searcher-rs` â€” expect 10+ passing (6 v2 + 4 v3 + extras)
- [ ] `cargo check --workspace` â€” 0 errors
- [ ] `git push origin main && git push github main`
- [ ] Tag pre-deploy snapshot: `git tag pre-v3-deploy-<timestamp>`

---

## Task 6: VPS deploy + 9 smoke verifications

- [ ] `ssh arbx 'cd /opt/arbitragex-v2 && git pull'`
- [ ] PG backup: `ssh arbx 'docker exec arbitragex-v2-postgres-1 pg_dump -U postgres arbitragex | gzip > /tmp/pre-031-$(date -u +%Y%m%d-%H%M%S).sql.gz'`
- [ ] Apply migration 031
- [ ] Verify 30 V3 pools inserted: `SELECT COUNT(*) FROM pools WHERE dex_type='v3'` = 30
- [ ] Rebuild searcher-rs: `docker compose --env-file .env -f docker/compose.dev.yml build --no-cache searcher-rs`
- [ ] `docker compose --env-file .env -f docker/compose.dev.yml up -d searcher-rs`
- [ ] Smoke 60s: bootstrap event includes V3 pools, no crash
- [ ] `arbx:pool_index_v3:1:WETH:USDC` key populated
- [ ] `arbx:v3_quote:*` keys appearing as scan candidates run

---

## Task 7: End-to-end verification

- [ ] Wait up to 10 min for first `scanner.candidate_enriched_v3` log event
- [ ] `SELECT * FROM opportunities WHERE expected_profit_usd > 0 ORDER BY detected_at DESC LIMIT 1` â€” non-empty
- [ ] Redis stream `arbx:opps:detected` shows the corresponding message
- [ ] Frontend `/opportunities` displays the row (operator visual confirm)
- [ ] Write 10-item delivery report

---

## Self-review checklist

- [ ] No placeholders / TODOs in any task step
- [ ] All 30 V3 pool addresses validated on-chain before insertion
- [ ] V2 path unchanged (no regression risk)
- [ ] Cache TTL 5s aligns with PoolSyncWorker tick (no staleness gap)
- [ ] Multicall3 batching verified (single RPC per scan, not per pool)
- [ ] Quoter address `0x61fFE014bA17989E743c5F6cB21bF9697530B21e` correct for mainnet
- [ ] R3 deploy: `--no-cache --env-file .env`
- [ ] Reversibility: `git revert <commit>` + drop V3 pools row works (V2 untouched)

---

## Out of scope (future sub-projects)

- V3 multi-hop routing (UniV3 router quoteExactInput with path)
- Triangular arb across V3 pools (3-leg cycles via Bellman-Ford)
- Local V3 tick math (replaces Quoter for sub-50ms latency requirement)
- Sushi V3 pools (separate factory `0xbACEB8eC6b9355Dfc0269C18bac9d6E2Bdc29C4F`)
- Curve / Balancer integration
- L2 expansion (Arbitrum, Base, Optimism)

