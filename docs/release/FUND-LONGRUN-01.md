# FUND-LONGRUN-01 — 381 intentos, 9 tokens, 1 signer: la partición `{0 / unreadable}` es POR TOKEN y no varía en el tiempo

- **Tarea**: `t122` (kind `work`) · **run_id**: `arbx-entrega-20261008` · **phase_id**: `implementation`
- **SHA_BASE**: `8414e51211d0a26d664b7e669af2eacbee8d1dd4` (revisión servida **dentro** del contenedor)
- **Permisos**: **SOLO LECTURA** sobre el VPS. Único path escrito: `docs/release/`. **No** se reinició, reparó, mergeó, desplegó, firmó ni se emitió transacción alguna.
- **Pre-registro**: `docs/release/FUND-LONGRUN-01-PREREG.md`, commit `c90b37731006a35e3106a60d31f70eccf7b643e6`, **publicado a las 05:57:21Z** (verificado con `git ls-remote`), **antes** de extraer un solo agregado. El primer timestamp de medición fue 05:57:47Z.
- **Corrección de premisa recibida del capitán durante la tarea**: el `0` **no** lo causa el fork. Ver §5 y §6: mi medición lo **confirma por el lado de la partición**.

---

## 1. El mínimo exigido: declarado antes, medido después

| id | exigencia declarada | valor medido | ¿cumple? |
|---|---|---|---|
| **MIN-1** | ≥ 200 intentos de `sim.funding_probe` | **381** | **sí** |
| **MIN-2** | ≥ 30 min de marcha continua sin reinicio | **1 842,8 s = 30,71 min** | **sí** |
| **MIN-3** | primera línea retenida a ≤ 5 s del arranque | **0,274 s** | **sí** |
| **MIN-4** | ventana del caudal ≥ 600 s | **954 s** | **sí** |
| **MIN-5** | si no alcanza, ESPERAR (tope 40 min) | esperados **882 s** (05:58:48Z → 06:13:30Z) | aplicado |
| **MIN-6** | tope agotado ⇒ declarar muestra insuficiente | no aplicó: el mínimo se alcanzó | — |

**Enmiendas al pre-registro: NINGUNA.** El primer vistazo dio 174 intentos y 889 s de uptime: **por debajo de MIN-1 y MIN-2**. No se relajó el mínimo ni se concluyó con esa muestra; se esperó.

Ventana de la pasada canónica: **`2026-10-08T05:43:00.420063Z` → `2026-10-08T06:13:41.600130Z`**, cerrada por filtro de timestamp (`$4 < "2026-10-08T06:13:42"`), todas las cifras de esta acta salen de esa ventana.

---

## 2. Control de canal y ventana de logs (AC7, R9)

**Control de canal primero, antes de reportar cualquier cero:**

| control | comando | resultado |
|---|---|---|
| Postgres | `psql -U postgres -d arbitragex -tAc 'SELECT 1'` | `1` · `psql_exit=0` |
| `awk` | `command -v awk` | `/usr/bin/awk` · `awk_exit=0` |
| `grep` | `command -v grep` | `/usr/bin/grep` · `grep_exit=0` |
| `docker` | `command -v docker` | `/usr/bin/docker` · `docker_exit=0` |

**R9 — ventana retenida:**

- `State.StartedAt` = `2026-10-08T05:42:58.789235265Z`, `State.Status` = `running`.
- primera línea retenida = `2026-10-08T05:42:59.063337Z` → **brecha 0,274 s**: la ventana cubre **todo** el ciclo de vida del contenedor.
- última línea = `2026-10-08T06:13:41.600162Z` · **71 267 líneas** totales.
- `log config` = `map[max-file:5 max-size:10m]` = **50 MB**. A las 06:14:36Z el log retenido medía **16 490 496 B / 73 958 líneas** ⇒ **222,97 B/línea** y crecimiento de **48,9 líneas/s** (71 267 → 73 958 en 55 s).
- **NO hubo rotación durante la medición** ⇒ ninguna de las ausencias de esta acta se apoya en una ventana truncada. *Forecast declarado como tal*: a 222,97 B/línea la capacidad de 50 MB son ~235 000 líneas, o sea ~31 % consumido; con la tasa de bytes derivada (≈10,9 KB/s) la rotación llegaría alrededor de las 07:03Z. Es una **proyección sobre una tasa de bytes derivada de una sola muestra**, no una medición.

