# WO-G-6 · GAP-3 (ronda 2) — test de render de los 3 sitios JSX del fix P0-2

> FIXER Gang Omniscience ronda 2 · 2026-09-08 ~23:30 local. Diffs marcados
> `// WO-G-6 (2026-09-07) · GAP-3 ronda 2` (mismo token grep que la ronda 1 —
> contención preservada). Reglas: RULE 00/R8 · §32/§33 (CERO interacción VPS,
> 0 requests HTTP — este gap es 100% local) · NO-GIT (0 commit/push/PR/deploy).

## §0 Qué cerré

**GAP-3** (WO-G-6-REVERIFICATION-R2.md §5): "los 3 sitios de render del fix no
tienen test de render RTL (los 8 tests nuevos son proyecciones puras; el path
JSX `:158-183` solo lo cubre tsc)". **CERRADO**: 3 tests nuevos renderizan el
JSX REAL del marquee y pinean los 3 sitios — 15/15 PASS, typecheck EXIT 0.

## §1 Desviación documentada del hint del cross-examiner (mesa redonda)

El hint pedía "1 test RTL con mock de `getOpportunitiesLive`". **Ese toolkit no
existe en este repo y está operator-gated por convención explícita**:

- `@testing-library/react`, `jsdom` y `happy-dom`: **MISSING** en
  `frontend/node_modules` y `node_modules` (verificado con test de existencia
  directo, no `ls|head` que enmascara exit codes).
- Vitest environment global = `node` (`frontend/vitest.config.ts:21`), sin
  override jsdom en ningún test.
- Canon repetido en 5+ archivos: "Why not @testing-library/react: Not
  installed; adding it requires **jsdom + operator approval** (toolkit-alignment
  memory)" — `components/__tests__/TokenChip.test.tsx:4-5`,
  `DeterministicAvatar.test.tsx:4-11`, `StatusPill`, `CrossChainSlot`,
  `components/ui/TokenIcon.test.tsx:3`.
- Instalarlo además mutaría `package.json` compartido en un árbol **sin
  lockfile** (memoria: "FE sin lockfile") con peers editando en vivo — riesgo
  desproporcionado para un gap LOW. **Rechazado; queda como opción del operador.**

