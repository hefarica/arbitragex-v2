#!/usr/bin/env python3
# t13 — dump hojas F/S/I + RESUMEN/LEEME + tally EXEC_CLASS/HOP_MASK de TESTS_ATOMICOS
import io, json, re, zipfile
import xml.etree.ElementTree as ET
from collections import Counter

XLSX = r"C:\Users\HFRC\AppData\Local\Temp\t8_ingest\ArbitrageX_v4_264_60hojas.xlsx"
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
for name in ["RESUMEN","LEEME","F01","F02","F03","F04","F05","F06","F07","F08","F09","F10","F11","F12","F13","F14","S01","S02","S03","S04","S05","S06","S07","S08","S09","S10","I01","I02","I03","I04","I05","I06"]:
    R[name] = read_sheet(name)

# tally EXEC_CLASS y HOP_MASK sobre TESTS_ATOMICOS (streaming, sin guardar todo)
ec = Counter(); hm = Counter(); tk = Counter()
first = None
n = 0
for _, el in ET.iterparse(io.BytesIO(z.read(name2tgt["TESTS_ATOMICOS"])), events=("end",)):
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
        if first is None:
            first = cells
        else:
            n += 1
            ec[cells.get("L", "")] += 1
            hm[cells.get("F", "")] += 1
        el.clear()
R["_tests_tally"] = {"n_rows": n, "exec_class": dict(ec), "hop_mask_top": dict(hm.most_common(10))}
print(json.dumps(R, ensure_ascii=False, indent=1, default=str))
