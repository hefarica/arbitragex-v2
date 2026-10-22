# FICHA REA-1 — Bucle descubrir→evaluar→clasificar y criterio de rating (artemis + simple-arbitrage)

Fecha: 2026-10-23 · Autor: rea-analista-a (t2, attempt 32ff7895) · Alimenta: PR1 TWO-LEG-01 (t6, worktree `feat/two-leg-01`)
Skills cargadas: `arbx-reverse-engineering-fusile`, `arbx-skills`. Protocolo: contrato primero → función objetivo línea por línea → tests del upstream ANTES de escribir.

## 0. Fuentes fijadas (sha, jamás main)

| Fuente | SHA fijado | Fecha del commit | Verificación |
|---|---|---|---|
| `paradigmxyz/artemis` | `4bb158070833ec2b789a2ece14c896c4b28ce3be` | 2023-10-04 | `git clone` + `git rev-parse HEAD` = tip de `origin/main` (rama por defecto `main`) |
| `flashbots/simple-arbitrage` | `03cf1058b5bd8079561e2505efc85f77e357b829` | 2021-06-21 | `git rev-parse HEAD` = tip de `origin/master` (rama por defecto `master`, 32 commits totales) |

Clones de lectura: `C:\Users\HFRC\Desktop\repos\rea-sources\{artemis,simple-arbitrage}`.

**Corrección a la ficha previa** (`docs/reaintegration/rea/12-simple-arbitrage.md`): afirmaba "Tests: Ninguno" y "último push 2024-02-03". Ambos datos son falsos al sha: existen `test/Arbitrage.test.ts` y `test/UniswappyV2EthPair.test.ts` con vectores numéricos exactos (§5), y la historia git termina en 2021-06-21 (`git log --all` — el último commit es `03cf105 2021-06-21 updated searcher FAQ doc`). La fecha 2024-02 es metadata de GitHub (p. ej. cambio de settings), no un push.

## 1. Licencias leídas EN el sha citado + estado allowlist FUSILE (tal cual el gate)

| Fuente | Licencia al sha | Artefacto | rea_adopt (veredicto literal) |
|---|---|---|---|
| artemis | **MIT OR Apache-2.0** (dual, permisiva) | `LICENSE-MIT` + `LICENSE-APACHE` en el árbol del sha | Sin marca: **BLOQUEADO** (`falta_confirmacion_operador`, exige `CONFIRMED: artemis patterns`). Con la marca pre-registrada en `docs/security/FUSILE_SOURCE_POLICY.md:38` como `confirmacion`: **GO_VERBATIM**, `modo_permitido: verbatim_con_atribucion`. La marca sale de la política escrita por el operador; la aprobación por PR sigue siendo del operador. |
| simple-arbitrage | **ISC declarada**, SIN archivo LICENSE | `package.json:5` → `"license": "ISC"`; `Get-ChildItem -Recurse *licen*|*copying*` → 0 archivos | **BLOQUEADO** (`fuera_de_allowlist`). Enmienda registrada por el gate: **`CONFIRMED: allowlist amendment flashbots/simple-arbitrage`** (tier propuesto 2). Hasta que el operador la firme: código verbatim NO entra. Los NÚMEROS son hechos citables y entran siempre (FUSILE binding rule 3). |

Nota de divergencia skill↔tool: la rúbrica transcrita en la skill (6 criterios, pesos [30,20,20,15,10,5]) difiere de la que devuelve `rea_frame` en esta versión (5 dimensiones, pesos [20,25,15,25,15], umbral ≥0.70). Gana el **tool** (skill §0). No se puntuaron candidatos alternativos porque la fase FIJA las dos fuentes por plan del operador; `rea_frame` corrió igualmente para dejar queries/rúbrica/tier registrados (destino `backend/searcher-rs`, tier 2).

## 2. Contrato primero (tipos/traits — el 80% de la semántica)

### 2.1 artemis@4bb1580 — `crates/artemis-core/src/types.rs`

