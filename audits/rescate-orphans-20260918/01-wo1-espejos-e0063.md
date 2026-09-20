# WO-1 — Reconciliación de espejos E0063 / `leg_fees_bps` (triangular_worker.rs)

> Programa RESCATE-ORPHANS-0918 · 2026-09-18 · builder Rust
> Protocolo: no-git-until-final-gate — CERO commit/push/PR ejecutado en este WO.

## 1. Inventario verificado (git, no aserción)

| Ítem | legfix | econ-f1 |
|---|---|---|
| Worktree | `C:/Users/HFRC/Desktop/arbx-legfix-20260918` | `C:/Users/HFRC/Desktop/arbx-legs-econ-f1-20260918` |
| Branch | `feat/legfix-e0063-20260918` | `feat/leg-econ-f1-20260918` |
| HEAD | `8794ece6ce64fd9f6704ab79ecfd49f5c4228a8c` | `8794ece6ce64fd9f6704ab79ecfd49f5c4228a8c` (idéntico) |
| `git status` | 1 archivo M: `backend/searcher-rs/src/workers/triangular_worker.rs` | ídem (+ warning CRLF→LF) |
| Tamaño delta | +2 líneas, 0 borradas | +2 líneas, 0 borradas |

`8794ece6` NO es ancestro de `main` local (`git merge-base --is-ancestor` → falso): sigue siendo commit local sin push.

El propio mensaje de `8794ece6` documenta la causa raíz del huérfano:
> *BLOQUEO SERIAL (reportado): triangular_worker.rs:599 (archivo prohibido) rompe E0063 — falta 'leg_fees_bps: None' en build_triangular_route_metadata; searcher-rs lib no compila hasta que el builder serial de ese archivo lo aterrice.*

Es decir: el commit añadió el campo `leg_fees_bps: Option<Vec<u32>>` a `shared_rs::candidates::RouteMetadata` y lo pobló en 5 de 6 constructores de literal de struct dentro de searcher-rs (orchestrator.rs:1110, persistence.rs:279 + :494, flashloan_arb_worker.rs:771, liquidation_worker.rs:478). El 6.º constructor (`build_triangular_route_metadata`, triangular_worker.rs:599) estaba bajo claim serial de otro builder → quedó fuera del commit → E0063 (missing field). Ambos builders paralelos intentaron cerrar ese mismo hueco antes de morir en 503.

## 2. Diff documentado de cada espejo (vs HEAD 8794ece6)

### 2a. Espejo `legfix` (`git diff` íntegro)
```diff
@@ -612,6 +612,8 @@ fn build_triangular_route_metadata(
         leg_amounts_in: None,
         leg_amounts_out: None,
         leg_zero_for_one: None,
+        // WO-LEGS-ECON-01 f1: fee por-leg no computado en esta construcción (R8-ausente).
+        leg_fees_bps: None,
     }
 }
```
- Intención: añadir el campo faltante con valor `None` (R8: ausente = no computado; la capa triangular sólo conoce el monto final del ciclo, no fee por hop).
- Posición: al FINAL del literal, tras `leg_zero_for_one` — mismo orden que la definición del struct en `shared-rs/src/candidates.rs:181` y que los otros 5 constructores.
- Indentación: 8 espacios (correcta, rustfmt-conforme).

### 2b. Espejo `econ-f1` (`git diff` íntegro)
```diff
@@ -610,6 +610,8 @@ fn build_triangular_route_metadata(
         // is not computed at this layer, so the ledger stays honestly absent
         // (R8) until the sizing kernel surfaces per-leg amounts.
         leg_amounts_in: None,
+            // WO-LEGS-ECON-01 f1: fees por-leg R8-ausente en esta construcción (builder serial).
+            leg_fees_bps: None,
         leg_amounts_out: None,
         leg_zero_for_one: None,
     }
```
- Intención: idéntica (campo faltante → `None`, R8).
- Posición: insertado ENTRE `leg_amounts_in` y `leg_amounts_out` — rompe el orden canónico del struct y parte en dos el bloque `leg_amounts_*` que el comentario previo describe como unidad.
- Indentación: 12 espacios (incorrecta; `cargo fmt --check` en CI lo marcaría).
- Efecto secundario: el archivo quedó con CRLF (warning de git) — un `git add` lo normalizaría, pero indica que el editor del builder tocó line endings.

## 3. Dictamen

- **¿Mismo cambio en dos formas?** SÍ. Semánticamente son el MISMO cambio: `leg_fees_bps: None` en el único constructor que faltaba. El compilador produce el mismo struct en ambos casos (orden de campos en literal es irrelevante para la semántica).
- **¿Divergencia real?** NO. No hay contradicción de valor ni de lógica → no aplica worst-wins R8 (ambos son el valor honesto `None`; ninguno fabrica fees).
- **¿Uno superior?** SÍ: **legfix** gana por (a) orden canónico consistente con struct y con los otros 5 constructores, (b) indentación rustfmt-correcta, (c) sin contaminación CRLF, (d) no fragmenta el bloque comentado `leg_amounts_*`.
- Verificación de completitud: `grep "RouteMetadata {"` en searcher-rs → 6 sitios de literal; los 6 tienen `leg_fees_bps` tras aplicar el delta legfix. `RouteMetadata::empty()` y tests de shared-rs ya lo tenían en 8794ece6. sim-ctl (`route_lookup.rs`) también cubierto en el commit.

## 4. Qué se aplicó

