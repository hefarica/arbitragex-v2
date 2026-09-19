# WO-E2 · VERIFY — AG2 stream-verifier (continuidad de cadena detected→terminus)
Fecha: 2026-09-17 (verificación en vivo 18:20–18:28Z) · Agente: ecc:database-reviewer (Gang Omniscience)
Board: `audits/hft-1000-20260917/GOAL-WORKORDERS.md` · Claim files: `backend/searcher-rs/src/opportunity_emitter.rs`, `backend/searcher-rs/src/counters.rs` (solo lectura, cero edits)
Presupuesto HTTP dominio: **0/5 usados**. 1 curl diagnóstico a `127.0.0.1:9001` (VPS-local, no dominio; sin respuesta — ver G-5).

## VEREDICTO: **PASS con 2 GAPS — 1 de ellos CRÍTICO y NUEVO para la mesa**

- **PASS (ventana pre-17:07Z):** la cadena detectada→scoring→PG→stream es CONTINUA por id, con
  timestamps coherentes al microsegundo y lag p99 < 0.5 s. La afirmación del ground-truth
  "stream detected fresco al segundo" era CIERTA hasta las 17:07:13Z (p99 = 0.40 s medido directo).
- **CONFIRMADO:** 0 accepts = el productor NUNCA emite la rama accept (3 señales independientes +
  lectura de código). El transporte no pierde nada que se le entregue.
- **GAP CRÍTICO (nuevo, hallazgo principal):** el pipeline de detección está STALL desde las
  **17:07:13Z** (~78 min al momento de verificar). Streams y PG mudos, heartbeat honesto en cero.
  El searcher fue recreado a las 17:07:15.6Z y desde el boot no produce NI UNA oportunidad.
  Ver §3. Esto NO es pérdida de transporte: es cero producción aguas arriba.

---

## 1. Continuidad de cadena por id (PRIMARY_SOURCE: Redis XRANGE + PG SELECT, VPS read-only §33)

Orden del código (CANONICAL_REPO, `backend/searcher-rs/src/opportunity_emitter.rs:512-524`):
`score_and_publish` (XADD `arbx:scoring:scored`) → `try_insert_pg_with_route` (INSERT PG) →
`publisher::publish` (XADD `arbx:opps:detected`). Verificado en vivo con 5 ids probe del burst
final (17:07:13.17Z) y un lote de 50 ids del muestreo de estado estable:

| Eslabón | Evidencia | Resultado |
|---|---|---|
| Stream detected (`arbx:opps:detected`) | XREVRANGE 400 + XRANGE 600 entradas | payload JSON íntegro con `id`, `detected_at`, `rejection_reason` |
| Stream scoring (`arbx:scoring:scored`) | XRANGE ventana 1789664830000–1789664834000 (463 entradas) | **5/5 probes presentes**, `emission_outcome:"rejected"`, `rejection_reason` VERBATIM (`spot_product_le_one`), `strategy_key` coherente |
| Orden scoring→detected | stream-ids emparejados | scoring XADD **precede** a detected XADD por **1–3 ms** en los 5 probes (conforme a código; el INSERT PG vive en ese hueco) |
| PostgreSQL `opportunities` | SELECT id IN (probes) | **5/5 presentes**: `status='rejected'`, razón verbatim, `detected_at` **idéntico al payload del stream al microsegundo** (ej. `2026-09-17 17:07:13.17012+00`) |
| Escala | SELECT count(*) WHERE id IN (50 ids del stream) | **50/50** — cero caídas stream→PG a escala de muestra |

**Sin saltos ni caídas silenciosas dentro de la cadena**: cada oportunidad rechazada que el
productor emite aparece en los tres stores con el mismo id, mismo `detected_at`, misma razón
(espejo CARDS-MIRROR-01 confirmado en vivo, consistente con WO-E5 §2).

## 2. Lag detección Rust → persistencia/stream (cuantificado)

`detected_at` se estampa en construcción (upstream de gates+SizeOptimizer); lag = ts del stream-id
(Reloj Redis al XADD) − `detected_at` del payload. PG insert ocurre ANTES del XADD, y
`updated_at−detected_at` en los probes = 344–354 ms — consistente (el PG se persiste dentro del
mismo span; `updated_at` queda fijado por el INSERT, no hay UPDATE posterior para filas rejected).

| Muestra | n | p50 | p90 | p99 | max | min | negativos |
|---|---|---|---|---|---|---|---|
| Estado estable 17:03:20–17:04:40Z | 600 | **132.7 ms** | 357.2 ms | **400.6 ms** | 410.3 ms | 1.2 ms | 0 |
| Burst pre-restart 17:07:13Z | 400 | 210.2 ms | 324.8 ms | 347.8 ms | 354.2 ms | 47.9 ms | 0 |

