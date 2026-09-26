# 20. PATRONES DE INTEGRACIÓN UNIVERSAL DE SISTEMAS (§20.1–§20.12)

CUÁNDO CARGAR ESTA REFERENCIA: al diseñar o auditar cualquier frontera entre componentes del stack (edge/api-server/searcher/sim-ctl/relays-client), feeds WebSocket de RPC, webhooks de terceros, buses Redis Streams, CDC desde PostgreSQL, indexación on-chain (The Graph/Ponder/viem), auth servicio-a-servicio, o evolución de schemas compartidos entre Rust y TypeScript. Triggers concretos: "el consumidor se rompió al cambiar el payload", "429 del proveedor", "el feed WS se congela y sirve datos viejos", "duplicados en la tabla tras un retry", "PG y Redis divergieron (dual-write)", "una reorg rompió el índice", "rotar credenciales sin downtime", "cómo propagar el trace por el bus".

| Concepto | Herramienta/Patrón | Nota clave |
|---|---|---|
| Contratos versionados | `schema_version` + serde/zod en frontera | Additive-only; canary consumer en shadow antes de deprecar |
| Paginación estable | Cursor keyset `(detected_at, id)` | Offset miente en datasets vivos: skew = filas saltadas/duplicadas |
| Presupuesto de rate-limit | `governor` por (host, proveedor) | El 429 es por proveedor; un limiter global contamina los sweeps |
| Resiliencia WS | ping/pong RFC 6455 + resync por gap de seq | Gap = pausar publicación; jamás servir stale silencioso (R8) |
| Webhooks entrantes | HMAC raw-body + timestamp/nonce + dedup por event id | Handler solo verify→persist→enqueue; procesar async con DLQ |
| Bus de eventos | Redis Streams `XREADGROUP`/`XACK`/`XAUTOCLAIM` | At-least-once: el consumidor SIEMPRE debe ser idempotente |
| Anti dual-write | Outbox transaccional + relay `FOR UPDATE SKIP LOCKED` | La publicación hereda la transacción del dominio |
| Indexación on-chain | Checkpoint (block, hash) + unwind en reorg | viem detecta bloques pero NO hace unwind: lo escribe el indexer |
| Auth servicio↔servicio | mTLS / OAuth2 client-credentials / SIWE+EIP-712 | Rotación con ventana dual-validity ≥ TTL de la credencial vieja |
| Tracing E2E | W3C `traceparent`, también como campo del stream | El bus no propaga contexto solo: hay que copiarlo por entry |
| Config y flags | 12-factor + feature flag con kill-switch por flag | `NEXT_PUBLIC_*` se hornea en build: flag de FE ≠ flag runtime |
| Backpressure | `tokio::sync::mpsc` acotado + COUNT/ack pacing | Productor bloqueado ruidosamente > consumer desbordado en silencio |

> Esta referencia extiende el núcleo (ver `00-nucleo-ingenieria-mev.md`), no lo repite: el núcleo §1.3 define el carrier pipeline, el bus y la idempotency key determinista; aquí profundizamos en las semánticas que hacen que esas piezas sobrevivan a fallos, versiones y proveedores hostiles.

## 20.1. CONTRATOS DE DATOS: SCHEMAS VERSIONADOS Y EVOLUCIÓN SIN ROTURA

El contrato (payload shape) entre searcher-rs, sim-ctl, api-server y el frontend es una API pública interna: se versiona igual que una API externa.

**Versionado explícito.** Todo mensaje cross-servicio lleva `schema_version: u32` en el primer campo. El despachador de la frontera elige el parser por versión; versión desconocida → error de frontera ruidoso (fail-fast), nunca "mejor esfuerzo".

**Validación en frontera, no en el fondo.** Rust: serde en el punto de entrada (`serde_json::from_str` con tipado estricto). TypeScript: `zod` con `.safeParse()` — jamás `JSON.parse()` + cast. Reglas:
- Contratos internos (ambos lados controlados): `#[serde(deny_unknown_fields)]` en Rust y `z.object({...}).strict()` en zod — rompen en compile/ci si el contrato deriva.
- Contratos de terceros (no controlamos el emisor): NO usar deny_unknown_fields/strict; usar `#[serde(default)]` + tolerancia a campos extra, o el proveedor añade un campo y nos cae la ingesta.
- Uniones discriminadas: `#[serde(tag = "kind")]` en Rust y `z.discriminatedUnion("kind", [...])` en zod (errores precisos; `z.union` da mensajes inútiles y `#[serde(untagged)]` es un diagnóstico opaco).
- Evolución additive-only: nuevos campos opcionales OK; jamás cambiar el tipo de un campo existente ni reusar un nombre; enums solo agregan variantes si el consumidor mapea lo desconocido a un `Unknown` explícito (`#[serde(other)]` en la variante catch-all).
- Documento vivo de compatibilidad: changelog del contrato por versión con fecha de deprecación de cada versión anterior.

