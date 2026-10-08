# FUND-E2E-VERIFY-01 — MEDICIÓN POST-ARREGLO **PENDIENTE**: el runtime todavía sirve el padre

**Resultado en una línea:** **NO se midió.** El deploy sobre `77b42b3d` **no cerró** (`status: in_progress`, estancado en el paso de gates desde `13:22:53Z`) y el **runtime sigue sirviendo `901eb947` — el PADRE, SIN el arreglo**. Medir la partición contra este runtime no diría nada sobre el arreglo: sería **mirar el sistema pre-arreglo y llamarlo «después»**, que es la etiqueta mal leída que este proyecto ya pagó. Se entrega **(a) todo lo que no depende del deploy**, con la **predicción sellada antes de mirar** y la **línea base anclada por comando propio**, y la medición post queda **PENDIENTE con su condición exacta**.

**Cápsula ORGDNA:** `run_id=arbx-entrega-20261008` · **Dueño:** Data · **Intento:** `0618815e-7de8-4191-953b-3a4eb5ade29b`
**Solo lectura** · **CERO cambios al motor** · **CERO escrituras** · **In scope:** `docs/data/`

---

## 0. ★ EL SHA CONTRA EL QUE SE MIDE — y por qué NO se midió

| instrumento | lectura literal | instante |
|---|---|---|
| `gh run view 37783694993` | `{"status":"in_progress","conclusion":"","headSha":"77b42b3dccc001455fda3e8d4437d973c9e98c4b","createdAt":"2026-10-08T13:21:23Z","updatedAt":"2026-10-08T13:22:53Z"}` | 13:41Z y 13:42Z |
| job del mismo run | job **«Wait for all deployment gates»**, step 3 **«Verify same-SHA main-push gates»** → `status: in_progress`, `startedAt: 2026-10-08T13:22:58Z`, **sin `completedAt`** | 13:42Z |
| **`curl /api/status`** | **`{"sha":"901eb947ff3bec359a3021a8db5b074e7080b79f","id":"37749114208","at":"2026-10-08T08:54:31Z"}`** · `ts: 2026-10-08T13:42:22Z` | 13:42:22Z |
| `gh run list --workflow gsim1-variance-benchmark.yml` | las 4 últimas: `901eb947` (09:26:57Z), `901eb947` (08:55:04Z), `ae7967fc` (06:16:41Z), `8414e512` (05:58:23Z) — **NINGUNA sobre `77b42b3d`** | 13:42Z |

**⇒ El runtime servido es `901eb947`, el PADRE. Y no existe ninguna corrida del benchmark sobre `77b42b3d`.** La condición que el contrato puso como precondición —«**no se mide contra un runtime que todavía sirve el padre**»— **no se cumple**, así que **no se midió**.

**Y el deploy no está simplemente lento: está parado en el gate.** `updatedAt` = `13:22:53Z` y a las `13:42Z` sigue igual: **~19 minutos sin avanzar**, con el job `113332915137` en «Verify same-SHA main-push gates». Eso es un dato operativo, no una excusa: **el paso que bloquea es el gate de same-SHA, no la construcción ni el despliegue.**

---

## 1. ★ LA PREDICCIÓN, SELLADA ANTES DE MIRAR

**Prueba de orden:** el archivo de predicción se escribió **antes** de cualquier medición, y su identidad queda declarada para que no se pueda reescribir después:

| | |
|---|---|
| ruta | `t156-prediccion.md` |
| **mtime UTC** | **`2026-10-08T13:40:44.4449036Z`** — anterior a la primera lectura de runtime (13:41:01Z) |
| **blob git** | **`8fdc5807ea89c845d4b12da433225c9a5a5643ff`** |
| sha256 | `890A43C3A75265D58A19ECEF29F904F87522845F5AB3B0E20D8CBDED51862C4D` |

**Lo que dice, textual:**

> **LINEA BASE PRE-ARREGLO** (instrumento sobre `901eb947`, el PADRE): `SAMPLE_REASON_DISTRIBUTION total=25  sim_failed:sim_signer_funding_slot_unresolved=7  no_simulation_row=18`
> **LA PREDICCION, EN UNA LINEA:** Si el arreglo funciona, `sim_signer_funding_slot_unresolved` tiene que DROPEAR.
> **NO DROPEA** (queda en 7, o sube, o se mueve a otra razón): la predicción es **FALSA**; hay OTRA causa en el camino del fondeo y **ESE es el hallazgo**. Se reporta crudo. **NO se maquilla con «el instrumento cambió» ni con una ventana distinta.**
> **Espero que el benchmark siga en `FAIL` con `samples_labeled=0`.**

**Estado de la predicción: SELLADA Y NO TESTEADA.** No está confirmada ni refutada — y **no testearla es la respuesta correcta a un runtime que sirve el padre**.

---

## 2. ★ (a) LA LÍNEA BASE, ANCLADA CON COMANDO PROPIO

### 2.1 Ventana de la tabla

