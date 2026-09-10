# -*- coding: utf-8 -*-
"""HP-04 (2026-09-08) — APPLY op_32 edges a la matriz canónica estrategia×operador.

Generador de diff quirúrgico (línea a línea, byte-exacto: preserva CRLF e indent del
vecino; las líneas no tocadas se copian idénticas) + validador de post-condiciones.
Idempotente: si op_32 ya está aplicado, solo valida. encoding='utf-8' explícito en
todo I/O (LEARNINGS).

Alcance (claim HP-04):
  - skills/arbitragex-ultra/knowledge_graph.jsonl        (+182 edges USES_SECONDARY op_32)
  - skills/arbitragex-ultra/capability_matrix.json       (+op_32 en 182 entries)
  - skills/arbitragex-ultra/operators/op_32/             (OPERATOR.json + SKILL.md nuevos)
  EXTENSIÓN DE SCOPE documentada (HP-01-CENSO.md "Invariante de 3 almacenes"):
  - skills/arbitragex-ultra/strategies/*/STRATEGY.json   (secondary_operators += op_32)
  - skills/arbitragex-ultra/strategies/*/SKILL.md        (línea Secondary += op_32)
  Sin tocarlas, la matriz quedaría contradictoria consigo misma (kg≡cm≡STRATEGY.json
  hoy 264/264). Reportado al BOARD en HP-04-APPLY.md para ratificación.

Diseño de edges (R8 — familia sin justificación = sin edge): EDGE_FAMILIES /
NO_EDGE_FAMILIES con justificación económica por familia. OBSERVE (8) sin decisión de
ejecución → sin selector Pareto. Rol: USES_SECONDARY en todos (clase
evaluación/selección como op_22/op_30 — censo HP-01-CENSO.md §(c)). // HP-04 (2026-09-08)
"""
from __future__ import annotations

import io
import json
import os
import re
import sys

ROOT = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
ULTRA = os.path.join(ROOT, "skills", "arbitragex-ultra")
KG = os.path.join(ULTRA, "knowledge_graph.jsonl")
CM = os.path.join(ULTRA, "capability_matrix.json")
STRATS = os.path.join(ULTRA, "strategies")
OP32 = os.path.join(ULTRA, "operators", "op_32")

EXPECTED_BEFORE = 2511  # censo HP-01-CENSO.md §(c)

EDGE_FAMILIES = {
    "MEV-01": "Ciclos multihop (R_CLOSED_CYCLE=25...): cada hop suma fee on-chain, "
              "decoherencia de estado y latencia de inclusion; el frente elige cuantos "
              "hops pagar (staleness medido 1.29-1.78 bps/bloque, world/graph-algorithms/"
              "BETTER_THAN_EXCEL.md §5; ejemplo GOAL MEV-01-016 triangular).",
    "MEV-03": "Backruns/subastas por evento: rentabilidad neta vs prob. de inclusion "
              "(EV*P(win|bid), BETTER_THAN_EXCEL.md §8) vs ventana temporal — el bid de "
              "gas es una posicion en el frente.",
    "MEV-05": "Split CEX/DEX: rentabilidad neta vs CVaR de inventario/contraparte del leg "
              "CEX vs latencia de transferencia cross-venue (op_16+op_23 presentes 14/14).",
    "MEV-06": "Seleccion de puente/ruta cross-chain: spread vs CVaR de puente (riesgo de "
              "finalidad/fallo) vs latencia de finalidad (X_BRIDGE=16, X_PREPOS=12; "
              "ejemplo GOAL).",
    "MEV-07": "Construccion de hedge en derivados: captura de basis/funding vs CVaR de "
              "margen/liquidacion vs latencia de roll/settlement (D_SETTLE=5; MEV-07-001 "
              "spot-perpetual legs 2-6 = set de candidatos).",
    "MEV-08": "Liquidaciones/subastas: bono de liquidacion vs ventana de competencia vs "
              "riesgo de rebote de health-factor (MEV-08-012 Liquidation discount "
              "arbitrage; health-factor vs profit del operador).",
    "MEV-09": "Bids de solver: margen del bid vs prob. de adjudicacion (Nash, op_24 en "
              "18/20) vs deadline de inclusion.",
}
NO_EDGE_FAMILIES = {
    "MEV-02": "Routing CFMM resoluble convexo a optimo global neto-de-fees "
              "(Angeris/Diamandis, BETTER_THAN_EXCEL.md §1); riesgo/latencia ~constantes "
              "intra-bloque atomico -> frente degenerado.",
    "MEV-04": "Set de candidatos pequeno (venta vs redencion); comparacion escalar "
              "NPV-por-dia-de-lockup suficiente; sin poblacion para orden de no-dominancia.",
    "MEV-10": "Sweeps discretos pequenos; decoherencia ya capturada por sizing "
              "(op_21/op_15); sin conflicto multi-objetivo medible in-stream.",
    "MEV-11": "Pares de plataformas pequenos; lockup estatico por mercado; odds-netas "
              "escalares suficientes.",
}

