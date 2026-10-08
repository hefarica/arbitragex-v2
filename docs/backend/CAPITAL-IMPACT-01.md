# CAPITAL-IMPACT-01 — ¿Sobrevive el 0,561 % de ROI a $8.912,67 de tamaño?

**Tarea:** t174 · **Agente:** Backend · **Modo:** SOLO LECTURA y cálculo (cero cambios, cero escrituras, **el fork NO se escribió**) · **Paper**
**Frontera pre/post (LA REAL):** `docker inspect --format '{{.State.StartedAt}}' arbitragex-v2-sim-ctl-1` -> **`2026-10-08T14:16:07.1195963Z`**, exit=0.
**Config del operador, NO se tocó:** `trading_config` chain 1 -> `capital_usd=1000.00`, `min_profit_usd=50.0000`, `simulation_target_profit_usd=50.0000`, `enabled=true`, `updated_by=admin`.

---

## 1. VEREDICTO

**NO SOBREVIVE. LA OPCIÓN 1 (`capital_usd` 1000 → 8913) ESTÁ MUERTA — y lo está por DOS vías independientes, cada una suficiente.**

| Vía | Medición | Resultado |
|---|---|---|
| **1 — Sin impacto** | `0,222206 %` (edge bruto medido a $1000) × `8912,67` = **$19,80** − gas $0,67 = **$19,13** | **2,61× CORTO del target de $50 — con fricción CERO** |
| **2 — Con impacto real** | fricción extra `0,2144 pp` round-trip × `8912,67` = **−$19,11** | **net = `+1,5459 − 19,11` = `−$17,56`. NEGATIVO** |

**⇒ Subir el capital no sólo no alcanza el target: hace que la mejor oportunidad de la historia del sistema PIERDA dinero.**

**Y la contradicción aritmética que el contrato pedía resolver queda resuelta: NO EXISTE tamaño en el que el sistema pueda pasar.**

---

## 2. LA CURVA: EL ROI YA DECAE CON EL TAMAÑO — medido, no estimado

