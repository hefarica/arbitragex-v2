# 21. DESBLOQUEAR CUALQUIER APP: METODOLOGÍA CAUSA-RAÍZ BAJO PRESIÓN

CUÁNDO CARGAR ESTA REFERENCIA: cualquier bug, hang o regresión que resista 15 minutos de intuición; build roto que no obedece a cambios obvios; boot que cuelga sin error; datos vacíos en el dashboard; perf degradada sin causa visible; tests flaky; "esto ayer funcionaba"; y especialmente ANTES de caer en shotgunning (5 cambios a la vez) o de reiniciar algo sin entender qué se arregló.

| Concepto | Herramienta/Patrón | Nota clave |
|---|---|---|
| Orden sagrado | observar → hipotetizar → predecir → experimentar → registrar | Saltarse un paso = disparar a ciegas |
| Protocolo hipótesis-driven | bitácora de experimentos, 1 variable por corrida | Predicción falsable ANTES de ejecutar |
| Bisección de causalidad | `git bisect run`, `cargo tree -d`, desactivar la mitad de la config | log2(n) experimentos en vez de n |
| Triage por síntoma | árboles de §21.4: build / boot / vacíos / hang / perf / flaky | El síntoma selecciona el árbol, no la intuición |
| Forense de logs | ventana de retención (R9/LOGFLOOD-01), correlation IDs, `--since/--until` | Rotación de logs = ausencia APARENTE |
| Red por eslabones | `dig` → `openssl s_client` → `curl -v`/`-w` → preflight CORS → `ping -M do` | Un eslabón por experimento |
| PostgreSQL | `EXPLAIN (ANALYZE, BUFFERS)`, `pg_blocking_pids()`, `lock_timeout` | Nunca un índice a ciegas sin leer el plan |
| Dependencias | lockfile como única verdad, `cargo update --precise`, pin de MSRV | Vendoring es el ÚLTIMO recurso |
| Contenedores | `--env-file` explícito, NEXT_PUBLIC horneado, healthchecks, uid mismatch | Reglas R3/R4/R6 del repo son la ley |
| Escalera de desbloqueo >1h | 8 peldaños §21.10, en orden, sin saltarse ninguno | El peldaño 8 (workaround) se documenta como deuda |
| Anti-patrones | shotgunning, cargo-cult, catch vacío, reiniciar sin entender | §21.11: reconocerlos en uno mismo |
| Cierre de ciclo | mini post-mortem + test de regresión + registro | El fix sin test de regresión ES deuda |

## 21.1 Principio 0 y el orden sagrado

Tres prohibiciones previas que gobiernan todo lo demás:

1. **Reproducir antes de tocar.** Si no puedes reproducir el fallo a demanda, no puedes verificar que lo arreglaste. Un bug "a veces" se convierte primero en bug "siempre" (o en bug "cuando hago X"), y después se ataca.
2. **Hipótesis antes de fix.** Todo fix sin hipótesis es cargo-cult con esteroides: si "funciona", no sabrás por qué, y volverá.
3. **Medir antes de teorizar.** Perf degradada se perfila, no se filosofa. Un flamegraph de 30 segundos vale más que una hora de conjeturas sobre allocs.

```
┌──────────┐    ┌──────────────┐    ┌─────────────┐    ┌───────────────┐    ┌───────────┐
│ OBSERVAR │───▶│ HIPOTETIZAR  │───▶│  PREDECIR   │───▶│ EXPERIMENTAR  │───▶│ REGISTRAR │
│ (repro + │    │ (lista H1..Hn│    │ ("si H vale,│    │ (UNA variable │    │ (bitácora │
│  evidencia)│   │  ordenadas)  │    │  veré Y")   │    │  por corrida) │    │  + vered.)│
└──────────┘    └──────────────┘    └─────────────┘    └───────────────┘    └───────────┘
      ▲                                                          │
      └───────────────── el resultado refuta → nueva ronda ◀──────┘
```

El ciclo se repite hasta que una hipótesis sobrevive a una predicción falsable que LA diferencia de sus rivales. "Probé X y ahora va" no es un veredicto: sin predicción previa, cualquier mejora post hoc es anecdótica (y los bugs flaky te lo cobrarán).

## 21.2 Protocolo hipótesis-driven

**Paso 1 — Enunciar el síntoma en una frase observable:** "el dashboard `/opportunities` devuelve array vacío aunque el searcher detecta" (no: "el frontend no carga").

