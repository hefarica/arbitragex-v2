# POOL-UNIVERSE-01 — el cuarto techo, separado por MECANISMO

**Cápsula ORGDNA:** `run_id=arbx-entrega-20261008` · `phase_id=implementation` · **Dueño:** Data · **Intento:** `bb46eec2-2831-48d0-a6ed-9dd4ac299443`
**Alcance declarado:** **chain 1 sola** (todo el universo medido está en chain 1), ventana **24 h** para rechazos, instantáneas fechadas abajo · **In scope:** `docs/data/` · **Out of scope:** `backend/`, `shared-rs/`, `docker/`, `contracts/`, `.github/`, `frontend/`, `docs/backend/`, `docs/release/`, `docs/sre/`, `docs/review/`, `docs/spec/`
**Permisos:** SOLO LECTURA sobre el VPS y el repo. **Cero cambios de umbrales, cero cambios de configuración, cero escrituras en el motor.** El único path escrito fue `docs/data/POOL-UNIVERSE-01.md`.

---

## 0. Control de instrumento — y DOS defectos míos, declarados

| control | comando | salida literal | instante |
|---|---|---|---|
| **canal VPS→Postgres** | `docker exec arbitragex-v2-postgres-1 psql -U postgres -d arbitragex -tAc "SELECT 1"` | **`1`** | 2026-10-08T07:02:12Z (y repetido en cada sonda) |
| redis | `docker exec arbitragex-v2-redis-1 redis-cli XLEN arbx:opps:detected` | **`10001`** | 2026-10-08T07:07:46Z |

**El canal devolvió fila antes de cada cero de este documento.** Ningún cero de acá es un canal mudo.

**Defecto 1 (propio, corregido):** escribí `'con_tvl>1e9='||count(*)||...` **sin `FILTER`** en el primer contador — etiquetaba `>1e9` y contaba **el total** (`4215`). Re-medido con `FILTER` correcto: `tvl_gt_1e9=19`, `tvl_gt_1e8=50`, `tvl_gt_1e6=143`. **El número equivocado era plausible y cerraba; por eso lo declaro.**

**Defecto 2 (propio, corregido):** `redis-cli XLEN arbx:opps:detected` pasado como **una sola cadena** (`$R "XLEN …"`) devuelve `ERR unknown command 'XLEN arbx:opps:detected'`. **Eso no era un cero: era un instrumento roto.** Re-medido con argumentos separados → `10001`.

---

## 1. El universo, RE-MEDIDO (no heredado)

```sql
SELECT 'total='||count(*)||' activos='||count(*) FILTER (WHERE is_active) FROM pools;
-- total=4215 activos=1313
```
```sql
SELECT 'chain='||chain_id||' total='||count(*)||' activos='||count(*) FILTER (WHERE is_active) FROM pools GROUP BY chain_id;
-- chain=1 total=4215 activos=1313
```

**Contraste con t123 (1 305 activos de 4 207, chain 1, ventana 24 h):**

| cifra | t123 declaró | re-medido (2026-10-08T07:02:31Z) | ¿coincide? |
|---|---|---|---|
| pools totales | 4 207 | **4 215** | **NO** (+8) |
| pools activos | 1 305 | **1 313** | **NO** (+8) |
| cadenas | chain 1 sola | **chain 1 sola** (`GROUP BY chain_id` da una sola fila) | **SÍ** |

**No coinciden, y la diferencia es la esperable: el universo creció 8 pools entre la medición de t123 y la mía.** El orden de magnitud y la composición (100 % chain 1) sí se confirman. Todo lo que sigue usa **mis** cifras, con su instante.

---

## 2. Los dos números del operador: **NO COMPUTADOS**

`1 847` asignados a un `LiquidityAgent (M07)` y `1 055` a `Quant (M03)`: **NO COMPUTADOS**, y no por falta de esfuerzo sino por admisión de la fuente — el operador confirmó textualmente que **la fuente no existe y las inventó**.