---

## 3. AC2/AC3 ★ — Cuántos tokens y cuántos signers: **9 tokens, 1 signer**

Cortes independientes del conteo de intentos, los tres coinciden:

| ruta | resultado |
|---|---|
| `… \| grep 'sim.funding_probe' \| wc -l` | **381** |
| `… \| grep sim.funding_probe \| awk -F'"' '$4 < "…06:13:42"' \| wc -l` | **381** |
| campo `outcome` (un valor distinto) = `sim_signer_funding_slot_unresolved` | **381/381** |
| campo externo `token` por intento (`grep -o`, suma) | **381** |
| `arbx_sim_funding_total{outcome="slot_unresolved"}` | **381** |

**Slots**: `"candidate_slots":[0,2,3,9]` en **381/381** intentos ⇒ **381 × 4 = 1 524 sondeos de slot**.

### 3.1 La tabla que decide: hecho `readback` y hecho `balance_of`, SEPARADOS, por token

| token | intentos | slots | `balance_of` = `"0"` | `balance_of` = `"unreadable"` | `readback_matches_sentinel` = `true` | `set_storage_at` = `"true"` |
|---|---|---|---|---|---|---|
| `0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2` (WETH) | 292 | 1 168 | **1 168** | **0** | 1 168 | 1 168 |
| `0x1abaea1f7c830bd89acc67ec4af516284b1bc33c` | 50 | 200 | 0 | **200** | 200 | 200 |
| `0x6982508145454ce325ddbe47a25d4ec3d2311933` (PEPE) | 18 | 72 | 0 | **72** | 72 | 72 |
| `0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48` (USDC) | 10 | 40 | 0 | **40** | 40 | 40 |
| `0x45804880de22913dafe09f4980848ece6ecbaf78` (PAXG) | 6 | 24 | 0 | **24** | 24 | 24 |
| `0x6b175474e89094c44da98b954eedeac495271d0f` (DAI) | 2 | 8 | 0 | **8** | 8 | 8 |
| `0xa606d433971e9ee140e234daa7c94c476e10ead1` | 1 | 4 | 0 | **4** | 4 | 4 |
| `0x95ad61b0a150d79219dcf64e1e6cc01f0b64c4ce` (SHIB) | 1 | 4 | 0 | **4** | 4 | 4 |
| `0x25b4f5d4c314bcd5d7962734936c957b947cb7cf` | 1 | 4 | 0 | **4** | 4 | 4 |
| **TOTAL** | **381** | **1 524** | **1 168** | **356** | **1 524** | **1 524** |

Conteos exactos por `grep -o` sobre la misma ventana, independientes del agrupador: `balance_of":"0"` = **1 168**; `balance_of":"unreadable"` = **356**; suma **1 524** ✔ (cero valores de un tercer tipo). `readback_matches_sentinel":true` = **1 524/1 524**; `:false` = **0**. `storage_readback` = el centinela `0x…5eedf00d00000001` en **1 524/1 524** (1 solo valor distinto). `signer_keyed_key` distintas = **4** (una por slot). `write_rejected` = **0 ocurrencias del campo** y **0 series** en la familia de métricas ⇒ ninguna escritura rechazada.

**Signers**: `signer` = `0x1234567890123456789012345678901234567890` en **381/381** intentos ⇒ **signers distintos = 1**.

### 3.2 Las tres respuestas

