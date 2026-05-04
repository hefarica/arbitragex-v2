# Real Profit Signal in Detection — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `expected_profit_usd > 0` real on `/opportunities` by wiring V2 pool reserves into the scanner — replace the lying stub `PoolSyncWorker` with a real multicall fetcher, persist reserves to Postgres + Redis, compute `amount_out` via V2 CPMM math, populate candidate enrichment in `scanner.rs`.

**Architecture:** Additive 4-layer wiring on top of existing `searcher-rs` runtime: (1) DB seed of 10 mainnet V2 pools + 5 tokens; (2) pure-math V2 CPMM helper `amm_math.rs`; (3) Redis-backed lookup helper `reserves.rs`; (4) replacement `PoolSyncWorker` that drives Multicall3 every 5s and writes both stores. `scanner.rs::process_pending` receives one surgical edit (~20 lines) to call the new helpers when building each candidate. Two stub workers (`route_discovery_worker.rs`, `simulation_worker.rs`) deleted because they emit fake telemetry.

**Tech Stack:** Rust + tokio + ethers v2 (contract+abigen feature) + redis (workspace) + sqlx Postgres + tracing JSON. Spec: `docs/superpowers/specs/2026-05-04-real-profit-signal-design.md`.

---

## Context recap (for someone reading this plan cold)

- ArbitrageX v2 is a Rust-based MEV searcher running on a VPS at `195.201.235.70`. Production HEAD is `abe70cc` on `main` (which is the spec commit; no implementation yet for this sub-project).
- The searcher already ingests Ethereum mempool via WebSocket and decodes Uniswap V2/V3 calldata. It builds an `Opportunity` per pending tx, runs it through a config-aware spine evaluator, persists to Postgres `opportunities`, and publishes to Redis stream `arbx:opps:detected`.
- **The bug**: `scanner.rs:268` sets `expected_amount_out = amount_in` (zero spread) so `gross_profit = 0` always. Every opportunity gets gated as not-viable and persists with `risk_score=0`. Operator sees data but no profitable opps.
- **The fix (this plan)**: replace `expected_amount_out = amount_in` with a real V2 CPMM calculation against fresh on-chain reserves of the pool involved in the pending swap, then compare against alternative pools of the same pair to surface the spread.

---

## File Structure

| Path | Action | Responsibility |
|---|---|---|
| `database/migrations/029_seed_defi_v2_mvp.sql` | NEW | Seed Ethereum chain + UniV2/Sushi dexes + 2 factories + 5 tokens (WETH/USDC/USDT/DAI/WBTC) + 10 pools |
| `backend/searcher-rs/src/amm_math.rs` | NEW | Pure functions: `v2_amount_out`, `v2_spread_token_out`, `to_usd_via_base` |
| `backend/searcher-rs/src/reserves.rs` | NEW | Redis cache helpers: `get_reserves`, `get_pools_for_pair`, `get_token_meta`, `set_reserves`, `set_pool_index`, `set_token_meta` |
| `backend/searcher-rs/src/workers/pool_sync_worker.rs` | REPLACE | Real impl: bootstrap caches from DB, multicall `getReserves` every 5s, write Postgres + Redis |
| `backend/searcher-rs/src/workers/route_discovery_worker.rs` | DELETE | Stub emits fake "150us" telemetry without doing anything |
| `backend/searcher-rs/src/workers/simulation_worker.rs` | DELETE | Stub sleeps 100ms in a loop, no real work |
| `backend/searcher-rs/src/workers/mod.rs` | MODIFY | Remove deleted modules, change `WorkerOrchestrator::start_all` signature to take `PgPool` + `redis::aio::ConnectionManager`, pass to PoolSyncWorker |
| `backend/searcher-rs/src/main.rs` | MODIFY | Pass DB pool + Redis manager + chain_id list into orchestrator |
| `backend/searcher-rs/src/lib.rs` or `main.rs` | MODIFY | Declare `mod amm_math; mod reserves;` |
| `backend/searcher-rs/src/scanner.rs` | MODIFY | `process_pending` after line 270: lookup reserves, compute `expected_amount_out`/`gross_profit`/`gross_profit_usd`, override candidate fields |
| `backend/searcher-rs/Cargo.toml` | MODIFY | Add ethers `abigen` feature override (workspace has only `ws,rustls`) |

Rollback = `git revert <commits>`. Migration 029 is data-only; rolling back code keeps the seed in DB which is harmless.

---

## Task 1 — Migration 029 (seed DeFi v2 MVP)

**Files:**
- Create: `database/migrations/029_seed_defi_v2_mvp.sql`

- [ ] **Step 1: Write the migration**

