# 16. Ingeniería de Latencia Nivel Máquina: presupuesto, medición y afinamiento

CUÁNDO CARGAR ESTA REFERENCIA: p99/p999 fuera de SLO en cualquier etapa del pipeline; antes de tocar un hot path "porque parece lento"; elegir/tunear allocator, runtime tokio o canales; evaluar migración epoll→io_uring; seleccionar región/colo o tuning de sockets; diseñar backpressure entre etapas; auditar latencia E2E (red→parse→decisión→firma→envío); benchmarkear Rust con criterion o perfilar con flamegraph; latencia del control-plane TypeScript.

| Concepto | Herramienta/Patrón | Nota clave |
|---|---|---|
| Distribución de latencia | crate `hdrhistogram` | 3 cifras significativas; exportar p50/p99/p999, no promedios |
| Micro-benchmark | `criterion` (`--save-baseline`/`--baseline`) | `std::hint::black_box` o el optimizador borra tu trabajo |
| CPU profiling | `cargo flamegraph` (Linux `perf`) | perf no existe en Windows: perfilar en el VPS (RULE 01) |
| Introspección async | `tokio-console` + `console-subscriber` | Requiere `RUSTFLAGS="--cfg tokio_unstable"` |
| Runtime | `tokio::runtime::Builder` | `worker_threads`, pool `spawn_blocking`, LIFO slot implícito |
| Backpressure | `tokio::sync::mpsc::channel(n)` acotado | `try_send` + shed honesto; `unbounded` = bomba de OOM |
| I/O async | epoll (mio/tokio) vs io_uring (`tokio-uring`, `monoio`) | En completion I/O el kernel es dueño del buffer hasta el CQE |
| JSON de market data | `simd-json` (`to_borrowed_value`), `sonic-rs` | Borrowed values: parse sin construir árbol poseído |
| Allocator global | `mimalloc` o `tikv-jemallocator` | Elegir UNO con medición; nunca ambos |
| Estado leído-por-muchos | `arc-swap` (estilo RCU) | Snapshot inmutable por bloque; lectores nunca bloquean |
| Transporte | `set_nodelay(true)` + pooling + TLS resumption | El handshake debe pagarse una vez por endpoint, no por request |
| Deadline entre etapas | `tokio::time::timeout_at` + budget `Instant` | Vencido = observation `deadline_exhausted` (R8), nunca colar |

El núcleo (referencia 00, §1-§10) define la arquitectura C-S-E y la observabilidad base; esta referencia baja al hueso de la latencia por capa. La latencia del hot path es mode-invariant (CLAUDE.md §34.1): en `PAPER_SHADOW` medimos exactamente las mismas etapas porque la telemetría ES el producto del modo.

## 16.1 Doctrina: medir antes de optimizar y presupuesto por etapa

**Perfil > intuición.** El modelo mental de dónde va el tiempo siempre está parcialmente equivocado; el perfilador no. Regla operativa: ningún PR de performance sin artefacto before/after reproducible (extensión de §37 del repo: la carga de la prueba es del cambio). Un benchmark en el dev Windows NO transfiere al VPS Linux (allocator, scheduler, stack TCP distintos): los números que valen son los del entorno de deploy.

**Reloj correcto.** Latencia SIEMPRE con reloj monótono (`std::time::Instant`); jamás `SystemTime` (saltos NTP rompen distribuciones). Si el coste de `Instant::now()` (~20-25 ns vía vDSO) importa en un loop de miles de muestras, el crate `quanta` ofrece timestamps monótonos con calibración cacheada.

**Presupuesto de latencia (SLO por etapa).** Asignar un p99 por etapa ANTES de medir, para convertir "es lento" en hipótesis falsable. Plantilla ilustrativa a calibrar con medición propia del sistema (estas cifras NO son datos medidos del repo — RULE 00):

