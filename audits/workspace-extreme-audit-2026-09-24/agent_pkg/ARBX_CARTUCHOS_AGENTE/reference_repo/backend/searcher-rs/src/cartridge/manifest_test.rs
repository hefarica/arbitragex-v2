//! Contract tests for the cartridge math manifest (RU-2).
//!
//! `cartridges/manifests/math_map.json` is the distilled, machine-readable form
//! of the canonical Excel catalog `ArbitrageX_264_Cartridge_Math_Architecture`
//! (sheet `02_CARTRIDGE_MATH_MAP` + initial mode from `01_MEV_MATRIX_1_11`),
//! produced by `scripts/gen_math_manifest.py`. The XLSX never enters the repo —
//! only this distilled JSON does, and these tests pin its contract so the
//! 264-cartridge math map cannot drift silently. `include_str!` makes the
//! artifact a compile-time dependency: touching it recompiles and re-runs this
//! suite.

use std::collections::HashSet;

use serde::Deserialize;

/// The committed math manifest, embedded at COMPILE TIME.
const MATH_MAP_JSON: &str = include_str!("../../cartridges/manifests/math_map.json");

const EXPECTED_ENTRIES: usize = 264;
/// Excel modes are law: 160 SHADOW + 104 PAPER (§34 — no mode flips by drift).
const EXPECTED_SHADOW: usize = 160;
const EXPECTED_PAPER: usize = 104;
/// Distinct detector families across the 264 strategies (sheet 03 catalog).
const EXPECTED_DETECTOR_FAMILIES: usize = 60;

#[derive(Deserialize)]
struct ManifestEntry {
    mev_id: String,
    detector_id: String,
    primary_ops: Vec<String>,
    equation: String,
    data_bindings: String,
    frontend_toggle: String,
    mode: String,
}

fn load_entries() -> Vec<ManifestEntry> {
    serde_json::from_str(MATH_MAP_JSON).unwrap()
}

/// `MEV-<group XX>-<strategy YYY>` (e.g. `MEV-01-001`).
fn is_valid_mev_id(id: &str) -> bool {
    let Some(rest) = id.strip_prefix("MEV-") else {
        return false;
    };
    let Some((group, number)) = rest.split_once('-') else {
        return false;
    };
    group.len() == 2
        && number.len() == 3
        && group.bytes().all(|b| b.is_ascii_digit())
        && number.bytes().all(|b| b.is_ascii_digit())
}

/// `op_<01..31>` — the 31 math operators of the master catalog.
fn op_catalog_index(op: &str) -> Option<u8> {
    let number = op.strip_prefix("op_")?;
    if number.len() != 2 || !number.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    number.parse::<u8>().ok().filter(|n| (1..=31).contains(n))
}

#[test]
fn manifest_has_exactly_264_entries() {
    assert_eq!(load_entries().len(), EXPECTED_ENTRIES);
}

#[test]
fn mev_ids_are_unique_and_well_formed() {
    let entries = load_entries();
    let mut seen = HashSet::with_capacity(EXPECTED_ENTRIES);
    for entry in &entries {
        assert!(
            is_valid_mev_id(&entry.mev_id),
            "malformed mev_id {:?} — expected MEV-XX-YYY",
            entry.mev_id
        );
        assert!(
            seen.insert(entry.mev_id.as_str()),
            "duplicate mev_id {:?}",
            entry.mev_id
        );
    }
}

#[test]
fn detector_ids_are_non_empty_across_the_60_families() {
    let entries = load_entries();
    let mut families = HashSet::new();
    for entry in &entries {
        assert!(
            !entry.detector_id.trim().is_empty(),
            "{}: empty detector_id",
            entry.mev_id
        );
        families.insert(entry.detector_id.as_str());
    }
    assert_eq!(
        families.len(),
        EXPECTED_DETECTOR_FAMILIES,
        "detector families drifted from the 60-family master catalog"
    );
}

#[test]
fn primary_ops_are_within_the_31_operator_catalog() {
    for entry in load_entries() {
        assert!(
            !entry.primary_ops.is_empty(),
            "{}: no primary operators",
            entry.mev_id
        );
        for op in &entry.primary_ops {
            assert!(
                op_catalog_index(op).is_some(),
                "{}: operator {:?} outside op_01..op_31",
                entry.mev_id,
                op
            );
        }
    }
}

#[test]
fn mode_split_is_exactly_160_shadow_104_paper() {
    let entries = load_entries();
    let mut shadow = 0usize;
    let mut paper = 0usize;
    for entry in &entries {
        match entry.mode.as_str() {
            "SHADOW" => shadow += 1,
            "PAPER" => paper += 1,
            other => panic!("{}: invalid mode {:?}", entry.mev_id, other),
        }
    }
    assert_eq!(
        shadow, EXPECTED_SHADOW,
        "SHADOW count drifted from the Excel"
    );
    assert_eq!(paper, EXPECTED_PAPER, "PAPER count drifted from the Excel");
}

#[test]
fn manifest_fields_carry_the_math_contract() {
    for entry in load_entries() {
        assert!(
            !entry.equation.trim().is_empty(),
            "{}: empty equation",
            entry.mev_id
        );
        assert!(
            !entry.data_bindings.trim().is_empty(),
            "{}: empty data_bindings",
            entry.mev_id
        );
        assert!(
            !entry.frontend_toggle.trim().is_empty(),
            "{}: empty frontend_toggle",
            entry.mev_id
        );
    }
}
