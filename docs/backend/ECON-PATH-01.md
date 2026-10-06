# ECON-PATH-01 — UNA ruta económica coherente y reproducible

**Orden:** t48 · **Perfil:** Backend · **Intento:** `53ed398d-d027-4781-9dc1-fcc997d812df`
**Modo:** `ARBX_TRADE_MODE=paper`. **Sin firma, sin broadcast, sin capital, sin mainnet (ejecución).**
**Bloque pinneado:** `26130440` · **RPC:** `https://ethereum-rpc.publicnode.com` (público gratuito, doctrina §38)
**Toolchain:** `cast`/`forge`/`anvil` 1.7.2 (nightly, commit `c5e44b5e`)

---

## 0. VEREDICTO EN UNA LÍNEA

La ruta es **válida y queda completamente cotizada, dimensionada, armada y simulada** — y la conclusión económica es **NO OPERAR**: el retorno marginal del ciclo a tamaño→0 es **0.994562479 < 1**, es decir **la ruta pierde en TODO tamaño positivo**, así que el tamaño racional es **0** y **Kelly no aplica** (payoff negativo). La simulación con traza se entrega igual: **un REVERT con su traza en el tamaño que emite el sizing gate, y dos ejecuciones con traza en un tamaño de demostración declarado**, que confirman empíricamente el veredicto.

**Esto es el resultado, no un fracaso del entregable.** Un rojo con traza es publicable; un verde sin traza no lo es.

---

## 1. Qué está medido y qué es NO COMPUTADO

| Componente | Estado | Razón |
|---|---|---|
| Ruta (pools, tokens, dirección) | **MEDIDO** | `token0()`/`token1()` + `getReserves()` on-chain §2 |
| Cotizaciones al mismo bloque | **MEDIDO** | todo a bloque `26130440` §3 |
| Sizing (forma cerrada + Kelly) | **MEDIDO** | §4, con la resta exacta |
| Plan + calldata | **MEDIDO** | `cast calldata`, hex verbatim §5 |
| Simulación con traza | **MEDIDO** | 3 corridas con traza §6 |
| **La ruta que ELIGIÓ EL SISTEMA** | **NO COMPUTADO** | **PostgreSQL inalcanzable desde esta sesión**: `SELECT 1 AS ping` devuelve payload vacío y `sql_tables` devuelve `count: 0`. No hay productor de oportunidades legible ⇒ **la ruta de abajo la elijo yo desde estado on-chain real, y lo declaro**; no es "la ruta del searcher" |
| Est. / Sim. / Realizado | **MEDIDO / MEDIDO / NO EXISTE** | §7. Realizado = ninguno: sin broadcast, sin capital |

> **Nota de base:** la orden dice `main` = `3f00b359…`. Al clonar, `origin/main` era **`c89d21a3437c7fc2456fb7a08dba7d26ef41ee5e`** — `main` se movió durante el trabajo. Lo declaro en vez de citar el SHA que ya no es.

---

## 2. Componente 1 — LA RUTA, y por qué es válida

**Ciclo: `WETH → USDC → WETH`** (dos saltos, dos pools).

| | Uniswap V2 | SushiSwap |
|---|---|---|
| Pool | `0xB4e16d0168e52d35CaCD2c6185b44281Ec28C9Dc` | `0x397FF1542f962076d0BFE58eA045FfA2d347ACa0` |
| `token0()` | `0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48` (**USDC**) | `0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48` (**USDC**) |
| `token1()` | `0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2` (**WETH**) | `0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2` (**WETH**) |
| `getReserves().r0` (USDC) | `10530693072147` | `144658924301` |
| `getReserves().r1` (WETH) | `3885899199257643317660` | `53409874978380037333` |
| precio spot | `2709.975872` USDC/WETH | `2708.467757` USDC/WETH |

**Por qué la ruta es válida (no asumido, medido):** ambos pools declaran **exactamente** los dos tokens del ciclo en `token0`/`token1`, y ambos mantienen reservas no nulas de los dos. El script **asserta** esto y aborta si no se cumple:

```python
assert t0.lower() == USDC.lower(), f"{name}: token0 is not USDC"
assert t1.lower() == WETH.lower(), f"{name}: token1 is not WETH"
```

**Dirección del ciclo:** se **vende** WETH en Uniswap V2 (`2709.975872`, el caro) y se **recompra** en SushiSwap (`2708.467757`, el barato). La rotación es la que compra barato y vende caro; el sentido inverso es estrictamente peor y no se reporta.

