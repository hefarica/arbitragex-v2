# ECON-ACCOUNTING-01 — contabilidad sin mezclar ESTIMADO / SIMULADO / REALIZADO

**Orden:** t49 · **Perfil:** Data · **Intento:** `472ad69b-a484-4c58-bf6e-060d64bb289a`
**Sobre la ruta de `t48` (ECON-PATH-01):** `WETH → USDC → WETH` por Uniswap V2 + SushiSwap, bloque pinneado `26130440`.
**Modo:** `ARBX_TRADE_MODE=paper`. Sin firma, sin broadcast, sin capital, sin settlement.

**Base medida por mí (no citada):** clon aislado en `C:\Users\HFRC\Desktop\arbx-t49\repo`, `git rev-parse HEAD` → **`c89d21a3437c7fc2456fb7a08dba7d26ef41ee5e`** (`main`).
La orden declara `main` = `3f00b359…`; **main se movió** — lo declaro en vez de citar un SHA que ya no es la cabeza.
**Artefacto de `t48` leído:** `docs/backend/ECON-PATH-01.md` (330 líneas) **desde el head de PR #816 (`df24cbfe…`)** — no está en `main`, su PR sigue abierto. Lo obtuve con
`gh api "repos/hefarica/arbitragex-v2/contents/docs/backend/ECON-PATH-01.md?ref=docs/econ-path-01" -H "Accept: application/vnd.github.raw"`.

**Alcance:** este documento **no edita código**, no toca workflows, scripts, backend, frontend ni `implementation-state/`.
Sólo clasifica. **No mueve P/N.**

---

## 0. REGLA DE CATEGORIZACIÓN (operativa, verificable por inspección de fuente)

Las tres categorías se separan por **el instrumento que produjo el número**, no por la intención de quien lo escribe.
El test discriminante es un test sobre la obtención:

| Categoría | Test operativo (inspeccionable) | Qué NO puede contener |
|---|---|---|
| **ESTIMADO** | Lecturas `view`/`staticcall` (sin camino de escritura, sin `--override-state-diff`) + aritmética de forma cerrada, **o** un parámetro declarado de la operación hipotética. | Gas ejecutado. Estado mutado. Overrides. |
| **SIMULADO** | Ejecución EVM que **habría** mutado estado, corrida **localmente** contra fork efímero, con traza y gas. Requiere overrides para financiar al caller. | Firma. Broadcast. Inclusión en bloque. Settlement. Gas pagado. |
| **REALIZADO** | Cambio de estado **incluido en un bloque minado** de la cadena objetivo: firma + broadcast + inclusión + settlement. Evidencia mínima: `tx hash` + receipt `status` + `blockNumber`. | Nada: si no hay esas cuatro cosas, no hay REALIZADO. |

**Test en una línea:** ¿mutó estado en el fork o necesitó overrides? → **SIMULADO**. ¿Sólo leyó y computó? → **ESTIMADO**. ¿El estado quedó escrito en un bloque minado? → **REALIZADO**.

**Dos reglas de forma, no negociables:**

1. **NO REALIZADO ≠ 0.** Un `0` es un valor computado (R8: `Some(0.0)` = computado y exactamente cero). La ausencia de un hecho de settlement es un **estado**, no una medición: se declara **NO REALIZADO**, nunca `0`.
2. **NO COMPUTADO** para lo que no se puede conciliar, con la razón exacta (falta productor, falta tabla, falta corrida). No se cierra un balance con una estimación presentada como hecho.

---

## 1. VEREDICTO CONTABLE

> **REALIZADO: NO REALIZADO.** No existe ningún valor realizado en esta ruta.
> **Razón:** `ARBX_TRADE_MODE=paper`; no hubo firma, ni broadcast, ni inclusión en bloque, ni settlement; no se movió capital.
> **No se rellena con el simulado ni con el estimado.** Las trazas de `t48` §6 declaran `Transaction successfully executed` **en un fork local** — eso es SIMULADO, y presentarlo como realizado sería exactamente el error que esta orden existe para impedir.

**NO COMPUTADO (no se puede conciliar todavía, con razón):** la **ruta que eligió el sistema** y el **contador `P/N`** — §4.

