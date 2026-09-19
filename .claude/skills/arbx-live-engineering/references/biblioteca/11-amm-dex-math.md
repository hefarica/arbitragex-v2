# 11. MATEMÁTICA DE AMMs Y DEXs — CPMM, Concentrated Liquidity, Stableswap, Weighted

CUÁNDO CARGAR ESTA REFERENCIA: implementar o auditar el cálculo de quotes/swaps para cualquier
tipo de pool (Uniswap V2 y forks, V3, Curve stable, Balancer weighted/boosted); trabajar el
sizing óptimo de un trade (closed-form 2-pool, búsqueda iterativa multi-hop, integración de gas
y flash loan fee); debugging de `expected_profit` que no coincide con el resultado simulado
(fee en dirección incorrecta, redondeo adversario, decimales mezclados); elegir oráculo para
pricing de decisiones (spot vs TWAP vs Chainlink vs mediana); portar crates de matemática AMM
(`uniswap-v3-math`, `amms`, `curve-stableswap`); integrar Uniswap V4 o forks con
hooks/dynamic fee (Algebra-style) — singleton, unlock, flash accounting, quoting (§11.10);
diagnosticar slippage anómalo al cruzar ticks
o rutas que atraviesan rangos sin liquidez.

| Concepto | Herramienta/Patrón | Nota clave |
|---|---|---|
| Quote exacta CPMM | `UniswapV2Library.getAmountOut/In` | fee sobre el INPUT; floor en output, ceil en input |
| Concentrated liquidity | crate `uniswap-v3-math` (`TickMath`, `SqrtPriceMath`, `SwapMath`) | swap por steps hasta tick boundary; fee por step |
| `sqrtPriceX96` | Q64.96 + `FullMath.mulDiv` (512-bit intermedio) | multiplicar naive en 256 bits overflowea (2^192·2^96) |
| Curve stableswap | crate `curve-stableswap`; on-chain `get_dy(i,j,dx)` | Newton para D; NO closed-form para sizing |
| Balancer weighted | invariante prod. ponderado + `IVault.queryBatchSwap` | spot = (w_i·B_o)/(w_o·B_i), no B_o/B_i |
| TWAP V3 | `pool.observe(uint32[])` sobre `tickCumulative` | promedio GEOMÉTRICO del precio; costo ∝ L·T |
| Chainlink | `AggregatorV3Interface.latestRoundData()` | leer `decimals()` del feed; check staleness `updatedAt` |
| Sizing 2-pool CPMM | closed-form §11.6 (igualación de precio marginal neto) | máximo único, cóncavo |
| Sizing multi-hop | ternary search / Newton sobre π(x) cóncava | gas = término lineal en el token de settlement |
| Decimales mixtos | normalizar 10^(d_in−d_out) DESPUÉS de la curva | minOut siempre en unidades raw del token de salida |
| Reserves manipuladas | re-simulación (revm/fork) en bloque objetivo + `amountOutMin` | quote cacheada del bloque N−1 es stale en N |

## 11.1 CONSTANT PRODUCT (Uniswap V2 / CPMM)

### 11.1.1 Derivación exacta

Invariante con fee: el swap mantiene `(x + γΔx)·(y − Δy) = x·y` donde γ es la fracción del input
que llega a la curva (V2: γ = 997/1000). Despejando:

```
Δy = γΔx·y / (x + γΔx)          (givenIn)
Δx = x·Δy / (γ·(y − Δy))        (givenOut)
```

Verificación contra el código real (`@uniswap/v2-periphery`, `UniswapV2Library.sol`):

```solidity
// getAmountOut — fee aplicado sobre el INPUT, floor en el output (división trunca)
function getAmountOut(uint amountIn, uint reserveIn, uint reserveOut)
    internal pure returns (uint amountOut)
{
    require(amountIn > 0, 'INSUFFICIENT_INPUT_AMOUNT');
    require(reserveIn > 0 && reserveOut > 0, 'INSUFFICIENT_LIQUIDITY');
    uint amountInWithFee = amountIn.mul(997);
    uint numerator = amountInWithFee.mul(reserveOut);
    uint denominator = reserveIn.mul(1000).add(amountInWithFee);
    amountOut = numerator / denominator;
}

// getAmountIn — floor + 1 = CEIL del input: garantizar que el output se alcanza
function getAmountIn(uint amountOut, uint reserveIn, uint reserveOut)
    internal pure returns (uint amountIn)
{
    require(amountOut > 0, 'INSUFFICIENT_OUTPUT_AMOUNT');
    uint numerator = reserveIn.mul(amountOut).mul(1000);
    uint denominator = reserveOut.sub(amountOut).mul(997);
    amountIn = (numerator / denominator).add(1);
}
```

Generalización a fee f en bps (forks con 0.25%/0.1%/1%): reemplazar 997/1000 por
`(10000−f)/10000`. Para fees "dinámicas" o aplicadas sobre el OUTPUT (algunos forks): la
dirección del fee cambia el redondeo seguro — nunca portar la fórmula sin leer el contrato
del fork (factory o par: getter `swapFee()`/`fee()` o constante en el bytecode; `feeToSetter`
solo designa el destinatario de la fee de protocolo, NO define el swapFee).

### 11.1.2 Por qué NUNCA redondear a favor del trader

Regla direccional (el adversario es el pool):

