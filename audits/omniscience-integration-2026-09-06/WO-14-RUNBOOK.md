# WO-14 · RUNBOOK DEL OPERADOR — D-4 (webhook 401) + token cloudflared + ALLOWED_ORIGINS

Fecha: 2026-09-06 · kind: **design** (los agentes NO ejecutan sobre el VPS: §32/§33 — este documento es la entrega).
Estado actual verificado read-only en VPS: **2026-09-07T00:21Z** (VPS en UTC). Diseño con diffs exactos: `WO-14-DESIGN.md` (mismo directorio).

**Alcance y límites**: configuración de observabilidad/perímetro únicamente. CERO executor, CERO wallets, CERO broadcast, CERO flips de modo (§32/§33/§34.3 intocables). Ningún valor secreto se escribe en este archivo ni debe pegarse en chat/terminal.

---

## §0 · Estado actual medido (evidencia viva, lectura read-only)

| Hecho | Evidencia | Dónde |
|---|---|---|
| Canal de alertas MUERTO: AM notifica cada ~5 min y muere en 401 | 6× `unexpected status code 401: http://api-server:8080/admin/alertmanager/webhook {"error":"unauthorized","source":"admin_token"}` entre 00:01:46Z y 00:21:46Z (`caller=dispatch.go:353`), con `num_alerts=5` en la mayoría (5 `RpcCircuitBreakerOpen` activas sin entrega) | `docker logs arbitragex-v2-alertmanager-1 --since 3h` (2026-09-07T00:21Z) |
| api-server registra el 401 del lado servidor | `POST /admin/alertmanager/webhook statusCode:401 responseTime:0-2ms` cada ciclo | `docker logs arbitragex-v2-api-server-1 --since 3h` |
| AM NO tiene paso de render ni token: env = solo `PATH`, entrypoint default `["/bin/alertmanager"]` | `docker inspect arbitragex-v2-alertmanager-1` | VPS |
| El receiver rutea TODO sin header de auth | `monitoring/alertmanager/alertmanager.yml:44-48` | repo (md5 verificado == VPS == contenedor: `08-monitoring-fleet-CROSS.md` C-0) |
| El gate exige SOLO el header `x-arbx-admin-token` (no parsea `Authorization`) | `shared-ts/src/middleware/index.ts:124-130` (`req.header("x-arbx-admin-token")` + `safeTokenEqual`); contrato 401 probado en `backend/api-server/src/routes/stubs.test.ts:102` | repo |
| Token cloudflared en claro en `argv[]` del unit, world-readable | `systemctl show cloudflared -p ExecStart` → `--no-autoupdate tunnel run --token <REDACTED>`; unit `644 root:root`; `/etc/cloudflared` NO existe; servicio `active` | VPS (no reproducimos el valor; hallazgo previo `03-api-ws.md` §6 / `08-monitoring-fleet.md` R7) |
| edge SIN `ALLOWED_ORIGINS` ni `EDGE_AUDIT_TOKEN` en env | `docker inspect arbitragex-v2-edge-1` → nombres de env: ausentes ambas claves | VPS |
| CORS refleja vacío siempre; bucket auditor #539 inerte (fail-closed) | `edge/worker/src/index.ts:290-299` (allowlist CSV vacía) e `index.ts:36,378-395` (token unset ⇒ header ignorado); prod sirve el worker vía `edge/worker/Dockerfile.node` → `node-server.ts` (env desde process.env) | repo |
| api-server WS lee la MISMA variable (superficie 2) | `backend/api-server/src/websocket.ts:245-251` // WO-14 (2026-09-06) línea corregida por drift (fn `parseAllowedOrigins`) | repo |
| Stack vivo = `compose.prod.yml` | label `com.docker.compose.project.config_files` = `/opt/arbitragex-v2/docker/compose.prod.yml` | VPS |

**Corrección al charter (importante)**: el charter sugería `http_config.authorization` (Bearer). El gate canónico NO lee `Authorization` — solo `x-arbx-admin-token`. Bearer seguiría en 401. El fix correcto es **`http_config.headers`** con ese header exacto (diff en §3.1). Validado contra código, no contra suposición.

---

## §1 · Matriz ítem → vía de aplicación

