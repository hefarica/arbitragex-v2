# LIVE-STATE-01 — El detector decide sobre el bloque de detección: ¿qué cambia si mira estado vivo?

**Tarea:** t179 · **Base:** `origin/main` = `77b42b3dccc001455fda3e8d4437d973c9e98c4b` · **Rama:** `docs/live-state-01`
**Predecesora directa:** t176 / PR #903 (`docs/backend/STALE-QUOTE-01.md`) · **Estado:** PAPER. Cero firmas, cero broadcast, cero deploy, cero escrituras al fork.
**`capital_usd` = `1000.00`, target = `50.0`, multiplicador = `3.0`: NO SE TOCAN.** Son decisión del OPERADOR.

---

## 0. LA ADVERTENCIA QUE ORDENA TODO — va primero para que nadie la lea después

El **NO económico ya estaba establecido y reforzado** antes de esta tarea: máximo neto **global sobre todos los tamaños** = **`−$0,6475`**; el gas es **24×** el mejor bruto; **ningún tamaño cierra la brecha**; y corregida a precio fresco la mejor fila pasa de **`+$1,5459`** a **`−$0,6475`** con **`0`** supervivientes de las 135 rentables.

**⇒ Esta tarea NO promete que aparezca dinero. Su valor es tener el instrumento correcto.** Y el resultado de (2) es un **negativo**, que es un resultado: **cero candidatos aparecen en las 373 filas rechazadas re-cotizadas.**

---

## 1. RESULTADO EN CINCO LÍNEAS

| # | Pregunta | Respuesta medida |
|---|---|---|
| **1** | ¿Qué cambia si decide sobre estado **vivo**? | **`1.043` de `1.061` (98,31 %) pierden el signo positivo.** `13` conservan bruto positivo **≤0,16 %**. `5` muestran bruto **>1 %**, económicamente imposible ⇒ **artefacto**, no hallazgo. |
| **2** | ¿Aparece algún candidato que con estado viejo no se veía? | **NO. `0` de `373` (0,00 %) filas rechazadas pasan a positivo**, en edades de 1 a 5.000 bloques (12 s a ~16,7 h). |
| **3** | ¿Decide el detector sobre estado viejo **en el momento de detectar**? | **NO — y esto CORRIGE a t176.** El cache que lo alimenta tiene **TTL 17–23 s** y **`blk` = cabeza ± 1**. Lo que envejece es **la fila**, no la cotización. |
| **4** | El defecto del nombre | **Confirmado y reforzado**: `100,0000 %` de las filas **con valor** cumplen `quote_block = block_number`. **`0` excepciones en 7.239.060.** |
| **5** | ¿Hay un tercer camino que t176 no listó? | **SÍ, y es el 27,48 % de la tabla.** `stamp_on_emit` rama `None`, guarda `has_figures` (`economics.rs:524-525`). **Confirmado por código y por datos.** |

---

## 2. LA CORRECCIÓN DE ESTA TAREA: el estado **en detección** ES fresco

t176 escribió: *«El detector nunca re-cotiza antes de simular. El sistema decide sobre estado que ya no existe.»*
La primera frase es cierta. **La segunda es falsa en el momento de detectar, y se corrige con el productor del estado.**

**El cache que alimenta al detector** (descubierto por SCAN de namespaces, nunca `KEYS`):

```
arbx:pool_reserves:1   →  897 claves V2
arbx:v3_slot0:1        →  429 claves V3
arbx:tokens:1          → 2503      arbx:dedup:pendingtx → 1004
```

**Lectura de los 5 pools WETH/USDT, `redis-cli GET` + `TTL`, exit=0:**

```
arbx:pool_reserves:1:0x0d4a11d5eeaac28ec3f61d100daf4d40471f1852
{"r0":"2965047699624429755188","r1":"7169522429528",
 "token0_addr":"0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2","blk":26149253,"ts":1791481454}   TTL=23
```

**⇒ El cache lleva `blk` y `ts`, y se refresca cada 17–23 s.** Comparado con la cabeza de detección (`max(block_number)`, `now()`): **el cache está a ±1 bloque de la cabeza**. Cuatro lecturas separadas dan `blk` = `26149194`, `26149228`, `26149253`, `26149481`, todas alineadas con la cabeza del momento.

**⇒ En el instante de detectar, el estado tiene ≤ ~20 s. `quote_block = block_number` es, por tanto, una etiqueta HONESTA de la frescura en el momento de emitir.** Lo que decae es la fila guardada, no la cotización.