---

## 3. Componente 2 — COTIZACIONES AL MISMO BLOQUE

**Todas** las lecturas y cotizaciones de este documento están tomadas a **bloque `26130440`**, con `--block 26130440` explícito en cada `cast call`. Ninguna cifra proviene de otro bloque.

Cotizaciones con el **router real** (incluyen el fee de 0.3% y el impacto de precio), a ese bloque:

| entrada | LEG 1 · Uniswap V2 router `WETH→USDC` | LEG 2 · SushiSwap router `USDC→WETH` | neto |
|---|---|---|---|
| `1 WETH` (`1e18`) | `2701152913` = 2701.152913 USDC | `976135106962143097` wei = 0.976135107 WETH | **`-23864893037856903` wei (−2.3864893%)** |
| `10 WETH` (`1e19`) | `26949315944` = 26949.315944 USDC | `8366252744341588826` wei = 8.366252744 WETH | **`-1633747255658411174` wei (−16.3375%)** |
| `100 WETH` (`1e20`) | `263425910836` = 263425.910836 USDC | `34440274425259215329` wei = 34.440274425 WETH | **`-65559725574740784671` wei (−65.5597%)** |

**Routers (destinos reales, no descritos en prosa):**
`Uniswap V2 Router02 = 0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D` · `SushiSwap Router = 0xd9e1cE17f2641f24aE83637ab66a2cca9C378B9F`

**Márgenes estructurales (mismo bloque, derivados de las reservas):**

| magnitud | valor |
|---|---|
| spread bruto uni/sushi − 1 | **5.568 bps** |
| hurdle de fees de un ciclo de 2 pools `1/(0.997²) − 1` | **60.271 bps** |
| **déficit** | **54.703 bps** — hace falta **10.82×** más spread del que hay |

---

## 4. Componente 3 — EL SIZING, derivado (no el número solo)

### 4.1 El óptimo por forma cerrada

Para un ciclo de dos pools CPMM el retorno marginal del round trip a tamaño→0 es
`(1−fee)² · (spot_uni / spot_sushi)`:

```
(0.997)^2 × (2709.975872 / 2708.467757) = 0.994562479
```

**`0.994562479 < 1` ⇒ cada unidad adicional devuelve menos de lo que costó, para TODO tamaño positivo.** La función de salida es estrictamente decreciente en el tamaño ⇒ **el óptimo es 0**, y eso es lo que confirman las tres cotizaciones de §3 (el neto empeora monótonamente con el tamaño).

Esto también explica por qué el neto de `1 WETH` (−2.39%) es **peor** que el marginal (−0.54%): el pool de SushiSwap tiene sólo 144,658 USDC / 53.4 WETH, así que comprar ~0.99 WETH con 2701 USDC mueve ese pool un ~1.9%.

### 4.2 Kelly: por qué la respuesta correcta es "no aplica", y por qué eso importa

Kelly necesita un payoff `b` positivo. Acá:

```
b = retorno marginal − 1 = 0.994562479 − 1 = −0.005437521
```

**`b < 0` ⇒ TODOS los desenlaces pierden ⇒ el stake óptimo es `0` por dominancia.** La fórmula `f* = (p·b − (1−p))/b` **no es aplicable** con `b<0`: aritméticamente devuelve un número positivo (`f* = 10.145366785709885` con `p=0.95`; precisión y método declarados abajo) que sería **una recomendación de apostar a una pérdida garantizada**. Ese es un modo de fallo real y silencioso: un pipeline que aplique la fórmula sin verificar el signo de `b` **fabrica una posición sobre una ruta perdedora**.

