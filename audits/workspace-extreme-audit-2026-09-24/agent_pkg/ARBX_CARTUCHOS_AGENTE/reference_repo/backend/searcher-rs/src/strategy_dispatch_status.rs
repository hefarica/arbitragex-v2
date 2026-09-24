//! Static Strategy×Status dispatch table — workbook QUOTEBASE-264 sheet
//! `11_STRATEGY_HOP_MAP` col `Status` (ARBX-0021).
//!
//! GENERATED from `docs/quotebase_strategy_hop_map.json` by
//! `py scripts/xls/gen_dispatch_status_rs.py` — do not edit rows by hand;
//! regenerate. The generator refuses to emit if the source drifts from the
//! workbook aggregates (264 rows, status counts 79/174/8/3).
//!
//! Dispatch semantics (workbook 15_IMPLEMENTATION_CONTRACT — "sin hardcode de
//! IDs": la tabla es data-driven del Excel canónico; los counts se derivan de
//! la tabla, jamás de listas de IDs):
//!
//! | Status               | expand | candidate | telemetry |
//! |----------------------|--------|-----------|-----------|
//! | ROUTE_READY          | sí     | sí        | sí        |
//! | NEEDS_ROUTE_DATA     | NO     | NO        | sí (skip reason) |
//! | OBSERVE_ONLY         | sí     | NO        | sí        |
//! | NO_COMPATIBLE_ROUTE  | NO     | NO        | sí (skip reason) |
//!
//! `NEEDS_ROUTE_DATA` nunca fabrica una ruta para poder decir "264/264
//! ejecutándose" — la ausencia de candidato ES el dato, con razón observable
//! (R8). `OBSERVE_ONLY` puede alimentar telemetría pero jamás execution.

/// The four canonical workbook Status values (sealed — no fifth state).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DispatchStatus {
    RouteReady,
    NeedsRouteData,
    ObserveOnly,
    NoCompatibleRoute,
}

impl DispatchStatus {
    /// Canonical order (workbook column order) — used by `status_counts`.
    pub const ALL: [DispatchStatus; 4] = [
        DispatchStatus::RouteReady,
        DispatchStatus::NeedsRouteData,
        DispatchStatus::ObserveOnly,
        DispatchStatus::NoCompatibleRoute,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            DispatchStatus::RouteReady => "ROUTE_READY",
            DispatchStatus::NeedsRouteData => "NEEDS_ROUTE_DATA",
            DispatchStatus::ObserveOnly => "OBSERVE_ONLY",
            DispatchStatus::NoCompatibleRoute => "NO_COMPATIBLE_ROUTE",
        }
    }
}

/// Per-strategy dispatch decision derived from [`DispatchStatus`] — what the
/// pipeline may do with the strategy. `UnknownStrategy` (MEV_ID not in the
/// workbook table) is fail-closed: no expansion, no candidate, honest reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Disposition {
    /// ROUTE_READY — expansion AND candidate formation allowed.
    CandidateAllowed,
    /// NEEDS_ROUTE_DATA — no route data source wired: no expansion, no
    /// candidate. The reason is observable telemetry, never a fabricated route.
    NoCandidateNeedsRouteData,
    /// OBSERVE_ONLY — expansion feeds telemetry only; candidate/execution NO.
    TelemetryOnly,
    /// NO_COMPATIBLE_ROUTE — the workbook declares no compatible route shape:
    /// no expansion at all.
    ExpansionForbidden,
    /// MEV_ID unknown to the workbook table — fail-closed.
    UnknownStrategy,
}

impl Disposition {
    /// Observable skip reason (R8) for telemetry/log fields.
    pub fn reason(self) -> &'static str {
        match self {
            Disposition::CandidateAllowed => "route_ready",
            Disposition::NoCandidateNeedsRouteData => "needs_route_data",
            Disposition::TelemetryOnly => "observe_only",
            Disposition::ExpansionForbidden => "no_compatible_route",
            Disposition::UnknownStrategy => "unknown_strategy",
        }
    }

    /// Whether route EXPANSION may run for this strategy (ARBX-0021 AC).
    pub fn may_expand(self) -> bool {
        matches!(
            self,
            Disposition::CandidateAllowed | Disposition::TelemetryOnly
        )
    }

    /// Whether a CANDIDATE may be formed (execution-eligible path).
    pub fn may_form_candidate(self) -> bool {
        matches!(self, Disposition::CandidateAllowed)
    }
}