**Paso 2 — Enumerar hipótesis H1..Hn y ordenarlas por plausibilidad ÷ coste de test.** Desempate: gana el experimento que discrimina entre MÚLTIPLES hipótesis a la vez (máxima información por corrida). Ejemplo para datos vacíos: H1 falta `DATABASE_URL` en el productor (coste: 1 `docker inspect`), H2 query del api-server filtra todo (coste: 1 curl), H3 retención de logs rotada engaña (R9), H4 el productor realmente no emite (coste alto: repro de ciclo completo).

**Paso 3 — Cada experimento cambia exactamente UNA variable.** Si cambias flag + versión + config y "ya va", aprendiste cero y probablemente sembraste dos bugs nuevos.

**Paso 4 — Predicción falsable, escrita ANTES de ejecutar:** "si H1 es cierta, `docker inspect searcher-rs` no tendrá `DATABASE_URL` y además veré `db.connected` ausente en sus logs de arranque". Ambas condiciones, la positiva y la negativa: qué verías si H1 es FALSA.

**Paso 5 — Bitácora de experimentos.** Formato mínimo:

```
| # | Hipótesis            | Experimento (1 var)         | Predicción si H es cierta        | Resultado | Veredicto |
|---|----------------------|-----------------------------|----------------------------------|-----------|-----------|
| 1 | falta DATABASE_URL   | docker inspect searcher-rs  | env sin DATABASE_URL, sin db.connected | env OK, db.connected SÍ | H1 refutada |
| 2 | query API filtra todo| curl localhost:8787/api/opportunities/live | body vacío, status 200        | body vacío | H2 confirmada → bisect del query |
```

La bitácora es la defensa contra el amnesia-debugging ("¿ya probamos desactivar el cache?") y contra el auto-engaño post hoc. En sesiones multi-agente del repo, además es evidencia reproducible (§34.5: artefactos, no afirmaciones).

## 21.3 Bisección de causalidad

**git bisect** — cuando "ayer funcionaba". Delimita good/bad conocidos y deja que la bisección haga log2(n) builds:

```
git bisect start
git bisect bad <sha-roto-o-HEAD>
git bisect good <sha-que-funcionaba>
git bisect run ./scripts/bisect-check.sh   # exit 0=good, 1..124=bad, 125=skip
git bisect reset
```

El script de chequeo debe ser determinista; si el síntoma es flaky, devuelve 125 (`git bisect skip`) en vez de mentir. El commit culpable obtenido no siempre es la causa: lee su diff como generador de hipótesis nuevas.

**Dependency bisect** — cuando el fallo llegó "solo" tras `cargo update` o `npm install`:

- Rust: `git diff Cargo.lock` para ver qué subió; rebajar uno a uno con `cargo update -p <crate> --precise <versión>` (el lockfile es la única verdad: nunca edites versiones a mano en `Cargo.toml` para "probar").
- Node: `git diff package-lock.json`; rebajar con `npm install <pkg>@<versión-exacta>`; `npm ls <pkg>` para ver quién arrastra cada versión.

**Binary search de configuración** — desactiva la MITAD de los módulos/flags/middleware y reproducir; si falla, la causa está en la mitad activa; si no, en la desactivada. En el repo esto es especialmente potente sobre cartuchos/operadores: la Master Matrix es mode-invariant (§34.1), así que la bisección se hace en shadow sin riesgo. Aplica también a features: `cargo tree -f "{p} {f}"` muestra qué feature-set efectivo compila cada crate, y `cargo tree -d` revela versiones duplicadas que explican traits "del mismo tipo pero incompatibles".

**Reducción progresiva del repro** — recorta la entrada hasta el mínimo que aún falla: menos hops en la ruta, un solo opportunity_id, un solo bloque. Para pipelines del repo, el patrón carrier del núcleo (§1.3: clave `arbx:validated_plan:{id}` en Redis) permite reinyectar exactamente el plan que falló; el replay sobre Anvil fork (núcleo §9.2) convierte "falla a veces en prod" en "falla siempre en mi laptop contra el bloque N".

## 21.4 Árboles de triage por clase de síntoma

El síntoma elige el árbol. Cada nivel tiene herramienta concreta. Nota de alcance: si el diagnóstico destapa vectores ofensivos de terceros (sandwich/frontrun observados en mempool), se tratan SOLO como riesgo a detectar y mitigar — ver skill `arbx-mev-ethics-gate`.