Lo que aporto como evidencia propia, no como eco:

```bash
# grep sobre el árbol del repo (rs/ts/tsx/md/json/sql/yml/yaml/toml)
grep -E 'LiquidityAgent|M07|M03|enum_source|"Quant"|QuantAgent'
# -> 0 coincidencias para LiquidityAgent, M07, M03 y Quant.
#    Los únicos hits reales son de `enum_source` (y un falso positivo en un hash de package-lock).
```

**Los identificadores `M07`/`M03` y los agentes `LiquidityAgent`/`Quant` NO existen en el repo.** El único eje de procedencia por pool que sí existe es **`enum_source`** (migración `095_pool_enum_gate.sql`, que documenta literalmente `SELECT enum_source, is_active, COUNT(*) FROM pools GROUP BY enum_source, is_active`), y su partición **no reproduce** esas cifras:

| `enum_source` | total | activos |
|---|---|---|
| `alchemy` | 2 842 | 33 |
| `reactive` | 1 210 | **1 208** |
| `subgraph_tvl` | 75 | 1 |
| `seed` | 60 | **60** |
| `dexscreener` | 28 | 11 |

**Ninguna combinación de estos cinco valores da 1 847 ni 1 055.** La aritmética del operador cerraba porque estaba construida para cerrar; **consistencia no es evidencia**.

---

## 3. Los TRES MECANISMOS, por separado

### (i) Pools que existen y el motor **NO DESCUBRE** — proxy medido: **6 666 pares** y **39 de 43 factories vacías**

| medición | comando | salida literal | instante |
|---|---|---|---|
| pares vistos sin pool indexado | `SELECT 'observed_sin_resolver='\|\|count(*) FROM observed_unindexed_pairs WHERE NOT is_resolved` | **`observed_sin_resolver=6666`** | 2026-10-08T07:08:13Z |
| factories registradas | `SELECT count(*) FROM factories` | **43** | 2026-10-08T07:03:13Z |
| **factories con CERO pools** | `SELECT count(*) FROM factories f WHERE NOT EXISTS (SELECT 1 FROM pools p WHERE p.factory_id=f.id)` | **39** (90,7 %) | 2026-10-08T07:08:13Z |

**Dos señales independientes y ambas verificables.** La segunda es la más limpia del mecanismo: **de 43 factories que el motor tiene registradas, 39 nunca produjeron un solo pool.** Ese no es un problema de indexación ni de admisibilidad: es descubrimiento que no llega.

*Naturaleza declarada del 6 666:* es un **proxy**. `observed_unindexed_pairs` cuenta pares vistos por la vía de eventos observados, **no** un censo on-chain. Un recuento on-chain completo daría otro número y **NO está medido**.

### (ii) Pools descubiertos que **NO SE INDEXAN** — **2 902 de 4 215** (68,9 %)

Están EN la base, con fila propia, y quedan fuera del universo evaluable por el filtro `is_active = TRUE`:

| medición | salida literal | instante |
|---|---|---|
| `is_active=false` | **`n=2902`** | 2026-10-08T07:03:13Z |
| = total − activos | `4215 − 1313 = 2902` ✔ | — |

**Y el mecanismo tiene un dueño claro, medido:** el 96,8 % de esa masa viene de **una sola fuente de descubrimiento**:

| `enum_source` | inactivos | activos |
|---|---|---|
| **`alchemy`** | **2 809** | 33 |
| `subgraph_tvl` | 74 | 1 |
| `dexscreener` | 17 | 11 |
| `reactive` | 2 | **1 208** |
| `seed` | 0 | **60** |

**2 809 de 2 902 (96,8 %) son `alchemy`.** `reactive` y `seed` casi no dejan residuo (2 y 0). No es un problema difuso de indexación: es **un camino de descubrimiento que inserta y no activa**.

### (iii) Pools indexados que se **RECHAZAN por admisibilidad** — ventana 24 h

