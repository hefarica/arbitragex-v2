# QUOTER-DECIMALS-01 — Cotizar lo que ya se detecta: tres causas de rechazo y el mapa de `decimals`

**Tarea:** t187 · **Base:** `origin/main` = `4902a47c16ef21b7683e5bf405ab49cd1c525b77` · **Rama:** `docs/quoter-decimals-01`
**Runtime medido:** `77b42b3d` · **Estado:** PAPER. Cero mainnet, cero firmas, cero broadcast, cero escrituras al fork.

---

## 0. ALCANCE — LO QUE PUEDO Y LO QUE NO PUEDO HACER, DICHO ANTES DE CUALQUIER NÚMERO

**`backend/` está FUERA de alcance ⇒ la reparación NO se aplica ⇒ NO existe un «después» que medir.** Y hay una segunda razón independiente: **no hubo reinicio.**

```
docker inspect --format '{{.State.StartedAt}}' arbitragex-v2-searcher-rs-1  →  2026-10-08T14:16:07.102088235Z
docker inspect --format '{{.State.StartedAt}}' arbitragex-v2-sim-ctl-1     →  2026-10-08T14:16:07.1195963Z
docker inspect --format '{{.State.StartedAt}}' arbitragex-v2-anvil-1       →  2026-10-08T14:16:01.110230687Z
```
Los tres del **mismo evento de deploy**, y **`searcher` y `sim-ctl` con el MISMO `StartedAt` que en t184**: no ha reiniciado nada. **⇒ «después» = NO COMPUTADO, con dos razones: (a) no se aplicó reparación; (b) no hay frontera pre/post.**

**Lo que SÍ se computa, y mueve el resultado: dónde vive de verdad cada defecto. TRES DE LAS CUATRO CAUSAS NO SON LO QUE SU NOMBRE DICE.** Eso es el hallazgo.

**Y «una reparación que no mueve el `eligible` se declara reparación sin efecto». Aquí no hay reparación y tampoco hay `eligible`: ver §6.**

---

## 1. RESULTADO EN CUATRO LÍNEAS

| Causa | El nombre dice | **La medición dice** |
|---|---|---|
| `v3_quote_unavailable` | «el Quoter no cotizó» | **El `QuoterV2` cotiza 8 de 8** combinaciones par×tier, `rc=0`, a bloque fijo. El Quoter **no está roto.** |
| `v3_pool_not_catalogued` | «el pool no está en el catálogo» | **72 de 72 pools de esas rutas ESTÁN en la tabla `pools`.** El defecto real: **21 de 72 no están en el índice V3 de Redis, y son exactamente los 21 con `fee_tier = 30` (V2)**. |
| `no_tradable_size` | «`amount_in_wei = 0`» | **CONFIRMADO: `106.638` de `106.703` = `99,94 %`.** El productor es el nombre. |
| **`decimals` del ciclo** | «EURC: on-chain 6, mapa 18» | **CONFIRMADO Y AMPLIADO: `8` de `37` entradas del `decimals.map` discrepan, y el valor erróneo es SIEMPRE `18`.** EURC: `6→18`, factor **1e12**. Y **PG `tokens.decimals` y Redis tienen el valor CORRECTO en los 8.** |

---

## 2. LOS CONTEOS, CON EL MISMO CORTE — y la tabla crece mientras se mide

Tres lecturas del mismo agregado, separadas por minutos:

```
#1  (GROUP BY, LIMIT 25)   unavailable 760.753 | not_catalogued 1.207.364 | no_tradable 106.697
#2  S1 (agregado)          unavailable 762.995 | not_catalogued 1.211.338 | no_tradable 106.732 | total 10.600.401 | maxblk 26.150.018
#3  (recon previo)         unavailable 760.962 | not_catalogued 1.207.638 | no_tradable 106.700 | total 10.573.163 | maxblk 26.149.986
```

**⇒ La tabla crece ~32 bloques entre lecturas. Las diferencias SON el crecimiento, y se declaran en vez de elegir la cifra que conviene.** Ninguna de las tres es «el ANTES» definitivo porque **no va a haber un DESPUÉS**: son tres fotos del mismo estado no reparado.

**El techo de mercado, en `opportunities.rejection_reason`** (`NULLS LAST`), con las tres causas señaladas:

| `rejection_reason` | n |
|---|---|
| `spread_negative_round_trip` | 5.785.136 |
| **`v3_pool_not_catalogued`** | **1.207.364** |
| `non_positive_profit` | 1.112.689 |
| `spread_zero_equilibrium` | 787.040 |
| **`v3_quote_unavailable`** | **760.753** |
| `single_pool_no_spread` | 334.838 |
| `v3_pair_no_pools` | 299.246 |
| **`no_tradable_size`** | **106.697** |
| `StrategyDisabled:triangular_arb` | 84.566 |
| `v3_multileg_budget_exhausted` | 38.218 |
| `v3_pool_revert` | 16.964 |

**Y el caso testigo del OPERADOR, en las rutas reales (6 h) — es el mayor infractor de cada causa:**

| dex_a | dex_b | `pair_symbol` | causa | n |
|---|---|---|---|---|
| UniswapV3 | UniswapV3 | `1abaea…/a0b869…` = **EURC/USDC** | `v3_pool_not_catalogued` | **112.481** |
| UniswapV3 | UniswapV2 | `1abaea…/a0b869…` | `v3_pool_not_catalogued` | 36.812 |
| UniswapV2 | UniswapV3 | `1abaea…/a0b869…` | `v3_pool_not_catalogued` | 36.689 |
| UniswapV3 | PancakeSwap V3 | `c02aaa…/dac17f…` = **WETH/USDT** | `v3_quote_unavailable` | **15.755** |
| UniswapV3 | UniswapV3 | `c02aaa…/dac17f…` | `v3_quote_unavailable` | 8.025 |
| UniswapV2 | SushiSwap | `c02aaa…/3f382d…` = **WETH/GHST** | **`no_tradable_size`** | **1.777** |
| SushiSwap | UniswapV3 | `c02aaa…/3f382d…` | `v3_pool_not_catalogued` | 1.751 |

**Los tres pares testigo son el nº1 de su causa. El enunciado del OPERADOR está confirmado en los datos.**

**Y los cuatro pares profundos de la allowlist SÍ se detectan** (`pair_symbol` en 30 min): `c02aaa…/dac17f…` **WETH/USDT 72.408** · `1abaea…/a0b869…` **EURC/USDC 20.646** · `c02aaa…/2260fa…` **WETH/WBTC 20.646** · `2260fa…/a0b869…` **WBTC/USDC 3.441** · `a0b869…/c02aaa…` **USDC/WETH 63**. **⇒ no es la cola larga: son los pares de la allowlist.**

---

## 3. EL DEFECTO DE `decimals` — LOCALIZADO, CON MAGNITUD, Y NO ES DONDE PARECÍA

**Método:** se tomaron **TODAS las entradas del `decimals.map` que el ciclo escribe en `route_metadata`** (ventana 2 h) — **37 tokens** — y se compararon contra **cuatro fuentes**: el contrato (`decimals()`, `eth_call` a bloque fijo), `tokens.decimals` (PG), `arbx:tokens:1:<addr>.decimals` (Redis) y el valor del mapa.

```
U1_tokens_en_el_mapa = 37      U1_discrepantes = 8
```

| token | on-chain | **mapa del ciclo** | PG `tokens` | Redis | factor |
|---|---|---|---|---|---|
| `0x1abaea1f7c…` **EURC** | **6** | **18** | 6 | 6 | **1e12** |
| `0xa1f410f13b…` | 6 | **18** | 6 | 6 | 1e12 |
| `0xac51066d7b…` | 6 | **18** | 6 | *(ausente)* | 1e12 |
| `0x2b591e99af…` | 8 | **18** | 8 | 8 | 1e10 |
| `0x72e4f9f808…` | 8 | **18** | 8 | 8 | 1e10 |
| `0x14fee68069…` | 9 | **18** | 9 | 9 | 1e9 |
| `0x95af4af910…` | 9 | **18** | 9 | 9 | 1e9 |
| `0xa606d43397…` | 9 | **18** | 9 | *(ausente)* | 1e9 |

**Tres hechos que el patrón obliga a leer:**

