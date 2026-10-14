# FUND-FUNNEL-01 — ¿Movió #879 el funnel de fondeo?

**Tarea:** t161 · **Agente:** Backend · **Modo:** SOLO LECTURA (cero cambios de código, cero reinicios, cero deploys)
**Instantes de medición:** `2026-10-08T14:01:22Z` y `2026-10-08T14:03:52Z` (ambos marcados por `date -u` en el VPS, canal ssh `arbx`)
**Canon:** `docs/backend/FUND-FUNNEL-01.PREDICCION.txt` — predicción pre-registrada ANTES de mirar.
**Especificación de sujeto (del contrato):** *"MEDIR SI #879 MOVIO EL FUNNEL, en la poblacion correcta y con el instrumento correcto."*

---

## 1. VEREDICTO

**PENDIENTE.**

No es «el fix no funcionó» y no es «el fix funcionó». Es que **#879 todavía no está corriendo**, y por eso la medición post-fix **no existe**: no es un cero, es un **no computado**.

La predicción pre-registrada queda **ni confirmada ni falsificada**, íntegra. Lo único que la medición de hoy establece es que **la línea base pre-fix sigue intacta y avanzando**, con sus identidades exactas. Ese es un resultado real y es el que se entrega.

Razón mecánica, en una línea: `origin/main` ya contiene el fix, pero **el proceso que emite las métricas corre un binario anterior**.

---

## 2. EL JUEZ: qué revisión corre el runtime

Fuente: `curl -s http://195.201.235.70/api/status` (exit=0), campo `deploy`.

```
"deploy":{"sha":"901eb947ff3bec359a3021a8db5b074e7080b79f","id":"37749114208","at":"2026-10-08T08:54:31Z"}
```

| Hecho | Valor | Artefacto |
|---|---|---|
| SHA del runtime | `901eb947ff3bec359a3021a8db5b074e7080b79f` | `GET /api/status` → `deploy.sha`, exit=0 |
| Deploy id del runtime | `37749114208` | `GET /api/status` → `deploy.id` |
| Timestamp del runtime | `2026-10-08T08:54:31Z` | `GET /api/status` → `deploy.at` |
| SHA de `origin/main` | `77b42b3dccc001455fda3e8d4437d973c9e98c4b` | `git rev-parse origin/main`, exit=0 |
| PR que introdujo el fix | `#879 from hefarica/fix/fund-read-calldata-01` | `git log --oneline origin/main` |
| Commit del fix | `1928b399 fix(sim-ctl): FUND-READ-CALLDATA-01 - balanceOf leia OTRA cuenta (calldata de 24 B)` | `git log --oneline origin/main` (tercero) |
| Estado del deploy de `77b42b3d` | run `37783694993`: `status=in_progress`, `conclusion=""` (vacío) | `gh run view 37783694993 --json status,conclusion`, exit=0 |
| Job `Wait for all deployment gates` | `completed/success` | `gh run view 37783694993 --json jobs`, exit=0 |
| Job `Deploy to VPS` | `in_progress` | `gh run view 37783694993 --json jobs`, exit=0 |
| Última actualización del run | `2026-10-08T13:46:55Z` | `gh run view 37783694993 --json updatedAt` |

**`901eb947` ≠ `77b42b3d`.** El runtime está 1 deploy por detrás de `main`. El deploy de `77b42b3d` existe y está **en vuelo**: pasó los gates, montó SSH, y quedó detenido en el paso `Deploy to VPS via SSH`.

**Consecuencia lógica:** el efecto de #879 no es medible hasta que `deploy.sha` cambie. Medir ahora sería medir el binario viejo y atribuirle el resultado al fix. **Eso es exactamente el error que esta tarea existe para no cometer.**

---

## 3. LA PREDICCIÓN, pre-registrada

Archivo: `docs/backend/FUND-FUNNEL-01.PREDICCION.txt` (blob en el commit de este PR).
Escrita y commiteada **antes** de cualquier lectura post-deploy. No se edita aquí: se cita.

- **P1 (principal)** — `verify_mismatch` por INTENTO dropea frente a `4,0000`.
- **P2 (la que decide si apareció éxito)** — la serie `seeded_fresh` APARECE (> 0).
- **P3 (el funnel)** — la igualdad exacta `slot_unresolved + rpc_err = arbx_simulation_total{anvil}` se ROMPE.
- **P4 (secundaria)** — `slot_unresolved` por intento baja (muestra n=25, ruido).
- **P5 (la palanca)** — si el fondeo deja de morir, el techo pasa a ser la capacidad del camino caro (736/h medido por t160) contra el camino barato (~123.300/h).

---