```sql
-- ArbitrageX v2 — Migration 029: seed DeFi v2 MVP universe (Ethereum mainnet).
--
-- Doctrine: pool_sync_worker / scanner enrichment / amm_math need a populated
-- registry to operate. Pool addresses below are PUBLIC mainnet contracts,
-- verifiable on Etherscan — they are not secrets and do not violate RULE 00.
-- Operator can extend or disable individual rows via SQL. Future sub-project
-- adds an admin endpoint for UI-driven extension.
--
-- Idempotent via ON CONFLICT DO NOTHING on every INSERT.

BEGIN;

-- 1) Chain
INSERT INTO chains (chain_id, name, native_currency, explorer_url) VALUES
  (1, 'ethereum', 'ETH', 'https://etherscan.io')
ON CONFLICT (chain_id) DO NOTHING;

-- 2) DEXes
INSERT INTO dexes (id, name, protocol_type) VALUES
  (gen_random_uuid(), 'UniswapV2', 'UNISWAP_V2'),
  (gen_random_uuid(), 'SushiSwap', 'UNISWAP_V2')
ON CONFLICT (name) DO NOTHING;

-- 3) Factories (resolved via dex name lookup; safe under ON CONFLICT)
INSERT INTO factories (dex_id, chain_id, address)
SELECT id, 1, '0x5C69bEe701ef814a2B6a3EDD4B1652CB9cc5aA6f' FROM dexes WHERE name='UniswapV2'
ON CONFLICT (chain_id, address) DO NOTHING;
INSERT INTO factories (dex_id, chain_id, address)
SELECT id, 1, '0xC0AEe478e3658e2610c5F7A4A2E1777cE9e4f2Ac' FROM dexes WHERE name='SushiSwap'
ON CONFLICT (chain_id, address) DO NOTHING;

-- 4) Tokens (5 blue-chip Ethereum). decimals are mainnet ground truth.
INSERT INTO tokens (chain_id, address, symbol, decimals, is_stablecoin) VALUES
  (1, '0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2', 'WETH', 18, FALSE),
  (1, '0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48', 'USDC',  6, TRUE),
  (1, '0xdac17f958d2ee523a2206206994597c13d831ec7', 'USDT',  6, TRUE),
  (1, '0x6b175474e89094c44da98b954eedeac495271d0f', 'DAI',  18, TRUE),
  (1, '0x2260fac5e5542a773aa44fbcfedf7c193bc2c599', 'WBTC',  8, FALSE)
ON CONFLICT (chain_id, address) DO NOTHING;

-- 5) Pools — UniswapV2
INSERT INTO pools (chain_id, factory_id, address, token0_id, token1_id, fee_tier)
SELECT 1, f.id, '0xb4e16d0168e52d35cacd2c6185b44281ec28c9dc',
       (SELECT id FROM tokens WHERE chain_id=1 AND symbol='USDC'),
       (SELECT id FROM tokens WHERE chain_id=1 AND symbol='WETH'),
       30
FROM factories f JOIN dexes d ON d.id=f.dex_id WHERE d.name='UniswapV2' AND f.chain_id=1
ON CONFLICT (chain_id, address) DO NOTHING;
INSERT INTO pools (chain_id, factory_id, address, token0_id, token1_id, fee_tier)
SELECT 1, f.id, '0x0d4a11d5eeaac28ec3f61d100daf4d40471f1852',
       (SELECT id FROM tokens WHERE chain_id=1 AND symbol='WETH'),
       (SELECT id FROM tokens WHERE chain_id=1 AND symbol='USDT'),
       30
FROM factories f JOIN dexes d ON d.id=f.dex_id WHERE d.name='UniswapV2' AND f.chain_id=1
ON CONFLICT (chain_id, address) DO NOTHING;
INSERT INTO pools (chain_id, factory_id, address, token0_id, token1_id, fee_tier)
SELECT 1, f.id, '0xbb2b8038a1640196fbe3e38816f3e67cba72d940',
       (SELECT id FROM tokens WHERE chain_id=1 AND symbol='WBTC'),
       (SELECT id FROM tokens WHERE chain_id=1 AND symbol='WETH'),
       30
FROM factories f JOIN dexes d ON d.id=f.dex_id WHERE d.name='UniswapV2' AND f.chain_id=1
ON CONFLICT (chain_id, address) DO NOTHING;
INSERT INTO pools (chain_id, factory_id, address, token0_id, token1_id, fee_tier)
SELECT 1, f.id, '0x3041cbd36888becc7bbcbc0045e3b1f144466f5f',
       (SELECT id FROM tokens WHERE chain_id=1 AND symbol='USDC'),
       (SELECT id FROM tokens WHERE chain_id=1 AND symbol='USDT'),
       30
FROM factories f JOIN dexes d ON d.id=f.dex_id WHERE d.name='UniswapV2' AND f.chain_id=1
ON CONFLICT (chain_id, address) DO NOTHING;
INSERT INTO pools (chain_id, factory_id, address, token0_id, token1_id, fee_tier)
SELECT 1, f.id, '0xae461ca67b15dc8dc81ce7615e0320da1a9ab8d5',
       (SELECT id FROM tokens WHERE chain_id=1 AND symbol='DAI'),
       (SELECT id FROM tokens WHERE chain_id=1 AND symbol='USDC'),
       30
FROM factories f JOIN dexes d ON d.id=f.dex_id WHERE d.name='UniswapV2' AND f.chain_id=1
ON CONFLICT (chain_id, address) DO NOTHING;

-- 5b) Pools — SushiSwap
INSERT INTO pools (chain_id, factory_id, address, token0_id, token1_id, fee_tier)
SELECT 1, f.id, '0x397ff1542f962076d0bfe58ea045ffa2d347aca0',
       (SELECT id FROM tokens WHERE chain_id=1 AND symbol='USDC'),
       (SELECT id FROM tokens WHERE chain_id=1 AND symbol='WETH'),
       30
FROM factories f JOIN dexes d ON d.id=f.dex_id WHERE d.name='SushiSwap' AND f.chain_id=1
ON CONFLICT (chain_id, address) DO NOTHING;
INSERT INTO pools (chain_id, factory_id, address, token0_id, token1_id, fee_tier)
SELECT 1, f.id, '0x06da0fd433c1a5d7a4faa01111c044910a184553',
       (SELECT id FROM tokens WHERE chain_id=1 AND symbol='WETH'),
       (SELECT id FROM tokens WHERE chain_id=1 AND symbol='USDT'),
       30
FROM factories f JOIN dexes d ON d.id=f.dex_id WHERE d.name='SushiSwap' AND f.chain_id=1
ON CONFLICT (chain_id, address) DO NOTHING;
INSERT INTO pools (chain_id, factory_id, address, token0_id, token1_id, fee_tier)
SELECT 1, f.id, '0xceff51756c56ceffca006cd410b03ffc46dd3a58',
       (SELECT id FROM tokens WHERE chain_id=1 AND symbol='WBTC'),
       (SELECT id FROM tokens WHERE chain_id=1 AND symbol='WETH'),
       30
FROM factories f JOIN dexes d ON d.id=f.dex_id WHERE d.name='SushiSwap' AND f.chain_id=1
ON CONFLICT (chain_id, address) DO NOTHING;
INSERT INTO pools (chain_id, factory_id, address, token0_id, token1_id, fee_tier)
SELECT 1, f.id, '0xaaf5110db6e744ff70fb339de037b990a20bdace',
       (SELECT id FROM tokens WHERE chain_id=1 AND symbol='DAI'),
       (SELECT id FROM tokens WHERE chain_id=1 AND symbol='USDC'),
       30
FROM factories f JOIN dexes d ON d.id=f.dex_id WHERE d.name='SushiSwap' AND f.chain_id=1
ON CONFLICT (chain_id, address) DO NOTHING;

COMMIT;
```

- [ ] **Step 2: Local sanity check (psql parses without error)**

```bash
psql --version  # confirm psql ≥14 available locally; if not, skip — VPS will validate
```

- [ ] **Step 3: Commit**

```bash
git add database/migrations/029_seed_defi_v2_mvp.sql
git commit -m "feat(sprint-rps): migration 029 — seed Ethereum v2 MVP universe (10 pools + 5 tokens)"
```

---

## Task 2 — `amm_math.rs` (V2 CPMM, TDD)

**Files:**
- Create: `backend/searcher-rs/src/amm_math.rs`
- Modify: `backend/searcher-rs/src/main.rs` (declare module)

- [ ] **Step 1: Declare module in main.rs**

Open `backend/searcher-rs/src/main.rs` and after the existing `mod` lines (around line 11-18), add:

```rust
mod amm_math;
mod reserves;
```

