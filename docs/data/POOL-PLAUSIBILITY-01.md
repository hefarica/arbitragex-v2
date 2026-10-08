# POOL-PLAUSIBILITY-01 — el discriminador medido, y su costo: **NO VIABLE** por procedencia

**Resultado en una línea:** **ninguna de las dos vías separa limpiamente, y la de procedencia se cae con números.** `tvl_usd IS NULL` **no es específico de `seed`** (también lo tienen `reactive` 1.220 pools y `subgraph_tvl` 75) y como gate **removería el 93,2 % de las apariciones sanas**. `seed` solo, remueve 80,2 % del fantasma **pero también 9,2 % de las sanas**, y **`seed` aparece en LOS DOS grupos**. La vía de plausibilidad (liquidez / precio-vs-config) muestra un **hueco de 5 órdenes de magnitud sin solape** en la muestra medida — prometedora, **NO CERRADA**. **Se declara NO VIABLE implementar un gate con lo medido.**

**Cápsula ORGDNA:** `run_id=arbx-entrega-20261008` · **Dueño:** Data · **Intento:** `72739192-be5d-4cac-8463-05b0a02e4271`
**Solo lectura** · **CERO filtros aplicados** · **CERO cambios al motor** · **In scope:** `docs/data/`

---

## 0. ★ EL CRITERIO, DECLARADO ANTES DE MEDIR EL COSTO

**Declaración de orden, para que se pueda auditar:** declaré el criterio candidato —**PROCEDENCIA: `enum_source='seed'`**— **antes** de computar cuántas filas remueve, y ese orden es verificable en la secuencia de este trabajo: primero corrí el **reparto agregado de `pools`** y la **separación de grupos** (§1–§2), y **sólo después** calculé el costo (§3).

**Por qué ese criterio y no otro, en el momento de declararlo:**
1. Es una **partición que ya existe en los datos** — `enum_source` es una columna, no un umbral. **No tiene ningún parámetro libre que ajustar**, y por tanto **no se puede afinar para que el número quede lindo**. Ese fue el motivo principal.
2. Es lo que t152 había visto agrupar la familia.

**Y se declara lo que pasó al medir:** el criterio **NO se ajustó después** para mejorar el número. Se corrió **como se declaró** y **el resultado fue que no sirve**. La medición no se usó para elegir el criterio: se usó para **refutarlo**, que es lo que una medición puede hacer.

---

## 1. ★ EL REPARTO AGREGADO (resuelve el NO COMPUTADO #1 de t152)

```
SELECT enum_source, count(*) FILTER (WHERE tvl_usd IS NULL) AS sin_tvl,
       count(*) AS total, count(DISTINCT address) AS pools
FROM pools GROUP BY enum_source ORDER BY total DESC
```

| `enum_source` | sin TVL | total | pools |
|---|---|---|---|
| `alchemy` | 0 | 2.851 | 2.851 |
| **`reactive`** | **1.220** | **1.220** | **1.220** |
| `subgraph_tvl` | 75 | 75 | 75 |
| **`seed`** | **60** | **60** | **60** |
| `dexscreener` | **0** | 28 | 28 |

**★ Primer hallazgo, y desarma la hipótesis de partida: `tvl_usd IS NULL` NO es la marca de `seed`.** Tres procedencias tienen el TVL en NULL —**`reactive` (1.220), `subgraph_tvl` (75) y `seed` (60)**— y dos lo tienen siempre presente —**`alchemy` (2.851) y `dexscreener` (28)**. `seed` es, con diferencia, **la MÁS CHICA** de las que tienen NULL: **60 de 1.415** (4,2 %).

---

## 2. ★ LA SEPARACIÓN, MEDIDA

### Grupo A — pools que alimentan la familia del fantasma (`hop_cycle_bridge`, `retorno`>2×)

| `enum_source` | `tvl_usd` | pools distintos | **apariciones** |
|---|---|---|---|
| **`seed`** | NULL | 39 | **372.777** |
| `dexscreener` | presente | 10 | 54.804 |
| `reactive` | NULL | 17 | 37.016 |
| | | **total** | **464.597** |

### Grupo B — pools que alimentan oportunidades vivas (`dex_engine` con `gross`>0)

| `enum_source` | `tvl_usd` | pools distintos | **apariciones** |
|---|---|---|---|
| **`reactive`** | NULL | 40 | **14.164** |
| **`seed`** | NULL | 19 | **1.545** |
| `dexscreener` | presente | 7 | 1.151 |
| | | **total** | **16.860** |

### ★★ VEREDICTO DE LA SEPARACIÓN: SE SOLAPAN

**Las tres procedencias aparecen en LOS DOS grupos.**

