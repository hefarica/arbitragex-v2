# API-LIMIT1-01 — una ruta, dos formas, y el techo de 300 s que no es del pipeline

**Tarea:** t103 · **run_id:** `arbx-entrega-20261007` · **permisos:** SOLO LECTURA sobre el runtime y `backend/`; único path escrito `docs/sre/`.
**`main` medido AHORA:** `git ls-remote origin refs/heads/main` → `fa6f5284cc5bd704cd05bf89d37b50c257adbc74` (**coincide con el SHA_BASE del briefing**). El gate mergeado incluye `if: always()` ×6, `JQ_RC` ×4 y `evidence: http=` ×1 (mi incremento #840 entró: `mergedAt 2026-10-07T19:20:38Z`, merge `62798da6`).

---

## 0. Veredicto en una línea

**La premisa "la ruta decide por la FORMA" se sostiene DENTRO de la ventana del evento y se cae como propiedad determinista:** en la ventana del api-server, `limit=1` tuvo **mediana 300.381 ms y 7 abortos** mientras `limit=20/50` dieron **92-98 ms**, pero **la MISMA forma `limit=1` también completó 5 veces en 74-117 ms**. Hoy (00:37Z) las tres formas responden 200 y en el api-server directo **cuestan lo mismo (~95 ms)**. La causa de la lentitud **NO ESTÁ COMPUTADA**; lo que sí quedó localizado es **el techo del aborto (~300 s), en la capa HTTP y sin override en el código**. Y **L4: el gate SÍ usa `limit=1`**, contra `http://127.0.0.1:8788` (borde interno), con tope `-m 10` — y ese 200 puede servirse desde una **caché del borde**.

## 1. L1 — Control de canal y ventana declarada ANTES de concluir

```
$ docker exec arbitragex-v2-postgres-1 psql -U postgres -d arbitragex -tAc "SELECT 1"
1
rc_c0=0
```

Ventana de logs del `api-server`, declarada antes de cualquier ausencia (R9/LOGFLOOD-01):

```
LOGCONFIG=json-file map[max-file:5 max-size:10m] STARTEDAT=2026-10-08T00:09:18.799407804Z
PRIMERA=[{"level":"info","time":"2026-10-08T00:09:20.467Z","service":
ULTIMA=[{"level":"warn","time":"2026-10-08T00:38:50.946Z","service":
LINEAS=72390
```

La primera línea retenida (`00:09:20.467Z`) es **1,7 s posterior** al `StartedAt` ⇒ **la ventana NO está rotada** y abarca toda la vida del contenedor: las ausencias dentro de ella **sí** son evidencia.

## 2. L2 — Las tres formas: confirmación **dentro de la ventana** y refutación **como propiedad**

### 2.1 Estado actual (2026-10-08, 00:37-00:38Z), tres formas del contrato

| comando (literal) | resultado |
|---|---|
| `curl -s -o NUL -w "%{http_code} %{time_total}" "http://195.201.235.70/api/opportunities/live?limit=20"` | **`200 1.063271`** |
| `curl -s -o NUL -w "%{http_code} %{time_total}" "http://195.201.235.70/api/opportunities/live?limit=50"` | **`200 1.074313`** |
| `curl -s -o NUL -w "%{http_code} %{time_total}" "http://195.201.235.70/api/opportunities/live?limit=1"` | **`200 0.470505`** (reloj local 0,54 s) |

Y contra el **api-server directo** (8080, sin borde de por medio), las tres formas **cuestan lo mismo**:

```
8080 limit=1 -> [200 0.092748]   8080 limit=20 -> [200 0.096059]   8080 limit=50 -> [200 0.095475]
```

### 2.2 La ventana del evento, forma por forma (log del api-server, 72.390 líneas sin rotar)

Extraído con `sed -n 's/.*"url":"\([^"]*opportunities\/live[^"]*\)".*"responseTime":\([0-9]*\).*/\1 -> \2/p'` → **75 pares**:

| forma exacta | n | mediana | max |
|---|---|---|---|
| `/api/v1/opportunities/live?limit=1` | **12** | **300.381 ms** | **300.696 ms** |
| `/api/v1/opportunities/live?limit=20` | 3 | 95 ms | 98 ms |
| `/api/v1/opportunities/live?limit=20&order=profit_usd` | 18 | 279 ms | 1.266 ms |
| `/api/v1/opportunities/live?limit=50` | 3 | 92 ms | 94 ms |
| `/api/v1/opportunities/live?limit=50&order=profit_usd` | 2 | 108 ms | 108 ms |
| `/api/v1/opportunities/live?viable_only=false&limit=50&…&route_representative=best_net` | 35 | 174 ms | 1.230 ms |
| `/api/v1/opportunities/live?order=profit_usd&max_age_seconds=300&route_representative=best_net` | 2 | 119 ms | 119 ms |

**Y el dato que obliga a no simplificar:** las 12 apariciones de `limit=1` **no son todas lentas**. Crudas, en orden de log:

```
?limit=1 -> 300696   ?limit=1 -> 300543   ?limit=1 -> 300381   ?limit=1 -> 300585
?limit=1 -> 300434   ?limit=1 -> 300301   ?limit=1 -> 300384        <-- 7 ABORTOS
?limit=1 -> 84       ?limit=1 -> 74       ?limit=1 -> 117   ?limit=1 -> 92   ?limit=1 -> 92   <-- 5 NORMALES
```

⇒ **La forma es BIMODAL, no deterministamente rota.** En la misma ventana y en el mismo proceso, `limit=1` abortó 7 veces a ~300 s **y** respondió 5 veces en 74-117 ms, mientras las formas de 20/50 filas nunca superaron **1.266 ms**. Estadísticamente la discriminación **se confirma** (mediana 300.381 ms vs 92-98 ms); como propiedad mecánica de la forma, **se refuta** (5/12 fueron normales). Reporto las dos mitades; quedarse con una sola sería el error que esta tarea persigue.

## 3. L5 — Dos capas, dos números (no se colapsan)

| capa | qué se mide | valores literales |
|---|---|---|
| **Borde** (Cloudflare y nginx) | corta la espera y devuelve su propio error | **504** tras **60,78 / 61,33 / 60,42 s** (t93, 3 intentos) |
| **api-server** | aborta la request y lo registra | `res:{statusCode:null} … responseTime:300696 / 300543 / 300381 / 300585 / 300434 / 300301 / 300384` ms, `msg:"request aborted"` |

**60,8 s y 300,7 s no son el mismo número: son dos decisiones distintas en dos capas distintas.** El borde corta primero (~60 s) y el api-server sigue trabajando hasta su propio techo (~300 s): por eso el usuario ve 60 s y el log del servidor registra 300 s para la MISMA request.

## 4. L3 — La causa: **NO COMPUTADA**, con lo que sí quedó excluido y localizado

### 4.1 Excluido por medición

| candidato | evidencia que lo excluye |
|---|---|
| El SQL / el handler dependen del `limit` | **Por construcción no**: `window_total` es `(COUNT(*) OVER ())::int` y el propio código documenta *"Window functions evaluate before LIMIT"* — el set agrupado se materializa entero antes del `LIMIT $1` (`opportunities-live.ts:438-448`, `:473`). Y medido: en 8080 las tres formas dan **0,0927 / 0,0961 / 0,0955 s**. |
| El predicado de la ventana escanea de más | `EXPLAIN` → **`Index Only Scan using idx_opp_detected_at`**, cost 1722.86, `rows=6422` estimadas; **11.116 filas reales** en la ventana de 300 s (rc=0). |
| Bloqueo de tabla | `pg_locks WHERE relation='opportunities'::regclass` → **0 filas** (rc=0). |
| Cola de queries atascadas | `pg_stat_activity … now()-query_start > 5 s` → **`0 vivas>5s`** (rc=0); wait events de las activas: sólo `-/-`. |
| Espera por pool agotado | `backend/api-server/src/index.ts:360-361`: `idleTimeoutMillis: 30_000`, **`connectionTimeoutMillis: 5_000`** ⇒ una adquisición de conexión no puede colgar 300 s: falla a los 5 s. |
| Un `statement_timeout` de PG a 300 s | `pg_settings` → **`statement_timeout = 0` (default)**; los roles sólo llevan `lock_timeout` (`postgres 5s`, `arbx_rw 5s`, `arbx_ro 10s`, `arbx_migrator 10s`) ⇒ **el techo de ~300 s no es de PostgreSQL**. |

### 4.2 Localizado: **dónde** está el techo del aborto (~300 s), no **por qué** la request era lenta

`backend/api-server/src/index.ts:2051` → `httpServer.listen(PORT, () => { … })`, y en **todo** `backend/api-server/src` el grep de `requestTimeout|headersTimeout|keepAliveTimeout|server.timeout` da **0 ocurrencias** ⇒ el servidor HTTP corre con los **defaults de Node**, y el default documentado de `http.Server.requestTimeout` es **300.000 ms**. Coincide con el techo medido (300.301-300.696 ms) y con la forma del registro (`res:{statusCode:null} msg:"request aborted"`, que es el camino de aborto por timeout de request de Node).

**Frontera declarada:** eso localiza *el borde del aborto* en la capa HTTP **sin override en el código** (leído), y hace coincidir el borde con un default documentado. **No verifiqué ese default dentro del proceso en ejecución** (requeriría leer la versión de Node / el estado del server, fuera del alcance de sólo-lectura de esta orden). Y —más importante— **el techo NO es la causa**: la causa es por qué 7 requests tardaron >300 s. **NO COMPUTADA.**

### 4.3 La evidencia exacta que la cerraría

1. **`pg_stat_statements`**: `available=1` pero **`installed=0`** → no hay histórico por sentencia (`ERROR: relation "pg_stat_statements" does not exist`, rc=1). **Habilitarla es un cambio de runtime: fuera de alcance.** Con ella, el `max_exec_time` de la sentencia viva diría si el tiempo se fue en el motor.
2. **Una captura muestreada durante una recurrencia** de `pg_stat_activity` (`query_start`, `wait_event_type`, `wait_event`) + `pg_locks` + el estado del pool del api-server, con la request `limit=1` en vuelo. El episodio duró ~8 minutos (00:23-00:31Z), así que un muestreador de 10 s lo habría atrapado — y no existía.

## 5. L4 — LA PREGUNTA QUE IMPORTABA: **el gate SÍ mide la forma rota**, sobre el borde interno

`origin/main:.github/workflows/pipeline-integrity.yml` **L200** (capa 3), literal:

```
"curl -s -m 10 -w '\n%{http_code}' 'http://127.0.0.1:8788/api/opportunities/live?limit=1' 2>/dev/null || echo '000'" 2>&1)"
```

- **Forma: `limit=1`** — la misma.
- **Superficie: `http://127.0.0.1:8788`** — el **borde interno, en loopback dentro del VPS**, NO el borde público (`edge-arbx.ape-tv.net`) ni el api-server directo (8080).
- **Tope: `-m 10`** — 10 s.

**Medición de la forma EXACTA del gate, hoy** (script LF por `bash -s`, `rc=0`):

```
gate_probe_rc=0
gate_probe_last_line=[200]
gate_probe_bytes=6298
```

⇒ **Hoy el gate mide bien.** Y durante el evento **habría dado rojo de capa 3**: con `-m 10` y una request que tarda 300 s, `curl` corta, el patrón `|| echo '000'` deja `000` como última línea y el veredicto es (L214-216):

```
::error::PIPELINE ROJO capa 3: la API respondio HTTP 000, no 200 — NO es 'cero oportunidades'.
```

Ese rojo **nombra la capa correcta** (no el pipeline entero) gracias al vocabulario de dos modos; pero **no distingue "la API está lenta" de "la API está caída"**, porque su presupuesto es 10 s y el degradado real tarda 300.

### 5.1 Matiz medido que el gate debe conocer: el borde sirve de CACHÉ

Ráfaga sobre la superficie del gate, misma URL:

```
rafaga_1 -> [200 0.121938]      rafaga_2 -> [200 0.002147]      rafaga_3 -> [200 0.002453]
tras_4s_pausa -> [200 0.097747]
```

El patrón **0,122 s (miss) → 0,0021 s (hit) → 0,0025 s (hit) → 0,098 s tras 4 s** prueba que el borde responde desde **caché con TTL de pocos segundos**. Consecuencia, acotada y medida: **un `200` de la capa 3 no prueba que el api-server haya contestado en ese instante** — puede venir de una respuesta cacheada. **DÓNDE vive esa caché: NO COMPUTADO** (`redis-cli --scan --pattern 'arbx:cache:opps*'` sobre `arbragex-v2-redis-1` → **`cache_keys=[]`**, `ttl=NO_KEY`), así que el TTL exacto tampoco está computado. El borde declara la ruta en `edge/worker/src/index.ts:699` (`proxy(c, "/api/v1/opportunities/live", "arbx:cache:opps", 2)`) — **cito la referencia del propio comentario del workflow (L180), no una lectura mía de `edge/`**, que está fuera de alcance.

## 6. L7 — Lo no computado, declarado como tal

1. **La causa de la lentitud de los 7 casos**: NO COMPUTADA (§4.3 con la evidencia que la cerraría).
2. **La verificación en-proceso del `requestTimeout` de Node**: NO COMPUTADA (§4.2).
3. **El TTL y el dueño de la caché del borde**: NO COMPUTADO (§5.1).
4. **`datconfig`**: la consulta falló (`ERROR: column "datconfig" does not exist`, rc=1) — en PG 15 la config por base vive en `pg_db_role_setting`; **no re-medido**, y por eso **no afirmo nada sobre timeouts a nivel base**.
5. **Fallo de MI instrumento, declarado**: mi primera extracción de pares url→responseTime devolvió **`pares_extraidos=0`** porque el patrón `[^}]*` no puede cruzar el `}` que cierra `traceId`. **Un 0 de extracción NO es una ausencia de eventos** — se veía porque ya tenía `requests_con_responseTime=536`. Rehecho con `sed`: **75 pares**.
6. Ventana de logs **sólo** del `api-server`. No declaré ventana de `edge` ni de `nginx`, así que **no concluyo ninguna ausencia sobre ellos**.

## 7. L6 — Alcance

Sólo lectura en el runtime: ningún reinicio, ningún cambio de configuración, ninguna reparación, **ningún merge, deploy ni push a `main`**. El único path escrito es este documento.

## 8. Reproducción

```bash
# L1 control y ventana
ssh arbx "docker exec arbitragex-v2-postgres-1 psql -U postgres -d arbitragex -tAc 'SELECT 1'"
ssh arbx "docker inspect arbitragex-v2-api-server-1 --format '{{.HostConfig.LogConfig.Config}} {{.State.StartedAt}}'"
# L2 las tres formas
curl -s -o NUL -w "%{http_code} %{time_total}" "http://195.201.235.70/api/opportunities/live?limit=1"
curl -s -o NUL -w "%{http_code} %{time_total}" "http://195.201.235.70/api/opportunities/live?limit=20"
curl -s -o NUL -w "%{http_code} %{time_total}" "http://195.201.235.70/api/opportunities/live?limit=50"
# L2 por forma dentro de la ventana
ssh arbx "docker logs arbitragex-v2-api-server-1 2>&1 | sed -n 's/.*\"url\":\"\([^\"]*opportunities/live[^\"]*\)\".*\"responseTime\":\([0-9]*\).*/\1 -> \2/p'"
# L4 la sonda EXACTA del gate
ssh arbx "curl -s -m 10 -w '\n%{http_code}' 'http://127.0.0.1:8788/api/opportunities/live?limit=1'"
# L3 lo que hoy NO existe (la evidencia que faltaría)
ssh arbx "docker exec arbitragex-v2-postgres-1 psql -U postgres -d arbitragex -tAc \"SELECT count(*) FROM pg_extension WHERE extname='pg_stat_statements'\""
```
