# FE-02a · DESIGN — Integridad de datos cross-page (mitad A: datos/lógica)

> **Agente:** ecc:typescript-reviewer (Gang Omniscience, misión frontend-doctrine 2026-09-07)
> **WO:** FE-02a · kind: **design** (CERO edición de código de producción — diffs especifícanse abajo)
> **Charter:** rubric typescript-reviewer (contratos de datos, tipado, lógica de transformación) aplicada a
> la caza de bugs de INTEGRIDAD DE DATOS en el pipeline `app/opportunities` → `lib/useWebSocket.ts`/`useOmniOpportunities`
> → `websocket-client.ts`/socket-lifecycle → `OpportunityTradeCard`/`OpportunityDetailDialog` → formatters/filtros.
> **Lexicon OMEGA:** TLS (flash loan) · Holonomic Loop Resolution (triangular) · Topological Yield ·
> Decoherencia de Estado (slippage) · Variedad de Liquidez (pool/DEX).

---

## 0. Estado del árbol (declaración R8 obligatoria)

- Branch: `feat/hops-live-01` @ `27aca289` (HOPS-LIVE-01 — top-K ciclos RU-3 al pipeline canónico).
- El árbol está **git-dirty**, pero NINGÚN archivo bajo mi claim está sucio: `websocket-client.ts` y
  `RuntimePostureBar.tsx` (sucios en el snapshot pre-sesión) fueron committeados en `325e3154`
  ("WO-01+WO-08 — WS client escucha new_opportunity + badge socket honesto"). Los archivos sucios
  actuales (`.claude/*`, `backend/api-server/src/edge-parity.test.ts`, `edge/worker/src/index.ts`,
  submódulos OZ, untracked `frontend/app/control/`+`ControlBoardLed.tsx` del programa CB) son de
  programas ajenos a FE-02a — auditados de solo lectura, no tocados.
- **VPS (solo lectura, 0 requests al dominio público — presupuesto público intacto):**
  - SHA desplegado `e65040f1` ≠ HEAD local `27aca289` → **HOPS-LIVE-01 NO está desplegado** y aun así
    el feed YA lleva triangular (ver §2.1) — la población multihop es anterior al deploy.
  - **D-11 SIGUE ROTO AHORA** (`journalctl cloudflared` 12:52:12Z): upgrades WS a
    `arbx.ape-tv.net/socket.io/` fallan con `originService=http://localhost:5173` (Next no puede
    servir el upgrade socket.io; nginx :80 con el location `/socket.io/` queda fuera de la ruta).
    Remediación = operador (dashboard CF), pendiente desde 2026-09-06. **Consecuencia: en el dominio
    público /opportunities corre hoy en POLLING** (WS ×3 errores → degradado), lo que activa el
    mecanismo de truncamiento BUG-02 como feed completo.

---

## 1. Resumen ejecutivo — tabla de bugs