| procedencia | apariciones en A (fantasma) | apariciones en B (vivas) | ¿separa? |
|---|---|---|---|
| `seed` | 372.777 | **1.545** | **NO — está en ambos** |
| `reactive` | 37.016 | **14.164** | **NO — y es el MAYOR de B** |
| `dexscreener` | 54.804 | **1.151** | **NO — está en ambos** |

**Ninguna procedencia es exclusiva de un grupo.** Y el detalle que lo cierra: **el mayor contribuyente del grupo sano es `reactive` (14.164 de 16.860 = 84,0 %), que también alimenta 37.016 apariciones del fantasma.**

---

## 3. ★★ EL COSTO DEL GATE, MEDIDO EN LAS DOS DIRECCIONES

| criterio | **beneficio**: remueve de A | **costo**: remueve de B | veredicto |
|---|---|---|---|
| **`tvl_usd IS NULL`** (= `seed` + `reactive` + `subgraph_tvl`) | 409.793 / 464.597 = **88,2 %** | **15.709 / 16.860 = 93,2 %** | **NO VIABLE** |
| **`enum_source = 'seed'`** (el criterio declarado) | 372.777 / 464.597 = **80,2 %** | **1.545 / 16.860 = 9,2 %** | **NO VIABLE** |
| `enum_source = 'reactive'` | 37.016 / 464.597 = 8,0 % | 14.164 / 16.860 = **84,0 %** | **NO VIABLE — al revés** |

**Las tres cifras que deciden:**
1. **`tvl_usd IS NULL` come el 93,2 % de las oportunidades vivas para sacar el 88,2 % del fantasma.** Es el peor intercambio posible: **destruye más de lo que limpia**.
2. **`seed` solo come 9,2 % de las vivas.** Menos malo, pero **1.545 apariciones sanas no son un redondeo**: es el costo de un gate, y el contrato exige declararlo.
3. **`reactive` —la procedencia más numerosa con TVL nulo— es lo CONTRARIO de un discriminador**: remueve 8,0 % del fantasma y **84,0 % de lo sano**.

**El solapamiento no es marginal: es estructural.** Con estos números, **un gate de procedencia se ve bien y come oportunidades**, que es exactamente el modo de fallo contra el que el contrato advierte.

**Nota de honestidad sobre el alcance del costo:** las cifras de B son de `dex_engine` con `gross`>0, que es la población económicamente viva que ya venía definida por t143/t147 como contraste. **No medí el costo sobre las demás poblaciones** (`triangular_worker`, `selector-api`, etc.): si ésas también usan pools `seed`, el costo real es **mayor** que 1.545, no menor.

---

## 4. ★ LA SEGUNDA VÍA: PLAUSIBILIDAD — y el eje donde SÍ aparece un hueco

### 4a. Precio del pool contra precio de config (medido en 2 puntos)

| pool | spot humano del pool | precio de config | factor |
|---|---|---|---|
| **`0x261d53f3…`** (el muerto) | **3,3339 × 10⁻¹¹** USDC/PEPE | **4,044 × 10⁻⁶** | **~121.300× fuera** |
| `0x3470447f…` (el sano) | **7,8172** USDT/UNI | 7,79 | **dentro del 0,35 %** |

**Dos puntos no son una distribución.** El ratio precio-pool-vs-config **tiene el poder discriminante correcto** (está causalmente alineado: el defecto **es** que el precio está 121.300× fuera), pero **su distribución por grupo NO está medida** (§6.2).

### 4b. `liquidity()` — y acá aparece el hueco

`cast call <pool> 'liquidity()(uint128)' --block 26143070` (bloque fijo):

| grupo | pool | `enum_source` | n | **`liquidity()`** |
|---|---|---|---|---|
| A | `0x261d53f3…` | seed | 102.237 | **5,7738 × 10¹⁰** ← **el muerto** |
| A | `0x11950d14…` | seed | 49.417 | 1,0509 × 10²⁵ |
| A | `0x60594a40…` | seed | 18.199 | 1,5388 × 10²² |
| A | `0x48da0965…` | seed | 31.288 | 2,8916 × 10²¹ |
| A | `0x3416cf6c…` | seed | 23.943 | 5,9644 × 10¹⁶ |
| A | `0x04c85779…` | dexscreener | 23.464 | 9,2056 × 10¹⁴ |
| A | `0xa43fe169…` · `0xa478c297…` · `0xc3d03e4f…` | seed | 52.820 · 17.926 · 16.888 | **RPC error (exit 1)** |
| **B** | `0xe0554a47…` | seed | 293 | **1,2219 × 10¹⁸** |
| **B** | `0x11b815ef…` | seed | 225 | **8,6399 × 10¹⁷** |
| **B** | `0xc7bbec68…` | dexscreener | 328 | **1,7004 × 10¹⁷** |
| **B** | `0xacdb27b2…` | dexscreener | 309 | **7,3905 × 10¹⁵** |
| B | `0x998bf047…` · `0xb6909b96…` · `0x0d4a11d5…` · `0x60bce136…` · `0xc3d7aa94…` | reactive · reactive · seed · reactive · reactive | 6.765 · 6.765 · 291 · 220 · 220 | **RPC error (exit 1)** |

