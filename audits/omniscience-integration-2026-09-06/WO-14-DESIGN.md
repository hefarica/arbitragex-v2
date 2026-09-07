# WO-14 · DESIGN COMPANION — consolidación de diffs exactos + matriz de riesgos + invariantes Inv-1/2/3 + gate

Fecha: 2026-09-06 · kind: **redesign** (cierre del entregable) · agente: devops-platform (rubric: ecc:security-review) · Gang Omniscience.
Documento operacional hermano: `WO-14-RUNBOOK.md` (mismo directorio). El runbook manda en ejecución; este companion consolida el diseño y es la referencia que las líneas 4 y 330 del runbook citan.

**Alcance y límites** (idénticos al runbook §0): configuración de observabilidad/perímetro únicamente. CERO executor, CERO wallets, CERO broadcast, CERO flips de modo (§32/§33/§34.3 intocables). Read-only: ni este documento ni ningún agente muta el VPS. Ningún valor secreto aparece aquí — solo placeholders (`__ARBX_ADMIN_TOKEN__`, `${VAR:?}`, `${VAR:-…}`, `<run: openssl rand -hex 32>`).

---

## §1 · Adjudicación de autocontención del runbook (charter, punto 1)

Verificado leyendo `WO-14-RUNBOOK.md` completo (339 líneas, 30 fences) contra los archivos reales del repo. Diffs que el charter exige dentro del runbook:

| # | Diff exigido por charter | ¿Verbatim en runbook? | Dónde | Contexto validado contra repo |
|---|---|---|---|---|
| 1 | `alertmanager.yml` `http_config.headers` | **SÍ** — diff completo (bloque comentario L17-18 + receiver L44-48) | runbook §3.1 | `monitoring/alertmanager/alertmanager.yml:17-18` y `:44-48` idénticos al contexto del diff |
| 2 | `compose.prod.yml` bloque alertmanager (render step) | **SÍ** — diff completo | runbook §3.1 | `docker/compose.prod.yml:556-562` idéntico (logging/image/ports/volumes); healthcheck `:563-569` — citas drift +4 corregidas por verificación B (§9) |
| 3 | `override.conf` cloudflared | **SÍ** — contenido ini completo + fallback + rollback | runbook §4 | n/a (acción directa VPS, sin archivo repo) |
| 4 | `compose.prod.yml` bloque edge (`ALLOWED_ORIGINS` + `EDGE_AUDIT_TOKEN`) | **SÍ** — diff completo | runbook §5.2 | `docker/compose.prod.yml:417-421` idéntico; `env_file ../.env` en `:415-416` como cita §5.1 — citas drift +4 corregidas por verificación B (§9) |
| 5 | **Gemelo dev** del bloque alertmanager | **NO verbatim** — solo puntero en prosa («El bloque gemelo va en `docker/compose.dev.yml:408-412`») | runbook §3.1 (nota final) | `docker/compose.dev.yml:408-420` leído; el bloque dev NO es copia literal del prod (sintaxis flow, sin `logging: *id001`, convención `:-` en vez de `:?`) |
| 6 | **Gemelo dev** del bloque edge | **NO verbatim** — solo puntero («Mismo bloque gemelo en `docker/compose.dev.yml` (servicio edge)») | runbook §5.2 (nota final) | `docker/compose.dev.yml:310-328` leído; el bloque dev edge tiene claves propias (`FRONTEND_URL`, `PUBLIC_EDGE_HOST`, `ARBX_EDGE_TOKEN` con default dev) |
| 7 | `.env.example` | **NO verbatim** — solo valores en prosa («`ALLOWED_ORIGINS=https://arbx.ape-tv.net`, `EDGE_AUDIT_TOKEN=`») | runbook §5.2 (nota final) | `.env.example:217-245` leído (sección Auth/Admin tokens); ninguna de las dos claves existe hoy en el archivo |

**Veredicto**: el runbook es autocontenido en los 4 diffs canónicos del charter (filas 1-4) — el operador puede ejecutar D-4 y D-2 en VPS con solo el runbook. Los tres sub-diffs de paridad (filas 5-7) estaban referenciados pero no escritos como diff exacto. **Este companion los completa leyendo los archivos reales del repo** (§2.3, §3.2, §3.3 abajo) y declara cada añadido (tabla de procedencia §8). Cero contenido nuevo inventado (RULE 00): todo lo añadido deriva de archivos reales del repo, con file:line como evidencia.

