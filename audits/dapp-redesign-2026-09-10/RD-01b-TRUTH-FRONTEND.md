# RD-01b · TRUTH-FRONTEND — mapa del dato en las 4 superficies (2026-09-10)

> WO: RD-01b · agente PhD gang omniscience. Pares citados: RD-01a/RD-01c (backend truth),
> RD-02 (EMIT-RICH), RD-03a (UNIFY), RD-05 (CARD-LIB). Insumos consumidos: FE-01-DESIGN §3/§4,
> FE-02a-DESIGN (BUG-01..05, GAP-11..13), FE-02b-DESIGN (FE-02b-01), WO-G-6-REPORT. Todo
> file:line verificado a ojo en el árbol a HEAD `feat/hops-live-01` @ 27aca289.

## 1. Transporte por superficie (cómo llega el dato HOY)

| Superficie | SSR (Server Component) | Post-hidratación | Card que renderiza |
|---|---|---|---|
| `/` home | REST `GET ${EDGE}/api/opportunities/live?limit=50` manual fetch, sin Zod, cast `OpportunityRow[]` (`app/page.tsx:40-82`) + `GET /api/readiness/decision` | NADA de oportunidades (isla `HomeStoreAggregation` lee omni-store hidratado por ArbxRealtimeProvider: route_discovery/pairs — no opportunities). El ticker del layout es otra superficie (ver §3) | `XRayCard` (vía `toXRayProps` `app/page.tsx:89-128`) + 4 `StatCard` |
| `/opportunities` | REST `GET /api/opportunities/live` (sin params — default server) `app/opportunities/page.tsx:11-44` | `useOmniOpportunities` (`lib/store/useOmniOpportunities.ts:75-78`): WS socket.io `subscribe:opportunities` → `new_opportunity` (`features/opportunities/socket-lifecycle.ts:72,81`); fallback polling 5s al MISMO REST (`useOmniOpportunities.ts:111-149`); refresh manual con `?viable_only=${viableOnly}&limit=50` (`OpportunitiesClient.tsx:213`) | `OpportunityTradeCard` + `OpportunitySummaryGrid` + dialog `OpportunityDetailDialog`→`OpportunityDetailTabs` + `QuarantinedEventsAuditTrail` |
| `/opportunities/exchange` | REST `GET /api/opportunities/live?viable_only=false&limit=50` (`exchange/page.tsx:23-28`) | mismo `useOmniOpportunities` con viableOnly fijo `false` (`OpportunitiesExchangeClient.tsx:63-66`); polling fallback 30s (const declarada; el real es 5s del hook); + `PriceTicker`→`usePricesStream` (WS room `subscribe:prices`→`prices:snapshot`/`prices:update` + REST fallback `/api/prices/live?chain_id=`, `lib/hooks/usePricesStream.ts:170,186,201-241`) + `getTradingConfig` + `usePaperModeState` | `OpportunityExchangeCard` (2 caras: Evaluada / DetectionDiagnosticCard) + dialog |
| `/opportunities/by-strategy` | REST `GET /api/opportunities/live` sin params (`by-strategy/page.tsx:26`) | POLL 4s mismo REST con URL RELATIVA `"/api/opportunities/live"` (`OpportunitiesByStrategyClient.tsx:157`) — sin EDGE_URL, depende del rewrite same-origin; SIN WS | `StrategyGroupCard` (agregación) + registry join (`by-strategy-grouping.ts`) |

Transporte transversal (fuera de las 4): `OpportunityTicker` en el LAYOUT RAÍZ
(`app/layout.tsx:139`) — REST `getOpportunitiesLive(20)` = `/api/opportunities/live?limit=20`
via `getValidated`+Zod cada 30s (`OpportunityTicker.tsx:91-109`). `ArbxRealtimeProvider`
(layout, `app/layout.tsx:143`) — WS rooms route_discovery/runtime_ack + REST pairs/anchor;
NO opportunities.

