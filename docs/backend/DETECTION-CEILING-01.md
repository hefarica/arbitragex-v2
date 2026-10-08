# DETECTION-CEILING-01 — ¿Existe ALGÚN (ruta, tamaño) donde el neto cruce el gas floor?

**Tarea:** t175 · **Agente:** Backend · **Modo:** SOLO LECTURA y cálculo (cero cambios, cero escrituras, **el fork NO se escribió**) · **Paper**
**Frontera pre/post:** `docker inspect --format '{{.State.StartedAt}}' arbitragex-v2-sim-ctl-1` -> **`2026-10-08T14:16:07.1195963Z`**, exit=0 — **sin cambio, NO hubo reinicio.**
**Puerta:** `deploy.sha=77b42b3dccc001455fda3e8d4437d973c9e98c4b`, `id=37783694993`, `at=13:47:10Z`, exit=0.

---

## 1. VEREDICTO

**NO EXISTE NINGÚN (ruta, tamaño) EN EL UNIVERSO BARRIDO DONDE EL NETO CRUCE EL GAS FLOOR. NI SIQUIERA EXISTE UNO DONDE EL NETO SEA POSITIVO.**

```
★ MÁXIMO NETO GLOBAL sobre TODOS los tamaños de la mejor ruta  =  −$0,6475   a  $91
★ GAS FLOOR (net ≥ 3×cost, cost 0,6491–0,6891)                 =  $1,95
★ Faltan $2,60.  Y el máximo BRUTO de toda la curva es +$0,0283 (a $100)
                 contra un gas de $0,676131  =>  EL GAS ES 24× EL BRUTO MÁXIMO
```

**Un negativo es el resultado, y se entrega como tal.** La herramienta no gana porque **su mejor edge en el universo medido no alcanza ni para pagar el gas de una sola operación** — no porque el tamaño esté mal elegido.

---

## 2. EL BARRIDO: 20 PARES ORDENADOS DE LOS 5 POOLS

### 2.1 Liquidez REAL, leída de la cadena

`docker exec arbitragex-v2-anvil-1 cast call <pool> '<sig>' --rpc-url http://localhost:8545`, exit=0. **Los cinco pools son WETH/USDT** (`token0 = 0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2`, `token1 = 0xdAC17F958D2ee523a2206206994597C13D831ec7`).

| Pool | Tipo | Fee | Liquidez real | Precio implícito |
|---|---|---|---|---|
| `0x0d4a11d5eeaac28ec3f61d100daf4d40471f1852` | **V2** | 0,30 % | **`getReserves()` = 2900,498828 WETH / 7 328 270,235623 USDT** ⇒ ≈$7,33 M | **$2526,56** |
| `0x11b815efb8f581194ae79006d24e0d814b7697f6` | V3 | 0,05 % | `liquidity()` = 700366674727580152 | $2533,00 |
| `0x6ca298d2983ab03aa1da7679389d955a4efee15c` | V3 | 0,05 % | `liquidity()` = 27106536726875741 | **$2533,54** |
| `0xc7bbec68d12a0d1830360f8ec58fa599ba1b0e9b` | V3 | 0,01 % | `liquidity()` = 170091345252275324 | $2532,24 |
| `0xacdb27b266142223e1e676841c1e809255fc6d07` | V3 | 0,01 % | `liquidity()` = 7251378828591346 | **$2530,43** |

**Precios de `slot0()`** (`sqrtPriceX96`), **no estimados**: `3987465133581222698216719` · `3987895813269048465582267` · `3986870851083312689704253` · `3985442794530995835338076`, con `tick` = −197949 / −197947 / −197952 / −197959.

### 2.2 Fórmulas declaradas — V2 y **V3** (t174 sólo tenía V2)

