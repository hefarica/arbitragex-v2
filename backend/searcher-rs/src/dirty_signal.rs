//! ARBX-0003 / workbook `09_RUNTIME_STRUCTURES` — cross-worker dirty-pool
//! signal (event state propagation, XLS-QB-05 wiring).
//!
//! Writers (`pool_sync_worker`) mark a pool when a successful refresh
//! CHANGED its cached state (a first observation counts as a change — it
//! is a state arrival, not a no-op). Consumers (the `route_discovery`
//! run_loop) drain the set each tick into their in-proc `DirtyPairEngine`,
//! so re-evaluation is seeded by the pairs that actually moved instead of
//! a global rebuild (workbook 08: DirtySeed×Beam bound).
//!
//! Transport: ONE Redis SET per chain — `arbx:dirty_pools:<chain_id>`,
//! members are canonical pool addresses (lowercase hex, the same
//! normalization the reserves cache keys use). Set membership is
//! idempotent, so re-marking an already-dirty pool is a no-op — the same
//! coalescing the in-proc bitset applies. The TTL refresh runs once per
//! tick (writer side) while the set is non-empty; a dead writer lets the
//! signal EXPIRE with the reserves caches themselves — honest absence,
//! never a stale-forever dirty set.
//!
//! Bound: naturally bounded by the pool universe (|set| ≤ pools synced);
//! no artificial cap (documented decision — the universe is finite and the
//! TTL is the leak backstop).

/// Redis SET key holding the dirty pool addresses for one chain.
pub const DIRTY_POOLS_KEY_PREFIX: &str = "arbx:dirty_pools:";

/// TTL for the dirty-pool set. Matches the reserves/slot0 cache TTL (30s)
/// so the signal lives exactly as long as the state it describes — a dead
/// writer clears both horizons together.
pub const DIRTY_TTL_SECS: u64 = 30;

/// Full Redis key for a chain's dirty-pool set.
pub fn dirty_pools_key(chain_id: u64) -> String {
    format!("{DIRTY_POOLS_KEY_PREFIX}{chain_id}")
}

/// Canonical set member for a pool address — same normalization the
/// reserves cache keys use (lowercase, trimmed). Idempotent input →
/// identical member → SET coalescing works.
pub fn normalize_member(pool_addr: &str) -> String {
    pool_addr.trim().to_ascii_lowercase()
}

/// Value-change decision for a V2 pool refresh: `true` on first observation
/// or when either reserve string differs. Unchanged refreshes do NOT
/// signal (a block tick with identical reserves re-evaluates nothing).
pub fn v2_changed(prev: Option<(&str, &str)>, r0: &str, r1: &str) -> bool {
    match prev {
        None => true,
        Some((p0, p1)) => p0 != r0 || p1 != r1,
    }
}

/// Value-change decision for a V3 pool refresh: `true` on first observation
/// or when `sqrtPriceX96` or `liquidity` differs (same string-compare
/// semantics — the entry stores decimal strings).
pub fn v3_changed(prev: Option<(&str, &str)>, sqrt_price_x96: &str, liquidity: &str) -> bool {
    match prev {
        None => true,
        Some((p, l)) => p != sqrt_price_x96 || l != liquidity,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_format_is_chain_scoped() {
        assert_eq!(dirty_pools_key(1), "arbx:dirty_pools:1");
        assert_eq!(dirty_pools_key(11155111), "arbx:dirty_pools:11155111");
    }

    #[test]
    fn member_normalization_is_idempotent() {
        assert_eq!(normalize_member("  0xABCdef  "), "0xabcdef");
        assert_eq!(
            normalize_member(&normalize_member("0xAB")),
            normalize_member("0xAB")
        );
    }

    #[test]
    fn v2_change_semantics() {
        // First observation = state arrival = dirty.
        assert!(v2_changed(None, "100", "200"));
        // Identical refresh = NOT dirty.
        assert!(!v2_changed(Some(("100", "200")), "100", "200"));
        // Either reserve moving = dirty.
        assert!(v2_changed(Some(("100", "200")), "101", "200"));
        assert!(v2_changed(Some(("100", "200")), "100", "201"));
    }

    #[test]
    fn v3_change_semantics() {
        assert!(v3_changed(
            None,
            "79228162514264337593543950336",
            "123456789"
        ));
        assert!(!v3_changed(
            Some(("79228162514264337593543950336", "123456789")),
            "79228162514264337593543950336",
            "123456789"
        ));
        assert!(v3_changed(
            Some(("79228162514264337593543950336", "123456789")),
            "79228162514264337593543950337",
            "123456789"
        ));
        assert!(v3_changed(
            Some(("79228162514264337593543950336", "123456789")),
            "79228162514264337593543950336",
            "123456790"
        ));
    }
}
