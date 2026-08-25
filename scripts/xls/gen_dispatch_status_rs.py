"""Genera backend/searcher-rs/src/strategy_dispatch_status.rs + fixture diferencial.

Fuente canónica: docs/quotebase_strategy_hop_map.json (264 filas, hoja
11_STRATEGY_HOP_MAP col Status del workbook QUOTEBASE-264). El build Docker de
searcher-rs usa context=backend/, así que el fixture vive DENTRO del crate
(mismo patrón que strategy_hop_mask.rs + su fixture).

Valida antes de emitir (fail-fast, sin emitir nada si falla):
- 264 filas, MEV_ID únicos y ascendentes
- Status ∈ {ROUTE_READY, NEEDS_ROUTE_DATA, OBSERVE_ONLY, NO_COMPATIBLE_ROUTE}
- counts == agregados del workbook: 79/174/8/3

Semántica de dispatch (ARBX-0021, workbook 15_IMPLEMENTATION_CONTRACT):
  ROUTE_READY         → expansión sí, candidato sí
  NEEDS_ROUTE_DATA    → expansión NO (sin data no se fabrica ruta), razón observable
  OBSERVE_ONLY        → expansión sí (telemetría), candidato/execution NO
  NO_COMPATIBLE_ROUTE → sin expansión

Uso: py scripts/xls/gen_dispatch_status_rs.py
Override de fuente: ARBX_QUOTEBASE_JSON=/ruta/a/hop_map.json
"""
import json
import os
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SRC = Path(os.environ.get(
    "ARBX_QUOTEBASE_JSON",
    ROOT / "docs" / "quotebase_strategy_hop_map.json",
))
OUT_RS = ROOT / "backend" / "searcher-rs" / "src" / "strategy_dispatch_status.rs"
OUT_FIXTURE = ROOT / "backend" / "searcher-rs" / "src" / "strategy_dispatch_status.fixture.json"

CANONICAL_STATUSES = ["ROUTE_READY", "NEEDS_ROUTE_DATA", "OBSERVE_ONLY", "NO_COMPATIBLE_ROUTE"]
EXPECTED_COUNTS = {"ROUTE_READY": 79, "NEEDS_ROUTE_DATA": 174, "OBSERVE_ONLY": 8, "NO_COMPATIBLE_ROUTE": 3}
ENUM_NAMES = {
    "ROUTE_READY": "RouteReady",
    "NEEDS_ROUTE_DATA": "NeedsRouteData",
    "OBSERVE_ONLY": "ObserveOnly",
    "NO_COMPATIBLE_ROUTE": "NoCompatibleRoute",
}


def load_and_validate():
    rows = json.loads(SRC.read_text(encoding="utf-8"))
    assert len(rows) == 264, f"expected 264 rows, got {len(rows)}"
    ids = [r["MEV_ID"] for r in rows]
    assert len(set(ids)) == 264, "duplicate MEV_ID"
    assert ids == sorted(ids), "MEV_IDs not ascending"

    out = []
    for r in rows:
        st = str(r["Status"]).strip().upper()
        assert st in CANONICAL_STATUSES, f"{r['MEV_ID']}: status {st!r} fuera del vocabulario canónico"
        out.append({
            "m": r["MEV_ID"],
            "st": st,
        })

    counts = {}
    for r in out:
        counts[r["st"]] = counts.get(r["st"], 0) + 1
    assert counts == EXPECTED_COUNTS, f"status counts drift: {counts}"

    return out


