import json, re, os, sys, io
sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding="utf-8", errors="replace")
BASE = r"C:\Users\HFRC\Desktop\arbitragex-v2-main (17)"
X = BASE + r"\audits\pipeline-cartridge-audit-2026-09-19\xlsx_extract"
STRATS = BASE + r"\backend\searcher-rs\cartridges\strategies"

def load(n):
    with open(X + "\\" + n + ".json", encoding="utf-8") as f:
        return json.load(f)

# ---- canonical tables ----
cat = load("ULTRA_11_STRATEGY_CATALOG")          # row0 header, rows 1..264 data
hop = load("QB_11_STRATEGY_HOP_MAP")             # row0 title, row1 header, rows 2..265 data
canon = {}
for r in cat[1:]:
    if not r or not r[0]:
        continue
    mid = str(r[0]).strip()
    canon[mid] = {
        "name": str(r[2]) if r[2] else "",
        "min_legs": str(r[7]).strip() if r[7] is not None else "",
        "max_legs": str(r[8]).strip() if r[8] is not None else "",
        "detector": str(r[15]).strip() if r[15] is not None else "",
        "exec_class": str(r[17]).strip() if r[17] is not None else "",
        "primary_ops": str(r[19]) if r[19] else "",
        "secondary_ops": str(r[20]) if r[20] else "",
        "status": str(r[36]).strip() if r[36] is not None else "",
        "enabled": str(r[28]).strip() if r[28] is not None else "",
    }
hopmap = {}
for r in hop[2:]:
    if not r or not r[0]:
        continue
    mid = str(r[0]).strip()
    hopmap[mid] = {
        "allowed_hops": str(r[15]).strip() if r[15] is not None else "",
        "hopmask": str(r[16]).strip() if r[16] is not None else "",
        "h2": str(r[9]).strip() if r[9] is not None else "",
        "h3": str(r[10]).strip() if r[10] is not None else "",
        "h4": str(r[11]).strip() if r[11] is not None else "",
        "h5": str(r[12]).strip() if r[12] is not None else "",
        "h6": str(r[13]).strip() if r[13] is not None else "",
        "h7": str(r[14]).strip() if r[14] is not None else "",
        "min_legs": str(r[7]).strip() if r[7] is not None else "",
        "max_legs": str(r[8]).strip() if r[8] is not None else "",
        "detector": str(r[6]).strip() if r[6] is not None else "",
    }
print("canonical catalog:", len(canon), "hopmap:", len(hopmap))

def ops_list(s):
    # "op_27 Path Ordering; op_21 Newton-Raphson; ..." -> [27,21,...]
    return sorted(int(m) for m in re.findall(r"op_(\d+)", s or ""))

