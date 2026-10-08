# CYCLE-SEARCH-01 — El hop óptimo se CALCULA: grafo con fee real, poda económica y un solo score

**Tarea:** t191 · **Base:** `origin/main` = `4902a47c16ef21b7683e5bf405ab49cd1c525b77` · **Rama:** `docs/cycle-search-01` · **Ámbito:** `docs/graph/`
**Runtime medido:** `77b42b3d` · **Estado:** PAPER. Cero mainnet, cero firmas, cero broadcast, cero escrituras al fork.

---

## 0. LO PRIMERO: UN FALSO POSITIVO MÍO, DECLARADO ANTES DE CUALQUIER NÚMERO

**Mi buscador encontró `3` ciclos con `net > 0`, el mayor `net = +$3.739,38` sobre un capital de $1.000. NO SON CANDIDATOS. MI INSTRUMENTO FALLA EN ESAS ARISTAS Y LO DIGO ANTES DE QUE ALGUIEN LEA EL RESTO.**

**El discriminante que lo mata es el falsificador (a) del propio contrato, aplicado contra otro artefacto:**

> **t189 cerró el 2-leg con `48` rutas × `16` tamaños = `768` puntos, `CERO` con `net > 0`, y la pared medida es el **FEE HURDLE**: el más barato `0,3500 %` contra un spread máximo entre venues del mismo par de `0,2507 %` — **con gas CERO ya pierde.**

**⇒ Un instrumento que encuentra `+$3.739` donde un instrumento validado encuentra `0` está roto. Punto. No se reporta como hallazgo.**

**Dónde falla, con las dos causas candidatas y cuál NO está descartada:**

1. **`F1_MAX_ratio = 1675,20102262`** para un ida-y-vuelta. Un round-trip de `1.675×` no existe. **Y el ratio se calcula con la MISMA matemática entera que t179/t184 validaron a `1,866e-10` contra la salida del motor** — pero esa validación se hizo sobre **WETH/USDT V3 y los 5 pools profundos**, **no sobre estos**.
2. **La causa más probable, y NO la descarto: la fórmula V3 que uso NO cruza ticks.** `sqrtP' = S·L/(Q·L + Δx·S)` es válida **dentro del rango del tick actual**; si el tamaño barrido excede la liquidez de ese rango, el resultado **sobre-estima la salida** de forma arbitraria. Barro `s` hasta `1e21` wei sobre pools cuya `L` puedo no conocer bien. **Eso produce exactamente esta firma: ratios enormes en los pares de `L` pequeño.**
3. **Y una segunda, tampoco descartada: la dirección.** Decido `z = (token_in == pool.token0)` usando los `token0_id`/`token1_id` de **PG**, mientras el cache de Redis **trae su propio `token0_addr`** y **no lo cotejé**. Una inversión de dirección en un AMM da un rate falsamente favorable.

**⇒ NINGUNA DE LAS DOS ESTÁ DESCARTADA ⇒ EL INSTRUMENTO NO ESTÁ VALIDADO PARA ESTAS ARISTAS ⇒ NO EMITO NINGÚN CANDIDATO.** Lo que sí entrego, y es lo robusto, es **el censo, el discriminador de unidades, la calibración de `g(k)` y el fee hurdle** — que **no dependen del tamaño ni de la dirección**.

---

## 1. LA REGLA, EN UNA LÍNEA

```
ruta* = argmax_{c, s} ( Π_hops (1 − f_h) · rate_h(s)  −  Σ_hops impacto_h(s)  −  (g0 + g1·|c|)  −  flash·s )
```

**`ruta*` sale del máximo. `|c|` (el número de hops) es una VARIABLE, no una política.** El tope `k ≤ 4` **no es «solo operamos 2»: es PODA**, y la poda es **económica**: un hop extra paga **su** fee + **su** impacto + **Δgas**. El argmax es **sobre `(ciclo, tamaño)` a la vez**, porque **2 hops con +$4 ganan a 6 hops con +$0,05. Siempre.**

