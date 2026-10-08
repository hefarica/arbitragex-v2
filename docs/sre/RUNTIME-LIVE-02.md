# RUNTIME-LIVE-02 — identidad de los bytes, recorrido real y contraste antes/después

**Tarea:** t93 (verificación independiente; el publicador fue Release-B/t92) · **Rama:** `sre/runtime-live-02` (base `main` = `327433332732b06975dccadc725940cbeaccd4ca`)
**run_id:** `arbx-entrega-20261007` · **phase_id:** implementation · **permissions:** SOLO LECTURA sobre el VPS y HTTP público. No se reinició, no se desplegó, no se mergeó, no se tocó el runtime.
**Alcance:** `docs/sre/` (único path escrito).

---

## 0. Veredicto en una línea

**La identidad coincide (11 servicios core = `main` = `32743333`), la cadena AVANZÓ UNA CAPA —`strategy_cyclic_route_not_simulatable_in_s4` pasó de 100 % a 0 %— y ahora se detiene en un motivo DISTINTO (`sim_signer_funding_slot_unresolved`, 98,740 % de lo post-deploy); `passed=true` sigue en 0. Y hay un hallazgo no previsto: la ruta que alimenta las cards (`/api/opportunities/live?limit=1`) **cuelga ~300 s** en ambas superficies mientras `limit=20`/`limit=50` responden en 126-1.266 ms.**

## 1. E0 — Control de canal PRIMERO (pasa)

```
$ docker exec arbitragex-v2-postgres-1 psql -U postgres -d arbitragex -tAc "SELECT 1"
1
rc_e0=0
```

`SELECT 1` **devuelve fila**. Todo cero de este informe es un cero **MEDIDO**, no un canal mudo. (Canal: `C:\Program Files\Git\usr\bin\ssh.exe` + alias `arbx`, con el script remoto por `bash -s` y **LF puro**, `CR=0` verificado: los `\r` en los argumentos remotos ya rompieron mediciones antes en esta campaña.)

## 2. E1 — Identidad por BYTES dentro del contenedor

25 contenedores; **14 con etiqueta** (las 11 core: frontend, edge, api-server, searcher-rs, sim-ctl, recon, selector-api, token-enricher, relays-client, socket-proxy, math-engine + las tres que siguen), **11 SIN etiqueta**:

| contenedor | `org.opencontainers.image.revision` |
|---|---|
| frontend · edge · api-server · searcher-rs · **sim-ctl** · recon · selector-api · token-enricher · relays-client · socket-proxy · math-engine | **`327433332732b06975dccadc725940cbeaccd4ca`** |
| promtail · loki | `efc4d2f009e04ecb1db58a637b89b33aa234de34` (imagen propia) |
| anvil | `4072e48705af9d93e3c0f6e29e93b5e9a40caed8` (imagen propia) |
| grafana · postgres · prometheus · vault · alertmanager · redis · minio · node_exporter · thanos-query · thanos-sidecar · thanos-store | **SIN_LABEL** — `11` |

**El vacío es NO COMPUTADO, no un fallo** (declarado así por el contrato): esos 11 servicios no publican la etiqueta.

## 3. E2 — Igualdad `main` == revisión del contenedor: AFIRMADA

```
$ git ls-remote origin refs/heads/main
327433332732b06975dccadc725940cbeaccd4ca	refs/heads/main
$ docker inspect --format '{{.Config.Labels.org.opencontainers.image.revision}}' arbitragex-v2-sim-ctl-1
327433332732b06975dccadc725940cbeaccd4ca
```

**Iguales, byte por byte de SHA.** No se hereda nada: la medición previa del briefing (`21d2039c`) quedó caducada por DOS deploys (`21d2039c → cd1114d1 → 32743333`) y se re-midió.

## 4. E3 — Recorrido real sobre el dominio vivo (dos superficies, NO confundibles)

