# WO-10 — DESIGN · Instrumentación de latencia end-to-end detección→broadcast

- **WO:** WO-10 · kind: apply (diseño re-autorado por el reemplazo — fail-honest abajo).
- **Origen:** informe Crypto Deep Analyser §6.11 / MN-006 — "latencia detección→broadcast NO instrumentada" (roadmap §4, eje Latencia: *"Instrumentar percentiles E2E (WO-10)… Solo entonces la cifra existe y se puede optimizar"*).
- **Fecha:** 2026-09-06. **Agente:** ecc:performance-optimizer (rubric ecc-rust-patterns), Gang Omniscience.
- **Reglas respetadas:** RULE 00 (cero mocks; ausencia = "no computado", jamás 0 fabricado) · R8 (None≠Some(0)) · R9 (cero logs por-ítem a info!) · §32/§33 (audit/scaffold/shadow; cero VPS-mutación) · NO-GIT (edición local + verificación solamente) · diffs marcados `// WO-10 (2026-09-06)`.

## 0. Autoría y estado (fail-honest)

El design-agent original de WO-10 murió por 429 (muro sistémico wf_29b60a15, board §Cronología). Su apply
quedó **parcialmente aterrizado** en el árbol compartido: `publisher.rs` (completo), `scanner.rs` (completo),
`opportunity_emitter.rs` (completo) y `websocket.ts` (completo) ya contenían la instrumentación al iniciar
este reemplazo — **pero sin diseño, sin reporte y sin ninguna verificación ejecutada**. Este documento
re-autora el diseño desde cero (contra el código releído línea a línea, no contra notas del caído), corrige
UNA justificación técnica falsa hallada en un comentario (§7.4), ejecuta la verificación completa (§8) y
escribe el reporte (`WO-10-APPLY.md`). Los diffs de §7 son los diffs REALES del árbol (verificables con
`git diff` en los 4 archivos).

## 1. El hueco (qué NO existía)

Antes de WO-10 el pipeline medía **flujo** (counters `pending_received`, `decoded_ok`, `OPPORTUNITIES_TOTAL`)
pero **ninguna duración**: no existía ninguna serie que respondiera "¿cuánto tarda una detección desde que el
tx llega al scanner hasta que el dashboard la recibe?". El informe §6.11 lo clasifica como gap MN-006 y el
roadmap lo hace prerrequisito de TODO optimizaje de latencia ("solo entonces la cifra existe"). El único
span temporal vivo era el `latency_ms` de `scanner.subscription_error` (scanner.rs:1209 — solo en errores de
provider, no del pipeline) y `pipeline_latency_ms` del `ConvergenceSignal` (websocket.ts:231 — un valor del
motor SED, no una medición de esta ruta).

## 2. La ruta medida (anclas file:line releídas)