1. **El valor erróneo es SIEMPRE `18`, en los 8 casos.** No hay dispersión: **no es un dato corrompido, es un DEFAULT.** El ciclo no lee `tokens.decimals`: **asume 18 y sigue.**
2. **`tokens.decimals` (PG) es correcto en los 8. Redis es correcto en los 6 que tienen entrada.** ⇒ **hay CUATRO mapas de `decimals` y el único que miente es el que se escribe por fila en `route_metadata.decimals.map`.** Un mapa que no sale del contrato, y que **tampoco sale de los dos catálogos internos que ya tienen el dato bueno.**
3. **El error no es cosmético: es un factor `1e9`–`1e12` en la cantidad.** Una ruta que atraviese cualquiera de esos 8 tokens convierte unidades humanas contra 18 decimales cuando el token tiene 6, 8 o 9.

**El caso testigo del OPERADOR queda CONFIRMADO en su causa:** `cast call 0x1aBaEA1f… 'decimals()(uint8)'` → **`6`**, `symbol()` → **`"EURC"`** (exit=0 ambos); el mapa del ciclo, en las filas EURC reales (`267.094` filas en 6 h), dice **`0x1abaea1f7c830bd89acc67ec4af516284b1bc33c=18`**. **Y EURC/USDC es el nº1 de `v3_pool_not_catalogued`.** Los dos defectos coinciden en el mismo token.

**LO QUE NO RE-DERIVO, y se dice:** el síntoma que reportó el OPERADOR — **un `amount_out_wei` de 12 sats de WBTC en una ruta de 5 hops** — **NO lo reproduje**. Medí la causa (el mapa), su población (8 de 37) y su magnitud (1e9–1e12). **La aritmética exacta que convierte ese factor en 12 sats no está re-derivada aquí y no se afirma.** Es un hueco, no un resultado.

---

## 4. `v3_pool_not_catalogued` — EL NOMBRE MIENTE

**Se midió la población exacta: los pools que aparecen en rutas marcadas `v3_pool_not_catalogued` (6 h).**

```
U1/S5_rutas_no_catalogadas_6h  = 72 pools distintos
S5_de_esas_cuantas_en_pools    = 72        ← 72 de 72
T3_activos_en_tabla            = 72        ← y los 72 ACTIVOS
T3_en_indice_v3 (Redis)        = 51
T3_NO_en_indice_v3 (Redis)     = 21
```

**⇒ LOS 72 POOLS ESTÁN EN LA TABLA `pools`, Y LOS 72 ESTÁN ACTIVOS.** Un `v3_pool_not_catalogued` cuyo pool **sí está catalogado** no puede significar lo que su nombre dice.

**Y el productor aparece al mirar los 21 que faltan:**

```
fee_tier de los 72 pools de esas rutas → 30 n=21 | 3000 n=15 | 10000 n=15 | 500 n=11 | 100 n=10
fuera del índice V3 de Redis          → 21
```

**⇒ `21` = `21`. Los pools que NO están en el índice V3 de Redis son EXACTAMENTE los de `fee_tier = 30`.**
**Y `30` no es un tier de Uniswap V3**: los tiers V3 son **100 / 500 / 3000 / 10000**. `30` son **basis points de un pool V2 (0,30 %)**.

**⇒ MECANISMO, medido y no inferido: la ruta incluye un pool V2; el verificador lo busca en el índice V3 de Redis; no lo encuentra; y marca la RUTA ENTERA como `v3_pool_not_catalogued`.** El rechazo no habla del catálogo: habla de que **la tabla `pools` mezcla V2 y V3 en la misma columna `fee_tier` sin discriminador**, y el índice de Redis sólo tiene V3.

### 4.1 El criterio de catalogación, con sus números

`pools` (chain 1): **4.279 total · 1.354 activos · 1.808 con `fee_tier` · 2.471 con `fee_tier` NULL.**

```
fee_tier:  NULL 2.471 | 30 902 | 3000 365 | 10000 315 | 100 142 | 500 75 | 2500 9
enum_source: alchemy 2.865 | reactive 1.250 | subgraph_tvl 76 | seed 60 | dexscreener 28
last_enumerated_at: 1.386 SIN FECHA (32,4 %) | 2.893 con fecha | 2026-08-08 → 2026-10-08T17:16:40Z
```

**Tres cosas que esto dice, y ninguna es buena:**