- **`balance_of` = centinela: 0 / 1 524.** Ninguno de los 9 tokens, en ninguno de los 4 slots, con `readback_matches_sentinel=true` y `set_storage_at="true"`, produjo el centinela por `balanceOf`.
- **La pregunta abierta de t114 se responde y se afina**: con 9 tokens el fallo **no es de WETH** — los 9 fallan. Lo que **cambia por token** es la **forma** del fallo, y es **totalmente determinista**: WETH ⇒ `"0"` (1168/1168 = 100 %), los otros 8 ⇒ `"unreadable"` (356/356 = 100 %).
- **AC3 — el dato que desbloquearía todo NO apareció**: no existe ningún caso donde `balanceOf` reproduzca el centinela. Se declara su **ausencia**, no se estima.

---

## 4. ★ Punto 3 del capitán: ¿la partición es estable por token, o varía con el tiempo?

**Cubetas temporales DISJUNTAS** sobre la misma ventana cerrada (B1 `05:43:00–05:50:40` · B2 `05:50:40–05:58:20` · B3 `05:58:20–06:06:00` · B4 `06:06:00–06:13:42`; totales por bucket **96 + 84 + 96 + 105 = 381** ✔):

| bucket | WETH intentos | WETH `"0"` | WETH `"unreadable"` | no-WETH intentos | no-WETH `"0"` | no-WETH `"unreadable"` |
|---|---|---|---|---|---|---|
| **B1** | 74 | 296 | **0** | 22 | **0** | 88 |
| **B2** | 67 | 268 | **0** | 17 | **0** | 68 |
| **B3** | 73 | 292 | **0** | 23 | **0** | 92 |
| **B4** | 78 | 312 | **0** | 27 | **0** | 108 |
| **TOTAL** | **292** | **1 168** | **0** | **89** | **0** | **356** |

**Resultado: ESTABLE POR TOKEN y NO varía con el tiempo.**

- WETH: **0 `unreadable` en 1 168 sondeos** repartidos en 4 cubetas y 30,7 min.
- no-WETH: **0 `"0"` en 356 sondeos** repartidos en las mismas 4 cubetas.
- No hay una sola excepción, ni un bucket donde el modo se dé la vuelta. La mezcla de tokens por bucket es aproximadamente estacionaria (22/17/23/27 intentos no-WETH), así que la estabilidad **no** es un artefacto de composición.
- Determinismo por slot también: los 4 slots dan la misma partición (coincide con el segundo lector independiente, `docs/review/FUND-INDEP-01.md` §1: `188/51` idéntico en los 4 slots).

**Lectura exigida por el contrato — ¿confirma o refuta?**

| hipótesis | predicción | medido |
|---|---|---|
| **fork / salud del endpoint** como causa de la partición | variación **correlacionada en el tiempo** (ráfagas de salud): WETH debería dar `unreadable` *a veces*, y los otros `"0"` *a veces* | **refutada**: 0/1 168 y 0/356, en 4 cubetas |
| **camino de lectura / calldata** (mecanismo medido por el capitán y por `FUND-INDEP-01`) como causa | partición **determinista por token**, invariante en el tiempo | **confirmada**: exactamente eso |

**Y una limitación declarada, no tapada**: los registros `sim.funding_probe` **no contienen el endpoint RPC** (`grep -c -i 'rpc|endpoint|http|url'` sobre las 381 líneas = **0**). La variación *por endpoint* **NO ES COMPUTABLE** con estos bytes. Lo que sí es computable —y es lo que se computó— es la variación **en el tiempo**, que es la variable observable de la salud del endpoint. La refutación del fork se apoya en el tiempo, no en una lectura de endpoint que el instrumento no registra.

---

## 5. Autocorrección: el límite que t114 declaró era FALSO, y está medido