Resolver URLs: server `INTERNAL_EDGE_URL` (Docker `http://edge:8787`) o `getApiBaseUrl()`;
browser REST = `""` same-origin → rewrites Next `next.config.js:110-139` (`/api/*`→
INTERNAL_EDGE, `/socket.io/:path*`→INTERNAL_API `http://api-server:8080`); browser WS =
same-origin (`api-client.ts:67-89`). RULE 02 OK (WS nunca via edge), con el matiz de que el
browser atraviesa el rewrite de Next, no directo a 8080.

**DOS modelos de contrato conviven** (insumo RD-05): `lib/schemas.ts:42-123`
`OpportunityRowSchema`/`OpportunitiesLiveSchema` (Zod, USADO por ticker vía getValidated;
home declara el tipo pero NO valida) vs `lib/store/types.ts:187-311` `OmniOpportunity` +
`mapToOmniOpportunity` (USADO por las 3 páginas de opportunities, raw sin Zod). El mapper
pasa `route_metadata`/`leg_symbols`/`token_*_info`/`simulated_*` verbatim; el Zod de schemas
NI LOS DECLARA (los strip si algo pasa por getValidated). RD-05 debe fusionar en un SSOT.

## 2. MATRIZ campo×superficie (dinero)

Leyenda: `/`=home, `/o`=/opportunities, `/x`=/opportunities/exchange, `/s`=/by-strategy,
`T`=ticker layout, `dlg`=solo dentro del DetailDialog (ⓘ en /o y /x). ✅=renderizado,
⚠️=parcial/tooltip, ❌=existe en el modelo pero NADIE lo renderiza, ➖=no existe en el modelo.