```
[WS pending tx]                                   scanner.rs:1303/1348 (streams)
  └─ get_tx (50ms cap)                            scanner.rs:1402-1424
  └─ t0: tx-on-hand ────────────────────────────  scanner.rs:1505  (Instant, monotónico)
       ├─ decode V2  route_decoder               scanner.rs:1524→1547  ──► stage=decode_route
       ├─ decode legado calldata                 scanner.rs:1596→1600/1609 ─► stage=decode_calldata
       ├─ gates (spine/strategy/token)           scanner.rs:2184-2354
       ├─ encoder+REVM (fail-closed hoy)         scanner.rs:2393-2430
       ├─ dedup opp                              scanner.rs:2608-2621
       ├─ PG insert                              scanner.rs:2627-2641
       ├─ publish XADD                           publisher.rs:153-176  ─────► stage=publish_xadd
       └─ t1: XADD completo ────────────────────  scanner.rs:2044/2243/2288/2338/2643
                                                   ──► stage=decode_to_publish_legacy (t0→t1)

[Path V2/cartridges — emisor central]
  Opportunity.detected_at (stamp de CONSTRUCCIÓN)
       engines: dex 768 · triangular 591 · liq 288 … · cartridge_boot.rs:1181 · orchestrator.rs:1549
  └─ emit_accepted/emit_rejected                 opportunity_emitter.rs:211/366
       ├─ t2: entrada I/O real (post-dedup-reclass, post-dry-run)
       │                                          opportunity_emitter.rs:298/414 (Instant)
       ├─ Gate-C scoring (advisory)              opportunity_emitter.rs:317/422
       ├─ PG insert + route                      opportunity_emitter.rs:323/430
       ├─ publish XADD                           publisher.rs:153-176
       └─ t3: XADD completo ────────────────────  opportunity_emitter.rs:333/439 ─► stage=emit_boundary (t2→t3)
          y wall-clock detected_at→t3 ──────────  opportunity_emitter.rs:334-337/440
                                                   ──► stage=construction_to_publish (guard R8)

[Pierna broadcast — api-server]
  PG trigger trg_notify_opportunity (mig 025) → LISTEN opportunities_channel
       │                                          api-server/src/index.ts:1847-1863
  └─ broadcastOpportunity: emit('new_opportunity') websocket.ts:447-448
     E2E = Date.parse(opp.detected_at) → post-emit  websocket.ts:455-462 (wall-clock, host compartido)
     hot legs: arbx:hot:detected/simulated stamps   websocket.ts:958-972
       (productor: hot_path_emitter.rs:91 detected_at_ms · :177 timestamp_ms)
```

**Descomposición del §6.11:** `decode_route` + `construction_to_publish` (Rust, Prometheus) + pierna
PG-NOTIFY→WS (api-server, summary 60s) ≈ detección→broadcast completa. `emit_boundary` aísla el emisor
central; `publish_xadd` aísla el round-trip Redis; `decode_to_publish_legacy` cubre la ruta legada
mono-proceso (vacía por diseño mientras `ARBX_ORCHESTRATOR_MODE=v2` — temprano-return en scanner.rs:1584-1586).

## 3. Métrica — se REUTILIZA el exporter Prometheus vivo (cero canal nuevo)

**Pregunta del charter: "¿metrics.rs existente?"** — Sí, y la cadena completa ya está viva:

| Eslabón | Ancla file:line |
|---|---|
| Definición de la familia | `publisher.rs:70-86` `arbx_pipeline_latency_seconds{stage}` registrada en `shared_rs::metrics::REGISTRY` |
| Mismo patrón que metrics.rs existente | `searcher-rs/src/metrics.rs:25-29,43` (todo el crate ya registra ahí) |
| Handler /metrics | `shared-rs/src/metrics.rs:447-462` `metrics_handler` = `REGISTRY.gather()` |
| Router que lo monta | `shared-rs/src/health.rs:57` (montado por `build_health_router`) |
| Bind del puerto | `searcher-rs/src/main.rs:339` (`SEARCHER_HEALTH_PORT`, default 9001) + `main.rs:1084,1105-1109` |
| Scrape Prometheus (prod) | `monitoring/prometheus/prometheus.prod.yml:30-32` `searcher-rs:9001` (dev: prometheus.yml:20-22) |

NO se crea ningún canal nuevo: la serie nace en el registry que el scraper ya está leyendo cada 15s.

### 3.1 Histograma y percentiles

`HistogramVec` con buckets `0.0005…10s` (publisher.rs:76-78) — escala log pensada para un hot-path
sub-ms con cola de milisegundos (XADD/PG). p50/p95/p99 se computan en consulta:

```promql
# p95 de cada stage (Grafana / curl a Prometheus):
histogram_quantile(0.95, sum by (le, stage) (rate(arbx_pipeline_latency_seconds_bucket{job="searcher-rs"}[5m])))
# p50 / p99: cambiar 0.95 por 0.50 / 0.99
```

Los percentiles NO se computan en el proceso (un histograma Prometheus los aproxima por buckets en
consulta — sin mantener muestras por ítem, sin cardenalidad por oportunidad).

### 3.2 Cero alloc en el hot-path

