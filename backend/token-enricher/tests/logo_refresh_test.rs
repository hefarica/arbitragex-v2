//! Integration tests for `token_enricher::logo_refresh` (RU-TOKEN-REFRESH).
//!
//! Same harness contract as `persistence_test.rs`: `#[sqlx::test(migrations =
//! false)]` provisions a fresh PostgreSQL per test; the minimum `tokens` DDL is
//! applied inline (production migrations are NOT modified — see that file for
//! the full rationale on 001/034).
//!
//! HTTP is served by mockito (already a dev-dependency) — the REAL
//! `TrustWalletClient` reqwest client probes it, so the tests exercise the
//! production HEAD/GET path end-to-end with NO runtime mocks (RULE 00).
//!
//! Redis assertions run only when a Redis is reachable (the CI
//! integration-tests workflow provisions one via REDIS_URL). Without Redis the
//! job must still pass — degraded to liveness-only mode is a designed behavior.
//!
//! At TEST RUNTIME these tests need a reachable Postgres (`#[sqlx::test]`) —
//! on a box without local PG, `cargo check --tests` still type-checks them;
//! execution is delegated to CI / VPS.

use token_enricher::logo_refresh::{run_logo_refresh, RefreshStats};
use token_enricher::trustwallet::TrustWalletClient;

/// Minimum role SQL for the test cluster (identical to persistence_test.rs).
const TEST_MIGRATION_001_ROLES: &str = r#"
DO $$ BEGIN CREATE ROLE arbx_migrator WITH LOGIN; EXCEPTION WHEN duplicate_object THEN NULL; END $$;
DO $$ BEGIN CREATE ROLE arbx_rw       WITH LOGIN; EXCEPTION WHEN duplicate_object THEN NULL; END $$;
DO $$ BEGIN CREATE ROLE arbx_ro       WITH LOGIN; EXCEPTION WHEN duplicate_object THEN NULL; END $$;
"#;

/// DDL-only equivalent of migration 034 for test databases (no GRANTs).
const MIGRATION_034_TOKENS_DDL: &str = r#"
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

/// Contract-literal of the (private) `logo_refresh::SIZES_HASH` const. Keeping
/// the literal here means a rename in the module must consciously update the
/// tests too — the Redis baseline layout is part of the job's contract.
const SIZES_HASH: &str = "arbx:token-logo-refresh:sizes";

async fn apply_token_migrations(pool: &sqlx::PgPool) -> sqlx::Result<()> {
    sqlx::raw_sql(TEST_MIGRATION_001_ROLES)
        .execute(pool)
        .await?;
    sqlx::raw_sql(MIGRATION_034_TOKENS_DDL)
        .execute(pool)
        .await?;
    Ok(())
}

/// Seed one token: resolved 30 days ago, last seen 1 day ago. `logo_url=None`
/// seeds a logo-less row.
async fn seed_token(pool: &sqlx::PgPool, chain_id: i32, addr: &str, logo_url: Option<&str>) {
    sqlx::query(
        r#"INSERT INTO tokens
             (chain_id, address, symbol, decimals, logo_url, resolved_via,
              resolved_at, last_seen_at)
           VALUES ($1, $2, 'TST', 18, $3, 'onchain_full',
                   NOW() - INTERVAL '30 days', NOW() - INTERVAL '1 day')"#,
    )
    .bind(chain_id)
    .bind(addr)
    .bind(logo_url)
    .execute(pool)
    .await
    .unwrap();
}

// ---------------------------------------------------------------------------
// Redis helpers (cmd-based — stable across redis crate minor API drift)
// ---------------------------------------------------------------------------

fn test_redis_url() -> String {
    std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".into())
}

/// Connect if a Redis is reachable; `None` → the test skips Redis assertions.
async fn try_redis() -> Option<redis::aio::MultiplexedConnection> {
    let client = redis::Client::open(test_redis_url().as_str()).ok()?;
    client.get_multiplexed_async_connection().await.ok()
}

fn icon_key(chain_id: i32, addr: &str) -> String {
    format!("arbx:token-icons:{chain_id}:{addr}")
}

fn baseline_field(chain_id: i32, addr: &str) -> String {
    format!("{chain_id}:{addr}")
}

async fn redis_del(conn: &mut redis::aio::MultiplexedConnection, keys: &[String]) {
    let mut cmd = redis::cmd("DEL");
    for k in keys {
        cmd.arg(k);
    }
    let _: redis::RedisResult<i64> = cmd.query_async(conn).await;
}

async fn redis_set_icon(conn: &mut redis::aio::MultiplexedConnection, key: &str, url: &str) {
    let _: redis::RedisResult<()> = redis::cmd("SET").arg(key).arg(url).query_async(conn).await;
}

async fn redis_exists(conn: &mut redis::aio::MultiplexedConnection, key: &str) -> Option<bool> {
    redis::cmd("EXISTS")
        .arg(key)
        .query_async::<_, i64>(conn)
        .await
        .ok()
        .map(|n| n == 1)
}