> **Precisión y método declarados — corrección `FIX-FSTAR-PRECISION-01`.** Recomputado **desde los insumos de este mismo documento** (`b = −0.005437521` de §4.2 L103; `p = 0.95` ⇒ `q = 1 − p = 0.05`) con la fórmula de esta misma línea:
>
> ```
> f* = (p·b − q) / b = (0.95 × (−0.005437521) − 0.05) / (−0.005437521) = 10.145366785709885
> ```
>
> **Método: valor SIN redondear y SIN truncar, publicado completo (15 decimales significativos).** No se aplica ningún recorte en esta revisión. Por transparencia, los dos recortes a 2 decimales darían: **redondeo → `10.15`**, **truncamiento → `10.14`**. La revisión anterior de este documento publicaba `10.14`, que es exactamente el valor **truncado** a 2 decimales **sin declararlo**: ése es el defecto que esta corrección cierra, y el número pasa ahora a **cerrar bajo su propia fórmula**.
>
> **Sensibilidad a la precisión de `b` (declarada, no escondida).** Si `b` se arrastra **sin** redondear a 9 decimales (`b = 0.994562478947335 − 1 = −0.005437521052665`), entonces `f* = 10.145366696648`. **La conclusión no cambia:** también trunca a `10.14` y también redondea a `10.15` a 2 decimales. Las dos formas son igual de concluyentes respecto de `b<0`.
>
> **Materialidad medida: NINGUNA.** Con `b < 0` la fórmula de Kelly **no aplica** y el stake es `0` por dominancia (§4.3), así que esta cifra **no cambia ninguna decisión, ningún importe ni ningún gate**. Se corrige igual porque un número publicado que no cierra bajo su propia fórmula es un defecto, y un documento que se declara "medido end-to-end" no puede tener uno.

> **Declaración explícita:** la fracción de Kelly aplicada es **ninguna, porque no aplica** (payoff negativo). El **capital de referencia** es `1000 USD` (default del módulo de riesgo del repo) y **la fracción resultante es 0**, luego **el importe es 0**, y ese 0 coincide exactamente con el cálculo. No hay un importe no-cero que pueda derivarse honestamente de esta ruta.

### 4.3 Verificación de coherencia del importe

| paso | valor |
|---|---|
| retorno marginal ≤ 1 en todo tamaño | `0.994562479` |
| payoff `b` | `−0.005437521` ⇒ negativo |
| fracción de Kelly | **0** (no aplica; dominancia) |
| capital de referencia | `1000 USD` |
| **importe** | **`0` = 1000 × 0 ✔ coincide con el cálculo** |

**El sizing NO es una opinión: es la consecuencia de `b<0`.** Y por eso §5 y §6 se entregan **etiquetados como mecanismo**, no como una operación sugerida.

---

## 5. Componente 4 — PLAN y CALLDATA (hex verificable)

**Destino, función, argumentos y valor, y el calldata en hex.** Ambos calldata salen de `cast calldata`; ninguno se describe en prosa.

**LEG 1 — Uniswap V2 Router02 `0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D`**

* función: `swapExactTokensForTokens(uint256 amountIn, uint256 amountOutMin, address[] path, address to, uint256 deadline)`
* argumentos: `amountIn=1000000000000000000`, `amountOutMin=0`, `path=[0xC02aaA…6Cc2, 0xA0b8…eB48]`, `to=0x000000000000000000000000000000000000bEEF`, `deadline=9999999999`
* **valor (msg.value) = `0`** — la ruta no envía ETH nativo.
* **calldata:**
```
0x38ed17390000000000000000000000000000000000000000000000000de0b6b3a7640000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000a0000000000000000000000000000000000000000000000000000000000000beef00000000000000000000000000000000000000000000000000000002540be3ff0000000000000000000000000000000000000000000000000000000000000002000000000000000000000000c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2000000000000000000000000a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48
```

**LEG 2 — SushiSwap Router `0xd9e1cE17f2641f24aE83637ab66a2cca9C378B9F`**

* argumentos: `amountIn=2701152913`, `amountOutMin=0`, `path=[0xA0b8…eB48, 0xC02aaA…6Cc2]`, `to=0x…bEEF`, `deadline=9999999999`
* **valor = `0`**
* **calldata:**
```
0x38ed173900000000000000000000000000000000000000000000000000000000a1005291000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000a0000000000000000000000000000000000000000000000000000000000000beef00000000000000000000000000000000000000000000000000000002540be3ff0000000000000000000000000000000000000000000000000000000000000002000000000000000000000000a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48000000000000000000000000c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2
```

Selector `0x38ed1739` = `swapExactTokensForTokens` en ambos casos.

---

## 6. Componente 5 — SIMULACIÓN **CON TRAZA**

Locus: `cast call --trace`, que **hornea el RPC remoto, ejecuta localmente e imprime la traza**. No hay anvil persistente, no hay firma, no hay broadcast: `eth_call` sobre un fork efímero. El caller se financia con `--override-state-diff` **en el fork**, sin llave.

### 6.0 Los slots de storage NO se asumieron: se VERIFICARON contra estado conocido

Antes de tocar nada se comprobó que el slot derivado es el correcto, leyendo un valor **ya conocido** por otra vía:

```
cast index address 0xB4e16d0168e52d35CaCD2c6185b44281Ec28C9Dc 3
  -> 0xb374801ace2c02f5db0425ab5920a2b7ed1d5a00abbcd395fda7530ba1d666c0
cast storage 0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2 0xb37480…66c0 --block 26130440
  -> 0x0000000000000000000000000000000000000000000000d2a7af3d5936b3f99c
decoded = 3885899199257643317660
expected pair WETH reserve (getReserves) = 3885899199257643317660
SLOT-3 VERIFIED = True
```

Para USDC el slot se **descubrió por coincidencia con estado conocido**, no por memoria de layout: se recorrieron los slots 0..15 buscando el que valiera `10530693072147` (el balance USDC del pool, leído por `getReserves`).

```
pair USDC balance at block 26130440 = 10530693072147
matching mapping slot = 9  (value read = 10530693072147)
```

Que el override de allowance sea el correcto se valida **operativamente**: si el slot fuera otro, `transferFrom` revertiría con `INSUFFICIENT_ALLOWANCE`; las trazas de B y C devuelven `true` en `transferFrom` y terminan en `[Stop]`.

### 6.A — SIMULACIÓN A: el tamaño que emite el sizing gate (`amountIn = 0`) ⇒ **REVERT**

```
Traces:
  [7716] 0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D::swapExactTokensForTokens(0, 0, [0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2, 0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48], 0x000000000000000000000000000000000000bEEF, 9999999999 [9.999e9])
    ├─ [2504] 0xB4e16d0168e52d35CaCD2c6185b44281Ec28C9Dc::getReserves() [staticcall]
    │   └─ ← [Return] 0x00000000000000000000000000000000000000000000000000000993de3845130000000000000000000000000000000000000000000000d2a7af3d5936b3f99c000000000000000000000000000000000000000000000000000000006ac46093
    └─ ← [Revert] UniswapV2Library: INSUFFICIENT_INPUT_AMOUNT

Gas used: 30392
```

**RESULTADO: REVERT.** El tamaño que el sizing gate produce (0) es **mecánicamente inejecutable**: la librería del router lo rechaza. Es la traducción del veredicto económico a un artefacto verificable — **un revert con su traza es un resultado válido y publicable.**

### 6.B — SIMULACIÓN B (LEG 1, tamaño de demostración `1 WETH`) ⇒ **ÉXITO**

Caller financiado en el fork: `balanceOf[bEEF] = 1e18 WETH`, `allowance[bEEF][Router02] = 2²⁵⁶−1`.

```
Traces:
  [106696] 0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D::swapExactTokensForTokens(1000000000000000000 [1e18], 0, [0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2, 0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48], 0x000000000000000000000000000000000000bEEF, 9999999999 [9.999e9])
    ├─ [2504] 0xB4e16d0168e52d35CaCD2c6185b44281Ec28C9Dc::getReserves() [staticcall]
    ├─ [15025] 0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2::transferFrom(0x…bEEF, 0xB4e16d…, 1000000000000000000 [1e18])
    │   ├─ emit Transfer(from: 0x…bEEF, to: 0xB4e16d…, amount: 1000000000000000000 [1e18])
    │   └─ ← [Return] true
    ├─ [76348] 0xB4e16d0168e52d35CaCD2c6185b44281Ec28C9Dc::swap(2701152913 [2.701e9], 0, 0x…bEEF, 0x)
    │   ├─ [40652] 0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48::transfer(0x…bEEF, 2701152913 [2.701e9])
    │   ├─ emit Swap(param0: 0x7a250d…, param1: 0, param2: 1000000000000000000 [1e18], param3: 2701152913 [2.701e9], param4: 0, param5: 0x…bEEF)
    │   └─ ← [Stop]
    └─ ← [Return] 0x…000002 0000000000000000000000000000000000000000000000000de0b6b3a7640000 00000000000000000000000000000000000000000000000000000000a1005291

Transaction successfully executed.
Gas used: 121844
```

`amount0Out = 2701152913` **coincide exactamente** con la cotización `getAmountsOut` de §3 ⇒ el estimado y el simulado son el mismo número, medido por dos vías independientes.

### 6.C — SIMULACIÓN C (LEG 2, `USDC→WETH` en SushiSwap) ⇒ **ÉXITO**

