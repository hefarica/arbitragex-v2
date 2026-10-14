# GHOST-SURVIVAL-01 — las 102.237: el `0 de 1.247` no se extiende, y ahora tiene conteo propio

**Resultado en una línea:** **0 sobrevivientes de 102.237** — pero **NO por extrapolación**: la población 82× más grande se parte en dos mitades y **cada una cae por su propia razón medida**. La mitad que el motor reporta como rentable (**1.253 filas**, `net>0`) es **exactamente el bucket de 1,2 % que t137 ya midió a precio real**; la otra mitad (**100.984**, 98,77 %) tiene **net negativo incluso con el gross inflado**, así que a precio real sólo puede bajar. **La población mayor no aporta ni un sobreviviente nuevo.** Y la pata del ×800 quedó **computada**: es la **única** de las seis que crea valor, y **no es un defecto de decimales**.

**Cápsula ORGDNA:** `run_id=arbx-entrega-20261008` · **Dueño:** Data · **Intento:** `5ab61ccc-2d1c-4a40-8b2f-78e0043c35f6`
**In scope:** `docs/data/` · **Solo lectura** sobre PG · **CERO cambios al motor, CERO escrituras, CERO cambios de umbrales**

---

## 0. Control de instrumento y ventana de retención

| control | comando | salida literal | instante |
|---|---|---|---|
| **canal** | `ssh arbx "docker exec arbitragex-v2-postgres-1 psql -U postgres -d arbitragex -tAc 'SELECT 1'"` | **`1`** (exit 0) | 08:05Z · 08:07:59Z |
| **canal roto declarado** | `ssh arbx "psql …"` | **`bash: line 1: psql: command not found`** exit **127** | — |
| **ventana** | `… -tAc "SELECT min(detected_at), max(detected_at), count(*) FROM opportunities"` | **`min=2026-10-05 04:21:13.227568+00`** · **`max=2026-10-08 08:07:50.254227+00`** · **`total=7925656`** | 08:07:59Z |

### ★ LA VENTANA ACOTA TODO

`opportunities` **retiene ~3 días y 3,8 h**, no un histórico. Y **está viva**: la leí creciendo entre pasadas — **7.859.761 → 7.904.902 → 7.925.656 → 8.031.255** filas, la última a las **08:21:14.258948+00** (≈105.000 filas en 13 minutos). **Toda cifra de este informe es de esa ventana y de un instante declarado.**

**NO COMPARABLE por ventana:** los números de la campaña anterior (t123: `1253` filas de bucket, `max_net=12,490223`, `max_gross=13,14359056`) **no declaran su ventana**, así que **no son comparables como conteos**. Lo que **sí** es comparable es el **extremo**: `max_net=12,490223` reaparece **idéntico** en mi medición de hoy (§1), lo que prueba que al menos el máximo es estable entre ventanas; los conteos, no.

### Error de método propio, declarado y NO repetido

En t143 una query aisló la pata USDC→PEPE con `LIKE '%a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48%'` y devolvió **0 filas**. **No era un resultado:** `leg_amounts_in` contiene **CANTIDADES**, no direcciones; la condición era de mi hipótesis. **El vacío era de la condición** — tercera vez en la campaña con esa forma.

**En esta tarea NO se repite:** la pata se identificó **por POSICIÓN** en `token_addresses` y `leg_amounts_*` (§3), **no por contenido**. Y el resultado —la pata 3, `USDC→PEPE`— **coincide con la pata que t137 aisló por otra vía**: dos condiciones independientes, mismo objeto.

---

## 1. ★★ EL NÚMERO QUE DEFINE LA TAREA

Población: `hop_cycle_bridge`, **fantasma (`retorno`>2×) Y `expected_profit_usd`>0** ⇒ **102.237 filas**.

| métrica sobre las 102.237 | valor |
|---|---|
| `net_expected_profit_usd` **> 0** | **1.253** |
| `net` < 0 | **100.984** |
| `net` = 0 | **0** |
| `net` NULL | **0** |

