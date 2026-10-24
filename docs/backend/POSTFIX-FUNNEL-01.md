# POSTFIX-FUNNEL-01 — Los que cruzan el fondeo: ¿mercado o defecto?

**Tarea:** t167 · **Agente:** Backend · **Modo:** SOLO LECTURA (cero cambios, umbrales, escrituras o reinicios) · **Paper**
**Puerta (3 lecturas, `14:26:16Z` / `14:27:05Z` / `14:28:55Z`):** `deploy.sha=77b42b3dccc001455fda3e8d4437d973c9e98c4b`, `id=37783694993`, `at=2026-10-08T13:47:10Z` — **sin cambio desde t164.**
**Corte usado en TODA cifra:** `simulated_at > '2026-10-08T14:16:07Z'` — **no** el `deploy.at` (ver Defecto #11). Cada cifra de este documento declara su corte en la misma línea.

---

## 1. VEREDICTO

**DEFECTO.**

Y no sólo para los 337. La respuesta se mide sobre **toda la historia de la tabla**:

```
SELECT passed, count(*) FROM simulations GROUP BY passed
  ->  f | 705564
```

**`passed = true` no ha ocurrido NUNCA. Ni una vez, en 705.564 simulaciones y 17 horas** (`min=2026-10-07 21:19:58Z`, `max=2026-10-08 14:28:32Z`).

**⇒ La población de VEREDICTOS DE MERCADO del sistema es CERO.** No «pocos»: **cero**, con conteo numerado, y no porque el instrumento no mire — porque la razón de fallo **de mercado** no existe en ninguna fila:

| Control (misma tabla, `count(*)` numerado) | Filas |
|---|---|
| `fail_reason ILIKE '%profit%'` | **0** |
| `fail_reason ILIKE '%slippage%'` | **0** |
| `fail_reason ILIKE '%allowance%'` | **0** |
| `fail_reason ILIKE '%insufficient%'` | **0** |
| **control POSITIVO del mismo `LIKE`** — `LIKE '%TRANSFER_FROM_FAILED%'` | **204** |
| **control POSITIVO** — `LIKE '%reverted%'` | **245** |

El control positivo prueba que el `LIKE` ve lo que hay: **hay 204 y 245 filas para patrones que existen, y 0 para razones de mercado.**

**Un veredicto de mercado exige que el mercado haya sido consultado.** No lo ha sido nunca.

---

## 2. ★★ DEFECTO #11 — EL CORTE DE t164 ESTABA MAL, Y LA DIFERENCIA ES DE 29 MINUTOS

t164 cortó en `deploy.at = 13:47:10Z`, que es **cuando arrancó el STEP del deploy**, no cuando el binario nuevo empezó a servir. La frontera real es el **reinicio del contenedor**:

```
docker inspect --format '{{.State.StartedAt}}' arbitragex-v2-sim-ctl-1
  -> 2026-10-08T14:16:07.1195963Z
  -> revision label = 77b42b3dccc001455fda3e8d4437d973c9e98c4b
```

**`13:47:10Z` → `14:16:07Z` = 28 min 57 s en los que el binario VIEJO seguía sirviendo.** El job `Deploy to VPS` no cerró hasta `14:16:35Z` (`gh run view`, exit=0), coherente con el reinicio a `14:16:07Z`.

### 2.1 La prueba de que la ventana era vieja

Corte `simulated_at > '2026-10-08T13:47:10Z' AND <= '2026-10-08T14:16:07Z'` — **4597 filas**:

| `fail_reason` | count |
|---|---|
| `candidate_incomplete:amount_in_wei_zero` | 4032 |
| **`sim_signer_funding_slot_unresolved`** | **561** |
| `funding_balanceof_timeout` | 4 |
| **Σ** | **4597** **EXACTO** |

**Es exactamente la firma del binario VIEJO**: `amount_in_wei_zero + slot_unresolved + balanceof_timeout`, y nada más. Ninguna de las tres razones nuevas aparece. ⇒ **la ventana contaminada es tráfico pre-arreglo, con nombre y apellido.**

### 2.2 La consecuencia — y hay que decirla sin suavizar

> **t164 reportó «565 fallos de fondeo en 36 min». Con el corte verdadero, `slot_unresolved` post-restart es CERO.**

Lo de «techo agrietado» **era un artefacto del corte**, no un hecho del sistema. Los 561 no son post-fix: son pre-fix.

**En justicia para t164: aplicó el corte que el contrato le mandó.** El defecto está en el corte, no en su ejecución. t164 además cazó, con ese mismo criterio, la ventana de 1 h que cruza el deploy — un nivel más arriba. Esta tarea aplica el mismo criterio un nivel más abajo, y encuentra el error de 29 minutos.

---

## 3. ★★ DEFECTO #12 — El `verify` del contrato devuelve `17` donde la verdad es `134`

El comando del contrato dice:

```
fail_reason IN ('TransferHelper: TRANSFER_FROM_FAILED','execution reverted','sim_timeout')
```

**Los valores reales están PREFIJADOS y no coinciden con esa lista.** Medido:

| Patrón, misma corrida, mismo corte | Filas |
|---|---|
| `IN ('TransferHelper: TRANSFER_FROM_FAILED','execution reverted','sim_timeout')` — **el del contrato** | **17** |
| `LIKE 'TransferHelper%'` — mi primer intento, **MALO** | **0** |
| `LIKE '%TRANSFER_FROM_FAILED%'` | **209** |
| `LIKE 'reverted:%'` — prefijo real | **209** |
| `LIKE 'rpc_error:%'` — prefijo real | **42** |
| `LIKE '%execution reverted%'` | **42** |
| `= 'sim_timeout'` — exacto, y sí coincide | **17** |

Los valores almacenados de verdad son:
```
reverted: TransferHelper: TRANSFER_FROM_FAILED, data: Some(String("0x08c379a0…"))
rpc_error: (code: 3, message: execution reverted, data: Some(String("0x")))
```

**⇒ El `verify` del contrato reporta 17 y descarta 192 filas en silencio.** Es **exactamente el modo de fallo del defecto #9** (la URL que devuelve un vector vacío con la forma de un cero), pero contra la tabla: **un falso cero por un patrón que no matchea.**

**Y `LIKE 'TransferHelper%'` → 0 es mi propio primer intento fallido**: el control positivo que el contrato exige —*«control positivo del `LIKE` sobre una familia que sí tiene filas»*— **es el que lo caza.** Sin él, el cero se habría reportado como ausencia.

---

## 4. EL DISCRIMINANTE: MERCADO **vs** DEFECTO

**Declaración honesta de cuándo se formó.** El contrato pide declararlo antes de contar. **No hay archivo de pre-registración previo a esta tarea, y no se va a fabricar uno.** Lo que sí ocurrió, en orden verificable:

1. **Antes de contar**, formé una hipótesis concreta y falsable: *los que cruzan fallan porque `token_in == token_out` y/o `amount_in_wei = 0`* — o sea, un defecto de construcción de la ruta.
2. **La corrí y la hipótesis MURIÓ**: `token_in == token_out` es **universal** (290/290 post-restart, 4597/4597 en la ventana vieja, 7893/7893 en 1 h) ⇒ **no discrimina nada**, es propiedad de toda la tubería. Y `amount_in_wei = 0` sólo en 60 de 290, no en los que ejecutan.
3. **Descartada esa, declaro el discriminante que sí separa**, y lo aplico:

> **VEREDICTO DE MERCADO** = el fallo ocurre **DESPUÉS** de que el mercado haya sido consultado: el swap se ejecutó contra el estado real del pool y el resultado fue malo (no rentable, slippage, el swap no cerró). Firma esperada: el revert viene **del pool/router**, o hay una razón basada en **profit/slippage**.
>
> **DEFECTO** = el fallo ocurre **ANTES**: la transacción no puede ni empezar. Firma esperada: el revert viene de la **capa de transferencia/aprobación**, y el estado del pool es irrelevante.

**Discriminante operativo, y es el que decide:** `TransferHelper.safeTransferFrom` es, **por construcción**, el jalado del token **previo al swap**. Un fallo ahí **no puede ser un veredicto de mercado**: el mercado todavía no opinó.

---

## 5. LOS DOS CONTENIDOS

Corte `simulated_at > '2026-10-08T14:16:07Z'`, lectura `14:28:55Z`, exit=0:

| `fail_reason` | count | Clasificación |
|---|---|---|
| `reverted: TransferHelper: TRANSFER_FROM_FAILED…` | **209** | **DEFECTO** (upstream del swap) |
| `rpc_error: (code: 3, message: execution reverted, data: 0x)` | **42** | **INDETERMINADO** (revert sin razón) |
| `sim_timeout` | **17** | **INFRAESTRUCTURA** (no completa; no es veredicto de mercado) |
| `candidate_incomplete:amount_in_wei_zero` | **69** | **RECHAZO PREVIO** (pre-existente, t160) |
| **Σ** | **337** | **EXACTO** |

```
209 + 42 + 17 + 69 = 337   -> cierra EXACTO
```
*(y `revert_risk_pct` = **100,00** en las 209 + 42 + 17 = 268 que sí corrieron; `NULL` en las 69 rechazadas antes de simular)*

### LOS DOS CONTENIDOS, que es lo que el contrato pide:

- **VEREDICTO DE MERCADO: `0`** — con `passed=true` = **0** sobre 705.564, y tres `ILIKE` de mercado en **0** con su control positivo en 204/245 (§1).
- **NO-MERCADO: `337`** — 209 defecto + 42 indeterminado + 17 infraestructura + 69 rechazo previo.

**El `rpc_error` con `data: 0x` (revert SIN cadena de razón) queda INDETERMINADO y así se declara.** No se le asigna bando por conveniencia: para decidirlo haría falta el trace o el contrato destino, y `raw_trace` es la columna que da **0 sobre el 100 % de las filas vivas** (defecto #8, **cuarta** confirmación: hoy `0` sobre 8411 positivas).

---

## 6. LAS TRES RAZONES NUEVAS, UNA POR UNA

### 6.1 Serie de crecimiento — CUATRO lecturas al mismo corte

| Lectura | ts | TransferHelper | rpc_error | sim_timeout | zero_amount | Σ |
|---|---|---|---|---|---|---|
| A | `14:27:30Z` | 187 | 38 | 17 | 62 | **304** |
| B | `14:27:55Z` | 193 | 39 | 17 | 64 | **313** |
| C | `14:28:21Z` | 199 | 39 | 17 | 65 | **320** |
| D | `14:28:55Z` | 209 | 42 | 17 | 69 | **337** |

**Las cuatro sumas cierran EXACTAS.** Ventana **A→D = 85 s**:

| Serie | A→D | Tasa |
|---|---|---|
| `TransferHelper` | 187 → 209 | **931,8/h** |
| `rpc_error` | 38 → 42 | **169,4/h** |
| `sim_timeout` | 17 → 17 | **0/h — PLANO en 4 lecturas** |
| `amount_in_wei_zero` | 62 → 69 | 296,5/h |
| **total** | 304 → 337 | 1397,6/h |
| **población que LLEGA a ejecución** (th+rpc) | 225 → 251 | **1101,2/h** |

**⇒ `TransferHelper` CRECE, y rápido.** `sim_timeout` está **plano en cuatro lecturas** (85 s) — que es lo que la regla de las tres lecturas exige para poder decirlo, y **la ventana se declara: 85 s es corta.**

**Ventana propia declarada**, como manda el contrato: sólo se compara contra sí misma. **No se compara con `1287,7/h`, `9361,6/h` (t163, 425 s) ni `736/h` (t160, método distinto): son NO COMPARABLES.**

### 6.2 `TransferHelper: TRANSFER_FROM_FAILED` — 209

**¿Mismo par o dispersas?** Las **filas son de oportunidades todas distintas** (225 filas → 225 `opportunity_id` distintos, medido), pero **los pares se repiten mucho**: los mayores son `UniswapV2|UniswapV3` 19, `SushiSwap|UniswapV3` 20, `UniswapV3|PancakeSwap V3`… sobre `c02aaa…/dac17f…` y familia. ⇒ **contrapartes DISPERSAS, pares REPETIDOS.**

**¿Allowance, balance, o token?** Medido sobre 187 filas:

| `token_in` de las filas que fallan | count |
|---|---|
| `0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2` (WETH) | 175 |
| `0x1abaea1f7c830bd89acc67ec4af516284b1bc33c` | 11 |
| `0x6b175474e89094c44da98b954eedeac495271d0f` (DAI) | 1 |
| **tokens distintos** | **3** |

- **NO es un token roto**: 3 tokens distintos, y el dominante es **WETH, un ERC-20 estándar**. ⇒ **la causa «token no estándar que devuelve false» (estilo USDT) queda REFUTADA como causa única.**
- **NO es balance token-específico**: el fallo es **uniforme al 100 %** sobre todos los tokens; un problema de balance sería selectivo (el firmante tendría unos y no otros).
- **Consistente con ALLOWANCE**: una sola puerta de aprobación, a nivel de firmante, falla para todo.

**⇒ CAUSA: NO COMPUTADA.** La hipótesis líder es **allowance**, y **no se declara como hecho**. El test que la cerraría, nombrado: leer `allowance[signer][router]` en el fork en el instante de la simulación. Este instrumento no lo tiene.

### 6.3 `rpc_error: execution reverted, data: 0x` — 42

Revert **sin cadena de razón** (`data: Some(String("0x"))`). **INDETERMINADO**: no se puede atribuir a mercado ni a defecto sin el trace. Crece a **169,4/h**.

### 6.4 `sim_timeout` — 17

**PLANO en 4 lecturas** (85 s). No es veredicto de mercado — es el simulador **no terminando**. Se declara como **infraestructura/rendimiento**, no como fallo de la construcción del trade. **La ventana de 85 s es corta y así se declara.**

---

## 7. ★★ LOS 561: ¿MISMO MECANISMO O UNO NUEVO?

**Ni uno ni otro: son PRE-ARREGLO.** El contrato pedía comparar con el ratio pre-fix. La comparación es más simple y más dura:

| Corte | `sim_signer_funding_slot_unresolved` | Artefacto |
|---|---|---|
| `> 14:16:07Z` (**restart real**) | **0** | `psql count(*)`, corte en la línea |
| `13:47:10Z → 14:16:07Z` (binario viejo) | **561** | `psql count(*)`, corte en la línea |
| 1 h sin corte (residuo) | **851** | `psql count(*)`, corte en la línea |

```
851 = 561 (ventana vieja) + 290 (antes de 13:47:10Z)   -> EXACTO
```

**⇒ El 100 % de los `slot_unresolved` de la última hora es binario VIEJO. Post-restart: cero.** El ratio `4,0000 → 0,0441` no hay que compararlo: **la serie dejó de emitirse.** El mecanismo pre-fix no «cambió» — **dejó de ocurrir**.

### 7.1 Presencia y ausencia, medidas POR SEPARADO (donde vive cada cifra)

| Fuente | `slot_unresolved` | Comando |
|---|---|---|
| **Contador** (Prometheus, loopback ssh, exit=0) | **vector VACÍO — la etiqueta no existe** | `arbx_sim_funding_total{outcome="slot_unresolved"}` |
| **PG, corte verdadero** | **0** (conteo numerado) | `count(*) WHERE simulated_at > '…14:16:07Z' AND fail_reason='sim_signer_funding_slot_unresolved'` |
| **PG, 1 h sin corte** | **851** (residuo viejo) | idem, `now() - interval '1 hour'` |

**Los dos coinciden en que la causa paró** — y eso se sostiene porque **PG lo dice con un conteo numerado, no por la ausencia de una etiqueta**. Es la medición que el contrato exige: **una etiqueta que desaparece no es una causa que desaparece**; aquí, además, la causa desapareció, y **se midió en dos fuentes independientes.**

**Taxonomía nueva del contador, `14:29:14Z` ts=`1791469754.428`:** `verify_mismatch=6` · `cache_hit=264` · `seeded_fresh=5`. **3 outcomes, distintos de los 4 pre-fix.** `slot_unresolved`, `balance_unreadable` y `rpc_err` **no aparecen**; `seeded_fresh` (era VACÍO en 6 lecturas pre-arreglo) crece `3 → 4 → 5`; `cache_hit` (etiqueta nueva) crece `258 → 264`.

---

## 8. ★★ LA TASA: **NO COMPUTADA** — y la identidad de t164 se cae

El contrato autoriza dos salidas: *«O se encuentra una identidad NUEVA que la sostenga (t164 ya propuso `cache_hit + seeded_fresh = anvil`, EXACTA en dos lecturas), O se declara NO COMPUTADA.»*

**La identidad propuesta NO sostiene. La medí:**

| Lectura | `cache_hit` | `seeded_fresh` | suma | `anvil` | Δ |
|---|---|---|---|---|---|
| t164 L2 | 123 | 3 | 126 | 126 | **0** |
| t164 L3 | 127 | 3 | 130 | 130 | **0** |
| t167 @`14:28:55Z` | 258 | 4 | 262 | 264 | **+2** |
| t167 @`14:29:14Z` | 264 | 5 | 269 | 271 | **+2** |

**Δ = +2 en dos lecturas consecutivas ⇒ NO es el skew de 8 ms entre scrapes** (que en t164 daba Δ=1 y no se repetía). **Es sistemático.**

**⇒ `cache_hit + seeded_fresh = anvil` es FALSA como identidad.** Las dos exactitudes de t164 fueron una coincidencia de dos muestras — exactamente el modo de fallo que la propia campaña viene cazando.

**⇒ SIN POBLACIÓN DELIMITADA NO HAY TASA. LA TASA DE FALLO DE FONDEO DE LA POBLACIÓN DEL FORK QUEDA `NO COMPUTADA`.** Se da el conteo crudo:

- `slot_unresolved` post-restart: **0**
- `funding_balanceof_timeout` post-restart: **0**
- `seeded_fresh` (acumulado desde el restart): **5**
- `cache_hit` (idem): **264**
- `verify_mismatch` (idem): **6**

**Y no se deriva ninguna tasa de eso.** Derivar una tasa de una población que no se puede delimitar está prohibido, y la prohibición se respeta **incluso cuando el conteo crudo favorece la historia que querríamos contar.**

---

## 9. CONTROLES

| Control | Resultado |
|---|---|
| Canal SQL `SELECT 1` | `1`, exit=0 |
| **Negativo `ON_ERROR_STOP`** | `SELECT esto_no_existe` -> **exit=1** + `ERROR: column "esto_no_existe" does not exist` |
| **Positivo del `LIKE`** | `%TRANSFER_FROM_FAILED%` -> **204**; `%reverted%` -> **245** · contra `%profit%` -> **0** |
| **Conteo numerado** | todo con `count(*)`, nunca un `GROUP BY` cuya vacuidad pudiera ser `NULL` |
| Suma del histograma completo | **705564 EXACTO** (13 buckets) |
| Suma de cada partición por corte | **EXACTA** en las 4 lecturas y en los 3 cortes |
| Externo `:9090` (defecto #9) | **`http_code=000`, exit=7** — reproducido por cuarta vez |
| `raw_trace` (defecto #8) | **0** sobre 8411 filas positivas — **cuarta** confirmación |

**Ninguna evidencia usa `195.201.235.70:9090` ni `195.201.235.70/metrics`.** Toda lectura de Prometheus es **loopback vía ssh**, exit=0.

---

## 10. LECTURA DEL SISTEMA COMPLETO (contexto, con corte declarado)

Histograma de **toda la tabla**, `count(*)` numerado, suma **705564 EXACTA**:

| `fail_reason` | count |
|---|---|
| `strategy_cyclic_route_not_simulatable_in_s4:dex_arb` | 576907 |
| `candidate_incomplete:amount_in_wei_zero` | 57829 |
| `sim_signer_funding_slot_unresolved` | 29660 |
| `strategy_cyclic_route_not_simulatable_in_s4:flashloan_arb` | 23743 |
| `strategy_cyclic_route_not_simulatable_in_s4:triangular` | 16920 |
| `reverted: TransferHelper: TRANSFER_FROM_FAILED…` | **204** |
| `build_error: amount invalid: zero amount_in` | 144 |
| `build_error: router not in catalog for chain=1 dex=unknown` | 84 |
| `rpc_error: (code: 3, … execution reverted, data: 0x)` | 41 |
| `sim_timeout` | 17 |
| `funding_balanceof_timeout` | 9 |
| `fork_acquire_failed` | 4 |
| `anvil_setStorageAt: error sending request for url (http://anvil:8545/)` | 2 |

**El 87,5 % de todo lo que el sistema ha simulado es `route_not_simulatable_in_s4`** (576907 + 23743 + 16920 = 617570 de 705564). Es un rechazo **previo** a la simulación, no un veredicto de mercado.

**⇒ La cadena es: 87,5 % no simulable → 8,2 % monto cero → 4,2 % fondeo → 0,03 % transferencia → 0 % mercado.**

---

## 11. LO QUE NO SE COMPUTA, CON SU RAZÓN

| No computado | Razón exacta |
|---|---|
| **Tasa de fallo de fondeo de la población del fork** | La identidad nueva (`cache_hit + seeded_fresh = anvil`) **falla con Δ=+2 sistemático** en dos lecturas (§8). Sin rótulo y sin identidad no hay población; sin población no hay tasa. |
| **Causa de `TRANSFER_FROM_FAILED`: allowance / balance / token** | **NO COMPUTADO.** Refutado «token no estándar» (3 tokens, WETH dominante estándar) y «balance» (uniforme al 100 %). Hipótesis líder: **allowance**. Test que la cierra, nombrado: `allowance[signer][router]` en el fork al instante de la sim. |
| Bando de `rpc_error` (42) | **INDETERMINADO**: revert con `data: 0x`, sin cadena de razón. Requiere trace; `raw_trace` es la columna muerta (defecto #8). |
| Capacidad del camino caro | **NO COMPUTADA aquí.** Sólo se midió crecimiento de poblaciones (§6.1), con ventana propia de **85 s**. No comparable con t163 (425 s) ni t160 (método distinto). |
| `no_simulation_row` | **NO MEDIDA.** |

**Regla aplicada:** un «no medido» **no se escribe como 0**; una etiqueta renombrada **no** se lee como un cero; y un cero **necesita su control positivo** — que aquí lo tiene (§9).

---

## 12. TRAZABILIDAD

- Leído: `GET /api/status`, `docker inspect` (`StartedAt` + etiqueta de revisión), Prometheus **loopback vía ssh**, PostgreSQL por `docker exec … psql`, `gh run view … --json jobs`, `git ls-remote`.
- **Cero** cambios al motor, umbrales, escrituras o reinicios. **Cero** deploys, firmas o broadcast. **Paper.** **NO** se re-disparó el benchmark.
- **Predicción heredada, no re-inventada**: `FUND-FUNNEL-01.PREDICCION.txt` (blob `773b9c9582a80caa676ec212e5428115079081d5`) + contrato de t164. **No se escribió pre-registración nueva**; la hipótesis que sí formé antes de contar (`token_in==token_out` / `amount=0`) **se declara y se reporta su refutación** (§4).
- Este documento toca **solo** `docs/backend/`.

**Firma:** Backend · t167 · attempt `1f99b16d-cc11-43d2-8a89-155f324cb823`
