#!/usr/bin/env python3
"""paper-runs-diff.py — control diferencial: v1.4.1 (fixture congelado) vs v1.6.x (instrumento vivo).

Regla de la casa (MANTIS 6992894, 2026-10-20):
  - el delta se publica POR `check_id` Y POR RAMA, nunca por número de sentencia;
  - cada fila declara población/presentes/nulos y `presentes == 0` es NOT COMPUTED,
    nunca verde (contrato §9.4);
  - un check que v1.4.1 tenía y v1.6.x perdió es un HALLAZGO que se arregla en v1.6.x.
    El fichero viejo no se resucita: por eso vive en scripts/fixtures/, no en scripts/.

Lo que este runner NO hace: no juzga si un veredicto es correcto. Publica el delta y
la población; la lectura del veredicto es del dueño del instrumento.

Uso con DB:
  python3 scripts/checks/paper-runs-diff.py \\
      --database-url "$DATABASE_URL" \\
      --cutoff 2026-10-20T02:00:00Z \\
      --export-sha256 <sha256 del export> \\
      --window-hours 24 \\
      --out diff-20261020.json

Uso sin DB (sandbox, CI):
  python3 scripts/checks/paper-runs-diff.py --self-test

Salida: código 0 = sin hallazgos; 1 = hallazgos (check perdido, o veredicto distinto por
check_id); 2 = error del runner. Un check en NOT COMPUTED NO es un hallazgo: es estado.
"""

from __future__ import annotations

import argparse
import csv
import io
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
from dataclasses import dataclass, field
from datetime import datetime, timezone

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(os.path.dirname(HERE))
DEFAULT_OLD = os.path.join(REPO, "scripts", "fixtures", "paper-runs-checks-v1.4.1.sql")
DEFAULT_NEW = os.path.join(REPO, "scripts", "paper-runs-checks.sql")
DEFAULT_MANIFEST = os.path.join(REPO, "scripts", "paper-runs-checks.check_id.manifest.txt")

# Columnas que declaran población. Si TODAS las presentes valen 0 y el veredicto dice OK
# o AMARILLO, la fila está mintiendo por proximidad: se degrada a NOT COMPUTED.
POP_COLS = (
    "filas_ventana", "filas", "presentes", "resueltas", "con_decision",
    "con_medido", "pares", "poblacion", "n",
)
VERDICT_COL = "verdict"
CHECK_COL = "check_id"
RAMA_COL = "rama"

# Estados que eximen de la compuerta de población.
NON_GREEN = ("NOT COMPUTED", "ROJO", "WARN", "ERROR", "VACUO", "NO ROW")

WRAPPER = re.compile(r"^(BEGIN|COMMIT|ROLLBACK|START\s+TRANSACTION|SET\s+TRANSACTION)\b", re.I)
COMMENT_ID = re.compile(r"^--\s*(C\d+[a-z]?)\b")


@dataclass
class Statement:
    idx: int
    line: int
    check_id: str | None
    text: str


@dataclass
class Block:
    stmt: Statement
    rows: list[dict] = field(default_factory=list)
    error: str | None = None
    stderr: str = ""


# ── lectura del SQL ────────────────────────────────────────────────────────────

def split_sql(path: str) -> list[Statement]:
    """Corta el fichero por sentencias terminadas en ';' y anota línea de inicio y
    el `-- Cxx` que la precede. Descarta los envoltorios de transacción: el runner
    abre su propia transacción READ ONLY, para que las dos versiones vean el mismo
    tipo de snapshot y para poder aislar el error de una sentencia con SAVEPOINT."""
    lines = open(path, encoding="utf-8").read().split("\n")
    out: list[Statement] = []
    cur: list[str] = []
    start = None
    last_id = None
    for i, line in enumerate(lines, 1):
        m = COMMENT_ID.match(line)
        if m:
            last_id = m.group(1)
        stripped = line.strip()
        # Las metacomandos de psql (\set ON_ERROR_STOP on) no son SQL: el runner fija los
        # suyos (ON_ERROR_STOP off + SAVEPOINT) y no debe reenviarlos dentro del bloque.
        if not stripped or stripped.startswith("--") or stripped.startswith("\\"):
            continue
        if start is None:
            start = i
        cur.append(line)
        if stripped.endswith(";"):
            text = "\n".join(cur)
            if not WRAPPER.match(text.strip()):
                out.append(Statement(len(out), start, last_id, text))
            cur, start = [], None
    if cur:
        text = "\n".join(cur)
        if not WRAPPER.match(text.strip()):
            out.append(Statement(len(out), start, last_id, text))
    return out


