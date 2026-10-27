# KELLY-GUARD-PRECONDITION-01 — precondición de capital para `ARBX_LIVE_EXEC_ENABLED`

- **Tarea:** `t70` (verificación). **Tipo:** precondición, **no** un merge.
- **PR bajo verificación:** **#827** — KELLY-GUARD-01, rama `security/kelly-guard-01`.
- **Base medida:** `origin/main` = `c89d21a3437c7fc2456fb7a08dba7d26ef41ee5e`.
- **VEREDICTO DE READINESS: `NO LISTO`.** `mergeable=MERGEABLE` y 31/31 checks auxiliares en verde,
  pero **`ci-gate` (uno de los DOS contexts requeridos de `main`) está en FAILURE**, por un
  **`cargo fmt --check` que falla sobre el propio fichero del guard**. Detalle y remediación en §2 y §7.
- **ORDEN:** #827 debe aterrizar en `main` **ANTES** de que `ARBX_LIVE_EXEC_ENABLED` pase a `true`.
  **No se mergea en esta orden.**

---

## 1. Estado de #827 (por comando, no por memoria)

```
$ gh pr view 827 --repo hefarica/arbitragex-v2 --json state,mergedAt,changedFiles,headRefOid,mergeable,baseRefName,additions,deletions
state=OPEN  mergedAt=null  changedFiles=1  head=1b5fd7156ccd176e635bdbdf048fd68ccf75f18a
mergeable=MERGEABLE  base=main  +168/-5

$ git log --oneline -1 origin/main
c89d21a3 Merge pull request #815 from hefarica/sre/runtime-identity-repeat-01
```

Fichero único del PR: `backend/prioritization-spine/src/bayesian_allocator.rs`.

## 2. ¿Está #827 listo para merge? **NO** — y esto es lo que falta

Los **contexts requeridos** de `main` (branch protection, por comando) son **exactamente dos**:

```
$ gh api repos/hefarica/arbitragex-v2/branches/main/protection --jq '.required_status_checks.contexts'
["ci-gate", "Verifier policy tests"]
```

| Contexto requerido | Estado en #827 |
|---|---|
| `Verifier policy tests` | **pass** ✅ |
| **`ci-gate`** | **FAIL** ❌ |

```
$ gh pr checks 827 --repo hefarica/arbitragex-v2     (resumen)
fail: 3      pass: 31
fail 21s  cargo check + clippy + test
fail  2s  ci-gate
fail 21s  lint-and-test-rust
```

**Causa nombrada (no es el guard, es FORMATO).** Los dos jobs de Rust mueren en su step de
`cargo fmt`, que es **bloqueante**; todo lo demás de esos jobs queda `skipped`:

```
# CI  -> job lint-and-test-rust (run 37409975618 / job 112095992013) sobre head 1b5fd715
 5 [failure] cargo fmt (blocking)              <-- acá muere
 6 [skipped] cargo clippy (gating — searcher-rs + relays-client)
 7 [skipped] cargo clippy (blocking — workspace, default features)
 8 [skipped] cargo test --lib (workspace)

# Rust CI -> job cargo check + clippy + test (run 37409975455 / job 112095990604)
 [failure] cargo fmt (blocking — workspace)    <-- acá muere
 [skipped] cargo check / clippy / cargo test --lib (workspace)
```

El error crudo, en el log del gate:

```
Diff in .../backend/prioritization-spine/src/bayesian_allocator.rs:627:
-        assert!(alloc.fraction > 0.0, "fraction debe ser > 0 con edge positivo");
+        assert!(
+            alloc.fraction > 0.0,
+            "fraction debe ser > 0 con edge positivo"
+        );
Diff in .../backend/prioritization-spine/src/bayesian_allocator.rs:639:
-        // f* = (0.5*0.1 - 0.5)/0.1 = -4.5 ⇒ no hay apuesta DENTRO del dominio
+                                          // f* = (0.5*0.1 - 0.5)/0.1 = -4.5 ⇒ ...
```

