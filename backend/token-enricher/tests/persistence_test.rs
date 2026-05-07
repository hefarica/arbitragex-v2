//! Integration tests for `token_enricher::persistence`.
//!
//! These tests use `#[sqlx::test(migrations = false)]`, which provisions a
//! fresh PostgreSQL test database per test but skips automatic migration
//! discovery. We then apply only the migrations needed for the `tokens`
//! table by hand via `include_str!`.
//!
//! Why not `migrations = "../../database/migrations"`? sqlx 0.7 scans the
//! migrations directory at compile time and rejects filenames that don't
//! match `<integer>_name.sql`. The repo contains `001b_role_passwords.sql`
//! (a runtime-only password-setting migration) which fails that parser.
//! Renaming that file would change schema-deploy ordering and is outside
//! Task 6's scope, so we apply migrations programmatically instead.
//!
//! At TEST RUNTIME these tests still need a reachable Postgres (the
//! `#[sqlx::test]` macro spins up a temp database per test). On a developer
//! box without local PG, `cargo check --tests` exercises macro expansion and
//! type-checks; actual execution is delegated to CI / VPS.

use alloy_primitives::Address;
use sqlx::Row;
use std::str::FromStr;
use token_enricher::persistence::{upsert_token, ResolvedToken};

/// SQL needed to bring a fresh test DB up to migration 034 (the `tokens`
/// table). We bundle the SQL into the test binary at compile time so the
/// tests are self-contained and don't depend on filesystem layout at runtime.
const MIGRATION_001_ROLES: &str = include_str!("../../../database/migrations/001_roles.sql");
const MIGRATION_034_TOKENS: &str = include_str!("../../../database/migrations/034_tokens_table.sql");

/// Apply the minimum migrations required for `tokens` table tests.
async fn apply_token_migrations(pool: &sqlx::PgPool) -> sqlx::Result<()> {
    sqlx::raw_sql(MIGRATION_001_ROLES).execute(pool).await?;
    sqlx::raw_sql(MIGRATION_034_TOKENS).execute(pool).await?;
    Ok(())
}

#[sqlx::test(migrations = false)]
async fn upsert_token_inserts_lowercase_address(pool: sqlx::PgPool) -> sqlx::Result<()> {
    apply_token_migrations(&pool).await?;

    let weth = Address::from_str("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2").unwrap();
    upsert_token(
        &pool,
        1,
        weth,
        ResolvedToken {
            symbol: Some("WETH".into()),
            decimals: Some(18),
            logo_url: Some("https://example.com/weth.png".into()),
            resolved_via: "onchain_full",
        },
    )
    .await
    .unwrap();

    let row = sqlx::query(
        "SELECT address, symbol, decimals, resolved_via FROM tokens WHERE chain_id=$1",
    )
    .bind(1_i32)
    .fetch_one(&pool)
    .await?;
    let address: String = row.get("address");
    let symbol: Option<String> = row.get("symbol");
    let decimals: Option<i16> = row.get("decimals");
    let resolved_via: String = row.get("resolved_via");
    assert_eq!(address, "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2");
    assert_eq!(symbol.as_deref(), Some("WETH"));
    assert_eq!(decimals, Some(18));
    assert_eq!(resolved_via, "onchain_full");
    Ok(())
}

#[sqlx::test(migrations = false)]
async fn upsert_token_idempotent_updates_last_seen(pool: sqlx::PgPool) -> sqlx::Result<()> {
    apply_token_migrations(&pool).await?;

    // Defect #1 fix: `&str + &str` does not compile — must use `format!`.
    let addr = Address::from_str(&format!("0x{}", "a".repeat(40))).unwrap();
    let r = ResolvedToken {
        symbol: Some("X".into()),
        decimals: Some(6),
        logo_url: None,
        resolved_via: "onchain_full",
    };
    upsert_token(&pool, 1, addr, r.clone()).await.unwrap();
    let first_row = sqlx::query("SELECT last_seen_at FROM tokens WHERE chain_id=$1")
        .bind(1_i32)
        .fetch_one(&pool)
        .await?;
    let first: chrono::DateTime<chrono::Utc> = first_row.get("last_seen_at");

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    upsert_token(&pool, 1, addr, r).await.unwrap();
    let second_row = sqlx::query("SELECT last_seen_at FROM tokens WHERE chain_id=$1")
        .bind(1_i32)
        .fetch_one(&pool)
        .await?;
    let second: chrono::DateTime<chrono::Utc> = second_row.get("last_seen_at");
    assert!(second > first);
    Ok(())
}

#[sqlx::test(migrations = false)]
async fn upsert_with_failed_status_persists(pool: sqlx::PgPool) -> sqlx::Result<()> {
    apply_token_migrations(&pool).await?;

    let addr = Address::from_str(&format!("0x{}", "f".repeat(40))).unwrap();
    upsert_token(
        &pool,
        1,
        addr,
        ResolvedToken {
            symbol: None,
            decimals: None,
            logo_url: None,
            resolved_via: "failed",
        },
    )
    .await
    .unwrap();
    let row = sqlx::query(
        "SELECT symbol, decimals, logo_url, resolved_via FROM tokens WHERE chain_id=$1",
    )
    .bind(1_i32)
    .fetch_one(&pool)
    .await?;
    let symbol: Option<String> = row.get("symbol");
    let decimals: Option<i16> = row.get("decimals");
    let logo_url: Option<String> = row.get("logo_url");
    let resolved_via: String = row.get("resolved_via");
    assert!(symbol.is_none() && decimals.is_none() && logo_url.is_none());
    assert_eq!(resolved_via, "failed");
    Ok(())
}