| # | Severidad | file:line | Defecto | Alimenta |
|---|---|---|---|---|
| BUG-01 | **CRITICAL** | `frontend/app/opportunities/OpportunitiesClient.tsx:40-50` | `routeKeyOf` colapsa TODO ciclo cerrado a UNA React key (8,288 filas/30 min comparten key idéntica — probado en PG vivo) → React dropea/sobrescribe cards hermanas | **BR-11** |
| BUG-02 | HIGH | `OpportunitiesClient.tsx:213`, `lib/store/useOmniOpportunities.ts:120`, `app/opportunities/page.tsx:14` | Ventana de adquisición `limit=50` (SSR sin limit → default 50) + ventana server 5 min `ORDER BY detected_at DESC` — bajo ráfaga de filas con USD, el multihop sale de las newest-50 y la UI nunca declara el truncamiento (R8) | **BR-11** |
| BUG-03 | HIGH | `lib/store/useOmniOpportunities.ts:208-213` + migraciones 025/107 + `omni-store.ts:340-353` | El push WS entrega la **fila cruda PG** (`row_to_json(NEW)`) SIN `simulated_*`/`leg_symbols`/`token_info`; el upsert REEMPLAZA y **borra la economía enriquecida** que el snapshot REST sí traía. En modo LIVE no hay snapshot periódico → toda card que llega por WS vive empobrecida ("—" en el dinero) | **BR-11** |
| BUG-04 | HIGH | `components/OpportunityTradeCard.tsx:539-583` | El comparador de `React.memo` omite TODO el bloque `simulated_*` + `paper_status` + `risk_score`: un UPDATE que solo cambia el dinero (el propósito exacto de la migración 107) **no re-renderiza** — la card congela cifras viejas | **BR-11** |
| BUG-05 | MEDIUM | `OpportunitiesClient.tsx:90` + `components/OpportunityDetailDialog.tsx:20-44` | El diálogo renderiza el objeto congelado al click (no re-selecciona del store): el X-Ray no ve updates de la fila abierta | BR-11 (UX) |
| BUG-06 | MEDIUM | `components/OpportunityTicker.tsx:42-43,51` | Ticker dropea TODA fila sin USD (`profit===null → null`) y **fabrica** `profit*0.1` como pseudo-yield (RULE 00); estado vacío con razón FALSA ("No topological convergence detected" cuando hay filas sin precio) | R8/RULE 00 |
| BUG-07 | LOW | `lib/websocket-client.ts:226` | `useHotOpportunities` setea `connected=true` al llamar `connect()` (optimista, no en el evento `connect`) — hoy latente (solo tests usan ese hook) | higiene WS |
| BUG-08 | LOW | `lib/store/omni-store.ts:313` + `features/opportunities/socket-lifecycle.ts:70-81` | `wsStatus` jamás muestra CONNECTING: initial `DISCONNECTED` → badge "IDLE" durante handshake/reconnect (nadie llama `connectStream()`) | R8 |
| BUG-09 | MEDIUM | `OpportunitiesClient.tsx:408-423` | Empty state "SCANNING MEMPOOL IN REAL-TIME" sin razón cuando la ventana de 5 min está vacía en POLLING (hoy la realidad del dominio público) | R8 |
| BUG-10 | LOW | `lib/schemas.ts:42-112` | `OpportunityRowSchema` (consumidores `getValidated`): sin `route_metadata`/`leg_symbols`/`cartridge_id` (strip silencioso) y con `amount_in_wei`/`chain_id` required — una sola fila futura malformada envenena el payload completo (all-or-nothing). Hoy latente: columnas NOT NULL, 0 nulls en 1 h (verificado PG) | deriva de contrato |
| GAP-11 | HIGH | `OpportunityTradeCard.tsx:410,413-430` + `backend/searcher-rs/src/size_optimizer.rs:968,1229` | **Financiamiento**: la card etiqueta TODA fila "Flash loan in (TLS)" incondicionalmente; `financing_mode` (OWN_CAPITAL/AAVE_FL/BALANCER_FL/V2_FLASH_SWAP) se computa en el kernel de sizing pero **no se persiste ni viaja en el wire** → la UI no puede mostrar el modo real bajo ningún nombre | **BR-11** (aceptación operador) |
| GAP-12 | HIGH | `OpportunityTradeCard.tsx:413-430` | **Waterfall por hop sin dinero en la card**: los hops renderizan `value={null}` siempre; `deriveLegLedger` (`lib/store/types.ts:613`) existe pero solo se consume en el tab Ledger del diálogo, y sólo para filas Sized 2-leg (el kernel triangular no emite leg amounts — gap del wire, item backend) | **BR-11** |
| GAP-13 | MEDIUM | `components/opportunities/OpportunitySummaryGrid.tsx:80-86` | `amount_in_wei` (el dinero CONFIGURADO) sólo vive en un tooltip como wei; la celda "in" usa exclusivamente `simulated_amount_in_usd` | BR-11 |

**Cadena root-cause BR-11 (no re-derivada — se CONSTRUYE sobre el seed del orquestador, LEARNINGS §Cerebro
2026-09-07 y `audits/cerebro-2026-09-07/GOAL-WORKORDERS.md:53`):** la API es inocente; en el frontend
conviven (a) colapso de keys que materializa 1 card por ruta-ciclo [BUG-01], (b) ventana newest-50 que
bajo ráfaga USD (pares USDT del flood XEN/AGLD — ver §2.2) desplaza multihop del único frame que el
cliente lee [BUG-02, letal mientras D-11 mantenga el feed en POLLING], y (c) el dinero que SÍ aparece
sobre cards enriquecidas se borra/congela con el siguiente push crudo [BUG-03+04]. La percepción
"cuando aparecen valores USD los hops>2 desaparecen" = la ráfaga de filas cotizadas activando (a)+(b)
simultáneamente.

---

## 2. Evidencia (file:line + PG/cloudflared vivo, 2026-09-07 ~13:45Z)

### 2.1 Población multihop y colapso de keys (PG read-only, container `arbitragex-v2-postgres-1`)

Distribución de hops (15 min, `route_metadata <> '{}'` — 100 % de las filas llevan topología):

```
hops=2: 40,772   hops=3: 5,536   hops=4: 30   hops=5: 126
```

Grupos de colisión de `routeKeyOf` (30 min) — `chain|strategy|token_in|token_out|dex_a|dex_b`:

```
1|triangular|0xc02a…cc2(WETH)|0xc02a…cc2(WETH)|uniswap-v2|''    → 8,288 filas MISMA key
1|dex_arb   |WETH|USDT|UniswapV3|PancakeSwap V3                 → 8,070
1|dex_arb   |USDC|USDT|UniswapV2|UniswapV3                       → 4,263
… (7 grupos >3,800 filas c/u)
```

