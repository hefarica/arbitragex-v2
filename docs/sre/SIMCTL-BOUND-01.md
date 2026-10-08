# SIMCTL-BOUND-01 — la cota de consumo de `sim-ctl`, con su derivación escrita

**Cápsula ORGDNA:** `run_id=arbx-entrega-20261007` · `phase_id=implementation` · `SHA_BASE=21d2039cc80b8c47c3eea6b223ce7e663da604fd`
**Dueño:** Data · **Permisos:** paper, sin firma, **sin merge** (el aterrizaje es de Release-B)
**In scope:** `backend/sim-ctl/src/consumer.rs`, `docs/sre/` · **Out of scope:** `backend/selector-api/`, `backend/searcher-rs/`, `backend/sim-core/src/sim_multistep.rs`, `.github/`, `docker/`
**Intento:** `7150ec32-dd7f-4bdf-8d5e-0ff76ada4ab3`

---

## 0. Qué es esto, y qué NO es (U6)

Acota el **costo** del consumo de `arbx:opps:validated` con dos puertas en el punto de entrada del consumidor.

**NO mejora ninguna métrica de negocio.** No hay una sola afirmación de rentabilidad en este documento y **está prohibido reportarlo como avance de rentabilidad**: el `passed=true` del sistema sigue dependiendo del juicio del simulador. Lo único que cambia es que el consumo deja de estar acotado por la suerte del throughput y pasa a estarlo por política declarada.

---

## 1. El riesgo con su número — y el multiplicador que **NO está computado** (U9)

**Medido** (Release-B, 2026-10-07T22:09:21Z): `simulations` creció **40.958 filas en 10 min ≈ 4.100/min ≈ 68 filas/s**, y la cadena corre **1:1** con `validated`. El cross-check es independiente y cierra: `entries-read` pasó de 12.247.832 a 12.452.860, **delta = 205.028 = exactamente `COUNT(*)` de `simulations`**. Sin tope: cada mensaje validado produce exactamente una simulación.

Hoy eso es barato **porque cada fila corta ANTES de tocar el fork**: el 100% de esas 205.028 filas es `strategy_cyclic_route_not_simulatable_in_s4`, y ese camino retorna sin adquirir snapshot ni ejecutar `eth_call`.

### ★ El multiplicador «68 filas/s → 68 simulaciones REVM/s» es **NO COMPUTADO**

La instrucción del capitán es explícita y la sostengo: convertir esa tasa en carga REVM exige **dos premisas que nadie midió**:

- **(a)** que el call site resuelve ruta para **todas** esas filas, y
- **(b)** que **todas** alcanzan el fork.

El upstream sugiere lo contrario: parte de esa masa seguirá muriendo en etapas más baratas. **Por tanto este documento no afirma la conversión**, y la cota **no se dimensionó con 68/s** (§3).

**La medición exacta que lo cerraría** (post-deploy, no antes): el **cociente entre filas que llegan al fork y el total de filas**. Observable por dos contadores que ya existen:

1. la **caída de la masa `cyclic_route_missing_route_metadata:*`** en `simulations.fail_reason` (esa familia es la que hoy se lleva el 100% de la masa que muere temprano), y
2. el **conteo de `simulated_profit_usd` no-NULL**, que sólo existe cuando la simulación llegó a ejecutar de verdad.

**La dimensión DEFINITIVA de la cota se fija con esa medición, no antes.** El valor que aterriza acá es un placeholder conservador con la derivación escrita, no una conclusión sobre carga REVM.

---

## 2. Las dos puertas, y por qué viven en el loop del consumidor

Ambas en `process_message` (`consumer.rs:771` y `:778`), **antes de cualquier I/O y antes del fork**. La cota vive en el loop del consumidor y **no atraviesa el trait `SimulatorBackend`** ni la adquisición de fork: por eso el `inScope` de esta tarea es `consumer.rs` + `docs/sre/` y los tests van **inline** como `#[cfg(test)] mod`.

### Puerta 1 — admisibilidad estructural, O(1) EN LÍNEA (U1, U2, U3, U4)

Sobre el mensaje **ya parseado**: no hay I/O, no hay lectura hacia adelante, no hay ordenamiento. Una entrada que no puede simularse jamás no paga un round-trip a PG ni un snapshot de fork.

