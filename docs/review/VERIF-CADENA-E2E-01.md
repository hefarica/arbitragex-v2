# VERIF-CADENA-E2E-01 — ¿se mueve la cadena entera después del deploy del fix del parser?

- **Tarea:** `t77` (kind: verification, round 1) — miembro `Release-B`, attempt 1
- **Ventana de observación:** `2026-10-06T13:36:15Z` (T0) → `2026-10-06T13:53:33Z` (T1) → `2026-10-06T13:54:31Z` (T2)
- **Duración efectiva:** 18 min 16 s
- **Método:** 100% lectura. Cero workflows emitidos (ver §1).
- **Artefacto:** este archivo. **Sin commit, sin push, sin PR** (untracked, como exige el contrato).

> ⚠️ **AVISO DE VIGENCIA (añadido 2026-10-07T22:13Z).** El veredicto de abajo —**"la cadena NO se mueve"**— es correcto **solo para la ventana del 2026-10-06** que se declara arriba, y **ya NO describe el estado actual**. Al 2026-10-07T22:12Z el runtime **sí** alcanzó a `main` (`21d2039c`), `simulations` pasó de 0 a **205 028+** filas y `selector-api` dejó de rechazar (`invalid_message` = 0). Lo que sigue abierto es otra cosa: `passed=true` = **0** y el 100 % de las filas falla por `strategy_cyclic_route_not_simulatable_in_s4`. **Para el estado posterior, ver §11.** Las tablas de §3–§7 tampoco son heredables sin su ventana y su superficie. Ver §11.A.

---

## VEREDICTO EN UNA LÍNEA

**La cadena NO se mueve, y ahora sé exactamente por qué: el fix del parser (`#838`) YA ESTÁ EN `main` pero NO llegó al runtime, porque el deploy del SHA intermedio `b1b2e600` FALLÓ en el gate `Verify same-SHA main-push gates` y el job `Deploy to VPS` quedó `skipped`.** El runtime sigue en `c89d21a3` (**17 merges** atrás) y `simulations` sigue en 0 con delta 0 en 18 minutos.

**La distinción que importa:** el fix **no llegó**. No es que "no funcione". Son dos afirmaciones distintas y los datos separan una de la otra (§3).

---

## 1. CRITERIO 1 — NO se emitió ningún workflow

**Declaración textual: NO ejecuté `gh workflow run`, ni push, ni PR, ni merge. Ninguna acción mía puede haber creado o modificado un run.**

**Prueba por inventario de comandos (el canal real de prueba):** todos mis comandos de esta tarea son de lectura — `gh api …/actions/runs?status=…` (GET), `gh run view`, `gh run list`, `git ls-remote`, `git fetch`, `git rev-list`, `curl.exe -s`, y por ssh: `docker ps`, `docker logs`, `docker inspect`, `docker exec … psql -tAc "SELECT …"`, `redis-cli XINFO/XLEN`, `date`, `grep`. No hay ningún comando de emisión en el registro.

**Prueba por el estado de la cola (antes y después):**

| | queued | in_progress | vivos |
|---|---|---|---|
| **T0** 13:36:15Z | 44 | 9 | **53** |
| **Después** 13:55Z | 28 | 13 | **41** |

Comando: `gh api "repos/hefarica/arbitragex-v2/actions/runs?status=queued&per_page=1" --jq .total_count` (idem `in_progress`).

**Lectura honesta:** el conteo BAJÓ (53 → 41). El movimiento del conteo no es mérito mío: durante la ventana otros actores mergearon `#838` (13:52:41Z), lo que creó ~16 runs nuevos. Lo que sí prueba el número es lo que importa: **no hay ningún incremento atribuible a esta tarea**, y mi inventario de comandos no contiene ninguna vía de emisión. **No puedo probar un negativo desde el conteo solo**; lo pruebo desde el inventario, y el conteo lo corrobora.

---

## 2. CRITERIO 2 — el deploy CERRÓ (con `failure`, no por cola)

Comando: `gh run list --workflow="Auto-Deploy VPS (Post-E2E)" --limit 3 --json databaseId,headSha,status,conclusion`

| run id | headSha | status | conclusion |
|---|---|---|---|
| `37466055080` | `b1b2e600b24e046e07e68b036e3996817853f93b` | completed | **failure** |
| `37464520553` | `2d22ce7083e940e93f7818c51247cf68e69e9b47` | completed | cancelled |
| `37464463756` | `65903cca856846984e319214d141798d3a6db3c9` | completed | cancelled |