**Publicar el contrato como artefacto.** Con `schemars` (`#[derive(JsonSchema)]`, `schema_for!(T)`) se exporta JSON Schema desde los tipos Rust reales; el lado TS lo consume para contract tests contra fixtures golden. Así el contrato no es un documento que envejece: es el compilador (regla del repo: el compilador es la ley).

**Canary consumer.** Antes de deprecar vN: (1) el productor emite vN y vN+1 en paralelo (campos extra, no dos mensajes); (2) un consumidor canario en shadow hace dual-parse de ambos y emite métrica de drift (`arbx_contract_drift_total{schema, version_pair}`); (3) drift = 0 durante una ventana ≥ 48h de tráfico real → retirar vN. Este patrón es el mismo espíritu del repo: dual validación antes de tocar productores (ver núcleo §1.3, carrier re-simulado).

**Tests de contrato.** Fixtures golden versionados junto al código (patrón repo: generador fail-fast + fixture in-crate). CI rompe si el productor serializa algo que el fixture vN no puede deserializar — eso es un breaking change aunque el compilador no lo vea.

## 20.2. REST: PRESUPUESTO, RETRIES, IDEMPOTENCIA Y COLAS DE LATENCIA

**Paginación: cursor vs offset.** Offset (`LIMIT/OFFSET`) es O(n) y miente en datasets vivos como `opportunities` (~92 GB históricos en el VPS): entre página y página entran filas nuevas y el offset salta o duplica. Cursor keyset es estable:

```sql
SELECT id, detected_at, ... FROM opportunities
WHERE (detected_at, id) < ($1, $2)      -- comparación de tuplas PG
ORDER BY detected_at DESC, id DESC
LIMIT $3;
```

Requiere índice que matchee el orden `(detected_at DESC, id DESC)`; el cursor devuelto al cliente es el tuple (serializado y firmado si es público). Regla: API externa → cursor siempre; admin interno de baja contención → offset tolerable.

**Presupuesto de rate-limit por host.** El núcleo §3.1 muestra el token bucket con `governor`; lo que añade producción es la granularidad: UN `RateLimiter` por (host, proveedor), no uno global — los 429 son por proveedor, y compartir bucket hace que un Alchemy saturado congele también los calls a Etherscan. Precedente del repo (2026-09-06): golpear un rate-limit con más presión solo generó 63/71 agentes muertos. Además: respetar headers de cuota del proveedor cuando existen (GitHub expone `X-RateLimit-Remaining`/`X-RateLimit-Reset`; Etherscan en cambio avisa el throttle in-band — HTTP 200 con `status:"0"` y `result:"Max rate limit reached"`: hay que parsear el cuerpo, no fiarse del status code) y bajar el burst antes del 429, no después.

**Backoff exponencial con jitter.** `sleep = rand_uniform(0, min(cap, base * 2^intento))` (full jitter, AWS Architecture Blog). Jitter es obligatorio: sin él, N consumidores que fallaron juntos reintentan juntos (thundering herd) y el proveedor vuelve a 429 — el patrón exacto de "self-contamination" registrado en el HG 0902. Retry solo para: (a) requests idempotentes, o (b) no-idempotentes con `Idempotency-Key`. Acotar `max_attempts` (3-5) y deadline total < presupuesto del llamador: un retry que excede el deadline del request padre es trabajo tirado a la basura.

**Idempotency keys (estándar Stripe).** Header `Idempotency-Key: <uuid o hash determinista>`; el servidor persiste `(key, request_fingerprint, response_snapshot)` con TTL (horas) y ante repetición devuelve la respuesta guardada. Fingerprint distinta con la misma key → `400` con error `idempotency_error` (convención Stripe): es un bug del cliente. El repo ya genera la key determinista (núcleo §1.3: keccak de chain_id+opportunity_id+target_block+route_hash+amount_in+mode); el resto del circuito (store + replay de respuesta) es lo que cierra el patrón.

**ETag/If-None-Match.** El edge sirve snapshots poll-eados por el dashboard: ETag fuerte = hash del cuerpo (`If-None-Match` → `304 Not Modified` sin cuerpo) ahorra ancho de banda y CPU de re-render. El 304 también evita recomputar si el contenido no cambió.

**Timeouts acotados.** Todo fetch con timeout explícito: `reqwest::Client::builder().timeout(...)` por cliente + `tokio::time::timeout` por semántica. El default sin timeout es infinito; un hanging RPC arrastra el worker completo.

**Hedged requests.** Para latencias de cola (Dean & Barroso, "The Tail at Scale", CACM 2013): si el request pasa p95 del endpoint, disparar una copia a otro endpoint y quedarse con la primera respuesta. Solo idempotentes, solo con presupuesto de rate-limit para el doble de carga, y con kill-switch por flag: en un proveedor saturado el hedging amplifica el 429 (misma lección anti-amplificación del respawn v1.2).