| símbolo | línea | qué garantiza |
|---|---|---|
| `sanitize_key` | `:137` | `if x.is_finite() { x } else { NEG_INFINITY }` — **nunca un `0.0` fabricado**, nunca un `NaN` |
| `key_of_amount_in_wei` | `:148` | la clave (`amount_in_wei`, string decimal) saneada; no-parseable → `NEG_INFINITY` |
| `inadmissible_reason` | `:172` | el predicado: `Some(reason)` ⇒ terminal; `None` ⇒ admisible |

**U3:** se excluye `amount_in_wei = 0`.
**U2:** la clave se sanea **antes de cualquier comparación**. Un no-finito va a `NEG_INFINITY`, que es un **valor de orden** (a diferencia de `NaN`), así que la comparación `> 0.0` es total y **un `NaN` no puede pasar nunca**.
**U4:** **no se filtra por rentabilidad.** El único borde es **exactamente cero** — identidad estructural («sin monto no hay swap»), no un umbral económico. `expected_profit_usd`, `net_expected_profit_usd` y `roi_pct` **no se leen**. Un umbral económico acá dejaría `passed=true` en 0 **para siempre** (medido: gross máx = 0 sobre 10.000 entradas, 0 con gross > 0) y nos dejaría ciegos justo cuando la simulación empieza a poder ejecutar.

Los motivos quedan dentro de la familia **ya existente** `candidate_incomplete:*`, que `persistence` clasifica como **gap de capacidad NO rechazante**: la fila explica por qué no corrió nada, y la oportunidad **no** se rechaza por esto (eso no es un veredicto de mercado).

> **Nota de diseño, declarada:** el predicado lee el `amount_in_wei` **del mensaje**; el monto autoritativo de la simulación viene de la fila PG (`route_lookup.rs`). Es deliberado — el punto es no tocar la base — y deja un riesgo residual **nombrado** en §7.

### Puerta 2 — tasa + concurrencia, y la semántica de DEFERRAL (U1, U5)

| símbolo | línea |
|---|---|
| `DEFAULT_MAX_SIMS_PER_SEC`, `DEFAULT_MAX_IN_FLIGHT` | `:111`, `:119` |
| `ENV_MAX_SIMS_PER_SEC`, `ENV_MAX_IN_FLIGHT` | `:123`, `:124` |
| `BoundPolicy::from_env`, `StreamBound` | `:232`, `:269` |
| `try_admit` / `note_deferred` / `note_inadmissible` / `maybe_report` | `:314` / `:326` / `:347` / `:364` |

**Un límite agotado es un DEFERRAL, nunca un veredicto.** El camino del tope **no persiste, no publica y — crítico — NO hace XACK**: la entrada **permanece en el PEL** del grupo y `recover_stale_pending` (XAUTOCLAIM) la redeliverá cuando vuelva el presupuesto.

Las dos alternativas se descartaron explícitamente, no por olvido:

- **ACK + descartar** → perdería en silencio una oportunidad real. Inaceptable.
- **ACK + persistir un veredicto «throttled»** → reclamaría una medición que **nunca corrió** y consumiría la oportunidad para siempre. Inaceptable por R8.

El deferral **reutiliza el camino de recuperación que ya existía** para errores de infraestructura transitoria. Costo declarado: el piso de redelivery es `CLAIM_MIN_IDLE_MS` = 120 s, y un backlog en pie es visible en `SIM_STREAM_PENDING_COUNT` / `SIM_STREAM_OLDEST_PENDING_MS`.

### Los DOS puntos de entrada están acotados (lección de t96)

`process_message` tiene **exactamente dos** call sites y los dos pasan por la cota:

1. el camino `>` de `read_batch` (entradas nuevas), y
2. el camino de **redelivery** de `recover_stale_pending` (`XAUTOCLAIM`, `consumer.rs:680`).

No es cosmético: un PEL sin cota sería un agujero por el que se escaparía **toda** la política — un backlog en pie se re-simularía a velocidad plena mientras el camino `>` se deja pastorear con educación. Lo encontró el compilador al cambiar la firma, y se verificó por grep exhaustivo (`process_message(`, `read_batch(`, `recover_stale_pending(`) en vez de esperar al próximo error.