- Árbol final: worktree **`arbx-legfix-20260918`** con su propio delta (ya estaba en disco al iniciar el WO; re-verificado tras 2 interrupciones — no se re-hizo, se re-verificó).
- Ninguna edición adicional fue necesaria: el delta legfix ES la reconciliación.
- Worktree `arbx-legs-econ-f1-20260918`: **congelado, NO tocado**. Su delta queda documentado en §2b y es descartable (subsumido por legfix). Recomendación al operador: `git checkout -- backend/searcher-rs/src/workers/triangular_worker.rs` en ese worktree + `git worktree remove` cuando WO-1 aterrice; la branch `feat/leg-econ-f1-20260918` apunta al mismo commit y puede borrarse tras el push de legfix.

## 5. Verificación

| Paso | Resultado |
|---|---|
| `cargo check -p searcher-rs` con target/ frío del worktree | ❌ **os error 4551** (Windows AppControl bloquea `build-script-build` de `quote v1.0.47`) — limitación §36.4 esperada |
| `cargo check -p searcher-rs` con `CARGO_TARGET_DIR` = target/ caliente del árbol principal | ✅ **Finished, 0 errores, 0 warnings** |
| Recheck forzado (`touch` shared-rs/candidates.rs + triangular_worker.rs) | ✅ `Checking shared-rs → prioritization-spine → sim-core → searcher-rs … Finished 11.04s` — confirma que se compiló el código reconciliado, no caché stale |
| `cargo test -p searcher-rs` (target/ caliente compartido) | ❌ **NO CONCLUYENTE**: 210 errores E0463 "can't find crate for shared_rs / searcher_rs / ethers / sqlx / wiremock…" + "import resolution is stuck" en derive serde. Diagnóstico: los rlibs de test-profile no existen en el target/ compartido para las rutas de este worktree (los del árbol principal fueron compilados desde otro path/fingerprint). NO es atribuible al delta (2 líneas, `None`). El test-profile frío en el propio worktree cae de nuevo en 4551. |

**Estado honesto (R8):** compilación de la lib VERIFICADA verde; suite de tests de searcher-rs **no verificada localmente** (≠ "rota"). Los tests de `persistence.rs` que ejercitan `leg_fees_bps` (:376, :415, :425, :431) y shared-rs 234/234 fueron reportados verdes por el commit 8794ece6 y no se tocan aquí. El gate de tests debe cerrarse en CI al abrir el PR (venue Linux sin AppControl).

## 6. Paquete de PR (preparado, NO ejecutado)

**Branch a pushear:** `feat/legfix-e0063-20260918` (tras commitear el delta de 2 líneas encima de `8794ece6`).
**Base:** `main`.

**Mensaje de commit propuesto (para el delta pendiente):**
```
fix(legs): leg_fees_bps: None en build_triangular_route_metadata — cierra E0063 (WO-LEGS-ECON-01 f1)

Cierra el bloqueo serial reportado en 8794ece6: el 6.º constructor de
RouteMetadata (triangular_worker.rs) no incluía el campo nuevo y searcher-rs
no compilaba. Valor None es honesto (R8): la capa triangular no computa fee
por hop en esta construcción.

Reconciliación RESCATE-ORPHANS-0918/WO-1: dos builders paralelos produjeron
el mismo cambio; se conserva la forma con orden canónico de campos.
cargo check -p searcher-rs verde (target caliente). Tests → CI.

Co-Authored-By: Claude Code <noreply@anthropic.com>
```

**Título PR:** `feat(legs): leg_fees_bps en RouteMetadata — fee por-leg persiste (WO-LEGS-ECON-01 f1)`

**Resumen del diff del PR (2 commits: 8794ece6 + fix E0063):**
- `backend/shared-rs/src/candidates.rs` (+84): campo `leg_fees_bps: Option<Vec<u32>>` serde default/skip; `attach_leg_fees` all-or-nothing con guard de longitud; tests.
- `backend/searcher-rs/src/persistence.rs` (+46): `build_route_metadata_from_plan` popula desde `RouteLeg.fee_bps` (collect Option<Vec> = all-or-nothing); tests round-trip.
- `backend/searcher-rs/src/{orchestrator,workers/flashloan_arb_worker,workers/liquidation_worker,workers/triangular_worker}.rs` (+3/+3/+3/+2): constructores directos → `None` (R8).
- `backend/sim-ctl/src/route_lookup.rs` (+1).
- `frontend/lib/store/types.ts` (+41), `OpportunityDetailTabs.tsx` (+45): wire + parse all-or-nothing + columna Fee (unidad dual-unit divulgada: V2/Curve/Balancer /1e4, V3 pips /1e6 — veredicto MATH-BATCH, nunca normalizado en fuente).
- Tests FE: `deriveLegs.test.ts` (+65), `OpportunityDetailTabs.test.tsx` (+31).

**Tests afectados / a exigir en CI:**
- Rust: `shared-rs` (234, reportados verdes en 8794ece6), `searcher-rs::persistence` tests `leg_fees_bps` (4 asserts), `cargo fmt --check`, `clippy`.
- FE: vitest 1269 + `tsc --noEmit` (reportados verdes en 8794ece6).
- Gate manual: ningún consumidor divide `leg_fees_bps` por 1e4 a ciegas (dual-unit).

**Limpieza post-merge (operador):** borrar branch espejo `feat/leg-econ-f1-20260918` y worktree `arbx-legs-econ-f1-20260918` (descartar su delta, subsumido).

## 7. Riesgos / notas
- El delta es mínimo y honesto; el riesgo real del PR está en 8794ece6 (ya revisado por su autor), no en la reconciliación.
- CRLF en econ-f1: si el operador decidiera usar ese árbol por error, `git add` normaliza pero rustfmt fallaría por la indentación de 12 espacios. Motivo adicional para descartar.
- Escalada al operador (BOARD): push de `feat/legfix-e0063-20260918` + apertura de PR.