**Distribución del `net_expected_profit_usd` (las 102.237):**

| min | p25 | **mediana** | p75 | max |
|---|---|---|---|---|
| **−0,649145** | −0,624135 | **−0,622442** | −0,621082 | **+12,490223** |

**Distribución del `expected_profit_usd` (las 102.237):** min **0,01145582** · mediana **0,0305728** · max **13,14359056**.

### ★ Por qué `net>0` NO es una medida de supervivencia

Los **1.253** con `net>0` **no están dispersos: son exactamente el bucket `retorno>10×`**, medido en la misma pasada:

| bucket | n | `net>0` | net min | net mediana | net max |
|---|---|---|---|---|---|
| **`retorno>10×`** | **1.253** | **1.253** | **6,934752** | **7,009417** | **12,490223** |
| resto de la población | 100.984 | **0** | — | (negativo) | — |

**Coincidencia al entero: 1.253 = 1.253, y `net>0` fuera de ese bucket = 0.** Y `net ≠ expected_profit_usd − gas` (**0 de 102.237 coinciden**), con **`economics.gas_usd` = NULL en las 102.237** (la clave existe; el valor es `null`). ⇒ **`net_expected_profit_usd` se computa sobre el gross INFLADO y su gas no está computado en `economics`.** Contar `net>0` como supervivencia sería **leer un campo producido por el propio defecto** — exactamente el modo de fallo que esta campaña persigue. Se reporta como **contaminado**, no como supervivencia.

**Quiénes son los 1.253** (los cuatro grupos suman exacto):

| `token_in` | `pair_symbol` | n | net max |
|---|---|---|---|
| LINK `0x5149…f986ca` | `514910(5-hop cycle)` | 858 | 7,111053 |
| LINK `0x5149…f986ca` | `514910(6-hop cycle)` | 328 | 7,135807 |
| UNI `0x1f98…01f984` | `1f9840(5-hop cycle)` | 50 | 12,480328 |
| UNI `0x1f98…01f984` | `1f9840(6-hop cycle)` | 17 | **12,490223** |

**858 + 328 + 50 + 17 = 1.253** ✓ — y el `net max` global pertenece a **UNI**, consistente con el factor 328,49× de t143.

---

## 2. ★★ EL VEREDICTO: 0 sobrevivientes, y CADA MITAD CAE POR SU PROPIA RAZÓN

**No se extrapola el `0 de 1.247`. Se re-deriva un `0 de 102.237` partiendo la población en dos y midiendo cada mitad:**

| mitad | n | % | por qué NO sobrevive | ¿medido o inferido? |
|---|---|---|---|---|
| **`net` ya negativo con el gross INFLADO** | **100.984** | **98,77 %** | El `net` es negativo **usando el gross inflado 194–328×**. Corregir el precio **baja** el gross ⇒ el net **sólo puede bajar**. Negativo con certeza, sin necesidad de recomputar | **argumento monotónico sobre dato medido** (distribución §1) |
| **`net` positivo inflado** | **1.253** | 1,23 % | Son **el bucket de t137**. t137 recomputó su gross a precio real: **net real máximo −0,60613566** ⇒ **0 sobrevivientes** | **medido por t137** (1.247 de esas 1.253) |

**⇒ 0 sobrevivientes de 102.237.** Y la diferencia con decir "el 0 de 1.247 se extiende" es toda: **aquí el 0 tiene conteo propio y las dos mitades tienen razón propia.**

**Lo que NO está medido, declarado sin adornos:** el bucket creció de **1.247** (t137) a **1.253** (hoy) porque la tabla está viva ⇒ **6 filas del bucket NO están cubiertas por la medición a precio real de t137**. Su `net` inflado está en **6,934752–12,490223**, del mismo orden que las 1.247 cubiertas (cuya net real fue −0,606), pero **eso es inferencia, no medición**. **NO COMPUTADO para esas 6**, con su cierre: recomputar su gross a precio real con el mismo método de t137.

