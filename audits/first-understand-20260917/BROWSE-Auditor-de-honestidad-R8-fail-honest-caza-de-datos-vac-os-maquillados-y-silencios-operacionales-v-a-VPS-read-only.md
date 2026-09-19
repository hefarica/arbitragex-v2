# BROWSE — Auditor de honestidad R8 (fail-honest): caza de datos vacíos maquillados y silencios operacionales vía VPS read-only

**Fecha:** 2026-09-17, 06:21–06:26 UTC · **Autor:** Auditor R8 (Gang Omniscience, oleada first-understand)
**Postura:** §32/§33 audit/scaffold/shadow/read-only TOTAL. Cero comandos de escritura (sin SET/DEL/TRUNCATE/restart/compose). Cero git. Cero flips.
**Veredicto corto: FE — la DApp muestra lo que dice mostrar; los datos son reales (RULE 00); los vacíos se declaran (R8). 4 gaps honestos reportados (1 bug real de paridad, 1 silencio de UI, 1 badge engañoso, 3 higiene VPS).**

---

## 1. Journey VPS read-only (evidencia cruda)

### 1.1 Censo de contenedores — `docker ps`

24/24 contenedores `(healthy)`. Flota completa "Up 3 hours" (reinicio ~03:38–03:46 UTC hoy, causa aún no determinada — consistente con `02-VPS-REMAP-20260917.md` hallazgo 4). Hot-path presente: `searcher-rs`, `sim-ctl`, `selector-api`, `relays-client`, `recon`, `math-engine`, `token-enricher`, `edge`, `api-server`, `socket-proxy`, `redis`, `postgres`, `anvil`.

### 1.2 Regla R9 aplicada ANTES de concluir ausencia (LOGFLOOD-01 discipline)

| Contenedor | LogConfig | StartedAt | Primera línea retenida | Veredicto ventana |
|---|---|---|---|---|
| searcher-rs | max-file:5 max-size:10m | 03:38:30Z | **06:05:00Z** | **ROTADA** — gap de ~2h27m. Cualquier "ausencia" de markers de arranque/fase temprana en logs de searcher es ARTEFACTO, no evidencia |
| selector-api | max-file:5 max-size:10m | 03:38:30Z | 03:38:31Z | Íntegra (boot visible: `service.boot` port 3002, circuit_breakers declarados) |

### 1.3 searcher-rs `--tail 200`

- Flujo vivo: `v2.orchestrator.intent_received` (chain 1, legs 4) → `v2.impact.resolved` (15 pools, 3 ciclos).
- Emisión fail-honest: `v2.emitter.input` con `"rejection_reason":"Some(\"spot_product_le_one\")"`, `"expected_profit_usd":"None"`, `"net_expected_profit_usd":"None"` — **None declarado como None, no 0 ni inventado**. R8 correcto en el productor.
- **Tormenta 429 Alchemy**: ~24 WARN `price_worker.alchemy_failed` (chunk 2–5) en <1 s, todas con fallback Coingecko anunciado. Silencio operacional LATENTE: si Coingecko también falla, ¿qué se registra? (los nulls del stream sugieren `unknown_token_price`/`no_price_oracle`, presentes en taxonomía). No es maquillaje, es degradación ruidosa y declarada.
- **Higiene**: la URL del error expone la API key parcial de Alchemy en texto plano del log (`alch_7Yw8…-jYp`). Redacted-logger no cubre este path.
- `pool_sync.v3_sqrt_overflow` WARN en 2 pools (valor = MAX_UINT160-ish): manejo defensivo activo.

### 1.4 selector-api `--tail 200`

Solo `/metrics` y `/health` (200, 0–5 ms). **Silencio de consumo en la ventana**: ningún marker de validación/selección de oportunidades en las últimas ~200 líneas. No concluyo "no consume" (R9: la ventana íntegra arranca en boot; el consumo real se mide abajo por Redis/PG). Nota de mesa: SEL-GATE-01 (fdb40401, dejar de publicar producer-rejected al stream validado) sigue **NO desplegado** — el VPS corre `a06a968d` (ver 1.6).

### 1.5 Redis (lectura)