# ---- cartridge scan ----
files = sorted(f for f in os.listdir(STRATS) if f.endswith(".rhai"))
print("rhai files:", len(files))
deviations = []
census = []
missing_canon = []
for fn in files:
    m = re.match(r"mev_(\d+)_(\d+)_.*\.rhai", fn)
    if not m:
        deviations.append({"file": fn, "issue": "filename no conforma patron mev_XX_YYY"})
        continue
    mid = f"MEV-{m.group(1)}-{m.group(2)}"
    path = os.path.join(STRATS, fn)
    txt = open(path, encoding="utf-8").read()
    lines = txt.splitlines()
    def find_val(key, scope_all=True):
        mm = re.search(key + r"\s*:\s*([^,\n\]]+)", txt)
        return mm.group(1).strip().strip('"') if mm else None
    name = find_val("name")
    version = find_val("version")
    fmev = find_val("mev_id")
    detector = find_val("detector_id")
    exec_class = find_val("execution_class")
    minl = find_val("min_legs")
    maxl = find_val("max_legs")
    pop = re.search(r"primary_operators\s*:\s*\[([^\]]*)\]", txt)
    sop = re.search(r"secondary_operators\s*:\s*\[([^\]]*)\]", txt)
    pop_l = sorted(int(x) for x in re.findall(r"\d+", pop.group(1))) if pop else []
    sop_l = sorted(int(x) for x in re.findall(r"\d+", sop.group(1))) if sop else []
    entry = {
        "file": fn, "mev_id": mid, "name": name, "version": version,
        "detector": detector, "exec_class": exec_class,
        "min_legs": minl, "max_legs": maxl,
        "primary_ops": pop_l, "secondary_ops": sop_l,
        "loc": len(lines),
        "has_evaluate": "fn evaluate_opportunity" in txt,
        "has_cpmm_out": "fn cpmm_out" in txt,
        "has_v3_quote_fn": bool(re.search(r"fn .*v3|sqrt_price|sqrtPrice|tick", txt)),
        "has_stableswap": bool(re.search(r"stable|newton|amplif", txt, re.I)),
        "has_net_gate": bool(re.search(r"net|profit", txt, re.I)),
        "gas_hint": "GAS_HINT_USD" in txt,
        "triggers": find_val("triggers"),
    }
    census.append(entry)
    # cross-checks
    c = canon.get(mid)
    if not c:
        missing_canon.append(mid)
        continue
    if fmev and fmev != mid:
        deviations.append({"file": fn, "issue": f"mev_id interno {fmev} != filename {mid}", "line": None})
    if detector and c["detector"] and detector != c["detector"]:
        deviations.append({"file": fn, "issue": f"detector {detector} != catalogo {c['detector']}"})
    if exec_class and c["exec_class"] and exec_class != c["exec_class"]:
        deviations.append({"file": fn, "issue": f"exec_class {exec_class} != catalogo {c['exec_class']}"})
    if minl is not None and c["min_legs"] and str(minl) != str(c["min_legs"]):
        deviations.append({"file": fn, "issue": f"min_legs {minl} != catalogo {c['min_legs']}"})
    if maxl is not None and c["max_legs"] and str(maxl) != str(c["max_legs"]):
        deviations.append({"file": fn, "issue": f"max_legs {maxl} != catalogo {c['max_legs']}"})
    cp, cs = ops_list(c["primary_ops"]), ops_list(c["secondary_ops"])
    if pop_l and cp and pop_l != cp:
        deviations.append({"file": fn, "issue": f"primary_ops {pop_l} != catalogo {cp}"})
    if sop_l and cs and sop_l != cs:
        deviations.append({"file": fn, "issue": f"secondary_ops {sop_l} != catalogo {cs}"})
    h = hopmap.get(mid)
    if h and h["allowed_hops"]:
        lo = int(h["allowed_hops"].split(",")[0])
        hi = int(h["allowed_hops"].split(",")[-1])
        if minl is not None and int(minl) != lo:
            deviations.append({"file": fn, "issue": f"min_legs {minl} != hop-map Allowed_Hops min {lo}"})
        # max_legs del catalogo puede exceder 7 (hops evaluables) — solo info
    if not entry["has_evaluate"]:
        deviations.append({"file": fn, "issue": "sin fn evaluate_opportunity"})

# ---- coverage both ways ----
repo_ids = {c["mev_id"] for c in census}
cat_ids = set(canon.keys())
print("repo ids:", len(repo_ids), "catalog ids:", len(cat_ids))
print("en repo sin catalogo:", sorted(repo_ids - cat_ids))
print("en catalogo sin repo:", sorted(cat_ids - repo_ids))

# ---- aggregates ----
from collections import Counter
print("\nversion:", Counter(c["version"] for c in census))
print("detector families:", Counter(c["detector"] for c in census))
print("exec_class:", Counter(c["exec_class"] for c in census))
print("has_cpmm_out:", sum(c["has_cpmm_out"] for c in census))
print("has_v3_fn:", sum(c["has_v3_quote_fn"] for c in census))
print("has_stableswap:", sum(c["has_stableswap"] for c in census))
print("has_gas_hint:", sum(c["gas_hint"] for c in census))
print("loc total:", sum(c["loc"] for c in census), "min/max:", min(c["loc"] for c in census), max(c["loc"] for c in census))
print("\nDEVIATIONS:", len(deviations))
for d in deviations[:60]:
    print(" ", d)

with open(BASE + r"\audits\pipeline-cartridge-audit-2026-09-19\pc02_census.json", "w", encoding="utf-8") as f:
    json.dump({"census": census, "deviations": deviations, "missing_canon": missing_canon}, f, ensure_ascii=False, indent=1)
print("\nsaved pc02_census.json")
