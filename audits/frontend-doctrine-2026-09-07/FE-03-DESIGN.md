# FE-03 — DOCTRINA DE ARQUITECTURA ENTERPRISE-PREMIUM PARA ESTA DApp

- **WO**: FE-03 · kind: **design** · **Agente**: ecc:performance-optimizer (Gang Omniscience)
- **Fecha**: 2026-09-07 · **Read-only sobre código de producción: CERO edición. CERO git. CERO VPS-mutación.**
- Todos los diffs de este documento son **PROPUESTAS marcadas `// FE-03 (2026-09-07)`**, NO aplicadas. Los aplica
  el orquestador vía PR (ola por ola, §8) tras adopción de la mesa en FE-04.
- **Claims bajo mi custodia** (nadie más los toca): `frontend/lib/store/**`, `frontend/lib/websocket-client.ts`,
  `frontend/lib/useWebSocket.ts`, `frontend/lib/api-client.ts`, `frontend/app/layout.tsx`,
  `frontend/package.json`, `frontend/next.config.js`.
- **Clasificación de evidencia** (regla de la mesa): `[CANONICAL_REPO]` = file:line leído por mí en este árbol;
  `[PRIMARY_SOURCE]` = verificado contra código servidor/migración; `[PEER]` = hallazgo de un par citado,
  no re-derivado; `[INFERRED]` = mi deducción declarada; `[UNKNOWN]` = no verificado.

---

## 0. Sincronía de mesa redonda (obligatoria — leída ANTES de diseñar)

**Leídos completos**:
- `audits/frontend-doctrine-2026-09-07/GOAL-WORKORDERS.md` (board de este programa).
- `.claude/skills/arbitragex-omniscience/LEARNINGS.md` (ledger de experiencia; **NO existe un LEARNINGS.md en la
  raíz** — fail-honest: el único ledger vivo es ese archivo + la MEMORY del operador).
- Pares: `audits/cerebro-2026-09-07/GOAL-WORKORDERS.md` (BR-00/BR-11), `audits/control-board-2026-09-07/CB-VERIFY-FRONTEND-VERIFY.md`
  (ambas mitades), `audits/omniscience-integration-2026-09-06/03-api-ws.md` (N3 — capa WS del api-server).
- **AUSENTES al momento de escribir**: `FE-01` (censo 0→57 páginas), `FE-02a/FE-02b` (caza de bugs cross-page),
  y cualquier otro reporte FE-*. El charter decía "si los reportes FE-02a/FE-02b ya existen, incorpóralos
  citando" — **no existen**; esta doctrina se diseña contra el pipeline verificado por mí (§1) y quedará
  reconciliada por FE-04 cuando FE-02 aterrice.

**Afirmaciones de pares que este diseño ADOPTA** (y de las que depende):
- `[PEER]` **N3/03-api-ws §0-§4**: el gateway socket.io vive sano en 8080 (rooms `opportunities`/`metrics`/
  `convergence`/`telemetry`/`route_discovery`/`runtime_ack`), PERO el dominio público corre el transporte
  **degradado a HTTP-polling** (CF tunnel → :5173 directo; el rewrite de Next es HTTP-only; nginx — que sí
  tiene el upgrade — quedó fuera de la ruta; remediación D-11 = operador). LEARNINGS `[ARBX]` lo confirma.
  → **Consecuencia para el Pilar A**: la doctrina de delta es OBLIGATORIAMENTE transport-agnóstica (el mismo
  `applySnapshotAndDeltas` sirve un push WS y un snapshot poll; §2.6).
- `[PEER]` **N3 §3**: streams hot `arbx:hot:detected`/`arbx:hot:simulated` VACÍOS (XLEN=0) → los eventos
  `opportunity:detected`/`opportunity:validated` que escucha `HotOpportunityWebSocket` **nunca disparan hoy**.
- `[PEER]` **BR-11 (cerebro GOAL, fila BR-11)**: la API live es INOCENTE — el "desaparecen hops>2 al aparecer
  USD" es un filtro/orden del FRONTEND. → **Consecuencia para el Pilar C**: todo filtro/orden que oculte filas
  debe DECLARAR cuántas oculta y por qué (§4.3); prohibido el empty/fold silencioso.
- `[PEER]` **CB-VERIFY-FRONTEND (ambas mitades)**: el patrón R1 (page Server Component puro → `useState(initialSnapshot)`
  → no-determinismo sólo en `useEffect`) ya está verificado PASS en /control con tests de markup estático
  (renderToStaticMarkup, 41/41) — esa convención de test es la que adoptan mis gates.

**Refutación/refinamiento a un par** (deber de la mesa): CB-VERIFY §6 registra el dominio `/control` → 404
esperado por NO-GIT. Nada que refutar. A **N3 §4** lo refino en un punto: su frase "el frontend corre sin
`INTERNAL_API_URL`" describe el contenedor, no el código — `next.config.js:88` tiene default
`http://api-server:8080`, así que el rewrite SIEMPRE apunta bien; la degradación es del transporte
(polling vs upgrade), no del target. No cambia su conclusión operativa.

---

## 1. Forense del pipeline REAL (la base de evidencia de los 5 pilares)

### 1.1 Transporte y eventos — qué llega y con qué granularidad `[CANONICAL_REPO]`+`[PRIMARY_SOURCE]`

| Evento/canal | Fuente real | Granularidad del delta | Payload | Quién lo consume HOY |
|---|---|---|---|---|
| `new_opportunity` (room `opportunities`) | PG `LISTEN opportunities_channel` (`backend/api-server/src/index.ts:1849-1854`) ← trigger `trg_notify_opportunity` (INSERT, migración 025) + `trg_notify_opportunity_update` (UPDATE, **migración 107** `database/migrations/107_opportunities_websocket_update_trigger.sql`, guard `WHEN (OLD.* IS DISTINCT FROM NEW.*)`) → `broadcastOpportunity` (`websocket.ts:447-448` `io.to('opportunities').emit('new_opportunity', opp)`) | **Fila completa, una por fila que REALMENTE cambió** (detección de cambio a nivel fila EN EL SERVIDOR; un UPDATE no-op nunca notifica). INSERT = nueva detección; UPDATE = economía computada / transición de status / valores de paper-execution | `row_to_json(NEW)` — fila completa de `opportunities` | `createOpportunitySocket` (`features/opportunities/socket-lifecycle.ts:76-81`) → `mapToOmniOpportunity` → buffer 1 Hz → `setOpportunities` |
| `opportunity:detected` / `opportunity:validated` | HotStreamer (Redis streams `arbx:hot:*`) | evento | `HotOpportunityEvent` | **NADIE en el feed vivo** — sólo `HotOpportunityWebSocket` (muerto, §1.2). Streams VACÍOS hoy `[PEER] N3 §3` |
| `route_discovery_telemetry` (room `route_discovery`) | Redis pub/sub `arbx:route_discovery:telemetry` | tick de loop (~30s/canal, presupuesto staleness 90s — `lib/store/realtime-slices.ts:166-171`) | 5 tipos de evento, discriminator `event`; sólo `.tick` pasa la Zod (`acceptTickPayload`, realtime-slices.ts:123-138) | `ArbxRealtimeProvider` (montado UNA vez en `app/layout.tsx:143`) → TelemetrySlice |
| `runtime_ack` (room homónimo, gate admin server-side `websocket.ts:313-324`) | broadcast POST-driven | evento | `RuntimeAckBroadcast` (Zod 1:1) | `ArbxRealtimeProvider` → RuntimeAckSlice |
| rooms `metrics` / `convergence` / `prices` / cartridge `telemetry` | gateway socket.io (rooms existen `[PEER] N3 §1`) | `[UNKNOWN]` cadencia exacta | `[UNKNOWN]` shapes no auditados aquí | hooks bespoke con su PROPIO `io()`: `lib/hooks/usePricesStream.ts:186`, `useConvergenceStream.ts:72`, `useCartridgeTelemetry.ts:75` |
| HTTP `GET /api/opportunities/live` | edge | snapshot 50 filas | `items[]` | TRES vías independientes: fallback poll 5 s (`useOmniOpportunities.ts:34,116-149`), refresh manual 4 s (`OpportunitiesClient.tsx:57,211-232`), y `OpportunityTicker` del layout con su propio fetch 30 s (`components/OpportunityTicker.tsx:71-89`) |
| HTTP `GET /api/quote/anchor`, `/api/pairs` | edge (REST-only, EMIT-02/06) | snapshot TTL 35 s (presupuesto staleness 105 s) | Zod validado | `ArbxRealtimeProvider` → QuoteAnchorSlice / PairsSlice |