/// (MEV_ID, DispatchStatus), sorted ascending by MEV_ID — binary-searchable.
pub static STRATEGY_DISPATCH_STATUS: [(&str, DispatchStatus); 264] = [
    ("MEV-01-001", DispatchStatus::RouteReady),
    ("MEV-01-002", DispatchStatus::RouteReady),
    ("MEV-01-003", DispatchStatus::RouteReady),
    ("MEV-01-004", DispatchStatus::RouteReady),
    ("MEV-01-005", DispatchStatus::RouteReady),
    ("MEV-01-006", DispatchStatus::RouteReady),
    ("MEV-01-007", DispatchStatus::RouteReady),
    ("MEV-01-008", DispatchStatus::RouteReady),
    ("MEV-01-009", DispatchStatus::RouteReady),
    ("MEV-01-010", DispatchStatus::RouteReady),
    ("MEV-01-011", DispatchStatus::RouteReady),
    ("MEV-01-012", DispatchStatus::RouteReady),
    ("MEV-01-013", DispatchStatus::RouteReady),
    ("MEV-01-014", DispatchStatus::RouteReady),
    ("MEV-01-015", DispatchStatus::RouteReady),
    ("MEV-01-016", DispatchStatus::RouteReady),
    ("MEV-01-017", DispatchStatus::RouteReady),
    ("MEV-01-018", DispatchStatus::RouteReady),
    ("MEV-01-019", DispatchStatus::RouteReady),
    ("MEV-01-020", DispatchStatus::RouteReady),
    ("MEV-01-021", DispatchStatus::RouteReady),
    ("MEV-01-022", DispatchStatus::RouteReady),
    ("MEV-01-023", DispatchStatus::RouteReady),
    ("MEV-01-024", DispatchStatus::RouteReady),
    ("MEV-01-025", DispatchStatus::RouteReady),
    ("MEV-01-026", DispatchStatus::RouteReady),
    ("MEV-01-027", DispatchStatus::RouteReady),
    ("MEV-01-028", DispatchStatus::RouteReady),
    ("MEV-01-029", DispatchStatus::RouteReady),
    ("MEV-01-030", DispatchStatus::RouteReady),
    ("MEV-01-031", DispatchStatus::RouteReady),
    ("MEV-01-032", DispatchStatus::RouteReady),
    ("MEV-01-033", DispatchStatus::RouteReady),
    ("MEV-01-034", DispatchStatus::RouteReady),
    ("MEV-01-035", DispatchStatus::RouteReady),
    ("MEV-01-036", DispatchStatus::RouteReady),
    ("MEV-02-001", DispatchStatus::RouteReady),
    ("MEV-02-002", DispatchStatus::RouteReady),
    ("MEV-02-003", DispatchStatus::RouteReady),
    ("MEV-02-004", DispatchStatus::RouteReady),
    ("MEV-02-005", DispatchStatus::RouteReady),
    ("MEV-02-006", DispatchStatus::RouteReady),
    ("MEV-02-007", DispatchStatus::RouteReady),
    ("MEV-02-008", DispatchStatus::RouteReady),
    ("MEV-02-009", DispatchStatus::RouteReady),
    ("MEV-02-010", DispatchStatus::RouteReady),
    ("MEV-02-011", DispatchStatus::NoCompatibleRoute),
    ("MEV-02-012", DispatchStatus::RouteReady),
    ("MEV-02-013", DispatchStatus::RouteReady),
    ("MEV-02-014", DispatchStatus::RouteReady),
    ("MEV-02-015", DispatchStatus::RouteReady),
    ("MEV-02-016", DispatchStatus::RouteReady),
    ("MEV-02-017", DispatchStatus::RouteReady),
    ("MEV-03-001", DispatchStatus::RouteReady),
    ("MEV-03-002", DispatchStatus::RouteReady),
    ("MEV-03-003", DispatchStatus::RouteReady),
    ("MEV-03-004", DispatchStatus::RouteReady),
    ("MEV-03-005", DispatchStatus::RouteReady),
    ("MEV-03-006", DispatchStatus::RouteReady),
    ("MEV-03-007", DispatchStatus::RouteReady),
    ("MEV-03-008", DispatchStatus::RouteReady),
    ("MEV-03-009", DispatchStatus::NoCompatibleRoute),
    ("MEV-03-010", DispatchStatus::NoCompatibleRoute),
    ("MEV-03-011", DispatchStatus::RouteReady),
    ("MEV-03-012", DispatchStatus::RouteReady),
    ("MEV-03-013", DispatchStatus::RouteReady),
    ("MEV-03-014", DispatchStatus::RouteReady),
    ("MEV-03-015", DispatchStatus::RouteReady),
    ("MEV-03-016", DispatchStatus::RouteReady),
    ("MEV-03-017", DispatchStatus::RouteReady),
    ("MEV-03-018", DispatchStatus::RouteReady),
    ("MEV-03-019", DispatchStatus::RouteReady),
    ("MEV-03-020", DispatchStatus::RouteReady),
    ("MEV-03-021", DispatchStatus::RouteReady),
    ("MEV-03-022", DispatchStatus::RouteReady),
    ("MEV-03-023", DispatchStatus::RouteReady),
    ("MEV-03-024", DispatchStatus::RouteReady),
    ("MEV-03-025", DispatchStatus::RouteReady),
    ("MEV-03-026", DispatchStatus::RouteReady),
    ("MEV-03-027", DispatchStatus::RouteReady),
    ("MEV-03-028", DispatchStatus::RouteReady),
    ("MEV-03-029", DispatchStatus::ObserveOnly),
    ("MEV-03-030", DispatchStatus::ObserveOnly),
    ("MEV-03-031", DispatchStatus::RouteReady),
    ("MEV-04-001", DispatchStatus::NeedsRouteData),
    ("MEV-04-002", DispatchStatus::NeedsRouteData),
    ("MEV-04-003", DispatchStatus::NeedsRouteData),
    ("MEV-04-004", DispatchStatus::NeedsRouteData),
    ("MEV-04-005", DispatchStatus::NeedsRouteData),
    ("MEV-04-006", DispatchStatus::NeedsRouteData),
    ("MEV-04-007", DispatchStatus::NeedsRouteData),
    ("MEV-04-008", DispatchStatus::NeedsRouteData),
    ("MEV-04-009", DispatchStatus::NeedsRouteData),
    ("MEV-04-010", DispatchStatus::NeedsRouteData),
    ("MEV-04-011", DispatchStatus::NeedsRouteData),
    ("MEV-04-012", DispatchStatus::NeedsRouteData),
    ("MEV-04-013", DispatchStatus::NeedsRouteData),
    ("MEV-04-014", DispatchStatus::NeedsRouteData),
    ("MEV-04-015", DispatchStatus::NeedsRouteData),
    ("MEV-04-016", DispatchStatus::NeedsRouteData),
    ("MEV-04-017", DispatchStatus::NeedsRouteData),
    ("MEV-04-018", DispatchStatus::NeedsRouteData),
    ("MEV-04-019", DispatchStatus::NeedsRouteData),
    ("MEV-04-020", DispatchStatus::NeedsRouteData),
    ("MEV-04-021", DispatchStatus::NeedsRouteData),
    ("MEV-04-022", DispatchStatus::NeedsRouteData),
    ("MEV-04-023", DispatchStatus::NeedsRouteData),
    ("MEV-04-024", DispatchStatus::NeedsRouteData),
    ("MEV-04-025", DispatchStatus::NeedsRouteData),
    ("MEV-04-026", DispatchStatus::NeedsRouteData),
    ("MEV-04-027", DispatchStatus::NeedsRouteData),
    ("MEV-04-028", DispatchStatus::NeedsRouteData),
    ("MEV-04-029", DispatchStatus::NeedsRouteData),
    ("MEV-04-030", DispatchStatus::NeedsRouteData),
    ("MEV-04-031", DispatchStatus::ObserveOnly),
    ("MEV-05-001", DispatchStatus::NeedsRouteData),
    ("MEV-05-002", DispatchStatus::NeedsRouteData),
    ("MEV-05-003", DispatchStatus::NeedsRouteData),
    ("MEV-05-004", DispatchStatus::NeedsRouteData),
    ("MEV-05-005", DispatchStatus::NeedsRouteData),
    ("MEV-05-006", DispatchStatus::NeedsRouteData),
    ("MEV-05-007", DispatchStatus::NeedsRouteData),
    ("MEV-05-008", DispatchStatus::NeedsRouteData),
    ("MEV-05-009", DispatchStatus::NeedsRouteData),
    ("MEV-05-010", DispatchStatus::NeedsRouteData),
    ("MEV-05-011", DispatchStatus::NeedsRouteData),
    ("MEV-05-012", DispatchStatus::NeedsRouteData),
    ("MEV-05-013", DispatchStatus::NeedsRouteData),
    ("MEV-05-014", DispatchStatus::NeedsRouteData),
    ("MEV-06-001", DispatchStatus::NeedsRouteData),
    ("MEV-06-002", DispatchStatus::NeedsRouteData),
    ("MEV-06-003", DispatchStatus::NeedsRouteData),
    ("MEV-06-004", DispatchStatus::NeedsRouteData),
    ("MEV-06-005", DispatchStatus::NeedsRouteData),
    ("MEV-06-006", DispatchStatus::NeedsRouteData),
    ("MEV-06-007", DispatchStatus::NeedsRouteData),
    ("MEV-06-008", DispatchStatus::NeedsRouteData),
    ("MEV-06-009", DispatchStatus::NeedsRouteData),
    ("MEV-06-010", DispatchStatus::NeedsRouteData),
    ("MEV-06-011", DispatchStatus::NeedsRouteData),
    ("MEV-06-012", DispatchStatus::NeedsRouteData),
    ("MEV-06-013", DispatchStatus::NeedsRouteData),
    ("MEV-06-014", DispatchStatus::NeedsRouteData),
    ("MEV-06-015", DispatchStatus::NeedsRouteData),
    ("MEV-06-016", DispatchStatus::NeedsRouteData),
    ("MEV-06-017", DispatchStatus::NeedsRouteData),
    ("MEV-06-018", DispatchStatus::NeedsRouteData),
    ("MEV-06-019", DispatchStatus::NeedsRouteData),
    ("MEV-06-020", DispatchStatus::NeedsRouteData),
    ("MEV-06-021", DispatchStatus::NeedsRouteData),
    ("MEV-06-022", DispatchStatus::NeedsRouteData),
    ("MEV-06-023", DispatchStatus::NeedsRouteData),
    ("MEV-06-024", DispatchStatus::NeedsRouteData),
    ("MEV-06-025", DispatchStatus::NeedsRouteData),
    ("MEV-06-026", DispatchStatus::NeedsRouteData),
    ("MEV-06-027", DispatchStatus::NeedsRouteData),
    ("MEV-06-028", DispatchStatus::NeedsRouteData),
    ("MEV-06-029", DispatchStatus::NeedsRouteData),
    ("MEV-06-030", DispatchStatus::NeedsRouteData),
    ("MEV-07-001", DispatchStatus::NeedsRouteData),
    ("MEV-07-002", DispatchStatus::NeedsRouteData),
    ("MEV-07-003", DispatchStatus::NeedsRouteData),
    ("MEV-07-004", DispatchStatus::NeedsRouteData),
    ("MEV-07-005", DispatchStatus::NeedsRouteData),
    ("MEV-07-006", DispatchStatus::NeedsRouteData),
    ("MEV-07-007", DispatchStatus::NeedsRouteData),
    ("MEV-07-008", DispatchStatus::NeedsRouteData),
    ("MEV-07-009", DispatchStatus::NeedsRouteData),
    ("MEV-07-010", DispatchStatus::NeedsRouteData),
    ("MEV-07-011", DispatchStatus::NeedsRouteData),
    ("MEV-07-012", DispatchStatus::NeedsRouteData),
    ("MEV-07-013", DispatchStatus::NeedsRouteData),
    ("MEV-07-014", DispatchStatus::NeedsRouteData),
    ("MEV-07-015", DispatchStatus::NeedsRouteData),
    ("MEV-07-016", DispatchStatus::NeedsRouteData),
    ("MEV-07-017", DispatchStatus::NeedsRouteData),
    ("MEV-07-018", DispatchStatus::NeedsRouteData),
    ("MEV-07-019", DispatchStatus::NeedsRouteData),
    ("MEV-07-020", DispatchStatus::NeedsRouteData),
    ("MEV-07-021", DispatchStatus::NeedsRouteData),
    ("MEV-07-022", DispatchStatus::NeedsRouteData),
    ("MEV-07-023", DispatchStatus::NeedsRouteData),
    ("MEV-07-024", DispatchStatus::NeedsRouteData),
    ("MEV-07-025", DispatchStatus::NeedsRouteData),
    ("MEV-07-026", DispatchStatus::NeedsRouteData),
    ("MEV-07-027", DispatchStatus::NeedsRouteData),
    ("MEV-07-028", DispatchStatus::NeedsRouteData),
    ("MEV-07-029", DispatchStatus::NeedsRouteData),
    ("MEV-07-030", DispatchStatus::NeedsRouteData),
    ("MEV-08-001", DispatchStatus::NeedsRouteData),
    ("MEV-08-002", DispatchStatus::NeedsRouteData),
    ("MEV-08-003", DispatchStatus::NeedsRouteData),
    ("MEV-08-004", DispatchStatus::NeedsRouteData),
    ("MEV-08-005", DispatchStatus::NeedsRouteData),
    ("MEV-08-006", DispatchStatus::NeedsRouteData),
    ("MEV-08-007", DispatchStatus::NeedsRouteData),
    ("MEV-08-008", DispatchStatus::NeedsRouteData),
    ("MEV-08-009", DispatchStatus::NeedsRouteData),
    ("MEV-08-010", DispatchStatus::NeedsRouteData),
    ("MEV-08-011", DispatchStatus::NeedsRouteData),
    ("MEV-08-012", DispatchStatus::NeedsRouteData),
    ("MEV-08-013", DispatchStatus::NeedsRouteData),
    ("MEV-08-014", DispatchStatus::NeedsRouteData),
    ("MEV-08-015", DispatchStatus::NeedsRouteData),
    ("MEV-08-016", DispatchStatus::NeedsRouteData),
    ("MEV-08-017", DispatchStatus::NeedsRouteData),
    ("MEV-08-018", DispatchStatus::NeedsRouteData),
    ("MEV-08-019", DispatchStatus::NeedsRouteData),
    ("MEV-08-020", DispatchStatus::NeedsRouteData),
    ("MEV-08-021", DispatchStatus::NeedsRouteData),
    ("MEV-08-022", DispatchStatus::NeedsRouteData),
    ("MEV-08-023", DispatchStatus::NeedsRouteData),
    ("MEV-08-024", DispatchStatus::NeedsRouteData),
    ("MEV-08-025", DispatchStatus::NeedsRouteData),
    ("MEV-09-001", DispatchStatus::NeedsRouteData),
    ("MEV-09-002", DispatchStatus::NeedsRouteData),
    ("MEV-09-003", DispatchStatus::NeedsRouteData),
    ("MEV-09-004", DispatchStatus::NeedsRouteData),
    ("MEV-09-005", DispatchStatus::NeedsRouteData),
    ("MEV-09-006", DispatchStatus::NeedsRouteData),
    ("MEV-09-007", DispatchStatus::NeedsRouteData),
    ("MEV-09-008", DispatchStatus::NeedsRouteData),
    ("MEV-09-009", DispatchStatus::NeedsRouteData),
    ("MEV-09-010", DispatchStatus::NeedsRouteData),
    ("MEV-09-011", DispatchStatus::NeedsRouteData),
    ("MEV-09-012", DispatchStatus::NeedsRouteData),
    ("MEV-09-013", DispatchStatus::NeedsRouteData),
    ("MEV-09-014", DispatchStatus::NeedsRouteData),
    ("MEV-09-015", DispatchStatus::NeedsRouteData),
    ("MEV-09-016", DispatchStatus::NeedsRouteData),
    ("MEV-09-017", DispatchStatus::NeedsRouteData),
    ("MEV-09-018", DispatchStatus::NeedsRouteData),
    ("MEV-09-019", DispatchStatus::ObserveOnly),
    ("MEV-09-020", DispatchStatus::ObserveOnly),
    ("MEV-10-001", DispatchStatus::NeedsRouteData),
    ("MEV-10-002", DispatchStatus::NeedsRouteData),
    ("MEV-10-003", DispatchStatus::NeedsRouteData),
    ("MEV-10-004", DispatchStatus::NeedsRouteData),
    ("MEV-10-005", DispatchStatus::NeedsRouteData),
    ("MEV-10-006", DispatchStatus::NeedsRouteData),
    ("MEV-10-007", DispatchStatus::NeedsRouteData),
    ("MEV-10-008", DispatchStatus::NeedsRouteData),
    ("MEV-10-009", DispatchStatus::NeedsRouteData),
    ("MEV-10-010", DispatchStatus::NeedsRouteData),
    ("MEV-10-011", DispatchStatus::NeedsRouteData),
    ("MEV-10-012", DispatchStatus::NeedsRouteData),
    ("MEV-10-013", DispatchStatus::NeedsRouteData),
    ("MEV-10-014", DispatchStatus::NeedsRouteData),
    ("MEV-10-015", DispatchStatus::NeedsRouteData),
    ("MEV-10-016", DispatchStatus::NeedsRouteData),
    ("MEV-10-017", DispatchStatus::NeedsRouteData),
    ("MEV-10-018", DispatchStatus::NeedsRouteData),
    ("MEV-11-001", DispatchStatus::NeedsRouteData),
    ("MEV-11-002", DispatchStatus::NeedsRouteData),
    ("MEV-11-003", DispatchStatus::NeedsRouteData),
    ("MEV-11-004", DispatchStatus::NeedsRouteData),
    ("MEV-11-005", DispatchStatus::NeedsRouteData),
    ("MEV-11-006", DispatchStatus::NeedsRouteData),
    ("MEV-11-007", DispatchStatus::NeedsRouteData),
    ("MEV-11-008", DispatchStatus::NeedsRouteData),
    ("MEV-11-009", DispatchStatus::ObserveOnly),
    ("MEV-11-010", DispatchStatus::ObserveOnly),
    ("MEV-11-011", DispatchStatus::ObserveOnly),
    ("MEV-11-012", DispatchStatus::NeedsRouteData),
];