- Todo lo que RECIBES del pool → floor (conservador). Si tu motor redondea el output hacia
  arriba, `expected_profit` queda sobreestimado: pasa el gate de profit y la tx real devuelve
  1..N wei menos → pérdida en gas o revert por `amountOutMin`.
- Todo lo que PAGAS al pool → ceil (`.add(1)` en getAmountIn). Redondear el input hacia abajo
  produce reverted txs sistemáticas por output insuficiente.

Un wei parece irrelevante hasta que el gate de profit opera en el margen: con thresholds de
sub-centavo, un redondeo adversario de 3 hops decide pass/reject. La política del repo es
fail-honest (RULE 00 / R8): el cálculo offline debe reproducir bitwise lo que el pool hará.

### 11.1.3 Reserves manipuladas dentro del mismo bloque

`getReserves()` (uint112, uint112, uint32) es una fotografía del último `swap/sync`. Dentro del
bloque N tu bundle compite por posición: una tx previa del mismo bloque cambia reserves y tu
quote del bloque N−1 queda stale. Vectores ofensivos (sandwich/backrun sobre tu ruta) se tratan
EXCLUSIVAMENTE como riesgos a detectar y mitigar — ver skill `arbx-mev-ethics-gate`. Mitigación
técnica en capas:

1. `amountOutMin` estricto POR HOP (no solo al final de la ruta) — cada hop es un punto de
   revert barato.
2. Re-simulación en el estado del bloque objetivo (revm con state override o fork Anvil; el
   pipeline ya lo hace, ver núcleo §1.2): la matemática offline es filtro, la simulación es
   veredicto (`arbx-simulation-mandatory`).
3. Para pricing de decisiones (no ejecución) nunca usar spot de pools movibles: TWAP/Chainlink
   (§11.5).

Propiedad útil para bounds: en V2 el producto k observado nunca DECRECE por un swap — la fee
se acumula dentro de reserves, así que k tras cada swap es ≥ al previo (y también crece con
mint/burn). El output de un hop está acotado por la reserva de salida: Δy < y siempre. En sizing, un plan que requiera Δy ≥ y del
hop siguiente es inválido por construcción.

## 11.2 CONCENTRATED LIQUIDITY (Uniswap V3)

### 11.2.1 Geometría: precio, tick, sqrtPriceX96

- Precio real: `price = (sqrtPriceX96 / 2^96)^2` (token1 por token0), Q64.96 fixed point.
- Relación tick↔precio: `price = 1.0001^tick`, es decir `tick = log(price)/log(1.0001)`.
  Un tick de spacing s mueve el precio un factor `1.0001^s`.
- Límites: `MIN_TICK = -887272` → `MIN_SQRT_RATIO = 4295128739`; `MAX_TICK = 887272` →
  `MAX_SQRT_RATIO = 1461446703485210103287273052203988822378723970342`.
- Fee tiers (100/500/3000/10000 hundredths of a bip = 0.01%/0.05%/0.3%/1%) con tickSpacing
  1/10/60/200 respectivamente. Solo ticks múltiplos del spacing pueden tener liquidez.

### 11.2.2 Liquidez L por rango

Una posición [P_a, P_b] aporta liquidez L constante mientras el precio está dentro del rango:

```
Δamount0 = L · (1/√P_a − 1/√P_b)     (token0 real comprometido si P recorre el rango)
Δamount1 = L · (√P_b − √P_a)         (token1)
```

En Q96 (forma de `SqrtPriceMath.getAmount0Delta/getAmount1Delta`, ambos con flag `roundUp`):

```
amount0 = mulDiv(L << 96, √B − √A, √A·√B)      // √A·√B como producto de X96
amount1 = mulDiv(L, √B − √A, 2^96)
```

`liquidityNet` (int128, campo de `ticks(int24)`) es la liquidez neta que ENTRA (positiva) o
SALE (negativa) al cruzar ese tick: la liquidez activa del pool (`pool.liquidity()`, uint128)
solo cambia al cruzar ticks inicializados. Consecuencia operativa: un pool puede tener
`liquidity() == 0` en el precio actual (todos los LPs quedaron a un lado del rango) — el swap
entonces mueve el precio a través de ticks vacíos gastando gas SIN transferir output, hasta
encontrar liquidez o agotar gas. Error clásico del searcher: agregar depth de V3 como un número
(L total o reserves de API) sin modelar por-rango; el sizing correcto simula el cruce de ticks.

### 11.2.3 El swap-step loop

Firma real del core: `SwapMath.computeSwapStep(sqrtRatioCurrentX96, sqrtRatioTargetX96,
liquidity, amountRemaining, feePips) → (amountIn, amountOut, sqrtRatioNextX96, feeAmount)`
(amountRemaining es int256: positivo = exactIn, negativo = exactOut). El pool
(`uniswapv3pool.swap(recipient, zeroForOne, amountSpecified, sqrtPriceLimitX96, data)`) itera:

```
┌──────────────────────────────────────────────────────────────────┐
│ while amountRemaining ≠ 0 y sqrtPrice ≠ sqrtPriceLimit:          │
│   1. target = sqrtRatio del tick inicializado más cercano        │
│      (límite del step: MIN/MAX_SQRT_RATIO o sqrtPriceLimitX96)   │
│   2. step = computeSwapStep(precio→target, L, resto, fee)        │
│      · exactIn: fee se cobra SOLO sobre el amountIn consumido    │
│      · si el resto no alcanza el target → step parcial,          │
│        sqrtPriceNext = getNextSqrtPriceFromInput(...)            │
│   3. si el step llegó al target: cruzar tick →                   │
│      liquidity += ticks[tick].liquidityNet  (signo según         │
│      zeroForOne); TickBitmap avanza al próximo inicializado      │
│   4. acumular amountIn/amountOut/feeAmount del step              │
└──────────────────────────────────────────────────────────────────┘
```

El fee NO es una única multiplicación: se cobra por step sobre el input de ese step. Una ruta
que cruza 40 ticks paga 40 fracciones de fee exactamente calculadas — replicar V3 como "V2 con
reserves virtuales y fee plano" da un error sistemático que crece con el tamaño del trade.

Slippage fuera de rango: al alejarse del precio actual la liquidez por rango decae (o se
agota) → el precio marginal del siguiente wei crece de forma discontinua en cada tick cruzado.
Para el motor: el quote V3 es una función escalonada suave por tramos, no una hipérbola.

### 11.2.4 Herramientas

- Crate `uniswap-v3-math` (crate.io): puertos auditados de `TickMath`, `SqrtPriceMath`,
  `SwapMath`, `TickBitmap` — port-with-validation según `docs/security/FUSILE_SOURCE_POLICY.md`.
- Crate `amms` (amms-rs): discovery/sync de pools V2/V3 + ERC-4626 con estado en memoria.
- Siempre contrastar contra `eth_call` al pool real (o simulación revm) antes de ejecutar.

## 11.3 CURVE STABLESWAP

### 11.3.1 Invariante con amplificación

Con n monedas, balances x_i escalados a la misma precisión (xp_i), Ann = A·n^n:

```
Ann·Σx_i + D = Ann·D + D^(n+1) / (n^n · Πx_i)
```

A es adimensional (típicamente 100–10000): A=0 degenera en constant product puro; A→∞ degenera
en constant sum. Eso explica el peg-to-peg: cerca del equilibrio la curva es casi plana
(liquidez profunda: cientos de millones rotan en pocos bps) y se endurece hacia CPMM en los
extremos, donde el precio se aparta del peg.

### 11.3.2 Resolución de D por Newton-Raphson

No hay closed-form para D (n≥2 en la forma amplificada). La iteración de la implementación de
referencia (patrón de `get_D` en los pools StableSwap de Curve, con convergencia cuadrática,
hasta 255 iteraciones, corte cuando |ΔD| < 1 unidad):

```
S = Σxp_i;  D = S
loop:
    D_P = D
    para cada xp_i:  D_P = D_P · D / (xp_i · n)     # termina siendo D^(n+1)/(n^n·Πxp)
    D_prev = D
    D = (Ann·S + n·D_P) · D / ((Ann−1)·D + (n+1)·D_P)
    si |D − D_prev| < 1: break
```

Es exactamente Newton sobre `f(D) = D^(n+1)/(n^n·Πx) + Ann·D − Ann·S − D` (derivable en 5
líneas; el denominador `(n+1)·D_P + (Ann−1)·D` es `f'(D)·D` reordenado). El cálculo de
`get_y` (balance de salida dado D) usa el mismo esquema Newton. Para el motor: crate
`curve-stableswap` (curvefi/curve-stableswap-rs) expone estos cálculos en Rust; en cadena,
`get_dy(i, j, dx)` ya devuelve el output neto de fee (pools clásicos: fee estática descontada
del INPUT — `dx_w_fee = dx − dx·fee/1e10`; pools más nuevos tipo stableswap-NG la cobran sobre
el OUTPUT — verificar por pool). Cuidado con la aridad por generación: pools legacy
`get_dy(int128,int128,uint256)`, pools nuevos `get_dy(uint256,uint256,uint256)`.

### 11.3.3 Fee dinámica y cripto pools

Los pools clásicos tienen fee estática (`fee()` en 1e10 denominación). Los crypto/tricrypto
pools usan fee dinámica función del desvío de precio interno post-trade y de parámetros
suavizados — la fórmula exacta varía por implementación y versión, así que aquí queda a nivel
conceptual (RULE 00: no derivar sizing de un crypto pool sin leer su contrato o simularlo).
Regla práctica: para Curve, el sizing numérico se hace sobre `get_dy`/simulación, jamás sobre
una closed-form tipo CPMM — el error de tratar stableswap como x·y=k es del orden del 100% del
spread en montos medianos.

## 11.4 BALANCER

### 11.4.1 Weighted pools

Invariante con pesos w_i (Σw_i = 1) sobre balances B_i: `Π (B_i/w_i)^{w_i} = k`.

Fórmulas de swap entre token i→o con γ = 1−swapFee (fee sobre el INPUT), del whitepaper de
Balancer y de la matemática del Vault:

```
outGivenIn:  ΔB_o = B_o · (1 − (B_i/(B_i + γ·ΔB_i))^(w_i/w_o))
inGivenOut:  ΔB_i = B_i · ((B_o/(B_o − ΔB_o))^(w_o/w_i) · 1/γ − 1)
spot (sin fee): S = (w_i·B_o)/(w_o·B_i)
```