**Hechos estructurales que la doctrina respeta**:
1. **El delta YA nace con fuente real del servidor** — migración 107 emite sólo filas que cambiaron. El trabajo
   restante del cliente es completar la cadena: deduplicar replays/overlaps (ya lo hace `ws-ingest-buffer.ts`,
   Map por id, última llegada gana el contenido) y **no escribir en el store cuando nada cambió** (hoy sí
   escribe: §1.3).
2. **El payload es fila-completa, no campo** — el diff POR CAMPO lo computa el cliente en la costura de ingesta
   (función pura `diffOmniOpportunity`, §2.3). No requiere cambio de backend ni de trigger.
3. **El transporte público hoy es POLLING** `[PEER] N3 §4` — el diseño debe funcionar idéntico en WS-live,
   WS-polling-degradado y poll puro. El snapshot de reconciliación post-reconnect ya es idempotente por el
   UPSERT por id existente (`omni-store.ts:325-354`).
4. Cap 8 KB de `pg_notify` (comentario migración 107) — la fila-completa cabe hoy; si un día no cabe, se recorta
   EN EL TRIGGER (doctrina del archivo), nunca se ensancha en silencio.

### 1.2 Inventario de patrones de estado — los N actuales `[CANONICAL_REPO]`

| # | Patrón | Dónde | Consumidores | Veredicto doctrina |
|---|---|---|---|---|
| 1 | **zustand omni-store** (slices: registry, opportunities, wallet, runtime×4, catalogs×3, quote, realtime) | `lib/store/*` | opportunities + exchange + home + 7 paneles | **CANÓNICO** (§3) |
| 2 | R1 snapshot local `useState(initialSnapshot)` + fetch en `useEffect` | ~14+ `*Client.tsx` (`grep initialSnapshot app/` = 26 archivos incl. páginas/tests) | cada página dueña de datos no compartidos | LEGÍTIMO como SIEMBRA del store o para datos page-local; vocabulario FetchStatus obligatorio (§3.4) |
| 3 | @tanstack/react-query 5.101.4 | `package.json:24` | SÓLO dentro de wagmi `Web3Provider` (`app/providers/Web3Provider.tsx`, `app/client-only/Web3Provider.tsx`) — **cero `useQuery` de datos de app** (`grep useQuery` = 2 hits, ambos Web3) | motor interno de wagmi; NO adoptar para datos de app (evitar 2ª capa de caché sobre el store) |
| 4 | Hooks stream bespoke con `io()` propio + `useState` | `lib/hooks/`: `usePricesStream`, `useConvergenceStream`, `useCartridgeTelemetry`, `useOpportunitiesStream`, … (16 archivos) | paneles sueltos | consolidar bajo la política del provider (ola 5, §8) |
| 5 | `HotOpportunityWebSocket` (clase) + `useHotOpportunities` + `adaptNewOpportunityToHotEvent` | `lib/websocket-client.ts` | **CERO consumidores** fuera de su propio test (`grep` app/components/features = 0; el feed vivo usa `socket-lifecycle` + `mapToOmniOpportunity`). Escucha eventos que NUNCA disparan (§1.1) | **DEAD — eliminar** (ola 1). Es un mapper PARALELO al vivo (`adaptNewOpportunityToHotEvent` vs `mapToOmniOpportunity`) = riesgo de drift silencioso |
| 6 | `useWebSocket<T>` genérico (native WebSocket + ring buffer 100) | `lib/useWebSocket.ts` | **CERO consumidores** (`grep from "@/lib/useWebSocket"` = 0). Además es native-WS contra un gateway socket.io = handshake incompatible por diseño | **DEAD — eliminar** (ola 1) |
| 7 | `lib/statemachine/useActionState`, `useRuntimeAckSocket` | statemachine | /control board, runtime ack | niche legítimo |
| 8 | `user-prefs` (localStorage `arbx_prefs`) + `admin-token` (localStorage/sessionStorage) | `lib/user-prefs.ts`, `lib/admin-token.ts` | preferencias/token | legítimo (R1: lecturas sólo en useEffect) |

### 1.3 Qué re-render dispara HOY `OpportunityTradeCard` (la cadena completa) `[CANONICAL_REPO]`

`/opportunities` hoy (`app/opportunities/page.tsx` SSR fetch → `OpportunitiesClient.tsx:81` selector del ARRAY
→ grid `.map` línea 436-449):

1. **Flush WS 1 Hz** (`useOmniOpportunities.ts:41,179-185`) → `setOpportunities(batch)` → merge por id con
   **rebuild del array completo** (`omni-store.ts:370-397`: nuevo array SIEMPRE que el batch tenga ≥1 fila) →
   **nueva referencia de `opportunities`** → `OpportunitiesClient` re-renderiza (sus ~15 bloques de header,
   contadores, banners) → `.map` de las 200 tarjetas → el **comparador memo de CADA tarjeta corre** (200×/s en
   burst) → sólo re-renderizan las tarjetas con campos visuales cambiados. El comparador
   (`OpportunityTradeCard.tsx:539-584`) ya es field-level + bucket de edad (`Object.is(agePrev, ageNext)`) +
   `sameJson` para `route_metadata`/`leg_symbols` — **el diff por campo YA existe a nivel tarjeta**; lo que NO
   existe es a nivel store/lista.
2. **Ticker 30 s de `now`** (`OpportunitiesClient.tsx:283-286`, corregido de 1 s por PERF 2026-08-09) → cambia
   la prop `now` de TODAS las tarjetas → todas re-renderizan cuando su bucket de edad cambia (≈ todas cada 30 s).
3. **Poll 5 s degradado / refresh manual** → mismo camino que el flush (merge por id).
4. **`OpportunityTicker` del layout** (`layout.tsx:139`) — feed independiente (fetch 30 s propio), NO comparte
   store con el grid: quinta vía de datos para la misma tabla `[CANONICAL_REPO] OpportunityTicker.tsx:71-89`.

**Costo estructural residual** (el que el Pilar A elimina): un cambio de UN número de UNA fila produce hoy
1 rebuild de array + 1 render del container de la página + 200 ejecuciones de comparador + 1 render de la
tarjeta entera (≈15 sub-bloques). La doctrina lo reduce a **1 escritura de métrica + 1 render de UNA hoja**
(§2.4). Precedentes de daño real que esto previene: MEM-RENDER-01 (tab edge 5.3 GB heap; ticker 1 s
re-renderizaba 200 cards/s — memoria del operador, corregidos por flush 1 Hz + memo + framer-motion fuera del
grid `OpportunityTradeCard.tsx:36-40`).

### 1.4 Versiones reales (insumo del Pilar E) `[CANONICAL_REPO]` `frontend/package.json`

`next 14.2.35` (:35) · `react`/`react-dom` **18.3.1** (:38-39) · `zustand 5.0.15` (:50) ·
`@tanstack/react-query 5.101.4` (:24) · `socket.io-client 4.8.3` (:42) · `tailwindcss 4.3.0` (:45) ·
`framer-motion ^11.18.2` (:31, presente pero fuera del grid vivo) · `typescript ^5.4.5` (:59) · `vitest 3.2.6`.

---