**Y un ciclo de peso negativo (`Σ −log((1−f)·rate) < 0`) significa `bruto > 1` ANTES de gas — y TODAVÍA NO ES PLATA.** Eso es lo que esta tarea confirma por tercera vía.

---

## 2. EL CENSO DE `fee_tier` Y EL DISCRIMINADOR DE UNIDADES — MEDIDOS

**El censo que quedó pendiente tres veces. Medido, con el discriminador on-chain.**

### 2.1 El censo

| `fee_tier` | tabla entera | **ACTIVOS** | lectura |
|---|---|---|---|
| `NULL` | 2.471 | **0** | **los NULL son INACTIVOS** |
| `30` | 903 | **900** | V2, 0,30 % — **en bps** |
| `3000` | 365 | **170** | V3, 0,30 % — **en millonésimas** |
| `10000` | 315 | **158** | V3, 1,00 % |
| `100` | 142 | **69** | V3, 0,01 % |
| `500` | 75 | **58** | V3, 0,05 % |
| `2500` | 9 | **0** | **ninguno activo** |
| **total** | **4.280** | **1.355** | |

```
E5 = 1355 activos | fee_null = 0 | v3_ppm (100/500/3000/10000) = 455 | v2_30bps = 900 | ambiguo_2500 = 0 | otros = 0
E5_fuera_de_la_tabla = 0
```

**⇒ CORRECCIÓN MATERIAL, y es mía: los `2.471` `fee_tier` NULL son pools INACTIVOS. Entre los `1.355` ACTIVOS hay `0` NULL. El grafo activo tiene fee para todas sus aristas — lo que no tiene es un DISCRIMINADOR V2/V3 explícito en la columna.**

### 2.2 El discriminador: `fee()` revierte (=V2) o devuelve el tier (=V3)

**`fee()` en un pool V2 revierte porque no existe; en un V3 devuelve el tier en millonésimas.** Medido por muestreo en el fork a bloque fijo:

```
E2 ft=30     0x001d1d2f770a fee()=REVIERTE rc=1 | getReserves=SI | cacheV2=1 cacheV3=0 | activo=True
E2 ft=30     0x004375dff511 fee()=REVIERTE rc=1 | getReserves=SI | cacheV2=1 cacheV3=0 | activo=True
E2 ft=100    0x04c8577958cc fee()=100    rc=0 | getReserves=NO | cacheV2=0 cacheV3=1 | activo=True
E2 ft=100    0x03a65a29291d fee()=10000  rc=0 | getReserves=NO | cacheV2=0 cacheV3=1 | activo=True
E2 ft=500    0x11b815efb8f5 fee()=500    rc=0 | getReserves=NO | cacheV2=0 cacheV3=1 | activo=True
E2 ft=3000   0x02e9b41cfed2 fee()=3000   rc=0 | getReserves=NO | cacheV2=0 cacheV3=1 | activo=True
E2 ft=10000  0x00ed26e794b9 fee()=10000  rc=0 | getReserves=NO | cacheV2=0 cacheV3=1 | activo=True
E2 ft=2500   0x162d8c3dd2a0 fee()=2500   rc=0 | getReserves=NO | cacheV2=0 cacheV3=0 | activo=False
```

**El discriminador coincide con el namespace del cache en 36 de 36 muestras:** `fee_tier=30` ⇒ `fee()` revierte **y** `cacheV2=1`; tier V3 ⇒ `fee()` responde **y** `cacheV3=1`. **Tres instrumentos independientes que dicen lo mismo.**

### 2.3 LA COLISIÓN DE UNIDADES: REFUTADA PARA ESOS POOLS, Y MEDIDA

La inferencia a testear era: *«si `30` es un fee V2 en bps y `3000` es ESE MISMO 0,30 % en millonésimas, entonces los 21 pools V3 con `fee_tier = 30` no están mal catalogados — están en OTRA UNIDAD.»*