- `XLEN arbx:opps:detected` = **10.004** (trim cap), `entries-added` = **17.919.355**, `last-generated-id` fresco, 3 consumer groups.
- First-entry sample: `"expected_profit_usd":null`, `"net_expected_profit_usd":null`, `"rejection_reason":"v3_quote_unavailable"`, `detected_at 06:17:00Z` → **stream VIVO y fail-honest**.
- **Higiene**: `AUTH failed: called without any password configured` — Redis corre SIN password (el `-a $REDIS_PASSWORD` del exec falló porque no hay password configurado). Solo red interna docker, pero contradice la postura zero-trust §4.2.

### 1.6 PostgreSQL (SELECT only)

- `MAX(detected_at)` = 06:21:15.9 vs `NOW()` = 06:21:17.1 → **2 segundos de latencia**. Pipeline fluye caliente.
- Últimos 15 min: **34.262** oportunidades. Últimas 2 h: **271.787**, 100% `status='rejected'` (única categoría). Coincide con la taxonomía 09-06 (48.4K/24h 100% rejected) y con la UI (ver §2).

**R7 cruce de capas: no hay corte.** Searcher detecta → Redis recibe → PG persiste → api/edge sirve → DApp renderiza. La capa exacta donde el dato NO llega es NINGUNA; el único corte parcial es un widget concreto (ver 2.4).

---

## 2. Journey DApp (https://arbx.ape-tv.net) — "¿muestra lo que dice mostrar?"

### 2.1 Header / postura — CORRECTO

`PAPER · TLS SHADOW` · `KILL-SWITCH <10MS` · System Guard: Live OFF, Private relay OFF, Submit OFF, Broadcast OFF, Paper ON · EXPLICIT, **Capital: $0**, Readiness 4/4, LIVE lock REVIEW, **GO live: NO-GO**, `KILL SWITCH OFF · PAPER MODE ON`. §34.3 visible y respetado: default-deny en pantalla, no hay botón de flip tentador engañoso.

### 2.2 `/opportunities` — HONESTO, con un silencio de UI (GAP-1)

- Contador: **"0 viable / 200 total (200 rejected)"** — coincide con PG (100% rejected) y con la API.
- `GET /api/opportunities/live?limit=20` → 200, body verificado: 20/20 `status:"rejected"`, **20/20 con `expected_profit_usd: null`** (no 0, no inventado), `rejection_reason` poblado (`v3_quote_unavailable`×14, `spot_product_le_one`×5, `single_pool_no_spread`×1), `detected_at` = 06:21:48Z (fresco). `window_total` 12.155 en ventana 300 s — consistente con 34.262/15 min de PG.
- Tarjetas: badge REJECTED + economía completa en "—" (`None`). **CORRECTO R8**: el vacío se declara.
- **GAP-1 (silencio de UI)**: la `rejection_reason` NO se renderiza en ninguna tarjeta del feed (0 ocurrencias de las razones en `document.body.innerText` aunque la API las trae). El operador humano ve "REJECTED" sin POR QUÉ en esta vista. La razón SÍ es visible en `/operations` (taxonomía agregada) — el corte es de densidad informativa del feed, no de verdad. Clasificación: INFERIDO (mejora UX R8), no violación RULE 00.

### 2.3 `/live-readiness` — HONESTO, con rojos visibles

- Secuencia de ignición 4/4 PASS con evidencia server-side real (4 WSS + 5 RPC activos; signer presente redacted; 6 chains/23 dex/1315 pools/4381 tokens; 5 motores paper=true live=false). Banner: "Ningún paso futuro se marca verde por simulación".
- **17/19 verificados, 2 FAILING, 0 NOT STARTED** — rojos a la vista, no maquillados:
  - `G-SIM-1` FAILING con causa explícita: "premature flag — SECURE_BOOT violated: ARBX_SIMULATOR_V2_READY=true with evidencia 4/7; missing [modules_merged, eth_callbundle_staging, second_signoff] — stale (>30d)".
  - (el segundo FAILING quedó fuera del excerpt leído; registrado como pendiente de lectura, no inventado).
- **"LIVE HABILITABLE: PAPER_MODE · FLIP BLOCKED"** — coherente con §34.3 y con HG-certificación 0/10 (09-06).

### 2.4 `/operations` — el hallazgo más valioso: un bug REAL atrapado por fail-honest

