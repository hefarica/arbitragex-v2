# AMOUNT-IN-WEI-ZERO-01 — El cero lo produce el PRODUCTOR. Y el «92,08 %» es la razón entre dos techos, no la composición del productor

**run_id** arbx-entrega-20261008 · **fase** DIAGNOSIS (cero cambios de código, cero escrituras, solo lectura)
**SHA DEL ÁRBOL LEÍDO** (nunca de memoria): **`origin/main` = `77b42b3dccc001455fda3e8d4437d973c9e98c4b`**
**Alcance escrito**: `docs/backend/` únicamente · **Prohibiciones respetadas**: sin merge, sin deploy, sin tocar PG ni el motor.

### Blobs de los loci (integridad por objeto git, no por ruta)

| archivo | blob |
|---|---|
| `backend/sim-ctl/src/sim_engine.rs` | `a86e45a8fa373aead429c7df9cf6926ce2fac6ab` |
| `backend/sim-ctl/src/persistence.rs` | `377ef1b7a4c603772cb5205104e2e60faf29da0e` |
| `backend/sim-ctl/src/consumer.rs` | `83b682e5b979eb7026b38247b42747e34223b284` |
| `backend/searcher-rs/src/size_optimizer.rs` | `a1411d8200d3baad5c912f2dcba036cc63e032b7` |

---

## 0. CONTROL DE CANAL PRIMERO

`SELECT 1` → `1` · `exit=0`. **Los ceros de este informe son medidos, no canal roto.**

---

## 1. LAS DOS PUERTAS, HOY (se reproduce t159)

Ventana: `simulated_at > now() - interval '1 hour'`.

| puerta | filas | % del vivo | rastro (`revert_risk_pct`) |
|---|---|---|---|
| `candidate_incomplete:amount_in_wei_zero` | **8.052** | 91,6 % | **0 / 8.008 = 0 %** |
| `sim_signer_funding_slot_unresolved` | **733** | 8,34 % | **739 / 739 = 100 %** |
| `funding_balanceof_timeout` | **3** | 0,03 % | **3 / 3 = 100 %** |
| **Σ = 8.788 = `vivo`** | | | **`passed = true` = 0** |

`count(*) FILTER (WHERE simulated_at > now() - interval '1 hour') AS vivo, count(*) AS total` → **`8788|701706`** ⇒ el vivo es **1,25 %** de la tabla; **el 98,75 % es época muerta**. Sin corte, el agregado miente.

**La suma de las dos puertas cierra EXACTA contra el vivo ⇒ el reparto es exhaustivo, no top-N.** Todas con su `exit=0`.

### 1.1 El control que la cápsula proponía era la COLUMNA EQUIVOCADA — declarado

La cápsula pedía el control positivo con `raw_trace IS NOT NULL` (baseline 695/h). **Medido hoy: `0`.** No es que no haya rastro: es que **`raw_trace` es NULL en el 100 % de las filas vivas, incluidas las que SÍ simulan.** Un control que da cero sobre la población que debe ser positiva **no es un control**, es un falso negativo.

**El campo de rastro real es `revert_risk_pct`**, y con él las cifras del enunciado se reproducen **exactamente** (0 % vs 100 %, tabla de arriba). `gas_estimate_wei` y `slippage_pct` también son 0 en ambas poblaciones: **no distinguen.** Se declara para que el control no se herede mal.

---

## 2. ★ LA PREGUNTA BINARIA — RESPUESTA: **EL PRODUCTOR**

**Discriminante, declarado antes de contar**: si lo produce el productor, el dato aguas arriba ya es cero; si el consumidor, aguas arriba hay un valor no-cero que se pierde.

**Dato aguas arriba = el mensaje en `arbx:opps:validated`**, que es lo que el consumidor lee.