```rust
// types.rs:16-21  [CONTRATO]
pub trait Collector<E>: Send + Sync {
    async fn get_event_stream(&self) -> Result<CollectorStream<'_, E>>;   // Stream<Item = E>
}
// types.rs:23-32  [CONTRATO] — el corazón del port
pub trait Strategy<E, A>: Send + Sync {
    async fn sync_state(&mut self) -> Result<()>;              // una vez al arranque (estado inicial on-chain)
    async fn process_event(&mut self, event: E) -> Vec<A>;     // 0..N acciones por evento
}
// types.rs:34-39  [CONTRATO]
pub trait Executor<A>: Send + Sync {
    async fn execute(&self, action: A) -> Result<()>;
}
```
- `CollectorStream<'a, E> = Pin<Box<dyn Stream<Item = E> + Send + 'a>>` (types.rs:14) [CONVENCIÓN-Rust].
- `CollectorMap`/`ExecutorMap` (types.rs:43-95): adaptadores que mapean tipos entre capas; `ExecutorMap` traga acciones con `None` (types.rs:88-94) [BORDE: acción filtrada ≠ error].

### 2.2 simple-arbitrage@03cf1058 — `src/EthMarket.ts:18-53` (contrato abstracto)

```ts
abstract class EthMarket {
  get tokens(): Array<string>;        // [token0, token1] del pair
  get marketAddress(): string;        // address del pool
  get protocol(): string;             // etiqueta de DEX
  abstract getTokensOut(tokenIn, tokenOut, amountIn): BigNumber;   // fórmula out (§4.1)
  abstract getTokensIn(tokenIn, tokenOut, amountOut): BigNumber;   // fórmula in  (§4.1)
  abstract sellTokensToNextMarket(tokenIn, amountIn, nextMarket): Promise<MultipleCallData>;  // pata 1 del bundle
  abstract sellTokens(tokenIn, amountIn, recipient): Promise<string>;                          // pata 2 del bundle
  abstract receiveDirectly(tokenAddress): boolean;  // ¿la pata 2 recibe el token sin prepare?
  abstract prepareReceive(tokenAddress, amountIn): Promise<Array<CallDetails>>;
}
```
`CrossedMarketDetails` (Arbitrage.ts:8-14): `{ profit, volume, tokenAddress, buyFromMarket, sellToMarket }` — **la estructura que cruza el rating**. `MarketsByToken = { [tokenAddress]: EthMarket[] }` (Arbitrage.ts:16).

## 3. El bucle completo (descubrir→evaluar→clasificar)

### 3.1 artemis — el motor genérico (`crates/artemis-core/src/engine.rs`)

Línea por línea:
- engine.rs:79-80 **[FORMULA/ORDEN]**: dos canales `tokio::sync::broadcast` — `event_sender` y `action_sender`, capacidad default **512** cada uno (engine.rs:33-34). Broadcast: toda estrategia ve todo evento.
- engine.rs:85-99 **[ORDEN]**: se spawnean PRIMERO los executors (suscriben `action_sender`); cada acción se ejecuta secuencialmente por executor; error de execute se loguea y NO detiene el loop (engine.rs:91-94) [BORDE: fallo de acción aislado].
- engine.rs:102-123 **[INVARIANTE del ciclo]**: `strategy.sync_state().await?` UNA vez ANTES de consumir el primer evento (engine.rs:105) — el estado inicial se hidrata antes de procesar; luego loop `recv → process_event → send(action)` por cada acción devuelta (engine.rs:112-117).
- engine.rs:126-138 **[ORDEN]**: los collectors se spawnean AL FINAL; todo lo que emiten entra al canal de eventos (fan-out a todas las estrategias).
- **[INVARIANTE]**: un evento puede producir `Vec<A>` vacío (types.rs:31) — "sin oportunidad" no es error; es el caso común.

La estrategia de arb (`crates/strategies/mev-share-uni-arb/src/strategy.rs`):
- strategy.rs:64-83 `sync_state`: carga `resources/v3_v2_pools.csv` → `pool_map: HashMap<H160(v3_pool) → V2PoolInfo{v2_pool, is_weth_token0}>` [CONVENCIÓN-descartada: CSV como fuente de pools — el destino ya tiene discovery].
- strategy.rs:86-111 `process_event`: **(a)** si el evento no tiene logs → `vec![]` (91-93) [BORDE]; **(b)** toma `event.logs[0].address` (94) — SOLO el primer log decide si el pool interesa (96-98) [ORDEN: el filtro es por dirección del primer log, no por todos]; **(c)** si el pool está en el mapa → genera bundles (104-108).
- strategy.rs:116-213 `generate_bundles`: **NO evalúa profit ni puntúa** — arma un ladder fijo de 14 tamaños (100_000 … 1e15 wei, strategy.rs:126-141) y manda un bundle por tamaño con `payment_percentage=0`, gas 400_000, validez `block+1 … block+30` (200-208). **[DESCARTADO para TWO-LEG-01: el shotgun de tamaños sin rating; el rating real del ecosistema Flashbots vive en simple-arbitrage (§4)]**. El ladder SÍ es precedente del ladder de tamaños del destino (`size_optimizer.rs` grid 16 puntos, FASE1_BASE §2.2).