**No está `pending` ni `queued`: cerró. Y cerró en `failure`.** Por eso NO aplica la salida "BLOQUEADO POR COLA"; aplica una peor y más precisa: **el deploy se rechazó solo**.

**Causa localizada (`gh run view 37466055080 --json jobs`):**

| job | status | conclusion | started | completed |
|---|---|---|---|---|
| `Wait for all deployment gates` | completed | **failure** | 13:38:13Z | 13:52:54Z |
| `Deploy to VPS` | completed | **skipped** | 13:52:55Z | 13:52:55Z |

Step que falla dentro del primer job: **`Verify same-SHA main-push gates`** (los demás steps: `success`).

**Consecuencia mecánica:** el step de deploy quedó `skipped`. **Nunca hubo SSH al VPS, nunca hubo rebuild, nunca hubo `up -d`.** El runtime no podía cambiar.

**Progresión del estado del run durante la ventana** (de mi propio polling cada 60 s): `pending` en T0 → `in_progress` a las 13:38:34Z → `completed/failure` a las 13:52:54Z.

---

## 3. CRITERIO 3 — identidad del runtime por el REMOTO: la brecha NO se cerró

Comando: `curl -s https://edge-arbx.ape-tv.net/status`

| | T0 (13:36:02Z) | T1 (13:53:28Z) |
|---|---|---|
| `deploy.sha` | `c89d21a3437c7fc2456fb7a08dba7d26ef41ee5e` | `c89d21a3437c7fc2456fb7a08dba7d26ef41ee5e` |
| `deploy.id` | `37405962576` | `37405962576` |
| `deploy.at` | `2026-10-06T03:20:27Z` | `2026-10-06T03:20:27Z` |

**El runtime NO cambió. Es el mismo objeto en T0 y en T1.** Los 7 servicios reportan `ok:true,status:200` (selector-api, sim-ctl, recon, relays-client, searcher-rs, math-engine, token-enricher); `env:"production-like"`; `killswitch.enabled:false`.

**La brecha, con números:**

| momento | main | runtime | brecha |
|---|---|---|---|
| Al abrir la tarea (según el brief) | `b1b2e600` | `c89d21a3` | 16 merges |
| **T1 (medido por mí)** | **`bceb31ef`** | `c89d21a3` | **17 merges / 41 commits** |

Comandos: `git ls-remote origin refs/heads/main` => `bceb31ef7c0fd6d4b50bcdee6fc8d33858334822`; `git rev-list --count c89d21a3..origin/main` => `41`; `git rev-list --merges --count c89d21a3..origin/main` => `17`.

**`main` se movió DURANTE mi ventana.** El log de `origin/main`:

```
bceb31ef | 2026-10-06T08:52:41-05:00 | Merge pull request #838 from hefarica/fix/selector-parse-01
b1b2e600 | 2026-10-06T07:49:12-05:00 | Merge pull request #833 from hefarica/docs/g8-tablero-01
079deb3f | 2026-10-06T07:46:24-05:00 | fix(selector-api): SELECTOR-PARSE-01 - schema estricto tiraba el 100% del stream por la clave economics
```

**Esto invierte la lectura del bloqueo:** el fix `#838` (commit `079deb3f`) **ya está en `main`** (`bceb31ef`, mergeado 13:52:41Z). Lo que falta no es el fix: es **el deploy**. Y el deploy que había en vuelo era el del SHA *anterior* (`b1b2e600`), que además falló.

**Siguiente oportunidad, ya encolada:** run **`37474306217`** sobre **`bceb31ef`**, `status: queued`, creado `2026-10-06T13:52:45Z` (4 s después del merge de `#838`). Ese es el run que decide, y **no lo seguí hasta el cierre porque excede la ventana de esta tarea**.

---

## 4. CRITERIO 4 — LA PRUEBA QUE DECIDE: `simulations` sigue en 0

**Control de canal OBLIGATORIO, ejecutado antes de reportar cualquier cero:**
`docker exec arbitragex-v2-postgres-1 psql -U postgres -d arbitragex -tAc "SELECT 1"` => **`1`** (en T0, T1 y T2). **El canal devuelve fila. Los ceros de abajo NO son artefacto de canal.**

**Contadores de por vida (`pg_stat_user_tables`) — comando y salida cruda:**

