# PR-D — fixture regenerado + R-1 / R-2 (2026-10-18)

Protocolo completo, en el orden exigido. **Todos los pasos PASS** salvo el aviso de disco.

| Paso | Resultado |
|---|---|
| (a) `scripts/gen_executor_fixtures.sh` | **PASS** — `ArbitrageExecutor: 9575 bytes`, `FlashLoanExecutor: 8587 bytes` escritos a `backend/sim-core/src/fixtures/` |
| (b) sha256 **antes/después de ambos** `.hex` al ledger | **PASS** — `docs/reaintegration/ledger/PR-D-fixture-regen-LEDGER.json` |
| (c) suite de paper del sim **completa** | **PASS** — `cargo test -p sim-core -p simulator-v2 -p sim-ctl`: **231 passed, 0 failed, 6 ignored** (14 suites). `paper_executors::tests::fixtures_match_forge_output_when_present` → **ok** (antes FAIL) |
| (d) `G-FIXTURE-METADATA-01` | **PASS** |
| (e) revert de un paso | `git revert` de este commit restaura ambos `.hex` **y** el job de CI en un solo paso |

## Por qué hacía falta regenerar (y no era un artefacto de toolchain)

El guard, ya insensible al hash de metadatos, lo dejó claro: `ArbitrageExecutor` tenía **código ejecutable idéntico** (9522 == 9522) y sólo difería el hash de metadatos; `FlashLoanExecutor` tenía **código ejecutable distinto**: 7982 vs 8534 bytes (primer byte distinto en el offset 9). El fixture commiteado **no era el contrato que compila `contracts/src`**, y `sim-core` lo despliega dentro del fork en proceso — paridad sim/live rota.

## R-1 — comparación no trivial

`declared + 2 == code.len()` produce un ejecutable **vacío** y dos slices vacíos comparan igual: PASS trivial. Añadidos `assert!(!fixture_code.is_empty())` y `assert!(!want_code.is_empty())` **antes** de la comparación.

## R-2 — subir la categoría del guard

- **Job de CI nuevo**: `.github/workflows/ci.yml::fixture-metadata-gate` (checkout con `submodules: recursive` + `dtolnay/rust-toolchain@1.91.0` + `foundry-rs/foundry-toolchain` + `bash scripts/gates/g-fixture-metadata-01.sh`). La imagen de CI admite ambas toolchains, así que se añadió.
- **Es bloqueante, no advisory**: el job entra en `needs` de `ci-gate` **y** en su `required` set (verificado: `ci-gate needs` incluye `fixture-metadata-gate` y el YAML parsea).
- **Ledger**: registrado como **MANUAL-ONLY** con **caducidad 2026-11-18** — la categoría manual aplica sólo donde no hay `forge` (el gate devuelve `SKIP=2`); en CI pasa a automático. Regla: retirar en cuanto `fixture-metadata-gate` lleve una semana en verde sobre `main`.

## Aviso de disco (declarado, no oculto)

Durante el paso (c) el disco llegó al **90 %** (2,8 GB libres) al compilar el workspace completo. Se paró el build del workspace, se liberó el `backend/target` propio (3,3 GB) y el paso (c) se completó con los tres crates del sim (`sim-core`, `simulator-v2`, `sim-ctl`), que es el alcance que pide el protocolo. No se ejecutó `cargo test --workspace` completo.