### 3.2 simple-arbitrage — el bucle con rating (`src/index.ts:46-66`)

```
on block:
  1. updateReserves(provider, allMarketPairs)          // index.ts:57 → UniswappyV2EthPair.ts:109-119
  2. bestCrossedMarkets = evaluateMarkets(marketsByToken)  // index.ts:58 → Arbitrage.ts:94-123  (§4)
  3. si vacío → "No crossed markets", return           // index.ts:59-62
  4. takeCrossedMarkets(best, blockNumber, 80)          // index.ts:64 → Arbitrage.ts:126-182 (§4.3)
```
- **[ORDEN]**: reservas SIEMPRE se refrescan ANTES de evaluar, en cada bloque (index.ts:56-58).
- **[FORMULA]**: `MINER_REWARD_PERCENTAGE` default **80** (index.ts:16, README) — porcentaje del profit al minero; variable de entorno, no literal.
- **[DESCARTADO]**: healthcheck HTTP post-submit (index.ts:39-44) y wallet/relay setup — convención del fuente.

## 4. Criterio de rating — línea por línea (Arbitrage.ts)

### 4.1 Fórmulas base del pool (`UniswappyV2EthPair.ts:150-161`) — idénticas a UniswapV2Router

```ts
// getAmountIn (L150-154)  [FORMULA]  — ceil + 1: la fee se descuenta del input
numerator   = reserveIn * amountOut * 1000
denominator = (reserveOut - amountOut) * 997
return numerator/denominator + 1        // BigInt div = floor; el +1 redondea ARRIBA (perjudica al que compra)
// getAmountOut (L156-161)  [FORMULA]  — floor
amountInWithFee = amountIn * 997
numerator   = amountInWithFee * reserveOut
denominator = reserveIn * 1000 + amountInWithFee
return numerator/denominator            // floor, sin corrección
```
- **[ORDEN crítico]**: la fee (997/1000, 0.30%) se aplica ANTES de la división, sobre `amountIn`, y en `getAmountIn` el `+1` va DESPUÉS del floor (L153).
- **[BORDE]**: `getTokensIn/getTokensOut` tiran si el token no está en balances (`getBalance` L121-125 lanza "bad token"); `prepareReceive` lanza si `amountIn <= 0` (L41-43).

### 4.2 `evaluateMarkets` (L94-123) — el clasificador

1. **Probe de precio con tamaño FIJO** (L99-105) **[FORMULA/umbral]**: cada mercado se pre-cotiza con **0.01 ETH** (`ETHER.div(100)`), AMBAS cotizaciones en unidades del TOKEN puente:
   - `buyTokenPrice = getTokensIn(token→WETH, 0.01e18)` = unidades del TOKEN que este mercado EXIGE para entregar 0.01 WETH (token caro ⇒ exige pocas).
   - `sellTokenPrice = getTokensOut(WETH→token, 0.01e18)` = unidades del TOKEN que este mercado ENTREGA por 0.01 WETH (token barato ⇒ entrega muchas).
2. **Regla de cruce** (L107-114) **[INVARIANTE del clasificador]**: para el par ordenado `(A=pricedMarket, B=pm)`, es candidato **sii `B.sellTokenPrice > A.buyTokenPrice`** (B entrega más tokens por 0.01 WETH de los que A exige para devolverlos) → se compran tokens en **B** (barato) y se venden en **A** (caro). El par se guarda como `push([A, B])` (L111), y `getBestCrossedMarket` lo lee `crossed[0]=sellToMarket=A`, `crossed[1]=buyFromMarket=B` (L34-35) **[ORDEN: el índice 0 VENDE, el 1 COMPRA — source de bugs si se invierte; verificado contra el vector V2: buyFrom=m1 reservas [1,1] (barato), sellTo=m0 [1,2] (caro)]**. Doble loop O(n²) sobre los mercados del token (incluye A==B y ambos sentidos: A-B y B-A se evalúan por separado).
3. **Mejor tamaño por par** — `getBestCrossedMarket` (L31-67):
   - Ladder de 9 tamaños **[FORMULA/constante]**: `TEST_VOLUMES = [0.01, 0.1, 1/6, 0.25, 0.5, 1, 2, 5, 10] ETH` (L19-29).
   - Para cada tamaño: `mid = buyFrom.getTokensOut(WETH→token, size)`; `proceeds = sellTo.getTokensOut(token→WETH, mid)`; `profit = proceeds − size` (L37-39) **[FORMULA: profit en WETH, fee de ambas patas incluida por las fórmulas §4.1; gas NO incluido en el rating]**.
   - **[BORDE/ORDEN — meet halfway]**: si el profit BAJA respecto al mejor acumulado (L40), prueba UN único punto medio `trySize = (size + bestVolume)/2` (L42); si mejora, reemplaza; y hace `break` del ladder en cualquier caso (L55) — el scan NO es exhaustivo: se corta en la primera caída.
   - **[BORDE]**: devuelve el mejor PARXA aunque `profit < 0` (el test no-cross lo demuestra, §5 V3) — el signo se filtra después.
