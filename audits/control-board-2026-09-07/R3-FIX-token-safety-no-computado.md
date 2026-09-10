# R3 FIX — TOKEN SAFETY hardcodeado "A 0 · B 0" → "— no computado" (Gang Omniscience, ronda 2, 2026-09-07/08)

- **WO**: R3 · kind: fix + verify · **Agente**: FIXER R3
- **Fuente del hallazgo**: `H-2-REVERIFY-ronda2.md` §3 R3 (cross-examiner adversarial de la
  ronda 2). Ancla original del defecto: `frontend/app/page.tsx:118-119` (pre-fix) pasaba
  `safetyA: 0, safetyB: 0` literales → `XRayCard.tsx:74` (pre-fix) renderizaba
  `TOKEN SAFETY: A 0 · B 0` en TODAS las cards del home. PRE-EXISTENTE en HEAD (verificado
  contra `git show HEAD:frontend/app/page.tsx` — los literales ya estaban), no introducido
  por H-2 ni flaggeado por el browse auditor (H-2-REVERIFY lo clasifica R8 categoría b:
  default disfrazado de dato).
- **Reglas honradas**: 0 git (sin commit/push/PR/deploy — NO-GIT 2026-08-23) · 0 escrituras
  VPS · 0 requests a dominio público (verificación 100% local) · 0 executor/wallets/broadcast
  (§32/§33) · RULE 00: no se fabricó dato alguno — se dejó de disfrazar un default como dato.

## 1. Sincronía de mesa redonda (leída ANTES de editar)

Leídos: `GOAL-WORKORDERS.md` (completo), `H-2-REVERIFY-ronda2.md` (fuente de mi WO — su
§4.2 despachó R1/R3/R4 como WO cosmético de honestidad), `H-2-FIX.md` (patrón de fix + el
estándar de marcado `// WO-XX`), `BROWSE-Auditor-R8-caza-de-deshonestidad-de-datos.md`
(fuente de la saga H — R3 es la misma clase b que su H-2, en otra métrica). Construyo
directamente sobre el R3 del reverify: confirmé su root-cause línea por línea antes de
editar (`OpportunityRowSchema` en `lib/schemas.ts:42-112` NO tiene campo de safety; el
`token_safety` de `schemas.ts:249` vive dentro de `AppConfigViewSchema` — otro schema de
config, sin conexión con la fila de oportunidad).

## 2. Root cause y fix

### 2a. Mapper — `frontend/app/page.tsx:118-126` (hunk `// WO-R3`)

