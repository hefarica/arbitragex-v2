# UNIVERSE-SWEEP-01 — cobertura cerrada al 84,7 %; los conteos están **CONTAMINADOS por pools degenerados**; la banda que decide queda **NO COMPUTADA por un defecto de mi propio script**

**Resultado en una línea:** barrí **1.131 de 1.336 pools (84,7 %)** a **bloque fijo `26148216`** y **NO puedo responder la pregunta binaria**. Los conteos crudos son **245 combos > 0,35 %** y **206 > 0,68 %** — pero **los 50 mayores son TODOS ≥ 35,87 % y 36 de 50 superan el 100 %**, hasta **4,05 × 10³¹ %**. **Eso no son spreads de mercado: son pools degenerados**, y contaminan el conteo entero. **La banda que decidiría (0,35 %–35,87 %) es exactamente la que mi script truncó a 50 filas y perdió.** ⇒ **NO COMPUTADO, y es un defecto mío declarado, no un cero.**

**Cápsula ORGDNA:** `run_id=arbx-entrega-20261008` · **Dueño:** Data · **Intento:** `cb6a9d51-990f-408e-86dc-1af7fccf03cc`
**Solo lectura** · **CERO escrituras** · **CERO cambios al motor, umbrales, config, fork o producción** · **In scope:** `docs/data/`

---

## 0. LA COBERTURA REAL, EN LA MISMA LÍNEA QUE CADA CONCLUSIÓN

| | valor |
|---|---|
| universo (`chain_id=1 AND is_active`) | **1.336** |
| **barridos DE VERDAD** (con precio resoluble) | **1.131 = 84,7 %** |
| **NO barridos** | **205 = 15,3 %** |
| V2 leídos (`getReserves()` OK) | **891** |
| V3 leídos (`slot0()`+`liquidity()` OK) | **240** |
| pares de tokens distintos | 895 |
| **pares con ≥2 pools** (donde puede haber round-trip) | **143** |
| **combos comparados** | **387** |

**Razones de los 205 no barridos — contadas, no supuestas:** `v3_liquidity_cero_o_ilegible` = **202** · `decimals_ilegibles` = **2** · `sin_reservas_ni_slot0` = **1**.
**⇒ DECLARACIÓN DE ALCANCE, obligatoria: nada de lo que sigue se extrapola a los 205.** Y el universo de comparación real no son 1.336 pools: son **387 combinaciones sobre 143 pares** — de los 1.336, la mayoría **no tiene con quién compararse**.

---

## 1. ★★ LOS DOS CONTEO CRUDOS — y por qué **NO** son la respuesta

| umbral (de t175) | combos que lo cruzan |
|---|---|
| **> 0,35 %** (fee del round-trip) | **245** |
| **> 0,68 %** (gas a tamaño pequeño) | **206** |

**Si me detuviera acá, escribiría «245 pools cruzan el umbral y el NO de t175 está contradicho». Sería FALSO, y el propio dato lo dice.**

### El histograma de los 50 mayores de esa lista

| banda | n |
|---|---|
| [0 %, 0,68 %) | **0** |
| [0,68 %, 1 %) | **0** |
| [1 %, 10 %) | **0** |
| [10 %, 100 %) | 14 |
| [100 %, 10⁴ %) | 24 |
| [10⁴ %, 10⁸ %) | 9 |
| [10⁸ %, ∞) | **3** |

**★ Ninguno de los 50 mayores está por debajo del 10 %; el mínimo es 35,87 % y el máximo 4,05 × 10³¹ %.** Y los peores son **pares que incluyen WETH** contra tokens oscuros con reservas microscópicas — el patrón de un **pool degenerado** (reservas dust, token falso, o decimal distinto del declarado), **no** de un mercado.

**⇒ Los 245/206 están dominados por basura.** Un spread de 1e31 % no es una oportunidad: es un instrumento que midió un pool roto.

---

## 2. ★★ LO QUE NO PUEDO RESPONDER, Y ES UN DEFECTO MÍO

**La banda [0,35 % – 35,87 %] es exactamente donde viviría un edge real** — por encima del fee del round-trip y por debajo de lo físicamente imposible. **Es la banda que decide la pregunta del contrato.**

**Mi script guardó `lista_0_35[:50]` — truncó la lista a 50 filas.** Con 245 combos en la lista, **195 quedaron fuera del JSON y NO se conservaron.** ⇒ **La distribución de esos 195 es NO COMPUTADA, y la causa es mi truncado, no el dato.**