t114 declaró textualmente *"25 intentos × 4 slots = 100 sondeos … un solo token (WETH) y un solo signer"*. **Medido sobre la ventana temprana de t114 (`≤ 05:45:30`, intentos de `05:43:00.420063Z` a `05:45:27.882901Z`): 32 intentos y 5 tokens distintos**, no 1:

| token | intentos | slots | `"0"` | `"unreadable"` |
|---|---|---|---|---|
| WETH | 25 | 100 | **100** | 0 |
| `0x1abaea…1bc33c` | 3 | 12 | 0 | 12 |
| PEPE | 2 | 8 | 0 | 8 |
| USDC | 1 | 4 | 0 | 4 |
| SHIB | 1 | 4 | 0 | 4 |
| **TOTAL** | **32** | **128** | **100** | **28** |

El corte de t114 (25 intentos, 100 slots, **76** `"0"` + **24** `"unreadable"`) se reconstruye **exactamente**: **19 intentos de WETH** (19×4 = 76 slots, 100 % `"0"`) + **6 intentos no-WETH** (6×4 = 24 slots, 100 % `"unreadable"`). 76 + 24 = 100 ✔.

**Consecuencia, dicha sin adornos**: los `24/100 "unreadable"` de t114 **no eran un tercer desenlace aleatorio de los sondeos de WETH** — eran **los 6 intentos de otros tokens**, que son `"unreadable"` al 100 %. t114 **agregó por encima del token** y después declaró que el universo de tokens era 1, lo que hizo que la partición pareciera una mezcla inexplicada. La conclusión **(i)** de t114 ("el índice no cubre el layout") se apoyaba en esa muestra mal agrupada. Esta acta **no la re-deriva ni la defiende**: registra la medición que la desmiente por composición, y se alinea con el mecanismo medido del calldata (§6).

---

## 6. Corrección de premisa del capitán: qué aporta esta medición y qué NO

**La corrección del capitán** (recibida durante la tarea): el calldata de `balance_of` en `8414e512` (`backend/sim-ctl/src/signer_funding.rs:413-418`) mide **24 B** (4 + 20) para un argumento estático de 32; `CALLDATALOAD(4)` rellena con ceros y `uint160` se queda con los 20 bytes bajos ⇒ el nodo consulta **otra dirección**. Medido por el capitán en **dos RPCs independientes** (`ethereum-rpc.publicnode.com` y `eth.drpc.org`) con WETH9 y el par UniswapV2 USDC/WETH: calldata 36 B ⇒ `0xd87404ab02c1bbc957`; calldata 24 B ⇒ **cero**. Y `FUND-INDEP-01` §1.3 llega al mismo mecanismo por otra vía (dirección realmente consultada `0x5678901234567890000000000000000000000000`; clave escrita presente 228 veces, clave leída 0 veces).

**Lo que aporta MI medición** (y es lo único que reclamo): la **invariancia por token en el tiempo y por slot** de §3.1/§4 — 1 524 sondeos, 4 cubetas, 9 tokens, **cero excepciones**. Es la firma de un defecto **determinista del camino de lectura**, y es incompatible con una causa de salud de fuente, que produciría variación temporal. **No** re-derivo el calldata, **no** re-mido los RPCs: esos artefactos son del capitán y de `FUND-INDEP-01` y se citan como tales.

**Y explícitamente: la partición `{0 / unreadable}` NO se atribuye a la fuente del fork en esta acta.**

---

## 7. AC4 — Los `"unreadable"` con su propio número, en dos rutas, y una discrepancia NO COMPUTADA

- **Ruta LOG (mía, medida en estos bytes)**: **356 / 1 524 slot-probes = 23,36 %**. t114 midió 24/100 = **24,0 %** (misma ruta, denominador = slots). **Δ = −0,64 pp: la fracción se MANTIENE.**
- **Ruta CONTADOR del propio instrumento (Prometheus, `arbx_sim_funding_total{outcome="balance_unreadable"}`)**: **445** a las 06:13:41Z. Denominadores posibles, ambos declarados: `445 / (445+381+1524) = 445/2350 =` **18,94 %** (suma de outcomes) y `445/1524 =` **29,20 %** (slots). La cifra que el contrato cita de t107 es **18,68 %** por esta vía: **Δ = +0,26 pp ⇒ se MANTIENE** por esa ruta y ese denominador.
- `"unreadable"` **nunca** se agrupa con `"0"`: son 356 y 1 168, hechos separados, y por token son **disjuntos** (§3.1).