**Reproducido de forma independiente** por mí (no sólo leído del CI):

```
$ git show 1b5fd715:backend/prioritization-spine/src/bayesian_allocator.rs > pr-file.rs
$ rustfmt --check --edition 2021 pr-file.rs      # rustfmt 1.8.0-stable
Diff in pr-file.rs:627: ... Diff in pr-file.rs:639: ...
exit=1
```

**No es un defecto de configuración de CI**: el mismo job en `main` está en verde.

```
$ gh run list --workflow "Rust CI" --branch main --limit 2
37382777279 success sha=31e1d8db        -> job "cargo check + clippy + test" :: success
```

⇒ Lo único que falta es **formato mecánico** (dos hunks de `rustfmt`, **cero semántica**) en el
fichero del guard. `mergeable=MERGEABLE`: no hay conflicto con `main`.

## 3. El guard es PORTANTE — medición ejecutable de `t50`

Medido **ejecutando el camino de producción** (`BayesianAllocator::assign`), no por lectura:

- **Mutante A (guarda neutralizada, sin clamp)** sobre el `b` que midió `t48`
  (`b = -0.005437521`, `p = 0.95`):

  ```
  assertion `left == right` failed: yield=-0.005437521 debió dar fracción 0
    left: 0.21132486540518713
   right: 0.0
  ```

  ⇒ **sin la guarda, el dimensionador emite `0.2113` (~21% del cap) desde un edge NEGATIVO** — una
  pérdida garantizada. La guarda es **portante**: no es cosmética.

- **Mutante B (semántica PRE-FIX fiel: `b = b.max(0.0)` + guarda neutralizada)**: el mismo input da
  `0` **sin razón nombrada** — indistinguible de una decisión de sizing real. Ese es el defecto que
  el PR cierra (el pre-fix era `:218 let b = expected_yield_ratio.max(0.0);` + `:221-225 → 0.0`).

- **Peligro con número** (fijado como test ejecutable en el PR,
  `kelly_out_of_domain_formula_would_return_10x_measured_hazard`): `f* = (b·p − q)/b` con
  `b = −0.005437521`, `p = 0.95` ⇒ **10.1453**, **positivo**. Es un fallo de la fórmula **fuera de
  su dominio** (Kelly asume `b > 0`), no un caso borde.

Contraste actual del PR: **`Rust tests` = SUCCESS** sobre este mismo head (`1b5fd715`); localmente,
en `t50`, la suite del allocator dio **13/13 passed** tras `cargo clean -p`

## 4. Ningún límite de riesgo tocado (por diff)

Valores en el head del PR (por comando):

```
$ git show 1b5fd715:backend/prioritization-spine/src/bayesian_allocator.rs | grep -E 'const KELLY_FRACTION_CAP|const KAPPA_VARIANCE_AVERSION|pub const POSTERIOR_TTL'
const KAPPA_VARIANCE_AVERSION: f64 = 2.0;
const KELLY_FRACTION_CAP: f64 = 0.5;
pub const POSTERIOR_TTL: Duration = Duration::from_secs(900);
```

Y el diff `main..head` **no modifica ninguna de esas declaraciones** — las únicas líneas que las
mencionan son **aserciones de test** que las USAN como cota:

```
$ git diff c89d21a3 1b5fd715 -- backend/prioritization-spine/src/bayesian_allocator.rs | grep -E '^[+-].*(KELLY_FRACTION_CAP|KAPPA_VARIANCE_AVERSION|POSTERIOR_TTL)'
+            (alloc.kelly_fraction - KELLY_FRACTION_CAP).abs() < 1e-9,
+            "kelly_pos = {} debería tocar la cota dura {KELLY_FRACTION_CAP}",
+        assert!(alloc.fraction <= KELLY_FRACTION_CAP + 1e-9);
```

⇒ `KELLY_FRACTION_CAP=0.5`, `KAPPA_VARIANCE_AVERSION=2.0`, `POSTERIOR_TTL=900s` **intactos**.

## 5. La guarda SOLO PUEDE REDUCIR — nunca aumentar