El spot NO es B_o/B_i: ignorar los pesos produce oportunidades falsas en pools 80/20 o 60/40.
Exponentes fraccionales w_i/w_o ⇒ sin aritmética entera trivial: usar la simulación o las
fórmulas con log/pow en precisión extendida, y validar siempre contra el Vault.

### 11.4.2 Single Vault y boosted pools

Arquitectura: UN solo contrato Vault custodia todos los fondos (V2 mainnet:
`0xBA12222222228d8Ba445958a75a0704d566BF2C8`); los pools son lógica + parámetros. APIs reales:

- `IVault.getPoolTokens(bytes32 poolId) → (IERC20[] tokens, uint256[] balances, uint256
  lastChangeBlock)` — balances ACTUALES del Vault, no cacheadas por el pool.
- `IVault.queryBatchSwap(SwapKind kind, BatchSwapStep[] swaps, IAsset[] assets,
  FundManagement funds)` — revierte con los deltas decodificables: patrón canónico para
  validar un quote multi-pool sin ejecutar.
- `IVault.flashLoan(...)` con fee 0 para la mayoría de tokens (verificar con
  `getFlashLoanInfo(IERC20[] tokens) → (uint256 feePercentage, bool feeEnabled)`), callback
  `receiveFlashLoan` (núcleo §2.2).

Boosted pools (bb-a-USD y sucesores): el pool tradea contra wrappers de yield y BPT "phantom";
los balances visibles están en unidades wrapped. Para comparar contra venues en underlying hay
que linearizar: los LinearPool exponen `getRate()` (wrapped→underlying), y los wrappers ERC-4626
exponen `previewRedeem`/`previewDeposit`. Error clásico: mezclar unidades wrapped/underlying
dentro del mismo hop del cálculo — el "spread" aparente es pura no-conversión.

## 11.5 ORÁCULOS

### 11.5.1 Spot vs TWAP

Spot (`slot0.sqrtPriceX96` en V3, `getReserves()` en V2): manipulable con un solo swap de bajo
coste en pools poco profundos — válido SOLO para verificar ejecutabilidad, jamás para pricing
de decisiones o colaterales. TWAP V3: `observe(uint32[] secondsAgos)` devuelve por timestamp
`tickCumulative` y `secondsPerLiquidityCumulativeX128`; el tick promedio de la ventana es
`ΔtickCumulative / Δseconds` y el precio `1.0001^tick · 10^(dec0 − dec1)`. OJO: es promedio del
log del precio = promedio GEOMÉTRICO de precios, no aritmético — la mayoría de los "precios
medios" intuitivos están mal construidos sobre esta confusión. La periferia V3 ya empaqueta el
cálculo: `OracleLibrary.consult(pool, secondsAgo)` devuelve el tick promedio y
`OracleLibrary.getQuoteAtTick(tick, baseAmount, baseToken, quoteToken)` lo convierte a quote.
En V2 el equivalente son `price0CumulativeLast`/`price1CumulativeLast` (UQ112x112, §11.7)
muestreados en dos bloques.

Costo de manipular un TWAP: sostener el spot desviado durante una fracción de la ventana T
cuesta ∝ liquidez × tiempo (whitepaper V3, sección Oracles): para mover el TWAP de ventana T en
factor m sosteniendo t < T segundos, el spot debe irse a ~m^(T/t) — costo exponencial en T/t,
más el fee de ida y vuelta del propio pool. Ventanas de 30 min en pools profundos son
económicamente intratables; ventanas de 1 bloque o pools sin liquidez no sirven.

### 11.5.2 Chainlink

`AggregatorV3Interface`: `latestRoundData() → (uint80 roundId, int256 answer, uint256
startedAt, uint256 updatedAt, uint80 answeredInRound)` y `decimals()`. Checks obligatorios:

- `decimals()` del feed (8 en la mayoría de price feeds, algunos 18): normalizar con
  `10^(tokenDecimals − feedDecimals)` ANTES de cualquier comparación.
- Staleness: `updatedAt` dentro del heartbeat del feed (varía por par; los aggregator docs
  publican heartbeat y deviation threshold por feed). Feed stale → descartar, no promediar.
- Sanity: `answer > 0` (un feed de precio jamás debe responder ≤ 0).

### 11.5.3 Consenso multi-oráculo con mediana

Con ≥3 fuentes independientes (Chainlink, TWAP V3 de venue distinta, mediana de quotes
cross-venue en reposo): tomar la MEDIANA, no el promedio — el promedio se contamina con un solo
outlier; la mediana tolera floor((n−1)/2) fuentes corruptas. Si |fuente − mediana|/mediana >
threshold → esa fuente queda fuera del consenso ese ciclo; si el consenso pierde quorum →
fail-honest (observación con razón exacta, R8), nunca degradar a spot de un pool movible.

## 11.6 TAMAÑO ÓPTIMO DE TRADE

### 11.6.1 Closed-form: arbitraje entre 2 pools CPMM

Setup: comprar Y en pool A (reservas x_A, y_A, fee γ_A) vendiendo X; vender ese Y en pool B
(reservas y_B de Y primero, x_B de X, fee γ_B). Con k_A = x_A·y_A, k_B = x_B·y_B:

```
Δy(Δx) = γ_A·Δx·y_A / (x_A + γ_A·Δx)
Δx_out(Δy) = γ_B·Δy·x_B / (y_B + γ_B·Δy)
π(Δx) = Δx_out(Δy(Δx)) − Δx
```

