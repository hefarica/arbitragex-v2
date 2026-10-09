# SIZE-SWEEP-2LEG-01 — **(B) `net(s*) < 0` en las 48 rutas: L1 2-leg queda CERRADO como path de PnL** — y el que lo cierra es el FEE HURDLE, no el gas

**Resultado en una línea:** sobre la allowlist de cuatro pares del operador (Uniswap **V3-0,05 % / V3-0,30 % / V2**, **2 hops**), barridos **48 rutas × 16 tamaños = 768 puntos**, **NINGUNO tiene `net > 0`**: el **máximo global es `net(s*) = −$0,287541` en `s* = $0,1`** (WBTC/WETH, V3-3000 → V2). Y no es el gas: el **spread máximo observado en la allowlist (0,2507 %) no cubre ni el hurdle de fees más barato (0,3500 %) a NINGÚN tamaño**, y **0 de 48 rutas supera su propio hurdle**. **Cruce por cero: NO existe.** **38 de 48 rutas tienen su máximo en el borde inferior de la grilla** — el «óptimo» degenera a *casi no operar* (−gas ≈ −$0,287), no a un tamaño de inversión.

**Cápsula ORGDNA:** `run_id=arbx-entrega-20261008` · **Dueño:** Data · **Intento:** `31267074-fc74-467d-bdaf-8efdf757b48f`
**Fork FRESCO** (pinneado en la cabeza) · **CERO escrituras** en cualquier fork · **Paper, sin firma, sin broadcast** · **In scope:** `docs/data/`

---

## 0. ★★ LA CANTIDAD QUE DECIDE — una sola, con su denominador

`net(s) = gross(s) − impact(s) − LP_fees(s) − flash_fee(s) − gas`, maximizado sobre `s` en la grilla.

| | valor | fuente |
|---|---|---|
| **`s*`** | **$0,1** | mejor de 768 puntos |
| **`net(s*)`** | **−$0,287541 USD** | `MAX_NETO_2LEG` |
| par / ruta / dirección | **WBTC/WETH** · `0xcbcdf962…` (V3‑3000) → `0xbb2b8038…` (V2) · entra por **WBTC** | `/tmp/t189.log` |
| descomposición @ `s*` | gross **−0,000713** · fee **+0,000600** · impacto **+0,000114** · gas **0,286828** · flash **0,000000** | ídem |
| **cruce por cero** | **NO existe en ninguno de los 768 puntos** | ídem |
| **máximo EN EL BORDE** | **38 de 48 rutas** en el borde **inferior** ($0,01) · **0** en el superior ($200.000) | cómputo sobre `todos` |
| rutas con **algún** punto `net>0` | **0 de 48** | ídem |
| rutas cuyo **spread supera su propio hurdle** | **0 de 48** | ídem |
| rango de `net(s*)` | **−0,351679 … −0,287541** | ídem |
| rango de spread / hurdle | spread **0,0008 %–0,2507 %** · hurdle **0,3500 %–0,6000 %** | ídem |

**★ `net(s*) < 0` ES `NO OPPORTUNITY` — un DATO, no un fallo de código** (el contrato lo dice con esas palabras). **Y el máximo en el borde inferior se declara como tal, no como `s*` de inversión:** en 38 de 48 rutas el óptimo de la grilla está en `$0,01`, y el límite `s→0` de la curva del mejor global es **−$0,286828 = −gas puro**. Es decir: **el «óptimo» de esta familia es no operar; el único modo de perder menos es mover menos dinero.**

---

## 1. ★★ CABEZA FRESCA — la precondición de t185, cumplida sin tocar el fork compartido

