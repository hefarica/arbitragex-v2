# WO-ARCHIVE-401-FIX (2026-09-17, fixer gang ronda 1)

**Gap:** Archivo Frío (`/operations`) atascado en "Cargando estado de archivo…" para un
visitante sin sesión admin: 2× GET `/api/admin/archive/status` → edge 401
`{"error":"missing_admin_token"}`; la tabla de retención NO transiciona a estado de error
(spinner-eterno de texto) y el bloque "Archivos existentes" declara "Sin archivos aún."
cuando el estado es DESCONOCIDO, no vacío.

**Fuente del gap (pares, mesa redonda):**
- `BROWSE-Operador-de-mesa-…-DApp-VPS-read-only….md` §4.3 (:126-130) — "agent-fixable".
- `omniscience-integration-2026-09-06/BROWSE-Auditora-de-honestidad-R8….md` :41 y :79 — el
  mismo defecto YA estaba reportado el 2026-09-06 ("decir 'Cargando…' cuando ya falló es
  un estado vacío mal etiquetado; el error es honesto, el label no") y seguía vivo hoy.
  Este fix lo cierra 11 días después.

## 1. Causa raíz (CANONICAL_REPO)

`frontend/app/operations/components/ArchivePanel.tsx` (pre-fix):
- `refresh()` (:55-64) SÍ capturaba el error y lo pintaba verbatim abajo (:240-244, bloque
  `error` con estilo destructive) — honestidad parcial confirmada por ambos browses.
- PERO el label de la fila de la tabla se decidía SOLO por `!status` (:201-207): con
  `status=null` tras 401, renderizaba "Cargando estado de archivo…" para siempre.
- El bloque de archivos usaba `(status?.archives?.files ?? []).length === 0` (:222) → con
  `status=null` el `?? []` colapsaba "desconocido" a "vacío" → "Sin archivos aún." fabricado
  (violación R8: unknown ≠ empty).
- El edge devuelve el 401 correcto y el client NO reintenta
  (`api-client.ts:999-1002`, `retries: 0` con comentario explícito "401 must surface
  immediately") — el defecto era 100% de wiring de estado en el componente, no de red.

## 2. Fix aplicado (marcado `// WO-ARCHIVE-401 (2026-09-17)`)

Archivo: `frontend/app/operations/components/ArchivePanel.tsx` (solo este + test nuevo).

1. **Labels fail-honest puros** `retentionStateLabel(error)` y `filesStateLabel(status, error)`:
   - `!status && error` → `Estado de archivo no disponible: <error verbatim>` (fila con
     `role="alert"`) y `Listado de archivos no disponible: estado de archivo inaccesible.`.
   - `!status && !error` → loading honesto (transitorio real: fetch en vuelo).
   - `status` real con 0 archivos → "Sin archivos aún." (vacío genuino, único camino legítimo).
2. **Fila de la tabla** (:201-215 post-fix): usa `retentionStateLabel(error)` — el 401
   transiciona a estado de error, se acaba el spinner-eterno.
3. **Bloque archivos** (:234-239 post-fix): usa `filesStateLabel(status, error)` — "Sin
   archivos aún." SOLO con status real.
4. **Extracción `ArchivePanelView`** (patrón repo del panel hermano
   `RejectionBreakdownPanel.tsx:97/:221`): View presentacional pura + wrapper con el
   poll. Motivo: el env de test frontend es `node` sin jsdom — sin View no había forma de
   alcanzar estáticamente la rama error. `<ArchivePanel />` se sigue usando sin props en
   `OperationsClient.tsx:233` (verificado, cero cambio de superficie).
5. `import * as React from "react"` classic-JSX (patrón repo para el path esbuild/vitest).

Semántica "keep last good status" intacta: si un poll posterior al 401 tiene éxito,
`setStatus` + `setError(null)` restauran la tabla; si falla habiendo status previo, la
tabla sigue renderizando el último status bueno + el error verbatim abajo (comportamiento
pre-fix preservado — test "last good status is kept" lo congela).

RULE 00: cero datos fabricados — el fix REMOVE una etiqueta fabricada ("Sin archivos aún."
bajo unknown); no agrega ninguna. §32/§33/§34.3: no aplica (frontend read-only UI);
default-deny/MainnetRefused intactos (no tocados). Cero git commit/push/PR/deploy.
Cero VPS. Cero HTTP manual (0/5).

## 3. Verificación

- **Test nuevo** `frontend/app/operations/components/__tests__/ArchivePanel.test.tsx`
  (patrón RejectionBreakdownPanel.test.tsx, renderToStaticMarkup): **9/9 PASS**.
  Cubre: 401 → fila de error con `role="alert"` + verbatim `missing_admin_token` y NUNCA
  "Cargando estado de archivo"; unknown ≠ empty en ambos bloques; loading honesto
  transitorio; camino sano intacto (tablas, "—" para `rows_beyond_window === null`,
  "Sin archivos aún." solo con status real); keep-last-good-status.
- **Superficie /operations**: 6 archivos / 49 tests PASS (incluye OperationsClient.test
  que importa ArchivePanel).
- **Suite completa frontend**: corrida 1 = 1264/1265 (1 fallo transitorio NO reproducido
  en corridas 2 y 3 — 1265/1265 PASS ambas; FAIL-honest: no capturé el id del test en la
  corrida 1; hipótesis INFERRED: edición en vuelo del par del gap 4.5, ver §4).
- **tsc --noEmit**: 0 errores en mis archivos. Único error del árbol:
  `lib/hooks/useTokenIcon.test.ts:68` — trabajo EN VUELO DE UN PAR (gap 4.5 token-icon:
  `useTokenIcon.ts` modificado + test untracked en working tree, no míos). Probado con
  stash-roundtrip: al retirar SOLO los tracked-modified el test untracked del par queda
  huérfano y saltan 7 errores → el error vive en el diff del par, no en HEAD ni en mi
  cambio. Working tree restaurado intacto (git status verificado post-pop).
- **eslint** sobre los 2 archivos tocados: exit 0, cero hallazgos.

## 4. Conflictos de claims de archivo (regla del charter)

CERO archivos de otros WO pisados. Coexistencia en working tree con diffs EN VUELO de
pares (documentados, no tocados): `RouteDiscoveryFunnelCard.tsx`,
`RouteDiscoveryOutcomesPanel.tsx` (gap 4.2 — 503 outcomes), `useTokenIcon.ts` +
`useTokenIcon.test.ts` untracked (gap 4.5 — iconos). Mis ediciones son disjuntas
(ArchivePanel.tsx + test nuevo). El fallo transitorio de la corrida 1 y el error tsc son
consistentes con esas ediciones en vuelo, pero NO lo puedo afirmar como OBSERVED (no
capturé el id) — queda INFERRED.

## 5. Residual declarado (fail-honest)

- Badge `auto: OFF` con `status=null` sigue mostrando OFF cuando el valor es desconocido
  (el Switch está disabled sin status, así que no hay toggle optimista — riesgo bajo).
  NO tocado: fuera del alcance quirúrgico del gap (la mesa puede abrir WO propio).
- `Progress value={usedPct ?? 0}` pinta barra 0% bajo unknown — cosmético, no tocado.
- El poll de 30 s sigue reintentando el 401 (correcto: es read-only barato y permite
  transición a verde si el visitante autentica; `retries: 0` por-request ya evita
  backoff-agresivo).
- Deploy: NO aplicado (NO-GIT). El VPS corre este panel sin el fix hasta que el operador
  despache (recordar RULE 03: rebuild --no-cache del frontend por NEXT_PUBLIC bake).

## 6. Comandos de verificación (reproducibles)

```bash
cd frontend
npx vitest run app/operations/components/__tests__/ArchivePanel.test.tsx   # 9/9
npx vitest run app/operations                                               # 6 files / 49
npx eslint app/operations/components/ArchivePanel.tsx \
  app/operations/components/__tests__/ArchivePanel.test.tsx                 # exit 0
npx tsc --noEmit   # 0 errores en superficie ArchivePanel (1 error preexistente del par 4.5)
```