| superficie / ruta | HTTP | bytes | latencia | server |
|---|---|---|---|---|
| **DApp** `http://195.201.235.70/` | **200** | **188.216** | — | `nginx/1.24.0 (Ubuntu)` |
| **DApp** `/opportunities` | **200** | **254.316** | — | `nginx/1.24.0 (Ubuntu)` |
| **DApp** `/api/health` | **200** | 68 | — | (proxy a api-server) |
| **EDGE** `https://edge-arbx.ape-tv.net/` | **404** | 21 | 0,38 s | `cloudflare` |
| **EDGE** `/opportunities` | **404** | 21 | — | `cloudflare` |
| **EDGE** `/api/opportunities` | **404** | 21 | 0,38 s | `cloudflare` |
| **EDGE** `/api/health` | **200** | 68 | 0,49 s | `cloudflare` |
| **EDGE** `/health` | **200** | 54 | 0,60 s | `cloudflare` |
| **EDGE** `/status` | **200** | 583 | 0,38 s | `cloudflare` |
| **EDGE** `/api/v1/opportunities/live?limit=1` | **404** | 21 | — | `cloudflare` (el edge declara SIN `v1` y agrega `v1` al reenviar) |
| **EDGE** `/api/opportunities/live?limit=1` | **504** | 16 | **60,78 / 61,33 / 60,42 s** (3 intentos) + un `000` | `cloudflare` |
| **DApp** `/api/opportunities/live?limit=1` | **504** | 176 | ~60 s | (nginx → api-server) |
| `195.201.235.70:8080/...` (api-server directo) | **000** | 0 | exit=7 | no publicado (loopback) |

**Son superficies distintas y se sirven distinto:** el DApp es **HTTP plano con nginx** sirviendo HTML (188 KB / 254 KB); el edge es un **borde de API detrás de Cloudflare** con allowlist por ruta que responde `404 {"error":"not_found"}` (21 B, `application/json`) a `/`, `/opportunities` y `/api/opportunities`. Confundirlas llevaría a leer un 404 legítimo del edge como "el DApp está caído".

### 4.1 Hallazgo no previsto: `limit=1` cuelga, `limit=20`/`limit=50` sirven

El 504 **no es transitorio**: 3 intentos consecutivos dieron 504 a los ~60,8 s (el gateway corta) y el 4º un `000`. La causa está medida en el log del api-server — **ventana retenida declarada ANTES de concluir nada (R9)**: `log driver json-file max-file:5 max-size:10m`, `State.StartedAt=2026-10-08T00:09:18.799407804Z`, **55.114 líneas retenidas**, primera línea retenida `2026-10-08T00:09` = **la ventana NO está rotada**, así que lo que sigue es el universo completo de esta vida del contenedor:

```
{"time":"2026-10-08T00:29:04.941Z","req":{"url":"/api/v1/opportunities/live?limit=1"},"res":{"statusCode":null},"responseTime":300696,"msg":"request aborted"}
{"time":"2026-10-08T00:30:19.381Z","req":{"url":"/api/v1/opportunities/live?limit=1"},"res":{"statusCode":null},"responseTime":300543,"msg":"request aborted"}
{"time":"2026-10-08T00:31:31.311Z","req":{"url":"/api/v1/opportunities/live?limit=1"},"res":{"statusCode":null},"responseTime":300381,"msg":"request aborted"}
{"time":"2026-10-08T00:29:03.127Z","req":{"url":"/api/v1/opportunities/live?...&limit=50&..."},"res":{"statusCode":200},"responseTime":388,"msg":"request completed"}
{"time":"2026-10-08T00:30:04.090Z","req":{"url":"/api/v1/opportunities/live?limit=20&order=profit_usd"},"res":{"statusCode":200},"responseTime":1266,"msg":"request completed"}
{"time":"2026-10-08T00:31:03.217Z","req":{"url":"/api/v1/opportunities/live?limit=20&order=profit_usd"},"res":{"statusCode":200},"responseTime":625,"msg":"request completed"}
```

**El discriminante es la forma de la consulta, no la ruta ni el deploy:** `?limit=1` se aborta a los ~300,7 s; `?limit=20` y `?limit=50` (incluso con `order=profit_usd` y `route_representative=best_net`) devuelven 200 en 126-1.266 ms. **La causa raíz NO está computada por esta verificación** (el path está en `backend/api-server/`, fuera de alcance): se reporta la discriminación medida y su consecuencia — el edge y la sonda `?limit=1` no obtienen dato, mientras el frontend con `limit=20/50` sí.

