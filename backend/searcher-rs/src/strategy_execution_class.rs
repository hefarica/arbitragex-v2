//! Static Strategy×Execution_Class table — workbook QUOTEBASE-264 sheet
//! `11_STRATEGY_HOP_MAP` col `Execution_Class` (ARBX-TW-005).
//!
//! GENERATED from `docs/quotebase_strategy_hop_map.json` by
//! `py scripts/xls/gen_execution_class_rs.py` — do not edit rows by hand;
//! regenerate. The generator refuses to emit if the source drifts (264 rows,
//! 29 distinct classes, cross-invariants with col Status).
//!
//! Execution_Class is EXECUTION-precondition ANNOTATION, not a dispatch
//! verdict: the workbook carries `EXTERNAL_DATA_REQUIRED` under BOTH
//! ROUTE_READY and NEEDS_ROUTE_DATA, so the class does NOT determine the
//! Status. Dispatch (expansion/candidate) stays keyed on col Status
//! (`strategy_dispatch_status`); the class enriches the observable reason
//! with WHAT the strategy would need to execute (R8).
//!
//! Cross-invariants pinned by the generator AND the tests below:
//! - `DETERMINISTIC_EXECUTABLE ⊆ ROUTE_READY` (37/37).
//! - `OBSERVE_ONLY` status ⟺ `OBSERVE_ONLY` class (8/8).