## 2. PILAR A — Patrón canónico de datos: **Snapshot SSR + Delta Quirúrgico por Campo (SDQ)**

### 2.1 La regla en una frase

> La página nace de un **snapshot SSR veraz** (R1); a partir de ahí, cada evento del servidor se reduce a un
> **diff por campo** en la costura de ingesta, y **sólo el campo que cambió se escribe**; el DOM se actualiza por
> **unidad hoja** (`<DeltaValue>`): UN número cambiado = UNA hoja re-renderizada. La lista se re-renderiza
> únicamente cuando cambia el ORDEN (entradas/salidas), jamás por un número.

### 2.2 Estado del store: normalizado en tres planos `[diferencia clave vs hoy]`

Hoy `opportunities: OmniOpportunity[]` mezcla orden + contenido + números en un array. La doctrina separa:

- `order: string[]` — identidad y posición de las tarjetas. Referencia ESTABLE salvo entrada/salida/TTL.
- `entities: Record<id, OmniOpportunity>` — contenido frío (identidad, topología `route_metadata`, símbolos,
  leg ledger por hop — lo que cambia rara vez y cuesta re-renderizar).
- `metrics: Record<id, OppHotMetrics>` — **los números calientes**: lo único que el feed actualiza en una fila
  viva. Cada registro lleva `v` (versión de fila: +1 por escritura real).

Campos calientes canónicos (los que existen en el wire y mutan post-detcción — `lib/store/types.ts:265-303`):
`net_usd` (`net_expected_profit_usd`), `gross_usd` (`expected_profit_usd`), `roi_pct`, `sim_in_usd`
(`simulated_amount_in_usd`), `status`. Todo lo demás es frío (la aparición de `simulated_cost_breakdown` /
`simulated_target` / leg ledger es un evento FRÍO de una vez: pasa de null a objeto y congela).

### 2.3 El diff por campo — función pura en la costura (INVARIANTE RULE 00 aquí)

```ts
// ── frontend/lib/store/types.ts — PROPUESTA, NO APLICADA ──────────────────────
// FE-03 (2026-09-07) — PILAR A §2.3: delta por campo en la costura de ingesta.
// Entrada: fila vieja del store + fila nueva del servidor (WS new_opportunity
// [mig 107: sólo filas que cambiaron] o snapshot poll/SSR). Salida: el diff
// declarado — JAMÁS un valor interpolado/extrapolado. Un campo ausente en el
// payload llega null por el mapper §28 (mapToOmniOpportunity) y aquí se
// propaga tal cual: null = "no computado" (R8), NUNCA placeholder.

/** Métricas calientes por fila — los números que el feed actualiza. */
export interface OppHotMetrics {
  /** Versión de fila: +1 por CADA escritura real (delta no vacío). */
  v: number;
  net_usd: number | null;      // net_expected_profit_usd — Topological Yield neto
  gross_usd: number | null;    // expected_profit_usd
  roi_pct: number | null;
  sim_in_usd: number | null;   // simulated_amount_in_usd (capital configurado, TLS)
  status: OpportunityStatus | null;
}
export type HotMetricField = Exclude<keyof OppHotMetrics, "v">;

export interface OmniDelta {
  /** true si nada observable cambió — el caller NO escribe nada (flush no-op). */
  empty: boolean;
  /** true si cambió un campo FRÍO → hay que reemplazar la entidad completa. */
  cold: boolean;
  /** sólo los campos calientes que cambiaron (para merge parcial de metrics). */
  hot: Partial<OppHotMetrics>;
}

/** Inicializa las métricas calientes de una fila recién ingerida. */
export function hotMetricsOf(opp: OmniOpportunity): Omit<OppHotMetrics, "v"> {
  return {
    net_usd: opp.net_expected_profit_usd,
    gross_usd: opp.expected_profit_usd,
    roi_pct: opp.roi_pct,
    sim_in_usd: opp.simulated_amount_in_usd,
    status: opp.status,
  };
}

/**
 * Diff por campo, puro y total. `sameJson` para objetos anidados usa la misma
 * disciplina del comparador de la tarjeta (contenido serializado, objetos
 * chicos: route_metadata/leg_symbols/token_info).
 */
export function diffOmniOpportunity(
  old: OmniOpportunity | null,
  next: OmniOpportunity,
): OmniDelta {
  if (old == null) {
    return { empty: false, cold: true, hot: hotMetricsOf(next) };
  }
  const hot: Partial<OppHotMetrics> = {};
  if (old.net_expected_profit_usd !== next.net_expected_profit_usd)
    hot.net_usd = next.net_expected_profit_usd;
  if (old.expected_profit_usd !== next.expected_profit_usd)
    hot.gross_usd = next.expected_profit_usd;
  if (old.roi_pct !== next.roi_pct) hot.roi_pct = next.roi_pct;
  if (old.simulated_amount_in_usd !== next.simulated_amount_in_usd)
    hot.sim_in_usd = next.simulated_amount_in_usd;
  if (old.status !== next.status) hot.status = next.status;

  // Campos fríos que la tarjeta/campo de detalle renderizan: identidad de ruta,
  // topología, símbolos, ledger por hop (BR-11), notas de simulación.
  const sameJson = (a: unknown, b: unknown): boolean =>
    JSON.stringify(a) === JSON.stringify(b);
  const cold =
    old.chain_id !== next.chain_id ||
    old.strategy_kind !== next.strategy_kind ||
    old.detected_at !== next.detected_at ||
    old.dex_a !== next.dex_a ||
    old.dex_b !== next.dex_b ||
    old.token_in !== next.token_in ||
    old.token_out !== next.token_out ||
    old.amount_in_wei !== next.amount_in_wei ||
    old.rejection_reason !== next.rejection_reason ||
    old.paper_status !== next.paper_status ||
    old.block_number !== next.block_number ||
    old.simulated_net_profit_usd !== next.simulated_net_profit_usd ||
    old.simulated_roi_pct !== next.simulated_roi_pct ||
    !sameJson(old.route_metadata, next.route_metadata) ||
    !sameJson(old.leg_symbols, next.leg_symbols) ||
    !sameJson(old.simulated_cost_breakdown, next.simulated_cost_breakdown) ||
    !sameJson(old.simulated_target, next.simulated_target) ||
    !sameJson(old.simulated_notes, next.simulated_notes) ||
    old.semantic_violations.length !== next.semantic_violations.length;

  return { empty: Object.keys(hot).length === 0 && !cold, cold, hot };
}
// ── fin PROPUESTA ─────────────────────────────────────────────────────────────
```

**INVARIANTE FE-03-A1 (RULE 00/R8)**: el delta JAMÁS inventa números. Los únicos escritores de
`entities`/`metrics` son filas provenientes del servidor (WS `new_opportunity`, poll `/api/opportunities/live`,
semilla SSR); el diff sólo PROPAGA lo que el servidor mandó. Un delta sin fuente (flush con cero cambios:
replays, overlap poll/WS, snapshot idéntico) produce **cero escrituras** — el flush no-op ES la observación
vacía R8, se cuenta (`deltaWrites` no incrementa) y jamás se rellena con un valor "fresco" fabricado. La
ausencia sigue siendo `null` (contrato §28 del mapper: `mapToOmniOpportunity`, `types.ts:324-438`).

### 2.4 Escritura del store y unidad de update DOM quirúrgica

