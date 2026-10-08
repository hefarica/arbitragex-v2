# FUND-FUNNEL-02 — ¿abrió la puerta? ¿`seeded_fresh` deja de estar vacío y dropea `verify_mismatch`?

**Tarea:** t163 · **Agente:** Backend · **Modo:** SOLO LECTURA (cero cambios de motor, cero umbrales, cero escrituras, cero reinicios)
**Instante de medición:** `2026-10-08T14:10:57Z` (marcado con `date -u` en el VPS, canal ssh `arbx`)
**Predecesora:** t161 / PR #890 (DRAFT, head `ca9096ad`) — dejó el veredicto **PENDIENTE** con la condición de cierre.

---

## 1. VEREDICTO

**PENDIENTE. La puerta NO abrió.**

Cláusula 1 de la condición, fallada:
```
deploy.sha = 901eb947ff3bec359a3021a8db5b074e7080b79f
cláusula:   deploy.sha != 901eb947ff3bec359a3021a8db5b074e7080b79f   -> FALSA
```
`deploy.sha` **sigue siendo exactamente el mismo** SHA pre-arreglo que t161 midió. Se declara PENDIENTE con la condición exacta (§7) y **se cierra**. No se espera en bucle. **No se re-dispara nada contra el runtime viejo.** El efecto de #879 sigue siendo **NO COMPUTADO, no un cero**.

---

## 2. LA PUERTA, LEÍDA DEL RUNTIME

Fuente: `curl -s --max-time 20 http://195.201.235.70/api/status | grep -o '"deploy":{[^}]*}'`, exit=0.

```
"deploy":{"sha":"901eb947ff3bec359a3021a8db5b074e7080b79f","id":"37749114208","at":"2026-10-08T08:54:31Z"}
```

| Cláusula de la condición | Requerido | Medido | Veredicto |
|---|---|---|---|
| 1. Dejó de ser el SHA viejo | `!= 901eb947…` | `== 901eb947…` | **FALSA** |
| 2. Es `77b42b3d…` o descendiente con `1928b399` | `== 77b42b3d…` | `== 901eb947…` | **FALSA** |
| 3. Job `Deploy to VPS` con `conclusion` no vacía | `!= ""` | `conclusion=""` | **FALSA** |

**Las tres fallan.** `origin/main` (`git ls-remote https://github.com/hefarica/arbitragex-v2.git refs/heads/main`, exit=0) = `77b42b3dccc001455fda3e8d4437d973c9e98c4b` — el fix **existe en el repo**, y **no está en el runtime**.

### 2.1 El deploy, leído por JOBS y STEPS (no por el status del RUN)

`gh run view 37783694993 --repo hefarica/arbitragex-v2 --json status,conclusion,jobs`, exit=0:

| Job | Estado | Ventana |
|---|---|---|
| `Wait for all deployment gates` | `completed/success` | `13:22:52Z → 13:46:12Z` |
| `Deploy to VPS` | `in_progress/` | `13:46:54Z → (sin cerrar)` |

Run-level: `status=in_progress`, `conclusion=""`, `headSha=77b42b3d…`, `updatedAt=2026-10-08T13:46:55Z`.

Steps del job `Deploy to VPS`, con `started_at`/`completed_at`:

```
1. Set up job                        => completed/success  [13:46:55Z -> 13:46:56Z]
2. Checkout repository               => completed/success  [13:46:56Z -> 13:47:04Z]
3. Recheck gates immediately before SSH => completed/success [13:47:04Z -> 13:47:05Z]
4. Setup SSH                         => completed/success  [13:47:05Z -> 13:47:07Z]
5. Deploy to VPS via SSH             => in_progress/       [13:47:07Z -> 0001-01-01T00:00:00Z]
6. Find simulator evidence for the deployed SHA        => pending/
7. Download exact-run simulator evidence               => pending/
8. Validate evidence identity before publication       => pending/
9. Publish unit-tests evidence after API recovery      => pending/
10. Publish dep-tree evidence after API recovery       => pending/
11. Cleanup SSH key                                    => pending/
22. Post Checkout repository                           => pending/
```

El `0001-01-01T00:00:00Z` es el **cero de Go para "sin cerrar"**, no una fecha. El step 5 arrastra **23,83 min** de ejecución (`13:47:07Z → 14:10:57Z`).

**NO se concluye que esté colgado.** El contrato registra **tres falsas alarmas** del capitán producidas por leer un timestamp congelado como bloqueo; esta tarea no repite ese error. Un `docker compose build --no-cache` de un workspace Rust tarda en ese orden. **Se reporta la duración como hecho medido y nada más.**

---

## 3. EL INSTRUMENTO: QUÉ CASO OCURRIÓ