| # | Anomalía (ID) | Vía | Archivos repo (PR) | Acción directa VPS |
|---|---|---|---|---|
| 1 | Webhook 401 cada 5 min (**D-4**) | **PR de repo + deploy** (§37: un PR = un ID) | `monitoring/alertmanager/alertmanager.yml` + `docker/compose.prod.yml` (bloque alertmanager) + bloque gemelo en `docker/compose.dev.yml` (paridad, mismo PR) | Ninguna nueva clave `.env` (reusa `ARBX_ADMIN_TOKEN` ya requerido) |
| 2 | Token cloudflared expuesto (`systemctl cat`) | **ACCIÓN DIRECTA DEL OPERADOR en VPS** (sin repo) | — | `/etc/cloudflared/tunnel-env` (0600) + drop-in systemd `override.conf` |
| 3 | `ALLOWED_ORIGINS` vacío + `EDGE_AUDIT_TOKEN` ausente (**D-2 / completa #539**) | **PR de repo (segundo, aparte)** + `.env` VPS primero | `docker/compose.prod.yml` (bloque edge) + gemelo en `docker/compose.dev.yml` + `.env.example` | `.env`: 2 claves + `up -d edge api-server` |
| 4 | Verificación post-aplicación | Este runbook §6 | — | — |

---

## §2 · FASE 0 (VPS, primero — desbloquea los guards `:?` de los PRs)

Duración ~2 min. Sin downtime.

```bash
ssh arbx
cd /opt/arbitragex-v2
cp .env .env.backup.$(date -u +%Y%m%dT%H%M%SZ)    # rollback de esta fase

# 1) ALLOWED_ORIGINS — origen público canónico (NO es secreto; ya público en el repo docs)
grep -q '^ALLOWED_ORIGINS=' .env || echo 'ALLOWED_ORIGINS=https://arbx.ape-tv.net' >> .env

# 2) EDGE_AUDIT_TOKEN — secreto NUEVO del bucket auditor (#539). Generar SIN imprimirlo:
grep -q '^EDGE_AUDIT_TOKEN=' .env || { U=$(openssl rand -hex 32); printf 'EDGE_AUDIT_TOKEN=%s\n' "$U" >> .env; unset U; }

# 3) Sanity sin exponer valores:
grep -c '^ALLOWED_ORIGINS=https://arbx.ape-tv.net$' .env   # → 1
grep -c '^EDGE_AUDIT_TOKEN=[0-9a-f]\{64\}$' .env           # → 1

# 4) Validar interpolación de compose SIN imprimir secretos:
docker compose --env-file .env -f docker/compose.prod.yml config --quiet && echo "COMPOSE OK"
```

> NUNCA correr `docker compose config` sin `--quiet` en una terminal compartida: interpola valores reales de `.env`.
> Distribución de `EDGE_AUDIT_TOKEN`: el mismo valor debe configurarse en el harness del barrido (Holy Grail, lado local del operador) como header `x-arbx-audit-token`. Sin eso, el bucket queda armado pero sin consumidor.

---

## §3 · FASE 1 — Ítem 1: D-4, header de admin en el receiver `default`

### 3.1 Diffs exactos (PR-1 · ID D-4 · archivos bajo claim WO-14)

**`monitoring/alertmanager/alertmanager.yml`** — agregar tras la línea 17 (`# DO NOT deploy…`):

```diff
 # DO NOT deploy this file to production without the render step.
 # Secrets classification: docs/operations/SECRETS_POLICY.md (T1 tier).
+
+# WO-14 (2026-09-06) D-4: the `default` receiver below ALSO carries a template
+# placeholder — __ARBX_ADMIN_TOKEN__ (dunder, sed-safe) — rendered by the
+# alertmanager entrypoint defined in docker/compose.prod.yml (busybox sed over
+# this template) from the container env. Interim channel: compose `environment`
+# reuses ARBX_ADMIN_TOKEN from .env, until vault-agent renders
+# /run/secrets/arbx.env + envsubst (then switch this placeholder to
+# ${ARBX_ADMIN_TOKEN} and drop the sed step — the channel documented above,
+# L5-11, absorbs it without further changes here).
```

Y en el receiver (L44-48):

```diff
 receivers:
   - name: default
     webhook_configs:
+      # WO-14 (2026-09-06) D-4: the api-server gate (requireAdminToken,
+      # shared-ts/src/middleware/index.ts:124-130) reads ONLY the
+      # x-arbx-admin-token header — every 5:00-min notify died 401
+      # (audits/omniscience-integration-2026-09-06/03-api-ws.md §5; still live
+      # 2026-09-07T00:21Z). `headers` (NOT `authorization`: the gate does not
+      # parse Authorization/Bearer — a Bearer token would keep 401-ing).
       - url: "http://api-server:8080/admin/alertmanager/webhook"
         send_resolved: true
+        http_config:
+          headers:
+            x-arbx-admin-token: "__ARBX_ADMIN_TOKEN__"
```

**`docker/compose.prod.yml`** — servicio `alertmanager` (hoy L556-572; // WO-14 (2026-09-06) +4 por comentario WO-13 en L406-409):

```diff
   alertmanager:
     logging: *id001
     image: prom/alertmanager:v0.27.0
+    # WO-14 (2026-09-06) D-4: render step for the __ARBX_ADMIN_TOKEN__ template
+    # placeholder in alertmanager.yml (alertmanager does NOT expand env vars —
+    # see that file's header L1-18). busybox sh validates the token charset
+    # (sed-safe: [A-Za-z0-9_-]) and sed renders to /tmp before exec. Token
+    # channel: compose environment <- .env ARBX_ADMIN_TOKEN (interim until
+    # vault-agent; the same value api-server already requires, compose L351).
+    # No image rebuild — stock prom/alertmanager + busybox.
+    environment:
+      ARBX_ADMIN_TOKEN: ${ARBX_ADMIN_TOKEN:?ARBX_ADMIN_TOKEN required}
+    entrypoint:
+    - /bin/sh
+    - -c
+    - >-
+      case "$$ARBX_ADMIN_TOKEN" in ""|*[!A-Za-z0-9_-]*) echo "ARBX_ADMIN_TOKEN missing or sed-unsafe" >&2; exit 1;; esac;
+      sed "s|__ARBX_ADMIN_TOKEN__|$$ARBX_ADMIN_TOKEN|g" /etc/alertmanager/alertmanager.yml > /tmp/alertmanager.rendered.yml;
+      exec /bin/alertmanager --config.file=/tmp/alertmanager.rendered.yml --storage.path=/alertmanager
     ports:
     - 127.0.0.1:9093:9093
     volumes:
     - ../monitoring/alertmanager/alertmanager.yml:/etc/alertmanager/alertmanager.yml:ro
```

Notas de escape (por si se edita a mano): en compose, `$$` = `$` literal que el shell del contenedor expande; el placeholder dunder evita toda colisión con la interpolación de compose. El bloque gemelo va en `docker/compose.dev.yml:408-412` (monta el mismo `alertmanager.yml` — sin él, dev sigue en 401). Diff verbatim del gemelo — con la convención dev `:-` (compose.dev.yml:269) en vez del `:?` prod: `WO-14-DESIGN.md` §2.3. // WO-14 (2026-09-06)

### 3.2 Pre-flight (VPS, después de `git pull`, ANTES de `up -d`) — sin exponer el token

```bash
cd /opt/arbitragex-v2
docker compose --env-file .env -f docker/compose.prod.yml run --rm --no-deps --entrypoint /bin/sh alertmanager -c '
  sed "s|__ARBX_ADMIN_TOKEN__|$ARBX_ADMIN_TOKEN|g" /etc/alertmanager/alertmanager.yml > /tmp/r.yml
  echo "placeholders_sin_consumir=$(grep -c "__ARBX_ADMIN_TOKEN__" /tmp/r.yml)"
  echo "header_presente=$(grep -c "x-arbx-admin-token:" /tmp/r.yml)"
  amtool check-config /tmp/r.yml && echo "AMTOOL OK"'
```

Esperado: `placeholders_sin_consumir=0`, `header_presente=1`, `AMTOOL OK` (amtool valida estrictamente — si `http_config.headers` no existiera en v0.27.0, falla AQUÍ y no en producción; no continué sin esto).

### 3.3 Aplicación (solo alertmanager; sin rebuild de imágenes)

```bash
docker compose --env-file .env -f docker/compose.prod.yml up -d --no-deps alertmanager
docker inspect arbitragex-v2-alertmanager-1 --format 'EP={{json .Config.Entrypoint}}' | head -c 120; echo
docker logs arbitragex-v2-alertmanager-1 --since 2m 2>&1 | tail -5    # arranque limpio, sin "error loading config"
```

### 3.4 Verificación D-4 (gate de aceptación — ver tabla §6)

```bash
# (a) render consumió el placeholder dentro del contenedor vivo:
docker exec arbitragex-v2-alertmanager-1 sh -c 'grep -c "__ARBX_ADMIN_TOKEN__" /tmp/alertmanager.rendered.yml'   # → 0

# (b) alerta sintética END-TO-END (la emite el OPERADOR — es la única mutación de prueba de este runbook):
STARTS=$(date -u +%Y-%m-%dT%H:%M:%SZ); ENDS=$(date -u -d '+3 minutes' +%Y-%m-%dT%H:%M:%SZ)
curl -s -o /dev/null -w 'am_api_status=%{http_code}\n' -XPOST http://127.0.0.1:9093/api/v2/alerts \
  -H 'content-type: application/json' \
  -d "[{\"labels\":{\"alertname\":\"WO14SyntheticProbe\",\"severity\":\"info\",\"service\":\"runbook-wo14\"},\"annotations\":{\"summary\":\"WO-14 end-to-end probe - safe to ignore\"},\"startsAt\":\"$STARTS\",\"endsAt\":\"$ENDS\"}]"
# → am_api_status=200 ; el notify sale tras group_wait=10s (alertmanager.yml:25)

sleep 30
docker logs arbitragex-v2-api-server-1 --since 2m 2>&1 | grep 'alertmanager_webhook.received'
# → {"event":"alertmanager_webhook.received","count":1,"persisted":1,...}

docker exec arbitragex-v2-postgres-1 psql -U postgres -d arbitragex -t -c \
  "SELECT action||' | '||target_id||' | '||created_at FROM audit_log WHERE actor='alertmanager' ORDER BY created_at DESC LIMIT 4;"
# → alert.firing | WO14SyntheticProbe | <ts>   (columnas: database/migrations/011_audit_log.sql:4-16)
#   y ≤10 min después: alert.resolved | WO14SyntheticProbe | <ts>   (send_resolved: true)

# (c) 0× 401 sostenido — esperar ≥16 min (≥3 ciclos de 5:00) y recién entonces:
docker logs arbitragex-v2-alertmanager-1 --since 20m 2>&1 | grep -c 'status code 401'      # → 0
docker logs arbitragex-v2-api-server-1 --since 20m 2>&1 | grep 'alertmanager/webhook' | grep -c '"statusCode":401'   # → 0
```

### 3.5 Rollback D-4

```bash
# canónico (deploy veraz): revert del PR-1 + redeploy
git revert <sha_PR-1> && git push origin main        # la cadena de deploy reconstruye
docker compose --env-file .env -f docker/compose.prod.yml up -d --no-deps alertmanager
docker logs arbitragex-v2-alertmanager-1 --since 3m 2>&1 | tail -3    # vuelve al estado conocido (401 c/5min, preexistente)
```

Troubleshooting: si persiste 401 → el valor renderizado difiere del gate: verificar (sin imprimirlo) `docker exec arbitragex-v2-alertmanager-1 sh -c 'wc -c <<<"$ARBX_ADMIN_TOKEN"'` vs `.env`. Si AM no arranca → `amtool check-config` del pre-flight marcó algo que se saltó; el healthcheck (`wget /-/healthy`, compose L563-569 // WO-14 (2026-09-06) drift +4) pondría el contenedor unhealthy sin afectar al resto del stack.

---

## §4 · FASE 2 — Ítem 2: token cloudflared fuera del unit file (acción directa VPS)

Patrón elegido: **EnvironmentFile 0600 + `TUNNEL_TOKEN` como env** (nada de token en `argv[]`; `systemctl cat/show` y `ps` quedan limpios; `/proc/<pid>/environ` es 0400 owner-root). La alternativa "usar `systemctl show` en vez de `cat`" solo evita imprimirlo de casualidad — no arregla la exposición del unit 644. Ventana: ~20-40 s de interrupción del túnel público.

```bash
ssh arbx

# 1) Capturar el token ACTUAL sin imprimirlo jamás en pantalla (sale del propio unit):
sudo install -d -m 0755 /etc/cloudflared
sudo sh -c 'umask 077; systemctl cat cloudflared.service | tr " =" "\n\n" | grep "^ey[A-Za-z0-9_.-]*$" | head -1 | sed "s/^/TUNNEL_TOKEN=/" > /etc/cloudflared/tunnel-env'
sudo chmod 0600 /etc/cloudflared/tunnel-env && sudo chown root:root /etc/cloudflared/tunnel-env
# sanity SIN exponer el valor:
sudo wc -c /etc/cloudflared/tunnel-env          # → >100 bytes
sudo head -c 13 /etc/cloudflared/tunnel-env     # → imprime solo el prefijo "TUNNEL_TOKEN=" (seguro)

# 2) Drop-in del unit (systemctl edit crea /etc/systemd/system/cloudflared.service.d/override.conf):
sudo systemctl edit cloudflared
```

Contenido del override (Nota: `ExecStart=` vacío RESETEA el original; la línea nueva es el argv actual SIN `--token`):

```ini
[Service]
EnvironmentFile=/etc/cloudflared/tunnel-env
ExecStart=
ExecStart=/usr/bin/cloudflared --no-autoupdate tunnel run
```

```bash
# 3) Aplicar y observar:
sudo systemctl daemon-reload
sudo systemctl restart cloudflared
systemctl is-active cloudflared                                                  # → active
journalctl -u cloudflared --since '2 min ago' --no-pager | tail -15              # → 4x "Registered tunnel connection", sin errores de credenciales

# 4) Higiene (esperado: 0 en los tres primeros; 1 en el último — solo root):
systemctl cat cloudflared | grep -c 'ey[A-Za-z0-9]'                              # → 0
systemctl show cloudflared -p ExecStart | grep -c 'ey[A-Za-z0-9]'                # → 0
ps -eo args | grep '[c]loudflared' | grep -c 'ey[A-Za-z0-9]'                     # → 0
sudo tr '\0' '\n' < /proc/$(pidof cloudflared)/environ | grep -c '^TUNNEL_TOKEN=' # → 1

# 5) Dominio público vivo (1 request — presupuesto):
curl -sI -o /dev/null -w 'public_status=%{http_code}\n' https://arbx.ape-tv.net/  # → 200 (o redirect de Next)
```

**Fallback documentado**: si esa versión de cloudflared ignorara `TUNNEL_TOKEN` (journal mostraría fallo de credenciales y el paso 5 caería), usar en el override `ExecStart=/usr/bin/cloudflared --no-autoupdate tunnel run --token ${TUNNEL_TOKEN}` — systemd expande `${TUNNEL_TOKEN}` desde el EnvironmentFile dentro de ExecStart; `systemctl cat/show` NO muestra el valor expandido (queda residual solo en `/proc/<pid>/cmdline`, legible por usuarios locales hasta actualizar cloudflared y volver al env puro).

**Cierre real de exposición (recomendado, luego del paso 4 verde)**: el token fue world-readable durante toda la vida del unit (644). Rotarlo en Cloudflare Zero Trust (Networks → Tunnels → tunnel → rotar token) y actualizar `/etc/cloudflared/tunnel-env` con el nuevo valor. Sin esto, el hardening solo cierra la ventana futura.

**Rollback**: `sudo rm -f /etc/systemd/system/cloudflared.service.d/override.conf && sudo systemctl daemon-reload && sudo systemctl restart cloudflared` — el unit original (token inline) vuelve a asumir; el túnel no cambia de identidad.

---

## §5 · FASE 3 — Ítem 3: `ALLOWED_ORIGINS` + `EDGE_AUDIT_TOKEN` en el edge (D-2 / #539)

### 5.1 Alivio inmediato (sin esperar PR — `.env` ya está listo de FASE 0)

`edge` ya monta `env_file: ../.env` (compose.prod.yml:415-416 // WO-14 (2026-09-06) drift +4) — con las claves de FASE 0 basta recrear. `api-server` lee la misma variable para el handshake WS (websocket.ts:245-251 // WO-14 (2026-09-06)) vía su propio `env_file` (L338-339) — recrear ambos en la misma ventana:

```bash
cd /opt/arbitragex-v2
docker compose --env-file .env -f docker/compose.prod.yml up -d edge api-server
```

Sin rebuild: son env de RUNTIME del contenedor edge/api-server — NO `NEXT_PUBLIC_*`, RULE 03 no aplica, el frontend no se toca.

### 5.2 PR-2 (ID D-2 / completa #539) — diff exacto, bloque `environment` del servicio `edge` (compose.prod.yml:417-421 // WO-14 (2026-09-06) drift +4)

```diff
     environment:
       API_SERVER_URL: http://api-server:8080
       ARBX_EDGE_TOKEN: ${ARBX_EDGE_TOKEN:?ARBX_EDGE_TOKEN required}
       EDGE_PORT: '8787'
       REDIS_URL: ${REDIS_URL:-redis://redis:6379}
+      # WO-14 (2026-09-06) D-2/#539: public-origin CORS allowlist + auditor
+      # bucket secret. Both were UNSET in prod (docker inspect 2026-09-07T00Z)
+      # -> empty CORS reflector (edge/worker/src/index.ts:290-299) and #539's
+      # auditor bucket inert fail-closed (index.ts:36,378-395). Values live
+      # ONLY in VPS .env (env_file ../.env already loads them); `:?` makes a
+      # future deploy without them FAIL FAST instead of silently regressing
+      # to an empty allowlist.
+      ALLOWED_ORIGINS: ${ALLOWED_ORIGINS:?ALLOWED_ORIGINS required (public origin)}
+      EDGE_AUDIT_TOKEN: ${EDGE_AUDIT_TOKEN:?EDGE_AUDIT_TOKEN required (auditor bucket secret)}
```

Mismo bloque gemelo en `docker/compose.dev.yml` (servicio edge) y documentación de ambas claves en `.env.example` (`ALLOWED_ORIGINS=https://arbx.ape-tv.net`, `EDGE_AUDIT_TOKEN=`) — este archivo tracked lleva el dominio público, jamás el secreto. Diffs verbatim: `WO-14-DESIGN.md` §3.2 (gemelo dev — defaults vacíos, fail-closed idéntico a hoy) y §3.3 (`.env.example`, sección Auth/Admin tokens). // WO-14 (2026-09-06)

### 5.3 Verificación ítem 3

```bash
docker exec arbitragex-v2-edge-1 sh -c 'printf "%s\n" "$ALLOWED_ORIGINS"'     # → https://arbx.ape-tv.net

# CORS refleja SOLO el origen legítimo, con credenciales (V-AT-1) — loopback, 0 requests públicos:
curl -s -D- -o /dev/null -H 'Origin: https://arbx.ape-tv.net' http://127.0.0.1:8787/health | grep -i 'access-control-allow-origin\|access-control-allow-credentials'
# → access-control-allow-origin: https://arbx.ape-tv.net
#   access-control-allow-credentials: true
curl -s -D- -o /dev/null -H 'Origin: https://evil.example' http://127.0.0.1:8787/health | grep -ci 'access-control-allow-origin'
# → 0   (origen ajeno sigue SIN reflejo — fail-closed intacto)

# Bucket auditor (#539) armado — el header con token correcto cae al bucket rl_audit:
curl -s -D- -o /dev/null -H "x-arbx-audit-token: $(grep '^EDGE_AUDIT_TOKEN=' /opt/arbitragex-v2/.env | cut -d= -f2)" http://127.0.0.1:8787/health | grep -i 'x-ratelimit-remaining'
# → x-ratelimit-remaining: <número>   (sin el header → bucket público "rl": comportamiento de hoy sin cambio)
```

### 5.4 Rollback ítem 3

```bash
cd /opt/arbitragex-v2
sed -i '/^ALLOWED_ORIGINS=/d; /^EDGE_AUDIT_TOKEN=/d' .env     # o restaurar .env.backup.* de FASE 0
docker compose --env-file .env -f docker/compose.prod.yml up -d edge api-server
# → estado fail-closed conocido: CORS vacío + bucket auditor inerte. Sin riesgo de datos.
# (tras mergear PR-2, el rollback además requiere revert del PR — el `:?` exigiría las claves)
```

---

## §6 · Tabla de verificación post-aplicación (consolidada)

| Ítem | Qué leer | Resultado esperado | Rollback |
|---|---|---|---|
| 1a Render AM | `docker exec arbitragex-v2-alertmanager-1 sh -c 'grep -c "__ARBX_ADMIN_TOKEN__" /tmp/alertmanager.rendered.yml'` | `0` (placeholder consumido) y `grep -c "x-arbx-admin-token:"` → `1` | §3.5 |
| 1b Entrega end-to-end | `audit_log` (psql §3.4b) + `alertmanager_webhook.received` en logs api-server | `alert.firing`+`alert.resolved` de `WO14SyntheticProbe`, actor=`alertmanager`, `persisted:1` | §3.5 |
| 1c Cese del 401 | `docker logs arbitragex-v2-alertmanager-1 --since 20m \| grep -c 'status code 401'` (ventana ≥16 min tras el fix) | `0` — y 0 líneas `"statusCode":401` en api-server para `/admin/alertmanager/webhook` | §3.5 |
| 2a Higiene unit | `systemctl cat cloudflared \| grep -c 'ey[A-Za-z0-9]'` y `systemctl show cloudflared -p ExecStart` y `ps -eo args \| grep '[c]loudflared'` | `0` en los tres | §4 rollback |
| 2b Túnel vivo | `systemctl is-active cloudflared` + `journalctl -u cloudflared --since '5 min'` + 1× `curl -sI https://arbx.ape-tv.net/` | `active`, 4× "Registered tunnel connection", público `200` | §4 rollback |
| 3a CORS legítimo | curl loopback con `Origin: https://arbx.ape-tv.net` (§5.3) | ACAO reflejado + `allow-credentials: true` | §5.4 |
| 3b CORS hostil | curl loopback con `Origin: https://evil.example` | `0` headers CORS (fail-closed) | §5.4 |
| 3c Bucket auditor | curl loopback con `x-arbx-audit-token` (§5.3) | `x-ratelimit-remaining` numérico (bucket `rl_audit`), público intacto | §5.4 |
| Global | `docker ps` salud de flota + readiness del dominio público | Sin containers nuevos en restart-loop; `/-/healthy` AM ok (healthcheck compose L563-569 // WO-14 (2026-09-06) drift +4) | — |

---

## §7 · Invariantes y gate (resumen ejecutivo — detalle en WO-14-DESIGN.md)

- **Inv-1 (D-4)**: para toda ventana ≥16 min post-fix, `grep -c 'status code 401'` en logs AM = 0 ∧ ∃ fila `audit_log(actor='alertmanager', target_id='WO14SyntheticProbe')` con `alert.firing` y `alert.resolved`.
- **Inv-2 (cloudflared)**: toda lectura pública del unit (`cat`/`show`) y `ps` exponen 0 substrings tipo token ∧ servicio `active` ∧ dominio público responde.
- **Inv-3 (CORS/bucket)**: ACAO se emite ⇔ `Origin ∈ {https://arbx.ape-tv.net}`; credenciales solo con reflexión; bucket auditor activo solo con token válido, bucket público invariante.
- **Gate**: PR-1 (D-4) y PR-2 (D-2/#539) — cada PR un solo ID (§37 P-∅), CI verde, pre-flight `amtool check-config` PASS antes de `up -d`, deploy veraz (SHA anclado), verificación §6 completa. Ítem cloudflared = gate propio del operador (§4 pasos 3-5).
- **Intocable**: §34.3 default-deny/`MainnetRefused`, gates `arbx-*`, kill-switch. Este runbook no altera matemática ni terminus de ejecución — solo el canal de notificación, higiene de secretos y perímetro CORS.
- **RULE 00**: cero valores fabricados — tokens/origen viven solo en `.env` VPS; el repo lleva placeholders (`__ARBX_ADMIN_TOKEN__`, `${VAR:?}`), nunca literales.

*WO-14 · devops-platform · Gang Omniscience · 2026-09-06 — design read-only; ejecución 100% del operador.*