```
t0 ─[red: WS feed]─► t1 ─[parse frame]─► t2 ─[detección+pricing]─► t3 ─[gates]─► t4 ─[firma/terminus]─► t5
        RTT+NIC           SIMD JSON            grafo en memoria        checks        paper: ledger / live: POST
◄───────────────────────── presupuesto E2E p99; cada etapa se mide contra su slice ─────────────────────────►
```

| Etapa | SLO p99 plantilla | Instrumento |
|---|---|---|
| recepción WS → frame completo | 200 µs | timestamp al leer (read_buf) vs timestamp del evento |
| parse tx (JSON + hex) | 150 µs | criterion en CI + hdrhistogram en vivo |
| update grafo + detección de ciclo | 800 µs | span por evento (núcleo §5.4) |
| simulación revm | ~15 ms | métricas ya existentes (núcleo §5.1) |
| pricing + risk gates | 1 ms | span por oportunidad |
| armado + firma | 2 ms | bench local; en paper el terminus no firma (§34) |
| envío al relay (cuando exista) | RTT medido | `ss -ti` del socket de submission |

Cuando una etapa rompe su slice, el deadline de §16.9 decide si el evento sigue vivo o se registra observation y se corta.

**Latencia vs throughput.** Son objetivos que chocan: batching sube throughput y sube latencia. Para el hot path de detección se optimiza latencia p99 bajo carga pico de mempool; para el backfill/analytics rige throughput. No confundir los budgets.

## 16.2 Distribuciones: p50/p99/p999, jitter y teoría de colas

**Percentiles.** p50 = experiencia mediana (casi irrelevante para competitividad); p99 = donde vive la cola de espera; p999 = donde viven GC, page faults y preemption. A 1.000 oportunidades/hora, un p99 se incumple 10 veces por hora: eso es una vez cada 6 minutos, no un evento raro.

**Jitter** = varianza de la latencia. Dos sistemas con p50 idéntico y p999 distinto son sistemas distintos; un dashboard que solo muestra p50 (o peor, el promedio) oculta exactamente lo que hay que arreglar. Fuentes típicas de jitter: pausas de GC (mayores en el control-plane TS), context switches involuntarios, page faults (mayor = disco), TLB misses en estructuras grandes, escalado de frecuencia, IRQ storms, vecinos ruidosos en VPS compartidos.

**Teoría de colas: la espera explota cerca de utilización 1.** Para una cola M/M/1 con tiempo de servicio S y utilización ρ=λ/μ, la espera media en cola es E[Wq]=S·ρ/(1−ρ):

| ρ | E[Wq] (×S) | Lectura operativa |
|---|---|---|
| 0.50 | 1.0 | cómodo; burst absorbido |
| 0.80 | 4.0 | la cola ya es visible en p99 |
| 0.90 | 9.0 | p99 se despega del p50 |
| 0.95 | 19 | jitter dominado por la cola |
| 0.99 | 99 | subir a ρ=0.995 ya duplica la espera; metastable |

Consecuencia: un servicio crítico en latencia se dimensiona a ρ≤0.5-0.7 incluso con CPU "sobrada", porque las llegadas del mempool son bursty por bloque, no Poisson limpio. La saturación puede hacerse metastable (reintentos que amplifican λ): shed honesto temprano (R8) es más barato que retry storms.

**Amplificación de cola entre etapas.** Si cada una de k etapas independientes tiene probabilidad p de ser "lenta", la probabilidad de que el pipeline E2E sea lento es ≈k·p (unión). Con 10 etapas al 1%: ~10% de eventos E2E lentos. Por eso cada etapa recibe su slice de §16.1 y se defiende con deadlines (§16.9), no con esperas infinitas. Concepto canonizado por Dean & Barroso, "The Tail at Scale" (CACM 2013).

**Coordinated omission (Gil Tene).** Un benchmark loop mide solo cuando él mismo no está detenido: si el sistema se atasca 1 s, el loop "espera" y no registra esa espera → colas truncadas artificiales. Corrección: cadenciar las iteraciones con un timer externo y registrar la latencia DESDE el instante previsto de envío (la API Java `recordValueWithExpectedInterval` lo automatiza; en Rust, cadenciar manualmente con ticks de `tokio::time::interval` y computar contra el tick esperado).