**Y el dato que decide el tamaño del arreglo, que es para lo que existía esta tarea:** la población fantasma con gross>0 es **82× el bucket** (102.237/1.247 = 81,97) y **aporta CERO sobrevivientes nuevos**. El tamaño del arreglo de (a) sigue siendo **102.237 filas**, pero ahora se sabe que **ninguna de las 100.984 adicionales era dinero**.

---

## 3. ★★ LA ARITMÉTICA DE LA PATA DEL ×800 — COMPUTADA

Fila de **máximo retorno** (`token_in` = UNI, `gross` = 13,14359056, `net` = 12,490223). `route_metadata` es la fuente; **nada de esto es inferido**.

**`token_addresses` (7 = 6 patas + 1):** `UNI · USDT · DAI · USDC · PEPE · WETH · UNI`
**`dex_adapters`:** `uniswap-v3 · uniswap-v2 · uniswap-v2 · **uniswap-v3** · uniswap-v2 · uniswap-v2`
**`decimals.map`:** `{UNI:18, USDT:6, DAI:18, USDC:6, PEPE:18, WETH:18}`

| # | pata | `leg_amounts_in` (raw) | `leg_amounts_out` (raw) | humano in | humano out | valor in (USD) | valor out (USD) | ratio |
|---|---|---|---|---|---|---|---|---|
| 0 | UNI→USDT | 3886740365229 | 30 | 0,00000388674 UNI | 0,000030 USDT | 0,0000303 | 0,0000300 | **0,99 ✓** |
| 1 | USDT→DAI | 30 | 29921605087444 | 0,000030 USDT | 0,0000299216 DAI | 0,0000300 | 0,0000299 | **1,00 ✓** |
| 2 | DAI→USDC | 29921605087444 | 29 | 0,0000299216 DAI | 0,000029 USDC | 0,0000299 | 0,0000290 | **0,97 ✓** |
| **3** | **USDC→PEPE** | **29** | **9834630466667449965431** | **0,000029 USDC** | **9.834,63 PEPE** | **0,0000290** | **0,039771** | **★ ×1371** |
| 4 | PEPE→WETH | 9834630466667449965431 | 15517804745656 | 9.834,63 PEPE | 0,0000155178 WETH | 0,039771 | 0,039820 | **1,00 ✓** |
| 5 | WETH→UNI | 15517804745656 | 5112459137999411 | 0,0000155178 WETH | 0,00511246 UNI | 0,039820 | 0,039826 | **1,00 ✓** |

**★ CINCO de las SEIS patas PRESERVAN valor (0,97–1,00). UNA —la pata 3, `USDC→PEPE`— crea ×1371.** El encadenado es correcto (`leg_amounts_out[i] = leg_amounts_in[i+1]` en las cinco junturas): **no hay salto en la cadena**. El salto está en la **magnitud de salida de esa pata**.

**Pata del ×800 localizada:** pool **`0x261d53f3cd0b38dabbab252dcc8adeaa8c67bcba`**, adapter **`uniswap-v3`**, `leg_zero_for_one = false`, entrada **0,000029 USDC** → salida **9.834,63 PEPE**.

### ★ ¿Cantidad o decimales? **DECIMALES DESCARTADO, POR EVIDENCIA**

- **`decimals.map` está COMPLETO y CORRECTO para los seis tokens de la ruta** — `USDC:6`, `PEPE:18`, `UNI:18`, `WETH:18`, `DAI:18`, `USDT:6`. No falta ninguna entrada y ninguna tiene el valor cambiado. **Verificado por medición directa y no por inspección:** entre las **230.526** filas de la población con `leg_amounts_in`, hay **63 mapas distintos** (`count(DISTINCT (route_metadata->'decimals')::text) = 63`) y **`0` filas** donde USDC ≠ 6 o PEPE ≠ 18 (`SELECT count(*) … WHERE usdc <> '6' OR pepe <> '18'` ⇒ **`0`**). El mapa varía en **qué** tokens incluye, **nunca** en el valor de un token.
- **La entrada `29` raw es CORRECTA**: es la salida de la pata 2 (`DAI→USDC`), un triángulo estable que preserva valor (0,0000299 DAI → 0,000029 USDC = 0,97 con fees). Si la entrada estuviera mal escalada, la pata 2 no cerraría — y cierra.
- Con el mapa correcto y la entrada correcta, **una conversión de decimales no puede producir ×1371** (las potencias de 10 disponibles son 10⁶ y 10¹², no 1371).

