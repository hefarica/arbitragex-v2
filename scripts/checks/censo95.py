#!/usr/bin/env python3
"""Recalcula y valida el censo 95 % (docs/cumplimiento-95/censo-<fecha>.json).

Contrato: un censo sin población completa NO tiene porcentaje. Este script no
"aproxima" nada: o el fichero es consistente con sus propias filas, o sale con
código != 0 y dice qué fila lo rompe.

Reglas que aplica:
  - una fila por id, id único y no vacío;
  - `familia` y `estado` dentro del enum de §0.2 / Anexo A;
  - si `sin_fila > 0` el porcentaje es NOT COMPUTED (nunca 0, nunca «provisional»);
  - las tres cifras se recalculan desde `filas`, no se copian del resumen;
  - `sin_fila` NO se puede maquillar de `vacuo`: `vacuo` exige razón y fuente.

Uso:
  python3 scripts/checks/censo95.py [ruta.json] [--json]
Salida: tres cifras, C95, sha256 del fichero. Exit 0 sólo si todo cuadra.
"""

from __future__ import annotations

import hashlib
import json
import sys
from pathlib import Path

REQUIRED = (
    "id",
    "familia",
    "dueño",
    "criterio_aceptacion",
    "estado",
    "evidencia",
    "fuente",
    "ts_observado",
)
FAMILIAS = {"codigo", "config", "humano", "vacuo", "duplicado", "bloqueo", "rojo-permanente"}
ESTADOS = {"hecho", "abierto", "bloqueado", "rojo-permanente"}

DEFAULT = Path("docs/cumplimiento-95/censo-2026-10-21.json")


def fail(msg: str) -> "NoReturn":  # type: ignore[name-defined]
    print(f"FAIL: {msg}", file=sys.stderr)
    raise SystemExit(1)


def main(argv: list[str]) -> int:
    as_json = "--json" in argv
    args = [a for a in argv if not a.startswith("--")]
    path = Path(args[0]) if args else DEFAULT
    if not path.is_file():
        fail(f"no existe el censo: {path}")

    raw = path.read_bytes()
    doc = json.loads(raw.decode("utf-8"))
    filas = doc.get("filas")
    if not isinstance(filas, list) or not filas:
        fail("`filas` ausente o vacía")
    poblacion = doc.get("poblacion_declarada")
    if not isinstance(poblacion, int) or poblacion <= 0:
        fail("`poblacion_declarada` debe ser un entero > 0")

    vistos: set[str] = set()
    for i, fila in enumerate(filas):
        for campo in REQUIRED:
            if not str(fila.get(campo, "")).strip():
                fail(f"fila {i} ({fila.get('id', '?')}): campo `{campo}` vacío — sin dueño o criterio no cuenta (§0.2)")
        if fila["id"] in vistos:
            fail(f"id duplicado: {fila['id']}")
        vistos.add(fila["id"])
        if fila["familia"] not in FAMILIAS:
            fail(f"{fila['id']}: familia `{fila['familia']}` fuera del enum")
        if fila["estado"] not in ESTADOS:
            fail(f"{fila['id']}: estado `{fila['estado']}` fuera del enum")
        if fila["familia"] == "vacuo" and "razón" not in fila.get("evidencia", "").lower() and "razon" not in fila.get("evidencia", "").lower():
            fail(f"{fila['id']}: `vacuo` sin razón escrita en `evidencia` (D.3.7)")

    con_fila = len(filas)
    if con_fila > poblacion:
        fail(f"con_fila={con_fila} > población declarada {poblacion}")

    sin_fila = poblacion - con_fila
    cerrados = sum(1 for f in filas if f["estado"] == "hecho")
    abiertos = sum(1 for f in filas if f["estado"] == "abierto")
    bloqueados = sum(1 for f in filas if f["estado"] == "bloqueado")
    rojos = sum(1 for f in filas if f["estado"] == "rojo-permanente")
    vacuo = sum(1 for f in filas if f["familia"] == "vacuo")
    duplicado = sum(1 for f in filas if f["familia"] == "duplicado")

    c95 = "NOT COMPUTED" if sin_fila > 0 else f"{100.0 * cerrados / poblacion:.2f} %"

    calc = {
        "poblacion_declarada": poblacion,
        "con_fila": con_fila,
        "sin_fila": sin_fila,
        "cerrados_por_medicion": cerrados,
        "abiertos": abiertos,
        "bloqueados": bloqueados,
        "rojo_permanente": rojos,
        "vacuo": vacuo,
        "duplicado": duplicado,
        "C95": c95,
    }

    guardado = doc.get("resumen") or {}
    for k, v in calc.items():
        if guardado.get(k) != v:
            fail(f"resumen incoherente en `{k}`: fichero={guardado.get(k)!r} recalculado={v!r}")

    if sin_fila > 0 and guardado.get("motivo", "").find("NOT COMPUTED") == -1:
        fail("sin_fila>0 exige `motivo` con NOT COMPUTED")

    sha = hashlib.sha256(raw).hexdigest()
    salida = {
        "fichero": str(path),
        "sha256": sha,
        "poblacion_declarada": poblacion,
        "con_fila": con_fila,
        "sin_fila": sin_fila,
        "cerrados_por_medicion": cerrados,
        "C95": c95,
        "GATE_VIVO": "se publica aparte, por su nombre y su ventana (D.2.2)",
        "HITO_ECONOMICO": "se publica aparte, binario: recon.total>0 (D.2.3)",
    }
    if as_json:
        print(json.dumps(salida, ensure_ascii=False, indent=2))
    else:
        print(f"fichero ................ {path}")
        print(f"sha256 ................. {sha}")
        print(f"poblacion_declarada .... {poblacion}   (declarada, no auditada)")
        print(f"con_fila ............... {con_fila}")
        print(f"sin_fila ............... {sin_fila}")
        print(f"cerrados_por_medicion .. {cerrados}")
        print(f"C95 .................... {c95}")
        print("GATE-VIVO / HITO-ECONÓMICO: se publican aparte, con su nombre y ventana.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