**Circuit breaker por dependencia.** El núcleo §3.1 implementa el breaker básico; en integración real faltan tres cosas: (1) estado half-open con N probes antes de cerrar; (2) instancia POR dependencia (RPC-1, RPC-2, price feed, cada uno el suyo) con métricas separadas `arbx_integration_breaker{dependency, state}`; (3) el 429/429-exhausted abre el breaker en vez de alimentar retries — un proveedor caído se degrada, no se asfixia.

## 20.3. WEBSOCKET RESILIENTE: HEARTBEAT, SNAPSHOT+DELTA, DEAD-MAN SWITCH

**Heartbeat/ping-pong.** RFC 6455: opcode `0x9` ping / `0xA` pong. Con `tokio-tungstenite`, el cliente responde automáticamente a pings del servidor pero NO los envía por sí solo: se necesita un loop propio que envíe `Message::Ping(payload)` cada T segundos — muchos proveedores (y proxies) matan conexiones silenciosas sin tráfico del cliente. TCP half-open es el fallo traicionero: el socket "vivo" no entrega nada; la única detección honesta es ausencia de frames de datos Y de pongs durante un umbral (p.ej. 2 × ping interval).

**Patrón snapshot+delta con control de secuencia.** Feed de estado (reserves, prices): primero snapshot completo con `seq=N`, luego deltas `seq=N+1...`. Reglas:
- Aplicar delta solo si `delta.seq == last_seq + 1`. Si `seq` salta → GAP: congelar la publicación downstream, marcar la vista como `stale`, pedir snapshot fresco, reconstruir, renumerar y reanudar.
- Nunca extrapolar ni aplicar deltas fuera de orden "porque se parecen": un delta aplicado sobre estado incorrecto contamina silenciosamente todo el grafo de pools.
- Esto es R8 del repo aplicado a feeds: sin datos íntegros → observation con razón exacta (`feed_gap_detected`), no datos falsos.

**Resuscripción con backfill.** Al reconectar: re-suscribir los mismos tópicos Y recuperar lo perdido desde el último checkpoint — `eth_getLogs(fromBlock, toBlock, address, topics)` por rangos acotados, dedup por `(block_number, tx_index, log_index)`. Una reconexión sin backfill produce un agujero permanente en el índice que nadie detecta hasta que una oportunidad fantasma lo expone.

**Dead-man switch del feed.** Watchdog: si el feed no entrega un bloque en k × block_time (Ethereum PoS: 12 s/slot; k=3 razonable), el consumidor que depende del feed se PAUSA a sí mismo (deja de detectar con datos viejos) y alerta — el resto del sistema sigue (misma doctrina que la gang: el caído no detiene al equipo). Pausar la detección con razón registrada es fail-honest; seguir detectando sobre estado de hace 60 s es fabricar.

**Reorgs.** Un feed de bloques también entrega bloques que luego desaparecen: ver §20.8 (unwind). Nota defensiva: la exposición a información del mempool público como insumo (y quien la usa contra ti) se rige por arbx-mev-ethics-gate; aquí solo tratamos la integridad del transporte.

## 20.4. gRPC: STREAMING, DEADLINES, RETRY Y LOAD BALANCING

**Streaming.** Server-streaming = feed de eventos (el server mantiene el stream abierto; el cliente itera `Streaming<T>` con `.message().await`); bidireccional = suscripciones negociadas (cliente pide filtros, server entrega deltas). En `tonic`, el handler server devuelve un stream construido sobre `tokio_stream::wrappers::ReceiverStream::new(rx)` — lo que coloca un `mpsc` acotado entre la lógica y la conexión: backpressure gratis.

**Deadline propagation.** gRPC define el header `grpc-timeout`. En `tonic` (≥0.10) el timeout por request se fija con `Request::set_timeout(Duration)`; PERO tonic NO propaga deadlines automáticamente entre hops: al recibir, hay que leer el tiempo restante y derivar el deadline del call hijo (interceptor o `tokio_util::sync::CancellationToken` cancelado al vencer el presupuesto). Regla: cada hop resta su latencia al presupuesto del trace; el hop final que recibe 50 ms no puede lanzar un sub-call de 5 s.

**Retry policy.** La spec de gRPC define retry en el service config JSON del canal:

```json
{ "methodConfig": [ {
    "name": [{ "service": "arbx.Feed" }],
    "retryPolicy": {
      "maxAttempts": 3, "initialBackoff": "0.1s", "maxBackoff": "1s",
      "backoffMultiplier": 2, "retryableStatusCodes": ["UNAVAILABLE"] }
} ] }
```

`tonic` NO implementa service config ni retries nativos: se implementa con una capa `tower::retry::Policy` sobre el `Channel`, o se delega a un proxy (Envoy/linkerd2 proxy) que sí consume ese JSON. Regla de pureza del repo: retries solo con `UNAVAILABLE`/transitorios; `INVALID_ARGUMENT` o `NOT_FOUND` jamás (reintentar un error determinista es DOS propio).

