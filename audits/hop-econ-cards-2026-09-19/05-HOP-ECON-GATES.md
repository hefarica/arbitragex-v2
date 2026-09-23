# 05-HOP-ECON-GATES — veredicto WO-05 (contraste independiente, 2026-09-19)

Alcance: contrastar si existe gate/filtro/reshape entre PG/redis y el browser que dropee o
degrade campos de economía por hop (route_metadata, per-leg ledger, símbolos, USD in/out).
READ-ONLY. Toda evidencia = archivo:línea re-leído directamente en el working tree
(cdb4c890, feat/s1-fee-dual-unit-20260918). ZERO MOCKS — nada de lo citado es inferido.

## A. GATES ACTIVOS QUE DROPEAN / DEGRADAN (evidencia archivo:línea)

| # | Gate | Capa | Evidencia | Efecto |
|---|------|------|-----------|--------|
| A1 | **Hot stream XADD plano de 4 campos** — `arbx:hot:detected` lleva SOLO id, chain_id, strategy_kind, detected_at_ms (nunca route_metadata). El hash completo `arbx:hot:opp:{id}` (300s TTL) no lo lee nadie del path WS. | searcher→redis | searcher-rs/src/hot_path_emitter.rs:79-94 (XADD campos) · :96-105 (hash opp completo, no consumido por WS) · :139-174 (hot:simulated igual — id/status/net_profit_wei/gas, sin ledger) | **DROP TOTAL** de ledger/símbolos/USD en TODO evento `opportunity:detected`/`opportunity:validated`. |
| A2 | **WS emitter type-narrow string-only** — HotOpportunity declara campos string; `parseFields` + emit no transportan objetos. | api-server WS | api-server/src/websocket.ts:887-897 (interfaz) · :964-998 (XREADGROUP→emitEntry) | Refuerza A1: el canal WS es estructuralmente incapaz de llevar el ledger (JSONB). |
| A3 | **Adaptador FE new_opportunity → HotEvent** — allowlist explícita de 4-5 campos; descarta todo lo demás del row PG (incl. route_metadata si llegara). | frontend WS | frontend/lib/websocket-client.ts:60-83 (adaptNewOpportunityToHotEvent) | DROP por diseño en el path PG-LISTEN→WS (para /ws/hot-opportunities). |
| A4 | **Kelly rescale nullea el ledger** (upstream del transporte, confirmado) | searcher sizing | searcher-rs/src/size_optimizer.rs:645-646 y :661-662 (`sized.leg_amounts_in = None` en AMBOS brazos del Kelly bind; test pin :3404) | Filas Sized-via-Kelly llegan a PG SIN ledger → tabs muestran "—" honesto. Cuán frecuente: depende del % de rows donde el cap Kelly bindea (no medible read-only local). |
| A5 | **Ledger all-or-nothing en attach** — mismatch de longitudes ⇒ ledger omitido (row sí persiste, sin ledger). | searcher persist | shared-rs/src/candidates.rs:203-230 (attach_leg_ledger) · orchestrator.rs:1129-1138 · cartridge_boot.rs:1596-1600 (attach_mismatch debug) | Degradación silenciosa-by-design (R8) si hops≠len; telemetría debug existe pero no es alerta. |
| A6 | **Zod OpportunityRowSchema sin route_metadata/leg_symbols** — `z.object` strippea claves no declaradas; si este schema se reusa en el camino de cards, el ledger se cae ahí. | frontend REST | frontend/lib/schemas.ts:59-133 (schema), :135-140 (OpportunitiesLiveSchema). Consumidor actual: OpportunityTicker.tsx:4,85 vía api-client.ts:282-283 | HOY inofensivo para las cards (ver B3), pero es una trampa lista: cualquier consumidor nuevo de `getOpportunitiesLive()` pierde el ledger sin error. |
| A7 | **Structural check en persistencia** — topology con token_addresses.len()≠hops+1 se persiste como '{}' (row sin NINGUNA topología). | searcher→PG | searcher-rs/src/persistence.rs:101-129 (structurally_ok check + warn persist.route_metadata_invalid) | Drop de topología completa (no sólo ledger) en filas malformadas; con warn log, sin contador/alerta dedicada. |

## B. GATES INOFENSIVOS (verificados, preservan economía por hop)