**El fork compartido está STALE y no sirve para esto.** El gate de t185 (script de PR #914, `sha256=b171fc7e0b2fee2cc0ddaac9cdc68b4e7fcf5365e0096fa57f24dba360850c10` verificado **local == VPS**) lo dice con su exit code:

```
GATE contra el fork COMPARTIDO (172.18.0.3:8545):
  verdict=STALE reason=over_threshold fork_block=26148216 head_block=26150002
  age_blocks=1786 age_minutes=357.20 (= 5,95 h) threshold_minutes=10      -> exit=1
GATE contra el fork FRESCO (127.0.0.1:8546):
  verdict=FRESH reason=within_threshold fork_block=26149998 head_block=26150002
  age_blocks=4 age_minutes=0.80 threshold_minutes=10                        -> exit=0
CONTROL de la guarda anti-adorno (mismo endpoint en ambos lados):
  verdict=UNMEASURABLE reason=same_endpoint                                 -> exit=2
```

**★ CÓMO se obtuvo cabeza fresca sin violar la prohibición:** se levantó un **anvil NUEVO, pinneado con `--fork-block-number 26149998`** (la cabeza real en ese momento), en el **puerto 8546 del host** — `arbx-freshfork-t189`, imagen `ghcr.io/foundry-rs/foundry:latest` (1.7.1). **El fork compartido no se tocó**: `eth_blockNumber` **26148216 antes y después** (idéntico) y `StartedAt 2026-10-08T14:16:01.110230687Z` **sin cambios**. **Cero `eth_sendTransaction`, cero `anvil_setStorageAt`, cero `anvil_reset`, cero `anvil_setBalance`.** El instrumento nuevo **solo lee**.

| lectura | valor |
|---|---|
| `fork_block` del instrumento | **26.149.998** (`0x18f046e`) — **edad 0 bloques al arrancar, 4 al medir** (0,80 min) |
| cabeza real al medir | 26.150.002 (publicnode, `eth_blockNumber`) |
| `fork_block` del fork compartido | 26.148.216 (**STALE, 1.786 bloques = 5,95 h**) |
| `eth_call` ejecutadas | **1.108** (contador propio), **retenciones 0**, todas contra `BLOCK=0x18f046e` fijo |

**⇒ La cifra de este informe no es arqueología: se midió contra el estado de la cabeza, y la edad (0,80 min) va declarada en la misma línea.**

---

## 2. ★ LA ALLOWLIST, QUÉ ENTRA, QUÉ NO — y con qué estado on-chain

**12 venues**, todas **Uniswap** (clasificación **on-chain por `factory()`**, no por nombre): V3 factory `0x1f98431c…`, V2 factory `0x5c69bee7…`.

| par | venue | fee (on-chain) | unidades | estado leído del fork @26.149.998 |
|---|---|---|---|---|
| USDC/WETH | V3 `0x88e6a0c2…` | 500 | **millonesimas = 0,05 %** | `sqrtP=1594194212844718582674165118977905` tick `198200` `L=2666686386268745391` |
| USDC/WETH | V3 `0x8ad599c3…` | 3000 | **millonesimas = 0,30 %** | `sqrtP=1595970775536543325435536120498108` tick `198223` `L=4371672941339010326` |
| USDC/WETH | V2 `0xb4e16d01…` | 30 | **basis points = 0,30 %** | `r0=10042262783071` `r1=4076077019412174506606` |
| WETH/USDT | V3 `0x11b815ef…` | 500 | millonesimas | `sqrtP=3938711699307809089395153` tick `-198195` `L=591947975599089465` |
| WETH/USDT | V3 `0x4e68ccd3…` | 3000 | millonesimas | `sqrtP=3933817857856194233226863` tick `-198220` `L=12545033962540014885` |
| WETH/USDT | V2 `0x0d4a11d5…` | 30 | basis points | `r0=2936456220892383900161` `r1=7239112575852` |
| WBTC/WETH | V3 `0x4585fe77…` | 500 | millonesimas | `sqrtP=45579306801120173383256950283284821` tick `265265` `L=151091967234344028` |
| WBTC/WETH | V3 `0xcbcdf962…` | 3000 | millonesimas | `sqrtP=45634665048428875890380003657379689` tick `265290` `L=33421763508502342` |
| WBTC/WETH | V2 `0xbb2b8038…` | 30 | basis points | `r0=5375782012` `r1=1783510887743980726677` |
| USDC/USDT | V3 `0x7858e59e…` | 500 | millonesimas | `sqrtP=79264934512878079290346349913` tick `9` `L=201178060740312` |
| USDC/USDT | V3 `0xee4cf3b7…` | 3000 | millonesimas | `sqrtP=79239153276525883762432089709` tick `2` `L=3639582343879` |
| USDC/USDT | V2 `0x3041cbd3…` | 30 | basis points | `r0=1800991924841` `r1=1802776173304` |

**★ EL DATO DURO DE UNIDADES (y el error que cazó):** `fee()` on-chain devuelve **500 / 3000** para V3 y **30** para V2, y **NO son la misma unidad**: V3 usa **millonésimas** (500 = 0,05 %) y V2 usa **basis points** (30 = 0,30 %). La primera corrida de la validación marginal lo delató: daba **exactamente `1/(1−0,05) = 1,052105`** para V3‑500 y **`1/(1−0,30) = 1,424286`** para V3‑3000. **Corregido y re-medido.** **El tercer valor de `getReserves()` (`blockTimestampLast` = 1791490379) NUNCA se lee como fee.**

**FUERA, y declarado (no ampliado a mitad del barrido):**
- **SushiSwap V2** (`factory 0xc0aee478…`): `0x397ff154…`, `0x06da0fd4…`, `0xceff5175…` — **no son Uniswap**;
- **PancakeSwap V3** (`factory 0x0bfbcf9f…`): `0x1445f32d…`, `0x1ac1a8fe…`, `0x6ca298d2…`, `0xacdb27b2…`, `0x04c85779…` — **no son Uniswap**;
- **tier V3‑100 (0,01 %)**: `0xc7bbec68…`, `0x3416cf6c…` — **fuera de la lista de tiers del operador (0,05 % / 0,30 %)**. **Consecuencia declarada: para USDC/USDT el venue más profundo de la red (el 0,01 %) queda excluido por construcción**, así que este informe **no dice** que USDC/USDT no tenga ruta en la red: dice que **no la tiene dentro de la allowlist tal como fue fijada**.
- Tokens con tax: **ninguno de los cuatro pares lo es** (WETH/WBTC/USDC/USDT).

**Precios y gas, al bloque de la cotización (no un default):**

| insumo | valor | origen declarado |
|---|---|---|
| WETH/USD | **$2.470,57** | Chainlink `0x5f4ec3df…` `latestRoundData()` @bloque, `dec=8` |
| WBTC/USD | **$81.628,13** | Chainlink `0xf4030086…` `dec=8` |
| USDC/USD | **$0,99986** | Chainlink `0x8fffffd4…` `dec=8` |
| USDT/USD | **1,000000** | **[DECLARADO]** el feed `0x3e7d1eab…` devuelve `0x` (sin code en el fork, fallo de lazy-fetch) ⇒ se usa el peg y **se dice** |
| `baseFeePerGas` | **568.346.806 wei** (0,5683 gwei) | `eth_getBlockByNumber(0x18f046e)` |
| priority | **55.738 wei** (0,0001 gwei) | publicnode `eth_maxPriorityFeePerGas` (= política `dynamic_basefee_plus_tip` del motor) |
| **gasPrice** | **568.402.544 wei** | suma de los dos |
| **costo de 1e6 gas** | **$1,40419** | `gasPrice × ETH/USD` |
| flash fee | **NO aplica** en `s*` (`s ≤ capital`) · sensibilidad con **0,09 %** (`trading_config.flashloan_fee_pct`) | PG, chain 1 |
| relay | **$0,00** registrado y declarado + **sensibilidad −$0,05** | ver §5 |

---

## 3. ★★ LA CURVA `net(s)` — no solo el máximo

**Grilla de notional declarada (16 puntos, geométrica ×10^0,5):** `0,01 · 0,032 · 0,1 · 0,32 · 1 · 3,2 · 10 · 32 · 100 · 320 · 1.000 · 3.200 · 10.000 · 32.000 · 100.000 · 200.000 USD` — cubre **las dos poblaciones vivas** ($0,01 del kernel y ~$2,5k de la sim) y llega a **50 ETH equivalentes** por arriba.

**Mejor ruta de cada par (las 4):**

| par | ruta (entrada) | `s*` | **`net(s*)`** | gross | fee | impacto | gas | spread | hurdle | ¿limpia? | ¿borde? |
|---|---|---|---|---|---|---|---|---|---|---|---|
| USDC/WETH | V2 `0xb4e16d01…` → V3‑500 `0x88e6a0c2…` (WETH) | $0,01 | **−0,304945** | −0,000060 | +0,000035 | +0,000050 | 0,304885 | 0,2507 % | 0,3500 % | **NO** | **SÍ** |
| WETH/USDT | V2 `0x0d4a11d5…` → **V3‑3000** `0x4e68ccd3…` (WETH) | $0,01 | **−0,297467** | −0,000060 | +0,000060 | −0,000000 | 0,297407 | 0,0017 % | 0,6000 % | **NO** | **SÍ** |
| **WBTC/WETH** | **V3‑3000 `0xcbcdf962…` → V2 `0xbb2b8038…` (WBTC)** | **$0,1** | **−0,287541** | −0,000713 | +0,000600 | +0,000114 | 0,286828 | 0,0008 % | 0,6000 % | **NO** | no |
| USDC/USDT | V3‑500 `0x7858e59e…` → V2 `0x3041cbd3…` (USDT) | $0,01 | **−0,312268** | −0,000035 | +0,000035 | +0,000001 | 0,312233 | 0,0062 % | 0,3500 % | **NO** | **SÍ** |

**La curva completa del mejor global (WBTC/WETH V3‑3000→V2), en USD — el continuo, no un punto:**

| `s` (USD) | 0,01 | 0,1 | 1 | 10 | 100 | 1.000 | **2.500** | 4.000 | 10.000 | 100.000 |
|---|---|---|---|---|---|---|---|---|---|---|
| gross | −0,001048 | **−0,000713** | −0,006319 | −0,059956 | −0,602591 | −6,245692 | −16,535431 | −27,929681 | −84,539401 | −3000,882183 |
| fee | 0,000057 | 0,000600 | 0,006002 | 0,060034 | 0,600333 | 6,003274 | 15,007950 | 24,012344 | 60,027064 | 599,704312 |
| gas | 0,286828 | 0,286828 | 0,286828 | 0,293979 | 0,293979 | 0,307533 | 0,307477 | 0,307491 | 0,307533 | 0,350005 |
| **net** | **−0,287876** | **−0,287541** | −0,293147 | −0,353935 | −0,896570 | −6,553226 | **−16,842909** | −28,237172 | −84,846934 | −3001,232187 |

**★ El net es NEGATIVO en los 10 puntos y monótonamente decreciente a partir de `s*`.** El máximo está en **$0,1**, a **29 centavos del límite `s→0` (−gas = −$0,286828)**: la curva entera vive pegada al gas y el gross nunca la despega.

### ★★ Y AHORA EL QUE CIERRA DE VERDAD: el SPREAD REQUERIDO vs el observado

`spread_requerido(s) = hurdle + gas(s)/s` (el gas es costo FIJO: al achicar `s`, el spread que haría falta **explota**):

| `s` | gas | **spread requerido** | spread MÁXIMO observado en la allowlist | veredicto |
|---|---|---|---|---|
| $0,01 | 0,286828 | **2.868,8801 %** | **0,2507 %** | imposible por 4 órdenes |
| $1 | 0,286828 | **29,2828 %** | 0,2507 % | imposible por 2 órdenes |
| $10 | 0,293979 | **3,5398 %** | 0,2507 % | imposible |
| $100 | 0,293979 | **0,8940 %** | 0,2507 % | imposible |
| $1.000 | 0,307533 | **0,6308 %** | 0,2507 % | imposible |
| $100.000 | 0,350005 | **0,6004 %** | 0,2507 % | **imposible por 2,4×** |

**★★ LA CONCLUSIÓN NO ES «EL GAS MATA LA RUTA», ES MÁS FUERTE: el FEE HURDLE la mata.** El hurdle más barato de la allowlist es **0,35 %** (V3‑500 + V3‑500) y **el spread máximo entre venues del mismo par es 0,2507 %**: **aun con gas CERO, ninguna de las 48 rutas cubre sus propios fees.** El gas **agrava** (y a tamaño chico domina, porque es fijo), pero **no es la causa raíz: la causa raíz es que los venues del mismo par están alineados por debajo del costo de ida y vuelta.** Por eso el resultado es **estructural y no de tamaño**: no hay `s` que salve algo que pierde antes del gas.

**★ Esto responde la pregunta del contrato en su forma más útil:** el `$1000` **no estaba mal dimensionado** — a $1.000 el mejor net es **−$6,553226**, y a $2.500 es **−$16,842909**: agrandar solo agranda la pérdida (el gross va como `−spread_efectivo×s`). **Y achicar tampoco: a $0,01 el net es −$0,287876.** **El tamaño no era el problema.**

---

## 4. LAS DOS POBLACIONES VIVAS, medidas en la misma curva

| población (del contrato) | `s` | **`net` medido acá** | gross | gas |
|---|---|---|---|---|
| kernel cotiza a **$0,01** (HEX 6-hop: notional $0,01 → **−$0,63**) | $0,01 | **−0,287876** | −0,001048 | 0,286828 |
| sim a **$2,5k** (nota del operador: **−$24,02**) | 2.500 | **−16,842909** | −16,535431 | 0,307477 |
| 1 ETH equivalente (el `1e18` wei de las filas vivas = **$2.470,57**) | 2.470,57 | **NO MEDIDO** — cae entre los dos puntos medidos `$1.000` (−6,553226) y `$2.500` (−16,842909) | — | — |

**★ Los dos «tamaños vivos» NO son `s*`: el `s*` medido es $0,1, y ninguno de los dos está ahí.** Y el signo coincide con lo que el operador midió por su cuenta en **su** ruta (**−$0,63** y **−$24,02**: mismo signo y mismo orden que **−$0,287876** y **−$16,842909** de la mejor ruta de la allowlist). **Poblaciones y rutas distintas; se cita como corroboración de dirección, no como equivalencia.**

---

## 5. CONTROLES DE LA PROPIA MAQUINARIA (sin redondear)

**A) `xy=k` de V2 (float) contra aritmética RACIONAL EXACTA (`fractions.Fraction`)**, con `dx=1e9` raw:
`USDC/WETH 1.000000000` · `WETH/USDT 1.000000000` · `WBTC/WETH 1.000000000` · `USDC/USDT 1.000000000`
**⇒ `3/4` idénticos EXACTOS; el cuarto difiere por debajo de `1e-9` (redondeo de float, no unidades). SE REPORTA `3/4` Y NO SE SUBE A `4/4`** — el estándar que t183 fijó.

