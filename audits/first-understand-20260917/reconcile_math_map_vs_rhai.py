#!/usr/bin/env python3
# // WO-02b (2026-09-17, fixer gang ronda 1) — reconciliación op_origen canónica:
# math_map.json (manifests) vs primary_operators de cada .rhai (cartuchos).
# Regla RULE 00: mide, no hereda. Reproducible: python3 reconcile_math_map_vs_rhai.py
# desde audits/first-understand-20260917/ (rutas relativas al repo raíz backend/).
import json, re, sys
from pathlib import Path

BASE = Path(__file__).resolve().parents[2] / "backend" / "searcher-rs" / "cartridges"
MATH_MAP = BASE / "manifests" / "math_map.json"
STRATS = BASE / "strategies"

# --- lado A: math_map.json (fuente canónica op_origen, WO-02a-DESIGN.md:246-250) ---
entries = json.loads(MATH_MAP.read_text(encoding="utf-8"))
assert isinstance(entries, list), "math_map.json debe ser una lista"

def parse_op(s):  # "op_27" -> 27
    m = re.fullmatch(r"op_(\d+)", s.strip())
    if not m:
        sys.exit(f"FATAL: token op no parseable en math_map.json: {s!r}")
    return int(m.group(1))

map_side = {}          # mev_id -> set(primary ints)
dupes = []
for e in entries:
    mid = e["mev_id"]
    ops = {parse_op(x) for x in e["primary_ops"]}
    if mid in map_side:
        dupes.append(mid)
    map_side[mid] = ops

# --- lado B: los 264 .rhai (declaración viva del cartucho) ---
rhais = sorted(STRATS.glob("*.rhai"))
rhai_side = {}         # mev_id -> set(primary ints)
unparsed = []
for p in rhais:
    txt = p.read_text(encoding="utf-8", errors="replace")
    mid_m = re.search(r"mev_id:\s*\"([^\"]+)\"", txt)
    prim_m = re.search(r"primary_operators:\s*\[([^\]]*)\]", txt)
    if not mid_m or not prim_m:
        unparsed.append(p.name); continue
    ints = {int(x) for x in re.findall(r"\d+", prim_m.group(1))}
    rhai_side[mid_m.group(1)] = rhai_side.get(mid_m.group(1), set()) | ints

# --- reconciliación ---
keys_map, keys_rhai = set(map_side), set(rhai_side)
only_map = sorted(keys_map - keys_rhai)
only_rhai = sorted(keys_rhai - keys_map)
common = keys_map & keys_rhai
mismatch = sorted(k for k in common if map_side[k] != rhai_side[k])

print(f"math_map.json entries       : {len(entries)}")
print(f"mev_id únicos (math_map)    : {len(keys_map)}  (duplicados: {dupes or 0})")
print(f".rhai files                 : {len(rhais)}  (no parseados: {unparsed or 0})")
print(f"mev_id únicos (.rhai)       : {len(keys_rhai)}")
print(f"mev_id solo en math_map     : {only_map or 0}")
print(f"mev_id solo en .rhai        : {only_rhai or 0}")
print(f"primary_ops == primary_operators (set) para los comunes: "
      f"{len(common) - len(mismatch)}/{len(common)}")
print(f"MISMATCHES                  : {mismatch or 0}")
for k in mismatch[:10]:
    print(f"  {k}: math_map={sorted(map_side[k])} rhai={sorted(rhai_side[k])}")
ok = (not dupes and not unparsed and not only_map and not only_rhai and not mismatch)
print(f"RESULTADO: {'264/264 IDENTICOS — RECONCILIADO' if ok and len(common)==264 else 'DIVERGENCIA DETECTADA'}")
sys.exit(0 if ok else 1)
