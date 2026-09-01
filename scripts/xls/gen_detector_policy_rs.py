"""Genera backend/searcher-rs/src/detector_policy.rs + fixture diferencial.

Fuente canónica: docs/quotebase_detector_policy.json (60 filas, hoja
13_DETECTOR_POLICY del workbook QUOTEBASE-264) + el link estrategia→detector
de docs/quotebase_strategy_hop_map.json (col Detector_ID, 264 filas). El build
Docker de searcher-rs usa context=backend/, así que el fixture vive DENTRO del
crate (mismo patrón que strategy_hop_mask.rs / strategy_dispatch_status.rs).

Valida antes de emitir (fail-fast, sin emitir nada si falla):
- 60 filas, Detector_ID únicos y ascendentes; col Strategies == conteo REAL de
  hop_map por detector (60/60, suma 264)
- los 264 Detector_ID de hop_map resuelven TODOS en la tabla (y los 60 quedan
  usados)
- invariante family-envelope: Allowed_Hops(strategy) ⊆ Hop_Use(detector) en
  las 264 filas (Min/Max_Legs NO se usan: llegan a 8, dominio distinto)
- Hop_Use parsea a lista u8 estrictamente creciente dentro de 2..=7
- vocabulario cerrado: Graph_Policy ⊆ 12 oraciones mapeadas a variantes,
  Hot_Seed ⊆ 5 modos, Do_Not_Do uniforme (1 valor en 60/60)
- coherencia OBSERVE: detector OBSERVE ⟺ graph_policy "OBSERVE_ONLY — …" ⟺
  hot_seed telemetry-only, y sus estrategias son EXACTAMENTE las 8 con Status
  OBSERVE_ONLY en hop_map
- DP-001 — columnas de contrato de ejecución: Execution_Class no vacía y
  SCREAMING, uniforme por familia (CADA estrategia del hop_map lleva la MISMA
  clase que su detector, invariante cruzada con la hoja 11) con vocabulario
  cerrado compartido 29/29 en ambas hojas; Required_Data y
  Exact_Discovery_Criterion no vacíos y distintos 60/60 (cada familia declara
  SU contrato de datos y SU criterio exacto — duplicados = drift copy/paste)

Semántica (ARBX-0026): el policy engine consume las dimensiones
GENÉRICAMENTE (sin hardcode por detector): Graph_Policy = anotación de qué
grafo/adapter familia usa; Hop_Use = envelope familiar que interseca con los
bounds por estrategia (strategy_hop_mask sigue siendo la fuente de bounds);
Do_Not_Do = guard universal (nunca reemplazar el math del detector por un
spread spot genérico); Hot_Seed = admisión de seeding (telemetry-only nunca
siembra candidato); Execution_Class = anotación de precondición de ejecución
compartida por toda la familia (NO veredicto de dispatch — misma doctrina
que strategy_execution_class); Required_Data = contrato del gate NEEDS_DATA
(los inputs que exige el criterio exacto, jamás aproximados);
Exact_Discovery_Criterion = el math propio del detector (refuerza el
Do_Not_Do universal).

Uso: py scripts/xls/gen_detector_policy_rs.py
Override de fuente: ARBX_DETECTOR_POLICY_JSON / ARBX_QUOTEBASE_JSON
"""
import json
import os
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SRC_POL = Path(os.environ.get(
    "ARBX_DETECTOR_POLICY_JSON",
    ROOT / "docs" / "quotebase_detector_policy.json",
))
SRC_HOP = Path(os.environ.get(
    "ARBX_QUOTEBASE_JSON",
    ROOT / "docs" / "quotebase_strategy_hop_map.json",
))
OUT_RS = ROOT / "backend" / "searcher-rs" / "src" / "detector_policy.rs"
OUT_FIXTURE = ROOT / "backend" / "searcher-rs" / "src" / "detector_policy.fixture.json"

EXPECTED_DETECTORS = 60
EXPECTED_STRATEGIES = 264

# Vocabulario cerrado del workbook — oración completa → variante Rust.
# Si el workbook agrega una oración nueva, el generador FALLA y obliga a
# decidir conscientemente el nombre de la variante aquí (fail-closed).
GP_VARIANTS = {
    "family detector exact criterion → compatible graph adapter": "FamilyCriterionAdapter",
    "cross-venue depth-aware pair comparison": "CrossVenueDepthPair",
    "basis/surface dislocation → instrument action graph": "BasisSurfaceInstrumentGraph",
    "state-event trigger → localized dirty subgraph": "StateEventDirtySubgraph",
    "orders/intents → candidate order/action graph": "OrdersIntentsActionGraph",
    "position/state trigger → lending/liquidation action graph": "PositionLendingLiquidationGraph",
    "prediction/claim graph; payout-equivalence filter": "PredictionClaimPayoutFilter",
    "NFT/asset venue graph; settlement token filter": "NftVenueSettlementFilter",
    "OBSERVE_ONLY — no opportunity=true": "ObserveOnlyNoOpportunity",
    "parity/redemption action edges + token valuation": "ParityRedemptionValuation",
    "dirty pair/edge → closed-cycle/order route search": "DirtyEdgeClosedCycleSearch",
    "per-domain graph + supported bridge/preposition edges": "PerDomainBridgeEdges",
}

