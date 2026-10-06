# PIPELINE-INTEGRITY-REPAIR-01 — el vigía era mudo; ahora dictamina

**Tarea:** t64 · **Rama:** `ci/pipeline-integrity-repair-01` · **Base:** `origin/main` = `c89d21a3437c7fc2456fb7a08dba7d26ef41ee5e`
**Alcance:** `.github/workflows/`, `docs/release/` (nada fuera de alcance fue tocado).
**Commits:** `9d3cbffc` → `abb71a47` → `3f4eb35` (3 commits, **1 archivo**).
**NO merge.** **NO** firma, **NO** broadcast, **NO** deploy.

---

## 1. El defecto: un vigía que no podía medir (y por eso no podía fallar con razón)

Archivo original `origin/main:.github/workflows/pipeline-integrity.yml` — **4801 bytes**, sha256
`02fcf2bf8d6d8a28…`, **112 líneas**:

| Línea | Contenido | Defecto |
|---|---|---|
| 41, 52, 95 | `ssh arbx "…"` | `arbx` es un alias de `~/.ssh/config` de la laptop del operador. En un runner de GitHub **no existe**, y tampoco había llave materializada. |
| 19 | `default: 'localhost'` | El input libre `vps_host` caía a `localhost`. |
| 34 | `VPS_HOST: ${{ inputs.vps_host \|\| 'localhost' }}` | Propaga ese `localhost` a los pasos. |
| 65 | `curl -sf "http://${VPS_HOST}:8787/api/v1/opportunities/live?limit=1"` | Mediría el **localhost del runner**, no el VPS. |
| 86 | `curl … "http://${VPS_HOST}:8080/socket.io/?EIO=4&transport=polling"` | Idem. |

Evidencia verbatim del primer paso muriendo, run **37428952107** (evento `schedule`,
`2026-10-06T07:19:32Z`, head `c89d21a3` **= el SHA desplegado**):

```
Verify E2E no-loss pipeline	UNKNOWN STEP	2026-10-06T07:19:36.3700066Z ssh: Could not resolve hostname arbx: Temporary failure in name resolution
Verify E2E no-loss pipeline	UNKNOWN STEP	2026-10-06T07:19:36.3722437Z ##[error]Process completed with exit code 255.
```

Conclusión por step de ese mismo run (`GET /repos/hefarica/arbitragex-v2/actions/runs/37428952107/jobs`):

```
2 failure :: Check Redis stream has recent detections      <-- murió 4 s después de empezar
3 skipped :: Check PG has opportunities in last 5 minutes
4 skipped :: Check API serves opportunities with correct types
5 skipped :: Check WS endpoint is alive
6 skipped :: Check Docker containers are healthy
7 success :: Summary
```

Es decir: **ninguna capa del pipeline fue jamás medida.** Ventana de los últimos 12 runs del
workflow (`gh run list --workflow pipeline-integrity.yml --limit 12`): **11 en
`completed/failure`, 1 encolado (el de esta reparación), 0 en `success`**.

---

## 2. La reparación, contrastada con el mecanismo que YA funcionaba

`auto-deploy-vps.yml` es el workflow que sí llega al VPS. Sus líneas **58-71** son el patrón
probado, citadas verbatim de `origin/main`:

```yaml
      - name: Setup SSH                                          # L58
        env:                                                     # L59
          VPS_KEY: ${{ secrets.VPS_SSH_KEY }}                    # L60
          VPS_HOST: ${{ secrets.VPS_SSH_HOST }}                  # L61
          VPS_PORT: ${{ secrets.VPS_PORT || '22' }}              # L62
        run: |                                                   # L63
          set -euo pipefail                                      # L64
          test -n "$VPS_KEY" && test -n "$VPS_HOST"              # L65
          [[ "$VPS_PORT" =~ ^[0-9]+$ ]] && (( VPS_PORT > 0 && VPS_PORT < 65536 ))  # L66
          install -m 700 -d ~/.ssh                               # L67
          umask 077                                              # L68
          printf '%s\n' "$VPS_KEY" > ~/.ssh/deploy_key           # L69
          ssh-keyscan -T 10 -p "$VPS_PORT" -H "$VPS_HOST" > ~/.ssh/known_hosts    # L70
          test -s ~/.ssh/known_hosts                             # L71
```

El archivo reparado (16100 bytes, sha256 `437a6cf5826773c6b480217d491e3ecc26a23e4535f2225224a34840777cee95`)
materializa **el mismo mecanismo, línea por línea**, en su propio primer paso:

| Reparado | auto-deploy-vps.yml |
|---|---|
| L75 `VPS_KEY: ${{ secrets.VPS_SSH_KEY }}` | L60 |
| L81 `test -n "$VPS_KEY" && test -n "$VPS_HOST"` | L65 |
| L85 `printf '%s\n' "$VPS_KEY" > ~/.ssh/deploy_key` | L69 |
| L86 `ssh-keyscan -T 10 -p "$VPS_PORT" -H "$VPS_HOST" > ~/.ssh/known_hosts` | L70 |

Las **6** sondas (`preflight` L96; `redis_stream` L116; `pg_flow` L145; `api_types` L178;
`ws_alive` L246; `docker_health` L276) corren **en el VPS**, por SSH, con
`-o BatchMode=yes -o StrictHostKeyChecking=yes` y `'bash -s' <<'REMOTE'`:

```
OUT=$(ssh -i ~/.ssh/deploy_key -p "$VPS_PORT" \
        -o ConnectTimeout=15 -o BatchMode=yes -o StrictHostKeyChecking=yes \
        "$VPS_USER@$VPS_HOST" 'bash -s' <<'REMOTE' 2>&1
```

Y se **eliminó** el input libre `vps_host` que defaultaba a `localhost` (además era superficie de
inyección: un input de workflow que aterrizaba dentro de un comando de shell).

**Sonda de conservación** (validador sobre el YAML parseado, salida real):
`ssh arbx` en bloques `run:` = **0** · `localhost` en bloques `run:` = **0** ·
`ssh -i ~/.ssh/deploy_key` = **6** · comandos mutantes en `run:`
(`docker restart|stop|rm|down`, `systemctl restart|stop|start`, `rm -`, `truncate`, `UPDATE`,
`DELETE`, `DROP`) = **0** · `yaml.safe_load` OK, 8 steps.

> Las 4 sondas restantes son **lecturas**: `docker exec … redis-cli XLEN` (L119),
> `psql -t -A -c "SELECT COUNT(*) …"` (L148), `curl -s` GET (L249, L279-280 `docker ps`). La sonda
> WS es un GET al endpoint de *polling* de engine.io (L249-250): no emite órdenes ni toca ledger,
> config ni estado del producto.

---

## 3. Los DOS rojos, ahora distinguidos (criterio 3)

| Modo | Vocabulario en el log | Qué significa | ¿Es un veredicto sobre el pipeline? |
|---|---|---|---|
| **CONNECTION** | `VERDICT=NO_COMPUTADO reason=connection (ssh rc=…)` + `::error::CONNECTION failure` | El instrumento **no pudo llegar** al host. | **NO.** No se midió nada. |
| **PIPELINE** | `VERDICT=PASS\|FAIL layer=<capa> reason=…` + `::error::PIPELINE_VERDICT=FAIL layer=<capa>` | El instrumento **sí midió** y la capa <capa> está rota. | **SÍ**, y nombra la capa. |

El `preflight` (L101-108) es el discriminador: si el SSH falla, imprime el primer modo y **corta
ahí**, declarando explícitamente en el log *"This says NOTHING about the pipeline; it is not a
pipeline verdict."* El step `Verdict summary` (`if: always()`, L302-312) imprime el `outcome` de
las 6 capas, de modo que un rojo nunca queda sin capa asignada.

**Por qué leer los `failure` consecutivos como "el pipeline está roto" es fallo de disciplina
evidencial (d):** los 11 runs anteriores a esta reparación concluyeron `failure` por la misma
causa de la L41 del archivo: el nombre `arbx` no resuelve en el runner. Ese `failure` es del
**chequeo**, no del **chequeado**. Un `failure` sin `VERDICT=` no llevaba forma de veredicto: era
una etiqueta (`failure`) leída como si fuera otra cosa (el estado del pipeline). Los 6 runs que
la orden cita son ese caso, y su conclusión correcta es **NO COMPUTADO**, no **ROTO**.

---

## 4. Un run REAL llega al VPS y dictamina (criterio 2)

**Run 1 — `37462970330`** (`workflow_dispatch`, `2026-10-06T12:24:03Z`, head `9d3cbffc`, ref
`ci/pipeline-integrity-repair-01`), step conclusions:

```
2 success :: Materialize SSH access (same mechanism as auto-deploy-vps.yml)
3 success :: Preflight — CONNECTION vs PIPELINE (the two red modes)
4 success :: Check Redis stream has recent detections
5 success :: Check PG has opportunities in last 5 minutes
6 failure :: Check API serves opportunities with correct types
7 skipped :: Check WS endpoint is alive
8 skipped :: Check Docker containers are healthy
9 success :: Verdict summary (pipeline, not connection)
```