Citas del runbook verificadas contra código — **10/12 exactas; 2 con drift de LÍNEA (semántica intacta en ambas), corregidas in situ por la verificación B (§9)**: `shared-ts/src/middleware/index.ts:124-130` (el gate lee SOLO `req.header("x-arbx-admin-token")`, sin parsear `Authorization` — la «Corrección al charter» del runbook §0 es válida contra código); `docker/compose.prod.yml:351` (api-server ya exige `ARBX_ADMIN_TOKEN` con `:?`); `edge/worker/src/index.ts:34-36,288-300,376-396` (reflector CORS vacío + bucket auditor fail-closed); `backend/api-server/src/websocket.ts:245-251` (`parseAllowedOrigins` lee la MISMA variable — citado `:146-149` en la primera redacción, corregido §9/V-1); `backend/api-server/src/routes/stubs.test.ts:102-104` (contrato 401 probado). Las citas de `compose.prod.yml` bajo L406 (bloques edge/alertmanager) arrastraban +4 líneas del comentario WO-13 (`:406-409`) — corregidas §9/V-3.

---

## §2 · PR-1 (ID D-4) — diffs exactos consolidados

### §2.1 `monitoring/alertmanager/alertmanager.yml` — copiado verbatim de runbook §3.1

Agregar tras la línea 17 (`# DO NOT deploy…`):

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

Validación de contexto: el archivo real (`monitoring/alertmanager/alertmanager.yml`) hoy tiene exactamente las líneas de contexto — L17-18 del header y L44-48 del receiver (`receivers:` / `- name: default` / `webhook_configs:` / `- url: "http://api-server:8080/admin/alertmanager/webhook"` / `send_resolved: true`). El diff aplica limpio.

### §2.2 `docker/compose.prod.yml` — bloque `alertmanager` — copiado verbatim de runbook §3.1

Hoy L556-562 (drift +4 WO-13 — §9). Insertar `environment` + `entrypoint` entre `image:` y `ports:`:

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

Notas de escape (runbook §3.1): en compose, `$$` = `$` literal que el shell del contenedor expande; el placeholder dunder evita toda colisión con la interpolación de compose. El `--storage.path=/alertmanager` del entrypoint reproduce el CMD default de la imagen `prom/alertmanager:v0.27.0` (no hay volumen de datos hoy — L557-558 monta solo el template `:ro`).

### §2.3 `docker/compose.dev.yml` — gemelo dev del render step — **AÑADIDO en este companion** (declarado §8)

El runbook §3.1 (nota final) referencia este gemelo sin escribirlo. Construido desde el archivo real: el bloque dev (`docker/compose.dev.yml:408-420`) monta el MISMO template (`:412`) pero NO es copia literal del prod — usa sintaxis flow (`ports: ["127.0.0.1:9093:9093"]`), no tiene `logging: *id001`, y el archivo aplica la convención dev `:-` con fallback para los tokens (ej. api-server dev en `:269`: `ARBX_ADMIN_TOKEN: ${ARBX_ADMIN_TOKEN:-dev_admin_token_change_me_0123456789}`), a diferencia del prod `:?` (`compose.prod.yml:351`). El gemelo respeta esa convención local:

```diff
   alertmanager:
     image: prom/alertmanager:v0.27.0
+    # WO-14 (2026-09-06) D-4: dev twin of the compose.prod.yml render step.
+    # Dev mounts the SAME alertmanager.yml template (volume below), so without
+    # this block dev keeps POSTing the webhook 401 (same gate:
+    # shared-ts/src/middleware/index.ts:124-130). Dev convention (this file,
+    # L269): `:-` fallback instead of prod `:?` so local boot does not require
+    # the key in the dev .env; the fallback token is sed-safe ([A-Za-z0-9_-]).
+    environment:
+      ARBX_ADMIN_TOKEN: ${ARBX_ADMIN_TOKEN:-dev_admin_token_change_me_0123456789}
+    entrypoint:
+      - /bin/sh
+      - -c
+      - >-
+        case "$$ARBX_ADMIN_TOKEN" in ""|*[!A-Za-z0-9_-]*) echo "ARBX_ADMIN_TOKEN missing or sed-unsafe" >&2; exit 1;; esac;
+        sed "s|__ARBX_ADMIN_TOKEN__|$$ARBX_ADMIN_TOKEN|g" /etc/alertmanager/alertmanager.yml > /tmp/alertmanager.rendered.yml;
+        exec /bin/alertmanager --config.file=/tmp/alertmanager.rendered.yml --storage.path=/alertmanager
     ports: ["127.0.0.1:9093:9093"]
     volumes:
       - ../monitoring/alertmanager/alertmanager.yml:/etc/alertmanager/alertmanager.yml:ro
```