Del diff, la guarda se compone de:

```
+        if !(expected_yield_ratio > 0.0) {      # rechazo ANTES de la fórmula
+            let mut alloc = Allocation::zero(...);  # fraction 0, usd 0
+            alloc.sizing = SizingDecision::NonPositiveYieldRatio;
+            return alloc;
...
+        if !(raw_kelly > 0.0) {                  # dentro de dominio: tampoco hay apuesta
+            ... Allocation::zero(...)
+            alloc.sizing = SizingDecision::KellyNoEdge;
+            return alloc;
```

1. Ambas ramas nuevas **retornan `Allocation::zero`** ⇒ `fraction = 0` y `usd_amount = 0`, que es
   el **mínimo posible** (`>= 0` por construcción y por el test preexistente
   `fraction_is_bounded_and_usd_within_cap_table`).
2. La rama válida (`b > 0`) **no cambia su aritmética**: `raw_kelly = ((p·b) − q)/b` y
   `kelly_pos = raw_kelly.clamp(0.0, KELLY_FRACTION_CAP)` siguen iguales.
3. Para `b > 0` con `raw_kelly <= 0`, antes se clampeaba a `0` y se seguía; ahora se retorna `0`
   con nombre. **Mismo VALOR, mejor RAZÓN** — no un aumento.
4. El clamp del pre-fix sobre `b` (`b.max(0.0)`) fue **eliminado**, no reemplazado por otro clamp.

⇒ **No hay ninguna ruta que aumente el tamaño.** Si el diff permitiera aumentar, esto sería un
BLOCKER: **no lo es**. Corrobora: los tests preexistentes de cota y monotonía siguen verdes
(`Rust tests` SUCCESS en el head; 13/13 local en `t50`).

## 6. ORDEN declarado (lo que el operador debe respetar)

> **#827 debe aterrizar en `main` ANTES de que `ARBX_LIVE_EXEC_ENABLED` se ponga en `true`.**
> Sin la guarda, el camino de producción emite `0,2113` (~21% del cap) desde un edge negativo
> medido — es decir, recomienda invertir en una pérdida.

## 7. Paso exacto para el operador (y lo que falta)

**A. Reparar el bloqueo (mecánico, cero semántica) — requiere una tarea con `backend/` en alcance.**
Este documento **no** lo hace: `backend/prioritization-spine/` está **fuera de alcance** en `t70`.

```bash
cd backend && cargo fmt -p prioritization-spine    # 2 hunks: :627 (assert!) y :639 (comentario)
git add backend/prioritization-spine/src/bayesian_allocator.rs
git commit -m "style(fmt): KELLY-GUARD-01 — cargo fmt (2 hunks mecánicos)"
git push    # re-dispara CI; ci-gate debe pasar a success
```

**B. Verificar y solo entonces mergear** (lo hace el operador; Security no mergea):

```bash
gh pr checks 827 --repo hefarica/arbitragex-v2        # exige: ci-gate = pass Y Verifier policy tests = pass
gh pr view 827 --repo hefarica/arbitragex-v2 --json mergeable,mergedAt    # MERGEABLE / null
```

**C. Recién entonces habilitar capital** (§34.3/§34.5): `ARBX_LIVE_EXEC_ENABLED=true` +
`ARBX_LIVE_EXEC_CHAINS`, con la autorización operativa correspondiente. **No antes** de `ci-gate` verde.

## 8. Declaraciones

- **NO se mergeó nada**: #827 sigue `mergedAt=null`; este documento se entrega por PR propio, también sin merge.
- **No se tocó la guarda** para hacerla más permisiva: ni `#[allow]`, ni `max(0.0)` «para que compile»,
  ni ningún cambio en `backend/` (fuera de alcance en esta orden).
- Paper: sin firma, sin broadcast, sin capital, sin deploy. `ARBX_LIVE_EXEC_ENABLED` **no** fue tocado.
- Sin merge ni push a `main` por mi parte; verificación por el remoto (`ls-remote`).