### 21.4.1 Build rompe — toolchain/lockfile PRIMERO, tipos después

```
build rompe
├─ ¿Cambió toolchain o lockfile?  → git status; git diff Cargo.lock / package-lock.json
│    └─ repro en HEAD~1: si HEAD~1 también falla, NO es tu commit (es entorno/caches)
├─ ¿Conflicto de features/versiones? → cargo tree -d; cargo tree -i -p <crate>; cargo tree -f "{p} {f}"
├─ ¿Tipos de TU código? → leer el PRIMER error del compilador, no el último
│    (los errores E0308/E0277 en cascada son ecos del primero; arréglalo y recompila)
└─ ¿Entorno nativo? → linker/permisos: en este repo, Windows AppControl (os error 4551)
     bloquea ejecutables release con target/ frío → compilar en el árbol principal con target/ caliente
```

Orden no negociable: descartar toolchain/lockfile ANTES de leer 40 líneas de errores de tipos. Medio planeta ha "arreglado" un error de tipos que era un `rustup` desalineado contra `rust-toolchain.toml`.

### 21.4.2 Boot cuelga — ¿en QUÉ syscall espera?

```
boot cuelga (sin crash, sin log)
├─ strace -f -p <PID> -e trace=%network,openat,futex
│    └─ la última syscall repetida dice qué espera: connect() a IP:puerto, openat() de ruta, futex() de un lock
├─ ¿DNS bloqueante? → en el strace, sendto/recvfrom hacia :53 sin respuesta;
│    discriminar en 10s: llamar al endpoint por IP literal; si arranca, es DNS/resolver
│    (getaddrinfo síncrono congela TODO el runtime async — un solo resolve bloqueante cuelga el proceso)
├─ ¿Dependencia de servicio? → docker compose ps; ¿postgres/redis están healthy?
│    arranque circular A-espera-B-espera-A → R6: depends_on con condition: service_healthy
├─ ¿Deadlock de arranque? → dump de TODOS los hilos (§21.4.4)
└─ ¿Timeout global ausente? → envolver cada await externo en tokio::time::timeout(dur, fut);
     un boot sin timeouts convierte "lento" en "colgado" indefinidamente
```

`lsof -p <PID>` y `ss -tnp` complementan: muestran a qué socket remoto está pegado el proceso.

### 21.4.3 Datos vacíos — trazado E2E productor → bus → DB → API → UI

Es la regla R7 del repo, ejecutada como cadena de custody: la PRIMERA etapa vacía después de una etapa llena es tu frontera de fallo.

```
[watcher/searcher] ──▶ [Redis stream] ──▶ [PostgreSQL] ──▶ [api/edge] ──▶ [UI]
   grep logs            XLEN                SELECT             curl          browser
```

```
1. ¿Detecta?     docker logs searcher-rs --tail 200 2>&1 | grep -i 'simulator.success'
2. ¿Redis tiene? docker exec redis redis-cli XLEN arbx:opps:detected
3. ¿PG tiene?    docker exec postgres psql -U postgres -d arbitragex \
                    -c 'SELECT MAX(detected_at) FROM opportunities;'
4. ¿API sirve?   curl -s localhost:8787/api/opportunities/live | head
```

Lectura de fronteras: Redis lleno + PG vacío → falta `DATABASE_URL` en el productor (R6). PG lleno + API vacía → query/filtro del api-server. API llena + UI vacía → edge/proxy/frontend. Y el vacío que SOLO ve la app (psql manual devuelve filas) → permisos o filtro silencioso: grants/RLS del rol de conexión (`SELECT` faltante, política que filtra todo), `search_path` equivocada, WHERE/soft-delete implícito del ORM — confírmalo ejecutando el query crudo como el rol real de la app. Antes de concluir "nunca ocurrió": ventana de retención R9 (`docker inspect <c> --format '{{.HostConfig.LogConfig.Config}}'` y comparar `State.StartedAt` contra el timestamp de la primera línea retenida) — la rotación fabrica ausencias (LOGFLOOD-01, `docs/incidents/2026-08-15-LOGFLOOD-01.md`). Y separa SIEMPRE vacío honesto (R8: observation con razón exacta `discovery_no_pool_found`, `watchlist_empty`…) de vacío del pipeline (evento perdido): el primero es respuesta correcta del sistema, el segundo es un bug. Cuidado con contadores: `grep -vc` cuenta líneas NO coincidentes (incluye vacías) y miente en cuerpos multilínea.

