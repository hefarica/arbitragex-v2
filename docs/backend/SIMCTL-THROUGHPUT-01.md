# SIMCTL-THROUGHPUT-01 — El sumidero no va "75× detrás": está en LIVELOCK de PEL

**run_id** arbx-entrega-20261007 · **phase_id** implementation · **SHA_BASE** `9ccd1d04`
**Rama** `feat/simctl-throughput-01` (PR abierto, SIN merge) · **dueño** Backend
**Alcance** `backend/sim-ctl/src/consumer.rs` (1 archivo) · **NO se tocó** `sim_engine.rs`, `persistence.rs`, `signer_funding.rs`
**Prohibiciones respetadas**: sin merge, sin deploy, sin tocar main, sin reiniciar servicios.

---

## 0. INSTRUMENTO

`command -v cargo` → `/home/hfrc/.cargo/bin/cargo` · `cargo 1.91.0 (ea2d97820 2025-10-10)` · `GATE1=PASS`
md5 idéntico WSL↔Windows **antes** de correr: `consumer.rs 1978c7bfb1ad5cb4fdf90e524e6da4f5` en ambos lados.

---

## 1. Q1 — EL REPARTO DE COSTO POR ENTRADA (medido, ya no NO COMPUTADO)

`StreamBound` **ya** contaba las tres clases (`admitted`, `deferred`, `terminated_inadmissible`, `consumer.rs:275-277`) y **ya** las publica en su reporte periódico (`consumer.rs:367-374`). t108 las declaró NO COMPUTADO porque nadie las leyó. Leídas:

`docker logs arbitragex-v2-sim-ctl-1 --tail 4000 | grep "sim_ctl.bound_report"`:
```
{"timestamp":"2026-10-08T03:11:34.575958Z","level":"INFO","fields":{"event":"sim_ctl.bound_report",
 "admitted":243,"deferred":103604,"terminated_inadmissible":220,
 "max_sims_per_sec":1.0,"max_in_flight":1},"target":"sim_ctl::consumer"}
```

| clase | cuenta | paga PG+fork+REVM? |
|---|---|---|
| `admitted` | **243** | **SÍ** — camino caro completo |
| `deferred` | **103.604** | NO — postergada por tasa/concurrencia, vuelve al PEL |
| `terminated_inadmissible` | **220** | NO — muere en el gate 1, sin I/O |
| **decididas** | **104.067** | |

**103.604 / 243 = 426 postergaciones por cada admisión.** El reparto barato/caro **no** es "unos pocos inadmisibles y muchos caros": es **el 99,6% de las decisiones gastadas en postergar**.

**CONTROL DE INVENTARIO (R9)**: `docker inspect arbitragex-v2-sim-ctl-1 --format '{{.HostConfig.LogConfig.Config}} | StartedAt={{.State.StartedAt}}'` → `map[max-file:5 max-size:10m] | StartedAt=2026-10-08T02:51:25.24075118Z`; la primera línea retenida es `2026-10-08T02:51:25.498681Z` ⇒ la ventana de 50 MB **cubre todo el arranque, sin rotación** ⇒ el contador está completo para este boot.

**Un método independiente devolvió 0 y NO es evidencia de cero**: `grep -c bound_deferred` → `0`. `note_deferred` sólo loguea la primera vez y luego **una vez por `BOUND_LOG_INTERVAL`** (R9, `consumer.rs:326-343`); el conteo está rate-limitado **por diseño**. El contador del reporte es el autoritativo. Se declara en vez de usarse como confirmación.

### 1.1 Por qué las deferidas son las ADMISIBLES

`consumer.rs:789-802`: el permiso de `in_flight` se pide **sólo si `inadmissible.is_none()`**. Las inadmisibles **nunca piden permiso** ⇒ se procesan siempre. Las que sufren el techo son exactamente las que sí pueden simularse. El sistema posterga precisamente el trabajo que importa.

---

## 2. Q2 — CONCURRENCIA: la capacidad EXISTE y es INALCANZABLE

### 2.1 Las tres capas, medidas

