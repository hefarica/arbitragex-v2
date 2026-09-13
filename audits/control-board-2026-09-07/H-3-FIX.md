# H-3 FIX — hero "Mejor Topological Yield": denominador honesto + variante condicionada

- **WO**: H-3 (Gang Omniscience ronda 1, 2026-09-07/08) · **Agente**: FIXER H-3
- **Origen del hallazgo**: `BROWSE-Auditor-R8-caza-de-deshonestidad-de-datos.md` §3 H-3 (BAJO —
  categoría c: framing verde sobre expectativas rechazadas).
- **Estado**: **DONE local** (edición + verificación; NO-GIT respetado — 0 commit/push/PR/deploy,
  0 escrituras VPS, 0 requests a dominio público, §32/§33/§34.3 intactos).

## 0. Sincronía de mesa redonda (leída ANTES de editar)

Leídos completos: `GOAL-WORKORDERS.md` + `BROWSE-Auditor-R8-caza-de-deshonestidad-de-datos.md`
(fuente de mi WO). Consultados por contexto: `CB-VERIFY-FRONTEND.md` (estado /control 404 =
nada desplegado, por eso mi verificación es local, no browser) y el propio `frontend/app/page.tsx`.
Construyo directamente sobre el root-cause del auditor (max-net sobre ventana 100% rejected,
`page.tsx:91-98` pre-fix) — no lo re-derivé, lo verifiqué contra el código y lo corregí.

## 1. El defecto (verbatim del auditor, verificado)