def load_manifest(path: str) -> list[dict]:
    """Manifiesto: una fila por RAMA de ejecución. Cabecera `#` ignorada."""
    if not os.path.exists(path):
        return []
    rows = []
    with open(path, encoding="utf-8") as fh:
        for line in fh:
            line = line.rstrip("\n")
            if not line or line.startswith("#"):
                continue
            if line.startswith("check_id"):
                continue
            parts = line.split("\t")
            if len(parts) < 3:
                continue
            rows.append({"check_id": parts[0], "rama": parts[1], "linea": int(parts[2])})
    return rows


def instrument(path: str, statements: list[Statement]) -> str:
    """Devuelve un guion psql con marcadores por sentencia y SAVEPOINT por bloque.

    El `\\echo @@B:<idx>`/`@@E:<idx>` permite atribuir cada bloque de CSV a su
    sentencia sin adivinar por orden; el SAVEPOINT permite que una sentencia que
    revienta (p.ej. ERROR 42703) no aborte la transacción y deje el resto sin medir.
    """
    parts = [
        "\\set ON_ERROR_STOP off",
        "\\pset format csv",
        "\\pset tuples_only off",
        "BEGIN TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY;",
    ]
    for st in statements:
        parts.append(f"\\echo @@B:{st.idx}:{st.line}")
        parts.append(f"SAVEPOINT sp_{st.idx};")
        parts.append(st.text)
        parts.append(f"\\echo @@E:{st.idx}")
        parts.append("\\if :ERROR")
        parts.append(f"ROLLBACK TO SAVEPOINT sp_{st.idx};")
        parts.append("\\endif")
    parts.append("ROLLBACK;")
    return "\n".join(parts) + "\n"


# ── ejecución ──────────────────────────────────────────────────────────────────

def run_sql(database_url: str, path: str, cutoff: str, timeout: int = 300) -> tuple[list[Block], str]:
    statements = split_sql(path)
    script = instrument(path, statements)
    by_idx = {st.idx: st for st in statements}
    with tempfile.NamedTemporaryFile("w", suffix=".psql", delete=False, encoding="utf-8") as fh:
        fh.write(script)
        tmp = fh.name
    try:
        proc = subprocess.run(
            ["psql", database_url, "-X", "-q", "-v", f"cutoff={cutoff}", "-f", tmp],
            capture_output=True, text=True, timeout=timeout,
        )
    finally:
        os.unlink(tmp)

    blocks: dict[int, Block] = {}
    cur = None
    buf: list[str] = []
    for line in proc.stdout.split("\n"):
        m = re.match(r"^@@B:(\d+):(\d+)$", line.strip())
        if m:
            cur = Block(by_idx[int(m.group(1))])
            blocks[cur.stmt.idx] = cur
            buf = []
            continue
        if line.strip().startswith("@@"):
            if cur is not None:
                cur.rows = parse_csv_block("\n".join(buf))
                cur.error = None if cur.rows else "no publicó fila"
            buf = []
            continue
        if cur is not None:
            buf.append(line)
    # El error de una sentencia no se puede leer del stdout (psql va a stderr y no dice
    # la línea en formato estable): se atribuye al bloque vacío más su stderr completo.
    if proc.returncode != 0 and proc.stderr.strip():
        for b in blocks.values():
            b.stderr = proc.stderr.strip()[:400]
    return list(blocks.values()), proc.stderr


def parse_csv_block(text: str) -> list[dict]:
    text = text.strip("\n")
    if not text.strip():
        return []
    rows = list(csv.reader(io.StringIO(text)))
    if not rows:
        return []
    header = [h.strip() for h in rows[0]]
    if CHECK_COL not in header:
        return []
    out = []
    for r in rows[1:]:
        if not r or all(not c.strip() for c in r):
            continue
        out.append({h: (r[i].strip() if i < len(r) else "") for i, h in enumerate(header)})
    return out


# ── atribución y compuertas ────────────────────────────────────────────────────

