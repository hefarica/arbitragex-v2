//! EMIT-01 (ARBX-FE-EMIT-01, FE-MASTER P3): token-universe snapshot for the
//! resolve endpoint — the Rust half behind `POST /api/admin/tokens/resolve`.
//!
//! The resolve is REQUEST-TIME (the operator's symbol list), so it cannot be
//! pre-computed; what this module publishes is the PRE-INDEXED universe the
//! api-server looks symbols up in:
//!   - `symbols` maps the NORMALIZED symbol (`norm_symbol`: trim+uppercase,
//!     shared-rs TW-002 — the REAL normalizer, never a TS re-implementation)
//!     to ALL its universe addresses. 0 matches ⇒ NOT_FOUND, 1 ⇒ RESOLVED,
//!     >1 ⇒ AMBIGUOUS — the row statuses of `TokenResolvePreviewRowSchema`.
//!   - `tokens` keeps the display map address → `{symbol, decimals}` as
//!     declared (EMIT-02 Layer-2: decimals ride the scan row, so the resolve
//!     wire surfaces them per row instead of an eternal null).
//!   - `kpis` carries the §6 combinatorics computed with the REAL
//!     `pair_index` functions (`within_chain_pairs` /
//!     `within_chain_directed_edges`, ARBX-0028) — never re-derived in TS.
//!
//! Written by `token_identity::index_for` on every index REBUILD (TTL
//! expiry, ~once per 30s per chain) — the same scan that feeds the gate, so
//! the snapshot is never staler than the identity it mirrors. Best-effort: a
//! Redis error is one warn line, never fatal (the gate keeps running).
//!
//! Honesty model (R8 / RULE 00): overflowed combinatorics surface as `null`
//! (checked math all the way down); `allowed_tokens` is the resolved count —
//! `universe_len` when the operator allowlist is empty (permissive — the
//! effective universe IS the whole universe).

use redis::aio::ConnectionManager;
use serde_json::{json, Value};
use tracing::warn;

use crate::pair_index;
use crate::reserves::UniverseToken;

/// Redis key holding the latest pre-indexed universe snapshot for one chain.
/// Read by `POST /api/admin/tokens/resolve` (api-server). Same chain-scoped
/// convention as the tick snapshot (`arbx:route_discovery:tick:<chain>`).
pub const TOKEN_UNIVERSE_KEY_PREFIX: &str = "arbx:token_universe:";

/// Snapshot TTL: `IDENTITY_TTL` (30s) + slack, so a reader between two
/// rebuilds always finds a fresh-enough snapshot and a dead searcher lets it
/// EXPIRE into an honest 404 (never a stale-forever universe).
pub const TOKEN_UNIVERSE_TTL_SECS: u64 = 35;

/// Full Redis key for a chain's universe snapshot.
pub fn token_universe_key(chain_id: u64) -> String {
    format!("{TOKEN_UNIVERSE_KEY_PREFIX}{chain_id}")
}

/// Build the snapshot payload (PURE — unit-tests without Redis).
///
/// `allowed_count`: `None` = permissive (empty operator allowlist — the
/// effective universe is the whole scan); `Some(n)` = the resolved allowlist
/// size. The §6 KPIs are computed over THAT effective N.
pub fn universe_snapshot_to_wire(
    chain_id: u64,
    universe: &[UniverseToken],
    allowed_count: Option<usize>,
) -> Value {
    // norm_addr/norm_symbol semantics (shared-rs TW-002) applied at BUILD
    // time: the TS side then only does EXACT map lookups on pre-normalized
    // keys — the normalizer exists once, here.
    let mut symbols: std::collections::BTreeMap<String, Vec<String>> =
        std::collections::BTreeMap::new();
    let mut tokens = serde_json::Map::new();
    for t in universe {
        let addr_n = t.address.trim().to_ascii_lowercase();
        if addr_n.is_empty() {
            continue; // degenerate universe row — skip, never fabricate
        }
        let sym_n = t.symbol.trim().to_ascii_uppercase();
        if sym_n.is_empty() {
            continue;
        }
        symbols.entry(sym_n).or_default().push(addr_n.clone());
        // Display map keeps the DECLARED symbol form (first-wins like the
        // resolver) plus the decimals the scan carried — {symbol, decimals}.
        tokens
            .entry(addr_n)
            .or_insert_with(|| json!({ "symbol": t.symbol, "decimals": t.decimals }));
    }

    // Deterministic snapshot: BTreeMap iteration is ordered; per-symbol
    // address lists preserve the scan order (first-wins display).
    let symbols_json: serde_json::Map<String, Value> =
        symbols.into_iter().map(|(k, v)| (k, json!(v))).collect();

    let effective_n = allowed_count.unwrap_or(tokens.len());
    // u128 (pair_index return) → JSON number only when it fits u64; the
    // workbook envelope (≤20k tokens ⇒ ≤ ~4×10^8 pairs) always fits — the
    // null path is the honest overflow guard, not an expected state.
    let fit = |v: Option<u128>| v.and_then(|x| u64::try_from(x).ok());
    json!({
        "chain_id": chain_id,
        "universe_len": tokens.len(),
        "symbols": Value::Object(symbols_json),
        "tokens": Value::Object(tokens),
        "kpis": {
            "allowed_tokens": effective_n,
            "possible_pairs": fit(pair_index::within_chain_pairs(&[effective_n])),
            "directed_token_pairs": fit(pair_index::within_chain_directed_edges(&[effective_n])),
        },
    })
}