```
docker exec arbitragex-v2-postgres-1 psql -U postgres -d arbitragex -tAc \
  "SELECT relname, n_live_tup, n_tup_ins FROM pg_stat_user_tables
   WHERE relname IN ('simulations','executions','paper_trade_runs') ORDER BY relname"
```
```
executions|0|0
paper_trade_runs|0|0
simulations|0|0
```

**Y `COUNT(*)` real, no estimado:**

```
docker exec arbitragex-v2-postgres-1 psql -U postgres -d arbitragex -tAc \
  "SELECT 'simulations' t, COUNT(*) FROM simulations
   UNION ALL SELECT 'executions', COUNT(*) FROM executions
   UNION ALL SELECT 'paper_trade_runs', COUNT(*) FROM paper_trade_runs"
```
```
simulations|0
executions|0
paper_trade_runs|0
```

**Dos controles adicionales que cierran las dos formas de falso cero:**

1. **No es tabla particionada.** `SELECT relname, relkind FROM pg_class WHERE relname IN ('simulations','executions','paper_trade_runs')` => `executions|r`, `paper_trade_runs|r`, `simulations|r`. `r` = ordinaria. Si fuera `p` (particionada), un `n_tup_ins=0` en el padre podría ocultar inserts en las hijas; **no es el caso**.
2. **Los contadores son de por vida.** `SELECT datname, coalesce(stats_reset::text,'NUNCA') FROM pg_stat_database WHERE datname='arbitragex'` => salida cruda **`arbitragex|NUNCA`**. `stats_reset = NUNCA` ⇒ **nunca se llamó `pg_stat_reset()` sobre esta base** ⇒ `n_tup_ins` acumula desde el inicio. **`n_tup_ins = 0` significa CERO inserts, jamás.**

**Estado: `simulations`, `executions` y `paper_trade_runs` NO dejaron de estar en 0. Siguen en 0.** Ninguna de las tres tablas ha recibido nunca un solo INSERT.

---

## 5. CRITERIO 5 — el DELTA: cero filas entraron durante la ventana

| tabla | T0 13:36:15Z | T2 13:54:31Z | **Δ en 18 min 16 s** |
|---|---|---|---|
| `simulations` (`n_tup_ins`, `COUNT(*)`) | `0`, `0` | `0`, `0` | **0** |
| `executions` (`n_tup_ins`, `COUNT(*)`) | `0`, `0` | `0`, `0` | **0** |
| `paper_trade_runs` (`n_tup_ins`, `COUNT(*)`) | `0`, `0` | `0`, `0` | **0** |

**Se declara como manda el contrato: un `simulations` que pasa de 0 a 0 en la ventana significa que el fix NO LLEGÓ.** Y en §3 está la causa medida de que no llegara: su deploy quedó `skipped` tras fallar el gate.

**FALSACIÓN — qué habría cambiado este veredicto:** un `deploy.sha` distinto de `c89d21a3` en `/status`; o cualquier `Δ > 0` en `n_tup_ins`; o un `entries-added` del stream `validated` mayor que `12 247 832`. Ninguna de las tres ocurrió.

---

## 6. CRITERIO 6 — `selector-api` y el stream: el corte está exactamente ahí

### 6.1 `consumer.invalid_message` — SIGUE TIRANDO EL 100%, y es la MISMA clave

Comando: `docker logs arbitragex-v2-selector-api-1 --since <ventana> 2>&1 | grep -c "consumer.invalid_message"`

| ventana | T0 (13:36:15Z) | T1 (13:53:33Z) |
|---|---|---|
| `--since 5m` | **2 987** | **2 677** |
| `--since 20m` | **11 295** | **10 867** |
| `--since 60m` | **22 378** | — |

≈ **535–600 rechazos por minuto**, sostenidos. **No cayó: sigue igual.**

**R9 — ventana de logs antes de concluir nada:** `docker inspect arbitragex-v2-selector-api-1 --format "{{.HostConfig.LogConfig.Config}}"` => `map[max-file:5 max-size:10m]` (**50 MB** retenidos). `State.StartedAt` = `2026-10-06T03:49:58Z`; primera línea retenida = `2026-10-06T09:32:50.648Z`. **La ventana de 20 min (13:16→13:36) está íntegramente dentro de lo retenido**, así que los conteos NO son artefacto de rotación. (Por lo mismo, ventanas anteriores a 09:32:50Z son NO COMPUTABLES con este `docker logs`.)