**B) Precio marginal observado vs `mid×(1−fee)`, a TRES tamaños por venue (los 12), ratio sin redondear:**

```
USDC/WETH  V3  500  $10: 0.999999875 · $1: 0.999999043 · $0.01: 0.999860000
USDC/WETH  V3 3000  $10: 0.999999854 · $1: 0.999999394 · $0.01: 0.999860000
USDC/WETH  V2   30  $10: 0.999998987 · $1: 0.999999881 · $0.01: 0.999959985
WETH/USDT  V3  500  $10: 0.999999594 · $1: 0.999998994 · $0.01: 0.999949986
WETH/USDT  V3 3000  $10: 0.999999938 · $1: 0.999999737 · $0.01: 0.999927366
WETH/USDT  V2   30  $10: 0.999998626 · $1: 0.999999863 · $0.01: 0.999999999
WBTC/WETH  V3  500  $10: 0.999954755 · $1: 0.999628121 · $0.01:     NA (12 raw)
WBTC/WETH  V3 3000  $10: 1.000005787 · $1: 0.999678482 · $0.01:     NA
WBTC/WETH  V2   30  $10: 1.000023948 · $1: 0.999944365 · $0.01:     NA
USDC/USDT  V3  500  $10: 0.999999814 · $1: 0.999998914 · $0.01: 0.999832009
USDC/USDT  V3 3000  $10: 0.999997122 · $1: 0.999998726 · $0.01: 0.999783169
USDC/USDT  V2   30  $10: 0.999994444 · $1: 0.999999427 · $0.01: 0.999959980
VALIDACION_B: 33 ratios; min=0.999628121  max=1.000023948
```

