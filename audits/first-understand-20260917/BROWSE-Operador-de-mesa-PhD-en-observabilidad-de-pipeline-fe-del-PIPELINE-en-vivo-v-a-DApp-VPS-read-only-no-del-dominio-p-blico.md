# BROWSE — Operador de mesa (PhD en observabilidad de pipeline) · fe del PIPELINE en vivo
## https://arbx.ape-tv.net · /opportunities · /live-readiness · /operations — 2026-09-17 ~06:20–06:26 UTC

> Rol: usuario humano de la DApp, navegación con criterio propio. Presupuesto HTTP respetado:
> 4 navegaciones manuales (2× /opportunities, /live-readiness, /operations) + lectura VPS
> read-only (docker ps / docker logs / redis-cli XLEN / psql SELECT). Cero auth, cero toggles,
> cero escritura (§32/§33). Clasificación de cada afirmación: OBSERVED (browser/VPS) salvo
> indicación.

---

## 0. Veredicto ejecutivo

**FE (fe sostenida) con 4 gaps periféricos de observabilidad.** Las tres páginas muestran
datos reales (RULE 00 OK), los estados vacíos son honestos y con razón legible (R8 OK), y
los contadores son consistentes entre paneles y contra la fuente de verdad (PG/Redis/logs).
El pipeline está VIVO y produciendo en el momento de la observación. Ningún panel mintió;
dos contadores menores están desalineados del wire (ver §4).

---

## 1. /opportunities — Live Network value Feed

**Evidencia (OBSERVED):**
- Badge de cabecera `Live opportunity stream status: LIVE` (tras hidratación; al primer pinto
  decía `IDLE` y el sub decía "Edge connection error · Loading..." — estado transitorio de
  carga, se resolvió solo en <10 s).
- 752 tarjetas renderizadas en el DOM al muestreo; **100% REJECTED** (conteo por texto:
  752/752).
- **Frescura verificada contra reloj del cliente**: timestamps de filas avanzan
  01:21:48 → 01:22:01 → 01:22:13 → 01:22:24 → 01:22:38 con clienteNow 01:22:41 (UTC-5) —
  edad 0–3 s por fila. Edad relativa visible "2s" + etiqueta "VIGENTE". Sin spinner eterno
  ni skeleton congelado (el "Loading opportunities…" inicial desapareció).
- Cada tarjeta trae razón de rechazo como badge (ej. `spot_product_le_one`) y TODAS las
  columnas no computadas como "—" con tooltip fail-honest: "Net Convergence Ratio (ROI %) —
  fail-honest '—' when not computed", "DETECTOR no emitido" ("detector_id no es columna del
  feed"), "LATENCIA no emitido", "no target". Ejemplo de tarjeta completa capturado en
  snapshot.