`SELECT min(simulated_at) || ' | ' || max(simulated_at) || ' | ' || count(*) FROM simulations` — **exit 0**:
**`2026-10-07 21:19:58.524132+00` | `2026-10-08 13:42:08.797598+00` | `699.889`**

**Corrección de columna declarada:** el verify del contrato usa `detected_at`, que **no existe** en `simulations` (`exit=1`, `ERROR: column "detected_at" does not exist`). La columna real es **`simulated_at`**. La ventana de `simulations` es **~16 h**, distinta de la de `opportunities` (~3 días).

### 2.2 ★ EL NÚMERO REAL NO ES 7 — es 29.010

`fail_reason` sobre `simulations`, con **corte temporal**:

| `fail_reason` | **total** | **última hora** | **últimos 15 min** |
|---|---|---|---|
| `strategy_cyclic_route_not_simulatable_in_s4:dex_arb` | 576.907 | — | — |
| `candidate_incomplete:amount_in_wei_zero` | 53.070 | 8.108 | 2.040 |
| **`sim_signer_funding_slot_unresolved`** | **29.010** | **609** | **214** |
| `strategy_cyclic_route_not_simulatable_in_s4:flashloan_arb` | 23.743 | — | — |
| `strategy_cyclic_route_not_simulatable_in_s4:triangular` | 16.920 | — | — |
| `build_error: amount invalid: zero amount_in` | 144 | — | — |
| `build_error: router not in catalog for chain=1 dex=unknown` | 84 | — | — |
| **`funding_balanceof_timeout`** | **5** | — | **2** |
| `fork_acquire_failed` | 4 | — | — |
| `anvil_setStorageAt: … Connection refused (os error 111)` | 2 | — | — |

**`sim_signer_funding_slot_unresolved`: total 29.010 · última hora 609 · últimos 15 minutos 214.**
**`passed = true`: 0 en total y 0 en la última hora.** *(Comando: `SELECT count(*) FILTER (WHERE passed) FROM simulations` → `0`.)*

**★ Re-anclaje, y es lo más importante de esta sección: el `7` de la línea base es el conteo DENTRO DE LA MUESTRA de 25 del benchmark, NO la población.** La población viva de esa misma causa es **29.010 filas**, a **~214 por 15 minutos** y **609 por hora**. **El radio de lo que se está arreglando es tres órdenes de magnitud mayor que el 7 de la línea base** — y quien lea «7» como el tamaño del problema se equivoca por 4.000×.

**★ Y el reencuadre de prioridad, con los mismos números:** la familia **`strategy_cyclic_route_not_simulatable_in_s4:*`** suma **617.570 de 699.889 = 88,2 %** de todas las filas de `simulations`. **`sim_signer_funding_slot_unresolved` es 29.010 / 699.889 = 4,1 %.** El arreglo ataca **la cuarta parte del 4 %**, no la mayoría del funnel.

### 2.3 El contador del funnel — ancla

`arbx_sim_funding_total{outcome}`, lectura a **`2026-10-08T13:41:54Z`**:

| `outcome` | valor |
|---|---|
| `verify_mismatch` | **8.752** |
| **`slot_unresolved`** | **2.188** |
| `balance_unreadable` | **1.300** |
| **`rpc_err`** | **4** |
| **TOTAL** | **12.244** |

**★ Dos series que el contrato NO listaba.** El enunciado dice «las tres series conocidas son `verify_mismatch`, `balance_unreadable` y `slot_unresolved`». Medido: **hay una CUARTA, `rpc_err` (4)**, y en PG aparece además **`funding_balanceof_timeout` (5 total, 2 en 15 min)** — **un segundo modo de fallo del camino de fondeo**, distinto del que el arreglo ataca. El arreglo corrige el **calldata** de `balanceOf`; **un `funding_balanceof_timeout` es la MISMA llamada agotándose**, y su existencia dice que hay **al menos dos formas de que el fondeo no resuelva**, no una.

**`write_rejected` — CONFIRMADO AUSENTE**: no aparece entre los `outcome` del contador. Concuerda con el enunciado (el `anvil_setStorageAt` nunca falla) y con PG, donde el único fallo de escritura es `anvil_setStorageAt: … Connection refused` con **2 filas**, y es un fallo de **conexión al servicio**, no de la operación.

**El contador es CUMULATIVO y monótono desde el arranque de `sim-ctl`** (medido creciendo: `slot_unresolved` 2.185 → 2.188 en ~90 s). **Consecuencia para el post: cuando el deploy aterrice, `sim-ctl` se reinicia y el contador vuelve a 0** ⇒ el delta post-arreglo será **directamente legible** sin restar. Eso es una ventaja de instrumento, y queda declarada.
**`process_start_time_seconds{job="sim-ctl"}` devolvió VACÍO** ⇒ **NO COMPUTADO** (no un cero): no pude fechar el arranque del contador por esa vía.

---

## 3. ★ LÍMITE DEL INSTRUMENTO, DECLARADO