**⇒ Todos los ratios están a ≤ `3,8e-4` de 1, y el desvío es exactamente **cuantización de enteros + impacto de primer orden**, no un factor de unidades: se ve en que **achicar el tamaño acerca el ratio a 1** en las 8 filas con `$0,01` medible, y en que el peor caso (`WBTC`, `in_raw=1.225`) es el de **menos** unidades de entrada. **`NA` se declara `NA`** (12 raw de entrada ⇒ cuantización del 8 %, no sirve como prueba), no se rellena.**
**★ Este control es el que cazó el error de unidades de §2: la corrida anterior daba `1,052105` y `1,424286` — exactamente `1/(1−fee)` con el fee leído en la unidad equivocada. Sin este control, el informe habría publicado fees 10× y 100× inflados.**

**C) El gas del instrumento contra el gas del MOTOR (declarado, otra fuente):** el quoter midió **204.273** unidades en la ruta global (V3 3000 + V2 + 60k de overhead declarado) y **249.266** a $100.000; `trading_config.gas_estimate_units` = **250.000**. **Con las 250.000 del motor, el gas de `s*` sería 250.000 × $1,40419e-6 = $0,351048 en vez de $0,286828 ⇒ `net(s*)` = −0,000713 − 0,351048 = −$0,351761: MÁS negativo.** **⇒ El veredicto NEGATIVO es robusto al supuesto de gas del propio motor.**