`/api/rejections/breakdown?hours=24` sigue en **503** con `canceling statement due to statement timeout` en 15.039 ms (el defecto de `/dev/shm` de #832; **no es nuevo** y no es de esta tarea).

## 5. E4 — Contraste ANTES/DESPUÉS: **SE COMPARA EL MIX, NO EL VOLUMEN**

**Frontera temporal tomada del artefacto, no supuesta:** `docker inspect --format '{{.State.StartedAt}}' arbitragex-v2-sim-ctl-1` → **`2026-10-08T00:09:18.781787388Z`** (searcher-rs 00:09:18.7799Z · api-server 00:09:18.7994Z · edge 00:09:19.0861Z: el deploy recreó los core en ese instante).

**El agregado MIENTE** y hay que decirlo primero, porque es la trampa que el contrato advirtió:

```
$ psql -tAc "SELECT passed, count(*), count(simulated_profit_usd) FROM simulations GROUP BY passed"
f|618832|0
$ psql -tAc "SELECT fail_reason, count(*) FROM simulations GROUP BY fail_reason ORDER BY 2 DESC LIMIT 15"
strategy_cyclic_route_not_simulatable_in_s4:dex_arb|576907
strategy_cyclic_route_not_simulatable_in_s4:flashloan_arb|23743
strategy_cyclic_route_not_simulatable_in_s4:triangular|16920
sim_signer_funding_slot_unresolved|1239
build_error: amount invalid: zero amount_in|24
```

Leído así, "99,9 % sigue en la familia vieja" ⇒ **conclusión falsa**. Con el corte temporal:

| época | filas | profit computado | clases (n · % de su época) |
|---|---|---|---|
| **PRE** (`< 00:09:18.781787388Z`) | **617.570** | **0** | `strategy_cyclic_route_not_simulatable_in_s4:dex_arb` 576.907 · 93,416 % · `:flashloan_arb` 23.743 · 3,845 % · `:triangular` 16.920 · 2,740 % |
| **POST** (`>= 00:09:18.781787388Z`) | **1.902** | **0** | `sim_signer_funding_slot_unresolved` 1.881 · **98,740 %** · `build_error: amount invalid: zero amount_in` 24 · 1,260 % |

**Proporciones, no totales:** la familia vieja es el **100,000 % de PRE** y el **0,000 % de POST**. Las tres familias que el contrato anticipaba (`cyclic_route_missing_route_metadata:*`, `route_path_not_representable:*`, `candidate_incomplete:*`) están en **0/0/0** post-deploy: el motivo nuevo no es ninguno de esos tres.

**Aguas abajo (medido, no inferido):**

```
simulations|618836|618838   executions|0|0   paper_trade_runs|0|0
VALIDATED_PLAN_KEYS=[0]
```

`executions` y `paper_trade_runs` en **0 filas / 0 inserciones** y `arbx:validated_plan:*` en **0 claves**: coherente con `passed=true` = 0 — ninguna oportunidad alcanza esas etapas.

**DEFERRAL vs VEREDICTO (la advertencia del contrato, medida):** el PEL **no** está acumulando: `XINFO GROUPS` → grupo `sim-ctl-g0`, **`pending 1`** (fue 0 y 3 en dos sondas), `entries-read 12.885.096 → 12.889.992`. Con `pending`≈1, **la caída de volumen NO es un tope de capacidad**: es que se escribe una fila cada ~8,2 entradas leídas (ver §7). Y la cota de #848 **no está desplegada**, así que no hay cota que agotar. Comparar conteos crudos habría producido exactamente la conclusión falsa que el contrato prohíbe.

## 6. E5 — La clase `strategy_cyclic_route_not_simulatable_in_s4`: DESAPARECIÓ

**Cambió de peso relativo, y de la forma más fuerte posible: 100 % → 0 %.** Conteos de ambos lados: **PRE 617.570** (576.907 `dex_arb` + 23.743 `flashloan_arb` + 16.920 `triangular`) · **POST 0**. En la misma ventana post-deploy se escribieron 1.902 filas: la clase no dejó de escribirse por falta de tráfico, dejó de **emitirse**.

El fix de #846 hizo lo que decía: la negativa por nombre se eliminó y la cadena **avanzó una capa**. El nuevo punto de parada es `sim_signer_funding_slot_unresolved` (98,740 %), con un residuo de `build_error: amount invalid: zero amount_in` (24 filas, 1,260 %).

## 7. E10 — RÉGIMEN DE MEDICIÓN DECLARADO

Ventana declarada de **120 s**, T0→T1 dentro de **un solo script** (09:21:43Z → 09:23:43Z… UTC `2026-10-08T00:21:43Z` → `00:23:43Z`):

| medida | T0 | T1 | delta | tasa |
|---|---|---|---|---|
| `entries-added` (llegada del productor) | 12.895.072 | 12.899.977 | **+4.905** | **40,9/s** |
| `entries-read` (consumo del grupo) | 12.885.096 | 12.889.992 | **+4.896** | **40,8/s** |
| `simulations` (filas persistidas) | 619.478 | 620.078 | **+600** | **5,0/s** |
| `lag` | 9.976 | 9.985 | +9 | ≈ `length` |
| `pending` (PEL) | 1 | 1 | 0 | — |

- **Cota efectiva de `sim-ctl`:** **#848 NO está desplegada** (declarado por t92 y coherente con el `env` del contenedor: ninguna variable de cota de admisión; sólo `ARBX_V3_QUOTE_BATCH_SIZE=8`, `ARBX_MULTIHOP_EMIT_MAX_PER_TICK=20`, `ARBX_POOL_ENUM_MAX_NEW_PER_TICK=50`, `ARBX_V3_MULTILEG_MAX_PROBES=2`). **No hay cota que agotar y el PEL no acumula.**
- **Backlog del PEL:** `pending` 1 (sondas: 3 · 1 · 1). `STREAM_LENGTH=10001` y `lag`≈9.985 ≈ la longitud del stream: el grupo va ~una longitud de stream por detrás con el stream topeado a 10.001.
- **Lectura del régimen, sin adornos:** llegan **40,9/s** y el grupo **lee 40,8/s** (no se cae), pero **persiste 5,0/s** ⇒ **~1 fila por cada 8,2 entradas leídas**. Con `pending`≈1 eso **no es deferral**: es una fracción de admisión. **La causa de esa fracción NO está computada acá** (vive en el path de admisión de `sim-ctl`, fuera de alcance) y **no se infiere**. Lo que sí queda declarado es que **el techo no lo está poniendo el PEL ni una cota inexistente**.
- **Sin este régimen, un veredicto sobre `passed=true` se lee fuera de contexto** — por eso va en la misma sección que el conteo: `passed=true` = **0** con la cadena escribiendo 5 filas/s, no por inanición.

## 8. E6 — Gobernanza: el gate sigue cerrado

```
GOV|relays-client|ARBX_LIVE_EXEC_ENABLED=False ARBX_TRADE_MODE=paper
GOV|sim-ctl|ARBX_LIVE_EXEC_ENABLED=False ARBX_TRADE_MODE=paper
```

**`ARBX_LIVE_EXEC_ENABLED=False` verificado DESPUÉS del deploy**, en los dos contenedores que llevan la variable. Sin firma, sin broadcast, sin capital.

## 9. E8 — El deploy SÍ ocurrió (no hay caso NO VERIFICADO)

Deploy **`37700927145`** sobre `32743333` = **`completed/success`**; los contenedores core se recrearon a las **`2026-10-08T00:09:18.78Z`** y su etiqueta horneada es `32743333`. Por tanto **no aplica** el escenario "deploy no ocurrió ⇒ NO VERIFICADO": el contraste antes/después de §5 está medido sobre un despliegue real, y **no se heredó ningún verde de `21d2039c` ni de `cd1114d1`**.

## 10. E9 — Integridad de la métrica: **NO APLICABLE**

`SELECT count(*) FROM simulations WHERE passed = true` → **0** (y `count(simulated_profit_usd)` en la época POST → **0**, es decir 100 % NULL = **NO COMPUTADO, no cero**).

**No hay un solo positivo que sea trazable O sospechoso, porque no hay positivos.** Se declara **NO APLICABLE** con esa razón, como manda el contrato. Procedimiento listo y validado (`bash -n` rc=0) para el día que aparezca el primero: `simulations.opportunity_id` → `opportunities.route_metadata`, comparando contra las patas de `simulations.raw_trace` (esquema confirmado: `id, opportunity_id, simulator, gas_estimate_wei, gas_price_wei, slippage_pct, revert_risk_pct, simulated_profit_usd, passed, fail_reason, raw_trace jsonb, trace_id, simulated_at`).

## 11. E7 — Cada cifra con su comando en la misma línea

Todo `rc=` de este informe viene del comando que lo precede; cada cifra lleva su comando o su `docker inspect` en la misma línea o en la tabla de §4. **Lo NO medido está declarado como tal** (§12), no inferido.

**Fallos de MI instrumento, declarados** (cazados por `rc` o por valor vacío, ninguno dejó una cifra falsa en este informe): (a) `SELECT … GROUP BY` con `count(*)` dentro de la expresión → `ERROR: aggregate functions are not allowed in GROUP BY` (`rc_cut=1`), reparado; (b) `ORDER BY 2` con una sola columna de salida → `ERROR: ORDER BY position 2 is not in select list`; (c) `awk '{print $2}'` sobre `XINFO STREAM` devolvió **vacío** porque la clave y el valor van en líneas separadas (`awk '/^entries-added/{getline; print}'` es lo correcto) — un `entries_added=` vacío que sin mirarlo se lee como 0; (d) en PowerShell, comillas dobles con `$?` producen `RC=True` (un booleano disfrazado de exit code): **comillas simples**.

## 12. Límites (lo que este informe NO prueba)

1. **La causa del cuelgue de `limit=1`** (§4.1) no está computada: se mide la discriminación (1 vs 20/50), no su mecanismo. `backend/` está fuera de alcance.
2. **La causa de la fracción 8,2:1** (§7) no está computada, por la misma razón.
3. **La ventana de logs de `api-server` es la de esta vida del contenedor** (55.114 líneas desde `00:09:18Z`, sin rotación). Para `sim-ctl`/`searcher-rs` **no se declaró ventana** y por eso **no se concluye ninguna ausencia** sobre ellos.
4. **`simulations` es la tabla agregada**: los 617.570 PRE se acumularon a lo largo de horas; el DESPUÉS son 1.902 filas de ~13 minutos de vida del contenedor. Las **proporciones** son comparables; los **totales no**, y así se reportan.
5. **El tamaño del DApp cambió respecto de la referencia heredada** (`~239.567 B` para `/`): mi medición de `/` da **188.216 B** y la de `/opportunities` **254.316 B**. Declaro la discrepancia; **no** computo su causa (no tengo el path exacto que midió la referencia).

## 13. Reproducción

```bash
# identidad y gobernanza (VPS, solo lectura)
ssh arbx "for c in \$(docker ps --format '{{.Names}}'); do echo \"\$c|\$(docker inspect --format '{{index .Config.Labels \"org.opencontainers.image.revision\"}}' \$c)\"; done"
docker inspect --format '{{.State.StartedAt}}' arbitragex-v2-sim-ctl-1
docker exec arbitragex-v2-relays-client-1 env | grep -E 'ARBX_LIVE_EXEC_ENABLED|ARBX_TRADE_MODE'
# corte temporal (la consulta que evita la conclusión falsa)
B=$(docker inspect --format '{{.State.StartedAt}}' arbitragex-v2-sim-ctl-1)
docker exec arbitragex-v2-postgres-1 psql -U postgres -d arbitragex -tAc \
 "SELECT CASE WHEN simulated_at >= '$B' THEN 'POST' ELSE 'PRE' END, coalesce(fail_reason,'<NULL>'), count(*) FROM simulations GROUP BY 1,2 ORDER BY 1,3 DESC"
# régimen
docker exec arbitragex-v2-redis-1 redis-cli XINFO GROUPS arbx:opps:validated
# superficies
curl -s -o NUL -w "%{http_code} %{size_download}" http://195.201.235.70/
curl -s -o NUL -w "%{http_code} %{size_download} %{time_total}" "https://edge-arbx.ape-tv.net/api/opportunities/live?limit=1"
```
