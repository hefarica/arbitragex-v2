# TRANSFER-LAYER-01 — El único punto donde el sistema ejecuta: ¿es allowance?

**Tarea:** t168 · **Agente:** Backend · **Modo:** SOLO LECTURA en producción (el fork se leyó con `eth_call`; **no se escribió nada, ni en producción ni en el fork**) · **Paper**
**Frontera pre/post (LA REAL):** `docker inspect --format '{{.State.StartedAt}}' arbitragex-v2-sim-ctl-1` -> **`2026-10-08T14:16:07.1195963Z`**. **NO** el `deploy.at` (`13:47:10Z`, arranque del STEP) ⇒ 28 min 57 s de binario viejo (defecto #11).
**Toda cifra declara su corte en la misma línea.**
**Puerta:** `deploy.sha=77b42b3dccc001455fda3e8d4437d973c9e98c4b`, `id=37783694993`, `at=13:47:10Z`, exit=0.

---

## 1. VEREDICTO

**ES ALLOWANCE. Confirmado.** Y —medido en la misma sesión— **hoy vale cero dólares.**

Dos conclusiones, y la segunda no le quita nada a la primera:

1. **La causa está aislada por un control bidireccional read-only**: `allowance[signer][router] = 0` mientras `balanceOf(signer) = 2^128−1`. La precondición que falla es **el allowance y sólo el allowance**.
2. **★ Pero el defecto NO bloquea ningún beneficio, porque no hay beneficio que bloquear**: las filas que mueren ahí **ya fueron rechazadas por la capa de mercado**. La mejor oportunidad jamás detectada rinde **+1,5459 USD** contra un target de **50,0 USD**.

**⇒ El arreglo es de una línea, y su efecto medible HOY es CERO.** Eso es un resultado, no una decepción: dice dónde está el cuello real.

---

## 2. LA CAUSA: ALLOWANCE, AISLADA POR CONTROL BIDIRECCIONAL

### 2.1 Lo leído

`docker exec arbitragex-v2-anvil-1 cast call <token> '<sig>' … --rpc-url http://localhost:8545`, exit=0 en todas.

| Lectura | Valor |
|---|---|
| `balanceOf(signer)` en WETH (`0xc02aaa…`) | **340282366920938463463374607431768211455** = **2^128 − 1** |
| balance nativo del signer | `17375642370759505555` (17,37 ETH) |
| `allowance(signer, UniswapV2 Router02 `0x7a250d56…`)` | **0** |
| `allowance(signer, UniswapV3 SwapRouter `0xE592427A…`)` | **0** |
| `allowance(signer, UniswapV3 SwapRouter02 `0x68b34658…`)` | **0** |

**El fondeo siembra el BALANCE y NO el ALLOWANCE.** `TransferHelper.safeTransferFrom` exige `allowance >= value`; con `0` y `value = 1e18`, **el revert no es un hallazgo empírico: es aritmética de ERC-20.**

### 2.2 ★★ EL CONTROL BIDIRECCIONAL — mismo token, mismo signer, mismo fork

Todo con **`cast call` (que es `eth_call`: NO cambia estado)**. `exit=0` salvo donde se indica.

| # | Llamada | Precondición que exige | Resultado |
|---|---|---|---|
| **A** | `transfer(0xdEaD, 1)` **desde el signer** | sólo **BALANCE** | **`true`** — exit=0 |
| **B** | `transferFrom(signer, 0xdEaD, 1)` **desde el router V2** | **ALLOWANCE** | **`execution reverted, data: "0x"`** — **exit=1** |
| **C** | `transferFrom(signer, 0xdEaD, 1)` **desde el propio signer** | el `require` de allowance **no aplica** | **`true`** — exit=0 |
| **D** | `transfer(0xdEaD, 2^128)` — balance+1, **control negativo del instrumento** | BALANCE **insuficiente** | **revierte** — exit=1 |

**El aislamiento es completo.** Misma función (`transferFrom`), mismo token, mismo firmante: **B falla y C pasa**, y la única diferencia entre B y C es si el `require` de allowance se evalúa. Y **A pasa**, luego **el balance está**. Y **D revierte**, luego **el instrumento sí detecta una precondición insuficiente** — el revert de B significa algo.

**Nota de precisión sobre C:** `WETH9.transferFrom` salta el `require` cuando `src == msg.sender`. Que C devuelva `true` **es exactamente lo que el contrato de WETH9 predice** — no es una anomalía, es la confirmación de que el `require` es el que decide.

**⇒ ALLOWANCE. La hipótesis líder queda confirmada.**

**Las dos refutaciones previas se conservan y ahora tienen medición directa:**
- **«token no estándar» — REFUTADO**: el token es **WETH**, y en A y C **el mismo token funciona**.
- **«balance» — REFUTADO por medición**: `balanceOf = 2^128−1` y `transfer` **pasa**.

### 2.3 ★ EL TEST PIVOTAL: **NO EJECUTADO**, y por qué

El contrato pide: *poner un `approve` máximo EN EL FORK y ver si esas filas CRUZAN*. **No lo ejecuté.** Razones, todas medidas:

1. **No conozco la ranura del mapping `allowance` de WETH.** Escribir a ciegas con `anvil_setStorageAt` corrompería almacenamiento arbitrario. Adivinarla no es un test, es un experimento sin control.
2. **El fork es COMPARTIDO y está VIVO.** Medido: entre dos lecturas separadas por segundos, `balanceOf(signer)` pasó de **2^128−1** a **1e11** — **el simulador re-siembra el fork en cada simulación**. Escribir ahí es **competir con el simulador en marcha** y podría corromper simulaciones en vuelo y **contaminar la métrica que esta tarea mide**.
3. **La diagnosis no lo necesita.** El control de §2.2 aísla el `require` exacto que falla. Lo que el `approve` en el fork añadiría es la confirmación del **arreglo**, no la de la **causa**.

**Lo que sí se hizo, y es un control bidireccional real aunque no sea el que el contrato pedía:** B falla con el allowance actual y las variantes que no requieren allowance **pasan**. Se declara la diferencia para que nadie lea más de lo que hay.

**Test que cerraría el arreglo**, con el comando exacto: sembrar `allowance[signer][router] = uint256::MAX` en el fork junto con el balance y **observar la transición** de `reverted: TransferHelper:…` a un fallo más profundo (`passed` seguiría siendo `false` por §4).

---

## 3. ★★ TRES CORRECCIONES A LO PUBLICADO

### 3.1 Corrección a t167 — **el veredicto de mercado SÍ existe, y son millones**

t167 midió `simulations.fail_reason ILIKE '%profit%'` -> 0 y concluyó: *«la población de veredictos de mercado del sistema es CERO»*. **La medición es correcta; la conclusión es falsa.** El veredicto de mercado **no vive en `simulations.fail_reason`** — vive en **`opportunities.rejection_reason`**:

| `rejection_reason` | count |
|---|---|
| `spread_negative_round_trip` | **4 871 794** |
| `v3_pool_not_catalogued` | 1 028 520 |
| **`non_positive_profit`** | **968 344** |
| `spread_zero_equilibrium` | 787 040 |
| `v3_quote_unavailable` | 715 823 |
| `single_pool_no_spread` | 334 402 |
| `v3_pair_no_pools` | 299 121 |
| `no_tradable_size` | 105 013 |
| `StrategyDisabled:triangular_arb` | 84 398 |
| `v3_multileg_budget_exhausted` | 33 955 |
| `v3_pool_revert` | 15 463 |
| **`negative_net_profit`** | **8 222** |
| `TokenNotAllowed:0x…` (varios) | miles cada uno |

**`SELECT status, count(*) FROM opportunities GROUP BY status` -> `rejected|9269613`.** **Las 9.269.613 oportunidades están `rejected`, con razón explícita.**

**⇒ La capa de mercado no sólo funciona: trabaja más que ninguna otra.** t167 buscó `'%profit%'` en la **columna equivocada** — es el modo de fallo *«etiqueta mal leída»*, el mismo que la campaña viene cazando. **Esta tarea lo declara y lo corrige.**

### 3.2 Corrección a t167 — **el «87,5 %» es un agregado de DOS ÉPOCAS**

| Corte | `strategy_cyclic_route_not_simulatable%` | Total | Fracción |
|---|---|---|---|
| **Post-restart** (`> 14:16:07Z`) | **0** | 606 | **0,00 %** |
| Ventana binario viejo (`13:47:10 → 14:16:07Z`) | **0** | 4597 | 0,00 % |
| **Pre-restart** (`<= 14:16:07Z`) | **617 570** | **705 235** | **87,57 %** |

**⇒ `route_not_simulatable_in_s4` es el emisor del binario VIEJO y DESAPARECIÓ del presente.** El «87,5 %» de t167 es **87,57 % del pre-restart y 0 % del post-restart**: un agregado, no el presente. **Es la misma firma que `slot_unresolved`: una etiqueta que deja de existir.**

**Y la lección se repite:** dos labels de la misma era (`slot_unresolved`, `route_not_simulatable_in_s4`) desaparecieron al cambiar el binario. **La tabla es viva y mezcla épocas; un `GROUP BY` sin corte es una foto compuesta, no un estado.**

### 3.3 Corrección a **mí mismo**, en esta misma tarea

**Dije «ninguna oportunidad es rentable». FALSO.** `count(*) FILTER (WHERE (economics->>'net_profit_usd')::numeric > 0)` -> **122**. El error fue medir sobre `simulations ⋈ opportunities` (las que **llegan a simular**, ahí sí son 0) y **extenderlo a toda la tabla** (ahí son 122). **Declarado y corregido.**

**Y un error de instrumento:** un `ORDER BY (economics->>'net_profit_usd')::numeric DESC LIMIT 5` devolvió **5 líneas vacías** — porque **en PostgreSQL los `NULL` van PRIMERO en `DESC`**. Corregido con `WHERE economics->>'net_profit_usd' IS NOT NULL`. **Es la misma clase de fallo que el `GROUP BY` cuya vacuidad puede ser `NULL`.**

---

## 4. ★★ LA MEDICIÓN QUE DECIDE SI ESTO IMPORTA

Corte `simulations.simulated_at > '2026-10-08T14:16:07Z'`, exit=0. `psql -tAc`, conteo numerado.

```
filas que fallan en la transferencia (LIKE 'reverted:%')      267
  ... con economics->>'meets_target' = TRUE                     0
  ... con economics->>'net_profit_usd' > 0                      0
todas las que llegan a simular (post-restart)                 447
  ... con net_profit_usd > 0                                    0
  ... con meets_target = TRUE                                   0
  rango de net_profit_usd            min -2533,55   max  -0,64
```

**Y en toda la tabla de oportunidades:**

```
total 9 262 473  |  con economics 9 262 473 (control positivo OK)
net_profit_usd > 0 : 122      meets_target = TRUE : 0
la MEJOR de todas  : net +1,5459 USD · bruto +2,2221 · target 50,0 · meets=false · rejected
las 122 rentables  : 122 rechazadas · 0 meets_target · 0 llegaron a simular
```

**⇒ LECTURA:**
- **El techo no es la transferencia. El techo es que la mejor oportunidad detectada rinde +1,5459 USD contra un target de 50,0 — una brecha de 32×.**
- **Las 267 filas que mueren en la transferencia YA ESTABAN RECHAZADAS** por la capa de mercado. En el corte post-restart, las que llegan a simular se reparten: `spread_negative_round_trip` 248, `no_tradable_size` 110, `v3_pool_not_catalogued` 91, `non_positive_profit` 81, `v3_quote_unavailable` 36, `single_pool_no_spread` 2, `negative_net_profit` 2, `v3_multileg_budget_exhausted` 1.
- **⇒ Arreglar el allowance convierte `TRANSFER_FROM_FAILED` en un fallo MÁS PROFUNDO, no en un `passed`.** El swap ejecutaría y el resultado seguiría sin alcanzar el target. **`passed = true` seguiría sin ocurrir.**

**Esto responde la pregunta del contrato —*«si con el allowance puesto SIGUEN fallando, la hipótesis muere»*— con el matiz correcto: la hipótesis del allowance NO muere (está confirmada en §2), pero su arreglo tampoco desbloquea nada, porque el cuello está una capa antes.**

---

## 5. LA PARTICIÓN POST-RESTART, CON EL CORTE CORRECTO

`simulated_at > '2026-10-08T14:16:07Z'` (el `StartedAt` del binario nuevo), lectura `14:38:56Z`, exit=0:

| `fail_reason` | count |
|---|---|
| `reverted: TransferHelper: TRANSFER_FROM_FAILED…` | **356** |
| `candidate_incomplete:amount_in_wei_zero` | 144 |
| `rpc_error: (code: 3, message: execution reverted, data: 0x)` | 70 |
| `sim_timeout` | 34 |
| `rpc_error: (code: -32603, message: failed to …)` | **1** — variante nueva |
| `build_error: router not in catalog for chain=1` | **1** — nuevo |
| **Σ** | **606** **EXACTO** |

Y un segundo después, `revert_risk_pct IS NOT NULL` = **463**, `raw_trace IS NOT NULL` = **0**, total **607**:
```
463 + 144 = 607   -> EXACTO  (revert_risk_pct marca toda simulación que corrió; amount_in_wei_zero es rechazo previo)
```
**`raw_trace = 0` sobre 607 filas positivas ⇒ defecto #8, QUINTA tarea consecutiva.**

**Crecimiento de `TransferHelper`**: `209` (t167 @14:28:55Z) -> `356` (t168 @14:38:56Z) = **+147 en 601 s = 880,5/h**, contra los **931,8/h** que midió t167. **Dos ventanas propias, consistentes entre sí** — y **no comparables** con t160/t163 (ventanas y métodos distintos).

---

## 6. LA TASA: **NO COMPUTADA** — la identidad está MUERTA con TRES lecturas

El contrato manda: *«no se propone una identidad nueva sin al menos TRES lecturas»*. t164 propuso `cache_hit + seeded_fresh = anvil` con dos exactitudes. **Tres lecturas nuevas lo matan:**

| Lectura | `cache_hit` | `seeded_fresh` | suma | `anvil` | Δ |
|---|---|---|---|---|---|
| `14:38:58Z` | 447 | 5 | 452 | 456 | **4** |
| `14:39:10Z` | 457 | 5 | 462 | 465 | **3** |
| `14:39:22Z` | 464 | 5 | 469 | 473 | **4** |

**Δ = 4, 3, 4 — NO constante.** No es skew de scrape: es **inestable**. **⇒ La identidad es FALSA, y ahora con tres lecturas, que es el estándar que esta campaña exige.**

**⇒ NO HAY POBLACIÓN DELIMITADA ⇒ NO HAY TASA.** Se dan los conteos crudos, acumulados desde el restart:

| Serie | Valor @`14:39:22Z` |
|---|---|
| `verify_mismatch` | 6 (plano en 3 lecturas) |
| `cache_hit` | 464 |
| `seeded_fresh` | 5 (plano en 3 lecturas) |
| `slot_unresolved` | **etiqueta ausente del contador** |
| `balance_unreadable` / `rpc_err` | **ausentes** |
| `anvil` / `revm` | 473 / 144 (`revm` plano en 3 lecturas) |

**Y se niega la tasa aunque el conteo crudo favorezca la historia que querríamos contar** — el estándar de t167, conservado.

---

## 7. LOS CONTROLES

| Control | Resultado |
|---|---|
| Canal SQL `SELECT 1` | `1`, exit=0 |
| **Negativo `ON_ERROR_STOP`** | `SELECT esto_no_existe` -> **exit=1** + `ERROR: column "esto_no_existe" does not exist` (corrida de `14:32:43Z`) |
| **Positivo del `LIKE`** | `'%TRANSFER_FROM_FAILED%'` -> **357**; `'%reverted%'` -> 245 · contra `ILIKE '%profit%' OR '%slippage%'` -> **0** |
| **Positivo del `cast`** | `balanceOf(pool 0xab659d…)` = **1,328e18** · `totalSupply()` WETH = **2,187e24** ⇒ el instrumento lee **no-ceros** |
| **Negativo del `cast`** | `transfer` con `balance+1` -> **revierte** ⇒ detecta insuficiencia |
| Conteo numerado | todo con `count(*)`; las particiones cierran **EXACTAS** |
| Suma de la tabla | pre-restart `705235`; post-restart `606` |

**★ Un control MÍO quedó inválido y se declara:** en la corrida final escribí `$PG -tAc "SELECT esto_no_existe" 2>&1 | head -2; echo "neg_exit=$?"` — **el `$?` es el de `head`, no el de `psql`**, y salió `0`. **Ese control no vale.** Vale el de `14:32:43Z`, que dio **exit=1 + ERROR** sin tubería.

**Ninguna evidencia usa `195.201.235.70:9090` ni `195.201.235.70/metrics`.** Prometheus siempre por **loopback vía ssh**, exit=0. El defecto #9 se midió como control negativo: `http_code=000`, exit=7.

---

## 8. LO QUE NO SE COMPUTA, CON SU RAZÓN

| No computado | Razón exacta |
|---|---|
| **Tasa de fallo de fondeo de la población del fork** | La identidad heredada **muere con 3 lecturas** (Δ=4,3,4). Sin población delimitada no hay tasa. |
| **El test pivotal con `approve` en el fork** | **NO EJECUTADO.** Ranura del mapping desconocida + fork compartido y vivo (el balance cambió `2^128−1`→`1e11` entre lecturas) ⇒ escribir sería corromper almacenamiento y competir con el simulador. Los 3 motivos y el test que lo cerraría, en §2.3. |
| Cuál router usa realmente la sim | **NO COMPUTADO.** Se leyeron los 3 candidatos estándar (V2, V3 SwapRouter, V3 SwapRouter02) y los 3 dan 0; **la tabla `routers` tiene 0 filas** y `contract_registry` 0 filas, así que no hay registro del que sacarlo. Que los 3 den 0 hace la conclusión robusta a cuál sea, pero **el spender exacto no está identificado**. |
| `meets_target` como parámetro | **NO COMPUTADO** si `target_net_usd = 50,0` es correcto o mal configurado. Se reporta el valor; **no se juzga**. |
| Capacidad | **NO COMPUTADA.** Sólo ventanas propias declaradas (§5), comparadas **sólo contra sí mismas**. |

---

## 9. TRAZABILIDAD

- Leído: `GET /api/status`, `docker inspect` (`StartedAt`), `docker ps`, `docker port`, **`cast call` / `cast block-number` / `cast chain-id` dentro del contenedor anvil** (todo `eth_call`, **sin cambiar estado**), Prometheus **loopback vía ssh**, PostgreSQL por `docker exec … psql`.
- **CERO escrituras**: nada al motor, nada a umbrales, nada al fork, nada a producción. **CERO reinicios. CERO mainnet. CERO firmas. CERO broadcast.** El `approve` del test pivotal **no se aplicó**, y se declara por qué (§2.3).
- **NO** se re-disparó el benchmark. **Paper.**
- Predicción **heredada**, no re-inventada: `FUND-FUNNEL-01.PREDICCION.txt` (blob `773b9c9582a80caa676ec212e5428115079081d5`) + contratos de t164/t167.
- Este documento toca **solo** `docs/backend/`.

**Firma:** Backend · t168 · attempt `82e260db-81b3-47ac-9a93-0f89ea8fe372`
