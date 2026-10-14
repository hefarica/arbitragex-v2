# FORK-BLOCK-01 — El fork lleva horas congelado: ¿el NO económico se midió contra el bloque equivocado?

**Tarea:** t184 · **Base:** `origin/main` = `4902a47c16ef21b7683e5bf405ab49cd1c525b77` · **Rama:** `docs/fork-block-01`
**Motiva:** t179 / PR #912 · **Estado:** PAPER. Cero firmas, cero broadcast, cero deploy, **cero escrituras al fork**, **cero cambios a su configuración**.
**`capital_usd` = `1000.00`, target = `50.0`, multiplicador = `3.0`: INTACTOS** (medido: `1:1000.00:50.0000:50.0000`). Cambiarlos es decisión del OPERADOR.

---

## 0. LA ADVERTENCIA QUE ORDENA TODO

**Un negativo es un resultado.** Si el bloque congelado no invalida nada, **eso es el resultado** y refuerza el NO económico con el bloque correcto. **No se busca otra vuelta.**

---

## 1. LA RESPUESTA, EN UNA LÍNEA

> **`26148216` NO es el bloque correcto para re-cotizar casi ninguna fila — pero el fork congelado NO invalida el NO económico, y además es IMPOSIBLE que lo haya fabricado: re-cotizar contra él voltea MÁS filas (`1.056` de `1.061`) y deja MENOS positivas (`5`) que re-cotizar contra la cabeza (`1.043` y `18`). El fork es ESTRICTAMENTE MÁS PESIMISTA que la cabeza. Un instrumento equivocado que es *más* pesimista no puede ser la explicación de un «no hay oportunidades».**

**Lo que el defecto SÍ invalida es la etiqueta**: el fork no es «precio fresco», es la cabeza de `2026-10-08T14:16:01Z`, **1.383 bloques / ~4,6 h** por detrás.

---

## 2. EL HECHO QUE LO MOTIVA, RE-MEDIDO — Y SU CAUSA, QUE ES MÁS EXACTA QUE «3,2 h»

### 2.1 El fork no avanza: cuarta vez que se confirma

```
curl -s -X POST -H 'Content-Type: application/json' \
  --data '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}' http://172.18.0.3:8545
lectura1: {"jsonrpc":"2.0","id":1,"result":"0x18efd78"}   exit=0
lectura2: {"jsonrpc":"2.0","id":1,"result":"0x18efd78"}   exit=0
lectura3: {"jsonrpc":"2.0","id":1,"result":"0x18efd78"}   exit=0
docker exec arbitragex-v2-anvil-1 cast block-number --rpc-url http://localhost:8545  →  26148216  exit=0
```

`0x18efd78` = **`26148216`**. **Tres lecturas idénticas por el puente más `cast block-number` por dentro: cuatro coincidencias.** La cabeza, en el mismo minuto: **`26149597`**.

### 2.2 La causa no es «3,2 h»: es el arranque del contenedor

```
docker inspect --format '{{.State.StartedAt}}' arbitragex-v2-anvil-1
→ 2026-10-08T14:16:01.110230687Z
```
Y el `sim-ctl`, del mismo evento de deploy: `2026-10-08T14:16:07.1195963Z` (**6 s después**).

**⇒ `26148216` ERA la cabeza de la cadena a las `14:16:01Z`.** El fork congela en el bloque que era cabeza **cuando arrancó**, y **su edad es exactamente la edad del contenedor**. No es un desfase misterioso: es un snapshot con fecha.

**Y el cross-check cierra el mecanismo**, con dos instrumentos independientes:

| Instrumento | Valor |
|---|---|
| Brecha en bloques | `(max(block_number) − 26148216)` = **`1.311`** |
| Bloques × 12 s | `1.311 × 12 = 15.732 s` = **`4,37 h`** |
| `StartedAt → now` | `18:39:12Z − 14:16:01Z` = **`4,39 h`** |

**⇒ `4,37 h` contra `4,39 h`: 0,46 % de diferencia.** **La edad del fork y la brecha de bloques son la MISMA cantidad medida de dos formas.** Eso convierte `--fork-url` sin `--fork-block-number` en un hecho medido, no inferido.

---

