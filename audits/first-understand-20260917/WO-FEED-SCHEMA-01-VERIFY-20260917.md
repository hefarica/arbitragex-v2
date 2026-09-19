# WO-FEED-SCHEMA-01 — Verificación independiente (convergencia dual) (2026-09-17, fixer gang ronda 1)

> Charter: "FEED-SCHEMA-01: edge serializa block_number como string, el Zod del
> frontend espera number → feed de oportunidades REST caído (retry 30s honesto).
> Requiere mirar serializador edge (dueño WO edge/api-server)."

## 0. Veredicto

**GAP YA CERRADO por el par `WO-G2-PARITY`** (mismo defecto, charter GAP-2 del
orquestador — ver `WO-G2-PARITY-FIX-2026-09-17.md` y entrada board
GOAL-WORKORDERS.md:655-685). Este fixer NO duplicó el trabajo: ejecutó la
disciplina de convergencia dual de la mesa — re-derivación independiente de la
causa raíz, re-verificación de TODAS las citas del par contra el código, re-ejecución
de los gates, y un barrido repo-wide de la clase de defecto que el par no
documentó explícitamente. **Resultado: fix CONFIRMADO, 0 refutaciones, 1
precisión sobre la premisa del propio charter de ambos fixers.**

## 1. Premisa del charter REFUTADA (herencia compartida con el par)

El charter de ESTE WO (y el hint "edge serializa") endosa al Edge Worker como
serializador del string. **REFUTADO con lectura de primera mano** (CANONICAL_REPO):

- `edge/worker/src/index.ts:445` — `async function proxy(...)` hace
  `await fetch(upstream)` → `const body = await upstream.text()` → retorna el
  body **verbatim** (KV `put(fullCacheKey, body, ttl)` guarda el texto crudo;
  HIT lo devuelve tal cual con `c.body(cached)`).
- `edge/worker/src/index.ts:666` — `/api/opportunities/live` → 
  `proxy(c, "/api/v1/opportunities/live", "arbx:cache:opps", 2)`: pass-through
  con caché KV 2 s. El Edge NO toca el body.

El string nace en **api-server**: `opportunities.block_number` es BIGINT
(`database/migrations/003_opportunities.sql:19`, verificado: `block_number BIGINT,`),
node-postgres devuelve int8 como string, y el LIVE_QUERY lo selecciona sin cast
(`backend/api-server/src/routes/opportunities-live.ts:268` — `o.block_number,`).
Concuerda con la refutación del par (§1 de su reporte) — el QA-WS §5.1
("el edge serializa") queda precisado por DOS fixers independientes.

## 2. Fix verificado in situ (working tree, marcadores `// WO-G2-PARITY (2026-09-17)`)

| Claim del par | Re-verificación propia | Evidencia |
|---|---|---|
| Interfaz honestada `number \| string \| null` | ✓ | opportunities-live.ts:207-213 |
| `normalizeBlockNumber()` null-safe, sin NaN al wire | ✓ | :426-440 (`Number.isSafeInteger(n) ? n : null`) |
| Mapper aplica normalizer | ✓ | :594 |
| `BlockNumberWireSchema = z.preprocess(...)` en :66→:83 y :1182→:1199 | ✓ | frontend/lib/schemas.ts:11, :83, :1199 |
| Pinneo Input=unknown en getValidated/postValidated | ✓ | frontend/lib/api-client.ts:141-142, :204 |
| Tests de regresión nuevos (4 backend + 5 frontend) | ✓ | suites corridas §3 |

## 3. Gates re-ejecutados por este fixer (todo local, 0/5 HTTP)

| Gate | Resultado |
|---|---|
| api-server vitest `opportunities-live.test.ts` | **17/17 PASS** (incl. 4 nuevos WO-G2-PARITY) |
| api-server `tsc --noEmit -p tsconfig.json` | **exit 0** |
| frontend vitest `lib/schemas.test.ts` | **18/18 PASS** (incl. 5 nuevos) |
| frontend `tsc --noEmit` | **exit 0** |