```ts
// ── frontend/lib/store/omni-store.ts — PROPUESTA (slice opportunities), NO APLICADA ──
// FE-03 (2026-09-07) — PILAR A §2.4: store normalizado + merge delta.
//   order    : referencia ESTABLE salvo entrada/salida → la lista NO se
//              re-renderiza por un número.
//   entities : sólo la fila con campo FRÍO cambiado recibe nueva referencia.
//   metrics  : merge parcial por fila + bump de versión v.
interface OpportunitySlice {
  order: string[];
  entities: Record<string, OmniOpportunity>;
  metrics: Record<string, OppHotMetrics>;
  /** Instrumentación del gate D: escrituras reales (deltas no vacíos). */
  deltaWrites: number;
  /** Flushes que no produjeron escritura (replays/overlaps) — observación R8. */
  flushNoOps: number;
  applySnapshotAndDeltas: (rows: OmniOpportunity[]) => void; // FE-03: sustituye a setOpportunities en el flujo vivo
  pruneStale: (maxAgeMs: number) => void;                    // adaptado a order/entities
  clearOpportunities: () => void;
  setWsStatus: (status: WsStatus) => void;
  // …(conectStream/disconnectStream/addOpportunity quedan como compat de ola 2)
}

// dentro de storeFactory:
applySnapshotAndDeltas: (rows) =>
  set((state) => {
    let writes = 0;
    let noOp = 0;
    const newIds: string[] = [];
    // Copias PEREZOSAS: se materializan sólo al primer cambio real. Un flush
    // totalmente no-op (replays/overlap) no asigna NADA — cero presión de GC,
    // cero referencias nuevas (gate A2).
    let entitiesNext: Record<string, OmniOpportunity> | null = null;
    let metricsNext: Record<string, OppHotMetrics> | null = null;

    for (const row of rows) {
      const old = state.entities[row.id];
      if (old === row) { noOp++; continue; }          // misma referencia (replay del buffer)
      const d = diffOmniOpportunity(old ?? null, row);
      if (d.empty) { noOp++; continue; }              // R8: delta vacío → CERO escritura
      writes++;
      if (d.cold || old == null) {
        entitiesNext ??= { ...state.entities };
        entitiesNext[row.id] = row;
      }
      const base = state.metrics[row.id] ?? { v: 0, ...hotMetricsOf(row) };
      metricsNext ??= { ...state.metrics };
      metricsNext[row.id] = { ...base, ...d.hot, v: base.v + 1 };
      if (old == null) newIds.push(row.id);
    }

    if (writes === 0) {
      return { flushNoOps: state.flushNoOps + noOp }; // el store de datos queda INTACTO
    }
    const entities = entitiesNext ?? state.entities;  // hot-only flush: entities intacto

    let order = state.order;                           // estable por defecto
    if (newIds.length > 0) {
      const kept = state.order.filter((id) => entities[id] != null);
      order = [...newIds, ...kept].slice(0, MAX_OPPORTUNITIES);
      for (const dropped of [...newIds, ...kept].slice(MAX_OPPORTUNITIES)) {
        delete entities[dropped];   // hot-only flush nunca entra aquí (newIds=0)
        delete metricsNext![dropped];
      }
    }
    return {
      entities,
      metrics: metricsNext!,
      order,
      deltaWrites: state.deltaWrites + writes,
      flushNoOps: state.flushNoOps + noOp,
      lastUpdate: new Date().toISOString(),
    };
  }),
// ── fin PROPUESTA ─────────────────────────────────────────────────────────────
```

La **unidad de update DOM quirúrgica** — la hoja que subscribe a UN primitivo:

```tsx
// ── frontend/components/opportunities/DeltaValue.tsx — PROPUESTA, NO APLICADA ──
// FE-03 (2026-09-07) — PILAR A §2.4: hoja quirúrgica. El selector devuelve un
// PRIMITIVO → zustand re-renderiza ESTA hoja y sólo ésta cuando ese número
// cambia. Es el átomo de "sólo se actualiza el número que cambió".
"use client";
import { useOmniStore } from "@/lib/store/omni-store";
import type { HotMetricField, OppHotMetrics } from "@/lib/store/types";

export function DeltaValue({
  id,
  field,
  format,
}: {
  id: string;
  field: HotMetricField;
  format: (v: number | OppHotMetrics["status"] | null) => string;
}) {
  const value = useOmniStore((s) => s.metrics[id]?.[field] ?? null);
  return (
    <span data-delta={`${id}:${field}`} suppressHydrationWarning>
      {format(value)}
    </span>
  );
}
// ── fin PROPUESTA ─────────────────────────────────────────────────────────────
```

Y la **tarjeta se suscribe por id** (el padre deja de pasar `opp` como prop de datos):

```tsx
// ── fragmento de OpportunitiesClient.tsx + OpportunityTradeCard.tsx — PROPUESTA, NO APLICADA ──
// FE-03 (2026-09-07) — PILAR A §2.4:
// (1) el grid mapea SOLO el orden — el container de página re-renderiza sólo
//     cuando cambia `order` (entradas/salidas), no en cada flush:
const order = useOmniStore((s) => s.order);
…
{order.map((id) => (
  <OpportunityTradeCard key={routeKeyOfId(id)} id={id} /* callbacks memoizados */ />
))}

// (2) dentro de la tarjeta: suscripción por fila (referencia de entidad) +
//     hojas DeltaValue para cada número del summary grid / ledger / header:
const opp = useOmniStore((s) => s.entities[id]);     // re-render de la tarjeta SOLO por campo frío
if (opp == null) return null;                        // salida del TTL entre flushes — honesto
…
<DeltaValue id={id} field="net_usd" format={formatProfitUSD} />
<DeltaValue id={id} field="roi_pct" format={formatPctOrDash} />
// ── fin PROPUESTA ─────────────────────────────────────────────────────────────
```

Resultado medible de la cadena completa: **cambio de 1 número de 1 fila ⇒ 1 merge parcial de `metrics[id]` ⇒
1 render de 1 `<DeltaValue>`**. La página-container, el grid, las otras 199 tarjetas y el resto de los ~14
sub-bloques de la tarjeta cambiada NO se renderizan. El comparador memo existente
(`OpportunityTradeCard.tsx:539-584`) se conserva como defense-in-depth y queda casi ocioso.

### 2.5 El reloj de edad también es hoja (mata la cascada de `now`)

```tsx
// ── frontend/components/opportunities/AgeLeaf.tsx + slice clock — PROPUESTA, NO APLICADA ──
// FE-03 (2026-09-07) — PILAR A §2.5: el tick 30s vive en UN interval del
// store (clock30s); cada tarjeta renderiza <AgeLeaf> que subscribe al bucket
// derivado. HOY: la prop `now` del padre re-renderiza TODAS las tarjetas
// cuando su bucket cambia (OpportunitiesClient.tsx:283-286 + comparador
// Object.is(agePrev, ageNext)). Con la hoja: re-renderiza la hoja, no la
// tarjeta. R1: el SSR pinta "--:--:--" (isMounted gate ya existente) y el
// primer tick llega en useEffect — sin mismatch.
"use client";
export function AgeLeaf({ detected_at }: { detected_at: string | null }) {
  const now = useOmniStore((s) => s.clock30s);
  const ageSecs =
    detected_at == null ? null : Math.max(0, Math.floor((now - Date.parse(detected_at)) / 1000));
  const isStale = ageSecs == null ? null : ageSecs > STALE_SECS; // STALE_SECS=12, hoy tarjeta:94
  return (
    <span suppressHydrationWarning data-age={detected_at ?? "sin-fecha"}>
      {ageSecs == null ? "—" : `${ageSecs}s`}
    </span>
  );
}
// FE-03: escritor ÚNICO del reloj (ni página, ni tarjeta):
//   useEffect(() => { const t = setInterval(() => useOmniStore
//     .setState({ clock30s: Date.now() }), 30_000); return () => clearInterval(t); }, [])
//   — vive en el provider de la página o en ArbxRealtimeProvider (ola 2).
// ── fin PROPUESTA ─────────────────────────────────────────────────────────────
```

### 2.6 Semántica de reconnect: el snapshot reconcilia, el delta acelera (transport-agnóstico)