El contador de Prometheus es **ACUMULATIVO DESDE EL ARRANQUE de `sim-ctl`**, y hay que declarar cuál de los dos casos ocurrió **antes** de comparar:

```
caso 1: el deploy reinició sim-ctl  -> el contador ARRANCA EN 0  -> el post se lee directo
caso 2: no lo reinició             -> el contador SIGUE AVANZANDO -> el delta se mide contra pre-valores
```

**Ocurrió el CASO 2.** `slot_unresolved` pasó de `2634` (t161-d) a `2786` — **avanzó, no volvió a 0**. Lo mismo `anvil` (`2639 → 2791`) y `revm` (`38038 → 39143`). ⇒ **`sim-ctl` NO se reinició** ⇒ el delta se mide **contra los pre-valores**, que es lo que hace §4. Confundir los dos casos **invertiría** la conclusión.

Declaración de arranque: **no computado.** `process_start_time_seconds{job="sim-ctl"}` = **vector vacío** (§6). No se puede fechar el arranque; se sabe que **no hubo uno nuevo** por el avance del contador, no por la métrica de uptime.

---

## 4. LOS CONTADORES @14:10:57Z

Fuente (forma correcta, loopback vía ssh): `curl -s --max-time 20 'http://localhost:9090/api/v1/query?query=arbx_sim_funding_total'`, exit=0. ts de Prometheus `1791468657.577`.

| Etiqueta | Valor |
|---|---|
| `slot_unresolved` | **2786** |
| `verify_mismatch` | **11144** |
| `balance_unreadable` | 1696 |
| `rpc_err` | **5** |
| `arbx_simulation_total{simulator="anvil"}` | **2791** |
| `arbx_simulation_total{simulator="revm"}` | 39143 |
| `arbx_sim_funding_total{outcome="seeded_fresh"}` | **vector VACÍO** |

```
2786 + 5 = 2791            -> slot_unresolved + rpc_err = anvil   EXACTO
11144 / 2786 = 4,0000      -> verify_mismatch / intentos = 4,0    EXACTO
```
*(identidades verificadas por herramienta, no a mano)*

### 4.1 Las DOS series de la predicción: NINGUNA se movió

| Serie | Predicción | Medido @14:10:57Z | Veredicto de la predicción |
|---|---|---|---|
| `verify_mismatch` por intento | tiene que **DROPEAR** de `4,0000` | **`11144 / 2786 = 4,0000`** | **NO dropeó** |
| `seeded_fresh` | tiene que **DEJAR DE ESTAR VACÍO** | **vector vacío** | **Sigue vacío** |

**Ninguna de las dos se movió. ESO es el hallazgo, y se entrega crudo.** Pero la lectura correcta **no** es «#879 no sirvió»: es que **#879 no está corriendo** (§2). Las dos series miden un binario que no contiene el fix. Un contador que no cambia **porque su causa no está desplegada** no es evidencia contra la causa.

### 4.2 La firma pre-arreglo, intacta a la SEXTA lectura

| Lectura | `slot_unresolved` | `verify_mismatch` | anvil | ratio |
|---|---|---|---|---|
| t160 (base) | 2468 | 9872 | 2473 | 4,0000 |
| t161-a | 2549 | 10196 | 2554 | 4,0000 |
| t161-b | 2563 | 10252 | 2568 | 4,0000 |
| t161-c @14:01:22Z | 2585 | 10340 | 2590 | 4,0000 |
| t161-d @14:03:52Z | 2634 | 10536 | 2639 | 4,0000 |
| **t163 @14:10:57Z** | **2786** | **11144** | **2791** | **4,0000** |

**+318 intentos** en la campaña (`2468 → 2786`) **sin romper el ratio ni una vez**. Con el binario nuevo emitiendo, `4,0000` no podría sostenerse: es una firma de **comportamiento**, y es lo que decide — no la métrica que se quiere medir.

### 4.3 El SHA del runtime: cinco lecturas, no dos

Regla de t161: *dos lecturas iguales no son una serie congelada, son dos muestras; toda afirmación de «está quieto» exige una tercera.*

`deploy.sha = 901eb947…` acumula **5 lecturas idénticas**: cuatro en t161 (`14:01:22Z`, `14:03:52Z`, `14:05:10Z` y las previas de esa tarea) y una en t163 (`14:10:36Z`). La regla de la tercera lectura **está satisfecha con margen**.

---

## 5. CONTROL CRUZADO POR PG (fuente independiente) Y CONTROLES NEGATIVOS

Fuente: `docker exec arbitragex-v2-postgres-1 psql -U postgres -d arbitragex -tAc "…" --set=ON_ERROR_STOP=1`, exit=0. Corte 1 h por **`simulated_at`** (no `detected_at`, que no existe).