**El motivo del rechazo es UNO SOLO y es el que el fix ataca:**
`docker logs … --since 5m 2>&1 | grep -o "Unrecognized key(s) in object: .[a-z]*." | sort | uniq -c`
```
   2987 Unrecognized key(s) in object: 'economics'
```
**2 987 de 2 987 = 100% `'economics'`.** El payload de ejemplo, crudo:
`{"level":"warn","time":"2026-10-06T09:32:50.648Z","service":"selector-api","event":"consumer.invalid_message","id":"1791279170647-0","err":"[…] \"Unrecognized key(s) in object: 'economics'\"}`

**`selector-api` no publica nada.** Filtrando el ruido de sus propios endpoints: `docker logs … --since 5m 2>&1 | grep -viE "consumer.invalid_message|/metrics|/health" | tail -5` => **vacío**.

### 6.2 `XINFO GROUPS arbx:opps:validated` — `last-delivered-id` NO avanza

Comando: `docker exec arbitragex-v2-redis-1 redis-cli XINFO GROUPS arbx:opps:validated`

| campo | T0 | T1 | ¿avanza? |
|---|---|---|---|
| `last-delivered-id` | `1789637172953-0` | `1789637172953-0` | **NO** |
| `entries-read` | `12247832` | `12247832` | **NO** |
| `pending` | `0` | `0` | — |
| `lag` | `0` | `0` | — |
| consumers | `175` | `175` | — |

**El contrato advertía que un `XLEN` estático en `10001` no prueba nada. Confirmado y superado:** el `XLEN` es estático (10001 en T0, T1 y T2) **y** `last-delivered-id` como `entries-read` están clavados. Además, la prueba más fuerte, `XINFO STREAM`:

| campo | T0 | T2 | Δ |
|---|---|---|---|
| `length` | 10001 | 10001 | 0 |
| `last-generated-id` | `1789637172953-0` | `1789637172953-0` | **0** |
| `entries-added` | `12247832` | `12247832` | **0** |

**El stream `arbx:opps:validated` NO recibió una sola entrada nueva en la ventana. Y no es de hoy: su `last-generated-id` (`1789637172953-0` ≈ ms epoch) y la PRIMERA entrada retenida (`recorded-first-entry-id` = `1789636936078-0`, con `detected_at":"2026-09-17T09:22:13.308310074Z"` y `"rejection_reason":"v3_quote_unavailable"`) sitúan el último movimiento real en **2026-09-17**, 19 días atrás.**

**Contraste que fija el diagnóstico — el stream de ENTRADA sí está vivo.** `arbx:opps:detected`: `length=10002`, `last-generated-id=1791293814232-0` (**hoy**), `entries-added=84398696`; y sus 3 grupos (`enricher`, `paper-archiver-g0`, `selector-g0`) tienen `last-delivered-id = 1791293814232-0`, es decir **al día con la cabeza**.

*(Nota metodológica: en un stream con recorte `MAXLEN` los campos `lag`/`entries-read` de `XINFO GROUPS` dejan de ser fiables — Redis solo los garantiza si no hubo borrados. El discriminador válido aquí es `last-delivered-id`/`entries-added`, no `lag`. `selector-g0` con `lag=872573` y `last-delivered-id` en la cabeza es exactamente esa inconsistencia, no una contradicción del hallazgo.)*

**El corte queda localizado en un solo lugar: `selector-api` consume `detected` al día, rechaza el 100% por la clave `'economics'`, y por eso `validated` no recibe NADA desde el 17-sep.**

### 6.3 `sim-ctl` en el mismo periodo: inactivo

`docker logs arbitragex-v2-sim-ctl-1 --since 5m 2>&1 | wc -l` => **`0`** en T0 y en T1.
**No está roto: no tiene qué consumir.** Coherente con §6.2 — si `validated` no recibe entradas, su grupo `sim-ctl-g0` no tiene de dónde leer.

---

## 7. CRITERIO 7 — el RECHAZO ECONÓMICO: NO COMPUTADO (y por qué)

**La condición del criterio NO se cumple, así que no se puede aplicar:** el criterio pide el desglose "si las filas entran pero casi todas son `passed=false`". **Las filas NO entran.** Con `simulations = 0` filas, el desglose por `passed` / motivo es **NO COMPUTADO** — no "0% passed", no "88% negativo". **No se reporta un cero que no fue medido.**