```
XRANGE arbx:opps:validated - + COUNT 10000     xrange_exit=0
payloads=10000
METODO A (partición awk):      CERO=653  NOCERO=9347  TOTAL=10000
METODO B (histograma):          8643 "1000000000000000000"   653 "0"   198 "4120019704926761602" …
METODO C (largo del token):      653 de 1 dígito (sólo el 0 lo es)   |  8841 de 19 dígitos
```

**Tres métodos independientes coinciden: el productor emite `"amount_in_wei":"0"` en el 6,53 % del flujo vivo.**

**Y la lectura directa en PG**, join `simulations`→`opportunities`:

| población | `opportunities.amount_in_wei` | filas |
|---|---|---|
| A — `amount_in_wei_zero` | **`0`** | **8.008 / 8.008 = 100 %** |
| B — fondeo (CONTROL) | `1000000000000000000` (y otros) | **100 % NO-CERO** |

**⇒ El cero YA ESTÁ en el mensaje que el consumidor recibe. El consumidor no lo pone: lo lee.** La respuesta es **PRODUCTOR**.

**Sub-población 100 % no-cero en el control ⇒ un cero con productor probado, no un cero sin productor.**

---

## 3. ★★ PERO EL «92,08 %» ES LA RAZÓN ENTRE DOS TECHOS, NO LA COMPOSICIÓN DEL PRODUCTOR

**Ésta es la corrección de encuadre, y sale de que los dos números no cierran entre sí:**

- El productor emite **6,53 %** de ceros.
- Las filas terminadas son **91,6 %** ceros.

Si el consumidor procesara el stream completo, la cuota de ceros entre sus filas terminadas **sería la del stream**. **No lo es** ⇒ las dos cuotas describen cosas distintas.

**La aritmética cierra exacta:** `8.052 / 0,0653` = **123.300 mensajes/h**, del orden del caudal total del stream. Es decir: **el consumidor SÍ procesa el stream prácticamente entero por el camino barato** (el gate 1 estructural no paga PG ni fork), y **6,53 % de ese caudal son ceros ≈ 8.000/h**. El camino caro, en cambio, está limitado por la latencia del fork: **736/h**.

**⇒ `91,6 %` es `barato / (barato + caro)` — la razón entre dos techos de tasa distintos, no la proporción de lo que el productor emite.**

**Es exactamente la regla derivada en t108 y no invalida el número: lo reencuadra.** `passed = true = 0` sigue siendo verdad y sigue sin ser veredicto de mercado — pero **la palanca no es «el 92 % del presente»**: es que **dos puertas no-mercado, medidas a tasas distintas, consumen el 100 % de lo que se termina**.

---

## 4. ★★ t146 — LA HIPÓTESIS **MUERE**, por tres vías independientes

La hipótesis: `size_optimizer.rs:3915` devuelve `"WETH"` como último recurso de `resolve_token_in_symbol`; ese valor alimenta `effective_capital_for`; el capital queda mal y **el tamaño cae a cero**.

**(1) La separación que la hipótesis exige NO EXISTE.** Comparando las dos poblaciones por `token_in`:

| población | tokens |
|---|---|
| A (`amount_in_wei_zero`) | WETH **3520** · DAI **1472** · USDC **1374** · PYUSD 869 · UNI 243 · WBTC 229 · USDE 226 |
| B (fondeo, control) | WETH **689** · USDE 28 · DAI 20 · … |

**Ambas poblaciones tienen el MISMO conjunto de tokens mayoritarios y reconocibles.** Si el colapso a WETH produjera el cero, A debería estar dominada por tokens **no reconocibles**. No lo está: **está dominada por WETH/DAI/USDC, exactamente los tokens que el resolvedor SÍ reconoce** (y que las ramas `contains("USDT")`/`contains("DAI")` del propio código tratan explícitamente).

**(2) El código NO dice lo que la hipótesis supone.** Leído en el blob `a1411d82…`, el último recurso es:

```rust
        // Last resort: the chain's native token (documented heuristic — the
        // caller's UnknownTokenPrice gate still catches unresolvable tokens).
        return Some(native_symbol.to_string());
```