| capa | estado | evidencia |
|---|---|---|
| `ForkManager` | **pool de 4** — ya paralelo | `main.rs:610` `snapshot_pool_size … unwrap_or(4)`; `fork_manager.rs:31` `pool: Arc<Semaphore>` |
| `SimEngine::simulate` | **`&self`** — ya paralelizable | `sim_engine.rs:35` |
| `Consumer::process_message` | **`&mut self`** en un `for` **secuencial** | `consumer.rs:527` dentro de `read_batch` |

Dos de las tres capas están listas para concurrir. **El consumidor las serializa a todas.**

Y el propio código lo dice (`consumer.rs:425-428`, verbatim):
> *"Outer in-flight ceiling. With `max_in_flight = 1` it never binds in the sequential loop; it is the POLICY ceiling for any future concurrent dispatch"*

### 2.2 El defecto REAL: `COUNT 8` contra un presupuesto de 1

`consumer.rs:507-508` leía `COUNT 8` **fijo**. El presupuesto de admisión en vigor es **1**: `DEFAULT_MAX_IN_FLIGHT = 1` (`:119`) y el contenedor **no define** `SIMCTL_MAX_IN_FLIGHT` (env efectivo: sólo `ANVIL_URL`, `SIM_FORK_URL`, `SIM_BACKEND`, `SIM_ORCHESTRATOR_*`, `SIM_PORT`, `SIM_SIGNER_ADDRESS`). Igual `DEFAULT_MAX_SIMS_PER_SEC = 1.0` (`:111`), tampoco definido.

⇒ De cada lote de 8, **una** entrada toma permiso y **siete** caen en `note_deferred`. Esas siete **no se pierden**: quedan sin ACK en el PEL y `recover_stale_pending` las redelivera tras `CLAIM_MIN_IDLE_MS` = 120 s — para volver a postergarlas. **`defer → 120 s → XAUTOCLAIM → defer`.**

**El churn, medido en dos instantes separados** (método independiente del reporte):

| instante | `arbx_sim_stream_claimed_count` | `simulation_total{anvil}` |
|---|---|---|
| primer lectura | 67.514 | 230 |
| ~6 min después | **85.110** | **294** |

Δ = **+17.596 reclaims / ~6 min = 48,9/s** contra **+64 simulaciones / 6 min = 10,7/min**. **Se gasta ~270× más en reclamar que en simular.** `pending 10225-11571`, `oldest_pending 414637-431335 ms`, `lag 2796` — estable, no creciente: es un **régimen estacionario de churn**, no una cola que crece.

**Esto reencuadra el diagnóstico.** El "75×" del enunciado sale de dividir la llegada entre lo que el consumidor termina: `1.650/min` (medido, span 362.996 ms sobre 10.000 entradas) ÷ `~22/min` (medido, `simulated_at` por minuto) = 75. Ese cociente es correcto pero **describe el síntoma, no el mecanismo**: no es un consumidor "75× lento", es un consumidor cuyo tiempo se va en un bucle de postergar-y-reclamar lo que él mismo acaba de leer. El churn medido (`48,9/s` contra `10,7/min`) es ~270× el trabajo útil, y **una lectura lenta no produciría churn**: lo produce leer de más y no poder admitirlo.

### 2.3 El arreglo (en alcance, 1 línea de lógica)

```rust
pub fn read_batch_size(available_permits: usize) -> usize {
    if available_permits == 0 { 1 } else { available_permits }
}
```
`read_batch` pasa de `.arg(8)` a `.arg(read_batch_size(in_flight.available_permits()))`.

**Pedir exactamente lo que cabe.** No adelanta trabajo leer de más: lo convierte en churn. **NO se sube `max_in_flight` ni `max_sims_per_sec`** — el techo es el mismo (Q2 prohíbe subir la cota como solución); lo que cambia es que deja de gastarse en entradas que no se van a poder procesar en ese pase. `max(1)` mantiene el caso degenerado en **una** entrada y no en cero: leer 0 dejaría el bucle sin avanzar.