# Una rama `sin filas` se llama igual en las dos versiones: v1.4.1 no imprime nada y
# v1.6.x imprime una fila sintética con `(sin filas)` en el discriminador. Si se deja
# el literal, el diff publica «sin_filas → (sin filas)» como si fuera un cambio de
# clase, y eso es deriva de etiqueta, no hallazgo (Jeffrey 6993815: 3 de 6 filas).
SIN_FILAS = {"", "sin_filas", "(sin filas)", "sin filas", "(sin_filas)", "sin-filas"}

# check_id -> columna que discrimina su rama. C10/C12b/C13/C15b son los únicos con más
# de una rama en el manifiesto; el resto emite una sola.
RAMA_DISCRIMINATOR = {
    "C10": "gas_measurement_state",
    "C12b": "lista",
    "C13": "estado",
    "C15b": "procedencia",
}


def canon_rama(value: str) -> str:
    v = (value or "").strip()
    return "sin_filas" if v.lower() in SIN_FILAS else v


def resolve_rama(check_id: str, row: dict, manifest_rows: list[dict]) -> str:
    """La rama es del DATO, no del número de línea del fichero ni del manifiesto.

    El manifiesto describe v1.6.x; aplicado al fixture v1.4.1 atribuye mal, así que se
    usa sólo como último recurso. El discriminador manda, y `sin filas` se canoniza.
    """
    explicit = row.get(RAMA_COL, "").strip()
    if explicit:
        return canon_rama(explicit)
    # Sólo estos checks tienen más de una rama en el manifiesto, y cada uno tiene UNA
    # columna discriminadora. Se declara la tabla en vez de adivinar: cuando el
    # discriminador faltaba (C15b), el fallback unía las ramas del manifiesto en
    # `veredicto+sin_filas` y el mismo check salía dos veces en `no_comparables`
    # (Jeffrey 7001460).
    col = RAMA_DISCRIMINATOR.get(check_id)
    if col:
        val = (row.get(col) or row.get("familia") or "").strip()
        if check_id == "C10":
            return canon_rama(val) or "(NULL)"
        if check_id == "C12b":
            return "sin_filas" if canon_rama(val) == "sin_filas" else f"lista={val}"
        return canon_rama(val) or "sin_filas"
    ramas = [m["rama"] for m in manifest_rows if m["check_id"] == check_id]
    return ramas[0] if len(ramas) == 1 else ("unica" if not ramas else "+".join(ramas))


def population_gate(row: dict) -> tuple[str, bool]:
    """presentes == 0 nunca es verde. Devuelve (veredicto, degradado)."""
    verdict = (row.get(VERDICT_COL) or "").strip()
    if not verdict:
        return verdict, False
    if any(verdict.upper().startswith(s) for s in NON_GREEN):
        return verdict, False
    pops = []
    for col in POP_COLS:
        val = (row.get(col) or "").strip()
        if val and re.fullmatch(r"-?\d+", val):
            pops.append(int(val))
    if pops and all(v == 0 for v in pops):
        return f"NOT COMPUTED: población 0 en todas las columnas ({'/'.join(c for c in POP_COLS if row.get(c))})", True
    return verdict, False


def verdict_class(verdict: str) -> str:
    v = (verdict or "").strip().upper()
    for token in ("NOT COMPUTED", "ROJO", "AMARILLO", "WARN", "VACUO", "OK"):
        if v.startswith(token):
            return "WARN" if token == "AMARILLO" else token
    return v.split(":")[0] if v else "NO ROW"


# ── informe ────────────────────────────────────────────────────────────────────

def summarize(blocks: list[Block], manifest_rows: list[dict], label: str) -> dict:
    per_check: dict[str, list[dict]] = {}
    for b in blocks:
        cid = b.stmt.check_id
        rows = b.rows or []
        if not rows:
            per_check.setdefault(cid or f"L{b.stmt.line}", []).append({
                "rama": "sin_filas",
                "verdict": "NO ROW",
                "clase": verdict_class("NO ROW"),
                "gate": False,
                "error": b.error or "sin filas",
                "cols": {},
            })
            continue
        for row in rows:
            cid_row = row.get(CHECK_COL) or cid or f"L{b.stmt.line}"
            verdict, gated = population_gate(row)
            per_check.setdefault(cid_row, []).append({
                "rama": resolve_rama(cid_row, row, manifest_rows),
                "verdict": verdict,
                "clase": verdict_class(verdict),
                "gate": gated,
                "error": None,
                "cols": {k: v for k, v in row.items() if k not in (VERDICT_COL,)},
            })
    return {"version": label, "checks": per_check}