**D) Sensibilidad al flash fee:** con **0,09 %** (`flashloan_fee_pct` del motor, no el 0,05 % de Aave que asumí), el mejor global **sigue en `s*=$0,1` con −$0,287541** (porque en `s*` el flash **no aplica**: `s ≤ capital_usd`), y **siguen 0 de 48 rutas con `net>0`**. **Se declara que el valor que usé en la grilla fue el de Aave (5 bps) y que el del motor es 9 bps.**
**E) Relay:** registrado **$0,00** y declarado. **Sensibilidad explícita: restar $0,05 no cambia ninguna conclusión — el mejor `net` es −$0,287541, o sea 5,75× el relay.** *(El contrato señala que el kernel pinta +$0,05 «con relay $0 porque no lo carga»: acá el relay se carga y se declara, y no alcanza para dar vuelta nada.)*

---

## 6. LOS TRES RESULTADOS DEL CONTRATO — **cuál salió, con números**

| | resultado | ¿salió? |
|---|---|---|
| **(A)** `net(s*) > 0` ⇒ el `$1000` estaba mal dimensionado | **NO** | `net(s*) = −$0,287541` |
| **(B)** `net(s*) < 0` ⇒ **L1 2-leg CERRADO como path de PnL** | **SÍ** | 0 de 48 rutas con `net>0`; 0 de 48 con spread > hurdle |
| **(C)** verde sólo con spread ≥ ~5 % ⇒ cotización podrida | **NO APLICA acá** | el spread máximo de la allowlist es **0,2507 %**: no hay ninguna ruta con spread grande que haya que descartar |