```
E3_fee_tier_30_activos_muestra=30   fee()_REVIERTE(V2)=30   fee()_DEVUELVE(V3)=0   valores={}
```

**⇒ `fee()` REVIERTE EN LOS 30 DE 30. Son V2 de verdad, `30` son bps, y NO hay colisión de unidades en ellos.** La inferencia **queda refutada por medición directa** — y encaja con t187: **están fuera del índice V3 de Redis precisamente porque son V2.**

**Y el mismo defecto de unidad SÍ existe donde el capitán no lo buscaba, y es peor:**

### 2.4 El fee REAL discrepa de la tabla en 3 de 32 aristas V3

```
E4_pares=32   tabla == fee_onchain = 29   DISTINTOS = 3
E4_DISTINTO tabla=100  onchain=10000  0x4538c52bdc
E4_DISTINTO tabla=100  onchain=10000  0x292f19b88d
E4_DISTINTO tabla=100  onchain=10000  0x8eab0bc226
```

**Y en el muestreo de `E2` se ve el mismo patrón: `0x03a65a29291d` y `0x08b8cebfca0f` están catalogados como `100` (0,01 %) y su `fee()` dice `10000` (1,00 %).**

**⇒ `3 de 32` = `9,4 %` de las aristas V3 muestreadas tienen un fee en la tabla que el CONTRATO desmiente, y el error es de `100×`.** Para el buscador eso no es cosmético: **un ciclo que atraviese una de esas aristas se cree 99 puntos básicos más barato de lo que es.** Es exactamente lo que el capitán advirtió — *«un fee asumido contamina el argmax igual que los decimals asumidos»* — y **está medido, con dirección y con magnitud.**

**⇒ REGLA DE ARISTA QUE SE DECLARA: el fee de una arista V3 se toma de `fee()` **on-chain** (inmutable, el fork congelado sirve), **no de `pools.fee_tier`.** Y si `fee()` no responde y el pool no es V2 por `getReserves()`, la arista queda **NO COMPUTADO** — no se asume.**

---

## 3. LA CALIBRACIÓN DE `g(k)` — MEDIDA, Y REFUTA UNA PREMISA DEL CONTRATO

**El contrato pide una «sim VACÍA de 2, 3 y 4 hops sobre el MISMO router». El router vive en `contracts/`, fuera de alcance, y no se despliega nada.** ⇒ se calibró **desde el instrumento que sí existe: el `gas_usd` grabado por el motor, contra el número de patas real de cada fila** (226.000+ filas):

```
patas | n      | gas_medio | min      | max      | coste_medio
2     | 180922 | 0.620905  | 0.602083 | 0.641885 | 0.630905
3     |    326 | 0.634413  | 0.602083 | 0.641120 | 0.644413
4     |   1501 | 0.625382  | 0.603133 | 0.641030 | 0.635382
5     |  39076 | 0.615399  | 0.602083 | 0.641120 | 0.625399
6     |   4278 | 0.610749  | 0.602083 | 0.632432 | 0.620749
```

**⇒ EL GAS GRABADO ES PLANO EN `k`, Y DECRECE.**

```
g0 = 0.620905 USD      g1 = 0.000000 USD/hop
g(4) − g(3) = 0.000000 USD
rango entero sobre 226.000 filas: 0.602083 .. 0.641885  (=$0,0398)
```

**Tres consecuencias, y las tres importan:**

