# LIVE-EMITIDO-01 — qué se emite HOY: el techo activo es `candidate_incomplete` (92,1 %), el funnel está **ABIERTO**, y los dos productores nuevos **no existen en PG**

**Resultado en una línea:** con corte temporal en **toda** cifra, lo que se emite hoy son **tres cosas** y sólo tres: **`candidate_incomplete:amount_in_wei_zero` 8.076/h (92,1 %)**, `sim_signer_funding_slot_unresolved` **693/h** y `funding_balanceof_timeout` **2/h** — y la suma da **8.771 = el `vivo_1h` exacto**. **El funnel NO está cerrado: 695 oportunidades distintas alcanzaron S4 en la última hora** (t158 midió el pasado; esto mide el presente). **El techo activo es `candidate_incomplete`, con 0 % de rastro ⇒ hueco de capacidad, y es 11,7× la familia del fondo.** Y **`cyclic_route_missing_route_metadata` y `route_path_not_representable` — que `main` documenta como los productores de HOY — tienen CERO filas, ni históricas.**

**Cápsula ORGDNA:** `run_id=arbx-entrega-20261008` · **Dueño:** Data · **Intento:** `83c57b21-540d-46e8-a8b4-54448d829f7f`
**Solo lectura** · **CERO cambios al motor** · **CERO escrituras** · **In scope:** `docs/data/`

---

## 0. ★★ LA REGLA QUE ORDENA TODO, CUANTIFICADA

`SELECT count(*) FILTER (WHERE simulated_at > now() - interval '1 hour') || ' vivo_1h | ' || count(*) FILTER (WHERE simulated_at > now() - interval '15 minutes') || ' vivo_15min | ' || count(*) || ' total | max=' || max(simulated_at) || ' | min=' || min(simulated_at) FROM simulations` — **exit 0**, instante `2026-10-08 13:50:31.346292+00`:

```
8771 vivo_1h | 2243 vivo_15min | 701127 total | max=2026-10-08 13:50:30.158027+00 | min=2026-10-07 21:19:58.524132+00
```

**★ `8.771 / 701.127 = 1,25 %`.** Es decir: **un agregado sobre la tabla entera mezcla 98,75 % de épocas muertas con 1,25 % de sistema vivo.** Eso es *«el agregado miente»* de `docs/backend/SIM-FUND-02.md:70`, **cuantificado con mi comando**. Es exactamente la resta que el contrato pedía, y es el error que t158 cazó: priorizar 617.570 filas de una época cerrada mientras lo vivo son 8.771.

**Desde acá, toda cifra de este informe lleva su ventana en la misma línea.**

---

## 1. ★★ QUÉ SE EMITE HOY — la distribución completa, dos ventanas

### Ventana **1 HORA** (la distribución cubre el **100 %** de la ventana)

| `fail_reason` | **1 h** | % de la ventana |
|---|---|---|
| **`candidate_incomplete:amount_in_wei_zero`** | **8.076** | **92,08 %** |
| `sim_signer_funding_slot_unresolved` | **693** | 7,90 % |
| `funding_balanceof_timeout` | **2** | 0,02 % |
| | **Σ = 8.771** | **= `vivo_1h` exacto ✓** |

### Ventana **15 MINUTOS** (para que una hora no oculte un cambio reciente)

| `fail_reason` | **15 min** |
|---|---|
| **`candidate_incomplete:amount_in_wei_zero`** | **1.976** |
| `sim_signer_funding_slot_unresolved` | **267** |
| | **Σ = 2.243 = `vivo_15min` exacto ✓** |

**★ Las dos sumas cierran al entero contra el total de su ventana: la distribución es exhaustiva, no un top-N.** Sólo existen **tres** familias vivas, y en 15 minutos **dos**.

**Y el dato que ordena las prioridades de esta célula:** el techo **activo** es **`candidate_incomplete` = 92,1 %**, contra `slot_unresolved` **7,90 %** ⇒ **11,7×**. Es decir: **la familia del fondo (que es lo que t146/t156 vienen midiendo) es 1/11,7 de lo que se emite hoy.** No invalida ese trabajo — el contrato de t156 lo dice — pero **sitúa su tamaño**: el arreglo del fondo ataca el **7,9 %** del presente, no la mayoría.

