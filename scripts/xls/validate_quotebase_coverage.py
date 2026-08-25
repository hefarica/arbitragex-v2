"""Suite paramétrica de coverage QUOTEBASE-264 (ARBX-0029).

AC: 264 estrategias mapeadas y status-respectadas; 1,436 combos validados;
60 detectores validados. Property: BIYECCIÓN. Differential: vs extracts.

Tres capas de datos, todas leídas por el suite (nada hardcodeado de IDs;
los conteos se DERIVAN y se cruzan entre archivos INDEPENDIENTES):

  A. Extracts crudos del Excel (docs/excel_*_extracted.json)
  B. JSONs canónicos por hoja (docs/quotebase_*.json)
  C. Artefactos Rust GENERADOS en el crate (tablas .rs + fixtures .json)

[1] ESTRATEGIAS (264): bijección hop_map ↔ extract; vocabulario Status de 4
    estados; fixtures generados (dispatch Status / Execution_Class) de
    acuerdo con la fuente; tabla HopMask del .rs de acuerdo; las 264
    presentes en la hoja expandida (el workbook mantiene combos para TODOS
    los estados — el gate de status vive en el dispatcher ARBX-0021, no en
    la disponibilidad de datos; pin para detectar un workbook filtrado).
[2] COMBOS (1,436 = Σ|Allowed_Hops|): pares (MEV_ID,Hop) únicos; conjunto
    de hops por estrategia == Allowed_Hops; HopMask_u8 de acuerdo y bit
    (Hop-2) SET sin bits extra; Detector_ID de acuerdo con el link del
    hop_map; Hop ∈ 2..=7; censo por hop calculado; Route_Cache_Key contiene
    strategy=<MEV_ID> y h=<Hop>; combos por detector == Σ|Allowed_Hops| de
    sus estrategias.
[3] DETECTORES (60): bijección fixture detector_policy ↔ JSON canónico ↔
    extract crudo; campos hu/hs/gp/sc iguales; links estrategia→detector
    del fixture == hop_map; col Strategies == conteo real.

Exit code != 0 ante CUALQUIER drift. Uso:
    py scripts/xls/validate_quotebase_coverage.py
"""
import json
import re
import sys
from pathlib import Path

# Consola Windows cp1252 — el suite imprime unicode del workbook.
if sys.stdout.encoding and sys.stdout.encoding.lower() not in ("utf-8", "utf8"):
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")

ROOT = Path(__file__).resolve().parents[2]
DOCS = ROOT / "docs"
CRATE = ROOT / "backend" / "searcher-rs" / "src"

N_STRATEGIES = 264  # canonical_excel_count (pin del programa, no hardcode de IDs)
N_DETECTORS = 60
STATUS_VOCAB = {"ROUTE_READY", "NEEDS_ROUTE_DATA", "OBSERVE_ONLY", "NO_COMPATIBLE_ROUTE"}

FAILURES = []


def check(cond, msg):
    if not cond:
        FAILURES.append(msg)
        print(f"  FAIL  {msg}")
    return cond


def load(p):
    return json.loads(p.read_text(encoding="utf-8"))


def parse_hop_mask_rs():
    """Extrae las filas ('MEV-xx-xxx', N) de STRATEGY_HOP_MASKS del .rs.

    Solo la sección de la tabla estática — un regex global capturaría
    call-sites de tests (p.ej. admissible_hop_bounds("MEV-01-001", 8)) y
    produciría falsos drifts.
    """
    src = (CRATE / "strategy_hop_mask.rs").read_text(encoding="utf-8")
    start = src.index("STRATEGY_HOP_MASKS")
    end = src.index("];", start)
    body = src[start:end]
    rows = re.findall(r'\("((?:MEV-\d+-\d+))",\s*(\d+)\)', body)
    return {m: int(v) for m, v in rows}