## 3. ¿ES `26148216` EL BLOQUE CORRECTO? LA PRUEBA DIRECTA, `FORK+k`

**Método**: para filas detectadas exactamente `k` bloques después del fork, re-cotizar **el mismo tamaño y la misma ruta** contra **(a) el estado del fork** y contra **(c) el estado de la cabeza**, y comparar contra **la salida guardada del motor** (`route_metadata.leg_amounts_out`), que es la cotización **en el bloque propio de la fila**.

```
G3b_FORK+10   comparables_FORK=15  err_p50=1.866e-10  err_max=1.866e-10  | CABEZA err_p50=2.190e-03
G3b_FORK+50   comparables_FORK=20  err_p50=7.683e-04                      | CABEZA err_p50=1.420e-03
G3b_FORK+200  comparables_FORK=18  err_p50=1.727e-03                      | CABEZA err_p50=9.918e-04
G3b_FORK+1000 comparables_FORK=23  err_p50=1.492e-03                      | CABEZA err_p50=1.230e-03
```

**⇒ A DIEZ BLOQUES DEL FORK, LA RE-COTIZACIÓN DESDE EL FORK REPRODUCE LA SALIDA GUARDADA DEL MOTOR CON ERROR `1,866e-10` — DIEZ DÍGITOS SIGNIFICATIVOS — MIENTRAS LA DE LA CABEZA ESTÁ A `2,190e-03`.**

Tres cosas quedan probadas de una vez:

1. **El estado del fork ES fielmente el estado en `26148216`.** No está corrupto, no está desplazado: es un snapshot exacto. **El defecto es cuándo, no qué.**
2. **El bloque correcto para re-cotizar una fila es SU PROPIO `block_number`**, y la salida guardada lo es: reproducirla exige el estado de ese bloque, a diez dígitos.
3. **Y hay un CRUCE medible**: a `+10` gana el fork (`1,9e-10` contra `2,2e-03`); a `+50`, `+200` y `+1000` **gana la cabeza** (`1,4e-03`, `9,9e-04`, `1,2e-03` contra `7,7e-04`, `1,7e-03`, `1,5e-03`). **⇒ Ningún instrumento sirve para todo el rango: el fork sirve a ±decenas de bloques de su bloque; la cabeza sirve a ±decenas de bloques de la cabeza; y para lo de en medio no sirve ninguno.** Eso es exactamente lo que significa «la fila se cotiza a su propio bloque».

### 3.1 El cero en el bloque exacto del fork TIENE productor

```
G3_ventana_alrededor_del_fork = 0 | 3975 | 602524
        (=fork)=0   (fork±5)=3.975   (fork±500)=602.524
G3_bloques_vecinos_con_filas = 26148186:429 | 26148187:429 | ... | 26148193:429
```

**⇒ NO EXISTE NI UNA FILA en el bloque `26148216`, y las hay en los bloques vecinos (429 por bloque).** El productor del cero es el **deploy**: `anvil` nace a las `14:16:01Z` y el `sim-ctl` a las `14:16:07Z`, con el cambio de binario en medio. **Hay un hueco de detección justo en el bloque del fork.**

**⇒ El control «filas del bloque exacto del fork» es INAPLICABLE, y se declara con su razón** — no como «no hay nada que medir». Por eso el control se corrió a `FORK+k`.

---

## 4. LA PREGUNTA: ¿LA MEDICIÓN ESTABA CONTRA EL ESTADO EQUIVOCADO?

### 4.1 Veredicto de signo en los tres estados — población rentable, `1.061` re-cotizables

```
G4_total=1061
signo_distinto_vs_PROPIO:  EN_FORK = 1056   EN_CABEZA = 1043
G4_ratio>1:  guardado=1061   fork=5   cabeza=18
G4_filas_mas_viejas_que_el_fork=119   mas_nuevas=917
```

| Estado contra el que se re-cotiza | filas con signo **distinto** al propio | filas con `ratio > 1` |
|---|---|---|
| **El propio `block_number`** (la salida guardada) | — (referencia) | **`1.061`** |
| **(a) el fork, `26148216`** | **`1.056`** | **`5`** |
| **(c) la cabeza** | **`1.043`** | **`18`** |