Universo de la ventana: `SELECT 'opportunities_24h='||count(*) FROM opportunities WHERE detected_at > now() - interval '24 hours'` → **`opportunities_24h=3989076`** (2026-10-08T07:06:25Z). **El 100 % tiene `rejection_reason` no nulo** (`con_rechazo=3989781`, `sin_rechazo=0` a las 07:03:13Z).

**Clasificación ECONOMICO vs NO EVALUADO, calculada en SQL (la aritmética es de la base, no mía):**
`ECONOMICO_evaluado=3473390 · NO_EVALUADO_resto=515596 · total=3988986`

| familia del mecanismo (iii) | n (24 h) | instante |
|---|---|---|
| `single_pool_no_spread` (par sin contraparte) | **175 555** | 2026-10-08T07:06:25Z |
| `v3_pool_not_catalogued` (pool fuera del catálogo del encoder) | **157 279** | ídem |
| `StrategyDisabled:*` (estrategia apagada) | **82 926** | ídem |
| `v3_quote_unavailable` (no se pudo cotizar) | **36 109** | ídem |
| `v3_pair_no_pools` (par sin pools) | **27 548** | ídem |
| `TokenNotAllowed:*` (token fuera de la allowlist) | **17 800** | ídem |
| `v3_pool_revert` | **1 946** | ídem |
| (resto de la familia NO EVALUADA: `no_tradable_size`, `v3_multileg_budget_exhausted`, otros) | ~16 433 | ídem |
| **suma NO EVALUADA** | **515 596** | ídem |

### Los tres, juntos y separados

| mecanismo | magnitud medida | unidad | arreglo |
|---|---|---|---|
| **(i) no descubre** | **6 666** pares (proxy) · **39/43** factories vacías | pares / factories | camino de descubrimiento |
| **(ii) no indexa** | **2 902** pools (96,8 % `alchemy`) | pools | activación / gate de indexación |
| **(iii) rechaza por admisibilidad** | **515 596** oportunidades en 24 h (**12,93 %** de 3 988 986) | oportunidades | catálogo del encoder, allowlist, estrategia |

**No se suman.** Son unidades distintas (pares, pools, oportunidades) y arreglos distintos. Un total único de «pools faltantes» no habría dicho cuál de los tres tocar.

---

## 4. La pista de t79, RE-MEDIDA

| t79 reportó | re-medido hoy | comando | instante |
|---|---|---|---|
| **6 017** pares observados y nunca indexados | **6 666** sin resolver (de 7 587 filas, 921 resueltas) | `SELECT 'no_resueltos='\|\|count(*) FILTER (WHERE NOT is_resolved)\|\|' resueltos='\|\|count(*) FILTER (WHERE is_resolved) FROM observed_unindexed_pairs` | 2026-10-08T07:04:34Z |
| **25,40 %** rechazado sin llegar a evaluar | **12,93 %** (515 596 de 3 988 986) | ver §3(iii) | 2026-10-08T07:06:25Z |

**El 6 017 quedó obsoleto por crecimiento, y lo puedo decir con la tendencia:** 6.017 (t79) → 6.203 (t84) → **6.666** (hoy, +649 sobre t79). El caudal es **vivo pero pequeño**: `vistos_ultimas_24h=64` y `nuevos_ultimas_24h=52`, con `max_last_seen=2026-10-08 07:01:27.079805+00` (≈3 min antes de la medición). No es una cola congelada.

**El 25,40 % NO lo puedo reconciliar y no lo voy a maquillar.** Mi 12,93 % es de otra base (3 988 986 vs 1 732 431 de t79), de otro instante, y con **mi** criterio de clasificación (económico = `spread_negative_round_trip` + `non_positive_profit`), que no tiene por qué ser el de t79. Doy el desglose por familia para que cualquiera lo recompute con el criterio que prefiera; **el criterio de t79 no está a mi alcance y por eso la diferencia queda sin explicar, no explicada.**

---

## 5. ¿Esto CONDICIONA la economía ya medida?