Todo ciclo cerrado tiene `token_in === token_out` y `dex_b` vacío en triangular → la key degenera a
`(chain, "triangular", token, dex_a)`. `opportunities.map()` produce **miles de children con la misma
key**: React reconcilia por key y dropea/sobrescribe hermanos — el grid materializa UNA card mutante
por grupo. Nota de datos: hops=5 (126) > hops=4 (30) en esta ventana — sin impacto FE, se reporta al Cerebro.

### 2.2 Ventana de adquisición (simulación exacta del frame cliente, PG 5 min)

```
newest-50:  50 filas · 6 triangular · 1 con expected_profit_usd
newest-100: 100 filas · 13 triangular · 3 con expected_profit_usd
```

Hoy la representación es proporcional (12 % vs 13 %) porque apenas hay filas cotizadas en ventana. El
mecanismo es **dependiente de ráfaga**: a tasa histórica del flood (183/s), las newest-50 se vuelven
~100 % flood-cotizado en segundos y la cuota multihop del frame colapsa a 0 — exactamente "cuando
aparecen valores USD, los hops>2 desaparecen". El cliente NUNCA pide más de 50
(`opportunities-live.ts:652` clampa 1..200; server `WHERE detected_at >= NOW()-300s ORDER BY
detected_at DESC LIMIT $1`) y la UI no declara el truncamiento.

### 2.3 Divergencia de contratos WS vs REST (BUG-03, por construcción)

- WS `new_opportunity` = `row_to_json(NEW)` (migraciones `025_opportunities_websocket_trigger.sql:6`
  y `107_opportunities_websocket_update_trigger.sql:33`): fila PG cruda. Columnas: SIN
  `token_in_info`, SIN `leg_symbols`, SIN `chain_base_token_symbol`, SIN `paper_status`, SIN
  `simulated_*`.
- REST `/api/v1/opportunities/live` = `opportunities-live.ts:545-622`: enriquece `token_in/out_info`,
  `leg_symbols` (:565-581), `chain_base_token_symbol`, `paper_status` (:591) y computa el bloque
  `simulated_*` (:491-543, trading_config desde Redis).
- El store hace upsert REPLACE por id (`omni-store.ts:340-353` addOpportunity / `:370-397`
  setOpportunities). En modo LIVE **no existe snapshot REST periódico** (el poll sólo corre degradado;
  el único REST es el SSR inicial y el botón manual) → cada INSERT/UPDATE por WS instala/empobrece la
  card con la forma cruda: escalera de capital "—", símbolos a shortAddr. Un UPDATE a una card
  enriquecida por SSR le BORRA el dinero mostrado.
- Divergencia adicional (INFO, sin pérdida hoy): el backend emite al MISMO room `opportunities` TRES
  nombres de evento — `new_opportunity` (puente LISTEN, `websocket.ts:448`) y
  `opportunity:detected`/`opportunity:validated` (HotStreamer `websocket.ts:952`). El page-hook
  escucha sólo `new_opportunity` (`socket-lifecycle.ts:81`); `websocket-client.ts` (WO-01) escucha
  los tres. Bifurcación de contrato esperando accidente.

### 2.4 WO-08 — verificación de regresión (PASA)

`RuntimePostureBar.tsx:103-133` (`socketChipProps`): `!wsConnected → DISCONNECTED` primero; LIVE exige
todos los canales conectados (LIVE o POLLING en superficie REST-nativa), con precedencia
disconnected>error>connecting>stale>degraded y detalle nominal — el fix WO-08 committeado
(`325e3154`, con `__tests__/RuntimePostureBar.test.tsx`) **se sostiene**; no encontré camino que
pinte LIVE en CONNECTING. Residuo menor honrado como BUG-08 (el badge del feed — OTRA fuente, el
`wsStatus` del OpportunitySlice — nunca muestra CONNECTING porque nadie llama `connectStream()`;
mostrar "IDLE" durante handshake es impreciso pero no verde-falso).

### 2.5 Hipótesis FALSADAS (charter: falsificar con evidencia)

1. **"Sort que empuja unscored bajo el fold"** — FALSADA para el cliente: ni el store ni
   OpportunitiesClient ordenan por score/USD; el orden es llegada (newest-first) y el server ordena
   `detected_at DESC` (`opportunities-live.ts:291`). El desplazamiento es por VENTANA+RATE (BUG-02),
   no por sort.
2. **"Filtro cliente por presencia de roi/usd dropea rows del grid"** — FALSADA para /opportunities:
   `mapToOmniOpportunity` no dropea nada (fail-honest por campo); el grid renderiza el store entero.
   El único drop-por-USD real es el TICKER (BUG-06) — layout chrome, no el grid.
3. **"Divergencia Zod silencia multihop en la página"** — FALSADA para /opportunities: la página NO
   parsea con Zod (mapper permisivo `types.ts:324`); el Zod (`schemas.ts`) sólo gobierna
   `getValidated` (ticker/otros) y además hoy parsea (columnas NOT NULL, 0 nulls/1 h). Riesgo latente
   registrado como BUG-10.