| `fail_reason` | count t163 | count t161 (baseline) |
|---|---|---|
| `candidate_incomplete:amount_in_wei_zero` | 8256 | 8027 |
| `sim_signer_funding_slot_unresolved` | 953 | 855 |
| `funding_balanceof_timeout` | 3 | 3 |
| **Σ** | **9212** | **8885** |

```
8256 + 953 + 3 = 9212     -> la partición cierra EXACTA
953 + 3 = 956             -> con_rastro (revert_risk_pct IS NOT NULL) EXACTO
```

Campo de rastro, misma corrida — `SELECT count(*) FILTER (WHERE revert_risk_pct IS NOT NULL), count(*) FILTER (WHERE raw_trace IS NOT NULL), count(*) …` ⇒ **`956|0|9212`**:

- `revert_risk_pct IS NOT NULL` = **956** = `953 + 3` ⇒ es el campo real, marca **exactamente** las dos ramas de fondeo.
- `raw_trace IS NOT NULL` = **0 sobre 9212 filas positivas** ⇒ **defecto #8 reconfirmado por segunda tarea consecutiva**. Un control que lee cero sobre la población positiva **no es un control**.

### 5.1 Los tres controles negativos

| Control | Comando | Resultado | Lectura |
|---|---|---|---|
| Canal SQL | `SELECT 1` | `1`, exit=0 | el canal transporta |
| Negativo SQL | `SELECT esto_no_existe` | **exit=1** + `ERROR: column "esto_no_existe" does not exist` | el exit=0 de arriba significa algo |
| Endpoint que no es Prometheus | `curl -o /dev/null -w` a `http://195.201.235.70/metrics` | `http_code=404 content_type=text/html; charset=utf-8`, exit=0 | es la consola Next.js; por eso la fuente es `:9090` |
| **Defecto del capitán** | `curl -o /dev/null -w` a `http://195.201.235.70:9090/api/v1/query?query=up` | **`external_9090_http_code=000`, exit=7** | **reproducido**: la URL externa **no** es alcanzable |

**★ El defecto del capitán queda reproducido por segunda vez, desde este lado, con el `http_code` a la vista.** Y su consecuencia está medida: `http_code=000` + `exit=7` **no** es un vector vacío silencioso a nivel de exit code — pero **quien lea solo el cuerpo de la respuesta sí ve un vector vacío**, que tiene la forma exacta de un cero. La forma correcta, usada en toda §4, es **loopback vía ssh** (`curl -s 'http://localhost:9090/api/v1/query?query=…'` **ejecutado en el VPS**), que devuelve exit=0 con datos. **La URL externa no se usa en ninguna evidencia de este documento.**

---

## 6. NO COMPUTADO, CON SU RAZÓN

| No computado | Razón exacta |
|---|---|
| Efecto de #879 sobre el funnel | El runtime sirve `901eb947`, anterior a `1928b399`. **La puerta no abrió.** |
| Drop de `verify_mismatch` | Requiere el binario nuevo emitiendo. Medido, no se movió — pero con el binario viejo, lo cual **no** lo refuta. |
| Aparición de `seeded_fresh` | Ídem. Vector vacío con el binario viejo. |
| `uptime` de `sim-ctl` | `process_start_time_seconds{job="sim-ctl"}` = **vector vacío** ⇒ **NO COMPUTADO, no cero**. La métrica no la exporta este proceso. |
| `no_simulation_row` | **NO MEDIDA.** No figura entre los 4 `outcome` de `arbx_sim_funding_total` a las 14:10:57Z. No se reporta como 0. |
| Antigüedad del arranque | **ESTIMACIÓN, declarada como tal.** `2468/733 ≈ 3,4 h` (método t160) y `2786/733 ≈ 3,8 h`, calculadas con el ritmo del camino caro de t160 — **no** con una medición del arranque, que no existe. |
| Fecha del arranque | **NO COMPUTADO.** Se sabe que **no hubo arranque nuevo** por el avance del contador (§3), no por la métrica de uptime. |

**Regla aplicada:** un «no medido» **no se escribe como 0**, y un campo sin productor se reporta como no computado con su razón.

---

## 7. LA CAPACIDAD, MEDIDA — Y POR QUÉ NO SE COMPARA

Ventana entre las dos lecturas completas: `t161-d @14:03:52Z` (ts `1791468232.641`) → `t163 @14:10:57Z` (ts `1791468657.570`) = **424,929 s = 7,082 min**.