def main():
    # ---- fuentes -----------------------------------------------------------
    hop = load(DOCS / "quotebase_strategy_hop_map.json")
    expanded = load(DOCS / "quotebase_strategy_hop_expanded.json")
    policies = load(DOCS / "quotebase_detector_policy.json")
    ext_strategies = load(DOCS / "excel_strategies_extracted.json")
    if isinstance(ext_strategies, dict):
        ext_strategies = ext_strategies.get("strategies", [])
    ext_detectors = load(DOCS / "excel_detectors_extracted.json")
    if isinstance(ext_detectors, dict):
        ext_detectors = ext_detectors.get("detectors", [])
    fx_status = load(CRATE / "strategy_dispatch_status.fixture.json")
    fx_class = load(CRATE / "strategy_execution_class.fixture.json")
    fx_policy = load(CRATE / "detector_policy.fixture.json")

    print("== [1] ESTRATEGIAS — mapeadas y status-respetadas ===================")
    check(len(hop) == N_STRATEGIES, f"hop_map {len(hop)} != {N_STRATEGIES}")
    hop_ids = [r["MEV_ID"] for r in hop]
    check(len(set(hop_ids)) == N_STRATEGIES, "MEV_IDs duplicados en hop_map")

    ext_ids = [r.get("MEV_ID") or r.get("MEV ID") for r in ext_strategies]
    ext_set = set(filter(None, ext_ids))
    check(len(ext_set) == N_STRATEGIES, f"extract strategies {len(ext_set)} != {N_STRATEGIES}")
    check(set(hop_ids) == ext_set, "bijección hop_map ↔ extract rota")

    st_counts = {}
    for r in hop:
        st = str(r["Status"]).strip().upper()
        check(st in STATUS_VOCAB, f"{r['MEV_ID']}: Status fuera de vocabulario: {st}")
        st_counts[st] = st_counts.get(st, 0) + 1
    check(sum(st_counts.values()) == N_STRATEGIES, "Status no cubre 264")
    print(f"  PASS  bijección 264↔264 · censo Status derivado: {st_counts}")

    # Fixtures generados ↔ fuente canónica (diferencial capa C ↔ capa B).
    fx_rows = {r["m"]: r["st"] for r in fx_status["rows"]}
    check(len(fx_rows) == N_STRATEGIES, "fixture dispatch != 264")
    for r in hop:
        check(fx_rows.get(r["MEV_ID"]) == str(r["Status"]).strip().upper(),
              f"{r['MEV_ID']}: fixture dispatch Status drift")
    fx_ec = {r["m"]: (r["ec"], r["st"]) for r in fx_class["rows"]}
    check(len(fx_ec) == N_STRATEGIES, "fixture execution_class != 264")
    for r in hop:
        ec, st = fx_ec[r["MEV_ID"]]
        check(st == str(r["Status"]).strip().upper(),
              f"{r['MEV_ID']}: fixture EC status drift")
        check(ec == str(r["Execution_Class"]).strip().upper(),
              f"{r['MEV_ID']}: fixture EC class drift")
    print("  PASS  fixtures dispatch+EC ↔ hop_map (528 checks de campo)")

    rs_masks = parse_hop_mask_rs()
    check(len(rs_masks) == N_STRATEGIES, f"tabla HopMask .rs {len(rs_masks)} != 264")
    for r in hop:
        check(rs_masks.get(r["MEV_ID"]) == r["HopMask_u8"],
              f"{r['MEV_ID']}: HopMask .rs drift")
    print("  PASS  tabla HopMask .rs ↔ hop_map (264)")

    exp_ids = {e["MEV_ID"] for e in expanded}
    check(exp_ids == set(hop_ids),
          "el workbook NO expandida todas las 264 (¿filtrado por status?)")
    print("  PASS  264/264 presentes en hoja expandida — el gate de status vive en el dispatcher, no en los datos")

    # ---- [2] COMBOS ---------------------------------------------------------
    print("== [2] COMBOS — hoja 12_STRATEGY_HOP_EXPANDED =======================")
    allowed = {}
    for r in hop:
        allowed[r["MEV_ID"]] = sorted(int(s) for s in str(r["Allowed_Hops"]).split(","))
    n_expected = sum(len(v) for v in allowed.values())
    check(len(expanded) == n_expected,
          f"expanded {len(expanded)} != Σ|Allowed_Hops| {n_expected}")
    pairs = [(e["MEV_ID"], int(e["Hop"])) for e in expanded]
    check(len(set(pairs)) == len(pairs), "pares (MEV_ID,Hop) duplicados")

    hops_by_strat = {}
    for e in expanded:
        m, h = e["MEV_ID"], int(e["Hop"])
        hops_by_strat.setdefault(m, []).append(h)
        check(2 <= h <= 7, f"{m}: Hop {h} fuera de 2..=7")
        check((int(e["HopMask_u8"]) >> (h - 2)) & 1,
              f"{m}@{h}: bit(Hop-2) NO set en HopMask_u8={e['HopMask_u8']}")
        for b in range(2, 8):
            check(not ((int(e["HopMask_u8"]) >> (b - 2)) & 1 and b not in allowed[m]),
                  f"{m}: bit extra h={b} fuera de Allowed_Hops")
        ck = str(e["Route_Cache_Key"])
        check(f"strategy={m}" in ck and f"h={h}" in ck,
              f"{m}@{h}: Route_Cache_Key no conforme: {ck}")
    for m, hs in hops_by_strat.items():
        check(sorted(hs) == allowed[m], f"{m}: hops expandidos {sorted(hs)} != Allowed_Hops {allowed[m]}")

    by_hop = {}
    for _, h in pairs:
        by_hop[h] = by_hop.get(h, 0) + 1
    print(f"  PASS  {len(expanded)} combos == Σ|Allowed_Hops| · pares únicos · censo por hop: {dict(sorted(by_hop.items()))}")

    hop_det = {r["MEV_ID"]: r["Detector_ID"] for r in hop}
    det_combos = {}
    for e in expanded:
        check(e["Detector_ID"] == hop_det[e["MEV_ID"]],
              f"{e['MEV_ID']}: Detector_ID drift entre hoja 11 y 12")
        det_combos[e["Detector_ID"]] = det_combos.get(e["Detector_ID"], 0) + 1
    det_expected = {}
    for r in hop:
        det_expected[r["Detector_ID"]] = det_expected.get(r["Detector_ID"], 0) + len(allowed[r["MEV_ID"]])
    for d, n in det_expected.items():
        check(det_combos.get(d, 0) == n,
              f"{d}: combos reales {det_combos.get(d, 0)} != Σ|Allowed_Hops| {n}")
    print(f"  PASS  Detector_ID hoja 11↔12 · combos por detector == Σ|Allowed_Hops| ({len(det_expected)} detectores)")

    # ---- [3] DETECTORES ----------------------------------------------------
    print("== [3] DETECTORES — hoja 13 + policy engine =========================")
    check(len(policies) == N_DETECTORS, f"policies {len(policies)} != {N_DETECTORS}")
    pol_ids = [p["Detector_ID"] for p in policies]
    check(len(set(pol_ids)) == N_DETECTORS, "Detector_ID duplicados en hoja 13")

    ext_det_ids = {r.get("Detector ID") or r.get("Detector_ID") for r in ext_detectors}
    ext_det_ids.discard(None)
    check(len(ext_det_ids) == N_DETECTORS, f"extract detectors {len(ext_det_ids)} != {N_DETECTORS}")
    check(set(pol_ids) == ext_det_ids, "bijección hoja 13 ↔ extract crudo rota")

    fx_det = {d["d"]: d for d in fx_policy["detectors"]}
    check(len(fx_det) == N_DETECTORS, "fixture detector_policy != 60")
    for p in policies:
        f = fx_det.get(p["Detector_ID"])
        check(f is not None, f"{p['Detector_ID']}: ausente en fixture generado")
        if f:
            check(f["gp"] == str(p["Graph_Policy"]).strip(), f"{p['Detector_ID']}: gp drift")
            check(f["hs"] == str(p["Hot_Seed"]).strip(), f"{p['Detector_ID']}: hs drift")
            check(f["hu"] == [int(s) for s in str(p["Hop_Use"]).split(",")],
                  f"{p['Detector_ID']}: hu drift")
            check(f["sc"] == p["Strategies"], f"{p['Detector_ID']}: sc drift")
    real_counts = {}
    for m, d in hop_det.items():
        real_counts[d] = real_counts.get(d, 0) + 1
    for p in policies:
        check(p["Strategies"] == real_counts.get(p["Detector_ID"], 0),
              f"{p['Detector_ID']}: col Strategies != conteo real")
    check(sum(p["Strategies"] for p in policies) == N_STRATEGIES,
          "Σ Strategies != 264")

    fx_links = {s["m"]: s["det"] for s in fx_policy["strategies"]}
    check(len(fx_links) == N_STRATEGIES, "links fixture != 264")
    for m, d in hop_det.items():
        check(fx_links.get(m) == d, f"{m}: link fixture drift")
    print("  PASS  bijección 60↔60↔60 · campos fixture↔JSON · links 264 · col Strategies == real (Σ=264)")

    # ---- resumen ------------------------------------------------------------
    print("=====================================================================")
    if FAILURES:
        print(f"COVERAGE FAIL: {len(FAILURES)} drift(s)")
        return 1
    print(f"COVERAGE PASS — {N_STRATEGIES} estrategias status-respetadas · "
          f"{len(expanded)} combos validados · {N_DETECTORS} detectores validados "
          f"(censo Status {st_counts})")
    return 0


if __name__ == "__main__":
    sys.exit(main())