- **`no_simulation_row` = NO MEDIDA, no «fallo».** El instrumento del benchmark reparte 25 muestras en `sim_failed:<razón>` y `no_simulation_row`. Si al correr el post `sim_failed` baja y `no_simulation_row` sube, **hay que dar los dos números**; no se elige el que conviene. **Y como la ventana del benchmark es de 25 muestras, un movimiento de 7 a 4 es indistinguible de ruido de muestreo** — el post tiene que mirar **también** el contador acumulativo (2.188) y la tabla (214/15 min), que tienen masa estadística, y no sólo la muestra de 25. **Sin eso, el post podría declarar una mejora que es varianza.**
- **`samples_labeled` se reporta aunque sea 0**: no es medible ahora porque **no hay corrida post-deploy**. Se declara **NO COMPUTADO**, no 0.
- **El benchmark NO pasa porque el fondeo funcione.** Su población **sigue siendo rechazos** (`passed = true` es **0** en 699.889 filas). Confundir «el fondeo funciona» con «el benchmark pasa» es el error que este proyecto paga desde F03; acá se declaran **separados**.
- **`opportunities`** a las 13:41:16Z: min `2026-10-05 04:21:13.227568+00` | max `2026-10-08 13:41:16.206876+00` | **9.109.237** filas — viva y creciendo. Los conteos de la campaña anterior **no son comparables por ventana**.

---

## 4. ★ LA CONDICIÓN EXACTA PARA COMPLETAR LA MEDICIÓN

**Precondición (las dos, no una):**
1. `gh run view 37783694993 --repo hefarica/arbitragex-v2 --json conclusion` ⇒ **`"success"`**
2. `curl -s http://195.201.235.70/api/status` ⇒ **`deploy.sha == "77b42b3dccc001455fda3e8d4437d973c9e98c4b"`**

**Y una tercera, que es la que mide de verdad:** que exista una corrida de `gsim1-variance-benchmark.yml` con **`headSha = 77b42b3d…`** **posterior** al cierre del deploy. Hoy **no existe ninguna**.

**Cuando se cumplan, el post consiste en — sin ambigüedad:**
1. `arbx_sim_funding_total{outcome}` **después del reinicio de `sim-ctl`** ⇒ el contador vuelve a 0 y el **delta es el valor mismo**: si `slot_unresolved` arranca en **0** con tráfico corriendo, la predicción **se sostiene**; si vuelve a subir, **se refuta**.
2. `fail_reason` sobre `simulations` con corte **posterior** al cierre ⇒ comparar contra **29.010 total / 609 h / 214 15min**.
3. El `SAMPLE_REASON_DISTRIBUTION` de la corrida con `headSha=77b42b3d` ⇒ comparar contra `7` — **sabiendo que 7 es la muestra, no la población**.
4. Distinguir, si se mueve, **a qué serie se movió**: `slot_unresolved` ≠ `balance_unreadable` ≠ `verify_mismatch` ≠ `rpc_err` ≠ **`funding_balanceof_timeout`**.

**Si `slot_unresolved` NO dropea, el hallazgo es que hay otra causa en el camino del fondeo** — y el candidato ya medido, con nombre y conteo, es **`funding_balanceof_timeout` (5 / 2 en 15 min)**. **Esa es la siguiente capa, y ya está localizada.**

---

## 5. PERMISOS E INTEGRIDAD

**Solo lectura.** Todos los comandos fueron `SELECT` (con `--set=ON_ERROR_STOP=1` **antes** de `-tAc`, tras el hallazgo de t155), `curl` de lectura a Prometheus y a `/api/status`, y consultas `gh run view/list`. **NO se disparó el benchmark**: hacerlo contra el runtime viejo y llamarlo «después» sería la etiqueta mal leída que el contrato prohíbe. CERO escrituras, CERO cambios al motor, CERO cambios de umbrales. Único archivo tocado: `docs/data/FUND-E2E-VERIFY-01.md`.

```bash
git hash-object docs/data/FUND-E2E-VERIFY-01.md
git rev-parse HEAD:docs/data/FUND-E2E-VERIFY-01.md
```

---

*No se midió, y ésa es la entrega correcta: el deploy no cerró —está parado en el gate de same-SHA desde las 13:22:53— y el runtime sigue sirviendo el padre, así que cualquier lectura de la partición describiría el sistema pre-arreglo. La predicción quedó sellada antes de mirar, con su hash y su `mtime`, y **sin testear**. Lo que sí quedó anclado vale por sí solo: **el número real no es 7, es 29.010 — y 214 cada 15 minutos**; el arreglo ataca el **4,1 %** de las filas de `simulations`, no la mayoría; hay **una cuarta serie (`rpc_err`) y un segundo modo de fallo del fondeo (`funding_balanceof_timeout`)** que el enunciado no listaba; y `passed = true` es **0** en 699.889 filas, así que el benchmark sigue sin poder pasar por el fondeo.*
