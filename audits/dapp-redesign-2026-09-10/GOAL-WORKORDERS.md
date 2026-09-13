# GOAL-WORKORDERS — DAPP-REDESIGN-01 (2026-09-10)

**/goal del operador (textual, 2026-09-10):** "los hops de 2 solo salen hops de 2 cuando sale el
precio; cuando hay hops de 3 no sale ni mierda de precio; no salen las cards pobladas con todos
los valores de dinero; no hay ni mierda de campos de usd implementados en las cards. Rediseña
desde 0 ambas páginas de oportunidades y exchange para que se hagan bien. Las cards y todo el
módulo de arbitrage y de oportunidades tiene que funcionar perfecto. No más excusas."

## §0 — VERDAD DEL DATO (recon orquestador, 2026-09-10T04:0xZ, `GET https://arbx.ape-tv.net/api/opportunities/live`, 50 opps)

| Campo | triangular 3-hop (n=7) | resto (n=43) | Total (50) |
|---|---|---|---|
| `amount_in_wei` | **7/7 = `0` literal** (¡cero, no None!) | 43/43 (0 en rechazados) | — |
| `expected_profit_usd` | **0/7 None** | **2/43** | 2/50 |
| `net_expected_profit_usd` | 0/7 | **0/43 — NUNCA poblado, ni en ganadores** | 0/50 |
| `simulated_amount_in_usd` | 0/7 | 0/43 — sim nunca llega a este payload | 0/50 |
| `simulated_net_profit_usd` | 0/7 | 0/43 | 0/50 |
| `route_metadata` (pool_addresses, dex_adapters) | ✅ 7/7 presente (3 pools) | parcial | — |
| `rejection_reason` | `spot_product_le_one` 7/7 | mixto | — |
| `status` / `paper_status` | `rejected` / `paper_rejected` | mixto | — |

**Conclusión raíz (dos capas, no una):**
1. **BACKEND**: el gate `spot_product_le_one` mata los 3-hop ANTES de sizing/simulación → el
   payload sale sin NINGÚN valor de dinero. Ni siquiera existe el monto de referencia: el
   rechazado lleva `amount_in_wei=0` literal (ambiguo, viola R8: 0≠None). `net_expected_profit_usd`
   no se puebla NUNCA (0/50) ni para los 2 ganadores. `simulated_*` no está cableado a este payload.
2. **FRONTEND**: las cards no tienen campos USD implementados; cuando un campo existe (los 2/50
   ganadores) la UI no lo muestra completo; los 3-hop muestran "nada" porque nada les llega Y
   porque la card no renderiza legs/valores.

Lo que el operador ve como "el precio sale a veces en 2-hop" = los 2/43 que pasan el gate y son
sizeados. **No es un bug de render de 3-hop: es que al 3-hop se le niega el dinero en origen,
y además la UI nunca tuvo los campos.**

### §0.1 — Censo de rutas que muestran oportunidades (find frontend/app, 2026-09-10)

- `/` (app/page.tsx) — home con OpportunityTicker
- `/opportunities` (app/opportunities/page.tsx) — "Oportunidades"
- `/opportunities/exchange` (app/opportunities/exchange/page.tsx) — "Exchanges"
- `/opportunities/by-strategy` (app/opportunities/by-strategy/page.tsx)
- **NO EXISTE `/opportunities/live` como página** — el operador lo nombra: el gang resuelve
  dónde vive hoy el concepto "live" (tab/filtro/ticker) y UNIFICA: una respuesta canónica a
  "¿dónde veo las oportunidades?" + mismo glosario + mismas cards en TODAS las superficies.
- ⚠️ `app/page.tsx.backup` — cruft sospechoso: inspeccionar (no borrar sin mirar).
- Relacionadas fuera de scope de cards: `/executions`, `/paper/history`, `/route-outcomes`.

### §0.2 — RD-01c TRUTH-SOURCES (verificado vivo 2026-09-10T04:25-04:47Z, VPS read-only + payload real en `fixtures/live-payload-rd01c.json`)

**Payload vivo re-capturado (corrige ventana §0, misma verdad estructural):** 50 opps = 13× triangular-3hop (`spot_product_le_one`, `amount_in_wei="0"`, cero dinero) + 37× dex_arb-2hop (29 `v3_quote_unavailable`, 4 `non_positive_profit` — los ÚNICOS con gross: 0 / 0.00020372×2 / 0, todos `amount_in_wei="0"` —, 4 `single_pool_no_spread`). `net/simulated/amount_in_usd/roi`: 0/50.

**Q1 — Fuentes de precio REALES (file:line, frescura, cobertura):**
- **Tier-0 Chainlink ON-CHAIN (autoritativa, ACTIVA)**: `searcher-rs/src/workers/price_worker.rs:852-947` (`fetch_chainlink`+`eth_call latestRoundData` selector 0xfeaf968c línea 259); oráculos PG `price_oracles` kind='chainlink' (VPS: 5 rows enabled). Log vivo `price_worker.tick_done chainlink_hits=5` @04:47:47Z. Cubre WETH/WBTC/USDC/USDT/DAI. Tick 30s (`DEFAULT_PERIOD_SECS` línea 71).
- **Tier-1 Alchemy / Tier-2 Coingecko**: price_worker.rs:734-795 / 798-842 (backoff 300s l.100). **VPS REAL: alchemy_hits=0, coingecko_hits=0, cache_misses=126/131** — NO aportan hoy.
- **DexScreener + GeckoTerminal tiers (token-enricher, ACTIVOS)**: `token-enricher/src/dexscreener.rs:482-528` y `geckoterminal_tier.rs:561-593` escriben el MISMO hash `arbx:token_prices:<chain>` + PUBLISH. Gates `ARBX_DEXSCREENER_ORACLE=active` / `ARBX_GECKOTERMINAL_ORACLE=active` (verificados en .env VPS). Intervalos 15s/60s default. Ellos pagan la cola larga (CAVA, 1INCH, HERA…). `total_reserve_in_usd` (geckoterminal l.67,442) es FILTRO de confianza, no feed reserves→USD.
- **Consumo hot-path**: `RedisCachedPriceOracle::snapshot_from_redis` (`shared-rs/src/price_oracle.rs:204-243`, key builder l.285) — triangular_worker por tick (triangular_worker.rs:1184-1190, cascada Redis→config l.1390-1401); orchestrator mergea snapshot a cfg ANTES de sizing (orchestrator.rs:822-833) y al evaluator (l.1184-1209). TTL hash = max(2×period,60s).
- **⚠️ INCONSISTENCIA (para RD-01a/RD-02)**: `triangular_engine.rs:656-679 extract_pricing` NO usa el snapshot Redis — solo `cfg.base_token_price_usd` (WETH) + 1.0 duro (USDC/USDT/DAI) + None. Y `base_token_price_usd` es config de operador (PUT admin `api-server/src/routes/trading-config.ts:401`) **stale: 2350.79 @2026-09-07 vs Chainlink vivo 2479.18 = −5.2% drift**. Todo USD del path engine-triangular usa el precio viejo.
- `forwardSimulate` de la API también usa config stale: `api-server/src/simulation/computeSimulatedNet.ts:160-163,196-206`.

