# WO-G2-PARITY-FIX — GAP-2 edge↔frontend parity: block_number string vs z.number() (2026-09-17, fixer gang ronda 1)

> Charter: GAP-2 del orquestador — "edge↔frontend parity break — block_number
> serialized as string vs z.number() at frontend/lib/schemas.ts:66,1182 leaves
> S-Curve/status widget in permanent 'Opportunity feed unavailable (retrying
> every 30s)' on /operations and /status; gate G2 §37 declared hole with live
> reproduction; fix = coerce/normalize (diff local, NO-GIT applies)".
> Reglas duras respetadas: RULE 00 (cero mocks), §32/§33 (cero VPS/cargo/git),
> NO-GIT (diff local solamente), diffs marcados `// WO-G2-PARITY (2026-09-17)`.

## 0. Veredicto

GAP-2 CERRADO en DOS capas (emisor + tolerante consumidor), verificado con
tests de regresión nuevos + suites completas + tsc. Causa raíz re-derivada de
primera mano (RULE 00), NO heredada del reporte del navegador.

## 1. Causa raíz (CANONICAL_REPO, re-derivada)

- `opportunities.block_number` es **BIGINT**: `database/migrations/003_opportunities.sql:19`
  (`block_number BIGINT,`).
- node-postgres devuelve **int8 como string** (comportamiento documentado del
  driver; el propio LIVE_QUERY ya lo sabe para otro campo:
  `backend/api-server/src/routes/opportunities-live.ts` — comentario del
  `window_total` "(COUNT(*) OVER ())::int ... because COUNT is bigint and
  node-postgres returns int8 as string").
- LIVE_QUERY selecciona `o.block_number` **SIN cast** (hoy :268) y el mapper
  `rowToOpportunity` lo copiaba verbatim (antes :572) mientras la interfaz
  propia `OpportunityLiveRow` declaraba `block_number: number | null` (antes
  :207) — **el tipo declarado mentía sobre el runtime**.
- El Edge Worker NO re-serializa nada: `edge/worker/src/index.ts:666`
  (`/api/opportunities/live` → `proxy(c, "/api/v1/opportunities/live",
  "arbx:cache:opps", 2)`) es pass-through con caché KV 2 s. El string nace en
  api-server; el Edge solo lo transporta (precisión sobre el
  "edge serializa block_number" del QA-WS §5.1 — el Edge es inocente).
- Frontend: `OpportunitiesLiveSchema` → `OpportunityRowSchema.block_number:
  z.number().nullable()` (frontend/lib/schemas.ts:66 pre-fix) rechazaba el
  payload COMPLETO vía `getValidated` (api-client.ts:181
  "edge response shape invalid: items.0.block_number: Expected number,
  received string") → `OpportunityTicker` (montado en el ROOT LAYOUT:
  frontend/app/layout.tsx:14,:139 → visible en TODAS las páginas, incl.
  /operations y /status) quedaba en "Opportunity feed unavailable — …
  (retrying every 30s)" (frontend/components/OpportunityTicker.tsx:132).
  Reproducción viva: FEED-SCHEMA-01 del BROWSE-QA-WS
  (`BROWSE-QA-de-WebSocket-en-vivo-…md` §5.1, screenshot
  `ws-qa-home-feed-schema-error-block-number-string.png`).

Nota de alcance (fail-honest): el charter dice "S-Curve/status widget"; el
S-Curve propiamente tal (`SCurveChart.tsx`) consume `SCurvePayload` SSR sin
block_number (grep 0 hits en operations-schemas.ts). El widget roto es el
**OpportunityTicker del root layout**, que es lo que el operador ve sobre
/operations y /status. Mismo síntoma, superficie identificada con precisión.

Segunda línea del charter (schemas.ts:1182, `AdminChainProbeResultSchema`):
auditada — su productor SIEMPRE emite number|null (`parseInt(...,16)` en
`backend/api-server/src/routes/admin-chains.ts:218`), NO hay productor vivo de
strings hoy. Se endureció defensivamente (misma clase int8-as-string) porque
el charter la nombra; pérdida del check `.int()` redundante documentada
(parseInt produce entero; NaN no puede llegar — el RPC devolvería hex o la
ruta erra antes).

## 2. Fix aplicado (working tree, NO-GIT, marcadores `// WO-G2-PARITY (2026-09-17)`)

### 2a. Emisor (la corrección de fondo — el contrato de wire se cumple)

`backend/api-server/src/routes/opportunities-live.ts`:
- Interfaz `OpportunityLiveRow.block_number` honestada a
  `number | string | null` con comentario de porqué (BIGINT → int8-as-string).
- Helper nuevo `normalizeBlockNumber(v)`: null pasa como null (R8: bloque no
  detectado ≠ bloque 0), string numérico → number, valores no
  safe-integer degradan a null (jamás NaN al wire — `JSON.stringify(NaN)`
  produciría un `null` silencioso, peor que un null explícito).
- Mapper: `block_number: normalizeBlockNumber(row.block_number)` (:594).

### 2b. Consumidor (paridad G2 + ventana de deploy-skew)

`frontend/lib/schemas.ts`:
- `BlockNumberWireSchema = z.preprocess(...)` — coerción quirúrgica de string
  numérico → number. Deliberadamente **NO** `z.coerce.number()`:
  `Number(null) === 0` fabricaría bloque 0 (violación R8). Strings no
  numéricos se dejan pasar para que `z.number()` los RECHACE (fail-honest: el
  feed entero fala con mensaje, no se dropea el campo en silencio). Regex
  `/^-?\d+$/` evita además que `Number("garbage") → NaN` atraviese
  `z.number()` (Zod 3 acepta NaN como number — verificación empírica en la
  derivación de este fix).
- :66 (`OpportunityRowSchema`) y :1182 (`AdminChainProbeResultSchema`) usan el
  helper.

### 2c. Tipado del cliente (consecuencia obligatoria de 2b)

`frontend/lib/api-client.ts`: firmas de `getValidated`/`postValidated`
`schema: z.ZodType<T>` → `z.ZodType<T, z.ZodTypeDef, unknown>`. Sin esto, la
inferencia de T colapsaba `block_number` a `unknown` en el tipo de retorno
(Input ≠ Output con el preprocess nuevo; error TS2345 en
OpportunityTicker.tsx:88). Pinneo de Input=unknown = T se infiere del OUTPUT.
Neutral para todos los schemas pre-existentes (Input==Output).

## 3. Verificación (todo local, cero git/cargo/VPS/HTTP — 0/5 requests)

| Gate | Resultado |
|---|---|
| api-server vitest `opportunities-live.test.ts` | **17/17 PASS** (13 preexistentes + 4 nuevos) |
| api-server vitest SUITE COMPLETA | **66 archivos / 851 tests PASS** |
| api-server `tsc --noEmit -p tsconfig.json` | exit 0 |
| frontend vitest `lib/schemas.test.ts` | **18/18 PASS** (13 preexistentes + 5 nuevos: 3 OpportunitiesLive + 2 AdminChainProbe) |
| frontend vitest SUITE COMPLETA | **127 archivos / 1283 tests PASS** |
| frontend `tsc --noEmit` | exit 0 |
| eslint (3 archivos tocados) | exit 0 |

Tests nuevos (regresión GAP-2):
- Backend (`opportunities-live.test.ts`, describe "WO-G2-PARITY"): mapper
  int8-string `"25995384"` → `25995384` typeof number; number sigue number;
  null queda null; `"garbage"` → null (nunca NaN al wire). Vía
  `__forTesting.rowToOpportunity` (export preexistente para regresiones).
- Frontend (`lib/schemas.test.ts`): string numérico parsea y coerciona a
  number; null permanece null; `"not-a-block"` RECHAZA el parse (fail-honest);
  AdminChainProbeResultSchema numérico/string-coerción/null.

## 4. Sincronía de mesa redonda

- Construye sobre: `BROWSE-QA-de-WebSocket-en-vivo-…md` §5.1 (FEED-SCHEMA-01,
  reproducción viva + screenshot) — este fix confirma su hipótesis de clase y
  cierra su "UNKNOWN causa raíz" (§ clasificación: era api-server vía Edge
  pass-through, no el serializador del Edge).
- No contradice hallazgos previos; precisión sobre el QA-WS: "El edge serializa
  block_number" es impreciso (el Edge NO toca el body; index.ts:666 proxy
  verbatim con caché KV 2s TTL).