Todos los `observe()` van a **hijos pre-resueltos** `Lazy<Histogram>` (`STAGE_*`, publisher.rs:90-101):
después del primer touch el cliente prom-client cachea el hijo tras la key de labels — el loop hace
`Instant::elapsed().as_secs_f64()` + `Histogram::observe(f64)`. **Cero `String`, cero `format!`, cero
resolución de labels en el loop** (labels solo `stage`, resueltas una vez en el `Lazy`).

### 3.3 Guard R8 para spans wall-clock

`construction_to_publish` es el único stage con reloj wall-clock (`detected_at` es un campo de fila PG
necesario de todos modos; no se añadió ningún timestamp nuevo al wire). Un step de NTP produciría elapsed
≤0 y una fila replaysada elapsed enorme → `observe_construction_to_publish` (publisher.rs:132-151) los
rechaza: `0 < ns ≤ 300s` entra al histograma; el resto se CUENTA en `arbx_pipeline_latency_invalid_total{stage,reason}`
(`non_positive_elapsed` / `stale_detected_at` / `out_of_range`) y **jamás** se inyecta al histograma.

### 3.4 Lazy-init deliberado = "no computado" honesto (RULE 00)

A diferencia de `init_orchestrator_metrics()` (metrics.rs:27-29 fuerza el registro de TODAS las series al
boot), la familia WO-10 es lazy: **una serie `stage=…` aparece en `/metrics` solo tras su primera
observación real**. Decisión explícita: un histograma presente-con-cero-cuentas se leería como "computado
y no hay datos" — exactamente el `Some(0.0)` que R8 prohíbe. Ausencia de serie = "no computado". El panel
Grafana mostrará "No data" hasta que el pipeline emita de verdad — ese ES el estado honesto (hoy: 100%
rejected sigue produciendo emisiones por la rama `emit_rejected`, así que las series de emitter/publish
cobran vida en cuanto el binario corra; `decode_to_publish_legacy` solo si el modo legado corre).

## 4. R9 — cero logs por-ítem

- **Rust:** la observación es R9-silenciosa (sin `tracing` en ningún punto WO-10). La superficie agregada
  ES Prometheus. Ningún `info!` nuevo por ítem (el doc-comment de publisher.rs:19-20 lo fija como contrato).
- **api-server (websocket.ts):** la pierna TS no tiene exporter propio dentro del claim (§5), así que su
  superficie agregada es **UNA línea de summary por ventana de 60s** (`Wo10LatencyWindow.summarize`,
  websocket.ts:96-107): `n`, `skipped`, `p50/p95/p99/max` — ring `Float64Array(8192)`, O(1) por observación.
  Sin per-emit logging (R9). La primera observación ancla la ventana (websocket.ts:80-83) para no emitir
  un summary casi-vacio justo tras boot/recreate.

## 5. Superficie de dashboard (solo métrica real, cero MOCKS)

1. **Rust (GA cierta):** paneles p50/p95/p99 en el dashboard Grafana existente
   `monitoring/grafana/dashboards/detection-pipeline.json` (ya grafica el funnel del searcher — targets
   `arbx_searcher_pending_total`, etc., líneas 17-74). El promql de §3.1 es el target literal. **Fuera del
   file-claim de WO-10** → se entrega como diff propuesto (§7.5), NO aplicado; ningún panel frontend se
   toca. Honestidad: serie ausente = "no computado" (§3.4) — el panel NO inventa ceros.
2. **api-server (pierna NOTIFY→WS):** hoy la superficie es el summary log 60s (§4). El aterrizaje
   Prometheus de ESTA pierna son 5 líneas en `shared-ts/src/metrics/index.ts` (histograma gemelo de
   `runtimeAckBroadcastLatencyMs`, L146) + un `.observe()` en websocket.ts — Prometheus YA scrapea
   `api-server:8080/metrics` (prometheus.prod.yml:50-52) y api-server YA importa métricas de `@arbx/shared`
   (websocket.ts:4-8), así que cero wiring adicional. No aplicado por límite de claim (§7.4); no por
   imposibilidad técnica.