RUST_TEMPLATE = '''//! Static Strategy×Status dispatch table — workbook QUOTEBASE-264 sheet
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
pub enum DispatchStatus {{
    RouteReady,
    NeedsRouteData,
    ObserveOnly,
    NoCompatibleRoute,
}}

impl DispatchStatus {{
    /// Canonical order (workbook column order) — used by `status_counts`.
    pub const ALL: [DispatchStatus; 4] = [
        DispatchStatus::RouteReady,
        DispatchStatus::NeedsRouteData,
        DispatchStatus::ObserveOnly,
        DispatchStatus::NoCompatibleRoute,
    ];

    pub fn as_str(self) -> &'static str {{
        match self {{
            DispatchStatus::RouteReady => "ROUTE_READY",
            DispatchStatus::NeedsRouteData => "NEEDS_ROUTE_DATA",
            DispatchStatus::ObserveOnly => "OBSERVE_ONLY",
            DispatchStatus::NoCompatibleRoute => "NO_COMPATIBLE_ROUTE",
        }}
    }}
}}

/// Per-strategy dispatch decision derived from [`DispatchStatus`] — what the
/// pipeline may do with the strategy. `UnknownStrategy` (MEV_ID not in the
/// workbook table) is fail-closed: no expansion, no candidate, honest reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Disposition {{
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
}}

impl Disposition {{
    /// Observable skip reason (R8) for telemetry/log fields.
    pub fn reason(self) -> &'static str {{
        match self {{
            Disposition::CandidateAllowed => "route_ready",
            Disposition::NoCandidateNeedsRouteData => "needs_route_data",
            Disposition::TelemetryOnly => "observe_only",
            Disposition::ExpansionForbidden => "no_compatible_route",
            Disposition::UnknownStrategy => "unknown_strategy",
        }}
    }}

    /// Whether route EXPANSION may run for this strategy (ARBX-0021 AC).
    pub fn may_expand(self) -> bool {{
        matches!(
            self,
            Disposition::CandidateAllowed | Disposition::TelemetryOnly
        )
    }}

    /// Whether a CANDIDATE may be formed (execution-eligible path).
    pub fn may_form_candidate(self) -> bool {{
        matches!(self, Disposition::CandidateAllowed)
    }}
}}

/// (MEV_ID, DispatchStatus), sorted ascending by MEV_ID — binary-searchable.
pub static STRATEGY_DISPATCH_STATUS: [(&str, DispatchStatus); {n}] = [
{rows}
];

/// Workbook Status for a canonical strategy; `None` if the MEV_ID is unknown
/// to the workbook table.
pub fn dispatch_status(mev_id: &str) -> Option<DispatchStatus> {{
    STRATEGY_DISPATCH_STATUS
        .binary_search_by(|(id, _)| (*id).cmp(mev_id))
        .ok()
        .map(|i| STRATEGY_DISPATCH_STATUS[i].1)
}}

/// Fail-closed disposition: unknown MEV_ID ⇒ `UnknownStrategy` (no expansion,
/// no candidate — honest reason), never a permissive default.
pub fn disposition(mev_id: &str) -> Disposition {{
    match dispatch_status(mev_id) {{
        Some(DispatchStatus::RouteReady) => Disposition::CandidateAllowed,
        Some(DispatchStatus::NeedsRouteData) => Disposition::NoCandidateNeedsRouteData,
        Some(DispatchStatus::ObserveOnly) => Disposition::TelemetryOnly,
        Some(DispatchStatus::NoCompatibleRoute) => Disposition::ExpansionForbidden,
        None => Disposition::UnknownStrategy,
    }}
}}

/// Per-status strategy counts, DERIVED from the table in canonical workbook
/// order (ROUTE_READY, NEEDS_ROUTE_DATA, OBSERVE_ONLY, NO_COMPATIBLE_ROUTE).
/// For telemetry: the 79/174/8/3 aggregates are computed, never hardcoded —
/// workbook drift changes the numbers here and trips the differential test.
pub fn status_counts() -> [(DispatchStatus, usize); 4] {{
    *counts()
}}

fn counts() -> &'static [(DispatchStatus, usize); 4] {{
    static C: std::sync::OnceLock<[(DispatchStatus, usize); 4]> = std::sync::OnceLock::new();
    C.get_or_init(|| {{
        let mut c = [
            (DispatchStatus::RouteReady, 0usize),
            (DispatchStatus::NeedsRouteData, 0),
            (DispatchStatus::ObserveOnly, 0),
            (DispatchStatus::NoCompatibleRoute, 0),
        ];
        for (_, st) in STRATEGY_DISPATCH_STATUS {{
            for slot in c.iter_mut() {{
                if slot.0 == st {{
                    slot.1 += 1;
                    break;
                }}
            }}
        }}
        c
    }})
}}

#[cfg(test)]
mod tests {{
    use super::*;

    /// Differential fixture — generated from the SAME canonical
    /// `docs/quotebase_strategy_hop_map.json` by the SAME script (same
    /// pattern as `strategy_hop_mask.fixture.json`).
    const FIXTURE: &str = include_str!("strategy_dispatch_status.fixture.json");

    /// (MEV_ID, status string) — parsed via `serde_json::Value` so the test
    /// needs no serde derive (proc-macro) at compile time.
    fn fixture() -> Vec<(String, String)> {{
        let v: serde_json::Value = serde_json::from_str(FIXTURE).expect("fixture parses");
        v["rows"]
            .as_array()
            .expect("rows array")
            .iter()
            .map(|r| {{
                (
                    r["m"].as_str().expect("m").to_string(),
                    r["st"].as_str().expect("st").to_string(),
                )
            }})
            .collect()
    }}

    /// One exemplar MEV_ID per status, FOUND dynamically in the fixture —
    /// the tests pin semantics without hardcoding ID lists.
    fn exemplar(status: &str) -> String {{
        fixture()
            .into_iter()
            .find(|(_, st)| st == status)
            .unwrap_or_else(|| panic!("fixture carries no {{}} exemplar", status))
            .0
    }}

    /// Full table↔fixture differential: every MEV_ID resolves to the exact
    /// workbook status, and no fixture row is missing from the table.
    #[test]
    fn table_matches_workbook_fixture_exactly() {{
        let fx = fixture();
        assert_eq!(fx.len(), 264);
        assert_eq!(STRATEGY_DISPATCH_STATUS.len(), 264);
        for (m, st) in &fx {{
            let expect = match st.as_str() {{
                "ROUTE_READY" => DispatchStatus::RouteReady,
                "NEEDS_ROUTE_DATA" => DispatchStatus::NeedsRouteData,
                "OBSERVE_ONLY" => DispatchStatus::ObserveOnly,
                "NO_COMPATIBLE_ROUTE" => DispatchStatus::NoCompatibleRoute,
                other => panic!("non-canonical status {{}}", other),
            }};
            assert_eq!(dispatch_status(m), Some(expect), "status drift for {{}}", m);
        }}
    }}

    /// Workbook aggregates tripwire: fixture-derived counts, table-derived
    /// `status_counts()`, and the canonical 79/174/8/3 must all agree — and
    /// cover every one of the 264 strategies.
    #[test]
    fn counts_match_workbook_aggregates() {{
        let fx = fixture();
        let mut from_fixture = [0usize; 4];
        for (_, st) in &fx {{
            let i = match st.as_str() {{
                "ROUTE_READY" => 0,
                "NEEDS_ROUTE_DATA" => 1,
                "OBSERVE_ONLY" => 2,
                "NO_COMPATIBLE_ROUTE" => 3,
                other => panic!("non-canonical status {{}}", other),
            }};
            from_fixture[i] += 1;
        }}
        assert_eq!(from_fixture, [79, 174, 8, 3]);
        let derived = status_counts();
        for (i, (st, count)) in derived.iter().enumerate() {{
            assert_eq!(
                *count,
                from_fixture[i],
                "table count drift for {{}}",
                st.as_str()
            );
        }}
        assert_eq!(
            derived.iter().map(|(_, c)| c).sum::<usize>(),
            264,
            "counts must cover all 264 strategies"
        );
    }}

    /// ARBX-0021 four-state semantics: exemplars found dynamically in the
    /// fixture, one per status; unknown id is fail-closed.
    #[test]
    fn four_state_dispatch_semantics() {{
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

        // Unknown MEV_ID → fail-closed with honest reason.
        assert_eq!(dispatch_status("MEV-99-999"), None);
        let d = disposition("MEV-99-999");
        assert_eq!(d, Disposition::UnknownStrategy);
        assert!(!d.may_expand());
        assert!(!d.may_form_candidate());
        assert_eq!(d.reason(), "unknown_strategy");
    }}

    /// Binary-search precondition: the static table is sorted and duplicate-free.
    #[test]
    fn table_sorted_unique() {{
        let ids: Vec<&str> = STRATEGY_DISPATCH_STATUS.iter().map(|(id, _)| *id).collect();
        let mut sorted = ids.clone();
        sorted.sort_unstable();
        assert_eq!(ids, sorted);
        let dedup_len = ids.iter().collect::<std::collections::BTreeSet<_>>().len();
        assert_eq!(dedup_len, ids.len());
    }}

    /// `as_str` round-trips the canonical workbook vocabulary exactly.
    #[test]
    fn as_str_matches_workbook_vocabulary() {{
        assert_eq!(DispatchStatus::RouteReady.as_str(), "ROUTE_READY");
        assert_eq!(DispatchStatus::NeedsRouteData.as_str(), "NEEDS_ROUTE_DATA");
        assert_eq!(DispatchStatus::ObserveOnly.as_str(), "OBSERVE_ONLY");
        assert_eq!(
            DispatchStatus::NoCompatibleRoute.as_str(),
            "NO_COMPATIBLE_ROUTE"
        );
        assert_eq!(DispatchStatus::ALL.len(), 4);
    }}
}}
'''


