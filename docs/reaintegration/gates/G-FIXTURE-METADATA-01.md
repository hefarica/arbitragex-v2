# G-FIXTURE-METADATA-01 — fixture vs `contracts/out`, metadata excluded (ADR-004)

- **Tipo:** gate de paridad sim/live (bytecode de los ejecutores que `sim-core` despliega en el fork en proceso).
- **Script:** `scripts/gates/g-fixture-metadata-01.sh` (exit 0 = PASS, 1 = FAIL, 2 = SKIP sin toolchain).
- **Implementación:** `split_solc_metadata` + test `fixtures_match_forge_output_when_present` en `backend/sim-core/src/paper_executors.rs`, y test `solc_metadata_split_is_explicit_and_fail_closed`.

## Decisión (ADR-004, 2026-10-18)

El guard compara el **bytecode ejecutable** ignorando **sólo** la sección CBOR de metadatos de solc que va al final (dos últimos bytes = longitud big-endian). Esa sección lleva la versión del compilador y el hash ipfs/keccak de los metadatos: **artefacto de build, no semántica**.

Nada es silencioso (R8):
- `split_solc_metadata` **sólo** recorta una sección final bien formada. Si `len < 2`, o la longitud declarada no cabe, **no recorta nada** y la comparación es byte a byte sobre el blob completo.
- El test afirma que **todo lo anterior a la sección es byte-idéntico**.
- El mensaje de fallo **nombra los rangos ignorados** (`DECLARED EXCLUSION — … fixture N bytes at [a, b), contracts/out M bytes at [c, d)`).
- Invariante comprobada: el split no pierde bytes.

## Resultado real (reproducible)

`contracts/out` reconstruido en limpio (`rm -rf out cache && forge build`):

| Contrato | código ejecutable (fixture vs out) | metadatos | veredicto |
|---|---|---|---|
| `ArbitrageExecutor` | **9522 == 9522 bytes, idénticos** | 53 vs 53 bytes, **distintos** | **PASS** — es exactamente el caso que la política cubre |
| `FlashLoanExecutor` | **7982 vs 8534 bytes** (primer byte distinto: offset 9) | 53 vs 53 | **FAIL — deriva REAL** |

`./scripts/gates/g-fixture-metadata-01.sh` → **FAIL**, y el fallo no viene del hash de metadatos: viene de que el fixture commiteado de `FlashLoanExecutor` **no es el contrato que compila hoy `contracts/src`** (le faltan ~552 bytes de código ejecutable respecto al build actual).

Detalle del primer byte divergente:
```
fixture: 30 60 80 52 34 80 15 61 00 13 57 5f …      (0x348015… = CALLVALUE;REVERT)
out:     30 60 80 52 60 01 60 04 55 34 80 15 …      (0x6001600455… = PUSH1 01;PUSH1 04;SSTORE)
```

## Por qué esto importa

`sim-core` despliega `FLASHLOAN_EXECUTOR_BYTECODE_HEX` dentro del fork en proceso para ejecutar el camino real de flash-loan en paper. Si el fixture es viejo, **el simulador no está ejecutando el contrato que está en `contracts/src`** — es una ruptura de paridad sim/live, no un detalle de build.

## Decisión pendiente (no aplicada)

No se regeneró el fixture: `scripts/gen_executor_fixtures.sh` lo actualizaría, pero eso cambia un artefacto commiteado que consume el camino de simulación y **debe decidirlo el propietario**. Opciones:
1. **Regenerar** (`scripts/gen_executor_fixtures.sh`) si `contracts/src/FlashLoanExecutor.sol` es la fuente de verdad → cierra la deriva.
2. Si el fixture es la referencia buena, el que está desactualizado es el `.sol` y hay que revisar qué se desplegó.

## Cobertura en CI (declarado)

La imagen de CI **no trae `forge`**, así que `contracts/out` no existe y el test **se auto-omite**: **CI no ejercita este gate hoy**. Se ejercita con `scripts/gates/g-fixture-metadata-01.sh` en cualquier máquina que sí construya los contratos. Queda declarado en `RESULTS-P4.json` y en el ledger de la iteración.

## Revert

Un paso: revertir el commit restaura la comparación byte a byte completa (que fallaba en `ArbitrageExecutor` por el hash de metadatos). No se tocó ningún artefacto de build.