**LB client-side vs proxy.** Client-side (resolver DNS + policy `round_robin` en clientes gRPC maduros): cero hops extra, pero el binario propio descubre y health-chequea — en tonic solo existe round-robin estático (`Channel::balance_list` sobre un set fijo de endpoints); resolver dinámico y balanceo health-aware hay que construirlos. Proxy-side (Envoy): LB, retries, outlier detection (ejection de endpoints con 5xx consecutivos) resueltos fuera del proceso, al coste de un hop. Para el stack actual (pocos servicios, un VPS): proxy-side simple o endpoint-picker propio con health checks; meter un mesh completo para 4 servicios es sobre-ingeniería.

## 20.5. WEBHOOKS ENTRANTES: VERIFICACIÓN, ANTI-REPLAY, IDEMPOTENCIA, DLQ

**Verificación HMAC sobre el raw body.** El HMAC se computa sobre los bytes crudos ANTES de cualquier parse (re-serializar JSON cambia el cuerpo): Node `crypto.createHmac("sha256", secret).update(rawBody)`, comparación timing-safe con `crypto.timingSafeEqual(a, b)` — jamás `===`. Convenciones reales: GitHub `X-Hub-Signature-256: sha256=<hex>` (HMAC del raw body); Stripe `Stripe-Signature: t=<ts>,v1=<hmac>` donde el payload firmado es `${t}.${rawBody}`.

**Anti-replay.** Dos capas: (1) ventana temporal — rechazar si `|now - t| > 5 min` (el `t` de Stripe está firmado, no solo presente); (2) nonce/event-id único — `SET arbx:webhook:seen:{event_id} 1 NX EX 86400` en Redis o tabla `processed_webhook_events(event_id PRIMARY KEY)`; insert-first, y si ya existía → `200 OK` inmediato sin reprocesar (el proveedor deja de reintentar).

**Procesamiento idempotente y asíncrono.** El handler hace SOLO: verify → dedup-check → persist evento crudo → enqueue → `2xx` en < 5 s (los proveedores deshabilitan endpoints lentos). El procesamiento vive en un worker del bus (§20.6) con la misma idempotency key determinista del dominio. Respuesta de error del PROCESSING no debe convertirse en error del HANDLER: separar transporte de semántica.

**Queue + DLQ.** Fallos de procesamiento → reintentos acotados (misma política §20.2) → si persiste, mover a `<cola>:dlq` con metadatos (`attempts`, `last_error`, `stack_digest`). Reglas: DLQ con profundidad > 0 es un SLI que alerta inmediatamente (no un cajón); todo DLQ tiene herramienta de replay manual (reinyectar tras fix); un mensaje que crashea el parser (poison) va a DLQ a la primera, no bloquea la cola.

**Riesgo defensivo.** Un secreto de webhook filtrado = endpoint falsificable: la rotación es parte del contrato (§20.9), y el endpoint propio tiene rate-limit + redacción de logs (nunca loguear el body crudo completo: puede contener PII del proveedor).

## 20.6. MENSAJERÍA: KAFKA vs REDIS STREAMS vs NATS JETSTREAM

| Dimensión | Kafka | Redis Streams | NATS JetStream |
|---|---|---|---|
| Log | Log persistente dedicado, retention por tiempo/tamaño | Log dentro de Redis (durabilidad = RDB/AOF de Redis) | Streams en JetStream (file/memory replication) |
| Grupos | Consumer groups, offsets en `__consumer_offsets` | `XGROUP`/`XREADGROUP` + PEL por consumer | Durable consumers, `AckPolicy::Explicit` |
| Orden | Por partición; misma key → misma partición | Orden total del stream (un solo log por clave de stream) | Por subject-consumer |
| Replay | Re-seek de offset nativo | Releer con `XRANGE` (id ≥ 0-0) mientras existan entries | `DeliverAll` al crear consumer |
| Ops | Cluster propio (KRaft), lag monitoring | Ya está en el stack (cero ops nuevas) | NATS server dedicado |
| Fit repo | Overkill hoy; correcto si el log de eventos debe vivir días con replay contractual | **Canónico**: `arbx:opps:detected` (núcleo §1.3, R7) | Si se necesita fan-out por subject jerárquico |

**At-least-once y por qué exactly-once es mito del lado del consumidor.** Kafka ofrece efectivamente-una-vez DENTRO del ecosistema Kafka (productor idempotente `enable.idempotence=true`, transacciones con `transactional.id`), pero el consumidor que procesa y escribe a PostgreSQL/Redis NO participa de esa transacción: crash entre "procesar" y "commit de offset" → reentrega → duplicado. Conclusión operativa innegociable: el consumidor es idempotente SIEMPRE (dedup por la key determinista del núcleo §1.3); las promesas del broker no lo excusan.