3. **Nada del panel /operations o /readiness se modifica dentro de WO-10** — ambos leen APIs del
   api-server, no Prometheus directo; cablear esa lectura sería un cambio de superficie fuera de claim y
   sin necesidad (Grafana ya es la superficie canónica de métricas del stack).

## 6. Qué NO medir (sesgos declarados)

1. **Clock skew cross-container:** `construction_to_publish` compara el stamp de CONSTRUCCIÓN (searcher-rs)
   contra `Utc::now()` en el MISMO proceso → sin skew por construcción. La pierna api-server
   (`new_opportunity`) compara `opportunities.detected_at` (escrito por searcher-rs) contra `Date.now()`
   (api-server) — ambos contenedores comparten el kernel clock del VPS host → skew ≈ 0 **por construcción**,
   pero la regla queda escrita: **esta pierna nunca se compara cross-host** (deploy multi-VM la invalidaría;
   declararlo, no medirlo). El guard del §3.3 absorbe steps de NTP locales.
2. **Sesgo de reconnect de flota (roadmap §0):** la flota fue recreada 4 veces durante la auditoría
   (22:58:05Z, 23:33:07Z, 23:45:26Z + ~00:1xZ sin merge); cada recreate = feed 0 por ~5-7 min. Consecuencias
   declaradas: (a) el ring TS de 60s se REINICIA con el proceso → todo summary dentro de los primeros 60s
   post-recreate cubre una ventana parcial (el anclaje de primera-observación evita el summary vacío, pero
   la ventana sigue siendo corta — NO usar esos summaries para percentiles de flota); (b) el histograma Rust
   NO se reinicia por recreate de contenedor (los buckets son de proceso; Prometheus conserva la historia,
   pero las ventanas `rate()[5m]` que crucen un recreate mezclan procesos — leer con ventana que no pise
   deploys, o anclar por `increase(container_start_time_seconds…)` como hace la regla FleetChurn del
   alerts.rules.yml:181); (c) regla operativa: **toda lectura de WO-10 lleva timestamp implícito** — misma
   advertencia que el roadmap §0 imprime sobre TODA medición de esta mesa.
3. **NO se mide (fuera de alcance WO-10):** la pierna WS→browser real (latencia de red del cliente;
   requeriría stamp en el cliente y es superficie frontend), la latencia de simulación REVM (S4 es otra
   familia con su propio gate), ni el dwell-time del mempool antes del WS (inobservable desde el receptor —
   el t0 es "tx-on-hand", y así se declara en cada stage: NO es "tx vista por la red").

## 7. Diffs (los REALES del árbol — `// WO-10 (2026-09-06)`)

### 7.1 `backend/searcher-rs/src/publisher.rs` (+216) — núcleo

Familia `arbx_pipeline_latency_seconds{stage}` (6 stages: `decode_route`, `decode_calldata`,
`construction_to_publish`, `emit_boundary`, `publish_xadd`, `decode_to_publish_legacy`) con hijos
pre-resueltos `STAGE_*`; guard `arbx_pipeline_latency_invalid_total{stage,reason}`; helper puro
`observe_construction_to_publish` (cap stale 300s, publisher.rs:124); span XADD dentro de `publish()`
(L162-174 — observado SOLO en éxito, el `?` propaga el error); 3 unit tests (L184-243): registro en el
registry vivo, span válido observado, clock-step/stale contado-no-inyectado.

### 7.2 `backend/searcher-rs/src/scanner.rs` (+115)

Origen `wo10_tx_start` a la entrada de `decode_and_score_tx` (L1505, post filtro de router — un tx no-router
no entra al pipeline y no sesúa); span `decode_route` V2 (L1524→1547); span `decode_calldata` legado en
AMBOS brazos (L1600 Err + L1609 Ok — la duración del decode es real aunque falle); `decode_to_publish_legacy`
en los 5 sitios de publish de la ruta legada (L2044 no-config · L2243 TokenNotAllowed · L2288
StrategyDisabled · L2338 StrategyConfigGate · L2643 scored). Todos monotónicos, R9-silenciosos.