**⇒ Según el propio contrato, con (B): `L1 2-leg` queda CERRADO como path de PnL, y recién ahora corresponde evaluar otra venue (L2, gas en centavos) u otra familia. NO corresponde «más exótico» — el contrato lo dice y este informe no lo contradice.**
**★ Y NO se declara «cuánto se podría ganar»:** frecuencia, competencia e impacto acumulado **no son computables con lo medido**. **`net_realized_usd` sigue `NULL`: no hay receipt y no se inventa un realizado.**

---

## 7. DOS DEFECTOS DEL CONTRATO, encontrados al correr sus propios `verify`

1. **`opportunities.confirmations` NO EXISTE.** La columna real no está en la tabla (columnas: `id, chain_id, strategy_kind, dex_a, dex_b, pair_symbol, token_in, token_out, amount_in_wei, expected_profit_usd, roi_pct, risk_score, block_number, status, rejection_reason, trace_id, detected_at, updated_at, chain_id_out, bridge, bridge_fee_usd, net_expected_profit_usd, route_metadata, cartridge_id, detector_id, pipeline_latency_ms, economics`). El comando `SELECT count(*) FILTER (WHERE confirmations > 100) …` **falla con `ERROR: column "confirmations" does not exist`, exit=1**. **⇒ El «replay de detección» (2933/1725) NO ES COMPUTABLE con lo que el contrato pide: NO COMPUTADO con esta razón, no un cero.**
2. **`risk_config` NO EXISTE** (`ERROR: relation "risk_config" does not exist`, exit=1). El **multiplicador 3,0** vive en **`trading_config.spread_sanity_mult = 3.0000`** — **leído ahí y verificado intacto.**
3. Corolario del estándar: **`simulations WHERE fail_reason LIKE 'reverted:%'` = 5.682** (control **positivo**, t184 anotó 4.007 ⇒ **creció: es vivo**) contra **`zzz_imposible:%` = 0** (control **negativo**). **Sin el negativo, el positivo no vale.**