**Discrepancia declarada, NO COMPUTADA**: el contador da **445** y el log da **356** en el mismo instante y sobre el mismo contenedor. Diferencia = **89**, que es **exactamente** el número de intentos con al menos un slot `unreadable` (50+18+10+6+2+1+1+1 = 89). Dos candidatos vivos, sin discriminar: (a) el contador incrementa por **lectura intentada** (con reintentos) y no por slot; (b) incrementa por slot **y** por intento con fallo. **No está establecido cuál**: no se convierte en hallazgo ni se descarta. (El segundo lector ya reportó una discrepancia menor, de 12, atribuida al intervalo de scrape; la de 89 es de otro orden y **no** se explica por scrape — un scrape hace que el contador vaya por detrás, no por delante.)

---

## 8. AC5 — Re-muestreo del caudal con ventana de **954 s** (la anterior fue de 60 s)

Dos muestras: **S1 = `2026-10-08T05:57:47Z`** · **S2 = `2026-10-08T06:13:41Z`** · **Δt = 954 s**.

| serie | S1 | S2 | Δ | **tasa** |
|---|---|---|---|---|
| `claimed_count` (Prometheus) | 33 936 | 70 461 | 36 525 | **38,28/s** |
| `entries-read` (Redis) | 13 848 641 | 13 910 004 | 61 363 | **64,32/s** |
| `entries-added` (Redis) | 13 860 114 | 13 921 477 | 61 363 | **64,32/s** |
| `pending` (`XINFO GROUPS`) | 10 737 | 12 291 | +1 554 | **+1,63/s** |
| `lag` (`XINFO GROUPS`) | 11 473 | 11 473 | **0** | 0/s |
| `pending_count` (Prometheus) | 13 803 | 13 875 | +72 | +0,075/s |
| `oldest_pending_ms` (Prometheus) | 220 063 | 222 641 | +2 578 ms | +2,70 ms/s |
| `simulations` `anvil` | 174 | 381 | 207 | 0,217/s = **13,02/min** |
| `simulations` `revm` | 40 | 77 | 37 | 0,039/s = 2,33/min |
| `simulations` total | 214 | 458 | 244 | 0,256/s = **15,35/min** |
| `bound_report.admitted` (evento de log) | 124 | 237 | 113 | **0,188/s** |
| `bound_report.deferred` (evento de log) | 59 781 | 121 743 | 61 962 | **103,27/s** |
| `bound_report.terminated_inadmissible` | 40 | 40 | **0** | 0/s |

- **`lag` no es backlog**: `entries-added − entries-read` = 13 921 477 − 13 910 004 = **11 473 exacto**, igual al `lag` reportado. Se reporta como **diferencia de contadores**, no como "11 473 entradas sin entregar". Y el consumidor lee **exactamente** al ritmo de llegada (ambos Δ = 61 363).
- **`claimed_count` a 38,28/s**. t110 midió **48,9/s** antes de #857 y **~0 en una ventana de 60 s** después. **Con 954 s el resultado es 38,28/s, no 0.** Una ventana de 60 s no puede distinguir "no hay reclamos" de "no hubo un pase de reclamo en esta ventana": eso es exactamente lo que el contrato dice que pasó. **Se declara la discrepancia con la lectura de 60 s, y NO se iguala `claimed_count` con "churn"**: no existe serie de churn en el registro (coincide con `FUND-INDEP-01` §4.2), y `claim_failures = 0` y `ghost_acked = 0` en las dos muestras.
- **Corroboración independiente del reprocesamiento**: en la ventana de 600,005 s del `bound_report`, entran **64,32/s** y se difieren **103,27/s** ⇒ **1,61 diferimientos por entrada leída**. Las dos tasas vienen de **ventanas distintas** (600,005 s vs 954 s), así que el cociente es **indicativo, no exacto**, y así se declara: es consistente con que la misma entrada vuelva a pasar por el consumidor.