## 16.3 Benchmarking y profiling en Rust

**criterion** (setup, medición, comparación con baseline):

```rust
use criterion::{criterion_group, criterion_main, Criterion, Throughput};
use std::hint::black_box;

fn bench_parse_mempool(c: &mut Criterion) {
    let payload = include_bytes!("../../fixtures/mempool_tx.json"); // fixture real, RULE 00
    let mut group = c.benchmark_group("parse/mempool_tx");
    group.throughput(Throughput::Bytes(payload.len() as u64));
    group.bench_function("simd_json_borrowed", |b| b.iter(|| {
        let mut buf = payload.to_vec();          // simd-json requiere buffer mutable
        black_box(simd_json::to_borrowed_value(&mut buf).unwrap())
    }));
    group.finish();
}
criterion_group!(benches, bench_parse_mempool);
criterion_main!(benches);
```

- `iter_batched(setup, routine, BatchSize::SmallInput)` excluye el setup del coste (crítico cuando el setup clona buffers).
- Regresiones: `cargo bench -- --save-baseline pre` → cambio → `cargo bench -- --baseline pre` reporta delta % con significancia. El baseline es el artefacto que §37 exige para el PR.
- Higiene del host de bench: governor `performance` (`cpupower frequency-set -g performance` o `tuned-adm profile latency-performance`), pin (`taskset -c`), varias corridas. Sin esto, criterion mide el jitter del host.

**cargo flamegraph / perf.** `cargo flamegraph --bench parse -- --bench` genera el SVG de muestreo (Linux; en el VPS). Equivalente manual: `perf record -F997 -g -- <bin>` + `perf report`. Para latencia con timestamps por span, `tracing-tracy` + profiler Tracy es una alternativa con vista temporal, no solo agregado.

**tokio-console.** Binario del runtime async: `console-subscriber` con `console_subscriber::init()`, compilar con `RUSTFLAGS="--cfg tokio_unstable"` y features de tracing de tokio, conectar el CLI `tokio-console`. Muestra por task: poll durations, cantidad de wakers, tiempo desde el último wake. Es LA herramienta para detectar "tarea que monopoliza un worker" (poll de ms en un runtime que debería ser de µs) y wakes en cascada.

**HdrHistogram en vivo.** Prometheus (núcleo §5.1) usa buckets fijos con interpolación (`histogram_quantile`); para diagnóstico fino, registrar también en un `hdrhistogram` local de 3 cifras significativas:

```rust
use hdrhistogram::Histogram;
let mut h = Histogram::<u64>::new_with_bounds(1, 60_000_000_000, 3)?; // 1ns..60s
h.record(start.elapsed().as_nanos() as u64).ok();   // bound generoso: nunca caer fuera
let p99 = h.value_at_quantile(0.99);
```

## 16.4 Runtime async: tuning de tokio

**Builder.** `tokio::runtime::Builder::new_multi_thread().worker_threads(n).enable_all().build()`; también `TOKIO_WORKER_THREADS` como env. `n` = cores que el servicio realmente posee; oversubscribir (más workers que cores, o N servicios en la misma caja cada uno con todos los cores) produce context-switch storms visibles en `vmstat 1` (columna `cs`). No hay flag público para el LIFO slot: es optimización interna del scheduler multi-thread — cuando una tarea despierta a otra desde el mismo worker, la despertada corre primero en ese worker (cache caliente). Diseñar en consecuencia: productor y consumidor del hot path en el MISMO runtime.

**Prohibido blocking en threads del runtime.** `std::thread::sleep`, fs síncrono, locks largos, DNS síncrono o cualquier syscall bloqueante dentro de una task congela el worker entero y aparece como p999 inexplicable. Síntoma en tokio-console: poll de cientos de ms. Correcto: `tokio::task::spawn_blocking` (pool propio, dimensionable con `Builder::max_blocking_threads`) para CPU/blocking que no debe tocar el reactor. `block_in_place` solo existe en runtime multi-thread y degrada el worker: último recurso.