- Postura runtime en cabecera: `KILL SWITCH OFF · PAPER MODE ON · socket CONNECTING ·
  routes DEGRADED · runtime_ack LIVE · pairs CONNECTING · quote_anchor CONNECTING` — cada
  badge con aria-label explicando la semántica (ej. socket: "the single socket.io connection
  shared by every WS channel").
- System guard: `Live OFF · relay OFF · submit OFF · broadcast OFF · Paper ON · Capital $0 ·
  GO live NO-GO` — coherente con §34.3 (terminus cerrado).

**Contraste con fuente de verdad (OBSERVED, VPS):**
- `docker logs arbitragex-v2-searcher-rs-1 --tail 50`: pipeline ACTIVO — 29×
  `cartridge.active_eval_enter` + 16× `cartridge.active_eval_summary` en 3 s de ventana
  (06:25:01→06:25:04Z), cada summary con histograma per-reason (`pertinent:269,
  negative:269, positive:0`, razones `missing_reserves:56`, `bridge_state_unavailable:31`,
  `funding_feed_unavailable:31`…). Patrón LOGFLOOD-01 correcto (per-item info resumido).
  → El feed de la DApp NO es decorativo: refleja evaluación real en curso.

**Dictamen /opportunities: CORRECTO.** Stream real, fresco al segundo, fail-honest en cada
columna vacía. Nota: el badge "socket CONNECTING" con feed LIVE implica que la frescura
llega vía fallback REST tick (`/api/opportunities/live?limit=20` + `/api/route-discovery/tick`
observados en network log; socket.io quedó en transporte polling — ver §4 gap 4).

## 2. /live-readiness — tarjetas verdes/rojas

**Evidencia (OBSERVED):** 25 tarjetas. **17/19 VERIFIED · 2 FAILING · 0 NOT STARTED**
(barra 17/19). Ignición operativa 4/4 (Topology Vault "4 active WSS + 5 active RPC";
signer presente redacted; topología "6 chains, 23 dexes, 1315 pools, 4381 tokens"; 5
resolution engines paper/shadow). Sello "VERIFIED 17/09/2026, 01:23:08 A.M. · SOURCE:
API-SERVER · UPDATED 13S AGO".

**Los 2 rojos, cada uno con razón legible (R8 OK):**
1. **G-SIM-1 FAILING**: "premature flag — SECURE_BOOT violated:
   ARBX_SIMULATOR_V2_READY=true with evidencia de checklist 4/7; missing evidence:
   [modules_merged, eth_callbundle_staging, second_signoff] — stale (>30d)". Fuente citada:
   `ENDPOINT · ARBX_SIMULATOR_V2_READY=TRUE + READINESS_EVIDENCE 4/7`.
2. **G-PIPE-1 FAILING**: "sim-ctl consumer stalled: 500 entries behind on
   arbx:opps:validated (deliverable, ≥500)" +
   `BACKLOG(SELECTOR)=0/DELIVERABLE BACKLOG(SIM-CTL)=500/DELIVERABLE SERVER-LAG=27543/10002
   (KILLSWITCH KEY PRESENT, ENABLED=FALSE)`. Corroborado por mí: Redis
   `XLEN arbx:opps:detected=10004`, `XLEN arbx:opps:validated=10002` (el "10002" del
   SERVER-LAG es este stream). Consistente con memoria de mesa (G-PIPE-1 500-behind).

**Anti-flip íntegro**: botón "Activate live mode" deshabilitado ("FLIP BLOCKED — 2
PENDING"; "no UI control to flip Live to ON"); GO/NO-GO NO-GO con top reasons críticos
listados; A.9 "AWAITING FIRST SIGN-OFF", ledger hash 5189814f… generado 2026-09-07,
sign-offs 0, "no sign button here" (sign-off solo por admin API). §34.3 intacto desde la
DApp (OBSERVED).

**Dictamen /live-readiness: CORRECTO.** "no false greens" se sostiene: cada verde cita
endpoint/archivo/query; cada rojo dice exactamente qué falta.

## 3. /operations — Convergence Metrics (consistencia entre paneles)

**Evidencia (OBSERVED):**
- Modo: `selected_execution_mode PAPER_SHADOW · coherent with boot mode`; tarjetas de modo
  muestran LIVE_MAINNET "default-deny: MainnetRefused (§34.3)" — doctrina visible.
- **Taxonomía de rechazo 24h**: "654,710 rechazadas de 654,710 oportunidades · 24 h"
  (100%). Top: `v3_quote_unavailable 502,626 (76.8%)` · `spot_product_le_one 81,175
  (12.4%)` · `non_positive_profit 36,269` · `single_pool_no_spread 31,688`.
- **Contraste PG (read-only)**: `SELECT status, COUNT(*) … 24h` → `rejected | 659768`
  (consultado ~1 min después de la pintada de la UI). 654,710 vs 659,768 = mismo orden de
  magnitud, delta ~5k explicable por ventana/elapsed. Detecciones última hora: 140,475
  (~39/s) → coherente con "PG INSERTED (PERIOD) 2500" (periodo corto) y con 752 tarjetas
  en feed con frescura de segundos. `MAX(detected_at)=06:25:24Z` = en vivo.
- Route Discovery Funnel: market events 235 → dirty pools 235 → pair seeds 64 → routes
  found 500 → dispatched 200; evaluados/outcomes/opportunities = "—" con nota explícita:
  "Nulo ⇒ ausencia real (R8), jamás un cero". Honest.
- Latency budget: **FAIL_p95 lat.total over SLA** mostrado en rojo — lat.total p95
  374.08 ms vs target 29 (headroom −345.08); lat.state p95 215.91 vs 3. El panel NO oculta
  el exceso.
- EV: CPI 0.0000 · SPI 0.00 · ops hoy 0/100 · CV $−1332.78 — ceros reales declarados.

**Dictamen /operations: CORRECTO en lo sustantivo** (magnitudes consistentes UI↔PG,
100% rechazadas reproducible, FAIL de latencia visible). Desalineaciones menores en §4.

## 4. GAPS encontrados (ninguno rompe la fe; todos de observabilidad)

1. **Scanner Pipeline Funnel en cero mientras el pipeline evalúa** (WO mismatchet):
   "1. Pending received 0 · Decoded 0 · … · Persisted  PG 0" en "last 60s", PERO los logs
   muestran evaluaciones de cartuchos disparadas por tx_hash de mempool EN ese minuto, y el
   propio panel reporta "PG INSERTED (PERIOD) 2500". Los ceros del funnel no son ausencia
   real — el contador del heartbeat no está cableado a la ruta activa (o muestrea otra
   etapa del decoder). Riesgo: falso "pipeline quieto" para un operador que no baje a logs
   (R9/log-window discipline). agent-fixable.
2. **503 en /api/route-discovery-outcomes/summary?hours=24** (console + network OBSERVED):
   deja "outcomes resueltos —" y "opportunities —" en el funnel de discovery. Es fail-loud
   honesto en consola, pero el panel solo muestra "—" sin razón visible en UI. Causa raíz
   no diagnosticada aquí (leería logs del api-server; fuera de mi presupuesto HTTP).
   agent-fixable (diagnóstico) / posiblemente carga (SERVER-LAG 27543).
3. **Archivo Frío atascado en "Cargando estado de archivo…"** + `edge HTTP 401:
   {"error":"missing_admin_token"}` en el bloque "Archivos existentes" (2× GET
   /api/admin/archive/status → 401). La tabla de retención no transiciona a estado de
   error; queda spinner-eterno de texto para un visitante sin token admin. Honestidad
   parcial (el 401 se muestra verbatim), wiring de estado incompleto. agent-fixable.
4. **socket.io sin upgrade a websocket**: `wss://arbx.ape-tv.net/socket.io/…transport=
   websocket` → ERR_INTERNET_DISCONNECTED (3×); el transporte queda en polling y el badge
   "socket CONNECTING" permanente. La frescura NO sufre (REST tick fallback verificado),
   así que es degradación de transporte, no de datos. No puedo atribuir si el bloqueo es
   del origen (VPS/nginx) o de mi red local — INFERRED, requiere re-verificación desde
   otra red. operator-gated (infra) para diagnosticar del lado VPS.
5. (menor, INFERRED del network log) **Lluvia de GET /api/v1/token-icon duplicados**
   (misma dirección 0x6982… PEPE ~30×, 0xd7ef… ~20×, con ráfaga de ERR_ABORTED): el feed
   re-pide iconos por render en vez de cachear por URL. Coste de ancho de banda del propio
   edge. agent-fixable.
6. (anomalía no reproducida) Tras la primera carga de /opportunities y una espera de 8 s,
   el router client-side navegó solo a `/` sin interacción ni error de consola (1×).
   No volvió a ocurrir en el segundo intento. Registro como observación sin hipótesis
   confirmada (INFERRED: posible race de prefetch RSC).

## 5. Síntesis de mesa redonda (para los pares)

- Confirma a INFORME.md §5.8 ("VPS 24/24 healthy"): flota Up 3h (reinicio ~03:46 UTC ya
  reportado por 02-VPS-REMAP-20260917 hallazgo 4), 24/24 healthy (OBSERVED hoy 06:24Z).
- **Contradice/aporta matiz a WO-02a** (workers legacy default-OFF, "hft_mempool_listener
  MUERTOS"): la ruta mempool→cartuchos ESTÁ disparando evaluaciones con tx_hash reales hoy
  (logs 06:25Z). Lo que está muerto puede ser el listener HFT específico, no la ingesta
  mempool→cartridge. Quien ficha searcher-rs debería conciliar qué worker emite
  `cartridge.active_eval_enter`.
- La dominancia `v3_quote_unavailable 76.8%` (24h) es el estado post-V3-QUOTE-CACHE-20260916
  (commit c5e1d854 local, desplegado?) — el VPS corre a06a968d (02-VPS-REMAP), así que esta
  taxonomía es la del árbol SIN el fix local del cache de quotes. Línea de base útil para
  verificar el fix tras deploy.
- G-SIM-1/G-PIPE-1 rojos + A.9 sin firma = la cadena anti-flip está viva en la UI; nadie
  puede flipear desde la DApp (verificado como usuario sin privilegios).

## 6. Artefactos

- `audits/first-understand-20260917/screenshots/browse-01-opportunities-live-feed.png` — feed LIVE, tarjetas REJECTED frescas.
- `audits/first-understand-20260917/screenshots/browse-02-live-readiness-17of19.png` — 17/19, 2 rojos con razón, flip blocked.
- `audits/first-understand-20260917/screenshots/browse-03-operations-funnel-taxonomy.png` — funnel, taxonomía 100% rechazadas, FAIL_p95 lat.total.

**Clasificación global**: OBSERVED todo lo citado con URL/log/query; INFERRIDO solo los
ítems 4.4, 4.5, 4.6 marcados como tales. RULE 00: nada fabricado; los "—" de UI se
contrastaron contra fuentes reales, no se rellenaron.