FOC π'(Δx)=0 (diferenciando la cadena) colapsa a la igualación de precios marginales netos de
fee: `(x_A + γ_A·Δx)·(y_B + γ_B·Δy) = M` con `M = √(γ_A·γ_B·k_A·k_B)`. Usando
`x_A + γ_A·Δx = k_A/(y_A − Δy)` (curva de A invertida) y sustituyendo, se despeja en cerrado:

```
Δy* = (M·y_A − k_A·y_B) / (γ_B·k_A + M)
Δx* = x_A·Δy* / (γ_A·(y_A − Δy*))
```

Condiciones de sentido: Δy* > 0 sii y_A/x_A > y_B/x_B (precio spot de X mayor en A). El check
de invertibilidad con enteros: trabajar en U256 con la división al final (§11.7). Esta fórmula
asume reserves frescas de AMBOS pools en el mismo estado — la condición §11.1.3 aplica.

### 11.6.2 Concavidad y máximo único

Cada curva CPMM tiene output marginal estrictamente decreciente; composición de cóncavas
crecientes = cóncava; π(0)=0, π'(0) = (precio relativo − 1) > 0 si hay spread, y π(Δx) → −∞
por el término −Δx. Por lo tanto π es estrictamente cóncava con MÁXIMO ÚNICO interior. Restar
términos lineales — flash loan fee (lineal en Δx), gas convertido al token — preserva la
concavidad: π_net(x) = π(x) − c·x − G sigue uni-módica. Consecuencia práctica: cualquier
método de búsqueda unidimensional converge; no hay óptimos locales espurios.

### 11.6.3 Multi-hop: métodos iterativos robustos

Con V3 (escalonada), Curve o Balancer en la ruta no hay closed-form. Patrón estándar:

- Bracket: `[0, upper]` con `upper = min(capital disponible, principal flash, cota por
  reserves de entrada de cada hop)`. La cota por hop: para CPMM cualquier Δx es "ejecutable"
  (Δy < y); para V3 la cota relevante es la liquidez por rango hasta el límite del precio.
- Ternary search sobre enteros U256 (cada iteración reduce el bracket a 2/3; ~500 iteraciones
  cubren el dominio 256-bit worst-case, en la práctica decenas con bracket decente) evaluando
  π_net con las fórmulas exactas por hop. Alternativa: Newton sobre π' con derivada numérica —
  más rápido pero sensible al ruido de redondeo entero: preferir ternary/golden-section en
  producción salvo perfilado previo.
- Validación: el óptimo del solver es una HIPÓTESIS; el veredicto es la re-simulación (revm)
  con ese amount exacto (`arbx-simulation-mandatory`). El SizeOptimizer del repo es el punto
  de integración natural de este patrón.

### 11.6.4 Gas dentro del sizing

`π_net(x) = π(x) − gas_units(x)·gas_price_eth·price_eth_in_token` donde gas_units puede
depender levemente de x (más ticks cruzados en V3): medir con simulación en 2–3 puntos y
linealizar. Peligro de circularidad: convertir ETH→token con el spot del MISMO pool que la ruta
manipula infla π; usar el precio post-trade o el del consenso de oráculos (§11.5.3), siempre el
peor caso de los dos. Si π_net(x*) ≤ 0 → no-trade honesto (observación `gas_floor_breach`,
no re-etiquetar — memoria R-0001). Flash fee lineal: Aave v3 = 0.05% del principal; Balancer
Vault = 0 para la mayoría de tokens — el fee del préstamo entra como c·x en la concavidad.

## 11.7 ARITMÉTICA ENTERA SEGURA

- **UQ112x112 (V2)**: reserves uint112; el precio acumulado del par es UQ112x112 (`encode:
  uint224 << 112`, división `uqdiv`). El overflow del producto de reserves no puede ocurrir por
  ancho de bits (112+112=224) y el par fuerza `balance <= uint112(-1)` en `_update` — al
  portar, replicar el check, no asumirlo.
- **FullMath (V3)**: toda operación con sqrtPriceX96 en 256 bits pasa por `mulDiv` con
  intermedio 512-bit (a·b puede ser 2^192·2^96 = 2^288 > 2^256). En Rust: `alloy-primitives`
  U256 con `U512` intermedio o `mul_mod`+corrección; el crate `uniswap-v3-math` ya trae los
  puertos correctos — no reimplementar.
- **Cero flotantes en el hot path**: f64 tiene 53 bits de mantisa; un pool con 300k WETH son
  3·10^23 wei > 2^53 ≈ 9·10^15 → errores de decenas-centenas de wei por operación, compuestos
  por hop, que rompen gates de profit en el margen. Todo el cálculo económico en U256/U512.
- **Decimales mixtos**: la curva opera en unidades RAW del pool. WETH=18, DAI=18, USDC=6,
  USDT=6, WBTC=8 — nunca hardcodear: leer `decimals()` del token (cachear por address). El
  factor `10^(d_in − d_out)` se aplica alrededor de la curva (convertir el input antes o el
  output después), nunca dentro. `amountOutMin` SIEMPRE en raw del token de salida de ese hop.
- **Direccionalidad floor/ceil** (extiende §11.1.2): floor para todo lo que recibes, ceil
  (`(num + den − 1)/den`) para todo lo que debes entregar; el ceil de U256 max() con den−1 es
  overflow — usar checked arithmetic (`checked_mul`/`checked_div` de alloy) en lugar de panic.