**Q2 — amount_in_usd para TODA opp HOY:** precio de token_in SÍ existe en emisión (snapshot Redis ≤30s). El MONTO real no existe para rejected (sizing nunca corrió — orchestrator.rs:866-880: engine-rejected saltan el SizeOptimizer). Lo ya-presente-en-memoria al rechazar y hoy descartado: `cycle_spot_product` (computado en triangular_engine.rs:435 SOLO para elegir el label), hop_reserves por leg (l.386-399 → spot_ratio=R_out/R_in), `cap_usd` real (`effective_capital_for`). Emitir `amount_in_usd=None+reason:"not_sized"` + `reference_capital_usd=cap` (real, no inventado).

**Q3 — spot per leg / cycle_spot_product:** `spot_product()` pub fn `triangular_worker.rs:227-238` (S=γ³·∏(R_out/R_in)); la usa `evaluate_cycle` (l.760) y el engine la re-computa al rechazar (triangular_engine.rs:431-435) — **captura = copiarla al payload (route_metadata), wiring puro**. Reserves en hot-path: ReservesEntry{r0,r1,blk,ts} `reserves.rs:35-48`, TTL writer 30s, knob `reserves_freshness_budget_s` default 60 (canonical_knobs.rs:129,202).

**Q4 — Gas real para net_***: `gas_cost_usd()` `shared-rs/src/trading_config.rs:475-479` llama `resolve_gas_price_gwei(0.0,0.0)` (l.452-460) — señales vivas SIEMPRE 0 → con `dynamic_basefee_plus_tip` (estrategia REAL del VPS) cae a **1.0 gwei fallback**. Gas REAL ya vivo: `arbx:gas_price_wei:1`=51136463 wei (0.0511 gwei), cada 10s TTL 60s (`gas_oracle_worker.rs:52,111-147`); su único consumidor es sim-ctl/RevmBackend (net-of-gas real en sims). Spine evalúa con `NetworkSignals::unknown` (orchestrator.rs:1205). Net estimado sobreestima gas ~20× (1 vs 0.051 gwei). Wiring: leer gas_price_wei_key en resolve o poblar live signals (firma ya existe).

**Origen de `amount_in_wei="0"` (R8):** `triangular_engine.rs:567-572` `unwrap_or_else(|| "0".to_string())` — literal en fuente. `net 0/50` explicado: engine-rejected nunca sizing (orch. 866-880) + optimizer-rejected conserva gross pero net=None por diseño (orch. 984-992) + el worker directo que SÍ calcula net inline (triangular_worker.rs:1512-1515) solo emite accepted. `simulated_* 0/50`: forwardSimulate exige `amount_in_wei≠"0"` (opportunities-live.ts:848-857).

**decisions[] completas en el output estructurado del agente RD-01c** (campo×fuente×disponible/riesgo). Síntesis: `cycle_spot_product`, `spot_price/leg`, `token_a_price_usd`, `reference_capital_usd` = disponibles HOY (wiring); `amount_in_usd` rejected = None+razón (honesto); `net_*` rejected con gas = disponible HOY pero con gas fallback 1 gwei (mejorar leyendo `arbx:gas_price_wei`); fix extract_pricing/forwardSimulate → snapshot Redis primero (cierra el −5% drift).

## §1 — WORK ORDERS

| WO | Título | Dueño (rol) | Gate de aceptación | Estado |
|---|---|---|---|---|
| RD-01 | TRUTH: forense pipeline del dinero (detector→payload) | 3 PhDs paralelos | Tabla campo×hop×población con file:line exactos; decisión documentada de qué campos REALES pueden emitirse para rejected sin violar RULE 00 | 🟡 (**RD-01c DONE** — §0.2) |
| RD-02 | EMIT-RICH: toda opp (2h/3h, passed/rejected) lleva dinero real o null honesto + razón | builder backend/edge | `expected_profit_usd`, `net_*`, `simulated_*`, per-leg `{pool,dex,token_in/out,spot_price,amount_in/out_usd}` y `cycle_spot_product` cuando existan en datos reales; `amount_in_wei` rechazado → null + reason (fix R8); cargo test verde en crates tocados | ⬜ |
| RD-03a | UNIFY: taxonomía única de oportunidades para las 4 superficies | arquitecto (fase DESIGN) | Respuesta canónica documentada en board: dónde se ven las oportunidades, qué rol tiene cada superficie, mismo glosario/filtros/estados/cards | ✅ **DONE 2026-09-10** — §5: TODAS las oportunidades viven en `/opportunities` (feed canónico, viables+rechazadas); `/opportunities/exchange` = lente de microestructura (misma población/cards); `/opportunities/by-strategy` = telemetría agregada; `/` = cockpit; `/opportunities/live` NO es página (es el wire) → redirect permanente a `/opportunities`; predicado viable ÚNICO = `paper_status==='paper_viable'` |
| RD-03 | PAGE-OPPORTUNITIES: rediseño desde 0 (según taxonomía RD-03a) + alinear ticker home `/` | builder frontend | Página nueva renderiza TODOS los campos dinero + legs por hop + estados honestos; vitest+tsc verde; R1 hydration | ⬜ |
| RD-04 | PAGE-EXCHANGE: rediseño desde 0 | builder frontend | Ídem RD-03 | ⬜ |
| RD-05 | CARD-LIB: OpportunityCard canónico + schemas Zod SSOT | builder frontend | 1 sola definición de campos dinero consumida por ambas páginas; streaming delta (solo actualiza el número que cambió); badges hops 2/3; estados vacíos honestos R8 | ⬜ |
| RD-06 | VERIFY-LOCAL: browser-verifier como usuario | dapp-browser-verifier | Playwright sobre build local con datos REALES del edge vivo: cards hops 2 y 3, sin console errors, vitest verde; lista explícita de qué acceptance son live-only (post-deploy) | ⬜ |
| RD-07 | SHIP: PR + CI + merge + deploy por-servicio + verify dominio | ORQUESTADOR (nadie más toca git) | CI verde, merge, deploy per-service con prune antes/después (disco <30GB libre), dominio vivo mostrando USD en 3-hop (o razón honesta documentada si el gate real lo rechaza: mostrar el valor del ciclo spot, no silencio) | ⬜ |
| RD-08 | DOCTRINE: sync boards + LEARNINGS.md | orquestador | Boards BR-11/hops-visibility, frontend-doctrine y CB actualizados; lecciones anexadas a LEARNINGS.md | ⬜ |