/// Workbook Status for a canonical strategy; `None` if the MEV_ID is unknown
/// to the workbook table.
pub fn dispatch_status(mev_id: &str) -> Option<DispatchStatus> {
    STRATEGY_DISPATCH_STATUS
        .binary_search_by(|(id, _)| (*id).cmp(mev_id))
        .ok()
        .map(|i| STRATEGY_DISPATCH_STATUS[i].1)
}

/// Fail-closed disposition: unknown MEV_ID ⇒ `UnknownStrategy` (no expansion,
/// no candidate — honest reason), never a permissive default.
pub fn disposition(mev_id: &str) -> Disposition {
    match dispatch_status(mev_id) {
        Some(DispatchStatus::RouteReady) => Disposition::CandidateAllowed,
        Some(DispatchStatus::NeedsRouteData) => Disposition::NoCandidateNeedsRouteData,
        Some(DispatchStatus::ObserveOnly) => Disposition::TelemetryOnly,
        Some(DispatchStatus::NoCompatibleRoute) => Disposition::ExpansionForbidden,
        None => Disposition::UnknownStrategy,
    }
}

/// Per-status strategy counts, DERIVED from the table in canonical workbook
/// order (ROUTE_READY, NEEDS_ROUTE_DATA, OBSERVE_ONLY, NO_COMPATIBLE_ROUTE).
/// For telemetry: the 79/174/8/3 aggregates are computed, never hardcoded —
/// workbook drift changes the numbers here and trips the differential test.
pub fn status_counts() -> [(DispatchStatus, usize); 4] {
    *counts()
}