def diff(old: dict, new: dict) -> dict:
    old_ids, new_ids = set(old["checks"]), set(new["checks"])
    lost = sorted(old_ids - new_ids)
    added = sorted(new_ids - old_ids)
    changed, no_comparable = [], []
    for cid in sorted(old_ids & new_ids):
        o = {r["rama"]: r["clase"] for r in old["checks"][cid]}
        n = {r["rama"]: r["clase"] for r in new["checks"][cid]}
        for rama in sorted(set(o) | set(n)):
            a, b = o.get(rama, "AUSENTE"), n.get(rama, "AUSENTE")
            if a == b:
                continue
            fila = {"check_id": cid, "rama": rama, "v141": a, "v16x": b}
            # Un lado sin fila o sin población no es un cambio de clase: es que ese lado
            # no midió. Va aparte y NO dispara exit 1 (regla publicada en 6993650).
            if {"NO ROW", "NOT COMPUTED", "AUSENTE"} & {a, b}:
                no_comparable.append(fila)
            else:
                changed.append(fila)
    return {"perdidos": lost, "nuevos": added, "cambiados": changed,
            "no_comparables": no_comparable}


def render_md(report: dict) -> str:
    h = report["header"]
    lines = [
        "## Diferencial del instrumento — v1.4.1 (fixture) vs v1.6.x (vivo)",
        "",
        f"`cutoff={h['cutoff']}` · `window_hours={h['window_hours']}` · "
        f"`export_sha256={h['export_sha256']}` · ejecutado `{h['run_at']}`",
        "",
        f"- ids v1.4.1: **{h['ids_v141']}** · ids v1.6.x: **{h['ids_v16x']}**",
        f"- filas por rama v1.4.1: **{h['ramas_v141']}** · v1.6.x: **{h['ramas_v16x']}**",
        f"- compuerta de población degradada a NOT COMPUTED: **{h['gated']}** fila(s)",
        "",
    ]
    d = report["delta"]
    lines.append(f"**Perdidos en v1.6.x (hallazgo):** {', '.join(d['perdidos']) or 'ninguno'}")
    lines.append(f"**Nuevos en v1.6.x:** {', '.join(d['nuevos']) or 'ninguno'}")
    lines.append("")
    if d["cambiados"]:
        lines += ["", "**Cambios de clase (delta):**", "",
                  "| check_id | rama | v1.4.1 | v1.6.x |", "|---|---|---|---|"]
        for row in d["cambiados"]:
            lines.append(f"| {row['check_id']} | {row['rama']} | {row['v141']} | {row['v16x']} |")
    else:
        lines.append("Sin cambios de clase de veredicto por `check_id`.")
    if d.get("no_comparables"):
        lines += ["", "**No comparable (un lado no midió; no cuenta como cambio):**", "",
                  "| check_id | rama | v1.4.1 | v1.6.x |", "|---|---|---|---|"]
        for row in d["no_comparables"]:
            lines.append(f"| {row['check_id']} | {row['rama']} | {row['v141']} | {row['v16x']} |")
    return "\n".join(lines)