## §1.1 — RD-01b TRUTH-FRONTEND · ENTREGADO (2026-09-10) → detalle en `RD-01b-TRUTH-FRONTEND.md`

Transporte (4 superficies = MISMO wire): todas SSR-fetch REST `GET /api/opportunities/live`
(vía `INTERNAL_EDGE_URL` server-side; browser same-origin → rewrite Next → edge:8787).
`/opportunities` y `/opportunities/exchange` post-hidratan con WS socket.io room
`subscribe:opportunities` → evento `new_opportunity` (api-server:8080, payload = fila CRUDA
PG `row_to_json` — FE-02a BUG-03 sigue vivo) con fallback polling 5s al mismo REST.
`/opportunities/by-strategy` e `/` NO usan WS de oportunidades (poll 4s / SSR-only + ticker
REST 30s). Matriz campo×superficie, concepto "live", duplicados y cruft: ver el entregable.
Hallazgos P0 para RD-03a/RD-05: (1) viableOnly de /opportunities es NO-OP en modo LIVE-WS
(solo filtra el URL del polling); (2) by-strategy fabrica $0.00/0.00% (coerce `?? 0` antes
de formatear — viola R8); (3) home XRayCard NO tiene NI UN campo USD; (4) tres semánticas
distintas de "hops" (legs heurístico / deriveLegs-con-sintéticas / hop_count); (5)
OpportunitiesTable+OpportunityEvidenceCell+useOpportunitiesStream+websocket-client+live-ticker
= cadena huérfana (los campos evidence/scoring viven ahí); (6) comparator de memo del
TradeCard NO cubre simulated_* (congela dinero — FIX-4 de FE-02a NO aplicado; el del
exchange card SÍ).

## §2 — REGLAS DEL GANG (inviolables)

1. **RULE 00**: cero mocks/datos inventados. Solo datos reales (payload vivo, reserves, PG/Redis RO). Fixture de test = payload REAL capturado del dominio (se guarda en `audits/dapp-redesign-2026-09-10/fixtures/`), jamás datos fabricados.
2. **R8 fail-honest**: `None` = no computado, `0.0` = cero exacto. `amount_in_wei=0` en rechazados es ambigüedad a ELIMINAR.
3. **NO-GIT**: builders no ejecutan git commit/push/branch. PRs = orquestador.
4. **Diffs taggeados** `// RD-XX (2026-09-10)`.
5. **§34.3 intocable**: nada de live_exec_policy/relays-client/broadcast.
6. **R1**: page.tsx Server Component puro + Client con `initialSnapshot`; no-determinismo solo en `useEffect`. **R5**: auditar componentes transitivos. **RULE 02**: REST→edge 8787, WS→api-server 8080 directo.
7. **Sin deploys ni toques a producción** (read-only sobre el dominio).
8. **Board es el eje**: leer ANTES, actualizar AL terminar. Citas entre pares por WO.
9. LEARNINGS.md (skill arbitragex-omniscience) se lee al inicio y se anexa al cierre (orquestador).

## §3 — RESTRICCIONES OPERATIVAS

- VPS ~13GB libres → deploys SOLO por-servicio con `docker builder prune -af --min-free-space 20GB` antes y después (DISK-GUARD-01) hasta RT-01.
- Ambiente builders: Windows/Git Bash; `py -3 -X utf8`; cargo workspace en `backend/`; NO Docker local (RULE 01); FE sin lockfile; `next dev` tarda ~175s.
- El payload vivo se audita READ-ONLY: `curl https://arbx.ape-tv.net/api/opportunities/live`.

## §4 — SINCRONÍA CON OTROS BOARDS

- `audits/frontend-doctrine-2026-09-07/GOAL-WORKORDERS.md` — censo FE-01 + hallazgos FE-02 (hops que desaparecen al aparecer USD) = insumo directo de RD-01/RD-03.
- BR-11 (hops-visibility, RC1-RC6 landed e45e1d06) — `route_metadata` ya existe en payload: NO redescubrir, consumir.
- CONTROL BOARD (CB-03 streaming delta) — RD-05 implementa la misma doctrina.

## §5 — RD-03a SPEC MAESTRA: UNIFY + CONTRATO + CARDS + LAYOUTS (LEY para RD-02/RD-03/RD-04/RD-05; RD-06 verifica contra esta sección)

Autor RD-03a (arquitecto), 2026-09-10. Insumos: RD-01a/RD-01b/RD-01c (§6 + §1.1 + output estructurado) +
verificación propia: `frontend/lib/schemas.ts` (OpportunityRowSchema actual), `frontend/lib/store/types.ts`
(OmniOpportunity + mapper), `backend/api-server/src/routes/opportunities-live.ts:560-633` (rowToOpportunity),
`backend/shared-rs/src/candidates.rs:130-170` (RouteMetadata), `frontend/components/nav-items.ts:55-59`,
fixture REAL `fixtures/live-payload-rd01c.json` (50 items, 44 keys c/u — base de TODOS los tests, RULE 00).

### 5.1 TAXONOMÍA ÚNICA — ¿dónde se ven las oportunidades?

**Respuesta canónica (una frase):** las oportunidades se ven TODAS en **`/opportunities`** — el feed
operacional canónico (viables + rechazadas, mismo wire `/api/opportunities/live`); las otras tres superficies
son vistas derivadas con UN rol único cada una. **`/opportunities/live` NO es ni será página** — es el nombre
del WIRE: redirect permanente `/opportunities/live` → `/opportunities`.

| Superficie | Rol único (cerrado) | Muestra | NO es |
|---|---|---|---|
| `/opportunities` | **FEED CANÓNICO** — la respuesta a "¿dónde veo las oportunidades?" | TODAS las detecciones de la ventana (viables + rechazadas), OpportunityCard full, filtros unificados, DetailDialog, badge de TRANSPORTE del feed | No "solo viables"; no una vista de exchange |
| `/opportunities/exchange` | **LENTE DE MICROESTRUCTURA** por par/venue (nav: "Exchange feed" → "Mercados & precios") | MISMA población (mismo wire, mismas cards, mismos filtros), reagrupada por par/venue; panel de ciclo con spot REAL por leg (sustituye "Buy px/Sell px" hardcodeado); PriceTicker | No es un segundo feed ni un segundo glosario |
| `/opportunities/by-strategy` | **TELEMETRÍA AGREGADA** por strategy_kind | Tabla honesta: counts, gross/net sumados SOLO donde existen (null sin coerce), histograma rejection_reason | No es feed de cards |
| `/` (home) | **COCKPIT ejecutivo** | StatCards agregadas honestas + ticker + top-N OpportunityCard compact | No es superficie primaria de oportunidades |

