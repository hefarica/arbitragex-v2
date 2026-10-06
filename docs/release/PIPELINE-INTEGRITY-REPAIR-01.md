# PIPELINE-INTEGRITY-REPAIR-01 — el vigía vuelve a poder medir

**Estado: instrumento REPARADO y verificado en su estructura; el veredicto del pipeline está MEDIDO.**
La corrida de Actions despachada para probarlo quedó **encolada sin adquirir runner** (clase A) — se declara
abajo sin adornos.

| | Valor |
|---|---|
| Workflow | `.github/workflows/pipeline-integrity.yml` (modificado) |
| PR | **#831** — https://github.com/hefarica/arbitragex-v2/pull/831 (OPEN, sin merge) |
| Rama | `ci/pipeline-integrity-connect-01` · head verificado por el remoto |
| Run despachado | **`37411885810`** (`gh workflow run … --ref ci/pipeline-integrity-connect-01`) |
| Estado de ese run | **`queued`, `runner_id=0`, `steps=0`** — declarado abajo |

---

## 1. El defecto, y por qué es peor que una molestia de CI

Todos los runs morían en el **PRIMER step**:

```
ssh: Could not resolve hostname arbx
exit 255
```

`pipeline-integrity.yml` usaba el alias de ssh **sin definirlo nunca**: no materializaba clave, ni host, ni
bloque `Host` en `~/.ssh/config`. **El alias sí funciona donde está definido**: `deploy.yml:57`,
`deploy-edge-only.yml:38`, `deploy-edge-only-v2.yml:39` y `audit-vps-wiring.yml:212` escriben ese bloque.
`pipeline-integrity.yml` **no era uno de ellos**.

**Alcance mayor, medido:** hay **29 apariciones** del alias desnudo en `.github/workflows/` y **solo 4
workflows lo definen**. Los otros 3 que lo usan sin definirlo tienen, por construcción, el mismo defecto.
*(No se tocan acá: fuera de esta orden. Se reporta.)*

**Y es grave por esto:** sus **6 `failure` consecutivos eran del CHEQUEO, no del pipeline.** Leídos como
*"el pipeline lleva días roto"* afirman algo que el log **no dice**. Es el modo de fallo (d): **una etiqueta
leída como si fuera otra**. El vigía del rechazo masivo —el órgano que debía detectar que las cards no
muestran oportunidades reales— **no podía conectarse**, y por eso nadie tenía el número.

---

## 2. Los DOS modos de rojo, ahora separados

| Modo | Cómo se emite | Qué afirma |
|---|---|---|
| **No pude conectar** | `VIGIA=SIN_CONEXION` + `::error::VIGIA SIN CONEXION (ssh rc=…)` | **Nada sobre el pipeline.** No se midió. Las capas quedan `NO EVALUADO`, no `ROJO`. |
| **El pipeline está roto** | `VIGIA=EVALUADO` + `::error::PIPELINE ROJO capa N` | Que **una capa real** está vacía o rota. |

El preflight emite `reachable=true/false` como *output*; las 5 capas tienen `if: steps.conn.outputs.reachable == 'true'`;
y el step final dice cuál de los dos modos ocurrió. **Un rojo de conexión ya no puede leerse como un rojo de pipeline.**

---

## 3. Qué cambia (sólo el transporte) y qué NO

**Cambia:**
1. **Materializa clave/host** con el mismo mecanismo que `auto-deploy-vps.yml:58-71` (el que funciona y lo
   prueba cada deploy): `install -m 700 -d ~/.ssh`, `umask 077`, la clave a `~/.ssh/deploy_key`,
   `ssh-keyscan` a `known_hosts`. **Más `chmod 600` explícito**: en `G2-CHANNEL-01` (t60) una clave en `0644`
   hizo fallar un run con `Load key: bad permissions`. No se repite.