4. **"Virtualización/paginación trunca"** — FALSADA: no hay virtualización ni paginación en el grid;
   el truncamiento real es server-limit + cap 200 + TTL 5 min (declarados arriba).

---

## 3. DISEÑO — diffs exactos + invariante + gate

> kind: design. NINGUNO de estos diffs está aplicado. Al aplicarlos (programa de apply del
> orquestador): marca `// WO-FE-02a (2026-09-07)` en cada hunk, orden LOCAL→repo→VPS→dominio vivo,
> NO-GIT hasta gate 100 %.

### FIX-1 (BUG-01, CRITICAL) — key por ciclo completo + dedupe por ruta en render

El comentario de diseño de la card ("re-detected route → MISMA card in place") exige identidad de
RUTA; para ciclos cerrados la identidad de ruta es la TOPOLOGÍA (`route_metadata`), no
`(token_in, token_out, dex_a, dex_b)`. Y como el store retiene múltiples detecciones de la misma
ruta (re-detecciones con ids distintos), el render debe deduplicar: una card por ruta, gana la
detección más reciente.

```tsx
// frontend/app/opportunities/OpportunitiesClient.tsx
// (añadir useMemo al import de react, línea 2)

// WO-FE-02a (2026-09-07): BR-11 BUG-01 — key por ciclo completo. La key vieja
// (chain|strategy|token_in|token_out|dex_a|dex_b) colapsa TODO ciclo cerrado a
// UNA key (token_in===token_out, dex_b vacío en triangular): 8,288 filas/30 min
// compartían `1|triangular|WETH|WETH|uniswap-v2|` (PG 2026-09-07) y React
// dropeaba/sobrescribía hermanos — "los hops>2 desaparecen".
function routeKeyOf(opp: OmniOpportunity): string {
  const rm = opp.route_metadata;
  if (rm && rm.dex_adapters.length > 0) {
    return [
      opp.chain_id,
      opp.strategy_kind ?? "",
      ...rm.dex_adapters,
      ...rm.pool_addresses,
    ].join(">");
  }
  // §29 fallback: sin topología persistida se conserva la key legada.
  return [
    opp.chain_id,
    opp.chain_id_out ?? "",
    opp.strategy_kind ?? "",
    opp.token_in,
    opp.token_out,
    opp.dex_a,
    opp.dex_b ?? "",
  ].join("|");
}

// WO-FE-02a (2026-09-07): una card POR RUTA (no por detección). El store puede
// retener N detecciones de la misma ruta (ids distintos); sin esto React vuelve
// a ver keys duplicadas. Gana la detección con detected_at mayor; sin fecha
// parseable NO puede reclamarse "más nueva" → prevalece la ya elegida (R8).
function latestByRoute(opps: OmniOpportunity[]): OmniOpportunity[] {
  const byRoute = new Map<string, OmniOpportunity>();
  for (const opp of opps) {
    const k = routeKeyOf(opp);
    const prev = byRoute.get(k);
    if (prev === undefined) {
      byRoute.set(k, opp);
      continue;
    }
    const tPrev = prev.detected_at == null ? NaN : Date.parse(prev.detected_at);
    const tNext = opp.detected_at == null ? NaN : Date.parse(opp.detected_at);
    if (!Number.isNaN(tNext) && (Number.isNaN(tPrev) || tNext > tPrev)) {
      byRoute.set(k, opp);
    }
  }
  return [...byRoute.values()];
}
```

Render (línea 435-450):

```tsx
      // WO-FE-02a (2026-09-07): render deduplicado por ruta (INV-A).
      const routeCards = useMemo(() => latestByRoute(opportunities), [opportunities]);
      <div className="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-4">
        {routeCards.map((opp) => (
          <OpportunityTradeCard
            key={routeKeyOf(opp)}
            /* …props idénticas… */
```

**Invariante INV-A (biyección render):** para todo snapshot del store,
`new Set(routeCards.map(routeKeyOf)).size === routeCards.length` (cero keys duplicadas entre
hermanos) y toda ruta con topología presente en el store tiene exactamente una card.
**Gate:** test unitario con dos filas 3-hop que comparten `(chain, kind, token_in, token_out, dex_a,
dex_b)` y difieren en el 3er pool → 2 keys distintas; dos detecciones de la MISMA ruta → 1 card con
el contenido de la más reciente.

### FIX-2 (BUG-02 + BUG-09) — ventana 200 + truncamiento declarado

```ts
// frontend/app/opportunities/page.tsx (línea 14) — WO-FE-02a (2026-09-07):
// el SSR pide la ventana completa (server clamp 200) — el default 50 era la
// mitad del frame forense del operador.
    const res = await fetch(`${EDGE_URL}/api/opportunities/live?limit=200`, {

// frontend/app/opportunities/OpportunitiesClient.tsx (línea 213)
      const url = `${EDGE_URL}/api/opportunities/live?viable_only=${viableOnly}&limit=200`;

// frontend/lib/store/useOmniOpportunities.ts (línea 120)
          `${getApiBaseUrl()}/api/opportunities/live?viable_only=${viable}&limit=200`,
```