Los números económicos vigentes (`spread_negative_round_trip`; `gross_profit_usd > 0` = 0 sobre 10 000 entradas; hurdle de round trip **59,91 bps**) están medidos **sobre el universo que PASA el filtro**. La pregunta honesta es si el universo FALTANTE podría contener dislocación que exceda el hurdle. **Se contesta en dos partes, y la segunda es NO COMPUTADA.**

### 5.1 Lo que SÍ se puede acotar: el techo de lo que el índice faltante podría añadir

Contrafactual medido — un viaje de ida y vuelta cross-venue necesita **≥2 pools del mismo par**:

```sql
WITH pares AS (SELECT token0_id, token1_id,
                      count(*) FILTER (WHERE is_active) AS n_act,
                      count(*) FILTER (WHERE NOT is_active) AS n_inact
               FROM pools GROUP BY token0_id, token1_id)
SELECT 'pares_totales_distintos='||count(*)||' con_>=2_ACTIVOS='||count(*) FILTER (WHERE n_act>=2)
    ||' con_>=2_INACTIVOS='||count(*) FILTER (WHERE n_inact>=2)
    ||' con_>=2_EN_LA_UNION='||count(*) FILTER (WHERE n_act+n_inact>=2) FROM pares;
```
→ **`pares_totales_distintos=3753 con_>=2_ACTIVOS=199 con_>=2_INACTIVOS=39 con_>=2_EN_LA_UNION=240`** (2026-10-08T07:08:13Z)

```sql
... SELECT 'hoy_ya_tenian_>=2='||count(*) FILTER (WHERE n_act>=2)
        ||' pasarian_a_>=2_solo_por_inactivos='||count(*) FILTER (WHERE n_act<2 AND n_act+n_inact>=2) FROM pares;
```
→ **`hoy_ya_tenian_>=2=199 pasarian_a_>=2_solo_por_inactivos=41`**

**El techo del cuarto techo, medido: indexar los 2 902 pools llevaría el universo con capacidad cross-venue de 199 a 240 pares — +41 pares (+20,6 %).** No es «otro orden de magnitud»: es un quinto más de pares, y esos 41 son el universo ENTERO que el índice faltante podría habilitar. La mayoría de los 2 857 pares con pool inactivo son **pares de un solo pool**: sólo **5** tienen pool en ambos lados hoy, y sólo **39** tienen ≥2 inactivos.

**Esto también dice algo duro sobre el mecanismo (iii):** `single_pool_no_spread` = **175 555** rechazos en 24 h. No es un fallo de admisibilidad que se arregle aflojando un umbral: es que **el par no tiene contraparte**. Aflojar el gate no crearía ni una pata.

### 5.2 Lo que NO se puede medir: la dislocación dentro de esos 41 pares → **NO COMPUTADO**

**La razón es medida, no supuesta:** la tabla que permitiría calcular un round trip es `pool_reserves`, y **no cubre ni un solo pool fuera del universo activo**:

| medición | comando | salida literal | instante |
|---|---|---|---|
| pools con reservas | `SELECT 'pool_reserves_distinct_pools='\|\|count(DISTINCT pool_id) FROM pool_reserves` | **877** | 2026-10-08T07:04:34Z |
| **reservas de pools inactivos** | `SELECT 'reservas_de_inactivos='\|\|count(*) FROM pool_reserves r JOIN pools p ON p.id=r.pool_id WHERE NOT p.is_active` | **`reservas_de_inactivos=0`** | 2026-10-08T07:06:25Z |
| cobertura por actividad | `SELECT 'activo='\|\|p.is_active\|\|' pools_con_reservas='\|\|count(DISTINCT r.pool_id) … GROUP BY p.is_active` | **`activo=true pools_con_reservas=877`** (una sola fila: no existe el grupo `false`) | 2026-10-08T07:04:34Z |

