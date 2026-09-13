# H-2 FIX — coacción null→0 en home (Gang Omniscience, ronda 1, 2026-09-07/08)

- **WO**: H-2 · kind: fix + verify · **Agente**: FIXER H-2
- **Fuente del hallazgo**: `BROWSE-Auditor-R8-caza-de-deshonestidad-de-datos.md` §3 H-2 y §5 fila 2
  (cards "+0.00%" y statcard "Decoherencia media 0.00%" con roi_pct null en todo el feed).
- **Reglas honradas**: 0 git (sin commit/push/PR/deploy — NO-GIT 2026-08-23) · 0 escrituras VPS ·
  0 requests a dominio público (verificación 100% local) · 0 executor/wallets/broadcast (§32/§33) ·
  RULE 00: no se fabricó dato alguno — sólo se dejó de fabricar "+0.00%".

## 1. Sincronía de mesa redonda (leída ANTES de editar)

Leídos: `GOAL-WORKORDERS.md` (completo), `BROWSE-Auditor-R8-caza-de-deshonestidad-de-datos.md`
(completo — fuente de mi WO), `CB-VERIFY-FRONTEND.md` (claims de `page.tsx`: refieren a
`frontend/app/control/page.tsx` de CB-03, NO a mi `frontend/app/page.tsx` — sin conflicto),
`CB-01-CROSS-EXAM.md` (citado vía browse §0). Construyo directamente sobre el H-2 del browse:
confirmé su root-cause línea por línea antes de editar (la expresión `(opp.roi_pct ?? 0).toFixed(2)`
con net≥0 produce literalmente "+0.00%"; su observación en vivo del journey 1 es la evidencia
primaria del comportamiento pre-fix).

## 2. Root cause y fix (frontend/app/page.tsx)

Números de línea AL MOMENTO de este reporte — el archivo es compartido con WO-H3/WO-H4 que
editan en paralelo y sigue creciendo (334 líneas ahora); los hunks WO-H2 están todos marcados
`// WO-H2 (2026-09-07)` y son localizables por ese marcador.

### 2a. Cards del feed — `toXRayProps().yield` (antes :57,59 → hoy :94-98)

- **Antes**: la existencia de net/gross gatedaba el render, pero el VALOR era
  `(opp.roi_pct ?? 0).toFixed(2)` con el signo tomado de net/gross → con net presente y roi null
  la card mostraba **"+0.00%"** fabricado (R8: None ≠ Some(0.0)).
- **Ahora**: `opp.roi_pct != null ? "+/-X.XX%" : "—"` — null propaga a "—", exactamente el patrón
  que confidence ya aplicaba (AUDIT-2026-08-29, el propio comentario del archivo lo documenta).
  El signo ahora pertenece a la magnitud mostrada (roi_pct), no a otra métrica — signo y valor
  coherentes. `Some(0.0)` (roi computado y exactamente cero) sigue renderizando "+0.00%" honesto.
- Los locales `net`/`gross` quedaron sin uso por MI cambio y fueron removidos (disciplina §3).

### 2b. Statcard "Decoherencia media" — `computeAvgRoiPct()` (antes :94-97 → hoy :123-133)

- **Antes**: `reduce(acc + (o.roi_pct ?? 0)) / opportunities.length` → 50 ítems todos-null
  promediaban a **0.00%** (y además dividía por la longitud total, mezclando computados con
  no-computados en el denominador).
- **Ahora**: función exportada `computeAvgRoiPct` que filtra los null y promedia SOLO los rois
  computados (espejo exacto del patrón `nets`/`bestNet` que ya vive 3 líneas abajo); feed
  all-null (o vacío) → `null` → StatCard renderiza "—" (StatCard ya sabía).

### 2c. Subtext honesto del mismo StatCard (hoy :254-265)

Con la propagación null, el estado vacío tiene DOS causas distintas. El subtext viejo decía
siempre "sin datos — feed vacío", que sería FALSE con feed poblado y roi no computado (un "—"
justificado por una razón falsa sigue siendo prosa deshonesta, R8). Ahora declara la causa real:
`"sin datos — roi no computado en el feed"` (feed no vacío) vs `"sin datos — feed vacío"`.

### 2d. Decisiones de alcance (documentadas, no silenciosas)

- `toXRayProps` y `computeAvgRoiPct` se exportaron como funciones puras para poder pinneearlas
  por test: la HomePage es un Server Component async y `renderToStaticMarkup` de
  react-dom/server@18 no soporta componentes async. Es el patrón repo (HomeStoreAggregation
  exporta `tallyStatuses`/`usToMs`/`totalP95Ms` para su propio test).