4. **Umbral de admisión** (L117) **[FORMULA/umbral]**: el candidato entra a la lista **sii `profit > 0.001 ETH`** (`ETHER.div(1000)`).
5. **Orden final** (L121) **[INVARIANTE del rating]**: `sort` descendente por `profit` — el ranking ES el profit bruto en WETH (sin gas, sin multiplicador). Empates → orden estable (comparador devuelve 0).

### 4.3 `takeCrossedMarkets` (L126-182) — consumir el ranking

- Recorre en orden de rating; para cada candidato construye las DOS patas: `buyCalls = buyFrom.sellTokensToNextMarket(WETH, volume, sellTo)` (L130) y `sellCallData = sellTo.sellTokens(token, inter, executor)` con `inter = getTokensOut(WETH→token, volume)` recomputado en vivo (L131-132) **[ORDEN: `inter` se recomputa, no se reusa el del rating]**.
- Gas: estimateGas; si falla → `continue` (siguiente del ranking) (L154-157); si `> 1_400_000` → desconfianza y `continue` (L149-152); si ok → `gasLimit = estimate*2` (L153) **[FORMULA de seguridad: x2]**.
- Simulación Flashbots en `blockNumber+1`; si error o `firstRevert` → `continue` (L167-171).
- Envía el MISMO bundle a **dos bloques**: `blockNumber+1` y `+2` (L173-177) **[INVARIANTE de entrega]**.
- **[BORDE]**: `return` tras el PRIMER envío exitoso (L179) — consume un solo candidato por bloque (TODO L125); si NINGUNO pasa → `throw "No arbitrage submitted to relay"` (L181) — vacío explota, no devuelve silencio.

## 5. Vectores numéricos del upstream (con verificación independiente)

**Artefacto reproducible**: `docs/rea/vectors_rea1_verify.mjs` — re-implementación BigInt del ladder + fórmulas, ejecutable con `node docs/rea/vectors_rea1_verify.mjs`. Corrida de esta sesión: exit_code 0, las 7 verificaciones `ok` (V1a–V1d, V2 volume, V2 profit, V3). Si el port de TWO-LEG-01 re-implementa el rating, ese script (o su trascripción a test Rust) es el test de paridad.

### V1 — `test/UniswappyV2EthPair.test.ts:20-27` (pool: tokens `[TOKEN, WETH]`, reservas `[1 TOKEN, 2 WETH]`)

| Entrada | Salida upstream | Re-impl | ✓ |
|---|---|---|---|
| `getTokensIn(TOKEN→WETH, out=0.1e18)` | `52789948793749671` (L21) | `52789948793749671` | ✓ |
| `getTokensIn(WETH→TOKEN, out=0.1e18)` | `222890894906943052` (L22) | `222890894906943052` | ✓ |
| `getTokensOut(TOKEN→WETH, in=52789948793749671)` | `ETHER.div(10).add(1)` = `100000000000000001` (L25) | `100000000000000001` | ✓ (el `+1` de getAmountIn — ida y vuelta gana 1 wei) |
| `getTokensOut(WETH→TOKEN, in=222890894906943052)` | `ETHER.div(10)` = `100000000000000000` (L26) | `100000000000000000` | ✓ |

### V2 — `test/Arbitrage.test.ts:21-32` "Calculate Crossed Markets" (EL vector del port)

Setup: market[0] reservas `[1 TOKEN, 2 WETH]`, market[1] reservas `[1 TOKEN, 1 WETH]`; `getBestCrossedMarket([groupedWethMarkets], TOKEN)` (par: [0]=sellTo=m0, [1]=buyFrom=m1).