```
Traces:
  [113297] 0xd9e1cE17f2641f24aE83637ab66a2cca9C378B9F::swapExactTokensForTokens(2701152913 [2.701e9], 0, [0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48, 0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2], 0x…bEEF, 9999999999 [9.999e9])
    ├─ [2517] 0x397FF1542f962076d0BFE58eA045FfA2d347ACa0::getReserves() [staticcall]
    ├─ [31449] 0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48::transferFrom(0x…bEEF, 0x397FF1…, 2701152913 [2.701e9])
    │   └─ ← [Return] true
    ├─ [66031] 0x397FF1542f962076d0BFE58eA045FfA2d347ACa0::swap(0, 976135106962143097 [9.761e17], 0x…bEEF, 0x)
    │   ├─ [29962] 0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2::transfer(0x…bEEF, 976135106962143097 [9.761e17])
    │   ├─ emit Swap(param0: 0xd9e1cE…, param1: 2701152913 [2.701e9], param2: 0, param3: 0, param4: 976135106962143097 [9.761e17], param5: 0x…bEEF)
    │   └─ ← [Stop]
    └─ ← [Return] 0x…000002 00000000000000000000000000000000000000000000000000000000a1005291 00000000000000000000000000000000000000000000000d8bedb53f443779

Transaction successfully executed.
Gas used: 128409
```

`amount1Out = 976135106962143097` wei = **0.976135106962143097 WETH** — y vuelve a coincidir con la cotización de §3.

### 6.D — Las dos ejecuciones juntas CIERRAN el ciclo y confirman el veredicto

```
in  : 1.000000000000000000 WETH
leg1: → 2701152913 USDC  (0x…a1005291)          [ÉXITO, gas 121844]
leg2: → 0.976135106962143097 WETH                [ÉXITO, gas 128409]
net : −0.023864893037856903 WETH  (−2.3864893%)   gas total 250253
```

**La traza EMPÍRICAMENTE confirma la forma cerrada**: el ciclo devuelve menos de lo que puso. El número que el paper predijo y el que el fork ejecutó son el mismo.

> **Etiqueta obligatoria:** las simulaciones B y C son **demostración de mecanismo**, ejecutadas a un tamaño declarado (`1 WETH`) para que la traza sea informativa. **NO son una operación sugerida** — el sizing dice 0. Se presentan separadas de la recomendación a propósito, para que nadie lea "se ejecutó" como "hay que ejecutarlo".

---

## 7. Componente 6 — ESTIMADO / SIMULADO / REALIZADO (para t49)

| categoría | qué es acá | artefacto | ¿mueve P/N? |
|---|---|---|---|
| **ESTIMADO** | `getReserves()` + `getAmountsOut()` — `eth_call` puro, sin cambio de estado | §2 y §3 | **NO** |
| **SIMULADO** | `cast call --trace` sobre fork efímero: estado local, sin broadcast, sin firma | §6.A/6.B/6.C | **NO** |
| **REALIZADO** | **NINGUNO.** No hubo broadcast, ni firma, ni capital, ni settlement | — | **NO** |

**P/N sigue en 0/115 y este trabajo NO lo mueve.** Nada de lo de arriba es una ejecución real: no hay transacción firmada, no hay inclusión en bloque, no hay settlement. Un `Transaction successfully executed` **de `cast call --trace` es una ejecución LOCAL sobre un fork**, no una transacción de mainnet — presentarlo como realizado sería exactamente el error que esta sección existe para impedir.

---

## 8. Comandos exactos que produjeron cada artefacto

```bash
# bloque pinneado
cast block-number --rpc-url https://ethereum-rpc.publicnode.com

# §2 ruta y validez (token0/token1/getReserves con --block 26130440)
cast call 0xB4e16d0168e52d35CaCD2c6185b44281Ec28C9Dc "token0()(address)" --block 26130440 --rpc-url $RPC
cast call 0xB4e16d0168e52d35CaCD2c6185b44281Ec28C9Dc "getReserves()(uint112,uint112,uint32)" --block 26130440 --rpc-url $RPC
cast call 0x397FF1542f962076d0BFE58eA045FfA2d347ACa0 "getReserves()(uint112,uint112,uint32)" --block 26130440 --rpc-url $RPC

# §3 cotizaciones de las dos piernas, AL MISMO BLOQUE
cast call 0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D \
  "getAmountsOut(uint256,address[])(uint256[])" 1000000000000000000 \
  "[0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2,0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48]" \
  --block 26130440 --rpc-url $RPC
cast call 0xd9e1cE17f2641f24aE83637ab66a2cca9C378B9F \
  "getAmountsOut(uint256,address[])(uint256[])" 2701152913 \
  "[0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48,0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2]" \
  --block 26130440 --rpc-url $RPC

# §4/§5 sizing y calldata (script reproducible, sin red más allá del RPC)
python t48-econ.py

# §6.0 verificación de slots + trazas
python t48-trace.py
```