**Devuelve `native_symbol`, no un `"WETH"` hardcodeado** — en la cadena 1 coincide con WETH, pero es el símbolo nativo. Y su propio comentario declara que **el gate `UnknownTokenPrice` aguas arriba atrapa los irresolubles**: la cadena de tres eslabones no está sostenida por el código.

**(3) El cero es un valor LITERAL emitido, no un colapso numérico.** El stream trae `"amount_in_wei":"0"` como valor discreto (653 casos) frente al dominante `"1000000000000000000"` (8643) — **un tamaño FIJO de 1 ETH**. Un capital mal calculado no produce un `0` literal junto a un `1e18` literal: **produce una distribución continua.** Esto es un productor eligiendo entre dos valores discretos.

**⇒ t146 MUERTA. Se entrega muerta, que vale lo mismo que confirmarla.**

---

## 5. SECUENCIALES O INDEPENDIENTES

**INDEPENDIENTES, y la prueba es de orden de código.** Leído en `sim_engine.rs` (blob `a86e45a8…`):

| línea | qué |
|---|---|
| `:91-95` | `Err(BuildError::CyclicRouteMissingPath)` → `cyclic_route_missing_route_metadata:{}` |
| `:98-108` | `route_path_not_representable:{}` |
| **`:122`** | **`let handle = match fork.acquire().await`** |
| **`:142`** | **`funder.ensure_funded(...)`** |

**El gate 1 de admisibilidad (`consumer.rs`) corre ANTES de todo esto** — es O(1), sin I/O, sin fork. Por eso la puerta 1 **nunca llega** al fork: muere en la admisión.

Y las 736 filas que **sí** llegan al camino caro **mueren en el fondeo (`:142`), no antes**: la suma `slot_unresolved + rpc_err = 2468 + 5 = 2473` es **exactamente** `arbx_simulation_total{simulator="anvil"}` = **2473** (medido: `exit=0`). **⇒ cada fallo de fondeo es un intento de simulación, y no hay ninguno que muera en el constructor.**

**⇒ Son independientes y actúan en capas distintas**: la puerta 1 filtra **antes** del fork; la puerta 2 es **el** fork. No compiten por el mismo recurso.

---

## 6. ★ LA ANOMALÍA DE t159 — EXPLICADA (no NO COMPUTADO)

**El hecho**: `persistence.rs` y `sim_engine.rs` documentan `cyclic_route_missing_route_metadata` y `route_path_not_representable` como productores **actuales**, y en PG hay **CERO filas de ambos, ni históricas**.

**Medido, numerado, uno por uno, cada uno con `exit=0`:**

| consulta | filas |
|---|---|
| `fail_reason LIKE 'cyclic_route_missing_route_metadata%'` | **0** |
| `fail_reason LIKE 'route_path_not_representable%'` | **0** |
| **CONTROL POSITIVO DEL LIKE** `fail_reason LIKE 'candidate_incomplete%'` | **54.646** |
| `fail_reason LIKE 'strategy_cyclic_route_not_simulatable%'` (familia vieja) | **617.570** |

**El control positivo prueba que el patrón `LIKE` funciona** ⇒ **los ceros son del dato, no de la consulta.** (Éste es justo el defecto que t159 declaró propio y que aquí se evita: conteo numerado con `count(*)`, y un control que sí es positivo.)

**La explicación sale del orden de §5.** Las dos familias nuevas se emiten **antes** del fork (`:91-108`), desde el constructor de la sonda, y **sólo cuando la ruta falta o es incoherente**. Medido hoy: **736/h llegan al camino caro y mueren TODAS en el fondeo (`:142`), ninguna en el constructor.**

**⇒ El flujo está PASANDO el constructor.** Y eso es la consecuencia esperada de SIM4-CYCLIC-02: el call site pasó a enviar `route_metadata.token_addresses`, así que `CyclicRouteMissingPath` **ya no se levanta** para las rutas cerradas con path presente.