HS_VARIANTS = {
    "Detector-specific threshold from exact criterion": "DetectorThreshold",
    "Spread/log-alpha/depth dislocation": "SpreadDislocation",
    "State change / post-event delta": "StateEventDelta",
    "No hot opportunity seed; telemetry evidence only": "TelemetryOnly",
    "Cross-domain price/settlement dislocation": "CrossDomainDislocation",
}

# Vocabulario cerrado Example_Surface (DP-002): token workbook → variante Rust.
# Es la clasificación que el RequiredDataGate runtime usa para saber qué
# CLASES de datos exige la familia (DEX_AMM → aristas del grafo; el resto aún
# sin adaptador en el tick pipeline → NotTracked honesto).
SURFACE_VARIANTS = {
    "DEX_AMM": "DexAmm",
    "PARITY_REDEMPTION": "ParityRedemption",
    "DEX_STATE": "DexState",
    "DERIVATIVES": "Derivatives",
    "LENDING": "Lending",
    "NFT": "Nft",
    "INTENT_AUCTION": "IntentAuction",
    "PREDICTION": "Prediction",
    "CROSS_CHAIN": "CrossChain",
    "CEX_DEX": "CexDex",
}


def parse_hop_use(raw: str) -> list:
    hops = [int(s.strip()) for s in str(raw).split(",")]
    assert all(2 <= h <= 7 for h in hops), f"hop {hops} fuera de 2..=7"
    assert hops == sorted(set(hops)), f"hops {hops} no estrictamente crecientes"
    return hops


def load_and_validate():
    pol = json.loads(SRC_POL.read_text(encoding="utf-8"))
    hop = json.loads(SRC_HOP.read_text(encoding="utf-8"))

    assert len(pol) == EXPECTED_DETECTORS, f"expected {EXPECTED_DETECTORS} detectors, got {len(pol)}"
    det_ids = [d["Detector_ID"] for d in pol]
    assert len(set(det_ids)) == EXPECTED_DETECTORS, "duplicate Detector_ID"
    assert det_ids == sorted(det_ids), "Detector_IDs not ascending"

    # Link estrategia→detector desde hop_map (264, MEV_ID ascendentes).
    assert len(hop) == EXPECTED_STRATEGIES, f"expected {EXPECTED_STRATEGIES} strategies, got {len(hop)}"
    strat_ids = [r["MEV_ID"] for r in hop]
    assert len(set(strat_ids)) == EXPECTED_STRATEGIES, "duplicate MEV_ID in hop_map"
    assert strat_ids == sorted(strat_ids), "hop_map MEV_IDs not ascending"
    by_det = {}
    for r in hop:
        by_det.setdefault(r["Detector_ID"], []).append(r)

    # Todos los links resuelven; los 60 detectores quedan usados.
    unresolved = sorted(set(by_det) - set(det_ids))
    assert not unresolved, f"hop_map Detector_IDs sin política: {unresolved}"
    unused = sorted(set(det_ids) - set(by_det))
    assert not unused, f"detectores sin estrategias en hop_map: {unused}"

    do_not_values = {str(d["Do_Not_Do"]).strip() for d in pol}
    assert len(do_not_values) == 1, f"Do_Not_Do no uniforme: {do_not_values}"
    do_not = do_not_values.pop()

    out_dets = []
    for d in pol:
        did = d["Detector_ID"]
        gp = str(d["Graph_Policy"]).strip()
        hs = str(d["Hot_Seed"]).strip()
        assert gp in GP_VARIANTS, f"{did}: Graph_Policy sin variante mapeada: {gp!r}"
        assert hs in HS_VARIANTS, f"{did}: Hot_Seed sin variante mapeada: {hs!r}"
        hu = parse_hop_use(d["Hop_Use"])
        # Col Strategies == conteo REAL de hop_map (60/60).
        real = len(by_det[did])
        assert d["Strategies"] == real, (
            f"{did}: col Strategies={d['Strategies']} != conteo real hop_map={real}"
        )
        # Family-envelope: Allowed_Hops ⊆ Hop_Use en TODAS sus estrategias.
        for r in by_det[did]:
            ah = [int(s.strip()) for s in str(r["Allowed_Hops"]).split(",")]
            extra = [h for h in ah if h not in hu]
            assert not extra, f"{r['MEV_ID']}: Allowed_Hops {ah} escapa Hop_Use {hu} de {did}"
        # DP-001 — columnas de contrato de ejecución por familia.
        es = str(d["Example_Surface"]).strip()
        assert es in SURFACE_VARIANTS, f"{did}: Example_Surface sin variante mapeada: {es!r}"
        ec = str(d["Execution_Class"]).strip()
        rd = str(d["Required_Data"]).strip()
        edc = str(d["Exact_Discovery_Criterion"]).strip()
        assert ec and rd and edc, (
            f"{did}: Execution_Class/Required_Data/Exact_Discovery_Criterion vacíos"
        )
        assert re.fullmatch(r"[A-Z][A-Z0-9_]*", ec), (
            f"{did}: Execution_Class no SCREAMING: {ec!r}"
        )
        # Uniformidad por familia: la clase del detector == la clase que
        # lleva CADA una de sus estrategias en el hop_map (invariante
        # cruzada con la col Execution_Class de la hoja 11).
        for r in by_det[did]:
            sec = str(r["Execution_Class"]).strip()
            assert sec == ec, (
                f"{r['MEV_ID']}: Execution_Class {sec!r} != clase de su detector {did}: {ec!r}"
            )
        out_dets.append({
            "d": did,
            "gp": gp,
            "hu": hu,
            "hs": hs,
            "es": es,
            "ec": ec,
            "rd": rd,
            "edc": edc,
            "sc": real,
        })

    # Coherencia OBSERVE (cross-invariante con col Status del hop_map).
    obs = next(x for x in out_dets if x["d"] == "OBSERVE")
    assert obs["gp"].startswith("OBSERVE_ONLY"), "OBSERVE sin graph policy OBSERVE_ONLY"
    assert obs["hs"] == "No hot opportunity seed; telemetry evidence only", "OBSERVE sin telemetry-only"
    st_obs = {r["MEV_ID"] for r in hop if str(r["Status"]).strip().upper() == "OBSERVE_ONLY"}
    det_obs = {r["MEV_ID"] for r in by_det["OBSERVE"]}
    assert st_obs == det_obs, f"OBSERVE_ONLY status {len(st_obs)} != detector OBSERVE {len(det_obs)}"

    # DP-001 — vocabulario cerrado compartido con la hoja 11 (29 clases
    # exactas en ambas direcciones).
    vocab_strat = {str(r["Execution_Class"]).strip() for r in hop}
    vocab_det = {d["ec"] for d in out_dets}
    assert vocab_det == vocab_strat, (
        f"vocabulario Execution_Class diverge hoja13↔hoja11: "
        f"solo13={sorted(vocab_det - vocab_strat)} solo11={sorted(vocab_strat - vocab_det)}"
    )
    assert len(vocab_det) == 29, f"expected 29 execution classes, got {len(vocab_det)}"
    # Cada familia declara SU contrato de datos y SU criterio exacto.
    assert len({d["rd"] for d in out_dets}) == EXPECTED_DETECTORS, "Required_Data duplicado"
    assert len({d["edc"] for d in out_dets}) == EXPECTED_DETECTORS, (
        "Exact_Discovery_Criterion duplicado"
    )

    out_strats = [{
        "m": r["MEV_ID"],
        "det": r["Detector_ID"],
        "ah": [int(s.strip()) for s in str(r["Allowed_Hops"]).split(",")],
        "st": str(r["Status"]).strip().upper(),
        "ec": str(r["Execution_Class"]).strip(),
    } for r in hop]

    return out_dets, out_strats, do_not


