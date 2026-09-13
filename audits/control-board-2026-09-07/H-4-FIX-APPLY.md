# FIX — H-4: statcard "Asimetrías detectadas" (Gang Omniscience, ronda 1, 2026-09-07)

- **WO**: H-4 (de `BROWSE-Auditor-R8-caza-de-deshonestidad-de-datos.md` §3 H-4 / §5 gap #4) ·
  kind: fix+verify · **Agente**: FIXER H-4
- **Fecha**: 2026-09-08 21:15–21:58 local (02:15–02:58Z 2026-09-09)
- **Estado**: **FIXED + VERIFICADO local** (NO-GIT intacto: 0 commits, 0 push, 0 PR, 0 deploy.
  VPS 100% read-only: 1 docker ps + 1 df + 1 redis XLEN; CERO mutación).

## 0. Sincronía de mesa redonda (leída ANTES de editar)

Leídos completos: `GOAL-WORKORDERS.md`, `BROWSE-Auditor-R8-caza-de-deshonestidad-de-datos.md`
(la fuente de H-4), `CB-VERIFY-FRONTEND.md`, `CB-VERIFY-FRONTEND-VERIFY.md` (grep de claims),
`CB-01-CROSS-EXAM.md` (grep). Construyo sobre:

- **BROWSE-Auditor-R8 H-4**: "el 50 es `opportunities.length` de un fetch `limit=50`
  (`page.tsx:27,90`) — es el tamaño de ventana, no un conteo del stream. Si el stream tuviera
  10K, seguiría diciendo 50. Subtext debería decir 'ventana live (limit=50)' o el endpoint
  debería exponer el total real." → **ejecuté AMBAS puntas** (ver §1).
- **Confirmación numérica propia (Redis read-only, 02:4xZ)**: `XLEN arbx:opps:detected` =
  **10.001**. El stream que el subtext viejo reclamaba contar tiene 10.001 entradas; el card
  decía 50. El auditor predijo "si tuviera 10K" — es 10K.
- **CB-VERIFY-FRONTEND / -VERIFY**: claims de archivo SOLO sobre `frontend/app/control/*`
  (CB-03). `frontend/app/page.tsx` y `backend/api-server/src/routes/opportunities-live.ts`
  sin claims al iniciar → sin conflictos de apropiación.
- **Hallazgo adicional (imposible de cuantificar con PG)**: quise groundear el total de
  ventana con `psql SELECT` y el contenedor postgres está **crash-loopeando por disco lleno**
  (ver §5 — escalado operator-gated).

## 1. El fix (ambas puntas del WO)

### Punta 1 — el endpoint ahora expone el total real (`window_total`)

`backend/api-server/src/routes/opportunities-live.ts`:

- `:278` — nueva columna en `LIVE_QUERY`: `(COUNT(*) OVER ())::int AS window_total`.
  Las window functions se evalúan ANTES del `LIMIT`, así que cuenta TODAS las filas que
  matchean el `WHERE` (ventana temporal `max_age_seconds` + filtro `viable_only`) aunque sólo
  se devuelvan las top-N. `::int` porque COUNT es bigint y node-postgres devuelve int8 como
  string. Los LEFT JOIN a `tokens` no hacen fan-out (PK chain_id+address), así que el conteo
  equivale a filas de `opportunities`.
- `:223` — `window_total: number` en `OpportunityLiveRow`.
- `:905` — envelope: `window_total: q.rows[0]?.window_total ?? 0`. 0 filas → 0
  (computado-y-exactamente-cero, R8; jamás null, jamás items.length). No filtra a los ítems
  (`rowToOpportunity` construye objeto explícito — verificado por test b).
- Costo: CERO round-trips extra (misma query). La ventana default (300s) cuenta ~centenares
  de filas vía índice de `detected_at`; el peor caso (max_age=24h, ~48K filas) es un request
  opt-in del caller. Sin cambio del edge worker (proxy JSON pasa-through, cache KV 2s intacto).

### Punta 2 — el statcard muestra el total real y el subtext es honesto

`frontend/app/page.tsx`:

- `:26` — `LIVE_FETCH_LIMIT = 50` (fuente única del parámetro URL y del label fallback).
- `:65-67` — `getHomeData` parsea `window_total` y `max_age_seconds` del envelope (ausente →
  null, nunca número fabricado — R8).
- `:143-158` — `detectedStat(windowTotal, detectedCount, maxAgeSeconds)` (helper puro
  exportado, patrón del par H-2 con `computeAvgRoiPct`): value = `windowTotal ?? detectedCount`;
  subtext declara SIEMPRE qué es el número:
  - con `window_total`: `ventana 5 min · feed muestra 50` (minutos derivados del
    `max_age_seconds` real del response, no hardcodeados);
  - sin `window_total` (API vieja/edge cache frío): `ventana live · límite 50`.
- `:230-240` — StatCard "Asimetrías detectadas" consume `detected.value/subtext`. El subtext
  `stream arbx:opps:detected` queda ELIMINADO: además del problema de ventana, era fuente
  equivocada — el número viene de la tabla PG `opportunities`, no del stream Redis
  (R7 los trata como etapas distintas que pueden divergir).

`frontend/lib/schemas.ts:119` — `window_total: z.number().optional()` en
`OpportunitiesLiveSchema` (opcional → responses de deployments previos parsean igual).

**Clasificación**: CANONICAL_REPO (el defecto, evidencia en page.tsx del audit) ·
INFERRED (semántica pre-LIMIT de window functions, standard PostgreSQL) · verificado por
tests propios.

## 2. Verificación

| Verificación | Resultado |
|---|---|
| `vitest run src/routes/opportunities-live.test.ts` (api-server) | **5/5 PASS** (nuevo) |
| `npm run typecheck` (api-server, tsc --noEmit) | **PASS** |
| `vitest run` page.statcard-h4 + page.test (H-2) + page.hero-h3 (H-3) | **19/19 PASS** |
| `tsc --noEmit -p tsconfig.json` (frontend) | **PASS** |
| Suite completa frontend | 1136/1137 — única falla `ControlBoard.test.tsx` = **WO CB-03, no mío** (panel defaults "SOLO 'shadow' exacto spawnea", archivo propio del par, cero overlap de imports con mi diff) |
| Suite completa api-server | 793/798 — 5 fallas en `websocket/readiness/canonical-knobs`; **las 3 en aislamiento: 62+2/62+2 PASS** → flakiness de carga local (3 agentes paralelos en la misma box), no código |

Tests de regresión nuevos (pinnean el contrato para siempre):

- `backend/api-server/src/routes/opportunities-live.test.ts` — (b) `window_total` fluye del
  COUNT de la fila (137 ≠ items.length 2) y NO filtra a ítems; (c) ventana vacía → 0 honesto;
  (e) LIVE_QUERY contiene `COUNT(*) OVER ()... AS window_total` (guard contra borrar la
  columna SQL mientras el fixture la inyecta). Harness = patrón rejection-breakdown.test.ts
  (fake pool por shape de SQL); fixture `chain_id=0` (fuera de `CHAIN_ID_TO_DEXSCREENER_SLUG`,
  liquidityReality.ts:95) → background token-validation sin red.
- `frontend/app/__tests__/page.statcard-h4.test.tsx` — derivación pura (137/2/300 → value 137
  "ventana 5 min · feed muestra 2"; 0 computado ≠ null; fallback API vieja; ventana
  sub-minuto → "1 min") + SSR markup: claim de stream AUSENTE en ambos modos.

## 3. Co-edición con pares (documentada, sin pisarse)

`frontend/app/page.tsx` fue editado EN PARALELO por los fixers **H-2** (roi_pct null → "—",
`computeAvgRoiPct`) y **H-3** (hero re-etiquetado + `viableCount`) — el archivo cambió en
disco dos veces durante mi sesión (avisos del editor). Resolución: releí completo, mis zonas
(getHomeData/HomeData/detectedStat/statcard "Asimetrías detectadas") NO solapan las suyas
(toXRayProps/computeAvgRoiPct/hero StatCard/Decoherencia subtext). El estado fusionado pasa
los 19 tests de los tres WOs + tsc. Sus tests (`page.test.tsx`, `page.hero-h3.test.tsx`)
mockean el envelope SIN window_total → mi fallback preserva su comportamiento esperado.

## 4. Pendiente / hand-off

1. **NO-GIT**: fix 100% local. Deploy = pipeline de PRs del operador (el browser-verifier
   post-deploy debería ver el card con "ventana 5 min · feed muestra 50" y valor real).
2. `/opportunities` (OpportunitiesClient:213) y `/opportunities/exchange` (:191) también
   consumen `limit=50` y muestran "{length} total" del store — el mismo mislabel que H-4
   pero en OTRA página; `window_total` ya está disponible en el envelope si algún WO futuro
   quiere cablearlo ahí. No lo toqué (surgical: H-4 es el statcard del home).

## 5. ESCALADO OPERADOR-GATED (rojo, 02:5xZ)

El gap #5 del R8 audit (disco 94.5%, crit 95%, 8.2 GB libres) **se materializó en outage**:

- `df -h /` en VPS: `/dev/sda1 150G 144G 0 100% /` — **0 bytes libres**.
- `arbitragex-v2-postgres-1`: **Restarting (1)** — crash-loop (no pude correr ni el SELECT
  read-only de cuantificación). PG caído = persistencia del pipeline caída; el feed
  `/api/opportunities/live` devolverá 503 `db_unavailable` (y el home mostrará honestamente
  "Feed no disponible — snapshot del servidor falló (R8 fail-honest)", por diseño).
- Acción de disco = operator-gated (acción VPS): builder prune / retención ARBX-RETENTION-01
  de nuevo / VACUUM. Yo NO muté nada (§32/§33). **Este es el motivo por el que la
  cuantificación del gap usa Redis XLEN (10.001) y no un conteo PG de ventana.**