Log: `VERDICT=PASS layer=connection (the host answered; the layers below will be judged next)` ·
`VERDICT=PASS layer=redis_stream` · `VERDICT=PASS layer=pg_flow`.
**El primer step ya no falla y el instrumento llega al VPS.**

**Run 2 — `37463450394`** (`workflow_dispatch`, `2026-10-06T12:28:10Z`, head `abb71a47`):
los 6 pasos se ejecutaron (el `if: always()` que agregué hace que un rojo no oculte las otras
capas). Veredictos **sobre el pipeline**, verbatim del log:

| Capa | Salida real | Veredicto |
|---|---|---|
| connection | `VERDICT=PASS layer=connection` @12:37:35.729 | PASS |
| redis_stream | `Redis XLEN arbx:opps:detected = 10000` @12:37:38.093 | PASS |
| pg_flow | `PG opportunities in last 5 min = 508` @12:37:40.484 | PASS |
| api_types | `VERDICT=FAIL layer=api_types reason=zero_items` @12:37:42.888 | **FAIL** |
| ws_alive | `VERDICT=PASS layer=ws_alive` @12:37:45.435 | PASS |
| docker_health | `VERDICT=PASS layer=docker_health` @12:37:47.871 | PASS |

`##[error]PIPELINE_VERDICT=FAIL layer=api_types — the API layer did not return usable opportunities`
(job `completed/failure`, y **el run no fue `skipped` en ninguna capa**).

---

## 5. El veredicto de hoy: el rojo de `api_types` NO es del pipeline (criterio 4)

La sonda imprimió su propia evidencia en el run 2:

```
probe_url=http://127.0.0.1:8080/api/opportunities/live?limit=1    http=404 bytes=160
probe_url=http://127.0.0.1:8080/api/v1/opportunities/live?limit=1 http=200 bytes=8644
probe_url=http://127.0.0.1:8787/api/opportunities/live?limit=1    http=200 bytes=8644
probe_url=http://127.0.0.1:8787/api/v1/opportunities/live?limit=1 http=404 bytes=21
answered_by=http://127.0.0.1:8080/api/v1/opportunities/live?limit=1
body_head={"count":1,"window_total":12,"window":"latest","viable_only":false,"max_age_seconds":300,"route_representative":"latest","diagnostics":{"missing_economics":…
API /opportunities/live returns 0 items
amount_in_wei type = null
```

Lectura honesta de estos bytes: la API **respondió HTTP 200 con 8644 bytes** y el cuerpo **empieza
con `{"count":1,…}`**. El paso siguiente fue `GET https://edge-arbx.ape-tv.net/api/opportunities/live?limit=1`
**desde mi estación**: `HTTP 200`, 6262 bytes, claves
`count, window_total, window, viable_only, max_age_seconds, route_representative, diagnostics, items, ts`,
`count=1 window_total=20 items=1`, y `amount_in_wei` de tipo **String**.

Por lo tanto `returns 0 items` / `amount_in_wei type = null` **no son un hecho del pipeline**: son el
resultado de mi propio parser. La línea que lo produjo era

```
COUNT=$(printf '%s' "$RESP" | jq 'if .items then (.items|length) elif .data then (.data|length) else 0 end' 2>/dev/null || echo 0)
```

es decir: un `0` que puede venir de (a) `jq` **ausente** en el host del VPS, o (b) un intérprete que
no entendió el payload. Un cero producido por un parser ausente **no es un hallazgo del pipeline**;
es exactamente el modo de fallo (d) — leer una etiqueta como si fuera otra cosa.

### 5.1 La reparación del parser (tercera iteración, commit `3f4eb35`)

El parser ahora **se nombra** y no depende de que exista un intérprete:

```
JQ=absent; command -v jq >/dev/null 2>&1 && JQ=present
KEYS=$(printf '%s' "$RESP" | grep -o '"[A-Za-z_][A-Za-z_0-9]*":' | sort -u | tr -d '":' | paste -sd, -)
COUNT=$(printf '%s' "$RESP" | grep -o '"count":[[:space:]]*[0-9][0-9]*' | head -1 | grep -o '[0-9][0-9]*$')
AS_STRING=$(printf '%s' "$RESP" | grep -c '"amount_in_wei":"')
AS_NUMBER=$(printf '%s' "$RESP" | grep -cE '"amount_in_wei":[0-9]')
echo "parser=grep jq=$JQ keys_seen=$KEYS"
echo "body_tail=$(printf '%s' "$RESP" | tail -c 300)"
```

