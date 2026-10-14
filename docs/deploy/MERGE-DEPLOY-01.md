# MERGE-DEPLOY-01 — Merge autorizado a `main` y verificación del auto-deploy

**Orden:** t24 (kind: integration, **attempt 2**) · **Intento:** `62abeeea-bca6-4869-a086-4034f638ced9`
**Ejecutor:** Reviewer · **Autorización:** operador ("Has todo esto vos") — merges a `main`; **prohibido** force-push, tocar el VPS, `workflow_dispatch`, mergear en rojo o con solapamiento > 0, reescribir ramas ajenas.
**Base:** `main` = **`c89d21a3`** → **final `2d22ce70`** · **Rama escrita:** `docs/merge-deploy-01` (PR abierto, **sin mergear**).
**Fecha:** 2026-10-06 (12:27Z–12:37Z los merges).

---

## 0. VEREDICTO EN UNA LÍNEA

**N = 15 merges ejecutados**, todos pasando **ambos gates duros** (verde + solapamiento cero contra `main` vivo); **0 mergeados en rojo**, **0 mergeados con solapamiento > 0**; `main` avanzó **15 veces** de forma verificada (`c89d21a3` → `2d22ce70`); y la **evidencia de deploy es la cola de `auto-deploy-vps.yml`**, no la respuesta del merge.

| Hecho | Artefacto |
|---|---|
| 15 merges, `main` `c89d21a3`→`2d22ce70` | §2 (tabla con head, merge commit y `main` antes→después) + `gh pr list --state merged` |
| Gate verde | §3: 31–33 checks `success`, 0 rojos, 0 pendientes al momento de cada merge |
| Gate revert | §4: solapamiento **0** recalculado contra `main` **vivo** antes de cada merge |
| 15 runs de auto-deploy creados (1:1 con los pushes) | §6 + `gh run list --workflow=auto-deploy-vps.yml` |
| El deploy del estado final | **`pending` al cierre** (§6) — **no afirmo deploy completado** |
| Checkout compartido intacto | `858b943fd80c8b5e1606d220aa87d63b86f4ba15` en `fix/perhop-reserves-01` |

---

## 1. ORDEN EJECUTADO (y una corrección de estado)