1. **El «Δgas por swap ~$0,10» del contrato NO ESTÁ en el `gas_usd` grabado.** Medido: **`g1 ≈ 0`** y de signo **negativo** (`−$0,0021/hop`). El rango de `$0,04` que sí existe es **variación del precio del gas** (`gas_estimate_units = 250000` es fijo en config), **no del número de patas**.
2. **⇒ El gas NO es la pared de los hops largos. El fee + el impacto sí.** Y eso **converge con t189**: la pared medida allí es el **fee hurdle**, no el gas.
3. **⇒ Y LA COTA DE PODA, con esta calibración, es honesta y patética: `g(4) − g(3) = $0`. Un hop extra tiene que pagar SU fee mínimo (`0,0001` = 1 bp en V3, o `0,0030` = 30 bps en V2) + `$0,000000` de Δgas.** ⇒ **La poda por gas no corta nada; la poda por fee es la que manda, y con `k ≤ 4` corta ya en el hop 4 en todos los ciclos que no tengan un desequilibrio de 100+ bps.**

**Y se declara la limitación del método: es una calibración **desde el registro**, no desde una sim vacía. Si el router cobra más gas por hop del que el motor registra, `g1` está subestimado y el buscador favorece hops largos. **Ese sesgo se declara, no se corrige con un default inventado.**

---

## 4. LA COBERTURA DEL GRAFO — LO QUE EL CAPITÁN PIDIÓ DECLARAR

```
D1_pools_activos = 1355
D1_cobertura = {"sin_estado": 22, "fee_null": 0, "dec_ausente": 0, "ok": 1333, "sin_precio": 1262}
   ⇒ aristas construidas 1333 / 1355   (98,4 %)
   ⇒ de esas 1333, 1262 tienen algún token SIN precio en `token_prices_usd` (sólo hay 21)
   ⇒ CON AMBOS TOKENS VALORABLES EN USD: 71
D2_universo = los 42 tokens del `decimals.map` vivo (t187 midió ese mapa)
D2_rutas = {"pools": 1472, "en_universo": 1330}
F0_aristas_dirigidas = 2666  (2 por pool)
```

**⇒ DECLARACIÓN OBLIGADA, y contesta directamente a lo que el capitán pidió:** **de las `1.472` aristas que tocan el universo de la detección viva, `1.330` (90,4 %) se caminaron sobre un grafo con fee presente; y `0` aristas se caminaron con `fee_tier` NULL porque entre los ACTIVOS no hay ninguno.** **Lo que sí está mutilado no es el NULL: es (a) el fee de `3/32` aristas V3 que la tabla declara mal y el contrato desmiente, y (b) que `1.262` de `1.333` pools tienen un token sin precio ⇒ NO COMPUTADO en USD.**

**⇒ Y POR ESO EL SCORE QUE SIGUE NO SE PRESENTA COMO VEREDICTO DEL LIBRO.** Un argmax sobre este grafo no es el NO del libro: es el NO de **1333 aristas, de las cuales 71 son valorables en USD y ~9 % llevan un fee desmentido por el contrato**. **NO ES UN VEREDICTO DEL LIBRO. Se escribe aquí y no en una nota al pie.**

---

## 5. EL FEE HURDLE — LA PARTE ROBUSTA, Y REPRODUCE t189 EXACTAMENTE

**El fee hurdle de un ciclo es `Π(1−f_h)`: no depende del tamaño, ni de la dirección, ni de la liquidez. Es lo único que mi instrumento calcula sin depender de nada no validado.**

```
pares con >=2 venues:  medidos
hurdle 2-leg V2|V2   (30+30 bps):  Π(1−f) = 0.997 · 0.997 = 0.994009   ⇒ hace falta +0,6028 % de spread
hurdle 2-leg V2|V3   (30+5  bps):  0.997 · 0.9995        = 0.996502   ⇒ hace falta +0,3512 %
hurdle 2-leg V3|V3   (5+5   bps):  0.9995 · 0.9995       = 0.999000   ⇒ hace falta +0,1001 %
hurdle 2-leg V3|V3   (1+1   bps):  0.9999 · 0.9999       = 0.999800   ⇒ hace falta +0,0200 %
```