**Dos precisiones de esquema que evitan un falso cómputo futuro:**
- La columna **no se llama `rejection_reason`** en `simulations`. Es **`fail_reason`**. Columnas reales (`information_schema.columns`): `id, opportunity_id, simulator, gas_estimate_wei, gas_price_wei, slippage_pct, revert_risk_pct, simulated_profit_usd, passed, fail_reason, raw_trace, trace_id, simulated_at`. Una query por `rejection_reason` contra `simulations` **falla**.
- `rejection_reason` sí existe en el payload de `opportunities` (se ve en la entrada retenida del stream `validated`).

> ⚠️ **PROCEDENCIA Y CADUCIDAD DE LA TABLA DE ABAJO (añadido 2026-10-07, no cambia el veredicto de t77).**
> **Superficie:** tabla `opportunities`, columna `rejection_reason` — el veredicto del **PRODUCTOR**, no el del selector.
> **Ventana:** `detected_at > now() - interval '1 hour'` medida a las **2026-10-06T≈13:55Z**. **N = 30 228.**
> **NO ES REPRODUCIBLE HOY.** La misma query sobre 2026-10-07T22:12:11Z (N = 227 730) da otro reparto: `v3_pair_no_pools` **38,5 % → 0,22 %**, y el dominante pasa a `spread_negative_round_trip` **83,5 %**. La población creció **7,5×** y el mix se reordenó porque la de t77 era **pre-fix**. **No heredar estas cifras sin su ventana.** Ver §11.

**Contexto medido (upstream, sí computable) — `opportunities` última 1 h: 30 228 filas**, por `rejection_reason`:

| motivo | filas | % |
|---|---|---|
| `v3_pair_no_pools` | 11 649 | 38,5 % |
| `non_positive_profit` | 6 558 | 21,7 % |
| `spread_negative_round_trip` | 6 338 | 21,0 % |
| `v3_pool_not_catalogued` | 3 629 | 12,0 % |
| `no_tradable_size` | 1 049 | 3,5 % |
| `v3_multileg_budget_exhausted` | 482 | 1,6 % |
| `v3_quote_unavailable` | 266 | 0,9 % |
| `v3_pool_revert` | 213 | 0,7 % |
| `single_pool_no_spread` | 20 | 0,07 % |
| `no_price_oracle` | 6 | 0,02 % |
| `negative_net_profit` | 4 | 0,01 % |

Comando: `psql -tAc "SELECT coalesce(rejection_reason,'NULL'), COUNT(*) FROM opportunities WHERE detected_at > now() - interval '1 hour' GROUP BY 1 ORDER BY 2 DESC LIMIT 12"`.
*(Los motivos de la última fila son 30 214 de 30 228; 14 filas caen fuera del `LIMIT 12` o tienen `rejection_reason` NULL. Se declara el residuo en vez de esconderlo.)*

**Tres lecturas de estos números, y solo la primera pertenece a esta tarea:**
1. **El productor upstream SÍ produce** (30 228 oportunidades/h, `n_tup_ins` de `opportunities` = 13 640 836). La cadena no está muerta en la detección; está cortada en la validación.
2. **No estamos en la etapa de rechazo económico.** El pipeline se corta ANTES: en el parser de `selector-api`. La pregunta "¿el mercado es malo?" **no es medible acá** porque ninguna oportunidad llega a simularse. Confundir ambas cosas es exactamente lo que el contrato prohíbe.
3. **Dato lateral relevante para el camino crítico: `v3_quote_unavailable` cayó a 266/h (0,9 %)** — el cuello único que `t54`/`t61` seguían ya no es el techo. Hoy el primer motivo es `v3_pair_no_pools` (38,5 %), estructural.

---

## 8. CRITERIO 8 — no se firma, no se emite, no se toca capital

- **Palancas, leídas del `.env` del VPS** (`grep -E "^(ARBX_TRADE_MODE|ARBX_LIVE_EXEC_ENABLED)=" /opt/arbitragex-v2/.env`, solo esas 2 claves, ningún valor secreto impreso):
  ```
  ARBX_LIVE_EXEC_ENABLED=False
  ARBX_TRADE_MODE=paper
  ```
  **Intactas: `paper` y `False`.** Sin cambios durante la verificación.