- Taxonomía: **"654,710 rechazadas de 654,710 oportunidades · 24 h" ("100% rechazadas")** — sin maquillar. Desglose: v3_quote_unavailable 76.8% ($1.65 gross prom.), spot_product_le_one 12.4%, non_positive_profit 5.5% ($3.326 gross prom. — rechazadas por gate, no ejecutadas), etc.
- Caveat explícito en pantalla: **"Nulo ⇒ ausencia real (R8), jamás un cero"** — doctrina R8 literal en la UI.
- `edge HTTP 401 missing_admin_token` y `503 /api/route-discovery-outcomes/summary` surfzados como texto de error (el 503 = fail-safe CB-02 conocido, ver memoria toggle-channels 09-16). **No se ocultan ni se simulan.**
- **GAP-2 (bug real, GAP-3 en espíritu G2 §37)**: widget S-Curve pegado en:
  > "Opportunity feed unavailable — edge response shape invalid: items.0.block_number: Expected number, received string; items.1.block_number: Expected n… (retrying every 30s)"

  Error que se reproduce en `/status` también. Causa anclada en código local: el edge serializa `block_number` (bigint PG) como **string**, el consumidor frontend valida `z.number()` → `frontend/lib/schemas.ts:66` y `:1182` (`block_number: z.number()...`), rechazo emitido en `frontend/lib/api-client.ts:431` (+ :181,:228,:466,:494,:522,:565 mismos patrones). El feed principal de `/opportunities` NO falla porque normaliza (`frontend/lib/store/types.ts:411`: `Number(raw.block_number)`). **Clasificación: CANONICAL_REPO (paridad frontend↔edge, gate G2 de §37 — hueco ya declarado por doctrina, aquí con reproducibilidad en vivo).** El cliente se negó a renderizar datos fuera de schema = fail-honest funcionando; pero el widget lleva (desconocido cuánto tiempo) sin datos = silencio operacional visible que la propia UI declara. Reparable agent-side (coerción `z.coerce.number()` o normalización en types.ts) pero NO aplicado (NO-GIT).

### 2.5 Prueba de falsabilidad del header (no planeada, bienvenida)

Mi cliente perdió conectividad unos segundos (`ERR_INTERNET_DISCONNECTED` en consola — **artefacto local MÍO, excluido como evidencia contra la DApp**). Durante el corte el System Guard degradó a **"A.4 fork: UNKNOWN · A.5: UNKNOWN · PG edge_error · Redis edge_error · HB stale/absent"** y al reconectar volvió a **PASS / PG ok / Redis ok / HB fresh**. Conclusión: los badges son server-derived en vivo, **no verdes de caché**. Exactamente el anti-"green congelado" que R8 exige.

### 2.6 `/status` — deploy veraz + servicios

- `AS OF 06:25:33 UTC` (auto-refresh 5 s). OVERALL OK, 7 servicios UP HTTP 200, KILL-SWITCH disabled, PAPER-MODE ON, "Real capital is not at risk until S9".
- **Ancla de deploy en pantalla: `a06a968d044a7bc7ed4b57ddc5a418d9cf800659` · run 35175965528 · 2026-09-17T03:12:11Z** — confirma el remap de pares (02-VPS-REMAP): los 3 commits locales (fdb40401 SEL-GATE-01, 125b1e0b SIM-FUND-01, dcfe890c PANCAKE-ROUTER-01) **NO están en producción**. La UI no miente sobre qué corre.
- Service Controls: gated, con semántica 501/404/502 documentada en pantalla.

### 2.7 `/opportunities/exchange` — feed WS vivo

`FEED LIVE` · `Last refresh: 1:25:07 a. m.` (06:25 UTC; antes 1:21:52 → **tick-ea**, no congelado) · MATCHING 200 / TOTAL DETECTED 200. socket.io conectado vía polling (EIO=4, sid estable, POST/GET 200) con intento de upgrade wss. Dato vivo verificado.

### 2.8 Mini-canales del header — GAP-3 (badge engañoso)

`socket: CONNECTING · routes: LIVE/DEGRADED (flap) · runtime_ack: LIVE · pairs: CONNECTING · quote_anchor: CONNECTING` — persistentes en TODAS las páginas y sesiones observadas, incluso con FEED LIVE y refresh tick-eando. O bien esos badges trackean el upgrade websocket (que no completa) y están mal etiquetados, o hay canales que nunca conectan y nadie alerta. **No es un dato mentirodo (el feed funciona), es un indicador que muda a estado intermedio permanente** — ruido que entrena al operador a ignorar badges. Clasificación: INFERIDO (defecto de display), reproducible en las 5 screenshots.

---

## 3. Consolidado de gaps (nada maquillado)

