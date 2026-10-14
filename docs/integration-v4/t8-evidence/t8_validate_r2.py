#!/usr/bin/env python3
# t8 ronda 2 — cierres: agregados col correcta, TEST_KIND/deriv_pos, monotonic por serie,
# SSOT json combos=1436 + hop_dist, MEV_ID diff A2 vs SSOT.
import csv, io, json, re, zipfile
import xml.etree.ElementTree as ET
from collections import Counter, defaultdict

TMP = r"C:\Users\HFRC\AppData\Local\Temp\t8_ingest"
CSV_A2 = TMP + r"\06_CLASIFICACION_clasificacion_264.csv"
XLSX   = TMP + r"\ArbitrageX_v4_264_60hojas.xlsx"
SSOT_J = TMP + r"\01_SSOT_strategy_hop_map.json"

with open(CSV_A2, newline="", encoding="utf-8-sig") as f:
    rows = list(csv.reader(f))
header, data = rows[0], rows[1:]

NS = "{http://schemas.openxmlformats.org/spreadsheetml/2006/main}"
NSR = "{http://schemas.openxmlformats.org/officeDocument/2006/relationships}"
z = zipfile.ZipFile(XLSX)
shared = []
if "xl/sharedStrings.xml" in z.namelist():
    for _, el in ET.iterparse(io.BytesIO(z.read("xl/sharedStrings.xml")), events=("end",)):
        if el.tag == NS + "si":
            shared.append("".join(t.text or "" for t in el.iter(NS + "t"))); el.clear()
wb = ET.fromstring(z.read("xl/workbook.xml"))
rels = ET.fromstring(z.read("xl/_rels/workbook.xml.rels"))
rid2t = {r.get("Id"): r.get("Target") for r in rels}
name2tgt = {}
for s in wb.iter(NS + "sheet"):
    t = rid2t.get(s.get(NSR + "id"), "")
    name2tgt[s.get("name")] = ("xl/" + t.lstrip("/")) if not t.startswith("xl/") else t

def read_sheet(name):
    out_rows = []
    for _, el in ET.iterparse(io.BytesIO(z.read(name2tgt[name])), events=("end",)):
        if el.tag == NS + "row":
            cells = {}
            for c in el.iter(NS + "c"):
                m = re.match(r"([A-Z]+)", c.get("r") or "")
                col = m.group(1) if m else "?"
                t = c.get("t"); val = ""
                if t == "s":
                    v = c.find(NS + "v"); val = shared[int(v.text)] if v is not None and v.text else ""
                elif t == "inlineStr":
                    isel = c.find(NS + "is"); val = "".join(x.text or "" for x in isel.iter(NS + "t")) if isel is not None else ""
                else:
                    v = c.find(NS + "v"); val = v.text if v is not None and v.text is not None else ""
                cells[col] = val
            row = []
            for col in sorted(cells.keys(), key=lambda k: (len(k), k)):
                idx = 0
                for ch in col: idx = idx * 26 + (ord(ch) - 64)
                idx -= 1
                while len(row) < idx: row.append("")
                row.append(cells[col])
            out_rows.append(row); el.clear()
    return out_rows

R = {}

# --- 1) AGREGADOS_DETECTOR con columna ESTRATEGIAS correcta ---
ad = read_sheet("AGREGADOS_DETECTOR")
adh, add = ad[0], ad[1:]
ci = adh.index("ESTRATEGIAS")
agg_map = {r[0]: r[ci] for r in add if r and r[0] != ""}
a2_counts = Counter(r[3] for r in data)
mism = {k: {"xlsx_estrategias": agg_map.get(k), "a2_csv": a2_counts.get(k)} for k in set(agg_map) | set(a2_counts) if str(agg_map.get(k)) != str(a2_counts.get(k))}
R["agregados_detector_fixed"] = {
    "header": adh, "n_rows": len(add),
    "estrategias_sum": sum(int(v) for v in agg_map.values()),
    "counts_match_a2_csv": not mism, "mismatches": mism,
    "observe_rows": {k: v for k, v in agg_map.items() if k.startswith("OBSERVE")},
    "non_observe_rows": len([k for k in agg_map if not k.startswith("OBSERVE")]),
}

# --- 2) TESTS_ATOMICOS: TEST_KIND, deriv_pos hunt, monotonic por serie ---
ta = read_sheet("TESTS_ATOMICOS")
tah, tad = ta[0], ta[1:]
idx = {c: i for i, c in enumerate(tah)}
kind_counts = Counter(r[idx["TEST_KIND"]] for r in tad)
R["test_kind_counts"] = dict(kind_counts)
# DELTA>0 por TEST_KIND
pos_by_kind = Counter()
sum_delta_by_kind = Counter()
sum_base_by_kind = Counter()
sum_imp_by_kind = Counter()
for r in tad:
    k = r[idx["TEST_KIND"]]; d = float(r[idx["DELTA"]])
    if d > 0: pos_by_kind[k] += 1
    sum_delta_by_kind[k] += d
    sum_base_by_kind[k] += float(r[idx["BASE_POS"]])
    sum_imp_by_kind[k] += float(r[idx["IMP_POS"]])