- **Antes**: `safetyA: 0, safetyB: 0` literales. Violaba el contrato del PROPIO archivo
  (`page.tsx:84-85` pre-fix: *"Every field derives from the API payload; anything the API
  leaves null renders as an honest '—'"*) — 0 no derivaba del payload, era un default
  vestido de dato.
- **Ahora**: `safetyA: null, safetyB: null` con comentario que documenta (a) por qué null
  (no hay campo en el wire), (b) el historial del literal, y (c) la condición de re-wiring:
  derivar scores reales por pierna SOLO cuando el backend emita token-safety en el payload
  live (skill `token-risk-and-asset-safety-filter`).

### 2b. Componente — `frontend/components/XRayCard.tsx:17-23` (props) y `:79-91` (render)

- **Antes**: `safetyA: number; safetyB: number` requeridos no-nulos + render incondicional
  `` `A ${safetyA} · B ${safetyB}` `` → cualquier caller sin fuente caía en 0 disfrazado.
- **Ahora**: `safetyA: number | null; safetyB: number | null` (espejo EXACTO del patrón
  `confidence` del mismo archivo, `XRayCard.tsx:8-10`: *"null = not computed — rendered as
  '—', never 0%"*). El tipo nullable es la FUERZA del fix: el compilador ya no acepta un 0
  accidental — un caller debe pasar null (no computado) o un score real. Render:
  - ambas piernas null (el estado de HOY, sin wiring) → **"— no computado"**;
  - alguna pierna computada → `A ${safetyA ?? "—"} · B ${safetyB ?? "—"}` (manejo
    exhaustivo del tipo nullable, no flexibilidad especulativa);
  - score computado y EXACTAMENTE cero → `A 0 · B 0` honesto (R8: Some(0.0) ≠ None — el
    fix NO sobre-corrige, mismo criterio que el test 4 de WO-H2).

### 2c. Decisión de alcance (documentada, no silenciosa)

Elegí props `number | null` (opción primaria del hint del cross-examiner) sobre "cambiar el
prop a string": con string, cualquier caller podría volver a inyectar "A 0 · B 0"
fabricado y el componente no podría distinguirlo; con null tipado, el contrato fail-honest
vive en la FIRMA. No toqué R1 (subtext 3ª causa) ni R4 (StatCard SSR "0") — son hunks de
otros fixers de la ronda (R1 ya está aterrizando en `detectedStat`, ver §6).

## 3. Tests de regresión — 5 nuevos (2 archivos, marcados WO-R3)

`frontend/components/__tests__/XRayCard.test.tsx` (describe nuevo `:75-96`, 4 tests):

1. Ambas piernas null → contiene "no computado", NO contiene "A 0 · B 0" (el bug R3 tal
   cual vivía en cada card del home).
2. Scores computados → "A 87 · B 92" (el camino del wiring futuro queda pineado).
3. Computado y EXACTAMENTE cero → "A 0 · B 0" (Some(0.0) honesto — anti sobre-corrección).
4. Wiring parcial (una pierna) → "A — · B 92", nunca "A 0" coaccionado.

Fixture base `:29-31` actualizado `safetyA: 0, safetyB: 0` → `null` (directamente trazable
a mi cambio de tipo: dejar 0 en el fixture sería el mismo disfraz dentro del test).

`frontend/app/__tests__/page.test.tsx` (describe nuevo `:98-107`, 1 test):

5. `toXRayProps(makeOpp())` → `safetyA`/`safetyB` **null**, nunca 0 — pinea el mapper contra
   la regresión exacta del gap (si alguien re-hardcodea 0, este test falla).

## 4. Verificación (todo contra el estado ACTUAL del árbol compartido)

| Check | Resultado |
|---|---|
| `npx vitest run` XRayCard + page + page.hero-h3 + page.statcard-h4 | **27/27 PASS** (XRayCard 7 = 3 pre-existentes + 4 míos; page 9 = 8 WO-H2 + 1 mío; H-3 4/4; H-4 7/7 — sin regresiones a pares) |
| `npx eslint` sobre los 4 archivos tocados | **exit 0** |
| `npx tsc --noEmit` (frontend) | 7 errores — **TODOS en `components/PreferenceVectorPanel.tsx` (untracked, ajeno — ver §5); 0 errores en mis 4 archivos** |
| Suite completa `npx vitest run` | **1160/1161 PASS** · 1 fail = `ControlBoard.test.tsx` (CB-03, casing "SOLO 'shadow' exacto" — pre-existente, 4º peer que lo re-confirma: H-2 §5, H-3 §5, H-4 §2, H-2-REVERIFY §1) |
| `git status --porcelain` (mis archivos) | `M page.tsx`, `M XRayCard.tsx`, `M XRayCard.test.tsx`, `?? page.test.tsx` — working tree, **0 commits** (NO-GIT intacto) |

## 5. Fallos ajenos encontrados durante mi verificación (hand-off, NO los pisé)

- **`frontend/components/PreferenceVectorPanel.tsx`** (`??` untracked, mtime 22:28 hoy):
  charter **HP-05 (2026-09-08)** en vuelo de otro agente — contiene los 7 errores de tsc
  (TS2532 ×6 en :77-79, TS7006 en :200) y hoy no es referenciado por ningún archivo (el
  wiring de su página aún no aterriza). No es mío, no está en HEAD, no lo toqué.
  **Hand-off al fixer HP-05**: tu archivo es hoy el ÚNICO bloqueador rojo de `tsc --noEmit`.
- **`ControlBoard.test.tsx`** (CB-03): fail de copy pre-existente re-confirmado (ver §4).
- Nota operativa (eco de H-2 §5): `npx vitest run | tail` reporta exit 0 del PIPE, no de
  vitest — el resumen real está en las líneas "Test Files / Tests" del output.

## 6. Conflicto/convivencia de claims de archivo (documentado, requerido por el WO)

- **`frontend/app/page.tsx`** — compartido por 5 WOs con hunks limpiamente intercalados:
  WO-H2 (yield/computeAvgRoiPct/subtext), WO-H3 (viableCount/hero), WO-H4
  (LIVE_FETCH_LIMIT/detectedStat), WO-H2-R1 (3ª causa `failed` en detectedStat — aterrizó
  EN VUELO durante mi edición; mi Edit aplicó limpio y no lo pisó), y **WO-R3** (yo,
  :118-126). Verificado por diff completo contra HEAD.
- **`frontend/app/__tests__/page.test.tsx`** — archivo creado por WO-H2: mi cambio es
  APPEND-ONLY (un describe nuevo al final; los 8 tests de H-2 intactos y re-corridos).
- **`frontend/components/__tests__/XRayCard.test.tsx`** — pre-existente (AUDIT-2026-08-29),
  sin claim de WO activo: actualicé el fixture base (2 líneas) y agregué describe + línea
  de header documentando el nuevo guard. Los 3 tests originales intactos y pasando.
- **`frontend/components/XRayCard.tsx`** — sin claim activo previo; hoy lleva mis hunks
  :17-23 y :79-91. Los comentarios AUDIT-2026-08-29 pre-existentes quedan intactos.
- **`frontend/lib/schemas.ts`** — NO tocado (el fix no requiere cambiar el schema: la
  ausencia del campo ES el estado honesto; cuando exista wiring, el schema ganará el campo
  y el mapper dejará de pasar null).

## 7. Entrega

- Editados: `frontend/app/page.tsx` (1 hunk `// WO-R3`), `frontend/components/XRayCard.tsx`
  (2 hunks `// WO-R3`), `frontend/components/__tests__/XRayCard.test.tsx` (fixture +
  describe `// WO-R3`), `frontend/app/__tests__/page.test.tsx` (describe `// WO-R3`
  append-only). Working tree, 0 commits.
- 5 tests de regresión R8. Este reporte.
- **Expectativa honesta**: el dominio público sigue mostrando "TOKEN SAFETY: A 0 · B 0"
  hasta que el operador apruebe el pipeline de deploy (NO-GIT) — mismo estado declarado
  que H-2/H-3/H-4 (R5 del reverify: deploy pendiente + PG caído = operator-gated).
- Para CB-06/BROWSE post-deploy: re-verificar el journey §2 fila 1 del auditor R8 — la fila
  TOKEN SAFETY de cada card debe decir "— no computado".
