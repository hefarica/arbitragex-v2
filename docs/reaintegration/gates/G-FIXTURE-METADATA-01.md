# G-FIXTURE-METADATA-01 — fixture vs `contracts/out`, metadata excluded (ADR-004)

- **Tipo:** gate de paridad sim/live (bytecode de los ejecutores que `sim-core` despliega en el fork en proceso).
- **Script:** `scripts/gates/g-fixture-metadata-01.sh` (exit 0 = PASS, 1 = FAIL, 2 = SKIP sin toolchain).
- **Implementación:** `split_solc_metadata` + test `fixtures_match_forge_output_when_present` en `backend/sim-core/src/paper_executors.rs`, y test `solc_metadata_split_is_explicit_and_fail_closed`.
- **Base de esta entrega (PR-D2):** `origin/main` = `d3087b84` (rama local `rea-pr-d2`).

## Decisión (ADR-004, 2026-10-18)

El guard compara el **bytecode ejecutable** ignorando **sólo** la sección CBOR de metadatos de solc que va al final (dos últimos bytes = longitud big-endian). Esa sección lleva la versión del compilador y el hash ipfs/keccak de los metadatos: **artefacto de build, no semántica**.

Nada es silencioso (R8):
- `split_solc_metadata` **sólo** recorta una sección final bien formada. Si `len < 2`, o la longitud declarada no cabe — o consumiría el blob entero (`declared + 2 == code.len()`) — **no recorta nada** y la comparación es byte a byte sobre el blob completo.
- El test afirma que **todo lo anterior a la sección es byte-idéntico**.
- El mensaje de fallo **nombra los rangos ignorados** (`DECLARED EXCLUSION — … fixture N bytes at [a, b), contracts/out M bytes at [c, d)`).
- Invariante comprobada: el split no pierde bytes.

## Medición ANTES de regenerar (base `d3087b84`, reproducible)

`forge build` limpio de `contracts/` y split del guard sobre el fixture commiteado:

| Contrato | código ejecutable (fixture vs out) | metadatos | veredicto |
|---|---|---|---|
| `ArbitrageExecutor` | **9522 == 9522 bytes, idénticos** | 53 vs 53 bytes, distintos | **PASS** — es exactamente el caso que la política cubre |
| `FlashLoanExecutor` | **7982 vs 8534 bytes** (primer byte distinto: offset 9) | 53 vs 53 | **FAIL — deriva REAL** |
| `ERC1967Proxy` (PR-D1) | **915 == 915 bytes, idénticos** | 53 vs 53 | **PASS** |

`test fixtures_match_forge_output_when_present ... FAILED` sobre `FlashLoanExecutor` con el mensaje `EXECUTABLE code drifted` (la exclusión de metadatos NO lo oculta). El fixture commiteado de `FlashLoanExecutor` **no era el contrato que compila hoy `contracts/src`** (le faltaban 552 bytes de código ejecutable).

Detalle del primer byte divergente (offset 9):
```
fixture: 30 60 80 52 34 80 15 61 00 13 57 5f …      (0x348015… = CALLVALUE;REVERT)
out:     30 60 80 52 60 01 60 04 55 34 80 15 …      (0x6001600455… = PUSH1 01;PUSH1 04;SSTORE)
```

## Por qué esto importa

`sim-core` despliega `FLASHLOAN_EXECUTOR_BYTECODE_HEX` dentro del fork en proceso para ejecutar el camino real de flash-loan en paper. Si el fixture es viejo, **el simulador no está ejecutando el contrato que está en `contracts/src`** — es una ruptura de paridad sim/live, no un detalle de build.

## Decisión aplicada (PR-D2, 2026-10-18)

Regenerado con `scripts/gen_executor_fixtures.sh`. **`contracts/src` es la fuente de verdad**: no hay despliegue on-chain que consultar (no existe `contracts/broadcast/`, ni `contracts/deployments/`, ni constante `EXECUTOR_ADDRESS`; `contracts/DEPLOY.md` es un runbook con placeholders).

sha256 antes/después (ver `docs/reaintegration/evidence/PR-D2-fixture-regen.json`):

| fixture | sha256 antes | sha256 después | bytes | ejecutable |
|---|---|---|---|---|
| `ArbitrageExecutor.bytecode.hex` | `86b0a5a1…5155e0c` | `02472306…ccf268d` | 9575 → 9575 | 9522 == 9522 (sólo metadatos) |
| `FlashLoanExecutor.bytecode.hex` | `bd10d1a8…786438f` | `1d91469e…f70985c` | 8035 → 8587 | 7982 → 8534 (deriva REAL: +552) |
| `ERC1967Proxy.bytecode.hex` | `a9245994…e9fc513f4` | `5b634aec…95050ddb` | 968 → 968 | 915 == 915 (sólo metadatos) |

Tras la regeneración:
- `./scripts/gates/g-fixture-metadata-01.sh` → **PASS (exit 0)**.
- `cargo test -p sim-core --lib` → **85 passed / 0 failed / 3 ignored**.
- `cargo test -p sim-ctl --lib` → **13 passed / 0 failed**.
- `G-PAPER-STACK-01` sigue **PASS** tras regenerar (el fixture del FLE cambia el bytecode desplegado ⇒ se re-verifica por ejecución).

### R-1 — bug latente del splitter (cerrado)

El caso `declared + 2 == code.len()` devolvía `executable = []` con todo el blob como metadatos; un guard que partiese **ambos** lados podía comparar dos slices vacíos y **PASAR trivialmente**. Corregido: `declared + 2 >= code.len()` no es un split válido → se devuelve el blob completo como ejecutable (comparación estricta). Además el guard afirma `!executable.is_empty()` en los dos lados (fixture y `contracts/out`), y `solc_metadata_split_is_explicit_and_fail_closed` fija el blob de 2 bytes `0x0000` y el caso `declared + 2 == code.len()`.

### R-2 — cobertura en CI (cerrada)

- **Primario:** job nuevo `fixture-metadata-gate` en `.github/workflows/ci.yml` (checkout con `submodules: recursive` + `dtolnay/rust-toolchain` 1.91.0 + `Swatinem/rust-cache` + `foundry-rs/foundry-toolchain`) que ejecuta este script end-to-end. Añadido al conjunto requerido de `ci-gate`: un gate rojo **bloquea** el PR.
- **Fallback declarado:** `MANUAL-ONLY` con **caducidad 2026-11-18** (un gate manual sin fecha nunca corre) si el job se retira.
- **Bug del script del gate (corregido):** invocaba `cargo` desde la raíz del repo, que **no tiene `Cargo.toml`** (sólo `backend/Cargo.toml`) → moría con *could not find `Cargo.toml`* antes de llegar al test. Ahora usa `--manifest-path backend/Cargo.toml`. También: `set -euo pipefail`, `forge build` ya no se silencia (log completo en el FAIL), y el `rc` de cargo se captura explícitamente para clasificar PASS/FAIL.

## Revert

Un paso: revertir el commit de PR-D2 (fixtures + R-1/R-2) restaura los fixtures pre-regeneración y el splitter previo a R-1. Fuera del `sim-core` no hay consumidor del artefacto.