/// (MEV_ID, Execution_Class), sorted ascending by MEV_ID — binary-searchable.
pub static STRATEGY_EXECUTION_CLASS: [(&str, &str); 264] = [
    ("MEV-01-001", "DETERMINISTIC_EXECUTABLE"),
    ("MEV-01-002", "DETERMINISTIC_EXECUTABLE"),
    ("MEV-01-003", "DETERMINISTIC_EXECUTABLE"),
    ("MEV-01-004", "DETERMINISTIC_EXECUTABLE"),
    ("MEV-01-005", "DETERMINISTIC_EXECUTABLE"),
    ("MEV-01-006", "DETERMINISTIC_EXECUTABLE"),
    ("MEV-01-007", "DETERMINISTIC_EXECUTABLE"),
    ("MEV-01-008", "DETERMINISTIC_EXECUTABLE"),
    ("MEV-01-009", "EXTERNAL_DATA_REQUIRED"),
    ("MEV-01-010", "EXTERNAL_DATA_REQUIRED"),
    ("MEV-01-011", "EXTERNAL_DATA_REQUIRED"),
    ("MEV-01-012", "EXTERNAL_DATA_REQUIRED"),
    ("MEV-01-013", "DETERMINISTIC_EXECUTABLE"),
    ("MEV-01-014", "DETERMINISTIC_EXECUTABLE"),
    ("MEV-01-015", "DETERMINISTIC_EXECUTABLE"),
    ("MEV-01-016", "DETERMINISTIC_EXECUTABLE"),
    ("MEV-01-017", "DETERMINISTIC_EXECUTABLE"),
    ("MEV-01-018", "DETERMINISTIC_EXECUTABLE"),
    ("MEV-01-019", "DETERMINISTIC_EXECUTABLE"),
    ("MEV-01-020", "DETERMINISTIC_EXECUTABLE"),
    ("MEV-01-021", "DETERMINISTIC_EXECUTABLE"),
    ("MEV-01-022", "DETERMINISTIC_EXECUTABLE"),
    ("MEV-01-023", "DETERMINISTIC_EXECUTABLE"),
    ("MEV-01-024", "DETERMINISTIC_EXECUTABLE"),
    ("MEV-01-025", "DETERMINISTIC_EXECUTABLE"),
    ("MEV-01-026", "DETERMINISTIC_EXECUTABLE"),
    ("MEV-01-027", "DETERMINISTIC_IF_REDEEMABLE"),
    ("MEV-01-028", "DETERMINISTIC_IF_REDEEMABLE"),
    ("MEV-01-029", "DETERMINISTIC_EXECUTABLE"),
    ("MEV-01-030", "DETERMINISTIC_SETTLEMENT"),
    ("MEV-01-031", "DETERMINISTIC_EXECUTABLE"),
    ("MEV-01-032", "DETERMINISTIC_EXECUTABLE"),
    ("MEV-01-033", "DETERMINISTIC_EXECUTABLE"),
    ("MEV-01-034", "DETERMINISTIC_EXECUTABLE"),
    ("MEV-01-035", "DETERMINISTIC_EXECUTABLE"),
    ("MEV-01-036", "DETERMINISTIC_EXECUTABLE"),
    ("MEV-02-001", "DETERMINISTIC_EXECUTABLE"),
    ("MEV-02-002", "DETERMINISTIC_EXECUTABLE"),
    ("MEV-02-003", "DETERMINISTIC_EXECUTABLE"),
    ("MEV-02-004", "DETERMINISTIC_EXECUTABLE"),
    ("MEV-02-005", "DETERMINISTIC_EXECUTABLE"),
    ("MEV-02-006", "DETERMINISTIC_EXECUTABLE"),
    ("MEV-02-007", "DETERMINISTIC_WITH_ORACLE"),
    ("MEV-02-008", "DETERMINISTIC_IF_ADAPTER"),
    ("MEV-02-009", "DETERMINISTIC_IF_ADAPTER"),
    ("MEV-02-010", "DETERMINISTIC_EXECUTABLE"),
    ("MEV-02-011", "DETERMINISTIC_IF_ADAPTER"),
    ("MEV-02-012", "DETERMINISTIC_WITH_DERIVATIVE_STATE"),
    ("MEV-02-013", "DETERMINISTIC_IF_ADAPTER"),
    ("MEV-02-014", "DETERMINISTIC_POST_STATE"),
    ("MEV-02-015", "DETERMINISTIC_SETTLEMENT"),
    ("MEV-02-016", "DETERMINISTIC_SETTLEMENT"),
    ("MEV-02-017", "DETERMINISTIC_EXECUTABLE"),
    ("MEV-03-001", "DETERMINISTIC_POST_STATE"),
    ("MEV-03-002", "DETERMINISTIC_POST_STATE"),
    ("MEV-03-003", "DETERMINISTIC_POST_STATE"),
    ("MEV-03-004", "DETERMINISTIC_POST_STATE"),
    ("MEV-03-005", "DETERMINISTIC_POST_STATE"),
    ("MEV-03-006", "DETERMINISTIC_POST_STATE"),
    ("MEV-03-007", "LATENCY_SENSITIVE"),
    ("MEV-03-008", "LATENCY_SENSITIVE"),
    ("MEV-03-009", "DETERMINISTIC_POST_ORACLE"),
    ("MEV-03-010", "DETERMINISTIC_POST_ORACLE"),
    ("MEV-03-011", "DETERMINISTIC_POST_STATE"),
    ("MEV-03-012", "DETERMINISTIC_POST_STATE"),
    ("MEV-03-013", "DETERMINISTIC_POST_STATE"),
    ("MEV-03-014", "DETERMINISTIC_POST_STATE"),
    ("MEV-03-015", "DETERMINISTIC_POST_STATE"),
    ("MEV-03-016", "DETERMINISTIC_POST_STATE"),
    ("MEV-03-017", "DETERMINISTIC_POST_STATE"),
    ("MEV-03-018", "DETERMINISTIC_POST_STATE"),
    ("MEV-03-019", "DETERMINISTIC_POST_STATE"),
    ("MEV-03-020", "DETERMINISTIC_POST_STATE"),
    ("MEV-03-021", "DETERMINISTIC_POST_STATE"),
    ("MEV-03-022", "DETERMINISTIC_POST_STATE"),
    ("MEV-03-023", "DETERMINISTIC_POST_STATE"),
    ("MEV-03-024", "DETERMINISTIC_POST_STATE"),
    ("MEV-03-025", "DETERMINISTIC_AUCTION"),
    ("MEV-03-026", "DETERMINISTIC_AUCTION"),
    ("MEV-03-027", "LATENCY_SENSITIVE"),
    ("MEV-03-028", "LATENCY_SENSITIVE"),
    ("MEV-03-029", "OBSERVE_ONLY"),
    ("MEV-03-030", "OBSERVE_ONLY"),
    ("MEV-03-031", "DETERMINISTIC_POST_STATE"),
    ("MEV-04-001", "DETERMINISTIC_IF_REDEEMABLE"),
    ("MEV-04-002", "DETERMINISTIC_IF_REDEEMABLE"),
    ("MEV-04-003", "DETERMINISTIC_IF_REDEEMABLE"),
    ("MEV-04-004", "DETERMINISTIC_IF_REDEEMABLE"),
    ("MEV-04-005", "DETERMINISTIC_IF_REDEEMABLE"),
    ("MEV-04-006", "DETERMINISTIC_IF_CONVERTIBLE"),
    ("MEV-04-007", "DETERMINISTIC_IF_CONVERTIBLE"),
    ("MEV-04-008", "DETERMINISTIC_IF_CONVERTIBLE"),
    ("MEV-04-009", "DETERMINISTIC_IF_CONVERTIBLE"),
    ("MEV-04-010", "DETERMINISTIC_IF_REDEEMABLE"),
    ("MEV-04-011", "DETERMINISTIC_IF_REDEEMABLE"),
    ("MEV-04-012", "DETERMINISTIC_IF_REDEEMABLE"),
    ("MEV-04-013", "DETERMINISTIC_IF_REDEEMABLE"),
    ("MEV-04-014", "DETERMINISTIC_IF_REDEEMABLE"),
    ("MEV-04-015", "DETERMINISTIC_IF_REDEEMABLE"),
    ("MEV-04-016", "DETERMINISTIC_IF_REDEEMABLE"),
    ("MEV-04-017", "DETERMINISTIC_IF_REDEEMABLE"),
    ("MEV-04-018", "DETERMINISTIC_IF_REDEEMABLE"),
    ("MEV-04-019", "SETTLEMENT_DELAY_SENSITIVE"),
    ("MEV-04-020", "SETTLEMENT_DELAY_SENSITIVE"),
    ("MEV-04-021", "SETTLEMENT_DELAY_SENSITIVE"),
    ("MEV-04-022", "SETTLEMENT_DELAY_SENSITIVE"),
    ("MEV-04-023", "DETERMINISTIC_IF_CONVERTIBLE"),
    ("MEV-04-024", "DETERMINISTIC_IF_SETTLEABLE"),
    ("MEV-04-025", "DETERMINISTIC_IF_REDEEMABLE"),
    ("MEV-04-026", "DETERMINISTIC_IF_REDEEMABLE"),
    ("MEV-04-027", "DETERMINISTIC_IF_REDEEMABLE"),
    ("MEV-04-028", "DETERMINISTIC_IF_REDEEMABLE"),
    ("MEV-04-029", "DETERMINISTIC_IF_SETTLEABLE"),
    ("MEV-04-030", "DETERMINISTIC_IF_SETTLEABLE"),
    ("MEV-04-031", "OBSERVE_ONLY"),
    ("MEV-05-001", "EXTERNAL_DATA_REQUIRED"),
    ("MEV-05-002", "EXTERNAL_DATA_REQUIRED"),
    ("MEV-05-003", "EXTERNAL_DATA_REQUIRED"),
    ("MEV-05-004", "EXTERNAL_DATA_REQUIRED"),
    ("MEV-05-005", "EXTERNAL_DATA_REQUIRED"),
    ("MEV-05-006", "EXTERNAL_DATA_REQUIRED"),
    ("MEV-05-007", "EXTERNAL_DATA_REQUIRED"),
    ("MEV-05-008", "EXTERNAL_DATA_REQUIRED"),
    ("MEV-05-009", "LATENCY_SENSITIVE"),
    ("MEV-05-010", "LATENCY_SENSITIVE"),
    ("MEV-05-011", "EXTERNAL_DATA_REQUIRED"),
    ("MEV-05-012", "EXTERNAL_DATA_REQUIRED"),
    ("MEV-05-013", "EXTERNAL_DATA_REQUIRED"),
    ("MEV-05-014", "EXTERNAL_DATA_REQUIRED"),
    ("MEV-06-001", "NONATOMIC_INVENTORY_REQUIRED"),
    ("MEV-06-002", "NONATOMIC_INVENTORY_REQUIRED"),
    ("MEV-06-003", "NONATOMIC_INVENTORY_REQUIRED"),
    ("MEV-06-004", "NONATOMIC_INVENTORY_REQUIRED"),
    ("MEV-06-005", "NONATOMIC_INVENTORY_REQUIRED"),
    ("MEV-06-006", "NONATOMIC_INVENTORY_REQUIRED"),
    ("MEV-06-007", "NONATOMIC_INVENTORY_REQUIRED"),
    ("MEV-06-008", "NONATOMIC_BRIDGE_REQUIRED"),
    ("MEV-06-009", "NONATOMIC_BRIDGE_REQUIRED"),
    ("MEV-06-010", "NONATOMIC_INVENTORY_REQUIRED"),
    ("MEV-06-011", "NONATOMIC_INVENTORY_REQUIRED"),
    ("MEV-06-012", "NONATOMIC_BRIDGE_REQUIRED"),
    ("MEV-06-013", "NONATOMIC_BRIDGE_REQUIRED"),
    ("MEV-06-014", "NONATOMIC_BRIDGE_REQUIRED"),
    ("MEV-06-015", "NONATOMIC_BRIDGE_REQUIRED"),
    ("MEV-06-016", "NONATOMIC_BRIDGE_REQUIRED"),
    ("MEV-06-017", "NONATOMIC_BRIDGE_REQUIRED"),
    ("MEV-06-018", "NONATOMIC_BRIDGE_REQUIRED"),
    ("MEV-06-019", "NONATOMIC_BRIDGE_REQUIRED"),
    ("MEV-06-020", "NONATOMIC_BRIDGE_REQUIRED"),
    ("MEV-06-021", "NONATOMIC_BRIDGE_REQUIRED"),
    ("MEV-06-022", "NONATOMIC_BRIDGE_REQUIRED"),
    ("MEV-06-023", "NONATOMIC_INVENTORY_REQUIRED"),
    ("MEV-06-024", "NONATOMIC_BRIDGE_REQUIRED"),
    ("MEV-06-025", "NONATOMIC_BRIDGE_REQUIRED"),
    ("MEV-06-026", "NONATOMIC_BRIDGE_REQUIRED"),
    ("MEV-06-027", "NONATOMIC_INVENTORY_REQUIRED"),
    ("MEV-06-028", "NONATOMIC_INVENTORY_REQUIRED"),
    ("MEV-06-029", "EXTERNAL_SETTLEMENT_REQUIRED"),
    ("MEV-06-030", "EXTERNAL_SETTLEMENT_REQUIRED"),
    ("MEV-07-001", "DERIVATIVE_DATA_REQUIRED"),
    ("MEV-07-002", "DERIVATIVE_DATA_REQUIRED"),
    ("MEV-07-003", "DERIVATIVE_DATA_REQUIRED"),
    ("MEV-07-004", "DERIVATIVE_DATA_REQUIRED"),
    ("MEV-07-005", "DERIVATIVE_DATA_REQUIRED"),
    ("MEV-07-006", "DERIVATIVE_DATA_REQUIRED"),
    ("MEV-07-007", "DERIVATIVE_DATA_REQUIRED"),
    ("MEV-07-008", "DERIVATIVE_DATA_REQUIRED"),
    ("MEV-07-009", "DERIVATIVE_DATA_REQUIRED"),
    ("MEV-07-010", "DERIVATIVE_DATA_REQUIRED"),
    ("MEV-07-011", "DERIVATIVE_DATA_REQUIRED"),
    ("MEV-07-012", "DERIVATIVE_DATA_REQUIRED"),
    ("MEV-07-013", "DERIVATIVE_DATA_REQUIRED"),
    ("MEV-07-014", "DERIVATIVE_DATA_REQUIRED"),
    ("MEV-07-015", "DERIVATIVE_DATA_REQUIRED"),
    ("MEV-07-016", "DERIVATIVE_DATA_REQUIRED"),
    ("MEV-07-017", "DERIVATIVE_DATA_REQUIRED"),
    ("MEV-07-018", "DERIVATIVE_DATA_REQUIRED"),
    ("MEV-07-019", "DERIVATIVE_DATA_REQUIRED"),
    ("MEV-07-020", "DERIVATIVE_DATA_REQUIRED"),
    ("MEV-07-021", "DERIVATIVE_DATA_REQUIRED"),
    ("MEV-07-022", "DERIVATIVE_DATA_REQUIRED"),
    ("MEV-07-023", "DERIVATIVE_DATA_REQUIRED"),
    ("MEV-07-024", "DERIVATIVE_DATA_REQUIRED"),
    ("MEV-07-025", "DERIVATIVE_DATA_REQUIRED"),
    ("MEV-07-026", "DERIVATIVE_DATA_REQUIRED"),
    ("MEV-07-027", "DERIVATIVE_DATA_REQUIRED"),
    ("MEV-07-028", "DERIVATIVE_DATA_REQUIRED"),
    ("MEV-07-029", "DERIVATIVE_DATA_REQUIRED"),
    ("MEV-07-030", "DERIVATIVE_DATA_REQUIRED"),
    ("MEV-08-001", "DETERMINISTIC_IF_POSITIONS"),
    ("MEV-08-002", "DETERMINISTIC_IF_POSITIONS"),
    ("MEV-08-003", "DETERMINISTIC_IF_POSITIONS"),
    ("MEV-08-004", "DETERMINISTIC_IF_POSITIONS"),
    ("MEV-08-005", "DETERMINISTIC_IF_POSITIONS"),
    ("MEV-08-006", "DETERMINISTIC_IF_REDEEMABLE"),
    ("MEV-08-007", "DETERMINISTIC_IF_REDEEMABLE"),
    ("MEV-08-008", "DETERMINISTIC_IF_REDEEMABLE"),
    ("MEV-08-009", "DETERMINISTIC_IF_POSITIONS"),
    ("MEV-08-010", "DETERMINISTIC_IF_POSITIONS"),
    ("MEV-08-011", "DETERMINISTIC_POSITION_STRATEGY"),
    ("MEV-08-012", "DETERMINISTIC_LIQUIDATION"),
    ("MEV-08-013", "DETERMINISTIC_LIQUIDATION"),
    ("MEV-08-014", "DETERMINISTIC_LIQUIDATION"),
    ("MEV-08-015", "DETERMINISTIC_LIQUIDATION"),
    ("MEV-08-016", "DETERMINISTIC_LIQUIDATION"),
    ("MEV-08-017", "DETERMINISTIC_LIQUIDATION"),
    ("MEV-08-018", "DETERMINISTIC_AUCTION"),
    ("MEV-08-019", "DETERMINISTIC_AUCTION"),
    ("MEV-08-020", "DETERMINISTIC_AUCTION"),
    ("MEV-08-021", "DETERMINISTIC_AUCTION"),
    ("MEV-08-022", "DETERMINISTIC_IF_REDEEMABLE"),
    ("MEV-08-023", "DETERMINISTIC_IF_REDEEMABLE"),
    ("MEV-08-024", "DETERMINISTIC_LIQUIDATION"),
    ("MEV-08-025", "DETERMINISTIC_IF_POSITIONS"),
    ("MEV-09-001", "DETERMINISTIC_SETTLEMENT"),
    ("MEV-09-002", "DETERMINISTIC_SETTLEMENT"),
    ("MEV-09-003", "DETERMINISTIC_SETTLEMENT"),
    ("MEV-09-004", "DETERMINISTIC_SETTLEMENT"),
    ("MEV-09-005", "DETERMINISTIC_SETTLEMENT"),
    ("MEV-09-006", "DETERMINISTIC_SETTLEMENT"),
    ("MEV-09-007", "DETERMINISTIC_SETTLEMENT"),
    ("MEV-09-008", "DETERMINISTIC_SETTLEMENT"),
    ("MEV-09-009", "DETERMINISTIC_AUCTION"),
    ("MEV-09-010", "AUTHORIZED_FLOW_ONLY"),
    ("MEV-09-011", "AUTHORIZED_FLOW_ONLY"),
    ("MEV-09-012", "DETERMINISTIC_SETTLEMENT"),
    ("MEV-09-013", "DETERMINISTIC_SETTLEMENT"),
    ("MEV-09-014", "DETERMINISTIC_SETTLEMENT"),
    ("MEV-09-015", "DETERMINISTIC_SETTLEMENT"),
    ("MEV-09-016", "DETERMINISTIC_SETTLEMENT"),
    ("MEV-09-017", "AUTHORIZED_FLOW_ONLY"),
    ("MEV-09-018", "AUTHORIZED_FLOW_ONLY"),
    ("MEV-09-019", "OBSERVE_ONLY"),
    ("MEV-09-020", "OBSERVE_ONLY"),
    ("MEV-10-001", "DETERMINISTIC_IF_FIRM_BID"),
    ("MEV-10-002", "SIGNAL_UNLESS_FIRM_EXIT"),
    ("MEV-10-003", "SIGNAL_UNLESS_FIRM_EXIT"),
    ("MEV-10-004", "SIGNAL_UNLESS_FIRM_EXIT"),
    ("MEV-10-005", "DETERMINISTIC_IF_FIRM_BID"),
    ("MEV-10-006", "DETERMINISTIC_IF_FIRM_EXIT"),
    ("MEV-10-007", "DETERMINISTIC_IF_FIRM_EXIT"),
    ("MEV-10-008", "DETERMINISTIC_IF_FIRM_BID"),
    ("MEV-10-009", "SIGNAL_UNLESS_FIRM_EXIT"),
    ("MEV-10-010", "DETERMINISTIC_IF_REDEEMABLE"),
    ("MEV-10-011", "DETERMINISTIC_IF_REDEEMABLE"),
    ("MEV-10-012", "DETERMINISTIC_IF_FIRM_BID"),
    ("MEV-10-013", "SIGNAL_UNLESS_FIRM_EXIT"),
    ("MEV-10-014", "DETERMINISTIC_IF_FIRM_BID"),
    ("MEV-10-015", "DETERMINISTIC_IF_FIRM_BID"),
    ("MEV-10-016", "DETERMINISTIC_IF_FIRM_BID"),
    ("MEV-10-017", "DETERMINISTIC_IF_REDEEMABLE"),
    ("MEV-10-018", "DETERMINISTIC_IF_REDEEMABLE"),
    ("MEV-11-001", "DETERMINISTIC_IF_COMPLETE_SET"),
    ("MEV-11-002", "DETERMINISTIC_IF_COMPLETE_SET"),
    ("MEV-11-003", "EXTERNAL_DATA_REQUIRED"),
    ("MEV-11-004", "DETERMINISTIC_IF_PAYOFF_MODEL"),
    ("MEV-11-005", "DETERMINISTIC_IF_COMPLETE_SET"),
    ("MEV-11-006", "DETERMINISTIC_IF_PAYOFF_MODEL"),
    ("MEV-11-007", "DETERMINISTIC_IF_PAYOFF_MODEL"),
    ("MEV-11-008", "DETERMINISTIC_IF_PAYOFF_MODEL"),
    ("MEV-11-009", "OBSERVE_ONLY"),
    ("MEV-11-010", "OBSERVE_ONLY"),
    ("MEV-11-011", "OBSERVE_ONLY"),
    ("MEV-11-012", "DETERMINISTIC_IF_MATCHED_CLAIM"),
];