### Extracción de `finish()` (`consumer.rs:902`)

El tail persist → XADD → ACK se extrajo a un método para que el camino inadmisible use **el MISMO** tail en vez de duplicarlo: un solo lugar donde un veredicto se persiste y se acusa recibo, un solo lugar donde un fallo de persistencia significa «no ACK, reintentar». El cuerpo se movió **verbatim** para todo caller preexistente.

---

## 3. El default y su derivación escrita (U5 + U9)

| knob | default | derivación |
|---|---|---|
| `SIMCTL_MAX_SIMS_PER_SEC` | **1.0** | ver abajo |
| `SIMCTL_MAX_IN_FLIGHT` | **1** | igual a lo que el consumidor ya hace (el loop es secuencial): es un **techo**, no un acelerador |

**Derivación del 1.0/s — con la aritmética a la vista, y NO desde 68/s.** La latencia por simulación del pipeline B2c está medida en **~2,084 s**, así que un consumidor **secuencial** no puede superar **1/2,084 ≈ 0,48 sims/s**. Una cota de **1,0/s queda por ENCIMA de ese techo físico**: por lo tanto **no estrangula** el despliegue de un consumidor, y a la vez acota una desbandada en **86.400 sims/día** en lugar de los **5,9 M/día** que permitiría un 68/s sin tope.

Es un número de **política de costo** con su premisa escrita — no una afirmación sobre carga REVM (U9), y el test `default_rate_sits_above_the_sequential_ceiling_and_is_not_the_row_rate` pincha exactamente esa propiedad (`DEFAULT > 1/2,084` y `DEFAULT ≪ 68`).

**El valor EFECTIVO se registra al arrancar** (`consumer.rs:414`, evento `sim_ctl.bound_policy_effective`), leído **de la struct resuelta** — no de esta documentación ni de memoria —, e incluye `rate_from_env` / `in_flight_from_env` para saber si la política vigente es la del entorno o la conservadora. Un valor presente pero inusable **NO se traga**: emite `sim_ctl.bound_env_unusable` y cae al default (un typo no puede cambiar la política en silencio).

**Mode-invariant (declarado):** PAPER == TESTNET == LIVE. Es política de **costo**; una cota que difiriera por modo sería otro sistema con el mismo nombre (doctrina §34.1).

---

## 4. Las dos estrategias refutadas, pinchadas como TEST (U1, U2)

**1. El ordenamiento top-N no es implementable.** Ordenar exige el universo en la mano, y `arbx:opps:validated` retiene `STREAM_MAXLEN` = **10.000** entradas (`consumer.rs:51`) a ~68 filas/s ⇒ una ventana de **~2,4 min** que nunca contiene el universo. **No existe un conjunto ordenable** ⇒ admisibilidad **en línea**.

**2. `f64::max(NaN, 0.0) = 0,0`** — un cero **indistinguible de un valor medido**, exactamente el modo de fallo de R8 («`Some(0.0)` = computado y exactamente cero»). Y **`NaN.partial_cmp(-0.1) = None`**: cualquier `partial_cmp(..).unwrap()` sobre la clave **cruda** panica y se lleva puesto **el loop entero del consumidor**.

Las dos están **pinchadas como test** para que una «simplificación» futura falle acá y no en producción:

| test (inline, `consumer.rs:1410+`) | qué pincha |
|---|---|
| `sanitize_key_maps_non_finite_to_the_worst_key_and_never_to_a_zero` | U2 |
| `refuted_max_substitute_would_fabricate_a_measured_zero` | **la refutación de `max()`** |
| `refuted_unwrap_on_the_raw_key_is_none_and_would_panic` | **la refutación del `unwrap` crudo** |
| `zero_and_unparseable_amounts_are_inadmissible_and_stay_capability_gaps` | U3 + familia de gap |
| `a_real_amount_is_admissible` | no sobre-rechazar |
| `predicate_ignores_profitability_entirely` | **U4, en los dos sentidos** |
| `verdict_depends_only_on_the_amount_key_not_on_route_or_strategy` | U1 (sin vista global) |
| `key_of_amount_in_wei_sanitises_the_unparseable_to_the_worst_key` | la clave nunca es `NaN` |
| `budget_admits_at_the_policy_rate_and_defers_everything_else` | U1 (pacing O(1), determinista) |
| `deferral_never_promotes_itself_and_the_counters_stay_separate` | un deferral no consume cupo |
| `an_unusable_policy_rate_cannot_panic_the_process` | `Duration::from_secs_f64` no puede paniquear |
| `default_rate_sits_above_the_sequential_ceiling_and_is_not_the_row_rate` | **U9** |
| `unusable_env_values_are_errors_and_absent_ones_are_not` | U5 |