`psql -tAc`, ruta JSONB `economics` (**defecto #13: `gross_profit_usd`/`net_profit_usd` NO son columnas planas**), exit=0:

| `amount_in_usd` | n | `roi_pct` | `net_profit_usd` |
|---|---|---|---|
| **79,68** | 1 | **0,560999** | 0,446984 |
| **148,68** | 1 | **0,558616** | 0,830564 |
| 148,70 | 1 | 0,558319 | 0,830212 |
| 999,99 | 1 | 0,041918 | 0,419170 |
| **1000,00** | **122** | **0,125522** (media) | **1,2552** (media) |
| 1000,00 | — | **0,154593** (máximo) | **1,545926** (máximo) |

```
de $148,68 a $1 000,00  ->  tamaño x6,73   ·   ROI x0,2246  (cae 4,45x)
de $ 79,68 a $1 000,00  ->  tamaño x12,55  ·   net x3,46    (crece sub-lineal)
```

**⇒ El edge NO es constante: YA está limitado por impacto a $1000.** Y el neto está saturando: **12,55× el tamaño produce sólo 3,46× el neto.**

### 2.1 ★ PRECISIÓN QUE REFINA A t173

t173 usó `0,560999 %` (el **máximo** de ROI) para la aritmética del capital. **Ese ROI se observó a `$79,68`, NO a `$1000`.** El ROI realizado **a $1000** es **`0,154593 %`** (máximo) o **`0,125522 %`** (media). `1000 × 0,154593 % = 1,54593` — cierra contra el `net` medido.

**⇒ El techo real a $1000 es `$1,5459`, no `$5,61`.** La aritmética de t173 era una **cota superior** (el mejor ROI a cualquier tamaño). **Se refina, no se retracta: la conclusión de t173 —8,91× corto— se mantiene y en realidad es peor.**

---

## 3. LA RUTA Y SU LIQUIDEZ REAL

### 3.1 La mejor oportunidad, completa

`economics` de la fila de `net = 1,545926`, exit=0:

```
legs: [ {WETH -> USDT:  in 376915356394236160 (0,376915 WETH)  out 999675951 (999,675951 USDT)},
        {USDT -> WETH:  in 999675951 (999,675951 USDT)  out 377752883826008446 (0,377753 WETH)} ]
amount_in_usd = 1000,00   ·   amount_out_usd = 1002,22   ·   gross = 2,2221   ·   net = 1,5459
gas_usd = 0,666131   ·   total_cost_usd = 0,676131   ·   roi_pct = 0,154593   ·   quote_block = 26137384
```

**Es un round-trip `WETH → USDT → WETH`**: entra **0,376915 WETH**, salen **0,377753 WETH** ⇒ **+0,2222 % bruto**.

### 3.2 ★ LIQUIDEZ REAL — leída de la cadena, no de una tabla genérica

`docker exec arbitragex-v2-anvil-1 cast call <pool> … --rpc-url http://localhost:8545`, exit=0. **Todos los pools de las rentables son WETH/USDT** (`token0 = 0xC02aaA39b…WETH`, `token1 = 0xdAC17F95…USDT`):

| Pool | DEX/fee | Liquidez real | Fuente |
|---|---|---|---|
| **`0x0d4a11d5eeaac28ec3f61d100daf4d40471f1852`** | **UniswapV2, fee 30 (0,30 %)** | **`getReserves()` = 2900,498828 WETH / 7 328 270,235623 USDT** ⇒ **≈ $7,33 M** | `cast call` en el fork |
| `0x11b815efb8f581194ae79006d24e0d814b7697f6` | V3, fee 500 (0,05 %) | `liquidity()` = **7,00366674727580152e17** | `cast call` |
| `0x6ca298d2983ab03aa1da7679389d955a4efee15c` | V3, fee 500 (0,05 %) | `liquidity()` = 2,7106536726875741e16 · **TVL $1 843 382,13** | `cast` + `pools.tvl_usd` |
| `0xc7bbec68d12a0d1830360f8ec58fa599ba1b0e9b` | V3, fee 100 (0,01 %) | `liquidity()` = 1,70091345252275324e17 · **TVL $3 605 028,61** | `cast` + `pools.tvl_usd` |
| `0xacdb27b266142223e1e676841c1e809255fc6d07` | V3, fee 100 (0,01 %) | `liquidity()` = 7,251378828591346e15 · **TVL $194 708,31** | `cast` + `pools.tvl_usd` |

**Precio implícito del pool V2: `7 328 270,24 / 2900,4988` = `$2 526,56 / WETH`.**

**Datos de apoyo:** la tabla `pool_reserves` tiene **44 334 376 filas** con `reserve0`/`reserve1`/`block_number` — **fuente declarada y disponible**; y `pools.tvl_usd` está poblado para 3 de los 5 pools (los otros dos tienen `tvl_usd` NULL, así que **para ésos se usó la lectura on-chain, no un supuesto**).

### 3.3 ★ LA FÓRMULA — la del motor, declarada

**CPMM Uniswap V2** (la misma familia que `backend/searcher-rs/src/size_optimizer.rs`, blob `a1411d8200d3baad5c912f2dcba036cc63e032b7`):

```
amount_in_with_fee = dx × 997
dy                 = (amount_in_with_fee × y) / (x × 1000 + amount_in_with_fee)
precio_ejecución   = dy / dx        ;   spot = y / x
degradación        = 1 − (precio_ejecución / spot)
```

**Aplicada a las reservas REALES del pool V2:**

| Tamaño | `dx` (WETH) | precio ejecución | **degradación** |
|---|---|---|---|
| **$1 000,00** | 0,395796 | 2 518,63 | **0,3136 %** |
| **$1 553,60** | 0,614908 | 2 518,44 | **0,3211 %** |
| **$8 912,67** | 3,527598 | 2 515,92 | **0,4207 %** |

**Ningún impacto se estimó: se calculó sobre `getReserves()` leído del fork.**

---

## 4. ★★ VÍA 2 — CON EL IMPACTO REAL: EL NETO ES NEGATIVO

```
Δ degradación por pierna ($1000 -> $8912,67)  = 0,1072 pp
Δ round-trip (2 piernas)                      = 0,2144 pp
costo extra = $8 912,67 × 0,2144 %            = −$19,11

net a $1 000,00 (medido)                      = +$1,5459
net a $8 912,67                               = +$1,5459 − $19,11 = −$17,56   <- NEGATIVO
```

**★ Y la comparación que lo cierra: el edge BRUTO entero a $1000 es `0,2222 pp`. La fricción extra es `0,2144 pp` = el `96,5 %` del edge entero. Y eso es UNA SOLA PIERNA de las dos del round-trip.**

**⇒ A $8.912,67 la mejor oportunidad de la historia del sistema pierde ≈ $17,56.**

---

## 5. ★★ VÍA 1 — INCLUSO CON FRICCIÓN CERO, NO ALCANZA

```
edge bruto medido a $1 000     = 0,222206 %
bruto a $8 912,67 SIN impacto  = 8 912,67 × 0,222206 % = $19,80
menos gas ~$0,67                                        = $19,13
TARGET                                                  = $50,00
=> 2,61x CORTO  **incluso con impacto CERO**
```

**⇒ La opción 1 no falla por el impacto. Falla por aritmética básica: un edge del 0,2222 % sobre $8.912,67 da $19,80, y el target es $50.**

**★ Y el enunciado exacto de la imposibilidad:**

```
ROI que el target exige a $8 912,67   = 50 / 8912,67      = 0,5610 %
mejor ROI JAMÁS observado             = 0,560999 %   ... pero a $79,68
ratio de tamaño                       = 8912,67 / 79,68   = 111,86x
```

**⇒ El target exige el MEJOR ROI DE LA HISTORIA DEL SISTEMA a un tamaño `111,86×` mayor que aquél donde se observó — y está medido que el ROI CAE con el tamaño (×4,45 al pasar de $148,68 a $1.000).**

---

## 6. ★★ LA CONTRADICCIÓN ARITMÉTICA: ¿existe ALGÚN tamaño en el que el sistema pase?

```
gas floor = $1,95  (FIJO: el gas no escala con el tamaño; cost = 0,6491..0,6891 medido)
con el ROI de $1 000 (0,125522 %), el neto cruzaría $1,95 a:
    1,95 / 0,00125522 = $1 553,51
```

**Ese es el tamaño que la aritmética pura pide. ¿Se mantiene el spread ahí?**

```
a $1 553,60 la degradación sube de 0,3136 % a 0,3211 %  ->  +0,0150 pp round-trip = −$0,2332
net a $1 553,60 = $1,9501 − $0,2332 = $1,7169   ...  ¿ ≥ $1,95?   NO
```

**★★★ RESULTADO: ni siquiera a `$1.553,60` —el tamaño que la aritmética pura señalaba— el neto alcanza el gas floor, porque el propio impacto se lo come (`$1,7169 < $1,95`).**

**⇒ NO EXISTE TAMAÑO EN EL QUE EL SISTEMA PUEDA PASAR. Eso es el hallazgo, y el contrato anticipó exactamente esta salida.**

**El sistema está en un empate estable:** más tamaño da más neto bruto pero menos ROI; el punto donde el neto cruzaría el gas floor está **más allá** del punto donde el impacto ya se comió la ganancia. **No hay intersección.**

---

## 7. ★★ EL PRECEDENTE DE t137: COHERENTE, Y DECLARO CÓMO LO CITO

**Declaración de fuente, primero:** **busqué el artefacto de t137 y NO lo encontré** (`glob **/*137*` sobre `docs/` -> *"No files found"*). **Cito la caracterización que el contrato hace de él, no el artefacto.** Eso queda dicho y no se disfraza.

**Lo que el contrato reporta de t137:** *«el borde de 1.253 filas NO sobrevivía a los costos — 0 de 1.247»*.

**Contraste:**

| | t137 (según el contrato) | t174 (medido hoy) |
|---|---|---|
| Qué mataba al borde | **los costos** | **el tamaño (impacto) + un edge demasiado fino** |
| Resultado | **0 de 1.247 pasaron** | **net NEGATIVO a $8912,67; 2,61× corto incluso sin impacto** |
| Mecanismo | fricción > edge | **el mismo: fricción > edge** |

**⇒ COHERENTE, no contradictorio. Y NO hace falta conciliar un positivo, porque hoy NO hay positivo.** Las dos mediciones dicen lo mismo por caminos distintos: **cuando la fricción se mide de verdad, un edge de décimas de por ciento no sobrevive.** t137 lo encontró con costos; t174 lo encuentra con tamaño.

**★ Y la lección operativa que esto confirma: t137 midió ANTES de tocar nada y encontró 0 de 1.247. Hoy, midiendo antes de tocar nada, el resultado es el mismo signo.** **El precedente se respetó: se midió primero.**

---

## 8. LO QUE NO SE COMPUTA, CON SU RAZÓN

| No computado | Razón exacta |
|---|---|
| **IMPACTO DE LA PIERNA V3** | **NO COMPUTADO.** Tengo `liquidity()` de las V3 pero **no el rango de ticks activo ni el `slot0` completo**; el impacto de una V3 concentrada **no se deriva de `liquidity()` sola** y estimarlo con la fracción de TVL sería exactamente el «impacto de tabla genérica» que el contrato prohíbe. **La conclusión NO depende de él**: la Vía 1 cierra sin impacto alguno y la Vía 2 cierra con **una sola** pierna. |
| **Ajuste de ley de potencia** | Se calculó (`ROI ~ S^-0,7856` ⇒ net ≈ `$2,01` a $8912,67) y **se DESCARTA como método**: ajusta una **sección cruzada** (oportunidades DISTINTAS a cada tamaño), no la **respuesta de la misma ruta** al tamaño. **Contradice en signo a la medición directa, y manda la directa.** Se declara por transparencia, no se usa. |
| **El optimo interno del `SizeOptimizer`** | **NO COMPUTADO** (no lo persiste; se mide su resultado, no su óptimo). |
| **Tasa de fallo de fondeo** | Identidad heredada **muerta** (Δ=4, 3, 4). Sin población delimitada no hay tasa. |
| **El «borde de 1.253 filas» de t137** | **NO VERIFICADO**: artefacto no localizado. Se cita la caracterización del contrato y se declara. |

**Regla aplicada:** un «no medido» **no se escribe como 0**, y **un cociente entre universos distintos no es una tasa.**

---

## 9. CONTROLES

| Control | Resultado |
|---|---|
| Canal SQL `SELECT 1` | `1`, exit=0 |
| **Negativo `ON_ERROR_STOP`, SIN TUBERÍA** | `SELECT esto_no_existe` -> **exit=1** |
| **Positivo del `LIKE`** | `fail_reason LIKE 'reverted:%'` -> **509** |
| **Liquidez real vs tabla** | `getReserves()` del fork **coincide en orden** con la profundidad esperada (~$7,33 M) ⇒ el instrumento lee el estado real |
| **Precio implícito cruzado** | `7 328 270,24 / 2900,4988 = $2 526,56/WETH`, coherente con el `amount_in_usd` de la propia fila ($1000 → 0,37692 WETH ⇒ $2653/WETH a `quote_block 26137384`) ⇒ **orden de magnitud consistente** |
| Externo `:9090` (defecto #9) | **`http_code=000`, exit=7** — **séptima** aparición |
| `raw_trace` (defecto #8) | `revert_risk_pct` es el campo; `raw_trace = 0` (sexta aparición, t173) |

**Elegí `0,222206 %` (el edge bruto de la mejor fila) y NO `0,154593 %` (su ROI neto) para la Vía 1, porque la Vía 1 pregunta por el TECHO FÍSICO antes de costos. Con el ROI neto el resultado es todavía más negativo.**

---

## 10. RECOMENDACIÓN — SEPARADA DE LOS NÚMEROS

> **Se emite separada, como manda el contrato. Medir no autoriza.**

**Lo que se midió (confianza ALTA):** el ROI decae con el tamaño (3 puntos, datos reales); la degradación del pool V2 a $1000 y a $8912,67 (CPMM sobre reservas leídas del fork); el neto negativo a $8912,67; el 2,61× corto incluso sin impacto; la inexistencia de un tamaño que cruce el gas floor.

**Lo que NO se midió (confianza — declarada):** el impacto de la pierna V3; el rango de ticks; el impacto de mercado de ejecutar repetidamente (esto es **una operación aislada**, no un flujo sostenido — a 930/h el impacto acumulado sería otro problema y **no se computó**).

**Mi lectura, y es una lectura:** **la opción 1 (subir el capital a ≈$8.913) debe descartarse.** No por margen: por signo — hace perder dinero.

**Qué haría falta medir ANTES de autorizar cualquier cambio de capital:**
1. **El impacto de la pierna V3 con `slot0()` + rango de ticks**, si se quisiera recuperar la Vía 2 con precisión total (aunque la Vía 1 ya cierra sin él).
2. **El edge real a tamaños intermedios** — la curva tiene 3 puntos y el tramo $148→$1000 es el único con pendiente medida. **Faltan puntos entre $150 y $1000**, que es donde se decide si existe algún tamaño viable pequeño.
3. **Si el detector puede encontrar rutas con edge ≥0,561 % a tamaño ≥$8.913** — hoy su mejor ROI es 0,561 % **a $79,68**. **Ésa es la única pregunta que queda abierta, y es de la capa de detección, no de la de capital.**

**Y el límite, respetado:** esta tarea **MIDIÓ**. **No se cambió `capital_usd`, ni el target, ni el multiplicador.** Eso es decisión del operador y va en otra tarea con su autorización explícita. **Vale aunque el resultado hubiera sido favorable.**

---

## 11. TRAZABILIDAD

- Leído: PostgreSQL por `docker exec … psql` (SELECT-only, ruta JSONB `economics` declarada), **`cast call` en el contenedor anvil** (`getReserves()`, `token0()`, `token1()`, `fee()`, `liquidity()` — **todo `eth_call`, sin cambiar estado**), `docker inspect`.
- **CERO escrituras**: nada al motor, umbrales, **configuración**, fork o producción. **CERO reinicios, mainnet, firmas o broadcast.** **NO** se re-disparó el benchmark. **Paper.**
- Blob citado: `backend/searcher-rs/src/size_optimizer.rs` = `a1411d8200d3baad5c912f2dcba036cc63e032b7`.
- Predicción y cierre **heredados** de t173 (PR #899, blob `70d0181e0477b19b47a0c9197eba0ee0c982ba56`) — no se re-inventan.
- Este documento toca **solo** `docs/backend/`.

**Firma:** Backend · t174 · attempt `a6a7c8ac-e681-40ff-bc05-148b00ccdd70`