**Batching de wakeups.** Cada `send` en un channel despierta al receptor; 1.000 wakes para 1.000 ítems de un mismo bloque es puro overhead. Patrón drain: al despertar, vaciar el canal antes de volver a `await`:

```rust
while let Some(ev) = rx.recv().await {
    handle(ev);
    while let Ok(ev) = rx.try_recv() {   // drenar el batch acumulado en UN wakeup
        handle(ev);
    }
    maybe_coalesce_state();
}
```

**Backpressure con bounded channels.** `mpsc::channel::<T>(cap)` acotado; el productor usa `try_send` y ante `TrySendError::Full(evt)` aplica shed honesto (contador + observation con la razón exacta, R8) o aplica la señal hacia atrás. `unbounded_channel` es un anti-patrón en hot path: convierte backpressure en crecimiento de memoria hasta OOM bajo burst.

## 16.5 I/O y parseo: epoll vs io_uring, zero-copy, JSON SIMD, monomorfización

**epoll vs io_uring.** epoll (vía `mio`, debajo de tokio) es readiness: "puedes leer ahora", tú provees el buffer por syscall. io_uring es completion: entregas el buffer AL kernel y lo recuperas con el CQE — menos syscalls (SQE/CQE rings), pero el ownership del buffer pasa al kernel hasta completar. Crates: `tokio-uring` (runtime dedicado; sus streams devuelven `(io::Result<usize>, buffer)` al completar porque tomaron ownership), `monoio` y `glommio` (thread-per-core). Migrar solo con evidencia: primero `strace -c -p PID` para contar syscalls/evento; con pocos sockets y frames pequeños, epoll + drain batching ya es rápido y io_uring añade complejidad de lifecycle de buffers.

**Parse zero-copy de frames WebSocket.** Leer directo a `BytesMut` con `AsyncReadExt::read_buf(&mut buf)` (append sin copia extra); trocear con `Bytes` (refcount, sin copia). Sobre el payload: parsear direcciones desde bytes (`alloy_primitives::Address::from_slice(&slice20)`), hashear el raw con `keccak256(bytes)` — JAMÁS `String::from_utf8(payload)` para volver a parsear hex después. Si se customiza el handshake HTTP de upgrade, `httparse` parsea headers sobre bytes sin allocar.

**JSON de market data.** `simd_json::to_borrowed_value(&mut buf)` parsea con SIMD devolviendo valores prestados del propio buffer (requisito: buffer mutable con padding); alternativa `sonic-rs`. Donde aplica serde clásico: `serde_json::from_slice` (no `from_reader`, que no puede inspeccionar el input completo y es documentadamente más lento); para serializar reutilizando buffer, `serde_json::to_writer(&mut buf, &v)` — `to_vec`/`to_string` devuelven un buffer nuevo cada llamada.

**Static dispatch (monomorfización).** En el hot path, generics concretos se monomorfizan e inlinean; `dyn Trait` añade vtable y corta inlining. Con conjunto cerrado (los operadores del repo son finitos), enum dispatch (`match` sobre un enum de estrategias) logra despacho estático sin genéricos virales. `dyn` sigue legítimo fuera del hot path; decidir con criterion, no con dogma.

**Pre-sizing.** Toda colección de tamaño predecible nace con capacidad: `Vec::with_capacity`, `HashMap::with_capacity` para tablas de pools en boot, `BytesMut::reserve` antes de leer frames de tamaño conocido. Un `push` que re-alloc en el loop de detección es un p999 gratis.

## 16.6 Allocators y layout de memoria

**Allocator global** (uno solo, elegido con benchmark):

```rust
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;          // o bien:
// static GLOBAL: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;
```

mimalloc: excelente generalista, funciona bien también en Windows dev. jemalloc (tikv-jemallocator): favorito en servicios Linux multithread de larga vida con perfiles de alloc heterogéneos. Verificar con `perf stat` (mayores/minor faults) y con el propio bench, no por moda.