| Campo | Valor upstream | Re-impl | ✓ |
|---|---|---|---|
| `volume` | `208333333333333333` wei ≈ 0.2083 ETH (L30) | `208333333333333333` | ✓ |
| `profit` | `0x012be1d487a428ce` = `84409320961485006` wei ≈ 0.0844 ETH (L31) | `84409320961485006` (0x12be1d487a428ce) | ✓ |

**Semántica del vector** (verificada): el ladder sube (0.01→…→1/6 ETH mejorando), en `0.25 ETH` el profit CAE → dispara el meet-halfway `(1/6 + 1/4)/2 = 208333333333333333` que es el óptimo reportado. **El test del port que NO ejercite el paso halfway NO reproduce este vector.**

### V3 — `test/Arbitrage.test.ts:33-43` "no cross" (control negativo)

Reservas iguales `[1,1]` y `[1,1]` → `getBestCrossedMarket` devuelve objeto (NO undefined) con `profit < 0` (L42). Re-impl: `profit = −253954640256575` < 0 ✓. **[INVARIANTE: el clasificador devuelve candidato negativo; el umbral 0.001 ETH (L117) es quien lo excluye]**

### Constantes del rating (todas ruta:linea del sha)

| Constante | Valor | Fuente |
|---|---|---|
| Probe de precio | 0.01 ETH = 1e16 wei | Arbitrage.ts:102-103 |
| Umbral de admisión | 0.001 ETH = 1e15 wei | Arbitrage.ts:117 |
| Ladder de tamaños | 9: [1e16, 1e17, 1.66…e17, 2.5e17, 5e17, 1e18, 2e18, 5e18, 1e19] | Arbitrage.ts:19-29 |
| Fee de pool | 997/1000 (0.30%) | UniswappyV2EthPair.ts:151-159 |
| Redondeo getAmountIn | floor + 1 | UniswappyV2EthPair.ts:153 |
| Miner reward default | 80% | index.ts:16 |
| Gas cap sospechoso | 1_400_000; gasLimit = estimate×2 | Arbitrage.ts:149,153 |
| Bloques destino del bundle | +1 y +2 | Arbitrage.ts:173 |
| Capacidad canales artemis | 512/512 (event/action) | engine.rs:33-34 |
| Ladder artemis | 14 tamaños 1e5…1e15 wei, gas 400_000, validez +1…+30 | strategy.rs:126-141,174,202-206 |

## 6. Semántica de la segunda pata — qué datos persiste de cada pool del ciclo

