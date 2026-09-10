# H-3 RE-VERIFY — re-verificación adversarial del fix del hero (Gang Omniscience ronda 2)

- **WO**: H-3 (re-verificación del fix de `H-3-FIX.md`) · **Agente**: VERIFIER H-3
- **Fecha**: 2026-09-08 22:0x–22:2x local · **Read-only total**: 0 git-writes, 0 VPS (ni
  lectura: innecesaria), 0 executor/wallets/broadcast (§32/§33/§34.3 intactos).
- **Presupuesto dominio público**: **1/5 requests** (curl único a `/` — §3).

## 0. Sincronía de mesa redonda (leída ANTES de verificar)

Leídos: `GOAL-WORKORDERS.md` (completo), `BROWSE-Auditor-R8-caza-de-deshonestidad-de-datos.md`
(fuente del WO), `H-3-FIX.md` (el fix bajo verificación), `H-2-FIX.md` y `H-4-FIX-APPLY.md`
(co-editores de `page.tsx`; H-2 §6.1 contiene un hand-off dirigido a H-3 — ver G1),
`CB-VERIFY-FRONTEND.md` §5 (estado no-deploy). Nada refutado; dos confirmaciones y una
corroboración (§5).

## 1. Veredicto

**El gap original H-3 quedó CERRADO localmente, sin regresiones.** Ambas mitades del "o" del
auditor (etiquetar denominador / bajar variante) están implementadas, marcadas
`// WO-H3 (2026-09-07)` y pinnadas por 4 tests que re-ejecuté yo mismo. Persisten **3 gaps
residuales** (§4): uno de prosa que un peer (H-2) hand-offeó a H-3 y no fue adoptado, uno de
scope de métrica declarado por H-3, y el no-deploy (verificado con evidencia primaria).

## 2. Verificación del fix (corrida por mí, no confío en los claims)

| Claim de H-3-FIX | Mi verificación | Resultado |
|---|---|---|
| Predicado canónico idéntico a `/opportunities` | `page.tsx:180-182` vs `OpportunitiesClient.tsx:289` lado a lado | **EXACTO** (`o.status !== "rejected" && o.status !== "failed"`, byte a byte en lógica) |
| viableCount del MISMO payload que bestNet | `page.tsx:171-182`: `nets` y `viableCount` derivan del mismo array `opportunities` de `getHomeData` | **CONFIRMADO** (misma población que el max — coherencia interna) |
| Label declara expectativa | `page.tsx:218` = `"Mejor Topological Yield · neto esperado"` | **CONFIRMADO**; grep en TODO el source frontend: el label viejo ya no existe en ninguna copia |
| Subtext con denominador vivo | `page.tsx:222` = `` `esperado · candidatas pre-gate · ${viableCount} viables` `` | **CONFIRMADO** (hermano del calificador del feed, `page.tsx:293`) |
| Verde condicionado a ≥1 viable | `page.tsx:225` = `variant={bestNet != null && viableCount > 0 ? "success" : "default"}` | **CONFIRMADO** — 0 viables Y ventana vacía pierden el verde |
| +4 tests SSR, 4/4 PASS | `npx vitest run app/__tests__/page.hero-h3.test.tsx` (corrida propia) | **4/4 PASS** |
| Tests no-vacíos (auditoría de aserciones) | `StatCard.tsx:62-94`: label vive en `<span>…</span>` (el `not.toContain("…· neto</span>")` del test 1 es significativo); la clase de variante se aplica en el div de valor DESPUÉS del label → alcanzable dentro del slice de 400 chars; el vecino "accent" usa `--primary-2` (`StatCard.tsx:66`), no `--success` → sin falso positivo | **TESTS REALES, no barrera de teatro** |
| tsc sin errores | `npx tsc --noEmit` | **EXIT 0** |
| Lint limpio | `npx eslint` sobre los 4 archivos tocados | **EXIT 0** |
| Suite completa sin regresiones mías | `npx vitest run` completa, exit capturado sin pipe | **1145/1146 PASS, exit 1** — única falla `components/__tests__/ControlBoard.test.tsx` (casing "SOLO 'shadow'…" = territory CB-03 en vuelo; cero overlap de archivos/imports con H-3; los tres fixers H-2/H-3/H-4 ya la reportaban pre-existente) |
| Satélites StatCard/GateSection solo-import | Leídos completos: `StatCard.tsx:3-7`, `GateSection.tsx:1-5` = comentario + `import * as React`, cero cambio de comportamiento | **CONFIRMADO** |
| Marcadores WO-H3 | `page.tsx:9-13, 176-182, 210-216` · `StatCard.tsx:3` · `GateSection.tsx:1` · header del test | **PRESENTES** |
| RULE 00 | viableCount deriva del payload real; los fixtures/vi.mock viven SOLO dentro del test (harness estándar del repo, sin tocar el path productivo) | **SIN VIOLACIÓN** |

