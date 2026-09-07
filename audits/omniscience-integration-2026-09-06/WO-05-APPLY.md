# WO-05 — APPLY: eliminación del código fantasma + wiring de NonceManager::refresh

- **Work-order:** WO-05 (Oleada 4 — apply) · Opción C de `WO-05-DESIGN.md`.
- **Fecha:** 2026-09-06 · agente rust-topology-engineer (rubric: ecc:rust-patterns + arbx-pre-edit-audit).
- **Árbol:** rama `fix/wo15-xinfo-shape` (árbol principal compartido, `target/` caliente §36.4 — NO-GIT: cero commit/push/staging).
- **Estado:** **APPLIED** — gates 4/4 verde. Runtime del resync: compile/lint/test verdes; camino runtime sin ejercicio anvil-fork (declarado, R8 — igual que el diseño §3.3).

---

## 1. Qué se aplicó

### (a) ELIMINACIÓN de `backend/relays-client/src/executor/` (−350 líneas)

Eliminación por **SISTEMA DE ARCHIVOS** (`rm -rf backend/relays-client/src/executor`) — NO `git rm`, NO staging. Los 4 archivos quedan como deletions **unstaged** visibles para el PR futuro del operador:

| Archivo | Líneas | Evidencia de muerte (re-verificada en esta sesión) |
|---|---|---|
| `executor/mod.rs` | 200 | `LiveTestnetExecutor` + máquina de 16 estados sin run-loop; imports muertos |
| `executor/nonce_manager.rs` | 64 | `Arc<Provider<Http>>` crudo — bypasea `HttpRpcPool` (G-RPC-1) |
| `executor/gas_oracle.rs` | 44 | importa `ethers_core`/`ethers_providers` como crates directas (no declaradas en workspace) |
| `executor/idempotency.rs` | 42 | `IdempotencyChecker` con firma que mod.rs llama mal (2 bloqueantes del diseño §1.3) |

Pre-edit-audit re-verificado ANTES del rm:
- `grep -rn "mod executor" backend/relays-client/` → **NONE** (la declaración jamás existió).
- `git log --all -S "mod executor;" -- main.rs` → vacío (0 commits).
- `grep -rn --include="*.rs" -E "executor::|LiveTestnetExecutor|check_or_insert|src/executor" backend/` → solo falsos positivos `round_trip_executor` de `prioritization-spine` (entidad distinta) → **CERO referencias reales**.

CERO edits adicionales requeridos: main.rs no lo declara (nada que quitar), ningún otro archivo lo referencia.

### (b) INTEGRACIÓN del wiring de `NonceManager::refresh` (fuga de nonce huérfano, ruta LIVE, mode-invariant)

**Diff 2 — `backend/relays-client/src/nonce_manager.rs`**
- Eliminada la línea `#[allow(dead_code)]` (era L55, sobre `pub async fn refresh`). El allow muere al existir call-site. Única modificación del archivo.

**Diff 3 — `backend/relays-client/src/submit_engine.rs`**

A) Helper privado `resync_nonce` en `impl SubmitEngine` (junto a `dropped`/`not_submitted`), firma `async fn resync_nonce(&self, chain_id: u64, addr: ethers::types::Address, cause: &str)` — submit_engine.rs:926-954. Fail-soft R8: `Ok` → `info!(event="nonce.resynced")`; `Err` → `warn!(event="nonce.resync_failed")` y el contador queda como estaba hasta el próximo evento de desync. Nunca fabrica un nonce — relee `eth_getTransactionCount` (pending) vía `pool.with_retry` (circuit breaker + failover EWMA intactos). Ruta por `self.nonce.as_ref()` (Option — nunca panic).

B) Los 5 call-sites (anclados por evento/branch; líneas post-apply):

| # | Causa | Ancla | Línea |
|---|---|---|---|
| 1 | `callbundle_abort` | `CallBundleDecision::Abort` (BE-05 fail-closed), primera sentencia del arm | submit_engine.rs:656-658 |
| 2 | `all_relays_failed` | rama `if !broadcast_result.any_success()`, primera sentencia | submit_engine.rs:689-691 |
| 3 | `inclusion_timeout` | `InclusionOutcome::Dropped` — arm convertido de expresión a bloque, resync primera sentencia | submit_engine.rs:818-820 |
| 4 | `build_error_post_nonce` | rama `Err(e)` genérica de `build_and_sign` (la del `not_submitted` con `build_error`), antes del return | submit_engine.rs:468-470 |
| 5 | `paper_short_circuit` | primera sentencia del bloque `if paper` (paso 5) | submit_engine.rs:512-514 |

Cada site lleva el marcador `// WO-05 (2026-09-06)` en línea propia encima de la llamada; el helper lleva el suyo (submit_engine.rs:926). Semántica idéntica al diseño §5 Diff 3: NO requieren resync `Included`/`Reverted` (el nonce SÍ aterrizó) ni los `not_submitted` previos a `build_and_sign` (nonce no consumido; nota: los BuildError pre-nonce que caen en el arm genérico también disparan el resync — inocuo, re-fetch idempotente, preferible a discriminar variantes §3).

---

## 2. Gates (comandos EXACTOS desde `backend/`)