Pierna server-side del broadcast (api-server, WO10, corroboración independiente): ventanas 60s
con n=1.8–2.8K/min, p50 139–239 ms, p99 325–721 ms — mismo orden de magnitud que mi medición
directa (detected_at→WS emit). **Ground-truth "fresco al segundo": VERDADERO pre-stall**
(p99 total < 1 s en la peor ventana). Tasa estable: PG 2.19–2.41K filas/min (17:00–17:06Z),
stream MAXLEN 10.000 ≈ 4 min de retención (entries-added lifetime = 19.405.381 — saturación de
MAXLEN = subconteo de delta en heartbeat, ya anotado por WO-E5 GAP-2).

## 3. HALLAZGO CRÍTICO — STALL de detección desde 17:07:13Z (caída NO silenciosa: fail-honest)

Lectura del reloj de pared (Redis TIME=1789669551 → 18:25:51Z):

- Último XADD `arbx:opps:detected` = 1789664833520 → **17:07:13.520Z**. Último XADD scoring =
  1789664833518. Ambos streams mudos **~78 min**.
- PG `max(detected_at)` = **17:07:13.170** — 0 filas en los últimos 20 min (29.701.866 lifetime).
- `arbitragex-v2-searcher-rs-1`: **Created=17:07:15.6Z** (único contenedor recreado; el resto data
  de 13:38Z), ExitCode=0, OOMKilled=false → recreación limpia (deploy/servicio individual),
  NO crash in-container. Quién lo recreó no es adjudicable read-only (HYPOTHESIS: acción de
  operador/CI en plena demo — el burst 77K/min de las 17:07:13 es el flush de cola previo al corte).
- Heartbeat 18:23–18:26Z (PRIMARY_SOURCE, `scanner.heartbeat`): `pending_received=0`,
  `decoded_ok=0`, `triangular_cycles_scanned=0`, `flashloan_arb_pairs_scanned=0`,
  todos los gates=0, `passed_all_gates=0`, `redis_stream_delta=0`, `db_persisted=0`,
  `pg_period_inserted=-1` (centinela §WO-E5 GAP-2). **El cero ES honesto**: los motores no están
  escaneando nada, no hay nada que rechazar. R8 OK — el stall es visible en telemetría.
- Causas visibles post-restart (logs searcher, read-only): `price_worker.alchemy_failed` ×4867
  (429 Too Many Requests, con `alchemy_hits=0, coingecko_hits=0, cache_misses=155` por tick →
  feed de precios MUERTO), `scanner.rpc_timeout` ×8817 ("RPC timeout fetching tx; discarding",
  `timeout_ms=50` contra EWMA de endpoints 22–343 ms — el flashbots EWMA 343 ms excede el
  timeout de 50 ms), `pool_discovery.failed` ×40, `rpc_pool.boot_check_failed` ×12. El V2
  gotea (38 intents en 78 min vs ~2.4K emisiones/min pre-restart).
- **R9 satisfecho antes de concluir ausencia** (CLAUDE.md §3 R9): LogConfig `max-file:5 ×
  max-size:10m` (50 MB); primera línea retenida = `service.boot` 17:07:16.735Z == StartedAt
  17:07:16.6Z → la ventana cubre TODO el run sin rotación; la "ausencia" no es artefacto.
  Corroboración independiente: XINFO `entries-added=19.405.381` vs `length=10000` y PG como
  segundo sink mudo a la misma marca exacta.

**Conclusión del charter (§"la brecha hoy conocida")**: la rama accept no aparece porque el
productor no la emite — triple evidencia: (a) status distribution últimas 3 h = SOLO `rejected`
(221.500 filas, cero `detected`/`validated`/`scored`); (b) 400/400 y 600/600 entradas de stream
con `rejection_reason` no-null; (c) scoring window 100% `emission_outcome:"rejected"`. Dominado
por `v3_quote_unavailable` (323/400 burst; 417/600 estable) — consistente con el cuello #1 del
board. `emit_accepted` existe y escribiría `status='detected'` con razón NULL
(persistence.rs:53-58) — simplemente nunca se invoca. **El transporte no pierde la rama accept:
nunca la recibió.**

## 4. Sincronía de mesa redonda — contradicción con WO-E4 ADJUDICADA

