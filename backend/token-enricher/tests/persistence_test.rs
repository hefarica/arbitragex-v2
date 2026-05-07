//! Integration tests for `token_enricher::persistence`.
//!
//! These tests use `#[sqlx::test(migrations = false)]`, which provisions a
//! fresh PostgreSQL test database per test but skips automatic migration
//! discovery. We then apply only the migrations needed for the `tokens`
//! table by hand via inline SQL.
//!
//! Why not `migrations = "../../database/migrations"`? sqlx 0.7 scans the
//! migrations directory at compile time and rejects filenames that don't
//! match `<integer>_name.sql`. The repo contains `001b_role_passwords.sql`
//! (a runtime-only password-setting migration) which fails that parser.
//! Renaming that file would change schema-deploy ordering and is outside
//! Task 6's scope, so we apply migrations programmatically instead.
//!
//! Why not `include_str!("../../../database/migrations/001_roles.sql")`?
//! That file ends with `GRANT CONNECT ON DATABASE arbitragex TO ...`, which
//! references the production database name. The sqlx::test ephemeral
//! databases are named `_sqlx_test_*`, causing PG to return "database
//! arbitragex does not exist" and aborting ALL tests at migration setup.
//! The fix is a test-local inline constant that creates the roles only
//! (no GRANT CONNECT, no CREATE EXTENSION pgcrypto) — production migrations
//! are NOT modified (that would break the deployed VPS).
//!
//! At TEST RUNTIME these tests still need a reachable Postgres (the
//! `#[sqlx::test]` macro spins up a temp database per test). On a developer
//! box without local PG, `cargo check --tests` exercises macro expansion and
//! type-checks; actual execution is delegated to CI / VPS.

use alloy_primitives::Address;
use sqlx::Row;
use std::str::FromStr;
use token_enricher::persistence::{upsert_token, ResolvedToken};

/// Minimum role SQL for the test cluster.
///
/// We create only the three roles referenced by migration 034's GRANT
/// statements. We intentionally omit:
///   - `CREATE EXTENSION pgcrypto` — not used by 034, requires superuser on
///     some cluster configs, irrelevant to token persistence logic.
///   - `GRANT CONNECT ON DATABASE arbitragex` — the ephemeral test DB is
///     named `_sqlx_test_*`; this GRANT would always fail with "database
///     arbitragex does not exist".
///   - `GRANT USAGE ON SCHEMA public` — test pool runs as superuser and
///     doesn't need it.
///
/// The `DO $$ EXCEPTION WHEN duplicate_object` idiom is cluster-level
/// idempotent: roles persist across databases in the same PG instance. If a
/// previous test in the same session already created these roles, the
/// EXCEPTION block silently no-ops rather than failing.
const TEST_MIGRATION_001_ROLES: &str = r#"
DO $$ BEGIN CREATE ROLE arbx_migrator WITH LOGIN; EXCEPTION WHEN duplicate_object THEN NULL; END $$;
DO $$ BEGIN CREATE ROLE arbx_rw       WITH LOGIN; EXCEPTION WHEN duplicate_object THEN NULL; END $$;
DO $$ BEGIN CREATE ROLE arbx_ro       WITH LOGIN; EXCEPTION WHEN duplicate_object THEN NULL; END $$;
"#;

/// The tokens table schema and its indexes, verbatim from migration 034.
const MIGRATION_034_TOKENS: &str =
    include_str!("../../../database/migrations/034_tokens_table.sql");