R["delta_gt0_by_kind"] = dict(pos_by_kind)
R["delta_sum_by_kind"] = {k: round(v, 2) for k, v in sum_delta_by_kind.items()}
R["base_sum_by_kind"] = {k: round(v, 2) for k, v in sum_base_by_kind.items()}
R["imp_sum_by_kind"] = {k: round(v, 2) for k, v in sum_imp_by_kind.items()}
# candidatos deriv_pos
R["deriv_candidates"] = {
    "delta_gt0_total": sum(pos_by_kind.values()),
    "delta_gt0_3560?": 3560 in pos_by_kind.values(),
    "delta_gt0_1982?": 1982 in pos_by_kind.values(),
    "delta_sum_total": round(sum(sum_delta_by_kind.values()), 2),
    "delta_sum_round_3560?": any(abs(v - 3560) < 1 for v in sum_delta_by_kind.values()),
}
# monotonic: por (MEV_ID, TEST_KIND), DELTA no-creciente en orden TEST_ID
series = defaultdict(list)
for r in tad:
    series[(r[idx["MEV_ID"]], r[idx["TEST_KIND"]])].append((r[idx["TEST_ID"]], float(r[idx["DELTA"]])))
viol_series = 0
viol_example = None
for key, vals in series.items():
    vals_sorted = sorted(vals, key=lambda x: x[0])
    ds = [d for _, d in vals_sorted]
    for i in range(len(ds) - 1):
        if ds[i + 1] > ds[i] + 1e-12:
            viol_series += 1
            if viol_example is None:
                viol_example = {"series": key, "at": i, "prev": ds[i], "next": ds[i + 1]}
            break
R["monotonic_by_series"] = {
    "n_series": len(series), "series_with_violation": viol_series, "example": viol_example,
    "monotonic_ok": viol_series == 0,
}

# --- 3) SSOT json: 264 / combos 1436 / hop_dist / MEV_ID diff ---
with open(SSOT_J, encoding="utf-8") as f:
    ssot = json.load(f)
strategies = ssot["strategies"]
hop_counter = Counter()
for s in strategies:
    for h in re.split(r"[/_]", str(s.get("mask", ""))):
        h = h.strip()
        if h.isdigit():
            hop_counter[int(h)] += 1
R["ssot"] = {
    "n_strategies": len(strategies),
    "distinct_detectors": len(set(s["detector"] for s in strategies)),
    "combos_total": sum(hop_counter.values()),
    "hop_dist": {str(k): v for k, v in sorted(hop_counter.items())},
    "hop_dist_expected": {"2": 245, "3": 262, "4": 260, "5": 233, "6": 233, "7": 203},
    "hop_dist_matches": {str(k): hop_counter.get(k) == v for k, v in {"2":245,"3":262,"4":260,"5":233,"6":233,"7":203}.items()},
    "masks_distinct": len(set(str(s.get("mask")) for s in strategies)),
}
ssot_mev = set(s["mev_id"] for s in strategies)
a2_mev = set(r[0] for r in data)
R["mev_diff"] = {
    "a2_only": sorted(a2_mev - ssot_mev)[:20], "n_a2_only": len(a2_mev - ssot_mev),
    "ssot_only": sorted(ssot_mev - a2_mev)[:20], "n_ssot_only": len(ssot_mev - a2_mev),
    "sets_equal": a2_mev == ssot_mev,
}
# detector set diff A2 vs SSOT vs AGREGADOS
a2_det = set(r[3] for r in data)
ssot_det = set(s["detector"] for s in strategies)
R["detector_diff"] = {
    "a2_csv": len(a2_det), "ssot_json": len(ssot_det),
    "a2_minus_ssot": sorted(a2_det - ssot_det), "ssot_minus_a2": sorted(ssot_det - a2_det),
    "agregados_rows": len(agg_map),
    "agregados_minus_a2": sorted(set(agg_map) - a2_det),
    "a2_minus_agregados": sorted(a2_det - set(agg_map)),
}
# hop_dist desde A2 csv HOPS (mismo calculo)
a2_hop = Counter()
for r in data:
    for h in re.split(r"[/_]", r[5]):
        if h.strip().isdigit(): a2_hop[int(h.strip())] += 1
R["a2_hop_dist"] = {str(k): v for k, v in sorted(a2_hop.items())}

print(json.dumps(R, ensure_ascii=False, indent=1, default=str))
