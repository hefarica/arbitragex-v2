# FUND-FUNNEL-03 — La puerta abrió: ¿`seeded_fresh` deja de estar vacío y dropea `verify_mismatch`?

**Tarea:** t164 · **Agente:** Backend · **Modo:** SOLO LECTURA (cero cambios, cero umbrales, cero escrituras, cero reinicios) · **Paper**
**Lecturas:** `14:22:09Z` · `14:22:33Z` · `14:22:59Z` (todas con `date -u` en el VPS, canal ssh `arbx`)
**Predecesoras:** t161 (PR #890) y t163 (PR #891) dejaron esto **PENDIENTE**. **La condición de cierre que ellas escribieron es la que se ejecuta aquí.**

---

## 1. VEREDICTO

**La puerta abrió. El funnel SE MOVIÓ.** Y con **dos matices que cambian la lectura ingenua** — se dan los tres.

| Predicción (heredada, fijada ANTES) | Medido | Veredicto |
|---|---|---|
| **(1)** `seeded_fresh` **deja de estar vacío** | **`3`** — era **vector VACÍO** en las 6 lecturas pre-arreglo | **CONFIRMADA** |
| **(2)** `verify_mismatch` por intento **dropea** | de `4,0000` a **`0,0441`** — factor **90,7×** | **CONFIRMADA en valor, NO COMPARABLE en unidad** (§5) |
| El fondo **deja** de ser el 100 % de lo que llega al fork | **134 fallos downstream de fondeo** que pre-fix **no existían** | **CONFIRMADO** |

**Los dos matices, y son el hallazgo de esta tarea:**

- **★ A — La taxonomía del instrumento CAMBIÓ. `slot_unresolved` no "bajó a cero": la etiqueta dejó de existir.** Pre-fix el contador emitía 4 outcomes (`slot_unresolved`, `balance_unreadable`, `verify_mismatch`, `rpc_err`); post-fix emite **3 distintos y otros** (`verify_mismatch`, **`cache_hit`**, **`seeded_fresh`**). Leer esa ausencia como «el desajuste se fue» sería **leer un renombre como una mejora** — y el control cruzado por PG lo desmiente: **la tabla sigue registrando `sim_signer_funding_slot_unresolved` = 561 filas post-deploy** (§6).
- **★ B — El fondeo SIGUE fallando.** 561 fallos de slot en 36 min post-deploy, más `funding_balanceof_timeout` 4. **No es «el techo de fondeo cerrado»: es «el techo de fondeo AGRIETADO»**, y con número.

---

## 2. LA PUERTA, RE-VERIFICADA ANTES DE MEDIR

### 2.1 El juez del runtime

`curl -s --max-time 20 http://195.201.235.70/api/status | grep -o '"deploy":{[^}]*}'`, exit=0, en **las tres lecturas**:

```
"deploy":{"sha":"77b42b3dccc001455fda3e8d4437d973c9e98c4b","id":"37783694993","at":"2026-10-08T13:47:10Z"}
```

`deploy.sha = 77b42b3d…` — **el fix está sirviéndose**. Idéntico en `14:22:09Z`, `14:22:33Z` y `14:22:59Z` ⇒ **tres lecturas**, que es lo que la regla exige para poder decir «estable».

### 2.2 La revisión DENTRO de los contenedores

| Comprobación | Resultado | Artefacto |
|---|---|---|
| Contenedores vivos | **25** | `docker ps --format '{{.Names}}\t{{.Status}}' \| wc -l` |
| Contenedores `healthy` | **25 / 25** | `docker ps --format '{{.Status}}' \| grep -c healthy` |
| `77b42b3d` en las revisiones | **11 ocurrencias** | `docker ps -q \| xargs docker inspect --format '…revision'` `\| grep -c 77b42b3d` |
| `901eb947` en las revisiones | **0 ocurrencias** | mismo comando, `\| grep -c 901eb947` |

Los 5 servicios de aplicación llevan la etiqueta `org.opencontainers.image.revision`:

```
sim-ctl       | arbitragex-v2-sim-ctl-1       | rev=77b42b3dccc001455fda3e8d4437d973c9e98c4b | healthy
api-server    | arbitragex-v2-api-server-1    | rev=77b42b3dccc001455fda3e8d4437d973c9e98c4b | healthy
searcher-rs   | arbitragex-v2-searcher-rs-1   | rev=77b42b3dccc001455fda3e8d4437d973c9e98c4b | healthy
recon         | arbitragex-v2-recon-1         | rev=77b42b3dccc001455fda3e8d4437d973c9e98c4b | healthy
relays-client | arbitragex-v2-relays-client-1 | rev=77b42b3dccc001455fda3e8d4437d973c9e98c4b | healthy
postgres      | arbitragex-v2-postgres-1      | img=postgres:15 (sin etiqueta de revisión)   | healthy
```

**Re-verificado por medición propia, no citado del informe de otro.** `main` por el remoto (`git ls-remote https://github.com/hefarica/arbitragex-v2.git refs/heads/main`, exit=0) = `77b42b3dccc001455fda3e8d4437d973c9e98c4b`.

---

## 3. EL CASO DEL INSTRUMENTO: **CASO 1** — RESETEÓ

El contador es **ACUMULATIVO desde el arranque de `sim-ctl`**, y los dos casos invierten la conclusión. **Se declara cuál ocurrió, y se declara por observación, no por suposición:**

| Serie | Pre-deploy (t163 @14:10:57Z) | Post-deploy (t164 @14:22:09Z) | Lectura |
|---|---|---|---|
| `arbx_simulation_total{anvil}` | 2791 | **122** | **CAYÓ ⇒ el contador volvió a empezar** |
| `slot_unresolved` | 2786 | **etiqueta ausente** | — |
| `revm` | 39143 | **28** | **CAYÓ** |

**⇒ CASO 1. El deploy reinició `sim-ctl` y el contador arranca de cero.** El post se lee **directo**, no por delta contra pre-valores.

**Testigo alternativo: NO disponible.** `process_start_time_seconds{job="sim-ctl"}` devuelve **vector VACÍO** en las tres lecturas ⇒ **NO COMPUTADO, no cero**; no puede fechar el arranque. La técnica que el contrato autoriza —*«o el propio reseteo observado»*— es la que se usó, y basta: `2791 → 122` no es ambiguo.

> **Estado desde el arranque, no ventana.** Todas las cifras de contador de §4 son **acumuladas desde el reinicio (~13:47Z hasta la lectura)**. Las cifras de PG de §6 llevan **su propio corte declarado**. **No se restan entre sí**: unidades distintas (evento acumulado vs fila por ventana), lección de t160.

---

## 4. EL CONTADOR, EN TRES LECTURAS

Loopback vía ssh (forma correcta), exit=0 en todas.

| Outcome | L1 @14:22:09Z | L2 @14:22:33Z | L3 @14:22:59Z |
|---|---|---|---|
| `seeded_fresh` | **3** | **3** | **3** |
| `cache_hit` | **120** | **123** | **127** |
| `verify_mismatch` | **6** | **6** | **6** |
| `slot_unresolved` | — | — | — |
| `balance_unreadable` | — | — | — |
| `rpc_err` | — | — | — |
| outcomes distintos | **3** | 3 | 3 |
| `arbx_simulation_total{anvil}` | **122** | **126** | **130** |
| `arbx_simulation_total{revm}` | **28** | **28** | **28** |

### 4.1 La identidad NUEVA del fork: `cache_hit + seeded_fresh = anvil`

La identidad pre-fix era `slot_unresolved + rpc_err = anvil` (EXACTA en 6 lecturas, terminando en `2786 + 5 = 2791`). **Su partición post-fix es otra:**

```
L1  120 + 3 = 123   vs anvil 122   -> Δ = 1
L2  123 + 3 = 126   vs anvil 126   -> EXACTO
L3  127 + 3 = 130   vs anvil 130   -> EXACTO
```

**En L2 y L3, EXACTA.** En L1 hay **Δ = 1** y tiene causa medida: los dos scrapes de L1 están a **8 ms** (`funding` ts `1791469330.015` vs `simtotal` ts `1791469330.023`) y un incremento cabalgó la frontera. **Se declara la discrepancia, no se esconde.**

⇒ **La población del fork ya no se parte en «no resuelto + error RPC»: se parte en «leído de caché» + «sembrado fresco».** Esa es la transición que esta tarea venía a medir.

### 4.2 El drop de `verify_mismatch`

```
pre-fix : 11144 / 2786 = 4,0000 por intento      <- denominador: slot_unresolved (por INTENTO)
post-fix:     6 /  136 = 0,0441 por intento      <- denominador NUEVO: seeded_fresh+cache_hit+verify_mismatch
factor  : 90,7×
```

**CONFIRMADA en valor. Declarada NO COMPARABLE en unidad**, y por eso: el denominador pre-fix (`slot_unresolved`) **ya no existe como etiqueta**, así que el 4,0000 y el 0,0441 **no se miden con el mismo instrumento**. El número se da, la comparabilidad se niega, y **la conclusión no se apoya en él**: se apoya en `seeded_fresh` (§5), que es DIRECTO.

### 4.3 Serie plana — con tres lecturas, no dos

`seeded_fresh = 3`, `verify_mismatch = 6` y `revm = 28` están **planos en las tres lecturas** (ventana L1→L3 = **49,020 s**). La regla de t161 —*dos lecturas iguales son dos muestras*— **queda satisfecha**, y la ventana se declara: **49 s es corta**. No se afirma más que «plano en 49 s».

---

## 5. LA SERIE QUE DECIDE: `seeded_fresh`

`arbx_sim_funding_total{outcome="seeded_fresh"}` — consultada **explícitamente** además de aparecer en la partición, exit=0:

```
L1 @14:22:09Z  ->  3
L2 @14:22:33Z  ->  3
L3 @14:22:59Z  ->  3
```

**Pre-fix era `{"status":"success","data":{"resultType":"vector","result":[]}}` — vector VACÍO — en las SEIS lecturas de t160/t161/t163.** Es la **única evidencia DIRECTA de un fondeo exitoso** y así quedó pre-registrada en `docs/backend/FUND-FUNNEL-01.PREDICCION.txt` (blob `773b9c9582a80caa676ec212e5428115079081d5`), **antes** de que la puerta abriera.

**⇒ P2 CONFIRMADA, y es la que decide.** Refuerzo independiente: `cache_hit = 127` es una etiqueta que **no existía pre-fix** y que **sólo puede existir si un fondeo tuvo éxito antes** (algo que se cachea tuvo que sembrarse). Y los 3 seedings explican los 127 reúsos: **3 ranuras se sembraron y el resto del tráfico las reutiliza en vez de volver a fondear** — que es exactamente el comportamiento que #879 habilitaba.

**No puede ser un artefacto de renombre**: `seeded_fresh` no existía antes, así que no hay etiqueta vieja cuya desaparición esté disfrazando este número.

---

## 6. CONTROL CRUZADO POR PG — Y EL CORTE QUE HACE FALTA

Fuente **independiente** del contador. `psql -tAc … --set=ON_ERROR_STOP=1`, exit=0.

### 6.1 El error de instrumento que se corrige aquí

El corte de 1 h **cruza el deploy** (`13:47:10Z`): mezcla tráfico pre y post. Sin cortar, `sim_signer_funding_slot_unresolved = 902` **se leería como si el fondeo siguiera fallando al ritmo de antes**, cuando en realidad es **residuo pre-fix envejeciendo dentro de la ventana**. Se corta.

### 6.2 Pre-deploy (residuo dentro de 1 h)

| Fuente | Valor | Artefacto |
|---|---|---|
| `slot_unresolved` pre-deploy en 1 h | **339** | `WHERE simulated_at > now()-'1 hour' AND simulated_at <= '2026-10-08T13:47:10Z'` |

### 6.3 **POST-deploy** (`simulated_at > '2026-10-08T13:47:10Z'`) — 36 min

| `fail_reason` | count | ¿existía pre-fix? |
|---|---|---|
| `candidate_incomplete:amount_in_wei_zero` | 4060 | sí |
| **`sim_signer_funding_slot_unresolved`** | **561** | sí — **y SIGUE** |
| **`reverted: TransferHelper: TRANSFER_FROM_FAILED…`** | **101** | **NO — NUEVO** |
| **`rpc_error: (code: 3, message: execution reverted…)`** | **18** | **NO — NUEVO** |
| **`sim_timeout`** | **15** | **NO — NUEVO** |
| `funding_balanceof_timeout` | 4 | sí |
| **Σ** | **4759** | |

```
4060 + 561 + 101 + 18 + 15 + 4 = 4759   -> la partición cierra EXACTA
561 + 101 + 18 + 15 + 4 = 699           -> revert_risk_pct EXACTO
699 + 4060 = 4759                       -> revert_risk_pct + amount_in_wei_zero = total EXACTO
```

**★ El hallazgo que el contador no puede dar.** Tres `fail_reason` **aparecen por primera vez** post-deploy: `TransferHelper: TRANSFER_FROM_FAILED` (101), `execution reverted` (18) y `sim_timeout` (15). **Son fallos que ocurren DOWNSTREAM del fondeo** — la ejecución llegó a intentarse y falló más adelante. Pre-fix, la distribución entera era `amount_in_wei_zero` + `slot_unresolved` + `balanceof_timeout` y **nada más**: **el 100 % moría en el fondeo y nadie llegaba a ejecutar.**

⇒ **134 filas atravesaron el fondeo y fallaron después.** Eso es el funnel moviéndose, medido por una fuente que **no** es el contador, y **no** afectada por el renombre de §1-A.

**★ Y el contrapeso honesto: `slot_unresolved = 561` sigue ahí.** El fondeo **no dejó de fallar**. Tabla completa 1 h (`8411`), con la misma partición cerrando EXACTA (`7376 + 902 + 97 + 17 + 13 + 6 = 8411`) y `revert_risk_pct = 1035 = 902+97+17+13+6` EXACTO — **`revert_risk_pct` marca toda simulación que produjo traza**; `amount_in_wei_zero` es el rechazo previo a la simulación y no la tiene.

**Descomposición de la frontera, declarada:** `902 (1 h) ≈ 561 (post) + 339 (pre)` — da **900, Δ=2**. La frontera no cae exactamente en `13:47:10Z` (filas escritas en ese instante o desfase de reloj). **Se declara la Δ, no se ajusta.**

### 6.4 Defecto #8 — TERCERA tarea consecutiva

`raw_trace IS NOT NULL` = **0 sobre 8411 filas positivas** (y 0 sobre 4759 post-deploy). `revert_risk_pct` es el campo real. **Confirmado por tercera tarea seguida**: un control que lee cero sobre la población positiva no es un control.

### 6.5 Controles negativos

| Control | Resultado |
|---|---|
| Canal SQL `SELECT 1` | `1`, exit=0 |
| Negativo `SELECT esto_no_existe` | **exit=1** + `ERROR: column "esto_no_existe" does not exist` |
| Externo `:9090` (defecto #9) | **`external_9090_http_code=000`, exit=7** — reproducido por tercera vez |

**Ninguna evidencia de este documento usa la URL externa `195.201.235.70:9090` ni `195.201.235.70/metrics`.**

---

## 7. EL FALSO ROJO DEL CI, COMPROBADO CON MEDICIÓN PROPIA

`gh run view 37783694993 --json status,conclusion,jobs`, exit=0:

```
run-level : status=completed  conclusion=failure  headSha=77b42b3d…  updatedAt=2026-10-08T14:16:36Z
jobs      : Wait for all deployment gates => completed/success [13:22:52Z -> 13:46:12Z]
            Deploy to VPS                 => completed/failure [13:46:54Z -> 14:16:35Z]
```

**El run cierra en `failure` y el deploy aterrizó bien.** No se toma la palabra de nadie: **se prueba desde aquí** — el runtime sirve `77b42b3d` (§2.1), los 5 servicios de aplicación llevan `77b42b3d` en su etiqueta de revisión, `901eb947` tiene **0 ocurrencias**, y **25/25 contenedores están `healthy`** (§2.2).

⇒ **El juez del deploy es `deploy.sha` del runtime, NO la `conclusion` del run.** Y **esto no es el veredicto de esta tarea**: es un **falso ROJO del CI** (carrera entre `[9/9b] G4 DEPLOY-VERAZ`, que exige los 7 servicios, y `[8/9]`, que sólo espera 3). **El arreglo del CI es otra tarea y merece su ciclo.** No se confunde el color del gate con el estado del arreglo.

---

## 8. EL TECHO: ¿SE MOVIÓ A LA CAPACIDAD DEL CAMINO CARO?

Ventana **L1→L3 = 49,020 s** (ambas post-reset, mismo proceso):

| Serie | L1→L3 | Tasa |
|---|---|---|
| `anvil` (camino del fork) | 122 → 130 | **587,5/h** |
| `cache_hit` | 120 → 127 | **514,1/h** |
| `revm` | 28 → 28 | **0/h** (plano en 3 lecturas) |

**Y aquí hay que ser preciso, no conveniente.** Las referencias disponibles son `anvil 1287,7/h` y `revm 9361,6/h` (t163, ventana de **424,929 s**) y `736/h` (t160, **método distinto**). Lo medido aquí —`587,5/h`, ventana de **49 s**— **no coincide con ninguna**, y **`revm` cayó a 0**.

**No se declara que el techo se movió a la capacidad del camino caro, ni que haya empeorado.** Las ventanas difieren en un factor **8,7** (49 s vs 425 s), `revm` lleva 49 s plano — que es corto—, y el flujo puede ser a ráfagas. **Son mediciones NO comparables y así se reportan.**

Lo que **sí** se puede afirmar con artefacto: **cambió la forma de la partición del fork** — pre-fix `revm` (39143) dominaba a `anvil` (2791) por **14×**; post-fix `anvil` (130) supera a `revm` (28) por **4,6×**. **El hecho se reporta; el mecanismo NO COMPUTADO** (no se instrumentó, y atribuirlo sería adivinar).

---

## 9. LO QUE NO SE PUEDE COMPUTAR, CON SU RAZÓN

| No computado | Razón exacta |
|---|---|
| **Tasa de fallo de fondeo de la población que llega al fork** | **La identidad que la definía ya no existe.** Pre-fix era `slot_unresolved + rpc_err = anvil`, EXACTA. Post-fix su partición es `cache_hit + seeded_fresh = anvil`, **y `rpc_err` desapareció como etiqueta**. Sin rótulo no hay población, **sin población no hay tasa**. Se da el conteo crudo (§6.3: 561 + 4 de fondeo) y se niega la tasa. |
| Comparabilidad de `4,0000 → 0,0441` | Denominadores de **taxonomías distintas** (§4.2). El número se da; la comparación no se sostiene. |
| `process_start_time_seconds{job="sim-ctl"}` | **Vector VACÍO** en las 3 lecturas ⇒ **NO COMPUTADO, no cero**. El arranque **no se fecha con esta métrica**; se infiere del reseteo observado (§3). |
| `no_simulation_row` | **NO MEDIDA.** No figura entre los 3 `outcome` observados. No se reporta como 0. |
| Mecanismo del vuelco `revm` ↔ `anvil` | **NO COMPUTADO.** Hecho medido (§8); causa no instrumentada. |
| Antigüedad del arranque | **ESTIMACIÓN:** el deploy `at=13:47:10Z` y la L1 `14:22:09Z` ⇒ **~35 min**. Es una **cota superior** (el job no cerró hasta `14:16:35Z`). Declarada como estimación. |

**Regla aplicada:** un «no medido» **no se escribe como 0**, y **una etiqueta renombrada no se lee como un cero**.

---

## 10. PREDICCIÓN: DE DÓNDE VIENE

**No se re-inventa.** Es **heredada** y estaba fijada **antes** de que la puerta abriera, verificable en dos artefactos previos a este deploy:

1. `docs/backend/FUND-FUNNEL-01.PREDICCION.txt` — blob `773b9c9582a80caa676ec212e5428115079081d5`, commiteada en t161 **antes de mirar** (P2 es literalmente «`seeded_fresh` APARECE (> 0)»).
2. El **texto del contrato de t164**, emitido por el capitán, con las dos series en los mismos términos.

**Y se respeta el estándar de t163:** no se escribe un archivo de pre-registración nuevo. Redactarlo *después* de leer los contadores sería fabricarlo. **El primer dato leído fue LA PUERTA** (`deploy.sha`, §2.1); los contadores vinieron después.

---

## 11. TRAZABILIDAD

- Leído: `GET /api/status` (VPS), `docker ps` / `docker inspect` (etiquetas de revisión), Prometheus **por loopback vía ssh**, PostgreSQL por `docker exec … psql`, `gh run view … --json jobs`, `git ls-remote`.
- **Cero** cambios al motor, umbrales, escrituras o reinicios. **Cero** deploys, firmas o broadcast. **Paper.**
- **NO** se re-disparó el benchmark: el runtime **ya no es el viejo**.
- **NO** se usó la URL externa `:9090` ni `/metrics` en ninguna evidencia: la primera se midió como control negativo (§6.5).
- Este documento toca **solo** `docs/backend/`.

**Firma:** Backend · t164 · attempt `e05b64d0-caa4-4d85-9d8a-9a0adca58b58`