### 8.1 Margen de retención — con su tasa y su **umbral de signo** (AC5, F4)

`MAXLEN` = **10 000** (`length` 10 002 en S1 y 10 000 en S2; `max-deleted-entry-id = 0-0`).

- **tasa de llegada = 64,32/s** (954 s)
- ventana retenida = 10 000 / 64,32 = **155,47 s**
- `oldest_pending_ms` = **222,64 s** (S2)
- **margen = 155,47 − 222,64 = ` −67,17 s` ⇒ SIGNO NEGATIVO**
- **umbral de signo = 10 000 / 222,64 = ` 44,92/s`** ⇒ la tasa medida **64,32/s está 19,4/s por encima del umbral**: el signo negativo **no** depende de este instante concreto, sólo su magnitud.

Contraste: t110 midió **+68,62 s a 21,1/s** y **−205,85 s a 70,75/s** (umbral ~28,8/s). Esta medición cae del mismo lado que su caso de tasa alta y **matiza la magnitud**: −67,17 s en vez de −205,85 s, porque `oldest_pending_ms` bajó de 347,19 s a 222,64 s y la tasa bajó de 70,75/s a 64,32/s. **El signo es función de la tasa y de `oldest_pending`, no una propiedad estructural** — igual que t110 concluyó.

---

## 9. AC6 — `bound_report`: es un EVENTO DE LOG, y su cadencia se mide

```
05:53:08.672649Z  admitted=124  deferred=59781  terminated_inadmissible=40  max_sims_per_sec=1.0  max_in_flight=1
06:03:08.677680Z  admitted=237  deferred=121743 terminated_inadmissible=40  max_sims_per_sec=1.0  max_in_flight=1
```

- **Cadencia medida = 600,005 s** entre los dos eventos (el contrato declara `BOUND_LOG_INTERVAL = 600 s`: coincide). El primero cae a **609,884 s** del arranque del contenedor.
- **La trampa evitada**: un `grep` en los primeros ~9 minutos habría dado **0** — un **cero de INSTRUMENTO** (la cadencia no había vencido), no un instrumento mudo. Aquí el instrumento habló **dos veces con números**, y ese es el dato que se reporta.
- **Observación declarada, sin interpretar**: el **tercer** evento, esperado a ~`06:13:08.68Z` (+600 s del segundo), **NO estaba presente** a las 06:13:41Z (+33 s), 06:14:36Z (+88 s) ni 06:15:15Z (+127 s): el `grep -c` siguió dando **2**. Se registra como **observación de instrumento**: la cadencia de 600,005 s está **medida** para los dos primeros eventos, y **no** está establecido que sea un intervalo estricto de reloj. **No** se convierte en cero de fenómeno, y **no** se concluye nada sobre lo que el tercer evento habría dicho.
- `terminated_inadmissible = 40` en las dos ventanas (Δ = 0): el log no determina si es acumulativo o por ventana ⇒ **ambiguo, declarado** (coincide con el segundo lector).
- `max_sims_per_sec = 1.0` y `max_in_flight = 1`: el techo de admisión es explícito y acota `simulations` a 0,256/s mientras llegan 64,32/s.

---

## 10. `passed=true` = 0, con productor PROBADO