| Gate | Comando | Resultado |
|---|---|---|
| Compile | `cargo check -p relays-client` | **PASS** — `Finished dev profile in 50.04s` |
| Lint | `cargo clippy -p relays-client -- -D warnings` | **PASS** — 0 warnings, 25.85s |
| Format (scoped) | `cargo fmt -p relays-client -- --check` | **PASS** — exit 0, sin diff |
| Test | `cargo test -p relays-client` | **PASS** — **77 passed; 0 failed; 1 ignored** (0.54s) |
| Format (workspace, informativo) | `cargo fmt --check` | **PASS** — exit 0 (sin Diff en ningún crate) |

- **Fallback AppControl NO necesario**: los binarios de test (dev profile) ejecutaron sin bloqueo (os error 4551 no observado). Tests ejercen los classifiers puros (`be05_*`, `a7_*`, `r0001_*`, `c1_*`, `h2_*`, `pkg6_*`) — verdes 77/77.
- Tests existentes NO cubren el camino runtime de `resync_nonce` (requiere seam RPC o anvil-fork — fuera de charter, declarado en diseño §5 "Qué NO se hace"). **APPLIED con verificación compile+lint+test; runtime del resync queda para el follow-up anvil-fork** (honesto R8).

## 3. Verificación post-apply (R8 — fail-honest)

| Afirmación | Comando | Resultado |
|---|---|---|
| `mod executor` sigue sin existir | `grep -rn "mod executor" backend/relays-client/src/` | **NONE** |
| Cero referencias de ruta al fantasma | `grep -rn --include="*.rs" "src/executor" backend/` | **ZERO** referencias |
| Directorio eliminado físicamente | `ls backend/relays-client/src/ \| grep -c executor` | 0 |
| Deletions visibles SIN staging | `git status --porcelain -- backend/relays-client/` | ` D` ×4 (executor/), ` M` nonce_manager.rs, ` M` submit_engine.rs — **nada staged** (`git diff --cached` vacío) |
| Diff total del árbol (solo lectura) | `git diff --stat -- backend/relays-client/` | 7 archivos, +68/−365 (incluye drift WO-04 preexistente, ver §5) |
| refresh: definición + call-site | `grep -n "\.refresh(" backend/relays-client/src/` | 1 uso real: submit_engine.rs:936 (dentro de `resync_nonce`, llamado desde los 5 sites) |
| `allow(dead_code)` eliminado | `grep -n "allow(dead_code)" nonce_manager.rs submit_engine.rs` | 0 apariciones |
| Marcadores propios | `grep -n "WO-05 (2026-09-06)" submit_engine.rs` | 6 (5 sites + helper) — solo en líneas WO-05 |

## 4. IntoCABLES respetados (§34.3)

- `live_exec_policy.rs`: **NO tocado** (no aparece en `git status` de relays-client).
- default-deny / `MainnetRefused` / `ARBX_LIVE_EXEC_ENABLED`: **intactos** — el diff no los referencia.
- Gates arbx-* (checklist 12 checks, kill-switch, ValidatedPlan fail-closed TTL 300s, value cap, callBundle fail-closed, EWMA relay-fee): **intactos** — `resync_nonce` es posterior a cada decisión y no sustituye ninguna: solo corrige el cache de nonce hacia la verdad on-chain. Este WO **REDUCE** superficie del terminus (segunda ruta de capital fantasma eliminada), no la habilita.
- Zero mocks / zero hardcodes (RULE 00): el helper no fabrica datos; `cause` es string de telemetría, el nonce proviene de `eth_getTransactionCount` real vía pool.

## 5. Declaraciones honestas

1. **Drift coexistente WO-04**: el working tree de relays-client ya traía `M bundle_builder.rs` (+11/−2) y 1 línea WO-04 en submit_engine.rs (`self.cfg.execution.priority_fee_gwei, // WO-04 (2026-09-06)`, ahora L446) ANTES de mi primer edit — verificado por `git diff` pre-edit. NO lo toqué; queda para el PR de WO-04. El diff stat de submit_engine.rs mezcla ambas WO (para el operador: la línea WO-04 es exactamente 1; el resto es WO-05).
2. **Rama compartida**: el árbol está en `fix/wo15-xinfo-shape` (no `a6-cbprom-01` del snapshot inicial — otro agente del gang conmutó la rama compartida, §36). Bajo NO-GIT esto no afecta las deletions/ediciones (quedan en el working tree), pero el PR futuro del operador debe partir de `git branch --show-current` verificado en ese momento.
3. **Runtime del resync unverified**: sin anvil-fork no se ejercita el camino real de desync (diseño §3.3 riesgo residual declarado). El warn `nonce.resync_failed` es la señal de observabilidad si el re-fetch falla en runtime.
4. **Parked (diseño §5 "Qué NO se hace")**: trait seam de `HttpRpcPool` para testear refresh; mover `nonce_mgr.next()` tras `estimate_gas` en bundle_builder; relay_eden/relay_beaver; idempotencia NX. Ninguno se hizo.

## 6. Evidencia de comando (resumen ejecutable)

```
cd backend
cargo check -p relays-client        # PASS 50.04s
cargo clippy -p relays-client -- -D warnings   # PASS 0 warnings
cargo fmt -p relays-client -- --check          # PASS
cargo test -p relays-client         # PASS 77/0/1
grep -rn "mod executor" ../backend/relays-client/src/   # NONE
git status --porcelain -- ../backend/relays-client/     #  D x4 (unstaged) + M x2
```

**Status final: APPLIED — diseño Opción C ejecutado completo, gates 4/4, INTOCABLES intactos, árbol listo para el PR del operador (un PR = un ID: WO-05 §2.5).**