async fn redis_hset(conn: &mut redis::aio::MultiplexedConnection, field: &str, val: &str) {
    let _: redis::RedisResult<()> = redis::cmd("HSET")
        .arg(SIZES_HASH)
        .arg(field)
        .arg(val)
        .query_async(conn)
        .await;
}

async fn redis_hdel(conn: &mut redis::aio::MultiplexedConnection, fields: &[String]) {
    let mut cmd = redis::cmd("HDEL");
    cmd.arg(SIZES_HASH);
    for f in fields {
        cmd.arg(f);
    }
    let _: redis::RedisResult<i64> = cmd.query_async(conn).await;
}

async fn redis_hget(
    conn: &mut redis::aio::MultiplexedConnection,
    field: &str,
) -> Option<Option<String>> {
    redis::cmd("HGET")
        .arg(SIZES_HASH)
        .arg(field)
        .query_async::<_, Option<String>>(conn)
        .await
        .ok()
}

/// Distinct lowercase 40-hex addresses (`0x` + 39 chars + one digit).
const ADDR_A: &str = "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa1";
const ADDR_B: &str = "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb2";
const ADDR_C: &str = "0xccccccccccccccccccccccccccccccccccccccc3";
const ADDR_D: &str = "0xddddddddddddddddddddddddddddddddddddddd4";
const ADDR_E: &str = "0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee5";

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// Mixed-outcome pass: healthy logo survives untouched, dead logo is NULLed
/// + marked failed, rate-limited (403) logo survives untouched (fail-safe:
/// a rate-limit blip must never mass-NULL logos). A logo-less row must not be
/// selected at all.
#[sqlx::test(migrations = false)]
async fn logo_refresh_nulls_dead_logos_only(pool: sqlx::PgPool) -> sqlx::Result<()> {
    apply_token_migrations(&pool).await?;

    let mut server = mockito::Server::new_async().await;
    // A: healthy, Content-Length matches the baseline → unchanged.
    let mock_a = server
        .mock("HEAD", "/a.png")
        .with_status(200)
        .with_header("content-length", "100")
        .create_async()
        .await;
    // B: logo removed upstream → 404 → dead.
    let mock_b = server
        .mock("HEAD", "/b.png")
        .with_status(404)
        .create_async()
        .await;
    // C: rate-limited → probe error, logo untouched.
    let mock_c = server
        .mock("HEAD", "/c.png")
        .with_status(403)
        .create_async()
        .await;

    let url_a = format!("{}/a.png", server.url());
    let url_b = format!("{}/b.png", server.url());
    let url_c = format!("{}/c.png", server.url());

    seed_token(&pool, 1, ADDR_A, Some(&url_a)).await;
    seed_token(&pool, 1, ADDR_B, Some(&url_b)).await;
    seed_token(&pool, 1, ADDR_C, Some(&url_c)).await;
    // Logo-less row: must NOT be selected (logo_url IS NULL).
    seed_token(&pool, 1, ADDR_E, None).await;

    let mut redis = try_redis().await;
    if let Some(conn) = redis.as_mut() {
        redis_del(
            conn,
            [
                icon_key(1, ADDR_A),
                icon_key(1, ADDR_B),
                icon_key(1, ADDR_C),
            ]
            .as_ref(),
        )
        .await;
        redis_hdel(
            conn,
            [
                baseline_field(1, ADDR_A),
                baseline_field(1, ADDR_B),
                baseline_field(1, ADDR_C),
            ]
            .as_ref(),
        )
        .await;
        // Baseline for A matches the HEAD Content-Length → unchanged.
        redis_hset(conn, &baseline_field(1, ADDR_A), "100").await;
        redis_set_icon(conn, &icon_key(1, ADDR_A), &url_a).await;
        redis_set_icon(conn, &icon_key(1, ADDR_B), &url_b).await;
        redis_set_icon(conn, &icon_key(1, ADDR_C), &url_c).await;
    }

    let tw = TrustWalletClient::new(None).unwrap();
    let stats = run_logo_refresh(&pool, &tw, &test_redis_url())
        .await
        .unwrap();

    assert_eq!(
        stats,
        RefreshStats {
            checked: 3, // ADDR_E (logo-less) was not selected
            updated: 0,
            unchanged: 1,
            dead: 1,
            errors: 1,
        }
    );

    // B: dead → logo NULLed + failed.
    let (logo_b, via_b): (Option<String>, String) =
        sqlx::query_as("SELECT logo_url, resolved_via FROM tokens WHERE address = $1")
            .bind(ADDR_B)
            .fetch_one(&pool)
            .await?;
    assert!(logo_b.is_none(), "dead logo must be NULLed");
    assert_eq!(via_b, "failed");

    // A: unchanged → row completely untouched (still backdated).
    let (logo_a, via_a, stale_a): (Option<String>, String, bool) = sqlx::query_as(
        "SELECT logo_url, resolved_via, last_seen_at < NOW() - INTERVAL '12 hours' \
         FROM tokens WHERE address = $1",
    )
    .bind(ADDR_A)
    .fetch_one(&pool)
    .await?;
    assert_eq!(logo_a.as_deref(), Some(url_a.as_str()));
    assert_eq!(via_a, "onchain_full");
    assert!(
        stale_a,
        "unchanged token must NOT have last_seen_at advanced"
    );

    // C: error → row completely untouched.
    let (logo_c, via_c): (Option<String>, String) =
        sqlx::query_as("SELECT logo_url, resolved_via FROM tokens WHERE address = $1")
            .bind(ADDR_C)
            .fetch_one(&pool)
            .await?;
    assert_eq!(logo_c.as_deref(), Some(url_c.as_str()));
    assert_eq!(via_c, "onchain_full");

    if let Some(conn) = redis.as_mut() {
        // Icon hot-cache: invalidated ONLY for the dead logo.
        assert_eq!(redis_exists(conn, &icon_key(1, ADDR_A)).await, Some(true));
        assert_eq!(redis_exists(conn, &icon_key(1, ADDR_B)).await, Some(false));
        assert_eq!(redis_exists(conn, &icon_key(1, ADDR_C)).await, Some(true));
        // Baselines: A kept at 100, B's field removed by the dead branch.
        assert_eq!(
            redis_hget(conn, &baseline_field(1, ADDR_A)).await,
            Some(Some("100".into()))
        );
        assert_eq!(
            redis_hget(conn, &baseline_field(1, ADDR_B)).await,
            Some(None)
        );
    }

    mock_a.assert();
    mock_b.assert();
    mock_c.assert();
    Ok(())
}