/// Apply the minimum migrations required for `tokens` table tests.
async fn apply_token_migrations(pool: &sqlx::PgPool) -> sqlx::Result<()> {
    sqlx::raw_sql(TEST_MIGRATION_001_ROLES).execute(pool).await?;
    sqlx::raw_sql(MIGRATION_034_TOKENS).execute(pool).await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Original 3 tests (preserved verbatim from cf62095)
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// IMPORTANT-2: promotion test — verifies IMPORTANT-1 fix in persistence.rs
// ---------------------------------------------------------------------------

/// Verifies that when a `failed` row is promoted to `onchain_full`:
///   - `resolved_via` is ratcheted up (the ratchet logic from Task 6),
///   - all nullable fields are populated,
///   - `resolved_at` is advanced (IMPORTANT-1 fix: was not updated before).
#[sqlx::test(migrations = false)]
async fn upsert_promotes_failed_to_onchain_full(pool: sqlx::PgPool) -> sqlx::Result<()> {
    apply_token_migrations(&pool).await?;
    let addr = Address::from_str(&format!("0x{}", "b".repeat(40))).unwrap();

    // 1) Insert as failed.
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

    let r1 = sqlx::query(
        "SELECT resolved_via, resolved_at FROM tokens WHERE chain_id=1",
    )
    .fetch_one(&pool)
    .await?;
    let resolved_via_1: String = r1.get("resolved_via");
    let resolved_at_1: chrono::DateTime<chrono::Utc> = r1.get("resolved_at");
    assert_eq!(resolved_via_1, "failed");

    // Sleep to ensure NOW() advances enough for the assertion below.
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // 2) Upsert with full resolution — should promote.
    upsert_token(
        &pool,
        1,
        addr,
        ResolvedToken {
            symbol: Some("FOO".into()),
            decimals: Some(18),
            logo_url: Some("https://example.com/foo.png".into()),
            resolved_via: "onchain_full",
        },
    )
    .await
    .unwrap();

    let r2 = sqlx::query(
        "SELECT resolved_via, symbol, decimals, logo_url, resolved_at FROM tokens WHERE chain_id=1",
    )
    .fetch_one(&pool)
    .await?;
    let resolved_via_2: String = r2.get("resolved_via");
    let symbol_2: Option<String> = r2.get("symbol");
    let decimals_2: Option<i16> = r2.get("decimals");
    let logo_url_2: Option<String> = r2.get("logo_url");
    let resolved_at_2: chrono::DateTime<chrono::Utc> = r2.get("resolved_at");

    // Ratchet up: resolved_via promoted.
    assert_eq!(resolved_via_2, "onchain_full");
    // Fields populated.
    assert_eq!(symbol_2.as_deref(), Some("FOO"));
    assert_eq!(decimals_2, Some(18));
    assert_eq!(logo_url_2.as_deref(), Some("https://example.com/foo.png"));
    // IMPORTANT-1 invariant: resolved_at advances on failed→success promotion.
    assert!(
        resolved_at_2 > resolved_at_1,
        "resolved_at must advance on failed→success promotion"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// IMPORTANT-3: needs_resolution coverage (4 tests)
// ---------------------------------------------------------------------------

/// Token never seen → needs resolution.
#[sqlx::test(migrations = false)]
async fn needs_resolution_returns_true_for_unknown_token(
    pool: sqlx::PgPool,
) -> sqlx::Result<()> {
    apply_token_migrations(&pool).await?;
    let addr = Address::from_str(&format!("0x{}", "1".repeat(40))).unwrap();
    assert!(
        token_enricher::persistence::needs_resolution(&pool, 1, addr)
            .await
            .unwrap()
    );
    Ok(())
}

/// Token resolved via onchain_full → does NOT need resolution.
#[sqlx::test(migrations = false)]
async fn needs_resolution_returns_false_for_resolved_token(
    pool: sqlx::PgPool,
) -> sqlx::Result<()> {
    apply_token_migrations(&pool).await?;
    let addr = Address::from_str(&format!("0x{}", "2".repeat(40))).unwrap();
    upsert_token(
        &pool,
        1,
        addr,
        ResolvedToken {
            symbol: Some("X".into()),
            decimals: Some(6),
            logo_url: None,
            resolved_via: "onchain_full",
        },
    )
    .await
    .unwrap();
    assert!(
        !token_enricher::persistence::needs_resolution(&pool, 1, addr)
            .await
            .unwrap()
    );
    Ok(())
}

/// Token recently failed (age ~0ms, well under the 7-day TTL) → does NOT
/// need resolution yet (cooling-down period).
#[sqlx::test(migrations = false)]
async fn needs_resolution_returns_false_for_recently_failed(
    pool: sqlx::PgPool,
) -> sqlx::Result<()> {
    apply_token_migrations(&pool).await?;
    let addr = Address::from_str(&format!("0x{}", "3".repeat(40))).unwrap();
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
    // resolved_at is NOW(); age is ~0 — well under 7 days.
    assert!(
        !token_enricher::persistence::needs_resolution(&pool, 1, addr)
            .await
            .unwrap()
    );
    Ok(())
}

/// Token failed 8 days ago (backdated via direct SQL) → DOES need resolution
/// (TTL expired, retry window open).
///
/// This test validates the X4 fix: the TTL comparison runs entirely in PG
/// using `NOW() - INTERVAL '7 days'`, so backdating `resolved_at` via SQL
/// correctly triggers the retry condition regardless of the Rust process clock.
#[sqlx::test(migrations = false)]
async fn needs_resolution_returns_true_for_old_failed(
    pool: sqlx::PgPool,
) -> sqlx::Result<()> {
    apply_token_migrations(&pool).await?;
    let addr = Address::from_str(&format!("0x{}", "4".repeat(40))).unwrap();
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
    // Backdate resolved_at to 8 days ago via direct SQL to simulate an expired TTL.
    // Because needs_resolution compares against PG's NOW(), this reliably triggers
    // the retry condition without requiring the test to actually wait 7 days.
    sqlx::query(
        "UPDATE tokens SET resolved_at = NOW() - INTERVAL '8 days' WHERE chain_id=1",
    )
    .execute(&pool)
    .await?;
    assert!(
        token_enricher::persistence::needs_resolution(&pool, 1, addr)
            .await
            .unwrap()
    );
    Ok(())
}