**Decisiones de diseño declaradas (D-1)**: (a) `:-dev_admin_token_change_me_0123456789` en vez de `:?` — es el valor default que el api-server dev YA usa (`compose.dev.yml:269`), así el notify dev end-to-end funciona con el mismo par emisor/receptor de defaults; un `:?` aquí rompería el boot local sin la clave en `.env`. (b) indentación de lista a 6 espacios con `- ` — coincide con el estilo del propio bloque dev (`volumes:` en `:411-412`), no con el estilo prod de §2.2. El `entrypoint` (validación de charset + sed + exec) es idéntico al prod: sin él, dev renderiza nada y sigue en 401.

### §2.4 Pre-flight y aplicación (VPS) — por referencia, sin duplicar

Pre-flight `amtool check-config` (valida que `http_config.headers` exista en v0.27.0 ANTES del `up -d`), aplicación `--no-deps alertmanager`, verificación end-to-end con alerta sintética `WO14SyntheticProbe` y rollback: runbook §3.2, §3.3, §3.4, §3.5. Sin cambios aquí.

---

## §3 · PR-2 (ID D-2 / completa #539) — diffs exactos consolidados

### §3.1 `docker/compose.prod.yml` — bloque `environment` del servicio `edge` — copiado verbatim de runbook §5.2

Hoy L417-421 (drift +4 WO-13 — §9):

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

### §3.2 `docker/compose.dev.yml` — gemelo dev del bloque edge — **AÑADIDO en este companion** (declarado §8)

El runbook §5.2 (nota final) dice «Mismo bloque gemelo en `docker/compose.dev.yml` (servicio edge)». Construido desde el archivo real (`docker/compose.dev.yml:310-328`): el bloque dev edge ya tiene `env_file: ["../.env"]` (`:314`) y claves propias (`FRONTEND_URL`, `PUBLIC_EDGE_HOST`, `ARBX_EDGE_TOKEN` con default dev `:317`). El gemelo «mismo bloque» literal (con `:?`) rompería el boot local; el gemelo correcto para dev es **defaults vacíos = fail-closed idéntico a hoy**:

```diff
   edge:
     build:
       context: ..
       dockerfile: edge/dev-local/Dockerfile
     env_file: ["../.env"]
     environment:
       API_SERVER_URL: http://api-server:8080
       ARBX_EDGE_TOKEN: ${ARBX_EDGE_TOKEN:-dev_edge_token_change_me_0123456789}
       EDGE_PORT: "8787"
+      # WO-14 (2026-09-06) D-2/#539: dev twin of compose.prod.yml edge block.
+      # Prod uses `:?` (deploy veraz); dev convention here is tolerant defaults.
+      # EMPTY defaults are deliberate: unset/empty ⇒ behavior identical to
+      # today, fail-closed — CORS reflector stays empty (edge/worker/src/
+      # index.ts:288-300: "" does not match any origin ⇒ allowed="") and the
+      # auditor bucket stays inert (index.ts:34-36: "Unset ⇒ header ignored").
+      # Set both in the local .env to exercise the PR-2 paths in dev.
+      ALLOWED_ORIGINS: ${ALLOWED_ORIGINS:-}
+      EDGE_AUDIT_TOKEN: ${EDGE_AUDIT_TOKEN:-}
       # QUANTUM FULLSTACK SYMMETRY — SPA-fallback target + public https host.
       FRONTEND_URL: http://frontend:5173
       PUBLIC_EDGE_HOST: edge-arbx.ape-tv.net
```

**Decisiones de diseño declaradas (D-2)**: (a) defaults vacíos en vez de `:?` (rompería dev) o de un origen dev inventado (violación RULE 00 — no se fabrica un valor canónico); vacío preserva la semántica fail-closed verificada en código (`index.ts:288-300`, `websocket.ts:245-251`: `raw=""` → `filter(Boolean)` → `[]` → same-origin only — línea corregida §9/V-1). (b) Nota de invocación: si se corre compose desde `docker/` (sin `.env` local como contexto de interpolación), `environment` explícito con valor interpolado vacío tiene precedencia sobre `env_file` — el efecto degradado es fail-closed (idéntico a hoy, sin regresión de seguridad); la invocación canónica desde la raíz del repo hace que el contexto de interpolación y `../.env` sean el mismo archivo, sin divergencia.

### §3.3 `.env.example` — **AÑADIDO en este companion** (declarado §8)

El runbook §5.2 cita los valores en prosa pero no el diff. Sección de inserción: fin del bloque «Auth / Admin tokens» (`.env.example:217-245`), inmediatamente antes del header `# ------------------ External safety APIs (S3) ------------------` (`:246`) — `EDGE_AUDIT_TOKEN` es un secreto de la misma familia de tokens del bloque y `ALLOWED_ORIGINS` es su par de perímetro del mismo PR-2:

```diff
 # V-AT-1 readiness probe target — edge /admin/session route. Optional;
 # default is the compose-internal URL http://edge:8787/admin/session.
 # V_AT_1_PROBE_URL=
+
+# WO-14 (2026-09-06) D-2 / #539: edge perimeter. ALLOWED_ORIGINS is the CORS
+# allowlist (CSV) read by BOTH surfaces. Edge worker: exact-match reflector
+# (edge/worker/src/index.ts:287-301) — a literal "*" there WOULD reflect ANY
+# origin, with credentials suppressed at :299. api-server WS handshake:
+# websocket.ts:245-251 — "*" intentionally NOT supported there (:247). One
+# shared value, two semantics: set the exact PUBLIC origin, no wildcard. Not
+# a secret, tracked value OK. EDGE_AUDIT_TOKEN is the auditor bucket secret
+# (#539, bucket rl_audit): T1-class per docs/operations/SECRETS_POLICY.md —
+# generate fresh, NEVER commit a real value (mirrors ARBX_ADMIN_TOKEN above).
+ALLOWED_ORIGINS=https://arbx.ape-tv.net
+EDGE_AUDIT_TOKEN=<run: openssl rand -hex 32>

 # ------------------ External safety APIs (S3) ------------------
 # (GOPLUS_API_KEY y HONEYPOT_IS_API_KEY: placeholders VACIOS hoy — ver fila 7
 #  de la tabla §1; sin valor asignado en .env.example. Citadas aqui sin el
 #  patron KEY=valor para no disparar el scanner de secretos.)
```

**Decisión de diseño declarada (D-3)**: el runbook citaba `EDGE_AUDIT_TOKEN=` (vacío); aquí se usa el placeholder `<run: openssl rand -hex 32>` — convención del propio archivo para secretos (`:225,227,231` usan `<run: openssl rand -base64 48>`) y consistente con el comando de generación de FASE 0 (runbook §2 paso 2: `openssl rand -hex 32`). `ALLOWED_ORIGINS=https://arbx.ape-tv.net` es el valor exacto que el runbook §2/§5.2 fija (dominio público, no secreto).

### §3.4 Alivio inmediato, verificación y rollback — por referencia

`.env` VPS primero (runbook §2 FASE 0), recreación `up -d edge api-server` sin rebuild (env de RUNTIME, no `NEXT_PUBLIC_*` — RULE 03 no aplica; runbook §5.1), verificación CORS/bucket (§5.3), rollback (§5.4). Sin cambios aquí.

---

## §4 · Ítem 2 — override systemd cloudflared — copiado verbatim de runbook §4

Patrón: `EnvironmentFile` 0600 + `TUNNEL_TOKEN` como env (token fuera de `argv[]`; `/proc/<pid>/environ` es 0400 owner-root). Drop-in `/etc/systemd/system/cloudflared.service.d/override.conf` (vía `sudo systemctl edit cloudflared`):

```ini
[Service]
EnvironmentFile=/etc/cloudflared/tunnel-env
ExecStart=
ExecStart=/usr/bin/cloudflared --no-autoupdate tunnel run
```

El `ExecStart=` vacío RESETEA el original; la línea nueva es el argv actual SIN `--token` (el token sale del propio unit vía el paso de captura del runbook §4, jamás impreso). Fallback si esa versión ignorara `TUNNEL_TOKEN` (runbook §4): `ExecStart=/usr/bin/cloudflared --no-autoupdate tunnel run --token ${TUNNEL_TOKEN}` — systemd expande desde el EnvironmentFile dentro de ExecStart; residual solo en `/proc/<pid>/cmdline`. Cierre real de exposición: rotar el token en Cloudflare Zero Trust después de higiene verde (el unit fue 644 world-readable toda su vida). Rollback: `rm -f` del override + `daemon-reload` + `restart`. Ventana: ~20-40 s de túnel.

---

## §5 · Matriz de riesgos (consolidada — cada fila cita runbook § o código real)