### 2.1 Sub-defecto nuevo: el cache V3 no se puede atribuir a un bloque

```
arbx:v3_slot0:1:0x11b815efb8f581194ae79006d24e0d814b7697f6
{"sqrt_price_x96":"3898324018919506971252098","liquidity":"528972784888323159","ts":1791480738}   TTL=17
```

**⇒ El registro V3 trae `ts` pero NO trae `blk`.** Un consumidor no puede saber a qué bloque corresponde el precio V3 que está usando. El V2 sí (`"blk":26149253`). **Asimetría real, no cosmética: 429 claves V3 sin bloque.**

---

## 3. TRES «AHORA» DISTINTOS — Y EL INSTRUMENTO DE t175/t176 ESTÁ CONGELADO

| Estructura | Bloque | ¿Avanza? | Evidencia |
|---|---|---|---|
| Cabeza de detección | **`26149405`** | sí | `SELECT max(block_number), now() FROM opportunities` → `26149405 \| 2026-10-08 17:44:20Z` |
| Cache Redis | **`26149253`** | sí (±1) | `GET arbx:pool_reserves:1:0x0d4a11d5…` |
| **Fork anvil** | **`26148216`** | **NO** | `docker exec arbitragex-v2-anvil-1 cast block-number` → idéntico en **tres lecturas separadas >2 h** |