El hero del home exhibía statcard `variant="success"` (verde) con label "Mejor Topological
Yield · neto" y valor `$4.2279` = `Math.max(net_expected_profit_usd ?? simulated_net_profit_usd)`
sobre la ventana live de 50 ítems — siendo esa ventana **100% rejected** (statuses=["rejected"],
reasons spot_product_le_one/v3_quote_unavailable) al momento de la observación (00:11Z). Es
decir: **verde de logro enmarcando la expectativa de candidatas que el propio sistema rechazó**.
La métrica era REAL (sin mock ni caché — el caso ventana-vacía ya renderea "— sin datos — feed
vacío"); el defecto era de framing (R8 categoría c).

## 2. El fix (ambas mitades del "o" del WO — denominador Y variante)

Archivo: `frontend/app/page.tsx` (3 hunks, marcados `WO-H3 (2026-09-07)`):

1. **`:176-182` — denominador de viabilidad**, computado del MISMO payload real:
   `viableCount = opportunities.filter(o => o.status !== "rejected" && o.status !== "failed").length`
   — predicado canónico idéntico a `/opportunities` (`app/opportunities/OpportunitiesClient.tsx:289`).
   RULE 00: ventana 100% rejected → 0; nada se inventa.
2. **`:210-228` — StatCard hero re-etiquetado**:
   - label: `"Mejor Topological Yield · neto esperado"` (declara que es EXPECTATIVA, nunca
     yield realizado; la procedencia "spine/sim" queda cubierta por "esperado" +
     `net_expected ?? simulated`).
   - subtext con datos: `` `esperado · candidatas pre-gate · ${viableCount} viables` `` — el
     denominador pedido por el WO, con el conteo vivo. Subtext hermano del calificador que ya
     usa la sección feed ("candidatas pre-gate · observación", `page.tsx:~285`).
   - **variante condicionada**: `variant={bestNet != null && viableCount > 0 ? "success" : "default"}`
     — el verde SOLO existe si la ventana contiene ≥1 candidata no-rechazada. Con la realidad
     de hoy (0 viables) el número vive en color neutral foreground. Estado vacío ("sin datos —
     feed vacío") también pierde el verde (antes el "—" se pintaba verde por el variant estático).
3. **`:9-13` — `import * as React from "react"`** — soporte SSR-test (patrón repo documentado en
   `HomeStoreAggregation.tsx:41-42` y `XRayCard.tsx`); el runtime automático de Next no cambia.

Archivos satélite (mismo patrón React-namespace, archivos NO reclamados por otros WO):
- `frontend/components/StatCard.tsx:1-7` (+5 líneas, solo import).
- `frontend/components/GateSection.tsx:1-5` (+5 líneas, solo import).

Barrera de regresión nueva: `frontend/app/__tests__/page.hero-h3.test.tsx` (4 tests,
renderToStaticMarkup, sin jsdom/red — mismo toolkit que `XRayCard.test.tsx`):

| Test | Escenario | Aserción clave |
|---|---|---|
| 1 | ventana 100% rejected (la realidad auditada) | label contiene "neto esperado"; subtext `esperado · candidatas pre-gate · 0 viables`; segmento hero SIN `text-[var(--success)]`; label viejo ausente |
| 2 | ventana mixta (1 viable + 1 rejected) | verde RETENIDO + denominador "1 viables" |
| 3 | status "failed" | cuenta como no-viable (predicado canónico) |
| 4 | ventana vacía | "sin datos — feed vacío", sin verde |

## 3. Verificación (ejecutada, no declarada)

- **Test nuevo**: 4/4 PASS (`npx vitest run app/__tests__/page.hero-h3.test.tsx`).
- **Coexistencia con pares en el MISMO archivo**: `app/__tests__/` completo (page.test.tsx de
  WO-H2 ×8 + page.statcard-h4.test.tsx de WO-H4 ×7 + page.hero-h3.test.tsx ×4) + XRayCard ×3 +
  HonestState ×4 = **26/26 PASS**.
- **`npx tsc --noEmit`** (frontend): **EXIT 0**.
- **`npx next lint`** sobre los 4 archivos tocados: 0 warnings/errors.
- **Suite completa frontend**: **1136/1137 PASS**. El único fail NO es mío (ver §5).

## 4. Archivo compartido page.tsx — coordinación con pares (claim respetado)

`frontend/app/page.tsx` se editó CONCURRENTEMENTE por tres fixers durante mi ejecución (lo vi
cambiar en disco 3 veces). TODOS los hunks quedaron marcados y coexisten:
- WO-H2: `toXRayProps` export + roi_pct null→"—" + `computeAvgRoiPct` (no los toqué).
- WO-H4: `LIVE_FETCH_LIMIT` + `window_total`/`max_age_seconds` + `detectedStat()` (no lo toqué).
- WO-H3 (mío): viableCount + StatCard hero + import React.
Mi edición del StatCard hero llegó a un archivo que YA contenía el fix de H-2 (lo re-leí antes
del segundo edit; el primer edit se aplicó sin conflicto). Observé errores tsc transitorios
(`displayedDetected`/`detectedSubtext` sin declarar) que eran el estado mid-edit de WO-H4 —
resueltos por H-4 mismo; el tsc final (EXIT 0) ya los incluye completos. **Nota para H-2**: su
comentario referencia `app/__tests__/page.test.tsx`, que no existía al inicio de mi ejecución y
sí existe ahora (8 tests PASS) — sin colisión con mi `page.hero-h3.test.tsx` (nombres distintos).

## 5. Fail del suite completo NO causado por H-3 (hand-off a CB-03)

`components/__tests__/ControlBoard.test.tsx` (1/1137): espera `SOLO 'shadow' exacto spawnea`
(minúsculas) mientras `ControlBoardLed.tsx` contiene esa forma Y `SOLO 'shadow' EXACTO spawnea`
(mayúsculas) — deriva test↔componente del territory CB-03 (archivos nuevos, en flujo). Evidencia
de no-influencia mía: `ControlBoardLed.tsx` importa solo react/lucide/zod/api-client/ui-table
(`ControlBoardLed.tsx:26-31`) — **cero relación** con StatCard/GateSection/page.tsx. No lo toqué
(claim de archivo). El owner CB-03 debe alinear casing.

## 6. Residuales y hand-offs (declarados, no corregidos — fuera de scope H-3)

1. **bestNet sigue siendo max sobre TODAS las filas de la ventana** (viables + rejected) — el WO
   pedía re-etiquetar/degradar variante, NO re-scopar la métrica. En ventana MIXTA el número
   verde aún puede estar driven por una candidata rejected; el denominador ("N viables")
   declara la población, y el caso extremo (0 viables = hoy) ya no es verde. Un max-viables-only
   sería refinamiento opcional — decisión de operador (cambia semántica de la métrica).
2. **Denominador = ventana fetcheada (≤50 filas)**, la MISMA población que produce el max —
   coherencia interna. No es `window_total` (campo de WO-H4, COUNT no acotado por limit):
   mezclar poblaciones sería su propia deshonestidad. Que nadie lea "0 viables" como claim
   stream-wide.
3. **Deploy pendiente**: NO-GIT — el fix vive solo local hasta los gates del operador. La DApp
   pública sigue mostrando el hero verde viejo hasta que se despliegue (CB-VERIFY-FRONTEND §5:
   ni /control ni estos fixes están desplegados). No deployar con audits en curso.

## 7. Clasificación de claims

- Predicado canónico de viabilidad (status ≠ rejected/failed): **CANONICAL_REPO**
  (`OpportunitiesClient.tsx:289-290`, gemelo en `app_backup`).
- Ventana live 100% rejected / $4.2279 verde: **PRIMARY_SOURCE** (fetch directo del auditor R8,
  §2 fila 7 + §3 H-3; no re-ejecutado por mí — presupuesto dominio).
- Patrón React-namespace para SSR-test: **CANONICAL_REPO** (`HomeStoreAggregation.tsx:41-42`).
- Que el verde condicionado elimina el framing deshonesto: **CANONICAL_WORKBOOK** (regla R8
  GOAL-WORKORDERS "estado desconocido se muestra como DESCONOCIDO" aplicada a framing).