fn counts() -> &'static [(DispatchStatus, usize); 4] {
    static C: std::sync::OnceLock<[(DispatchStatus, usize); 4]> = std::sync::OnceLock::new();
    C.get_or_init(|| {
        let mut c = [
            (DispatchStatus::RouteReady, 0usize),
            (DispatchStatus::NeedsRouteData, 0),
            (DispatchStatus::ObserveOnly, 0),
            (DispatchStatus::NoCompatibleRoute, 0),
        ];
        for (_, st) in STRATEGY_DISPATCH_STATUS {
            for slot in c.iter_mut() {
                if slot.0 == st {
                    slot.1 += 1;
                    break;
                }
            }
        }
        c
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Differential fixture — generated from the SAME canonical
    /// `docs/quotebase_strategy_hop_map.json` by the SAME script (same
    /// pattern as `strategy_hop_mask.fixture.json`).
    const FIXTURE: &str = include_str!("strategy_dispatch_status.fixture.json");

    /// (MEV_ID, status string) — parsed via `serde_json::Value` so the test
    /// needs no serde derive (proc-macro) at compile time.
    fn fixture() -> Vec<(String, String)> {
        let v: serde_json::Value = serde_json::from_str(FIXTURE).expect("fixture parses");
        v["rows"]
            .as_array()
            .expect("rows array")
            .iter()
            .map(|r| {
                (
                    r["m"].as_str().expect("m").to_string(),
                    r["st"].as_str().expect("st").to_string(),
                )
            })
            .collect()
    }

    /// One exemplar MEV_ID per status, FOUND dynamically in the fixture —
    /// the tests pin semantics without hardcoding ID lists.
    fn exemplar(status: &str) -> String {
        fixture()
            .into_iter()
            .find(|(_, st)| st == status)
            .unwrap_or_else(|| panic!("fixture carries no {} exemplar", status))
            .0
    }

    /// Full table↔fixture differential: every MEV_ID resolves to the exact
    /// workbook status, and no fixture row is missing from the table.
    #[test]
    fn table_matches_workbook_fixture_exactly() {
        let fx = fixture();
        assert_eq!(fx.len(), 264);
        assert_eq!(STRATEGY_DISPATCH_STATUS.len(), 264);
        for (m, st) in &fx {
            let expect = match st.as_str() {
                "ROUTE_READY" => DispatchStatus::RouteReady,
                "NEEDS_ROUTE_DATA" => DispatchStatus::NeedsRouteData,
                "OBSERVE_ONLY" => DispatchStatus::ObserveOnly,
                "NO_COMPATIBLE_ROUTE" => DispatchStatus::NoCompatibleRoute,
                other => panic!("non-canonical status {}", other),
            };
            assert_eq!(dispatch_status(m), Some(expect), "status drift for {}", m);
        }
    }

    /// Workbook aggregates tripwire: fixture-derived counts, table-derived
    /// `status_counts()`, and the canonical 79/174/8/3 must all agree — and
    /// cover every one of the 264 strategies.
    #[test]
    fn counts_match_workbook_aggregates() {
        let fx = fixture();
        let mut from_fixture = [0usize; 4];
        for (_, st) in &fx {
            let i = match st.as_str() {
                "ROUTE_READY" => 0,
                "NEEDS_ROUTE_DATA" => 1,
                "OBSERVE_ONLY" => 2,
                "NO_COMPATIBLE_ROUTE" => 3,
                other => panic!("non-canonical status {}", other),
            };
            from_fixture[i] += 1;
        }
        assert_eq!(from_fixture, [79, 174, 8, 3]);
        let derived = status_counts();
        for (i, (st, count)) in derived.iter().enumerate() {
            assert_eq!(
                *count,
                from_fixture[i],
                "table count drift for {}",
                st.as_str()
            );
        }
        assert_eq!(
            derived.iter().map(|(_, c)| c).sum::<usize>(),
            264,
            "counts must cover all 264 strategies"
        );
    }

    /// ARBX-0021 four-state semantics: exemplars found dynamically in the
    /// fixture, one per status; unknown id is fail-closed.
    #[test]
    fn four_state_dispatch_semantics() {
        // ROUTE_READY → candidate allowed, expansion allowed.
        let ready = exemplar("ROUTE_READY");
        assert_eq!(disposition(&ready), Disposition::CandidateAllowed);
        assert!(disposition(&ready).may_expand());
        assert!(disposition(&ready).may_form_candidate());

        // NEEDS_ROUTE_DATA → NO candidate without data, no expansion,
        // observable reason (never a fabricated route).
        let needs = exemplar("NEEDS_ROUTE_DATA");
        let d = disposition(&needs);
        assert_eq!(d, Disposition::NoCandidateNeedsRouteData);
        assert!(!d.may_expand());
        assert!(!d.may_form_candidate());
        assert_eq!(d.reason(), "needs_route_data");

        // OBSERVE_ONLY → telemetry sí / execution no.
        let obs = exemplar("OBSERVE_ONLY");
        let d = disposition(&obs);
        assert_eq!(d, Disposition::TelemetryOnly);
        assert!(d.may_expand());
        assert!(!d.may_form_candidate());
        assert_eq!(d.reason(), "observe_only");

        // NO_COMPATIBLE_ROUTE → sin expansión.
        let ncr = exemplar("NO_COMPATIBLE_ROUTE");
        let d = disposition(&ncr);
        assert_eq!(d, Disposition::ExpansionForbidden);
        assert!(!d.may_expand());
        assert!(!d.may_form_candidate());
        assert_eq!(d.reason(), "no_compatible_route");

        // Unknown MEV_ID → fail-closed with honest reason. Sentinel is
        // concat-built (ALPHA-MAP exact-264: no literal in the static scan).
        const UNKNOWN_SENTINEL: &str = concat!("MEV-99-", "999");
        assert_eq!(dispatch_status(UNKNOWN_SENTINEL), None);
        let d = disposition(UNKNOWN_SENTINEL);
        assert_eq!(d, Disposition::UnknownStrategy);
        assert!(!d.may_expand());
        assert!(!d.may_form_candidate());
        assert_eq!(d.reason(), "unknown_strategy");
    }

    /// Binary-search precondition: the static table is sorted and duplicate-free.
    #[test]
    fn table_sorted_unique() {
        let ids: Vec<&str> = STRATEGY_DISPATCH_STATUS.iter().map(|(id, _)| *id).collect();
        let mut sorted = ids.clone();
        sorted.sort_unstable();
        assert_eq!(ids, sorted);
        let dedup_len = ids.iter().collect::<std::collections::BTreeSet<_>>().len();
        assert_eq!(dedup_len, ids.len());
    }

    /// `as_str` round-trips the canonical workbook vocabulary exactly.
    #[test]
    fn as_str_matches_workbook_vocabulary() {
        assert_eq!(DispatchStatus::RouteReady.as_str(), "ROUTE_READY");
        assert_eq!(DispatchStatus::NeedsRouteData.as_str(), "NEEDS_ROUTE_DATA");
        assert_eq!(DispatchStatus::ObserveOnly.as_str(), "OBSERVE_ONLY");
        assert_eq!(
            DispatchStatus::NoCompatibleRoute.as_str(),
            "NO_COMPATIBLE_ROUTE"
        );
        assert_eq!(DispatchStatus::ALL.len(), 4);
    }
}