**ESTIMADO** y **SIMULADO** sí existen y se separan en §2 y §3.

---

## 2. ESTIMADO — valores derivados o declarados, sin ejecución de escritura

| # | Valor | Magnitud | Artefacto donde vive | De dónde sale exactamente |
|---|---|---|---|---|
| E1 | `2709.975872` | USDC/WETH spot UniV2 | `docs/backend/ECON-PATH-01.md` §2 L45 | `getReserves()` del pool `0xB4e16d…` @ `26130440` + división por decimales (6/18) |
| E2 | `2708.467757` | USDC/WETH spot Sushi | `ECON-PATH-01.md` §2 L45 | `getReserves()` del pool `0x397FF1…` @ `26130440` |
| E3 | `0.994562479` (12 dp: `0.994562478940`) | retorno marginal a tamaño→0 | §4.1 L91 | `(0.997)² × (E1/E2)` — forma cerrada |
| E4 | `−0.005437521` (12 dp: `−0.005437521060`) | payoff `b` | §4.2 L103 | `E3 − 1` |
| E5 | **`10.14` publicado** (recomputado: `10.1454` → 2 dp `10.15`) | `f*` con `p=0.95` | §4.2 L106 | `(p·b − (1−p))/b`. **Discrepancia H1 en §6** |
| E6 | `5.568` bps | spread bruto uni/sushi | §3 L77 | `(E1/E2 − 1) × 10⁴` |
| E7 | `60.271` bps | hurdle de fees de 2 pools | §3 L78 | `(1/0.997² − 1) × 10⁴` |
| E8 | `54.703` bps | déficit | §3 L79 | `E7 − E6` |
| E9 | `10.82×` | spread faltante | §3 L79 | `E7 / E6` |
| E10 | `0` | fracción de Kelly **aplicada** | §4.2 L108, §4.3 L116 | No aplica: `b<0` ⇒ dominancia. **No es una medición: es la decisión de tamaño** |
| E11 | `1000 USD` | capital de referencia | §4.3 L117 | **Parámetro declarado** (default del módulo de riesgo del repo) — no medido en esta ruta. Sub-etiqueta: *parámetro*, no *derivado* |
| E12 | `0` | importe | §4.3 L118 | `E11 × E10`. **Trampa declarada en §3.3** |
| E13 | `2701152913` / `976135106962143097` (1 WETH) | salidas de las dos piernas | §3 L66 | `getAmountsOut` por los routers reales @ `26130440` |
| E14 | `26949315944` / `8366252744341588826` (10 WETH) | salidas de las dos piernas | §3 L67 | `getAmountsOut` @ `26130440` — **nunca ejecutadas con traza** |
| E15 | `263425910836` / `34440274425259215329` (100 WETH) | salidas de las dos piernas | §3 L68 | `getAmountsOut` @ `26130440` — **nunca ejecutadas con traza** |
| E16 | `−23864893037856903` / `−1633747255658411174` / `−65559725574740784671` wei | netos de las tres tallas | §3 L66-68 | `salida_leg2 − entrada`, aritmética exacta (BigInt) |
| E17 | `−2.3864893%` / `−16.3375%` / `−65.5597%` | netos en % | §3 L66-68 | `E16 / talla` |

**Por qué E10/E11/E12 son ESTIMADO y no otra cosa:** describen la **operación hipotética** (cuánto se habría puesto), no un hecho ocurrido. No hubo capital, así que no hay "importe ejecutado" que buscar.

### 2.1 El `0` del importe, y por qué no es un realizado

`importe = 0` es el valor **computado** de una decisión de tamaño (`1000 × 0`), y coincide con el veredicto económico: con `b<0` el tamaño óptimo es `0` por dominancia.

> **Declaración anti-confusión:** ese `0` **no** significa "la ruta costó 0", **no** significa "P&L realizado 0", y **no** es evidencia de que no se perdió nada. Es un `0` de **decisión** en la capa ESTIMADO. El P&L realizado **no existe** (§3), y la distinción `0` vs `NO EXISTE` es exactamente el punto de esta orden.

---