2. **Forma explícita** `ssh -i ~/.ssh/deploy_key -p "$VPS_PORT" "$VPS_USER@$VPS_HOST"`. **Cero líneas
   ejecutables usan el alias** (verificado: 0 coincidencias en líneas `run:`; 6 usos de la forma explícita).
3. **Elimina el input libre `vps_host`** (`type: string`, default `localhost`). Dos motivos medidos:
   (a) en el `cron` es vacío ⇒ el default `localhost` hacía que las capas 3 y 4 apuntaran **al runner**, y su
   rojo no hablaba del pipeline; (b) era superficie de inyección dentro de una URL. Ahora el host sale de
   `secrets.VPS_SSH_HOST`. Sin input libre, en línea con lo que `t54`/`t58` ya descartaron.
4. **Preflight con discriminador de causa**: `ssh` usa `rc=255` para sus propios errores, así que un fallo de
   conexión **no** se reporta como "contenedor no encontrado". Es exactamente la lección que `t60` pagó.
5. **Las capas 3 y 4 corren el `curl` EN el VPS**, no en el runner. Motivo medido: el edge está publicado
   **sólo en loopback** (`compose.prod.yml`: `- 127.0.0.1:8788:8787`), así que un `curl` desde el runner a
   `<host>:8787` no llega nunca y su rojo hablaría del **transporte**.

**NO cambia:** las capas, sus umbrales, sus mensajes de pipeline y sus veredictos. **Esta orden restaura el
instrumento; no arregla el pipeline.**

---

## 4. El veredicto del gate HOY — medido, capa por capa

Corrido con el camino SSH habilitado, ejecutando **las mismas sondas que el workflow**, con el **control
obligatorio** primero:

| Control | Salida | Lectura |
|---|---|---|
| `ssh arbx "… psql … -tAc 'SELECT 1'"` | **`1`** | **El canal habla** ⇒ los ceros de abajo son reales, no basura del instrumento. |

| Capa | Sonda | Medición | Veredicto |
|---|---|---|---|
| 1 Redis | `redis-cli XLEN arbx:opps:detected` | **10002** | **VERDE** |
| 2 PG | `COUNT(*) FROM opportunities WHERE detected_at > NOW() - INTERVAL '5 minutes'` | **4432** | **VERDE** |
| **3 API** | `curl 127.0.0.1:8788/api/v1/opportunities/live?limit=1` | **HTTP 404**, cuerpo vacío | **ROJO** |
| 4 WS | `curl -o /dev/null -w '%{http_code}' 127.0.0.1:8080/socket.io/?EIO=4&transport=polling` | **200** | **VERDE** |
| 5 Docker | `docker ps … health=unhealthy` + total | 0 unhealthy · **25** contenedores | **VERDE** |

### El hallazgo de la capa 3, con su matiz medido

La capa 3 da rojo **y el matiz importa** — se midió contra los tres puertos:

| Destino | `/api/v1/opportunities/live?limit=1` | `/api/status` |
|---|---|---|
| edge `127.0.0.1:8788` | **404** | **200** |
| edge `127.0.0.1:8787` | **404** | — |
| **api-server `127.0.0.1:8080`** | **200** | — |

**El edge está vivo** (sirve `/api/status` con 200), pero **no sirve `.../opportunities/live`** — ese path
responde **404** por el edge y **200** por el api-server directo. La sonda usa el edge porque es el mismo
destino que el workflow original intentaba (`<VPS_HOST>:8787`, el edge según RULE 02); `8788` es su puerto
publicado en loopback. **El rojo es entonces una medición fiel de la intención original, no un artefacto del
puerto elegido.**

**NO se parchea.** Esta orden restaura la capacidad de medir; el veredicto es otro asunto y se reporta:
**el gate, una vez conectado, da ROJO HOY, en la capa 3.**

---

## 5. La corrida de prueba: despachada, y qué pasó

El workflow **ya existía en `main`**, así que es despachable con `--ref`:

```
gh workflow run "Pipeline Integrity Gate" --repo hefarica/arbitragex-v2 --ref ci/pipeline-integrity-connect-01
  -> https://github.com/hefarica/arbitragex-v2/actions/runs/37411885810   (exit 0)
```

**Estado de ese run al cierre: `queued`, `runner_id=0`, `steps=0`** — la clase A (runner nunca adquirido),
con el repo con decenas de runs en cola en ese momento. **No es un defecto del arreglo**: el run del canal
de t60 atravesó la misma cola (~16 min) y terminó `success`.

**Se declara sin adornos:** la evidencia del veredicto de §4 es una **ejecución directa de las mismas sondas
por el camino SSH habilitado**, no la salida de ese run de Actions. La conclusión del step de Actions queda
**pendiente de que la cola lo despache**. No se declara como si ya hubiera corrido.

**Y una precisión necesaria sobre el `--ref`:** la corrida despachada usa la versión del workflow **de mi
rama**, no la de `main`. Es la única forma de probar el arreglo sin mergear (un `workflow_dispatch` sólo es
despachable si el archivo existe en la rama por defecto, y existe; el `ref` elige **qué versión** corre). El
arreglo llega a `main` por el PR #831, que **no se mergea**.

---

## 6. Declaraciones exigidas

- **NO mueve P/N: sigue `0/115`.**
- **No se firma, no se emite, no se toca capital.** `ARBX_TRADE_MODE=paper` intacto.
- **`ARBX_DRIFT_TRACKER_MODE` y cualquier otra palanca de producto quedan como están.** No se tocó ninguna.
- **No se arregló el pipeline.** Sólo el transporte del instrumento.
- **No se mergea.** Rama + PR #831.

### Un pendiente de higiene que declaro

Queda **1 aparición** del literal del alias en un **comentario** del encabezado (L15), describiendo el
defecto. **Cero líneas ejecutables lo usan** (verificado). Lo ideal es que un grep dé 0 también en la prosa;
lo dejo declarado en vez de afirmar que el archivo está limpio.

---

## 7. Evidencia reproducible

```
gh workflow run "Pipeline Integrity Gate" --ref ci/pipeline-integrity-connect-01   -> run 37411885810, exit 0
gh api repos/hefarica/arbitragex-v2/actions/runs/37411885810/jobs                 -> queued, runner_id=0, steps=0
git ls-remote origin refs/heads/ci/pipeline-integrity-connect-01                  -> coincide con HEAD local (t55)

# las sondas de §4, con el control obligatorio primero:
ssh arbx "docker exec arbitragex-v2-postgres-1 psql -U postgres -d arbitragex -tAc 'SELECT 1'"          -> 1
ssh arbx "docker exec arbitragex-v2-redis-1 redis-cli XLEN arbx:opps:detected"                          -> 10002
ssh arbx "docker exec arbitragex-v2-postgres-1 psql -U postgres -d arbitragex -tAc \"SELECT COUNT(*) FROM opportunities WHERE detected_at > NOW() - INTERVAL '5 minutes'\""  -> 4432
ssh arbx "curl -s -o /dev/null -w '%{http_code}' 'http://127.0.0.1:8788/api/v1/opportunities/live?limit=1'"  -> 404
ssh arbx "curl -s -o /dev/null -w '%{http_code}' 'http://127.0.0.1:8080/api/v1/opportunities/live?limit=1'"  -> 200
ssh arbx "curl -s -o /dev/null -w '%{http_code}' 'http://127.0.0.1:8788/api/status'"                        -> 200
ssh arbx "curl -sf -o /dev/null -w '%{http_code}' 'http://127.0.0.1:8080/socket.io/?EIO=4&transport=polling'" -> 200
ssh arbx "docker ps --filter name=arbitragex-v2 --format '{{.Names}}' | wc -l"                          -> 25

grep -c 'ssh arbx' en líneas ejecutables del archivo  -> 0
```