Estado vacío con razón (reemplaza copy de `OpportunitiesClient.tsx:415-419`):

```tsx
            <p className="text-sm mt-1">
              {/* WO-FE-02a (2026-09-07): INV-C — el vacío declara su razón y el
                  truncamiento de la ventana, jamás "scanning" sobre feed vacío. */}
              {viableOnly
                ? "Sin viables en la ventana. \"Show all\" muestra también las rechazadas."
                : feedStatus === "POLLING"
                  ? "Ventana de 5 min sin filas (feed degradado a polling; límite 200 últimas detecciones)."
                  : "Ventana de 5 min sin filas nuevas (límite 200 últimas detecciones, TTL 5 min por detección)."}
            </p>
```

**Invariante INV-C (ventana honesta):** todo truncamiento (limit server, TTL 5 min, cap 200 del
store) es visible en copy o contador — nunca un vacío sin razón.
**Gate:** test del copy por estado (LIVE/POLLING × viableOnly); nota de presupuesto: payload worst-case
~200 × 2-3 KB ≈ 0.5 MB cada 5 s en POLLING — aceptable hoy, tunnable por env si FE-03 (performance)
lo reclama.

### FIX-3 (BUG-03) — el push crudo no borra la economía enriquecida

Helper puro en `lib/store/types.ts` (seam testeable, misma disciplina que `ws-ingest-buffer.ts`):

```ts
// WO-FE-02a (2026-09-07): BUG-03 — el payload WS es la fila PG cruda
// (row_to_json, migraciones 025/107): sin token_info/leg_symbols/simulated_*.
// El upsert verbatim BORRABA la economía que el snapshot REST sí trajo. Merge
// por campo: el push gana donde trae valor; lo enriquecido-sólo sobrevive
// cuando el push lo omite. null sigue siendo null (R8) — nada se inventa.
export function mergeRawPush(
  prev: OmniOpportunity | undefined,
  next: OmniOpportunity,
): OmniOpportunity {
  if (!prev) return next;
  return {
    ...next,
    token_in_info: next.token_in_info ?? prev.token_in_info,
    token_out_info: next.token_out_info ?? prev.token_out_info,
    chain_base_token_symbol: next.chain_base_token_symbol ?? prev.chain_base_token_symbol,
    leg_symbols: next.leg_symbols ?? prev.leg_symbols,
    paper_status: next.paper_status ?? prev.paper_status,
    simulated_net_profit_usd: next.simulated_net_profit_usd ?? prev.simulated_net_profit_usd,
    simulated_amount_in_usd: next.simulated_amount_in_usd ?? prev.simulated_amount_in_usd,
    simulated_roi_pct: next.simulated_roi_pct ?? prev.simulated_roi_pct,
    simulated_cost_breakdown: next.simulated_cost_breakdown ?? prev.simulated_cost_breakdown,
    simulated_target: next.simulated_target ?? prev.simulated_target,
    simulated_at: next.simulated_at ?? prev.simulated_at,
    simulated_notes: next.simulated_notes ?? prev.simulated_notes,
  };
}
```

Wiring en el hook (lectura no-reactiva legal fuera de render):

```ts
// frontend/lib/store/useOmniOpportunities.ts (líneas 208-213)
      onOpportunity: (opp) => {
        const mapped = mapToOmniOpportunity(opp as unknown as Record<string, unknown>);
        // WO-FE-02a (2026-09-07): BUG-03 — merge contra la fila ya presente.
        const existing = useOmniStore
          .getState()
          .opportunities.find((o) => o.id === mapped.id);
        buffer.upsert(mergeRawPush(existing, mapped));
      },
```

**Invariante INV-B (no-borrado silencioso):** un push WS para id X nunca deja en null un campo que el
store tenía no-null, salvo que el propio wire del push lo afirme (RULE 00: null = no computado por el
productor, no "borrado").
**Gate:** unit test — fila enriquecida (simulated_*=valores, leg_symbols) + push crudo mismo id →
campos enriquecidos preservados, campos crudos (status, expected_profit_usd) actualizados; push con
simulated_* no-null → gana el push. Contención FE: el remedio estructural (enriquecer en el puente
LISTEN o persistir el bloque) es ítem del workstream backend BR-11 (§4).

### FIX-4 (BUG-04) — el comparador de memo cubre el dinero que renderiza

