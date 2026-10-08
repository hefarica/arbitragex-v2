# COST-COMPUTE-01 — ¿el borde de las 1.253 filas sobrevive a los costos reales?

**Respuesta: NO. Sobreviven `0` de `1.247`.** Y no es que el fee se lo coma: **el gas es 16,26×–17,00× el gross real**, sobre un notional medio de **5 cienmilésimas de dólar**. Además el borde **nunca fue un borde**: el ciclo registrado devuelve **759×–1.315×** su entrada, y su conversión a USD está inflada **327,53× (UNI)** / **194,57× (LINK)** porque el token de entrada se priceó al **precio de WETH**.

**Cápsula ORGDNA:** `run_id=arbx-entrega-20261008` · `phase_id=implementation` · **Dueño:** Data · **Intento:** `34021d64-f4fe-4c96-bcc8-a26558c87a03`
**Alcance:** chain 1 · **solo `SELECT`** · **CERO cambios de umbrales, configuración o escrituras en el motor** · **In scope:** `docs/data/`

---

## 0. Control de instrumento

| control | comando | salida literal | instante |
|---|---|---|---|
| **canal VPS→Postgres** | `psql -U postgres -d arbitragex -tAc "SELECT 1"` | **`1`** — antes de CADA cifra | 07:21:24Z · 07:23:58Z · 07:25:29Z · 07:27:13Z · 07:28:28Z · 07:29:50Z · 07:30:28Z |

**Ventana declarada, idéntica en todas las consultas:** `detected_at > now() - interval '24 hours'`, evaluada en cada instante listado arriba (2026-10-08). La ventana es **móvil**: por eso cada cifra lleva su hora.

**Dos defectos de instrumento MÍOS, declarados:**
1. `round(double precision, integer)` **no existe** en Postgres → dos consultas fallaron hasta castear `power(10::numeric, dec)`. El error se ve en la salida, no se ocultó.
2. Una consulta de `trading_config` salió **VACÍA** por concatenar un `NULL` (en Postgres `'a'||NULL` = `NULL`): no era "no hay configuración", era mi query. Re-hecha con `COALESCE`.

---

## 1. La re-medición de las 1.253

```sql
SELECT 'n='||count(*)||' max_net='||max(net_expected_profit_usd)||' max_gross='||max(expected_profit_usd)
    ||' min_net='||min(net_expected_profit_usd)
FROM opportunities
WHERE detected_at > now() - interval '24 hours'
  AND rejection_reason='StrategyDisabled:triangular_arb' AND net_expected_profit_usd>0;
-- n=1248 max_net=12.490223 max_gross=13.14359056 min_net=6.934752     (07:21:24Z)
```

| cifra | t123 | re-medido (07:21:24Z) | diferencia |
|---|---|---|---|
| filas con `net>0` que mueren por `StrategyDisabled:triangular_arb` | **1 253** | **1 248** | **−5** |
| `max net` | 12,490223 | **12,490223** | **0** ✔ |
| `max gross` | 13,143591 | **13,14359056** | **0** ✔ |
| `net` mínimo | — | **6,934752** | nueva |
| filas totales con `net>0` en la ventana | 1 453 | **1 448** | **−5** |

**La diferencia ES parte del entregable y es −5 en ambas puntas:** cayeron 5 filas con `net>0` y las 5 que cayeron eran de este bucket. Ventana móvil de 24 h ⇒ el conteo baja a medida que el borde de la ventana se corre. **Los dos extremos (max net, max gross) coinciden exacto con t123**, lo que confirma que se midió el mismo objeto.

**Distribución de las 1 248** (07:23:58Z): `net` min **6,934752** · mediana **7,009377** · max **12,490223**; `gross` min **7,58085355** · mediana **7,660036** · max **13,14359056**; **`roi_pct` no nulo = 0**.