Y el veredicto distingue tres rojos atribuibles —
`reason=count_zero_or_key_absent`, `reason=amount_in_wei_is_number`,
`reason=amount_in_wei_absent_from_payload` — de un PASS que **cita la cuenta medida**
(`VERDICT=PASS layer=api_types count=<n> amount_in_wei=string`). El `body_tail` se imprime porque
`items` vive en el **final** del objeto: el run 2 solo imprimía los primeros 300 bytes.

### 5.2 Veredicto declarado

- **Conexión, Redis, PG, WS, Docker: VERDE** (medidos en el run 2, con su cifra en el log:
  `XLEN=10000`, `508` filas en 5 min, WS 200, contenedores sin `unhealthy`).
- **api_types: NO COMPUTADO en el run 2** — el `FAIL` que emitió quedó **atribuido al parser del
  instrumento**, con la evidencia de bytes (200/8644) que lo contradice como hecho del pipeline.
- **Hallazgo de pipeline que este trabajo NO reporta** (porque sería falso): que la API sirva cero
  oportunidades. La lectura independiente del borde público dice `count=1 items=1`.

---

## 6. Límites de este artefacto

1. **La rama no es `main`.** Los runs 1-3 corren sobre `ci/pipeline-integrity-repair-01`,
   **no** sobre `main`. Esto satisface el criterio 2 en sustancia (run real, SSH real al VPS,
   veredicto real de capa) pero el instrumento **no vigila todavía lo que se despliega**: el step
   restante es el **merge**, que esta orden prohíbe.
2. **El pool de runners está saturado** (12 runs ajenos `IN_PROGRESS` y un lote de 12 workflows
   encolado a `2026-10-06T12:57:39Z` por la rama `security/kelly-guard-01`). El run 1 tardó
   ~9.5 min de cola y el run 3 seguía encolado 25+ min después de creado (`12:39:00Z`). Un run
   encolado **no es** un run exitoso ni un run fallido: es `queued`, y así se declara.
3. **`docker_health` tiene un umbral declarado** (`RUNNING -lt 10` → `PASS warning=only_N_containers`).
   Es un aviso suave, no una medida de salud por contenedor.
4. **Los `failure` con `VERDICT=` de este workflow ahora son accionables; los `failure` sin
   `VERDICT=` siguen siendo posibles** y siempre significan `NO_COMPUTADO` (esa es la regla de la
   L312, impresa en cada resumen).

---

## 7. Alcance económico y de producto (criterio 5) — nada se movió

- **No mueve P/N (0/115).** No se tocó ningún ledger, contador, oportunidad, cartucho ni tabla.
- **Sin firma, sin emisión, sin broadcast, sin deploy, sin capital.** Las 6 sondas son lecturas
  (`XLEN`, `SELECT COUNT(*)`, `curl -s` GET, `docker ps`); el validador confirma **0** comandos
  mutantes en bloques `run:`.
- **`ARBX_DRIFT_TRACKER_MODE` y cualquier otra palanca de producto quedan como están**: el diff de
  la rama contra `origin/main` es **1 archivo** (`.github/workflows/pipeline-integrity.yml`) y
  **0** líneas de `.env`, compose, backend o frontend.
- **Identidad del clon configurada antes del primer commit y verificada en el REMOTO:**
  `git config --local user.name=arbx-sre`, `user.email=sre@arbx.local`;
  `git ls-remote origin refs/heads/ci/pipeline-integrity-repair-01` =
  `3f4eb353e18d4549eb298427c07cd3db330fb276` = HEAD local (el commit existe en el remoto, no solo
  el push devolvió 0).

## 8. Reproducción

```bash
gh run list --repo hefarica/arbitragex-v2 --workflow pipeline-integrity.yml --limit 12 \
  --json databaseId,status,conclusion,event,createdAt,headSha
gh api repos/hefarica/arbitragex-v2/actions/runs/37428952107/jobs --jq '.jobs[0].steps[] | "\(.number) \(.conclusion) :: \(.name)"'
gh api repos/hefarica/arbitragex-v2/actions/runs/37463450394/jobs --jq '.jobs[0].steps[] | "\(.number) \(.conclusion) :: \(.name)"'
gh run view 37463450394 --repo hefarica/arbitragex-v2 --log | grep -E 'VERDICT=|probe_url=|answered_by='
gh run view 37428952107 --repo hefarica/arbitragex-v2 --log | grep -E 'resolve hostname|exit code'
```