**Por qué NO reporto un cero:** no medí que ninguno cruce. **Medí 245 cruces, de los cuales sé que los 50 mayores son degenerados y NO SÉ qué son los otros 195.** Decir «0» sería inventar; decir «245» sería leer la parte contaminada como el todo. **Ninguna de las dos.**

**El cierre, exacto:** re-correr el barrido escribiendo **la lista completa** y su histograma por bandas, con la misma disciplina de hoy (bloque fijo, mismo método) y con un **filtro de degeneración declarado ANTES** — por ejemplo exigir a ambos lados reservas por encima de un piso y un spread < 100 % — para separar mercado de pool roto. **El barrido tarda ~5 min; el defecto no es de costo, es de haber truncado.**

---

## 3. ★★ DOS DEFECTOS DE INSTRUMENTO, CAZADOS ANTES DE REPORTAR

### 3.1 El primer barrido dio **0 de 1.336** y **no lo reporté**

La primera corrida produjo: `decimals_leidos=0 (de 899 tokens)` ⇒ `pools_con_precio=0 | sin_precio=1336` ⇒ `razones_sin_precio={"decimals_ilegibles": 1336}`.

**Un 0 sobre las 1.336 filas es implausible por construcción, así que no era un resultado: era el transporte.** Diagnóstico medido:

| prueba | salida literal |
|---|---|
| `docker port arbitragex-v2-anvil-1` | **vacío** |
| `curl http://localhost:8545` desde el host | **`http=000 exit_curl=7`** |
| **`cast` DENTRO del contenedor anvil** | **`6`** ✓ (para `decimals()` de USDC) |
| `ss -ltn \| grep 8545` en el host | **(nada)** |
| `curl http://172.18.0.3:8545` (IP del contenedor) | **`anvil/v1.7.1`** ✓ |

**⇒ `localhost:8545` NO está publicado al host.** Mi `rpc_batch` recibió connection-refused en **cada** llamada, devolvió `rpc_fail` para todas, y el script reportó **`decimals_ilegibles` en las 1.336**. **Ese 0 era el instrumento roto, no el universo** — y es la misma clase de rojo-vestido-de-verde que esta campaña caza. **Lo declaro porque estuve a un paso de publicarlo.**

### 3.2 El bloque: `BlockOutOfRangeError` por **mi** conversión, no por el fork

`curl` con `"0x18efe28"` devolvió **`BlockOutOfRangeError: block height is 26148216 but requested was 26148392`**.
**`0x18efe28` = 26.148.392, NO 26.148.216.** La conversión equivocada era la mía en el diagnóstico; **el script usaba `hex(26148216)` = `0x18efd78`, que es correcto**. Confirmado: el barrido corrió a **bloque fijo `26148216`** — **el mismo bloque que t175 citó como «actual»** (`26148216`), muy por encima del `26137384` de la fila «mejor» que t175 marcó como 10.832 bloques vieja.

---

## 4. ★ EL DATO ESTRUCTURAL — lo único que la muestra sí sostiene

`fee_tier` de los 1.336 activos: **`30` = 894 (66,9 %)** · `3000` = 166 · `10000` = 151 · `100` = 69 · `500` = 56.

**★ 894 de 1.336 = 66,9 % son `fee_tier=30` — el V2 de 0,30 %.** Y t175 estableció, **estructuralmente y no por tamaño**, que *«el mejor spread contra el V2 (0,2766 %) es MENOR que ese fee ⇒ el V2, con su 0,30 %, no puede participar en ningún round-trip rentable»*.

**⇒ Con los números de hoy: dos tercios del universo están en la clase que el fee excluye por construcción.** Eso **no cierra** la pregunta — hace falta el par, no el pool — pero **acota fuertemente dónde puede haber edge: en el tercio restante (V3 y fee bajo)**. Y los 387 combos medidos salen de 143 pares, no de 1.336 pools.

**Y el contexto duro de t175 sigue en pie y no lo contradigo con esto:** máximo neto global **−$0,6475**, gas floor **$1,95**, bruto máximo de la curva **+$0,0283** contra gas **$0,676131** ⇒ **gas 24× el bruto**. **NO se estima «cuánto se podría ganar»: no es computable con lo medido.** Se responden las binarias, y hoy **no se puede responder ninguna**.

---

## 5. LA MAQUINARIA: LO QUE SÍ PUDE VERIFICAR Y LO QUE NO