## 3. Evidencia primaria: el fix NO está desplegado (1 request)

`curl https://arbx.ape-tv.net/` → HTTP 200; el RSC payload del hero servido HOY dice, textual:

```
"label":"Mejor Topological Yield · neto","value":"—","subtext":"sin datos — feed vacío",
"variant":"success","animate":false
```

Tres lecturas: (a) el fix vive solo local (NO-GIT intacto — correcto); (b) el root-cause del
auditor R8 era exactamente este `variant:"success"` estático — el hero desplegado está VERDE
con value "—" ahora mismo; (c) el mismo payload muestra `"subtext":"stream
arbx:opps:detected"` (H-4 viejo, también pendiente de deploy) y el banner "Feed no
disponible — snapshot del servidor falló" → **corroboración desde afuera del outage PG
(disco lleno) que H-4 §5 escaló operator-gated, sin gastar acceso VPS**.

## 4. Gaps RESTANTES (persisten tras el fix)

1. **G1 — subtext del hero declara causa falsa cuando el feed está poblado sin nets**
   (agent-fixable). `page.tsx:220-224`: con `bestNet == null` el subtext es SIEMPRE
   `"sin datos — feed vacío"` — pero bestNet también es null cuando la ventana tiene filas
   cuyos nets no fueron computados (rechazos pre-económica: UnknownTokenPrice,
   TokenNotAllowed, etc. — población abundante según la taxonomía de rechazos). En ese estado
   el hero dice "feed vacío" mientras el feed de abajo renderiza cards: misma categoría R8-c
   que H-2 arregló para el statcard hermano (`page.tsx:261-267` hace el split
   `detectedCount > 0`). **H-2 hand-offeó exactamente esto a H-3 en `H-2-FIX.md` §6.1 y no
   fue adoptado ni declarado en los residuales de H-3-FIX.** Fix: el mismo split
   (`detectedCount > 0 ? "sin datos — nets no computados en el feed" : "sin datos — feed vacío"`).
2. **G2 — bestNet sigue siendo max sobre TODAS las filas (viables + rejected)**
   (operator-gated por semántica). `page.tsx:171-175`: en ventana mixta el número verde puede
   estar driven por una candidata rejected; el denominador declara la población pero no el
   driver. Declarado por el propio H-3 (§6.1) como decisión de operador: re-scopar a
   viables-only cambia la semántica de la métrica (un refinamiento razonable una vez el
   operador elija).
3. **G3 — no-desplegado** (operator-gated, NO-GIT). Verificado con evidencia primaria (§3):
   la DApp pública sigue mostrando el hero verde viejo (y el H-4/H-2 viejos). El deploy pasa
   por el pipeline de PRs del operador; además hoy el PG está caído por disco lleno, así que
   cualquier browser-verify post-deploy requiere primero la acción de disco (H-4 §5).

Observaciones NO-gap (documentadas para que nadie las persiga): pluralización "1 viables"
(cosmético, pinnado por test 2); StatCard animado renderiza "$0" en pre-mount SSR
(comportamiento pre-existente repo-wide de la animación, no tocado por H-3).

## 5. Confirmaciones / refutaciones a pares

- **CONFIRMO H-2-FIX §6.1**: el hand-off del split del subtext del hero sigue VIVO (G1) —
  H-3 no lo adoptó. Es el único punto donde el fix de H-3 queda por detrás del estándar que
  el propio H-2 estableció 3 líneas más abajo en el mismo archivo.
- **CONFIRMO H-3-FIX §6.1/§6.3**: ambos residuales persisten tal como fueron declarados
  (G2/G3) — honestos, sin overclaim.
- **CORROBORO H-4-FIX-APPLY §5** desde afuera: banner "Feed no disponible" en la home pública
  = PG caído vivo al momento de esta verificación (sin gastar ssh).
- **Nada refutado**: las cifras de H-3-FIX (tests, tsc, lint, suite) reprodujeron exactas.

## 6. Clasificación de claims

- Predicado canónico viabilidad: **CANONICAL_REPO** (verificado lado a lado, §2).
- Fix local completo y sin regresiones: **PRIMARY_SOURCE** (corridas propias: vitest 4/4,
  tsc 0, eslint 0, suite 1145/1146).
- Hero desplegado viejo/verde + feed caído: **PRIMARY_SOURCE** (RSC payload del dominio,
  §3).
- Alcance del estado "feed poblado sin nets": **INFERRED** de la taxonomía de rechazos
  documentada por la mesa (memoria rejection-taxonomy 2026-09-06: 48.4K/24h 100% rejected,
  mayormente pre-económica) — el estado es alcanzable, hoy no es el estado observable porque
  el feed está caído, no porque sea imposible.
