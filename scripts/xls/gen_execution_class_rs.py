"""Genera backend/searcher-rs/src/strategy_execution_class.rs + fixture diferencial.

Fuente canónica: docs/quotebase_strategy_hop_map.json (264 filas, hoja
11_STRATEGY_HOP_MAP col Execution_Class del workbook QUOTEBASE-264). El build
Docker de searcher-rs usa context=backend/, así que el fixture vive DENTRO del
crate (mismo patrón que strategy_hop_mask.rs / strategy_dispatch_status.rs).

Valida antes de emitir (fail-fast, sin emitir nada si falla):
- 264 filas, MEV_ID únicos y ascendentes
- 29 clases distintas (vocabulario del workbook)
- invariantes cruzadas con col Status:
  * DETERMINISTIC_EXECUTABLE ⊆ ROUTE_READY (37/37)
  * OBSERVE_ONLY(status) ⟺ OBSERVE_ONLY(clase) (8/8)

Semántica (ARBX-TW-005): Execution_Class es ANOTACIÓN de precondiciones de
EJECUCIÓN — NO determina Status (EXTERNAL_DATA_REQUIRED aparece en ambos).
El dispatch (candidato/expansión) sigue keyeado por col Status
(strategy_dispatch_status.rs); la clase enriquece la razón observable de lo
que la estrategia necesitaría para ejecutar (R8).

Uso: py scripts/xls/gen_execution_class_rs.py
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
OUT_RS = ROOT / "backend" / "searcher-rs" / "src" / "strategy_execution_class.rs"
OUT_FIXTURE = ROOT / "backend" / "searcher-rs" / "src" / "strategy_execution_class.fixture.json"

EXPECTED_DISTINCT = 29
EXPECTED_STATUS = {
    "ROUTE_READY": 79, "NEEDS_ROUTE_DATA": 174,
    "OBSERVE_ONLY": 8, "NO_COMPATIBLE_ROUTE": 3,
}


def load_and_validate():
    rows = json.loads(SRC.read_text(encoding="utf-8"))
    assert len(rows) == 264, f"expected 264 rows, got {len(rows)}"
    ids = [r["MEV_ID"] for r in rows]
    assert len(set(ids)) == 264, "duplicate MEV_ID"
    assert ids == sorted(ids), "MEV_IDs not ascending"

    out = []
    for r in rows:
        ec = str(r["Execution_Class"]).strip().upper()
        assert ec, f"{r['MEV_ID']}: empty Execution_Class"
        out.append({
            "m": r["MEV_ID"],
            "ec": ec,
            "st": str(r["Status"]).strip().upper(),
        })

    classes = {r["ec"] for r in out}
    assert len(classes) == EXPECTED_DISTINCT, (
        f"distinct Execution_Class drift: {len(classes)} != {EXPECTED_DISTINCT}"
    )

    # Invariantes cruzadas col Status (validadas con el propio source).
    st_counts = {}
    for r in out:
        st_counts[r["st"]] = st_counts.get(r["st"], 0) + 1
    assert st_counts == EXPECTED_STATUS, f"status counts drift: {st_counts}"

    de = [r for r in out if r["ec"] == "DETERMINISTIC_EXECUTABLE"]
    assert all(r["st"] == "ROUTE_READY" for r in de), (
        "DETERMINISTIC_EXECUTABLE must be a subset of ROUTE_READY"
    )
    obs_st = {r["m"] for r in out if r["st"] == "OBSERVE_ONLY"}
    obs_ec = {r["m"] for r in out if r["ec"] == "OBSERVE_ONLY"}
    assert obs_st == obs_ec, "OBSERVE_ONLY status must match OBSERVE_ONLY class"

    return out


RUST_TEMPLATE = '''//! Static Strategy×Execution_Class table — workbook QUOTEBASE-264 sheet
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
pub static STRATEGY_EXECUTION_CLASS: [(&str, &str); {n}] = [
{rows}
];

/// Workbook Execution_Class for a canonical strategy; `None` if the MEV_ID is
/// unknown to the workbook table.
pub fn execution_class(mev_id: &str) -> Option<&'static str> {{
    STRATEGY_EXECUTION_CLASS
        .binary_search_by(|(id, _)| (*id).cmp(mev_id))
        .ok()
        .map(|i| STRATEGY_EXECUTION_CLASS[i].1)
}}

/// Per-class strategy counts, DERIVED from the table (name-ascending). The
/// 29-class census is computed, never hardcoded — workbook drift changes it
/// here and trips the differential test.
pub fn class_counts() -> &'static [(&'static str, usize)] {{
    static C: std::sync::OnceLock<Vec<(&'static str, usize)>> = std::sync::OnceLock::new();
    C.get_or_init(|| {{
        let mut v: Vec<(&'static str, usize)> = Vec::new();
        for (_, ec) in STRATEGY_EXECUTION_CLASS {{
            match v.iter_mut().find(|slot| slot.0 == ec) {{
                Some(slot) => slot.1 += 1,
                None => v.push((ec, 1)),
            }}
        }}
        v.sort_unstable();
        v
    }})
}}

#[cfg(test)]
mod tests {{
    use super::*;

    /// Differential fixture — generated from the SAME canonical
    /// `docs/quotebase_strategy_hop_map.json` by the SAME script.
    const FIXTURE: &str = include_str!("strategy_execution_class.fixture.json");

    /// (MEV_ID, class, status) — parsed via `serde_json::Value` so the test
    /// needs no serde derive (proc-macro) at compile time.
    fn fixture() -> Vec<(String, String, String)> {{
        let v: serde_json::Value = serde_json::from_str(FIXTURE).expect("fixture parses");
        v["rows"]
            .as_array()
            .expect("rows array")
            .iter()
            .map(|r| {{
                (
                    r["m"].as_str().expect("m").to_string(),
                    r["ec"].as_str().expect("ec").to_string(),
                    r["st"].as_str().expect("st").to_string(),
                )
            }})
            .collect()
    }}

    /// Full table↔fixture differential: every MEV_ID resolves to the exact
    /// workbook class, and no fixture row is missing from the table.
    #[test]
    fn table_matches_workbook_fixture_exactly() {{
        let fx = fixture();
        assert_eq!(fx.len(), 264);
        assert_eq!(STRATEGY_EXECUTION_CLASS.len(), 264);
        for (m, ec, _) in &fx {{
            assert_eq!(
                execution_class(m),
                Some(ec.as_str()),
                "class drift for {{}}",
                m
            );
        }}
    }}

    /// 29-class census consistency: fixture-derived == table-derived, 29
    /// distinct classes covering all 264 strategies (workbook tripwire).
    #[test]
    fn class_census_matches_workbook() {{
        let fx = fixture();
        let mut from_fixture: Vec<(String, usize)> = Vec::new();
        for (_, ec, _) in &fx {{
            match from_fixture.iter_mut().find(|slot| slot.0 == *ec) {{
                Some(slot) => slot.1 += 1,
                None => from_fixture.push((ec.clone(), 1)),
            }}
        }}
        from_fixture.sort();
        let derived = class_counts();
        assert_eq!(derived.len(), 29, "distinct class drift");
        assert_eq!(
            derived.iter().map(|(_, c)| c).sum::<usize>(),
            264,
            "census must cover all 264 strategies"
        );
        for ((dn, dc), (fn_, fc)) in derived.iter().zip(from_fixture.iter()) {{
            assert_eq!(*dn, fn_.as_str(), "census name mismatch");
            assert_eq!(*dc, *fc, "census count drift for {{}}", dn);
        }}
    }}

    /// Cross-invariant 1 (workbook): every DETERMINISTIC_EXECUTABLE strategy
    /// is ROUTE_READY — the execution-eligible archetype never appears
    /// blocked for route data. Pinned data-level via the fixture's status
    /// column (same source JSON as strategy_dispatch_status's fixture, and
    /// the generator re-asserts it pre-emission).
    #[test]
    fn deterministic_executable_implies_route_ready() {{
        for (m, ec, st) in fixture() {{
            if ec == "DETERMINISTIC_EXECUTABLE" {{
                assert_eq!(st, "ROUTE_READY", "workbook invariant broken for {{}}", m);
            }}
        }}
    }}

    /// Cross-invariant 2 (workbook): OBSERVE_ONLY status ⟺ OBSERVE_ONLY
    /// class — the telemetry-only state is coherent across both columns.
    #[test]
    fn observe_only_status_iff_class() {{
        for (m, ec, st) in fixture() {{
            let is_obs_class = ec == "OBSERVE_ONLY";
            let is_obs_status = st == "OBSERVE_ONLY";
            assert_eq!(
                is_obs_class, is_obs_status,
                "OBSERVE_ONLY coherence broken for {{}}",
                m
            );
        }}
    }}

    /// Binary-search precondition: the static table is sorted and duplicate-free.
    #[test]
    fn table_sorted_unique() {{
        let ids: Vec<&str> = STRATEGY_EXECUTION_CLASS.iter().map(|(id, _)| *id).collect();
        let mut sorted = ids.clone();
        sorted.sort_unstable();
        assert_eq!(ids, sorted);
        let dedup_len = ids.iter().collect::<std::collections::BTreeSet<_>>().len();
        assert_eq!(dedup_len, ids.len());
    }}
}}
'''


def main():
    rows = load_and_validate()
    table = "\n".join(f'    ("{r["m"]}", "{r["ec"]}"),' for r in rows)
    rust = RUST_TEMPLATE.format(n=len(rows), rows=table)
    fixture = {
        "_source": "docs/quotebase_strategy_hop_map.json (hoja 11_STRATEGY_HOP_MAP col Execution_Class, workbook QUOTEBASE-264)",
        "_generator": "scripts/xls/gen_execution_class_rs.py",
        "rows": rows,
    }
    OUT_RS.write_text(rust, encoding="utf-8", newline="\n")
    OUT_FIXTURE.write_text(json.dumps(fixture, indent=1), encoding="utf-8", newline="\n")
    print(f"OK  {OUT_RS.relative_to(ROOT)}  ({len(rows)} filas)")
    print(f"OK  {OUT_FIXTURE.relative_to(ROOT)}")
    classes = {}
    for r in rows:
        classes[r["ec"]] = classes.get(r["ec"], 0) + 1
    print(f"    clases distintas: {len(classes)} (esperado {EXPECTED_DISTINCT})")


if __name__ == "__main__":
    sys.exit(main())
