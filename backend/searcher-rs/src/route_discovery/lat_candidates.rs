//! ARBX-FE-EMIT-09 (FE-MASTER P9): per-candidate latency rows for the tick
//! wire — the unblocker of FE-0037 (§45 waterfall por candidato).
//!
//! `latency_budget.rs` samples at the CYCLE granularity (`LatencyLog`
//! accumulates µs per stage per tick and keeps a windowed history): there is
//! no structure where one route can say how long IT took. This module is
//! that structure's pure half — the timings are captured in the worker's
//! per-candidate loops (annotation + dispatch planning → `gates_us`;
//! adapter pass → `reprice_us`) and handed here as `CandidateSample`s.
//!
//! ## Wire contract (v1, worker-only — see the design doc of 2026-08-24)
//! `tick_summary["lat_candidates"]` = top-K rows by `total_us` desc, and
//! `tick_summary["lat_candidates_meta"]` = the once-per-tick honesty block:
//! ```json
//! {
//!   "attribution": { "gates": "measured", "reprice": "measured-upper-bound" },
//!   "cap": 10, "sampled": 2, "truncated": false, "dropped": 0
//! }
//! ```
//!
//! ## Absence semantics (R8 — the rule that governs every key)
//! - `stages.reprice_us` is ABSENT (not 0, not null) when the route never
//!   traversed the adapter this tick: non-triangular by construction, or
//!   skipped (scoped-out / F_e-prefiltered / malformed) — absence is the
//!   real state, and `route_kind` on the row tells the consumer which.
//! - `gates_us` is always present: the annotation loop visits every route.
//! - `total_us` = Σ traversed stages. It is NOT the tick's wall-clock and
//!   never claims to be (decode/state/emit are candidate-invariant and stay
//!   in the aggregate `lat_stages` rows; `refine` stays null until ARBX-0008).
//!
//! ## Attribution honesty
//! `gates` is measured around `engine.evaluate` + `plan_dispatch` calls.
//! `reprice` is an UPPER BOUND — the F_e prefilter math and the backfill
//! multicall ride inside the timed segment (the same caveat the aggregate
//! `lat.reprice` row already declares for the workbook budget).
//!
//! ## Bounds (LOGFLOOD discipline)
//! Rows are capped top-K (`cap`, env `ARBX_ROUTE_DISCOVERY_LAT_CANDIDATES_CAP`,
//! default [`DEFAULT_CAP`]); the meta block makes any cut observable
//! (`truncated` + `dropped` vs `sampled`) — a recorte is never silent.

use serde_json::{json, Map, Value};

/// Default top-K of per-candidate rows riding the tick summary. A payload
/// bound, not a sampling bound: capture covers every route, selection keeps
/// the K slowest. Sibling of `lat_window` (512) among infra knobs.
pub const DEFAULT_CAP: usize = 10;

/// One candidate's traversed-stage timings (µs). Built by the worker's
/// capture sites; consumed by [`select_top_k`].
#[derive(Debug, Clone)]
pub struct CandidateSample {
    pub route_hash: String,
    /// `RouteKind::as_str()` token (v2v2/v2v3/v3v2/v3v3/triangular/multihop).
    pub route_kind: String,
    pub hops: u8,
    pub gates_us: u64,
    /// `None` until the adapter pass actually served this route this tick.
    pub reprice_us: Option<u64>,
}

impl CandidateSample {
    /// Σ traversed stages — gates plus reprice when present. Not wall-clock.
    pub fn total_us(&self) -> u64 {
        self.gates_us + self.reprice_us.unwrap_or(0)
    }
}

/// The selection result plus the counters the meta block discloses.
#[derive(Debug)]
pub struct Selection {
    pub rows: Vec<CandidateSample>,
    /// Cap actually applied (>= 1 — a 0 env value clamps, it cannot disable).
    pub cap: usize,
    /// Candidates captured this tick (before the cut).
    pub sampled: usize,
    pub truncated: bool,
    pub dropped: usize,
}

/// Top-K by `total_us` descending, ties broken by `route_hash` ascending —
/// a deterministic order the FE can pin and tests can assert. `cap` clamps
/// to >= 1: the cap is a payload bound, not an off switch (capture without
/// emission would report nothing while paying the timing cost — dishonest
/// both ways).
pub fn select_top_k(mut samples: Vec<CandidateSample>, cap: usize) -> Selection {
    let cap = cap.max(1);
    let sampled = samples.len();
    samples.sort_by(|a, b| {
        b.total_us()
            .cmp(&a.total_us())
            .then_with(|| a.route_hash.cmp(&b.route_hash))
    });
    let dropped = sampled.saturating_sub(cap);
    let rows: Vec<CandidateSample> = samples.into_iter().take(cap).collect();
    Selection {
        rows,
        cap,
        sampled,
        truncated: dropped > 0,
        dropped,
    }
}