- **Prometheus**: la familia `arbx_simulation_total` tiene **sólo** `{passed="false",simulator="anvil"} = 381` y `{passed="false",simulator="revm"} = 77`. **La serie `passed="true"` NO EXISTE** ⇒ se reporta como **ausencia de serie (cero de instrumento)**, NO como un cero medido.
- **Postgres (cero medido)**: `executions` = **0** filas · `paper_trade_runs` = **0** filas (`psql -tAc 'SELECT count(*)'`, canal probado con `SELECT 1` → `1`).
- **Productor probado**: dos simuladores EVM distintos emiten `passed="false"` ⇒ el camino produce, y produce cero aprobaciones.

---

## 11. Defectos de MIS propias mediciones (declarados antes de que otro los encuentre)

1. **`rb_true=0 / rb_false=1524` en mi primer `awk`: artefacto de parser.** `readback_matches_sentinel` es un booleano **sin comillas** (`:true`), y con `-F'"'` el campo `$(i+2)` cae en la clave siguiente. **Lo cacé cruzando contra el conteo directo** (`grep -o ':true'` = 1 524 / `:false` = 0) y **no reporté** ese resultado. El `awk` quedó validado sólo para los campos con valor **entrecomillado** (`balance_of`, `token`, `slot`), y su partición por token se validó contra los conteos exactos por `grep -o` (1 168 / 356 ✔).
2. **`balance_of por valor` con `cut -d'"' -f4` mezcló la clave con el valor** (el primer objeto de cada línea arranca con `{`), dando un cubo espurio de 393. Se descartó esa ruta y se usó el `grep -o` de la cadena literal completa.
3. **La afirmación "un solo token" de mi t114 era FALSA** (§5), y con ella la conclusión (i) de esa acta. Medido, no inferido: 5 tokens en la ventana temprana y reconstrucción exacta 19×WETH + 6×otros = 100 slots.
4. **`tr -d '\\'` emite `tr: warning: an unescaped backslash at end of string`** en todas las pasadas: ruido en stderr, **sin** efecto en stdout (los conteos cierran por tres vías independientes). Se declara para que nadie lo lea como fallo de comando.

---

## 12. Lo que NO queda establecido (fail-honest)

1. **Variación por endpoint**: el log **no registra el endpoint RPC** (0 coincidencias de `rpc|endpoint|http|url` en 381 líneas) ⇒ **NO COMPUTADO**. La refutación del fork se apoya en la invariancia **temporal**, que sí es computable.
2. **Reconciliación `balance_unreadable` 445 (contador) vs 356 (log)**: dos candidatos vivos (§7), sin discriminar. **NO COMPUTADO.**
3. **`claim_count` 38,28/s ≠ "churn"**: no existe serie de churn en el registro. La tasa está medida; **su descomposición entre reclamo nuevo y reprocesamiento NO está medida**.
4. **El tercer `bound_report` ausente a +600 s**: observación de instrumento (§9), **sin causa establecida**.
5. **Qué token es** `0x1abaea1f7c830bd89acc67ec4af516284b1bc33c`, `0xa606d433971e9ee140e234daa7c94c476e10ead1` y `0x25b4f5d4c314bcd5d7962734936c957b947cb7cf`: las etiquetas `(WETH)`, `(PEPE)`, `(USDC)`, `(PAXG)`, `(DAI)`, `(SHIB)` provienen de `docs/review/FUND-INDEP-01.md` §1 (segundo lector). Los tres sin etiqueta quedan **sin etiquetar** — no se les asigna símbolo por parecido.
6. **Por qué el dispatcher de los 8 tokens no-WETH no devuelve 32 B decodificables con el calldata corto**: no medido en esta acta. Candidatos (a) y (b) de `FUND-INDEP-01` §3, sin discriminar.

---

## 13. Respuesta directa al objetivo de la fase

**¿Cuántos tokens y cuántos signers distintos aparecen?**
**9 tokens distintos y 1 signer**, en **381 intentos** (1 524 sondeos) sobre **1 842,8 s = 30,71 min** de marcha continua, con el mínimo (≥200 intentos y ≥30 min) **pre-registrado y publicado antes de mirar**.