**Cero reservas para pools inactivos ⇒ no hay una sola cotización computable sobre el universo faltante.** La tabla tiene 42 697 848 filas y 4 868 341 de las últimas 24 h — está viva y es densa **dentro** del universo activo, y es **vacía** fuera de él. La limitación no es de esfuerzo: **el motor nunca registró las reservas de esos pools.**

**Y el proxy que uno querría usar en su lugar tampoco se sostiene.** `tvl_usd` de los inactivos suma 63 596 563 667,77 contra 47 426 862,29 de los activos — pero eso **no** es «el universo faltante tiene 1 341× la liquidez»:

| medición | activos | inactivos | instante |
|---|---|---|---|
| n | 1 313 | 2 902 | 2026-10-08T07:06:25Z |
| suma `tvl_usd` | 47 426 862,29 | **63 596 563 667,77** | ídem |
| suma **sin los top-10** | 47 426 862,29 | 26 153 085 073,12 | ídem |
| **mediana `tvl_usd`** | **25 299,15** | **0,00** | ídem |
| `tvl_usd IS NULL` / `= 0` | — | **1 345 / 1 562** | ídem |
| con `volume_usd_24h > 1000` | **31** | **1 531** | ídem |

Los dos primeros pools por TVL son `act=false src=alchemy tvl=7 535 635 023,20 vol24=0,34` y `tvl=6 073 530 633,54 vol24=0,34`: **miles de millones de TVL con 34 centavos de volumen en 24 h.** Ese `tvl_usd` no está corroborado por su propio volumen y no se sostiene como medida de liquidez. La mediana del pool inactivo es **0,00**.

**Dirección, con lo medido y sin intuición:** el universo faltante **no** se comporta como un tesoro de liquidez escondido (mediana 0,00; TVL outlier-driven y autocontradictorio; **cero** reservas registradas). Lo que sí tiene es **volumen registrado**: 1 531 pools inactivos con >1000 USD en 24 h, agregando 29 996 924,94 — comparable a los 28 666 998,31 del universo activo (que lo logra con sólo 31 pools). **Hay actividad ahí; lo que no hay es con qué medir su dislocación.** Que esa actividad contenga o no un round trip > 59,91 bps **es exactamente lo que no se puede contestar con los datos registrados**, y por eso la respuesta es **NO COMPUTADO**, no un sí ni un no.

---

## 6. Puntos NO COMPUTADOS, con la evidencia exacta que los cerraría

| # | Qué no está computado | Por qué (medido) | Qué lo cerraría |
|---|---|---|---|
| 1 | **1 847** (`LiquidityAgent M07`) y **1 055** (`Quant M03`) | El operador confirmó que **la fuente no existe**; grep del repo: **0 coincidencias** de esos identificadores; `enum_source` (único eje real) no los reproduce | Una tabla/columna real de asignación por agente, o el artefacto que las produjo. **No existe ninguna hoy.** |
| 2 | **Dislocación del universo faltante** (¿supera 59,91 bps?) | `reservas_de_inactivos=0` sobre 42 697 848 filas de `pool_reserves`: **no hay una sola cotización** para esos pools | Reservas/`slot0`/ticks para los 41 pares con ≥2 pools que hoy quedan fuera — p. ej. extender el escritor de `pool_reserves` a pools no activos, o un muestreo on-chain declarado (bloque + lista de pools) |
| 3 | **Censo on-chain real del mecanismo (i)** | `observed_unindexed_pairs` (6 666) es un **proxy** por eventos observados, no un censo | Un recuento de pools por factory vía RPC (`PoolCreated` logs) sobre una ventana declarada, contra la tabla `pools` |
| 4 | **El 25,40 % de t79** | Mi 12,93 % es de otra base (3 988 986 vs 1 732 431), otro instante y **mi** criterio de clasificación | El criterio de clasificación de t79 (qué familias contó como «sin evaluar»). Sin eso, la diferencia queda **sin explicar**, no explicada |
| 5 | **Por qué `alchemy` inserta sin activar** (2 809 pools) | Medido **que** ocurre (96,8 % del mecanismo ii), **no** por qué | El gate de activación de ese camino (`pool_enumeration_worker` / migración 095) y su condición de activación |