**Misterio `/opportunities/live` resuelto:** "live" era el nombre del endpoint wire sobrecargado en 6
acepciones. Quedan reducidas así: (a) redirect permanente en `next.config.js`; (b) el título de
`/opportunities` deja de decir "Live MEV Feed" → **"Detecciones"** + badge de transporte; (c) el LED per-card
"LIVE" se renombra **"NUEVA"** (frescura <12s); (d) las páginas vecinas `/live-readiness` y `/live-testnet`
pertenecen al eje TERMINUS (readiness operacional) y quedan fuera de este módulo.

### 5.2 GLOSARIO ÚNICO (5 ejes + 2 definiciones cerradas — obligatorio en TODAS las superficies)

1. **TRANSPORTE** (solo header del feed, `/opportunities` y exchange): `LIVE | STALE | POLLING | CONNECTING`
   — estado del canal WS/poll. NUNCA per-card.
2. **FRESCURA** (per-card): LED **"NUEVA"** si `detected_at` < 12s. (Elimina la colisión con el badge LIVE.)
3. **TERMINUS de ejecución** (badge global sidebar/header): `PAPER | TESTNET | LIVE` — §34.3. Nunca per-card.
4. **LIFECYCLE** (per-card, StatusPill): los 9 valores de `status` (detected…failed).
5. **VEREDICTO económico** (per-card, chip): `Viable | Rechazada` — **predicado ÚNICO en todas partes**
   (chip, contador, filtro client-side, param server `viable_only`): `paper_status === 'paper_viable'`
   (≡ `rejection_reason IS NULL`). Los otros dos predicados actuales (contador `status!=='rejected'` de
   /opportunities y el exclude-null del ExchangeFilterBar) se ELIMINAN.
6. **RECHAZO** (sub-estado de 5): `{code, detail}` decodificado vía catálogo único
   (`frontend/lib/rejection-catalog.ts` NUEVO) con TODOS los códigos reales del fixture:
   `spot_product_le_one, v3_quote_unavailable, non_positive_profit, single_pool_no_spread, gas_floor_breach,
   kelly_negative_edge, net_negative, strategy_not_simulatable_in_s4`… code desconocido → "Razón sin decodificar".

**Hops — UNA definición:** `hop_count = route_metadata.dex_adapters.length`; null (sin topología) → badge
"—". PROHIBIDAS las otras dos: heurística `dexes_used.length` (home) y contar legs sintéticas §29 para el
CONTEO (las sintéticas solo se dibujan en la ruta con marcador SYNTHETIC LEGACY VIEW, jamás en el HopBadge).

**Dinero — nombres cerrados en toda UI:** Bruto (`expected_profit_usd`, spread AMM pre-costos) · Neto
(`net_expected_profit_usd`) · Sim Net / Sim Capital (`simulated_*`) · Capital (monto sizado) · Capital ref
**CAP** (`reference_capital_usd`, cota del operador — SIEMPRE con badge "CAP") · Spot por leg (`spot_price`)
· S del ciclo (`cycle_spot_product`, solo triangular) · Gas est. (`gas_estimate_usd` + etiqueta fuente
`oracle | config_fallback`). "Decoherencia" en home StatCard = slippage_usd medio (de
`simulated_cost_breakdown`); sin breakdowns honestos → "—". NUNCA roi bajo ese nombre.

### 5.3 CONTRATO DE DATOS — Zod SSOT (crea RD-05; consumen TODAS las superficies y el ticker)

Archivo NUEVO `frontend/lib/schemas/opportunity-live.ts` — ÚNICO contrato del wire.
`OpportunityRowSchema`/`OpportunitiesLiveSchema` de `lib/schemas.ts` quedan DEPRECADOS (ticker migra en este
mismo programa; alias temporal máximo 1 release). Verificado contra payload REAL (44 keys/item).

```ts
// ─── RouteMetadata (JSONB: shared-rs/candidates.rs + extensión RD-02 §5.4) ───
export const RouteLegMetaSchema = z.object({
  pool: z.string(),                                   // "" honesto si solo se conoció factory
  dex: z.string(),
  token_in: z.string(),
  token_out: z.string(),
  spot_price: z.number().nullable(),                  // raw R_out/R_in; null = reserves faltantes al tick
  spot_price_fee_adjusted: z.number().nullable(),     // γ·R_out/R_in
  reserve_in_wei: z.string().nullable(),
  reserve_out_wei: z.string().nullable(),
  amount_in_wei: z.string().nullable(),               // solo si sizing computó (HOPS-LEDGER-04)
  amount_out_wei: z.string().nullable(),
  spot_price_usd: z.number().nullable(),              // spot × precio quote; null = unpriced
  amount_in_usd: z.number().nullable(),
  amount_out_usd: z.number().nullable(),
});
export const RouteMetadataSchema = z.object({
  pool_addresses: z.array(z.string()),
  token_addresses: z.array(z.string()),               // length = hops+1
  dex_adapters: z.array(z.string()),                  // length = hops → hop_count
  decimals: z.record(z.string(), z.number().int()),
  leg_amounts_in: z.array(z.string()).nullable().optional(),    // sized only (all-or-nothing)
  leg_amounts_out: z.array(z.string()).nullable().optional(),
  leg_zero_for_one: z.array(z.boolean()).nullable().optional(),
  // ── RD-02 (2026-09-10), emit-time en el punto de decisión ──
  legs: z.array(RouteLegMetaSchema).nullable().optional(),      // length = hops
  cycle_spot_product: z.number().nullable().optional(),         // S real; null = no triangular/sin reserves
  token_a_price_usd: z.number().nullable().optional(),          // precio base usado (fuente snapshot)
  reference_capital_usd: z.number().nullable().optional(),      // CAP del operador (effective_capital_for)
  candidate_amount_usd: z.number().nullable().optional(),       // SOLO sizing-rejects (gas_floor/kelly/net_negative)
  rejection_detail: z.string().nullable().optional(),           // valor del gate, p.ej. "S=0.9873 ≤ 1"
  gas_estimate_usd: z.number().nullable().optional(),
  gas_source: z.enum(["oracle", "config_fallback"]).nullable().optional(),
});

// ─── Item (44 keys reales del fixture + 2 nuevos del mapper api-server) ───
export const RejectionSchema = z.object({ code: z.string(), detail: z.string().nullable() });
export const AmountInStateSchema = z.enum(["sized", "trigger_value", "trigger_value_zero", "never_sized"]);
export const OpportunityLiveItemSchema = z.object({
  id: z.string(), chain_id: z.number(), strategy_kind: z.string(),
  cartridge_id: z.string().nullable(),
  dex_a: z.string(), dex_b: z.string().nullable(), pair_symbol: z.string().nullable(),
  token_in: z.string(), token_out: z.string(),
  token_in_info: TokenInfoSchema.nullable(), token_out_info: TokenInfoSchema.nullable(), // espejo wire
  leg_symbols: z.record(z.string(), z.string()).nullable(),
  chain_base_token_symbol: z.string().nullable(),
  // R8: en rechazados '0' NUNCA llega ambiguo (mapper api-server §5.4-B1). null = no computado.
  amount_in_wei: z.string().nullable(),
  amount_in_state: AmountInStateSchema, // 'sized' | 'trigger_value' (>0 real del tx) | 'trigger_value_zero' | 'never_sized'
  expected_profit_usd: z.number().nullable(),          // Bruto
  net_expected_profit_usd: z.number().nullable(),      // Neto
  roi_pct: z.number().nullable(), risk_score: z.number().nullable(),
  rejection_reason: z.string().nullable(),             // back-compat string (no borrar del wire)
  rejection: RejectionSchema.nullable(),               // estructurado {code, detail}
  paper_status: z.enum(["paper_viable", "paper_rejected"]).nullable(),
  status: z.string(), detected_at: z.string(), trace_id: z.string(), block_number: z.number().nullable(),
  chains_used: z.array(z.number()), dexes_used: z.array(z.string()),
  chain_id_out: z.number().nullable(), bridge: z.string().nullable(), bridge_fee_usd: z.number().nullable(),
  route_metadata: RouteMetadataSchema.nullable(),
  simulated_net_profit_usd: z.number().nullable(),
  simulated_amount_in_usd: z.number().nullable(),
  simulated_roi_pct: z.number().nullable(),
  simulated_cost_breakdown: SimulatedCostBreakdownSchema.nullable(),
  simulated_target: SimulatedTargetSchema.nullable(),
  simulated_at: z.string().nullable(), simulated_notes: z.array(z.string()).nullable(),
});
export const OpportunitiesLiveEnvelopeSchema = z.object({
  count: z.number(), window_total: z.number().optional(),
  window: z.string(), max_age_seconds: z.number().optional(), viable_only: z.boolean().optional(),
  items: z.array(OpportunityLiveItemSchema), ts: z.string(),
});
```

