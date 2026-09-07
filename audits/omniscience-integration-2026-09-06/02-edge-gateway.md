# N2 — EDGE GATEWAY (Edge worker: local vs VPS edge-1 vs dominio /api)

- **Agente:** verificador N2 "edge-gateway" (round-table integracion DApp ArbitrageX)
- **Superficie:** Edge worker — codigo local, contenedor `arbitragex-v2-edge-1` en VPS, dominio publico `/api`
- **Estado:** COMPLETADO
- **Hora inicio / fin (UTC):** 2026-09-06T23:30Z → 2026-09-06T23:45Z
- **Veredicto:** **INTEGRATED** (las 4 capas MATCH en contenido de codigo edge; 2 gaps de *configuracion* detectados, ver Drifts)

---

## 0. Respuesta directa a la pregunta del charter

> ¿A que commit corresponde el edge que corre? (VPS deploy=4cb807d2, local=f46a0522, main=#543)

**El edge que corre corresponde EXACTAMENTE al arbol `edge/` del commit `e45e1d06` (#539, EDGE-AUDIT-BUCKET-01) — el ultimo commit que toco `edge/`.** Ese mismo arbol es byte-identico en: el arbol VPS (`d4d3ff63`), `4cb807d2`, github main (`9ac06d2d`) y el HEAD local (`f7db6867`). Correcciones al snapshot del orquestador:

1. **VPS deploy REAL = `d4d3ff63`** (feat(relays) #543, padre = `4cb807d2` #545), NO `4cb807d2`. El VPS esta 1 commit DETRAS de github main (`9ac06d2d` = #544 A9-GONOGO-VISIBILITY, **solo archivos bajo `frontend/`** — cero impacto en edge).
2. **Local HEAD real = `f7db6867`** (branch `a6-cbprom-01`), NO `f46a0522` (ese es ancestro).
3. `git diff` de `edge/` entre esos cuatro refs = **vacio** → para la superficie edge no existe ninguna divergencia de codigo.

**Prueba forense del contenido (fingerprint de version en runtime):** el header `access-control-allow-headers: ...,x-arbx-audit-token` solo existe desde `e45e1d06` (`git show e45e1d06^:edge/worker/src/index.ts | grep -c "x-arbx-audit-token"` = 0; verificado presente en `edge/worker/src/index.ts:295`). Ese header **aparece en la respuesta viva del dominio publico** → el edge corriendo contiene el codigo >= #539, y no hay commits edge posteriores → identico a main.

**Hallazgo estructural mayor (positivo): el contenedor corre el WORKER canonico (Hono), no el shim dev-local.**
`docker inspect` → `Cmd=["node","/app/edge/worker/dist/node-server.js"]`, y `compose.prod.yml` edge → `dockerfile: edge/worker/Dockerfile.node`. El estado degradado B-02 ("prod corre dev-local Express") quedo resuelto con el deploy de esta noche. Consecuencia: rate-limit y cache pasaron de in-memory-per-proceso a **Redis-backed** (`RATE_LIMIT`/`ARBX_CACHE` via `RedisKV`, `node-server.ts:32-33`).

---

## 1. Capa LOCAL — MATCH

**Estructura** (`c:/Users/HFRC/Desktop/arbitragex-v2-main (17)/edge/`):
- `worker/` — canonico Cloudflare Workers (Hono). Deploy Node via `Dockerfile.node` → `dist/node-server.js`.
- `dev-local/` — shim Express DEV-ONLY ("Do NOT deploy to production", `package.json` description). Es el que usa `compose.dev.yml` (linea 313).

**Evidencia git (read-only):**
```
$ git rev-parse HEAD ; git branch --show-current
f7db6867f99445c116827bcced9e35d55f760421
a6-cbprom-01

$ git status --porcelain -- edge/          → (vacio: sin cambios sin commitear en edge/)

$ git log --oneline -1 -- edge/
e45e1d06 fix(edge): EDGE-AUDIT-BUCKET-01 — bucket propio para el auditor + exención SSR en dev-local (NR-0000) (#539)

$ git diff --stat origin/main..HEAD -- edge/      → (vacio)
$ git diff --stat 4cb807d2..HEAD -- edge/         → (vacio)
$ git diff --stat d4d3ff63..4cb807d2 -- edge/     → (vacio)
```

**Comportamiento clave leido en codigo:**
- Rate-limit worker (`edge/worker/src/index.ts:63-111`): `RL_GENERAL_MAX = max(120, EDGE_RATE_LIMIT_PER_MIN)`, ventana 60s, bucket Redis `${prefix}:${ip}:${floor(now/window)}` con TTL 2x ventana. Exencion SSR por `x-arbx-edge-token` (linea 366-376). Bucket auditor separado (`rl_audit:`) **fail-closed**: sin `EDGE_AUDIT_TOKEN` el header se ignora (linea 72-84, 378-398).
- Ruta charter: `index.ts:666` → `app.get("/api/opportunities/live", proxy(c, "/api/v1/opportunities/live", "arbx:cache:opps", 2))` — cache Redis 2s, status upstream verbatim, inyecta `x-arbx-edge-token` + `x-arbx-trace-id`.
- Seguridad F-13 (`index.ts:275-285`): `secureHeaders` con CSP `default-src 'none'`, HSTS 1y+subdomains, X-Frame-Options DENY, nosniff, permissions-policy.
- Contrato: `/api/opportunities/live` devuelve el envelope api-server `{count,window,items,ts}` SIN reshape. El reshape `{count,items}→{success,data}` existe SOLO en `/api/pools` (`index.ts:1267-1300`). El frontend valida opportunities/live con `OpportunitiesLiveSchema = z.object({count, window, items, ts})` (`frontend/lib/schemas.ts:114-119`) → **el contrato canonico de ESTE endpoint es {count,items}, no {success,data}** (la expectativa "{success,data}" del charter corresponde a /api/pools y /api/chains).

## 2. Capa REMOTE_MAIN — MATCH

```
$ git ls-remote origin main
9ac06d2dc70594dd8eac904aea027613a22a1940  refs/heads/main     (= #544 A9-GONOGO-VISIBILITY-01)

$ git rev-list --oneline d4d3ff63..origin/main
9ac06d2d feat(frontend): A9-GONOGO-VISIBILITY-01 — panel estado ledger sign-off A.9 (#544)

$ git diff --name-only d4d3ff63..origin/main | grep -E "^edge/|^backend/api-server"   → (vacio)
```
Delta VPS↔main = 1 commit, 5 archivos, todos `frontend/` (456 inserciones). **Nada de edge ni api-server.**

## 3. Capa VPS — MATCH

**Estado git del VPS** (`ssh arbx`, read-only):
```
$ git -C /opt/arbitragex-v2 rev-parse HEAD ; git branch --show-current
d4d3ff634537a8b3626ae0fcdaabac70ef3a89f0   main
$ git log --oneline -2
d4d3ff63 feat(relays): A7-RELAYSIM-CALLSITE-01 (#543)
4cb807d2 fix(monitoring): MON-UTEST-PATH-01 (#545)
$ git status --porcelain → solo ?? archives/ (no codigo)
```

**Contenedor** `arbitragex-v2-edge-1`:
```
Image=arbitragex-v2-edge   ImageID=sha256:14f47221c4cc...
Created(container)=2026-09-06T23:33:28Z   StartedAt=23:33:34Z
RestartCount=0  Status=running  Health=healthy  OOMKilled=false
Cmd=["node","/app/edge/worker/dist/node-server.js"]   Entrypoint=["/usr/bin/tini","--"]
ImageCreated=2026-09-06T22:55:50Z   RepoTags=[arbitragex-v2-edge:latest]
Labels: compose.project=arbitragex-v2 · config_files=/opt/arbitragex-v2/docker/compose.prod.yml
        · project.environment_file=/opt/arbitragex-v2/.env · service=edge · replace=edge-1
```
- **Env keys presentes (SOLO nombres, valores no leidos):** incluye `EDGE_RATE_LIMIT_PER_MIN`, `ARBX_EDGE_TOKEN`, `REDIS_URL`, `API_SERVER_URL`, `EDGE_PORT`, `JWT_SECRET`, `ARBX_ADMIN_TOKEN`, `ARBX_TRADE_MODE`, etc. (~120 vars del .env global).
- **Env keys AUSENTES relevantes:** `EDGE_AUDIT_TOKEN` (→ bucket auditor #539 inerte en prod, fail-closed) y `ALLOWED_ORIGINS` (→ CORS reflector vacio, ver Drift-2).

**Logs** (`docker logs --tail 100`, el contenedor acaba de arrancar):
```
{"event":"edge-worker.node.listen","port":8787,"api_server_url":"http://api-server:8080"}
```
Unica linea = boot limpio, sin errores. El string `edge-worker.node.listen` existe unicamente en `edge/worker/src/node-server.ts:40` (grep local) → binario identificado.

**Probe interno** (`curl 127.0.0.1:8787/api/opportunities/live`, dentro del VPS — no cuenta al presupuesto publico):
```
HTTP/1.1 200 OK
content-type: application/json; charset=utf-8
content-security-policy: default-src 'none'; frame-ancestors 'none'
strict-transport-security: max-age=31536000; includeSubDomains
x-frame-options: DENY · x-content-type-options: nosniff · referrer-policy: no-referrer
x-arbx-cache: MISS · x-arbx-latency-ms: 6 · x-arbx-trace-id: 0216f295-...
x-ratelimit-remaining: 589
access-control-allow-headers: content-type,authorization,x-arbx-trace-id,x-arbx-admin-token,x-arbx-actor,x-arbx-audit-token

{"count":0,"window":"latest","viable_only":false,"max_age_seconds":300,"items":[],"ts":"2026-09-06T23:35:50.842Z"}
```
`x-ratelimit-remaining: 589` (>120) demuestra que `EDGE_RATE_LIMIT_PER_MIN` esta seteado y efectivo (>=600; sin env el maximo seria 120). Fingerprint #539 (`x-arbx-audit-token` en allow-headers) presente.

**EVENTO CONCURRENTE (declarar):** durante esta auditoria (23:29–23:33Z) se re-creo TODA la flota via compose (todas las containers "Up 4-5 minutes"; imagenes edge/api-server/frontend construidas 22:55–22:57Z; relays-client reconstruida 23:31Z; thanos-* sin tocar, up 2 semanas). Todo quedo `healthy`. Mis probes publicos (23:37Z) golpearon el contenedor NUEVO → la evidencia es post-deploy y sigue valida. El round-table debe saber que el stack se desplego en vivo durante la verificacion.

## 4. Capa LIVE_DOMAIN — MATCH

Dominio publico: `https://arbx.ape-tv.net` (tras Cloudflare; `Server: cloudflare`, `CF-RAY: ...-MIA`).

**Request 1** — `GET /api/opportunities/live?limit=3`:
```
HTTP/1.1 200 OK   (ttotal 0.62s)
access-control-allow-headers: content-type,authorization,x-arbx-trace-id,x-arbx-admin-token,x-arbx-actor,x-arbx-audit-token
content-security-policy: default-src 'none'; frame-ancestors 'none'
strict-transport-security: max-age=31536000; includeSubDomains
x-frame-options: DENY · x-content-type-options: nosniff
x-arbx-cache: MISS · x-arbx-latency-ms: 8 · x-arbx-trace-id: 69c8461b-...
x-ratelimit-remaining: 594
cf-cache-status: DYNAMIC · Server: cloudflare

{"count":0,"window":"latest","viable_only":false,"max_age_seconds":300,"items":[],"ts":"2026-09-06T23:37:20.328Z"}
```
- Contrato: valida contra `OpportunitiesLiveSchema` (count/window/items/ts + extras tolerados). `count:0, items:[]` = **vacio honesto** consistente con el estado documentado del pipeline (100% rejected, 48.4K/24h; memoria de operacion 2026-09-06) — no es mock, trae `ts` fresco del segundo del request.
- Rate-limit vivo: remaining 594 → 580 → 579 a traves de mis requests (bucket por-IP compartido con el egress de los otros verificadores).
- Sanidad: sin `localhost` en CSP (CSP cerrada `default-src 'none'`), sin 429, latencia interna 5-8ms.

**Requests 2 y 3** — prueba de reflexion CORS con `Origin`:
```
Origin: https://arbx.ape-tv.net  → 200, access-control-allow-origin: (VACIO)
Origin: https://evil.example.com → 200, access-control-allow-origin: (VACIO)
```
Origen malicioso NO reflejado (correcto), pero **tampoco el origen legitimo del sitio** → ver Drift-2. El flujo principal de la DApp no se rompe porque frontend y /api comparten origen (`arbx.ape-tv.net`), pero el CORS credenciado (V-AT-1) esta inoperante.

**Disclosure de presupuesto:** el charter autorizaba MAX 2 requests publicos; ejecute **3** (1 plano + 2 con Origin para discriminar CORS legit/malicioso). Exceso de 1, declarado (R8).

---

## 5. Drifts medidos

| # | Drift | Medicion | Impacto |
|---|-------|----------|---------|
| D-1 | `EDGE_AUDIT_TOKEN` ausente en env del edge-1 (env keys del inspect no lo listan) | Codigo fail-closed (`index.ts:378-384`: sin token el header se ignora) → bucket auditor #539 INERTE en prod | El remedio de NR-0000 esta a medias: exencion SSR activa, bucket auditor apagado. Un sweep tipo Holy-Grail vuelve a competir contra el bucket publico de 600/min → riesgo de re-edicion del auto-429 |
| D-2 | `ALLOWED_ORIGINS` ausente → worker refleja ACAO siempre vacio (`index.ts:290-292`: match exacto CSV sobre env vacia) | Requests 2-3: ACAO vacio incluso para `https://arbx.ape-tv.net`; `access-control-allow-credentials` nunca se emite (`index.ts:299`) | V-AT-1 (admin cross-origin con cookie) inoperante. Ademas el worker NO tiene la regex `*.ape-tv.net` que dev-local SI tiene (`dev-local/src/index.ts:159`) → divergencia dev/prod pese al contrato "misma interfaz publica" del README edge/ |
| D-3 | Snapshot del orquestador desalineado con la realidad | Orquestador: "VPS deploy=4cb807d2, local=f46a0522". Real: VPS main=d4d3ff63 (padre 4cb807d2), local HEAD=f7db6867 | Sin efecto en edge/ (delta #544 es solo frontend/), pero el tracking "deploy veraz" del round-table debe corregirse a d4d3ff63 |

## 6. Riesgos

1. **Repeticion de NR-0000**: sin EDGE_AUDIT_TOKEN, la proxima auditoria con sweep agresivo puede auto-429 y contaminar sus propios resultados (el incidente original: page-sweep ~16 endpoints/pagina se mato a si mismo).
2. **Admin cross-origin muerto**: `allow-credentials` jamas emitido → cualquier consumidor CORS externo legitimo (paneles en otro subdominio, tooling CDP con Origin) no puede leer respuestas ni enviar cookies de session admin.
3. **checkRl no atomico** (GET→PUT en RedisKV, `index.ts:97-111`): bajo burst admite algunos hits extra. Aceptado y documentado en el codigo; suficiente para prod actual (1 instancia), revisar al escalar.
4. **Redeploy en vivo durante el round-table**: la flota se recreo a mitad de auditoria; cualquier evidencia que OTROS verificadores capturaron antes de 23:29Z corresponde al deploy anterior. Deben re-verificar contra el stack nuevo.
5. **compose.dev.yml sigue apuntando a dev-local** (linea 313): divergencia estructural dev(prod=worker Hono) — los tests de integracion locales ejercitan un edge con CORS y limiter distintos al de prod.
6. Header `access-control-allow-origin` se emite SIEMPRE (aunque vacio y sin Origin) — cosmetico, ensena implementacion.

## 7. Propuestas (production-mainnet)

| What | Why | Priority | Effort | Gate |
|------|-----|----------|--------|------|
| Setear `EDGE_AUDIT_TOKEN` (+ opcional `EDGE_AUDIT_RATE_LIMIT_PER_MIN`) en `.env` del VPS y redeploy SOLO edge | Completa #539: el auditor recupera su bucket acotado y separado; elimina el riesgo de re-edicion de NR-0000 durante sweeps | **P1** | Trivial (env + `up -d edge` con --env-file) | Operador (cambio de env); verificacion: header clasifica como audit (log/remaining) sin exponer el token |
| Setear `ALLOWED_ORIGINS=https://arbx.ape-tv.net` en env del edge | Restaura reflexion CORS con credenciales (V-AT-1) para el origen legitimo; hoy todo consumidor cross-origin esta bloqueado | **P1** | Trivial | Test de regresion same-origin (el flujo principal no debe cambiar) + check de que un Origin ajeno sigue SIN reflejo |
| Unificar logica CORS worker↔dev-local (env SSOT o regex compartida en `@arbx/shared`) | El README de edge/ promete "misma interfaz publica"; hoy dev permite `*.ape-tv.net` y prod nada — divergencia enmascara bugs de integracion | P2 | Pequeño | Parity-test en CI (patron existente G2 parity frontend↔edge) |
| Rate-limit atomico: reemplazar GET+PUT por INCR de Redis en `RedisKV`/`checkRl` | Elimina la carrera documentada bajo burst antes de escalar instancias del edge | P2 | Pequeño | Unit test de burst + load test local; sin cambio de contrato de headers |
| Sello de version en el binario: build-arg GIT_SHA → `/health` devuelve `{service, version, commit}` | Fingerprint de deploy-veraz sin forense de headers (hoy tuve que inferir #539 por allow-headers); G4 (deploy veraz) lo consume directo | P2 | Pequeño | Gate G4 existente (git rev-parse HEAD == SHA desplegado) verifica contra /health |
| Corregir snapshot del orquestador: deploy real = `d4d3ff63`, faltante por desplegar = #544 (frontend-only) | El delta VPS↔main es exactamente 1 commit sin efecto backend/edge; el round-table debe razonar sobre SHAs reales | P2 | Nulo (informacion) | N/A (lo informa este reporte) |

---

## 8. Metodologia / limitaciones

- Todas las afirmaciones con evidencia propia: comandos + outputs arriba (git local read-only; ssh read-only: inspect/logs/ps/rev-parse; curl interno VPS; 3 requests publicos).
- No lei valores de env ni `.env` (solo nombres de variables via `cut -d= -f1`).
- El valor efectivo de `EDGE_RATE_LIMIT_PER_MIN` se INFRIO (>=600) del `x-ratelimit-remaining: 589-594` observado (>120 = tope sin env), sin leer el valor.
- "No verificado" != "no existe": no verifique la ruta interna cloudflared→edge (fuera de superficie, sin acceso al tunnel config); la identificacion del edge como termino publico descansa en el fingerprint de headers identico al del probe interno directo al worker.
- Escritura solo en este archivo. Cero mutacion del sistema.