WO-E4 (`WO-E4-VERIFY.md` §2) afirma "server broadcastea ~2.3K oportunidades/min" medido a las
"17:15–17:18Z". **La atribución temporal es incorrecta**: con `docker logs --timestamps`, las dos
ventanas que E4 cita (n=2786, n=2364) son las de **17:05:37Z y 17:06:48Z** — las ÚLTIMAS DOS de la
historia del log. El broadcast murió a las 17:06:48Z, 10 min ANTES de la navegación de E4. Sus
números de latencia siguen válidos (coinciden con los míos), pero su GAP-2 ("server broadcastea
2.3K/min mientras la página muestra 0 viable / 0 total") se disuelve en dos mitades: (1) a las
17:16 el server NO broadcasteaba oportunidades desde hacía 10 min; (2) aún broadcasteando, la
página mostraría 0 viable porque el 100% es rejected y el LIVE_QUERY filtra `rejection_reason IS
NULL` (por diseño R8). Lo que E4 vio fluir a 33–47 eventos/s en su socket eran OTROS canales
(route_discovery_telemetry / runtime_ack — `route_discovery.tick` ×393 sigue vivo post-restart),
no la room `opportunities`. El GAP-2 de E4 debe re-etiquetarse: la desconexión página-datos no es
el hook page-local, es el stall de §3 + el filtro viable-by-design.

Con WO-E5 no hay contradicción: su `window_total=111.162/1h` (medido ~12:xx–13:xxZ, pre-stall) y
su GAP-2 (MAXLEN saturado) cuadran con mis números. 00-SYNTHESIS §G-ACC-1 (accepts>0 no observado)
reforzado: ahora además sims/execs congelados por stall.

## 5. GAPS

- **G-1 · CRÍTICO — Detection stall post-restart (17:07:15Z)**: cero emisiones en 78+ min;
  price feed muerto por 429 (Alchemy) + scanner rpc_timeout 50 ms vs EWMA real + pool_discovery
  failed. Dueño tentativo: WO de detección/funnel (buscar en logs del deploy 17:07 qué cambió;
  `rpc_pool.boot_check_failed` ×12 sugiere arranque degradado). Bloquea CUALQUIER demo E2E "con
  datos fluyendo en vivo" desde las 17:07Z. NOTA para el operador: la demo del orquestador corrió
  contra un pipeline ya stallado ~40 min antes de la ventana de E4.
- **G-2 · Medio — Par de relojes burst/stall**: el último minuto (944 filas + burst 77K/min
  aparente) es artefacto de flush pre-restart; cualquier métrica de "tasa" tomada sobre los
  últimos N entries del stream está sesgada por ese burst. Usar ventanas XRANGE por tiempo (como
  aquí) para medir tasas.
- **G-3 · Menor — `pg_period_inserted=-1` en heartbeat** mientras los inserts fluían pre-restart
  (E5 GAP-2): confirmado que post-restart el -1 es legítimo; la lectura PG del heartbeat seguía
  fallando ANTES del restart con inserts sanos — sigue abierto, dueño WO-E5/E6.
- **G-4 · Info — scoring stream al tope** (`arbx:scoring:scored` length=100.001 vs MAXLEN 100.000,
  aproximado `~`): misma clase de artefacto R9 que el detected.
- **G-5 · Info — `/metrics` del searcher (puerto 9001) no responde desde la red host** (curl 0
  líneas): los histogramas WO-10 solo son scrapeables dentro de la red compose. Post-restart la
  familia `arbx_pipeline_latency_seconds` está honest-ausente (R8) — confirmado por grep 0 hits
  en el endpoint (cuando responde dentro de la red).

## 6. Pregunta de ataque a WO-E4 (la que ordena el charter: ¿puede el polling-400 tragarse eventos intermedios?)

> Tu clasificación del polling-400 como "ruido sin pérdida de frames" descansa en **estabilidad
> del `sid`**, pero `sid` estable NO prueba entrega completa: engine.io en long-poll es
> one-in-flight — si un ciclo de poll muere a mitad de respuesta (tunnel 400/timeout), el batch
> que el server ya había desencolado en esa respuesta se pierde a menos que engine.io lo
> reenvíe y el cliente lo deduplique por id. ¿Verificaste **continuidad de secuencia** (ids/
> contadores monótonos por canal) entre ciclos de poll, o solo continuidad de sesión? Experimento
> de cierre: contar eventos emitidos server-side en una ventana (WO10 n) vs eventos recibidos
> client-side en el mismo canal y ventana. Y con §3 en la mano: a las 17:16 la room
> `opportunities` llevaba 10 min sin emitir NADA — ¿de qué canal exactamente eran los 33–47
> eventos/s que contaste? Si no atribuiste por room, tu "transporte sano" está demostrado para
> los canales de telemetría, no para opportunities.

## 7. Evidencia de verificación ejecutada

- Redis (read-only §33): XLEN/XINFO ambos streams; XRANGE/XREVRANGE muestras 400+600+ventana
  scoring 463; TIME. Cero comandos de escritura.
- PostgreSQL (SELECT-only): information_schema, SELECT probes (5), SELECT IN (50), counts por
  minuto/status/max(detected_at). Cero escrituras.
- Docker read-only: ps, inspect (LogConfig/StartedAt/Created/OOMKilled/ExitCode), logs searcher +
  api-server (grep/histogramas de eventos), --timestamps para adjudicar E4.
- Tests locales (claim files, sin edits): `cargo test -p searcher-rs --lib opportunity_emitter`
  → **13/13 PASS**; `--lib counters` → **7/7 PASS**. CERO git/commit/push/deploy; VPS intacto.

## 8. Lexicon
TLS = flash loan · Holonomic Loop Resolution = arbitraje triangular · Topological Yield = neto ·
Decoherencia de Estado = slippage · Variedad de Liquidez = pool/DEX.