- **Kill-switch** (`/status`): `enabled:false, reason:"operator_deactivate", triggered_by:"g7-drill-reviewer-cycle2", updated_at:"2026-10-06T03:34:49.145Z"` — estado pre-existente, **no lo toqué**.
- **Sin firma, sin broadcast, sin capital.** No ejecuté ningún `cast`, ningún envío a relay, ninguna transacción. El modo es `paper` y `ARBX_LIVE_EXEC_ENABLED=False`, así que el terminus no puede firmar.
- **Sin escritura en el VPS más allá de lectura:** todos mis comandos fueron `docker ps/logs/inspect/exec … psql SELECT`, `redis-cli XINFO/XLEN`, `date`, `grep`. **Cero `docker restart`/`up`/`build`, cero edición de archivos, cero `redis-cli` de escritura.**
- **7/7 servicios `ok:true,status:200`; 25 contenedores `Up … (healthy)`** — no perturbé el runtime.

---

## 9. LÍMITES — lo que esta verificación NO prueba

1. **No seguí el deploy de `bceb31ef` (run `37474306217`) hasta el cierre:** excede la ventana y el contrato la acota. Queda `queued` con su run id. **Nadie debe leer este documento como "el deploy de `#838` falló": falló el de `b1b2e600`, que es otro.** El destino de `37474306217` está **NO COMPUTADO**.
2. **No verifiqué que el fix funcione en producción.** Verifiqué que **no llegó** (§3) y que el defecto que ataca sigue activo al 100 % (§6.1). Que `#838` cure el 100 % de los rechazos está probado en sus tests, **no por mí y no en producción**.
3. **Ventanas de log anteriores a `09:32:50Z` son no computables** con este `docker logs` (rotación `max-file:5 × max-size:10m`). El "2 043/20 min ANTES del fix" del brief **no lo pude reproducir ni refutar**: mi medición (11 295/20 min en T0) corresponde a otra ventana. **Declaro la discrepancia en vez de adoptar la cifra ajena.**
4. **No re-medí el runtime del VPS** (no `docker inspect` de imagen/SHA dentro de los contenedores): la identidad la tomé de `/status` por el remoto, que es lo que el contrato pide.
5. **No emití, no commiteé, no pusheé.** Este documento queda **untracked** a propósito.

---

## 10. LO QUE LE TOCA AL CAPITÁN (hallazgo, no opinión)

1. **El camino crítico ya no es el parser: es el gate de deploy.** `#838` está en `main`; el deploy de `b1b2e600` murió en `Verify same-SHA main-push gates` y el de `bceb31ef` (`37474306217`) está encolado. **Mientras ese gate siga fallando, ningún fix que se mergee llega al VPS.** Vale diagnosticar por qué falló ese step: con `main` moviéndose cada pocos minutos, un gate "same-SHA" es candidato estructural a fallar por supersesión.
2. **Tras aterrizar el deploy hay que RE-MEDIR, no heredar:** el veredicto de esta tarea caduca con el próximo deploy. Los discriminadores a repetir son tres y son baratos: `Δ entries-added` de `validated` (>0), `Δ n_tup_ins` de `simulations` (>0), y el conteo de `consumer.invalid_message --since 5m` (debe caer de ~2 700 a ~0).
3. **Un solo motivo explica el 100 % del corte** (`'economics'` contra el `.strict()`), así que el primer chequeo post-deploy es binario y no admite zona gris.

---

## 11. ADDENDUM 2026-10-07 — PROCEDENCIA, CADUCIDAD Y AUTOCORRECCIONES

> No cambia el veredicto de t77 (correcto para su ventana). Fecha sus cifras, resuelve la procedencia que el capitán pidió, y refuta DOS afirmaciones mías de §10.

### A. Superficies y ventanas, declaradas (era el pedido)

| id | superficie | filtro / alcance | instante | N |
|---|---|---|---|---|
| **A1** (la de §7) | tabla `opportunities`, columna `rejection_reason` — veredicto del **PRODUCTOR** | `detected_at > now() - interval '1 hour'` | 2026-10-06T≈13:55Z | 30 228 |
| **A2** | idem A1 | idem | **2026-10-07T22:12:11Z** | **227 730** (≈3 795/min) |
| **B** | stream `arbx:opps:validated`, **todas** las entradas retenidas | IDs `1791411016711-0` → `1791411161025-0` = **~2 min 24 s** | 2026-10-07T≈22:12:41Z | 10 001 |

### B. La tabla de §7 CADUCÓ — no es reproducible