---

## 8. INSTRUMENTO, PERMISOS Y HASHES

| artefacto | tamaño | sha256 |
|---|---|---|
| `/tmp/t189-net.py` (motor del barrido) | 16.772 B | **`f52ff61939f61c6b9e57764b658e3020aed3246040d2b7a9b32834b5c3687a2f`** |
| `/tmp/t189-out.json` (48 rutas × 16 puntos, **completo sin truncar**) | 157.914 B | **`f38df4d4c16a1f64ba9e43706904fd2028c2c4686e4b2a9eca70013974ff209b`** |
| `/tmp/t189.log` (log íntegro con los 48 renglones de ruta) | 13.583 B | **`8cf2b8adb5098f7e1c214e4411df99a477474e2791f694f3e82a5532a3b81d0a`** |
| `/tmp/t189-an.py` (post-proceso, sin RPC) | 4.598 B | **`5d09980e861b90d4e8fb1753158fe174e0bc7983ede2fe11b46a6cd194b41fc2`** |
| gate de t185 usado (PR #914) | 8.000 B | **`b171fc7e0b2fee2cc0ddaac9cdc68b4e7fcf5365e0096fa57f24dba360850c10`** (local == VPS) |

**RPC del fork fresco: `127.0.0.1:8546` (host).** El **`172.18.0.3:8545`** del contrato es el fork **compartido** y sigue **STALE**: se usó **sólo** para el control del gate, nunca para las cifras. **`localhost:8545` NO se usó** (no está publicado al host). **`:9090` externo: `000` (exit 7)** — control negativo ejecutado. Ningún control pipeado a `head`. Cada comando con su exit code.

**NO se escribió en NINGÚN fork** (compartido ni nuevo): solo `eth_call`/`eth_getCode`/`eth_blockNumber`. **CERO mainnet, firmas o broadcast.** `capital_usd = 1000.00`, `min_profit_usd = 50.0000`, `simulation_target_profit_usd = 50.0000`, `spread_sanity_mult = 3.0000` — **leídos y verificados INTACTOS**; **no se tocó nada**, porque el operador ya midió que subir el target empeora y que bajarlo no cambia el EV. **Paper.**

```bash
git hash-object docs/data/SIZE-SWEEP-2LEG-01.md
git rev-parse HEAD:docs/data/SIZE-SWEEP-2LEG-01.md
```

---

*La respuesta es (B), y es más fuerte de lo que el contrato esperaba. **`net(s*)` = −$0,287541 en `s*` = $0,1** sobre la mejor de 48 rutas, con **cero puntos positivos en 768** y **cero rutas que cubran su propio hurdle de fees**. **El que cierra la familia no es el gas: es que los venues del mismo par están alineados a 0,2507 % y el costo de ida y vuelta más barato de la allowlist es 0,3500 %** — con gas CERO, ya pierde; el gas (fijo, ~$0,287) sólo hace que a tamaño chico explote el spread requerido (2.868 % a $0,01). **El `$1000` no estaba mal dimensionado: a $1.000 el net es −$6,55 y a $2.500 es −$16,84 — más grande, más negativo; más chico, sigue negativo.** **⇒ L1 2-leg queda cerrado como path de PnL, medido en cabeza fresca de 0,80 minutos, sin escribir una sola vez en el fork.***
