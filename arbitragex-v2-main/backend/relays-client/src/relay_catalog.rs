//! DB-backed relay catalog loader (migration 013 — `relays` table).
//!
//! No-hardcode doctrine: the relay catalog is operator-owned. Services ask the
//! DB "which relays are enabled for chain X?"; they never assume a relay
//! exists because of a config file entry or a code default.
//!
//! This module is read-only from the hot-path's perspective. Mutations go
//! through api-server's admin CRUD endpoints (migration 013 + api-server
//! commit 2ebe5c0).
//!
//! Load-on-boot today; a hot-reload subscriber is a future follow-up (listen
//! on a Redis pub/sub channel emitted by api-server on CRUD events).

use anyhow::{Context, Result};
use sqlx::postgres::PgPool;
use tracing::{info, warn};

/// A relay catalog row, as consumed by the hot path. Mirrors the columns of
/// `relays` (migration 013) minus audit metadata.
///
/// `auth_scheme`, `auth_secret_ref`, `priority` are carried through today but
/// not yet consumed by the hot path — `auth_*` are picked up when Phase 6 wires
/// per-relay Vault secret lookups; `priority` is consumed once we support
/// multiple simultaneous relays per chain. The fields are kept (rather than
/// dropped) so the row type stays a 1:1 mirror of the table schema.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct RelayCatalogEntry {
    pub name: String,
    pub chain_id: i32,
    pub endpoint: String, // non-empty by DB constraint when enabled
    pub auth_scheme: String,
    pub auth_secret_ref: Option<String>,
    pub priority: i32,
}

/// Load all enabled relays for a chain, ordered by priority (lower first).
///
/// Returns an empty Vec if DB is reachable but no rows match. Returns an
/// Err only on DB errors — callers decide whether that's fatal (relays
/// disabled) or degraded (fall back to env var).
type RelayCatalogRow = (String, i32, Option<String>, String, Option<String>, i32);

pub async fn load_enabled(pool: &PgPool, chain_id: i32) -> Result<Vec<RelayCatalogEntry>> {
    let rows: Vec<RelayCatalogRow> = sqlx::query_as(
        r#"
        SELECT name, chain_id, endpoint, auth_scheme, auth_secret_ref, priority
          FROM relays
         WHERE enabled = TRUE
           AND chain_id = $1
           AND endpoint IS NOT NULL
           AND length(endpoint) >= 8
         ORDER BY priority ASC, name ASC
        "#,
    )
    .bind(chain_id)
    .fetch_all(pool)
    .await
    .context("relay_catalog.load_enabled")?;

    let entries: Vec<RelayCatalogEntry> = rows
        .into_iter()
        .filter_map(
            |(name, chain_id, endpoint_opt, auth_scheme, auth_secret_ref, priority)| {
                // The SQL `length(endpoint) >= 8` already filters, but re-check
                // defensively so callers never receive an empty endpoint.
                endpoint_opt
                    .filter(|e| e.len() >= 8)
                    .map(|endpoint| RelayCatalogEntry {
                        name,
                        chain_id,
                        endpoint,
                        auth_scheme,
                        auth_secret_ref,
                        priority,
                    })
            },
        )
        .collect();

    info!(
        event = "relay_catalog.loaded",
        chain_id,
        count = entries.len(),
        names = ?entries.iter().map(|e| e.name.as_str()).collect::<Vec<_>>(),
        "relay catalog loaded from DB"
    );
    Ok(entries)
}

/// Convenience: find the enabled flashbots entry for a chain, if any.
pub fn find_flashbots(entries: &[RelayCatalogEntry], chain_id: i32) -> Option<&RelayCatalogEntry> {
    entries
        .iter()
        .find(|e| e.name == "flashbots" && e.chain_id == chain_id)
}

/// Log a warning when the catalog is empty. Separate helper so the boot path
/// can decide whether to fall through to env-var fallback or bail.
pub fn warn_if_empty(entries: &[RelayCatalogEntry], chain_id: i32) {
    if entries.is_empty() {
        warn!(
            event = "relay_catalog.empty",
            chain_id,
            "relay catalog is empty for this chain — no relays will accept bundles. \
             Populate via POST /admin/relays (onboarding step 4)."
        );
    }
}