**¿Se distingue "falla WETH" de "falla todo"?**
**Sí, y la respuesta no es ninguna de las dos que el contrato anticipaba.** El fallo es de **los 9 tokens** (`balance_of` = centinela: **0/1 524**), pero **no es uniforme**: se parte **por token**, de forma **totalmente determinista y estable en el tiempo** — WETH ⇒ `"0"` (1 168/1 168) y los otros 8 ⇒ `"unreadable"` (356/356), con **cero excepciones en 4 cubetas temporales disjuntas**. Esa firma es de un defecto **determinista del camino de lectura**, y **refuta** que la partición venga de la salud del endpoint/fork. El `readback` sigue diciendo **sí** (1 524/1 524) mientras `balanceOf` dice **no** (0/1 524): los dos hechos, separados, por token.

**Margen de retención**: **−67,17 s** con tasa **64,32/s** (954 s) y **umbral de signo 44,92/s** ⇒ signo **NEGATIVO**, a 19,4/s por encima del umbral.

---

### Anexo — comandos y artefactos citados

```
[control]  command -v awk|grep|docker   -> /usr/bin/* · exit 0 (tres)
[control]  docker exec arbitragex-v2-postgres-1 psql -U postgres -d arbitragex -tAc 'SELECT 1'   -> 1 · exit=0
[R9]       docker inspect arbitragex-v2-sim-ctl-1 --format '{{.State.StartedAt}}'   -> 2026-10-08T05:42:58.789235265Z
[R9]       docker logs arbitragex-v2-sim-ctl-1 2>&1 | head -1   -> 2026-10-08T05:42:59.063337Z
[N]        docker logs arbitragex-v2-sim-ctl-1 2>&1 | grep -c sim.funding_probe   -> 381
[token]    … | grep sim.funding_probe | awk -F'"' '$4 < "2026-10-08T06:13:42"' | grep -o 'sim.funding_probe","token":"0x[0-9a-f]*"' | sort | uniq -c
[hechos]   … | tr -d '\\' | grep -o 'balance_of":"0"' | wc -l                -> 1168
           … | tr -d '\\' | grep -o 'balance_of":"unreadable"' | wc -l       -> 356
           … | tr -d '\\' | grep -o 'readback_matches_sentinel":true' | wc -l -> 1524   (":false" -> 0)
           … | tr -d '\\' | grep -o 'storage_readback":"0x…5eedf00d00000001"' | wc -l -> 1524
[cubetas]  … | awk -F'"' (buckets B1..B4 por substr del timestamp) x (token, balance_of)   -> §4
[caudal]   docker exec arbitragex-v2-redis-1 redis-cli XINFO GROUPS arbx:opps:validated   -> §8 (S1 05:57:47Z / S2 06:13:41Z)
           docker exec arbitragex-v2-redis-1 redis-cli XINFO STREAM arbx:opps:validated   -> §8
           curl -s localhost:3003/metrics | grep arbx_sim                                 -> §8, §10
[bound]    docker logs … | grep bound_report            -> 2 eventos, Δ 600,005 s          -> §9
[PG]       psql -tAc 'SELECT count(*) FROM executions'        -> 0                            -> §10
           psql -tAc 'SELECT count(*) FROM paper_trade_runs'  -> 0                            -> §10
[pre-reg]  docs/release/FUND-LONGRUN-01-PREREG.md @ c90b37731006a35e3106a60d31f70eccf7b643e6 (05:57:21Z)
[externo]  docs/review/FUND-INDEP-01.md   (segundo lector independiente; calldata 24 B; etiquetas de token)
[externo]  corrección de premisa del capitán (dos RPCs independientes, calldata 36 B vs 24 B)
```

*Acta de re-muestreo en marcha larga. No modifica ningún artefacto medido: sólo lee bytes del contenedor desplegado.*