```
V2  sell: dy = (dx·997/1000 · y) / (x + dx·997/1000)          buy: dx = (dy·997/1000 · x) / (y + dy·997/1000)
V3  token0 in: 1/√P' = 1/√P + Δx/L      out token1 = L(√P − √P')
    token1 in: √P' = √P + Δy/L          out token0 = L(√P' − √P)/(√P·√P')
    fee: Δ_efectivo = Δ·(1 − fee/10⁶)
```

**★ VALIDACIÓN POR PRECIO MARGINAL** (el instrumento se comprueba contra sí mismo, no contra un tercero):

```
sellWETH V3 1e-6 WETH  ->  ratio 1,000000   ·   buyWETH V3 1 USDT  ->  ratio 1,000000
sellWETH V2 1e-3 WETH  ->  ratio 1,000000   ·   buyWETH V2 1 USDT  ->  ratio 1,000000
```
**Los cuatro dan 1,000000. Las fórmulas son correctas.** Y de paso: **el V2 del pool principal da degradación `0,3136 %` a $1000 — idéntico al de t174**, así que las dos tareas usan la misma máquina.

### 2.3 Spread vs. fee del round-trip, los 20 pares

**Sólo 3 de 20 tienen neto positivo ANTES de impacto:**

| Comprar en | Vender en | spread | fees | **neto pre-impacto** |
|---|---|---|---|---|
| **`0xacdb27b266`** | **`0x6ca298d298`** | **0,1231 %** | 0,060 % | **+0,06307 %** ← MEJOR |
| `0xacdb27b266` | `0xc7bbec68d1` | 0,0717 % | 0,020 % | +0,05166 % |
| `0xacdb27b266` | `0x11b815efb8` | 0,1015 % | 0,060 % | +0,04146 % |

**★ Y el muro del fee, medido:** los 4 peores son **todos V3→V2**, con **spread NEGATIVO** (−0,15 % a −0,28 %) y **fee 0,31–0,35 %**. **⇒ El mejor spread contra el V2 (0,2766 %) es MENOR que el fee del round-trip (0,350 %). El V2, con su 0,30 %, no puede participar en ningún round-trip rentable con estos pools.** Eso es estructural, no de tamaño.

---

## 3. ★★ EL BARRIDO DE TAMAÑOS SOBRE EL PAR QUE SÍ TIENE EDGE

Par: **comprar en `0xacdb27b266`, vender en `0x6ca298d298`**. Gas fijo `total_cost_usd = 0,676131` (medido en la fila real; **el gas no escala con el tamaño**).

| tamaño | USDT out | bruto | **bruto %** | **net** | ¿≥$1,95? |
|---|---|---|---|---|---|
| $50 | 50,02 | +$0,0228 | 0,04569 % | −$0,6533 | no |
| **$91** | 91,03 | **+$0,0286** | **0,03143 %** | **−$0,6475** | **no** |
| $100 | 100,03 | **+$0,0283** | 0,02831 % | −$0,6478 | no |
| $200 | 199,99 | −$0,0128 | −0,00642 % | −$0,6890 | no |
| $300 | 299,88 | −$0,1234 | −0,04113 % | −$0,7995 | no |
| $500 | 499,45 | −$0,5523 | −0,11047 % | −$1,2285 | no |
| $1.000 | 997,17 | −$2,8341 | −0,28341 % | −$3,5102 | no |
| $2.000 | 1 987,45 | −$12,5498 | −0,62749 % | −$13,2259 | no |
| $5.000 | 4 917,72 | −$82,2816 | −1,64563 % | −$82,9578 | no |
| $20.000 | 18 712,27 | −$1 287,7319 | −6,43866 % | −$1 288,4080 | no |

**★ BARRILLO FINO (paso $1 hasta $20.000): el MÁXIMO GLOBAL es `−$0,6475` a `$91`.**

### 3.1 ★ LA PENDIENTE MEDIDA en el tramo que t174 dejó ciego ($150–$1.000)

**Cuatro puntos nuevos, medidos** (ninguno ajustado):