**Arenas en hot path.** `bumpalo::Bump` por evento/oportunidad: allocs lineales casi gratis (`bump.alloc(x)`), todo el scratch (`bumpalo::vec![in &bump; 0u8; 64]`, strings de `bumpalo::collections`) muere de una vez al dropear el bump. Ideal para el parse→decisión de UNA oportunidad; jamás compartir la arena entre tareas concurrentes.

**Inline containers.** Rutas de 2-5 hops: `SmallVec<[Hop; 8]>` vive en stack sin tocar el heap; `ArrayVec<u8, 32>` con `try_push` acota por construcción. Cada `Vec`/`String` evitado en el loop es una alloc (y una potencial re-alloc) menos.

**Reuso de `BytesMut`.** Mantener un pool por worker de buffers de recepción (extensión del pooling del núcleo §7.1, pero por-worker para evitar el lock del pool); `Bytes::freeze()` comparte el contenido por refcount cuando el frame debe sobrevivir al buffer.

**Cero `String`/`Vec` en el hot loop.** Números con `itoa::Buffer`/`ryu::Buffer` (stack, sin alloc); direcciones como `Address` ([u8;20]) y cantidades como `U256` — el hex solo en el borde serializador.

**False sharing.** Dos contadores atómicos escritos por workers distintos en la misma línea de 64 B se invalidan la caché mutuamente: p99 CPU sin culpa del algoritmo. Separar con `crossbeam_utils::CachePadded<T>` o `#[repr(align(64))]`.

**SoA vs AoS.** Si el bucle de pricing toca reserves de N pools pero no sus metadatos, separar campos: array contiguo de reserves (SoA del campo caliente) = iteración secuencial que el prefetcher de hardware alimenta solo; AoS salta de struct en struct desperdiciando cada línea de caché cargada.

**NUMA, hugepages, prefetch.** NUMA: mantener threads, IRQs de la NIC y memoria en el mismo nodo (`numactl --cpunodebind=0 --membind=0`; nodo de la NIC en `/sys/class/net/<if>/device/numa_node`; verificar con `lscpu` — muchas VPS exponen un solo nodo). Hugepages: reducen TLB misses en arenas grandes, pero THP `always` introduce stalls de compactación — preferir `madvise` o mapeos 2MB explícitos para las arenas de §16.2. Prefetch manual (`std::arch::x86_64::_mm_prefetch`): último recurso después de que el layout (adyacencia) ya no da más; medir con `perf stat` (cache-misses) antes y después.

## 16.7 Transporte de red y topología de despliegue

**TCP_NODELAY.** `stream.set_nodelay(true)?` en TODO socket de submission y clientes RPC: desactiva Nagle; sin esto, la interacción Nagle+delayed-ACK puede sumar ~40 ms por request pequeño. Es el fix de mejor ratio costo/beneficio de toda la sección.

**Buffers de socket.** Con `socket2`: `SockRef::from(&stream).set_recv_buffer_size(1 << 20)?` (idem send); Linux capea por `net.core.rmem_max`/`wmem_max` (sysctl). Buffers mayores ayudan a absorber bursts de frames; NO aceleran mensajes pequeños individuales.

**BBR vs cubic.** `sysctl net.ipv4.tcp_congestion_control=bbr` (módulo `tcp_bbr`, Linux ≥4.9). BBR brilla en caminos con pérdida; intra-región con RTT de <2 ms, cubic suele ser suficiente. Decidir mirando `ss -ti` (rtt/cwnd/retrans por socket), no por defecto.

**Región/colo.** El presupuesto de red se divide en dos RTT distintos: feed (WS del mempool/RPC, influye en DETECTAR) y submission (relay/builder, influye en GANAR). Medir p99 RTT desde candidatos al conjunto de endpoints relevantes (`mtr --report -c 100`, `ping -i 0.2`): la región correcta minimiza la suma ponderada por criticidad. En VPS cloud la elección es región del proveedor (para el repo: regiones Hetzner — medir contra los relays usados, ver referencia operacional `vps-readonly-y-mapa.md`). No asumir: medir desde cada candidato.