## 3. SIMULADO — ejecución local contra fork, con traza y gas, sin broadcast

| # | Valor | Magnitud | Artefacto donde vive | De dónde sale exactamente |
|---|---|---|---|---|
| S1 | `2701152913` | `amount0Out` LEG 1, 1 WETH | `ECON-PATH-01.md` §6.B L203, L205 | Frames `swap(...)` + `emit Swap(param3: 2701152913)` de la traza |
| S2 | `976135106962143097` | `amount1Out` LEG 2 | §6.C L223, L225 | `swap(0, 976135106962143097, …)` + `transfer` del WETH |
| S3 | `−23864893037856903` wei (`−2.3864893%`) | neto del ciclo a 1 WETH | §6.D L241 | `S2 − 1e18`, con las dos piernas ejecutadas |
| S4 | `121844` | gas de fork LEG 1 | §6.B L210 | Línea `Gas used: 121844` de la corrida B |
| S5 | `128409` | gas de fork LEG 2 | §6.C L230 | Línea `Gas used: 128409` de la corrida C |
| S6 | `250253` | gas de fork total | §6.D L241 | `S4 + S5`. **La suma de dos simulados es simulada** |
| S7 | `[Revert] UniswapV2Library: INSUFFICIENT_INPUT_AMOUNT` + gas `30392` | corrida A (`amountIn = 0`) | §6.A L185, L187 | La traza; **reproducida por mí** (§7.2) |
| S8 | `balanceOf[0x…bEEF]=1e18 WETH`, `allowance=2²⁵⁶−1`, slots `3` y `9` | setup de la simulación | §6.0 L160-174, §6.B L194 | Derivación + verificación de slot contra reserva conocida; **estado de fork, no de mainnet** |

### 3.1 S7 no es un costo incurrido

El revert es un **resultado de mecanismo**, no un gasto. El `30392` de gas es **gas de fork**: no se pagó, no se firmó, no se incluyó.
> **No se registra como pérdida, ni como costo, ni como P&L.** Un revert acá **no cuesta nada** porque no ocurrió on-chain. Registrarlo como pérdida sería inventar un hecho; registrarlo como `0` sería inventar una medición.

### 3.2 Lo que NO es SIMULADO aunque lo parezca

- **Las tallas 10 y 100 WETH (E14, E15) son ESTIMADO, no SIMULADO.** Se cotizaron con `getAmountsOut` y **nunca se ejecutaron con traza**. No tienen gas de fork ni estado mutado; tratarlas como simuladas sería prestarles una evidencia que no existe.
- **El `cast call` de una función `view` (E13) es ESTIMADO.** No muta estado, no necesita overrides, no produce escritura. Que el EVM corra por debajo no lo convierte en una ejecución de la operación.
- **El gas de fork no es el gas que se pagaría.** Es el gas **medido** en un fork con overrides; sirve para el mecanismo, no para costear.

### 3.3 La única cosa que migra de categoría, declarada con su causa

Un mismo número aparece en dos capas: `2701152913` (LEG 1) y `976135106962143097` (LEG 2) al tamaño `1 WETH` están **cotizados** (§3 L66, ESTIMADO) y **ejecutados con traza** (§6.B/§6.C, SIMULADO).

> **CONCORDANCIA (no promoción).** El valor se lista **una sola vez**, en su categoría más fuerte (**SIMULADO**), y la coincidencia se registra como **verificación cruzada**, no como una segunda contabilidad. Dos instrumentos que miden lo mismo no suman: **confirman**.
>
> **Migración declarada:** `ESTIMADO → SIMULADO`, cantidad `S1`, `S2` y `S3`, causa = *re-medición por un instrumento más fuerte (ejecución con traza) al mismo bloque pinneado y al tamaño declarado `1 WETH`*.
>
> **Techo de la migración:** para ahí. Un `Transaction successfully executed` de `cast call --trace` es una ejecución **local sobre un fork**; **ningún valor migra a REALIZADO** por ejecutarse en un fork. Y las tallas 10/100 WETH **no migran**: quedan en ESTIMADO porque no hay ejecución que las mida.

---

## 4. REALIZADO = NO REALIZADO · NO COMPUTADO