/// Persist the snapshot (EMIT-01 writer side). Best-effort like the
/// telemetry sinks: one warn line on failure, never fatal.
pub async fn write_universe_snapshot(
    redis: &mut ConnectionManager,
    chain_id: u64,
    universe: &[UniverseToken],
    allowed_count: Option<usize>,
) {
    let payload = universe_snapshot_to_wire(chain_id, universe, allowed_count);
    let json = match serde_json::to_string(&payload) {
        Ok(s) => s,
        Err(e) => {
            warn!(event = "token_universe.snapshot_serialize_failed", chain_id, error = %e);
            return;
        }
    };
    let res: redis::RedisResult<()> = redis::cmd("SET")
        .arg(token_universe_key(chain_id))
        .arg(&json)
        .arg("EX")
        .arg(TOKEN_UNIVERSE_TTL_SECS)
        .query_async(redis)
        .await;
    if let Err(e) = res {
        warn!(event = "token_universe.snapshot_set_failed", chain_id, error = %e);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(addr: &str, sym: &str, decimals: u8) -> UniverseToken {
        UniverseToken {
            address: addr.to_string(),
            symbol: sym.to_string(),
            decimals,
        }
    }

    #[test]
    fn key_is_chain_scoped() {
        assert_eq!(token_universe_key(1), "arbx:token_universe:1");
        assert_eq!(token_universe_key(11155111), "arbx:token_universe:11155111");
        assert_eq!(TOKEN_UNIVERSE_TTL_SECS, 35);
    }

    /// Symbols are normalized (trim+uppercase — TW-002 form) and grouped to
    /// ALL their addresses: one address ⇒ RESOLVED-shaped, several ⇒
    /// AMBIGUOUS-shaped. Addresses normalize to lowercase identity form.
    /// The display map keeps the DECLARED symbol + the carried decimals
    /// (EMIT-02 Layer-2): first-wins like the resolver.
    #[test]
    fn symbols_index_groups_addresses_case_insensitively() {
        let v = universe_snapshot_to_wire(
            1,
            &[
                row("  0xABC  ", " weth ", 18),
                row("0xdef", "Weth", 18),
                row("0x123", "USDC", 6),
            ],
            None,
        );
        let syms = v["symbols"].as_object().unwrap();
        assert_eq!(syms.len(), 2);
        assert_eq!(syms["WETH"], json!(["0xabc", "0xdef"]));
        assert_eq!(syms["USDC"], json!(["0x123"]));
        let toks = v["tokens"].as_object().unwrap();
        assert_eq!(toks["0xabc"], json!({ "symbol": " weth ", "decimals": 18 }));
        assert_eq!(toks["0xdef"], json!({ "symbol": "Weth", "decimals": 18 }));
        assert_eq!(toks["0x123"], json!({ "symbol": "USDC", "decimals": 6 }));
        assert_eq!(v["universe_len"], json!(3));
        assert_eq!(v["chain_id"], json!(1));
    }

    /// Permissive (None): KPIs over the whole universe. Restricted (Some(n)):
    /// over the resolved allowlist size. The combinatorics are the REAL
    /// pair_index values (parity asserted live, no literals).
    #[test]
    fn kpis_use_effective_n_and_real_pair_index() {
        let uni = &[
            row("0xa", "AAA", 18),
            row("0xb", "BBB", 18),
            row("0xc", "CCC", 6),
        ];
        let permissive = universe_snapshot_to_wire(1, uni, None);
        assert_eq!(permissive["kpis"]["allowed_tokens"], json!(3));
        assert_eq!(
            permissive["kpis"]["possible_pairs"],
            json!(pair_index::within_chain_pairs(&[3]).unwrap())
        );
        assert_eq!(
            permissive["kpis"]["directed_token_pairs"],
            json!(pair_index::within_chain_directed_edges(&[3]).unwrap())
        );

        let restricted = universe_snapshot_to_wire(1, uni, Some(2));
        assert_eq!(restricted["kpis"]["allowed_tokens"], json!(2));
        assert_eq!(
            restricted["kpis"]["possible_pairs"],
            json!(pair_index::within_chain_pairs(&[2]).unwrap())
        );
    }

    /// Degenerate rows (empty after trim) are skipped, never fabricated —
    /// and an empty universe yields an honest empty snapshot with zero KPIs.
    #[test]
    fn degenerate_rows_skip_and_empty_universe_is_honest() {
        let v = universe_snapshot_to_wire(1, &[row("   ", "WETH", 18), row("0xok", "  ", 6)], None);
        assert_eq!(v["universe_len"], json!(0));
        assert_eq!(v["symbols"].as_object().unwrap().len(), 0);
        assert_eq!(v["kpis"]["allowed_tokens"], json!(0));
        assert_eq!(v["kpis"]["possible_pairs"], json!(0));
        assert_eq!(v["kpis"]["directed_token_pairs"], json!(0));
    }

    /// Top-level wire keys are EXACT (the endpoint serves this verbatim to a
    /// `.strict()`-adjacent consumer contract).
    #[test]
    fn snapshot_wire_keys_exact() {
        let v = universe_snapshot_to_wire(1, &[row("0xa", "AAA", 18)], None);
        let obj = v.as_object().unwrap();
        for key in ["chain_id", "universe_len", "symbols", "tokens", "kpis"] {
            assert!(obj.contains_key(key), "missing key {}", key);
        }
        assert_eq!(obj.len(), 5);
        let kpis = v["kpis"].as_object().unwrap();
        for key in ["allowed_tokens", "possible_pairs", "directed_token_pairs"] {
            assert!(kpis.contains_key(key), "missing kpi {}", key);
        }
        assert_eq!(kpis.len(), 3);
    }
}