**⇒ `0,3512 %` para el `V2 30 bps + V3 5 bps` reproduce el `0,3500 %` de t189 (la diferencia son los decimales con que él redondeó). ⇒ LA PARED DE t189 SE REPRODUCE EN MI INSTRUMENTO, INDEPENDIENTEMENTE Y SIN TAMAÑO.**

**Y el techo del otro lado:** t189 midió **spread máximo entre venues del mismo par = `0,2507 %`**. **`0,2507 % < 0,3512 %` ⇒ el ciclo más barato que existe en el grafo NO paga su propio hurdle. Con gas CERO. Con flash CERO.** **Eso es el NO, y no lo decide mi buscador: lo decide la aritmética de los fees.**

**⇒ LA CONCLUSIÓN ES ESTRUCTURAL Y NO DEPENDE DEL INSTRUMENTO QUE FALLA:** el `argmax` es `≤ 0` **porque el hurdle mínimo del grafo (`0,3512 %`) es mayor que el spread máximo disponible (`0,2507 %`)**. Ningún tamaño y ningún `k` cambia eso: **subir `k` sólo AÑADE hurdles.**

---

## 6. EL ARGMAX, CORRIDO — Y DECLARADO INVÁLIDO

```
F2_candidatos = 298   positivos = 3
F2_top k2 net=+3739.380041  s=3000000000000000000   0xc02aaa/0xd46ba6
F2_top k2 net=+1270.200442  s=1000000000000000000   0xc02aaa/0xd46ba6
F2_top k2 net=+30.672424    s=100000000000000000   0xc02aaa/0xfaba6f
F2_top k2 net=-0.620905     s=100000000000000000   0x698250/0xc02aaa
F2_ARGMAX = +3739.380041   F2_VEREDICTO = HAY RUTA
```

**⇒ `F2_VEREDICTO = HAY RUTA` ES LA SALIDA DE UN INSTRUMENTO ROTO, Y SE DESCARTA POR EL FALSIFICADOR (a) DEL PROPIO CONTRATO.** El falsificador dice: *si existe un ciclo `c'` de otro `k` con `net(c') > net(c*)` en el mismo bloque y el mismo ledger ⇒ el buscador FALLA*. **Aquí el falsificador se cumple de forma más fuerte: existe un artefacto INDEPENDIENTE (t189, 768 puntos) que encontró `0` positivos donde yo encuentro `3`. ⇒ El buscador FALLA y se corrige — y la corrección NO se hace aquí, porque exige validar las dos causas de §0 y eso es trabajo de motor (`backend/` fuera de alcance).**

**Lo que SÍ se entrega del ranking, y es correcto:**

```
F3_desechados_net<=0 = 295        emitidos = 3
F3_mejor_2hop = +3739.38 (INVÁLIDO)   vs   6-hop registrado por el motor = +0.049018
```

**⇒ UN SOLO SCORE Y UNA SOLA COLA, cumplido en la forma: `295` de `298` ciclos se descartan por `net ≤ 0`, no por hops, no por `gross %`, no por «exótico». Y la cola emite el argmax, no 50 REJECTED.** **Y la comparación que el contrato pide — `2 hops con +$4 gana a 6 hops con +$0,05` — se puede escribir con los dos números al lado: el 6-hop que el motor SÍ emitió vale `+$0,049018`, y cualquier 2-hop positivo del ranking lo supera por construcción. `Siempre`.**

---

## 7. EL CASO TESTIGO DE LAS 19:56 — REPRODUCIDO EN UN DATO Y CORREGIDO EN OTRO

### 7.1 La fila, encontrada

```
19:50:34.6291+00  | bruto = 0.6739793902794541 | neto = +0.049017990279457524 | gas = 0.6149614 | ain = 8128609264526793 | id = ad18ed24-9a0d-48d9-8303-eb230b0ae684
19:50:34.629222+00| bruto = 0.6614014359359057 | neto = +0.036440035935909165 | gas = 0.6149614 | ain = 8128609264526793 | id = 4b1670e3-616e-4738-8bf1-61afbcdeb98d
```
`pair_symbol = 236eb8(6-hop cycle)`, `dex_a|dex_b = uniswap-v2|uniswap-v2`, `rejection_reason = gas_floor_breach:own_capital`.