**Quién las produjo** (dato que t123 no tenía y que decide la §3): `strategy=triangular`, **`detector=hop_cycle_bridge`**, `cartridge_id=(null)`, **n=1248**. Son **ciclos de 5 y 6 patas**, con `dex_adapters` mezclando **V3 y V2** (`["uniswap-v3","uniswap-v3","uniswap-v3","uniswap-v2","uniswap-v2"]` en la muestra).

---

## 2. El 0/0 de t123: **CONFIRMADO**

```sql
SELECT 'computed='||count(*) FILTER (WHERE economics->>'computation_status'='computed')
    ||' net_profit_no_null='||count(*) FILTER (WHERE economics->>'net_profit_usd' IS NOT NULL)
    ||' con_economics='||count(*) FILTER (WHERE economics IS NOT NULL)||' total='||count(*)
FROM opportunities WHERE detected_at > now() - interval '24 hours'
  AND rejection_reason='StrategyDisabled:triangular_arb' AND net_expected_profit_usd>0;
-- computed=0 net_profit_no_null=0 con_economics=1248 total=1248     (07:21:24Z)
```

**`computed=0` · `net_profit_no_null=0` · `con_economics=1248`.** Las 1 248 TIENEN objeto `economics`, y **ninguna** tiene un solo componente de costo calculado:

```json
{"legs": [], "gas_usd": null, "roi_pct": null, "bribe_usd": null, "quote_block": 26142549,
 "dex_fees_usd": null, "error_reason": "StrategyDisabled:triangular_arb", "meets_target": null,
 "slippage_usd": null, "amount_in_usd": null, "amount_in_wei": null, "flash_fee_usd": null,
 "amount_out_usd": null, "amount_out_wei": null, "net_profit_usd": null, "target_net_usd": null,
 "total_cost_usd": null, "other_costs_usd": null, "gross_profit_usd": null,
 "simulation_block": null, "target_delta_usd": null, "computation_status": "error",
 "not_computed_reasons": {}}
```

`computation_status = "error"` con `error_reason = "StrategyDisabled:triangular_arb"`: el productor escribió **el motivo del rechazo como motivo de no-cómputo**, y `not_computed_reasons` está **vacío** (`{}`) — el contrato que exige explicar cada `null` **no se cumple por este camino**. `legs: []` vacío: ni siquiera la descomposición por pata.

---

## 3. ★ El confundidor: el `unwrap_or(30)` — **NO entra en estas filas (0 %)**

t123 lo citó como `backend/dex_engine.rs:439-440`. La ruta real y literal:

```rust
// backend/searcher-rs/src/engines/dex_engine.rs:439-440
let fee_a = pool_a.fee_bps.unwrap_or(30);
let fee_b = pool_b.fee_bps.unwrap_or(30);
```

**Pero el productor de estas 1 248 filas es `hop_cycle_bridge`, no el DEX engine.** Y su propio código, en el punto donde construye la pata de ruta, dice lo contrario:

```rust
// backend/searcher-rs/src/route_discovery/hop_cycle_bridge.rs:796-799
// The intent's own fee tier (`None` when the producer did not know it).
// The N-leg cycle kernel prices its own fee constant; nothing is
// defaulted here.
fee_bps: leg.fee_bps,
```

⇒ **Proporción de filas de este conjunto donde entra el `unwrap_or(30)` de `dex_engine.rs`: `0 %`.** El default del DEX engine vive en **otro productor**, y para esta población **no está en el camino**.

**Pero la conclusión NO es "entonces el fee es el real", y esto es lo que hay que declarar en su lugar:** hay **tres** fuentes de fee distintas en el sistema y una de ellas sí es un default por sustancia:

| fuente | valor | ¿real o cota? |
|---|---|---|
| `dex_engine.rs:439-440` `unwrap_or(30)` | 30 bps | **NO aplica** (0 % de estas filas) |
| kernel de ciclo N-leg (`hop_cycle_bridge`) | **su propia constante de fee** | **COTA para las patas V3** |
| `trading_config.lp_fee_default_pct` | **0,0030 = 30 bps** | el default del sistema |