### 21.4.4 Hang / deadlock — el stack de TODOS los hilos

```
hang/deadlock
├─ Dump completo de stacks: gdb -batch -ex "thread apply all bt" -p <PID>   (o rust-gdb)
│    └─ buscar dos hilos donde cada uno espera el recurso que el otro sostiene (lock ordering)
├─ ¿Tasks tokio, no hilos OS? → tokio-console (crate console-subscriber + RUSTFLAGS="--cfg tokio_unstable";
│    `cargo install tokio-console`): muestra tareas vivas, en qué wake/spawn esperan y su edad
├─ ¿Esperas sin timeout? → auditar TODOS los .lock()/recv()/await de la ruta: cada espera externa
│    necesita timeout o cancelación; "esperar para siempre" no es política, es bug latente
└─ Servicios Node (api/edge): node --inspect + Chrome DevTools (chrome://inspect) →
     capturar un CPU profile en el panel Performance: muestra el stack JS congelado
     exacto (también detecta event loop bloqueado por CPU)
```

Realidad incómoda: ni tokio ni parking_lot detectan deadlocks de lógica por ti (a diferencia de la JVM con `jstack`); el dump de stacks es tu único testigo. Un `strace -f -p <PID> -c` (tabla de syscalls por hilo) confirma rápido si alguien gira en futex sin avanzar.

### 21.4.5 Perf degradada — perfil PRIMERO, teoría después

```
perf degradada
├─ Perfil CPU: cargo flamegraph --bin <bin>   (Linux/VPS; perf + cargo-flamegraph)
│    └─ ¿torre en lock/mutex? → contention · ¿mar de allocs? → churn · ¿poll de red dominante? → I/O síncrono
├─ ¿N+1 de base de datos? → pg_stat_statements: queries con calls altísimo y mean_exec_time bajo,
│    ejecutadas en loop → IN_/JOIN o batching (ver §21.7)
├─ ¿Event loop bloqueado? → CPU-bound o I/O síncrono dentro de async → tokio::task::spawn_blocking
├─ ¿Alloc churn en hot-path? → crate dhat (heap profiling en proceso) o jemallocator como
│    #[global_allocator] comparando antes/después
└─ ¿Contención de locks? → flamegraph lo señala; reducir sección crítica, sharding, o paso de mensajes (núcleo §3.1)
```

Regla de oro: cualquier afirmación de perf ("es el GC", "son los locks") exige un artefacto — flamegraph, métrica `arbx_simulation_duration_seconds` (núcleo §5.1), o números before/after. Sin artefacto, es opinión.

### 21.4.6 Tests flaky — determinismo o cuarentena

```
test flaky
├─ ¿Azar/semillas? → proptest persiste seeds en archivos proptest-regressions: reinyecta la seed culpable
├─ ¿Reloj real? → fakear el tiempo: #[tokio::test(start_paused = true)] arranca el reloj
│    en pausa y auto-avanza de forma determinista al deadline de cada sleep
│    (requiere feature test-util de tokio); NUNCA sleeps reales en tests
├─ ¿Orden/paralelismo? → repro en serie: cargo test -- --test-threads=1 (o RUST_TEST_THREADS=1);
│    vitest: sequence.shuffle true en config para CAZAR orden-dependencia, no solo sufrirla
├─ ¿Recursos compartidos? → puertos fijos colisionando, misma DB/tabla, tmp dir compartido, cache de CI tibio
└─ ¿Persiste sin causa? → CUARENTENA con issue abierto: moverlo a suite/cfg aparte (exclusión explícita
     y visible en la config), jamás @skip silencioso — un skip sin issue es un bug con VPN
```

El flaky no arreglado es un detector de carrera real que decidiste ignorar; con suerte solo rompe CI, sin suerte rompe el hot-path en producción primero.

## 21.5 Forense de logs