- **`fee_tier = 30` (902 pools) y `fee_tier = 2500` (9) NO son tiers V3.** La columna guarda dos semánticas distintas ⇒ **cualquier consumidor que la lea como tier V3 se equivoca en 911 filas.**
- **`fee_tier` NULL en `2.471` de `4.279` (57,7 %).** Sin tier no hay arista cotizable.
- **`1.386` pools (32,4 %) sin `last_enumerated_at`**: no se puede saber si su enumeración está viva.
- **Fuente y frescura**: `alchemy` 2.865 + `reactive` 1.250 + `subgraph_tvl` 76 + `seed` 60 + `dexscreener` 28. La enumeración más reciente es **`2026-10-08T17:16:40Z`**, ~3 h antes de esta medición.

**⇒ «El catálogo V3 se completa o se declara su límite»: SE DECLARA EL LÍMITE, y es medible.** El índice V3 de Redis tiene **779 claves `arbx:pool_index_v3:*` y 973 pools distintos**; la tabla tiene **1.808 con tier**. **NO SE INVENTA NINGÚN POOL.** El índice se puebla de una fuente externa (alchemy/reactive/subgraph) que no controlo desde aquí: **completarlo es trabajo de la sesión de enumeración, no de esta, y `backend/` está fuera de alcance.**

---

## 5. `v3_quote_unavailable` — EL `QuoterV2` **NO** FALLA. 8 de 8.

**Llamada REAL, `eth_call` sobre el fork, a BLOQUE FIJO DECLARADO `26148216`.** `QuoterV2 = 0x61fFE014bA17989E743c5F6cB21bF9697530B21e` (se verificó que tiene código en el fork, `rc=0`). Respuesta cruda:

```
U3_bloque = 26148216   rc=0

WETH/USDC fee500   rc=0  →  (2529726328, 1574845530155820749471847757969311, 1, 90007)   ⇒ $2.529,73 / WETH
WETH/USDC fee3000  rc=0  →  (2518117288, 1576491116082495060987856820115894, 1, 90063)   ⇒ $2.518,12
WETH/USDT fee500   rc=0  →  (2531548407, 3987178755122843609592420,           0, 99755)   ⇒ $2.531,55
WETH/USDT fee3000  rc=0  →  (2519529497, 3982820281556226740351625,           1, 91004)   ⇒ $2.519,53
WBTC/WETH fee500   rc=0  →  (32542221680996149225, 45198878021019267414602437010381412, 2, 118376)
WBTC/WETH fee3000  rc=0  →  (32389507155893205388, 45125769430182048710009326486949941, 2, 118044)
USDC/USDT fee500   rc=0  →  (1000418,  79264545352247709153027308367,         1, 98032)   ⇒ 1,000418 USDT/USDC
USDC/USDT fee3000  rc=0  →  (997276,   79239131567338945709567933509,         1, 98042)   ⇒ 0,997276 USDT/USDC
```

**(tupla = `amountOut`, `sqrtPriceX96After`, `initializedTicksCrossed`, `gasEstimate`.)**

**⇒ CERO fallos. CERO errores. `rc=0` en las 8 combinaciones de los 4 pares de la allowlist × 2 fee tiers, incluido `USDC/USDT`, que cotiza por un pool directo.**

**⇒ El contrato dice «el Quoter no cotizó». Medido: el Quoter cotiza.** La etiqueta `v3_quote_unavailable` **no describe una avería del `QuoterV2`**. Y **no se declara qué describe**: el camino interno que la emite vive en `searcher-rs` (**fuera de alcance**) y **no se localizó**.

**⇒ UN QUOTER QUE FALLA CON RAZÓN NOMBRADA ES DIAGNOSTICABLE. AQUÍ EL QUOTER NO FALLA, Y LA RAZÓN DEL RECHAZO NO ES EL QUOTER.** Es un resultado, y es un hallazgo de instrumento.

**Contraste útil medido en el mismo fork congelado:** `QuoterV2.WETH9()` → `0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2`, `rc=0`. Y los precios implicados (`$2.518–$2.531` WETH, WBTC/WETH `32,54`) son coherentes con `token_prices_usd` de la config (**`WETH: 2469,45`**, **`WBTC: 81728,88`**), no con el fork de hace horas: **el `QuoterV2` lee el fork, y la config tiene el precio de producción. Se declara la diferencia; no se mezclan.**

---

