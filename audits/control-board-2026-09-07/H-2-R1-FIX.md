# H-2-R1 FIX — subtexts de 3 causas honestas en los statcards del home (Gang Omniscience ronda 2)

- **WO**: H-2-R1 (gap R1 de `H-2-REVERIFY-ronda2.md` §3, adoptando también el hand-off
  H-2-FIX §6.1 + H-3-REVERIFY G1) · kind: fix + verify · **Agente**: FIXER H-2-R1
- **Fecha**: 2026-09-08 22:3x–22:5x local
- **Reglas honradas**: 0 git (0 commit/push/PR/deploy — NO-GIT 2026-08-23; HEAD intacto
  `27aca289`, branch `feat/hops-live-01`, todo en working tree) · 0 escrituras VPS · **0
  requests a dominio público** (verificación 100% local; presupuesto 5/5 intacto) ·
  0 executor/wallets/broadcast (§32/§33/§34.3) · RULE 00: no se fabricó dato alguno —
  sólo se dejó de declarar una causa falsa.

## 0. Sincronía de mesa redonda (leída ANTES de editar)

Leídos completos: `GOAL-WORKORDERS.md`, `H-2-REVERIFY-ronda2.md` (fuente del gap R1),
`H-3-REVERIFY.md` (G1 = la mitad del scope), `H-2-FIX.md` (§6.1 hand-off al hero),
`H-4-FIX-APPLY.md` (§5 outage PG + hunk detectedStat), `H-3-FIX.md`. Código leído completo
pre-edit: `frontend/app/page.tsx`, `StatCard.tsx` y las 3 suites pineadas
(`page.test.tsx`, `page.hero-h3.test.tsx`, `page.statcard-h4.test.tsx`). No contradigo
ningún hallazgo previo; ejecuto exactamente el fix que el reverify R1 + G1 especificaron.

## 1. Root cause (confirmado por lectura propia)

`frontend/app/page.tsx` (estado pre-mi-fix): los tres statcards del home declaraban la
causa del estado vacío con solo 2 ramas. Cuando el fetch falla
(`getHomeData` → `source="server-fetch-failed"` → `opportunities=[]`):

- **Decoherencia media** (hunk H-2, :261-267 del reverify): `detectedCount=0` → subtext
  **"sin datos — feed vacío"** — causa FALSA; la sección del feed vecina dice
  "Feed no disponible — snapshot del servidor falló" (:328-335 post-mi-fix).
- **Hero** (hunk H-3): `bestNet=null` → siempre "feed vacío" — además declarado por
  H-3-REVERIFY G1: también es falso con ventana poblada sin nets computados.
- **Asimetrías detectadas** (hunk H-4): `detectedStat(null, 0, null)` → **value 0** bajo
  **"ventana live · límite 50"** — un Some(0) que el sistema jamás computó (R8:
  None ≠ Some(0)) con label de ventana live: la doble deshonestidad.

Con PG crash-loopeando (H-4 §5, corroborado desde fuera por H-3-REVERIFY §3), el 503 es
el estado DOMINANTE de hoy: al deployar, el home habría culpado "feed vacío" con el feed
caído. Clasificación: **CANONICAL_REPO** (lectura directa del código fusionado).

## 2. El fix (3 hunks, todos marcados `// WO-H2-R1 (2026-09-07)`)

Split de **3 causas** — la rama `failed` va PRIMERO (domina sin importar los conteos):

```
failed ? "sin datos — feed no disponible"
       : detectedCount > 0 ? "<métrica> no computado en el feed"
       : "sin datos — feed vacío"
```

1. **Decoherencia media** (`page.tsx:286-300`): la rama del gap R1 tal cual el hint del
   cross-examiner — `"sin datos — roi no computado en el feed"` conserva la semántica de
   H-2 §2c; la rama `failed` se antepone. La variable `failed` (:181) ya estaba en scope
   y sin uso en ese hunk.
2. **Hero** (`page.tsx:236-252`): mismo split con la métrica correcta del hunk —
   `"sin datos — nets no computados en el feed"` (bestNet es max de nets; wording del
   hand-off H-2-FIX §6.1 + H-3-REVERIFY G1, no "roi"). Con esto el gap **G1 de
   H-3-REVERIFY queda CERRADO** (era la mitad de mi scope).
3. **detectedStat** (`page.tsx:147-176`): parámetro 4º opcional `failed = false`;
   cuando falla → `{ value: "—", subtext: "sin datos — feed no disponible" }`. El valor
   "—" es la respuesta honesta (nada fue computado) y es la MISMA que ya dan hero y
   Decoherencia en ese estado (consistencia de fila). Call site pasa `failed` (:189).
   El guard `animate={typeof detected.value === "number" && detected.value > 0}` (:269)
   acompaña el ensanchamiento del return a `number | string` — sin animación sobre "—".
   Default `false` preserva las 3 llamadas pineadas de H-4 byte a byte.