**Un rojo propio, declarado:** la primera versión de `key_of_amount_in_wei_…` afirmaba `is_finite()` sobre la clave saneada. **`NEG_INFINITY` no es finito** — mi aserción y su comentario estaban mal, y el test falló. La propiedad correcta es **«nunca `NaN`»** (`NEG_INFINITY` *es* un valor de orden, y eso es justo lo que vuelve total la comparación). Corregido.

---

## 5. Interfaz de razones — la lección de t99 aplicada (U3)

Renombrar una familia de razones **es cambiar una interfaz**. Esta tarea **introduce un valor nuevo** (`candidate_incomplete:amount_in_wei_zero`) dentro de una familia existente, así que se clasificaron lectores y escritores **en la base rebasada**:

| lector | línea | cómo lee | ¿le afecta un valor nuevo? |
|---|---|---|---|
| `persistence::is_sim_capability_gap` | `persistence.rs:175` | `starts_with("candidate_incomplete")` | **no** — familia |
| `sim_taxonomy::classify_fail_reason` | `sim_taxonomy.rs:66` | marcador `contains` | **no** — y `"amount_in"` ya era marcador estructural (`:81`) |
| `main.rs::a3_candidate_incomplete` | `main.rs:366` | familia (HTTP 422) | **no** |
| `recon::drift_tracker` | `drift_tracker.rs:119` | familia (422) | **no** |

**Ningún lector enumera valores exactos**: todos son de familia (prefijo/substring/igualdad a nivel familia). El valor nuevo clasifica como **gap estructural no rechazante en los DOS clasificadores**, verificado sobre la base que ya contiene el renombre de F-01 (`cyclic_route_missing_route_metadata`, con la familia vieja **conservada** para filas históricas — `persistence.rs:135` y `:150`).

---

## 6. Las 4 puertas (U7) — ledger real

Contenido verificado: `consumer.rs` **md5 `9918264bd1358152115a65846a294ecf`**, 1.686 líneas, idéntico en WSL y en el clon de Windows.

| puerta | resultado | evidencia |
|---|---|---|
| `cargo fmt --all -- --check` | **EXIT=0** | `diffs_de_formato=0` |
| `RUSTFLAGS='-D warnings' cargo check -p sim-ctl` | **EXIT=0** | `Checking sim-ctl v0.1.0` presente · **0 warnings · 0 errores** |
| `cargo test -p sim-ctl --no-fail-fast` | **EXIT=0** | 6 binarios, **todos `ok`**, 0 fallos · **13/13 tests propios `ok`, 0 FAILED** |
| `cargo test -p sim-core --no-fail-fast` | **EXIT=0** | 3 binarios, **todos `ok`**, 0 fallos |

**Cambio:** `backend/sim-ctl/src/consumer.rs`, **+727/−10**, un solo archivo. `backend/` no se tocó en ningún otro punto.

### ★ Dos falsos verdes que cacé, y por qué importan

Esta sección existe porque **las puertas me mintieron dos veces** y ninguna de las dos mentiras era del código:

1. **`Finished in 4,16 s` sin la línea `Checking sim-ctl`.** Otro proceso (`cargo test -p sim-ctl`) tenía tomado el target compartido, y el target que copié traía *fingerprints de otro árbol*: cargo declaró **`Fresh sim-ctl v0.1.0`** sobre un fuente que había cambiado. **Un exit 0 no prueba que el evento ocurrió** — se fuerza recompilación con `touch` y se EXIGE ver `Checking sim-ctl` antes de creerle.
2. **`bash script.sh` no es login shell: `cargo` no está en el PATH y las 4 puertas devolvieron `127`.** Con el log vacío, `grep -c` devolvía **0** y el resumen se leía como «0 diffs de formato / 0 warnings»: un **rojo disfrazado de verde perfecto**. Guard: el ledger verifica `command -v cargo` y **exige la línea de compilación** en la salida.