OP32_OPERATOR_JSON = {
    "id": 32,
    "code": "op_32",
    "name": "NSGA-II Multi-Objective",
    "canonical_role": "Pareto ranking/selection over [rentabilidad_neta, riesgo_CVaR, "
                      "latencia] of already-evaluated candidates; empty front = None "
                      "(fail-honest R8).",
    "enabled": True,
    "engine_present": False,
    "calibration_state": "NOT_IMPLEMENTED",
    "formula": "min F(x) = (-rentabilidad_neta(x), CVaR(x), latencia(x)) - NSGA-II "
               "fast non-dominated sort + crowding distance",
    "implementation_file": "backend/math-engine/src/operators/op_32_nsga2.rs",
}

OP32_SKILL_MD = """# NSGA-II Multi-Objective (op_32)

## Identity
- **ID**: 32 / op_32
- **Canonical Role**: Pareto ranking/selection over [rentabilidad_neta, riesgo_CVaR,
  latencia] of already-evaluated candidates; empty front = None (fail-honest R8).
- **Enabled**: YES | **Engine**: NO (PENDIENTE HP-03)
- **Calibration**: NOT_IMPLEMENTED

## Mathematical Definition
```
min F(x) = (-rentabilidad_neta(x), CVaR(x), latencia(x))  s.t. x in Omega (candidates)
```
NSGA-II: fast non-dominated sort + crowding distance + elitism; selection by
normalized preference vector [rentabilidad, riesgo, latencia].

## Pipeline Phase
RANK - selection among evaluated candidates (USES_SECONDARY en el knowledge graph,
clase evaluacion/seleccion como op_22/op_30). Nunca descubre ni dimensiona.

## Implementation
- File (PENDIENTE HP-03): `backend/math-engine/src/operators/op_32_nsga2.rs`
- Register (PENDIENTE HP-03): registry de operators (mod.rs:6 instruction) +
  `matrix/topology_map.rs` COLS 31->32 (264x31 -> 264x32).
- Tests (PENDIENTE HP-03): non-dominance of front, crowding distance, preference
  selects from front, 1-objective degeneration, seed determinism, bounded generations.

## Excel Traceability
- Source: PENDIENTE OPERADOR - 12_OPERATOR_CONTROL no tiene fila op_32; anadir fila 35
  y columna op_32 en 13_STRAT_OP_MATRIX para cerrar el PARTIAL honesto de
  scripts/excel_canon/build_canonical_artifacts.py (ops_match).
- Matrix: 13_STRAT_OP_MATRIX column `op_32` (PENDIENTE OPERADOR).

## Calibration
NOT_IMPLEMENTED - el motor no existe aun (engine_present=false hasta HP-03). Frente de
Pareto vacio = None honesto, jamas solucion fabricada (R8).

## Edges (HP-04 2026-09-08)
182 edges USES_SECONDARY en 7 familias justificadas (MEV-01/03/05/06/07/08/09, sin
OBSERVE); MEV-02/04/10/11 SIN edge por R8 (justificaciones economicas por familia en
audits/op32-highperf-2026-09-08/HP-04-APPLY.md). // HP-04 (2026-09-08)
"""


def die(msg: str) -> None:
    print(f"FAIL: {msg}")
    sys.exit(1)


# consola Windows cp1252 no imprime '≡' — stdout utf-8 (LEARNINGS: encoding explicito)
try:
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")
except Exception:
    pass


def read_raw(path: str) -> str:
    """Lee sin tocar line endings (newline='' desactiva toda traduccion)."""
    with io.open(path, "r", encoding="utf-8", newline="") as f:
        return f.read()


def write_raw(path: str, text: str) -> None:
    with io.open(path, "w", encoding="utf-8", newline="") as f:
        f.write(text)


def split_eol(line_keepends: str) -> tuple[str, str]:
    """'  "op_30"\\r\\n' -> ('  "op_30"', '\\r\\n')."""
    m = re.match(r"^(.*?)(\r?\n)?$", line_keepends, re.S)
    body = m.group(1)
    eol = m.group(2) or ""
    return body, eol


