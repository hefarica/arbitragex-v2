# P4 — Gates completos (iteración 3) — 2026-10-18

- **Base:** rama `REAINTEGRATION-P3-D04-D07-01` @ `4f2c88a3` (el commit que pasó `ci-gate` y `Verifier policy tests`).
- **Ejecutor:** Super Agent (sandbox con `cargo` 1.85.0/1.91.0 + `forge` 1.8.5 + `node` v22.23.3; `docker` **ausente**).
- **Alcance:** los gates obligatorios de P4. Lo que no se pudo correr va declarado como deuda, nunca simulado (R8).
- **Disciplina de disco:** builds con `CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0` (los binarios de test bajan de ~260-380 MB a una fracción; `target` se quedó en ~2.3 GB en vez de ~13 GB). `backend/target` se limpia al cerrar.

## 1. Resultado por gate

| Gate | Resultado | Evidencia |
|---|---|---|
| `cargo fmt --check` (workspace) | **PASS** | 0 diffs |
| `cargo clippy --workspace --all-targets -- -D warnings` | **PASS tras reparación** (era **FAIL**) | 0 errores |
| `cargo test --workspace` | **FAIL 1 / 4440** | 4416 passed · **1 failed** · 23 ignored · 28 suites |
| `tsc --noEmit` frontend | **PASS** | exit 0 |
| `tsc --noEmit` api-server | **PASS** | exit 0 (requiere `shared-ts` construido) |
| `tsc --noEmit` selector-api | **PASS** | exit 0 (requiere `shared-ts` construido) |
| `vitest run` frontend | **PASS** | 155 archivos · 1639 tests |
| `vitest run` api-server | **PASS** | 77 archivos · 1020 tests |
| `vitest run` selector-api | **PASS** | 9 archivos · 85 tests |
| `vitest run` shared-ts | **PASS** | 51 tests |
| `forge fmt --check` | **PASS** | exit 0 |
| `forge test` (incluye invariantes) | **PASS** | 240 tests · 0 failed · 9 skipped |
| `forge invariant` | **NO EXISTE el subcomando** en forge 1.8.5 | los invariantes corren dentro de `forge test` (ver §3) |
| `gitleaks` | **FAIL** | 345 hallazgos, todos `generic-api-key` |
| `npm audit --omit=dev` | **FAIL** | 38 vulnerabilidades (1 critical, 7 high, 30 moderate) |
| determinismo (replay 2× → mismo hash) | **PARCIAL** | omega-search 2× → mismo hash de resultado (§5) |

## 2. Reparación de `clippy` (única cambio de código de esta iteración)

`cargo clippy --workspace --all-targets -- -D warnings` estaba **ROJO** en `4f2c88a3` con una cascada que sólo se ve al ir arreglando (clippy aborta en el primer crate que falla). Barrido por crate: **11 errores en 3 crates**, todos en código de **test**:

| Crate | Errores | Lint | Archivo |
|---|---|---|---|
| `sim-ctl` | 3 | `neg_cmp_op_on_partial_ord` ×1, `assertions_on_constants` ×2 | `src/consumer.rs:1536,1691,1696` |
| `recon` | 2 | `bool_assert_comparison` | `src/drift_tracker.rs:1018,1019` |
| `sed-core` | 6 | `expect_used`/`expect_err_used` | `src/types/{errors,infrastructure,kill_switch}.rs` |

Reparaciones (todas preservan semántica):
- `sim-ctl` — `!(x > 0.0)` → `x.partial_cmp(&0.0) != Some(Ordering::Greater)` (**equivalente exacto para todo valor, incluido NaN**, donde `x <= 0.0` sería falso). Los dos `assert!` sobre constantes → `const _: () = assert!(...)` (invariante de compilación, no de runtime).
- `recon` — `assert_eq!(expr, true/false)` → `assert!(expr)` / `assert!(!expr)`.
- `sed-core` — `#[allow(clippy::unwrap_used, clippy::expect_used)]` **sólo en el módulo `#[cfg(test)]`** (la doctrina prohíbe unwrap/expect en **producción**; el lint del workspace los eleva a error con `-D warnings`).

Resultado: `cargo clippy --workspace --all-targets -D warnings` → **exit 0**.

## 3. Hallazgo: `forge invariant` no es un subcomando

`forge invariant` devuelve el usage (exit 2) en forge 1.8.5. Los tests de invariante (`ExecutorInvariant.t.sol`, `FlashLoanInvariant.t.sol`) **sí** corren y **pasan** dentro de `forge test`. La línea del gate debe ser `forge test` (que los incluye) o `forge test --match-contract Invariant`; `forge invariant` es un comando obsoleto.