| Campo (fuente) | `/` | `/o` | `/x` | `/s` | `T` | `dlg` | Dónde exactamente |
|---|---|---|---|---|---|---|---|
| `expected_profit_usd` (gross) | ❌ | ✅ | ✅ | ❌ | ❌ | ✅ | /o ladder "Gross out (AMM spread)"+SummaryGrid "Gross" (`OpportunityTradeCard.tsx:167,438`); /x "Gross out (AMM)"=capital+gross (`OpportunityExchangeCard.tsx:247,256`); dlg Economics |
| `net_expected_profit_usd` (net canónico) | ✅ stat | ✅ | ✅ | ✅ | ❌ | ✅ | `/` StatCard bestNet (`page.tsx:190-194`); /o Net yield+Summary "Net"; /x "Net Yield"; /s Total profit+filas; dlg Economics |
| `roi_pct` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | `/` XRay "yield"+StatCard "Decoherencia media"; /o ROI%; /x ROI; /s Avg ROI (¡coerce null→0!); T marquee |
| `risk_score` | ❌ | ✅ | ❌ | ❌ | ❌ | ✅ | /o SummaryGrid "Risk"; dlg Economics+Gates |
| `amount_in_wei` | ❌ | ⚠️ | ❌ | ❌ | ❌ | ✅ | /o SOLO tooltip del Summary "in" (`OpportunitySummaryGrid.tsx:85`); dlg Economics "Amount In (wei)" |
| `simulated_amount_in_usd` | ❌ | ✅ | ✅ | ❌ | ❌ | ✅ | /o ladder "Flash loan in (TLS)"=capitalIn + Summary "in"; /x "Monto a invertir/prestar (TLS)" |
| `simulated_net_profit_usd` | ✅ fallback | ✅ | ✅ | ❌ | ❌ | ✅ | fallback de net en todas las caras (~$ con badge SIM) |
| `simulated_roi_pct` | ❌ | ❌ | ❌ | ❌ | ❌ | ✅ | SOLO dlg tab Simulation (`OpportunityDetailTabs.tsx:446-449`) |
| `simulated_cost_breakdown` (9 líneas) | ❌ | ✅ | ✅ (4/9) | ❌ | ❌ | ✅ | /o costRows 8 líneas (`OpportunityTradeCard.tsx:192-201`); /x Gas/LP/slip/interés(flash) (`OpportunityExchangeCard.tsx:410-425`); dlg cascada §39 completa |
| `simulated_target` (veredicto+sizing) | ❌ | ✅ | ✅ | ❌ | ❌ | ✅ | /o Target+Applied strategy config; /x "Target" + fallback capitalIn; dlg Gates (obs/req/delta/required/cap/sugerido) |
| `simulated_at` | ❌ | ❌ | ❌ | ❌ | ❌ | ✅ | dlg Simulation "Simulated At" |
| `simulated_notes` | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | **NADIE** (solo app_backup) |
| `gas_used` (unidades gas) | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | **NADIE** (USD gas sí; unidades no; solo OpportunitiesTable HUÉRFANO) |
| `net_profit_wei` | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | **NADIE** (websocket-client/apex dead code) |
| `confidence_score_bps` | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ | SOLO `/` XRayCard "% conf" (`page.tsx:105-108`) |
| `posterior_probability_bps` / `kelly_fraction_bps` / `scoring_*` (6 campos) | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | **NADIE** — su único renderer (OpportunityEvidenceCell) es HUÉRFANO |
| `simulation_status` / `sim_classification` | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ | SOLO `/` XRay "SIM VERDICT" |
| `revert_reason` | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | **NADIE** |
| `trace_hash` / `evidence_hash` | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | **NADIE** (evidence column huérfana) |
| `evidence_gate` / `net_profit_gate` | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | **NADIE** |
| `bridge_fee_usd` / `bridge` | ❌ | ❌ | ❌ | ❌ | ❌ | ✅ | dlg Provenance |
| `route_metadata` legs (pool/dex/tokens) | ❌ | ✅ | ✅ | ❌ | ❌ | ✅ | /o ladder hops (labels, SIN dinero por hop); /x "Ruta"; dlg Route tabla |
| `leg_amounts_in/out` + `cycle_delta_wei` (HOPS-LEDGER-04) | ❌ | ❌ | ❌ | ❌ | ❌ | ✅ | **SOLO dlg tab Ledger** (`OpportunityDetailTabs.tsx:289-378`) — ninguna card |
| `hop_count` (derivado dex_adapters.length) | ❌ (usa heurística legs) | ✅ | ⚠️ (usa legCount=sintéticas incluidas) | ❌ | ❌ | ✅ | /o Summary "hops"; /x cuenta "legs" de deriveLegs |
| `paper_status` (paper_viable/rejected) | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ | **NADIE** lo pinta como tal (StatusPill pinta `status`; exchange pinta estado→REJECTED) |
| per-leg spot price / `cycle_spot_product` | ➖ | ➖ | ⚠️ "Buy px / Sell px" HARDCODED "—" (`OpportunityExchangeCard.tsx:479-483`) | ➖ | ➖ | ➖ | no existen en NINGÚN modelo FE — RD-02 debe emitirlos |
| `financing_mode` (OWN_CAPITAL/AAVE_FL/…) | ➖ | ➖ (etiqueta TODO "Flash loan in (TLS)" incondicional) | ➖ | ➖ | ➖ | ➖ | GAP-11 FE-02a: no viaja en el wire |

Home StatCards derivadas: bestNet (`net??simulated`), avgRoi, detected (window_total),
capital $0.00 estructural. `/s` formatos: `formatProfitUSD`/`formatPctOrDash` (`lib/format.ts:36-73`).

**Conclusión matriz para RD-03/RD-05:** la card MÁS pobre es la del HOME (cero USD, cero
route_metadata, "legs" heurístico) y la más rica es el dialog (que nadie abre por defecto).
Las USD fields SÍ existen en OmniOpportunity — el home nunca las mapea y by-strategy solo
usa net/roi (mal: coerce null→0).

## 3. ¿Dónde vive "live"? ¿Por qué el operador cree que existe /opportunities/live?

No existe la página. El concepto "live" está SOBRECARGADO en ≥6 acepciones visibles:

1. **Endpoint**: TODO se llama `/api/opportunities/live` (4 páginas + ticker) — el nombre
   del wire es la fuente #1 de la creencia.
2. **Título de página**: `/opportunities` se titula "Live MEV Feed"
   (`OpportunitiesClient.tsx:299`) con badge feedStatus LIVE/STALE/POLLING/CONNECTING
   (estado del TRANSPORTE WS).