**Tabla fail-honest por campo (null vs 0 — LEY R8, la card la cita en el tooltip):**

| Campo | `null` significa | `0` significa | Fuente |
|---|---|---|---|
| expected_profit_usd | murió antes del profit-math (spot gate 3h, v3_quote_unavailable, single_pool) | computado y exactamente cero | wire hoy |
| net_expected_profit_usd | rechazo pre-spine sin gross / sizing-reject sin carrier | gross cubierto exacto por gas | RD-02 A4 |
| amount_in_wei | never_sized (3h engine-reject) o trigger zero (2h) | PROHIBIDO el '0' ambiguo en rechazados | mapper B1 |
| amount_in_state | — (enum siempre presente) | — | mapper B1 |
| simulated_* | RULE 00: rechazados jamás se simulan | sim real dio 0 | sin cambio |
| cycle_spot_product | no triangular / reserves faltantes | S=0 exacto | RD-02 A2 |
| spot_price (leg) | reserves de esa leg faltantes | ratio 0 (pool vacío real) | RD-02 A2/A3 |
| reference_capital_usd | config sin cap para (token, estrategia) | cap declarado 0 | RD-02 A2 (badge CAP SIEMPRE) |
| candidate_amount_usd | engine-rejected (sizing nunca corrió) | sizing computó 0 | RD-02 A3/A4 |
| gas_estimate_usd | ni oracle ni config | ~0 (gas real 0.051 gwei → mostrar 4 dec) | RD-02 A5 |
| roi_pct | sin net ni gross computables | ROI 0 exacto | wire hoy |

### 5.4 PAYLOAD EXTENSION (RD-02 — exactamente esto, campo por campo)

**A. Rust searcher-rs/shared-rs (emit-time — reserves TTL 30s hace el post-hoc no confiable):**
- **A1** `RouteMetadata` (candidates.rs:131-170) += `legs, cycle_spot_product, token_a_price_usd,
  reference_capital_usd, candidate_amount_usd, rejection_detail, gas_estimate_usd, gas_source`
  (`#[serde(default, skip_serializing_if = "Option::is_none")]` — rows viejos siguen parseando).
- **A2** `triangular_engine.rs:431-435` → al rechazar poblar: `legs` (spot raw+fee-adj de `r_f64`,
  reserves orientadas por leg), `cycle_spot_product = sp` (ya computado y descartado),
  `token_a_price_usd`, `reference_capital_usd` (`effective_capital_for` :678),
  `rejection_detail = "S={sp:.4} ≤ 1"`. **NO relajar el gate** (teorema S≤1 — el ciclo NO gana con
  ningún monto positivo; sizing sería desperdicio).
- **A3** `size_optimizer.rs`: carrier en `OptimizeOutcome::Rejected` (:156-186) =
  `Option<RejectedNumbers { gross_usd, net_usd, amount_in_wei, detail }>` — en gas_floor/kelly/net_negative
  los números YA se computan (:507-533,559) y hoy se descartan; y legs spot 2-hop (:856-879) para
  sizing-rejects.
- **A4** `orchestrator.rs` branch temprano (:1119-1143): (a) attach RouteMetadata enriquecido TAMBIÉN en
  rechazos (patrón :1049-1111); (b) si gross existe → `net = gross − gas` (patrón :1330-1332);
  (c) si el carrier trae números → poblar expected/net/candidate_amount_usd.
- **A5** `resolve_gas_price_gwei` (trading_config.rs:452-479) lee Redis `arbx:gas_price_wei:<chain>`
  (gas_oracle_worker.rs:146, 10s/TTL 60s; VPS real 0.051 gwei) con fallback = config actual etiquetado
  `gas_source:'config_fallback'` (hoy cae SIEMPRE a 1 gwei ≈ 20× el real).
- **A6** `extract_pricing` (triangular_engine.rs:656-679) lee snapshot Redis `arbx:token_prices:<chain>`
  PRIMERO (cascada canonizada price_oracle.rs:105-117) — cierra el drift −5.2% del base_token_price_usd
  stale (2350.79 config vs 2479.18 Chainlink vivo).
- **Tests Rust** (cargo test -p searcher-rs / shared-rs): extender los tests existentes del engine
  (p.ej. `spot_product_le_one_emits_rejected`) para asertar el route_metadata enriquecido; los valores de
  aserción salen de las reserves del propio test, jamás hardcodeados mágicos.

**B. api-server (mapper — route_metadata pasa verbatim :617-622, CERO migración SQL):**
- **B1** `rowToOpportunity` (opportunities-live.ts:560-633): `amount_in_wei` honesto —
  `status==='rejected' && amount_in_wei==='0'` → `null` + `amount_in_state`:
  `strategy_kind==='triangular'` → `'never_sized'`; sino → `'trigger_value_zero'` (el '0' 2-hop ES
  tx.value real del trigger: se ETIQUETA la verdad, no se borra). Rechazado con value>0 → mantener wei +
  `'trigger_value'`. No-rechazado con monto → `'sized'`. (Derivación de estado real — RULE 00 intacta.)