- **Slippage param**: el patrón `apply_slippage(amount_out, bps)` del núcleo §6.2 trunca hacia
  abajo (seguro). No "mejorarlo" con redondeo al entero más cercano.

## 11.8 DIAGRAMA: FLUJO DE UNA QUOTE MULTIHOP

```
amount_in (raw token0)
   │
   ├─ hop1: curva del pool (V2 exacta / V3 tick-sim / Curve get_dy / Balancer exp)
   │        fee sobre input ─ floor del output ─ decimales NO se tocan aquí
   ▼
out1 (raw token_mid) ── minOut1 estricto ──▶ input hop2
   │
   ├─ hop2: idem …
   ▼
out_final ── normalización 10^(d0−dn) ── π = out_final − amount_in − flash_fee − gas·price
   │
   ├─ π > threshold ──▶ sizing (§11.6) ──▶ re-sim revm ──▶ gates arbx-*
   └─ π ≤ 0 ──▶ observación fail-honest (razón exacta), NUNCA fabricar Opportunity
```

## 11.9 TABLA FINAL DE DECISIÓN

| Tipo de pool | Matemática aplicable | Errores comunes que destruyen P&L |
|---|---|---|
| V2 / CPMM y forks | `getAmountOut/In` exacta (§11.1); closed-form 2-pool (§11.6.1) | fee cobrado sobre el output (forks distintos); reserves stale intra-bloque; redondear output hacia arriba; comparar precios sin normalizar decimales |
| Uniswap V3 | tick-sim por steps (§11.2.3); `uniswap-v3-math` | usar `slot0` como precio ejecutable; olvidar tickSpacing al iterar ticks; multiplicar sqrtPriceX96 sin FullMath (overflow); asumir liquidez total disponible cuando `liquidity()==0` en el rango; fee como multiplicación única en vez de por-step |
| Curve stableswap | Newton para D (§11.3.2); sizing numérico sobre `get_dy`/simulación | tratarla como CPMM (error ~total del spread); asumir fee estática en crypto pools; aridad `get_dy` (int128 vs uint256); balances con precisión nativa distinta por moneda |
| Curve crypto/tricrypto | conceptual: fee dinámica, invariante distinta | cualquier closed-form propia sin leer el contrato o simular (RULE 00) |
| Balancer weighted/boosted | exponencial w_i/w_o (§11.4.1); validar con `queryBatchSwap` | spot calculado sin pesos; balances cacheados del pool en vez de `getPoolTokens`; mezclar wrapped/underlying en boosted (linearizar con `getRate()`/`previewRedeem`) |
| ERC-4626 / wrappers | conversión lineal por rate (`previewRedeem`) | sumar unidades wrapped + underlying en el mismo hop; ignorar fees del wrapper |
| Uniswap V4 / hooks | curva base = V3 (§11.2) solo si el hook no interviene; quote vía StateView/QuoterV4 o revm; hook desconocido = simulación obligatoria (§11.10) | tratar V4 como V3 sin más; ignorar hook fee/fee dinámica; olvidar settle/take; suponer native=WETH |

## 11.10 UNISWAP V4: SINGLETON, HOOKS Y FLASH ACCOUNTING

V4 no cambia la curva: `TickMath`/`SqrtPriceMath`/`SwapMath` del core son los mismos de V3 —
§11.2 aplica íntegro para geometría, ticks y swap-step loop. Cambia todo el alrededor: un solo
contrato para todas las pools, un callback obligatorio con lock transient, contabilidad por
deltas, y hooks que pueden invalidar la closed-form como veredicto.

### 11.10.1 Arquitectura: singleton y poolId como identidad

En V2/V3 cada pool es un contrato desplegado por una factory y su identidad ES su address. En
V4 hay UN `PoolManager`: el estado de TODAS las pools vive en mappings indexados por `PoolId`
(bytes32). La identidad de una pool es el struct `PoolKey` completo:

```
PoolKey { Currency currency0; Currency currency1; IHooks hooks; IPoolManager poolManager;
           uint24 fee; bytes32 parameters; }   // parameters empaqueta tickSpacing + flags
PoolId = keccak256(abi.encode(PoolKey))        // helper PoolIdLibrary.toId()
```

- `Currency` es un wrapper de address; se ordena numéricamente → `currency0 < currency1`, y el
  native ETH (`address(0)`) SIEMPRE ordena como currency0.
- Consecuencia para el agregador: la clave de venue deja de ser "address del contrato de
  pool" y pasa a ser `(poolId, hooks)`. Dos pools con el mismo par de currencies y distinto
  hook son DOS mercados distintos: misma curva base, fee y comportamiento diferentes. Indexar
  V4 por PoolKey completo con `hooks` como campo de primera clase (decide §11.10.4).
- Descubrimiento por eventos del singleton (firmas reales de `IPoolManager`):

```solidity
event Initialize(PoolId indexed id, Currency indexed currency0, Currency indexed currency1,
                 uint24 fee, int24 tickSpacing, IHooks hooks);
event ModifyLiquidity(PoolId indexed id, address indexed sender,
                      int256 liquidityDelta, bytes32 salt);
event Swap(PoolId indexed id, address indexed sender, int128 amount0, int128 amount1,
           uint160 sqrtPriceX96, uint128 liquidity, int24 tick, uint24 fee);
```