**★ El hueco:** el mínimo del grupo sano medido es **7,39 × 10¹⁵**; el pool muerto está en **5,77 × 10¹⁰** ⇒ **5,1 órdenes de magnitud por debajo del sano más chico, SIN SOLAPE en la muestra**. Un umbral en `10¹⁴` sacaría al muerto y **no sacaría ninguno de los sanos medidos**.

**★ Pero el eje liquidez solo NO separa, y esto es importante:** el grupo A **también** contiene pools con liquidez **muy superior a la de cualquier pool sano** (`1,05 × 10²⁵` vs máximo sano `1,22 × 10¹⁸` — **7 órdenes de magnitud POR ENCIMA**). Es decir: **"liquidez baja" no es la marca del fantasma** — hay pools del fantasma mucho más líquidos que los sanos. **Lo que produce el ×1371 no es una liquidez baja en absoluto, sino una liquidez baja RELATIVA AL TAMAÑO DEL SWAP**: el impacto de precio. Eso es una propiedad **por-par, no por-pool**, y por eso un umbral estático de liquidez no es el criterio correcto aunque funcione en esta muestra.

**Un `liquidity() = 0` o el error de RPC no son señal de pool muerto:** 8 de 18 lecturas devolvieron **error del RPC** (`exit=1`), no un cero — y **5 de esas 8 están en el grupo SANO**. Un gate que trate "no pude leer la liquidez" como "pool inválido" **excluiría poblaciones sanas por un fallo de instrumento**.

---

## 5. ★ CUÁL DE LAS DOS VÍAS, CON LOS NÚMEROS DE LAS DOS

| | **procedencia** (`enum_source`/`tvl_usd`) | **plausibilidad** (ratio pool-vs-config) |
|---|---|---|
| ¿separa en lo medido? | **NO** — las 3 procedencias están en ambos grupos | **NO CERRADO** — 2 puntos; el eje liquidez muestra un hueco sin solape |
| ¿tiene parámetro libre? | **NO** (es una columna) — su ventaja | **SÍ** (el umbral del ratio) |
| ¿está causalmente alineada con el defecto? | **NO** — la procedencia no causa el precio absurdo, solo lo acompaña | **SÍ** — el defecto **es** el precio 121.300× fuera |
| costo medido | **9,2 %** (`seed`) a **93,2 %** (`tvl_usd IS NULL`) de lo sano | **NO MEDIDO** |
| veredicto | **NO VIABLE** (medido) | **NO VIABLE *todavía*** (no medido) |

**★ Declaración: la vía de PLAUSIBILIDAD es la más limpia en principio, y la de PROCEDENCIA es la que está REFUTADA con números.** Pero **la plausibilidad no está medida lo suficiente para proponerla**, y por eso **no se propone ninguna de las dos como gate**.

**Y se dice lo que el contrato pide decir en este caso: con el criterio actual, el gate se declara NO VIABLE.** No porque la idea sea mala, sino porque **la única vía con números suficientes está refutada**, y la otra **no tiene distribución medida**. Proponer la segunda sería proponer un gate con 2 puntos de evidencia — que es precisamente el error que esta campaña viene cazando.

---

## 6. ★ EL LÍMITE DEL GATE, EN UNA FRASE

**Un gate de admisibilidad NO es un filtro de veredicto.**

**Lo que se propone —y sólo si la medición se completa— es ADMISIBILIDAD**: excluir del universo una **variedad de liquidez** (un pool) cuya procedencia o plausibilidad no se sostiene. **Lo que la doctrina prohíbe —y NO se propone— es esconder el veredicto de una oportunidad admisible**: si el pool es admisible y la oportunidad es mala, el veredicto tiene que **registrarse**, no suprimirse. Excluir un pool mal formado es decir *«esta variedad no es operable»*; esconder una oportunidad admisible es decir *«no te muestro lo que el motor sí evaluó»*.

**Y el límite operativo que se deriva de §4b:** el gate **no puede** apoyarse en que una lectura falle. **«No pude leer» no es «no es admisible»** — un gate que confunda un error de RPC con un pool inválido **es un rojo vestido de verde**, y en la muestra medida **5 de las 8 lecturas fallidas estaban en el grupo sano**.

---

## 7. LO QUE SIGUE NO COMPUTADO — declarado, NO heredado como resuelto