def main():
    rows = load_and_validate()
    table = "\n".join(
        f'    ("{r["m"]}", DispatchStatus::{ENUM_NAMES[r["st"]]}),' for r in rows
    )
    rust = RUST_TEMPLATE.format(n=len(rows), rows=table)
    fixture = {
        "_source": "docs/quotebase_strategy_hop_map.json (hoja 11_STRATEGY_HOP_MAP col Status, workbook QUOTEBASE-264)",
        "_generator": "scripts/xls/gen_dispatch_status_rs.py",
        "rows": rows,
    }
    OUT_RS.write_text(rust, encoding="utf-8", newline="\n")
    OUT_FIXTURE.write_text(json.dumps(fixture, indent=1), encoding="utf-8", newline="\n")
    print(f"OK  {OUT_RS.relative_to(ROOT)}  ({len(rows)} filas)")
    print(f"OK  {OUT_FIXTURE.relative_to(ROOT)}")
    counts = {}
    for r in rows:
        counts[r["st"]] = counts.get(r["st"], 0) + 1
    print(f"    counts: {counts} (esperado {{'ROUTE_READY': 79, 'NEEDS_ROUTE_DATA': 174, 'OBSERVE_ONLY': 8, 'NO_COMPATIBLE_ROUTE': 3}})")


if __name__ == "__main__":
    sys.exit(main())