**Offsets/acks.** Commit de offset DESPUÉS de procesar (at-least-once); auto-commit (`enable.auto.commit=true` en Kafka) = at-most-once en crash = pérdida silenciosa, prohibido en camino de datos. En Redis Streams el análogo es `XACK` tras procesar; entries sin ack viven en el PEL: monitorear `XPENDING <stream> <group>` y reclamar de consumers muertos con `XAUTOCLAIM`:

```text
loop:
  entries = XREADGROUP GROUP sim-ctl consumer-1 COUNT 32 BLOCK 5000
            STREAMS arbx:opps:detected >          # '>' = solo nuevas
  for e in entries: process(e); XACK arbx:opps:detected sim-ctl e.id
  # recuperación (consumer muerto): robar PEL idle > 60s
  XAUTOCLAIM arbx:opps:detected sim-ctl consumer-2 60000 0-0 COUNT 16
```

`XPENDING` alto sostenido = consumers muriendo o procesamiento colgado — alerta, no rutina. El invariante repo §33.1.3 (`XLEN arbx:opps:detected` delta=0 en fases de control-plane) se verifica con Redis RO antes/después de cualquier cambio aquí.

**Ordering por clave.** Kafka: particionar por la clave de dominio (p.ej. `opportunity_id`) preserva orden por entidad, no global. Redis Streams: el stream ES orden total — si un dominio necesita orden estricto, no partir el stream por hash; si necesita throughput paralelo, partir en streams por shard natural (`arbx:opps:detected:{chain_id}`) aceptando orden por shard.

**Trimming y retention.** `XADD ... MAXLEN ~ 10000` (trim aproximado, barato) para streams calientes; PG es el store de verdad histórico (R7), Redis es el bus caliente — no al revés.

**DLQ y poison messages en el bus.** La misma disciplina del §20.5 aplicada al stream: reintentos acotados por entry (contador en el consumidor, no en Redis); si el fallo persiste, `XADD` del entry a `arbx:opps:detected:dlq` con metadatos (`attempts`, `last_error`, `consumer`) y `XACK` del original para no frenar el grupo. Un poison message (crashea el parser) va a DLQ a la primera, sin reintentos. El PEL no es DLQ: es estado de redelivery, no cuarentena — monitorear ambos por separado.

**Backpressure.** El patrón universal: canal acotado `tokio::sync::mpsc::channel(cap)` entre ingest y procesamiento — `.send().await` bloquea al productor, que a su vez frena el `XREADGROUP` (no leer más de N sin ack) o pausa particiones (Kafka `consumer.pause`). Un consumidor que "resuelve" backpressure descartando en silencio viola R8: si se descarta, se descarga con observation y razón (`consumer_overrun`).

## 20.7. CDC: OUTBOX TRANSACCIONAL Y DEBEZIUM

**El problema dual-write.** Escribir a PostgreSQL y publicar al bus son DOS operaciones: si la segunda falla, PG tiene el dato y el bus no (consumidores ciegos); si se reintenta mal, el bus tiene duplicados. La divergencia es silenciosa y acumulativa.

**Outbox transaccional (el anti-dual-write).** En la MISMA transacción del dominio se inserta el evento en una tabla `outbox`:

```sql
BEGIN;
INSERT INTO opportunities (...) VALUES (...);
INSERT INTO outbox (event_id, aggregate_type, aggregate_id, event_type, payload)
VALUES (gen_random_uuid(), 'opportunity', $id, 'opportunity.detected', $json_payload);
COMMIT;
-- relay (proceso aparte, quizás otro servicio):
SELECT event_id, payload FROM outbox
WHERE published_at IS NULL ORDER BY id LIMIT 100
FOR UPDATE SKIP LOCKED;   -- múltiples relays sin pisarse
```

`SKIP LOCKED` permite N relays paralelos sin contención — a costa del orden global: con UN solo relay publicando en orden de `id` el orden se preserva; con N relays solo queda orden por-agregado, y solo si los eventos del mismo agregado particionan a la misma clave/partición downstream (§20.6). El evento llega al bus at-least-once (relay puede publicar y morir antes del UPDATE) → consumidores idempotentes (§20.6).

**Debezium (PG → bus).** Conector PostgreSQL sobre replicación lógica (`plugin.name=pgoutput`, slot creado con `pg_create_logical_replication_slot`, `wal_level=logical`), config clave: `topic.prefix`, `table.include.list`, `snapshot.mode`. Particiona por PK de tabla → orden por clave preservado por partición. ADVERTENCIA operativa crítica para este VPS: un slot de replicación SIN consumer retiene WAL indefinidamente — el repo ya vivió el disco al 100% por un burst de WAL (precedente pg-wal-burst): cualquier CDC por slot exige alerta de `pg_replication_slots` (lag en bytes) y plan de kill del slot. Por eso el outbox+relay Rust es la opción por defecto del stack: cero piezas nuevas, cero retención de WAL.