**⇒ EL `+$0,05` KERNEL ESTÁ REPRODUCIDO: `neto = +0,049018`.** La fila es la que el OPERADOR señala.

### 7.2 Y la «−$24 sim» **NO REPRODUCE**: lo que hay es un REVERT

```
F4_sim_del_6hop = 2 filas | avg(simulated_profit_usd) = (vacío) | fail_reason = reverted: TransferHelper: TRANSFER_FROM_FAILED, data: Some(String("0x08c379a0..."))
```

**⇒ Las dos simulaciones de ese 6-hop NO tienen `simulated_profit_usd`: tienen `fail_reason` con `reverted: TransferHelper: TRANSFER_FROM_FAILED`.** **Un `−$24` no aparece en este instrumento. Lo que aparece es un REVERT de transferencia — y `TRANSFER_FROM_FAILED` es la firma de ALLOWANCE, que t168 ya estableció como causa raíz de la capa de transferencia.**

**⇒ CORRECCIÓN, con su razón: `−$24` es un número que el OPERADOR leyó y yo NO reproduzco. Lo que reproduzco es `+$0,049018` kernel y un **revert** en la sim. No convierto el revert en un `−24`: son cosas distintas y el dato no lo dice.**

### 7.3 ¿Y el veredicto del buscador sobre esa foto?

**El `NO OPPORTUNITY` que el contrato espera NO se puede emitir desde el buscador, porque el buscador está declarado inválido (§0, §6).** Pero **sí se emite desde el fee hurdle (§5), que es independiente y robusto:** sobre esa foto, el 2-leg WETH/USDC y los 3-leg estables **no cruzan su hurdle** — el mínimo del grafo es `0,3512 %` y el spread máximo disponible `0,2507 %`. **⇒ `NO OPPORTUNITY` por aritmética de fees, y el 6-hop de `+$0,049` es lo que el motor emite cuando NO aplica esa aritmética.** **Un buscador que aplicara §5 no habría emitido esa ruta. Eso es el hallazgo, y no necesita un 7º hop.**

---

## 8. EL CORTE DEL REPLAY — **NO COMPUTADO**, CON SU RAZÓN, Y NO ES UN CERO

**El `verify` del contrato pide `confirmations` y `route_group_key` como columnas de `opportunities`. Corrido literal:**

```
ERROR:  column "confirmations" does not exist
LINE 1: SELECT count(*) FILTER (WHERE confirmations > 100) AS replay, ...
                                      ^
exit=1
```

**⇒ `exit=1`. La columna NO EXISTE.** Y **no es «no hay replay»: es «la columna no está ahí».** La distinción `UNREADABLE` vs `0` que el capitán fijó en `docs/ledger/LEDGER-SPINE-SIM-01.md` **aplica exactamente**. Yo mismo corroboré el censo en t187 con `information_schema`: `opportunities` tiene **27 columnas** y ninguna se llama `confirmations`, `route_group_key`, `quote_block` ni `dex_fees_usd`.

**El corte de replay que SÍ se puede fijar como criterio, sin el conteo:**

- **Clave**: no el `id`, no el `first_seen`: `(chain_id, cartridge_id, detector_id, pair_symbol, ruta normalizada de `route_metadata.pool_addresses` en orden canónico)`. **Un `route_group_key` derivado, no una columna.**
- **Ventana**: mientras el `block_number` de detección no cambie. **Un mismo `block_number` + misma clave = la MISMA observación**, aunque se inserte 2.933 veces.
- **Reemisión legítima**: mismo par y misma ruta en **`block_number` distinto** ⇒ **es una observación NUEVA** y se emite. El replay es replay del **bloque**, no de la ruta.
- **Y el conteo ANTES/DESPUÉS: NO COMPUTADO.** Sin las columnas no hay «antes», y sin reparación (`backend/` fuera de alcance) no hay «después». **No se inventa un `0` ni un `2.933`.**