**⇒ El «precio fresco» de t175 (PR #902) y t176 (PR #903) NO se midió contra la cadena viva: se midió contra un snapshot congelado **971 bloques (~3,2 h)** por detrás.** El `anvil` arranca con `--fork-url "$ANVIL_FORK_URL"` **sin `--fork-block-number`**: se congela donde estaba la cabeza al arrancar el contenedor y no vuelve a avanzar.

**⇒ El defecto que t176 nombró se reproduce UN NIVEL MÁS ABAJO: el instrumento de medida también decide sobre estado que ya no existe.** El hallazgo de t175/t176 no muere —**el estado viejo sobre-estima** — pero la etiqueta «precio fresco» era optimista: era «precio congelado hace 3,2 h».

### 3.1 Retractación de una lectura de t176

t176 escribió: *«el bloque del fork (`26148216`) avanza mientras la tabla no — la tabla se quedó en `26148485` de máximo»*.
**FALSO, y se retracta:** el fork **NO avanza** (`26148216` en tres lecturas). Lo que avanza es la tabla (`26148485 → 26149405`). La lectura quedó invertida.

---

## 4. EL TERCER CAMINO: LOCALIZADO CON PATH + LÍNEA + BLOB, Y CONFIRMADO POR DATOS

### 4.1 La identidad, desglosada — y la trampa `jsonb 'null'` ≠ SQL NULL

El `verify` del contrato pide `SELECT count(*), count(economics), count(economics->'quote_block')`. Medido, exit=0:

```
10092737 | 10092737 | 7317839
```

**`count(economics->'quote_block')` cuenta la CLAVE, no el valor.** Un valor JSON `null` **sí** cuenta. Por eso hace falta `->>`:

```
SELECT count(*), count(economics->'quote_block'),
       count(*) FILTER (WHERE economics->>'quote_block' IS NULL)          AS json_null,
       count(*) FILTER (WHERE (economics->>'quote_block')::bigint = block_number) AS identicos
FROM opportunities;
→ 9982350 | 9982350 | 2743290 | 7239060
```

**⇒ `9.982.350 − 2.743.290 = 7.239.060` EXACTO.** Es decir:

> **`100,0000 %` de las filas que TIENEN valor cumplen `quote_block = block_number`. Cero excepciones en 7.239.060 filas.**

**⇒ La afirmación de fondo de t176 queda CONFIRMADA y más fuerte** (cero excepciones), y su denominador **se corrige**: no eran «todas las filas», eran las filas **con** valor. Las otras **2.743.290 (27,48 %)** no difieren: **están vacías**.

### 4.2 Quién las vacía: la rama `None` de `stamp_on_emit` — NO listada por t176

`backend/searcher-rs/src/economics.rs` — blob **`173e9227d308867bd561fba9c44a06e7d395a85c`**

| Línea | Contenido | Papel |
|---|---|---|
| `:246` | `let quote_block = sized.candidate.opportunity.block_number;` | **el productor**: alias de la detección |
| `:275` | `quote_block,` | el campo del objeto `computed` |
| `:454` | `quote_block: None,` | **`economics_partial` nace SIN bloque** |
| `:484` | `quote_block: None,` | **`economics_error` nace SIN bloque** |
| `:494-530` | `pub fn stamp_on_emit(opp: &mut Opportunity)` | **las dos ramas** |
| `:500-501` | `if e.quote_block.is_none() { e.quote_block = opp.block_number; }` | rama `Some(e)` — **la que citó t176** |
| **`:524-525`** | **`if has_figures && obj.quote_block.is_none() { obj.quote_block = opp.block_number; }`** | **TERCER CAMINO, con guarda `has_figures`** |

Y el comentario del propio autor, `:521-523`, dice el resto:
> *«The partial object inherits the row's own block evidence (the figures were computed against it); **the error object stays block-less — no quote existed (R8)**.»*

`backend/searcher-rs/src/opportunity_emitter.rs` — blob **`938f382f8b1ba311e9ec399618d9da4a83de44c2`**: `:396` es el **único** call-site de `stamp_on_emit`; `:395` el doc-comentario; `:1792` el doc del test; **`:1794-1802`** el test `always_compute_producer_object_wins_verbatim` que fija la semántica:
```rust
assert_eq!(e.quote_block, Some(12_345_678), "quote_block backfilled");
```

`backend/shared-rs/src/contracts.rs` — blob **`0a70fa5608bcfe39697c6d10dd523b4985c903b7`**, `:233`: `pub quote_block: Option<u64>,` → **`Option` es lo que hace el `null` legal, no un bug de serialización.**

**⇒ SON CUATRO PUNTOS DE ESCRITURA, no dos:** `:246` (constante) · `:275` (campo) · `:500-501` (rama A) · **`:524-525` (rama B)**. **El tercero es el que produce el 27,48 % de vacíos.**

### 4.3 Confirmación por DATOS — el mecanismo, no sólo el conteo

Si la hipótesis es correcta, **toda** fila con `quote_block` vacío tiene que ser una fila **sin cifras** (`has_figures == false`). Medido:

```
J2   (últimos 20.000 bloques) = 2361663 | 2361663 | 217746
      qb_null=2.361.663 · qb_null_sin_cifras=2.361.663 · qb_no_null_sin_cifras=217.746
J2_estado = error n=2362288
J2_total_tabla = 2767841 | 2757057 | 10784
```

**⇒ `qb_null == qb_null_sin_cifras` EXACTO, y `computation_status = "error"` en el 100 % de ellas.** La familia es la de **fallo de cotización**, como predice `economics_error`:

| Familia con `quote_block` vacío | n |
|---|---|
| `v3_pool_not_catalogued` | 1.127.414 |
| `v3_quote_unavailable` | 687.310 |
| `single_pool_no_spread` | 334.734 |
| `v3_pair_no_pools` | 299.157 |
| `spread_negative_round_trip` | 257.682 |
| `v3_pool_revert` | 16.367 |

**Código y dato coinciden en el MECANISMO.** No es correlación: es la misma condición.

### 4.4 Y un CUARTO camino, que se declara NO EXPLICADO

`J2_total_tabla` deja **`10.784`** filas (y `10.525` en una lectura previa) **con cifras (`expected_profit_usd` no nulo) y `quote_block` vacío**. Son **0,39 %** de los vacíos.

**NO COMPUTADO — no se declara mecanismo.** Hipótesis candidatas, **sin verificar**: (a) `always_compute_enabled()` devolviendo `false` en `:495-497` y saliendo antes de sellar; (b) objeto `economics` adjuntado por un productor **después** de la frontera de emisión. **Ninguna se midió.** Se entrega como hueco con su tamaño.

---

## 5. EL NOMBRE MIENTE — se DOCUMENTA y se DEJA, y por qué

**Decisión declarada: se documenta en este artefacto y el campo NO se renombra.**

Tres razones, en orden de peso:

1. **Renombrar no arregla nada.** El problema no es que `quote_block` se llame así: es que **no existe ningún campo que diga el bloque de una cotización VIVA**. Un nombre mejor sobre el mismo dato sigue siendo el bloque de detección. Lo que falta es **un segundo campo** (`live_quote_block`) o **re-cotizar**, y eso es una decisión del OPERADOR, no un renombre.
2. **El coste es real y el beneficio es cero.** El nombre viaja por el contrato compartido (`contracts.rs:233`), por el wire y por `api-server/src/routes/opportunities-live.ts:736`, que lo **enumera como columna servida al frontend**. Renombrar es cambio de esquema + contrato + UI, con lectores ya desplegados, para ganar exactamente 0.
3. **El daño ya está contenido por medición.** El campo hizo perder tiempo **una vez** (t176 lo dice). A partir de aquí está localizado con path, línea y blob, y con la identidad escrita: **`quote_block ≡ block_number` donde hay valor**. Un lector futuro tiene el hecho.

**⇒ Recomendación al OPERADOR, no ejecutada:** añadir `live_quote_block: Option<u64>` **junto** al actual y dejar el viejo como está. Eso no rompe nada y hace medible la brecha por fila. **No se implementa aquí: `backend/` está fuera de alcance.**

---

## 6. LA PREGUNTA (1): antes / después con estado **vivo**

**Método, sin cambiar de instrumento a mitad de la comparación.** Por cada fila rentable, la misma ruta y el **mismo tamaño**, re-cotizados contra el **estado vivo** (`arbx:pool_reserves:1:*` / `arbx:v3_slot0:1:*`). La ruta se reconstruye de `route_metadata.pool_addresses` + `token0()`/`token1()` (inmutables, del fork congelado) + `amount_in_wei`. El estado vivo se lee **en una sola generación** (el `blk` del sentinela idéntico antes y después de la lectura).

**Población rentable: `1.096` filas.** Recomputadas: **`1.061`**. `34` no computables con razón declarada (`no_2_pools`).

### 6.1 Veredicto SIN PRECIO (sólo ratio — no depende de ningún campo sospechoso)

El bruto es `r = salida_final / entrada`. La fila está en la población rentable porque su **`r` guardado es `>1` en las `1.061`** (por construcción). Contra estado vivo:

```
L2_total=1061   ratio_guardado>1=1061   ratio_vivo>1=18   ratio_vivo>1.01=5
FLIP_GROSS_POS_A_NO_POS = 1043
```

| Edad (bloques) | n | **pierden el bruto** | conservan `r>1` | conservan `r>1,01` |
|---|---|---|---|---|
| **0–10** | 71 | **58** | **13** | **0** |
| 10–100 | 46 | **46** | 0 | 0 |
| 100–1.000 | 825 | **825** | 0 | 0 |
| 1.000–5.000 | **0** | — | — | — |
| >5.000 | 119 | **114** | **5** | **5** |
| **TOTAL** | **1.061** | **1.043 (98,31 %)** | **18** | **5** |

**⇒ `1.043` de `1.061` pierden el signo. La degradación es total a partir de los 10 bloques: `0` de `871` sobreviven entre 10 y 1.000 bloques.**

### 6.2 Los `5` con bruto `>1 %`: **ARTEFACTO DECLARADO, no hallazgo**

Discriminante ejecutado, fila por fila (`K3`). Para `id 0f580ba9`, `gap=16.722`:
```
net_vivo=+43.4624  net_guardado=+1.0530
r_vivo=1.0441512   r_guard=1.0907896   ain=7364828765100715   cost=0.6887
pools=[0xd3772a96…, 0x9cbc2a6a…]   fees=[None, None]   tokens=['0xade00c28','0xade00c28']   dec={'0xade00c28':18,'0xc02aaa…':18}
```
**Tres razones por las que NO es candidato, en orden de fuerza:**

1. **`r_guard = 1,0908` es imposible.** Un ida-y-vuelta por dos pools V2 al 0,30 % da `≈ 0,994` en el mejor caso. **`+9,08 %` de bruto sin riesgo en un bloque no existe.** La fila **ya estaba corrupta al guardarse**.
2. **`+$43,46` sobre `r_vivo = 1,0442` exige `amount_in_usd ≈ $1.000`, pero `amount_in_wei = 0,0073648` con `dec=18`.** Los dos campos de la MISMA fila se contradicen por ~5 órdenes de magnitud.
3. **`r_vivo = 1,0442` (>4 %) sigue siendo implausible** para 0,30 %+0,30 %, y **los 5 son exactamente los 5 que superan el filtro de plausibilidad `r > 1,01`.** El filtro y el conjunto coinciden: **no hay un sexto caso ambiguo.**

**⇒ Se declaran ARTEFACTO DE INSTRUMENTO, con su número, y NO se presentan como candidatos.**

### 6.3 Los `13` con bruto plausible: tampoco son candidatos útiles, y la razón es medible

`r_vivo = 1,0015838 … 1,0015839` — **idéntico a 8 dígitos en quince filas con `ain` distinto**, todas `gap=0`, `dex_a|dex_b = unknown|unknown`, `pools = [0xf04543fb…, 0x7825de55…]`, tokens `0x50d1c977` (LDO).

Para que `0,158 %` de bruto cubra un coste de **`$0,6381`**, el notional tiene que superar **`$404`**. La fila **declara** `amount_in_usd ≈ $1.000`, luego contaría como positivo… **pero `amount_in_wei = 0,0079572` LDO**. A un precio de LDO del orden de `$1`, el notional real es **`≈ $0,008`** y el coste lo aplasta.

**⇒ La contradicción `amount_in_usd` vs `amount_in_wei` no es de las 5: es sistémica.** Medida:

```
L1 = 57 | 1096
```
**`57` de `1.096` filas rentables (5,2 %) declaran un precio implícito `> $100.000 / token`.** Los supervivientes se concentran exactamente ahí.

**⇒ VEREDICTO (1), en la forma más defendible:** con el instrumento correcto, **`1.043` de `1.061` (98,31 %) caen**; **`0` de las `1.061` sobrevive como candidato validado**; y **los `18` que el conteo USD marcaba como supervivientes viven todos en el 5,2 % de filas cuyos propios campos se contradicen.**

**⇒ El «sobreviven 0» de t176 SE CONFIRMA en sustancia**, con un instrumento que además nombra el porqué: **la fila guardada sobre-estima el edge, y en 57 casos sobre-estima el propio notional.**

---

## 7. LA PREGUNTA (2): ¿aparece algún candidato que con el estado viejo NO se veía?

### 7.1 Primero, la pared del instrumento — y es un hallazgo propio

Para re-cotizar una fila rechazada **al mismo tamaño** hace falta su cotización por pierna. **La familia que decide el veredicto de mercado NO la guardó.** Medido (`J1`, ventana de 500 bloques, exit=0):

| `rejection_reason` | n | con `leg_amounts_out` |
|---|---|---|
| **`spread_negative_round_trip`** | **283.519** | **0** |
| `v3_pool_not_catalogued` | 54.621 | **0** |
| `non_positive_profit` | 43.021 | 42.338 |
| `v3_quote_unavailable` | 6.734 | **0** |
| `negative_net_profit` | 851 | 851 |
| `no_tradable_size` | 500 | **0** |
| `v3_pool_revert` | 368 | **0** |
| `gas_floor_breach:own_capital` | 109 | 108 |

**⇒ `spread_negative_round_trip` — el `52,5 %` de la tabla — tiene `leg_amounts_out` en `0` de `283.519` filas. Igual `v3_pool_not_catalogued`, `v3_quote_unavailable`, `no_tradable_size`, `v3_pool_revert`.**
**⇒ La evidencia para re-auditar el rechazo mayoritario NUNCA SE PERSISTIÓ.** No es que nadie la haya mirado: **no existe**. Es un hueco de instrumento, y es la razón por la que `H5` (la primera pasada de esta tarea) devolvió `0` en los cuatro buckets: **el cero tenía productor, y el productor era la ausencia del campo, no la ausencia de flips.** Se declara antes de dar cualquier número.

### 7.2 Y aun así se mide: la ruta se reconstruye de INMUTABLES

`token0()`/`token1()` **no cambian nunca** ⇒ leerlos del fork congelado es válido. Con ellos + `token_in` + `amount_in_wei` la ruta `A → B → A` queda determinada sin ningún campo mutable.

```
K2_gap1     filas=66   recot=61   FLIP_A_POSITIVO=0   siguen_neg=61
K2_gap5     filas=117  recot=117  FLIP_A_POSITIVO=0   siguen_neg=117
K2_gap20    filas=80   recot=74   FLIP_A_POSITIVO=0   siguen_neg=74
K2_gap100   filas=61   recot=58   FLIP_A_POSITIVO=0   siguen_neg=58
K2_gap1000  filas=16   recot=16   FLIP_A_POSITIVO=0   siguen_neg=16
K2_gap5000  filas=47   recot=47   FLIP_A_POSITIVO=0   siguen_neg=47
K2_gap20000 filas=0
```

| Edad | filas | re-cotizadas | **pasan a positivo** | siguen negativas |
|---|---|---|---|---|
| 1 bloque | 66 | 61 | **0** | 61 |
| 5 | 117 | 117 | **0** | 117 |
| 20 | 80 | 74 | **0** | 74 |
| 100 | 61 | 58 | **0** | 58 |
| 1.000 | 16 | 16 | **0** | 16 |
| 5.000 | 47 | 47 | **0** | 47 |
| **TOTAL** | **387** | **`373`** | **`0` (0,00 %)** | **`373`** |

> **⇒ NO APARECE NINGÚN CANDIDATO. `0` de `373`, en todo el rango de edades (12 s a ~16,7 h).**

**Y el control de hipótesis nula aguanta en la dirección correcta:** a `gap=1` y `gap=5` —donde el estado vivo es el mismo que el de detección— **tampoco voltea ninguna** (`0` de `178`). Si el motor produjera flips espurios, aquí es donde se verían. **No los produce.**

**`gap20000` = `0` filas: NO COMPUTADO con su razón** — en esa ventana las filas con `leg_amounts_out` y pools dentro del tope de 8 no existen. **No es «no hay flips»: es «no hay filas que re-cotizar».**

---

## 8. LA NO-EXPIRACIÓN, MEDIDA — no asumida

`opportunities` **no expira nada**. Todo lo que entra se queda y sigue en la tabla. Y ahora se mide **en tiempo real**, sin el supuesto de 12 s/bloque:

```
H2 = 9984897 | 24.471 | 69.465 | 85.462
     n=9.984.897 · p50=24,471 h · p90=69,465 h · max=85,462 h (3,56 días)

H2_umbrales = 9739635 | 8863707 | 5103637 | 2535001 | 9988293
              >1 h       >6 h       >24 h      >48 h      total
```

| Antigüedad | filas | % |
|---|---|---|
| > 1 hora | 9.739.635 | 97,51 % |
| > 6 horas | 8.863.707 | 88,74 % |
| **> 24 horas** | **5.103.637** | **51,11 %** |
| **> 48 horas** | **2.535.001** | **25,38 %** |

**Y la magnitud en bloques** (`V4`, `V5`, exit=0): `p50=26142027 · p90=26147982 · max=26149405`; filas con brecha `> 5.000` bloques = **`6.489.649` = `64,30 %`** del total.

**⇒ Este es el instrumento que faltaba y es la magnitud del problema: la MEDIANA de la tabla tiene 24,5 horas y una de cada cuatro filas pasa de 48 horas.** No es una cola: es la mayoría. Y encaja con §6: **la degradación ya es total a los 10 bloques (~2 min), así que el 88,74 % de la tabla describe estado que murió hace más de 6 horas.**

### 8.1 La no-expiración, medida con el tiempo real, corrige el supuesto de t176

t176 dio **p50 25,0 h** y **max 82,6 h** usando **`12 s/bloque` como supuesto declarado**. Con `now() − detected_at` —sin supuesto— son **`24,471 h`** y **`85,462 h`**. **El supuesto de t176 era bueno (error 2 %) y ahora está medido**, no estimado.

### 8.2 Y la proporción >5.000 bloques BAJA (69,51 % → 64,30 %) — no porque el problema mejore

`6.489.649` sobre `10.092.737`. En t176 eran `4.706.661` sobre `6.775.692` = **69,51 %**. **Las filas viejas no se van: la tabla crece por delante y la fracción se diluye.** El numerador crece; el denominador crece más rápido. **Una fracción decreciente sobre una cola que nunca se vacía no es una mejora.**

---

## 9. RESPETO DEL TECHO DEL INSTRUMENTO Y DE LA COBERTURA

- **`1.331` pools NO COMPUTADOS con su razón** (t175 barrió **5 de 1.336 = 0,37 %**; t177 los está barriendo). **Esta tarea NO cierra ese hueco y no presenta el NO como universal.**
- **El hallazgo de antigüedad es de OTRA población**: las **oportunidades** (`10,09 M` filas), **no los pools**. Que la tabla sea vieja **no extiende la cobertura de pools de 5 a 1.336.**
- **`quote_block` y `gross`/`net` viven en el JSONB `economics`, NO son columnas planas.** El `verify` del contrato lo confirma: `SELECT count(*), count(quote_block) …` → **`ERROR: column "quote_block" does not exist`, exit=1** (**defecto #14, reproducido**).
- **`fail_reason` va PREFIJADO.** `LIKE 'reverted:%'` → **`3413`** (control positivo, exit=0). Nunca `IN (...)`.
- **El techo de mercado vive en `opportunities.rejection_reason`**, no en `simulations.fail_reason`.
- **`revert_risk_pct` es el campo de rastro**, no `raw_trace`.
- **`serve` externo `:9090` → `000`/exit 7** (**defecto #9, décima aparición**). Prometheus por **loopback via ssh**, exit=0.
- **La frontera es `docker inspect StartedAt`, no `deploy.at`** → `2026-10-08T14:16:07.1195963Z`, **sin cambio en la 4.ª lectura**.
- **Una ausencia en la ventana NO es un cambio de estado.** Corroborado con un segundo instrumento: **`cache_hit` `1191 → 4907`** y **`slot_unresolved` `1 → 23`** ⇒ **el proceso vive y acumula; no hubo reinicio.** Y `slot_unresolved` **sigue apareciendo** — la corrección de t175 se respeta (la etiqueta existe; sólo bajó la tasa).

---

## 10. NO SE ESTIMA «CUÁNTO SE PODRÍA GANAR»

**No es computable con lo medido**: depende de frecuencia, competencia e impacto acumulado. **Ni una cifra de ganancia aparece en este artefacto.** Se responden las preguntas binarias y se dan los conteos:

| Pregunta binaria | Conteo |
|---|---|
| ¿El detector decide sobre estado viejo **al detectar**? | **NO** — cache TTL 17–23 s, `blk` = cabeza ±1 |
| ¿La **fila** guardada sobre-estima el edge? | **SÍ** — `1.043` de `1.061` (98,31 %) caen |
| ¿Sobrevive alguna **neta** a re-cotizar? | **`0` validadas.** `13` con bruto ≤0,16 % en filas cuyo notional se contradice; `5` con bruto >1 % imposible |
| ¿Aparece un candidato **nuevo** desde el rechazo? | **NO** — `0` de `373` |
| ¿`quote_block` es una cotización? | **NO** — `≡ block_number` en el 100,0000 % de las filas con valor |
| ¿Hay un tercer camino? | **SÍ** — `economics.rs:524-525`; y un cuarto sin explicar (`10.784`) |
| ¿La identidad es universal? | **No en el denominador**: `27,48 %` está vacío por diseño, no por desacuerdo |

**Y no se declara el techo de detección movido por comparar ventanas distintas** — el error que t175 documentó al rechazar `587,5/h` en 49 s contra `1287,7/h` en 425 s. Aquí no se compara ninguna tasa.

---

## 11. CONTROLES — todos ejecutados, con su exit code literal

| Control | Resultado |
|---|---|
| **Canal** `SELECT 1` | `1`, **exit=0** |
| **Negativo** `SELECT esto_no_existe` | **exit=1** + `ERROR: column "esto_no_existe" does not exist` — **sin tubería** |
| **Control positivo del `LIKE`** | `LIKE 'reverted:%'` → **`3413`**, exit=0 (en t176 fueron 861; la familia crece) |
| **`fee()` vs cache, 29/29** | `fee()` **revierte** en los 23 pools con entrada V2 y **devuelve tier** en los 6 con `slot0`. **Discriminador independiente que coincide 23+6=29** |
| **`token0()` vs `cache.token0_addr`** | **23/23 V2 coinciden** |
| **Instantánea coherente del cache** | mismo `blk` antes y después de leer los 29 pools (probe4) y los 33 (L2) |
| **CERO-GAP / error vs edad** | ver abajo — **el control que valida el motor** |
| **Hipótesis nula a estado fresco** | `gap=1`: `0` flips de `61` · `gap=5`: `0` de `117` |
| **Ningún control pipeado a `head`** | los tres, sin tubería |
| **Conteo numerado** | todo por `count(*)`; `GROUP BY` siempre con `NULLS LAST` |

### 11.1 EL CONTROL QUE VALIDA EL MOTOR: curva de error contra la salida guardada

No se puede leer el fork «a bloque 0 de brecha» cuando el cache avanza cada 12 s. **Se mide la curva y se extrapola al origen:**

| Brecha | comparables | **error relativo máx** | error p50 |
|---|---|---|---|
| 0 | 0 | — | — |
| **1** | 23 | **`5,148e-10`** | `5,148e-10` |
| **5** | 23 | **`5,148e-10`** | `5,148e-10` |
| 50 | 23 | `4,747e-04` | `4,747e-04` |
| 500 | 23 | `1,452e-04` | `1,452e-04` |
| 5.000 | 0 | — | — |

**⇒ A 1 y 5 bloques de brecha, mi re-cotización reproduce la salida guardada del motor con error relativo `5,148e-10` — nueve dígitos significativos.** Eso valida **las dos fórmulas** (V2 `dy=(dx·997/1000·y)/(x+dx·997/1000)` y V3 en las dos direcciones) **y** la lógica de dirección por `token0`. **Y el crecimiento del error con la brecha (`5e-10 → 4,7e-4`) ES la decadencia del estado, medida contra la propia salida del motor como referencia.**

**Cambio de instrumento declarado, no escondido:** el `GROUP BY` por razón sobre el JSONB agotó el timeout sobre ~10 M casts; se sustituyó por `block_number`. Y el primer `H5` devolvió `0` en cuatro buckets por filtrar `leg_amounts_out` — **un cero causado por el filtro, no por el mercado**; se corrigió reconstruyendo la ruta de inmutables y se declara (§7.1).

---

## 12. DEFECTOS DEL CAPITÁN REPRODUCIDOS

| # | Defecto | Evidencia |
|---|---|---|
| **#14** (nuevo) | El `verify` pide `SELECT count(*), count(quote_block) …` — **no existe como columna plana** | `ERROR: column "quote_block" does not exist`, **exit=1** |
| **#9** (décima aparición) | `:9090` externo | `external_9090_http_code=000`, **exit=7** |
| **#13/#14** misma clase | `quote_block` y `gross`/`net` viven en el JSONB `economics` | ruta JSONB obligatoria en todo el artefacto |
| **#12** | `fail_reason` va prefijado | `LIKE 'reverted:%'` → `3413`; `IN (...)` daría otra cosa |
| **#11** | El corte es `StartedAt`, no `deploy.at` | `2026-10-08T14:16:07.1195963Z` |
| **#8** | `raw_trace` vacío en positivos | el campo de rastro es `revert_risk_pct` |

**Defectos NUEVOS de esta tarea, no del capitán:**
- **El fork `anvil` no avanza y no lo dice.** `26148216` en tres lecturas >2 h, mientras la cabeza iba de `26148485` a `26149405`. **Todo consumidor que lea «estado vivo» del fork mide 3,2 h de historia.**
- **El cache V3 no lleva `blk`** (429 claves): el precio V3 usado no es atribuible a un bloque.
- **`opportunities` no expira** y no marca nada como caducado: la mediana de lo servible tiene 24,5 h.
- **El rechazo mayoritario no guarda su cotización por pierna** (`0` de `283.519`).
- **`amount_in_usd` vs `amount_in_wei` se contradicen** en `57` de `1.096` filas rentables (5,2 %).
- **Filas con `block_number` NULO** en la población rentable (encontrado por un `TypeError`): la clave de antigüedad puede faltar.
- **Riesgo de carrera en el instrumento**: leer el cache de N pools tarda más que su TTL si N es grande. Con N=33 la generación avanza durante la lectura. **Declarado: obliga a leer el `blk` antes y después y a exigir igualdad.**

---

## 13. HUECOS ABIERTOS — declarados, NO cerrados

1. **`1.331` pools sin barrer** (t175 cubrió `5` de `1.336`). Tarea con forma propia (t177).
2. **El cuarto camino** (`10.784` filas con cifras y `quote_block` vacío) — **NO EXPLICADO**. §4.4.
3. **`gap=0` y `gap≥5.000` en la curva de error**: `0` comparables. **NO COMPUTADO con su razón**, no «sin error».
4. **`gap20000` en la población rechazada**: `0` filas re-cotizables.
5. **La contradicción `amount_in_usd`/`amount_in_wei`**: medida su frecuencia (`57/1.096`), **no su causa**. Requiere `backend/`, fuera de alcance.
6. **Ninguna propuesta de arreglo se implementa aquí.** `backend/`, `shared-rs/`, `docker/`, `contracts/` están fuera de alcance. **Medir no autoriza a cambiar.**

---

## 14. ALCANCE, Y LO QUE NO SE HIZO

- **CERO cambios al motor, umbrales, configuración, fork o producción.** `capital_usd` sigue en **`1000.00`**, el target en **`50.0`** y el multiplicador en **`3.0`**.
- **CERO escrituras al fork.** Todas las lecturas de cadena son `eth_call` (`cast call`, `cast block-number`). El fork es **compartido y vivo** y **no se tocó**: ni un `setStorageAt` ni un `anvil_*` de escritura.
- **CERO mainnet, firmas o broadcast.** No se usó ninguna RPC de mainnet: el estado vivo salió del **cache interno** (`arbx:pool_reserves:1:*` / `arbx:v3_slot0:1:*`), que es el que **el propio detector** usa.
- **No se re-disparó el benchmark.** Paper.

**Publicación:** `git add` normal · rama `docs/live-state-01` · PR en **DRAFT** · integridad por `git hash-object` · **nunca `Out-File`**.