## 4. Barrido repo-wide de la clase (aporte NUEVO de este fixer)

El par fixeó las dos superficies nombradas por su charter pero no barrió la
clase "Zod consume block_number" en todo el repo. Barrido propio (grep completo):

- **Consumidores Zod de block_number en frontend = exactamente 2**:
  `OpportunityRowSchema` (schemas.ts:83) y `AdminChainProbeResultSchema`
  (schemas.ts:1199) — ambos ya usan `BlockNumberWireSchema`. Clase cerrada.
- WS: `ArbxRealtimeProvider` solo escucha `connect`/`route_discovery_telemetry`/
  `runtime_ack`/`disconnect` (:153-185); ni `realtime-slices.ts` ni
  `useRouteDiscoveryTelemetry.ts` validan block_number (grep 0 hits). El evento
  WS `broadcastOpportunity` (api-server index.ts:1876, PG LISTEN→socket.io) NO
  tiene consumidor frontend de block_number — fuera de la clase.
- `fork-status.ts:72-77`: ya defensivo pre-fix (`typeof blockNumber !== "number"`
  → 404 honesto `fork_not_ready`, nunca bloque fabricado) — claim del par §5
  re-verificado CORRECTO.
- `opportunities-bridge-archiver.ts:254` y `paper-trade-archiver.ts`/:paper/executor.ts:
  block_number solo como PARÁMETRO de INSERT (escritores, no emisores al wire) —
  sin defecto de la clase.

## 5. Sincronía de mesa redonda

- Construye sobre: `WO-G2-PARITY-FIX-2026-09-17.md` (fix) y
  `BROWSE-QA-de-WebSocket-en-vivo-…md` §5.1 (reproducción viva FEED-SCHEMA-01).
- **CONVERGENCIA DUAL**: causa raíz, citas y gates reproducidos independientemente
  por este fixer — 0 discrepancias de fondo.
- Nota operativa para el operador (heredada del par, ratificada): el fix de fondo
  viaja en api-server + frontend (deploy-gated, NO-GIT cumplido; RULE 03 rebuild
  frontend). Con SOLO frontend deployado el feed ya revive (coerción tolera el
  string del api-server viejo). Verificación post-deploy sugerida:
  `curl -s <edge>/api/opportunities/live | jq '.items[0].block_number | type'` → `"number"`.
- Sin colisión de archivos: este fixer NO tocó ningún archivo de código (verificación
  pura). Los 5 archivos modificados del par permanecen intactos.
- Fail-honest operativo: durante la lectura del reporte del par se observó una
  anomalía NTFS/GitBash (el archivo `WO-G2-PARITY-FIX-2026-09-17.md` listaba en
  `ls` pero `cat`/`Get-Content -LiteralPath` devolvían ENOENT; legible SOLO vía
  pipeline `Get-ChildItem | Get-Content`). No afecta el contenido verificado;
  registrada por si un par posterior tropieza con lo mismo.

## 6. Clasificación de afirmaciones

- Edge pass-through + string nace en api-server: CANONICAL_REPO (proxy :445 leído
  completo; migration 003:19; LIVE_QUERY :268 sin cast).
- Fix presente y correcto: CANONICAL_REPO (los 5 archivos leídos con marcadores).
- Gates: artefactos reproducibles propios (salidas vitest/tsc de este fixer).
- Síntoma del feed caído: PRIMARY_SOURCE heredado (QA-WS §5.1 + screenshot).
- "Clase cerrada repo-wide": INFERRED de barrido grep exhaustivo (los 2 consumidores
  Zod + WS sin validación de block_number).

## 7. Estado

FEED-SCHEMA-01: **CERRADO y DOBLE-VERIFICADO** (fix WO-G2-PARITY + esta
verificación independiente). Cero código tocado por este fixer, cero git, cero
cargo, cero VPS, cero HTTP (0/5).