- **Verbosidad correcta:** `RUST_LOG=info,arbx_sim_ctl=debug,hyper=warn` — sube SOLO el crate investigado. Regla R9 para hot-loops: per-ítem a `debug!` + UN summary `info!` con histograma de razones (R8); un loop honesto que emite 183 líneas/s destruye la observabilidad de todo el sistema.
- **Ventana de retención (R9):** antes de afirmar "el evento X nunca ocurrió", verifica cuánto retiene el contenedor (`docker inspect <c> --format '{{.HostConfig.LogConfig.Config}}'`) y compara `State.StartedAt` contra el timestamp de la primera línea retenida. Brecha = ventana rotada = tu "nunca" es un artefacto.
- **Correlation IDs:** toda línea de investigación debe poder grepparse por el identificador del entity en flight — `opportunity_id` en spans `#[instrument]` (núcleo §5.2) — y seguirlo a través de servicios: `docker logs searcher-rs 2>&1 | grep '<uuid>'` → mismo uuid en logs del consumidor → misma key en Redis (`arbx:validated_plan:{id}`).
- **Bisección temporal de logs:** acota el intervalo del incidente y lee SOLO esa franja: `docker logs --since 2026-09-15T10:00:00 --until 2026-09-15T10:05:00 <c>`. La pregunta correcta no es "qué errores hay" sino "qué cambió en el patrón en T": el primer `warn!` nuevo tras minutos de calma es generalmente la cabeza del hilo, no el grito más repetido.

## 21.6 Red: cadena eslabón por eslabón

Nunca "la red está mal": un eslabón concreto está mal. Cadena completa, herramienta por eslabón:

| Eslabón | Síntoma típico | Herramienta | Comando canalla |
|---|---|---|---|
| DNS | "no resuelve" aquí pero sí allá | dig | `dig +short @1.1.1.1 host` vs resolver local → stale cache del resolver (flush: `resolvectl flush-caches`) |
| TLS/SNI | cert mismatch o handshake cuelga | openssl | `openssl s_client -connect h:443 -servername h` (sin SNI el server sirve el cert por defecto) |
| TCP/ruta | conecta a veces, MTU | mtr / ping | `mtr -r <host>`; MTU: `ping -M do -s 1472 <host>` (1472+28=1500) — TLS cuelga con paquetes pequeños OK = MTU |
| Proxy | funciona en mi shell, no en el contenedor/CI | curl -v / env | `env \| grep -i proxy`; `curl --noproxy '*' url` discrimina en 5s si `HTTP_PROXY` secuestra la request |
| CORS | browser bloquea, curl funciona | preflight curl | `curl -i -X OPTIONS -H "Origin: https://app" -H "Access-Control-Request-Method: POST" <url>` — el server DEBE responder los headers ACAO; con `credentials: include` el wildcard `*` es ilegal por spec |
| Latencia total | "está lento" | curl -w | ver escalera abajo |

Escalera de timing con `curl -w` (descompone "lento" en su eslabón):

```
curl -o /dev/null -s -w 'dns=%{time_namelookup} tcp=%{time_connect} tls=%{time_appconnect} ttfb=%{time_starttransfer} total=%{time_total}\n' https://host/path
```

dns alto → resolver; tcp alto → ruta/proxy; tls alto → SNI/certificados/MTU; ttfb alto con tcp/tls OK → el backend (app) es el culpable. En VPS, recuerda la regla del repo: REST → Edge (8787), WebSocket → api-server directo (8080), nunca por Edge — un WS "lento" cruzando el edge es arquitectura, no red.

## 21.7 PostgreSQL: el plan primero, siempre

**EXPLAIN antes que cualquier teoría de índices.** Regla: nunca agregar un índice a ciegas — cada índice taxa writes y puede no usarse (el planner lo ignora si las estadísticas dicen otra cosa).

```
SET track_io_timing = on;   -- activa I/O Timings en BUFFERS (solo tu sesión)
EXPLAIN (ANALYZE, BUFFERS) SELECT ... ;
```

Qué leer en el plan, en orden: (1) **rows estimadas vs reales** — desviación grande = estadísticas viejas → `VACUUM (ANALYZE) tabla;` o el planner decide mal con datos buenos; (2) `Filter` con alta selectividad sobre muchas filas = candidato a índice AHORA sí justificado; (3) `Nested Loop` con inner `Seq Scan` repetido N veces = N+1 de la app; (4) BUFFERS `read` altos vs `hit` = falta de memoria/índice clustered. Índice nuevo → `CREATE INDEX CONCURRENTLY` (no bloquea writes; no funciona dentro de transacción).

**Locks:** la memoria del repo tiene el precedente (FREEZE-01): un DELETE masivo sin `lock_timeout` congeló el pipeline. Diagnóstico en vivo:

```sql
SELECT pid, wait_event_type, wait_event, state, left(query, 80)
FROM pg_stat_activity
WHERE cardinality(pg_blocking_pids(pid)) > 0 OR wait_event_type = 'Lock';
```

Prevención: `SET lock_timeout = '3s';` (`statement_timeout` para acotar lo propio) en toda sesión operativa del repo; session-level, no global a ciegas.

**Pool exhaustion:** síntoma canónico `FATAL: sorry, too many clients already`. `SELECT count(*), state FROM pg_stat_activity GROUP BY state;` — montaña de `idle in transaction` = código que abre transacción y no cierra (bug del ORM/cliente), no falta de `max_connections`. VACUUM FULL solo en ventana de mantenimiento (toma lock exclusivo de tabla; el precedente de pacing de CHECKPOINT del repo aplica a cualquier operación masiva).

**Slow log:** `ALTER SYSTEM SET log_min_duration_statement = 250; SELECT pg_reload_conf();` + `pg_stat_statements` (requiere `shared_preload_libraries`) para el ranking de coste acumulado por query — ataca primero la query con más tiempo TOTAL (calls × mean), no la más lenta individual.

## 21.8 Dependencias: el lockfile es la única verdad

- **Verdad única:** `Cargo.lock` / `package-lock.json` commiteados. Reproducibilidad = el lockfile del VPS es bit-identical al de CI (`npm ci`, no `npm install`, en deploy). Cualquier "en mi máquina compila" se responde con el hash del lockfile.
- **Duplicados y conflictos:** `cargo tree -d` (versiones duplicadas), `cargo tree -i -p <crate>` (quién arrastra qué), `npm ls <pkg>` / `npm dedupe`; en npm, el campo `overrides` de package.json fuerza una versión cuando dos dependencias divergen.
- **Pin de toolchain/MSRV:** `rust-version = "..."` en `Cargo.toml` + `rust-toolchain.toml` con `channel = "..."` — el "no compila" post-rustup muere aquí. Verificación MSRV sistemática: la herramienta comunitaria `cargo-msrv`.
- **Vendoring = último recurso:** `cargo vendor` (imprime el bloque `[source]` para `.cargo/config.toml`) solo cuando el entorno no puede resolver red; y en este repo, TODO porte de código externo pasa por `docs/security/FUSILE_SOURCE_POLICY.md` (port-with-validation, no copy ciego).

## 21.9 Contenedores: las cinco traiciones clásicas

1. **Invalidación de cache:** el orden de layers es la cache policy — copia lockfiles ANTES del fuente (patrón del núcleo §4.1: build de deps con `main.rs` stub, luego el código real). Cuando sospeches cache sucio: `--no-cache` Y `--progress=plain` para VER qué capa ejecutó.
2. **Propagación de env:** NEXT_PUBLIC_* se hornea en `next build` (RULE 03): cambiar `.env` y hacer `restart` NO aplica nada. Secuencia obligatoria del repo: `docker compose --env-file .env -f docker/compose.dev.yml build --no-cache frontend` + `up -d frontend`; verificar con `curl -I http://127.0.0.1:5173/opportunities` (si la CSP contiene `localhost`, la regla fue violada — RULE 04). Y sin `--env-file` explícito, compose NO lee el `.env` de la raíz: todo cae al fallback.
3. **entrypoint vs cmd:** `docker run imagen <args>` SOBREESCRIBE `CMD` pero se concatena a `ENTRYPOINT`; para entrar a depurar: `docker run --rm -it --entrypoint sh <imagen>` (la imagen "no arranca" puede ser un CMD incorrecto, no un binario roto).
4. **Healthchecks y orden de arranque:** patrón R6 — `healthcheck: test: ["CMD-SHELL", "pg_isready -U postgres"]` + `depends_on: { postgres: { condition: service_healthy } }`; todo productor de datos del dashboard debe loguear `db.connected` verificable.
5. **Permisos y red:** uid mismatch (bind mount propiedad de root, proceso como uid 999 del postgres oficial → EACCES); alias de red: dentro de la red de compose los servicios se resuelven por nombre/alias — `docker compose exec <svc> getent hosts postgres` antes de culpar al DNS externo.

## 21.10 Escalera de desbloqueo (>1h atascado)

Cuando llevas más de una hora, la intuición ya entregó todo lo que tenía. Sube la escalera EN ORDEN, un peldaño por vez:

| # | Peldaño | Acción concreta |
|---|---|---|
| 1 | Doc exacta de la versión EN USO | `docs.rs/<crate>/<versión-exacta>/` (no la latest); `npm view <pkg> version` vs tu lockfile. La mitad de los "bugs" son firmas de otra versión. |
| 2 | Repro mínimo aislado | el caso más pequeño que falla, fuera del sistema (binario de 30 líneas o script). Si NO falla aislado, la causa es la interacción — cambia el árbol de triage. |
| 3 | Diferencial contra baseline | algo funcional comparable: `git diff <good>..<bad> --stat`, `diff` de configs, diff de `docker inspect` env. Pregunta: ¿QUÉ cambia entre ambos? |
| 4 | Bisect | §21.3: git bisect run / dependency bisect / config halves |
| 5 | Instrumentar | agregar el log/métrica/trace que falta en el punto ciego (núcleo §5). Si no puedes verlo, no puedes debuggearlo. |
| 6 | Leer el fuente de la dependencia | `~/.cargo/registry/src/...` (o `node_modules/<pkg>/src`). Leer el código real mata las suposiciones sobre "qué hace la lib". |
| 7 | Búsqueda del mensaje EXACTO | el error VERBATIM (con puntuación): `gh search issues "..." --repo owner/repo`, `gh search code "..."`, issues del upstream. Alguien ya pagó este debugging. |
| 8 | Workaround documentado como DEUDA | si el upstream está roto: bypass mínimo, comentario con link al issue, entrada en la bitácora como DEUDA EXPLÍCITA. Jamás hack silencioso. |

## 21.11 Anti-patrones (reconocerlos en uno mismo)

| Anti-patrón | Por qué fracasa | Corrección |
|---|---|---|
| Shotgunning (5 cambios a la vez) | Si "ya va", no sabes cuál lo arregló ni qué rompiste | 1 variable por experimento (§21.2) |
| Cargo-cult fix ("copié el fix del otro servicio") | Copia el ritual sin la causa; síntomas distintos, causa distinta | Hipótesis propia + predicción falsable |
| "A mí me funciona" | Tu máquina ≠ VPS: env horneado, proxy, DNS, uid, versiones (§21.9) | Diferencial contra baseline, peldaño 3 |
| Reiniciar sin entender | El reinicio enmascara; el estado corrompido/regresará | Si reiniciar arregló, pregunta POR QUÉ: leak de memoria, estado pegado, conexión zombi — y arregla eso |
| Silenciar el error (catch vacío / `.ok()`) | Convierte fallo en datos faltantes; rompe Fail-Honest (R8) | El error se propaga o se registra con razón exacta |
| Arreglar el síntoma | El síntoma reaparece por otra vía; la causa queda viva | 5-why hasta la causa raíz + test de regresión |

## 21.12 Cierre de ciclo: el bug termina cuando dejó de ser posible

Todo bug que consumió >1h cierra con tres artefactos, sin excepción:

1. **Mini post-mortem** — en `docs/incidents/YYYY-MM-DD-<NOMBRE>.md` (convención del repo; LOGFLOOD-01 es el ejemplo canónico): síntoma, cadena causal, causa raíz, fix, prevención. Media página basta; lo caro no es escribirlo, es re-aprender el bug dentro de seis meses.
2. **Test de regresión** — falla sin el fix, pasa con él: `test('regression #<issue>: <síntoma>')` en vitest, `mod regression_<issue>` en Rust. El fix sin test de regresión ES deuda: nada impide que un merge futuro lo revierta sin que nadie lo note.
3. **Registro de aprendizaje** — la lección destilada a una línea reusable en el ledger del proyecto (MEMORY/LEARNINGS según convención vigente): gotcha + herramienta + condición de reaparición.

El sistema que no cierra ciclos no acumula experiencia; acumula reincidencia.

## GOBERNANZA

Todo lo anterior opera bajo modo shadow/paper y es metodología de diagnóstico: está subordinado a los gates `arbx-*` (paper-trade-first, simulation-mandatory, risk-limits-enforcement, pre-execute-checklist) y a CLAUDE.md §34 — LIVE_MAINNET sigue gated con default-deny en el terminus de ejecución. Ningún "fix urgente" justifica bypassar un gate, firmar, o broadcast con capital real; la emergencia se resuelve con revert + gate nuevo (§37), no con excepciones.