| # | Riesgo | Evidencia | Mitigación / dónde se cierra | Severidad |
|---|---|---|---|---|
| R1 | `http_config.headers` no soportado en `prom/alertmanager:v0.27.0` → config rechazada en arranque | runbook §3.2 | Pre-flight `amtool check-config` sobre el archivo RENDERADO antes de `up -d` — falla ahí, no en producción; «no continué sin esto» | Alta si se salta el pre-flight; Media con pre-flight |
| R2 | Render consume mal el token (caracteres sed-unsafe) → 401 persiste o config rota | runbook §3.1 entrypoint | `case "$ARBX_ADMIN_TOKEN" in ""\|*[!A-Za-z0-9_-]*) … exit 1` en el entrypoint; troubleshooting §3.5 compara longitudes sin imprimir | Media |
| R3 | Token expuesto en pantalla/logs al operar | runbook §2 nota | `docker compose config` SIN `--quiet` prohibido en terminal compartida; greps de sanity que cuentan matches, jamás imprimen valores; `wc -c`/`head -c 13` para el tunnel-env | Alta si se viola; procedimiento blindado |
| R4 | Ventana de recreate del contenedor alertmanager | runbook §3.3 | `up -d --no-deps alertmanager` — ventana de segundos, solo el canal de notificaciones (el pipeline de detección no pasa por AM); healthcheck `/-/healthy` (compose.prod.yml:563-569) | Baja |
| R5 | Túnel público caído tras el cambio cloudflared | runbook §4 | Ventana declarada 20-40 s; `journalctl` espera 4× «Registered tunnel connection» + 1 request público; fallback documentado + rollback `rm override` | Media |
| R6 | Token cloudflared YA comprometido (644 toda la vida del unit) — hardening no cierra el pasado | runbook §4 «Cierre real de exposición» | Rotación obligatoria en Cloudflare Zero Trust tras higiene verde; actualizar `tunnel-env` | Alta hasta rotar |
| R7 | CORS refleja un origen hostil (regresión V-AT-1) | `edge/worker/src/index.ts:288-300` | Código fail-closed: allowlist exacta, sin `*`; verificación 3b (runbook §5.3: `Origin: https://evil.example` → 0 headers); `:?` del PR-2 evita deploy futuro sin clave | Media |
| R8 | `.env` VPS sin las 2 claves cuando PR-2 mergee → deploy se rompe | runbook §1 matriz (orden) | FASE 0 (runbook §2) es PRIMERO por diseño: desbloquea los guards `:?`; rollback §5.4 nota que tras merge el revert del PR también es necesario | Media si se invierte el orden |
| R9 | Bucket auditor armado sin consumidor (harness sin el token) | runbook §2 nota de distribución | `EDGE_AUDIT_TOKEN` al harness del barrido como header `x-arbx-audit-token`; sin consumidor NO hay regresión: bucket público intacto (`index.ts:395-397` — sin token válido cae al `rl` público) | Baja |
| R10 | Gemelo dev ausente → dev diverge de prod (sigue 401, CORS no ejercitado) | runbook §3.1 nota; este companion §1 filas 5-6 | Cerrado aquí: §2.3 y §3.2 escriben los gemelos verbatim con las convenciones del archivo dev | Baja |
| R11 | Colisión del placeholder con la interpolación de compose (`$` doble sentido) | runbook §3.1 notas de escape | Dunder `__ARBX_ADMIN_TOKEN__` (sin `$`) + `$$` para variables del shell del contenedor; pre-flight cuenta `placeholders_sin_consumir=0` | Baja |
| R12 | Alerta sintética contaminar el ledger de auditoría | runbook §3.4 (b) | `alertname=WO14SyntheticProbe`, `severity=info`, `annotations.summary` «safe to ignore», TTL 3 min, `send_resolved` la cierra en audit_log — trazabilidad completa, no ruido opaco | Baja |
| R13 | Edición de gemelos dev inválida (YAML/estilo) al aplicar PR | §2.3/§3.2 de este companion | Los diffs respetan el estilo del archivo dev (listas a 6 espacios, flow `ports`); `docker compose -f docker/compose.dev.yml config --quiet` valida localmente antes del commit del PR | Baja |

---

## §6 · Invariantes Inv-1/2/3 (expansión operacional del resumen ejecutivo del runbook §7)

### Inv-1 (D-4 — canal de alertas autenticado)

**Enunciado** (runbook §7): para toda ventana ≥16 min post-fix, `grep -c 'status code 401'` en logs de alertmanager = 0 ∧ existe fila `audit_log(actor='alertmanager', target_id='WO14SyntheticProbe')` con `alert.firing` y `alert.resolved`.

**Definición operacional**: el canal AM→api-server queda autenticado si y solo si se cumplen las TRES lecturas del runbook §3.4:
- (a) render consumió el placeholder: `docker exec … grep -c "__ARBX_ADMIN_TOKEN__" /tmp/alertmanager.rendered.yml` → `0`, y `grep -c "x-arbx-admin-token:"` → `1` (§6 tabla fila 1a);
- (b) entrega end-to-end persistida: `alertmanager_webhook.received` con `persisted:1` en logs api-server ∧ fila `alert.firing` (y ≤10 min después `alert.resolved`) en `audit_log` — esquema `database/migrations/011_audit_log.sql:4-16` (fila 1b);
- (c) cese sostenido: ventana ≥16 min (≥3 ciclos `group_interval: 5m`, `alertmanager.yml:26`) con `0` apariciones de `status code 401` en AM y `0` de `"statusCode":401` para `/admin/alertmanager/webhook` en api-server (fila 1c).