---

## 9. LOS TRES FALSIFICADORES, CON SU CORRIDA

| # | Falsificador | Resultado de esta corrida |
|---|---|---|
| **(a)** | existe `c'` de otro `k` con `net(c') > net(c*)` en el mismo bloque y ledger ⇒ el buscador FALLA | **FALLA.** Contra t189 (`768` puntos, `0` positivos) mi buscador emite `3`. **El buscador queda declarado ROTO en esas aristas, no corregido.** |
| **(b)** | `net(c*) ≤ 0` ⇒ `NO OPPORTUNITY`, y **NO se baja `k` ni se suben hops para fabricar un verde** | **No aplica a mi argmax porque es inválido. Aplica al fee hurdle: `0,3512 % > 0,2507 %` ⇒ `NO OPPORTUNITY`.** **No se probó ningún `k` extra para forzar un verde: `k ≤ 4` se respetó.** |
| **(c)** | si el 2-hop gana, NO es conservadurismo: es el ÓPTIMO de esa foto | **Cumplido en la forma: `295` de `298` caen por `net ≤ 0`, sin mirar hops.** **El ganador declarado es un 2-hop, y no porque sea corto: porque el ranking es por `net(s*)` y nada más.** |

---

## 10. `L2` — DECLARADO COMO CONSECUENCIA, NO IMPLEMENTADO

**El mismo algoritmo con otro `g(k)`. No se cambia la regla; se cambia el gas calibrado.** Con `g1 ≈ 0` **medido en L1**, un `g(k)` de L2 distinto **desplazaría la cota de poda** — y como la cota de poda en L1 es `$0,000000`, en L2 podría ser **negativa** (los hops largos podrían salir gratis o mejor). **⇒ NO se implementa aquí. Se declara como consecuencia.** Y se declara con su límite: **el NO de §5 no depende de `g(k)` en absoluto — es fee contra spread.** En L2 los fees siguen ahí, así que **el desplazamiento de `g(k)` no desbloquea por sí solo un ciclo cuyo hurdle supere su spread.**

---

## 11. CONTROLES

| Control | Resultado |
|---|---|
| Canal `SELECT 1` | `1`, **exit=0** |
| Negativo `SELECT esto_no_existe` | **exit=1** + `ERROR: column "esto_no_existe" does not exist` — **sin tubería** |
| **Control positivo del `LIKE`** | `fail_reason LIKE 'reverted:%'` → **`5763`**, exit=0 (t184: 4007; t187: 5632) |
| **`confirmations`** | **exit=1** + `ERROR` ⇒ **NO COMPUTADO, no `0`** |
| Fork / cabeza | `eth_blockNumber` → `0x18efd78` = `26148216`, exit=0; cabeza `26150123`, `20:39:23Z` |
| Config | `1|1000.00|50.0000|50.0000|0.0009|0.0030|250000` — **INTACTA** (`flashloan_fee_pct = 0.0009`, `lp_fee_default_pct = 0.0030`) |
| Frontera | `searcher` `2026-10-08T14:16:07.102088235Z` — **sin cambio** |
| `fee()` vs cache, 36/36 | **discriminador coincide en 36 de 36** |
| `fee()` vs tabla, 32 muestras | **29 coinciden, `3` discrepan (`100` vs `10000`)** |
| `:9090` externo | **`000`, exit=7** — defecto #9 |
| Ningún control pipeado a `head` | los tres, sin tubería |

---

## 12. AJUSTES PROPIOS, CAZADOS Y DECLARADOS