```
$100 -> 0,02831 %    ·    $200 -> −0,00642 %    ·    $300 -> −0,04113 %
$500 -> −0,11047 %   ·    $1 000 -> −0,28341 %
```

**⇒ El bruto CRUZA CERO entre `$100` y `$200`.** La ganancia no «decae»: **cambia de signo**. **No se reintrodujo ningún ajuste de ley de potencia** (t174 lo descartó con razón y aquí no vuelve).

### 3.2 La aritmética que lo cierra

```
bruto necesario para cruzar el floor = floor + gas = 1,95 + 0,676131 = $2,6261
con el edge SIN impacto (0,06307 %):  tamaño = 2,6261 / 0,0006307 = $4 164
pero la liquidez VIRTUAL del pool de compra es $365 218  (x_virt = 144,15 WETH)
=> a $4 164 el impacto es 4 164/365 218 = 1,14 %, o sea 18x el edge
=> NO HAY INTERSECCIÓN
```

---

## 4. ★★ IMPACTO ACUMULADO — el hueco que t174 declaró y que decide si el sistema puede GANAR

**t174 lo dejó NO COMPUTADO con estas palabras:** *«Esto es una operación aislada; a 930/h el impacto acumulado es otro problema y no lo medí.»* **Aquí se mide, con el estado de los pools actualizándose entre operación y operación.**

| op # | precio compra | precio venta | **spread** | bruto | net |
|---|---|---|---|---|---|
| **1** | $2530,43 | $2533,54 | **+0,1231 %** | +$0,0286 | **−$0,6475** |
| **2** | $2531,69 | $2533,21 | **+0,0599 %** | −$0,0289 | −$0,7050 |
| **3** | $2532,95 | $2532,87 | **−0,0034 %** | −$0,0864 | −$0,7625 |
| 10 | $2541,80 | $2530,51 | −0,4443 % | −$0,4872 | −$1,1633 |
| 50 | $2592,66 | $2517,25 | −2,9087 % | −$2,7274 | −$3,4035 |
| 100 | $2656,95 | $2501,19 | −5,8626 % | −$5,4126 | −$6,0887 |

**★ EL POOL SE AUTO-ARBITRA EN 3 OPERACIONES: el spread pasa de `+0,1231 %` a `−0,0034 %` en tres round-trips de $91.** Es exactamente el mecanismo clásico — **el arb se cierra a sí mismo** — y ahora está medido.

### 4.1 ¿Cuántas operaciones por hora soporta un pool?

```
a $100/op, de 930 intentos (1 hora al ritmo que declara el sistema): 0 con net > 0
=> el pool sostiene 0 operaciones antes de que la ganancia desaparezca
=> y la PRIMERA ya da net = −$0,6478: NO HAY GANANCIA QUE SOSTENER
                                 (a $91/op: 0 de 300; a $200/op: 0 de 300)
```

**⇒ COMPUTADO, y el resultado es cero.** No por el impacto acumulado — **porque la operación aislada ya es negativa**. El acumulado sólo empeora algo que ya no existe.

**Y la consecuencia que el contrato pedía:** **930/h no es «optimista» ni «pesimista»: es irrelevante.** El sistema podría ejecutar 1/hora o 930/hora y el resultado sería el mismo, **porque ninguna de las dos primeras cruza cero.**

---

## 5. ★★ HALLAZGO COLATERAL: LA MEJOR OPORTUNIDAD DE LA HISTORIA ES UNA COTIZACIÓN VIEJA

```
fila de mayor net_profit_usd:  quote_block = 26137384   ·   precio implícito = $2653,12/WETH
bloque ACTUAL del fork:        26148216
spot del V2 AHORA:             $2526,56/WETH
=> Δ = 10 832 bloques   ·   el precio se movió −4,77 %
```

**Y los `quote_block` VIVOS están en ~`26145350`–`26145376`** (1056 filas cada uno). **⇒ La fila «mejor» es ~8 000 bloques MÁS VIEJA que las cotizaciones activas.**

