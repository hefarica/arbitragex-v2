//! PostgreSQL reconciliation query for the token enricher.
//!
//! `find_unresolved_tokens` is the bridge between the `opportunities` table
//! and the `tokens` registry: it returns every (chain_id, address) pair
//! referenced by an opportunity row that either has no corresponding token
//! row at all, or whose token row is a TTL-expired `'failed'` entry (the
//! 7-day retry window has opened).
//!
//! ## Why NOT just use `needs_resolution` per-address
//!
//! `persistence::needs_resolution` performs a single-row lookup. For
//! reconciliation we need a *set-difference* across potentially millions of
//! opportunity rows — a single SQL query is orders of magnitude faster than
//! N individual point-lookups.
//!
//! ## Caller contract (preempting cs-validator concern)
//!
//! The caller (Task 8 reconciliation tick) MUST tolerate an empty `Vec`
//! return — this is the normal steady-state once all tokens are resolved.
//! When `find_unresolved_tokens` returns 0 rows, the tick should idle-sleep
//! and retry later; it MUST NOT treat empty as an error.
//!
//! ## Race with Redis consumer (cs-validator concern)
//!
//! `find_unresolved_tokens` (PG-driven) and `EnricherConsumer::run`
//! (Redis-driven) are two independent producers of resolution work. They are
//! intentionally not coordinated: the downstream resolver (Task 8) calls
//! `persistence::needs_resolution` before dispatching each item, which
//! provides idempotent deduplication. The deliberate trade-off is simplicity
//! over strict once-only delivery — duplicate resolution attempts are safe
//! because `upsert_token` is idempotent.
//!
//! ## LIMIT and binding type
//!
//! PostgreSQL accepts BIGINT for LIMIT parameters; `i64` maps correctly via
//! sqlx. A `limit` of 0 would return no rows — callers should use a
//! reasonable positive value (e.g. 100–500).

use anyhow::{Context, Result};
use sqlx::PgPool;

/// Returns `(chain_id, address_lowercase)` tuples for tokens that need
/// resolution.
///
/// A token needs resolution when:
/// 1. It appears in `opportunities.token_in` or `opportunities.token_out`
///    (or `token_out` on `chain_id_out` for cross-chain opportunities), AND
/// 2. There is NO existing row in the `tokens` table for that `(chain_id,
///    address)` pair, OR the existing row has `resolved_via = 'failed'`
///    AND `resolved_at < NOW() - INTERVAL '7 days'` (TTL retry).
///
/// ### TTL retry semantics (Defect #4 fix)
///
/// A naive `WHERE NOT EXISTS (SELECT 1 FROM tokens WHERE ...)` would
/// permanently skip `'failed'` tokens. The corrected WHERE clause is:
///
/// ```text
/// NOT EXISTS (
///   SELECT 1 FROM tokens t
///   WHERE t.chain_id = opps.chain_id
///     AND t.address  = opps.address
///     AND NOT (t.resolved_via = 'failed' AND t.resolved_at < NOW() - INTERVAL '7 days')
/// )
/// ```
///
/// Reading the double negation: "skip if there exists a token row where it
/// is NOT a TTL-expired failed row" — i.e. skip resolved tokens and
/// recently-failed ones. Tokens that are absent altogether or whose failure
/// TTL has expired pass through.
///
/// ### Return type and Task 8 caller contract
///
/// Returns `Vec<(i32, String)>` because PostgreSQL's `INTEGER` column type
/// maps to `i32` via sqlx.  Callers (the Task 8 main loop) **must** cast the
/// `chain_id` to `u64` for `persistence` functions, with an explicit
/// positivity guard before the cast:
///
/// ```rust,ignore
/// for (c, addr) in find_unresolved_tokens(&pool, limit).await? {
///     if c <= 0 { continue; }   // guard: malformed row → skip, not u64::MAX
///     let chain_id = c as u64;
///     // ...
/// }
/// ```
///
/// This avoids the pitfall where a malformed `chain_id = 0` or negative value
/// produces `u64::MAX` via wrapping cast.
pub async fn find_unresolved_tokens(pool: &PgPool, limit: i64) -> Result<Vec<(i32, String)>> {
    // Short-circuit: PostgreSQL rejects negative LIMIT at runtime and a
    // LIMIT 0 query always returns empty.  Guard both at the boundary so
    // callers never need to think about it.
    if limit <= 0 {
        return Ok(vec![]);
    }

    // The UNION deduplicates: both DISTINCT and set semantics apply.
    // token_in uses the opportunity's own chain_id.
    // token_out uses chain_id_out when present (cross-chain), otherwise chain_id.
    // All addresses are lowercased to match the tokens.address CHECK constraint
    // (`address ~ '^0x[a-f0-9]{40}$'`).
    let rows: Vec<(i32, String)> = sqlx::query_as(
        r#"
        SELECT chain_id, address FROM (
          SELECT DISTINCT chain_id, LOWER(token_in) AS address
            FROM opportunities
          UNION
          SELECT DISTINCT
            COALESCE(chain_id_out, chain_id) AS chain_id,
            LOWER(token_out) AS address
            FROM opportunities
        ) opps
        WHERE NOT EXISTS (
          SELECT 1 FROM tokens t
          WHERE t.chain_id = opps.chain_id
            AND t.address  = opps.address
            AND NOT (t.resolved_via = 'failed' AND t.resolved_at < NOW() - INTERVAL '7 days')
        )
        LIMIT $1
        "#,
    )
    .bind(limit)
    .fetch_all(pool)
    .await
    .context("find_unresolved_tokens")?;
    Ok(rows)
}