**Por qué es COTA:** los ciclos tienen **patas V3** (3 de 5 o 4 de 6), y en V3 el fee real por pool es **5, 30 o 100 bps** según el tier. El kernel usa **una constante**, no el tier de cada pool. Sobre las patas V2 (30 bps canónico) la constante es **exacta**; sobre las V3 es **un default en sustancia**. Por lo tanto **cualquier cifra en bps de este camino es una COTA en el eje fee**, y así se reporta.

**Y el `lp_fee_default_pct = 0,0030` es, en el papel, el mismo 30 bps del confundidor.** El confundidor de t123 **existe en el sistema**; lo que se corrige es **dónde**: no en la ruta de estas 1 248 filas.

---

## 4. El cómputo de costos y si el borde sobrevive

**Qué corrí exactamente (declarado, sin escribir nada):** **NO re-ejecuté el motor** (eso podría escribir y estaba prohibido) y **no inventé un costo**. Leí la fórmula del propio productor y la **re-apliqué sobre los datos registrados** con **los precios y parámetros del propio sistema**:

- Fórmula del productor (`searcher-rs/src/workers/triangular_worker.rs:1599-1607`, la que documenta la misma convención para el camino del worker): **`net = gross − gas_cost_usd`**. Es decir: **el `net>0` de t123 ya tiene el gas restado, y NO tiene nada más restado.**
- Precios: `trading_config.token_prices_usd` de chain 1 → **`UNI: 7.84` · `LINK: 13.17`**.
- Parámetros de costo de `trading_config` (07:27:47Z, `WHERE chain_id=1`): `min_profit_usd=50.0000` · `max_slippage_pct=0.0050` · **`flashloan_fee_pct=0.0009`** · **`lp_fee_default_pct=0.0030`** · `gas_estimate_units=250000` · `gas_price_strategy=dynamic_basefee_plus_tip` · `failure_risk_buffer_pct=0.0004` · `capital_usd=1000.00`.

### El notional, medido

El `route_metadata` de cada fila trae las patas reales: `leg_amounts_in`, `leg_amounts_out`, `decimals`, `token_addresses`. Con eso:

| medición (n=1247 con `route_metadata`) | valor | instante |
|---|---|---|
| **notional** (USD, al precio configurado) | min **0,0000304425** · max **0,0000518050** · **media 0,00005027** | 07:28:28Z / 07:30:28Z |
| **gross real** (USD, al precio configurado) | media **0,03936725** · max **0,040051** | ídem |
| **gas restado** (`gross − net` registrados) | media **0,650701** | ídem |

**El notional medio es de 5 cienmilésimas de dólar.** Contra un `min_profit_usd` de **50**, el mejor gross real (**0,040051**) está **1 248× por debajo** de la propia puerta de rentabilidad del sistema.

### La aritmética, por par, contra el hurdle de 59,91 bps

**Distribución del neto real** (`gross_real − gas`), n=1247, **no un promedio**:

| mínimo | mediana | máximo |
|---|---|---|
| **−0,62562409** | **−0,61126743** | **−0,60613566** |

**`sobreviven_a_gas = 0` · `mueren_por_gas = 1247` · `total = 1247`.**

**Ratio gas / gross real:** min **16,26×** · mediana **16,55×** · max **17,00×**.

**Cuántas cruzan el hurdle de 59,91 bps:** la pregunta se responde **en los dos ejes, porque el resultado difiere y mezclarlos sería el error**:

- **En dólares (el eje que decide una ejecución): `0` de `1247`.** Todas pierden entre 61 y 63 centavos.
- **En tasa (bps del notional): el ciclo *parece* cruzar el hurdle por 5 órdenes de magnitud** — pero ese número **no es una tasa de arbitraje, es el fantasma de la §5**. Un hurdle de 59,91 bps es un umbral de **tasa**; compararlo con una tasa fantasma da un "sí" vacío.