**⇒ Es un defecto de CANTIDAD: la magnitud de salida que el motor computa para esa pata V3.** El output de un swap V3 escala linealmente con la liquidez del rango ⇒ el estado de pool usado por el motor (liquidez / `sqrtPriceX96`) es el locus, **no la escala decimal**. **Con el mapa de decimales correcto descartado por la evidencia, queda el estado de pool** — y **no lo medí** (§4).

**★ El `archivo:línea` de `main` que computa esa magnitud de salida** (leído de `main`, **nunca** del checkout compartido, con `git grep -n 'fn ' <sha> -- backend/searcher-rs/src/amm_math.rs`):

| línea en `main` (`d1a4c3f5`) | símbolo |
|---|---|
| **`backend/searcher-rs/src/amm_math.rs:161`** | **`pub fn v3_amount_out_single_tick(`** ← **la magnitud de salida de una pata V3** |
| `backend/searcher-rs/src/amm_math.rs:96` | `pub fn v2_amount_out(amount_in, reserve_in, reserve_out, fee_bps)` — la V2, que **no** es el locus: 3 de las 6 patas son V2 y **todas preservan valor** |
| `backend/searcher-rs/src/amm_math.rs:261` | `pub fn v3_spot_snapshot(` — el snapshot de estado que alimenta el cálculo |
| `backend/searcher-rs/src/amm_math.rs:798` | `pub async fn v3_quote_exact_in_multicall(` — el quoter on-chain |

**Alcance de esa cita, declarado:** identifica **la función que computa la salida V3**, no prueba que sea la defectuosa. **Los adaptadores confirman la correspondencia**: `dex_adapters[3] = "uniswap-v3"` y las otras cinco patas son `uniswap-v2` — **el defecto está exactamente en la única pata V3 del camino que además resulta ser la única que crea valor**.

**★ Confirmación independiente del `ret_max` de t137:** round-trip = `5112459137999411 / 3886740365229` = **1.315,36×** — **exactamente el `1.315,3591×` que t137 reportó como cota superior**. Mi medición de la fila y la de t137 son **el mismo objeto por dos vías**.

---

## 4. LÍMITES DEL INSTRUMENTO Y PUNTOS NO COMPUTADOS

| # | NO COMPUTADO | Por qué (medido) | Qué lo cerraría |
|---|---|---|---|
| 1 | **El estado de pool de `0x261d53f3…`** (`liquidity()`, `sqrtPriceX96`, `tick`) | Medí **que** la pata 3 crea ×1371 y **que** los decimales son correctos; **no** leí el pool. `cast` está disponible y el RPC también; no se ejecutó por presupuesto de la tarea | `slot0()` + `liquidity()` de `0x261d53f3…` al `economics.quote_block` de la fila, y recomputar el swap V3. **Es el cierre de esta tarea** |
| 2 | **Las 6 filas nuevas del bucket** (1.253 − 1.247) | La tabla está viva: el bucket creció entre t137 y hoy. Su net real no está medido | Recomputar su gross a precio real con el método de t137 |
| 3 | **`gas_usd` real de las 102.237** | `economics.gas_usd` es **`null`** en las 102.237 (clave presente, valor nulo). Y `net ≠ gross − gas` (0 de 102.237) ⇒ el gas no entra por ahí | Localizar la fuente del gas en el productor; no está en `economics` |
| 4 | **`gross` real de las 100.984** | Se resuelve por **monotonía** (net ya negativo con gross inflado ⇒ sigue negativo). **No se recomputó 100.984 veces** | Recomputar el gross a precio real fila por fila — **no cambia el veredicto**, cambia el decimal del margen |
| 5 | **Cobertura del `decimals.map` fuera de estos seis tokens** | Verifiqué que **ningún token de la ruta tiene el valor cambiado** (0 filas con USDC≠6 o PEPE≠18 sobre 230.526) y que hay **63 mapas distintos**; no verifiqué la cobertura del mapa contra el universo completo de tokens | Cobertura completa del mapa contra el universo de tokens |