## 4. Hallazgo: el fixture de bytecode de `sim-core` no coincide con `contracts/out`

`cargo test --workspace` falla en **exactamente 1** test:

```
sim-core::paper_executors::tests::fixtures_match_forge_output_when_present
panicked at sim-core/src/paper_executors.rs:549
fixture ArbitrageExecutor drifted from contracts/out — regenerate via scripts/gen_executor_fixtures.sh
```

Caracterización exacta (no es una divergencia de lógica):

- Los dos blobs miden **19152 chars hex (9576 bytes)** y son **idénticos hasta el char 19066 (byte 9532)**.
- Lo único distinto es el **hash de metadatos solc** del final (CBOR `a264697066735822…64736f6c63430008180033`): `…122039b0c7ae…` (fixture) vs `…12207d06463f…` (recién compilado).
- El test **se auto-omite** cuando `contracts/out` no existe (por eso CI está verde: la imagen CI no tiene forge). Al ejecutar `forge build` localmente, el guard se activa y detecta que el fixture commiteado no corresponde al hash de metadatos del build actual.

**No se regeneró el fixture**: regenerar un artefacto commiteado que consume `sim-core` es un cambio de producto (y si el hash de metadatos depende del toolchain, commitearlo ataría el repo a esa versión). Queda como **decisión de Hector/equipo**: (a) regenerar el fixture, (b) hacer el guard insensible al hash de metadatos (comparar sólo el cuerpo del bytecode), o (c) documentar que el guard es local-only.

## 5. Determinismo (replay 2×)

No existe en el repo un harness de *replay con hash*. Lo más cercano y verificable: la suite de `omega-search` (crate standalone, ADR-003) ejecutada **dos veces** produce el **mismo hash de resultado**:

```
run1: de9309d320c863e2006c9c25a854981f
run2: de9309d320c863e2006c9c25a854981f
(48 tests: 16 + 32, 0 failed)
```

Se declara **PARCIAL**: prueba reproducibilidad de esa suite, no el gate completo "replay 2× → mismo hash" sobre un fixture del sistema.

## 6. gitleaks y npm-audit (detalle)

- **gitleaks 8.28.0** (`gitleaks detect --config .gitleaks.toml`, 1852 commits, ~844 MB): **345 hallazgos, todos `generic-api-key`**, en **36 archivos**. Concentración: `repo-vps-audits/history.jsonl` (192), `repo-vps-audits/SCAN-*/nodes.csv` (6 c/u), `docs/auditoria/OMEGA_README_CONTRACTS.md` (6). Es un hallazgo de **triaje**, no un veredicto de secreto real: son artefactos de auditoría, pero el gate tal como está especificado está ROJO.
- **npm audit --omit=dev**: 1 **critical** (`next`), 7 **high** (`axios`, `braces`, `fast-uri`, `http-proxy-middleware`, `micromatch`, `nanoid`, `ws`), 30 moderate. Gate ROJO.

## 7. Deuda declarada (no ejecutado)

- **P5 (E2E 3 planos):** **`docker` NO está disponible** en este sandbox (`docker`/`docker-compose` ausentes) ⇒ **P5 plano 1 (repo vivo: docker compose con todo en running + `/health`) queda como deuda explícita, no simulado**. Los planos 2 (dominio vivo) y 3 (Chromium 1440/1280/1024/390) tampoco se intentaron.
- **P6/P7/P8:** no alcanzados.
- **`npm ci`:** requiere caché escribible (`npm ci --cache /tmp/npmcache`); la caché por defecto (`/opt/npm-cache`) es de root y da `EACCES`.
- **Invariantes de runtime** ("relays-client único firmante", "capital $0/broadcast=false", "kill-switch <10ms"): siguen sin verificar; exigen E2E.
- **`forge invariant`:** el gate debe reescribirse (§3).

## 8. Reproducir

```bash
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0
(cd shared-ts && npm run build)                      # @arbx/shared, prerequisito de los tsc
(cd backend && cargo fmt --check)
(cd backend && cargo clippy --workspace --all-targets -- -D warnings)
(cd backend && cargo test --workspace)
(cd frontend && npx tsc --noEmit && npx vitest run)
(cd backend/api-server && npx tsc --noEmit && npx vitest run)
(cd backend/selector-api && npx tsc --noEmit && npx vitest run)
(cd contracts && forge fmt --check && forge test)
gitleaks detect --source . --config .gitleaks.toml --redact
npm audit --omit=dev
rm -rf backend/target                                 # limpieza de disco
```
