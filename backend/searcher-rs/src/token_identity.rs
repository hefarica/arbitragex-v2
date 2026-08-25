//! ARBX-0018 — composition site of the address-keyed token identity.
//!
//! Owns the per-chain `TokenIdentityIndex` cache (30s TTL — same horizon as
//! the reserves caches, so identity metadata is never staler than the state
//! it gates). One composition point: universe from `reserves::
//! scan_token_universe`, allowlist from the operator's `TradingConfigState`;
//! the resolved index is what the scanner attaches to the evaluator via
//! `with_token_identity`.
//!
//! Failure semantics (R8 fail-honest): a failed/timed-out universe scan
//! logs once per TTL window and reuses the LAST GOOD index when one exists;
//! on a cold failure the caller gets an EMPTY-UNIVERSE index — a non-empty
//! operator allowlist then fails closed (`TokenNotAllowed:<addr>`), exactly
//! the pre-index cold-cache behaviour. Never fabricates a permissive index.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;

use shared_rs::token_identity::normalize_allowlist_entry;
use shared_rs::token_identity::TokenIdentityIndex;
use shared_rs::trading_config::TradingConfigState;
use tokio::sync::RwLock;

use crate::reserves;

/// Universe scan bound (see `reserves::scan_token_universe`).
pub const MAX_UNIVERSE_TOKENS: usize = 20_000;

/// Identity index staleness horizon. Matches the reserves cache TTL: the
/// token-enricher writes `arbx:tokens:*` rarely (boot + new-token
/// discovery), so 30s is generous while keeping a bounded staleness for
/// newly enriched tokens (rejected ≤30s, honest reason, admitted on the
/// next refresh — same shape as the legacy cold-cache path).
pub const IDENTITY_TTL: Duration = Duration::from_secs(30);

/// Hard cap on the universe scan (Redis SCAN on a huge/degenerate keyspace
/// must never stall the scan path — precedent: ReservesCache hydration
/// timeout, scanner.rs P0 boot-hang fix).
const SCAN_TIMEOUT: Duration = Duration::from_secs(10);

type CacheEntry = (Arc<TokenIdentityIndex>, Instant);

static CACHE: RwLock<Option<HashMap<u64, CacheEntry>>> = RwLock::const_new(None);

/// Per-chain resolved identity index, cached for [`IDENTITY_TTL`]. The
/// scanner calls this once per gate evaluation; the SCAN+MGET fan-out only
/// runs on TTL expiry (amortized ~2 universe reads/minute/chain).
pub async fn index_for(
    redis: &mut redis::aio::ConnectionManager,
    chain_id: u64,
    cfg: &TradingConfigState,
) -> Arc<TokenIdentityIndex> {
    {
        let g = CACHE.read().await;
        if let Some(map) = g.as_ref() {
            if let Some((idx, at)) = map.get(&chain_id) {
                if at.elapsed() < IDENTITY_TTL {
                    // Allowlist changed since this index was built → rebuild
                    // now rather than gating on a stale operator intent.
                    if idx_matches_symbols(idx, &cfg.allowed_token_symbols) {
                        return idx.clone();
                    }
                }
            }
        }
    }

    let universe = match tokio::time::timeout(
        SCAN_TIMEOUT,
        reserves::scan_token_universe(redis, chain_id, MAX_UNIVERSE_TOKENS),
    )
    .await
    {
        Ok(Ok(u)) => u,
        Ok(Err(e)) => {
            tracing::warn!(event = "token_identity.scan_failed", chain_id, error = %e);
            Vec::new()
        }
        Err(_) => {
            tracing::warn!(event = "token_identity.scan_timeout", chain_id);
            Vec::new()
        }
    };

    // The shared resolver consumes (addr, symbol) tuples; the scan now yields
    // `UniverseToken` rows (decimals ride along for the EMIT-01 snapshot).
    let pairs: Vec<(String, String)> = universe
        .iter()
        .map(|t| (t.address.clone(), t.symbol.clone()))
        .collect();
    let idx = Arc::new(TokenIdentityIndex::resolve(
        chain_id,
        &cfg.allowed_token_symbols,
        &pairs,
    ));

    if !idx.unresolved_symbols().is_empty() {
        tracing::info!(
            event = "token_identity.unresolved_symbols",
            chain_id,
            symbols = ?idx.unresolved_symbols(),
            universe_len = idx.universe_len(),
            "operator allowlist entries matched no token in the universe (R8 honest)"
        );
    }

    let mut g = CACHE.write().await;
    let map = g.get_or_insert_with(HashMap::new);
    map.insert(chain_id, (idx.clone(), Instant::now()));

    // EMIT-01: publish the pre-indexed universe snapshot on every REBUILD
    // (this path only runs on TTL expiry — ~1 write per 30s per chain).
    // `None` = permissive (empty operator allowlist — effective universe is
    // the whole scan); `Some` = resolved allowlist size. Feeds
    // POST /api/admin/tokens/resolve (api-server).
    let allowed_count = if idx.is_permissive() {
        None
    } else {
        Some(idx.allowed_addr_count())
    };
    crate::token_resolve_signal::write_universe_snapshot(redis, chain_id, &universe, allowed_count)
        .await;

    idx
}

/// A cached index matches the current config when its permissiveness
/// agrees with the operator list AND it carries no unresolved gap that a
/// universe refresh could fill. Cheap structural check — the TTL/rebuild
/// path stays the refresh authority; this only catches operator edits
/// landing inside a TTL window.
fn idx_matches_symbols(idx: &TokenIdentityIndex, allowed_symbols: &[String]) -> bool {
    // TW-002: form-aware shared normalization — address entries lowercase
    // (identity), symbols uppercase — the same convention `resolve` uses,
    // so the cache check and the resolver can never disagree on form.
    let wanted: Vec<String> = allowed_symbols
        .iter()
        .map(|s| normalize_allowlist_entry(s))
        .filter(|s| !s.is_empty())
        .collect();
    if wanted.is_empty() {
        return idx.is_permissive();
    }
    !idx.is_permissive() && idx.unresolved_symbols().is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &str) -> String {
        v.to_string()
    }

    #[test]
    fn cache_match_detects_permissive_drift() {
        let permissive = TokenIdentityIndex::resolve(1, &[], &[]);
        let restricted = TokenIdentityIndex::resolve(1, &[s("WETH")], &[]);
        assert!(idx_matches_symbols(&permissive, &[]));
        assert!(!idx_matches_symbols(&restricted, &[]));
        assert!(!idx_matches_symbols(&permissive, &[s("WETH")]));

        // WETH unresolved in the empty universe → a refresh may fill it →
        // mismatch forces rebuild even within TTL.
        assert!(!idx_matches_symbols(&restricted, &[s("WETH")]));

        let resolved = TokenIdentityIndex::resolve(1, &[s("WETH")], &[(s("0xweth"), s("WETH"))]);
        assert!(idx_matches_symbols(&resolved, &[s("WETH")]));
        assert!(idx_matches_symbols(&resolved, &[s(" weth ")]));
    }
}