**DNS.** Resolver en boot y cachear; para endpoints críticos, `/etc/hosts` o un resolver en proceso (`hickory-resolver`, ex trust-dns) con caché TTL. Un lookup por conexión nueva está OK; un lookup por request es un bug de pooling.

**Pooling/keep-alive.** El pool de hyper (núcleo §7.2) mantiene conexiones tibias; la señal de que el pooling falla es ver `ClientHello` TLS por request en una captura (`tcpdump -i any -w cap.pcap 'tcp port 443'` + `tshark -r cap.pcap -Y "tls.handshake.type == 1"`). Post-warmup debe haber CERO handshakes nuevos.

**TLS.** TLS 1.3 cuesta 1-RTT de handshake; con session resumption (caché de sesiones/tickets del `ClientConfig` de rustls, compartido como un único `Arc<ClientConfig>` en todas las conexiones) la reconexión evita el handshake completo. Objetivo: pagar TCP+TLS una vez por endpoint y luego solo payloads.

**QUIC.** `quinn` (QUIC en Rust) elimina head-of-line de TCP y permite 0-RTT al reconectar; útil SOLO si el endpoint opuesto expone HTTP/3 (la mayoría de JSON-RPC es HTTP/1.1 o h2 sobre TCP). Para submission la latencia dominante es la política de inclusion del builder, no el transporte: no migrar sin medir.

## 16.8 Concurrencia y estado compartido

**Sharding: dashmap.** `DashMap<K,V>` parte en shards con `RwLock` propio; `DashMap::with_shard_amount(n)` (potencia de 2) al tamaño del working set. `get()` devuelve un guard que MANTIENE el read-lock del shard: jamás sostenerlo a través de un `.await` (serializa el shard tras el scheduler) ni hacer lookups anidados que puedan deadlockear shards (regla práctica: lock→lookup→drop→procesar).

**Atomics con Ordering correcto.** `Relaxed` para contadores estadísticos sin ordenamiento; par Release(store)/Acquire(load) para publicar datos (el Acquire garantiza ver lo escrito ANTES del Release); `SeqCst` solo cuando se necesita orden total entre variables atómicas distintas. Un Ordering flojo NO falla al compilar: produce lecturas stale intermitentes que solo se ven en el p999 bajo carga — la peor clase de bug.

**Lock-free: crossbeam.** `crossbeam_channel::bounded(n)` para puentes thread↔task (MPMC rápido y con backpressure); `crossbeam_queue::ArrayQueue` para colas fijas lock-free en el borde; `crossbeam_epoch` para reclaims diferidos en estructuras RCU artesanales (solo si de verdad se necesita — arc-swap cubre el 90% de los casos).

**Double-buffer / RCU con arc-swap.** El patrón del estado leído-por-muchos-escrito-por-uno (tabla de pools/reserves refrescada por bloque, leída por todos los workers de detección):

```rust
use arc_swap::ArcSwap;
use std::sync::{Arc, LazyLock};
// ArcSwap::from_pointee no es const fn (crea un Arc): la static necesita LazyLock
static POOLS: LazyLock<ArcSwap<PoolTable>> =
    LazyLock::new(|| ArcSwap::from_pointee(PoolTable::default()));

// escritor (1× por bloque): construye la tabla COMPLETA fuera de línea y swapea el puntero
POOLS.store(Arc::new(new_table));
// lectores (hot path): snapshot inmutable consistente, sin lock, sin copia
let snapshot: Arc<PoolTable> = POOLS.load_full();
```

Los lectores nunca bloquean al escritor ni entre sí; cada snapshot es una generación atómica. Diagnóstico de contención de locks: en flamegraph aparecen como `futex`/parking lot; tokio-console lo muestra como tareas eternamente sin poll.

## 16.9 Deadline propagation entre etapas

Budget explícito que viaja con el evento por todo el pipeline:

```rust
#[derive(Clone, Copy)]
pub struct Deadline { pub at: std::time::Instant }

impl Deadline {
    pub fn remaining(&self) -> std::time::Duration {
        self.at.saturating_duration_since(std::time::Instant::now())
    }
    pub fn expired(&self) -> bool { std::time::Instant::now() >= self.at }
}
```

- Cada etapa chequea `remaining()` ANTES del trabajo caro: si `remaining() < costo_estimado`, no se simula ni se arma nada — se registra observation `deadline_exhausted` (R8 fail-honest) y se corta la rama. Una oportunidad que llegaría tarde al bloque objetivo es simulación quemada gratis.
- Envolver la etapa entera: `tokio::time::timeout_at(deadline.at, etapa_fut)` — vencido el plazo, el future se cancela por `Drop`. Ojo: las tasks spawnadas al margen NO se cancelan solas; propagar con `tokio_util::sync::CancellationToken` para que los side-jobs mueran con el pipeline.
- El budget E2E de §16.1 se descompone en slices por etapa y se AUDITA: tasa de shed por etapa es una métrica de primer orden (alertable como las del núcleo §5.3); shed creciente = saturación temprana, no "mala suerte".

## 16.10 Control-plane TypeScript

**El control-plane NO debe estar en el camino de µs.** La arquitectura C-S-E (núcleo §1) pone el hot path en Rust; la disciplina del lado TS es que así siga siendo y que, donde TS sí procesa streams (edge worker, agregación de cards), su latencia se mida igual.

**Offload a workers.** `worker_threads` + transferables: `worker.postMessage({ frame: buf }, [buf])` MUEVE el `ArrayBuffer` (zero-copy: el emisor pierde la referencia; structured clone de objetos planos sí copia). Para señalización de baja latencia entre main y worker: `SharedArrayBuffer` + `Atomics.wait`/`Atomics.notify` sobre un `Int32Array` compartido (ring de productor/consumidor sin copia).

**Presión de GC.** V8 para en minor GC (scavenger) proporcional a la tasa de allocación del young generation: el equivalente TS del p999 de Rust. Mitigación: hot loop sin allocs (typed arrays reutilizados `Float64Array`, pools de objetos, evitar closures que capturen y aloquen); agrandar el young gen `--max-semi-space-size=64` (MiB) reduce frecuencia de scavenges; diagnosticar pausas con `--trace-gc`. El monitor del loop de eventos (el "p999" del proceso Node):

```js
const { monitorEventLoopDelay } = require('node:perf_hooks');
const h = monitorEventLoopDelay({ resolution: 20 });   // resolución en ms (default 20)
h.enable();
setInterval(() => {
  h.disable();
  emit_metric('eventloop_p99_ms', h.percentile(99));   // valores en ms; exportar, no loguear per-tick
  h.reset(); h.enable();
}, 10_000).unref();
```

**Backpressure.** Colas acotadas con drop-contador; en fan-out WS, un consumidor lento se desconecta, NUNCA se le acumula buffer infinito; streams Node con `highWaterMark` explícito y respeto a `pause()/resume()`.

**HTTP cliente.** `undici`: `new undici.Agent({ connections: 128, pipelining: 10 })` (instalable como dispatcher global con `undici.setGlobalDispatcher`) multiplexa sobre conexiones keep-alive; `setNoDelay(true)` en sockets crudos propios.

## 16.11 Anti-patrones con cicatrices