3. **Home**: sección rotulada `/ opportunities · live` (`app/page.tsx:319`).
4. **Card exchange**: LED "LIVE" = card fresca <12s (STALE si vieja) — MISMO nombre,
   semántica DIFERENTE al badge del feed (`OpportunityExchangeCard.tsx:346-349`).
5. **Modo paper/live**: badge del terminus (`modeLabel`, usePaperModeState) + sidebar
   "LIVE TRADING ENABLED" (`app-sidebar.tsx:234`) + botón "EXECUTE (LIVE SHADOW)".
6. **Páginas vecinas**: `/live-readiness` (nav), `/live-testnet` (huérfana), tile "Live
   opportunities" en `app/page.tsx.backup:37`.

El ticker del layout (marquee inferior, `OpportunityTicker.tsx`) muestra `roi_pct` o "—"
por detección — NO es una página de oportunidades pero contribuye a la percepción.

**Respuesta canónica sugerida a RD-03a:** las oportunidades viven HOY en 4 rutas + 1 ticker;
"live" nombra al wire, no a una página. Unificar glosario: feed/transporte (WS/polling) vs
frescura (vigente/stale) vs terminus (paper/live) vs lifecycle (status 9 valores).

## 4. page.tsx.backup y demás cruft

- `app/page.tsx.backup` (109 líneas): home ANTERIOR — hub de tiles "Platform control plane"
  con 10 links (/status, /opportunities "Live opportunities", /executions, /live-readiness,
  /risk, /recon, /paper/history, /settings/credentials, /config, /killswitch) +
  HomeKpiStrip + ProgressRealCard (getStatus/getReconSummary). Reemplazado por el home
  XRay/StatCard actual. Cruft muerto, sin importar desde ningún lado. NO confundir operadores.
- **`app_backup/`** (árbol COMPLETO de la app anterior) y **`components_backup/`** y
  **`features_backup/`**: copias muertas que aún contienen los únicos renderers de
  bridge_fee_usd/simulated_notes/gas_used (seductor para grep — nunca confundir con vivo).
- Huérfanos vivos en árbol actual: `features/opportunities/OpportunitiesTable.tsx` +
  `OpportunityEvidenceCell.tsx` (0 importers no-test), `lib/hooks/useOpportunitiesStream.ts`
  (0 importers), `lib/websocket-client.ts` (solo su test), `components/live-ticker.tsx`
  (0 importers).

## 5. Duplicados/inconsistencias entre superficies (insumo directo RD-03a)

1. **3 semánticas de hops**: `/` legs = `dexes_used?.length ?? (dex_b?2:1)` (`page.tsx:91`);
   `/x` legs = `deriveLegs().length` (INCLUYE sintéticas §29); `/o`+dlg hops =
   `hop_count` = dex_adapters.length (null sin topología). Tres números distintos para la
   misma fila.
2. **Predicado "viable" divergente**: `/o` header cuenta viable =
   `status!=="rejected"&&!=="failed"` (null CUENTA como viable,
   `OpportunitiesClient.tsx:289`); ExchangeFilterBar viableOnly EXCLUYE null
   (`ExchangeFilterBar.tsx:212`); server `viable_only` es otro predicado más (R8: ver RD-01a).
3. **viableOnly de /opportunities es NO-OP en modo LIVE-WS**: solo parámetro del URL de
   polling-fallback/refresh manual (`useOmniOpportunities.ts:118-121`) — el render mapea el
   store SIN filtro (`OpportunitiesClient.tsx:436`). El toggle promete "Viable only" y en
   LIVE no filtra nada. Exchange sí filtra client-side (applyExchangeFilters).
4. **by-strategy fabrica ceros (VIOLACIÓN R8)**: `totalProfit=reduce(net??0)` y
   `avgRoi=reduce(roi??0)/n` ANTES de formatear → "Total profit: $0.00" y "Avg ROI: 0.00%"
   cuando NADA fue computado (hoy net 0/50) (`OpportunitiesByStrategyClient.tsx:54-57,112`).
   `formatProfitUSD`/`formatPctOrDash` ya manejan null — el `?? 0` los derrota.