---

## 2. ★★★ EL FUNNEL EN PRESENTE: **ABIERTO**, no cerrado

t158 midió `llegaron_a_S4_alguna_vez = 0` sobre **593.827** oportunidades distintas — **y eso era el pasado**, como el propio contrato advierte. La medición del presente:

| ventana | oportunidades distintas con rastro | filas con rastro | filas de la ventana |
|---|---|---|---|
| **1 hora** | **695** | **695** | 8.771 |
| **15 minutos** | **267** | **267** | 2.243 |

**⇒ 695 oportunidades distintas SÍ alcanzaron S4 en la última hora. El corte NO sigue abierto: el sistema simula.**

**★ Y el control de consistencia que lo cierra:** las filas con rastro de la ventana (**695**) son **exactamente** `sim_signer_funding_slot_unresolved` (**693**) + `funding_balanceof_timeout` (**2**) = **695** ✓. **Las únicas familias que simulan son las dos del camino del fondo, y su suma explica el 100 % del rastro de la hora.** Por construcción, `candidate_incomplete` (8.076) aporta **0**.

**★ Pero `passed = true` sigue en 0**: `passed_true_1h = 0 | passed_true_total = 0` sobre **701.127** filas, con corte. ⇒ **el sistema simula y ninguna simulación pasa.** Las dos cosas son ciertas a la vez y se declaran separadas: **que el funnel esté abierto no significa que el benchmark pase.**

---

## 3. ★★ EL DISCRIMINANTE, REUSADO (no reinventado) — con CONTROL en la ventana viva

El de t158, sin cambios: **rastro de evaluación presente o ausente**, por fila, sobre los seis campos (`raw_trace`, `gas_estimate_wei`, `gas_price_wei`, `slippage_pct`, `revert_risk_pct`, `simulated_profit_usd`) + `passed`.

**CONTROL OBLIGATORIO, corrido sobre la ventana VIVA (1 h):**

| familia | n (1 h) | **con rastro** | % |
|---|---|---|---|
| **`candidate_incomplete:amount_in_wei_zero`** | **8.076** | **0** | **0 %** |
| `sim_signer_funding_slot_unresolved` | **693** | **693** | **100 %** |
| `funding_balanceof_timeout` | **2** | **2** | **100 %** |

**⇒ El discriminante funciona en el presente igual que en el histórico: 100 % vs 0 %.** Sin este control, el 0 de `candidate_incomplete` no valdría; con él, **vale**. *(Y es la misma estructura que t158 validó sobre nueve familias — se reusa la propiedad, se re-mide el valor.)*

**⇒ `candidate_incomplete:amount_in_wei_zero` es un HUECO DE CAPACIDAD, y está VIVO.** El `max` de esa familia es **`2026-10-08 13:50:13`** — el instante de la medición. **No es un fósil: se está escribiendo ahora.**

---

## 4. ★★★ LOS DOS PRODUCTORES NUEVOS: **CERO FILAS — NI HISTÓRICAS**

El contrato los declara «el candidato» y pide medirlos. Medidos:

| consulta | salida literal | exit |
|---|---|---|
| `count(*) FILTER (WHERE fail_reason LIKE 'cyclic_route_missing_route_metadata%')` | **`0`** | 0 |
| `count(*) FILTER (WHERE fail_reason LIKE 'route_path_not_representable%')` | **`0`** | 0 |
| `count(DISTINCT fail_reason) FILTER (WHERE fail_reason LIKE 'cyclic%route%')` | **`0`** | 0 |
| `count(DISTINCT fail_reason) FILTER (WHERE fail_reason LIKE 'route_path%')` | **`0`** | 0 |
| `total_tabla` | **701.136** | 0 |

**★ Un `count(*)` devuelve un NÚMERO, no un vacío: ese `0` es un cero numerado, no un NULL ni una consulta vacía.** Y hay **control positivo del `LIKE`**: en la misma ventana viva, `LIKE 'candidate_incomplete%'` = **8.032** y `sim_signer_funding_slot_unresolved` = **8.734** ⇒ **el patrón funciona; el 0 es del dato.**