### 4.1 La categoría REALIZADO, vacía y declarada

| Hecho que debería existir para que haya REALIZADO | Estado | Razón |
|---|---|---|
| Firma de transacción | **NO REALIZADO** | No hay llave usada, no hay `tx hash` que firmar |
| Broadcast / inclusión en bloque | **NO REALIZADO** | `ARBX_TRADE_MODE=paper`; el sistema corrió sin broadcast |
| Settlement on-chain | **NO REALIZADO** | Sin inclusión no hay settlement |
| P&L realizado | **NO REALIZADO** | Requiere lo anterior |
| Gas efectivamente pagado | **NO REALIZADO** | `S4`/`S5`/`S6` son gas de **fork**, no un costo incurrido |
| `amountOut` settled | **NO REALIZADO** | `S1`/`S2` son salidas de fork |

**Ninguna celda de esta tabla se rellena con SIMULADO ni con ESTIMADO.** Y ninguna se rellena con `0`: `0` sería un valor computado y acá no hay medición — hay **ausencia de hecho**.

### 4.2 Cross-check de esquema: la columna llamada `actual` tampoco es realizado

El repo tiene columnas con nombre `actual_*` en `paper_trade_runs` (`actual_gas_cost_usd`, `actual_profit_usd`). **Su propio string de procedencia declara que NO son settlement on-chain.** Verificado en `main` `c89d21a3`:

```
backend/api-server/src/routes/risk-circuit-breakers.ts:171
const ACTUAL_GAS_PROVENANCE = "sim-ctl replay via drift_tracker (not on-chain settled)";
```

Es decir: **incluso el campo que el esquema llama "actual" es un replay simulado**, no un realizado. Un lector que mapee `actual_*` → REALIZADO cometería el error de esta orden **en la capa de datos**.

Y hoy ese campo **no tiene productor corriendo**: el drift-tracker sólo arranca si la variable vale exactamente `on` (`backend/recon/src/main.rs:344`) y `.env.example:347` declara `ARBX_DRIFT_TRACKER_MODE=off`. Consecuencia para esta contabilidad: el costo de gas **realizado** de cualquier corrida paper es **NULL**, y NULL **no es `0`** (R10, E2E-COMPUTE GUARD: un wire sin productor no es un valor cero).

### 4.3 NO COMPUTADO — lo que no se puede conciliar todavía, con la razón

| # | Qué | Por qué NO se puede conciliar | Artefacto de la frontera |
|---|---|---|---|
| N1 | **La ruta que eligió el sistema** | PostgreSQL **inalcanzable** desde esta sesión: `sql_query 'SELECT 1 AS ping'` → `{"ok": true, "data": "", "rows_affected": 0}` (payload **vacío**, no un conjunto de filas de tamaño 0); `sql_tables` → `{"count": 0, "tables": []}`. Sin ledger legible **no hay productor de oportunidades** que consultar ⇒ no sé qué ruta habría elegido el searcher. La ruta de `t48` **la eligió su autor** desde estado on-chain y lo declara (§1 L27, §9.1 L313). | Probes propios, §7.4 |
| N2 | **`P/N = 0/115`** | **No se recalcula**: PG inalcanzable (N1) y **no localicé productor del contador en el árbol**. Se cita la convención de tres informes previos de la campaña, no una medición propia. | `docs/sre/RUNTIME-IDENTITY-PROBE-01.md:130`, `docs/sre/RUNTIME-IDENTITY-REPEAT-01.md:142`, `docs/sre/TRACK-PROBE-01.md:161` |
| N3 | **El valor REALIZADO de esta ruta** | Falta la **corrida con broadcast + settlement**. No es un dato que esté en otro lado esperando ser leído: **no ocurrió**. | §4.1 |
| N4 | **Conciliación ruta ↔ ledger paper** | Falta la **fila**: la ruta no nace de una oportunidad persistida, así que no hay `paper_trade_runs.id` que ligar. Y aun con la fila, `actual_gas_cost_usd` es NULL (§4.2). | §2 L27 de `ECON-PATH-01.md`; §4.2 |

---

## 5. ESTO NO MUEVE P/N