5. **Dos caras para el mismo rechazo**: `/o` TradeCard muestra card completa con StatusPill
   REJECTED; `/x` isUnevaluatedShell (todas las economics null) → DetectionDiagnosticCard
   "Por qué NO pasó". El 3-hop rechazado (sin dinero) se ve COMPLETAMENTE distinto según la
   página. Bonus: `decodeRejectionReason` NO conoce `spot_product_le_one` → "Razón sin
   decodificar" (`OpportunityExchangeCard.tsx:181-191`).
6. **Gross con dos semánticas**: `/o` "Gross out (AMM spread)" = expected_profit_usd crudo;
   `/x` "Gross out (AMM)" = capitalIn + expected. Misma etiqueta-familia, distinta matemática.
7. **"Decoherencia" mezcla conceptos**: `/x` "Decoherencia (slip)" = slippage_usd; `/`
   StatCard "Decoherencia media (Convergence Ratio)" = avg roi_pct; XRay "DECOHERENCIA" =
   roi_pct. Un nombre, tres cosas.
8. **Estados duplicados**: lifecycle status (9), paper_status (2, NADIE lo renderiza),
   feedStatus (4, transporte), LED card (3, frescura), QUARANTINED (§30), Evaluada/Detección
   (gate /x), [PAPER] tag XRay, veredicto target PASS/FAIL, SIM/`~` prefix. El operador ve
   5 "estados" distintos sobre la misma fila según la página.
9. **Filtros duplicados**: viableOnly existe en /o (toggle roto en LIVE), en /x (chip que
   funciona) y como param server `viable_only` (3 predicados distintos, ver #2). Chain
   selector solo en /x. min-net-yield solo en /x. Notification threshold (toast) en /o.
10. **Units**: ROI en % + "bps" (SummaryGrid convierte roi×100 con label bps — confuso con
    confidence_score_bps real); USD con 4 formatos distintos (usd4, usd(), formatProfitUSD
    +/-, usdAmount/usdCost atlas). XRay "TLS AMOUNT" siempre "—" (placeholder muerto).
11. **Comparator memo divergente (bug latente /o)**: TradeCard memo NO cubre
    simulated_* (FIX-4 FE-02a NO aplicado — `OpportunityTradeCard.tsx:561-582`); exchange
    memo SÍ (`OpportunityExchangeCard.tsx:552-578`). Un UPDATE WS que solo cambia dinero no
    re-renderiza la card de /o pero sí la de /x.
12. **BUG-03 FE-02a sigue estructural**: WS push = fila cruda PG (websocket.ts:447-448,
    `row_to_json` via NOTIFY, sin leg_symbols/token_info) y el store REEMPLAZA por-id a
    nivel fila (omni-store.ts:340-353,370-397 — merge por id, NO por campo) → una card
    enriquecida por el snapshot REST puede ser empobrecida por el push WS del mismo id.

## 6. Cita a pares / dependencias

- **RD-01a/RD-01c**: confirmen qué columns servidas trae el SELECT de
  opportunities-live vs la fila cruda del NOTIFY (BUG-03) — mi matriz asume el mapper FE
  verbatim.
- **RD-02**: los campos que el FE NO puede mostrar porque NO EXISTEN en el wire:
  per-leg spot_price, cycle_spot_product, financing_mode, detector_id, per-leg USD.
  El "Buy px / Sell px" hardcoded "—" de /x es el hueco exacto del reclamo "no sale precio".
- **RD-05**: OpportunityCard canónico debe nacer de OmniOpportunity (no de
  OpportunityRowSchema), absorber XRayCard+TradeCard+ExchangeCard y PINTAR
  leg ledger/cycle_delta (hoy solo dlg).
- **RD-06**: al verificar, chequear específicamente: toggle viableOnly en modo LIVE (debe
  filtrar client-side o desaparecer), by-strategy sin $0.00, hops 2 y 3 con mismo glosario.