- NO toqué H-3 (hero verde), H-4 (subtext "Asimetrías detectadas"), ni H-1 (readiness-extras.ts).

## 3. Tests de regresión — `frontend/app/__tests__/page.test.tsx` (nuevo, 8 tests)

Fixtures con forma real `OpportunityRow` (schemas.ts:42-112; `roi_pct: z.number().nullable()`).
Reproducen el shape exacto observado en vivo por el browse auditor (net presente, roi null,
status rejected):

1. roi null + net presente → yield **"—"**, jamás "+0.00%" (el bug H-2 tal cual vivía).
2. roi null + sólo gross → "—".
3. roi computado → "+0.42%" / "-1.50%" (signo de la propia magnitud).
4. roi computado y EXACTAMENTE cero → "+0.00%" (Some(0.0) honesto — el fix NO sobre-corrige).
5. 50 ítems all-null (ventana live 100% rejected) → `computeAvgRoiPct` = **null**, jamás 0.
6. Feed vacío → null.
7. Mixto [1.0, null, 3.0] → 2 (nulls fuera de numerador Y denominador).
8. Todos cero computados → 0 real (no null).

## 4. Verificación (todo contra el estado ACTUAL del archivo compartido)

| Check | Resultado |
|---|---|
| `npx vitest run app/__tests__/page.test.tsx` | **8/8 PASS** (re-corrido final tras las ediciones de los pares) |
| `npx tsc --noEmit` (frontend) | **exit 0** (incluye los edits H-3/H-4 in-flight) |
| `npx eslint app/page.tsx app/__tests__/page.test.tsx` | **exit 0** |
| Suite completa `npx vitest run` | **1125/1130 PASS** · 2 archivos fallidos · **5 fallos NINGUNO mío** (ver §5) |

## 5. Fallos de la suite completa = trabajo EN VUELO de pares (no mío, no los pisé)

- `app/__tests__/page.hero-h3.test.tsx` — **4 fallos**, todos `ReferenceError: React is not
  defined` al renderizar la HomePage vía el harness `renderHomeWith` del par WO-H3 (runtime JSX
  clásico sin namespace React en su camino de render; el error cae en la región `viableCount`
  de SU hunk, page.tsx:152 del momento de la corrida). Mis hunks no introducen JSX ni referencias
  a React (`computeAvgRoiPct` es TS puro; el StatCard existía). **Hand-off al fixer H-3**.
- `components/__tests__/ControlBoard.test.tsx` — **1 fallo** de copy del panel de defaults de
  CB-03 ("SOLO 'shadow' exacto spawnea" esperado vs contenido reconciliado). Archivo in-flight
  del claim CB-03. **Hand-off a la mesa CB**.
- Nota operativa: la primera corrida en background reportó "exit 0" por el pipe `| tail`
  (enmascara el exit real de vitest) — la corrida final capturó exit real = 1. Lección para
  los pares: capturar el exit code de vitest sin pipe intermedio.

## 6. Conflicto/convivencia de claims de archivo (documentado, requerido por el WO)

`frontend/app/page.tsx` es compartido EN ESTE MOMENTO por TRES fixers con hunks distintos y
limpiamente intercalados (verificado por diff completo): **WO-H2** (yo: :94-98 yield, :123-133
helper, :254-265 subtext), **WO-H3** (viableCount + hero re-label/variant + `import * as React`),
**WO-H4** (LIVE_FETCH_LIMIT + envelope windowTotal/maxAgeSeconds). Sin sobreescritura mutua.
Recomendaciones a los pares:

1. **WO-H3**: tu subtext del hero (`"sin datos — feed vacío"` cuando bestNet es null) hereda la
   misma ambigüedad de dos causas que yo arreglé en §2c — con feed poblado pero sin nets, "feed
   vacío" es falso. Sugiero el mismo split `detectedCount > 0 ? "sin nets computados" : "feed vacío"`.
2. **WO-H3**: tu test falla por React-namespace (ver §5) — no lo arreglé yo para no pisar tu claim.
3. Los números de línea de este reporte son volátiles mientras H-3/H-4 sigan editando; el
   marcador `// WO-H2 (2026-09-07)` es el ancla estable.

## 7. Entrega

- Editado: `frontend/app/page.tsx` (3 hunks marcados WO-H2; working tree, NO-GIT — 0 commits).
- Nuevo: `frontend/app/__tests__/page.test.tsx` (8 tests de regresión R8).
- Este reporte. El estado DEPLOYADO en el dominio público sigue mostrando el bug hasta que el
  operador apruebe el pipeline de deploy (NO-GIT) — expectativa honesta, no un claim de fix en vivo.