RUST_TEMPLATE = '''//! Static detector policy table — workbook QUOTEBASE-264 sheet
//! `13_DETECTOR_POLICY` (ARBX-0026), linked to strategies via the hop map's
//! `Detector_ID` column (264 rows).
//!
//! GENERATED from `docs/quotebase_detector_policy.json` +
//! `docs/quotebase_strategy_hop_map.json` by
//! `py scripts/xls/gen_detector_policy_rs.py` — do not edit rows by hand;
//! regenerate. The generator refuses to emit if the source drifts (60 rows,
//! Strategies == real hop_map counts summing 264, family-envelope
//! Allowed_Hops ⊆ Hop_Use, closed Graph_Policy/Hot_Seed vocabularies,
//! uniform Do_Not_Do, OBSERVE coherence, Execution_Class family-uniform
//! with sheet 11's closed shared 29-class vocabulary, non-empty distinct
//! Required_Data/Exact_Discovery_Criterion contracts).
//!
//! The policy dimensions are consumed GENERICALLY (no per-detector
//! hardcode):
//! - `GraphPolicy` — which graph family/adapter the detector's exact
//!   criterion maps to (annotation; discovery wiring key).
//! - `hop_use` — family hop envelope. Per-strategy bounds stay canonical in
//!   `strategy_hop_mask`; this envelope INTERSECTS them
//!   (`envelope_hop_bounds`) so a strategy can never escape its family.
//! - `Do_Not_Do` — universal guard: detector math is never replaced by a
//!   generic spot-price spread shortcut.
//! - `HotSeed` — hot-seed admission; the telemetry-only mode observes and
//!   emits evidence but never seeds a candidate (matches the 8 OBSERVE_ONLY
//!   strategies).
//! - `execution_class` — execution-precondition ANNOTATION shared verbatim
//!   by every strategy of the family (== sheet 11 col `Execution_Class`,
//!   closed 29-class vocabulary); not a dispatch verdict (same doctrine as
//!   `strategy_execution_class`).
//! - `example_surface` — the workbook's data-domain token (closed 10);
//!   `required_data_gate` maps it to the data classes the runtime can
//!   actually observe per tick (DP-002).
//! - `required_data` — the runtime NEEDS_DATA gate contract: the inputs the
//!   exact criterion needs. Absent input → observe, never approximate (R8).
//! - `exact_discovery_criterion` — the detector's OWN math; together with
//!   `DO_NOT_RULES` it forbids replacing it by a generic spot-price spread.

/// Graph family the detector's exact criterion maps to (12 workbook sentences).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GraphPolicy {{
{gp_variants}
}}

impl GraphPolicy {{
    /// Full workbook sentence (sheet 13 col `Graph_Policy`).
    pub fn as_str(self) -> &'static str {{
        match self {{
{gp_match}
        }}
    }}
}}

/// Hot-seed admission mode (5 workbook modes).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum HotSeed {{
{hs_variants}
}}

impl HotSeed {{
    /// Full workbook sentence (sheet 13 col `Hot_Seed`).
    pub fn as_str(self) -> &'static str {{
        match self {{
{hs_match}
        }}
    }}

    /// `false` only for the telemetry-only mode: the detector observes and
    /// emits evidence but must never seed a hot opportunity candidate.
    pub fn may_seed(self) -> bool {{
        !matches!(self, HotSeed::TelemetryOnly)
    }}
}}

/// Detector's workbook `Example_Surface` token (closed 10-token vocabulary).
/// The RequiredDataGate maps SURFACE → data classes the runtime can actually
/// observe per tick; a surface with no tracked class gates `NotTracked`
/// (honest unknown — never Ready-by-default, R8).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DetectorSurface {{
{surface_variants}
}}

impl DetectorSurface {{
    /// Workbook token (sheet 13 col `Example_Surface`).
    pub fn as_str(self) -> &'static str {{
        match self {{
{surface_match}
        }}
    }}
}}

/// One detector row of sheet `13_DETECTOR_POLICY`.
pub struct DetectorPolicy {{
    pub detector_id: &'static str,
    pub graph_policy: GraphPolicy,
    /// Family hop envelope (strictly ascending, 2..=7).
    pub hop_use: &'static [u8],
    pub hot_seed: HotSeed,
    /// Sheet 13 col `Example_Surface` (closed 10 tokens) — the data-domain
    /// classification the RequiredDataGate keys on (DP-002).
    pub example_surface: DetectorSurface,
    /// Sheet 13 col `Execution_Class` — annotation shared verbatim by every
    /// strategy of the family (== sheet 11 col; closed 29 classes).
    pub execution_class: &'static str,
    /// Sheet 13 col `Required_Data` — inputs the exact criterion needs
    /// (runtime NEEDS_DATA gate contract; never approximated, R8).
    pub required_data: &'static str,
    /// Sheet 13 col `Exact_Discovery_Criterion` — the detector's own math.
    pub exact_discovery_criterion: &'static str,
    /// Strategies linked to this detector (== hop_map real count).
    pub strategy_count: u16,
}}

impl DetectorPolicy {{
    /// Inclusive hop bounds of the family envelope.
    pub fn hop_bounds(&self) -> (u8, u8) {{
        (
            *self.hop_use.first().expect("non-empty hop_use"),
            *self.hop_use.last().expect("non-empty hop_use"),
        )
    }}

    /// Whether `hop` is inside the family envelope.
    pub fn allows_hop(&self, hop: u8) -> bool {{
        self.hop_use.contains(&hop)
    }}
}}

/// 60 detector policies, sorted ascending by Detector_ID — binary-searchable.
pub static DETECTOR_POLICIES: [DetectorPolicy; {n_det}] = [
{det_rows}
];

/// (MEV_ID, Detector_ID) link from the hop map, sorted ascending by MEV_ID.
pub static STRATEGY_DETECTOR: [(&str, &str); {n_strat}] = [
{strat_rows}
];

/// Workbook policy for a detector; `None` if unknown to the sheet.
pub fn detector_policy(detector_id: &str) -> Option<&'static DetectorPolicy> {{
    DETECTOR_POLICIES
        .binary_search_by(|p| p.detector_id.cmp(detector_id))
        .ok()
        .map(|i| &DETECTOR_POLICIES[i])
}}

/// Detector linked to a canonical strategy; `None` if the MEV_ID is unknown.
pub fn detector_of_strategy(mev_id: &str) -> Option<&'static str> {{
    STRATEGY_DETECTOR
        .binary_search_by(|(id, _)| (*id).cmp(mev_id))
        .ok()
        .map(|i| STRATEGY_DETECTOR[i].1)
}}

/// Policy of a canonical strategy via its detector link.
pub fn policy_for_strategy(mev_id: &str) -> Option<&'static DetectorPolicy> {{
    detector_of_strategy(mev_id).and_then(detector_policy)
}}

/// Universal `Do_Not_Do` guard — uniform across all 60 detectors.
pub static DO_NOT_RULES: [&str; 1] =
    ["{do_not}"];

/// Sheet 13 col `Do_Not_Do`: detector math must never be replaced by a
/// generic spot-price spread shortcut.
pub fn do_not_rules() -> &'static [&'static str] {{
    &DO_NOT_RULES
}}

/// Intersect per-strategy admissible bounds with the detector family
/// envelope. Unknown strategy or detector → `None` (fail-closed, same
/// doctrine as `strategy_dispatch_status`); empty intersection → `None`.
pub fn envelope_hop_bounds(mev_id: &str, strategy_bounds: Option<(u8, u8)>) -> Option<(u8, u8)> {{
    let (smin, smax) = strategy_bounds?;
    let (dmin, dmax) = policy_for_strategy(mev_id)?.hop_bounds();
    let lo = smin.max(dmin);
    let hi = smax.min(dmax);
    (lo <= hi).then_some((lo, hi))
}}

/// Per-GraphPolicy detector counts, DERIVED from the table (as_str order).
pub fn graph_policy_counts() -> &'static [(GraphPolicy, usize)] {{
    static C: std::sync::OnceLock<Vec<(GraphPolicy, usize)>> = std::sync::OnceLock::new();
    C.get_or_init(|| {{
        let mut v: Vec<(GraphPolicy, usize)> = Vec::new();
        for p in &DETECTOR_POLICIES {{
            match v.iter_mut().find(|slot| slot.0 == p.graph_policy) {{
                Some(slot) => slot.1 += 1,
                None => v.push((p.graph_policy, 1)),
            }}
        }}
        v.sort_unstable_by_key(|(g, _)| g.as_str());
        v
    }})
}}

/// Per-HotSeed detector counts, DERIVED from the table (as_str order).
pub fn hot_seed_counts() -> &'static [(HotSeed, usize)] {{
    static C: std::sync::OnceLock<Vec<(HotSeed, usize)>> = std::sync::OnceLock::new();
    C.get_or_init(|| {{
        let mut v: Vec<(HotSeed, usize)> = Vec::new();
        for p in &DETECTOR_POLICIES {{
            match v.iter_mut().find(|slot| slot.0 == p.hot_seed) {{
                Some(slot) => slot.1 += 1,
                None => v.push((p.hot_seed, 1)),
            }}
        }}
        v.sort_unstable_by_key(|(h, _)| h.as_str());
        v
    }})
}}

#[cfg(test)]
mod tests {{
    use super::*;

    /// Differential fixture — generated from the SAME canonical sources by
    /// the SAME script.
    const FIXTURE: &str = include_str!("detector_policy.fixture.json");

    /// (detector, graph sentence, hop_use, seed sentence, strategy_count).
    fn fixture_detectors() -> Vec<(String, String, Vec<u8>, String, usize)> {{
        let v: serde_json::Value = serde_json::from_str(FIXTURE).expect("fixture parses");
        v["detectors"]
            .as_array()
            .expect("detectors array")
            .iter()
            .map(|d| {{
                (
                    d["d"].as_str().expect("d").to_string(),
                    d["gp"].as_str().expect("gp").to_string(),
                    d["hu"]
                        .as_array()
                        .expect("hu")
                        .iter()
                        .map(|h| h.as_u64().expect("u8") as u8)
                        .collect(),
                    d["hs"].as_str().expect("hs").to_string(),
                    d["sc"].as_u64().expect("sc") as usize,
                )
            }})
            .collect()
    }}

    /// (MEV_ID, detector, allowed_hops, status).
    fn fixture_strategies() -> Vec<(String, String, Vec<u8>, String)> {{
        let v: serde_json::Value = serde_json::from_str(FIXTURE).expect("fixture parses");
        v["strategies"]
            .as_array()
            .expect("strategies array")
            .iter()
            .map(|r| {{
                (
                    r["m"].as_str().expect("m").to_string(),
                    r["det"].as_str().expect("det").to_string(),
                    r["ah"]
                        .as_array()
                        .expect("ah")
                        .iter()
                        .map(|h| h.as_u64().expect("u8") as u8)
                        .collect(),
                    r["st"].as_str().expect("st").to_string(),
                )
            }})
            .collect()
    }}

    fn fixture_do_not() -> String {{
        let v: serde_json::Value = serde_json::from_str(FIXTURE).expect("fixture parses");
        v["do_not"].as_str().expect("do_not").to_string()
    }}

    /// Full table↔fixture differential: every detector resolves to the exact
    /// workbook policy sentence/hops/seed/count, and every strategy resolves
    /// to its linked detector.
    #[test]
    fn table_matches_workbook_fixture_exactly() {{
        let fx = fixture_detectors();
        assert_eq!(fx.len(), 60);
        for (d, gp, hu, hs, sc) in &fx {{
            let p = detector_policy(d).expect("detector resolves");
            assert_eq!(p.graph_policy.as_str(), gp.as_str(), "graph drift {{}}", d);
            assert_eq!(p.hop_use, hu.as_slice(), "hop_use drift {{}}", d);
            assert_eq!(p.hot_seed.as_str(), hs.as_str(), "seed drift {{}}", d);
            assert_eq!(p.strategy_count as usize, *sc, "count drift {{}}", d);
        }}
        for (m, det, _, _) in fixture_strategies() {{
            assert_eq!(
                detector_of_strategy(&m),
                Some(det.as_str()),
                "link drift {{}}",
                m
            );
        }}
    }}

    /// Per-detector strategy_count == real strategies linked in the fixture,
    /// summing 264 (workbook tripwire against silent link drift).
    #[test]
    fn strategy_counts_cover_all_264() {{
        let fx = fixture_strategies();
        assert_eq!(fx.len(), 264);
        let mut per_det: Vec<(String, usize)> = Vec::new();
        for (_, det, _, _) in &fx {{
            match per_det.iter_mut().find(|slot| slot.0 == *det) {{
                Some(slot) => slot.1 += 1,
                None => per_det.push((det.clone(), 1)),
            }}
        }}
        for (d, _, _, _, sc) in fixture_detectors() {{
            let real = per_det
                .iter()
                .find(|slot| slot.0 == d)
                .map(|slot| slot.1)
                .unwrap_or(0);
            assert_eq!(real, sc, "count vs real link mismatch {{}}", d);
        }}
        assert_eq!(
            DETECTOR_POLICIES
                .iter()
                .map(|p| p.strategy_count as usize)
                .sum::<usize>(),
            264,
            "census must cover all 264 strategies"
        );
    }}

    /// Graph/seed census: fixture-derived == table-derived, closed
    /// vocabularies (12 graph families, 5 seed modes over 60 detectors).
    #[test]
    fn policy_census_matches_workbook() {{
        let fx = fixture_detectors();
        let mut gp_fx: Vec<(String, usize)> = Vec::new();
        let mut hs_fx: Vec<(String, usize)> = Vec::new();
        for (_, gp, _, hs, _) in &fx {{
            match gp_fx.iter_mut().find(|slot| slot.0 == *gp) {{
                Some(slot) => slot.1 += 1,
                None => gp_fx.push((gp.clone(), 1)),
            }}
            match hs_fx.iter_mut().find(|slot| slot.0 == *hs) {{
                Some(slot) => slot.1 += 1,
                None => hs_fx.push((hs.clone(), 1)),
            }}
        }}
        gp_fx.sort();
        hs_fx.sort();
        let gp_t = graph_policy_counts();
        let hs_t = hot_seed_counts();
        assert_eq!(gp_t.len(), 12, "graph family drift");
        assert_eq!(hs_t.len(), 5, "seed mode drift");
        assert_eq!(gp_t.iter().map(|(_, c)| c).sum::<usize>(), 60);
        assert_eq!(hs_t.iter().map(|(_, c)| c).sum::<usize>(), 60);
        for ((gv, gc), (ff, fc)) in gp_t.iter().zip(gp_fx.iter()) {{
            assert_eq!(gv.as_str(), ff.as_str(), "graph census name");
            assert_eq!(*gc, *fc, "graph census count {{}}", ff);
        }}
        for ((hv, hc), (ff, fc)) in hs_t.iter().zip(hs_fx.iter()) {{
            assert_eq!(hv.as_str(), ff.as_str(), "seed census name");
            assert_eq!(*hc, *fc, "seed census count {{}}", ff);
        }}
    }}

    /// Family-envelope invariant: every strategy's Allowed_Hops stays inside
    /// its detector's Hop_Use (data-level pin; the generator re-asserts it
    /// pre-emission, `envelope_hop_bounds` enforces it at runtime).
    #[test]
    fn family_envelope_respected() {{
        for (m, det, ah, _) in fixture_strategies() {{
            let p = detector_policy(&det).expect("detector resolves");
            for h in ah {{
                assert!(
                    p.allows_hop(h),
                    "{{}}: hop {{}} escapes family envelope of {{}}",
                    m,
                    h,
                    det
                );
            }}
        }}
    }}

    /// OBSERVE coherence: the OBSERVE detector is graph OBSERVE_ONLY + seed
    /// telemetry-only, and its strategies are EXACTLY the 8 with Status
    /// OBSERVE_ONLY in the hop map (cross-invariant with
    /// strategy_dispatch_status's census).
    #[test]
    fn observe_detector_coherence() {{
        let p = detector_policy("OBSERVE").expect("OBSERVE resolves");
        assert!(p.graph_policy.as_str().starts_with("OBSERVE_ONLY"));
        assert!(!p.hot_seed.may_seed());
        let fx = fixture_strategies();
        let st_obs: Vec<&str> = fx
            .iter()
            .filter(|(_, det, _, _)| det == "OBSERVE")
            .map(|(m, _, _, _)| m.as_str())
            .collect();
        let det_obs: Vec<&str> = fx
            .iter()
            .filter(|(_, _, _, st)| st == "OBSERVE_ONLY")
            .map(|(m, _, _, _)| m.as_str())
            .collect();
        assert_eq!(st_obs, det_obs, "OBSERVE detector != OBSERVE_ONLY status");
        assert_eq!(st_obs.len(), 8);
    }}

    /// Do_Not_Do is uniform across the sheet and exposed verbatim.
    #[test]
    fn do_not_rules_uniform() {{
        let fx = fixture_do_not();
        assert_eq!(do_not_rules(), &[fx.as_str()]);
        assert!(fx.contains("generic spot-price spread"));
    }}

    /// (detector, surface, execution_class, required_data, criterion) — the
    /// DP-001/002 contract columns, fixture side.
    fn fixture_contracts() -> Vec<(String, String, String, String, String)> {{
        let v: serde_json::Value = serde_json::from_str(FIXTURE).expect("fixture parses");
        v["detectors"]
            .as_array()
            .expect("detectors array")
            .iter()
            .map(|d| {{
                (
                    d["d"].as_str().expect("d").to_string(),
                    d["es"].as_str().expect("es").to_string(),
                    d["ec"].as_str().expect("ec").to_string(),
                    d["rd"].as_str().expect("rd").to_string(),
                    d["edc"].as_str().expect("edc").to_string(),
                )
            }})
            .collect()
    }}

    /// (MEV_ID, Execution_Class) — strategy-side classes from the hop map.
    fn fixture_strat_classes() -> Vec<(String, String)> {{
        let v: serde_json::Value = serde_json::from_str(FIXTURE).expect("fixture parses");
        v["strategies"]
            .as_array()
            .expect("strategies array")
            .iter()
            .map(|r| {{
                (
                    r["m"].as_str().expect("m").to_string(),
                    r["ec"].as_str().expect("ec").to_string(),
                )
            }})
            .collect()
    }}

    /// DP-001/002: the four contract columns ride the table VERBATIM, 60/60
    /// non-empty, and Required_Data/Exact_Discovery_Criterion stay distinct
    /// per family (a duplicate is copy/paste drift).
    #[test]
    fn execution_contracts_match_workbook() {{
        let fx = fixture_contracts();
        assert_eq!(fx.len(), 60);
        for (d, es, ec, rd, edc) in &fx {{
            let p = detector_policy(d).expect("detector resolves");
            assert_eq!(
                p.example_surface.as_str(),
                es.as_str(),
                "surface drift {{}}",
                d
            );
            assert_eq!(p.execution_class, ec.as_str(), "class drift {{}}", d);
            assert_eq!(p.required_data, rd.as_str(), "required_data drift {{}}", d);
            assert_eq!(
                p.exact_discovery_criterion,
                edc.as_str(),
                "criterion drift {{}}",
                d
            );
            assert!(!rd.trim().is_empty(), "empty required_data {{}}", d);
            assert!(!edc.trim().is_empty(), "empty criterion {{}}", d);
        }}
        let mut rd: Vec<&str> = DETECTOR_POLICIES.iter().map(|p| p.required_data).collect();
        let mut edc: Vec<&str> = DETECTOR_POLICIES
            .iter()
            .map(|p| p.exact_discovery_criterion)
            .collect();
        rd.sort_unstable();
        edc.sort_unstable();
        rd.dedup();
        edc.dedup();
        assert_eq!(rd.len(), 60, "required_data duplicates");
        assert_eq!(edc.len(), 60, "criterion duplicates");
    }}

    /// DP-002: Example_Surface is a closed 10-token vocabulary over the 60
    /// detectors — the RequiredDataGate's data-domain key.
    #[test]
    fn surface_vocabulary_closed() {{
        let mut surfaces: Vec<&str> = DETECTOR_POLICIES
            .iter()
            .map(|p| p.example_surface.as_str())
            .collect();
        surfaces.sort_unstable();
        surfaces.dedup();
        assert_eq!(surfaces.len(), 10, "surface vocabulary drift");
        let expected = [
            "CEX_DEX",
            "CROSS_CHAIN",
            "DERIVATIVES",
            "DEX_AMM",
            "DEX_STATE",
            "INTENT_AUCTION",
            "LENDING",
            "NFT",
            "PARITY_REDEMPTION",
            "PREDICTION",
        ];
        assert_eq!(surfaces, expected);
        // Same census fixture-side (workbook tripwire).
        let fx = fixture_contracts();
        let mut fx_surfaces: Vec<&str> = fx.iter().map(|(_, es, _, _, _)| es.as_str()).collect();
        fx_surfaces.sort_unstable();
        fx_surfaces.dedup();
        assert_eq!(fx_surfaces.len(), 10);
    }}

    /// DP-001: Execution_Class is family-uniform — each of the 264
    /// strategies carries exactly its detector's class, and both sheets
    /// share the same closed 29-class vocabulary.
    #[test]
    fn execution_class_family_uniform() {{
        let fx = fixture_strat_classes();
        assert_eq!(fx.len(), 264);
        let mut classes: Vec<&str> = Vec::new();
        for (m, ec) in &fx {{
            let p = policy_for_strategy(m).expect("strategy resolves");
            assert_eq!(
                p.execution_class,
                ec.as_str(),
                "class not family-uniform {{}}",
                m
            );
            if !classes.contains(&p.execution_class) {{
                classes.push(p.execution_class);
            }}
        }}
        classes.sort_unstable();
        classes.dedup();
        assert_eq!(classes.len(), 29, "closed vocabulary drift");
        let mut table_classes: Vec<&str> = DETECTOR_POLICIES
            .iter()
            .map(|p| p.execution_class)
            .collect();
        table_classes.sort_unstable();
        table_classes.dedup();
        assert_eq!(classes, table_classes, "sheet13 vocab != sheet11 vocab");
    }}

    /// Binary-search preconditions: both static tables sorted and unique.
    #[test]
    fn tables_sorted_unique() {{
        let dets: Vec<&str> = DETECTOR_POLICIES.iter().map(|p| p.detector_id).collect();
        let mut sorted = dets.clone();
        sorted.sort_unstable();
        assert_eq!(dets, sorted);
        let ids: Vec<&str> = STRATEGY_DETECTOR.iter().map(|(id, _)| *id).collect();
        let mut sorted = ids.clone();
        sorted.sort_unstable();
        assert_eq!(ids, sorted);
        assert_eq!(
            ids.iter().collect::<std::collections::BTreeSet<_>>().len(),
            ids.len()
        );
    }}

    /// Envelope intersection semantics: identity while the strategy stays
    /// inside its family (the canonical case — 0 violations), clamping and
    /// empty-intersection behavior on synthetic out-of-family bounds, and
    /// fail-closed on unknown strategy.
    #[test]
    fn envelope_intersection_semantics() {{
        // Canonical: MEV-01-015 (R_CLOSED_CYCLE, allowed {{2}}) inside 2..=7.
        assert_eq!(
            envelope_hop_bounds("MEV-01-015", Some((2, 2))),
            Some((2, 2))
        );
        // Clamp: synthetic strategy bounds escaping the family get cut back.
        assert_eq!(
            envelope_hop_bounds("MEV-01-015", Some((2, 8))),
            Some((2, 7))
        );
        // Bounded family from the fixture (first detector with max hop 4).
        let det_fx = fixture_detectors();
        let bounded = det_fx
            .iter()
            .find(|(_, _, hu, _, _)| *hu.last().expect("hu") == 4)
            .expect("bounded family exists");
        let strat_fx = fixture_strategies();
        let member = strat_fx
            .iter()
            .find(|(_, det, _, _)| det == &bounded.0)
            .expect("member strategy exists");
        // Identity inside the envelope…
        let (lo, hi) = member
            .2
            .first()
            .copied()
            .zip(member.2.last().copied())
            .expect("ah");
        assert_eq!(
            envelope_hop_bounds(&member.0, Some((lo, hi))),
            Some((lo, hi)),
            "identity inside family {{}}",
            member.0
        );
        // …empty intersection beyond it → None.
        let beyond = bounded.2.last().expect("hu") + 1;
        assert_eq!(
            envelope_hop_bounds(&member.0, Some((beyond, beyond + 2))),
            None,
            "empty intersection must forbid expansion {{}}",
            member.0
        );
        // Fail-closed on unknown strategy / missing bounds. The unknown-id
        // sentinel is concat-constructed (ALPHA-MAP exact-264, 2026-09-01): a
        // literal would re-enter the static MEV-ID namespace and drift the scan.
        const UNKNOWN_SENTINEL: &str = concat!("MEV-99-", "999");
        assert_eq!(envelope_hop_bounds(UNKNOWN_SENTINEL, Some((2, 3))), None);
        assert_eq!(envelope_hop_bounds("MEV-01-015", None), None);
    }}
}}
'''


