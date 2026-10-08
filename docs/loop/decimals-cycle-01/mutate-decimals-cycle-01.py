#!/usr/bin/env python3
"""DECIMALS-CYCLE-01 (t197) — MUTANTE del falsificador.

Restaura el DEFECTO en el locus reparado: el `decimals.map` del ciclo vuelve a
construirse con la tabla canonica hardcodeada (`canonical_token_decimals_str`),
cuyo arm desconocido es **18** — exactamente el comportamiento pre-fix que t187
midio como 8/37 entradas erroneas.

El test focalizado de los 8 casos TIENE que fallar con este mutante aplicado y
pasar sin el. Mismo numero de tests en ambos lados (el mutante no anade ni
quita tests: cambia una implementacion).

Uso:  python3 mutate-decimals-cycle-01.py <RAIZ_BACKEND>
"""
import pathlib
import sys

ORCH = "searcher-rs/src/orchestrator.rs"

OLD = """    let mut map = HashMap::new();
    let mut unresolved: Vec<String> = Vec::new();
    for addr in token_addresses {
        let lc = addr.to_lowercase();
        match redis_decimals.get(&lc).or_else(|| pg_decimals.get(&lc)) {
            Some(d) => {
                map.insert(lc, *d);
            }
            None => {
                if !unresolved.contains(&lc) {
                    unresolved.push(lc);
                }
            }
        }
    }
"""

NEW = """    let mut map = HashMap::new();
    let unresolved: Vec<String> = Vec::new();
    let _ = (redis_decimals, pg_decimals);
    for addr in token_addresses {
        let lc = addr.to_lowercase();
        // ── MUTANTE (falsificador t197): comportamiento PRE-FIX ──
        // La tabla canonica hardcodeada con su default de 18. Si los tests
        // siguen pasando con esto, son vacuosos.
        map.insert(
            lc,
            crate::engines::dex_engine::canonical_token_decimals_str(addr),
        );
    }
"""


def main() -> None:
    root = pathlib.Path(sys.argv[1]).resolve()
    p = root / ORCH
    raw = p.read_text(encoding="utf-8")
    if "MUTANTE (falsificador t197)" in raw:
        print("MUTANTE_YA_APLICADO")
        return
    n = raw.count(OLD)
    if n != 1:
        raise SystemExit(f"ABORT: el cuerpo del resolutor aparece {n} veces (se esperaba 1)")
    p.write_text(raw.replace(OLD, NEW, 1), encoding="utf-8")
    print("MUTANTE_APLICADO (default 18 restaurado en build_cycle_decimals_map)")


if __name__ == "__main__":
    main()