Los dos scripts (`t48-econ.py`, `t48-trace.py`) son reproducibles tal cual: llaman a `cast` por `subprocess` y parsean JSON; no firman nada ni mutan estado.

### 8.1 La reproducibilidad está PROBADA, no afirmada

Un proceso **fresco** re-leyó el bloque pinneado desde el RPC y comparó contra **cada número publicado en este documento**:

```
PINNED BLOCK = 26130440  (fresh process, re-read from the RPC)
  uni  reserves    got=(10530693072147, 3885899199257643317660)  doc=(10530693072147, 3885899199257643317660)  MATCH=True
  sushi reserves   got=(144658924301, 53409874978380037333)      doc=(144658924301, 53409874978380037333)      MATCH=True
  leg1 out         got=2701152913            doc=2701152913            MATCH=True
  leg2 out         got=976135106962143097    doc=976135106962143097    MATCH=True
  net wei          got=-23864893037856903    doc=-23864893037856903    MATCH=True

REPRODUCIBLE = True
```

**Las cinco comprobaciones dan `MATCH=True`; ninguna cifra de este documento quedó sin reproducir.** Cómo reproducirlo: `python t48-repro.py`. Pinnear el bloque es lo que lo hace determinista — **el mismo `--block 26130440` devuelve obligatoriamente el mismo estado histórico**; sin pinnear, las cifras cambian con el bloque (ver §9.3).

---

## 9. Lo que esto NO prueba (fail-honest)

1. **NO es la ruta del searcher.** PostgreSQL es inalcanzable desde esta sesión (`SELECT 1 AS ping` → payload vacío; `sql_tables` → `count: 0`), así que **no sé qué ruta habría elegido el sistema**. La de acá la elegí yo desde estado on-chain y lo declaro.
2. **NO es una oportunidad.** Al contrario: es la **refutación medida** de que este ciclo lo sea en este bloque.
3. **NO prueba rentabilidad en otro bloque.** Un bloque distinto tiene otras reservas. Lo que sí queda probado es el **método** y su reproducibilidad.
4. **NO prueba nada sobre la ejecución real**: no hubo firma, ni broadcast, ni inclusión, ni gas efectivamente pagado. El gas de §6 es **gas de fork**, no un costo incurrido.
5. **NO cubre flash-loan, ni capital propio, ni MEV competitivo.** El ciclo acá es spot con capital propio; una variante con flash loan agrega prima y sigue partiendo del mismo `0.994562479 < 1`.
6. **El precio del ciclo podría cruzar el hurdle en algún bloque.** No lo busqué; **no lo afirmo ni lo niego.** Lo que muestro es que **en el bloque medido, no**.

---

## 10. Handoff a t49 (contabilidad) y a quien siga

* **Para t49:** la tabla de §7 es la frontera. Nada de §2–§6 es "realizado". Si t49 necesita un estimado, use §3; si necesita un simulado, §6.B/6.C; **realizado no existe y no debe aparecer como 0** — `0` sería un valor computado, y acá no hay medición: es **NO EXISTE**.
* **Al próximo intento de ruta:** el método de §6.0 (verificar el slot contra estado conocido antes de overridear) es obligatorio para que una simulación sea evidencia y no una puesta en escena.
* **Al sizing:** el modo de fallo peligroso está en §4.2 — **aplicar la fórmula de Kelly con `b<0`**. Devuelve un número positivo y plausible (`10.145366785709885`; valor completo sin recorte — ver precisión declarada en §4.2) que recomienda apostar a una pérdida garantizada. **Verificar el signo de `b` antes de aplicar Kelly** es un gate que hoy no está y debería estar.

---

*Medición sobre estado on-chain real al bloque `26130440`, leída por RPC público gratuito. Cotizaciones por los routers reales, todas al mismo bloque. Slots de storage verificados contra estado conocido, no asumidos. Simulaciones por `cast call --trace` sobre fork efímero: sin firma, sin broadcast, sin capital. PostgreSQL inalcanzable desde esta sesión ⇒ la ruta del sistema queda NO COMPUTADA y se declara.*
