//! SIMWIRE-02c P1-5 — redelivery idempotency against live PG.
//!
//! XAUTOCLAIM redelivers a PEL entry whose FINAL XACK failed after
//! persist+XADD already succeeded. The partial unique index from migration
//! 113 + `insert_simulation`'s `ON CONFLICT (opportunity_id) WHERE
//! simulator = 'revm' DO NOTHING` must make that redelivery a no-op:
//! `Ok(false)` → the consumer skips the downstream XADD (exactly-once
//! publish — no double paper-trade downstream).
//!
//! Runs in the CI integration job (live PG + migrations 112/113 applied by
//! `automation/scripts/migrate.sh` before `cargo test`). Outside CI without
//! `DATABASE_URL` it skips loudly — fail-honest, never fabricated.
//!
//! If migration 113 is NOT applied, PG rejects the ON CONFLICT target
//! ("no unique or exclusion constraint matching") and this test FAILS —
//! that is the intended loud guard, not a flake.

#[path = "../src/persistence.rs"]
mod persistence;

use chrono::Utc;
use persistence::insert_simulation;
use shared_rs::contracts::{SimulationResult, SimulatorKind};
use sqlx::postgres::PgPool;
use uuid::Uuid;

const WETH: &str = "0xC02AAA39b223FE8D0A0e5C4F27eAD9083C756Cc2";
const USDC: &str = "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48";

async fn seed_opportunity(pool: &PgPool) -> Uuid {
    let route_metadata = serde_json::json!({
        "pool_addresses": ["0xB4e16d0168e52d35CaCD2c6185b44281Ec28C9Dc"],
        "token_addresses": [WETH, USDC],
        "dex_adapters": ["uniswap-v2"],
        "decimals": {"map": {}}
    });
    sqlx::query_scalar(
        r#"
        INSERT INTO opportunities (
            chain_id, strategy_kind, dex_a, token_in, token_out,
            amount_in_wei, status, trace_id, route_metadata
        ) VALUES (1, 'dex_arb', 'uniswap-v2', $1, $2, $3, 'validated', gen_random_uuid(), $4)
        RETURNING id
        "#,
    )
    .bind(WETH)
    .bind(USDC)
    .bind(1_000_000_000_000_000_000i64)
    .bind(sqlx::types::Json(&route_metadata))
    .fetch_one(pool)
    .await
    .expect("seed opportunity")
}

struct DropRow(PgPool, Uuid);
impl Drop for DropRow {
    fn drop(&mut self) {
        let pool = self.0.clone();
        let id = self.1;
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                let _ = sqlx::query("DELETE FROM simulations WHERE opportunity_id = $1")
                    .bind(id)
                    .execute(&pool)
                    .await;
                let _ = sqlx::query("DELETE FROM opportunities WHERE id = $1")
                    .bind(id)
                    .execute(&pool)
                    .await;
            })
        });
    }
}