- **B2** `rejection = { code: rejection_reason, detail: route_metadata?.rejection_detail ?? null }`.
- **B3** Passthrough del resto (legs/cycle/caps/gas ya viajan dentro de route_metadata).
- **B4** `websocket.ts broadcastOpportunity` (:447-462) emite el item ENRIQUECIDO (misma
  rowToOpportunity) — mata BUG-03 (push crudo empobrece snapshot) EN ORIGEN. El store FE además hace
  merge por campo (§5.5) por defensa en profundidad.
- **Tests api-server**: edge-parity + opportunities-live con fixture REAL
  `fixtures/live-payload-rd01c.json` (regresión: 13 triangular DEBEN llevar legs+S tras deploy; hasta
  entonces renderizan null honesto + razón).

**C. Se queda null honesto (RULE 00 — JAMÁS inventar):** `simulated_*` en rechazados (forwardSimulate
exige amount>0+gross; simular con CAP sería sim falsa — decisión RD-01c); `amount_in_wei/usd` y
`candidate_amount_usd` para engine-rejected 3-hop (sizing nunca corrió — no existen); spot de legs sin
reserves al tick.

**D. Edge worker:** sin cambios (JSON verbatim); correr parity tests existentes.

### 5.5 OpportunityCard API (RD-05) — `frontend/components/opportunities/card/`

Nace de `OmniOpportunity` extendido (mapper ÚNICO `mapToOmniOpportunity`), NO de OpportunityRowSchema.
Absorbe XRayCard + OpportunityTradeCard + OpportunityExchangeCard + OpportunitySummaryGrid.

```
OpportunityCard props:
  data: OmniOpportunity            // del store SSOT
  variant: 'full' | 'compact'      // full = /opportunities + exchange; compact = home top-N
  onOpenDetail?: (id: string) => void
Sub-componentes (exports separados para test):
  MoneyField { label, value: number|null, kind: 'usd'|'pct'|'ratio'|'wei'|'gwei',
               state?: 'computed'|'cap_reference'|'estimated'|'not_computed', hint? }
    // null → "—" + tooltip con la razón de la tabla 5.3; cap_reference → badge "CAP";
    // estimated → badge "EST." (gas fallback). JAMÁS coerce a 0.
  LegRow { index, leg: RouteLeg(+spot/amounts opcionales), synthetic? }
    // token_in→token_out (leg_symbols), dex, pool shortAddr, spot raw (+fee-adj tooltip),
    // per-leg USD si llega; synthetic → marcador SYNTHETIC LEGACY VIEW
  HopBadge { hops: number|null }   // "2 HOPS" / "3 HOPS" / "—" — definición única §5.2
  VerdictPill { paper_status, status, rejection? } // Viable/Rechazada + lifecycle + rechazo decodificado
```

**Streaming delta (doctrina CB-03):**
1. Store upsert = **merge por campo** (omni-store.ts:325-397): por cada key del incoming, set si
   `incoming[key] !== undefined`; JAMÁS borrar keys ausentes del push (conserva
   leg_symbols/token_info/simulated_* del snapshot).
2. Card = `React.memo` con comparator exacto sobre TODOS los campos que renderiza (money fields +
   rejection + legs JSON + paper_status — FIX-4 de FE-02a nace aplicado; heredado por ambas páginas).
3. El comparator evita re-render si el número no cambió; el store notifica por referencia de item.

**Estados honestos visibles (obligatorios):** NOT_COMPUTED ("—" + razón) · CAP_REFERENCE · ESTIMATED ·
SYNTHETIC LEGACY VIEW · QUARANTINED (semantic_violations no vacío) · NUEVA (<12s).

**Jerarquía de la card full (orden de lectura = lo que el operador quiere PRIMERO):**
1. Header: VerdictPill + HopBadge + par/ruta corta + LED NUEVA + edad.
2. Dinero principal: **Neto** (fallback Sim Net con badge SIM) | **Bruto** | **Capital** (sized; si no,
   Capital ref CAP) | ROI.
3. Ruta: LegRow por hop (token→token, dex, spot) + (3-hop) **S del ciclo**.
4. ¿Por qué sí/no?: rejection {code+detail} decodificado, o gates si viable.
5. Meta: gas est. (fuente), risk, trace corto.

### 5.6 LAYOUT desde 0 (RD-03 / RD-04)

**`/opportunities` (RD-03):** `page.tsx` Server puro (R1): fetch REST `?limit=50` → initialSnapshot
parseado con `OpportunitiesLiveEnvelopeSchema` → `<OpportunitiesFeedClient initialSnapshot>`. Client:
header ("Detecciones" + badge TRANSPORTE + window_total + counts viable/rechazada con el predicado único)
→ FilterBar unificada (Veredicto [Todas|Viables|Rechazadas] — filtro client-side SIEMPRE activo;
Hops [Todas|2|3+]; Estrategia; búsqueda) → grid OpportunityCard full → DetailDialog (tabs
Economics/Route/Ledger/Simulation/Provenance consumiendo MoneyField/LegRow). WS
`subscribe:opportunities` + fallback poll 5s (RULE 02: WS→api-server 8080 directo).

**`/opportunities/exchange` (RD-04) — "Mercados & precios":** mismo patrón SSR + initialSnapshot.
Layout: PriceTicker arriba → lista ordenable por (Neto | S del ciclo | spread) agrupada por par → al
seleccionar, **CycleDetailPanel** (NUEVO) con LegRow completo (spot real por leg = "Buy px/Sell px"
REALES) → misma FilterBar (mismos chips/predicados §5.2). Cards = la MISMA OpportunityCard full.
`DetectionDiagnosticCard`/`isUnevaluatedShell`/grossOut-dual se ELIMINAN (una sola representación).

**`/` home (RD-03):** cockpit: StatCards honestas (Best Net → "—" si 0 computados; Decoherencia =
slippage medio real o "—") + ticker (migra a SSOT Zod) + top-3 OpportunityCard compact. XRayCard se
absorbe; nada de "TLS AMOUNT: —" hardcodeado → MoneyField.

**`/opportunities/by-strategy` (RD-03):** tabla por strategy_kind: detecciones, viables, rechazadas,
gross/net sumando SOLO valores no-null con contador honesto ("Neto medio (12/40 computados)"),
top-3 rejection_reason. fetch con URL del edge (no relativa). Sin coerce `?? 0` (fix R8).

**Redirect:** `next.config.js` → `{ source: '/opportunities/live', destination: '/opportunities',
permanent: true }` (308).

### 5.7 ALINEACIÓN OBLIGATORIA con hallazgos RD-01b (fixes incluidos en los builders)