**`P/N` sigue en `0/115`.** Este trabajo **no** lo mueve, **no** cierra ningún criterio de negocio y **no** convierte en verificado ningún otro criterio.
Lo que hace es una sola cosa: **separar tres capas que estaban juntas en el mismo documento**, de modo que ningún número simulado pueda leerse como realizado.

**Cuándo sí movería el criterio económico, declarado por adelantado:** sólo si la evidencia de esta ruta queda **ligada** al criterio, lo que exige las **tres** cosas a la vez:

1. una **fila** en el ledger paper ligada a la ruta (hoy N4: no existe);
2. un **productor** del campo `actual_*` corriendo (hoy §4.2: switch en `off` ⇒ NULL);
3. **settlement** con broadcast para que exista la capa REALIZADO (hoy §4.1: no existió).

Ninguna de las tres está. Por eso el `0` de `P/N` **no** se toca, y por eso este documento **no** se apoya en `P/N` para nada.

---

## 6. DISCREPANCIAS Y RECONCILIACIÓN DE ETIQUETAS (fail-honest)

**H1 — `f*` publicado no reproduce bajo su propia fórmula.** `t48` §4.2 publica `f* = 10.14` con `p=0.95`.
Recomputado desde sus propios insumos (`b = −0.005437521060`): `(0.95·b − 0.05)/b =` **`10.145366684`** → a 2 decimales redondeando es **`10.15`**; `10.14` sale **truncando** a 2 decimales (`Math.trunc(10.1454·100)/100 = 10.14`) o pre-redondeando `b` a 6 decimales (`−0.005438` → `10.1446` → `10.14`).
**No puedo determinar cuál de las dos rutas usó `t48`** (no publica el paso intermedio) y no lo afirmo.
**Materialidad: ninguna.** Con `b<0` la fórmula **no aplica** y el stake es `0` por dominancia: la diferencia `0.0054` no cambia ninguna decisión. Pero es un **número publicado que no reproduce**, y en una casa que ya midió una colisión de etiquetas esa clase de defecto se registra.

**H2 — los porcentajes SÍ son consistentes (no es discrepancia).** `−16.3375%` y `−65.5597%` son `−16.3374726…` y `−65.5597256…` redondeados a 4 decimales. Verificado, no hay defecto.

**H3 — colisión de etiquetas `MEDIDO` vs categorías contables (declarada para que nadie la lea mal).**
`t48` §1 llama **`MEDIDO`** a las lecturas de §2/§3. En mi esquema esas lecturas son **insumos**, no valores contables: `token0`/`token1`, `getReserves`, `cast storage` de slots, calldata, direcciones y `msg.value=0` **no pertenecen a ninguna de las tres categorías** porque no son resultados de la operación hipotética.
**Mapeo declarado, sin ambigüedad:** `MEDIDO` (t48) = **capa de insumo** ⇒ alimenta E1-E2 (ESTIMADO) y el setup de S8 (SIMULADO). **`MEDIDO` no es una cuarta categoría contable** y no debe leerse como tal.
Misma clase de riesgo que la colisión #792/#793 de la campaña: dos etiquetas para la misma evidencia. Acá se reconcilian por escrito en vez de dejarlas convivir.

**H4 — `t48` §7 ya traía una frontera y era correcta en lo esencial.** Su tabla `ESTIMADO / SIMULADO / REALIZADO` (§7 L252-256) coincide con la mía en el reparto grueso, y su §10 L324 ya advertía *"realizado no existe y no debe aparecer como 0"*. **Este documento no la contradice: la endurece** — un valor por fila, una sola categoría por valor, la concordancia declarada como no-promoción y la migración `ESTIMADO → SIMULADO` con su techo.

**H5 — la etiqueta temporal de `t48` sobre `main`.** `t48` declara que al clonar `origin/main` era `c89d21a3…` (§1 L30). Hoy, en mi clon, **sigue siendo `c89d21a3…`**. La orden de mi tarea decía `3f00b359…`: esa referencia está **desactualizada** y la corrijo con medición propia, no con la cita del informe vecino.

---

## 7. VERIFICACIÓN — comandos y salidas exactas (categoría por categoría)