**★★ Y esto es un hallazgo propio, más fuerte que el que la tarea esperaba:** `persistence.rs:294-304` lista esas dos familias como **los productores ACTUALES** (`cyclic_route_missing_route_metadata:<kind>`, `route_path_not_representable:<kind>`), y `sim_engine.rs:76-90` documenta el cambio a ellas. **En PG no hay UNA SOLA FILA de ninguna de las dos, ni histórica.** Combinado con t158 (la familia vieja paró a las `00:08:50` y su productor no existe en `main`), la conclusión es que **ninguna familia «cíclica» está emitiendo nada** — ni la retirada, ni las nuevas. **El código de `main` describe un comportamiento que no se está produciendo.**

**NO COMPUTADO, y no se declara cero:** **por qué**. No medí si esas ramas no se alcanzan (ningún ciclo llega a `sim_engine`), si el deploy en curso las activará, o si se persisten con otro nombre. Son tres hipótesis y **ninguna medida** ⇒ se declaran como no computadas con su cierre.

---

## 5. ★ EL OTRO TECHO VIVO: `candidate_incomplete`, por variante

| | valor | ventana |
|---|---|---|
| **1 hora** | **8.076** | `simulated_at > now() - interval '1 hour'` |
| **15 minutos** | **1.976** | `… '15 minutes'` |
| **total** | **54.150** | tabla entera |
| **`max(simulated_at)`** | **`2026-10-08 13:50:13.887008+00`** | — |
| **variantes distintas** | **1** (`amount_in_wei_zero`) ⇒ **residuo = 0** | — |
| **con rastro** | **0** | 1 h |

**⇒ Es un hueco de capacidad ACTIVO y de UNA SOLA variante.** El contrato pedía separar por variante: **hay una sola**, `candidate_incomplete:amount_in_wei_zero` (t108 ya la había tocado). **Residuo = 0**: no hay variantes ocultas bajo ese prefijo.

**Esto es lo que t158 declaró NO COMPUTADO y ahora está medido: el techo que domina HOY es `candidate_incomplete`, no el fósil del S4 ni la familia del fondo.**

---

## 6. ★ INSTRUMENTO DEL CONTADOR — con dos instantes, y una DISCREPANCIA declarada

**Camino correcto, y el endpoint prohibido verificado:** `curl -s -o /dev/null -w 'http_code=%{http_code} content_type=%{content_type}\n' http://195.201.235.70/metrics` ⇒ **`http_code=404 content_type=text/html; charset=utf-8`** ⇒ es la consola Next.js, NO Prometheus. **Se usa la API de Prometheus en el host** (`curl -s 'http://localhost:9090/api/v1/query?query=arbx_sim_funding_total'`).

**Dos instantes, 45,03 s aparte:**

| `outcome` | **t1** `1791467453.096` | **t2** `1791467498.126` | **Δ** | **ritmo/h** |
|---|---|---|---|---|
| `verify_mismatch` | 9.440 | 9.496 | **+56** | **4.478** |
| `slot_unresolved` | 2.360 | 2.374 | **+14** | 1.119 |
| `balance_unreadable` | 1.340 | 1.345 | **+5** | 400 |
| **`rpc_err`** | **4** | **5** | **+1** | **80** |
| **TOTAL** | **13.144** | **13.220** | **+76** | **6.076** |

**★ Las CUATRO series crecen ⇒ el contador está vivo y las cuatro están activas. `rpc_err` pasó de 4 a 5: CUARTA serie confirmada en DOS instantes** (t156 la vio en 2, t158 en 2, acá crece) — y el contrato del capitán decía «tres series conocidas». **Y `funding_balanceof_timeout` NO es la misma clase que `slot_unresolved`**: en PG la primera es ×2 y la segunda ×693 en la misma hora, y `balanceof_timeout` es un **timeout de la llamada** mientras `slot_unresolved` es **la ranura sin resolver** — mismo camino, clase distinta, y el arreglo del calldata cubre uno.

**`process_start_time_seconds{job="sim-ctl"}` ⇒ `{"resultType":"vector","result":[]}` = VACÍO ⇒ NO COMPUTADO, no cero.**

### ★ DISCREPANCIA ENTRE INSTRUMENTOS, declarada y no maquillada

