//! Static Strategy×Hop admissibility table — workbook QUOTEBASE-264 sheet
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
pub static STRATEGY_HOP_MASKS: [(&str, u8); 264] = [
    ("MEV-01-001", 63),
    ("MEV-01-002", 63),
    ("MEV-01-003", 63),
    ("MEV-01-004", 63),
    ("MEV-01-005", 63),
    ("MEV-01-006", 63),
    ("MEV-01-007", 63),
    ("MEV-01-008", 63),
    ("MEV-01-009", 63),
    ("MEV-01-010", 63),
    ("MEV-01-011", 63),
    ("MEV-01-012", 63),
    ("MEV-01-013", 63),
    ("MEV-01-014", 63),
    ("MEV-01-015", 1),
    ("MEV-01-016", 2),
    ("MEV-01-017", 4),
    ("MEV-01-018", 62),
    ("MEV-01-019", 62),
    ("MEV-01-020", 62),
    ("MEV-01-021", 62),
    ("MEV-01-022", 62),
    ("MEV-01-023", 63),
    ("MEV-01-024", 63),
    ("MEV-01-025", 63),
    ("MEV-01-026", 63),
    ("MEV-01-027", 62),
    ("MEV-01-028", 63),
    ("MEV-01-029", 62),
    ("MEV-01-030", 63),
    ("MEV-01-031", 63),
    ("MEV-01-032", 63),
    ("MEV-01-033", 63),
    ("MEV-01-034", 63),
    ("MEV-01-035", 63),
    ("MEV-01-036", 63),
    ("MEV-02-001", 63),
    ("MEV-02-002", 63),
    ("MEV-02-003", 63),
    ("MEV-02-004", 63),
    ("MEV-02-005", 63),
    ("MEV-02-006", 63),
    ("MEV-02-007", 63),
    ("MEV-02-008", 63),
    ("MEV-02-009", 63),
    ("MEV-02-010", 63),
    ("MEV-02-011", 63),
    ("MEV-02-012", 63),
    ("MEV-02-013", 63),
    ("MEV-02-014", 63),
    ("MEV-02-015", 63),
    ("MEV-02-016", 63),
    ("MEV-02-017", 63),
    ("MEV-03-001", 63),
    ("MEV-03-002", 63),
    ("MEV-03-003", 63),
    ("MEV-03-004", 63),
    ("MEV-03-005", 63),
    ("MEV-03-006", 63),
    ("MEV-03-007", 63),
    ("MEV-03-008", 63),
    ("MEV-03-009", 63),
    ("MEV-03-010", 63),
    ("MEV-03-011", 63),
    ("MEV-03-012", 63),
    ("MEV-03-013", 63),
    ("MEV-03-014", 63),
    ("MEV-03-015", 63),
    ("MEV-03-016", 63),
    ("MEV-03-017", 63),
    ("MEV-03-018", 63),
    ("MEV-03-019", 63),
    ("MEV-03-020", 63),
    ("MEV-03-021", 63),
    ("MEV-03-022", 63),
    ("MEV-03-023", 63),
    ("MEV-03-024", 63),
    ("MEV-03-025", 63),
    ("MEV-03-026", 62),
    ("MEV-03-027", 63),
    ("MEV-03-028", 63),
    ("MEV-03-029", 63),
    ("MEV-03-030", 63),
    ("MEV-03-031", 63),
    ("MEV-04-001", 63),
    ("MEV-04-002", 63),
    ("MEV-04-003", 63),
    ("MEV-04-004", 63),
    ("MEV-04-005", 63),
    ("MEV-04-006", 63),
    ("MEV-04-007", 63),
    ("MEV-04-008", 63),
    ("MEV-04-009", 63),
    ("MEV-04-010", 63),
    ("MEV-04-011", 63),
    ("MEV-04-012", 63),
    ("MEV-04-013", 63),
    ("MEV-04-014", 62),
    ("MEV-04-015", 63),
    ("MEV-04-016", 63),
    ("MEV-04-017", 63),
    ("MEV-04-018", 63),
    ("MEV-04-019", 63),
    ("MEV-04-020", 63),
    ("MEV-04-021", 63),
    ("MEV-04-022", 63),
    ("MEV-04-023", 63),
    ("MEV-04-024", 63),
    ("MEV-04-025", 63),
    ("MEV-04-026", 63),
    ("MEV-04-027", 63),
    ("MEV-04-028", 6),
    ("MEV-04-029", 63),
    ("MEV-04-030", 63),
    ("MEV-04-031", 63),
    ("MEV-05-001", 63),
    ("MEV-05-002", 63),
    ("MEV-05-003", 2),
    ("MEV-05-004", 63),
    ("MEV-05-005", 63),
    ("MEV-05-006", 63),
    ("MEV-05-007", 63),
    ("MEV-05-008", 63),
    ("MEV-05-009", 63),
    ("MEV-05-010", 63),
    ("MEV-05-011", 63),
    ("MEV-05-012", 63),
    ("MEV-05-013", 63),
    ("MEV-05-014", 63),
    ("MEV-06-001", 63),
    ("MEV-06-002", 63),
    ("MEV-06-003", 63),
    ("MEV-06-004", 63),
    ("MEV-06-005", 63),
    ("MEV-06-006", 63),
    ("MEV-06-007", 63),
    ("MEV-06-008", 63),
    ("MEV-06-009", 63),
    ("MEV-06-010", 63),
    ("MEV-06-011", 2),
    ("MEV-06-012", 62),
    ("MEV-06-013", 62),
    ("MEV-06-014", 63),
    ("MEV-06-015", 63),
    ("MEV-06-016", 63),
    ("MEV-06-017", 63),
    ("MEV-06-018", 63),
    ("MEV-06-019", 63),
    ("MEV-06-020", 63),
    ("MEV-06-021", 63),
    ("MEV-06-022", 63),
    ("MEV-06-023", 63),
    ("MEV-06-024", 63),
    ("MEV-06-025", 63),
    ("MEV-06-026", 63),
    ("MEV-06-027", 63),
    ("MEV-06-028", 63),
    ("MEV-06-029", 63),
    ("MEV-06-030", 63),
    ("MEV-07-001", 31),
    ("MEV-07-002", 31),
    ("MEV-07-003", 31),
    ("MEV-07-004", 31),
    ("MEV-07-005", 31),
    ("MEV-07-006", 31),
    ("MEV-07-007", 31),
    ("MEV-07-008", 31),
    ("MEV-07-009", 31),
    ("MEV-07-010", 31),
    ("MEV-07-011", 31),
    ("MEV-07-012", 31),
    ("MEV-07-013", 31),
    ("MEV-07-014", 31),
    ("MEV-07-015", 31),
    ("MEV-07-016", 31),
    ("MEV-07-017", 31),
    ("MEV-07-018", 31),
    ("MEV-07-019", 31),
    ("MEV-07-020", 31),
    ("MEV-07-021", 31),
    ("MEV-07-022", 31),
    ("MEV-07-023", 31),
    ("MEV-07-024", 31),
    ("MEV-07-025", 31),
    ("MEV-07-026", 31),
    ("MEV-07-027", 31),
    ("MEV-07-028", 31),
    ("MEV-07-029", 31),
    ("MEV-07-030", 31),
    ("MEV-08-001", 7),
    ("MEV-08-002", 7),
    ("MEV-08-003", 7),
    ("MEV-08-004", 7),
    ("MEV-08-005", 7),
    ("MEV-08-006", 7),
    ("MEV-08-007", 7),
    ("MEV-08-008", 7),
    ("MEV-08-009", 7),
    ("MEV-08-010", 7),
    ("MEV-08-011", 7),
    ("MEV-08-012", 7),
    ("MEV-08-013", 7),
    ("MEV-08-014", 7),
    ("MEV-08-015", 7),
    ("MEV-08-016", 7),
    ("MEV-08-017", 7),
    ("MEV-08-018", 7),
    ("MEV-08-019", 7),
    ("MEV-08-020", 7),
    ("MEV-08-021", 7),
    ("MEV-08-022", 7),
    ("MEV-08-023", 7),
    ("MEV-08-024", 7),
    ("MEV-08-025", 7),
    ("MEV-09-001", 63),
    ("MEV-09-002", 63),
    ("MEV-09-003", 63),
    ("MEV-09-004", 63),
    ("MEV-09-005", 63),
    ("MEV-09-006", 63),
    ("MEV-09-007", 63),
    ("MEV-09-008", 62),
    ("MEV-09-009", 63),
    ("MEV-09-010", 63),
    ("MEV-09-011", 63),
    ("MEV-09-012", 63),
    ("MEV-09-013", 63),
    ("MEV-09-014", 63),
    ("MEV-09-015", 63),
    ("MEV-09-016", 63),
    ("MEV-09-017", 63),
    ("MEV-09-018", 63),
    ("MEV-09-019", 63),
    ("MEV-09-020", 63),
    ("MEV-10-001", 63),
    ("MEV-10-002", 63),
    ("MEV-10-003", 63),
    ("MEV-10-004", 62),
    ("MEV-10-005", 63),
    ("MEV-10-006", 63),
    ("MEV-10-007", 63),
    ("MEV-10-008", 63),
    ("MEV-10-009", 63),
    ("MEV-10-010", 63),
    ("MEV-10-011", 63),
    ("MEV-10-012", 63),
    ("MEV-10-013", 63),
    ("MEV-10-014", 63),
    ("MEV-10-015", 63),
    ("MEV-10-016", 63),
    ("MEV-10-017", 63),
    ("MEV-10-018", 63),
    ("MEV-11-001", 63),
    ("MEV-11-002", 63),
    ("MEV-11-003", 63),
    ("MEV-11-004", 63),
    ("MEV-11-005", 62),
    ("MEV-11-006", 63),
    ("MEV-11-007", 63),
    ("MEV-11-008", 63),
    ("MEV-11-009", 63),
    ("MEV-11-010", 63),
    ("MEV-11-011", 63),
    ("MEV-11-012", 63),
];