### 7.1 ESTIMADO — re-medido por mí al bloque pinneado

```
$ RPC=https://ethereum-rpc.publicnode.com ; B=26130440     # cast 1.7.2-nightly (c5e44b5e)
$ cast call 0xB4e16d0168e52d35CaCD2c6185b44281Ec28C9Dc "getReserves()(uint112,uint112,uint32)" --block $B --rpc-url $RPC
10530693072147 [1.053e13] / 3885899199257643317660 [3.885e21] / 1791254675 [1.791e9]
$ cast call 0x397FF1542f962076d0BFE58eA045FfA2d347ACa0 "getReserves()(uint112,uint112,uint32)" --block $B --rpc-url $RPC
144658924301 [1.446e11] / 53409874978380037333 [5.34e19] / 1791252647 [1.791e9]
$ cast call 0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D "getAmountsOut(uint256,address[])(uint256[])" 1000000000000000000 "[WETH,USDC]" --block $B --rpc-url $RPC
[1000000000000000000, 2701152913]
$ cast call 0xd9e1cE17f2641f24aE83637ab66a2cca9C378B9F "getAmountsOut(uint256,address[])(uint256[])" 2701152913 "[USDC,WETH]" --block $B --rpc-url $RPC
[2701152913, 976135106962143097]
# 10 WETH:  [1e19, 26949315944]  →  [26949315944, 8366252744341588826]
# 100 WETH: [1e20, 263425910836] →  [263425910836, 34440274425259215329]
```

**Las 8 lecturas coinciden exactamente con `t48` §2 y §3.** Aritmética derivada, recomputada aparte
(`node verify-accounting.mjs`, insumos = reservas de arriba; no toca red):

```
spot_uni 2709.975872 | spot_sushi 2708.467757 | marginal 0.994562478940 | b -0.005437521060
f* 10.145366684 (2dp-round 10.15 | 2dp-trunc 10.14)  ← H1
spread 5.568 bps | hurdle 60.271 bps | deficit 54.703 bps | ratio 10.82 | gas sum 250253
netos exactos: -23864893037856903 | -1633747255658411174 | -65559725574740784671
```

### 7.2 SIMULADO — una corrida reproducida por mí (independiente de `t48`)

```
$ cast call 0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D \
    "swapExactTokensForTokens(uint256,uint256,address[],address,uint256)" \
    0 0 "[0xC02a…6Cc2,0xA0b8…eB48]" 0x000000000000000000000000000000000000bEEF 9999999999 \
    --block 26130440 --trace --rpc-url https://ethereum-rpc.publicnode.com
Traces:
  [7716] 0x7a25…488D::swapExactTokensForTokens(0, 0, [0xC02a…6Cc2, 0xA0b8…eB48], 0x…bEEF, 9999999999 [9.999e9])
    ├─ [2504] 0xB4e16d…C9Dc::getReserves() [staticcall]
    └─ ← [Revert] UniswapV2Library: INSUFFICIENT_INPUT_AMOUNT
Gas used: 30392
Error: Transaction failed.
```

**Byte a byte igual a `t48` §6.A** (mismo revert, mismo gas `30392`). Esta corrida necesita **cero overrides**: revierte antes de tocar `transferFrom`, por eso es reproducible sin financiar al caller.
**Las corridas `S1`-`S6` NO las re-ejecuté**: requerirían replicar los overrides de balance/allowance de `t48` §6.0. Su artefacto es `ECON-PATH-01.md` §6.B/§6.C con las trazas verbatim; **mi categorización no depende de re-ejecutarlas, y lo declaro** en vez de presentar como propia una corrida que no hice.

### 7.3 REALIZADO — la ausencia, verificada al nivel que se puede

```
$ git grep -n "ACTUAL_GAS_PROVENANCE = " -- backend/api-server/src/routes/risk-circuit-breakers.ts   # en main c89d21a3
backend/api-server/src/routes/risk-circuit-breakers.ts:171:const ACTUAL_GAS_PROVENANCE = "sim-ctl replay via drift_tracker (not on-chain settled)";
$ git grep -n 'drift_mode == ' -- backend/recon/src/main.rs
backend/recon/src/main.rs:344:                if drift_mode == "on" {
$ git grep -n "ARBX_DRIFT_TRACKER_MODE" -- .env.example
.env.example:347:ARBX_DRIFT_TRACKER_MODE=off
```