def main():
    dets, strats, do_not = load_and_validate()

    # rustfmt (max_width 100): brazo inline si cabe, bloque si no.
    def arm(enum: str, variant: str, sentence: str) -> str:
        inline = f'            {enum}::{variant} => "{sentence}",'
        if len(inline) <= 100:
            return inline
        return f'            {enum}::{variant} => {{\n                "{sentence}"\n            }}'

    gp_variants = "\n".join(f"    {v}," for v in GP_VARIANTS.values())
    gp_match = "\n".join(arm("GraphPolicy", v, s) for s, v in GP_VARIANTS.items())
    hs_variants = "\n".join(f"    {v}," for v in HS_VARIANTS.values())
    hs_match = "\n".join(arm("HotSeed", v, s) for s, v in HS_VARIANTS.items())
    surface_variants = "\n".join(f"    {v}," for v in SURFACE_VARIANTS.values())
    surface_match = "\n".join(
        f'            DetectorSurface::{v} => "{s}",' for s, v in SURFACE_VARIANTS.items()
    )
    det_rows = "\n".join(
        "    DetectorPolicy {\n"
        f'        detector_id: "{d["d"]}",\n'
        f'        graph_policy: GraphPolicy::{GP_VARIANTS[d["gp"]]},\n'
        f'        hop_use: &[{", ".join(str(h) for h in d["hu"])}],\n'
        f'        hot_seed: HotSeed::{HS_VARIANTS[d["hs"]]},\n'
        f'        example_surface: DetectorSurface::{SURFACE_VARIANTS[d["es"]]},\n'
        f'        execution_class: "{d["ec"]}",\n'
        f'        required_data: "{d["rd"]}",\n'
        f'        exact_discovery_criterion: "{d["edc"]}",\n'
        f'        strategy_count: {d["sc"]},\n'
        "    },"
        for d in dets
    )
    strat_rows = "\n".join(f'    ("{s["m"]}", "{s["det"]}"),' for s in strats)

    rust = RUST_TEMPLATE.format(
        gp_variants=gp_variants,
        gp_match=gp_match,
        hs_variants=hs_variants,
        hs_match=hs_match,
        surface_variants=surface_variants,
        surface_match=surface_match,
        n_det=len(dets),
        det_rows=det_rows,
        n_strat=len(strats),
        strat_rows=strat_rows,
        do_not=do_not,
    )
    fixture = {
        "_source": "docs/quotebase_detector_policy.json (hoja 13_DETECTOR_POLICY) + link Detector_ID de docs/quotebase_strategy_hop_map.json",
        "_generator": "scripts/xls/gen_detector_policy_rs.py",
        "do_not": do_not,
        "detectors": dets,
        "strategies": strats,
    }
    OUT_RS.write_text(rust, encoding="utf-8", newline="\n")
    OUT_FIXTURE.write_text(json.dumps(fixture, indent=1), encoding="utf-8", newline="\n")
    print(f"OK  {OUT_RS.relative_to(ROOT)}  ({len(dets)} detectores)")
    print(f"OK  {OUT_FIXTURE.relative_to(ROOT)}  ({len(strats)} links)")
    print(f"    do_not uniforme: {do_not[:60]}...")


if __name__ == "__main__":
    sys.exit(main())
