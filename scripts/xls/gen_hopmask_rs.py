"""Genera backend/searcher-rs/src/strategy_hop_mask.rs + fixture diferencial.

Fuente canónica: docs/quotebase_strategy_hop_map.json (264 filas, hoja
11_STRATEGY_HOP_MAP del workbook QUOTEBASE-264). El build Docker de searcher-rs
usa context=backend/, así que el fixture vive DENTRO del crate (mismo patrón que
cartridge/manifest_test.rs con cartridges/manifests/math_map.json).

Valida antes de emitir (fail-fast, sin emitir nada si falla):
- 264 filas, MEV_ID únicos y ascendentes
- HopMask_u8 == bits recomputados H2..H7 (bit h-2)
- distribución por hop == 16_COVERAGE: 245/262/260/233/233/203

Uso: py scripts/xls/gen_hopmask_rs.py
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
OUT_RS = ROOT / "backend" / "searcher-rs" / "src" / "strategy_hop_mask.rs"
OUT_FIXTURE = ROOT / "backend" / "searcher-rs" / "src" / "strategy_hop_map.fixture.json"

EXPECTED_HOP_DIST = {2: 245, 3: 262, 4: 260, 5: 233, 6: 233, 7: 203}
EXPECTED_SURFACE_COUNTS = {
    "DEX_AMM": 53, "DEX_STATE": 31, "PARITY_REDEMPTION": 31, "CEX_DEX": 14,
    "CROSS_CHAIN": 30, "DERIVATIVES": 30, "LENDING": 25, "INTENT_AUCTION": 20,
    "NFT": 18, "PREDICTION": 12,
}


def htrue(v):
    return v is True or str(v).strip().upper() == "TRUE"


def load_and_validate():
    rows = json.loads(SRC.read_text(encoding="utf-8"))
    assert len(rows) == 264, f"expected 264 rows, got {len(rows)}"
    ids = [r["MEV_ID"] for r in rows]
    assert len(set(ids)) == 264, "duplicate MEV_ID"
    assert ids == sorted(ids), "MEV_IDs not ascending"

    out = []
    for r in rows:
        hops = [h for h in range(2, 8) if htrue(r.get(f"H{h}"))]
        mask = 0
        for h in hops:
            mask |= 1 << (h - 2)
        assert int(r["HopMask_u8"]) == mask, f"{r['MEV_ID']}: mask {r['HopMask_u8']} != recomputed {mask}"
        out.append({
            "m": r["MEV_ID"],
            "mask": mask,
            "h": hops,
            "s": r["Surface"],
        })

    dist = {h: sum(1 for r in out if h in r["h"]) for h in range(2, 8)}
    assert dist == EXPECTED_HOP_DIST, f"hop distribution drift: {dist}"

    surf_counts = {}
    for r in out:
        surf_counts[r["s"]] = surf_counts.get(r["s"], 0) + 1
    assert surf_counts == EXPECTED_SURFACE_COUNTS, f"surface counts drift: {surf_counts}"

    return out


RUST_TEMPLATE = '''//! Static Strategy×Hop admissibility table — workbook QUOTEBASE-264 sheet
//! `11_STRATEGY_HOP_MAP` (XLS-QB-02).
//!
//! GENERATED from `docs/quotebase_strategy_hop_map.json` by
//! `py scripts/xls/gen_hopmask_rs.py` — do not edit rows by hand; regenerate.
//! The generator refuses to emit if the source drifts from the workbook
//! aggregates (264 rows, hop distribution 245/262/260/233/233/203).
//!
//! `HopMask_u8` encoding: bit `h-2` for `h in 2..=7` (mask 63 = all hops).
//! 264 strategies → 1,436 valid Strategy×Hop combos. The differential tests
//! below parse the committed fixture (same generator, same source) and pin
//! the table to it, so hand-edits surface as CI failures.
//!
//! Hot-path contract (workbook 15_IMPLEMENTATION_CONTRACT step 9): the mask
//! test is O(1) per (strategy, hop) and happens BEFORE any route expansion.

/// (MEV_ID, HopMask_u8), sorted ascending by MEV_ID — binary-searchable.
pub static STRATEGY_HOP_MASKS: [(&str, u8); {n}] = [
{rows}
];

/// HopMask for a canonical strategy; `None` if the MEV_ID is unknown to the
/// workbook table (unknown ⇒ no hop is admissible).
pub fn hop_mask(mev_id: &str) -> Option<u8> {{
    STRATEGY_HOP_MASKS
        .binary_search_by(|(id, _)| (*id).cmp(mev_id))
        .ok()
        .map(|i| STRATEGY_HOP_MASKS[i].1)
}}

/// Whether a closed cycle of exactly `hops` legs is admissible for `mev_id`.
/// Hops outside the canonical 2..=7 range are never admissible.
pub fn hop_allowed(mev_id: &str, hops: u8) -> bool {{
    if !matches!(hops, 2..=7) {{
        return false;
    }}
    hop_mask(mev_id).is_some_and(|m| m & (1 << (hops - 2)) != 0)
}}

/// Every hop admissible for a strategy, ascending (2..=7). Empty for an
/// unknown MEV_ID — honest-empty, never a default.
pub fn allowed_hops(mev_id: &str) -> Vec<u8> {{
    match hop_mask(mev_id) {{
        Some(mask) => (2..=7).filter(|h| mask & (1 << (h - 2)) != 0).collect(),
        None => Vec::new(),
    }}
}}

#[cfg(test)]
mod tests {{
    use super::*;

    /// Differential fixture — generated from the SAME canonical
    /// `docs/quotebase_strategy_hop_map.json` by the SAME script. Docker
    /// builds with context=backend/, so the fixture lives inside the crate
    /// (same pattern as `cartridge/manifest_test.rs` + math_map.json).
    const FIXTURE: &str = include_str!("strategy_hop_map.fixture.json");

    /// (MEV_ID, mask, allowed hops, surface) — parsed via `serde_json::Value`
    /// so the test needs no serde derive (proc-macro) at compile time.
    fn fixture() -> Vec<(String, u8, Vec<u8>, String)> {{
        let v: serde_json::Value = serde_json::from_str(FIXTURE).expect("fixture parses");
        v["rows"]
            .as_array()
            .expect("rows array")
            .iter()
            .map(|r| {{
                (
                    r["m"].as_str().expect("m").to_string(),
                    r["mask"].as_u64().expect("mask") as u8,
                    r["h"]
                        .as_array()
                        .expect("h")
                        .iter()
                        .map(|h| h.as_u64().expect("hop") as u8)
                        .collect(),
                    r["s"].as_str().expect("s").to_string(),
                )
            }})
            .collect()
    }}

    /// Full table↔fixture differential: every MEV_ID resolves to the exact
    /// workbook mask, and no fixture row is missing from the table.
    #[test]
    fn table_matches_workbook_fixture_exactly() {{
        let fx = fixture();
        assert_eq!(fx.len(), 264);
        assert_eq!(STRATEGY_HOP_MASKS.len(), 264);
        for row in &fx {{
            assert_eq!(hop_mask(&row.0), Some(row.1), "mask drift for {{}}", row.0);
        }}
    }}

    /// Re-derives each mask from its H2..H7 columns (bit h-2) — pins the
    /// ENCODING, not just the values.
    #[test]
    fn fixture_masks_match_hop_bit_encoding() {{
        for row in fixture() {{
            let recomputed: u8 = row.2.iter().fold(0u8, |acc, &h| {{
                assert!((2..=7).contains(&h), "hop out of range in fixture");
                acc | (1 << (h - 2))
            }});
            assert_eq!(row.1, recomputed, "encoding drift for {{}}", row.0);
            assert_eq!(
                allowed_hops(&row.0),
                row.2,
                "allowed_hops drift for {{}}",
                row.0
            );
        }}
    }}

    /// Workbook 16_COVERAGE hop distribution — aggregate drift tripwire.
    #[test]
    fn hop_distribution_matches_16_coverage() {{
        let fx = fixture();
        let dist: Vec<usize> = (2..=7)
            .map(|h| fx.iter().filter(|r| r.2.contains(&h)).count())
            .collect();
        assert_eq!(dist, vec![245, 262, 260, 233, 233, 203]);
    }}

    /// Strategy counts per surface (workbook 11_STRATEGY_HOP_MAP).
    #[test]
    fn strategy_counts_per_surface_match_workbook() {{
        let fx = fixture();
        for (surface, expected) in [
            ("DEX_AMM", 53usize),
            ("DEX_STATE", 31),
            ("PARITY_REDEMPTION", 31),
            ("CEX_DEX", 14),
            ("CROSS_CHAIN", 30),
            ("DERIVATIVES", 30),
            ("LENDING", 25),
            ("INTENT_AUCTION", 20),
            ("NFT", 18),
            ("PREDICTION", 12),
        ] {{
            assert_eq!(
                fx.iter().filter(|r| r.3 == surface).count(),
                expected,
                "count drift for {{surface}}"
            );
        }}
    }}

    /// Spot-checks: the DEX–DEX archetype admits every hop; unknown ids and
    /// out-of-range hops are never admissible.
    #[test]
    fn spot_checks_and_rejections() {{
        assert_eq!(hop_mask("MEV-01-001"), Some(63));
        assert_eq!(allowed_hops("MEV-01-001"), vec![2, 3, 4, 5, 6, 7]);
        assert!(hop_allowed("MEV-01-001", 7));

        assert_eq!(hop_mask("MEV-99-999"), None);
        assert!(allowed_hops("MEV-99-999").is_empty());
        assert!(!hop_allowed("MEV-01-001", 1));
        assert!(!hop_allowed("MEV-01-001", 8));
    }}

    /// Binary-search precondition: the static table is sorted and duplicate-free.
    #[test]
    fn table_sorted_unique() {{
        let ids: Vec<&str> = STRATEGY_HOP_MASKS.iter().map(|(id, _)| *id).collect();
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
    table = "\n".join(f'    ("{r["m"]}", {r["mask"]}),' for r in rows)
    rust = RUST_TEMPLATE.format(n=len(rows), rows=table)
    fixture = {
        "_source": "docs/quotebase_strategy_hop_map.json (hoja 11_STRATEGY_HOP_MAP, workbook QUOTEBASE-264)",
        "_generator": "scripts/xls/gen_hopmask_rs.py",
        "rows": rows,
    }
    OUT_RS.write_text(rust, encoding="utf-8", newline="\n")
    OUT_FIXTURE.write_text(json.dumps(fixture, indent=1), encoding="utf-8", newline="\n")
    print(f"OK  {OUT_RS.relative_to(ROOT)}  ({len(rows)} filas)")
    print(f"OK  {OUT_FIXTURE.relative_to(ROOT)}")
    total = sum(len(r["h"]) for r in rows)
    print(f"    combos válidos Strategy×Hop: {total} (esperado 1436: {'OK' if total == 1436 else 'DRIFT'})")


if __name__ == "__main__":
    sys.exit(main())