```ts
// frontend/components/OpportunityTradeCard.tsx — comparador (líneas 561-582)
    return (
      p.id === n.id &&
      p.status === n.status &&
      p.expected_profit_usd === n.expected_profit_usd &&
      p.net_expected_profit_usd === n.net_expected_profit_usd &&
      p.roi_pct === n.roi_pct &&
      p.risk_score === n.risk_score &&                                   // WO-FE-02a (2026-09-07)
      p.paper_status === n.paper_status &&                               // WO-FE-02a (2026-09-07)
      p.rejection_reason === n.rejection_reason &&                       // WO-FE-02a (2026-09-07)
      p.simulated_net_profit_usd === n.simulated_net_profit_usd &&       // WO-FE-02a (2026-09-07)
      p.simulated_amount_in_usd === n.simulated_amount_in_usd &&         // WO-FE-02a (2026-09-07)
      sameJson(p.simulated_cost_breakdown, n.simulated_cost_breakdown) &&// WO-FE-02a (2026-09-07)
      sameJson(p.simulated_target, n.simulated_target) &&                // WO-FE-02a (2026-09-07)
      p.detected_at === n.detected_at &&
      /* …resto sin cambios (route_metadata/leg_symbols/dex/token logos/symbols,
         isMounted, simLoading, strategyConfig, callbacks, edad)… */
```

**Gate:** test — props idénticas salvo `simulated_cost_breakdown` (o `simulated_amount_in_usd`) →
`comparator(prev,next) === false` (re-render). Hoy el UPDATE-only-money NO re-renderiza.

### FIX-5 (BUG-05) — el X-Ray lee la fila VIVA del store

```tsx
// frontend/app/opportunities/OpportunitiesClient.tsx
  const [selectedId, setSelectedId] = useState<string | null>(null);   // WO-FE-02a (2026-09-07)
  const onInspect = useCallback((opp: OmniOpportunity) => setSelectedId(opp.id), []);
  // …
      <OpportunityDetailDialog
        opportunityId={selectedId}                                      // WO-FE-02a (2026-09-07)
        onClose={() => setSelectedId(null)}
      />

// frontend/components/OpportunityDetailDialog.tsx
export function OpportunityDetailDialog({ opportunityId, onClose }: {
  opportunityId: string | null;                                         // WO-FE-02a (2026-09-07)
  onClose: () => void;
}) {
  // Fila viva del SSOT; si la fila salió del store (TTL) se conserva la última
  // vista conocida — el Sheet no se cierra a mitad de lectura (R8: datos
  // "as-of", nunca re-inventados).
  const live = useOmniStore((s) =>
    opportunityId == null ? null : s.opportunities.find((o) => o.id === opportunityId) ?? null,
  );
  const snap = useRef<OmniOpportunity | null>(null);
  if (live != null) snap.current = live;
  const opp = live ?? snap.current;
  /* …resto idéntico… */
```

**Gate:** test — store con fila X, se abre sheet, se dispara update de X (scored→USD) → el sheet
refleja el update en el mismo render cycle.

### FIX-6 (GAP-11/12/13 — aceptación del operador) — el dinero completo en la card

(a) **Modo de financiamiento veraz.** Interim honesto (sin wire): el renglón de la escalera deja de
afirmar TLS incondicionalmente:

```tsx
// frontend/components/OpportunityTradeCard.tsx (líneas 408-412)
          <LedgerRow
            up
            /* WO-FE-02a (2026-09-07): GAP-11 — el modo de financiamiento NO
               viaja en el wire (size_optimizer.rs:968 computa financing_mode y
               no lo persiste). Interim honesto: si el bloque simulado trae
               TLS fee > 0 la ruta se financió con flash; si no, se declara no
               emitido — jamás se afirma OWN_CAPITAL sin wire (RULE 00). */
            label={
              flashFee != null && flashFee > 0
                ? `Financiamiento TLS (flash fee $${flashFee.toFixed(2)})${opp.chain_base_token_symbol ? ` · ${opp.chain_base_token_symbol}` : ""}`
                : `Capital in${opp.chain_base_token_symbol ? ` · ${opp.chain_base_token_symbol}` : ""} (modo financiamiento: no emitido en el wire)`
            }
            value={capitalInUsd}
          />
```

Cuando el backend persista `financing_mode` (§4): chip VERBATIM con el token
(`OWN_CAPITAL|AAVE_FL|BALANCER_FL|V2_FLASH_SWAP`) — sin traducción que esconda la configuración
(palabra del operador), solo el sufijo físico "(TLS)" para los modos flash.

(b) **Waterfall por hop con dinero real.** La escalera consume `deriveLegLedger` (ya existe,
`types.ts:613`) y humaniza con `weiToHuman` (hoist del dialog a un util compartido
`lib/store/types.ts` o `lib/format.ts` — mismo código, export):