## 6. `no_tradable_size` — EL PRODUCTOR ES EL NOMBRE, Y HAY UN RESIDUO

```
Q3:  ain_cero = 106.638 | ain_null = 0 | ain_pos = 65 | total = 106.703
```

**⇒ `106.638 / 106.703 = 99,94 %` tienen `amount_in_wei = 0`.** El productor es exactamente el que el nombre declara. **CERO NULL ⇒ no hay ambigüedad.**

**Y el residuo de `65` se declara, no se redondea:** hay filas marcadas `no_tradable_size` **con `amount_in_wei > 0`**. Muestra medida: `pair_symbol` + `ain` + `dex_a|dex_b` (ver §11 huecos: no se explica su causa).

**Su peso en el feed vivo:** en 30 min, `amount_in_wei = 0` sobre el total de filas — **la magnitud de lo que el `forwardSimulate` del api-server rechaza en seco sin mirarlo** (exige `amount_in_wei > 0`).

**Y su caso testigo:** `WETH/GHST` (`c02aaa…/3f382d…`) sobre `UniswapV2|SushiSwap` = **1.777** en 6 h. **GHST on-chain = `18`** (`cast call … decimals()` → `18`, `rc=0`), `tokens.decimals` = `18`, Redis = `18`, mapa del ciclo = `18` **⇒ GHST NO es uno de los 8 discrepantes.** Su causa es otra y **no se declara cuál: no medida.**

---

## 7. EL EFECTO: `eligible` Y «FORK GASTADO» — **NO COMPUTADO**, CON SU RAZÓN

El contrato pide `eligible` y fork gastado antes/después. **Se intentó y no está. Se declara como NO COMPUTADO, no como cero** — la distinción `UNREADABLE` vs `0` que el capitán fijó en `docs/ledger/LEDGER-SPINE-SIM-01.md`.

**`eligible` NO EXISTE como métrica de Prometheus.** Catálogo completo filtrado:
```
curl -s http://localhost:9090/api/v1/label/__name__/values | grep -iE 'eligib|viable|opportun|funding|accepted'
→ arbx_opportunity_total · arbx_pipeline_last_opportunity_insert_unixtime · arbx_sim_funding_total
   (más net_conntrack_listener_conn_accepted_total y node_memory_Unaccepted_bytes, ajenos)
```
**⇒ Ninguna métrica llamada `eligible` ni `viable`. NO COMPUTADO: la métrica no está en el instrumento.** El endpoint de oportunidades del api-server devolvió vacío en el intento ⇒ **tampoco por ahí. NO COMPUTADO, no 0.**

**«Fork gastado»: el instrumento más cercano es el contador de fondeo del `sim-ctl`, y es un CONTADOR, no un presupuesto.**
```
arbx_sim_funding_total  (ts 1791491676.138, loopback via ssh, exit=0)
  slot_unresolved 32 · rpc_err 12 · balance_unreadable 6 · verify_mismatch 140 · cache_hit 7933 · seeded_fresh …
```
**`cache_hit` `4907` (t184) → `7933` ⇒ el contador crece ⇒ el proceso vive** (no se lee una ausencia como cambio de estado; se corrobora). **Pero no existe ninguna métrica de «gastado», así que la parte «gastado» es NO COMPUTADA con su razón.**

**⇒ RESUMEN DEL EFECTO: (a) no se aplicó reparación (`backend/` fuera de alcance), (b) no hubo reinicio (`StartedAt` idéntico a t184), (c) `eligible` no existe como métrica, (d) «gastado» no existe como métrica. CUATRO razones independientes, todas declaradas, ninguna convertida en un cero.** **No se declara mejora de PnL: `net(s*)` es t189 y esta tarea no lo toca.**

---

## 8. LO QUE NO SE TOCA — medido, no prometido

**`trading_config` chain 1, leído:** `capital_usd = 1000.00` · `min_profit_usd = 50.0000` · `simulation_target_profit_usd = 50.0000` · `min_roi_pct = 0.3` · `max_slippage_pct = 0.005` · `flashloan_fee_pct = 0.0009` · `lp_fee_default_pct = 0.003` · `enabled_dex_ids = null`. Las otras 5 cadenas en `capital_usd = 0.00`. **INTACTOS. Cambiarlos es decisión del OPERADOR.**