---

## 7. Alcance, y el bloqueo de publicación

- **Chain 1 sola** — confirmado por `GROUP BY chain_id`, que devuelve **una sola fila** (4 215 total / 1 313 activos).
- **Ventanas declaradas, cada cifra con su instante.** Instantáneas: 07:02:12Z · 07:02:31Z · 07:03:13Z · 07:04:34Z · 07:06:25Z · 07:07:46Z · 07:08:13Z (todas del 2026-10-08). Las tablas vivas (`opportunities`, `observed_unindexed_pairs`) crecen entre sondas; por eso el 24 h se cita con hora.
- **Solo lectura**: únicamente `SELECT`, `redis-cli XLEN` (lectura) y consultas de `information_schema`. **Cero cambios de umbrales, cero configuración, cero escrituras en el motor.**
- **Bloqueo de publicación declarado:** en el checkout compartido `git check-ignore` sobre `docs/data/` devuelve **`.gitignore:73:data/`** con exit 0 ⇒ **`docs/data/` está git-ignored** (precedente t119). No es regresión de t52: ese checkout está en `858b943f` del 2026-09-26, **anterior** al merge de #818. **No modifiqué `.gitignore`** (fuera de alcance). El artefacto queda escrito en `docs/data/`; publicarlo desde este árbol requiere **`git add -f`** o un árbol al día.

---

## 8. Reproducción

```bash
PG='docker exec arbitragex-v2-postgres-1 psql -U postgres -d arbitragex -tAc'
$PG "SELECT 1"                                                   # CONTROL DE CANAL
$PG "SELECT count(*)||' | '||count(*) FILTER (WHERE is_active) FROM pools"
$PG "SELECT enum_source, count(*), count(*) FILTER (WHERE is_active) FROM pools GROUP BY 1 ORDER BY 2 DESC"
$PG "SELECT count(*) FROM observed_unindexed_pairs WHERE NOT is_resolved"
$PG "SELECT count(*) FROM pool_reserves r JOIN pools p ON p.id=r.pool_id WHERE NOT p.is_active"
WITH_CTE="WITH pares AS (SELECT token0_id, token1_id, count(*) FILTER (WHERE is_active) AS n_act, count(*) FILTER (WHERE NOT is_active) AS n_inact FROM pools GROUP BY token0_id, token1_id) SELECT count(*) FILTER (WHERE n_act>=2), count(*) FILTER (WHERE n_act<2 AND n_act+n_inact>=2) FROM pares"
$PG "$WITH_CTE"
docker exec arbitragex-v2-redis-1 redis-cli XLEN arbx:opps:detected   # argumentos SEPARADOS
```

---

*Un techo que se suma como si fuera uno no dice cuál de los tres arreglar. Acá van separados: **6 666 pares y 39/43 factories** que no se descubren, **2 902 pools** (96,8 % `alchemy`) que no se indexan, **515 596 oportunidades en 24 h** (12,93 %) que se rechazan por admisibilidad. Y el techo de lo que el índice faltante podría añadir está medido: **+41 pares**. Lo que sigue sin poder medirse — la dislocación dentro de esos 41 — se declara **NO COMPUTADO**, con la razón exacta: **cero reservas registradas** para los pools que quedan fuera.*

---

## 9. Integridad

El `sha256` de este documento se declara **en el cierre de t135** (mensaje al capitán y `output` de la tarea): un archivo **no puede contener su propio hash** sin cambiar ese hash. Se declara ahí en vez de fabricar una línea autoconsistente.

Verificación (sintaxis pwsh de este host: `curl` es alias de `Invoke-WebRequest` y `sed` no está en el PATH):

```powershell
Get-ChildItem -Recurse -File docs/data | ForEach-Object { "{0}  {1}  {2}" -f (Get-FileHash $_.FullName -Algorithm SHA256).Hash, $_.Length, $_.FullName }
```