```tsx
          {legs.length > 0 ? (
            legs.map((l) => {
              // WO-FE-02a (2026-09-07): GAP-12 — dinero por hop cuando el
              // ledger existe (filas Sized); si no, el renglón degradado actual.
              const legLedger = ledger?.[l.index];
              return (
                <LedgerRow
                  key={l.index}
                  label={`Hop ${l.index + 1}/${legs.length} · ${legSym(l.token_in)}→${legSym(l.token_out)}`}
                  value={null}
                  muted={!legLedger}
                  hint={
                    legLedger
                      ? `${l.dex || "—"} · in ${weiToHuman(legLedger.amount_in_wei, rm?.decimals?.[l.token_in.toLowerCase()]) ?? legLedger.amount_in_wei} → out ${weiToHuman(legLedger.amount_out_wei, rm?.decimals?.[l.token_out.toLowerCase()]) ?? legLedger.amount_out_wei}`
                      : `${l.dex || "—"}${l.synthetic ? " · syn" : ""}`
                  }
                />
              );
            })
          ) : /* …rama actual sin cambios… */ }
          {/* WO-FE-02a (2026-09-07): GAP-12 — entrega final del ciclo: Δ en wei
              del token base SOLO en el hop de cierre (deriveLegLedger ya lo
              restringe); humano con decimals del token base; verbatim si no. */}
          {ledger != null && ledger[ledger.length - 1]?.cycle_delta_wei != null && (
            <LedgerRow
              up={!ledger[ledger.length - 1].cycle_delta_wei!.startsWith("-")}
              down={ledger[ledger.length - 1].cycle_delta_wei!.startsWith("-")}
              label="Entrega final (Δ ciclo)"
              value={null}
              hint={`${weiToHuman(ledger[ledger.length - 1].cycle_delta_wei!, rm?.decimals?.[rm.token_addresses[0]?.toLowerCase()]) ?? ledger[ledger.length - 1].cycle_delta_wei!} ${opp.chain_base_token_symbol ?? ""}`.trim()}
            />
          )}
```

(INV-D: **cero conversión wei→USD en el FE** — el USD por hop exige ancla USD del wire por token, que
no existe; se muestra el monto EXACTO humanizado por leg y el Δ de cierre. `const ledger =
deriveLegLedger(opp)` y `const rm = opp.route_metadata` arriba, junto a `const legs = deriveLegs(opp)`
línea 214.)

(c) **Dinero configurado visible** (GAP-13): la celda "in" del SummaryGrid añade la forma humana del
`amount_in_wei` configurado cuando `route_metadata.decimals` conoce los decimals del token base
(mismo `weiToHuman`); sin decimals → el wei verbatim ya vive en el title (hoy) y se añade al value
como texto mono corto. Sin fabricar USD.

**Gate:** snapshot tests de la card con fixture triangular 3-hop con ledger + sin ledger (dos estados
honestos); assertion de NO-existencia: el string "Flash loan in (TLS)" no aparece en fixture sin
flashFee.

### FIX-7 (BUG-06) — ticker fail-honest

```ts
// frontend/components/OpportunityTicker.tsx
function opportunityToTickerItem(opp: OpportunityRow): TickerItem | null {
  const profit = opp.net_expected_profit_usd ?? opp.expected_profit_usd ?? null;
  // WO-FE-02a (2026-09-07): BUG-06 — la fila SIN precio NO se dropea: el
  // marquee muestra la detección con yield pendiente. Dropearla mintió el
  // estado vacío ("no convergence detected" habiendo filas).
  const pair = opp.pair_symbol ?? `${opp.token_in.slice(0, 6)}…/${opp.token_out.slice(0, 6)}…`;
  const from = opp.dex_a ?? "Unknown";
  const to = opp.dex_b ?? opp.dex_a ?? "Unknown";
  // WO-FE-02a (2026-09-07): RULE 00 — eliminado el pseudo-yield `profit*0.1`
  // (fabricación): roi_pct del wire o null ("—").
  const yieldPct = opp.roi_pct ?? null;
  return { pair, from, to, yield: yieldPct, ago: formatAgo(opp.detected_at) };
}
```

Render: `item.yield == null ? "—" : …toFixed(2)%`; y el estado vacío cambia a razón verdadera:
`"Feed activo — N detecciones sin precio aún (scoring pendiente)"` cuando items>0 pero todos sin
precio (contando las dropeadas antes del map). **Gate:** unit test — 3 filas (1 con roi, 2 sin) → 3
items, 2 con "—"; 0 con precio → copy "sin precio aún", NO "no convergence".

### FIX-8 (BUG-08) — CONNECTING real en el badge del feed

```ts
// frontend/lib/store/useOmniOpportunities.ts — dentro del effect del socket
// (tras obtener wsUrl válido, antes de createOpportunitySocket):
    setWsStatus("CONNECTING"); // WO-FE-02a (2026-09-07): el consumidor YA está
    // montado y handshaking — "IDLE" era el estado de "sin consumidor".
```

(socket-lifecycle podría además emitir CONNECTING en `reconnect_attempt`; mínimo = el set inicial.)
**Gate:** test — mount del hook con WS_URL válido → wsStatus CONNECTING antes del primer connect.

### FIX-9 (BUG-07, higiene latente) — `useHotOpportunities`