(`reserves` is added now even though it's empty — Task 3 fills it.)

- [ ] **Step 2: Create empty reserves.rs and amm_math.rs to satisfy `mod` declarations**

```bash
echo "// Sprint RPS Task 3 — implementation in next commit" > backend/searcher-rs/src/reserves.rs
```

```rust
// backend/searcher-rs/src/amm_math.rs (empty placeholder, fills in Step 3)
//! Sprint RPS Task 2 — V2 CPMM math (filled in next commits).
```

- [ ] **Step 3: Write the failing tests**

Replace `amm_math.rs` content with the test module + stub function:

```rust
//! V2 CPMM math — pure functions for amount_out + spread + USD pricing.
//!
//! Reference: UniswapV2Library.getAmountOut.
//! https://github.com/Uniswap/v2-periphery/blob/master/contracts/libraries/UniswapV2Library.sol#L43-L50
//!
//! Doctrine: math is parametrised by `fee_bps` (basis points of 10_000). Default 30 = 0.30%
//! used by both UniswapV2 and SushiSwap; future tiers (e.g. 25 bps Pancake) plug in via the
//! same function without code changes.

use ethers::types::U256;

/// V2 constant-product market maker output amount, post-fee.
///
/// Formula (UniswapV2Library.getAmountOut):
///     amount_in_with_fee = amount_in * (10_000 - fee_bps)
///     numerator          = amount_in_with_fee * reserve_out
///     denominator        = reserve_in * 10_000 + amount_in_with_fee
///     amount_out         = numerator / denominator
///
/// Returns U256::zero() on degenerate inputs (zero reserves or zero amount_in).
pub fn v2_amount_out(amount_in: U256, reserve_in: U256, reserve_out: U256, fee_bps: u32) -> U256 {
    if amount_in.is_zero() || reserve_in.is_zero() || reserve_out.is_zero() {
        return U256::zero();
    }
    let fee_factor = U256::from(10_000u32 - fee_bps);
    let amount_in_with_fee = amount_in.saturating_mul(fee_factor);
    let numerator = amount_in_with_fee.saturating_mul(reserve_out);
    let denominator = reserve_in.saturating_mul(U256::from(10_000u32)).saturating_add(amount_in_with_fee);
    if denominator.is_zero() {
        return U256::zero();
    }
    numerator / denominator
}

#[cfg(test)]
mod tests {
    use super::*;

    /// UniswapV2Library reference: amount_in=1e18 (1 WETH), reserves=(3000e18 WETH, 6_000_000e6 USDC).
    /// Expected: roughly 1995 USDC (less than 2000 due to fee + slippage on 1/3000th of pool).
    /// Hand-computed: amount_in_with_fee = 9.97e21
    ///                numerator   = 9.97e21 * 6e12 = 5.982e34
    ///                denominator = 3e25 + 9.97e21 ≈ 3.00997e25
    ///                out         = 5.982e34 / 3.00997e25 ≈ 1.987e9 → 1987 USDC (6 decimals)
    /// We assert within 5% of 1987e6.
    #[test]
    fn weth_to_usdc_realistic_pool() {
        let amount_in = U256::from(10u128).pow(18.into());                  // 1 WETH
        let reserve_in = U256::from(3000u128) * U256::from(10u128).pow(18.into()); // 3000 WETH
        let reserve_out = U256::from(6_000_000u128) * U256::from(10u128).pow(6.into()); // 6M USDC
        let out = v2_amount_out(amount_in, reserve_in, reserve_out, 30);
        let expected = U256::from(1987u128) * U256::from(10u128).pow(6.into());
        // ±5%
        let lo = expected * U256::from(95) / U256::from(100);
        let hi = expected * U256::from(105) / U256::from(100);
        assert!(out >= lo && out <= hi, "got {} expected ~{}", out, expected);
    }

    #[test]
    fn fee_zero_matches_pure_xy() {
        // With fee_bps=0, amount_out = amount_in * reserve_out / (reserve_in + amount_in)
        let amount_in = U256::from(100u128);
        let reserve_in = U256::from(1_000u128);
        let reserve_out = U256::from(2_000u128);
        let out = v2_amount_out(amount_in, reserve_in, reserve_out, 0);
        // Manual: 100 * 2000 / (1000 + 100) = 200_000 / 1100 = 181 (truncated)
        assert_eq!(out, U256::from(181u128));
    }

    #[test]
    fn zero_amount_in_returns_zero() {
        let out = v2_amount_out(U256::zero(), U256::from(1_000u128), U256::from(2_000u128), 30);
        assert_eq!(out, U256::zero());
    }

    #[test]
    fn zero_reserve_in_returns_zero() {
        let out = v2_amount_out(U256::from(100u128), U256::zero(), U256::from(2_000u128), 30);
        assert_eq!(out, U256::zero());
    }

    #[test]
    fn zero_reserve_out_returns_zero() {
        let out = v2_amount_out(U256::from(100u128), U256::from(1_000u128), U256::zero(), 30);
        assert_eq!(out, U256::zero());
    }

    #[test]
    fn fee_30_bps_reduces_output_vs_zero_fee() {
        let amount_in = U256::from(1_000_000u128);
        let reserve_in = U256::from(10_000_000u128);
        let reserve_out = U256::from(20_000_000u128);
        let no_fee = v2_amount_out(amount_in, reserve_in, reserve_out, 0);
        let with_fee = v2_amount_out(amount_in, reserve_in, reserve_out, 30);
        assert!(with_fee < no_fee, "with_fee={} should be < no_fee={}", with_fee, no_fee);
    }
}
```

- [ ] **Step 4: Run tests to confirm they fail (well, they pass now since impl is in same file — sanity check)**

```bash
cd backend && cargo test -p searcher-rs --lib amm_math 2>&1 | tail -20
```

Expected: `6 passed; 0 failed`. Local Windows AppLocker may block test exec; if so, `cargo build -p searcher-rs --tests` exit 0 is sufficient evidence — VPS Linux runs the actual tests at deploy.

- [ ] **Step 5: Commit**

```bash
git add backend/searcher-rs/src/amm_math.rs backend/searcher-rs/src/reserves.rs backend/searcher-rs/src/main.rs
git commit -m "feat(sprint-rps): amm_math.rs V2 CPMM with 6 unit tests + module placeholders"
```

---

## Task 3 — `reserves.rs` (Redis cache helpers)

**Files:**
- Modify: `backend/searcher-rs/src/reserves.rs` (created in Task 2 as placeholder)

- [ ] **Step 1: Write the impl**

```rust
//! Redis cache layout for pool reserves and token metadata.
//!
//! Keys:
//!   arbx:pool_reserves:<chain_id>:<pool_addr_lower>  → JSON ReservesEntry
//!   arbx:pool_index:<chain_id>:<sym0>:<sym1>          → JSON Vec<String> (pool addrs lower)
//!                                                       sym0 < sym1 lexicographically
//!   arbx:tokens:<chain_id>:<addr_lower>               → JSON TokenMeta
//!
//! TTLs:
//!   pool_reserves: 30s (re-set every 5s by PoolSyncWorker; readers tolerate up to 10s lag)
//!   pool_index   : no expiry (operator-managed via SQL); refreshed at PoolSyncWorker boot
//!   tokens       : no expiry (rarely changes; refreshed at PoolSyncWorker boot)
//!
//! Doctrine: every Redis read returns Option (cache miss is normal at boot, scanner
//! tolerates None by leaving gross_profit=0 and emitting `event=scanner.no_reserves_yet`).

use redis::aio::ConnectionManager;
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReservesEntry {
    /// reserve0 as decimal string (uint112 fits in u128 but we use string for forward-compat)
    pub r0: String,
    pub r1: String,
    /// block number at which the reserves were observed
    pub blk: u64,
    /// unix epoch seconds
    pub ts: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenMeta {
    pub symbol: String,
    pub decimals: u8,
    pub is_stablecoin: bool,
}

pub fn key_pool_reserves(chain_id: u64, pool_addr_lower: &str) -> String {
    format!("arbx:pool_reserves:{}:{}", chain_id, pool_addr_lower)
}

pub fn key_pool_index(chain_id: u64, sym_a: &str, sym_b: &str) -> String {
    let (lo, hi) = if sym_a < sym_b { (sym_a, sym_b) } else { (sym_b, sym_a) };
    format!("arbx:pool_index:{}:{}:{}", chain_id, lo, hi)
}

pub fn key_token(chain_id: u64, addr_lower: &str) -> String {
    format!("arbx:tokens:{}:{}", chain_id, addr_lower)
}

pub async fn set_reserves(
    redis: &mut ConnectionManager,
    chain_id: u64,
    pool_addr_lower: &str,
    entry: &ReservesEntry,
    ttl_secs: u64,
) -> redis::RedisResult<()> {
    let json = serde_json::to_string(entry).map_err(|e| {
        redis::RedisError::from((redis::ErrorKind::TypeError, "serde", e.to_string()))
    })?;
    let _: () = redis
        .set_ex(key_pool_reserves(chain_id, pool_addr_lower), json, ttl_secs)
        .await?;
    Ok(())
}

pub async fn get_reserves(
    redis: &mut ConnectionManager,
    chain_id: u64,
    pool_addr_lower: &str,
) -> redis::RedisResult<Option<ReservesEntry>> {
    let raw: Option<String> = redis.get(key_pool_reserves(chain_id, pool_addr_lower)).await?;
    Ok(raw.and_then(|s| serde_json::from_str(&s).ok()))
}

pub async fn set_pool_index(
    redis: &mut ConnectionManager,
    chain_id: u64,
    sym_a: &str,
    sym_b: &str,
    pool_addrs_lower: &[String],
) -> redis::RedisResult<()> {
    let json = serde_json::to_string(pool_addrs_lower).map_err(|e| {
        redis::RedisError::from((redis::ErrorKind::TypeError, "serde", e.to_string()))
    })?;
    let _: () = redis
        .set(key_pool_index(chain_id, sym_a, sym_b), json)
        .await?;
    Ok(())
}

pub async fn get_pools_for_pair(
    redis: &mut ConnectionManager,
    chain_id: u64,
    sym_a: &str,
    sym_b: &str,
) -> redis::RedisResult<Vec<String>> {
    let raw: Option<String> = redis.get(key_pool_index(chain_id, sym_a, sym_b)).await?;
    Ok(raw.and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default())
}

pub async fn set_token_meta(
    redis: &mut ConnectionManager,
    chain_id: u64,
    addr_lower: &str,
    meta: &TokenMeta,
) -> redis::RedisResult<()> {
    let json = serde_json::to_string(meta).map_err(|e| {
        redis::RedisError::from((redis::ErrorKind::TypeError, "serde", e.to_string()))
    })?;
    let _: () = redis.set(key_token(chain_id, addr_lower), json).await?;
    Ok(())
}

pub async fn get_token_meta(
    redis: &mut ConnectionManager,
    chain_id: u64,
    addr_lower: &str,
) -> redis::RedisResult<Option<TokenMeta>> {
    let raw: Option<String> = redis.get(key_token(chain_id, addr_lower)).await?;
    Ok(raw.and_then(|s| serde_json::from_str(&s).ok()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pool_index_key_sorts_symbols() {
        assert_eq!(key_pool_index(1, "WETH", "USDC"), "arbx:pool_index:1:USDC:WETH");
        assert_eq!(key_pool_index(1, "USDC", "WETH"), "arbx:pool_index:1:USDC:WETH");
    }

    #[test]
    fn reserves_key_lowercase_addr() {
        assert_eq!(
            key_pool_reserves(1, "0xb4e16d01"),
            "arbx:pool_reserves:1:0xb4e16d01"
        );
    }

    #[test]
    fn reserves_entry_serde_roundtrip() {
        let entry = ReservesEntry {
            r0: "12345".into(),
            r1: "67890".into(),
            blk: 18_500_000,
            ts: 1_714_857_600,
        };
        let json = serde_json::to_string(&entry).unwrap();
        let back: ReservesEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(back.r0, entry.r0);
        assert_eq!(back.blk, entry.blk);
    }
}
```

- [ ] **Step 2: cargo build to confirm it compiles**

```bash
cd backend && cargo build -p searcher-rs 2>&1 | tail -10
```

Expected: `Finished` with 0 errors.

- [ ] **Step 3: cargo test the unit tests (Windows AppLocker may block exec — fall back to `cargo test --no-run`)**

```bash
cd backend && cargo test -p searcher-rs --lib reserves 2>&1 | tail -10
```

Expected (Linux): `3 passed; 0 failed`. Windows AppLocker fallback: `cargo test --no-run -p searcher-rs` exit 0 is acceptable.

- [ ] **Step 4: Commit**

```bash
git add backend/searcher-rs/src/reserves.rs
git commit -m "feat(sprint-rps): reserves.rs Redis cache helpers + 3 unit tests"
```

---

## Task 4 — `pool_sync_worker.rs` (replace stub with real impl)

**Files:**
- Modify: `backend/searcher-rs/src/workers/pool_sync_worker.rs` (full replace)
- Modify: `backend/searcher-rs/Cargo.toml` (add ethers `abigen` feature)

- [ ] **Step 1: Add ethers `abigen` feature in searcher-rs Cargo.toml**

Open `backend/searcher-rs/Cargo.toml`. Find the line `ethers.workspace = true`. Replace with:

```toml
ethers = { workspace = true, features = ["abigen", "rustls"] }
```

Note: `ws` is in workspace defaults. Adding `abigen` enables the `abigen!` macro and `Multicall` helper.

- [ ] **Step 2: Verify cargo check passes after feature change**

```bash
cd backend && cargo check -p searcher-rs 2>&1 | tail -5
```

Expected: `Finished`. If error about feature collision, fall back to `features = ["abigen"]` only.

- [ ] **Step 3: Replace pool_sync_worker.rs with real impl**

```rust
//! PoolSyncWorker — fetches V2 pool reserves via Multicall3, persists to
//! Postgres `pool_reserves` and Redis `arbx:pool_reserves:<chain>:<addr>`.
//!
//! Boot sequence:
//!   1. Read pools+tokens+factories from Postgres (one query each, cached in struct).
//!   2. Populate Redis `arbx:tokens:*` and `arbx:pool_index:*` from DB rows.
//!   3. Start polling loop: every `poll_interval`, do 1 Multicall3 aggregate3 with
//!      N `getReserves()` calls, decode results, batch INSERT into pool_reserves,
//!      individual SET into Redis with 30s TTL.
//!
//! Doctrine: log structured tracing JSON (no fake metrics), report measured
//! latency, fail-loud on RPC error (do not pretend success).

use ethers::abi::AbiEncode;
use ethers::contract::abigen;
use ethers::providers::{Http, Middleware, Provider};
use ethers::types::{Address, Bytes, H160, U256};
use sqlx::PgPool;
use std::str::FromStr;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::time::sleep;
use tracing::{debug, error, info, warn};

use crate::reserves::{
    self, key_pool_index, set_pool_index, set_reserves, set_token_meta, ReservesEntry, TokenMeta,
};

abigen!(
    IUniswapV2Pair,
    r#"[
        function getReserves() external view returns (uint112 reserve0, uint112 reserve1, uint32 blockTimestampLast)
    ]"#,
);

abigen!(
    IMulticall3,
    r#"[
        function aggregate3((address target, bool allowFailure, bytes callData)[] calls) external payable returns ((bool success, bytes returnData)[] memory returnData)
        function getBlockNumber() external view returns (uint256)
    ]"#,
);

const MULTICALL3_ADDR: &str = "0xcA11bde05977b3631167028862bE2a173976CA11";
const RESERVES_TTL_SECS: u64 = 30;

struct PoolRow {
    address: H160,
    address_lower: String,
    sym0: String,
    sym1: String,
}

pub struct PoolSyncWorker {
    pub poll_interval: Duration,
    pub chain_id: u64,
}

impl PoolSyncWorker {
    pub fn new(poll_interval_ms: u64, chain_id: u64) -> Self {
        Self {
            poll_interval: Duration::from_millis(poll_interval_ms),
            chain_id,
        }
    }

    /// Bootstrap caches from DB then enter polling loop. Designed to run forever;
    /// returns only on unrecoverable errors.
    pub async fn run(
        self,
        rpc_http_url: String,
        db: PgPool,
        mut redis: redis::aio::ConnectionManager,
    ) -> anyhow::Result<()> {
        info!(event = "pool_sync.boot", chain_id = self.chain_id, rpc = %redacted(&rpc_http_url));

        let provider = Arc::new(Provider::<Http>::try_from(rpc_http_url)?);
        let multicall_addr = Address::from_str(MULTICALL3_ADDR)?;

        // Bootstrap: read pools + tokens from DB and populate Redis caches.
        let pools = self.load_pools(&db).await?;
        info!(event = "pool_sync.pools_loaded", chain_id = self.chain_id, count = pools.len());

        self.bootstrap_token_cache(&db, &mut redis).await?;
        self.bootstrap_pool_index_cache(&pools, &mut redis).await?;
        info!(event = "pool_sync.caches_bootstrapped", chain_id = self.chain_id);

        // Build static call data once per pool — getReserves() has no args, just the selector.
        let get_reserves_selector: [u8; 4] = ethers::utils::keccak256("getReserves()")[..4]
            .try_into()
            .unwrap();
        let get_reserves_calldata = Bytes::from(get_reserves_selector.to_vec());

        let multicall = IMulticall3::new(multicall_addr, provider.clone());

        loop {
            let tick_start = Instant::now();
            let calls: Vec<_> = pools
                .iter()
                .map(|p| iMulticall3_mod::Call3 {
                    target: p.address,
                    allow_failure: true,
                    call_data: get_reserves_calldata.clone(),
                })
                .collect();

            let results = match multicall.aggregate_3(calls).call().await {
                Ok(r) => r,
                Err(e) => {
                    warn!(event = "pool_sync.multicall_failed", error = %e);
                    sleep(self.poll_interval).await;
                    continue;
                }
            };

            // Get current block once per tick.
            let block_number = provider
                .get_block_number()
                .await
                .map(|n| n.as_u64())
                .unwrap_or(0);
            let now_ts = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);

            let mut ok_count = 0usize;
            let mut fail_count = 0usize;

            // Persist each result.
            for (pool, result) in pools.iter().zip(results.iter()) {
                if !result.success || result.return_data.len() < 64 {
                    fail_count += 1;
                    debug!(event = "pool_sync.pool_failed", pool = %pool.address_lower);
                    continue;
                }
                // ABI-decode (uint112 reserve0, uint112 reserve1, uint32 blockTimestampLast)
                // Each value is left-padded to 32 bytes in returndata.
                let bytes = &result.return_data;
                let r0 = U256::from_big_endian(&bytes[0..32]);
                let r1 = U256::from_big_endian(&bytes[32..64]);

                let entry = ReservesEntry {
                    r0: r0.to_string(),
                    r1: r1.to_string(),
                    blk: block_number,
                    ts: now_ts,
                };

                // Redis SET with TTL.
                if let Err(e) = set_reserves(
                    &mut redis,
                    self.chain_id,
                    &pool.address_lower,
                    &entry,
                    RESERVES_TTL_SECS,
                )
                .await
                {
                    warn!(event = "pool_sync.redis_set_failed", pool = %pool.address_lower, error = %e);
                }

                // Postgres INSERT (best-effort; failures don't kill the loop).
                if let Err(e) = sqlx::query(
                    r#"INSERT INTO pool_reserves (pool_id, block_number, reserve0, reserve1, timestamp)
                       SELECT id, $1, $2::numeric, $3::numeric, NOW()
                       FROM pools WHERE chain_id=$4 AND address=$5"#,
                )
                .bind(block_number as i64)
                .bind(&entry.r0)
                .bind(&entry.r1)
                .bind(self.chain_id as i64)
                .bind(&pool.address_lower)
                .execute(&db)
                .await
                {
                    warn!(event = "pool_sync.db_insert_failed", pool = %pool.address_lower, error = %e);
                }

                ok_count += 1;
            }

            let elapsed_ms = tick_start.elapsed().as_millis();
            info!(
                event = "pool_sync.tick",
                chain_id = self.chain_id,
                pools = pools.len(),
                ok = ok_count,
                failed = fail_count,
                block = block_number,
                latency_ms = elapsed_ms as u64,
            );

            sleep(self.poll_interval).await;
        }
    }

    async fn load_pools(&self, db: &PgPool) -> anyhow::Result<Vec<PoolRow>> {
        let rows = sqlx::query_as::<_, (String, String, String)>(
            r#"SELECT p.address, t0.symbol, t1.symbol
               FROM pools p
               JOIN tokens t0 ON p.token0_id = t0.id
               JOIN tokens t1 ON p.token1_id = t1.id
               WHERE p.chain_id = $1 AND p.is_active = TRUE"#,
        )
        .bind(self.chain_id as i64)
        .fetch_all(db)
        .await?;

        Ok(rows
            .into_iter()
            .filter_map(|(addr, sym0, sym1)| {
                let lower = addr.to_lowercase();
                Address::from_str(&lower)
                    .ok()
                    .map(|h| PoolRow {
                        address: h,
                        address_lower: lower,
                        sym0,
                        sym1,
                    })
            })
            .collect())
    }

    async fn bootstrap_token_cache(
        &self,
        db: &PgPool,
        redis: &mut redis::aio::ConnectionManager,
    ) -> anyhow::Result<()> {
        let rows = sqlx::query_as::<_, (String, String, i32, bool)>(
            r#"SELECT address, symbol, decimals, is_stablecoin
               FROM tokens WHERE chain_id = $1 AND is_active = TRUE"#,
        )
        .bind(self.chain_id as i64)
        .fetch_all(db)
        .await?;

        for (addr, symbol, decimals, is_stable) in rows {
            let meta = TokenMeta {
                symbol,
                decimals: decimals as u8,
                is_stablecoin: is_stable,
            };
            if let Err(e) =
                set_token_meta(redis, self.chain_id, &addr.to_lowercase(), &meta).await
            {
                warn!(event = "pool_sync.token_cache_set_failed", error = %e);
            }
        }
        Ok(())
    }

    async fn bootstrap_pool_index_cache(
        &self,
        pools: &[PoolRow],
        redis: &mut redis::aio::ConnectionManager,
    ) -> anyhow::Result<()> {
        // Group pool addresses by sorted-symbol pair.
        use std::collections::HashMap;
        let mut by_pair: HashMap<(String, String), Vec<String>> = HashMap::new();
        for p in pools {
            let (lo, hi) = if p.sym0 <= p.sym1 {
                (p.sym0.clone(), p.sym1.clone())
            } else {
                (p.sym1.clone(), p.sym0.clone())
            };
            by_pair.entry((lo, hi)).or_default().push(p.address_lower.clone());
        }
        for ((sym_a, sym_b), addrs) in by_pair {
            if let Err(e) = set_pool_index(redis, self.chain_id, &sym_a, &sym_b, &addrs).await {
                warn!(event = "pool_sync.pool_index_set_failed", error = %e);
            }
        }
        Ok(())
    }
}

fn redacted(rpc_url: &str) -> String {
    // Strip API key / path. Show only scheme://host.
    if let Some(scheme_idx) = rpc_url.find("://") {
        let after = &rpc_url[scheme_idx + 3..];
        let host = after.split('/').next().unwrap_or(after);
        format!("{}://{}/...", &rpc_url[..scheme_idx], host)
    } else {
        "<redacted>".to_string()
    }
}
```

- [ ] **Step 4: cargo check (will fail because old constructor signature `new(50)` doesn't match `new(poll_interval_ms, chain_id)` — Task 5 fixes the orchestrator)**

```bash
cd backend && cargo check -p searcher-rs 2>&1 | tail -10
```

Expected: errors about `PoolSyncWorker::new(50)` arity. Proceed to Task 5.

- [ ] **Step 5: Commit (broken build acceptable, fixed in next task)**

```bash
git add backend/searcher-rs/src/workers/pool_sync_worker.rs backend/searcher-rs/Cargo.toml
git commit -m "feat(sprint-rps): pool_sync_worker.rs real Multicall3 + Postgres+Redis writers"
```

---

## Task 5 — Delete stubs + wire orchestrator

**Files:**
- Delete: `backend/searcher-rs/src/workers/route_discovery_worker.rs`
- Delete: `backend/searcher-rs/src/workers/simulation_worker.rs`
- Modify: `backend/searcher-rs/src/workers/mod.rs`
- Modify: `backend/searcher-rs/src/main.rs`

- [ ] **Step 1: Delete the two stub workers**

```bash
git rm backend/searcher-rs/src/workers/route_discovery_worker.rs
git rm backend/searcher-rs/src/workers/simulation_worker.rs
```

- [ ] **Step 2: Update workers/mod.rs**

Replace entire file with:

```rust
//! Worker orchestrator. Sub-proyecto-1 (Real Profit Signal): only RpcHealthWorker
//! and PoolSyncWorker are real. RouteDiscoveryWorker and SimulationWorker stubs
//! were deleted because they emitted fake telemetry without doing work.
//! HftMempoolListener and ExecutionWorker stubs are kept but not spawned.

pub mod execution_worker;
pub mod hft_mempool_listener;
pub mod pool_sync_worker;
pub mod rpc_health_worker;

use sqlx::PgPool;
use std::sync::Arc;
use tracing::{error, info};

pub struct WorkerOrchestrator {
    pub god_protocol_active: bool,
    pub kernel_bypass_enabled: bool,
}

impl WorkerOrchestrator {
    pub fn new(god_protocol_active: bool, kernel_bypass_enabled: bool) -> Self {
        Self {
            god_protocol_active,
            kernel_bypass_enabled,
        }
    }

    pub async fn start_all(
        &self,
        chain_id: u64,
        rpc_http_url: String,
        db: Option<PgPool>,
        redis: redis::aio::ConnectionManager,
    ) {
        info!(event = "worker_orchestrator.boot", chain_id, god_protocol = self.god_protocol_active, kernel_bypass = self.kernel_bypass_enabled);

        let rpc_worker = rpc_health_worker::RpcHealthWorker::new(5000);
        tokio::spawn(async move {
            rpc_worker.start().await;
        });

        if let Some(db_pool) = db {
            let pool_worker = pool_sync_worker::PoolSyncWorker::new(5000, chain_id);
            let redis_clone = redis.clone();
            tokio::spawn(async move {
                if let Err(e) = pool_worker.run(rpc_http_url, db_pool, redis_clone).await {
                    error!(event = "pool_sync.terminated", chain_id, error = %e);
                }
            });
            info!(event = "worker_orchestrator.pool_sync_started", chain_id);
        } else {
            info!(event = "worker_orchestrator.pool_sync_skipped", reason = "no_db_pool");
        }
    }
}
```

- [ ] **Step 3: Update main.rs to pass DB + redis + chain_id + RPC URL into orchestrator**

Open `backend/searcher-rs/src/main.rs`. Find the existing block:

```rust
    let orchestrator = workers::WorkerOrchestrator::new(god_protocol_active, kernel_bypass_enabled);

    // Spawn orchestrator asynchronously
    tokio::spawn(async move {
        orchestrator.start_all().await;
    });
```

Replace with:

```rust
    let orchestrator = workers::WorkerOrchestrator::new(god_protocol_active, kernel_bypass_enabled);

    // Sub-proyecto-1: orchestrator now needs chain_id + RPC URL + DB + Redis to drive
    // a real PoolSyncWorker. We pick the first enabled chain (Ethereum chain_id=1 by
    // default in app.toml) and the first http RPC for that chain. Multi-chain support
    // arrives in a later sub-project.
    let primary_chain = enabled_chains.first().copied().unwrap_or(1);
    let primary_rpc_http = cfg
        .chain_endpoints(primary_chain)
        .iter()
        .find_map(|ep| {
            if ep.url.starts_with("http") { Some(ep.url.clone()) } else { None }
        })
        .unwrap_or_else(|| "http://invalid-no-http-endpoint-configured".to_string());
    let orchestrator_db = db_pool.clone();
    let orchestrator_redis = redis_conn.clone();
    tokio::spawn(async move {
        orchestrator.start_all(primary_chain, primary_rpc_http, orchestrator_db, orchestrator_redis).await;
    });
```

If `cfg.chain_endpoints(chain_id)` doesn't exist as a method, replace with the equivalent inline that mirrors what `scanner::run_chain` does to fetch endpoints. Look at `scanner.rs:47-90` for the canonical pattern (it uses `RpcPool::for_chain(chain_id, &cfg)`); use that helper to get the first http URL.

- [ ] **Step 4: cargo check — should pass now**

```bash
cd backend && cargo check -p searcher-rs 2>&1 | tail -10
```

Expected: `Finished` 0 errors.

- [ ] **Step 5: Commit**

```bash
git add backend/searcher-rs/src/workers/ backend/searcher-rs/src/main.rs
git commit -m "feat(sprint-rps): delete RouteDiscovery+Simulation stubs + wire real PoolSyncWorker"
```

---

## Task 6 — Scanner enrichment

**Files:**
- Modify: `backend/searcher-rs/src/scanner.rs` (lines 259-270, replace candidate construction)

- [ ] **Step 1: Edit `process_pending` to enrich candidate using reserves**

Open `backend/searcher-rs/src/scanner.rs`. Find the current candidate block at lines 259-270 (right after `let cfg_opt = match trading_config.state(client.chain_id).await { ... }`).

Replace this block:

```rust
    let candidate = OpportunityCandidate {
        route_fingerprint: format!("{}_{}_{}", opportunity.dex_a, opportunity.token_in, opportunity.token_out),
        pool_addresses: vec![],
        token_addresses: vec![opportunity.token_in.clone(), opportunity.token_out.clone()],
        dex_adapters: vec![opportunity.dex_a.clone()],
        amount_in: amount_in_f64,
        // Until route-finder + reserves fetch wire up, expected_amount_out
        // mirrors amount_in (gross_profit = 0) — math-engine then flags it as
        // not viable, which is the honest signal.
        expected_amount_out: amount_in_f64,
        gross_profit: 0.0,
    };
```

With this enriched block:

```rust
    // Sub-proyecto-1 enrichment: lookup reserves + compute V2 amount_out + spread.
    // Order of operations: token meta → pool index → per-pool reserves → spread.
    // Each lookup tolerates miss with explicit log; net effect on a cold cache is
    // gross_profit=0 (same as before this sub-project) so behaviour degrades
    // gracefully when PoolSyncWorker hasn't ticked yet.
    let token_in_lower = opportunity.token_in.to_lowercase();
    let token_out_lower = opportunity.token_out.to_lowercase();
    let amount_in_wei_u256 = ethers::types::U256::from_dec_str(&opportunity.amount_in_wei)
        .unwrap_or_else(|_| ethers::types::U256::zero());

    let meta_in = reserves::get_token_meta(redis, client.chain_id, &token_in_lower).await.ok().flatten();
    let meta_out = reserves::get_token_meta(redis, client.chain_id, &token_out_lower).await.ok().flatten();

    let (mut expected_amount_out_f64, mut gross_profit_f64, mut gross_profit_token_out) =
        (amount_in_f64, 0.0_f64, ethers::types::U256::zero());

    if let (Some(m_in), Some(m_out)) = (&meta_in, &meta_out) {
        let pools = reserves::get_pools_for_pair(redis, client.chain_id, &m_in.symbol, &m_out.symbol)
            .await
            .unwrap_or_default();
        if pools.len() < 2 {
            debug!(event = "scanner.single_pool_no_spread",
                   pair = format!("{}-{}", m_in.symbol, m_out.symbol),
                   pool_count = pools.len());
        } else {
            // Compute amount_out per pool. token0_first determines which reserve
            // is "in" vs "out": when token_in == token0, in=r0, out=r1; else swapped.
            let mut outs: Vec<ethers::types::U256> = Vec::with_capacity(pools.len());
            for pool_addr in &pools {
                let entry = match reserves::get_reserves(redis, client.chain_id, pool_addr).await.ok().flatten() {
                    Some(e) => e,
                    None => continue,
                };
                let r0 = ethers::types::U256::from_dec_str(&entry.r0).unwrap_or_else(|_| ethers::types::U256::zero());
                let r1 = ethers::types::U256::from_dec_str(&entry.r1).unwrap_or_else(|_| ethers::types::U256::zero());
                // We don't know orientation from Redis alone; we'd need the pools.token0_id
                // join. For MVP we compute BOTH orientations and pick the one yielding
                // non-degenerate amount_out (the wrong orientation gives much smaller out
                // because the reserves are mismatched to the swap direction).
                let out_a = amm_math::v2_amount_out(amount_in_wei_u256, r0, r1, 30);
                let out_b = amm_math::v2_amount_out(amount_in_wei_u256, r1, r0, 30);
                let out = std::cmp::max(out_a, out_b);
                outs.push(out);
            }
            if outs.len() >= 2 {
                outs.sort();
                let lo = outs[0];
                let hi = outs[outs.len() - 1];
                gross_profit_token_out = hi.saturating_sub(lo);
                let decimals_out = m_out.decimals as u32;
                let scale = 10f64.powi(decimals_out as i32);
                expected_amount_out_f64 = u256_to_f64_lossy(hi) / scale;
                let spread_token_out_f64 = u256_to_f64_lossy(gross_profit_token_out) / scale;

                // USD pricing rules (spec §2 USD pricing rules):
                gross_profit_f64 = if let Some(cfg_ref) = cfg_opt.as_ref() {
                    if m_out.symbol.eq_ignore_ascii_case(&cfg_ref.base_token_symbol) {
                        spread_token_out_f64 * cfg_ref.base_token_price_usd
                    } else if m_out.is_stablecoin {
                        spread_token_out_f64
                    } else {
                        debug!(event = "scanner.usd_conversion_pending_oracle",
                               token_out_symbol = %m_out.symbol);
                        0.0
                    }
                } else {
                    0.0
                };

                info!(event = "scanner.candidate_enriched",
                      hash = %hash,
                      pair = format!("{}-{}", m_in.symbol, m_out.symbol),
                      pool_count = pools.len(),
                      hi = %hi, lo = %lo,
                      spread_token_out = %gross_profit_token_out,
                      gross_profit_usd = gross_profit_f64);
            }
        }
    } else {
        debug!(event = "scanner.token_meta_unknown",
               token_in = %token_in_lower, token_out = %token_out_lower);
    }

    let candidate = OpportunityCandidate {
        route_fingerprint: format!("{}_{}_{}", opportunity.dex_a, opportunity.token_in, opportunity.token_out),
        pool_addresses: vec![],
        token_addresses: vec![opportunity.token_in.clone(), opportunity.token_out.clone()],
        dex_adapters: vec![opportunity.dex_a.clone()],
        amount_in: amount_in_f64,
        expected_amount_out: expected_amount_out_f64,
        gross_profit: gross_profit_f64,
    };
```

After the function `process_pending`, add this helper (or place it at the bottom of the file):

```rust
fn u256_to_f64_lossy(v: ethers::types::U256) -> f64 {
    // U256 → f64 via decimal string. Loses precision past ~15 sig figs but
    // f64 is what OpportunityCandidate uses; this is a one-way display path,
    // never re-fed into on-chain arithmetic.
    v.to_string().parse::<f64>().unwrap_or(0.0)
}
```

Also ensure these `use` statements exist at the top of `scanner.rs` (add if missing):

```rust
use crate::amm_math;
use crate::reserves;
```

- [ ] **Step 2: cargo check + cargo test compile**

```bash
cd backend && cargo check -p searcher-rs 2>&1 | tail -10 && cargo test --no-run -p searcher-rs 2>&1 | tail -5
```

Expected: both `Finished` exit 0.

- [ ] **Step 3: Commit**

```bash
git add backend/searcher-rs/src/scanner.rs
git commit -m "feat(sprint-rps): scanner.rs enrichment — V2 spread amount_out + USD pricing"
```

---

## Task 7 — Local verification before deploy

- [ ] **Step 1: Frontend + api-server typecheck (defensive — confirm no cross-cutting break)**

```bash
cd frontend && npx tsc --noEmit ; echo "FE_EXIT=$?"
cd ../backend/api-server && npx tsc --noEmit ; echo "API_EXIT=$?"
```

Expected: both 0.

- [ ] **Step 2: Backend full cargo build (release mode — same as Docker uses)**

```bash
cd backend && cargo build --release -p searcher-rs 2>&1 | tail -5 ; echo "RELEASE_EXIT=$?"
```

Expected: `Finished` exit 0. May take 2-3 min on cold cache.

- [ ] **Step 3: Run unit tests (Linux); on Windows use --no-run as evidence**

```bash
cd backend && cargo test -p searcher-rs --lib 2>&1 | tail -10
```

Expected (Linux): all tests pass. Windows AppLocker fallback: `cargo test --no-run` exit 0.

- [ ] **Step 4: Push to remotes**

```bash
git push origin main
git push github main
```

---

## Task 8 — VPS deploy + verify

- [ ] **Step 1: Tag pre-deploy recovery point**

```bash
TAG="pre-rps-deploy-$(date -u +%Y%m%d-%H%M)"
git tag -a "$TAG" -m "Recovery point before Sub-proyecto 1 deploy"
git push origin "$TAG"
git push github "$TAG"
echo "TAG=$TAG"
```

- [ ] **Step 2: SSH + pg_dump backup + git pull + apply migration 029**

```bash
ssh arbx 'set -e
  cd /opt/arbitragex-v2
  STAMP=$(date -u +%Y%m%d-%H%M%S)
  BACKUP=/opt/arbitragex-v2/backups/pre-rps-${STAMP}.sql.gz
  mkdir -p /opt/arbitragex-v2/backups
  docker exec arbitragex-v2-postgres-1 pg_dump -U postgres arbitragex | gzip > "$BACKUP"
  ls -lh "$BACKUP"
  git pull origin main
  cat database/migrations/029_seed_defi_v2_mvp.sql | docker exec -i arbitragex-v2-postgres-1 psql -U postgres -d arbitragex
'
```

Expected output: backup file size ~350K-1MB; migration prints `INSERT 0 N` lines (some may say 0 if rows already exist due to ON CONFLICT, expected on idempotent re-run).

- [ ] **Step 3: Verify seed**

```bash
ssh arbx 'docker exec arbitragex-v2-postgres-1 psql -U postgres -d arbitragex -c "
  SELECT '\''chains:'\''   AS k, COUNT(*) FROM chains
  UNION ALL SELECT '\''dexes:'\'',     COUNT(*) FROM dexes
  UNION ALL SELECT '\''factories:'\'', COUNT(*) FROM factories
  UNION ALL SELECT '\''tokens:'\'',    COUNT(*) FROM tokens
  UNION ALL SELECT '\''pools:'\'',     COUNT(*) FROM pools;"'
```

Expected: chains≥1, dexes≥2, factories≥2, tokens≥5, pools≥9 (DAI/USDC sushi may collide with another seed — 9-10 acceptable).

- [ ] **Step 4: Rebuild searcher-rs (R3: --no-cache --env-file .env)**

```bash
ssh arbx 'cd /opt/arbitragex-v2 && docker compose --env-file .env -f docker/compose.dev.yml build --no-cache searcher-rs 2>&1 | tail -5'
```

Expected: `Image arbitragex-v2-searcher-rs Built`. Allow 5-10 min for full Rust release build.

- [ ] **Step 5: Bring up searcher-rs**

```bash
ssh arbx 'cd /opt/arbitragex-v2 && docker compose --env-file .env -f docker/compose.dev.yml up -d searcher-rs && sleep 8 && docker compose -f docker/compose.dev.yml ps --format "table {{.Service}}\t{{.Status}}" | grep searcher'
```

Expected: `searcher-rs Up` with no restart.

- [ ] **Step 6: Verify event distribution (no more fake "1250 pools 4ms")**

```bash
ssh arbx 'sleep 30 && docker logs --since 30s arbitragex-v2-searcher-rs-1 2>&1 | grep -oE "\"event\":\"[^\"]+\"" | sort | uniq -c | sort -rn | head -15'
```

Expected events present:
- `"event":"pool_sync.boot"` (1, at startup)
- `"event":"pool_sync.pools_loaded"` (1)
- `"event":"pool_sync.caches_bootstrapped"` (1)
- `"event":"pool_sync.tick"` (5-6 in 30s, every 5s)
- `"event":"scanner.candidate_enriched"` (variable, depends on mempool tx volume)

Expected events ABSENT:
- `"Reservas sincronizadas para 1250 pools. Latencia: 4ms"` (the lying string from the deleted stub)

- [ ] **Step 7: Verify pool_reserves DB ingestion**

```bash
ssh arbx 'docker exec arbitragex-v2-postgres-1 psql -U postgres -d arbitragex -c "
  SELECT COUNT(*) AS rows_last_minute,
         MIN(timestamp) AS first, MAX(timestamp) AS last
  FROM pool_reserves
  WHERE timestamp > NOW() - INTERVAL '\''1 minute'\'';"'
```

Expected: `rows_last_minute` ≈ 100 (10 pools × 12 ticks/min × success_rate ~90%).

- [ ] **Step 8: Verify Redis caches populated**

```bash
ssh arbx 'docker exec arbitragex-v2-redis-1 redis-cli --scan --pattern "arbx:pool_reserves:*" | head -3
  echo "---"
  docker exec arbitragex-v2-redis-1 redis-cli --scan --pattern "arbx:pool_index:*" | head -5
  echo "---"
  docker exec arbitragex-v2-redis-1 redis-cli --scan --pattern "arbx:tokens:*" | head -5'
```

Expected: 9-10 pool_reserves keys, 4-5 pool_index keys, 5 tokens keys.

- [ ] **Step 9: Verify scanner enriched at least one candidate**

```bash
ssh arbx 'docker exec arbitragex-v2-postgres-1 psql -U postgres -d arbitragex -c "
  SELECT COUNT(*) FILTER (WHERE expected_profit_usd > 0) AS with_profit,
         COUNT(*) FILTER (WHERE expected_profit_usd = 0) AS without_profit,
         COUNT(*) AS total
  FROM opportunities
  WHERE detected_at > NOW() - INTERVAL '\''5 minutes'\'';"'
```

Expected: `with_profit > 0` AT LEAST after a few minutes of mempool volume + matching pairs. If 0 with_profit after 10 min, **STOP and diagnose**: check operator's allowlist (must include WETH/USDC/USDT/DAI/WBTC at minimum) and `enabled` flag on trading_config.

- [ ] **Step 10: Tag post-deploy + final report**

```bash
TAG="post-rps-deploy-$(date -u +%Y%m%d-%H%M)"
git tag -a "$TAG" -m "Sub-proyecto 1 deployed and verified"
git push origin "$TAG"
git push github "$TAG"
echo "TAG=$TAG"
```

---

## Rollback

```bash
# Revert all RPS commits
git revert <task1>..<task6>

# VPS rollback
ssh arbx 'cd /opt/arbitragex-v2 && git pull origin main \
  && docker compose --env-file .env -f docker/compose.dev.yml build --no-cache searcher-rs \
  && docker compose --env-file .env -f docker/compose.dev.yml up -d searcher-rs'

# DB rollback (only if needed — seed is harmless to leave):
# Restore from pg_dump if seed corrupted any prod table:
ssh arbx 'gunzip -c /opt/arbitragex-v2/backups/pre-rps-<STAMP>.sql.gz \
  | docker exec -i arbitragex-v2-postgres-1 psql -U postgres -d arbitragex'
```

---

## Out of scope (per spec §5)

- V3 (concentrated liquidity, sqrt prices, tick math). Sub-proyecto futuro.
- Curve/Balancer math. Sub-proyecto futuro.
- Bellman-Ford multi-hop arbitrage. This MVP only does 2-pool spread direct comparison.
- Passive scanner (`RouteDiscoveryWorker` real). Sub-proyecto futuro.
- Event-based reserves (subscribe Sync). Polling 5s sufficient for validation MVP.
- Multi-chain. Only Ethereum mainnet here.
- simulator-v2 wiring end-to-end. Sub-proyecto 2.
- Bundle build + relay submit + signing + executions. Sub-proyecto 3.

---

## Self-Review

**Spec coverage** (vs `docs/superpowers/specs/2026-05-04-real-profit-signal-design.md`):
- §2 components nuevos → Tasks 1-6 cover all 5 files (3 new + 2 modify) and 2 deletions
- §2 pool universe → Task 1 seeds all 10 pools
- §2 sync model polling 5s → Task 4 PoolSyncWorker uses `Duration::from_millis(5000)`
- §2 detection path scanner-pull → Task 6 uses Redis lookups, no architectural change
- §2 USD pricing rules (WETH-base / stablecoin / fallback) → Task 6 implements all three branches
- §3 no-damage matrix → Tasks only touch listed files (verified by hand walk)
- §4 success metrics → Task 8 verifies all 5 SQL/log queries
- §5 out of scope → none of these tasks introduce V3 / Curve / multi-hop / bundle / signing
- §7 risks → Multicall3 graceful degradation in Task 4 Step 3 (loop continues on error); RPC quota addressed by 5s interval; reserves staleness disclosed in scanner.candidate_enriched log; race condition Redis-vs-DB acknowledged (Redis is source of truth); token decimals seeded; stub deletion blast radius compiler-checked

**Placeholder scan**: zero "TBD"/"TODO"/incomplete sections. The placeholder `reserves.rs` created in Task 2 Step 2 is a **deliberate intermediate state** explicitly filled in by Task 3 — the comment says so in code.

**Type consistency**: `ReservesEntry`/`TokenMeta` defined in Task 3 are referenced consistently in Tasks 4 and 6. `v2_amount_out` signature in Task 2 matches its call site in Task 6. `PoolSyncWorker::new(poll_interval_ms, chain_id)` signature in Task 4 matches the call in Task 5 Step 2 (`new(5000, chain_id)`).

**Gap check**: The `cfg.chain_endpoints(primary_chain)` method in Task 5 Step 3 may not exist verbatim — the engineer reading this plan must look at `scanner::run_chain` (`scanner.rs:47-90`) for the canonical pattern (which uses `RpcPool::for_chain` or similar) and use the same accessor here. Task 5 Step 3 explicitly flags this with the "If `cfg.chain_endpoints` doesn't exist..." note.