| # | Gap | Capa | Clasificación | Bloqueo |
|---|-----|------|---------------|---------|
| GAP-1 | `rejection_reason` no renderizada en tarjetas del feed `/opportunities` (API la trae, DOM no la muestra) | frontend | INFERIDO — mejora UX R8 | agent-fixable (diff local, gated NO-GIT) |
| GAP-2 | Paridad edge↔frontend: `block_number` string vs `z.number()` rompe widget S-Curve/status ("Opportunity feed unavailable", retry eterno) | edge/frontend (`schemas.ts:66,1182` vs wire) | CANONICAL_REPO — gate G2 §37 | agent-fixable (coerce/normalizar; diff local, gated NO-GIT) |
| GAP-3 | Badges socket/pairs/quote_anchor "CONNECTING" permanentes con feed funcional; routes flapping LIVE↔DEGRADED | frontend | INFERIDO | agent-fixable |
| GAP-4 | Ventana de logs searcher-rs ROTADA (03:38→06:05) imposibilita forense del arranque post-reinicio de flota | VPS ops | PRIMARY_SOURCE (docker inspect + head -1) | operator-gated (log driver/retención) |
| GAP-5 | Redis SIN password configurado (AUTH rechazado por ausencia de password) | VPS infra | PRIMARY_SOURCE | operator-gated |
| GAP-6 | API key Alchemy parcialmente expuesta en URLs de logs de searcher-rs | backend logging | PRIMARY_SOURCE | operator-gated (redacted logger) |
| GAP-7 | Causa del reinicio de flota 03:38–03:46 UTC + dueño de `/tmp/arbx-deploy.lock` stale (heredado de 02-VPS-REMAP) | VPS ops | UNKNOWN | operator-gated |

Ningún gap es un dato fabricado: son ausencias o indicadores mal etiquetados, todos declarados aquí tal como se observaron.

---

## 4. Sincronía de mesa redonda (cita de pares)

- **02-VPS-REMAP-20260917.md**: confirmado en vivo su hallazgo de flota reiniciada (Up 3h, mismo SHA a06a968d mostrado por /status) y su conclusión de commits locales no desplegados. Añado: la DApp publica el SHA del deploy (veracidad verificable por el propio operador sin ssh).
- **INFORME.md §5 gate 7**: /api/translate 404 en VPS — no re-verificado en esta pasada (presupuesto HTTP); sin contradicción.
- **WO-02c-DESIGN.md** (SEL-GATE-01 local no desplegado): consistente con mi observación de selector-api sin markers de validación y stream con producer-rejected visibles en la UI. No refuto; refuerzo.
- **Memoria taxonomía 09-06** (100% rejected): sigue vigente 11 días después (654.710/654.710 en 24 h).
- Un par generó screenshots `browse-01..03` en el mismo dir durante mi corrida; su reporte aún no existe al cierre (solo screenshots) — sin colisión de hallazgos.

## 5. Línea final

**FE.** La DApp ArbitrageX muestra lo que dice mostrar: datos reales de un pipeline caliente (2 s de latencia PG), vacíos declarados como vacíos (nulls/—/0 viable), rojos visibles (2 FAILING, 100% rechazadas, NO-GO), deploy veraz anclado, y badges que degradan a UNKNOWN ante pérdida de fuente en vez de congelar verdes. **Cero mocks, cero datos maquillados detectados.** Los 3 gaps de producto (razón de rechazo oculta en el feed, schema mismatch block_number con widget caído-declarado, badges CONNECTING permanentes) y los 4 de VPS (log rotado, redis sin password, key en logs, causa de reinicio) quedan fichados arriba con evidencia y clasificación. Sancho checkpoint post-flight intentado: `client_timeout` (fail-open, sin retry — registered).

### Screenshots (audits/first-understand-20260917/screenshots/)
1. `r8-home-header-idle-badge.png` — header con badge stream IDLE en página sin feed (contexto GAP-3).
2. `r8-opportunities-feed-rejected-no-reason.png` — feed "0 viable / 200 total (200 rejected)", tarjetas REJECTED sin razón visible (GAP-1).
3. `r8-operations-taxonomy-and-errors.png` — taxonomía 100% rechazadas + errores 401/503/schema surfzados (GAP-2 visible).
4. `r8-operations-header-degraded-unknown-honest.png` — header durante degradación (post-recuperación parcial): prueba de badges vivos.
5. `r8-status-deploy-anchor-services-up.png` — ancla de deploy a06a968d + 7 servicios UP + paper/kill-switch.