/// Workbook Execution_Class for a canonical strategy; `None` if the MEV_ID is
/// unknown to the workbook table.
pub fn execution_class(mev_id: &str) -> Option<&'static str> {
    STRATEGY_EXECUTION_CLASS
        .binary_search_by(|(id, _)| (*id).cmp(mev_id))
        .ok()
        .map(|i| STRATEGY_EXECUTION_CLASS[i].1)
}

/// Per-class strategy counts, DERIVED from the table (name-ascending). The
/// 29-class census is computed, never hardcoded — workbook drift changes it
/// here and trips the differential test.
pub fn class_counts() -> &'static [(&'static str, usize)] {
    static C: std::sync::OnceLock<Vec<(&'static str, usize)>> = std::sync::OnceLock::new();
    C.get_or_init(|| {
        let mut v: Vec<(&'static str, usize)> = Vec::new();
        for (_, ec) in STRATEGY_EXECUTION_CLASS {
            match v.iter_mut().find(|slot| slot.0 == ec) {
                Some(slot) => slot.1 += 1,
                None => v.push((ec, 1)),
            }
        }
        v.sort_unstable();
        v
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Differential fixture — generated from the SAME canonical
    /// `docs/quotebase_strategy_hop_map.json` by the SAME script.
    const FIXTURE: &str = include_str!("strategy_execution_class.fixture.json");

    /// (MEV_ID, class, status) — parsed via `serde_json::Value` so the test
    /// needs no serde derive (proc-macro) at compile time.
    fn fixture() -> Vec<(String, String, String)> {
        let v: serde_json::Value = serde_json::from_str(FIXTURE).expect("fixture parses");
        v["rows"]
            .as_array()
            .expect("rows array")
            .iter()
            .map(|r| {
                (
                    r["m"].as_str().expect("m").to_string(),
                    r["ec"].as_str().expect("ec").to_string(),
                    r["st"].as_str().expect("st").to_string(),
                )
            })
            .collect()
    }

    /// Full table↔fixture differential: every MEV_ID resolves to the exact
    /// workbook class, and no fixture row is missing from the table.
    #[test]
    fn table_matches_workbook_fixture_exactly() {
        let fx = fixture();
        assert_eq!(fx.len(), 264);
        assert_eq!(STRATEGY_EXECUTION_CLASS.len(), 264);
        for (m, ec, _) in &fx {
            assert_eq!(
                execution_class(m),
                Some(ec.as_str()),
                "class drift for {}",
                m
            );
        }
    }

    /// 29-class census consistency: fixture-derived == table-derived, 29
    /// distinct classes covering all 264 strategies (workbook tripwire).
    #[test]
    fn class_census_matches_workbook() {
        let fx = fixture();
        let mut from_fixture: Vec<(String, usize)> = Vec::new();
        for (_, ec, _) in &fx {
            match from_fixture.iter_mut().find(|slot| slot.0 == *ec) {
                Some(slot) => slot.1 += 1,
                None => from_fixture.push((ec.clone(), 1)),
            }
        }
        from_fixture.sort();
        let derived = class_counts();
        assert_eq!(derived.len(), 29, "distinct class drift");
        assert_eq!(
            derived.iter().map(|(_, c)| c).sum::<usize>(),
            264,
            "census must cover all 264 strategies"
        );
        for ((dn, dc), (fn_, fc)) in derived.iter().zip(from_fixture.iter()) {
            assert_eq!(*dn, fn_.as_str(), "census name mismatch");
            assert_eq!(*dc, *fc, "census count drift for {}", dn);
        }
    }

    /// Cross-invariant 1 (workbook): every DETERMINISTIC_EXECUTABLE strategy
    /// is ROUTE_READY — the execution-eligible archetype never appears
    /// blocked for route data. Pinned data-level via the fixture's status
    /// column (same source JSON as strategy_dispatch_status's fixture, and
    /// the generator re-asserts it pre-emission).
    #[test]
    fn deterministic_executable_implies_route_ready() {
        for (m, ec, st) in fixture() {
            if ec == "DETERMINISTIC_EXECUTABLE" {
                assert_eq!(st, "ROUTE_READY", "workbook invariant broken for {}", m);
            }
        }
    }

    /// Cross-invariant 2 (workbook): OBSERVE_ONLY status ⟺ OBSERVE_ONLY
    /// class — the telemetry-only state is coherent across both columns.
    #[test]
    fn observe_only_status_iff_class() {
        for (m, ec, st) in fixture() {
            let is_obs_class = ec == "OBSERVE_ONLY";
            let is_obs_status = st == "OBSERVE_ONLY";
            assert_eq!(
                is_obs_class, is_obs_status,
                "OBSERVE_ONLY coherence broken for {}",
                m
            );
        }
    }

    /// Binary-search precondition: the static table is sorted and duplicate-free.
    #[test]
    fn table_sorted_unique() {
        let ids: Vec<&str> = STRATEGY_EXECUTION_CLASS.iter().map(|(id, _)| *id).collect();
        let mut sorted = ids.clone();
        sorted.sort_unstable();
        assert_eq!(ids, sorted);
        let dedup_len = ids.iter().collect::<std::collections::BTreeSet<_>>().len();
        assert_eq!(dedup_len, ids.len());
    }
}