- **Ninguna `kelly_fraction_bps` fue leída ni aplicada.** Y se declara: **`kelly_fraction_bps` NO es columna de `trading_config`** (el esquema tiene 38 columnas y no está entre ellas).
- **El apetito de riesgo no se toca**, por la razón del OPERADOR: sin edge estadístico que dimensionar, subirlo sobre EV negativo sólo agranda la pérdida.
- **`token_prices_usd` tiene 21 precios**, incluido `WBTC: 81728.8828117184` — el `$81k` que el contrato cita. **WETH: `2469.454228`.**
- **Y un defecto de higiene medido en la allowlist: `allowed_token_symbols` tiene 26 entradas y 21 únicas ⇒ 5 DUPLICADOS** (`WETH`, `DAI`, `WBTC`, `USDC`, `USDT`, cada uno ×2).

---

## 9. CONTROLES — TODOS EJECUTADOS, CON SU EXIT CODE LITERAL

| Control | Resultado |
|---|---|
| Canal `SELECT 1` | `1`, **exit=0** |
| Negativo `SELECT esto_no_existe` | **exit=1** + `ERROR: column "esto_no_existe" does not exist` — **sin tubería** |
| **Control positivo del `LIKE`** | `fail_reason LIKE 'reverted:%'` → **`5632`**, exit=0 (t184: 4007) |
| **El cero con su productor** | `fail_reason = 'reverted'` (forma desnuda) → **`0`**, y es **0 con productor declarado: el prefijo SIEMPRE lleva sufijo `:<motivo>`**. Un cero en la forma desnuda vale, precisamente, porque la forma prefijada da 5632. |
| Fork, bloque fijo | `eth_blockNumber` → `0x18efd78` = **`26148216`**, exit=0, y `cast block-number` → `26148216` |
| `1.808` vs `2.471` | conteo numerado con `count(*) FILTER`, no un `GROUP BY` cuyo vacío pueda ser NULL |
| Columnas inexistentes | `SELECT confirmations FROM opportunities` → **exit=1 + ERROR** · `SELECT route_group_key FROM opportunities` → **exit=1 + ERROR** |
| Ningún control pipeado a `head` | los tres, sin tubería |
| `:9090` externo | **`000`**, exit=7 — **defecto #9** |

**Y la corrección del capitán se corrobora con MI PROPIA medición de esquema, no con su palabra:** `opportunities` tiene **27 columnas** y **ninguna** se llama `confirmations`, `route_group_key`, `quote_block` ni `dex_fees_usd`; `simulations` **no tiene** `economics`. **Cuatro lecturas de `information_schema` a `0` columnas, más dos `SELECT` que mueren con `exit=1`.** **⇒ «la columna no está ahí» ≠ «no hay replay». Aplica la distinción `UNREADABLE` vs `0`.**

---

## 10. AJUSTES PROPIOS, CAZADOS Y DECLARADOS

El estándar que exige «si se caza un ajuste propio, se declara y se corrige». **Cuatro, y el tercero es el que importa:**

1. **`Bad substitution`** — lancé el recon con `sh` y usé `${PIPESTATUS[0]}`, que es de `bash`. La mitad del script (Q5/Q6) no corrió. **Declarado y re-lanzado con `bash`.**
2. **`FileNotFoundError: 'cast'`** — `cast` **no está en el PATH del host**: vive **dentro** del contenedor `arbitragex-v2-anvil-1`. Las llamadas al `QuoterV2` murieron enteras en el primer intento. **Declarado y re-lanzado por `docker exec … cast`.**
3. **★★ MI PRIMERA AUDITORÍA DE `decimals` DIO UN FALSO «TODO OK»: `discrepantes = 0` sobre 38 tokens.** **La población estaba mal elegida: audité los `token_in` de las filas vivas — donde el mapa casualmente acierta — en vez de LAS ENTRADAS DEL `decimals.map`.** Corregida la población (§3), **`discrepantes = 8`**. **Un instrumento que da «todo OK» por elegir la población cómoda es exactamente el modo de fallo que esta campaña caza. Se declara aquí y no se entierra.**
4. **El cero de mi primera búsqueda de los pares testigo** — filtré `pair_symbol ILIKE '%USDT%'` y **`pair_symbol` es PREFIJO HEX, no símbolo** (`c02aaa…/dac17f…`). Devolvió vacío y estuve a un paso de leerlo como «esos pares no existen». **El cero tenía productor: mi filtro.** Re-buscado por par y por dex (§2), los tres casos testigo son el nº1 de su causa.