- WS LIVE: `new_opportunity` (fila-cambiada, mig 107) → diff → escritura quirúrgica.
- WS degradado/POLLING (la realidad del dominio público hoy, `[PEER] N3 §4`): snapshot 50 filas cada 5 s por
  la MISMA `applySnapshotAndDeltas` — el diff absorbe el overlap: filas sin cambio = no-op contado. El costo
  diferencial del snapshot vs push es O(filas·campos) de diff puro, sin renders.
- **Post-reconnect NO se asume delta**: el primer evento tras reconnect pasa por diff contra el estado local;
  si el estado local es viejo (gap), el snapshot poll del ciclo degradado ya reconcilió (UPSERT por id —
  doctrina existente `omni-store.ts:325-354` conservada en el merge delta). Nunca se "estira" un delta sobre
  un gap: sin fuente = sin escritura (R8).

**INVARIANTE FE-03-A2**: *un cambio de UN campo de UNA fila produce exactamente UNA escritura de métrica y UN
re-render de UNA hoja; la lista (y la página) re-renderizan SOLO por cambio de `order` (entrada/salida/TTL).
Un flush sin cambios produce CERO escrituras observables (contadores `deltaWrites`/`flushNoOps` lo prueban).*

**GATE FE-03-A** (verificación, convención de tests de markup estático del repo — `[PEER] CB-VERIFY §3.1`):

1. Unit (vitest, node env — el buffer ya es seam puro testable, precedente `ws-ingest-buffer.ts:1-17`):
   sembrar store con 200 filas; `applySnapshotAndDeltas(batch)` con 1 campo caliente cambiado en 1 fila ⇒
   `Object.is(before.order, after.order) === true`; `after.entities` conserva referencias de las 199 intactas;
   `after.metrics[id].v === before.metrics[id].v + 1` y las otras 199 versiones IDÉNTICAS; `deltaWrites === +1`.
2. Unit R8: batch con 200 filas idénticas ⇒ retorna estado sin tocar datos (`deltaWrites` +0,
   `flushNoOps` +200) — el flush no-op es una NO-escritura, no un refresh fabricado.
3. Unit regla de oro: campo ausente en payload → mapper lo trae null → diff lo propaga null → hoja pinta "—"
   (assert contra `formatProfitUSD(null)`), NUNCA 0 ni el valor viejo disfrazado.
4. Render: montar grid con probe-counter en hoja vs shell; disparar 1 cambio numérico; assert hoja re-render ×1,
   shell ×0, container ×0.
5. Browser (FE-05, post-apply): en feed vivo, `performance.getEntriesByType("event")`/React Profiler — commits/s
   sobre el grid ≤ presupuesto §5; visible en el atributo `data-delta` (la hoja es auditable por selector CSS).

---

## 3. PILAR B — UN patrón de estado global estándar

### 3.1 El canónico: **zustand omni-store (slices + selectores atómicos)**

Evidencia de por qué ES el canónico (no una apuesta nueva): ya es el SSOT de opportunities+exchange+home,
de realtime/católogos/quote (`lib/store/*`), ya tiene reglas de performance documentadas en su cabecera
(`omni-store.ts:11-14`: nunca `useOmniStore()` sin selector; `useShallow` para objetos), ya corre en producción
con 1098 tests verdes `[PEER] CB-VERIFY §1 1c`, y zustand 5 es la dependencia pinneada (`package.json:50`).

### 3.2 Reglas de división (la tabla de decisión — UNA página para toda la mesa)

| Tipo de estado | Hogar canónico | Anti-patrón prohibido |
|---|---|---|
| Datos de servidor compartidos por ≥2 superficies | **omni-store slice** con `FetchStatus` (`"idle"\|"loading"\|"ready"\|"error"` — vocabulario ya existente `runtime-slices.ts:36`) | useState local + fetch propio duplicado |
| Datos de servidor page-local | patrón R1: snapshot SSR → **siembra del store** o `useState(initialSnapshot)` + fetch con vocabulario `FetchStatus` | Zustand-sprawl: crear slice para dato de una sola página efímera |
| Números vivos del feed | **`metrics` normalizado + hojas `<DeltaValue>`** (§2.4) | array monolito que muta referencia por un número |
| Estado UI efímero (isMounted, diálogos, toggles, simLoading) | `useState` local | meterlo al store global |
| Preferencias del operador | `user-prefs` (localStorage, lecturas en useEffect — R1) | localStorage ad-hoc por página |
| Datos de wallet | wagmi/react-query (ya aislado en `app/wallet/layout.tsx` — B-01, `layout.tsx:107-111`) | propagar Web3 al layout raíz |
| Conexiones realtime | política del `ArbxRealtimeProvider` (canal declarado en `REALTIME_CHANNELS`) — `realtime-slices.ts:42-47` | `io()` propio por hook/panel |

### 3.3 Olas de consolidación (sin big-bang — detalle en §8)