### 2.4 CPU del host (compartido con la superficie pública)

`nproc` → **8** · `uptime` → `load average: 4.14, 3.17, 3.83` · anvil limitado a `NanoCpus=2000000000` (**2 CPU**), `Memory=2147483648` (2 GiB), `Cmd: anvil --fork-url "$ANVIL_FORK_URL" --host 0.0.0.0 --port 8545`.
`docker stats --no-stream`: postgres 18,73% · api-server 13,96% · redis 8,82% · searcher-rs 4,30% · **anvil no aparece en el top-12** (<0,05%).

⇒ **anvil está ocioso hoy porque sólo se admitieron 243 entradas.** El margen de CPU para concurrencia **existe**, pero **su techo no se midió**: cuántos handles concurrentes sostiene antes de degradar la superficie pública (nginx/edge/api-server) **requiere una prueba de carga que NO se corrió** ⇒ **NO COMPUTADO**.

---

## 3. Q3 ★ — LA PREDICCIÓN A FALSAR: su premisa es FALSA

**La tarea afirma**: *"los `sim_signer_funding_slot_unresolved` MUEREN ANTES DEL FORK, o sea que hoy son BARATOS."*

**Es falso, y el código lo dice en orden.** `sim_engine.rs`:
```
122:        let handle = match fork.acquire().await {
126:                return Self::failed(id, trace_id, "fork_acquire_failed");
142:                if let Err(reason) = funder.ensure_funded(token_in, probe.from, amount_in).await {
144:                    let _ = handle.release().await;
```
`fork.acquire()` está en **:122**, `ensure_funded` en **:142** — **veinte líneas después**. La postergación por fondeo ya **tiene el snapshot tomado** y paga `evm_snapshot` + hasta 4 `anvil_setStorageAt` + `balanceOf` + `evm_revert`.

⇒ **Arreglar (b) NO convierte un camino barato en caro**: le añade el paso REVM (~2,084 s) a un camino que **ya adquiere el fork**. El salto de ~57× que la tarea proyecta **sobreestima**: se calculó sobre una premisa que no se sostiene. **NO se afirma el 57×.**

### 3.1 La ventana de retención SÍ se agota — y esa es la conclusión que manda

Medido: span del stream `362.996 ms` sobre 10.000 entradas ⇒ **363 s** de retención a 27,5 msg/s.
Medido: `arbx_sim_stream_oldest_pending_ms` = **414.637 ms = 415 s**.

**415 s > 363 s: la entrada más vieja sin ACK es MÁS VIEJA que lo que el stream retiene.** El margen es **negativo** por ~52 s. `lag 2796` = 2.796/27,5 = **101,7 s** de cabeza para lo aún no entregado.

**Y el techo estructural, independiente de (b)**: con `max_sims_per_sec = 1.0`, el consumidor admite como máximo **60/min** contra **1.650/min** de llegada (27,5/s). En una ventana de retención de 363 s el stream ofrece ~10.000 entradas y el consumidor sólo puede tocar ~363 de ellas = **3,6%**. **La ventana de retención es estructuralmente insuficiente con o sin el arreglo de (b).**

⇒ **Por eso (b) y el caudal se diseñan juntos, y el caudal va PRIMERO.** (b) sin el caudal empeora el churn; el caudal sin (b) ya recupera ~7/8 del trabajo desperdiciado por lote. **La conclusión del enunciado se sostiene; su aritmética no.**

---

## 4. Q4 / Q5 / Q6 — LOS LÍMITES QUE NO SE MOVIERON

