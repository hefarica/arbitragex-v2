//! Integration tests for `token_enricher::reconciliation`.
//!
//! Uses `#[sqlx::test(migrations = false)]` for the same reason as
//! `persistence_test.rs`: `sqlx 0.7` rejects the `001b_role_passwords.sql`
//! filename at compile time (non-numeric prefix).  We apply migrations
//! programmatically via `apply_reconciliation_migrations`.
//!
//! Migration order applied here:
//!   1. Inline `TEST_MIGRATION_001_ROLES` — creates arbx_rw / arbx_ro roles
//!      without `GRANT CONNECT ON DATABASE arbitragex` (would fail on the
//!      ephemeral `_sqlx_test_*` database) and without `CREATE EXTENSION
//!      pgcrypto` (needs superuser on some clusters; PG 13+ has built-in
//!      `gen_random_uuid()` used by the `opportunities` DEFAULT clause).
//!   2. `003_opportunities.sql` — creates the `opportunities` table.
//!   3. `033_opportunities_fail_honest_and_cross_chain_slots.sql` — ADDs
//!      `chain_id_out`, `bridge`, `bridge_fee_usd` columns referenced by
//!      `find_unresolved_tokens`'s COALESCE(chain_id_out, chain_id).
//!   4. `034_tokens_table.sql` — creates the `tokens` table.
//!
//! ## Address space
//!
//! Task 6 (`persistence_test.rs`) uses addresses based on hex chars:
//! `c02aaa...` (WETH), `a`×40, `b`×40, `f`×40, `1`×40, `2`×40, `3`×40,
//! `4`×40.  This file uses: `5`×40, `6`×40, `7`×40, `8`×40, `9`×40, and
//! specific mixed-case test vectors — all distinct.
//!
//! NOTE: tests require PostgreSQL 13+ for the built-in `gen_random_uuid()`
//! function used in opportunity inserts. Older PG versions need the pgcrypto
//! extension; the inline TEST_MIGRATION_001_ROLES intentionally does not
//! create that extension because PG 13+ provides gen_random_uuid() in core.

use sqlx::PgPool;
use token_enricher::reconciliation::find_unresolved_tokens;

/// Minimum role SQL for the test cluster.  Same idiom as persistence_test.rs.
const TEST_MIGRATION_001_ROLES: &str = r#"
DO $$ BEGIN CREATE ROLE arbx_migrator WITH LOGIN; EXCEPTION WHEN duplicate_object THEN NULL; END $$;
DO $$ BEGIN CREATE ROLE arbx_rw       WITH LOGIN; EXCEPTION WHEN duplicate_object THEN NULL; END $$;
DO $$ BEGIN CREATE ROLE arbx_ro       WITH LOGIN; EXCEPTION WHEN duplicate_object THEN NULL; END $$;
"#;

const MIGRATION_003_OPPORTUNITIES: &str =
    include_str!("../../../database/migrations/003_opportunities.sql");

const MIGRATION_033_CROSS_CHAIN: &str = include_str!(
    "../../../database/migrations/033_opportunities_fail_honest_and_cross_chain_slots.sql"
);

/// DDL-only equivalent of migration 034 for test databases.
///
/// Production migration 034 ends with GRANT statements. When passed to
/// `sqlx::raw_sql` as part of a multi-statement batch, those GRANTs abort
/// the implicit transaction and roll back the preceding CREATE TABLE, causing
/// PG 42P01 ("relation tokens does not exist") on every test that follows.
/// This inline constant omits the GRANTs — the sqlx::test pool runs as
/// superuser and needs no explicit grants.
/// Production `database/migrations/034_tokens_table.sql` is NOT modified.
const MIGRATION_034_TOKENS: &str = r#"
CREATE TABLE IF NOT EXISTS tokens (
  chain_id      INTEGER     NOT NULL,
  address       TEXT        NOT NULL,
  symbol        TEXT        NULL,
  decimals      SMALLINT    NULL,
  logo_url      TEXT        NULL,
  resolved_via  TEXT        NOT NULL
    CHECK (resolved_via IN ('onchain_full','onchain_partial','trustwallet_only','failed')),
  resolved_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  last_seen_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  PRIMARY KEY (chain_id, address),
  CONSTRAINT chk_address_format CHECK (address ~ '^0x[a-f0-9]{40}$')
);
CREATE INDEX IF NOT EXISTS idx_tokens_last_seen ON tokens(last_seen_at DESC);
"#;