fn revm_result(opp: Uuid, passed: bool, fail_reason: Option<&str>) -> SimulationResult {
    SimulationResult {
        opportunity_id: opp,
        passed,
        gas_estimate_wei: Some("300000".to_string()),
        gas_price_wei: Some("20000000000".to_string()),
        slippage_pct: None,
        revert_risk_pct: None,
        simulated_profit_usd: None,
        simulator: SimulatorKind::Revm,
        fail_reason: fail_reason.map(str::to_string),
        simulated_at: Utc::now(),
        trace_id: Uuid::new_v4(),
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn revm_redelivery_persists_once_and_reports_the_duplicate() {
    let db_url = std::env::var("DATABASE_URL").unwrap_or_default();
    if db_url.is_empty() {
        eprintln!(
            "SKIP: DATABASE_URL not set — idempotency test needs live PG with migrations 112+113"
        );
        return;
    }
    let pool: PgPool =
        shared_rs::db_pool::options_with_timeouts(&shared_rs::db_pool::PoolConfig::from_env(2))
            .connect(&db_url)
            .await
            .expect("PG connect");
    let opp_id = seed_opportunity(&pool).await;
    let _guard = DropRow(pool.clone(), opp_id);

    // ---- Delivery 1: the crash scenario's FIRST successful persist. ----
    // Market rejection (a chain-state verdict) — persists the row AND flips
    // the opportunity to 'rejected'.
    let first = revm_result(
        opp_id,
        false,
        Some("multistep_call_revert:SIMWIRE-02C-FIXTURE"),
    );
    let first_trace = first.trace_id;
    let fresh = insert_simulation(&pool, &first)
        .await
        .expect("first delivery must persist");
    assert!(fresh, "first delivery must report inserted_fresh=true");

    // ---- Delivery 2: XAUTOCLAIM redelivery of the SAME verdict slot ----
    // (persist+XADD landed, final XACK failed). A different trace_id
    // simulates a genuinely re-executed attempt — idempotency must hold
    // regardless.
    let second = revm_result(opp_id, false, Some("multistep_call_revert:REDELIVERY"));
    let dup = insert_simulation(&pool, &second)
        .await
        .expect("redelivery must not error — it is a duplicate, not a fault");
    assert!(
        !dup,
        "redelivery must report inserted_fresh=false so the consumer skips the XADD"
    );

    // ---- Exactly-once evidence ----
    let (rows, kept_trace, kept_reason): (i64, Uuid, Option<String>) = sqlx::query_as(
        "SELECT COUNT(*), (ARRAY_AGG(trace_id::text ORDER BY simulated_at))[1]::uuid, \
         (ARRAY_AGG(fail_reason ORDER BY simulated_at))[1] \
         FROM simulations WHERE opportunity_id = $1 AND simulator = 'revm'",
    )
    .bind(opp_id)
    .fetch_one(&pool)
    .await
    .expect("inspect simulations");
    assert_eq!(rows, 1, "exactly one revm verdict row — no double persist");
    assert_eq!(
        kept_trace, first_trace,
        "DO NOTHING must keep the FIRST verdict's trace_id (not the redelivery's)"
    );
    assert_eq!(
        kept_reason.as_deref(),
        Some("multistep_call_revert:SIMWIRE-02C-FIXTURE"),
        "DO NOTHING must keep the FIRST verdict's fail_reason"
    );

    // The duplicate return also skipped the status UPDATE path — the row
    // already carries the terminal state from delivery 1.
    let status: String = sqlx::query_scalar("SELECT status FROM opportunities WHERE id = $1")
        .bind(opp_id)
        .fetch_one(&pool)
        .await
        .expect("opp status");
    assert_eq!(status, "rejected", "delivery 1's rejection stands");
}

#[tokio::test(flavor = "multi_thread")]
async fn anvil_attempts_are_not_deduped_by_the_revm_index() {
    // The partial index is deliberately WHERE simulator='revm': legacy
    // anvil history legitimately holds MULTIPLE attempts per opportunity
    // (per-attempt diagnostics). Two anvil rows must BOTH land as fresh.
    let db_url = std::env::var("DATABASE_URL").unwrap_or_default();
    if db_url.is_empty() {
        eprintln!("SKIP: DATABASE_URL not set — anvil multi-attempt case needs live PG");
        return;
    }
    let pool: PgPool =
        shared_rs::db_pool::options_with_timeouts(&shared_rs::db_pool::PoolConfig::from_env(2))
            .connect(&db_url)
            .await
            .expect("PG connect");
    let opp_id = seed_opportunity(&pool).await;
    let _guard = DropRow(pool.clone(), opp_id);

    let mk = |passed: bool, reason: Option<&str>| SimulationResult {
        opportunity_id: opp_id,
        passed,
        gas_estimate_wei: Some("250000".to_string()),
        gas_price_wei: Some("20000000000".to_string()),
        slippage_pct: None,
        revert_risk_pct: None,
        simulated_profit_usd: None,
        simulator: SimulatorKind::Anvil,
        fail_reason: reason.map(str::to_string),
        simulated_at: Utc::now(),
        trace_id: Uuid::new_v4(),
    };

    let a = insert_simulation(&pool, &mk(false, Some("gas_floor_breach")))
        .await
        .expect("anvil attempt 1");
    let b = insert_simulation(&pool, &mk(true, None))
        .await
        .expect("anvil attempt 2");
    assert!(a, "anvil attempt 1 must be fresh");
    assert!(
        b,
        "anvil attempt 2 must ALSO be fresh — the idempotency index is revm-only"
    );

    let rows: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM simulations WHERE opportunity_id = $1 AND simulator = 'anvil'",
    )
    .bind(opp_id)
    .fetch_one(&pool)
    .await
    .expect("count anvil rows");
    assert_eq!(
        rows, 2,
        "both anvil attempts persisted (per-attempt diagnostics)"
    );
}