**⇒ El `+$1,545926` de t173/t174 es un resultado de estado que ya no existe.** El edge de `0,2222 %` que t174 midió como «el mejor jamás detectado» **está calculado a precios de $2653**, y hoy los pools están a **$2526,56**. **No es que el edge no sobreviva al tamaño: es que el edge medido es de una foto anterior.**

**Declaración honesta de qué se conserva de t173/t174:** la **curva** (el ROI decae con el tamaño) y el **mecanismo** (fricción > edge) siguen en pie porque se midieron con los `economics` de cada fila. **Lo que hay que relativizar es el VALOR ABSOLUTO `+1,5459`**: es de una cotización que ya expiró. **Se declara, no se retracta: la conclusión de t174 era un NO, y esto sólo la refuerza.**

---

## 6. ★★ CORRECCIÓN MÍA, Y AFECTA TRES INFORMES PREVIOS

**Lo que reporté en t164 (PR #890), y repetí en t167 (PR #895) y t173 (PR #899):**

> *«La taxonomía del instrumento CAMBIÓ. `slot_unresolved` no "bajó a cero": la etiqueta dejó de existir.»*
> *«Que una etiqueta desaparezca NO es que la causa desaparezca.»*

**ES FALSO, y ahora está medido.** `StartedAt` = `2026-10-08T14:16:07.1195963Z`, **sin cambio: NO hubo reinicio.** Y el contador, `curl -s 'http://localhost:9090/api/v1/query?query=arbx_sim_funding_total'` por loopback, exit=0, **ts `1791471873.213`**:

```
slot_unresolved     = 1        <- LA ETIQUETA EXISTE Y DISPARÓ
rpc_err             = 1
balance_unreadable  = 1
verify_mismatch     = 10
cache_hit           = 997
seeded_fresh        = 7
outcomes distintos  = 6        <- eran 3 cuando yo dije "la taxonomía cambió"
```

**Y PG lo confirma en la misma ventana:** `SELECT count(*) FROM simulations WHERE simulated_at > '2026-10-08T14:16:07Z' AND fail_reason='sim_signer_funding_slot_unresolved'` -> **`1`**, exit=0.

**⇒ NO HUBO RENOMBRE. HUBO UNA CAÍDA DE TASA:**

```
pre-arreglo : slot_unresolved 2786 / anvil 2791 = 99,82 %
post-arreglo: slot_unresolved    1 / anvil 1046 =  0,0956 %      (anvil = 1046, ts 1791471873.205)
=> el fallo de fondeo cayó ~1 044x, y NO es cero
```

**★★ Y el error es de un tipo que esta campaña entera viene cazando: LEÍ UNA AUSENCIA EN MI VENTANA COMO UN CAMBIO DE ESTADO.** Es exactamente `«la ventana de logs no lo muestra» no es «nunca ocurrió»`. **Lo cometí en tres informes consecutivos, y lo cazó el mismo instrumento que yo estaba usando — al mirarlo una vez más.**

**★ Y hay una consecuencia buena:** el techo de fondeo **NO está cerrado, pero está AGRIETADO de verdad y con número** — **99,82 % → 0,0956 %**. El arreglo de #879 **funciona**, y ese número es más informativo que «la etiqueta desapareció».

**Declaración de alcance de la corrección:** la contradicción que yo planteé (*«una etiqueta que desaparece no es una causa que desaparece»*) era **correcta como principio** y **mal aplicada como hecho**: la etiqueta no había desaparecido. **El principio se conserva; el hecho se retracta.**

---

## 7. EL UNIVERSO: CUÁNTOS SE BARRIERON Y CUÁNTOS QUEDARON FUERA

```
SELECT count(*) FROM pools WHERE chain_id=1                                    ->  4 259
SELECT count(*) FROM pools WHERE chain_id=1 AND is_active                      ->  1 336
SELECT count(*) FROM pools WHERE chain_id=1 AND is_active AND tvl_usd IS NOT NULL -> 44
```

**★ LA TABLA NO PUEDE DAR LA LIQUIDEZ: sólo 44 de 1 336 pools activos tienen `tvl_usd`.**

| | count | |
|---|---|---|
| Pools chain 1 | 4 259 | |
| Activos | 1 336 | |
| **Barridos DE VERDAD** (reservas/`slot0` leídos del fork) | **5** | los 5 que sostienen las rutas rentables medidas |
| **NO barridos** | **1 331** | **razón: sin `tvl_usd` en la tabla (1 292) y sin reservas leídas on-chain para el resto.** Leer 1 331 pools con `cast` exigiría 1 331 RPC round-trips contra un fork compartido y vivo |

**⇒ NO SE EXTRAPOLA. El veredicto dice «en el universo BARRIDO».** Lo que **sí** se puede afirmar del universo entero: **los 5 pools barridos son los que el propio sistema identificó como sus mejores rutas** (48+48+9+9+4 filas de las rentables), **y el mejor edge que el sistema encontró en 9,3 M de oportunidades vive en ellos.** Que exista un pool no barrido con un edge >0,1231 % es **NO COMPUTADO** — y se declara como el límite del barrido, **no como un cero**.

---

## 8. LÍMITES DEL INSTRUMENTO

| Límite | Declaración |
|---|---|
| **V3 en rango único** | Las fórmulas V3 suponen **un solo tick range activo**. Válido para los tamaños probados (el impacto calculado los mantiene dentro; el mayor lleva el precio un 6 % y **ahí la fórmula deja de ser válida**) ⇒ **para tamaños ≥ $5 000 la cifra es indicativa**. **No cambia el veredicto: el máximo está en $91** |
| Ventana de la tabla viva | `opportunities` crece (9 323 311 → más). Cada cifra lleva su corte |
| Los 1 331 pools no barridos | **NO COMPUTADO**, con la razón (§7) |
| `tvl_usd` ausente en 1 292 | Por eso **la liquidez se lee de la cadena**, prohibido estimarla por fracción de TVL |
| `slot_unresolved` en PG vs contador | **Ambos = 1**, medidos por separado (§6) |

---

## 9. CONTROLES

| Control | Resultado |
|---|---|
| **Canal `SELECT 1`** | `1`, exit=0 |
| **Negativo `ON_ERROR_STOP`, SIN TUBERÍA** | `SELECT esto_no_existe` -> **exit=1** |
| **Positivo del `LIKE`** | `fail_reason LIKE 'reverted:%'` -> **715**, exit=0 |
| **Marginal V2/V3** | 4/4 = **1,000000** (§2.2) — el instrumento se valida contra sí mismo |
| **Reproducción cruzada con t174** | V2 degradación `0,3136 %` a $1000 — **idéntica** |
| Externo `:9090` (defecto #9) | **`http_code=000`, exit=7** — **octava** aparición |

**Ningún control se pipeó a `head`** (el `$?` sería el de `head`). **Ninguna evidencia usa la URL externa. El fork NO se escribió** (todo `cast call` = `eth_call`).

---

## 10. LO QUE NO SE COMPUTA, CON SU RAZÓN

| No computado | Razón |
|---|---|
| **«Cuánto se podría ganar»** | **NO SE ESTIMA** — depende de frecuencia, de competencia y de flujo, y **no es computable con lo medido.** Se responde **una** pregunta binaria y se responde NO |
| Los 1 331 pools no barridos | Sin `tvl_usd` (1 292) ni reservas on-chain leídas (§7). **NO COMPUTADO, no cero** |
| Tasa de fallo de fondeo **general** | Se da la de §6 (`1/1046 = 0,0956 %`) **con su definición declarada**. La identidad pre-arreglo (`slot_unresolved + rpc_err = anvil`) **ya no cierra exacta** (1+1=2 ≠ 1046), así que la cifra es un **cociente declarado**, no una identidad |
| La muestra de simulación | **No se usó para nada**: esta tarea es de mecánica de pools, no de simulación |

---

## 11. RECOMENDACIÓN — SEPARADA DE LOS NÚMEROS

> **Se emite separada, como manda el contrato. Medir no autoriza.**

**Lo medido (confianza ALTA):** las fórmulas V2 y V3 validadas contra su propio precio marginal (4/4 = 1,000000) · la liquidez de los 5 pools leída on-chain · **3 de 20 pares con edge positivo antes de impacto** · **el máximo global de neto = `−$0,6475`** · el bruto cruza cero entre $100 y $200 · el pool se auto-arbitra en 3 operaciones · 0 operaciones con net>0 de 930 · la cotización de la mejor fila está ~10 832 bloques vieja.

**Lo NO medido:** los 1 331 pools restantes · el impacto real a tamaños ≥$5 000 (rango único) · «cuánto se podría ganar».

**Mi lectura, y es una lectura: con la detección que hay hoy, el sistema no puede pasar en ningún tamaño.** No es un problema de capital ni de configuración: **el gas ($0,676) es 24× el mejor bruto ($0,028) y no existe un tamaño que cierre la brecha, porque el tamaño que la cerraría ($4 164) está 18× más allá de donde el impacto ya mató el edge.**

**Qué haría falta medir ANTES de autorizar cualquier cambio — de target, de capital, de multiplicador de gas o de universo:**
1. **Barrer los 1 331 pools no barridos** — si en alguno hay un edge >0,35 % (para vencer fees) o >0,68 % del gas sobre tamaño pequeño, la conclusión cambia. **Es una tarea con forma propia** (lectura masiva de `getReserves`/`slot0`).
2. **Medir el flujo real**, no la foto: un edge que existe 1 bloque cada N y se cierra en 2 operaciones no es una estrategia, es una coincidencia. **Requiere ventana temporal, que esta tarea no tiene.**
3. **Revisar por qué la cotización está 10 832 bloques vieja** (§5) — un sistema que opera sobre estado expirado mide mal **incluso cuando mide bien**.

**Y el límite, respetado:** `capital_usd` sigue en `1000.00` y el target en `50.0`. **No se tocó nada.**

---

## 12. TRAZABILIDAD

- Leído: **`cast call` en el contenedor anvil** (`getReserves()`, `slot0()`, `liquidity()`, `fee()`, `token0()`, `token1()`, `block-number` — **todo `eth_call`, sin cambiar estado**), PostgreSQL por `docker exec … psql` (SELECT-only), Prometheus **loopback vía ssh**, `docker inspect`.
- **CERO escrituras** al motor, umbrales, configuración, **fork** o producción. **CERO reinicios, mainnet, firmas o broadcast.** **NO** se re-disparó el benchmark. **Paper.**
- Entrada heredada de t174 (PR #901, blob `be9616fe00219b3740b0cfe9479384c8bbd77834`); la fórmula V2 es la del motor (`size_optimizer.rs`, blob `a1411d8200d3baad5c912f2dcba036cc63e032b7`).
- **Dos errores propios cazados y corregidos antes de publicar**, los dos de unidades: (a) usé `fee=30` como denominador dando 3 % de fee en el V2 — **el 30 son basis points (3/1000 ⇒ ×997)**, y el tercer valor de `getReserves()` es `blockTimestampLast`, **no un fee**; (b) mezclé unidades humanas y **raw** en las fórmulas V3, dando salida nula. **Ambos se detectaron con la validación por precio marginal antes de reportar nada** — y (a) demuestra que **t174 tenía razón y este código nuevo estaba mal.**
- Este documento toca **solo** `docs/backend/`.

**Firma:** Backend · t175 · attempt `ceb5d01f-f468-4a93-8ce3-fb9d3e3c2dfc`