## 4. INSTRUMENTO Y SU UNIDAD

Serie: `arbx_sim_funding_total{outcome=…}` — contador **ACUMULATIVO DESDE EL ARRANQUE** de `sim-ctl`.
Unidades, cerradas por t160 y **no re-derivadas aquí**:

| Etiqueta | Unidad | Relación |
|---|---|---|
| `verify_mismatch` | por **SLOT** sondeado | 4 candidatos por intento ⇒ 4× los intentos |
| `slot_unresolved` | por **INTENTO** | el denominador honesto |
| `balance_unreadable` | contenida en `verify_mismatch` | no se suma |
| `write_rejected` | nunca dispara | no se usa |
| `seeded_fresh` | el **único** éxito (par escritura+lectura cerrado) | ausente en todo t160 |

`verify_mismatch` **NO es un `fail_reason`**: es una **etiqueta de métrica**. Buscarla en `simulations` es un error de categoría (defecto ya declarado).

---

## 5. LA MEDICIÓN

Lectura coherente en un solo instante — `2026-10-08T14:03:52Z`, ts de Prometheus `1791468232.634`
(vía `curl -s --max-time 20 'http://localhost:9090/api/v1/query?query=…'` **desde el VPS**, exit=0 — ver defecto #9):

| Etiqueta | Valor @14:03:52Z |
|---|---|
| `slot_unresolved` | **2634** |
| `verify_mismatch` | **10536** |
| `balance_unreadable` | 1451 |
| `rpc_err` | **5** |
| `arbx_simulation_total{simulator="anvil"}` | **2639** |
| `arbx_simulation_total{simulator="revm"}` | 38038 |
| `seeded_fresh` | **vector vacío ⇒ AUSENTE** |
| outcomes distintos en la serie | 4 |

### 5.1 Las identidades exactas siguen intactas

```
2634 + 5 = 2639            → slot_unresolved + rpc_err = anvil   EXACTO
10536 / 2634 = 4,0000      → verify_mismatch / intentos = 4,0    EXACTO
```

### 5.2 Y siguen intactas mientras el contador AVANZA

Cinco lecturas en ventana, todas con la misma firma:

| Lectura | `slot_unresolved` | `verify_mismatch` | anvil | ratio |
|---|---|---|---|---|
| t160 (base) | 2468 | 9872 | 2473 | 4,0000 |
| t161-a | 2549 | 10196 | 2554 | 4,0000 |
| t161-b | 2563 | 10252 | 2568 | 4,0000 |
| t161-c @14:01:22Z | 2585 | 10340 | 2590 | 4,0000 |
| t161-d @14:03:52Z | 2634 | 10536 | 2639 | 4,0000 |

El contador avanzó `2468 → 2634` (+166 intentos) **conservando el ratio exacto**. No es una foto: es una firma viva del binario pre-#879. Si #879 estuviera corriendo, el ratio no podría quedarse en `4,0000` con el contador moviéndose.

### 5.3 P2: `seeded_fresh` sigue AUSENTE

`arbx_sim_funding_total{outcome="seeded_fresh"}` devuelve **vector vacío** en las dos lecturas de esta tarea (`14:01:22Z`, `14:03:52Z`), exit=0 en ambas.
Ausencia ≠ cero: es **no computado**, y su razón es la de §2 — el productor de esa etiqueta es el código de #879, que no corre. En las cinco lecturas (t160 + cuatro de t161) **nunca apareció**.

### 5.4 OBSERVACIÓN RETIRADA — el control funcionó

En una lectura intermedia registré que `arbx_simulation_total{simulator="revm"}` no avanzaba (`37659` en dos lecturas separadas ~12 min). **La tercera lectura la falsifica: `revm = 38038` @14:03:52Z ⇒ avanzó +379.**

Se **retira** la observación y se conserva el rastro, porque el modo de fallo que evitó es el que esta campaña viene pagando: dos lecturas iguales **no** son una serie congelada, son dos muestras. Sin la tercera lectura habría quedado escrito en un artefacto un «hecho» que era un artefacto de muestreo. **No se concluye nada sobre `revm`**; solo se deja constancia de que las dos series avanzan y de que `revm` lo hace mucho más rápido que `anvil` (`+379` contra `+43` en la misma ventana), consistente con §4: el camino barato lleva el caudal.

### 5.5 La partición de la tabla viva (control cruzado independiente)

Los contadores son de Prometheus; la tabla es de PostgreSQL. **Dos fuentes independientes.** Corte vivo, 1 h por `simulated_at`
(`docker exec arbitragex-v2-postgres-1 psql -U postgres -d arbitragex -tAc "SELECT fail_reason, count(*) FROM simulations WHERE simulated_at > now() - interval '1 hour' GROUP BY fail_reason ORDER BY 2 DESC"`, exit=0):

| `fail_reason` | count |
|---|---|
| `candidate_incomplete:amount_in_wei_zero` | 8027 |
| `sim_signer_funding_slot_unresolved` | 855 |
| `funding_balanceof_timeout` | 3 |
| **Σ** | **8885** |

```
8027 + 855 + 3 = 8885   → la partición cierra EXACTA contra el total de la tabla
855 + 3 = 858           → con_rastro (revert_risk_pct IS NOT NULL) @14:03:52Z, EXACTO
```

Verificación del campo de rastro, misma consulta (`count(*) FILTER (WHERE revert_risk_pct IS NOT NULL), count(*) FILTER (WHERE raw_trace IS NOT NULL), count(*)` ⇒ `858|0|8885`, exit=0):

- `revert_risk_pct IS NOT NULL` = **858** = `855 + 3` ⇒ es el campo real, y marca **exactamente** las dos ramas de fondeo.
- `raw_trace IS NOT NULL` = **0 sobre 8885 filas positivas** ⇒ **reconfirmado defecto #8**: un control que lee cero sobre la población positiva no es un control.

Nota de instrumento: la partición de arriba mide **filas de tabla**; los contadores de §5 miden **eventos acumulados desde el arranque**. Son unidades distintas y **no se restan entre sí** (lección de t160).

---

## 6. LO QUE NO SE PUEDE COMPUTAR, Y POR QUÉ

| No computado | Razón exacta |
|---|---|
| Efecto de #879 sobre el funnel | El runtime es `901eb947`, anterior a `1928b399`. El fix no está desplegado. |
| Dónde cae `verify_mismatch` post-fix | Requiere el binario nuevo emitiendo. |
| Si `seeded_fresh` aparece | Requiere el binario nuevo emitiendo. |
| `uptime` de `sim-ctl` | `process_start_time_seconds{job="sim-ctl"}` ⇒ **vector vacío**. La métrica no la exporta este proceso. Sin uptime no se puede fechar el arranque ni normalizar por tiempo. **NO COMPUTADO, no cero.** |
| `no_simulation_row` | **NO MEDIDA.** La etiqueta no aparece entre los `outcome` de `arbx_sim_funding_total` (4 outcomes distintos @14:03:52Z: `slot_unresolved`, `balance_unreadable`, `verify_mismatch`, `rpc_err`). No se reporta como 0: no se midió. |
| Antigüedad del arranque de `sim-ctl` | **ESTIMACIÓN, declarada como tal.** Con el ritmo del camino caro medido por t160 (**733/h**, que es el `693/h` de la tabla de 25 redondeado por esa tarea), `2468/733 ≈ 3,4 h` desde el arranque hasta la base de t160. Recalculada con el contador de §5: `2634/733 ≈ 3,6 h`. **Es una estimación por un ritmo ajeno, no una medición de `process_start_time_seconds`** — que no existe (§ arriba). |
| Si el ratio `4,0000` colapsa | Depende de los tres primeros. |

**Regla aplicada:** un campo no computado se reporta como no computado, con su razón. No se rellena con la predicción. Un «no medido» **no** se escribe como `0`.

---

## 7. DEFECTOS DE CONTRATO ENCONTRADOS

Se declaran, no se silencian.

- **#9 — el comando de verificación del contrato falla por la dirección, no por el servicio.** El `verify` del contrato dice `curl -s --max-time 20 'http://195.201.235.70:9090/api/v1/query?query=…'`. Medido: **`exit=7`** (conexión rechazada). Pero el mismo endpoint por loopback, **desde el VPS**, responde **`exit=0` con datos**: `curl -s --max-time 20 'http://localhost:9090/api/v1/query?query=arbx_simulation_total'`. ⇒ El puerto 9090 está publicado **solo en loopback**, no en la interfaz pública. **El servicio está vivo; la dirección del contrato es la que no llega.** Corrección de una palabra: usar `localhost:9090` ejecutando en el VPS (tercera vía equivalente: `docker exec arbitragex-v2-prometheus-1 wget -qO- 'http://localhost:9090/api/v1/query?query=…'`).
  **Por qué importa igual:** quien copie el comando del contrato **tal como está escrito** obtiene `exit=7` y un vector vacío, y la lectura natural de eso es «no hay datos» — un falso negativo con la forma exacta de un cero. Un `exit=7` sobre una consulta de métricas **nunca** debe leerse como ausencia de eventos.
- **#10 — el contrato asume un deploy cerrado que no cerró.** La instrucción *"si el deploy no cerró, se declara PENDIENTE con la condición exacta y se cierra"* resultó ser la rama correcta, y es la que se ejecuta. `gh run view 37783694993` ⇒ `status=in_progress`, `conclusion=""`.
- **#8 (heredado de t160, sigue vigente)** — el campo de traza es `revert_risk_pct`, **no** `raw_trace`: `raw_trace IS NOT NULL` = `0` en el 100 % de las filas vivas, **incluidas las que sí simulan**. No es un control.

---

## 8. CONTROLES DE MÉTODO

Un instrumento sin controles no es un instrumento.

| Control | Resultado | Lectura |
|---|---|---|
| Canal ssh vivo | `SELECT 1` → `1`, exit=0 | el canal transporta |
| Control negativo SQL | `SELECT esto_no_existe` → **exit=1 + `ERROR: column … does not exist`** | el exit=0 de arriba significa algo |
| `:9090` por IP pública | **`exit=7`** | el contrato no llega; ver defecto #9 |
| `:9090` por loopback (VPS) | **`exit=0`, con datos** | el servicio está vivo y mide |
| Endpoint prohibido `/metrics` | `http_code=404`, `content_type=text/html; charset=utf-8` | coincide con lo declarado: es la consola, no Prometheus |
| Partición de la tabla viva | `8027 + 855 + 3 = 8885` **EXACTO** | la tabla cierra contra sí misma |
| `revert_risk_pct` vs las ramas de fondeo | `855 + 3 = 858` **EXACTO** | el campo de rastro marca justo las dos ramas de fondeo |
| `raw_trace` sobre población positiva | **`0` sobre `8885`** | **NO es control** — defecto #8 reconfirmado en esta misma corrida |
| Identidad `4,0000` | **EXACTA** en 5 lecturas | el instrumento ve lo que dice ver |
| Identidad `slot_unresolved+rpc_err = anvil` | **EXACTA** en 5 lecturas | dos series independientes concuerdan |
| `seeded_fresh` sobre población positiva | **vector vacío** | ausencia real, con productor identificado (§2) |

**Controles que esta tarea NO puede usar, y por qué** (heredados, no re-litigados):
- `raw_trace IS NOT NULL` — defecto #8: da cero en la población positiva, **reconfirmado hoy sobre `8885` filas**.
- La tabla de `simulations` — mide **filas/ventana**; el contador mide **eventos desde el arranque**: unidades distintas, no comparables directamente. Se dan las dos (§5 y §5.5) y se dice cuál es la primaria: **el contador**.
- Cualquier `GROUP BY` cuya vacuidad pudiera ser un `NULL` — se cuenta con `count(*)`, y la partición de §5.5 se contrasta contra el total.
- La muestra de 25 del benchmark — n=25 no distingue 7 de 4 de ruido (t156).

---

## 9. CÓMO SE CIERRA ESTE VEREDICTO

Condición de cierre, **exacta y mecánica**. Ninguna parte requiere juicio:

1. `curl -s http://195.201.235.70/api/status` ⇒ `deploy.sha` **≠ `901eb947…`**, y ese SHA es `77b42b3d…` o un descendiente que contenga `1928b399`.
2. **Y** el job `Deploy to VPS` del run correspondiente pasa a `completed/success` con `conclusion` no vacía.
3. Recién entonces se relee `arbx_sim_funding_total` y se contrasta P1/P2/P3 contra los valores de §5.1.

**Criterio de falsación de P1, ya escrito, se respeta:** si al correr el binario nuevo `verify_mismatch / intentos` sigue en `~4,0000`, #879 **no movió el funnel**.
**Criterio de P2:** si `seeded_fresh` sigue ausente con el binario nuevo, **no hubo un solo fondeo exitoso** y P1 se cumpliría en vano.

**NO se re-dispara el benchmark contra el runtime viejo.** Hacerlo produciría un número atribuido al fix que el fix no causó.

---

## 10. TRAZABILIDAD

- Artefactos leídos: `GET /api/status` (VPS), `gh run view 37783694993`, `git log origin/main`, Prometheus **por loopback desde el VPS** (`curl http://localhost:9090/api/v1/query`), PostgreSQL por `docker exec arbitragex-v2-postgres-1 psql`.
- Ninguna escritura sobre el VPS. Ningún reinicio. Ningún deploy. Ninguna firma. Ningún broadcast.
- Este documento no modifica código: solo `docs/backend/FUND-FUNNEL-01.md` y `docs/backend/FUND-FUNNEL-01.PREDICCION.txt`.

**Firma:** Backend · t161 · attempt `98ac3935-4fba-488b-a924-f4390aee60f8`