`websocket-client.ts:226`: mover `setConnected(true)` al callback `socket.on("connect")` del cliente
(exponiendo `onConnected(cb)` o aceptando el logger-hook). Hoy sólo lo usan tests — sin producción
expuesta; se repara para que el día que se monte no mienta. **Gate:** test con ioFactory falso que
nunca conecta → `connected === false`.

### FIX-10 (BUG-10, deriva de contrato) — `OpportunityRowSchema`

Añadir opcionales-permisivos para lo que el wire YA lleva y el Zod strippea:
`route_metadata: z.record(z.unknown()).nullable().optional()`, `leg_symbols` ídem,
`cartridge_id: z.string().nullable().optional()`. NO relajar los required actuales (columnas NOT
NULL — el required es un tripwire de deriva, se conserva). **Gate:** test parse con payload vivo
forma opportunities-live (fixture del propio route test) — ítems preservan route_metadata.

---

## 4. Dependencias BACKEND declaradas (NO son FE-02a — alimentan la aceptación BR-11)

1. **Persistir + servir `financing_mode`** (GAP-11): el dato vive en
   `backend/searcher-rs/src/size_optimizer.rs:968,1229` (`crate::financing::selected_mode`) y hoy sólo
   alcanza logs de rechazo (sufijo `:financing_mode`) y telemetría route_discovery. Se necesita
   columna/wire + SELECT en `opportunities-live.ts` + trigger payload. El FE ya está diseñado para
   consumirlo verbatim (FIX-6a).
2. **Ledger por leg del kernel triangular** (GAP-12): `attach_leg_ledger` (shared-rs) sólo emite para
   filas Sized 2-leg V2/V3 (`types.ts:117-130` documenta el contrato all-or-nothing). El waterfall
   USD-por-hop para triangular exige que el kernel emita `leg_amounts_in/out` por hop (el FE ya
   renderiza cuando existe — FIX-6b).
3. **(Estructural para BUG-03)** enriquecer el payload del puente LISTEN en
   `backend/api-server/src/index.ts:1847-1863` (o persistir el bloque simulated) para que el stream
   lleve la misma riqueza que el REST. El FIX-3 es contención; éste es el remedio.
4. **D-11 (operador)**: apuntar el public hostname del tunnel a `localhost:80` — sin WS el feed
   público corre POLLING y BUG-02 es el frame completo. (LEARNINGS §D-11; verificado vivo §0.)

---

## 5. Plan de verificación (LOCAL solamente — NO-GIT, sin deploy)

1. `npx tsc --noEmit` (workspace frontend) — cero errores.
2. `npx vitest run` sobre los suites afectados + nuevos:
   `frontend/lib/store/__tests__/` (mergeRawPush, latestByRoute/routeKeyOf, regresión 200-cap+TTL),
   `frontend/components/__tests__/` (comparador card, dialog vivo, ticker honesto, empty-state copy),
   `frontend/lib/__tests__/schemas` (FIX-10). Windows AppControl: correr vitest directo (patrón
   memoria del proyecto).
3. Invariantes automáticas: INV-A (biyección keys), INV-B (no-borrado), INV-C (vacío con razón),
   INV-D (cero wei→USD derivado).
4. Validación de dominio vivo (pasos 3-4 del orden sagrado): responsibilidad del orquestador con
   browser Chromium tras apply+deploy — criterios: grid con >1 card triangular simultánea, escalera
   con dinero persistente tras push WS, ticker sin "—" fabricado.

## 6. Sincronía de mesa

- **Construye sobre:** seed BR-11 del orquestador (LEARNINGS §Cerebro 2026-09-07;
  `audits/cerebro-2026-09-07/GOAL-WORKORDERS.md:53`) — API inocente; NO re-derivado, confirmado y
  LOCALIZADO en el frontend con mecanismo múltiple.
- **Alimenta:** BR-11 (BUG-01..05, GAP-11..13) · CB-03 /control (BUG-08/09: el estado del feed que
  el Control Board audita debe declarar ventana y CONNECTING real; WO-08 verificado PASA §2.4 —
  citado para `audits/control-board-2026-09-07/CB-01-VERIFY.md` si lo retoman) · FE-03 doctrina
  (FIX-1 reduce renders por dedupe de rutas; presupuesto payload declarado en FIX-2).
- **NO cubre** (mitad B — FE-02b): hydration, a11y, overflow, memoria. No existe reporte FE-02b al
  cierre (dir `audits/frontend-doctrine-2026-09-07/` sólo contiene GOAL-WORKORDERS.md) — nada que
  citar ni duplicar.
- **Datos curiosos para el Cerebro:** hops=5 (126/15 min) > hops=4 (30/15 min) — asimetría a
  investigar en RU-3; todo el feed 100 % con route_metadata ≠ '{}'.

— ecc:typescript-reviewer · WO FE-02a · 2026-09-07 · read-only sobre árbol dirty declarado (§0).