def fam_of(sid: str) -> str:
    return sid.rsplit("-", 1)[0]  # MEV-01-016 -> MEV-01


def load_targets(cm: dict) -> tuple[list[str], set[str]]:
    observe = {s for s, e in cm.items() if s != "Legend" and e["detector"] == "OBSERVE"}
    targets = sorted(
        s for s in cm
        if s != "Legend" and fam_of(s) in EDGE_FAMILIES and s not in observe
    )
    return targets, observe


def main() -> int:
    kg_raw = read_raw(KG)
    cm_raw = read_raw(CM)
    cm = json.loads(cm_raw)
    targets, observe = load_targets(cm)
    tset = set(targets)

    # ── sanity de disenio ───────────────────────────────────────────────────
    expect_fam = {"MEV-01": 36, "MEV-03": 29, "MEV-05": 14, "MEV-06": 30,
                  "MEV-07": 30, "MEV-08": 25, "MEV-09": 18}
    got_fam = {}
    for s in targets:
        got_fam[fam_of(s)] = got_fam.get(fam_of(s), 0) + 1
    if got_fam != expect_fam:
        die(f"targets por familia {got_fam} != {expect_fam}")
    if len(targets) != 182:
        die(f"targets={len(targets)} != 182")

    already = '"to": "op_32"' in kg_raw
    if already:
        print("op_32 YA presente en knowledge_graph — modo validate-only\n")
    else:
        # ── 1) kg: insertar edge op_32 tras el ultimo op-edge de cada target ──
        kg_lines = kg_raw.splitlines(keepends=True)
        parsed = []
        for i, ln in enumerate(kg_lines):
            s = ln.strip()
            if not s:
                die(f"linea vacia {i+1} en kg")
            try:
                parsed.append(json.loads(s))
            except Exception as e:
                die(f"kg no parsea linea {i+1}: {e}")
        if len(parsed) != EXPECTED_BEFORE:
            die(f"kg pre-condicion: {len(parsed)} lineas != {EXPECTED_BEFORE}")
        last_op_idx: dict[str, int] = {}
        for i, e in enumerate(parsed):
            if e.get("type") == "Strategy->Operator":
                last_op_idx[e["from"]] = i
        missing = tset - set(last_op_idx)
        if missing:
            die(f"targets sin bloque op-edge: {sorted(missing)}")
        insert_after = {last_op_idx[s]: s for s in targets if True}
        # un solo insert por target garantizado (indices unicos por definicion)
        if len(insert_after) != len(targets):
            die("colision de indices de insercion")
        new_lines: list[str] = []
        for i, ln in enumerate(kg_lines):
            new_lines.append(ln)
            if i in insert_after:
                e = parsed[i]
                to_num = int(e["to"].split("_")[1])
                if to_num >= 32:
                    die(f"{e['from']}: ultimo op {e['to']} !< op_32")
                _, eol = split_eol(ln)
                new_lines.append(
                    '{"from": "%s", "rel": "USES_SECONDARY", "to": "op_32", '
                    '"type": "Strategy->Operator"}%s' % (insert_after[i], eol)
                )
        new_kg = "".join(new_lines)

        # ── 2) cm: cirugia en el bloque operators de cada entry target ───────
        cm_lines = cm_raw.splitlines(keepends=True)
        entry_re = re.compile(r'^  "(MEV-\d{2}-\d{3})": \{')
        close_re = re.compile(r"^  \},?$")
        cur = None
        in_ops = False
        last_item = None  # (idx, indent)
        plan: dict[str, tuple[int, str]] = {}  # sid -> (last_item_idx, indent)
        for i, ln in enumerate(cm_lines):
            body, _ = split_eol(ln)
            m = entry_re.match(body)
            if m:
                cur = m.group(1)
                in_ops = False
                last_item = None
                continue
            if close_re.match(body):
                cur = None
                in_ops = False
                last_item = None
                continue
            if cur is not None:
                if body.strip() == '"operators": [':
                    in_ops = True
                    last_item = None
                elif in_ops and body.strip() == "],":
                    if cur in tset:
                        if last_item is None:
                            die(f"{cur}: bloque operators vacio/inesperado")
                        plan[cur] = last_item
                    in_ops = False
                elif in_ops and body.strip().startswith('"op_'):
                    indent = body[: len(body) - len(body.lstrip())]
                    last_item = (i, indent)
                elif in_ops and body.strip() == "[]":
                    die(f"{cur}: operators vacio")
        if set(plan) != tset:
            die(f"plan cm incompleto: faltan {sorted(tset - set(plan))[:5]}")
        # aplicar de abajo hacia arriba
        for sid in sorted(plan, key=lambda s: plan[s][0], reverse=True):
            idx, indent = plan[sid]
            body, eol = split_eol(cm_lines[idx])
            if body.endswith(","):
                die(f"{sid}: ultimo item ya tiene coma")
            cm_lines[idx] = body + "," + eol
            cm_lines.insert(idx + 1, f'{indent}"op_32"{eol}')
        new_cm = "".join(cm_lines)

        # ── 3) STRATEGY.json + SKILL.md por target ──────────────────────────
        new_strat: dict[str, tuple[str, str]] = {}
        for sid in targets:
            p = os.path.join(STRATS, sid, "STRATEGY.json")
            raw = read_raw(p)
            lines = raw.splitlines(keepends=True)
            in_sec = False
            last_item = None
            for i, ln in enumerate(lines):
                body, _ = split_eol(ln)
                if body.strip() == '"secondary_operators": [':
                    in_sec = True
                    last_item = None
                elif in_sec and body.strip() == "]":
                    if last_item is None:
                        die(f"{sid}: secondary_operators vacio")
                    idx, indent = last_item
                    b, eol = split_eol(lines[idx])
                    if b.endswith(","):
                        die(f"{sid}: ultimo item ya con coma")
                    lines[idx] = b + "," + eol
                    lines.insert(idx + 1, f'{indent}"op_32"{eol}')
                    in_sec = False
                    break
                elif in_sec and body.strip().startswith('"op_'):
                    indent = body[: len(body) - len(body.lstrip())]
                    last_item = (i, indent)
            else:
                die(f"{sid}: no encontre cierre de secondary_operators")
            new_strat[sid] = ("".join(lines), None)  # json despues

            sp = os.path.join(STRATS, sid, "SKILL.md")
            sraw = read_raw(sp)
            slines = sraw.splitlines(keepends=True)
            hits = 0
            for i, ln in enumerate(slines):
                body, eol = split_eol(ln)
                if body.startswith("- **Secondary**: "):
                    hits += 1
                    if "op_32" in body:
                        die(f"{sid}: op_32 ya en SKILL.md")
                    slines[i] = body + ", op_32" + eol
            if hits != 1:
                die(f"{sid}: Secondary lines = {hits}")
            new_strat[sid] = (new_strat[sid][0], "".join(slines))

        # ── WRITE (todo lo anterior murio antes si algo estaba mal) ─────────
        write_raw(KG, new_kg)
        write_raw(CM, new_cm)
        for sid, (j, s) in new_strat.items():
            write_raw(os.path.join(STRATS, sid, "STRATEGY.json"), j)
            write_raw(os.path.join(STRATS, sid, "SKILL.md"), s)
        os.makedirs(OP32, exist_ok=True)
        pj = os.path.join(OP32, "OPERATOR.json")
        ps = os.path.join(OP32, "SKILL.md")
        if not os.path.exists(pj):
            # CRLF como los siblings (op_31/OPERATOR.json = 11 lineas CRLF)
            txt = json.dumps(OP32_OPERATOR_JSON, ensure_ascii=False, indent=2)
            write_raw(pj, txt.replace("\n", "\r\n"))
        if not os.path.exists(ps):
            write_raw(ps, OP32_SKILL_MD.replace("\n", "\r\n"))
        print(f"APLICADO: 182 edges op_32 USES_SECONDARY + 182 STRATEGY.json/SKILL.md "
              f"+ operators/op_32/ creado\n")

    # ── VALIDATOR (re-lee TODO de disco) ───────────────────────────────────
    ok = True
    out = []

    def check(cond: bool, label: str) -> None:
        nonlocal ok
        out.append(f"[{'PASS' if cond else 'FAIL'}] {label}")
        if not cond:
            ok = False

    kg2 = read_raw(KG)
    lines = [l for l in kg2.splitlines() if l.strip()]
    edges, perr = [], 0
    for ln in lines:
        try:
            edges.append(json.loads(ln))
        except Exception:
            perr += 1
    check(perr == 0, f"JSONL parsea 100% ({len(lines)} lineas, {perr} errores)")
    check(len(lines) == EXPECTED_BEFORE + 182,
          f"conteo total = {EXPECTED_BEFORE} + 182 = {EXPECTED_BEFORE + 182} "
          f"(real: {len(lines)})")
    check(kg2.endswith("\n"), "kg termina en newline")

    seen, dupes = set(), 0
    for e in edges:
        k = (e.get("from"), e.get("rel"), e.get("to"))
        if k in seen:
            dupes += 1
        seen.add(k)
    check(dupes == 0, f"sin edges duplicados (from,rel,to) — dupes={dupes}")

    valid_pairs = {
        ("Strategy->Detector", "DETECTED_BY"),
        ("Strategy->Operator", "USES_PRIMARY"),
        ("Strategy->Operator", "USES_SECONDARY"),
        ("Strategy->Family", "BELONGS_TO"),
        ("Strategy->Surface", "REQUIRES_SURFACE"),
    }
    check(all(set(e.keys()) == {"from", "rel", "to", "type"} for e in edges)
          and all((e["type"], e["rel"]) in valid_pairs for e in edges),
          "schema homogeneo (4 keys exactas + enums type/rel)")

    op32 = [e for e in edges if e.get("to") == "op_32"]
    check(len(op32) == 182, f"op_32 edges = 182 (real: {len(op32)})")
    check(all(e["rel"] == "USES_SECONDARY" and e["type"] == "Strategy->Operator"
              for e in op32),
          "todos los op_32 son USES_SECONDARY / Strategy->Operator")
    bad = [e["from"] for e in op32 if e["from"] not in tset]
    check(not bad, f"op_32 solo en las 182 estrategias objetivo ({bad[:5]})")
    obs32 = [e["from"] for e in op32 if e["from"] in observe]
    check(not obs32, f"ningun OBSERVE lleva op_32 ({obs32})")
    leak = [e["from"] for e in op32 if fam_of(e["from"]) in NO_EDGE_FAMILIES]
    check(not leak, f"ninguna familia R8-sin-edge lleva op_32 ({leak})")

    cm2 = json.loads(read_raw(CM))
    kg_ops: dict[str, set] = {}
    for e in edges:
        if e["type"] == "Strategy->Operator":
            kg_ops.setdefault(e["from"], set()).add(e["to"])
    mism = sum(1 for s, en in cm2.items()
               if s != "Legend" and set(en["operators"]) != kg_ops.get(s, set()))
    check(mism == 0, f"kg ≡ capability_matrix 264/264 (mismatches: {mism})")

    sec_re = re.compile(r"^- \*\*Secondary\*\*: (.+?)\s*$")
    sj_mism = sk_mism = 0
    for sid in cm2:
        if sid == "Legend":
            continue
        sj = json.loads(read_raw(os.path.join(STRATS, sid, "STRATEGY.json")))
        if set(sj["primary_operators"]) | set(sj["secondary_operators"]) != kg_ops.get(sid, set()):
            sj_mism += 1
        sl = read_raw(os.path.join(STRATS, sid, "SKILL.md")).splitlines()
        secs = [sec_re.match(l).group(1) for l in sl if sec_re.match(l)]
        if len(secs) != 1 or ("op_32" in secs[0]) != (sid in tset):
            sk_mism += 1
    check(sj_mism == 0, f"kg ≡ STRATEGY.json 264/264 (mismatches: {sj_mism})")
    check(sk_mism == 0, f"SKILL.md Secondary espeja op_32 exactamente en los 182 "
                        f"(mal: {sk_mism})")

    pj = os.path.join(OP32, "OPERATOR.json")
    ps = os.path.join(OP32, "SKILL.md")
    check(os.path.isfile(pj) and os.path.isfile(ps),
          "operators/op_32/{OPERATOR.json,SKILL.md} existen")
    op32j = json.loads(read_raw(pj))
    check(list(op32j.keys()) == ["id", "code", "name", "canonical_role", "enabled",
                                 "engine_present", "calibration_state", "formula",
                                 "implementation_file"],
          "OPERATOR.json op_32 schema = patron op_01..op_31 (9 keys)")
    check(op32j["engine_present"] is False
          and op32j["calibration_state"] == "NOT_IMPLEMENTED",
          "op_32 honesto: engine_present=false / NOT_IMPLEMENTED (HP-03 pendiente)")

    per_fam = {}
    for e in op32:
        per_fam[fam_of(e["from"])] = per_fam.get(fam_of(e["from"]), 0) + 1
    check(per_fam == expect_fam, f"conteo por familia {per_fam}")

    print("===== HP-04 VALIDATOR =====")
    for l in out:
        print(l)
    print("VALIDATOR:", "ALL PASS" if ok else "FAILURES PRESENT")
    return 0 if ok else 1


if __name__ == "__main__":
    raise SystemExit(main())