| motivo | A1 (10-06, N=30 228) | **A2 (10-07, N=227 730)** |
|---|---|---|
| `v3_pair_no_pools` | 11 649 — **38,5 %** | 494 — **0,22 %** |
| `v3_pool_not_catalogued` | 3 629 — 12,0 % | 3 223 — 1,4 % |
| `spread_negative_round_trip` | 6 338 — 21,0 % | **190 135 — 83,5 %** |
| `non_positive_profit` | 6 558 — 21,7 % | 17 120 — 7,5 % |

La población creció **7,5×** y el mix se reordenó. Causa: A1 se midió **pre-fix** (parser cortando el 100 %), así que describía una población distinta. **No heredar A1 sin su ventana.**

### C. Las dos superficies COINCIDEN en la forma (mismo instante)

| motivo | A2 (PG, 1 h) | B (stream, 2,4 min) |
|---|---|---|
| `spread_negative_round_trip` | 83,5 % | 81,5 % |
| `non_positive_profit` | 7,5 % | 10,9 % |
| `single_pool_no_spread` | 3,3 % | 3,0 % |
| `v3_pool_not_catalogued` | 1,4 % | 2,2 % |
| `v3_multileg_budget_exhausted` | 0,34 % | 0,37 % |

⇒ **La discrepancia con una muestra del stream NO es de superficie.** El stream cubre 2,4 min y PG 60 min: misma foto, distinta exposición.

### D. Diferencia ESTRUCTURAL entre superficies (esta sí hay que saberla)

`v3_quote_unavailable`, `v3_pair_no_pools` y `no_tradable_size` = **0 de 10 001 en el stream**, pero presentes en PG (1,5 % + 0,22 % + 0,21 % ≈ **1,9 %**). Son **motivos terminales del PRODUCTOR**: nunca se publican al stream. **Una muestra del stream los subcuenta por construcción**, no por ventana.

### E. Explicación fechada de una muestra del stream con reparto muy distinto

En t77 medí el stream `validated` **CONGELADO desde 2026-09-17** (`last-generated-id 1789637172953-0`; primera entrada retenida con `detected_at:2026-09-17T09:22:13Z`). **Se descongeló a las 2026-10-07T21:19:58Z**, cuando `simulations` empezó a poblar. Una muestra de `validated` tomada **antes** de ese instante muestreó una población de **19 días**. Comprobado hoy: `last-generated-id = 1791411161025-0` (hoy), `entries-added` 12 247 832 → **12 467 008**.

### F. La cifra que t77 declaró NO COMPUTABLE, y que ahora sí lo es

§7 declaró no computable "¿el mercado es malo?" porque el pipeline estaba cortado. Hoy, con el pipeline moviéndose: **91,0 % de la población de 1 h es económicamente negativa** (`spread_negative_round_trip` 83,5 % + `non_positive_profit` 7,5 %). **Corrobora el "88 % económicamente negativo" del brief, que en t77 no pude ni confirmar ni refutar.** Aparecen además dos clases ausentes en t77: `StrategyDisabled:triangular_arb` (4 554 — **2,0 %**) y `TokenNotAllowed:0x…` (515 — 0,23 %).

### G. Autocorrecciones a §10 (mis hipótesis, refutadas por medición posterior)

- **§10.1 queda REFUTADA.** El gate no es "estructuralmente propenso a fallar": falló por supersesión con causa literal `##[error]RuntimeError: Target superseded by main; deploy the newer validated commit`, y **tras esa falla hubo 6 deploys consecutivos `success`** ⇒ **transitorio**. Dato operativo: la ventana del gate dura **14 min 41 s**.
- **§10.2 queda CORREGIDA.** `n_tup_ins` **ya no sirve** como prueba para `simulations`: leí `n_tup_ins = 201 348` con `COUNT(*) = 201 674` en el mismo instante ⇒ el contador **perdió historia**. **Regla: usar `COUNT(*)`.** (Mecanismo NO COMPUTADO.)

### H. Estado al 2026-10-07T22:12Z (contexto, NO resultado de esta tarea)

`runtime = main = 21d2039c` (brecha cerrada) · `invalid_message` = **0** (era 11 295/20 min) · `simulations` = **205 028+** con **`passed=true` = 0** y `simulated_profit_usd` **100 % NULL** · **100 %** del fallo = `strategy_cyclic_route_not_simulatable_in_s4:*` · `executions` = 0 y `paper_trade_runs` = 0. El veredicto de t77 aplica a SU ventana; éste es el estado posterior.