/// `lat_candidates` rows as wire JSON. `stages` carries ONLY the traversed
/// keys — `reprice_us` is inserted, never defaulted (R8 presence-of-key).
pub fn rows_value(sel: &Selection) -> Value {
    Value::Array(
        sel.rows
            .iter()
            .map(|s| {
                let mut stages = Map::new();
                stages.insert("gates_us".to_string(), json!(s.gates_us));
                if let Some(reprice) = s.reprice_us {
                    stages.insert("reprice_us".to_string(), json!(reprice));
                }
                json!({
                    "route_hash": s.route_hash,
                    "route_kind": s.route_kind,
                    "hops": s.hops,
                    "stages": Value::Object(stages),
                    "total_us": s.total_us(),
                })
            })
            .collect(),
    )
}

/// `lat_candidates_meta` — the once-per-tick honesty block: static
/// attribution vocabulary, the cap applied, and the cut counters.
pub fn meta_value(sel: &Selection) -> Value {
    json!({
        "attribution": {
            "gates": "measured",
            "reprice": "measured-upper-bound",
        },
        "cap": sel.cap,
        "sampled": sel.sampled,
        "truncated": sel.truncated,
        "dropped": sel.dropped,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(hash: &str, kind: &str, gates: u64, reprice: Option<u64>) -> CandidateSample {
        CandidateSample {
            route_hash: hash.to_string(),
            route_kind: kind.to_string(),
            hops: 3,
            gates_us: gates,
            reprice_us: reprice,
        }
    }

    #[test]
    fn orders_by_total_desc_with_hash_tiebreak() {
        let sel = select_top_k(
            vec![
                sample("0xb", "triangular", 10, Some(5)), // total 15
                sample("0xa", "triangular", 10, Some(5)), // total 15 — tie, a < b
                sample("0xc", "v2v2", 40, None),          // total 40
            ],
            10,
        );
        let hashes: Vec<&str> = sel.rows.iter().map(|s| s.route_hash.as_str()).collect();
        assert_eq!(hashes, vec!["0xc", "0xa", "0xb"]);
    }

    #[test]
    fn cap_cuts_bottom_and_discloses_the_cut() {
        let samples: Vec<CandidateSample> = (0..12)
            .map(|i| sample(&format!("0x{i:02x}"), "triangular", 100 - i as u64, None))
            .collect();
        let sel = select_top_k(samples, 10);
        assert_eq!(sel.rows.len(), 10);
        assert_eq!(sel.sampled, 12);
        assert!(sel.truncated);
        assert_eq!(sel.dropped, 2);
        // The K KEPT are the slowest (100..=91), the two fastest were dropped.
        assert_eq!(sel.rows.last().unwrap().gates_us, 91);
    }

    #[test]
    fn cap_zero_clamps_to_one_not_off() {
        let sel = select_top_k(vec![sample("0xa", "v2v2", 5, None)], 0);
        assert_eq!(sel.cap, 1);
        assert_eq!(sel.rows.len(), 1);
    }

    #[test]
    fn reprice_key_is_absent_not_zeroed() {
        let sel = select_top_k(
            vec![
                sample("0xa", "triangular", 10, Some(7)),
                sample("0xb", "v2v2", 20, None),
            ],
            10,
        );
        let rows = rows_value(&sel);
        // Selection order is total_us DESC: 0xb (20) first, 0xa (17) second.
        let v2 = &rows[0];
        assert!(
            v2["stages"].get("reprice_us").is_none(),
            "absence, not 0/null"
        );
        assert_eq!(v2["total_us"], json!(20));
        let tri = &rows[1];
        assert_eq!(tri["stages"]["reprice_us"], json!(7));
    }

    #[test]
    fn total_is_sum_of_traversed_stages() {
        let s = sample("0xa", "triangular", 96, Some(1240));
        assert_eq!(s.total_us(), 1336);
    }

    #[test]
    fn empty_capture_yields_empty_rows_and_honest_meta() {
        let sel = select_top_k(Vec::new(), 10);
        assert_eq!(rows_value(&sel), Value::Array(Vec::new()));
        let meta = meta_value(&sel);
        assert_eq!(meta["sampled"], json!(0));
        assert_eq!(meta["truncated"], json!(false));
        assert_eq!(meta["dropped"], json!(0));
        assert_eq!(meta["cap"], json!(10));
    }

    #[test]
    fn meta_carries_the_attribution_vocabulary() {
        let meta = meta_value(&select_top_k(Vec::new(), 4));
        assert_eq!(meta["attribution"]["gates"], json!("measured"));
        assert_eq!(
            meta["attribution"]["reprice"],
            json!("measured-upper-bound")
        );
    }

    #[test]
    fn row_shape_is_the_wire_contract() {
        let sel = select_top_k(vec![sample("0xabc", "triangular", 96, Some(1240))], 10);
        let row = &rows_value(&sel)[0];
        assert_eq!(row["route_hash"], json!("0xabc"));
        assert_eq!(row["route_kind"], json!("triangular"));
        assert_eq!(row["hops"], json!(3));
        assert_eq!(row["stages"]["gates_us"], json!(96));
        assert_eq!(row["total_us"], json!(1336));
    }
}