| Serie | Delta | Tasa medida |
|---|---|---|
| `anvil` (camino del fork) | `2639 → 2791` = +152 | **1287,7/h** |
| `revm` (camino barato) | `38038 → 39143` = +1105 | **9361,6/h** |
| `slot_unresolved` | `2634 → 2786` = +152 | 1287,7/h |
| `verify_mismatch` | `10536 → 11144` = +608 | 5151,0/h |

**Y aquí hay que ser preciso, no conveniente:** las cifras de referencia de t160 eran **736/h** (camino caro) y **~123.300/h** (barato). Lo medido aquí **no coincide con ninguna de las dos** — `1287,7/h` está por encima de `736/h`, y `9361,6/h` muy por debajo de `~123.300/h`.

**No se declara que el techo se movió, ni que t160 se equivocó.** Las ventanas son distintas (425 s aquí), el método con que t160 obtuvo `736/h` y `~123.300/h` **no se reprodujo en esta tarea**, y una ventana corta sobre un flujo a ráfagas no es comparable con un techo derivado por otro camino. **Son dos mediciones no comparables, y así se reportan.**

**La pregunta de la capacidad sigue NO COMPUTADA**, y ahora con más razón: la pregunta (*«si el fondeo se arregla, ¿el límite pasa a ser la latencia del fork?»*) **presupone el fondeo arreglado**, y el fondeo **no está arreglado en el runtime**. No se elige un límite por conveniencia.

---

## 8. LA PREDICCIÓN: QUÉ ES PRE-REGISTRADO Y QUÉ NO

**Declaración honesta, porque aquí es donde una tarea como ésta se puede contaminar.**

La predicción de esta tarea **no se pre-registró en un archivo nuevo**, y **no se va a escribir uno ahora**. Escribir hoy un `FUND-FUNNEL-02.PREDICCION.txt` — *después* de haber leído los contadores de §4 — sería fabricar una pre-registración, y una pre-registración fabricada es peor que no tener ninguna.

Lo que **sí** está fijado antes de esta medición, y es verificable:

1. **El texto del contrato de t163** contiene la predicción literal, en las dos series, y fue emitido por el capitán **antes** de que esta tarea arrancara: *«(1) `verify_mismatch` por INTENTO tiene que DROPEAR… (2) `seeded_fresh` tiene que DEJAR DE ESTAR VACIO»*.
2. **`docs/backend/FUND-FUNNEL-01.PREDICCION.txt`** (blob `773b9c9582a80caa676ec212e5428115079081d5`), commiteada en t161 **antes** de mirar, contiene P1 y P2 en los mismos términos.
3. **Orden de operaciones de esta tarea:** el primer dato leído fue *la puerta* (`deploy.sha`, §2) — **no** los contadores. Los contadores de §4 se leyeron después. La secuencia está en el registro de comandos.

⇒ La predicción es **heredada y pre-existente**, no nueva ni retroactiva. Los criterios de falsación se respetan tal como se escribieron.

---

## 9. CÓMO CIERRA ESTE VEREDICTO

Condición, **exacta y mecánica**. Ninguna parte requiere juicio:

1. `curl -s --max-time 20 http://195.201.235.70/api/status` ⇒ `deploy.sha` **≠ `901eb947…`**;
2. **y** ese SHA es `77b42b3d…` o un descendiente que contenga `1928b399` (verificable con `git rev-parse` / `git merge-base --is-ancestor` sobre `origin/main`);
3. **y** el job `Deploy to VPS` de ese run tiene `conclusion` **no vacía**.

Recién entonces se relee `arbx_sim_funding_total` y se contrasta:
- **P1** — `verify_mismatch / intentos` deja de ser `4,0000`. **Si sigue en `~4,0000`: #879 NO movió el funnel, y eso es el hallazgo.**
- **P2** — `seeded_fresh` deja de ser vector vacío. **Si sigue vacío: no hubo un solo fondeo exitoso.**

**No se espera en bucle. No se re-dispara el benchmark contra el runtime viejo.**

---

## 10. TRAZABILIDAD

- Leído: `GET /api/status` (VPS), `gh run view 37783694993 --json status,conclusion,jobs` (con steps), `git ls-remote` a `origin/main`, Prometheus **por loopback vía ssh**, PostgreSQL por `docker exec … psql`.
- **Cero** cambios al motor, umbrales, escrituras o reinicios. **Cero** deploys, firmas o broadcast. **Paper.**
- **NO** se re-disparó el benchmark contra el runtime viejo.
- **NO** se usó la URL externa `195.201.235.70:9090` en ninguna evidencia: se midió como control negativo (§5.1) y se usó loopback vía ssh.
- Este documento toca **solo** `docs/backend/`.

**Firma:** Backend · t163 · attempt `75596f06-8b7b-4fc7-b771-6802aded28c6`