**★ El fee NO es lo que se come el borde.** El `flashloan_fee_pct = 0,0009` (9 bps) aplicado al notional medio de 0,00005027 USD vale **≈0,0000000452 USD** — **ocho órdenes de magnitud por debajo** del gas. **Lo que se come el borde es el gas (16,5×), sobre un notional de polvo.** La hipótesis de la cápsula ("o se lo come el fee") **no se sostiene**: el fee es irrelevante a este tamaño; el gas es el que decide.

---

## 5. ★ Por qué el borde **nunca fue un borde**: dos defectos aguas arriba, medidos

### (a) El ciclo es un fantasma: devuelve **759×–1.315×** su entrada

`route_metadata` trae las patas reales, así que el retorno del ciclo es **medible sin precios**:

```sql
(retorno) = (leg_amounts_out[-1]) / (leg_amounts_in[0])
```

| medición (n=1247) | valor |
|---|---|
| retorno mínimo | **759,0854×** |
| retorno mediana | **767,0036×** |
| retorno máximo | **1.315,3591×** |
| **retorno > 10×** | **1247 de 1247** |
| retorno ≤ 1× | **0** |

**Ningún ciclo de arbitraje devuelve 700 veces su entrada.** Los ciclos son de 5–6 patas y las muestras van de `LINK→USDT→USDC→PEPE→WETH→LINK` a `UNI→…→WETH→UNI`. Esto es la clase de defecto que el propio repo ya bautizó en `dex_engine.rs:441-453` ("the phantom-positive case"): el math kernel devuelve un positivo que el router no reproduce.

`amount_in_wei` (la columna) es **idéntico** a `route_metadata.economics_amount_in_wei` en **1247/1247** ⇒ el borde y el tamaño registrado hablan del **mismo** trade, y ese trade es imposible.

### (b) El gross en USD está inflado ~195×–328×: **el token de entrada se priceó al precio de WETH**

El precio implícito sale de `gross_usd / ganancia_en_tokens`:

| token de entrada | n | precio **configurado** | precio **implícito** (mediana) | **ratio** |
|---|---|---|---|---|
| `0x1f9840a8…` (**UNI**) | 67 | **7,84** | **2 567,85** | **327,53×** |
| `0x51491077…` (**LINK**) | 1 180 | **13,17** | **2 562,44** | **194,57×** |

**★ Los dos tokens dan el MISMO precio implícito (~2.562–2.568 USD), y ese número es el precio de WETH:** `token_prices_usd` dice `"WETH": 2569.6407241870997`.

⇒ **El conversor a USD priceó el token de entrada (UNI, LINK) al precio de WETH, no al suyo.** Los ciclos **pasan por WETH** (`token_addresses` termina en `…c02aaa…`), y el conversor tomó el precio de esa pata en lugar del token de entrada. Un factor de 194×–328× en la única cifra en dólares del borde.

### El efecto combinado

`gross_usd` registrado = fantasma × precio equivocado. La cifra **12,490223 USD** de t123 **no sobrevive a ninguna lectura**: al precio correcto el gross real máximo es **0,040051 USD** y el gas (**0,650701**) lo supera **16,5×**.

---

## 6. Qué significa para t130 (y qué NO)

**t130 haría que estas oportunidades dejen de morir por nombre y pasen a ser evaluadas. La respuesta medida es que evaluarlas NO destrabaría dinero de esta población:** las 1 247 mueren por gas, con un margen de **16,5×**, y su mejor gross real está **1 248× por debajo** del `min_profit_usd = 50` que el propio sistema exige.

**Lo que NO se concluye:** que t130 no valga. Sigue siendo el arreglo correcto de **identidad** (`StrategyDisabled` es una etiqueta de configuración, no un veredicto económico) y **sí** destraba el cómputo de costos — pero **con estos datos el cómputo daría negativo**. La prioridad por razones **económicas** no se sostiene con esta población; la prioridad por **corrección** sí.

**Y queda un hallazgo más caro que el de t130, y es de otra tarea:** el fantasma de 767× y el precio de WETH aplicado a UNI/LINK. Eso sí puede estar inflando bordes **en todas las poblaciones** que pasen por este camino, no sólo en las que mueren por nombre.