---

## 11. DEFECTOS DEL CAPITÁN REPRODUCIDOS Y NUEVOS

| # | Defecto | Evidencia |
|---|---|---|
| **#9** | `:9090` externo | `external_9090_http_code=000`, **exit=7** |
| **#15** | `simulations` NO tiene `economics` | `information_schema` → **0 columnas** |
| **#13/#14** | `quote_block` y `gross`/`net` en el JSONB `economics`, no columnas planas | `information_schema` → **0 columnas** |
| **#12** | `fail_reason` prefijado | `LIKE 'reverted:%'` = **5632**; la forma desnuda = **0** |
| — | `confirmations` / `route_group_key` **no existen** en `opportunities` | dos `SELECT` con **exit=1 + ERROR** |
| — | `dex_fees_usd` **no existe** | `information_schema` → 0 |

**Defectos NUEVOS, medidos aquí por primera vez:**
1. **El `decimals.map` del ciclo defaultea a `18`** y **no lee `tokens.decimals` (PG) ni el catálogo de Redis**, que tienen el valor correcto en los 8 discrepantes. **Factor `1e9`–`1e12`.**
2. **`quote_block`/`decimals` al margen: `route_metadata.decimals` tiene UNA sola subclave, `map`** (y `route_metadata` tiene 9 claves en total, todas medidas).
3. **`pools` mezcla V2 y V3 en la misma columna `fee_tier`**: `30` (902 filas) y `2500` (9) **no son tiers V3**. Y **`fee_tier` NULL en 2.471 de 4.279 (57,7 %)**.
4. **`1.386` pools (32,4 %) sin `last_enumerated_at`.**
5. **`v3_pool_not_catalogued` etiqueta rutas cuyos pools SÍ están catalogados (72/72)**: el productor medido es que **21 de 72 no están en el índice V3 de Redis y son exactamente los 21 con `fee_tier = 30`.**
6. **`v3_quote_unavailable` NO corresponde a un fallo del `QuoterV2`: 8 de 8 cotizan.**
7. **`allowed_token_symbols` con 5 duplicados** sobre 26 entradas.
8. **Un `0x0009050f27a9eb231256bafbb901b906ee6cc9cf` en el `decimals.map` sin respuesta on-chain** (`decimals()` revierte) — **SIN CONTEXTO, 1 token, declarado.**

---

## 12. HUECOS ABIERTOS — DECLARADOS, NO CERRADOS

1. **`backend/` fuera de alcance ⇒ CERO reparaciones aplicadas.** Las tres causas siguen vivas con sus conteos de §2. **Lo que se entrega es el diagnóstico con productor y magnitud, no el arreglo.**
2. **El camino interno que emite `v3_quote_unavailable` NO se localizó.** Visto desde fuera, el `QuoterV2` funciona: la causa está en `searcher-rs`.
3. **La aritmética exacta de los `12 sats` de WBTC no se re-derivó** (§3). Causa medida, síntoma citado del OPERADOR, cadena no reproducida.
4. **La causa de los `65` `no_tradable_size` con `amount_in_wei > 0`** no se determinó.
5. **La causa de `WETH/GHST` (`no_tradable_size`, 1.777 en 6 h)**: sus 4 mapas coinciden en `18`, así que **no es el defecto de decimals**; sin causa declarada.
6. **El catálogo V3 se declara en su límite, no se completa**: el índice vive de fuentes externas (alchemy/reactive/subgraph) fuera de este alcance. **No se inventó ningún pool.**
7. **`redis` no tiene entrada para 3 de los 37 tokens del mapa** — ausencia declarada, no un cero.

---

**NO SE INVENTA NINGÚN CANDIDATO NI NINGÚN PnL. NO SE ESTIMA «CUÁNTO SE PODRÍA GANAR».** Un negativo es un resultado: **tres de las cuatro causas no son lo que su nombre dice, y el defecto de `decimals` tiene productor, población y magnitud.**

**Publicación:** `git add` normal · rama `docs/quoter-decimals-01` · PR en **DRAFT** · integridad por `git hash-object` · **nunca `Out-File`**.