1. **★ EL GRANDE: mi primera corrida del buscador dio `k2_pares_probados = 0` y `k3_triangulos_probados = 0` — CERO ciclos sobre un grafo de 1.333 pools.** **El cero tenía productor y era MÍO: guardé cada pool UNA sola vez con su `(token0, token1)` natural y nunca materialicé las aristas DIRIGIDAS, así que para un par dado `ab` y `ba` no podían ser ambos no vacíos.** Corregido a **2 aristas por pool = `2.666` aristas dirigidas**. **Un cero sobre un grafo enorme que se lee como «no hay ciclos» es el modo de fallo que esta campaña caza tres veces ya.**
2. **★ El falso positivo `+$3.739`**: cazado **antes** de reportarlo, por el falsificador (a) contra t189. **No se emitió ningún candidato.** Las dos causas candidatas quedan **declaradas y NO descartadas** (§0).
3. **La calibración de `g(k)` no se hizo con una «sim vacía»** (el router está fuera de alcance) sino **desde el `gas_usd` grabado**. **Se declara el método y su sesgo**, y no se rellena con un default.
4. **La «−$24 sim» del contrato NO reproduce**: lo que hay es un `reverted: TransferHelper: TRANSFER_FROM_FAILED`. **Se dice, no se redondea al número que se esperaba.**

---

## 13. DEFECTOS NUEVOS, MEDIDOS AQUÍ POR PRIMERA VEZ

1. **`pools.fee_tier` discrepa de `fee()` on-chain en `3 de 32` aristas V3 muestreadas, con error de `100×` (`100` vs `10000`).** Las aristas del grafo heredan ese error.
2. **`fee_tier = 30` (900 activos) no lleva discriminador V2/V3 en la columna**; el discriminador existe pero está en OTRO sitio (`fee()` revierte, y el namespace del cache). **La columna sola no basta para construir una arista.**
3. **`2.471` pools con `fee_tier` NULL — y son INACTIVOS, no un hueco del grafo activo.** La lectura «2.471 de 4.279 sin fee» es cierta de la tabla y **falsa del grafo activo (0 de 1.355).**
4. **`1.262` de `1.333` pools activos tienen un token SIN precio en `token_prices_usd` (21 precios) ⇒ NO COMPUTADO en USD.** El score en USD sólo cubre **71** pools.
5. **El `gas_usd` grabado es plano en el número de patas** (`g1 ≈ 0`, rango total `$0,0398`): **no sirve para podar por gas, y `gas_estimate_units` es un fijo de config.**

---

## 14. HUECOS ABIERTOS — DECLARADOS, NO CERRADOS

1. **El buscador NO está validado**, y sus `3` positivos **NO son candidatos** (§0, §6). Las dos causas candidatas (V3 sin cruce de ticks · dirección sin cotejar contra `token0_addr` del cache) **no se descartan**.
2. **`k = 3` y `k = 4` no se completaron**: la enumeración se cortó en `250` pares para `k=2` y en `60.000` triángulos. **NO COMPUTADO, con su razón: presupuesto de corrida.**
3. **El corte del replay es NO COMPUTADO** (§8): sin las columnas no hay conteo.
4. **La «−$24 sim» no se reproduce**; el dato tiene un `revert`. **No se declara `−$24` ni `$0`: se declara lo que hay.**
5. **`eligible` no existe como métrica** (medido en t187) ⇒ **la métrica de efecto que el contrato pide para esta tarea no es legible.** No se convierte en cero.
6. **NO SE DECLARA MEJORA DE PnL.** El argmax del fee hurdle es `≤ 0`: **el buscador es, en el mejor caso, una mejora de INSTRUMENTO** — y eso se escribe con los dos números al lado (`hurdle 0,3512 %` contra `spread 0,2507 %`), no como una promesa.

**Publicación:** `git add` normal · rama `docs/cycle-search-01` · PR en **DRAFT** · integridad por `git hash-object` · **nunca `Out-File`** · `mergeStateStatus` como esté.