Prosa coherente con el feed vecino: los tres cards y la sección del feed ahora declaran
la MISMA causa en el estado failed ("feed no disponible" ↔ "Feed no disponible —
snapshot del servidor falló") — la contradicción intra-página que R1 señala queda muerta.

## 3. Tests de regresión — `frontend/app/__tests__/page.subtext-causes.test.tsx` (nuevo, 5 tests)

Mismo harness que `page.statcard-h4.test.tsx` (renderToStaticMarkup, sin jsdom, sin red;
vi.mock api-client + stubGlobal fetch — el fixture vive SOLO en el test, RULE 00):

- **Pure**: `detectedStat(null,0,null,true)` → `{"—","feed no disponible"}` (nunca 0
  bajo "ventana live · límite 50"); `failed` domina incluso con window_total presente.
- **SSR failed** (fetch `ok:false`): los 3 cards dicen "sin datos — feed no disponible";
  ausencia de las causas falsas ("feed vacío", "ventana live · límite 50"); sin verde en
  el hero; y "Feed no disponible — snapshot del servidor falló" PRESENTE (coherencia
  intra-página, la aserción del R1).
- **SSR poblado sin nets/rois** (fetch OK): hero → "nets no computados en el feed",
  Decoherencia → "roi no computado en el feed" (el caso G1, alcanzable según taxonomía
  de rechazos 48.4K/24h 100% rejected).
- **SSR vacío real** (fetch OK, 0 ítems): "feed vacío" RETENIDO en hero y Decoherencia
  (guard de la 3ª rama).

## 4. Verificación (corrida por mí, exit codes capturados sin pipes enmascaradores)

| Check | Resultado |
|---|---|
| `vitest run` 4 suites de page (H-2 + H-3 + H-4 + H-2-R1) | **25/25 PASS** (incluye el 9º test WO-R3 aparecido en paralelo — §6) |
| `tsc --noEmit` (frontend) | 7 errores — **TODOS en `components/PreferenceVectorPanel.tsx`**, archivo nuevo ajeno, huérfano (lo importa nadie; verificado por grep). `page.tsx`/StatCard/mi test: **0 errores** (el ensanchamiento `number \| string` tipa limpio) |
| `eslint app/page.tsx app/__tests__/page.subtext-causes.test.tsx` | **exit 0** |
| Suite completa frontend | **1160/1161 PASS** — única falla `ControlBoard.test.tsx` = CB-03 pre-existente (H-2 §5, H-3 §2, H-4 §2 y ambos reverifies ya lo reportaban; cero overlap con mis archivos) |
| NO-GIT | `git log` HEAD intacto `27aca289`; working tree only; 0 commits |

## 5. Coordinación de claims de archivo (documentada, sin pisar)

`frontend/app/page.tsx` sigue siendo co-editado. Mis 4 zonas (detectedStat firma+cuerpo,
call site :189, subtext hero :244-251, subtext Decoherencia :289-297 + guard animate
:269) NO solapan los hunks de H-2 (toXRayProps/computeAvgRoiPct), H-3 (viableCount/
variant) ni H-4 (windowTotal parse). Sus 16 tests pineados siguen verdes sin edición a
sus archivos de test — la rama default `failed=false` es no-op para sus call-paths.
Los comentarios de H-2/H-3/H-4 quedaron intactos; agregué los míos al lado (el "two
honest causes" de H-2 queda corregido por mi comentario adyacente "THREE causes now",
sin reescribir la palabra del par).

## 6. Co-edición EN VIVO durante mi sesión (2 eventos, documentados)

1. **WO-R3 en paralelo**: `page.test.tsx` creció 8→9 tests DURANTE mi sesión (bloque
   "WO-R3: toXRayProps token-safety null propagation" — el fix del gap R3 del reverify).
   El aviso "file modified on disk" me forzó a releer antes de mi último edit. El estado
   fusionado pasa 25/25 — convivencia limpia, no lo pisé.
2. **`components/PreferenceVectorPanel.tsx`** (untracked) rompe `tsc` con 7 errores
   propios y `frontend/app/config/trading/page.tsx` apareció modificado — trabajo en
   vuelo de OTRO WO (desconocido para mí, no está en los reportes que leí). **Hand-off a
   la mesa**: quien sea su dueño debe cerrar sus TS2532/TS7006 antes del gate de
   typecheck global; mi WO lo deja EXACTAMENTE como lo encontró (§3 disciplina).
3. Conflicto de claim NO existe con ningún WO conocido: mis líneas son aditivas a hunks
   ajenos y el único archivo compartido de test no fue tocado (creé el mío propio).

## 7. Pendiente / hand-off

1. **NO-GIT**: fix 100% local. Post-deploy, CB-06/BROWSE debería ver en outage PG: los
   3 statcards "—" con "sin datos — feed no disponible" (antes: 0/"feed vacío"), y con
   feed sano + ventana rejected: "roi no computado en el feed" + hero "nets no
   computados en el feed".
2. **R4 persiste** (StatCard SSR pinta "0" pre-mount — por eso mis aserciones de valor
   son a nivel de helper puro y no del markup estático): gap LOW cosmético reportado por
   el reverify §3 R4, NO adoptado aquí (scope R1 only).
3. **R2 persiste** (by-strategy "0.00%" fabricado): sigue siendo el agent-fixable más
   urgente según el reverify §4.1 — no es mío.
4. Para el próximo verifier: mis aserciones negativas (`not.toContain`) usan slices
   scoped por label (`segment()`) para no falso-positivar contra los otros cards.

## 8. Clasificación de claims

- Defecto 2-causas en los 3 subtexts + Some(0) en detectedStat failed: **CANONICAL_REPO**
  (lectura del código pre-fix; cita del reverify R1 confirmada línea a línea).
- Comportamiento post-fix: **PRIMARY_SOURCE** local (25/25 + 1160/1161 + tsc/eslint
  propios; el DOM público sigue mostrando el código viejo hasta deploy aprobado —
  expectativa honesta, no claim en vivo).
- "Estado dominante de hoy = 503 PG": **INFERRED** de H-4 §5 + H-3-REVERIFY §3
  (corroboración payload RSC) — no lo re-sondeé (presupuesto dominio público intacto).