| # | Gate | Evidencia | Por qué es inofensivo |
|---|------|-----------|----------------------|
| B1 | LIVE_QUERY SQL selecciona route_metadata tal cual | opportunities-live.ts:288 (SELECT o.route_metadata) | Sin proyección de subcampos, sin WHERE sobre JSONB. |
| B2 | rowToOpportunity pasa verbatim ({}→null) | opportunities-live.ts:630-635 | Único reshape: objeto vacío→null (semántica R8, no drop). leg_symbols hidratado :585-601, :778-826 (batch tokens + eth_call on-demand). |
| B3 | Cards path NO pasa por Zod: fetch directo + mapToOmniOpportunity | OpportunitiesClient.tsx:239-252 · useOmniOpportunities.ts:128-138 | res.json()→mapper preserva route_metadata/leg_symbols (types.ts:401,433). |
| B4 | parseRouteMetadata NO filtra entradas; ledger validado all-or-nothing y marcado route_ledger_invalid | frontend/lib/store/types.ts:501-550 (stringArray sin filtro de entries :506-511; validLegLedgerInput :490-499) | Malformed ⇒ flags de diagnóstico, no invención ni poda por-hop. |
| B5 | Edge worker proxy = passthrough byte-verbatim + KV 2s | edge/worker/src/index.ts:445-474 (proxy), :666 (ruta live), :482+ (proxyPassThrough) | body íntegro text()→c.body(); cache_query-scoped; sólo re-serializa status, jamás el JSON. |
| B6 | Normalizaciones int8/BIGINT (block_number, pipeline_latency_ms) | opportunities-live.ts:455-459, :614, :623 | Ya aterrizadas (WO-G2-PARITY); no tocan economía por hop. |
| B7 | Ventana 300s + viable_only opt-in (default false) + LIMIT 50/200 | opportunities-live.ts:316-329, :676-690 | Filtran FILAS, no CAMPOS; default muestra rejected con razón visible. |
| B8 | ENABLED_STRATEGIES / trading_config gates | searcher-rs/src/main.rs:873-877 · counters.rs:68-71 (gate_strategy_disabled) | PRE-PG (el candidate nunca se persiste como viable); no toca el transporte. La anomaly "allowlist stale" es disponibilidad de filas, no degradación de campos. |
| B9 | TTL prune 5min / MAX_ITEMS 200 / WsIngestBuffer dedup | useOmniOpportunities.ts:41-45, :179-213 | Vigencia de cards, no campos. |
| B10 | Legacy: validate() decimals gate (post-2026-08-10) | persistence.rs:88-100 | CERRADO — el fix structural ya no exige decimals (ROOT-CAUSE 2026-08-10 documentado in situ). |

## C. NO ENCONTRADOS (buscado explícitamente, sin evidencia en repo)

- Ninguna proyección/DELETE de subcampos de route_metadata en SQL del api-server
  (grep LIVE_QUERY + rutas opportunities*: sólo SELECT íntegro).
- Ningún allowlist de campos en el edge worker (proxy text→body verbatim; no re-parse).
- Ningún serializer con pick/omit sobre route_metadata/leg_* en api-server o frontend
  (sólo los dos ws adapters citados en A2/A3 y el Zod ticker A6).
- Ninguna mutación de leg_amounts_* fuera de size_optimizer (grep leg_amounts en searcher-rs:
  sólo kernel, orchestrator threading :960-969, cartridge_boot attach).

## D. DISCREPANCIAS vs mapa WO-01

| # | Claim WO-01 | Verificación independiente | Veredicto |
|---|-------------|---------------------------|-----------|
| D1 | "pasa VERBATIM opportunities-live.ts:288,622-627" | Exacto: :288 (SELECT), :630-635 (verbatim + {}→null). | CONFIRMO (rango :627→:635 menor). |
| D2 | "+leg_symbols :577" | Real: construcción :585-601, hidratación :778-826. | CONFIRMO (línea :577 es comentario del bloque; rango real citado arriba). |
| D3 | "FE preserva en types.ts:501-550" | Real: tipo RouteMetadataWire :131-139, preservación ledger :535-548, passthrough leg_symbols :401. | CONFIRMO sustancia; el rango :501-550 corresponde a parseRouteMetadata, también exacto. |
| D4 | "Kelly rescale lo nullea (size_optimizer.rs:3404)" | :3404 es el TEST que lo pinea; el código que nullea es :645-646 y :661-662 (ambos brazos). | CONFIRMO comportamiento; CORRIJO cita (código ≠ test). |
| D5 | "(implícito) el dato llega al dominio vivo por WS" | NO: el path WS dropea TODO el ledger (A1-A3). El ledger sólo llega al browser por REST /api/opportunities/live (poll 4-5s del hook). | **DISCREPANCIA MATERIAL**: "WS vivo" y "cards hop-econ" son carriles distintos; WO-04 debe verificar el REST feed, no el WS, para economía por hop. |
| D6 | Anomalía "(a) 93 claves sin reparar" | Contexto: e2e-demo-20260917/00-SYNTHESIS.md §"CATALOG-BACKFILL-01 sin deploy" — 93 claves legado de labels de quote-path, sin deploy. | FUERA del path de transporte: afecta precisión de labels/etiquetado, NO el ledger ni símbolos por hop. No es gate de este ciclo. |
| D7 | Anomalía "(b) 2º path de quote no instrumentado" | Sin artefacto de telemetría; es cobertura de instrumentación de quotes. | FUERA del path: no toca transporte de campos; afecta confianza del dato origen, no su llegada. |

## E. Conclusión operativa

1. El ledger por hop SOLO llega al browser por el carril REST (edge→api-server /api/v1/opportunities/live),
   que está LIMPIO de gates de campos (B1-B6). Las cards WO-03 (DONE) leen ese carril → correcto por diseño.
2. El carril WS (opportunity:detected/validated/new_opportunity) NO puede llevar economía por hop
   (A1-A3): es un feed de latencia con 4 campos correlación. Si el operador espera hop-econ "en vivo WS",
   ese es el gap estructural a cerrar (work order nueva, no defecto del transporte existente).
3. Ubicación honesta del "—": filas pre-HOPS-LEDGER-04, filas Kelly-rescaled (A4), attach mismatch (A5),
   topología malformada persistida como '{}' (A7). Todos R8-by-design; A4 es el único dependiente del
   sizing runtime y merece una métrica (kelly_ledger_null_ratio) para dimensionar el impacto.

— hermes (ccr-glm53), WO-05. Sin commits, sin push, sin deploy. Cero mocks.