**Del pool (EthMarket/UniswappyV2EthPair), por pata del ciclo:**
1. `marketAddress` (EthMarket.ts:31-33) — address del POOL (no del router).
2. `protocol` (EthMarket.ts:27-29) — etiqueta del DEX (en artemis: qué pool es v2 vs v3).
3. `tokens: [token0, token1]` en ORDEN del pair (EthMarket.ts:19-21; se construyen con `[pair[0], pair[1]]` crudos, UniswappyV2EthPair.ts:67) — la DIRECCIÓN de la pata se deriva comparando `tokenIn === tokens[0]` (UniswappyV2EthPair.ts:184-192) y decide `amount0Out` vs `amount1Out` (L181-189). En artemis el bit de dirección equivalente es `is_weth_token0` (strategy.rs:31, V2V3PoolRecord types.rs:29).
4. `_tokenBalances[token] = reserva` por token, actualizada en bloque (UniswappyV2EthPair.ts:109-119 `updateReserves`; `setReservesViaOrderedBalances` L127-136 solo escribe si difiere — L133 `_.isEqual` guard **[ORDEN: update idempotente, no invalida caches]**).
5. Filtro de población de pools (UniswappyV2EthPair.ts:98-100): sólo pairs con WETH (`getUniswappyMarkets` L59-64 descarta pairs sin WETH), agrupados `marketsByToken[tokenNoWETH]`, y tras updateReserves **sólo sobreviven los pools con `balance(WETH) > 1 ETHER`** (L99) **[INVARIANTE: pool sin liquidez real >1 ETH sale de la población ANTES del rating]**. Sólo tokens con `>1` mercado entran a candidatos (L89-94).
6. Blacklist de 1 token (L15-17) **[CONVENCIÓN-descartada: la lista específica es config del operador, no viaja]`.

**Del candidato clasificado (CrossedMarketDetails, Arbitrage.ts:8-14)** — lo que "persiste" el rating por candidato: `profit` (wei), `volume` (wei), `tokenAddress` (token puente), `buyFromMarket` (ref pool A), `sellToMarket` (ref pool B). Es EXACTAMENTE el par (pool A, pool B) con dirección implícita en el orden de tokens — el análogo en el destino es `dex_a, dex_b + route_metadata.pool_addresses[0..1] + token_in/token_out` del INSERT (persistence.rs:148-166).

## 7. Mapa al destino (TWO-LEG-01, sobre mapa de FASE1_BASE.md §2.1)

| Semántica del fuente | Destino | Nota |
|---|---|---|
| Bucle on-block: refresh reservas → evaluar → clasificar (index.ts:56-64) | `scanner.rs` productores + `persistence::insert_opportunity_with_route` (persistence.rs:80, INSERT L148-166) | el destino ya tiene el bucle; falta asegurar que el PAR completo llega al INSERT |
| `CrossedMarketDetails{buyFromMarket, sellToMarket}` | `hop_cycle_bridge.rs:737-744` (hoy sólo `dex_adapters[0]`/`[1]`) y `dex_engine.rs:1302-1328` (2-pool sí llena `dex_a/dex_b`) | TWO-LEG-01: el emisor del ciclo persiste AMBAS patas (pool A + pool B) como par first-class, con `dex_b` no-null y `pool_addresses[1]` en route_metadata |
| Rating: probe→cruce→mejor tamaño→umbral→sort desc profit | criterio de orden del emisor al persistir candidatos | la fila persistida debe llevar profit_wei del rating (ya existe `economics`, persistence.rs:192-201) |
| Meet-halfway del ladder (L40-55) | `size_optimizer.rs` golden_section 2-leg (L2753/L3650) | NO se porta el ladder (el destino tiene optimizador); se porta la INVARIANTE: el candidato se reporta con su profit al tamaño óptimo, no al primer tamaño |
| `sync_state` antes del primer evento (engine.rs:105) | hidratación de pools del scanner antes de emitir | invariante de arranque |
| Filtro de población `balance(WETH) > 1 ETHER` (L99) | (relaciona con PR3 POBLACION-FILTER-01) | aquí sólo se documenta la semántica upstream: población = pools con liquidez real |

**Test del port (regla de oro)**: transcribir V2 (y V1 como fórmula base) como casos del suite del searcher: dado par de pools con esas reservas, el rating produce `volume=208333333333333333`, `profit=84409320961485006`, y la fila emitida lleva `dex_b` = pool B no-null + `pool_addresses[1]` presente. Control negativo: V3 (sin cruce → candidato excluido por umbral, NO fila con profit negativo). Cita obligatoria en commit/PR: `REA: flashbots/simple-arbitrage@03cf1058b5bd8079561e2505efc85f77e357b829 src/Arbitrage.ts:31-67,94-123` (números) y `REA: paradigmxyz/artemis@4bb158070833ec2b789a2ece14c896c4b28ce3be crates/artemis-core/src/engine.rs:75-141` (bucle).

## 8. Qué se descarta a propósito

- Todo código TS/JS del fuente (sin enmienda de allowlist, además el destino es Rust — se reimplementa semántica).
- Shotgun de 14 bundles de artemis sin rating (strategy.rs:126-141) — el destino tiene sizing propio; artemis aporta el BUCLE, no el sizing.
- CSV de pools y blacklist específica (config del operador, G7: nada de addresses ajenas viaja).
- `MINER_REWARD_PERCENTAGE`, healthcheck, wallets, FlashbotsBundleProvider setup (convenciones/infra del fuente; el destino tiene relays-client).
- Logging `console.log`/`info!` del fuente (el destino tiene su telemetría).
- Licencia: no se copia texto de licencia ajeno al repo destino salvo atribución (artemis GO_VERBATIM exige atribución si algún día entra verbatim).

## 9. Pendientes / no computado (fail-honest)

- `contracts/BundleExecutor.sol` del fuente (existe en el sha, `contracts/`) NO se leyó línea a línea en esta ficha — es materia de FICHA_REA4 (t4), no de esta.
- La rúbrica del tool no se aplicó a candidatos alternativos (la fase fija las fuentes); score de candidatos: **no computado**, razón: no había decisión que tomar.
- `TEST_VOLUMES` en wei exactos: 1/6 y 1/4 ETH son divisiones BigInt (`ETHER.div(6)` = 166666666666666666), el re-impl las reprodujo exactamente (V2 pasa con volume 208333333333333333 = (1e18/6 + 1e18/4)/2 floor).
- Ejecución de la suite jasmine del upstream: no ejecutada (dependencias TS de 2021); los vectores se verificaron por re-implementación independiente en BigInt (§5), que es la prueba falsable de la lectura.