def self_test() -> int:
    """Valida splitter, atribución por rama, compuerta de población y delta — con datos
    sintéticos. Verde aquí NO dice nada del SQL; dice que el runner no miente al agregar."""
    failures = []

    # 1. splitter sobre los dos ficheros reales: nº de sentencias y de ids, y que no se
    #    cuele ni el BEGIN/ROLLBACK de envoltorio ni el metacomando \set.
    for path, expect_ids, expect_stmts in ((DEFAULT_OLD, 22, 22), (DEFAULT_NEW, 23, 23)):
        if not os.path.exists(path):
            failures.append(f"falta {path}")
            continue
        sts = split_sql(path)
        ids = sorted({s.check_id for s in sts if s.check_id})
        if len(ids) != expect_ids:
            failures.append(f"{os.path.basename(path)}: {len(ids)} ids, esperaba {expect_ids}")
        if len(sts) != expect_stmts:
            failures.append(f"{os.path.basename(path)}: {len(sts)} sentencias, esperaba {expect_stmts}")
        if any(s.check_id is None for s in sts):
            failures.append(f"{os.path.basename(path)}: sentencia sin check_id (metacomando colado)")
        if any(WRAPPER.match(s.text.strip()) for s in sts):
            failures.append(f"{os.path.basename(path)}: se colaron BEGIN/ROLLBACK como sentencia")
        if any(s.text.lstrip().startswith("\\") for s in sts):
            failures.append(f"{os.path.basename(path)}: se coló un metacomando psql")

    # 2. atribución de rama: C10 canaliza por gas_measurement_state
    row = {"check_id": "C10", "gas_measurement_state": "measured", "filas": "12", "verdict": "OK"}
    if resolve_rama("C10", row, []) != "measured":
        failures.append("C10 no atribuye la rama por gas_measurement_state")

    # 3. compuerta: OK con población 0 se degrada
    v, gated = population_gate({"verdict": "OK", "filas_ventana": "0", "filas": "0"})
    if not gated or not v.startswith("NOT COMPUTED"):
        failures.append("OK con población 0 no se degrada")

    # 4. un NOT COMPUTED legítimo no se toca
    v, gated = population_gate({"verdict": "NOT COMPUTED: sin filas", "filas": "0"})
    if gated:
        failures.append("NOT COMPUTED se degradó dos veces")

    # 5. un check presente en v1.4.1 y ausente en v1.6.x es hallazgo
    old = {"version": "v1.4.1", "checks": {"C01": [{"rama": "unica", "clase": "OK"}],
                                            "C09": [{"rama": "unica", "clase": "OK"}]}}
    new = {"version": "v1.6.x", "checks": {"C01": [{"rama": "unica", "clase": "ROJO"}]}}
    d = diff(old, new)
    if d["perdidos"] != ["C09"]:
        failures.append(f"no detectó el check perdido: {d['perdidos']}")
    if not d["cambiados"] or d["cambiados"][0]["check_id"] != "C01":
        failures.append("no detectó el cambio de clase en C01")

    # 6. csv: cabecera + fila con comas entrecomilladas
    rows = parse_csv_block('check_id,verdict\nC01,"WARN: a, b"\n')
    if len(rows) != 1 or rows[0]["verdict"] != "WARN: a, b":
        failures.append("el parser CSV rompe con comas entrecomilladas")

    # 7. un bloque sin filas no revienta el diff (KeyError 'clase', Jeffrey 6993815)
    empty = summarize([Block(Statement(0, 41, "C09", "SELECT 1;"))], [], "v")
    try:
        diff(empty, {"version": "n", "checks": {"C09": [{"rama": "sin_filas", "clase": "NO ROW"}]}})
    except KeyError as exc:
        failures.append(f"fila NO ROW sin 'clase': KeyError {exc}")

    # 8. `sin filas` canónico: el literal de cada versión no es un cambio de clase
    if canon_rama("(sin filas)") != "sin_filas" or canon_rama("") != "sin_filas":
        failures.append("canon_rama no unifica (sin filas)/vacío")
    if resolve_rama("C13", {"estado": "(sin filas)"}, []) != "sin_filas":
        failures.append("C13 no canoniza la rama sin filas")
    if resolve_rama("C12b", {"lista": "(sin filas)"}, []) != "sin_filas":
        failures.append("C12b no canoniza la rama sin filas")
    a = {"version": "v1.4.1", "checks": {"C10": [{"rama": "sin_filas", "clase": "NO ROW"}]}}
    b = {"version": "v1.6.x", "checks": {"C10": [{"rama": "sin_filas", "clase": "NO ROW"}]}}
    if diff(a, b)["cambiados"] or diff(a, b)["no_comparables"]:
        failures.append("la rama sin filas genera delta espurio")

    # 10. C15b se atribuye por `procedencia` y una sola fila por check en no_comparables
    if resolve_rama("C15b", {"procedencia": "(sin filas)"}, []) != "sin_filas":
        failures.append("C15b no canoniza la rama sin filas por procedencia")
    a = {"version": "v1.4.1", "checks": {"C15b": [{"rama": "sin_filas", "clase": "NO ROW"}]}}
    b = {"version": "v1.6.x", "checks": {"C15b": [{"rama": "sin_filas", "clase": "NOT COMPUTED"}]}}
    d3 = diff(a, b)
    if len(d3["no_comparables"]) != 1 or d3["no_comparables"][0]["rama"] != "sin_filas":
        failures.append(f"C15b no colapsa a una fila: {d3['no_comparables']}")

    # 9. NOT COMPUTED frente a OK es no comparable, no cambio de clase
    a = {"version": "v1.4.1", "checks": {"C14": [{"rama": "unica", "clase": "OK"}]}}
    b = {"version": "v1.6.x", "checks": {"C14": [{"rama": "unica", "clase": "NOT COMPUTED"}]}}
    d2 = diff(a, b)
    if d2["cambiados"]:
        failures.append("NOT COMPUTED contado como cambio de clase")
    if len(d2["no_comparables"]) != 1:
        failures.append("NOT COMPUTED no aparece en no_comparables")

    if failures:
        print("SELF-TEST FALLA")
        for f in failures:
            print(" -", f)
        return 1
    print("SELF-TEST OK — 10 comprobaciones (splitter real, rama, compuerta, delta, csv, NO ROW,"
          " sin_filas, no comparable, C15b)")
    print("AVISO: esto verifica el RUNNER, no el SQL. Sin una corrida con --database-url este"
          " instrumento esta NO VERIFICADO.")
    return 0


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--database-url", default=os.environ.get("DATABASE_URL"))
    ap.add_argument("--cutoff", help="ISO-8601 UTC, desnudo (sin comillas)")
    ap.add_argument("--export-sha256", default="NO DECLARADO")
    ap.add_argument("--window-hours", default="NO DECLARADO")
    ap.add_argument("--old", default=DEFAULT_OLD)
    ap.add_argument("--new", default=DEFAULT_NEW)
    ap.add_argument("--manifest", default=DEFAULT_MANIFEST)
    ap.add_argument("--out", default="paper-runs-diff.json")
    ap.add_argument("--self-test", action="store_true")
    args = ap.parse_args(argv)

    if args.self_test:
        return self_test()
    if not args.database_url:
        print("falta --database-url (o DATABASE_URL). Sin DB no hay lectura: usa --self-test.",
              file=sys.stderr)
        return 2
    if not args.cutoff:
        print("falta --cutoff: sin corte no hay ventana, y sin ventana no hay evidencia.",
              file=sys.stderr)
        return 2

    manifest = load_manifest(args.manifest)
    old_blocks, old_err = run_sql(args.database_url, args.old, args.cutoff)
    new_blocks, new_err = run_sql(args.database_url, args.new, args.cutoff)

    old_sum = summarize(old_blocks, manifest, "v1.4.1")
    new_sum = summarize(new_blocks, manifest, "v1.6.x")
    # Las columnas de población NO se recortan: cada fila del informe declara
    # población/presentes/nulos, que es lo que hace auditable el veredicto.

    report = {
        "header": {
            "run_at": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
            "cutoff": args.cutoff,
            "window_hours": args.window_hours,
            "export_sha256": args.export_sha256,
            "old_file": os.path.relpath(args.old, REPO),
            "new_file": os.path.relpath(args.new, REPO),
            "ids_v141": len(old_sum["checks"]),
            "ids_v16x": len(new_sum["checks"]),
            "ramas_v141": sum(len(v) for v in old_sum["checks"].values()),
            "ramas_v16x": sum(len(v) for v in new_sum["checks"].values()),
            "gated": sum(1 for s in (old_sum, new_sum) for rows in s["checks"].values()
                         for r in rows if r.get("gate")),
            "stderr_old": old_err.strip()[:400],
            "stderr_new": new_err.strip()[:400],
        },
        "delta": diff(old_sum, new_sum),
        "v141": old_sum,
        "v16x": new_sum,
    }
    with open(args.out, "w", encoding="utf-8") as fh:
        json.dump(report, fh, ensure_ascii=False, indent=2)
    print(render_md(report))
    print(f"\nJSON: {args.out}")
    findings = bool(report["delta"]["perdidos"] or report["delta"]["cambiados"])
    return 1 if findings else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