## 20.8. INDEXACIÓN BLOCKCHAIN: SUBGRAPHS, PONDER, WATCHERS Y REORGS

**The Graph.** Subgraphs (GraphQL sobre graph-node): útil para análisis batch y datasets históricos, NO para hot-path — sync lag variable y no controlable. Toda query a un subgraph lleva `_meta { block { number } }` y el consumidor RECHAZA (fail-honest) si `_meta.block.number` está más atrás que `latest - tolerancia`. Costes: la red descentralizada cobra query fees en GRT; el hosted service gratuito fue deprecado — presupuestar antes de diseñar alrededor.

**Ponder.** Indexer app propio en TypeScript (paquete `@ponder/core`): config declarativa (`createConfig` con redes y contratos — `abi`, `address`, `startBlock`), handlers por evento que escriben una DB derivada, y GraphQL API generado. El framework persiste checkpoints por bloque y REVIERTE handlers automáticamente en reorg. Encaja para vistas derivadas del dashboard; no sustituye al hot-path Rust.

**viem watch\* y confirmaciones.** `watchBlockNumber` (con `poll: false` usa WS subscription; `poll: true` sondea), `watchContractEvent(client, { address, abi, eventName, onLogs })`, `getLogs(client, { fromBlock, toBlock, event })`. Para txs de pago/settlement: `waitForTransactionReceipt(client, { hash, confirmations: N })` — espera N bloques antes de resolver; en Ethereum PoS 2-3 confirmaciones cubren reorgs realistas, finalidad dura solo a ~2 epochs. Regla repo: estado "confirmed" en UI/persistencia = con confirmaciones explícitas, no al primer receipt.

**Reorg handling (unwind) — el indexer lo escribe.** viem entrega bloques; NO deshace tu estado. Patrón: cada evento indexado guarda `(block_number, block_hash)`; al recibir un bloque cuyo `parentHash` no matchea el checkpoint:

```text
on_block(b):
  if b.parentHash != checkpoint.hash:            # reorg detectada
    ancestor = walk_parents(b) hasta coincidir con cadena indexada
    unwind: DELETE eventos WHERE block_number > ancestor.number   # idempotente por PK
    checkpoint = (ancestor.number, ancestor.hash)
  index(b)  #  getLogs(b) → upsert por (address, topic0, block, log_index)
  persistir checkpoint EN LA MISMA TRANSACCIÓN que los eventos
```

Checkpoint transaccional con los datos = restart-safe: un crash a mitad de bloque re-procesa el bloque completo sin duplicar (upsert idempotente). Deep reorg (>6 bloques) alerta inmediata: es evento de seguridad, no rutina.

**Backfill por rangos.** `eth_getLogs` en chunks (2k-10k bloques según límite del proveedor), resumable desde el checkpoint, con presupuesto de rate-limit del §20.2 — y ejecutado en bloques vacíos como el warmup de rutas del núcleo §1.2, jamás compitiendo con el feed caliente.

## 20.9. AUTH ENTRE SERVICIOS: DEL API KEY AL SIWE+EIP-712

**API keys.** Mínimo viable interno: prefijo por servicio (`arbx_searcher_...`) para revocación granular y trazabilidad en logs redactados; SIEMPRE header `Authorization: Bearer`, jamás query param (los query params quedan en logs de proxy, browser history, referrers).

**mTLS.** Certs cliente+servicio mutuamente verificados: cada servicio tiene identidad criptográfica y no hay token que robar en tránsito. En `tonic` (feature `tls`): `ServerTlsConfig` con identidad del server y `with_client_ca_root(...)` para exigir y validar el cert cliente. En el VPS single-host el radio es pequeño, pero la regla vale: mTLS cuando el salto cruza fronteras de confianza (edge ↔ mundo), API key interno de baja exposición.

**OAuth2 client-credentials (RFC 6749 §4.4).** `grant_type=client_credentials&scope=...` → `access_token` + `expires_in`. Cachear el token hasta `0.8 × expires_in` y refrescar proactivo (timer), no on-demand: un 401 en hot-path por token vencido es un bug de planificación, no un "retry".

**Firma HMAC de requests.** Canonical string `(method, path, body_sha256, timestamp)` → HMAC-SHA256 → header `X-Signature`; el receptor re-canonicaliza (¡contrato exacto de serialización! cualquier ambigüedad = firmas inválidas intermitentes), verifica en ventana temporal (anti-replay, igual §20.5).

**SIWE + EIP-712 para wallets.** SIWE (EIP-4361) = login off-chain con mensaje estructurado (`domain`, `address`, `statement`, `uri`, `version`, `chain-id`, `nonce`, `issued-at`) firmado con `signMessage` (EIP-191). Para acciones estructuradas posteriores, EIP-712 typed data: firma con viem `signTypedData({ domain, types, primaryType, message })` y verificación server-side con `verifyTypedData`. Reglas: `nonce` de un solo uso generado server-side, sesión con expiración corta vinculada a la `address`, `domain`/`chain-id` verificados contra lo esperado (anti-phishing de dominio).