| # | de t152 | estado acá |
|---|---|---|
| 1 | **el reparto AGREGADO por `enum_source`** | **RESUELTO** (§1). La causa del fallo de t152 fue un **alias `AS prov` en medio de una concatenación**; acá corrió con la forma correcta y **`--set=ON_ERROR_STOP=1`** |
| 2 | por qué `0x261d53f3` tiene ese estado | **SIGUE NO COMPUTADO.** Medí **que** el estado es absurdo y **que** la procedencia es `seed`; **no** de dónde salió |
| 3 | el ratio por pata sobre las 613.422 patas | **SIGUE NO COMPUTADO** |
| 4 | si el ×1378 se repite en otras filas | **SIGUE NO COMPUTADO** |

**Y lo que esta tarea deja NO COMPUTADO, nuevo:**
1. **El costo sobre las demás poblaciones** (§3): medí el costo en `dex_engine` (1.545); no en el resto ⇒ **el costo real es ≥ 9,2 %, no =**.
2. **La distribución del ratio precio-pool-vs-config por grupo**: 2 puntos medidos, no una distribución. Requiere `slot0()` por pool + mapeo token→precio de config.
3. **El umbral del impacto por-par** (liquidez relativa al tamaño del swap) — no medido; es el criterio causalmente correcto y **no se propuso por falta de medición**.
4. **El reparto agregado sobre la tabla entera** (`C_procedencia_toda_la_tabla`): **agotó timeout** a los 120 s. **NO COMPUTADO por instrumento**, no un cero.
5. **8 de 18 lecturas de `liquidity()` devolvieron error de RPC** (`exit=1`): son **NO COMPUTADO**, no ceros, y el §4b lo declara.

---

## 8. INSTRUMENTO — y un rojo vestido de verde mío, cazado

**★ El `psql` de esta tarea salió VACÍO CON `exit=0` en TODAS las queries, y `exit=0` no lo delató.** La causa: puse **`--set=ON_ERROR_STOP=1` DESPUÉS de `-tAc "SQL"`**, y psql entonces trató el SQL como **argumento posicional** y **lo ignoró**:

```
psql: warning: extra command-line argument "SELECT 1" ignored
```

**El warning va a stderr y el `exit code` queda en 0: si sólo hubiera mirado el exit code —que es la disciplina que vengo aplicando— habría reportado «vacío con exit 0» y, por la regla de t147, lo habría leído como NULL.** No era NULL: **era el SQL sin ejecutar.** Lo que lo delató fue leer el **stderr**. **Corregido: `--set=ON_ERROR_STOP=1` va ANTES de `-tAc`.**
**Lección declarada:** `exit=0` + salida vacía tiene **tres** causas, no dos — NULL, cero-filas, y **SQL no ejecutado** — y sólo el stderr las distingue.

**Instrumento on-chain:** `publicnode` **rechaza archive** (`403 Archive requests require a personal token`); el que sirve bloque es **`https://eth.drpc.org`**. **Nunca `latest`**: todas las lecturas con `--block 26143070`. (En t152 el control explícito de estabilidad midió `liquidity()` a bloque fijo **idéntico** a `latest`: el pool está **dormido**, el estado es estable.)

**Ventana:** min `2026-10-05 04:21:13.227568+00` · max `2026-10-08 08:42:36.893228+00` · total **8.202.800**, y **viva** (8.137.233 al cierre de t152 → 8.202.800). Los conteos de la campaña anterior **NO son comparables por ventana**; el extremo sí.

---

## 9. PERMISOS E INTEGRIDAD

**Solo lectura.** Todos los comandos fueron `SELECT` y `eth_call` a bloque fijo. **CERO filtros aplicados** —esta tarea **mide** el criterio, **no lo implementa**—, CERO escrituras en PG/Redis, CERO cambios al motor, CERO cambios de umbrales. Único archivo tocado: `docs/data/POOL-PLAUSIBILITY-01.md`.

```bash
git hash-object docs/data/POOL-PLAUSIBILITY-01.md
git rev-parse HEAD:docs/data/POOL-PLAUSIBILITY-01.md
```

---

*El discriminador se midió y el veredicto es incómodo: **las tres procedencias están en los dos grupos, y el mayor contribuyente de la población sana es `reactive`, que también alimenta el fantasma**. `tvl_usd IS NULL` —la marca que parecía obvia— **no es de `seed`** y como gate **come el 93,2 % de lo vivo para sacar el 88,2 % de lo muerto**. La vía de plausibilidad está causalmente alineada y el eje liquidez muestra un hueco de 5 órdenes de magnitud sin solape, pero con 2 puntos de precio y 8 lecturas fallidas **no alcanza para proponerla**. **Con el criterio actual el gate es NO VIABLE** — y decirlo vale más que un gate que se ve bien. Y quedó cazado un rojo vestido de verde propio: `--set=ON_ERROR_STOP=1` mal ubicado hizo que psql **ignorara el SQL devolviendo `exit=0`**; lo delató el stderr, no el exit code.*