/// Apply the minimum migrations required for reconciliation tests.
///
/// Order matters: roles first (GRANTs in 003/034 reference them), then
/// `opportunities`, then the ALTER that adds cross-chain columns (033),
/// then `tokens`.
async fn apply_reconciliation_migrations(pool: &PgPool) -> sqlx::Result<()> {
    sqlx::raw_sql(TEST_MIGRATION_001_ROLES)
        .execute(pool)
        .await?;
    sqlx::raw_sql(MIGRATION_003_OPPORTUNITIES)
        .execute(pool)
        .await?;
    sqlx::raw_sql(MIGRATION_033_CROSS_CHAIN)
        .execute(pool)
        .await?;
    sqlx::raw_sql(MIGRATION_034_TOKENS).execute(pool).await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Test 1 — token referenced by opportunities but absent from tokens table
// ---------------------------------------------------------------------------

/// An opportunity row referencing two addresses that have no entry in the
/// `tokens` table must be returned by `find_unresolved_tokens`.
///
/// Uses mixed-case input addresses to verify that the SQL `LOWER()` call
/// normalises them before the set-difference comparison.
#[sqlx::test(migrations = false)]
async fn finds_token_in_addresses_not_yet_in_tokens(pool: PgPool) -> sqlx::Result<()> {
    apply_reconciliation_migrations(&pool).await?;

    // Mixed-case addresses — LOWER() in the query normalises them to match the
    // tokens.address CHECK constraint (`^0x[a-f0-9]{40}$`).
    sqlx::query(
        r#"
        INSERT INTO opportunities
          (chain_id, strategy_kind, dex_a, token_in, token_out, amount_in_wei, trace_id)
        VALUES (1, 'dex_arb', 'uniswap-v2',
                '0xAAAaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaAAA',
                '0xbbbBBBbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbBBB',
                100, gen_random_uuid())
        "#,
    )
    .execute(&pool)
    .await?;

    let unresolved = find_unresolved_tokens(&pool, 100).await.unwrap();
    let addrs: Vec<String> = unresolved.iter().map(|(_, a)| a.clone()).collect();

    // The SQL applies LOWER() — we expect all-lowercase forms.
    assert!(
        addrs.contains(&"0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string()),
        "token_in (after LOWER) must appear in unresolved list; got: {addrs:?}"
    );
    assert!(
        addrs.contains(&"0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string()),
        "token_out (after LOWER) must appear in unresolved list; got: {addrs:?}"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Test 2 — token already successfully resolved must NOT be returned
// ---------------------------------------------------------------------------

/// An opportunity referencing a token that already has a non-failed row in
/// the `tokens` table must be excluded from `find_unresolved_tokens`.
#[sqlx::test(migrations = false)]
async fn skips_tokens_already_resolved(pool: PgPool) -> sqlx::Result<()> {
    apply_reconciliation_migrations(&pool).await?;

    let lc = "0xcccccccccccccccccccccccccccccccccccccccc";

    sqlx::query(
        r#"
        INSERT INTO opportunities
          (chain_id, strategy_kind, dex_a, token_in, token_out, amount_in_wei, trace_id)
        VALUES (1, 'dex_arb', 'uniswap-v2', $1, $1, 100, gen_random_uuid())
        "#,
    )
    .bind(lc)
    .execute(&pool)
    .await?;

    // Insert as successfully resolved (non-failed).
    sqlx::query(
        r#"INSERT INTO tokens (chain_id, address, resolved_via) VALUES (1, $1, 'onchain_full')"#,
    )
    .bind(lc)
    .execute(&pool)
    .await?;

    let unresolved = find_unresolved_tokens(&pool, 100).await.unwrap();
    assert!(
        !unresolved.iter().any(|(_, a)| a == lc),
        "already-resolved token must NOT appear in unresolved list"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Test 3 — TTL-expired failed token must be returned for retry (Defect #4)
// ---------------------------------------------------------------------------

/// A token with `resolved_via = 'failed'` whose `resolved_at` is older than
/// 7 days must be returned by `find_unresolved_tokens` — the TTL has expired
/// and it should be retried.
///
/// This validates the double-negation WHERE clause fix for Defect #4: a naive
/// `NOT EXISTS (SELECT 1 FROM tokens WHERE chain_id=... AND address=...)` would
/// permanently exclude failed tokens; the corrected clause re-includes them
/// once the TTL window opens.
#[sqlx::test(migrations = false)]
async fn includes_old_failed_for_retry(pool: PgPool) -> sqlx::Result<()> {
    apply_reconciliation_migrations(&pool).await?;

    let lc = "0xdddddddddddddddddddddddddddddddddddddddd";

    sqlx::query(
        r#"
        INSERT INTO opportunities
          (chain_id, strategy_kind, dex_a, token_in, token_out, amount_in_wei, trace_id)
        VALUES (1, 'dex_arb', 'uniswap-v2', $1, $1, 100, gen_random_uuid())
        "#,
    )
    .bind(lc)
    .execute(&pool)
    .await?;

    // Token previously failed, and the failure is 8 days old (TTL expired).
    sqlx::query(
        r#"
        INSERT INTO tokens (chain_id, address, resolved_via, resolved_at)
        VALUES (1, $1, 'failed', NOW() - INTERVAL '8 days')
        "#,
    )
    .bind(lc)
    .execute(&pool)
    .await?;

    let unresolved = find_unresolved_tokens(&pool, 100).await.unwrap();
    assert!(
        unresolved.iter().any(|(_, a)| a == lc),
        "TTL-expired failed token must be returned for retry; got: {unresolved:?}"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Test 4 — recently failed token must NOT be returned (TTL still active)
// ---------------------------------------------------------------------------

/// A token with `resolved_via = 'failed'` that failed recently (within the
/// 7-day cooling-off window) must NOT be returned — we respect the TTL.
///
/// This is the symmetric complement of Test 3 and ensures we don't hammer
/// tokens that legitimately failed minutes ago with immediate retries.
#[sqlx::test(migrations = false)]
async fn skips_recent_failed(pool: PgPool) -> sqlx::Result<()> {
    apply_reconciliation_migrations(&pool).await?;

    let lc = "0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee";

    sqlx::query(
        r#"
        INSERT INTO opportunities
          (chain_id, strategy_kind, dex_a, token_in, token_out, amount_in_wei, trace_id)
        VALUES (1, 'dex_arb', 'uniswap-v2', $1, $1, 100, gen_random_uuid())
        "#,
    )
    .bind(lc)
    .execute(&pool)
    .await?;

    // Token failed recently — `resolved_at` defaults to NOW(), well within
    // the 7-day window.
    sqlx::query(r#"INSERT INTO tokens (chain_id, address, resolved_via) VALUES (1, $1, 'failed')"#)
        .bind(lc)
        .execute(&pool)
        .await?;

    let unresolved = find_unresolved_tokens(&pool, 100).await.unwrap();
    assert!(
        !unresolved.iter().any(|(_, a)| a == lc),
        "recently-failed token must NOT be returned (TTL cooling-off); got: {unresolved:?}"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Test 5 — LIMIT parameter caps the number of returned rows
// ---------------------------------------------------------------------------

/// Inserts 10 distinct unresolved tokens and asserts that `limit=5` returns
/// exactly 5 rows.  Validates that the `LIMIT $1` binding is effective and
/// that the `limit <= 0` early-return guard does not suppress positive limits.
///
/// Address pattern `0x{:040x}` of `0x100..=0x109` produces
/// `0x0000000000000000000000000000000000000100` through `...0109` —
/// 10 distinct lowercase hex addresses that do not collide with any address
/// used in Tasks 6 or 7 (which use single-character-repeated patterns and
/// c02aaa…).
#[sqlx::test(migrations = false)]
async fn limit_caps_returned_rows(pool: sqlx::PgPool) -> sqlx::Result<()> {
    apply_reconciliation_migrations(&pool).await?;

    // Insert 10 distinct unresolved tokens.
    for i in 0..10u64 {
        let addr = format!("0x{:040x}", 0x100u64 + i);
        sqlx::query(
            r#"INSERT INTO opportunities
               (chain_id, strategy_kind, dex_a, token_in, token_out, amount_in_wei, trace_id)
               VALUES (1, 'dex_arb', 'uniswap-v2', $1, $1, 100, gen_random_uuid())"#,
        )
        .bind(&addr)
        .execute(&pool)
        .await?;
    }

    let unresolved = find_unresolved_tokens(&pool, 5).await.unwrap();
    assert_eq!(
        unresolved.len(),
        5,
        "LIMIT 5 must cap result count to 5 even with 10 unresolved rows; got: {}",
        unresolved.len()
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Test 6 — cross-chain token_out routes to destination chain via chain_id_out
// ---------------------------------------------------------------------------

/// Inserts a cross-chain opportunity with `chain_id=1` (Ethereum) and
/// `chain_id_out=137` (Polygon) and asserts that:
///   - `token_in` is returned with `chain_id = 1` (source chain).
///   - `token_out` is returned with `chain_id = 137` (destination chain via
///     `COALESCE(chain_id_out, chain_id)`).
///
/// This locks the non-NULL `chain_id_out` branch of `find_unresolved_tokens`
/// so future cross-chain work (Sub-Projeto D) cannot regress it silently.
///
/// `chk_cross_chain_distinct` requires `chain_id_out IS NULL OR chain_id_out
/// <> chain_id`; using 1 and 137 satisfies this constraint.
///
/// Addresses `0x1111…` and `0x2222…` (42 chars: prefix + 40 hex) are distinct
/// from all addresses used in prior tests.
#[sqlx::test(migrations = false)]
async fn cross_chain_token_out_routes_to_destination_chain(pool: sqlx::PgPool) -> sqlx::Result<()> {
    apply_reconciliation_migrations(&pool).await?;

    let token_in_addr = "0x1111111111111111111111111111111111111111";
    let token_out_addr = "0x2222222222222222222222222222222222222222";

    // Cross-chain opportunity: source chain 1 (Ethereum), destination chain 137 (Polygon).
    // `bridge` is TEXT NULL per migration 033 — omitting it lets it default NULL.
    sqlx::query(
        r#"INSERT INTO opportunities
           (chain_id, chain_id_out, strategy_kind, dex_a, token_in, token_out, amount_in_wei, trace_id)
           VALUES (1, 137, 'dex_arb', 'uniswap-v2', $1, $2, 100, gen_random_uuid())"#,
    )
    .bind(token_in_addr)
    .bind(token_out_addr)
    .execute(&pool)
    .await?;

    let unresolved = find_unresolved_tokens(&pool, 100).await.unwrap();

    // token_in lives on source chain 1.
    assert!(
        unresolved
            .iter()
            .any(|(c, a)| *c == 1 && a == token_in_addr),
        "token_in must be returned with source chain_id=1; got: {unresolved:?}"
    );
    // token_out lives on destination chain 137 (via COALESCE(chain_id_out, chain_id)).
    assert!(
        unresolved
            .iter()
            .any(|(c, a)| *c == 137 && a == token_out_addr),
        "token_out must be returned with destination chain_id_out=137; got: {unresolved:?}"
    );
    Ok(())
}