Ninguna de las dos toca al código; las dos tocan a la **evidencia**. Quedan escritas para que el próximo que lea «verde» sepa qué se exigió para decirlo.

---

## 7. Riesgos residuales, nombrados

1. **Divergencia mensaje ↔ fila PG en el monto.** El predicado lee el `amount_in_wei` **del mensaje**; la simulación usa el de la fila (`route_lookup.rs`). Son la misma fuente en origen (selector-api escribe la fila y publica el mensaje), pero **no lo medí**. Si divergieran, una entrada con mensaje malo y fila buena se terminaría con un gap en vez de simularse. **Detector:** un pico de `candidate_incomplete:amount_in_wei_zero` en `simulations.fail_reason` con oportunidades cuya fila PG tenga monto > 0.
2. **Piso de deferral de 120 s** (`CLAIM_MIN_IDLE_MS`) y backlog en pie en el PEL. **Detector:** `SIM_STREAM_PENDING_COUNT` / `SIM_STREAM_OLDEST_PENDING_MS`.
3. **La cota de concurrencia es un TECHO, no un acelerador** hoy: con `max_in_flight = 1` y un loop secuencial nunca es la restricción activa — la tasa lo es. Se declara así en vez de presentarla como carga.
4. **El régimen de overflow ya existe sin esta cota.** Con llegadas a ~68 filas/s y un techo secuencial de ~0,48 sims/s, el consumidor **no puede** seguir el ritmo en cuanto las simulaciones ejecuten; la retención del stream es de ~2,4 min (`MAXLEN` 10.000), así que las entradas **nunca leídas** se recortan. **Esto no lo crea la cota** (1,0/s está por encima del techo físico, §3): lo **nombra**. La mitigación no es subir la cota, es **reducir la admisión aguas arriba o paralelizar**.
5. **Ventana de consumo real sin tope hasta que esto se despliegue** — riesgo aceptado y declarado por la célula, con la tasa de §1. No se oculta: se nombra.
6. **Lo que este cambio NO prueba:** nada sobre Redis de producción, nada sobre rentabilidad, y ninguna mejora de métrica de negocio.

---

## 8. Alcance y base (U8)

- **NO se mergea y NO se despliega.** Rama propia + PR contra `main`, con `mergedAt=null`; el aterrizaje es de Release-B.
- **Base declarada, medida por el REMOTO antes de empujar:** la rama de **#846** en su head **`4ccecb66`** (`docs/sim4-cyclic-01`, que ya trae F-01 y F9 de t91). `main` se movió a **`cd1114d1`** (merge del PR #844), y **#846 diverge de `main`** (su merge-base es `21d2039c`). Mi commit se **rebasó** sobre el head nuevo antes de empujar — sin force.
- **Consecuencia de la base, declarada para el revisor:** el diff del PR contra `main` mostrará la **UNIÓN** con #846 (7 archivos: `persistence.rs`, `sim_engine.rs`, `tx_builder.rs`, `docs/backend/SIM4-CYCLIC-01.md`, …) porque el merge-base es `21d2039c`. **Mi cambio son 2 archivos**: `backend/sim-ctl/src/consumer.rs` y `docs/sre/SIMCTL-BOUND-01.md`.

---

## 9. Reproducción

```bash
cd backend
cargo fmt --all -- --check
RUSTFLAGS='-D warnings' cargo check -p sim-ctl     # exigir la linea "Checking sim-ctl"
cargo test -p sim-ctl --no-fail-fast               # 13/13 propios
cargo test -p sim-core --no-fail-fast
```

---

*Una cota sin derivación escrita es una corazonada con formato de política. Acá el default tiene su aritmética, el multiplicador que NO se midió se declara NO COMPUTADO, y las dos estrategias refutadas quedan pinchadas como test para que no vuelvan por la puerta de atrás.*