- **Ola 1 (limpieza muerta)**: eliminar `HotOpportunityWebSocket`+`useHotOpportunities`+
  `adaptNewOpportunityToHotEvent` (`lib/websocket-client.ts`) y `useWebSocket` (`lib/useWebSocket.ts`) —
  CERO consumidores verificados (§1.2 #5/#6). Sus tests (`websocket-client.test.ts`) se retiran CON ellos
  (test de código muerto no es cobertura). Resultado: UN solo cliente de transporte por gateway
  (`socket-lifecycle` para opportunities; provider para el resto).
- **Ola 2**: slice opportunities normalizado + hojas (§2) + migración de los 3 consumidores del array
  (`OpportunitiesClient.tsx:81`, `OpportunitiesExchangeClient.tsx:69`, `HomeStoreAggregation.tsx:273`) vía
  selector derivado de migración:

```ts
// ── frontend/lib/store/omni-store.ts — PROPUESTA, NO APLICADA ──────────────────
// FE-03 (2026-09-07) — PILAR B §3.3: selector derivado de MIGRACIÓN. Reconstruye
// el array para consumidores legacy SIN que el store guarde un array vivo
// (se elimina al terminar la ola 2 — no dejarlo permanente).
export function useOpportunityList(): OmniOpportunity[] {
  const order = useOmniStore((s) => s.order);
  const entities = useOmniStore((s) => s.entities);
  return useMemo(
    () => order.map((id) => entities[id]).filter((o) => o != null),
    [order, entities],
  );
}
// ── fin PROPUESTA ─────────────────────────────────────────────────────────────
```

- **Ola 5**: los hooks bespoke de `lib/hooks/` migran a canales declarados en el provider (los rooms ya existen
  server-side `[PEER] N3 §1`: `metrics`, `convergence`, cartridge `telemetry`; `prices` `[UNKNOWN]` — auditar
  shape en la ola) o se declaran formalmente page-local con justificación en comentario canónico.
- `OpportunityTicker` (layout) pasa a leer del store (sembrado por el provider) en vez de su 5ª vía de fetch
  propia — o se declara page-local consciente del costo (decisión de FE-04).

### 3.4 Difusión estándar: ganchos con nombre, nunca `useOmniStore()` pelado

Se conserva y se hace OBLIGATORIA la regla de la cabecera del store (`omni-store.ts:11-14`): selectores
atómicos por campo; `useShallow` sólo al desestructurar objetos; ganchos exportados con nombre
(`useWsStatus`, `useTickStatus`, … patrón ya existente `omni-store.ts:471-520`). Lint alcanzable:
`no-restricted-syntax` sobre `useOmniStore()` sin argumento (regla ESLint añadible en ola 2).

**INVARIANTE FE-03-B**: *existe EXACTAMENTE UN store de datos de servidor (omni-store) con vocabulario
`FetchStatus` uniforme; cero clientes de transporte duplicados (un `io()` por gateway, propiedad del provider
o del ciclo de vida page-local documentado); cero `useOmniStore()` sin selector.*

**GATE FE-03-B** (CI, grep-level — el mismo estilo que los guardianes smoke 9):
`grep -rE "new HotOpportunityWebSocket|useHotOpportunities|from ['\"]@/lib/useWebSocket['\"]" frontend/{app,components,lib,features}` → **0 hits** tras ola 1;
`grep -rn "io(" frontend/lib frontend/components frontend/app` → sólo `socket-lifecycle.ts`, `ArbxRealtimeProvider.tsx` y ciclos page-local listados en un allowlist comentado;
`grep -rn "useOmniStore()" frontend/` → **0 hits** (selector siempre). Los tres como script
`frontend/scripts/fe03-state-lint.sh` (sketch en §8) wired al CI existente.

---

## 4. PILAR C — Estándares de superficie: skeleton uniforme, error fail-honest VISIBLE, empty CON razón

### 4.1 Estado actual (base sobre la que se estandariza — NO se inventa desde cero)

- Skeletons YA existen y son uniformes en origen: `components/skeletons.tsx` (`SkeletonPageHeader`,
  `SkeletonTable rows×columns`) + `app/opportunities/loading.tsx` (route-level streaming del skeleton mientras
  el Server Component fetchea — App Router Next 14).
- Error fail-honest YA es patrón verificado: `api-client.ts` devuelve union `{ok:true,data}|{ok:false,error}`
  con error VERBATIM (`edge HTTP {status}: {body}`, `api-client.ts:163`), y /control lo pinta en `<code>`
  con test que pina el string exacto `[PEER] CB-VERIFY §7 5b` (test:342-343).
- Empty honesto YA existe en /opportunities ("SCANNING MEMPOOL…" con razón viableOnly, `OpportunitiesClient.tsx:408-423`)
  y en OpportunitiesEmpty; pero es HETEROGÉNEO across 58 páginas `[CANONICAL_REPO] find app -name page.tsx | wc -l` = 58
  — el censo de cobertura exacto es trabajo de FE-01; esta doctrina fija el CONTRATO.

### 4.2 El contrato: toda superficie de datos es un `DataSurfaceState` con 4 variantes y nada más

```tsx
// ── frontend/components/DataSurfaceState.tsx — PROPUESTA, NO APLICADA ──────────
// FE-03 (2026-09-07) — PILAR C: la tríada canónica. Toda superficie de datos
// renderiza EXACTAMENTE una de 4 variantes. El tipo hace imposible el empty
// sin razón: la variante `empty` LLEVA `reason` obligatorio.
"use client";

export type SurfaceVariant =
  | { kind: "skeleton"; shape: "table" | "cards" | "kpi"; rows?: number; cols?: number }
  | { kind: "error"; title: string; detail: string /* VERBATIM, nunca traducido/maquillado */; endpoint?: string; onRetry?: () => void }
  | { kind: "empty"; reason: string; hiddenByFilter?: { label: string; count: number } }
  | { kind: "data" };

/** Proyección pura (testeable sin render — misma disciplina que surfaceStateOf de CB). */
export function surfaceStateOf(input: {
  status: "idle" | "loading" | "ready" | "error";
  count: number;
  error?: string | null;
  /** OBLIGATORIO cuando count puede ser 0: la razón honesta de la ausencia. */
  reasonIfEmpty: string;
  hiddenByFilter?: { label: string; count: number };
}): SurfaceVariant {
  if (input.status === "error") return { kind: "error", title: "DATA SOURCE ERROR", detail: input.error ?? "unknown error" };
  if (input.status === "loading" || input.status === "idle") return { kind: "skeleton", shape: "cards" };
  if (input.count === 0)
    return { kind: "empty", reason: input.reasonIfEmpty, hiddenByFilter: input.hiddenByFilter };
  return { kind: "data" };
}

export function DataSurfaceState({ variant, children }: { variant: SurfaceVariant; children?: React.ReactNode }) {
  switch (variant.kind) {
    case "skeleton":
      return <div role="status" aria-busy="true">{/* delega a components/skeletons.tsx por shape */}</div>;
    case "error":
      return (
        <div role="alert" className="mb-8 p-4 bg-destructive/10 border border-destructive/30 rounded-xl flex items-start gap-4 text-destructive">
          <div className="min-w-0">
            <h3 className="font-bold">{variant.title}</h3>
            <p className="text-sm mt-1"><code className="break-all">{variant.detail}</code></p>
            {variant.endpoint && <p className="text-xs mt-1 text-muted-foreground">{variant.endpoint}</p>}
          </div>
        </div>
      );
    case "empty":
      return (
        <div role="status" className="mb-8 p-4 bg-muted/50 border border-border rounded-xl text-muted-foreground">
          <h3 className="font-bold tracking-wide">SIN RESULTADOS</h3>
          <p className="text-sm mt-1">{variant.reason}</p>
          {variant.hiddenByFilter && variant.hiddenByFilter.count > 0 && (
            <p className="text-xs mt-1" data-testid="hidden-by-filter">
              {variant.hiddenByFilter.count} filas ocultas por: {variant.hiddenByFilter.label}
            </p>
          )}
        </div>
      );
    case "data":
      return <>{children}</>;
  }
}
// ── fin PROPUESTA ─────────────────────────────────────────────────────────────
```

### 4.3 Emparejamiento con BR-11 (sincronía de mesa)

BR-11 (hops>2 desaparecen cuando aparecen valores USD — el filtro vive en el FRONTEND, API inocente) es
EXACTAMENTE el anti-patrón que `hiddenByFilter` vuelve imposible de cometer en silencio:

- Todo `filter`/`sort` que pueda ocultar filas de una superficie declara su efecto visible: “N filas ocultas
  por: {nombre del filtro}” — incluido el caso sort-bajo-el-fold (si el orden esconde unscored bajo el fold,
  el header declara el criterio de orden activo).
- El vocabulario de `reasonIfEmpty` usa la taxonomía R8 del pipeline (`watchlist_empty`, `impact_zero`,
  `no_base_candidates`, …) cuando el backend la provee; si no, la razón exacta observada en el cliente
  (“fuente 0 filas en la ventana”, “WS degradado + 0 filas en snapshot”). PROHIBIDO el “No data” desnudo y
  PROHIBIDO llenar el vacío con datos decorativos (RULE 00).
- Los estados vacíos ya honestos del repo se citan como canon de migración: /operations funnel,
  reject-breakdown (24h: 48.4K/100% rejected — el vacío CON razón es información operativa, no un bug).

### 4.4 Skeletons: shape-matched por contrato

El skeleton debe pre-declarar la FORMA del dato que llega (rows/cols de la tabla real, columnas de cards) —
`SkeletonTable rows={10} columns={7}` ya lo hace; la doctrina lo hace obligatorio para CLS (presupuesto §5:
CLS ≤ 0.1). Un skeleton genérico de spinner está PROHIBIDO en superficies de datos (esconde la forma y miente
sobre la latencia restante).

**INVARIANTE FE-03-C**: *toda superficie de datos renderiza exactamente una variante {skeleton-shape-matched,
error-verbatim-visible, empty-con-razón, data}; el empty sin razón y el filtro que oculta filas sin declararlo
están prohibidos; el error se muestra verbatim con endpoint y retry.*

**GATE FE-03-C** (por superficie, en el test de la página — el patrón ya PASS en /control, generalizar):
- test “empty con razón”: fixture fuente vacía ⇒ markup contiene `SIN RESULTADOS` + `reasonIfEmpty` literal.
- test “error verbatim”: fixture `{ok:false,error:"edge HTTP 503: …"}` ⇒ markup contiene el string exacto en
  `<code>` (precedente CB test:341-347).
- test “filtro honesto”: fixture con N filas que el filtro activo oculta ⇒ `data-testid="hidden-by-filter"`
  contiene N y el label. (Este test es el guard de regresión de BR-11.)
- FE-01 (censo) audita el coverage de las 58 páginas contra este contrato — gap list para FE-04.

---

## 5. PILAR D — Presupuesto de rendimiento MEDIBLE (con precedentes y techo)

### 5.1 Presupuesto por ruta y runtime

Precedentes que fijan la severidad: MEM-RENDER-01 (tab /opportunities edge: **5.3 GB heap**; ticker 1 Hz
re-renderizaba 200 cards/s; corregidos: flush 1 Hz + pruneStale 5 min + framer-motion fuera + memo). Tasas
reales del feed: ~54–122 eventos/min en bursts de producción (comentario MEM-RENDER-01, `useOmniOpportunities.ts:37-39`),
MAX_OPPORTUNITIES=200 (`omni-store.ts:187`), TTL 5 min (`:45`).

```json
// ── frontend/perf-budgets.json — PROPUESTA, NO APLICADA ───────────────────────
// FE-03 (2026-09-07) — PILAR D: presupuestos EXPLÍCITOS por ruta + runtime.
// Baseline actual = primera medición FE-05; el techo es compromiso de regresión.
{
  "routes": {
    "/":                        { "firstLoadJSKb": 200 },
    "/opportunities":           { "firstLoadJSKb": 250 },
    "/opportunities/exchange":  { "firstLoadJSKb": 260 },
    "/operations":              { "firstLoadJSKb": 280 },
    "/wallets":                 { "firstLoadJSKb": 450, "note": "wagmi+rainbowkit aislado por B-01" }
  },
  "runtime": {
    "gridCommitsPerSecondMax": 2,
    "leafRendersPerFieldChange": 1,
    "heapMbOpportunities10minMax": 300,
    "domNodesOpportunitiesSteadyMax": 25000,
    "clsMax": 0.1,
    "lcpSecondsMaxOverTunnel": 2.5
  }
}
// ── fin PROPUESTA ─────────────────────────────────────────────────────────────
```

Justificación de cada techo:
- `firstLoadJSKb`: Next 14 App Router + zustand + socket.io-client entran cómodos en 250 KB gzip para la ruta
  del feed; /wallets hereda el paquete Web3 (por eso el aislamiento B-01 del layout es doctrina, `layout.tsx:107-111`).
- `gridCommitsPerSecondMax=2`: el flush es 1 Hz; +1 margen para tick de reloj/estado WS. Un feed vivo NO debe
  comprometer el árbol más rápido de lo que llega la verdad.
- `heapMbOpportunities10minMax=300`: 17× bajo el precedente de 5.3 GB; con 200 filas × (entity ~2 KB
  serializada + metrics ~80 B) ≈ 0.5 MB de estado de datos — el resto es DOM/React; 300 MB deja margen de
  DevTools sin normalizar el fiasco.
- `domNodesOpportunitiesSteadyMax=25000`: 200 tarjetas × ~120 nodos; con `content-visibility` (§6) el COSTE de
  render de los fuera-de-viewport tiende a 0 aunque el DOM exista; si la medición supera el techo, la ola de
  virtualización JS se justifica CON cifras (no a ciegas).

### 5.2 Cómo se mide (el presupuesto sin medición es decoración — RULE 00 del performance)

1. **Build (CI, cada PR)**: parsear la tabla de rutas de `next build` (ya la imprime) contra
   `perf-budgets.json` — script `frontend/scripts/fe03-budget-check.mjs`; falla el check si una ruta excede.
   Análisis de composición bajo demanda: `ANALYZE=true` (webpack-bundle-analyzer) para atribuir regresiones.
2. **Runtime unit (CI)**: los contadores del store (`deltaWrites`, `flushNoOps`) + test de la invariante A2
   (§2.6 gate) — el "re-render/s" queda anclado por construcción (hoja×1) y lo audita el test de render con
   probe-counter.
3. **Browser journey (FE-05, post-deploy de cada ola)**: Chromium sobre el dominio vivo: CDP
   `Performance.getMetrics` (`JSHeapUsedSize` t=0 vs t=10 min en /opportunities con feed vivo), conteo de nodos
   (`document.getElementsByTagName("*").length`), y commits observados (React Profiler programático en build de
   diagnóstico o contador de hojas instrumentado `data-delta`). Presupuesto de red del agente: el journey ES
   el viaje del browser — sin barridos manuales (regla de la mesa, 429).

**INVARIANTE FE-03-D**: *cada ruta tiene techo explícito versionado en `perf-budgets.json`; el techo se mide
en CI (build) y en el journey (FE-05); una regresión >20% sobre el techo bloquea el merge (espíritu §37:
la carga de la prueba es del cambio).*

**GATE FE-03-D**: `fe03-budget-check.mjs` verde en CI (existencia de la tabla parseada + todas las rutas bajo
techo); journey FE-05 registra heap/nodos/commits en el reporte con screenshot; el PR de cada ola incluye la
medición antes/después en su descripción.

---

## 6. PILAR E — Vanguardia SIN reescribir el mundo (verificado contra package.json REAL)

Verificado ANTES de proponer (§1.4): **React 18.3.1 / Next 14.2.35**. Tabla aplicable/no-aplicable con evidencia:

| Técnica | ¿Aplicable? | Evidencia y consequence |
|---|---|---|
| `useOptimistic` / `use()` / Form Actions de React | **NO** | React 18.3.1 (`package.json:38-39`); son React 19. Migrar exige Next 15 + react-dom 19 + eslint-config bump = "reescribir el mundo" — fuera de doctrina. **Alternativa 18.x ya viva**: el estado optimista por-card manual (`SimEvidence` en `OpportunityTradeCard.tsx:96-101,131,229-244`) es exactamente el patrón, sin API nueva. Revisión: post-decisión del operador de migrar Next 15/React 19 (jamás en un PR "de paso" — §37 P-∅). |
| `startTransition` / `useDeferredValue` | **PARCIAL — con precisión importante** | Disponibles en 18. `useDeferredValue`: SÍ para derivados caros de input (filtros/sort de tablas grandes: by-strategy/exchange typing). `startTransition` sobre escrituras zustand: **NO recomendado** — zustand 5 viaja por `useSyncExternalStore`, cuyas actualizaciones externas no son transition-safe (riesgo de tearing documentado); el flush 1 Hz ya regula el ritmo. **Hallazgo conexo `[CANONICAL_REPO]`**: `startTransition` está importado y NUNCA usado (`useOmniOpportunities.ts:18` — import muerto): la ola 2 lo ELIMINA (surgical cleanup) o lo usa sólo donde React controla el estado. |
| Streaming SSR (Suspense) | **SÍ — ya encendido en nivel ruta; profundizar a panel** | Next 14 App Router: `loading.tsx` de /opportunities ya streamea el skeleton mientras el Server Component fetchea (`app/opportunities/loading.tsx` + `export const dynamic="force-dynamic"` page.tsx:9). Doctrina: cada panel KPI lento de /operations y /readiness puede envolverse en `<Suspense fallback={shape-matched skeleton}>` para que el shell pinte ANTES de que el fetch más lento resuelva. Restricción R1/R5: el fallback y el contenido deben tener la misma forma (CLS §5). |
| Edge-cache de estáticos | **PARCIAL — el code-side ya está bien; el lever real es del operador** | `/_next/static/*` es inmutable-por-hash por defecto en Next (nada que codear). El dominio público pasa por CF tunnel → :5173 directo `[PEER] N3 §4`: la cache rule de CF para `/_next/static` es configuración del DASHBOARD de CF (operador), no del repo — se declara como recomendación D-11-adjacent, NO como diff. No proponer "output: standalone/export" (el SSR dinámico es la arquitectura). |
| `content-visibility: auto` (virtualización nativa CSS) | **SÍ — la joya vanguardia de costo ~0** | Soportado en Chromium/Edge/Firefox modernos; el repo YA usa el concepto en la tarjeta exchange (`OpportunityExchangeCard.tsx:313` "native virtualization"). El browser salta layout/paint de tarjetas fuera del viewport sin librería, sin JS, sin virtualización manual: |
| Virtualización JS (react-window/virtuoso) | **CONDICIONAL** | Sólo si la medición §5 supera el techo de DOM/nodos con `content-visibility` puesto. Hoy no hay dependencia de virtualización en el lockfile (`grep react-window|virtuoso` = 0 hits). |
| View Transitions API | OPCIONAL (nice-to-have) | `document.startViewTransition` en navegación de pestañas (/control tabs) — cero dependencias; se marca experimental-here, NO bloquea olas. |
| Server Actions para forms admin | **DEFERIDO** | Las mutaciones admin ya viajan api-client validado (Zod + admin-token + cookie httpOnly V-AT-1); migrar a Actions re-rutearía la auth — riesgo sin beneficio medible hoy. |

```css
/* ── frontend/app/globals.css — PROPUESTA, NO APLICADA ───────────────────────── */
/* FE-03 (2026-09-07) — PILAR E: virtualización nativa del grid de tarjetas.
   El navegador salta layout/paint de las tarjetas fuera del viewport.
   contain-intrinsic-size reserva el alto medio (medir en FE-05 baseline) para
   que la scrollbar no salte (CLS). Precedente: OpportunityExchangeCard.tsx:313. */
.arbx-card-grid > * {
  content-visibility: auto;
  contain-intrinsic-size: auto 320px;
}
/* ── fin PROPUESTA ─────────────────────────────────────────────────────────────
   (el grid de OpportunitiesClient.tsx:435 adopta la clase arbx-card-grid en ola 2) */
```

**INVARIANTE FE-03-E**: *cada técnica moderna se marca aplicable/no-aplicable CONTRA la versión real pinneada
(package.json), con alternativa 18.x cuando aplica la limitación; nada de la lista exige migrar React/Next para
progresar; la migración React 19 es decisión explícita del operador con PR propio.*

**GATE FE-03-E**: la tabla anterior con file:line ES el artefacto; guard CI anti-drift:
`grep -rE "useOptimistic|useFormStatus|from ['\"]react['\"]\s*\{[^}]*use\(" frontend/{app,components,lib}` → 0 hits
(mientras React 18.3.1 esté pinneado); y el budget-check §5 detecta si wagmi/next crecen de versión
(pin alert: package.json diff tocando `react`/`next` requiere re-abrir esta tabla).

---

## 7. Tabla maestra de invariantes + gates (resumen para FE-04)

| Pilar | Invariante | Gate | Dónde se verifica |
|---|---|---|---|
| A · Datos | FE-03-A1: el delta JAMÁS inventa números — sólo propaga cambios reales del servidor; delta sin fuente = observación vacía (no-op contado), ausencia = null | Unit §2.6.1-2-3 (vitest) | CI por PR |
| A · Datos | FE-03-A2: 1 campo de 1 fila ⇒ 1 escritura + 1 hoja; lista/página re-renderizan SOLO por `order` | Unit + render-probe §2.6.4 | CI por PR |
| A · Datos | Transport-agnóstico: WS push y poll snapshot usan el mismo applySnapshotAndDeltas; reconnect reconcilia por snapshot, jamás estira delta sobre gap | Unit (batch idéntico ⇒ no-op) + journey POLLING (dominio actual) | CI + FE-05 |
| B · Estado | FE-03-B: UN store de servidor (omni-store, FetchStatus uniforme); cero transportes duplicados; selector siempre | `fe03-state-lint.sh` (3 greps §3.4) | CI por PR |
| C · UI | FE-03-C: {skeleton-shape, error-verbatim, empty-con-razón, data}; filtro que oculta filas lo declara | Tests por superficie (§4.4) incl. guard BR-11 | CI por PR |
| D · Perf | FE-03-D: techo por ruta versionado; medido en build (CI) y journey (browser); regresión >20% bloquea | `fe03-budget-check.mjs` + journey FE-05 | CI + FE-05 |
| E · Vanguardia | FE-03-E: aplicabilidad verificada contra package.json pinneada; cero APIs React 19 en el árbol | grep anti-drift §6 | CI por PR |

---

## 8. Plan de aplicación incremental (olas — SIN big-bang; el orquestador adjudica PRs)

| Ola | Contenido | Riesgo | Gate de salida |
|---|---|---|---|
| 1 | **Limpieza muerta**: eliminar `lib/websocket-client.ts` (clase+hook+adaptador) y `lib/useWebSocket.ts` + sus tests; import muerto `startTransition` fuera | Bajo (cero consumidores verificados §1.2) | suite verde + state-lint grep=0 |
| 2 | **SDQ core**: `diffOmniOpportunity`+`OppHotMetrics` (types) → slice normalizado + `applySnapshotAndDeltas` + reloj store (omni-store) → `DeltaValue`/`AgeLeaf` → migración de los 3 consumidores + `useOpportunityList` derivado temporal → `arbx-card-grid` content-visibility | Medio ( superficie = opportunities+exchange+home) | gates A1/A2 + Profiler journey |
| 3 | **Contrato C**: `DataSurfaceState`+`surfaceStateOf` + adopción en las superficies top (opportunities, operations, exchange, control) + guard BR-11 (`hiddenByFilter`) | Bajo-medio (aditivo por página) | tests §4.4 por superficie |
| 4 | **Presupuesto D**: `perf-budgets.json` + `fe03-budget-check.mjs` + baseline FE-05 documentada | Bajo | budget-check verde con baseline |
| 5 | **Consolidación B**: canales del provider (metrics/convergence/telemetry ya existen server-side; `prices` auditar) + `OpportunityTicker` al store + deepening Suspense por panel | Medio | state-lint io()-allowlist=0 extra |
| — | *Post-operatorio (NO en este programa)*: migración Next 15/React 19 (reabre tabla E), CF cache rule `/_next/static` (dashboard del operador, junto a D-11), virtualización JS sólo si §5 la exige | — | — |

Cada ola = UN PR con su ID de anomalía (§37 P-∅), sus gates, y medición antes/después. Los diffs de este
documento son el material de origen; al aplicar se marcan `// FE-03 (2026-09-07)` en el archivo destino.

---

## 9. Límites honestos de ESTE reporte (RULE 00 aplicado a mí mismo)

1. **CERO código editado** — todos los diffs son propuestas dentro de este documento. CERO tsc/vitest corridos
   (no hay nada mío que compilar; los gates describen corridas FUTURAS de las olas).
2. **FE-01/FE-02a/FE-02b NO existían** al escribir (§0) — el coverage exacto de estados empty/error por página
   es afirmado como HETEROGÉNEO por muestreo propio (loading.tsx existe en /opportunities; skeletons.tsx es
   compartido), NO por censo completo. FE-04 reconciliará.
3. `[UNKNOWN]` deliberados: shapes/cadencias de los rooms `metrics`/`convergence`/`prices` (no auditados aquí —
   la ola 5 los audita ANTES de migrar); peso real del bundle por ruta (la tabla `next build` se corre en ola 4).
4. La evidencia de tasas del feed (~54-122 eventos/min, 5.3 GB heap) proviene de comentarios de código y de la
   MEMORY del operador (MEM-RENDER-01) — `[CANONICAL_REPO]` como documentación, no medición mía de hoy.
5. Navegación del dominio vivo: **0/5 requests** usados (no había nada que verificar en vivo para un design-doc
   sin apply; el journey es de FE-05 post-ola). Lectura de VPS: ninguna necesaria — N3 ya documentó el transporte.
6. Presupuesto de la mesa respetado: 0 requests HTTP, 0 mutaciones, 0 git.

**Para FE-04 (el plan de todos)**: este documento aporta (a) el patrón SDQ especificado contra el pipeline real
con diffs por campo y unidad hoja, (b) el estándar único de estado con tabla de decisión y olas, (c) el contrato
de superficie con el guard BR-11, (d) presupuestos medibles con mecanismo de medición, (e) la tabla de
vanguardia verificada contra las versiones pinneadas. Lo que FE-04 debe añadir desde SUS fuentes: la priorización
económica exacta por página (necesita FE-01) y la secuencia de PRs contra el board real.