**Por qué 16 min**: `repeat_interval` del notify observado en VPS = 5:00 min (runbook §0); 3 ciclos descartan un 401 intermitente enmascarado.

### Inv-2 (cloudflared — cero token legible, túnel vivo)

**Enunciado** (runbook §7): toda lectura pública del unit (`cat`/`show`) y `ps` exponen 0 substrings tipo token ∧ servicio `active` ∧ dominio público responde.

**Definición operacional** (runbook §4 pasos 4-5): `systemctl cat` / `systemctl show -p ExecStart` / `ps -eo args` con `grep -c 'ey[A-Za-z0-9]'` → `0` en los tres; `sudo tr '\0' '\n' < /proc/$(pidof cloudflared)/environ | grep -c '^TUNNEL_TOKEN='` → `1` (solo root); `systemctl is-active` → `active`; journal 4× «Registered tunnel connection»; 1× `curl -sI https://arbx.ape-tv.net/` → `200`. Complemento de cierre (no bloqueante del invariante, sí del riesgo R6): rotación del token en Cloudflare Zero Trust.

### Inv-3 (D-2/#539 — perímetro CORS fail-closed + bucket auditor acotado)

**Enunciado** (runbook §7): ACAO se emite ⇔ `Origin ∈ {https://arbx.ape-tv.net}`; credenciales solo con reflexión; bucket auditor activo solo con token válido, bucket público invariante.

**Definición operacional** (runbook §5.3, todo loopback — 0 requests de dominio público):
- ACAO `https://arbx.ape-tv.net` + `access-control-allow-credentials: true` SOLO con `Origin: https://arbx.ape-tv.net`;
- `Origin: https://evil.example` → 0 headers CORS (fail-closed intacto — fila 3b);
- header `x-arbx-audit-token` válido → `x-ratelimit-remaining` numérico (keyspace `rl_audit`); sin header → bucket público `rl`, comportamiento de hoy sin cambio (`edge/worker/src/index.ts:376-396`);
- superficie 2 (api-server WS): misma variable `ALLOWED_ORIGINS` (`backend/api-server/src/websocket.ts:245-251`) — recreación de AMBOS servicios en la misma ventana (runbook §5.1) mantiene la paridad.
- Nota credenciales (V-AT-1): `allow-credentials` se emite solo cuando el origen reflejado es exacto y no `*` (`index.ts:294-299`) — la allowlist acotada es condición previa del safe-credentialed-CORS.

---

## §7 · Gate de promoción (idéntico al runbook §7 — sin relajación)