**⇒ EL FORK VOLTEA MÁS FILAS Y DEJA MENOS POSITIVAS QUE LA CABEZA.** No es un instrumento que «esconde» oportunidades: es un instrumento que las destruye **igual o más** que el correcto.

### 4.2 Y la dirección del error, medida por fila

`917` de las `1.061` filas rentables (**86,43 %**) tienen `block_number` **POSTERIOR** al del fork; **`119`** son anteriores. En la tabla entera: **`9.137.741` (89,17 %)** anteriores, **`1.053.526` (10,28 %)** posteriores, **`0`** exactamente en él.

**⇒ Para el 89 % de la tabla el fork es un estado MÁS NUEVO que la fila** (una «frescura» legítima, aunque 4,6 h vieja respecto de la cabeza). **Para el 10 % restante —y para el 86 % de las filas RENTABLES— el fork es un estado ANTERIOR a la propia fila**: re-cotizar ahí no mide la decadencia de la fila, **mide una prehistoria en la que su oportunidad aún no se había formado.**

**⇒ El fork es el bloque equivocado en LAS DOS DIRECCIONES.** Y por eso voltea casi todo: no está «cerca», está **en otro punto del tiempo**.

---

## 5. ENTONCES, ¿QUÉ INVALIDA EL DEFECTO Y QUÉ NO?

### 5.1 NO invalida el hallazgo de decadencia de t179 — y esto se puede demostrar

**t179 midió la decadencia `cache` contra `guardado`, y el `cache` ES la cabeza** (`arbx:pool_reserves:1:0x0d4a11d5…` → `{"r0":"2950138623343163056090","r1":"7205394983057","blk":26149597,"ts":1791485639}`, contra `max(block_number)=26149597`, **mismo bloque**). **El fork no participó en ese número.** El `1.043` de `1.061` no está contaminado.

**Y se re-corre el control de t179 en esta tarea, con los dos instrumentos a la vez:**

```
G5_gap0  comparables=23 | CABEZA err_p50=3.724e-10 | FORK err_p50=2.718e-03
G5_gap1  comparables=23 | CABEZA err_p50=6.123e-10 | FORK err_p50=2.718e-03
G5_gap5  comparables=23 | CABEZA err_p50=1.104e-04 | FORK err_p50=2.828e-03
G5_gap50 comparables=23 | CABEZA err_p50=4.326e-04 | FORK err_p50=2.275e-03
```

**⇒ La curva de decadencia reproduce con la cabeza (`3,7e-10` → `6,1e-10` → `1,1e-04` → `4,3e-04`), y el fork está entre 3 y 7 órdenes de magnitud peor en TODOS los tramos medidos.** t179 tenía razón y su instrumento era el correcto.

### 5.2 AJUSTE PROPIO QUE CAZO Y DECLARO: un número de t179 no reproduce

t179 reportó, en su curva `K1`, **`gap5 err_p50 = gap5 err_max = 5,148e-10`** — con el máximo **igual** a la mediana. **Aquí `gap5` da `1,104e-04`, seis órdenes de magnitud mayor.** Con 23 filas y 6 pools, un `max == p50` idéntico es la firma de una muestra degenerada (las 23 filas compartiendo la misma diferencia). **⇒ La cifra de `gap5` de t179 NO reproduce y se corrige a `1,104e-04`.** Su `gap1` sí reproduce (`6,123e-10` aquí frente a `5,148e-10` allí: mismo orden) y su conclusión —**a estado fresco la re-cotización reproduce la salida del motor a ~1e-10**— **queda confirmada por el `gap0` nuevo (`3,724e-10`) y por el `FORK+10` (`1,866e-10`)**, que son mediciones independientes.

### 5.3 SÍ invalida la etiqueta «precio fresco» de t175/t176 — y con una consecuencia concreta

t175/t176 midieron un «estado fresco» contra el fork. **Medido: ese estado tiene fecha, `2026-10-08T14:16:01Z`, y para el 86 % de las filas rentables es ANTERIOR a la fila misma.** La medición que hicieron **no sobre-estimó el edge por usar estado viejo: lo sub-estimó.** Un instrumento más pesimista del correcto.