El contrato pedía: primero los PRs propios (#798, #800, #799) y después los peers que pasaran los gates. **Medido al empezar:**

| PR | Estado real al abrir este intento | Acción |
|---|---|---|
| **#798** (gas-coverage) | **MERGED** — `mergedAt=2026-10-05 18:33:45Z`, merge `6e2acc02` | **No re-mergeado** (ya estaba). Verificado, no asumido. |
| **#800** (labels) | **MERGED** — `mergedAt=2026-10-05 18:34:02Z`, merge `8ecfbaa4` | **No re-mergeado** |
| **#799** (gates) | **OPEN · `mergeStateStatus=BLOCKED`** | **NO MERGEADO** — check en **ROJO** (§3) |
| peers elegibles | 15 OPEN·CLEAN·verdes | **15 MERGEADOS** uno por uno (§2) |

⇒ De los 3 PRs "propios" del contrato, dos ya estaban en `main` desde ~18 h antes (por otra sesión) y el tercero **no podía mergearse**: su gate de verde falla. El trabajo real de esta orden fue el resto.

---

## 2. LOS 15 MERGES, UNO POR UNO

Comando por merge: `gh pr merge <N> --merge --match-head-commit <head_verificado>` y, después, `git ls-remote origin refs/heads/main` para confirmar el avance (**la verificación es del remoto, no de la respuesta del comando**).

| # | PR | head verificado | merge commit | `main` antes → después |
|---|---|---|---|---|
| 1 | #816 | `1a340a47` | `bb6c673d` | `c89d21a3` → `bb6c673d` |
| 2 | #817 | `d1cc0ec6` | `2efb13ff` | `bb6c673d` → `2efb13ff` |
| 3 | #818 | `72dda1c4` | `71e852a9` | `2efb13ff` → `71e852a9` |
| 4 | #820 | `e6177a93` | `10a423dd` | `71e852a9` → `10a423dd` |
| 5 | #821 | `00aba67e` | `d3fc2c42` | `10a423dd` → `d3fc2c42` |
| 6 | #822 | `dd0c0e6a` | `2c40aa5a` | `d3fc2c42` → `2c40aa5a` |
| 7 | #823 | `83942e39` | `e7baa20e` | `2c40aa5a` → `e7baa20e` |
| 8 | #824 | `9c29764a` | `43a60708` | `e7baa20e` → `43a60708` |
| 9 | #825 | `4f5ebde6` | `b2df138d` | `43a60708` → `b2df138d` |
| 10 | #826 | `49dcb0d4` | `b43f4fba` | `b2df138d` → `b43f4fba` |
| 11 | #828 | `e3a613d0` | `68c75e75` | `b43f4fba` → `68c75e75` |
| 12 | #829 | `1a71cbe1` | `afea6d26` | `68c75e75` → `afea6d26` |
| 13 | #830 | `63a1028b` | `11845fbb` | `afea6d26` → `11845fbb` |
| 14 | #831 | `b295e164` | `65903cca` | `11845fbb` → `65903cca` |
| 15 | #832 | `0257c3cc` | `2d22ce70` | `65903cca` → **`2d22ce70`** |

**`gh pr list --state merged` confirma los 15** con esos mismos merge commits (verificación por tercero). `#828` y `#829` son PRs **míos** (t56, t63): lo declaro por transparencia; el gate que se les aplicó es el mismo que a los demás.

---

## 3. GATE 2 — VERDE (por PR, medido, no asumido)

Al momento de cada merge: **todos los checks en `success`/`neutral`/`skipped`, 0 rojos y 0 pendientes**, y `mergeStateStatus=CLEAN`.

- Los 15 elegibles tenían **31–33 checks verdes** con **0 rojos / 0 pendientes**.
- **`#799` quedó FUERA por este gate**: `gh pr checks 799` → **30 `pass` + 1 `fail`**: `gitleaks (secrets scan)` (`https://github.com/hefarica/arbitragex-v2/actions/runs/37355608678/job/111917118422`). La API del check devuelve `output.summary=null`, así que **no re-medí** la causa concreta: la causa citada por `t22` —hallazgos **históricos** en la familia `.claude/settings*.bak`, ajenos al diff de #799— queda como **cita de otro artefacto, no como medición mía**. Lo que sí es mío: el check está en `failure` y el gate es absoluto ⇒ **no se mergea**, aunque su solapamiento sea 0 (§4).
- **Pendiente ≠ verde**: la primera pasada dejó 14 PRs en `mergeStateStatus=UNKNOWN` (estado transitorio mientras GitHub recalcula tras mover `main`). Los **esperé** (polling) y recién entonces se aplicaron los gates (§9, I-01).

---

## 4. GATE 3 — REVERT (el decisivo)

Método (conservador, declarado): `files(PR)` ∩ `files(compare <baseRefOid_PR>...main)`, **recalculado contra `main` VIVO antes de cada merge** — no contra el `main` del arranque.

- **Solapamiento = 0 en los 15** ⇒ ninguno quedó `BLOQUEADO_POR_RIESGO_DE_REVERT`; no hubo que reportar bloqueos por esta causa.
- Razón estructural del lote: 12 de los 15 son **un archivo nuevo** bajo `docs/…` o `audits/…`; los 3 restantes (#825, #831) tocan `ci/`/`scripts` y también salieron con solapamiento 0.

**Anexo medido — los 4 casos que el contrato marcaba como "no mergear sin re-materializar" (`#797`, `#796`, `#795`, `#794`):** hoy están **MERGED desde 2026-10-05 19:15** (otra sesión, ~18 h antes de esta orden), con merge commits `4a142e7c`, `08a0e263`, `8a9f9a7b`, `01bcf007`. **No los toqué.**

Y medí el daño anticipado en los dos archivos que el contrato nombra: en los cuatro casos el blob de `main` **antes** del merge (`p1`) es **idéntico** al blob resultante del merge y al del head del PR (`984fb949e5…` en `native_operator_adapter.rs`; `c9c0b31784…` en el test de contrato) ⇒ **el revert silencioso NO se materializó en esos archivos**; los PRs contenían ya la versión de `main`. La advertencia del contrato es correcta **como mecanismo**; en estos cuatro, la medición no la confirma.
*(Nota de honestidad: mi primer test dio "REVERT CONFIRMADO" por una lógica defectuosa —comparaba `main` con el head en vez de con el padre del merge—; lo rehice con los padres y el resultado es el de arriba. §9, I-03.)*

---

## 5. NO MERGEADOS — reportados, sin tocar

| Grupo | PRs | Estado medido | Por qué no se mergeó |
|---|---|---|---|
| **Rojo (gate 2)** | **#799** (30 pass /**1 fail** gitleaks) | OPEN · BLOCKED | Check en rojo. Absoluto. |
| **Drafts** | #706, #243, #236 | OPEN · `isDraft=true` | Excluidos por contrato |
| **DIRTY (conflicto)** | #472, **#783** | OPEN · `DROP` | Excluidos por contrato; el conflicto lo confirma GitHub |
| **DIRTY (otros)** | #674, #476, #462, #450, #449, #463, #430, #433 | OPEN · DIRTY | No mergeables sin resolver conflicto |
| **BLOCKED con checks verdes** | #788, #786, #785, #728, #707 | OPEN · BLOCKED | **Causa medida**: falta el check requerido **`Verifier policy tests`** (`ci-gate=SI`), y `main` exige ambos ⇒ la protección de rama los bloquea |
| **Rojos (otros)** | #827 (3), #732 (2), #729 (1), #440 (1), #243 (1), #236 (1) | OPEN | Gate 2 |

---

## 6. VERIFICACIÓN DEL DEPLOY — la evidencia es el run, no el merge

`auto-deploy-vps.yml` dispara con `push: branches: [main]`, `concurrency.group=production-vps`, `cancel-in-progress: false`, y el job de deploy está condicionado a `github.ref == 'refs/heads/main'`.

**15 pushes ⇒ 15 runs creados** (1:1, medido):

| Run | SHA | Estado | Lectura |
|---|---|---|---|
| `37463425916` | `bb6c673d` | **failure** | `RuntimeError: Target superseded by main; deploy the newer validated commit` ⇒ **supersesión por diseño**, no rotura |
| `37463546298` … `37464463756` (13 runs) | intermedios | **cancelled** | cada push posterior canceló el anterior en la cola |
| **`37464520553`** | **`2d22ce70`** | **`pending`** al cierre | **es el que importa**. URL: `https://github.com/hefarica/arbitragex-v2/actions/runs/37464520553` |

**Lo que NO afirmo:** que el estado final esté desplegado. Al cierre de este informe el run del estado final está **`pending`** — y `pending` no es deploy completado. Queda un watcher corriendo para capturar su conclusión; se reportará como **evidencia suplementaria** (append-only), no como parte de este veredicto.

**Punto de partida, sí verificado:** el `main` anterior (`c89d21a3`) **sí tenía deploy completado y verde** — run `37405962576`: jobs `Wait for all deployment gates => success` y `Deploy to VPS => success`, duración **61,6 min**.

---

## 7. COSTO REAL (medido) vs. el declarado en la orden

- Declarado: «cada push encola un deploy (65 min de gates + 120 min de job) ⇒ N merges = N deploys encolados».
- **Medido:** N=15 merges ⇒ **15 runs creados**, de los cuales **14 terminaron temprano** (1 superseded, 13 cancelled) y **solo 1 llega a desplegar**. La cola efectiva drena a **un** deploy de ~60 min, no a 15 × 60. El costo real de publicar 15 PRs fue **≈1 deploy**.
- Estado de la cola al cierre: `1 pending` (el final) + `1 queued` residual (`37463546298`, sha `2efb13ff`).

---

## 8. DECLARACIONES OBLIGATORIAS

- **Ningún merge acredita la identidad del runtime.** El runtime sigue declarando su SHA por variable de entorno; verificar eso exige mirar el VPS, y **tocar el VPS estaba prohibido**. Este trabajo acredita que `main` avanzó y que el pipeline disparó — **no** que el contenedor en producción ejecute `2d22ce70`.
- **Ninguno mueve P/N.** `P/N = 0/115` sin cambio: el censo mide contra el SSOT (`ACCEPTANCE-MATRIX-v1.md` / `arbx-scope-1.0.0`), no contra el patch. G1-G8 es un universo aparte y **no se suma a N**.
- **Riesgo estructural declarado:** el estado combinado de 15 merges **nunca fue validado por CI como conjunto** (cada PR corrió sus checks contra su propia base). El gate de solapamiento reduce el riesgo de revert, **no** valida interacciones semánticas.
- **Prohibiciones respetadas, verificables:** sin force-push; sin tocar el VPS; sin `workflow_dispatch` (los 15 runs son `event=push`); sin mergear en rojo (#799 vivo); sin mergear con solapamiento > 0; sin reescribir ramas ajenas; y **siempre** `gh pr view <N>` **con número** (el comando sin número resuelve al PR de la rama del checkout — la trampa que `t21` ya midió).

---

## 9. INCIDENTES Y DEFECTOS DE MI PROPIO INSTRUMENTO (declarados)

| ID | Qué pasó | Consecuencia / corrección |
|---|---|---|
| **I-01** | Mi gate exigía `mergeStateStatus=CLEAN`, y tras el primer merge GitHub devolvió **`UNKNOWN`** (estado transitorio) en los 14 restantes ⇒ los bloqueó **a todos** | El contrato manda esperar: agregué **polling** y recién entonces se aplicaron los gates. Sin esa corrección habría reportado 14 falsos bloqueos |
| **I-02** | Mi primer loop murió por **timeout de 120 s** de la herramienta **a mitad de camino, con 5 merges ya ejecutados** | Medí el estado real antes de continuar: los 5 eran merges **completos y consistentes**; no hubo merge perdido ni duplicado. El resto corrió en **background** |
| **I-03** | Mi **primer test de revert silencioso** era lógicamente inválido (declaraba revert cuando `main == head`, que es el caso normal) y dio **"REVERT CONFIRMADO"** en #794-797 | Lo rehice con los **padres del merge**: `blob(p1) == blob(merge)` ⇒ **sin revert**. El primer resultado era un artefacto de mi instrumento |
| **I-04** | En el test corregido, dos "merge commits" no tienen segundo padre (squash/FF) y mi script tiró errores de `Substring` sobre null | El dato válido (p1 vs merge) salió igual; los errores quedan declarados, no ocultados |

---

## 10. HANDOFF

| Consumidor | Qué recibe |
|---|---|
| **Operador** | `main` publicó los 15 PRs que pasaban los gates. Falta **un** dato: la conclusión del deploy del estado final (run `37464520553`, pendiente). El estado combinado no fue validado por CI como conjunto: si algo falla en el deploy final, el `revert` es la vía, no el force-push |
| **CI / repo admin** | 5 PRs (#788, #786, #785, #728, #707) están bloqueados por `main` porque **no se les reporta el check requerido `Verifier policy tests`**. No es un defecto de los PRs: es un filtro de ruta del workflow |
| **Seguridad** | `#799` sigue **vivo y bloqueado** por el scan de secretos rojo (hallazgos históricos en `.claude/settings*.bak`, según `t22`): exige **rotación del operador**, no un merge |
| **Quien audite este trabajo** | Todo lo de arriba se re-deriva con `gh pr view <N>`, `gh pr checks <N>`, `gh run list --workflow=auto-deploy-vps.yml` y `git ls-remote origin refs/heads/main`. **Ningún merge de este lote se apoya en la respuesta del comando: se apoya en que `main` avanzó en el remoto** |

---

*Ejecutado con `gh` (API de GitHub) y `git ls-remote`; **ningún** comando tocó el VPS ni reescribió ramas. Los gates se aplicaron **antes** de cada merge, contra `main` vivo. `P/N = 0/115` sin cambio. Único path escrito: `docs/deploy/MERGE-DEPLOY-01.md` (rama + PR, sin mergear).*