1. Filtro viable client-side en /opportunities (fin del NO-OP) — RD-03.
2. by-strategy sin `?? 0` — RD-03.
3. Comparator memo cubre simulated_*/risk/paper — RD-05 (nace en la card nueva).
4. Merge por campo en store (BUG-03) — RD-05 + RD-02 B4 en origen.
5. Dos schemas → UNO (ticker migra; OpportunityRowSchema deprecado) — RD-05.
6. **Cruft — propuesta al orquestador para el PR ship (nadie borra en builders):**
   `app_backup/`, `components_backup/`, `features_backup/`, `app/page.tsx.backup`,
   `live-ticker.tsx`, `useOpportunitiesStream.ts`, `websocket-client.ts`,
   `OpportunitiesTable.tsx`, `OpportunityEvidenceCell.tsx` — huérfanos (0 importers vivos),
   duplican conceptos y envenenan greps (contienen los únicos "renderers" de campos muertos).

### 5.8 ACCEPTANCE (browser-verificable; [live_only] = requiere deploy de RD-02)

Ver lista completa en el output estructurado RD-03a (AC-1..AC-17) — resumen:
redirect live→/opportunities (local) · counts consistentes con predicado único · toggle Viables filtra en
modo WS · card 3-hop rechazada muestra S + spot por leg + CAP + rechazo decodificado [live_only] ·
amount "—" con razón (nunca '0' ambiguo) [live_only] · 2-hop non_positive_profit con Neto = gross−gas
[live_only] · cero "$0.00" fabricados en by-strategy · misma fila idéntica en /o y /x · push WS no
empobrece (leg_symbols persisten) [live_only] · Buy/Sell px reales o "—"+razón (adiós hardcode)
[live_only] · HopBadge único en todas las superficies · home honesta · una sola card en exchange ·
console limpia + sin hydration mismatch (R1) · LED "NUEVA" no "LIVE" · vitest+tsc verde con fixture REAL.

### 5.9 FILES_PLAN (dueño por archivo — SIN solapamiento de writers)

Ver output estructurado RD-03a. Regla: RD-02 = backend+api-server; RD-05 = schemas/store/card-lib
(COMÚN: RD-03 y RD-04 solo CONSUMEN la card); RD-03 = feed/home/by-strategy/redirect;
RD-04 = exchange; RD-06 = verify (no escribe código de producto).

## §6 — RD-01a TRUTH-BACKEND (forense del dinero, 2026-09-10) — file:line exactos

Pipeline canónico verificado: `RouteIntent` (mempool ó RU-3 scanner) → orchestrator
(`on_route_intent`: engines fan-out Step 5 → sizing Step 6 → `process_candidate` →
`OpportunityEmitter::emit_rejected/accepted` → PG `opportunities` + stream Redis) →
`opportunities-live.ts` LIVE_QUERY (PG) → payload. El worker legacy 3-hop
(`triangular_worker.rs:1410-1420`) hace skip silencioso en evaluate_cycle=None —
**NO** emite los rechazos; los rows `spot_product_le_one` vivos SOLO salen del engine V2
(`triangular_engine.rs`) vía orchestrator. El emitter es passthrough: no puebla dinero.

### Q1 — ¿Dónde se computa `expected_profit_usd` y por qué tan pocos lo llevan?
- **2-hop (dex_arb)**: `dex_engine.rs:309-317` — spread de probe sobre reserves V2 reales
  (`compute_gross_usd`) o V3 projector (`compute_v3_gross_usd:415`); lo escribe directo en la
  Opportunity (`build_accepted_opportunity:761`). Si el sizing rechaza después, el gross
  sobrevive (HARDENING `orchestrator.rs:986-991`).
- **3-hop (triangular)**: `triangular_worker.rs:824-825` (profit_token × price, dentro de
  `evaluate_cycle`), copiado a la opp en `triangular_engine.rs:490` SOLO en el camino aceptado.
- **Por qué casi nadie lo lleva**: (a) 3-hop muere ANTES del profit-math en el spot gate
  (`evaluate_cycle` retorna None → rechazo con gross None, `triangular_engine.rs:426-455`);
  (b) 2-hop con pool V3 → `v3_quote_unavailable` (gross None por construcción,
  `dex_engine.rs:333-357`); (c) solo dex V2/V2 con spread precio sobrevive → los pocos
  rows con gross (fixture RD-01c: 4/37 dex_arb `non_positive_profit` con gross 0 ó 0.0002,
  net=None). "2/43" del §0 es varianza de ventana de captura (misma mecánica).

### Q2 — Punto exacto de `spot_product_le_one` + datos vivos ahí
- Gate matemático: `triangular_worker.rs:760-762` (`spot_product()` definido en `:227-240`,
  S = γ³·∏(R_out/R_in)). Etiqueta de rechazo: `triangular_engine.rs:436-441`.
- **Datos REALES vivos en ese punto** (todos descartados hoy):
  - `hop_reserves` orientados por leg `(reserve_in, reserve_out)` U256 —
    `fetch_hop_reserves` `triangular_engine.rs:386-399`; fuente ReservesCache hidratada de
    Redis `arbx:pool_reserves:<chain>:<pool>` (`triangular_engine.rs:109-120`,
    keys `reserves.rs:86-87`, TTL 30s, escritor pool_sync_worker).
  - `r_f64` por leg + **`sp` (cycle spot product) YA COMPUTADO y descartado** —
    `triangular_engine.rs:431-435`.
  - Precio spot por leg derivable: `R_out/R_in` (raw) o `γ·R_out/R_in` (fee-ajustado) —
    misma data.
  - `token_a_price_usd` + `cap_usd` — `extract_pricing` `triangular_engine.rs:416,656-680`
    (WETH=base_token_price, stables=1.0, resto None) y `effective_capital_for`.
  - **Monto del SizeOptimizer: NO EXISTE** — sizing se SALTA para engine-rejected
    (`orchestrator.rs:866-880`). Solo existe cap_usd (cota del operador).

### Q3 — `net_expected_profit_usd`: dónde DEBERÍA poblarse y por qué 0/50
Sitios de población en el path canónico (solo 3):
1. `orchestrator.rs:941/948` — cuando `OptimizeOutcome::Sized` (sizing exitoso);
2. `orchestrator.rs:1330-1332` — spine Evaluated CON rechazo (net = gross − gas);
3. `orchestrator.rs:1354-1355` — spine Evaluated limpio (net = outcome.net_profit_usd).
**0/50 porque TODAS las filas observadas fueron rechazadas ANTES del spine**: los
engine-rejected (3-hop spot gate) y los sizing-rejected (`non_positive_profit` etc.) llevan
`rejection_reason` seteada y caen en el branch temprano `orchestrator.rs:1119-1143`
("Engine-level rejection: no need to evaluate, just emit rejected") que emite ANTES de
cualquier población de net. Además `OptimizeOutcome::Rejected` NO transporta números
(`size_optimizer.rs:156-186`): en gas_floor_breach/kelly se computan gross/net/amount y se
descartan (`apply_kelly_constraints:507-533,559`).