---

## 7. Límites y puntos NO COMPUTADOS

| # | Qué NO está computado | Por qué (medido) | Qué lo cerraría |
|---|---|---|---|
| 1 | **Slippage ejecutable** | No se re-ejecutó el motor; el impacto por tamaño del CPMM está implícito en el kernel pero **no lo medí**. El registro sólo trae `max_slippage_pct = 0,0050` como parámetro, no como costo aplicado | Re-ejecución del kernel a tamaños declarados, o `getAmountOut` on-chain sobre las patas reales |
| 2 | **Fee real por pata V3** | El kernel usa **su constante**; las patas V3 tienen tier real 5/30/100 bps. **El borde en bps es COTA, no valor** | El tier efectivo por pool (ya está en `pools.fee_tier`) inyectado al kernel |
| 3 | **Precio por token en el conversor a USD** | Medido **que** el implícito es el de WETH (`~2565` para UNI y LINK), **no** la línea exacta del conversor que lo hace | El sitio del conversor que resuelve el precio del token de entrada |
| 4 | **El fantasma de 759×–1.315×** | Medido **que** ocurre en 1247/1247 y **que** el kernel ya tiene un guardia para ese caso (`dex_engine.rs:441-453`), **no** por qué atraviesa este camino | Traza del kernel sobre **una** fila (los `pool_addresses` y `leg_amounts_*` están todos en `route_metadata`) |
| 5 | **Filas sin `route_metadata`** | 1248 filas, **1247** con ratio computable: **1 fila** no aportó los campos | Esa fila suelta |

---

## 8. Método y reproducción

```sql
-- canal primero
SELECT 1;
-- la ventana y el bucket
SELECT count(*) FROM opportunities WHERE detected_at > now() - interval '24 hours'
  AND rejection_reason='StrategyDisabled:triangular_arb' AND net_expected_profit_usd>0;
-- el 0/0
SELECT count(*) FILTER (WHERE economics->>'computation_status'='computed'),
       count(*) FILTER (WHERE economics->>'net_profit_usd' IS NOT NULL), count(*)
FROM opportunities WHERE detected_at > now() - interval '24 hours'
  AND rejection_reason='StrategyDisabled:triangular_arb' AND net_expected_profit_usd>0;
-- el retorno del ciclo, sin precios
SELECT (route_metadata->'leg_amounts_out'->>-1)::numeric / NULLIF((route_metadata->'leg_amounts_in'->>0)::numeric,0)
FROM opportunities WHERE ...;
```
```bash
# el confundidor, con su linea
git grep -n 'unwrap_or(30)' -- backend/searcher-rs/src/engines/dex_engine.rs
# el productor y su fee
sed -n '796,799p' backend/searcher-rs/src/route_discovery/hop_cycle_bridge.rs
```
**No se ejecutó ninguna escritura**: todo `SELECT`, más lectura de código. No se re-ejecutó el motor.

---

## 9. Integridad

`sha256` declarado **en el cierre de t137** (un archivo no puede contener su propio hash). Verificación **por blob de git**, nunca con `Out-File` (re-codifica y mete CRLF):

```bash
git hash-object docs/data/COST-COMPUTE-01.md
git rev-parse HEAD:docs/data/COST-COMPUTE-01.md
```

---

*La pregunta era si el borde sobrevive a los costos. La respuesta es que **no hay borde al que sobrevivir**: el ciclo devuelve 767× lo que pone, la cifra en dólares está inflada 195×–328× porque priceó UNI y LINK al precio de WETH, y el gas —16,5× el gross real— termina de cerrarlo sobre un notional de 5 cienmilésimas de dólar. El fee no tuvo nada que ver: es ocho órdenes de magnitud más chico que el gas. **Un resultado negativo vale igual**, y éste además dice dónde mirar: no en el gate que las rechaza, sino en el kernel que las inventa.*