- Cero colisión de archivos con pares: `frontend/lib/schemas.ts`,
  `schemas.test.ts`, `api-client.ts`, `backend/api-server/src/routes/
  opportunities-live.ts{,.test.ts}` NO estaban tocados por ningún otro WO del
  board (gitStatus al inicio: modified list no los incluye; los archivos del
  gap 4.5/4.2/4.1 — useTokenIcon, RouteDiscoveryFunnelCard, ArchivePanel,
  PipelineFunnelCard, operations-schemas — intactos por este fix).
- Para WO-06/operador (deploy-gated, NO-GIT): al deployar, recordar RULE 03
  (rebuild --no-cache frontend) Y que el fix de fondo viaja en api-server;
  con solo frontend deployado el feed YA revive (coerción 2b tolera el string
  del api-server viejo); con solo api-server deployado también (wire ya
  number). Verificación en vivo sugerida post-deploy: el ticker del root
  layout debe salir de "Opportunity feed unavailable" y
  `curl -s <edge>/api/opportunities/live | jq '.items[0].block_number | type'`
  debe devolver `"number"`.

## 5. Residuales documentados (NO tocados, fail-honest)

- `backend/api-server/src/routes/fork-status.ts:72-91` valida `block_number`
  numérico contra sim-ctl con chequeo explícito (`typeof` guard) — ya era
  defensivo, sin bug.
- `paper-trade-archiver.ts:286` escribe `opp.block_number` hacia PG como
  PARÁMETRO de INSERT (pg serializa number→int8 correctamente) — fuera de
  alcance del parity break, sin defecto.
- La retención del string en el wire para payloads cacheados en KV del Edge
  (TTL 2 s) es irrelevante: la coerción 2b cubre la ventana.
- `app_backup/` contiene copias stale de componentes (SCurveChart etc.) —
  no tocado (backup).

## 6. Clasificación de afirmaciones

- BIGINT en migration 003:19 + int8-as-string de node-postgres: CANONICAL_REPO
  (fuente leída) + comportamiento documentado del driver citado por el propio
  LIVE_QUERY.
- Edge pass-through: CANONICAL_REPO (edge/worker/src/index.ts:666 + función
  `proxy` leída).
- Síntoma y reproducción viva: PRIMARY_SOURCE (heredado del QA-WS §5.1,
  screenshot + DOM).
- "El widget roto es el OpportunityTicker del root layout, no el SCurveChart":
  INFERRED de lectura de código (layout.tsx:139 monta el ticker globalmente;
  SCurvePayload no contiene block_number).