Lectura: el único campo con nombre "actual" es **replay simulado** (por su propio string), y hoy **no tiene productor corriendo** (switch en `off`) ⇒ el costo de gas realizado es **NULL = ausencia**, nunca `0`.
**No hay ningún `tx hash` que citar en esta sección.** No existe, y no lo invento.

### 7.4 NO COMPUTADO — la frontera, con mis propios probes

```
$ sql_query: SELECT 1 AS ping
{ "ok": true, "data": "", "rows_affected": 0 }        # payload VACÍO — no es "0 filas"
$ sql_tables
{ "count": 0, "tables": [] }
```

**PostgreSQL inalcanzable desde esta sesión.** Por eso N1, N2 y N4 quedan **NO COMPUTADO con razón**, no cerrados con una estimación.

---

## 8. HALLAZGO DE UBICACIÓN: `docs/data/` está IGNORADO por git

El contrato de esta orden pone el entregable en **`docs/data/`**. Medido, ese directorio **no existe en el repo**:

```
$ git check-ignore -v docs/data/ECON-ACCOUNTING-01.md
.gitignore:73:data/	docs/data/ECON-ACCOUNTING-01.md
$ git ls-files docs/data | Measure-Object | % Count
0
```

La regla **`data/` de `.gitignore:73`** (sin ancla, sin `/` inicial) matchea **cualquier** directorio llamado `data` a cualquier profundidad — incluido `docs/data/`. En todo el historial del repo **no hay un solo archivo rastreado** bajo `docs/data/`.

**Contraste medido:** los directorios hermanos por perfil **sí** están rastreados (`docs/backend/ECON-PATH-01.md` de t48, `docs/sre/RUNTIME-IDENTITY-PROBE-01.md`, `docs/quant/GAS-COST-A6-COVERAGE-CONTRACT.md`).

**Qué hice y por qué:** el archivo se incorporó a la rama en la ruta exacta del contrato con `git add -f`, para no cambiar en silencio la ubicación que la orden fijó. **Queda declarado, no oculto.**
**Qué NO hice:** no edité `.gitignore` — está fuera de mi alcance (`In scope: docs/data/`). **Recomendación al operador:** si `docs/data/` es la ubicación canónica del perfil Data, la regla necesita una negación explícita (`!docs/data/`) en una orden aparte, porque hoy **cualquier** entregable escrito ahí es invisible para `git add` por defecto.

---

## 9. RESUMEN OPERATIVO EN UNA TABLA

| Capa | Existe | Categoría | Se puede usar para costear / decidir tamaño | Mueve P/N |
|---|---|---|---|---|
| Cotizaciones y aritmética (§3, §4 de t48) | sí | **ESTIMADO** | Sí, para **decidir** (y la decisión fue: no operar) | No |
| Trazas de fork (§6 de t48) | sí | **SIMULADO** | **No** para costear: el gas es de fork, no pagado | No |
| Settlement, firma, P&L realizado | **no** | **NO REALIZADO** | **No** | No |
| Ruta del sistema, `P/N`, conciliación con ledger | **no reconciliable** | **NO COMPUTADO** (N1-N4) | **No** | No |

**Veredicto contable en una línea:** la ruta de `t48` tiene **estimado** y **simulado** correctamente separados y **cero realizado**; el balance **no se cierra con el simulado**, y lo que no se puede conciliar queda declarado como **NO COMPUTADO** con su razón.

---

*Medición sobre estado on-chain real al bloque `26130440` por RPC público gratuito; el crudo de la capa ESTIMADO y una corrida de la capa SIMULADO re-medidos por mí; la capa REALIZADO declarada NO REALIZADA por ausencia de firma, broadcast e inclusión; PostgreSQL inalcanzable ⇒ la ruta del sistema y el contador `P/N` quedan NO COMPUTADOS. Sin firma, sin broadcast, sin capital, sin settlement. Este documento no mueve `P/N`.*