- **Logging en hot loop** — LOGFLOOD-01 (CLAUDE.md R9): un loop honesto emitiendo 183 líneas/s llenó 50 MB de logs en ~10 min y generó un falso diagnóstico de deadlock. Regla: per-ítem a `debug!`, UN summary agregado a `info!`. Además del ruido, cada línea formatea (`format!` = alloc).
- **Métricas que alocan**: construir label strings por muestra (`format!("pool_{}", id)`) es más caro que la operación medida; labels estáticos + cardinalidad acotada.
- **Dashboards de p50/promedio**: dos sistemas con igual p50 y distinto p999 son indistinguibles ahí. Exigir p99+p999 por etapa.
- **Benchmark sin `black_box`**: el optimizador elimina el trabajo y devuelve "0.3 ns" — no es speed, es eliminación.
- **`unbounded_channel` como fix de backpressure**: convierte overload en OOM diferido.
- **Lock (std o guard de dashmap) sostenido a través de `.await`**: p999 fantasma que solo carga aparece; tokio-console lo delata.
- **Oversubscription**: más workers que cores (o varios runtimes repartiéndose la caja) → lluvia de context switches (`vmstat 1`, columna `cs`).
- **Optimizar sin artefacto before/after**: viola §37; PR rechazable.
- **Migrar a io_uring "porque es moderno"** sin conteo de syscalls (`strace -c`) que lo justifique.
- **Números de dev Windows presentados como VPS Linux**: stack distinto, conclusión inválida (RULE 01).

## 16.12 Checklist: auditoría de latencia por capa

Cada capa, la pregunta y la herramienta que la responde (ejecutar en el VPS, sobre el servicio real bajo carga real o replay):

| # | Capa | Pregunta | Herramienta | Criterio de pase |
|---|---|---|---|---|
| 1 | Red | ¿RTT p99 y pérdida hacia feed y relay? | `mtr --report -c 100`; `ss -ti` por socket | RTT p99 dentro del slice de §16.1; retrans ≈ 0 |
| 2 | Red | ¿Handshake TLS por request? | `tshark -Y "tls.handshake.type == 1"` post-warmup | 0 ClientHello nuevos |
| 3 | Kernel | ¿Syscalls por evento? | `strace -c -p <PID>` | presupuesto por evento conocido y estable |
| 4 | Parse | ¿p99 del parser? | bench criterion + hdrhistogram en vivo | < slice §16.1; sin re-alloc (pre-sizing verificado) |
| 5 | CPU | ¿Dónde se queman ciclos? | `cargo flamegraph` / `perf record -F997 -g` | hotspot identificado con % y dueño por etapa |
| 6 | Scheduler | ¿Polls largos / wakes en cascada? | tokio-console | ningún poll > ~100 µs en hot path |
| 7 | Colas | ¿Utilización y shed por etapa? | contadores de shed + Prometheus | ρ≤0.7 nominal; shed ≈ 0 fuera de bursts |
| 8 | Memoria | ¿Allocs/evento estables? | `perf stat` (faults), stats del allocator, bench | sin crecimiento por evento (arena/pool efectivos) |
| 9 | Decisión | ¿p999 por etapa? | hdrhistogram exportado por etapa | p999 ≤ ~10× p50; sin bimodalidad oculta |
| 10 | Firma | ¿Costo de firmar conocido? | bench local del signer | costo medido y documentado; en paper el terminus NO firma (default-deny §34) |
| 11 | Envío | ¿RTT del POST de submission? | timestamps propios + `ss -ti` | dentro del slice; conexión reutilizada |
| 12 | E2E | ¿p99 del pipeline compuesto? | hdrhistogram por pipeline id (t0→t5) | presupuesto §16.1 cumplido en ventana de 1 h |

Fila 10 es deliberada sobre el terminus: en `PAPER_SHADOW` no hay firma ni broadcast — lo que se audita es el COSTO conocido de la pieza para el día que los gates lo habiliten, no su ejecución. Cualquier uso ofensivo de ventaja de latencia (frontrun/sandwich contra terceros) está fuera de alcance y es veto de `arbx-mev-ethics-gate`.

## GOBERNANZA

Todo lo anterior es ingeniería del hot path mode-invariant, subordinada a los gates `arbx-paper-trade-first`, `arbx-simulation-mandatory`, `arbx-risk-limits-enforcement` y `arbx-pre-execute-checklist`, y a CLAUDE.md §34: LIVE_MAINNET es gated, el terminus `relays-client` es default-deny y NADA de esta referencia autoriza flip a live, firma o broadcast con capital real. Los vectores ofensivos de latencia se tratan solo como riesgos a detectar/mitigar bajo `arbx-mev-ethics-gate`.