**Adaptación honesta del intent del hint** (mismas 3 aserciones: `'—'`
presente, ausencia de `▲`/`▼`, sr-only `'yield not computed'`), con la
convención canónica del repo: `renderToStaticMarkup` (misma alineación que
`StatCard.test.tsx`, `XRayCard`, `OpportunityTradeCard`: "no jsdom, no
network"). El mock de `getOpportunitiesLive` quedó **innecesario por
construcción**: el fetch del contenedor vive en un `useEffect`
(`OpportunityTicker.tsx:87-106`) que JAMÁS corre bajo render estático — por eso
el path JSX era inalcanzable sin jsdom. La solución fue hacer el path
alcanzable (§2), no simular el efecto.

## §2 El fix — extracción presentacional `TickerMarquee`

`frontend/components/OpportunityTicker.tsx`:

1. **Extracción** (`:159-212`): el JSX del marquee (sr-only + track + map)
   pasó del cuerpo del contenedor a `export function TickerMarquee({ items })`
   — **movimiento puro, sin cambio de comportamiento**: mismo JSX byte-a-byte,
   mismo markup de salida; el contenedor termina en `return <TickerMarquee
   items={items} />` (`:156`). Esto vuelve testeable el path `:151-185` de la
   ronda 1 (ahora `:166-212`) con render estático, y documenta que el marquee
   es una función pura de `items`.
2. **React value-import** (`:6-9`): `import * as React from "react"` — patrón
   de repo citado por el par WO-H3 en `StatCard.tsx:3-7` ("the classic JSX
   path needs the React namespace under vitest's node transform; Next's
   automatic runtime at build time is unaffected") y `components/ui/tabs.tsx:3`.
   Necesario porque el JSX del ticker **jamás se había ejecutado en vitest**
   (eso ES el gap-3): primera corrida falló con `ReferenceError: React is not
   defined` — exactamente el gotcha documentado en
   `OpportunityTradeCard.test.tsx:13-16`.

**Mapeo de líneas para verificadores futuros** (R2 citaba las de ronda 1):
sr-only `:155`→`:175` · celda de yield `:170-172`→`:190-192` · flecha condicional
`:173-177`→`:193-197`.

## §3 Los tests (3 nuevos, `OpportunityTicker.test.tsx:143-213`)

Los fixtures pasan por la proyección REAL `opportunityToTickerItem`
(wire-row→item, pineada al wire vivo documentado en WO-G-6-REPORT.md §2 — RULE
00: cero datos decorativos) y de ahí al JSX REAL — sin copias del markup:

1. **roi_pct null** (el caso vivo 100%): `<span>—</span>` sin clase pos/neg;
   `▲`/`▼`/`class="arr` AUSENTES; sr-only contiene `yield not computed`; la
   fila sin precio ES visible (keep-rows BUG-06: pair + dexes en el html).
2. **roi_pct computado** (discriminador): `+2.31%` con `class="pos"` +
   `class="arr pos">▲`, `-0.42%` con `neg` + `▼` — prueba que la flecha SÍ
   renderiza cuando corresponde, haciendo NO-vacías las ausencias del test 1.
   Cubre la rama alternativa del sr-only (valor computado, no "yield not
   computed").
3. **Ventana mixta** (1 roi + 2 null): la condición es POR-FILA, no del latest
   — exactamente 2 flechas (1 fila computada × duplicación seamless-loop) y 4
   celdas `—`; sr-only resume la primera fila.

## §4 Verificación

| Check | Resultado |
|---|---|
| `npx vitest run components/__tests__/OpportunityTicker.test.tsx` | **15/15 PASS** (12 de rondas previas + 3 nuevos), 2.14s |
| `npm run typecheck` (workspace frontend) | **EXIT 0** |
| Suite frontend COMPLETA (`npm test`) | **1173 passed / 2 failed** (127 archivos, 74s) |
| Contención marker `WO-G-6` (rg frontend+backend) | SOLO los 2 archivos canónicos (componente + test); backend 0 |

**Adjudicación de los 2 fallos de la suite (ninguno mío):**
- `ControlBoard.test.tsx:645` — PRE-EXISTENTE del programa CB-03 (untracked,
  en vuelo por otro WO); ya adjudicado idéntico por R1 §4 y R2 §3. Cero ruta
  de import hacia el ticker. No pisado.
- `ControlScopeBadge.test.tsx` — timeout 5000ms bajo carga del árbol
  compartido (corrida con collect 476s — múltiples peers editando en vivo);
  **re-ejecutado aislado: 6/6 PASS**. Misma clase de transitorio que la race
  de StatCard que R2 §3b documentó. No hay camino de import
  ControlScopeBadge→OpportunityTicker.

**Bonus observado para la mesa:** typecheck verde implica que
`PreferenceVectorPanel.tsx` (GAP-4, dueño HP-05) fue reparado/retirado por su
dueño desde el reporte R2 (23:13) — el árbol compartido volvió a estar verde
para todos los peers.

## §5 Claims de archivo y conflictos

- Archivos tocados: `frontend/components/OpportunityTicker.tsx` +
  `frontend/components/__tests__/OpportunityTicker.test.tsx` — **ambos
  canónicos del propio WO-G-6** (ronda 1). Cero conflicto con otros WO; el
  diff vs HEAD acumula ronda 1 + ronda 2 sin commitear (NO-GIT).
- No toqué `getOpportunitiesLive`/`api-client` ni ningún archivo de WO-H4,
  HP-05, CB-03.
- **Residual honesto**: el wiring del efecto (`getOpportunitiesLive` →
  `setItems`, interval 30s) sigue cubierto solo por tsc + el browser-check de
  deploy — testearlo exige jsdom/RTL (operator-gated, §1) o e2e Playwright.
  Igual que en R2; GAP-3 no pedía ese tramo.

## §6 Estado de gaps tras esta ronda

| # | Estado |
|---|---|
| GAP-1 (fix NO deployed) | SIGUE — operator-gated (GIT→VPS) |
| GAP-2 (espejo backend `Some(0.0)`) | SIGUE — scope de otro programa |
| **GAP-3 (test de render)** | **CERRADO aquí** |
| GAP-4 (typecheck rojo HP-05) | Observado RESUELTO en vivo (verificar su dueño) |

— FIXER ronda 2 · Gang Omniscience · 2026-09-08