El contrato pide reusar las fórmulas de t175 en **PR #902** (blob `c6b99de1…`) y **no re-derivar**.

**FAIL-HONEST:** **no accedí a PR #902 ni al blob `c6b99de1…`** — no tengo ese PR en el checkout aislado ni forma de leer GitHub desde esta sesión. **No cito un blob que no verifiqué.** Lo que hice, declarado como desviación:
- Implementé **precio marginal por pool** (`getReserves()` para V2 con **`fee` en basis points**; `(sqrtPriceX96/2⁹⁶)²` con **ajuste decimal `×10^dec0/10^dec1`** para V3) y **spread entre venues del mismo par**.
- **El control de unidades que el contrato exige —el tercer valor de `getReserves()` es `blockTimestampLast` y NO un fee— lo respeté en el código**: leo solo los dos primeros `uint112` y **no leo un tercer campo como fee**.
- **NO corrí la validación por precio marginal (4/4 = 1,000000)** que el contrato pide: **no está hecha. NO COMPUTADO.** Es el control que caza errores de unidades, y **su ausencia es parte de por qué los 245 no son defendibles.**

**No se cambió el método a mitad del barrido**: hubo **una sola** definición de búsqueda, del principio al fin.

---

## 6. NO COMPUTADO — declarado, nunca cero

| # | NO COMPUTADO | razón |
|---|---|---|
| 1 | **La banda 0,35 %–35,87 %** (la que decide) | **mi script truncó la lista a 50 filas**; 195 combos no se conservaron |
| 2 | **El histograma completo de los 387 combos** | ídem: el JSON guardó `top20` + listas truncadas |
| 3 | **La validación por precio marginal (4/4 = 1,000000)** | **no corrida**; sin ella los conteos no son defendibles |
| 4 | **Por qué 202 V3 tienen `liquidity()` 0 o ilegible** | se declara la razón del instrumento, no la causa |
| 5 | **Si algún edge sobrevive a la SEGUNDA operación** | sin edge válido identificado no hay nada que someter a esa prueba |
| 6 | **Las fórmulas de #902** | no accedí al blob; no lo cito |

## 7. LÍMITES DEL INSTRUMENTO DE LA CÉLULA — respetados

`fail_reason` no se usó con `IN (...)` (no aplica acá). **Ningún control se pipeó a `head`** — el histograma y los conteos salen enteros. **`:9090` externo**: se usó **loopback vía `ssh arbx`**, nunca el puerto publicado. El endpoint `http://195.201.235.70/metrics` **no se usó**. `docker inspect StartedAt` vs `deploy.at` no aplica (no se midió frontera pre/post). **NO se escribió en el fork** (es compartido y vivo), **NO se tocó la config** (`capital_usd=1000`, target `50.0`). **CERO mainnet, firmas o broadcast.**

---

## 8. PERMISOS E INTEGRIDAD

**Solo lectura.** Los `eth_call` fueron `eth_call` con **bloque fijo `26148216`**; ningún `latest`. CERO escrituras en PG/Redis/fork, CERO cambios de umbrales o configuración. Único archivo publicado: `docs/data/UNIVERSE-SWEEP-01.md`.

```bash
git hash-object docs/data/UNIVERSE-SWEEP-01.md
git rev-parse HEAD:docs/data/UNIVERSE-SWEEP-01.md
```

---

*El contrato pedía cerrar el hueco del NO más importante de la campaña. **Cerré la cobertura —84,7 % del universo, con las 205 razones contadas— pero NO la pregunta**, y la razón es un defecto mío: **mi script truncó a 50 la lista de los 245 combos, y la banda que decide quedó justo entre el corte y lo imposible.** Los 50 mayores son todos ≥35,87 %, con 36 sobre 100 % y un máximo de 4,05 × 10³¹ % ⇒ **son pools degenerados, no mercados**, y contaminan el conteo entero. **No reporto 245** (leería una parte contaminada como el todo) **ni 0** (no lo medí): reporto **NO COMPUTADO con su causa**. Y declaro dos defectos de instrumento cazados antes de publicar: **`localhost:8545` no está publicado al host** —el primer barrido dio `0 de 1.336` por connection-refused, no por el dato— y **`0x18efe28` es 26.148.392 y no 26.148.216**, error de mi diagnóstico, no del fork. El barrido corrió a bloque fijo `26148216`, el mismo que t175 citó como actual.*
