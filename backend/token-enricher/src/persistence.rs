//! PostgreSQL persistence for the token registry.
//!
//! The `tokens` table (migration 034) stores enrichment results with R8
//! fail-honest semantics: `symbol`/`decimals`/`logo_url` are nullable and
//! `resolved_via` distinguishes how the data was obtained (or that it failed).
//!
//! NOTE: We use the runtime `sqlx::query` / `sqlx::query_as` API rather than
//! the compile-time-checked `sqlx::query!` macros. Reason: the macros require
//! either a live `DATABASE_URL` at compile time or a committed `.sqlx/` offline
//! cache, neither of which exists in this repo. The trade-off is acceptable
//! because the SQL statements here are short, the column types are tightly
//! known, and the schema lives in version control under
//! `database/migrations/034_tokens_table.sql`.

use alloy_primitives::Address;
use anyhow::{Context, Result};
use sqlx::postgres::PgPool;

#[derive(Clone, Debug)]
pub struct ResolvedToken {
    pub symbol: Option<String>,
    pub decimals: Option<u8>,
    pub logo_url: Option<String>,
    /// One of: 'onchain_full' | 'onchain_partial' | 'trustwallet_only' | 'failed'.
    /// Constrained at the DB layer by migration 034 CHECK.
    pub resolved_via: &'static str,
}

/// Insert or update a token row.
///
/// Address is normalized to lowercase per the migration 034 CHECK
/// (`address ~ '^0x[a-f0-9]{40}$'`). Existing non-NULL fields are preserved
/// via `COALESCE` (a later partial resolution does not erase a prior good
/// value). The `resolved_via` column is upgraded only when the previous
/// status was `'failed'` and the new status is something better — this
/// implements the failure-state recovery semantics required by Task 6.
///
/// `resolved_at` is also advanced when a `'failed'` row is promoted to a
/// non-failed status, so observability queries for "when was this token
/// successfully resolved?" return the correct timestamp.
pub async fn upsert_token(
    pool: &PgPool,
    chain_id: u64,
    address: Address,
    t: ResolvedToken,
) -> Result<()> {
    // alloy 1.x: format!("{address:#x}") emits "0x" + 40 lowercase hex chars,
    // which matches the migration 034 CHECK constraint exactly.
    let addr_lc = format!("{address:#x}");
    let chain_id_i32 = i32::try_from(chain_id)
        .map_err(|_| anyhow::anyhow!("chain_id {chain_id} exceeds i32 range"))?;
    sqlx::query(
        r#"
        INSERT INTO tokens (chain_id, address, symbol, decimals, logo_url, resolved_via, resolved_at, last_seen_at)
        VALUES ($1, $2, $3, $4, $5, $6, NOW(), NOW())
        ON CONFLICT (chain_id, address) DO UPDATE SET
            symbol       = COALESCE(EXCLUDED.symbol,       tokens.symbol),
            decimals     = COALESCE(EXCLUDED.decimals,     tokens.decimals),
            logo_url     = COALESCE(EXCLUDED.logo_url,     tokens.logo_url),
            resolved_via = CASE
                WHEN tokens.resolved_via = 'failed' AND EXCLUDED.resolved_via <> 'failed'
                  THEN EXCLUDED.resolved_via
                ELSE tokens.resolved_via
              END,
            resolved_at  = CASE
                WHEN tokens.resolved_via = 'failed' AND EXCLUDED.resolved_via <> 'failed'
                  THEN NOW()
                ELSE tokens.resolved_at
              END,
            last_seen_at = NOW()
        "#,
    )
    .bind(chain_id_i32)
    .bind(&addr_lc)
    .bind(t.symbol.as_deref())
    .bind(t.decimals.map(|d| d as i16))
    .bind(t.logo_url.as_deref())
    .bind(t.resolved_via)
    .execute(pool)
    .await
    .context("upsert tokens")?;
    Ok(())
}

/// Returns true if the token should be (re-)resolved.
///
/// Semantics:
/// - Never seen before → `true`.
/// - `resolved_via = 'failed'` and the row is older than 7 days → `true`
///   (TTL retry: give the resolver another chance after the cooldown).
/// - Any other case → `false` (we already have something useful).
///
/// The TTL comparison is performed entirely server-side using PostgreSQL's
/// `NOW()` and `INTERVAL '7 days'`. This avoids Rust↔PG clock-skew issues
/// that can occur in containerised environments where the process clock and
/// the database server clock drift independently.
pub async fn needs_resolution(pool: &PgPool, chain_id: u64, address: Address) -> Result<bool> {
    let addr_lc = format!("{address:#x}");
    // Returns a boolean column: true iff the row is a failed token whose
    // resolved_at is more than 7 days in the past (server-side comparison).
    // fetch_optional returns None when the row does not exist.
    let chain_id_i32 = i32::try_from(chain_id)
        .map_err(|_| anyhow::anyhow!("chain_id {chain_id} exceeds i32 range"))?;
    let row: Option<(bool,)> = sqlx::query_as(
        r#"
        SELECT
          (resolved_via = 'failed' AND resolved_at < NOW() - INTERVAL '7 days') AS retry
        FROM tokens
        WHERE chain_id = $1 AND address = $2
        "#,
    )
    .bind(chain_id_i32)
    .bind(&addr_lc)
    .fetch_optional(pool)
    .await
    .context("needs_resolution lookup")?;
    match row {
        None => Ok(true),
        Some((retry,)) => Ok(retry),
    }
}