/// Changed asset: Content-Length differs from the baseline → re-download
/// confirms → PG row re-affirmed (last_seen_at advanced, URL kept) + Redis
/// icon cache invalidated + baseline moved to the new length.
#[sqlx::test(migrations = false)]
async fn logo_refresh_updates_changed_logo_and_invalidates_cache(
    pool: sqlx::PgPool,
) -> sqlx::Result<()> {
    apply_token_migrations(&pool).await?;

    let mut server = mockito::Server::new_async().await;
    // HEAD says the asset grew (re-brand replaced the bytes).
    let mock_head = server
        .mock("HEAD", "/d.png")
        .with_status(200)
        .with_header("content-length", "2048")
        .create_async()
        .await;
    // Re-download confirmation: 2048 real bytes.
    let mock_get = server
        .mock("GET", "/d.png")
        .with_status(200)
        .with_body(vec![b'x'; 2048])
        .create_async()
        .await;

    let url_d = format!("{}/d.png", server.url());
    seed_token(&pool, 1, ADDR_D, Some(&url_d)).await;

    let mut redis = try_redis().await;
    if let Some(conn) = redis.as_mut() {
        redis_del(conn, &[icon_key(1, ADDR_D)]).await;
        redis_hdel(conn, [baseline_field(1, ADDR_D)].as_ref()).await;
        // Old baseline: the original 1024-byte asset.
        redis_hset(conn, &baseline_field(1, ADDR_D), "1024").await;
        redis_set_icon(conn, &icon_key(1, ADDR_D), &url_d).await;
    }

    let tw = TrustWalletClient::new(None).unwrap();
    let stats = run_logo_refresh(&pool, &tw, &test_redis_url())
        .await
        .unwrap();

    assert_eq!(
        stats,
        RefreshStats {
            checked: 1,
            updated: 1,
            unchanged: 0,
            dead: 0,
            errors: 0,
        }
    );

    // PG: URL re-affirmed, resolved_via untouched, last_seen_at advanced.
    let (logo_d, via_d, fresh_d): (Option<String>, String, bool) = sqlx::query_as(
        "SELECT logo_url, resolved_via, last_seen_at > NOW() - INTERVAL '2 hours' \
         FROM tokens WHERE address = $1",
    )
    .bind(ADDR_D)
    .fetch_one(&pool)
    .await?;
    assert_eq!(logo_d.as_deref(), Some(url_d.as_str()));
    assert_eq!(via_d, "onchain_full");
    assert!(fresh_d, "updated token must have last_seen_at advanced");

    if let Some(conn) = redis.as_mut() {
        // Icon hot-cache invalidated → the api-server cascade re-reads PG.
        assert_eq!(redis_exists(conn, &icon_key(1, ADDR_D)).await, Some(false));
        // Baseline moved to the new Content-Length.
        assert_eq!(
            redis_hget(conn, &baseline_field(1, ADDR_D)).await,
            Some(Some("2048".into()))
        );
    }

    mock_head.assert();
    mock_get.assert();
    Ok(())
}