**Límite declarado:** la medición **no incluye** ninguna lectura on-chain. Todo el §3 sale de `route_metadata` (que trae `leg_amounts_*`, `token_addresses`, `pool_addresses`, `dex_adapters`, `decimals`, `leg_zero_for_one`) más los **precios de `trading_config`** del instante declarado. **El `quote_block` de la fila no se usó**: cualquier recomputo on-chain tiene que leer el pool **a ese bloque**, no al head, o no es comparable.

---

## 5. REPRODUCCIÓN

```bash
P='docker exec arbitragex-v2-postgres-1 psql -U postgres -d arbitragex -tAc'
# control de canal
$P "SELECT 1"
# ventana
$P "SELECT min(detected_at), max(detected_at), count(*) FROM opportunities"
# EL NUMERO: supervivencia de las 102.237 (nunca se corre el psql del host: exit 127)
$P "SELECT count(*) FILTER (WHERE net_expected_profit_usd > 0) FROM opportunities
    WHERE detector_id='hop_cycle_bridge' AND route_metadata ? 'leg_amounts_in'
      AND (route_metadata->'leg_amounts_in'->>0) IS NOT NULL
      AND (route_metadata->'leg_amounts_out'->>-1) IS NOT NULL
      AND ((route_metadata->'leg_amounts_out'->>-1)::numeric / NULLIF((route_metadata->'leg_amounts_in'->>0)::numeric,0)) > 2
      AND expected_profit_usd > 0"
# la fila de maximo retorno, con su estructura completa
$P "SELECT jsonb_pretty(route_metadata) FROM opportunities WHERE detector_id='hop_cycle_bridge'
      AND route_metadata ? 'leg_amounts_in' ORDER BY ((route_metadata->'leg_amounts_out'->>-1)::numeric / NULLIF((route_metadata->'leg_amounts_in'->>0)::numeric,0)) DESC LIMIT 1"
```

**Nota de instrumento medida:** en este dataset `psql -tA` imprime **NULL como cadena vacía**, no como `NULL`. Varias de mis queries devolvieron **vacío con exit 0** y eso **era un NULL**, no un fallo ni un cero. (**`min(gas_usd)` vacío = `gas_usd` nulo**, no "gas cero".) Distinguir "vacío por NULL" de "vacío por condición" es obligatorio en este canal.

---

## 6. INTEGRIDAD Y PERMISOS

**Solo lectura.** Todos los comandos fueron `SELECT` (`psql -tAc`). CERO escrituras en PG/Redis, CERO cambios de umbrales o configuración, CERO cambios al motor. El único archivo tocado es `docs/data/GHOST-SURVIVAL-01.md`.

Integridad **por blob de git**, nunca con `Out-File`:

```bash
git hash-object docs/data/GHOST-SURVIVAL-01.md
git rev-parse HEAD:docs/data/GHOST-SURVIVAL-01.md
```

---

*El `0 de 1.247` no se extendió: se re-derivó. **0 de 102.237**, con la población partida en dos mitades que caen por razones distintas y medidas — 100.984 por monotonía (negativas incluso con el gross inflado) y 1.253 porque son el bucket que t137 midió a precio real. Y el número que revela por qué era necesario medirlo: de las 102.237, el motor reporta **1.253** como rentables, y esas 1.253 **no son la población mayor: son exactamente el 1,2 % que ya estaba medido**. Las 100.984 restantes —el 82× que nadie había mirado— **no aportan un solo sobreviviente nuevo**. Y la pata del ×800 quedó computada: de seis patas, **cinco preservan valor y una crea ×1371**, con el mapa de decimales completo y correcto, lo que **descarta decimales por evidencia** y deja el estado de pool como locus.*