/// HopMask for a canonical strategy; `None` if the MEV_ID is unknown to the
/// workbook table (unknown ⇒ no hop is admissible).
pub fn hop_mask(mev_id: &str) -> Option<u8> {
    STRATEGY_HOP_MASKS
        .binary_search_by(|(id, _)| (*id).cmp(mev_id))
        .ok()
        .map(|i| STRATEGY_HOP_MASKS[i].1)
}

/// Whether a closed cycle of exactly `hops` legs is admissible for `mev_id`.
/// Hops outside the canonical 2..=7 range are never admissible.
pub fn hop_allowed(mev_id: &str, hops: u8) -> bool {
    if !matches!(hops, 2..=7) {
        return false;
    }
    hop_mask(mev_id).is_some_and(|m| m & (1 << (hops - 2)) != 0)
}

/// Every hop admissible for a strategy, ascending (2..=7). Empty for an
/// unknown MEV_ID — honest-empty, never a default.
pub fn allowed_hops(mev_id: &str) -> Vec<u8> {
    match hop_mask(mev_id) {
        Some(mask) => (2..=7).filter(|h| mask & (1 << (h - 2)) != 0).collect(),
        None => Vec::new(),
    }
}

/// Tightest `(min, max)` admissible hop span for a strategy after
/// intersecting its HopMask with the requested `[min_hops, max_hops]` range
/// (each side clamped to the canonical `2..=7`). `None` when the
/// intersection is empty or the MEV_ID is unknown — the caller must SKIP
/// expansion and report the skip (R8), never run a silently-empty search.
///
/// The returned pair is the mask's admissible EXTENT within the request: a
/// mask admitting only {2, 5} yields `(2, 5)`, so hops 3–4 inside the span
/// may still be enumerated by an observe-only expansion (safe
/// over-approximation — it can never under-report candidates). Exact
/// per-hop gating belongs to per-strategy dispatch (workbook
/// 15_IMPLEMENTATION_CONTRACT step 9 — XLS-QB-03 consumer:
/// `route_discovery_worker`'s multi-hop pass).
pub fn admissible_hop_bounds(mev_id: &str, min_hops: u8, max_hops: u8) -> Option<(u8, u8)> {
    let mask = hop_mask(mev_id)?;
    let lo = min_hops.max(2);
    let hi = max_hops.min(7);
    (lo..=hi)
        .filter(|h| mask & (1 << (h - 2)) != 0)
        .fold(None::<(u8, u8)>, |acc, h| match acc {
            None => Some((h, h)),
            Some((first, _)) => Some((first, h)),
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Differential fixture — generated from the SAME canonical
    /// `docs/quotebase_strategy_hop_map.json` by the SAME script. Docker
    /// builds with context=backend/, so the fixture lives inside the crate
    /// (same pattern as `cartridge/manifest_test.rs` + math_map.json).
    const FIXTURE: &str = include_str!("strategy_hop_map.fixture.json");

    /// (MEV_ID, mask, allowed hops, surface) — parsed via `serde_json::Value`
    /// so the test needs no serde derive (proc-macro) at compile time.
    fn fixture() -> Vec<(String, u8, Vec<u8>, String)> {
        let v: serde_json::Value = serde_json::from_str(FIXTURE).expect("fixture parses");
        v["rows"]
            .as_array()
            .expect("rows array")
            .iter()
            .map(|r| {
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
            })
            .collect()
    }

    /// Full table↔fixture differential: every MEV_ID resolves to the exact
    /// workbook mask, and no fixture row is missing from the table.
    #[test]
    fn table_matches_workbook_fixture_exactly() {
        let fx = fixture();
        assert_eq!(fx.len(), 264);
        assert_eq!(STRATEGY_HOP_MASKS.len(), 264);
        for row in &fx {
            assert_eq!(hop_mask(&row.0), Some(row.1), "mask drift for {}", row.0);
        }
    }

    /// Re-derives each mask from its H2..H7 columns (bit h-2) — pins the
    /// ENCODING, not just the values.
    #[test]
    fn fixture_masks_match_hop_bit_encoding() {
        for row in fixture() {
            let recomputed: u8 = row.2.iter().fold(0u8, |acc, &h| {
                assert!((2..=7).contains(&h), "hop out of range in fixture");
                acc | (1 << (h - 2))
            });
            assert_eq!(row.1, recomputed, "encoding drift for {}", row.0);
            assert_eq!(
                allowed_hops(&row.0),
                row.2,
                "allowed_hops drift for {}",
                row.0
            );
        }
    }

    /// Workbook 16_COVERAGE hop distribution — aggregate drift tripwire.
    #[test]
    fn hop_distribution_matches_16_coverage() {
        let fx = fixture();
        let dist: Vec<usize> = (2..=7)
            .map(|h| fx.iter().filter(|r| r.2.contains(&h)).count())
            .collect();
        assert_eq!(dist, vec![245, 262, 260, 233, 233, 203]);
    }

    /// Strategy counts per surface (workbook 11_STRATEGY_HOP_MAP).
    #[test]
    fn strategy_counts_per_surface_match_workbook() {
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
        ] {
            assert_eq!(
                fx.iter().filter(|r| r.3 == surface).count(),
                expected,
                "count drift for {surface}"
            );
        }
    }

    /// Spot-checks: the DEX–DEX archetype admits every hop; unknown ids and
    /// out-of-range hops are never admissible.
    #[test]
    fn spot_checks_and_rejections() {
        assert_eq!(hop_mask("MEV-01-001"), Some(63));
        assert_eq!(allowed_hops("MEV-01-001"), vec![2, 3, 4, 5, 6, 7]);
        assert!(hop_allowed("MEV-01-001", 7));

        assert_eq!(hop_mask("MEV-99-999"), None);
        assert!(allowed_hops("MEV-99-999").is_empty());
        assert!(!hop_allowed("MEV-01-001", 1));
        assert!(!hop_allowed("MEV-01-001", 8));
    }

    /// Binary-search precondition: the static table is sorted and duplicate-free.
    #[test]
    fn table_sorted_unique() {
        let ids: Vec<&str> = STRATEGY_HOP_MASKS.iter().map(|(id, _)| *id).collect();
        let mut sorted = ids.clone();
        sorted.sort_unstable();
        assert_eq!(ids, sorted);
        let dedup_len = ids.iter().collect::<std::collections::BTreeSet<_>>().len();
        assert_eq!(dedup_len, ids.len());
    }

    /// Differential: for every fixture strategy, the full-range bounds equal
    /// (first, last) of its admissible-hop list; sub-ranges intersect
    /// correctly on both edges.
    #[test]
    fn admissible_bounds_match_fixture_extent() {
        for row in fixture() {
            let hops = &row.2;
            let full = admissible_hop_bounds(&row.0, 0, 9);
            assert_eq!(full, hops.first().zip(hops.last()).map(|(&a, &b)| (a, b)));
            // Left-edge cut: raise min past the first admissible hop.
            if let Some(&first) = hops.first() {
                let cut = admissible_hop_bounds(&row.0, first + 1, 7);
                let rest: Vec<u8> = hops.iter().copied().filter(|h| *h > first).collect();
                assert_eq!(cut, rest.first().zip(rest.last()).map(|(&a, &b)| (a, b)));
            }
        }
    }

    /// XLS-QB-03 dispatch semantics: mask-all MEV-01-001 keeps the requested
    /// span; empty intersections and unknown ids return None (skip honestly).
    #[test]
    fn admissible_bounds_dispatch_semantics() {
        assert_eq!(admissible_hop_bounds("MEV-01-001", 2, 7), Some((2, 7)));
        assert_eq!(admissible_hop_bounds("MEV-01-001", 3, 5), Some((3, 5)));
        assert_eq!(admissible_hop_bounds("MEV-01-001", 0, 9), Some((2, 7)));
        // A strategy whose mask excludes the whole requested span → None.
        let fx = fixture();
        let restricted = fx
            .iter()
            .find(|r| r.2.first() == Some(&3))
            .expect("fixture carries a ≥3-hop-only strategy");
        assert_eq!(admissible_hop_bounds(&restricted.0, 2, 2), None);
        // Unknown id → None regardless of span.
        assert_eq!(admissible_hop_bounds("MEV-99-999", 2, 7), None);
    }
}