**⇒ Y aquí está la razón de que el NO no sea un artefacto del bloque equivocado, en una línea: para que el bloque equivocado explicara `passed = true = 0`, el estado equivocado tendría que ser MÁS FAVORABLE que el real. Medido: es MENOS favorable (`5` positivas contra `18`). La dirección del error excluye la explicación.**

### 5.4 Y la hipótesis de la tarea, contestada explícitamente

> *«Si la fila se detecta a la cabeza y la simulación corre en un fork 1.189 bloques por detrás, TODA fila se simula contra un estado en el que su edge ya no existe. Eso explicaría `passed = true = 0` por el instrumento, no por el mercado.»*

**MEDIDO Y REFUTADO EN SU CONSECUENCIA.** Es cierto que en el estado del fork el edge ya no existe (a 1.383 bloques, `1.056` de `1.061` voltean). **Pero en el estado CORRECTO —la cabeza— tampoco existe: `1.043` de `1.061` voltean.** Los dos dicen NO, y **el equivocado dice NO con más fuerza**. **⇒ El estado equivocado no explica el NO: el NO es lo que ambos estados dicen. Y el correcto lo dice con `18` matices en vez de `5` — matices que t179 ya mostró que son artefactos del campo de notional, no candidatos.**

---

## 6. LO QUE NO SE HIZO, Y POR QUÉ — DECLARADO

- **NO se escribe en el fork.** Ni `setStorageAt` ni ningún `anvil_*` de escritura. **Todas las lecturas son `eth_call` y `eth_blockNumber`.**
- **NO se cambia la configuración del fork.** Arreglar `--fork-block-number` es **otra tarea y otra decisión** (toca el motor). Aquí sólo se **mide si el defecto invalida lo medido** — y no lo invalida.
- **Si medir el bloque propio hubiera exigido escribir, se habría declarado NO COMPUTADO.** No fue necesario: **el bloque propio de una fila ES su salida guardada**, y eso quedó probado a diez dígitos (§3).
- **`1.331` pools de t175 siguen sin barrer** (cubrió 5 de 1.336 = 0,37 %). **Esta tarea no cierra ese hueco y no lo presenta como universal.**
- **Los «37 medibles de t183» no se usaron: no tengo su definición de población en este árbol.** Se declara. Las poblaciones usadas son las que se pueden definir sobre el esquema: **la rentable (`net_profit_usd > 0`, `1.096` filas)** y **la rechazada por estado por edad** (`spread_negative_round_trip`).

---

## 7. NO-EXPIRACIÓN E IDENTIDAD — CITADAS, NO RE-DERIVADAS

