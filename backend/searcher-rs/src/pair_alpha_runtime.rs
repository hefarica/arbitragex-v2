//! EMIT-06b (ARBX-FE-EMIT-06 follow-up): publish the per-pair directed
//! alpha (fe_normalization r15 — forward/reverse NEVER collapse) so
//! `GET /api/pairs` can serve real `alpha_forward` / `alpha_reverse`.
//!
//! The computation itself is the PURE core's job (`QuoteState::pair_alpha`
//! over dense ids, executable rates from the tick's graph); this module is
//! only the WIRE + WRITER half, mirroring `quote_anchor_runtime`:
//!
//!   - Redis hash `arbx:pairs:alpha:<chain>`, field = canonical pair key
//!     `<aAddr>|<bAddr>` with addresses in ASCENDING byte order — the SAME
//!     key `backend/api-server/src/routes/pairs.ts` groups its pool rows by,
//!     so the join is string-equality with zero re-derivation.
//!   - Field value = `{"forward": F_e | null, "reverse": F_e | null}` — the
//!     directed normalized executable rates (`NormalizedEdge::f_e`). `null`
//!     is the R8 "not computed" (missing anchor price / unpriced edge);
//!     the ln form is derivable display-side but is NOT duplicated on the
//!     wire (the §13 contract carries ONE number per direction).
//!   - The hash is rewritten ATOMICALLY per publish (MULTI: DEL + HSETs +
//!     EXPIRE): a reader either sees the previous tick's complete table or
//!     this tick's — never a half-drained mix. TTL 35s = tick cadence +
//!     slack, so a dead searcher lapses into the endpoint's honest nulls
//!     (never a stale-forever alpha).
//!   - Gated by the SAME `fe_prefilter` knob as the lane that computes it:
//!     knob OFF ⇒ nothing is written ⇒ the hash expires ⇒ alpha serves
//!     null (deployed v1 behavior, unchanged).
//!
//! Direction choice per pair: the pair's directed rate is the BEST edge
//! across its parallel pools (max executable rate) — the worker glue picks
//! it while walking `graph.edges`; "best edge per direction" is the FE-0019
//! drawer's language too. Rows whose tokens fail dense mapping are counted
//! by the glue and skipped (never fabricated).

use redis::aio::ConnectionManager;
use serde_json::{json, Value};
use tracing::warn;

/// Redis hash key holding this tick's per-pair alpha table. Field = canonical
/// pair key; read by `GET /api/pairs` (api-server routes/pairs.ts).
pub const PAIR_ALPHA_KEY_PREFIX: &str = "arbx:pairs:alpha:";

/// Snapshot TTL: tick cadence (~30s) + slack — mirrors QUOTE_ANCHOR_TTL_SECS
/// (same liveness rationale: expire into honest nulls, never serve stale).
pub const PAIR_ALPHA_TTL_SECS: u64 = 35;

pub fn pair_alpha_key(chain_id: u64) -> String {
    format!("{PAIR_ALPHA_KEY_PREFIX}{chain_id}")
}

/// One pair's directed alpha in wire form: the `f_e` of each direction, with
/// `None` = not computed (R8 — the endpoint renders null, never 0).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PairAlphaRow {
    pub forward: Option<f64>,
    pub reverse: Option<f64>,
}

/// Canonical hash field for a pair: `<aAddr>|<bAddr>`, addresses ascending
/// (byte order == lowercase-hex lexicographic for EVM addresses). The pair
/// table on the other side groups by the SAME key, so canonicalization is
/// THIS module's tested invariant, not the reader's guesswork.
pub fn canonical_pair_field(a: &str, b: &str) -> String {
    if a <= b {
        format!("{a}|{b}")
    } else {
        format!("{b}|{a}")
    }
}

/// Field value for one row: `{"forward": .., "reverse": ..}` — numbers are
/// passed through verbatim (finite by construction in the pure core: the
/// `normalized_edge` guard rejects non-finite rates before here), nulls stay
/// null. EXACTLY two keys.
pub fn pair_alpha_row_to_value(row: &PairAlphaRow) -> Value {
    json!({
        "forward": row.forward,
        "reverse": row.reverse,
    })
}