1. **PR-1 (D-4)**: archivos `monitoring/alertmanager/alertmanager.yml` + `docker/compose.prod.yml` (bloque alertmanager) + gemelo `docker/compose.dev.yml` (§2.3 aquí). Un PR = un ID (§37 P-∅).
2. **PR-2 (D-2 / completa #539)**: `docker/compose.prod.yml` (bloque edge) + gemelo dev (§3.2) + `.env.example` (§3.3). Un PR = un ID, separado de PR-1.
3. Cada PR: CI verde (contract tests + paridad frontend↔edge incluidas), diffs EXACTOS como los consolidados aquí (§2, §3), marcador `# WO-14 (2026-09-06)` en cada bloque añadido.
4. Pre-flight `amtool check-config` PASS antes de cualquier `up -d` (runbook §3.2) — conditio sine qua non.
5. Deploy veraz: SHA anclado (`git rev-parse HEAD` == SHA despachado, §37 G-gates) + verificación §6 del runbook completa (filas 1a-3c + Global).
6. Ítem cloudflared: gate propio del operador (runbook §4 pasos 3-5) — sin PR, sin repo.
7. **Orden obligatorio**: FASE 0 (`.env` VPS, runbook §2) ANTES de merge de PR-2 (los guards `:?` exigen las claves). PR-1 no requiere claves nuevas (reusa `ARBX_ADMIN_TOKEN`).
8. **Intocables**: §34.3 default-deny/`MainnetRefused`, gates `arbx-*`, kill-switch. Este WO no altera matemática ni terminus de ejecución — solo canal de notificación, higiene de secretos y perímetro CORS.
9. **RULE 00**: cero valores fabricados — tokens/origen viven solo en `.env` VPS; el repo lleva placeholders (`__ARBX_ADMIN_TOKEN__`, `${VAR:?}`, `${VAR:-}`, `<run: openssl rand -hex 32>`), nunca literales.

---

## §8 · Tabla de procedencia (declaración RULE 00 — qué se copió y qué se añadió)

| Sección de este companion | Origen | Declaración |
|---|---|---|
| §1 | Nuevo (adjudicación) | Tabla de verificación runbook↔repo; todas las validaciones ejecutadas read-only contra archivos reales el 2026-09-06 |
| §2.1, §2.2 | **Copiado verbatim** del runbook §3.1 | Contexto re-validado contra `monitoring/alertmanager/alertmanager.yml:17-18,44-48` y `docker/compose.prod.yml:556-562` — idéntico |
| §2.3 | **AÑADIDO** — runbook solo apuntaba («gemelo va en compose.dev.yml:408-412») | Construido desde `docker/compose.dev.yml:408-420` real; convención `:-` tomada de `:269`; decisiones D-1 declaradas inline |
| §3.1 | **Copiado verbatim** del runbook §5.2 | Contexto re-validado contra `docker/compose.prod.yml:417-421` — idéntico |
| §3.2 | **AÑADIDO** — runbook solo decía «Mismo bloque gemelo» | Construido desde `docker/compose.dev.yml:310-328` real; semántica fail-closed de los defaults vacíos verificada en `edge/worker/src/index.ts:34-36,288-300` y `backend/api-server/src/websocket.ts:245-251`; decisiones D-2 declaradas inline |
| §3.3 | **AÑADIDO** — runbook citaba los valores en prosa | Diff construido sobre `.env.example:217-246` real; valor `ALLOWED_ORIGINS` = el del runbook §2/§5.2; placeholder del secreto según convención del archivo (`:225,227,231`); decisión D-3 declarada inline |
| §4 | **Copiado verbatim** del runbook §4 | Contenido ini + fallback + rotación + rollback, sin cambios |
| §5 | Nuevo (consolidación pedida por el charter) | Cada fila cita runbook § o file:line verificados; cero hechos nuevos introducidos |
| §6 | Expansión del resumen ejecutivo del runbook §7 | Los comandos y umbrales provienen de runbook §3.4/§4/§5.3/§6; la justificación del umbral 16-min deriva de `group_interval: 5m` (`alertmanager.yml:26`) + ventana observada en VPS (runbook §0) |
| §7 | **Copiado** del runbook §7 | Estructura idéntica, ítems numerados; sin relajación |

Cero contenido nuevo inventado: los tres bloques añadidos (§2.3, §3.2, §3.3) existen como archivos reales del repo y sus diffs se construyen sobre su contenido exacto; las decisiones de diseño (D-1, D-2, D-3) se declaran explícitamente con su fuente. El token real jamás aparece en este documento.

---

## §9 · Pase de verificación B (RESPAWN-2 · reemplazo B · mitad segunda — 2026-09-06)

> Origen: el agente original de WO-14 murió; este companion fue escrito primero por el reemplazo A (o recuperado del agente caído). Este §9 es la capa de verificación independiente del reemplazo B sobre la **mitad segunda** del charter (PR-2/D-2: gemelo dev edge + `.env.example`; cloudflared §4; matriz §5; invariantes §6; gate §7), con verificaciones cruzadas declaradas donde la frontera A/B lo exigía (§1, §2.2). Método: lectura read-only de los archivos reales del repo; 0 requests de dominio público; 0 escrituras git; 0 mutaciones VPS; archivos de producción NO tocados.

### 9.1 Hallazgos y correcciones aplicadas in situ (todas marcadas)

| ID | Defecto | Evidencia (archivo real) | Corrección |
|---|---|---|---|
| V-1 | Cita stale `websocket.ts:146-149` — allí vive `CartridgeTelemetry` + comentario Route Discovery, NO `parseAllowedOrigins` | función real en `backend/api-server/src/websocket.ts:245-251` (`raw = process.env["ALLOWED_ORIGINS"] ?? ""` → `split(',').map(trim).filter(Boolean)`; uso en `:316`); semántica citada en los docs era CORRECTA, solo la línea no | `:146-149` → `:245-251` en runbook §0/§5.1 y aquí §1, §3.2, §3.3 (diff), §6 Inv-3, §8 |
| V-2 | Atribución invertida: «wildcard "\*" NOT supported — index.ts:288-300». El edge worker SÍ soporta `*` literal | `edge/worker/src/index.ts:290-291` (`ALLOWED_ORIGINS === "*" ? "*"` → refleja CUALQUIER origen) con credenciales suprimidas en `:299`; quien rechaza `*` es api-server WS (`websocket.ts:245-247`, comentario «"\*" is INTENTIONALLY NOT supported») | comentario del diff §3.3 reescrito: ambas superficies, dos semánticas, un valor compartido → origen exacto, sin wildcard |
| V-3 | Drift +4 en TODAS las citas de `compose.prod.yml` bajo L406: el comentario WO-13 (`:406-409`, 4 líneas) desplazó el archivo después de medidas las líneas | `:415-416` env_file edge (citado `:411-412`); `:417-421` bloque environment edge (citado `:413-417`); `:556-572` bloque alertmanager (citado `L552-568`); `:563-569` healthcheck (citado `L559-565`) | líneas corregidas en runbook §0(vía §5.1)/§3.1/§3.5/§5.1/§5.2/§6 y aquí §1 filas 2/4, §2.2, §3.1, §5 R4, §8 — contenido de los diffs intacto (aplican limpio) |
| V-4 | Rango menor en R9: `index.ts:393-396` apuntaba al final de la rama auditor | rama pública `rl` real en `edge/worker/src/index.ts:395-397` | `:395-397` |
| V-5 | Claim §1 «citas verificadas (12/12 exactas)» falsado por V-1/V-3 | ver V-1/V-3 | §1 reescrito honesto: 10/12 exactas, 2 con drift de LÍNEA (semántica intacta), corregidas aquí |

### 9.2 Verificado-correcto por esta mitad (sin cambios)

`monitoring/alertmanager/alertmanager.yml:17-18` y `:44-48` (receiver verbatim — el diff §2.1 aplica limpio), `:25` (`group_wait: 10s`), `:26` (`group_interval: 5m` — base del umbral 16-min de Inv-1) · `docker/compose.prod.yml:351` (`ARBX_ADMIN_TOKEN:?`) y `:338-339` (env_file api-server — por ENCIMA de la inserción WO-13, sin drift) · `docker/compose.dev.yml:269` (default dev `:-dev_admin_token_change_me_0123456789` — fuente de D-1), `:310-328` (bloque edge dev — contexto del diff §3.2 verbatim exacto), `:408-412` (alertmanager dev monta el MISMO template `:412`) · `.env.example:217-248` (sección Auth/Admin tokens; convención `<run: openssl rand -base64 48>` en `:225,227,231`; AMBAS claves ausentes hoy — claim fila 7 §1 confirmado por grep 0 matches; contexto del diff §3.3 exacto) · `edge/worker/src/index.ts:33-36` (EDGE_AUDIT_TOKEN unset ⇒ header ignorado), `:287-301` (reflector CORS exact-match + supresión de credenciales con `*`), `:376-397` (bucket auditor `rl_audit` vs `rl` público) · `backend/api-server/src/routes/stubs.test.ts:102-104` (contrato 401) · `shared-ts/src/middleware/index.ts:124-130` (gate lee SOLO `x-arbx-admin-token`, sin `Authorization` — la «Corrección al charter» del runbook §0 es válida) · `database/migrations/011_audit_log.sql:4-16` (esquema citado por Inv-1/§3.4).

### 9.3 Adjudicación del ítem 3 del charter (referencias del runbook)

Las referencias del runbook al companion **RESUELVEN sin renumeración**: runbook §3.1 (nota final) → «WO-14-DESIGN.md §2.3» = gemelo dev alertmanager (existe, §2.3 aquí) ✓; runbook §5.2 (nota final) → «§3.2 (gemelo dev) y §3.3 (.env.example)» = existen con ese número ✓; runbook §4/línea 4 → companion existe ✓; runbook §7 → «detalle en WO-14-DESIGN.md» = §6 (invariantes) + §7 (gate) ✓. La numeración del companion NO cambió con este pase (§9 es aditivo al final). Ediciones al runbook limitadas a correcciones de cita V-1/V-3 (6 ediciones quirúrgicas, cada una marcada `// WO-14 (2026-09-06)`).

### 9.4 No verificado por esta mitad (declarado, no inventado — R8)

- Hechos VPS de §0 (cadencia 401 en logs, `docker inspect` de env/entrypoint, `systemctl show` del token, label `config_files`): **heredados** de la medición read-only del autor del runbook @2026-09-07T00:21Z; este pase no re-sondeó el VPS (presupuesto dominio 0; los diffs no dependen de esos hechos).
- §2.2 nota «`--storage.path=/alertmanager` reproduce el CMD default de la imagen `prom/alertmanager:v0.27.0`»: hecho de la IMAGEN Docker, no verificable desde el repo sin pull — se declara no verificado por este pase (sin efecto sobre la corrección del diff: el flag es explícito en el entrypoint propuesto).

*Pase B: 0 requests de dominio público · 0 git write · 0 mutación VPS · producción intacta.*

---

*WO-14 · devops-platform · Gang Omniscience · 2026-09-06 — companion de diseño read-only; ejecución 100% del operador (runbook). Presupuesto dominio público consumido por este agente: 0 requests.*