Con fee dinámica, el evento `Swap` es la única fuente post-hoc de la fee EJECUTADA (la del
PoolKey es un flag, no el valor).

### 11.10.2 unlock(): lock transient y flujo del ejecutor

Toda mutación de estado (initialize, modifyLiquidity, swap, donate, take, settle, sync, skim)
exige ejecutarse DENTRO de `PoolManager.unlock(bytes calldata data) returns (bytes memory)`:
unlock activa un lock guardado en transient storage (TSTORE/TLOAD, EIP-1153) y llama
`IUnlockCallback(msg.sender).unlockCallback(data)`; al retornar exige que TODOS los deltas
del caller estén en cero o revierte todo. Es un flash-swap universal: no hay aprobaciones
intermedias; la frontera de atomicidad es el callback (patrón de callback del ejecutor
on-chain: ver referencia 14 §14.2 — aquí solo matemática/quote/adaptador).

Swap contra el manager:

```solidity
struct SwapParams { int256 amountSpecified; uint160 sqrtPriceLimitX96; }
function swap(PoolKey memory key, SwapParams memory params, bytes calldata hookData)
    external returns (BalanceDelta swapDelta);
```

TRAMPA de signo (rompe rutas enteras al portar de V3): en V4 `amountSpecified < 0` es exactIn
y fija `zeroForOne = true` (currency0 → currency1); en V3 amountSpecified POSITIVO era
exactIn. Un wrapper que reusa el convenio de signos de V3 ejecuta el swap INVERSO o la
dirección opuesta — síntoma: quotes descabelladas y reverts por `sqrtPriceLimitX96`.

Lo que MUERE del flujo V2/V3 en el ejecutor: no `approve()` de tokens hacia routers por hop,
no transfer ERC-20 por hop. Dentro del callback: swap(s) → `settle` los débitos → `take` los
créditos → unlock verifica el cuadre. Multihop dentro del MISMO unlock NETEA deltas: si hop1
deja -X WETH y hop2 deja +X WETH, el neto por currency se transfiere UNA sola vez.

### 11.10.3 Flash accounting: deltas, settle/take, claims ERC-6909, native

Contabilidad delta-based por currency: `currencyDelta(address, Currency) → int256` con
convenio débito/crédito — POSITIVO = debes al manager, NEGATIVO = el manager te debe. El swap
devuelve `BalanceDelta` (int128 amount0/int128 amount1 packeados). Al salir del callback todo
delta debe ser cero:

- Débito → `settle`: pagar con transfer al manager (native con `msg.value`). Alternativa sin
  mover tokens: mint de claims ERC-6909 — el propio PoolManager es emisor ERC-6909 (el
  crédito queda tokenizado, rescatable luego con burn). Para el ejecutor del repo, settle/take
  con transfer es el default: claims introducen un segundo activo a reconciliar en el ledger
  (R8) y solo valen con una estrategia de netting explícita.
- Crédito → `take(Currency currency, address to, uint256 amount)`; con native el manager
  envía ETH directamente.
- Desbalance balance-vs-reserva (donations al manager): `sync(Currency)` + `skim(Currency, address)`.

Native ETH = `Currency.wrap(address(0))`: el manager acepta ETH nativo SIN envolver a WETH.
Suponer native=WETH (pagar WETH donde el manager espera ETH) es revert garantizado; el
adaptador debe decidir por currency si el débito se salda con transfer de token o con value.

```
unlock(data)                          // lock TSTORE
 └─ unlockCallback(data):
      delta = manager.swap(key, params, hookData)            // BalanceDelta +debito/-credito
      ...mas hops... netear por currency...
      debito neto currency_i  -> pagar (settle)              // transfer o msg.value
      credito neto currency_j -> manager.take(currency_j, this, amount)
 unlock exige deltas == 0 o revierte TODO (atomicidad del bundle)
```

### 11.10.4 Hooks: permisos en la dirección, fee dinámica, muerte de la closed-form

- Permisos: los 14 bits bajos de la DIRECCIÓN del hook codifican qué callbacks implementa (la
  address se construye con vanity para que sus bits igualen sus flags). El manager despacha
  cada call point SOLO si el bit está en 1. Call points de `IHooks`: beforeInitialize,
  afterInitialize, beforeAddLiquidity, afterAddLiquidity, beforeRemoveLiquidity,
  afterRemoveLiquidity, beforeSwap, afterSwap, beforeDonate, afterDonate — más 4 flags
  `*_RETURNS_DELTA` (before/afterSwap, afterAdd/RemoveLiquidity) con los que el hook devuelve
  deltas propios: cobra o inyecta currency DENTRO del swap.
- Fee: el `uint24 fee` lleva tiers estáticas como V3 (100/500/3000/10000) más flags en bits
  altos: `0x800000` = fee dinámica (la determina el hook en runtime vía el getter de fee-de-hook
  del core, `IHookFee`), `0x400000` = hook swap fee (porción de la fee va al hook),
  `0x200000` = fee sobre retiro de liquidez. La fee efectiva de un pool con flag dinámico NO
  se lee del PoolKey: es función de estado y caller.