/// Build the full (field, json) list for one publish — one entry per row,
/// canonical fields. Duplicate fields (same pair passed twice) are a glue
/// bug: the LAST write wins in HSET semantics, so this stays total but the
/// glue is expected to dedupe upstream (counted, not fabricated).
pub fn pair_alpha_fields(rows: &[(String, String, PairAlphaRow)]) -> Vec<(String, String)> {
    rows.iter()
        .map(|(a, b, row)| {
            (
                canonical_pair_field(a, b),
                pair_alpha_row_to_value(row).to_string(),
            )
        })
        .collect()
}

/// Writer side (EMIT-06b). Atomic MULTI: DEL the previous tick's table, HSET
/// this tick's rows, EXPIRE — a reader never observes a half-drained mix.
/// Best-effort like the other signal writers: one warn on failure, never
/// fatal (the tick keeps running; the endpoint falls back to honest nulls
/// once the previous table lapses).
pub async fn write_pair_alpha_snapshot(
    redis: &mut ConnectionManager,
    chain_id: u64,
    rows: &[(String, String, PairAlphaRow)],
) {
    let key = pair_alpha_key(chain_id);
    let fields = pair_alpha_fields(rows);
    let res: redis::RedisResult<()> = redis::pipe()
        .atomic()
        .del(&key)
        .hset_multiple(&key, &fields)
        .expire(&key, PAIR_ALPHA_TTL_SECS as i64)
        .query_async(redis)
        .await;
    if let Err(e) = res {
        warn!(event = "pair_alpha.snapshot_set_failed", chain_id, error = %e);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_field_orders_ascending() {
        // "0xaaa..." > "0xbbb..." is FALSE byte-wise: 'a' < 'b', so aaa
        // sorts FIRST — the field is invariant to argument order.
        let a = "0xaaaa000000000000000000000000000000000001";
        let b = "0xbbbb000000000000000000000000000000000002";
        assert_eq!(canonical_pair_field(a, b), format!("{a}|{b}"));
        assert_eq!(canonical_pair_field(b, a), format!("{a}|{b}"));
    }

    #[test]
    fn row_value_carries_nulls_honestly() {
        let v = pair_alpha_row_to_value(&PairAlphaRow {
            forward: None,
            reverse: None,
        });
        assert_eq!(v.get("forward"), Some(&Value::Null));
        assert_eq!(v.get("reverse"), Some(&Value::Null));
        assert_eq!(v.as_object().map(|o| o.len()), Some(2)); // EXACTLY 2 keys
    }

    #[test]
    fn row_value_passes_numbers_verbatim() {
        let v = pair_alpha_row_to_value(&PairAlphaRow {
            forward: Some(1.0004),
            reverse: Some(0.9996),
        });
        assert_eq!(v.get("forward"), Some(&serde_json::json!(1.0004)));
        assert_eq!(v.get("reverse"), Some(&serde_json::json!(0.9996)));
    }

    #[test]
    fn fields_are_canonical_and_roundtrip_as_json() {
        let rows = vec![(
            "0xbbbb000000000000000000000000000000000002".to_string(),
            "0xaaaa000000000000000000000000000000000001".to_string(), // deliberately reversed
            PairAlphaRow {
                forward: Some(1.5),
                reverse: None,
            },
        )];
        let fields = pair_alpha_fields(&rows);
        assert_eq!(fields.len(), 1);
        let (field, raw) = &fields[0];
        assert!(field.starts_with("0xaaaa"));
        let parsed: Value = serde_json::from_str(raw).unwrap();
        assert_eq!(parsed.get("forward"), Some(&serde_json::json!(1.5)));
        assert_eq!(parsed.get("reverse"), Some(&Value::Null));
    }

    #[test]
    fn key_format_is_chain_suffixed() {
        assert_eq!(pair_alpha_key(1), "arbx:pairs:alpha:1");
        assert_eq!(pair_alpha_key(137), "arbx:pairs:alpha:137");
    }
}