De t179 (PR #912), sin volver a medirlas:
- **Identidad corregida:** `9.982.350 − 2.743.290 (27,48 %) = 7.239.060` ⇒ **100,0000 % de las filas CON valor cumplen `quote_block = block_number`, CERO excepciones.** ⇒ todo lo de este artefacto se enuncia sobre **las filas CON valor**.
- **El detector SÍ mira estado fresco**: `arbx:pool_reserves:1` (897 claves V2) y `arbx:v3_slot0:1` (429 V3) llevan `blk` con **TTL 17–23 s** ⇒ al detectar el estado tiene **≤ ~20 s** y `quote_block = block_number` es **honesto**. **Lo que envejece es la fila, no la cotización.**
- **No-expiración, sin el supuesto de 12 s:** p50 **24,471 h** · p90 **69,465 h** · max **85,462 h** · **>24 h 51,11 %** · **>48 h 25,38 %**.

---

## 8. CONTROLES — TODOS EJECUTADOS, CON SU EXIT CODE LITERAL

| Control | Resultado |
|---|---|
| Canal `SELECT 1` | `1`, **exit=0** |
| Negativo `SELECT esto_no_existe` | **exit=1** + `ERROR: column "esto_no_existe" does not exist` — **sin tubería** |
| **Control positivo del `LIKE`** | `LIKE 'reverted:%'` → **`4007`**, exit=0 |
| **El cero con su productor** | `spread_negative_round_trip`: `leg_amounts_out` en **`0` de `280.962`** en la ventana. **Cero confirmado con su razón: el campo no se persiste.** |
| Fork × 3 + `cast` | `26148216` las cuatro veces |
| Cabeza vs fork | `1.383` bloques; `1.311 × 12 s = 4,37 h` vs `StartedAt → now = 4,39 h` |
| `FORK+10` desde el fork | `1,866e-10` ⇒ el fork es fiel a `26148216` |
| `gap0` desde la cabeza | `3,724e-10` ⇒ la cabeza es fiel a la cabeza |
| Config | `1:1000.00:50.0000:50.0000` — **intacta** |
| `:9090` externo | **`000`, exit=7** — **defecto #9, undécima aparición** |
| Ningún control pipeado a `head` | los tres, sin tubería |

### 8.1 DOS AJUSTES PROPIOS, CAZADOS Y DECLARADOS ANTES DEL NÚMERO

El estándar que t183 cumplió cazando dos ajustes propios se aplica a mí:

1. **Un cero cuyo productor era MI parser.** Añadí `--json` a `cast call` y eso rompió `token0()`/`token1()`; como el motor de re-cotización sale temprano si `token0` es `None`, la población rentable devolvió **`G4_total=0`** y los errores de `G3b`/`G5` salieron **`0,000e+00`**. La firma que lo delató: con `fee=None` para todos los pools, el camino del fork tomó la rama V2 y leyó **`fork=23`** — **exactamente los 23 pools V2 reales de 33**. Corregido parseando por líneas (`cast call` humano: una línea por valor de retorno).
2. **Un cero cuya productora era una lista VACÍA.** `(eF[len(eF)//2] if eF else 0)` imprime **`0,000e+00` para una lista sin elementos** — indistinguible de una concordancia perfecta. Corregido: ahora se imprime **`comparables=N`** y **`NO_COMPUTADO(vacio)`** si no hay ninguno. **Es la misma clase de error que t179 encontró en el `leg_amounts_out`: un cero que no se puede leer sin su productor.**

---

## 9. DEFECTOS DEL CAPITÁN REPRODUCIDOS

| # | Defecto | Evidencia |
|---|---|---|
| **#9** (undécima aparición) | `:9090` externo | `external_9090_http_code=000`, **exit=7** |
| **#13/#14** | `quote_block` y `gross`/`net` en el JSONB, no columnas planas | ruta `economics->>` obligatoria |
| **#12** | `fail_reason` prefijado | `LIKE 'reverted:%'` → `4007`; nunca `IN (...)` |
| **#11** | Frontera = `StartedAt` | `anvil` `2026-10-08T14:16:01.110230687Z` |
| **#8** | `revert_risk_pct` es el campo de rastro | no `raw_trace` |
| — | `172.18.0.3:8545` sí, `localhost:8545` no, desde el host | `curl` al puente: exit=0; el fork se lee por `docker exec … cast` |

**Y un ajuste de comilla MÍO, declarado**: el control del `LIKE` corrido desde `bash` por `ssh` falló con **`syntax error at or near ":"`**, exit=1, porque mi escape de comillas no sobrevivió la cadena PowerShell→ssh→bash. **Se re-corrió por el motor Python (sin comillas de shell) y dio `4007`.** El fallo era del canal, no del SQL.

---

## 10. ALCANCE Y HUECOS

- **CERO cambios al motor, umbrales, configuración, fork o producción.**
- **CERO mainnet, firmas o broadcast.** El estado del fork y el de la cabeza se leyeron **en local** (`anvil` y el cache interno `arbx:pool_reserves:1`), que es el que **el propio detector** usa.
- **NO SE ESTIMA NINGUNA GANANCIA.** No es computable con lo medido: falta flujo real, competencia e impacto acumulado.
- **Huecos declarados:** los 1.331 pools de t175 · la definición de población de t183 · el hueco de detección en el bloque del fork (su productor está identificado —el deploy— pero **no se midió su anchura exacta**) · el 4.º camino de `quote_block` vacío con cifras (10.784 filas, de t179).
- **Ninguna propuesta de arreglo se implementa aquí.** `backend/`, `docker/` y `contracts/` están fuera de alcance. **Medir no autoriza a cambiar.**

**Publicación:** `git add` normal · rama `docs/fork-block-01` · PR en **DRAFT** · integridad por `git hash-object` · **nunca `Out-File`**.