### 7.3 `backend/searcher-rs/src/opportunity_emitter.rs` (+27)

`t2` de entrada a I/O real en `emit_accepted` (L298) y `emit_rejected` (L414) — DESPUÉS de las
reclasificaciones (economics-gate, tier-gate, dry-run) para no doble-contar cuando accepted recae en
rejected, y ANTES de scoring/PG/XADD; observes post-XADD (L333-337, L439-440). El clone `rejected` preserva
`detected_at` verbatim → el span construction_to_publish de una fila rechazada refleja TODA la evaluación
que sobrevivió (gates + SizeOptimizer) antes de su rechazo honesto.

### 7.4 `backend/api-server/src/websocket.ts` — pierna TS

`wo10PercentileStats` pura (L51-65, exportada para tests); `Wo10LatencyWindow` ring 8192 + summary 60s
(L71-109); `wo10E2eMsFromStamp` (L115-119); observación E2E en `broadcastOpportunity` post-emit
(L447-464, payload NO mutado — contrato byte-idéntico testeado) y en `emitEntry` para las piernas hot
(L958-972, stamps `detected_at_ms`/`timestamp_ms` del productor). **Corrección de este reemplazo:** el
comentario del predecesor justificaba no-aterrizar el histograma con "prom-client is not resolvable from
backend/api-server" — FALSO (websocket.ts importa y observa `runtimeAckBroadcastLatencyMs` de @arbx/shared,
L4-8+547). La razón verdadera: api-server no tiene dependencia directa de prom-client
(package.json:18 solo `@arbx/shared "*"`), así que el histograma nuevo DEBE definirse en
`shared-ts/src/metrics/index.ts` — fuera del file-claim de WO-10. Comentario corregido in situ (L26-39).

### 7.5 FUERA de claim (propuestos, no aplicados)

- `shared-ts/src/metrics/index.ts`: +5 líneas — `export const pipelineBroadcastLatencyMs = new Histogram({ name: 'arbx_pipeline_broadcast_latency_ms', help: '…', labelNames: ['leg'], buckets: [5,10,25,50,100,250,500,1000,2500,5000,10000] })` junto a L146; y en websocket.ts `pipelineBroadcastLatencyMs.labels(source).observe(ms)` en los dos `observe()` de ventana (solo cuando `ms !== null`). Dueño: quien tenga el claim de shared-ts.
- `monitoring/grafana/dashboards/detection-pipeline.json`: panel "Detección→broadcast p50/p95/p99" con el promql de §3.1 por stage. Dueño: claim de monitoring/.

## 8. Verificación (ejecutada por este reemplazo — ver APPLY para output crudo)

| Gate | Comando | Resultado |
|---|---|---|
| Rust compila | `cargo check -p searcher-rs` (backend/, target caliente) | **EXIT=0**, 39.75s, 0 warnings |
| Tests R8 del publisher | `cargo test -p searcher-rs --lib publisher` | **3/3 ok** (registry-vivo · span válido · clock-step/stale rechazados) |
| Tests TS WO-10 | `vitest run src/websocket-wo10.test.ts` | **7/7 PASS** |
| No-regresión TS familia | `vitest run websocket` | **17/17 PASS** (5 archivos: wo10 7 · hot-streamer 3 · rooms 4 · base 2 · carnot 1) |
| Typecheck | `tsc --noEmit -p tsconfig.json` (api-server) | **EXIT=0** |

## 9. Límites declarados (R8)

- La pierna api-server NO está en Prometheus hasta que aterrice §7.5 (hoy: summary log 60s) — declarado, no oculto.
- `decode_to_publish_legacy` estará vacía en modo V2 puro (temprano-return scanner.rs:1584) — es la
  semántica correcta de "ese path no corrió", no un hueco.
- Los percentiles del histograma son aproximados por buckets (± límite de bucket) — suficiente para
  p50/p95/p99 en la escala de milisegundos de esta ruta; sin promesa de precisión sub-bucket.
- Nada de esto corre en el VPS todavía (NO-GIT): la serie nace en producción tras el PR+deploy del operador.