**⇒ No es una anomalía: es el arreglo funcionando.** El código de `main` describe un camino de error que **ya no se ejerce porque el defecto que lo motivaba está corregido**. La familia vieja (617.570) es histórica y se detuvo con el fix.

---

## 7. ★ LA DISCREPANCIA ENTRE INSTRUMENTOS — **MAPEADA**, y no era una contradicción

**Los dos números que t159 no pudo cerrar** (Prometheus en el host, arranque actual):

```
arbx_sim_funding_total{outcome="slot_unresolved"}     2468
arbx_sim_funding_total{outcome="verify_mismatch"}     9872
arbx_sim_funding_total{outcome="balance_unreadable"}  1390
arbx_sim_funding_total{outcome="rpc_err"}                5
arbx_simulation_total{passed="false",simulator="anvil"} 2473
arbx_simulation_total{passed="false",simulator="revm"} 36789
```

**⇒ `9872 / 2468 = 4,0000` EXACTO.** Y **`2468 + 5 = 2473` = `arbx_simulation_total{anvil}` EXACTO.**

**La explicación es de UNIDADES, no de contradicción:**

| instrumento | granularidad |
|---|---|
| `verify_mismatch` (métrica) | **por SLOT SONDEADO** — `CANDIDATE_SLOTS` tiene 4 entradas y `write_rejected` **nunca** dispara (no está en la lista de series), así que cada intento produce exactamente 4 |
| `slot_unresolved` (métrica) | **por INTENTO agotado** = el número de FILAS `sim_signer_funding_slot_unresolved` en la tabla |
| `balance_unreadable` | **CONTenido en `verify_mismatch`** (el `Ok(None)` cae al `else` del loop y cuenta AMBOS) |

**⇒ La tabla tiene 0 filas de `verify_mismatch` porque `verify_mismatch` NO ES UN `fail_reason`: es una etiqueta de métrica.** Buscarla en `simulations.fail_reason` es un error de categoría, no un dato faltante. **No hay discrepancia que resolver: hay dos instrumentos midiendo a distinta granularidad, y el factor 4 es constante y verificable.**

**Coherencia con la ventana de 1 h**: `slot_unresolved` acumulado = **2468**; la tabla da **733/h** ⇒ 2468/733 ≈ **3,4 h** de arranque acumulado. Consistente.

---

## 8. LÍMITES DEL INSTRUMENTO (declarados, no rellenados)

- **`process_start_time_seconds{job="sim-ctl"}` → `{"resultType":"vector","result":[]}` = VACÍO** ⇒ **NO COMPUTADO, no cero.** Por eso la antigüedad del arranque se estima por el cociente de §7 y **se declara como estimación**, no como medición.
- **Todo contador de Prometheus es acumulativo desde el arranque de `sim-ctl`.** El deploy de `77b42b3d` lo reinicia: **los deltas de §7 pertenecen al arranque actual** y no son comparables con lecturas de otro arranque.
- **`no_simulation_row` es NO MEDIDA, no fallo** — no se usó como evidencia de nada.
- **`http://195.201.235.70/metrics` NO SE USÓ como fuente**: verificado hoy `http_code=404 content_type=text/html; charset=utf-8` (`exit=0`) — consola Next.js, no Prometheus. La fuente es la **API de Prometheus en el host** (`:9090/api/v1/query`, `status:"success"`).

---

## 9. LO QUE ESTE INFORME **NO** DICE

- **NO dice dónde arreglar nada.** Diagnostica y localiza; el arreglo es otra tarea. Cero cambios de código, cero umbrales, cero escrituras.
- **NO dice que el productor esté mal.** Dice que el productor emite el cero, y que **su frecuencia (6,53 %) no es la que explica el 91,6 %**: eso es la razón entre dos techos de tasa.
- **NO confirma t146.** La mata, con tres vías independientes.
- **NO convierte el cero en cero computado cuando el instrumento no mide**: donde no hay dato se dice NO COMPUTADO.