**Rotación sin downtime.** Ventana de dual-validity: publicar la credencial nueva → aceptar vieja+Nueva durante un overlap ≥ TTL máximo de la credencial vieja (o ≥ un ciclo completo de refresh) → revocar la vieja → auditar rechazos residuales. Nunca hard-switchoff sincronizado: siempre hay un proceso con la credencial vieja cacheada. Para claves asimétricas, el mecanismo estándar es `kid` tipo JWKS (el consumidor refresca el keyset, no está pinneado).

**Mapa repo.** `SIM_SIGNER_ADDRESS` crash-on-boot si falta (RULE 02) — fail-fast de identidad, no default; secretos SOLO en `.env.mcp` gitignored con `.env.mcp.example` tracked (§33.3); roles read-only para Postgres/Redis MCP (§33.2); nada de esto toca el terminus de firma (`relays-client`), que es default-deny por §34.3.

## 20.10. OBSERVABILIDAD DE INTEGRACIONES: TRACE CONTEXT Y SLO POR DEPENDENCIA

**W3C Trace Context.** Header `traceparent: 00-{trace-id 32 hex}-{parent-id 16 hex}-{flags 02 hex}` (+ `tracestate` opcional). Regla repo: se propaga en TODOS los hops — edge → api-server → searcher → sim-ctl — Y a través del bus: cada entry de `XADD` lleva un campo `traceparent` que el consumer extrae y continúa como padre, porque el stream NO propaga contexto por sí solo. Sin esto, el span del worker cuelga huérfano y R7 (cadena searcher→redis→pg→api→dashboard) se reconstruye a mano.

**Instrumentación.** El núcleo §5.4 inicializa el tracer; la forma que integra con los `#[instrument]` del núcleo §5.2 es `tracing_opentelemetry::layer().with_tracer(tracer)` sobre `tracing_subscriber::registry()`: los spans existentes se exportan solos — cero instrumentación extra para el código ya anotado.

**Correlation IDs.** Además del trace: `x-request-id` / `x-correlation-id` en headers HTTP y como campo persistido (stream entry, fila PG `trace_id`). El correlation ID sobrevive a sistemas que dropean headers (y a consultas psql de forense); el traceparent da el grafo causal. Ambos, no uno.

**SLI/SLO por dependencia externa.** Cada dependencia (RPC-1, RPC-2, price feed, subgraph, API de exchange) es un "servicio" con SLO propio:
- SLI: availability = `1 - errores/total` (errores = 5xx, timeout, breaker abierto), latencia p99, `429 rate`, `rate-limit remaining` cuando el proveedor lo expone.
- Métricas: `arbx_integration_requests_total{dependency, outcome}` y `arbx_integration_duration_seconds{dependency}` (histogram), nomenclatura consistente con núcleo §5.1.
- Alerting multiwindow burn-rate (Google SRE Workbook): ventana rápida (p.ej. 1h/5m, burn 14.4) para page, lenta (6h/30m, burn 6) para ticket. La dependencia degrada ANTES de que la detección lo note: el alerta es sobre la dependencia, no sobre el síntoma downstream.

**Synthetic probes.** La cadena R7 (`docker logs` → `XLEN` → `SELECT MAX(detected_at)` → `curl /api/opportunities/live`) automatizada como probe periódico que publica su propio resultado: detecta "todo verde pero datos estancados" que los health checks por servicio no ven (ver también R9: verificar ventana de logs antes de concluir ausencia).

## 20.11. CONFIG: 12-FACTOR, FLAGS CON KILL-SWITCH, SECRETS, HOT-RELOAD

**12-factor env.** Config por entorno (`std::env::var`), misma imagen para todos los ambientes. Fail-fast colectivo al boot: parsear y validar TODAS las variables ANTES de arrancar cualquier servicio y reportar la lista completa de faltantes (RULE 02: `SIM_SIGNER_ADDRESS` ausente = crash, es seguridad). Defaults silenciosos para variables críticas en prod = prohibido; default solo para variables de calidad de vida (`RUST_LOG`).

**Feature flags con kill-switch por flag.** Cada flag: default-off, evaluado EN RUNTIME (no horneado), override instantáneo (Redis key o archivo observado) y — condición de diseño — apagable individualmente Y en masa por el kill-switch global (<10 ms, CLAUDE.md §9). Flags reales del repo: `ARBX_ORCHESTRATOR_MODE` / `ARBX_CARTRIDGE_MODE` (migración, §34.2) y `ARBX_LIVE_EXEC_ENABLED` (default-deny del terminus, §34.3 — intocable: ningún flag de producto lo sustituye).