Los dos instrumentos **no reconcilian término a término**:
- **Contador** (ritmo/h): `verify_mismatch` **4.478** domina · `slot_unresolved` 1.119 · `balance_unreadable` 400 · `rpc_err` 80 ⇒ **6.076/h**.
- **Tabla** (1 h): `candidate_incomplete` **8.076** domina · `slot_unresolved` 693 · `balanceof_timeout` 2 ⇒ **8.771/h**. **Y `verify_mismatch` tiene CERO filas en la tabla, siendo la serie que más crece en el contador.**

**NO COMPUTADO: por qué `verify_mismatch` crece 4.478/h en el contador y aporta 0 filas a `simulations`.** Son instrumentos que cuentan cosas distintas (el contador cuenta **intentos de fondeo**; la tabla, **filas persistidas**), y eso **explica la dirección pero no el mapeo**. No se declara equivalencia ni se elige el número que conviene: **se dan los dos y se declara que no cierran.**

---

## 7. NO COMPUTADO Y LÍMITES

| # | NO COMPUTADO | razón |
|---|---|---|
| 1 | **Por qué las dos familias nuevas tienen 0 filas** | Tres hipótesis (rama no alcanzada / deploy pendiente / otro nombre de persistencia), **ninguna medida** |
| 2 | **El mapeo contador→tabla** (§6) | Dirección explicable, mapeo no medido |
| 3 | **Si `candidate_incomplete` crece o está estable** | Medí su valor en 1 h y 15 min (**8.076** y **1.976**, coherentes con régimen estable a ~8.000/h), pero **no dos instantes del mismo contador** para esa familia |
| 4 | **`candidate_incomplete` en el contador de Prometheus** | No existe una serie con ese nombre; su distribución se midió por PG |

**Límites:** `no_simulation_row` = **NO MEDIDA**, no fallo. **Un contador acumulativo vuelve a 0 al reiniciar** ⇒ los deltas de §6 valen **sólo entre t1 y t2** y no son comparables tras un deploy. **NO se reabre t156**: su predicción sigue esperando sus tres condiciones. **NO se disparó el benchmark** (se declara).

**Y un ajuste propio, declarado como pidió el contrato:** al armar las consultas de los dos productores nuevos usé primero `GROUP BY fail_reason` **sin depender de un número** — lo que devolvió **vacío con exit 0**, la forma de salida que en este canal puede ser un NULL o una consulta sin filas. **No lo reporté como cero en ese momento**: lo re-corrí con `count(*)` numerado y con un **control positivo del `LIKE`**. El cero recién ahí vale.

---

## 8. PERMISOS E INTEGRIDAD

**Solo lectura.** `SELECT` con `--set=ON_ERROR_STOP=1` **antes** de `-tAc`, `curl` de lectura a la API de Prometheus y el control negativo del endpoint. **NO se disparó el benchmark.** CERO escrituras, CERO cambios al motor, CERO cambios de umbrales. Único archivo publicado: `docs/data/LIVE-EMITIDO-01.md`.

```bash
git hash-object docs/data/LIVE-EMITIDO-01.md
git rev-parse HEAD:docs/data/LIVE-EMITIDO-01.md
```

---

*El agregado mentía, y ahora está cuantificado: **1,25 %** de la tabla es sistema vivo y el 98,75 % son épocas muertas. Mirado con corte, lo que se emite hoy son tres cosas, y el techo activo **no** es el fósil del 88,2 % ni la familia del fondo: es **`candidate_incomplete`, 8.076 filas por hora, 92,1 % del presente**, con **0 % de rastro** y **0 % de rastro también en su control** — hueco de capacidad, vivo, una sola variante, residuo cero. El **funnel está ABIERTO**: 695 oportunidades distintas alcanzaron S4 en la última hora, y son exactamente las 693 + 2 del camino del fondo, lo que cierra la aritmética al entero. Pero `passed = true` sigue en **0**: **que el funnel esté abierto no es que el benchmark pase.** Y los dos productores que `main` documenta como los de HOY tienen **cero filas, ni históricas** — el código describe un comportamiento que no se está produciendo, y eso queda declarado NO COMPUTADO en su porqué.*
