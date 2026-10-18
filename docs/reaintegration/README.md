# REA — Reinvención Estructural Aditiva sobre `arbitragex-v2`

Iteración **2026-10-18** · base auditada `fe62cdceb6dc0b18afed8db03b7dc7ab64e38d6b` (main, 14-oct-2026) · read-only sobre 16 fuentes.

## Contenido

| Ruta | Qué es |
|---|---|
| `00-INVENTORY.json` | Inventario P0 máquina-legible + los 7 hallazgos de drift D-01…D-07 + estado de toolchain |
| `rea/NN-<repo>.md` | 16 fichas REA (12 secciones cada una): propósito/madurez, arquitectura, algoritmos, esquemas, resiliencia, tests, licencia, qué se extrae/descarta, destino exacto, ADR, gates, riesgo+revert |
| `adr/ADR-001-curve-duplicate.md` | D-03 — resolución del Curve duplicado (canónico = `dexes/CurveAdapter.sol`) |
| `adr/ADR-002-admin-root-404.md` | D-02 — `/admin` raíz permanece 404 (sin consumidor) |
| `gates/G-BACKUP-01.md` | Gate nuevo de exclusión backup/recovery (cierra D-01) |
| `gates/G-ADMIN-ROOT-01.md` | Gate nuevo de consistencia `/admin` (cierra D-02) |
| `ledger/LEDGER_SHA256.json` | Ledger SHA256 de cada artefacto de esta iteración |
| `MANIFEST_SHA256.json` | Manifiesto de la iteración (ledger + gates + fichas) |
| `FINAL-CLOSURE.md` | Qué se cerró, qué queda fuera y por qué (deuda declarada) |
| `evidence/` | Salidas crudas reproducibles de los gates |

Scripts de gate (fuera de `docs/`):

- `scripts/gates/g-backup-01.sh`
- `scripts/gates/g-admin-root-01.sh`

## Regla de fusión

ADITIVA. Se extrae el **patrón**, nunca se copia-pega código. Las fuentes con licencia copyleft (`freqtrade` GPL-3.0, `btrader` GPL-3.0, `nautilus_trader` LGPL-3.0) y sin licencia (`whack-a-mole`, `DexCex_bot`, `simple-arbitrage`, `cryptofeed` NOASSERTION, `kelp` NOASSERTION) se tratan como **sólo patrón conceptual** (ver `G-LICENSE-01`/`G-LICENSE-02` en las fichas).

## Nota de alcance

Esta iteración cubre **P0 (inventario/drift)** y **P1 (REA de fuentes)**. P2–P8 (ADR por integración, implementación, tests como gates, E2E en 3 planos, veredicto por campo) **no** se ejecutaron: sin toolchain Rust/Foundry y sin `node_modules` en el sandbox de esta iteración. Detalle en `FINAL-CLOSURE.md`.
