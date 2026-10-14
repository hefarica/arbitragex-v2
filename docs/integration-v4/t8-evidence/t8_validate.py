#!/usr/bin/env python3
# t8 — t6-datos: ingesta/validacion A2 csv + A8 xlsx + asserts vs SSOT congelado (t9)
# Solo stdlib. Entradas en %TEMP%\t8_ingest (extraidas de arbitragex-prompt-supremo-INTEGRADO.zip).
# Salida: JSON por stdout (redirigido a T8_ingest_validation.json).
import csv, hashlib, io, json, re, sys, zipfile
import xml.etree.ElementTree as ET
from collections import Counter, defaultdict

TMP = r"C:\Users\HFRC\AppData\Local\Temp\t8_ingest"
CSV_A2 = TMP + r"\06_CLASIFICACION_clasificacion_264.csv"
XLSX   = TMP + r"\ArbitrageX_v4_264_60hojas.xlsx"
SSOT_J = TMP + r"\01_SSOT_strategy_hop_map.json"

def sha256(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()

EXPECTED = {
    "csv_a2": "38f038a82b5d16979e17d002c406d249d9979d164166e2cdaac063d0b80a7786",
    "xlsx":   "eb5b803cb66d0dcbb5c17c9885e9a3dc041e01e94c5bc22a37ef0482e4e09cc3",
    "ssot_json": "7fb529c607071a210a0b3bc627dd27b475008f2031445e9acccf317884861122",
}
out = {"input_hashes": {}, "checks": {}, "a2_csv": {}, "xlsx": {}, "ssot": {}, "a1_figures": {}, "asserts_264_60_1436_0": {}, "divergences": {}, "prohibited_0636": {}}

for k, p in [("csv_a2", CSV_A2), ("xlsx", XLSX), ("ssot_json", SSOT_J)]:
    h = sha256(p)
    out["input_hashes"][k] = {"measured": h, "expected": EXPECTED[k], "match": h == EXPECTED[k]}

# ---------- A2 csv ----------
A2_HEADER = ["MEV_ID","GRUPO","FAMILIA","DETECTOR","SUPERFICIE","HOPS","BASE_POS","IMP_POS","DELTA"]
with open(CSV_A2, newline="", encoding="utf-8-sig") as f:
    rows = list(csv.reader(f))
header, data = rows[0], rows[1:]
a2 = {}
a2["header_ok"] = header == A2_HEADER
a2["header_measured"] = header
a2["n_cols"] = len(header)
a2["no_index_column"] = header[0] == "MEV_ID" and all(h.strip() != "" for h in header) and not any(re.match(r"^(Unnamed|index|idx|\s*)$", h) for h in header)
a2["n_rows"] = len(data)
a2["all_rows_9_cols"] = all(len(r) == 9 for r in data)
deltas = [float(r[8]) for r in data]
non_incr = all(deltas[i] >= deltas[i+1] - 1e-12 for i in range(len(deltas)-1))
strict_dec = all(deltas[i] > deltas[i+1] for i in range(len(deltas)-1))
a2["delta_non_increasing"] = non_incr
a2["delta_strictly_decreasing"] = strict_dec
a2["delta_first_last"] = [deltas[0], deltas[-1]]
mev_ids = [r[0] for r in data]
a2["mev_ids_unique"] = len(set(mev_ids)) == 264
a2["distinct_detectors"] = len(set(r[3] for r in data))
a2["distinct_surfaces"] = len(set(r[4] for r in data))
a2["hops_distinct"] = sorted(set(r[5] for r in data))
a2["base_pos_sum"] = round(sum(float(r[6]) for r in data), 6)
a2["imp_pos_sum"] = round(sum(float(r[7]) for r in data), 6)
a2["delta_gt0_count"] = sum(1 for d in deltas if d > 0)
out["a2_csv"] = a2

# ---------- xlsx ----------
NS = "{http://schemas.openxmlformats.org/spreadsheetml/2006/main}"
NSR = "{http://schemas.openxmlformats.org/officeDocument/2006/relationships}"
z = zipfile.ZipFile(XLSX)

# shared strings
shared = []
if "xl/sharedStrings.xml" in z.namelist():
    for _, el in ET.iterparse(io.BytesIO(z.read("xl/sharedStrings.xml")), events=("end",)):
        if el.tag == NS + "si":
            txt = "".join(t.text or "" for t in el.iter(NS + "t"))
            shared.append(txt)
            el.clear()

# sheet name -> file
wb = ET.fromstring(z.read("xl/workbook.xml"))
rels = ET.fromstring(z.read("xl/_rels/workbook.xml.rels"))
rid2target = {r.get("Id"): r.get("Target") for r in rels}
sheets = []
for s in wb.iter(NS + "sheet"):
    rid = s.get(NSR + "id")
    tgt = rid2target.get(rid, "")
    if not tgt.startswith("xl/"):
        tgt = "xl/" + tgt.lstrip("/")
    sheets.append((s.get("name"), tgt))
sheet_names = [n for n, _ in sheets]
out["xlsx"]["n_sheets"] = len(sheet_names)
out["xlsx"]["sheet_names"] = sheet_names
EXPECTED_59 = ["LEEME","RESUMEN","CLASIFICACION_264","AGREGADOS_SUPERFICIE","AGREGADOS_DETECTOR","INVARIANTES","CRITERIOS_T6","PLAN_T1_T8","TESTS_ATOMICOS"] + [f"A{i:02d}" for i in range(1,21)] + [f"F{i:02d}" for i in range(1,15)] + [f"S{i:02d}" for i in range(1,11)] + [f"I{i:02d}" for i in range(1,7)]
out["xlsx"]["sheets_match_expected_59"] = sheet_names == EXPECTED_59
out["xlsx"]["registro_inclusiones_present"] = "REGISTRO_INCLUSIONES" in sheet_names
out["xlsx"]["sheets_ge_60"] = len(sheet_names) >= 60

def read_sheet(target, max_rows=None):
    """Devuelve lista de filas (list[str]) usando shared strings; iterparse incremental."""
    rows_out = []
    pending = None
    for _, el in ET.iterparse(io.BytesIO(z.read(target)), events=("end",)):
        if el.tag == NS + "row":
            cells = {}
            for c in el.iter(NS + "c"):
                ref = c.get("r") or ""
                col = re.match(r"([A-Z]+)", ref)
                col = col.group(1) if col else "?"
                t = c.get("t")
                if t == "s":
                    v = el_v = c.find(NS + "v")
                    val = shared[int(el_v.text)] if el_v is not None and el_v.text else ""
                elif t == "inlineStr":
                    isel = c.find(NS + "is")
                    val = "".join(t2.text or "" for t2 in isel.iter(NS + "t")) if isel is not None else ""
                else:
                    v = c.find(NS + "v")
                    val = v.text if v is not None and v.text is not None else ""
                cells[col] = val
            if cells:
                ncols = 0
                cols = sorted(cells.keys(), key=lambda k: (len(k), k))
                # construir por posicion desde A
                row = []
                ci = 0
                for col in cols:
                    idx = 0
                    for ch in col:
                        idx = idx * 26 + (ord(ch) - 64)
                    idx -= 1
                    while len(row) < idx:
                        row.append("")
                    row.append(cells[col])
                rows_out.append(row)
            el.clear()
            if max_rows and len(rows_out) >= max_rows:
                break
    return rows_out

name2tgt = dict(sheets)

# TESTS_ATOMICOS — columnas + 264000 filas + metricas
TA_COLS = ["MEV_ID","STRATEGY","DETECTOR","FAMILY","SURFACE","HOP_MASK","TEST_ID","TEST_KIND","BASE_POS","IMP_POS","DELTA","EXEC_CLASS","ASSERT"]
ta = read_sheet(name2tgt["TESTS_ATOMICOS"])
ta_header, ta_data = ta[0], ta[1:]
ta_report = {
    "header_ok": ta_header[:13] == TA_COLS,
    "header_measured": ta_header[:13],
    "n_data_rows": len(ta_data),
    "expected_rows": 264000,
}
per_mev = Counter(r[0] for r in ta_data)
ta_report["distinct_mev_ids"] = len(per_mev)
ta_report["all_mev_have_1000"] = all(v == 1000 for v in per_mev.values())
ta_report["assert_value_counts"] = dict(Counter(r[12] for r in ta_data if len(r) > 12))
base_sum = sum(float(r[8]) for r in ta_data if len(r) > 8 and r[8] != "")
imp_sum = sum(float(r[9]) for r in ta_data if len(r) > 9 and r[9] != "")
ta_report["base_pos_sum"] = round(base_sum, 6)
ta_report["imp_pos_sum"] = round(imp_sum, 6)
ta_deltas = [float(r[10]) for r in ta_data if len(r) > 10 and r[10] != ""]
ta_report["delta_gt0_count"] = sum(1 for d in ta_deltas if d > 0)
ta_report["delta_pct"] = round((imp_sum / base_sum - 1) * 100, 4) if base_sum else None
# monotonic: dentro de cada MEV_ID, DELTA no-creciente por TEST_ID ascendente
mono_ok, mono_blocks = True, 0
prev_mev, prev_d = None, None
for r in ta_data:
    if len(r) < 12: continue
    mev, d = r[0], float(r[10]) if r[10] != "" else 0.0
    if mev != prev_mev:
        mono_blocks += 1
    else:
        if d > prev_d + 1e-12:
            mono_ok = False
    prev_mev, prev_d = mev, d
ta_report["monotonic_within_mev_blocks"] = mono_ok
ta_report["monotonic_blocks"] = mono_blocks
out["xlsx"]["tests_atomicos"] = ta_report

# CLASIFICACION_264 sheet vs A2 csv
cl = read_sheet(name2tgt["CLASIFICACION_264"])
cl_header, cl_data = cl[0], cl[1:]
cl_mev = [r[0] for r in cl_data]
out["xlsx"]["clasificacion_264"] = {
    "header_measured": cl_header[:9],
    "n_rows": len(cl_data),
    "header_equals_a2_csv": cl_header[:9] == header,
    "mev_set_equals_a2_csv": set(cl_mev) == set(mev_ids),
    "row_order_equals_a2_csv": cl_mev == mev_ids,
}

# AGREGADOS cross-check
def agg_counts(sheet):
    rows = read_sheet(name2tgt[sheet])
    h, d = rows[0], rows[1:]
    return h, d
for agg, col_idx in (("AGREGADOS_SUPERFICIE", 4), ("AGREGADOS_DETECTOR", 3)):
    h, d = agg_counts(agg)
    agg_map = {}
    for r in d:
        if len(r) >= 2 and r[0] != "":
            agg_map[r[0]] = r[1]
    a2_counts = Counter(r[col_idx] for r in data)
    mism = {k: {"xlsx": agg_map.get(k), "a2_csv": a2_counts.get(k)} for k in set(agg_map) | set(a2_counts) if str(agg_map.get(k)) != str(a2_counts.get(k))}
    out["xlsx"][agg.lower()] = {
        "header": h[:4],
        "n_rows": len(d),
        "counts_match_a2_csv": not mism,
        "mismatches": mism,
    }

# A01 sheet — cifras declaradas (fuente A1)
a01 = read_sheet(name2tgt["A01"])
out["xlsx"]["a01_raw_first20"] = a01[:20]

# INVARIANTES sheet (6/6 OK segun t9)
inv = read_sheet(name2tgt["INVARIANTES"])
out["xlsx"]["invariantes_raw"] = inv[:30]

# ---------- SSOT json ----------
with open(SSOT_J, encoding="utf-8") as f:
    ssot = json.load(f)
def walk_leaves(o, acc):
    if isinstance(o, dict):
        for k, v in o.items(): walk_leaves(v, acc)
    elif isinstance(o, list):
        acc.append(o)
    return acc
ssot_report = {}
# estrategia: encontrar entradas tipo {strategy, hop(s)/mask}
strategies = ssot.get("strategies") if isinstance(ssot, dict) else None
keys_top = list(ssot.keys()) if isinstance(ssot, dict) else ["<list>"]
ssot_report["top_keys"] = keys_top[:12]
out["ssot"] = ssot_report
out["ssot"]["raw_head"] = json.dumps(ssot, ensure_ascii=False)[:1500]

# ---------- prohibido 0.636 ----------
def scan_0636(values):
    hits = []
    for v in values:
        try:
            if abs(float(v) - 0.636) < 1e-9:
                hits.append(v)
        except (ValueError, TypeError):
            if isinstance(v, str) and "0.636" in v:
                hits.append(v)
    return hits
hits_ta = scan_0636([r[10] for r in ta_data if len(r) > 10]) + scan_0636([r[8] for r in ta_data if len(r) > 8]) + scan_0636([r[9] for r in ta_data if len(r) > 9])
hits_csv = scan_0636([r[8] for r in data])
hits_a01 = scan_0636([c for r in a01 for c in r])
out["prohibited_0636"] = {
    "hits_tests_atomicos_delta_base_imp": len(hits_ta),
    "hits_a2_csv_delta": len(hits_csv),
    "hits_a01_sheet": len(hits_a01),
    "verdict": "OK" if (len(hits_ta) + len(hits_csv) + len(hits_a01)) == 0 else "VIOLACION",
}

# ---------- asserts 264/60/1436/0 vs SSOT ----------
out["asserts_264_60_1436_0"] = {
    "strategies_264": {"a2_csv_measured": len(set(mev_ids)), "ok": len(set(mev_ids)) == 264},
    "detectors_60": {"a2_csv_measured": a2["distinct_detectors"], "ok": a2["distinct_detectors"] == 60},
    "combos_1436": {"hop_dist_sum_expected": 245+262+260+233+233+203, "note": "verificar contra SSOT json abajo"},
    "zero_divergences": "pendiente_mevid_diff_abajo",
}

print(json.dumps(out, ensure_ascii=False, indent=1, default=str))