- CONSECUENCIA MATEMÁTICA (razón de ser de esta sub-sección): un hook es código ARBITRARIO
  que corre antes/después del swap y puede (a) cambiar la fee efectiva por trade, (b) tomar o
  pagar currency vía RETURNS_DELTA, (c) revertir condicionalmente (discriminar por caller,
  tamaño u origen). La closed-form de §11.1–§11.6 MUERE como veredicto en pools con hook: la
  curva base sigue siendo V3, pero la transformación input→output efectiva la define el hook.
  Regla operativa:
  1. Quote cerrada local SOLO con fee estática y hook de semántica acotada — verificable sin
     confianza: un hook SIN bits de swap en su address no puede intervenir en un swap (los
     bits son parte de la identidad del pool; on-chain, no promesa).
  2. Cualquier otro caso: la curva V3 como FILTRO de sizing (§11.6) y el VEREDICTO es la
     simulación revm con el hook real ejecutándose (doctrina §11.1.3 y referencia 12 §12.7;
     `arbx-simulation-mandatory`). Hook desconocido = venue hostil, no quoted venue.
- Riesgo a detectar/mitigar (jamás a ejecutar — `arbx-mev-ethics-gate`): hooks con beforeSwap
  que alteran precio/fee según el caller habilitan discriminación y sandwich dirigido; una
  quote cacheada de pool con hook vale cero en el bloque objetivo (RULE 00: fail-honest con
  razón exacta, p.ej. `hook_unknown_unsimulated`).
- Forks "Algebra-style" (QuickSwap/Algebra, dynamic fee por volatilidad): mismo tratamiento —
  fee función de estado ⇒ closed-form muere igual aunque no haya hooks formales.

### 11.10.5 Quoting real: StateView y QuoterV4

- Lectura de estado (equivalente de `slot0()`/`liquidity()`/ticks del pool V3, pero contra el
  singleton y por PoolId): periférico `StateView` — `getSlot0(PoolId) → (uint160 sqrtPriceX96,
  int24 tick, uint24 protocolFee)`, `getLiquidity(PoolId) → uint128`, `getTickInfo`, bitmap
  de ticks por word, feeGrowth global. Para la tick-sim de §11.2.3 basta slot0 + liquidity +
  ticks vía StateView.
- Quote exacta sin ejecutar: `QuoterV4` (`quoteExactInputSingle(PoolKey, SwapParams)`),
  patrón heredado del QuoterV2 de V3: entra en `unlock`, ejecuta el swap REAL contra el
  manager y revierte deshaciendo el estado, decodificando el resultado del revert
  (revert-with-quote). OJO: (a) es para `eth_call` — el revert es parte del diseño, jamás en
  un bundle; (b) CON hooks, QuoterV4 EJECUTA los hooks reales en el estado actual: la quote
  incluye su efecto, pero sigue siendo estado presente, no el del bloque objetivo (re-simular
  igual, §11.1.3); (c) round-trip RPC por quote: pipeline de validación, no hot path.
- Direcciones canónicas mainnet (despliegue oficial de Uniswap Labs, enero 2025; verificadas
  contra la página de deployments de la doc oficial — docs.uniswap.foundation, hoy
  developers.uniswap.org — y corroboradas en Etherscan; mismo criterio RULE 00 que §11.4.2
  con el Vault de Balancer):

| Contrato | Address mainnet (chainId 1) |
|---|---|
| PoolManager | `0x000000000004444c5dc75cB358380D2e3dE08A90` |
| StateView | `0x7ffe42c4a5deea5b0fec41c94c136cf115597227` |
| PositionManager | `0xbd216513d74c8cf14cf4747e6aaa6420ff64ee9e` |
| QuoterV4 (Quoter) | `0x52f0e24d1c21c8a0cb1e5a5dd6198556bd9e1203` |

  En otras chains NO asumir los mismos addresses (aviso explícito de la doc): leer el feed
  `deployments.json` de la doc oficial por chain antes de configurar (RULE 00 /
  `arbx-no-hardcode-doctrine`: config declarativa, no literales en código).

### 11.10.6 Gas: medir, no asumir

- Singleton + lock TSTORE + netting de §11.10.3: el swap sin hook ahorra ~10-20% de gas vs
  V3 per-pool (whitepaper v4), y el multihop dentro de un unlock ahorra más (una transfer por
  currency neta en vez de una por hop). Eso es el CASO SIN HOOK.
- Cada hook activo añade un call frío al contrato del hook + TODO su código: un beforeSwap
  pesado se come el ahorro del singleton y más allá. El ~10-20% es techo, no promedio.
- Integrarlo al sizing como §11.6.4: medir con revm en 2-3 puntos del dominio del amount
  (cada tamaño cruza distinta cantidad de ticks) y linealizar; jamás una constante de gas
  heredada de V3.

## GOBERNANZA

Este conocimiento está subordinado a los gates `arbx-*` (`arbx-paper-trade-first`,
`arbx-simulation-mandatory`, `arbx-risk-limits-enforcement`, `arbx-pre-execute-checklist`) y a
CLAUDE.md §34: LIVE_MAINNET es gated con default-deny en el terminus (`relays-client`). Ninguna
fórmula de esta referencia autoriza flip a live, broadcast con capital real, ni ejecutar
vectores ofensivos contra terceros (ver `arbx-mev-ethics-gate`). En shadow/paper la matemática
es idéntica (§34.1 hot-path mode-invariant); el veredicto final siempre es la simulación.