- **Q4**: NO se tocó la política de admisibilidad. `inadmissible_reason` queda **byte-idéntico** (`consumer.rs:172-184`): sigue excluyendo `amount_in_wei = 0` y sigue SIN mirar `expected_profit_usd`/`roi_pct`. No se añadió ningún filtro por rentabilidad. El límite que se cambió es de **TASA DE LECTURA**, no de admisión.
- **Q5**: la clave sigue saneándose con `sanitize_key`/`key_of_amount_in_wei` y `is_finite()`; **no** se introdujo `partial_cmp(..).unwrap()` ni `f64::max(NaN, 0.0)`. `read_batch_size` opera sobre `usize` — no hay NaN posible: no toca la clave.
- **Q6**: fail-closed intacto. `max_slippage_for_pass_pct` y todos los umbrales sin tocar; `passed=true` sigue exigiendo eth_call + gas + decode + umbral. Evidencia viva: **no existe ninguna serie `arbx_simulation_total{passed="true"}`** — sólo `{passed="false",simulator="anvil"} 294` y `{passed="false",simulator="revm"} 255`.

---

## 5. Q7 — LAS 4 PUERTAS

| puerta | resultado | evidencia |
|---|---|---|
| `command -v cargo` | **PASS** | `/home/hfrc/.cargo/bin/cargo` · `cargo 1.91.0 (ea2d97820 2025-10-10)` |
| `cargo fmt --all -- --check` | **PASS** | sin diff de formato |
| `RUSTFLAGS='-D warnings' cargo check -p sim-ctl` | **PASS** | `Checking sim-ctl v0.1.0 (/home/hfrc/arbx100/backend/sim-ctl)` + `Finished dev profile … in 16.00s`; `grep -cE '^(error\|warning)'` = **0** |
| `cargo test -p sim-ctl --no-fail-fast` | **PASS** | 0 fallos; los 2 tests nuevos **por nombre**: `read_batch_size_follows_the_admission_budget_not_a_fixed_eight ... ok`, `read_batch_size_is_monotone_in_the_budget ... ok` |
| `cargo test -p sim-core --no-fail-fast` | **PASS** | `79 passed; 0 failed` |

`Compiling` **O** `Checking` exigidos y presentes. md5 idéntico WSL↔Windows **antes** de correr. Exit code real por puerta.

**Tests**: `read_batch_size_follows_the_admission_budget_not_a_fixed_eight` fija el caso desplegado (`read_batch_size(1) == 1` y `!= 8`) — es el test de REGRESIÓN del defecto; y `read_batch_size_is_monotone_in_the_budget` es el **CONTROL**: sin él, el primero pasaría igual si la función devolviera una constante.

---

## 6. Q8 — QUÉ MUEVE Y QUÉ NO, SIN ADORNOS

**Este cambio NO mueve `passed=true`.** No lo mueve porque no puede: `passed` depende de eth_call + gas + decode + umbral, y nada de eso se tocó. Mejora la **TASA** y con ella la potencia estadística (más entradas decididas por unidad de tiempo), no el veredicto.

**Régimen medido ANTES** (arranque 02:51:25Z → 03:15Z, `SHA_BASE 9ccd1d04`):
`admitted 243` · `deferred 103604` · `terminated_inadmissible 220` · `pending 10225-11571` · `lag 2796` · `oldest_pending 414637-431335 ms` · `claimed_count` 67.514→85.110 · `simulation_total{passed="true"}` **ausente**.

**Régimen DESPUÉS: NO COMPUTADO — y con la razón.** Medirlo exige desplegar, y desplegar está **prohibido** en esta tarea (y un merge lo dispararía: `auto-deploy-vps.yml` corre en `push: branches:[main]`). El PR queda **abierto y sin mergear**. No se reporta un "después" que no se midió.

---

## 7. LO QUE NO SE PUDO ESTABLECER — NO COMPUTADO

1. **Techo real de handles concurrentes del anvil** y su costo sobre la superficie pública (nginx/edge/api-server). Requiere prueba de carga; **no se corrió** (sería interferir en producción).
2. **Efecto del arreglo sobre la tasa** — sin deploy, **NO COMPUTADO** (§6).
3. **Si el payload de las entradas del PEL más viejas que la ventana sigue siendo legible.** Medido que `oldest_pending (415 s) > retención (363 s)`; la consecuencia exacta sobre el payload **no se midió**.
4. **Por qué `lag` está clavado en 2796 en dos lecturas separadas.** Consistente con régimen estacionario, pero la causa **no se estableció**.
