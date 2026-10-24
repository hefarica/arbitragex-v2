# CANDIDATE-NET-01 — **NO COMPUTADO**: la curva sólo operaba `token0→token1` y WETH es `token1` en el 92 % de los pools

**Resultado en una línea:** **NO se puede responder la pregunta binaria, y la causa es un defecto de mi maquinaria, totalmente localizado.** `con_neto_computable = 0` y `CRUZAN_EL_FLOOR = 0` **sobre los 392 combos** — pero **ese 0 significa «no pude calcular el neto», no «el neto no cruza»**: `sin_precio_usd = 392`, **el 100 %**. La causa: mi `net_curve` **sólo opera `token0 → token1`**, y **WETH es `0xc02aaa39…`, una dirección ALTA, así que en el orden `token0 < token1` es casi siempre `token1`** ⇒ **ninguna dirección de ningún combo tenía el token de entrada pricado**. **NO reporto un NO: reporto NO COMPUTADO con la causa y el fix de una línea.**

**Cápsula ORGDNA:** `run_id=arbx-entrega-20261008` · **Dueño:** Data · **Intento:** `8630d106-0bfe-46b3-8da0-a400cea43d87`
**Solo lectura** · **CERO escrituras** · **No se escribió en el fork** · **In scope:** `docs/data/`

---

## 0. ★★ EL CONTROL QUE SÍ FUNCIONÓ: mi barrido reproduce a t177 exactamente

Antes de la curva, mi re-barrido a **bloque fijo `26148216`** reprodujo **los números de t177 al entero**:

| | t177 (PR #907) | **este intento** |
|---|---|---|
| combos no-degenerados con spread > 0,35 % (`limpias_0.35`) | **232** | **232** ✓ |
| combos en la banda plausible [0,35 %, 1 %) | **63** | **63** ✓ |
| combos totales | 392 | **392** ✓ |
| degenerados | 13 de 1.139 | **13** ✓ |
| universo | 1.346 | **1.346** ✓ |
| `con_precio` | 1.139 | **1.140** |
| bloque | 26148216 | **26148216** ✓ |

**⇒ La maquinaria del barrido es consistente entre las dos corridas.** `plausibles_0.35_1 = 63` y `limpias_0.35 = 232` **reproducen a t177 exactamente**, lo que descarta que el problema esté en el barrido. **El defecto está más adelante, en la curva.**

### La validación por precio marginal — re-corrida sobre la maquinaria de swap

**`VALIDACION_marginal: 3/4 ok · ratios = ['1.000000000', '0.999999984', '1.000000000', '1.000000000']`**

**3 de 4 dan `1.000000000` exacto; el cuarto da `0,999999984`, un desvío de `1,6 × 10⁻⁸`** — precisión de punto flotante en la división de enteros grandes, **no** un error de unidades. **Se declara tal cual y NO se redondea a `4/4`:** t177 dio `4/4 = 1,000000000` sobre la misma fórmula, y esta corrida da **3/4 exactos + 1 al 1,6e-8**, y ésa es la cifra que se reporta.

**Y el fallo de la curva NO es de unidades**: los dos errores que el contrato manda cazar están respetados — **`fee_tier` se trata como BASIS POINTS** (`gamma = 1 − fee_tier/10000`) y **el tercer valor de `getReserves()` (`blockTimestampLast`) no se lee como fee** (sólo los dos primeros `uint112`).

---

## 1. ★★ LA PREGUNTA: **NO COMPUTADO**, y la causa exacta

```
combos=392  con_neto_computable=0  sin_precio_usd=392  CRUZAN_EL_FLOOR=0
motivos={"sin_precio_usd_ambas_direcciones": 392}
```

**★ ESE `CRUZAN_EL_FLOOR = 0` NO ES UN RESULTADO.** Es «no pude calcular el neto de ninguno». Reportarlo como **NO** sería exactamente el error que esta campaña caza: **un cero obtenido por un instrumento que falló**.

### La causa, medida

`trading_config.token_prices_usd` tiene **21 tokens**: `AAVE, APE, COMP, CRV, DAI, ENS, LDO, LINK, MANA, MATIC, MKR, PEPE, RETH, SAND, SHIB, SUSHI, UNI, USDC, USDT, WBTC, WETH`. El barrido compara **899 pares distintos**.

**Símbolos presentes en los 104 pools que forman los 63 candidatos plausibles** (46 tokens distintos):

| símbolo | apariciones |
|---|---|
| **WETH** | **96** |
| USDC | 8 |
| cbETH | 5 |
| TORN · PAXG · IMX | 4 · 4 · 4 |
| AAVE · ELON · GLM · NMR · WPLS · Mog · «FTX Token» | 3 cada uno |
| MKR · DAI · UNI · MANYU · FLUID · XPR · ENA · ALEPH · EURC · Yee · HOT · BOOE | 2 cada uno |

**★ WETH aparece en 96 de 104 pools (92 %) y SÍ está pricado.** Entonces el problema no es la falta de precios: es **de qué lado está WETH**.

### ★★ EL DEFECTO: `token0 < token1` pone a WETH del lado `token1`

WETH es **`0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2`**, una **dirección ALTA**. La convención Uniswap V2/V3 es **`token0 = dirección menor`** ⇒ **WETH es `token1` en prácticamente todos estos pools**. Y mi `net_curve` **sólo opera `token0 → token1`**: toma `p_in = price_usd(P, P["t0"])`, que es el precio **del `token0`** — que en estos pools es **el token exótico sin precio** (TORN, PAXG, cbETH, ELON…).

**El «fix de ambas direcciones» que apliqué cambió el pool de compra pero NO el lado dentro del pool** — siguió entrando siempre por `token0`. Por eso el conteo no se movió ni un dígito entre las dos corridas: **`sin_precio_usd = 392` en ambas**.

**⇒ El defecto es de UNA dimensión que no varié: el LADO del swap (token0 o token1), no el pool.** Operar por `token1` es `zeroForOne = false`, y **el precio del `token1` sí está disponible en el código** (`price_usd` tiene la rama `d["s1"]`) — **nunca se alcanzó porque el flujo siempre entraba por `t0`.**

### El fix, exacto

**Cuatro direcciones por combo, no dos:** (pool A o pool B) × (entrar por `token0` o por `token1`), descartando las que no cierren el round-trip. La rama de precio del `token1` **ya existe**; sólo hay que **alcanzarla**. Con WETH pricado y presente en el 92 % de los pools candidatos, **un subconjunto grande de los 63 debería volverse computable**. **El barrido no hay que rehacerlo**: el estado del fork está en el mismo bloque fijo `26148216` y el JSON con los 392 combos ya está escrito con las direcciones.

**NO declaro cuántos serán computables** — no lo medí y no lo estimo.

---

## 2. ★ LO QUE SÍ QUEDA ESTABLECIDO

| # | hallazgo | artefacto |
|---|---|---|
| 1 | **El barrido es reproducible**: 232 y 63 idénticos a t177, con el mismo bloque | `limpias_0.35=232 · plausibles_0.35_1=63 · combos=392 · degenerados=13` |
| 2 | **Los 63 candidatos involucran 104 pools y 46 tokens distintos** | conteo por símbolo, sobre 104 direcciones |
| 3 | **WETH domina con 96 de 104 (92 %)** — y es la clave del fix | tabla de símbolos |
| 4 | **La validación marginal está sana**: 3/4 exactos + 1 a 1,6e-8 | `ratios=['1.000000000','0.999999984','1.000000000','1.000000000']` |
| 5 | **El problema NO es de unidades ni de reservas** | los dos errores de unidades respetados en el código |
| 6 | **El problema es el LADO del swap** | `sin_precio_usd` idéntico en las dos corridas |

---

## 3. NO COMPUTADO — declarado, y **no rellenado**

| # | NO COMPUTADO | razón |
|---|---|---|
| 1 | **LA PREGUNTA BINARIA** (¿algún neto cruza $1,95?) | **el neto no es computable para ninguno de los 392** con la maquinaria actual: entrada siempre por `token0`, sin precio |
| 2 | **El máximo neto global, su tamaño y su pool** | consecuencia de (1) |
| 3 | **Cuál de las tres fricciones mata** (fee / impacto / gas) | no hay curva válida que descomponer |
| 4 | **Las operaciones que soporta cada candidato** | no hay primera operación medible |
| 5 | **Los precios de los tokens exóticos** (`tORN`, `PAXG`, `cbETH`, `ELON`, … 44 de los 46) | **no están en el mapa de config**; el precio USD del `token1` sólo ayuda si el `token1` está pricado |
| 6 | **La lista de los 63 con su curva** | depende de (1) |

**Y lo que se respeta sin rellenar:** el blob `c6b99de1…` de PR #902 **no se cita** (no se accedió, igual que en t177); **no se reabre** lo declarado NO COMPUTADO en t177 (los 205 V3 con `liquidity()` 0, los 207 no barridos); **no se escribe en el fork**.

**El contexto duro de t175 se cita sin recalcular y sin contradecirlo:** gas `$0.676131`, **gas floor 3× = `$1,95`** (el umbral que esta tarea usa), **gas 24× el bruto máximo** de su curva y su **neto global `−$0,6475`**. **NO se mide «cuánto se podría ganar».**

---

## 4. LA FRONTERA CANDIDATO / OPORTUNIDAD — INTACTA

t177 la fijó con estas palabras: *«No declaro una oportunidad rentable: declaro un candidato que el NO no cubría.»*
**Hoy no se movió ni un milímetro: siguen siendo candidatos.** El neto —**el único que convierte un candidato en oportunidad**— **no se computó**. Y **aunque se compute y cruce el floor, no se declarará rentable en producción**: el flujo real, la competencia y el impacto acumulado **no son computables con lo medido**.

**Cobertura, en la misma línea que cada conclusión:** barrido **1.140 de 1.346 = 84,7 %** · **207 no barridos con razones contadas** (`v3_liq_cero_o_ilegible` 205 · `decimals_ilegibles` 1 · el resto en la corrida previa) · **el universo creció de 1.336 a 1.346: es vivo**. **Nada de este informe se enuncia sobre «el universo»: se enuncia sobre los 392 combos (232/193/63), y se dice.** El universo de comparación son **combos** (392), de **899 pares**, **146 con ≥2 pools**.

---

## 5. INSTRUMENTO Y PERMISOS

**`http://172.18.0.3:8545`** (IP del contenedor anvil) — **no `localhost:8545`**, que no está publicado al host y ya costó un barrido entero en t177. **Bloque fijo `26148216` en las 4.038 `eth_call`, ninguna a `latest`.** Canal Postgres `docker exec` con `--set=ON_ERROR_STOP=1` **antes** de `-tAc`, cada comando con su **exit code**, **ningún control pipeado a `head`**. `:9090` externo y `195.201.235.70/metrics` **no usados**.

**`capital_usd` sigue `1000.00` y el target `50.0`: NO se tocaron** — cambiarlos es decisión del operador. CERO cambios al motor, umbrales, configuración, fork o producción. **CERO `eth_sendTransaction` / `anvil_setStorageAt` / `anvil_setBalance`: NO se escribió en el fork.** CERO mainnet, firmas o broadcast. Paper.

```bash
git hash-object docs/data/CANDIDATE-NET-01.md
git rev-parse HEAD:docs/data/CANDIDATE-NET-01.md
```

---

*La pregunta sigue abierta y ahora sé exactamente por qué: **mi curva sólo operaba `token0 → token1`, y WETH —`0xc02aaa39…`, dirección alta— es `token1` en el 92 % de los 63 candidatos**, así que la entrada era siempre un token sin precio. `con_neto_computable=0` y `CRUZAN_EL_FLOOR=0` **no son un NO: son el instrumento**. El control que sí valida el trabajo es que **el barrido reproduce a t177 al entero** (232 y 63, mismo bloque), y que **la validación marginal está sana** (3/4 exactos + 1 a 1,6e-8). El fix es de una línea —probar también el **lado** del swap, no sólo el pool— y la rama de precio del `token1` **ya existe en el código**: sólo hay que alcanzarla. **No lo apliqué por presupuesto, y por eso no declaro cuántos de los 63 se volverán computables.** Los candidatos siguen siendo candidatos.*