**Gotcha Next.js (RULE 03/04).** `NEXT_PUBLIC_*` se hornea durante `next build`: cambiar `.env` sin rebuild = flag fantasma. Flags de frontend runtime → servidas por el backend (endpoint de config); rebuild `--no-cache` + `--env-file .env` solo para lo horneado de verdad.

**Secret manager vs env.** Env vars para no-secretos (URLs, puertos, niveles de log); credenciales fuera del proceso (Docker secrets / secrets manager, ver núcleo §6.1) — en archivos versionados solo placeholders `${VAR}` (§33.2), con el patrón repo `.env.mcp.example` tracked / `.env.mcp` gitignored. Rotación según §20.9.

**Hot-reload SOLO de config no crítica.** Con `arc-swap` (`ArcSwap<Config>` + `load()` en cada uso) y observador de archivo (crate `notify`) o poll de Redis key: niveles de logging, umbrales de diagnóstico, sampling de tracing. Lo crítico (endpoints RPC, auth, límites de riesgo) cambia con restart controlado y deploy veraz — hot-reload de un risk limit es exactamente el tipo de cambio silencioso que el hardening §37 prohíbe.

## 20.12. MATRIZ DE DECISIÓN, ANTI-PATRONES Y MAPEO AL STACK

**Matriz de decisión rápida.**

| Necesidad | Elección | Por qué |
|---|---|---|
| Bus entre servicios con Redis ya presente | Redis Streams | Cero ops nuevas; R7/R8 ya modelados sobre él |
| Log de eventos con replay contractual de días | Kafka | Retention + re-seek nativo; coste de cluster |
| Fan-out por subject jerárquico + KV | NATS JetStream | Modelo de subjects + KV integrado |
| Estado derivado del dashboard desde eventos on-chain | Ponder (o subgraph para batch) | Checkpoints y reorgs manejados por el framework |
| Feed caliente de bloques/logs | viem watch* + WS propio (§20.3) | Control total de latencia y de gap-handling |
| Eventos de dominio → bus sin divergencia | Outbox + relay (§20.7) | La única cura real del dual-write |
| Paginación pública de `opportunities` | Cursor keyset | Estable bajo insert continuo |

**Anti-patrones que esta referencia prohíbe** (cada uno con su referencia): dual-write sin outbox (§20.7); retry sin jitter ni presupuesto (§20.2); auto-commit de offsets en camino de datos (§20.6); procesar webhooks sync en el handler (§20.5); servir vista stale tras gap de secuencia sin marcar (§20.3, R8); timeout por defecto infinito (§20.2); reorg ignorada en indexer (§20.8); flag de frontend horneado esperado como runtime (§20.11, RULE 03); exactly-once asumido del broker sin consumidor idempotente (§20.6); secreto en query param o YAML versionado (§20.9, §33.2).

**Mapeo al stack ArbitrageX.**

```text
[RPC/WS proveedores]──(§20.2 presupuesto+breaker, §20.3 resync)──▶ [searcher-rs]
      │                                                          │ outbox/carrier (§20.7, núcleo §1.3)
      ▼                                                          ▼
 [Redis Streams arbx:opps:detected] ◀── XLEN delta=0 (§33) ── [productores]
      │ XREADGROUP/XACK/XAUTOCLAIM (§20.6)        traceparent por entry (§20.10)
      ▼
 [sim-ctl workers]──(§20.1 contratos vN/vN+1)──▶ [PostgreSQL R7]──(§20.7 CDC opcional)──▶ bus
      │                                                        ▲
      ▼                                  cursor keyset (§20.2)    │
 [api-server WS :8080 directo, RULE 02] ────────────────────────┘
      ▲ snapshot+delta + stale flag (§20.3)
      │
 [edge :8787 REST]──flags runtime (§20.11)──▶ [frontend Next.js]
                                     [relays-client terminus §34: default-deny, intocado por esta ref]
```

Cada patrón de esta referencia aterriza en un punto concreto de ese mapa; si un cambio de integración no puede señalar su casilla (y su invariante: R7 cadena, R8 honestidad, XLEN delta=0), es diseño sin anclaje y vuelve a la mesa de dibujo.

## GOBERNANZA

Este conocimiento está subordinado a los gates `arbx-*` (arbx-paper-trade-first, arbx-simulation-mandatory, arbx-risk-limits-enforcement, arbx-pre-execute-checklist) y a CLAUDE.md §34: LIVE_MAINNET es gated, el terminus `relays-client` es default-deny y ningún patrón de integración aquí descrito autoriza flip a live, broadcast con capital real, ni modificación de la allowlist `ARBX_LIVE_EXEC_ENABLED`/`ARBX_LIVE_EXEC_CHAINS` fuera del switch autorizado (orden 2026-09-17: prohibido añadir restricciones a mainnet). Aplicar cualquiera de estos patrones al stack requiere pasar el embudo §37 y verificar con los invariantes R7/R8 del repo.