### Q4 — `simulated_*`: origen y cableado
- En el payload live NO vienen del simulador Rust: se computan EN RUTA —
  `opportunities-live.ts:807-877` (`forwardSimulate`/`inverseSize`) con fallbacks `:498-543`.
- `forwardSimulate` (`computeSimulatedNet.ts:222-233`) exige gross≠null **Y**
  `amountInUsd`≠null (amount>0 + precio, `:173-192`) → para filas con amount="0" y/o
  gross null devuelve null SIEMPRE; el fallback `simulated_amount_in_usd` es literal
  `return null` (`opportunities-live.ts:513`). ⇒ 0/50 estructural.
- El simulador Rust (sim-ctl/revm) escribe en tabla `simulations`
  (`sim-ctl/persistence.rs:30-48`) y el paper executor en `paper_trade_runs`
  (`paper/executor.ts:348-368`) — **NINGUNA está joineada por LIVE_QUERY** ⇒ no llegan al
  payload por diseño actual.

### Q5 — Quién escribe `amount_in_wei="0"` literal
El contrato Rust `Opportunity.amount_in_wei` es `String` NO-opcional
(`shared-rs/src/contracts.rs:55`) ⇒ None no expresable. Dos escritores del "0":
1. **3-hop**: `triangular_engine.rs:570-572` — `unwrap_or_else(|| "0".to_string())` cuando
   el rechazo no tiene monto (NUNCA fue un tamaño: no-cómputo coercido a 0 = violación R8).
2. **2-hop**: `dex_engine.rs:748` usa `intent.amount_in` = `tx.value` del tx mempool
   (`route_intent.rs:41-44`) ó `0` explícito de RU-3 (`route_scanner_worker.rs:29,381`) —
   es un 0 REAL del trigger, pero igual NO es un tamaño calculado.
Persistencia: `searcher-rs/persistence.rs:85-86` (BigDecimal). El probe 1e18 solo existe en
`build_rejected_opportunity` (`dex_engine.rs:821`).

### Q6 — DECISIÓN RD-02: campos REALES emittables para rejected sin violar RULE 00
| Campo | Fuente de verdad file:line | Estado hoy | Emittable |
|---|---|---|---|
| per-leg `spot_price` (R_out/R_in, raw y fee-adj) | reserves orientadas EN MANO: 3-hop `triangular_engine.rs:431-434`; 2-hop `size_optimizer.rs:856-879` | descartado | ✅ emit-time (TTL 30s — post-hoc no confiable) |
| `cycle_spot_product` | `triangular_engine.rs:435` (computado y descartado) | descartado | ✅ gratis |
| per-leg `reserves` (in/out) | mismas reserves; keys Redis `reserves.rs:86-87` | descartado | ✅ (peso: strings wei) |
| `cap_usd` / capital cap | `triangular_engine.rs:678` (`effective_capital_for`) | interno | ✅ etiquetado como CAP (no como monto) |
| gross/net/amount EN sizing-rejects (gas_floor/kelly/net≤0) | `size_optimizer.rs:507-533,559` (numeros calculados, descartados) | descartado | ✅ requiere carrier en `OptimizeOutcome::Rejected` |
| gas estimate | Rust `state.gas_cost_usd()` (`size_optimizer.rs:736,956`) ó TS `gasCostUsd(cfg)` (`computeSimulatedNet.ts:201-207`, snapshot Redis trading_config) | no viaja | ✅ real |
| rejection_reason estructurado | string plano ya en payload (`opportunities-live.ts:600`) | ✅ plano | ✅ enriquecer con el valor numérico del gate |
| monto SizeOptimizer para engine-rejected | NO EXISTE (sizing saltado `orchestrator.rs:866-880`) | — | ❌ no inventar; solo cap_usd |
| Fix R8 amount_in_wei | escritores Q5 + `contracts.rs:55` | "0" | decisión RD-02: (a) Option<String> en contrato (invasivo) ó (b) mapper honesto en `rowToOpportunity` (rejected + "0" → null). NOTA: el "0" 2-hop ES tx.value real — distinguir etiqueta |

**Vehículo**: `RouteMetadata` (`shared-rs/candidates.rs:131-169`) ya soporta
`leg_amounts_in/out` (wei exacto) + `decimals` map; el payload pasa `route_metadata`
verbatim (`opportunities-live.ts:617-622`) ⇒ campos nuevos llegan al frontend SIN cambio
de SQL (columna JSONB). Cero mocks: todo sale de reserves/precio/config reales del tick.

### Correcciones/refutaciones al §0 (para pares)
- "2/43 con gross" es varianza de captura (fixture RD-01c: 4/37, mismo mecanismo) — la
  mecánica es la documentada arriba, no un bug distinto.
- Precisión de causa en §0.1: el 3-hop no solo "no llega a sizing" — es que el branch
  temprano `orchestrator.rs:1119` emite rechazos ANTES del spine, que es donde net se
  puebla. Net 0/50 vale TAMBIÉN para los que sí llevan gross.
- `simulated_*` 0/50 NO es falta de cableado de sim-ctl al payload (eso también es cierto,
  tablas no joineadas) — es que la ÚNICA fuente viva de esos campos (forwardSimulate TS)
  exige amount>0+gross, imposible con amount="0".

## §7 — INCIDENTE ORQUESTADOR (2026-09-10 ~05:4xZ): edge/ borrado del árbol compartido

- `edge/` completo (21 archivos, tracked) apareció DELETED (` D`) en el árbol compartido durante
  la ventana BUILD del gang. Restaurado con `git checkout -- edge/` (versión HEAD de la branch).
- La versión UNCOMMITTED de `edge/worker/src/index.ts` (con las 10 asimetrías de paridad incl.
  `/api/v1/control-board`, la que hacía pasar `edge-parity.test.ts` a las 04:43Z) está PERDIDA:
  no está en HEAD, no en el stage del PR #558, no en transcripts del gang.
- **EFECTO para RD-06/RD-02**: el árbol tiene ahora test de paridad NUEVO + worker VIEJO →
  `edge-parity.test.ts` FALLARÁ en vitest local. NO es regresión del gang. Si el fixer la toca:
  la acción correcta es emparejar (revertir el test a la versión que matchee el worker presente)
  o reescribir el worker según la spec de paridad del test — nunca dejar el par inconsistente.
- El upgrade CB de paridad (worker+test juntos) reaterriza en PR propio post-gang (RD-07).
- Forense del borrador: sin `rm`/`git rm` en transcripts del gang. Sospechosos restantes: algún
  test de api-server con fs.rm y path relativo escapado (corrió vitest 04:43-05:00Z), o agente
  no auditado. Pendiente identificar para cerrar como lección LEARNINGS.
