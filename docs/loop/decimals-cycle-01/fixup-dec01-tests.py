#!/usr/bin/env python3
"""DECIMALS-CYCLE-01 (t197) — fix-up de los 3 literales de test.

El anclaje exacto `math_redis: redis.clone(),` solo existe en scanner.rs: los
tests construyen su `OrchestratorContext` con otra expresion. Este script
inserta `token_decimals_provider: None,` justo despues de la linea `math_redis:`
de cada literal, preservando la indentacion. Idempotente.

Uso: python3 fixup-dec01-tests.py <RAIZ_BACKEND>
"""
import pathlib
import re
import sys

FILES = [
    "searcher-rs/tests/v2_shadow_replay.rs",
    "searcher-rs/tests/cartridge_shadow_replay.rs",
    "searcher-rs/tests/orchestrator_parallel_run.rs",
]
PAT = re.compile(r"^([ \t]*)math_redis:[^\n]*,\s*$", re.M)


def main() -> None:
    root = pathlib.Path(sys.argv[1]).resolve()
    for f in FILES:
        p = root / f
        raw = p.read_text(encoding="utf-8")
        if "token_decimals_provider" in raw:
            print(f"YA_APLICADO {f}")
            continue
        m = PAT.search(raw)
        if not m or PAT.search(raw, m.end()):
            print(f"AMBIGUO_O_SIN_ANCLAJE {f} (matches={len(PAT.findall(raw))})")
            continue
        line = m.group(0)
        ins = line + "\n" + m.group(1) + "token_decimals_provider: None,"
        p.write_text(raw[: m.start()] + ins + raw[m.end() :], encoding="utf-8")
        print(f"OK {f} | anclaje={line.strip()}")


if __name__ == "__main__":
    main()
