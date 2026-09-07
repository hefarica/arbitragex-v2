# N1 — Frontend Web: código local vs dominio en vivo

- **Agente:** verificador N1 (frontend-web), round-table integración omniscience
- **Superficie:** Frontend web — `frontend/` local vs `https://arbx.ape-tv.net`
- **Estado:** COMPLETADO
- **Veredicto global: DEGRADED** — la webapp está viva y sana (200, healthy, paridad público↔contenedor 28/28 assets), pero **el dominio sirve un build 1 PR de frontend detrás de main**, la CSP está **100% en report-only (nada enforcing)** y el build **no expone SHA/versión verificable**.
- **Hora:** 2026-09-06 ~18:35 (-0500) / 23:35 UTC
- **Presupuesto público:** 4 requests HTTP usados de 5 permitidos (`/` headers+HTML, `/opportunities`, `/live-readiness`).

---

## 1. CAPA LOCAL — DRIFT

Comandos y outputs crudos (clone Windows `arbitragex-v2-main (17)`):

```
$ git branch --show-current
a6-cbprom-01
$ git rev-parse main
28d48cdde5e486fa12fa343ec7510f4b0231ac75        # main LOCAL = #531 (STALE)
$ git rev-parse origin/main
9ac06d2dc70594dd8eac904aea027613a22a1940        # origin/main cacheado = GitHub main ACTUAL
$ git status --short -- frontend/
(vacío → frontend/ SIN cambios no commiteados)
$ git log --oneline -6 9ac06d2d
9ac06d2d feat(frontend): A9-GONOGO-VISIBILITY-01 — panel estado ledger sign-off A.9 (#544)
d4d3ff63 feat(relays): A7-RELAYSIM-CALLSITE-01 — wire relay_no_submit_sim (#543)
4cb807d2 fix(monitoring): MON-UTEST-PATH-01 (#545)
de576965 test(searcher): G7-SCOPEDREEVAL-01 (#541)
...
$ ls -la frontend/features/readiness/GoNoGoSignOffCard.tsx
-rw-r--r-- 1 HFRC ... 12248 Sep  6 18:27 GoNoGoSignOffCard.tsx   # el árbol local SÍ tiene #544
```

- El working tree (a6-cbprom-01 @ f7db6867, que mergeó origin/main=9ac06d2d hoy 18:27) **incluye el PR #544** (panel Go/No-Go A.9, +456 líneas / 5 archivos frontend).
- **Drift local A:** el ref `main` local está ~14+ commits atrás (28d48cdd, era #531). "Compilar el main local" daría una app más vieja que la que corre en vivo. La referencia correcta de "código local fresco" es origin/main=9ac06d2d.
- **Drift local B (riesgo §36):** el HEAD del árbol compartido se movió HOY por agentes concurrentes: `git reflog` muestra checkout a6-cbprom-01↔a9-gonogo-visibility-01 y merges de origin/main a las 17:32, 18:01 y 18:27 (-0500).
- Remotes: solo `origin = https://github.com/hefarica/arbitragex-v2.git` (sin remote VPS bare; RULE 01 documenta origin=VPS bare + github=GitHub — divergencia menor de configuración del clone).

### Hallazgo semilla CSP — CONFIRMADO en código

`frontend/next.config.js` (leído completo):

```js
// L18-19: "CSP-Report-Only: strict policy that LOGS violations but does not enforce.
//          Switch to 'Content-Security-Policy' once the report stream is clean ≥7 days."
// L43:  "script-src 'self' 'unsafe-inline' 'unsafe-eval'",
// L149: { key: "content-security-policy-report-only", value: csp() },
```

La ÚNICA cabecera CSP que emite el app es **report-only**. Además la política ya nace débil (`unsafe-inline` + `unsafe-eval` en script-src, pendientes de nonce). El flip a enforcing que el propio comentario promete lleva pendiente desde junio (N5 fix 2026-06-13).

- Sin mecanismo de identidad de build: grep `GIT_SHA|BUILD_SHA|NEXT_PUBLIC_BUILD|deploy_sha|buildId` en `frontend/` = **0 matches**. `package.json` = `@arbx/frontend` v0.1.0, Next **14.2.35**.

## 2. CAPA REMOTE_MAIN (GitHub) — MATCH

```
$ git ls-remote origin refs/heads/main
9ac06d2dc70594dd8eac904aea027613a22a1940	refs/heads/main
```

- GitHub `main` = **9ac06d2d (#544, PR de FRONTEND)**, cuyo padre es **d4d3ff63 (#543)**.
- El cache local de origin/main coincide (fetch hecho hoy por otro agente) → la vista del clone sobre el remoto es actual.
- Diff frontend exacto VPS→GitHub main: `git diff --stat d4d3ff63 9ac06d2d -- frontend/` =
  `GoNoGoSignOffCard.tsx +317, page.tsx +2, api-client.ts +16, schemas.ts +55, test +66` → **5 archivos, 456 inserciones, 0 borrados**.

## 3. CAPA VPS — DRIFT

```
$ ssh arbx "cd /opt/arbitragex-v2 && git rev-parse HEAD && git branch --show-current"
d4d3ff634537a8b3626ae0fcdaabac70ef3a89f0
main
$ git log --oneline -3
d4d3ff63 (#543) / 4cb807d2 (#545) / de576965 (#541)     ← #544 NO está en la historia visible
$ git merge-base --is-ancestor 9ac06d2d HEAD
fatal: Not a valid object name 9ac06d2d                  ← el repo VPS NI TIENE el objeto #544
$ git status --short        → solo "?? archives/" (árbol limpio)

$ docker ps --format ... | grep -i front
arbitragex-v2-frontend-1  c67a2f98c2f3  Up 34 minutes (healthy)  2026-09-06 22:58:04 +0000
$ docker inspect arbitragex-v2-frontend-1 --format 'image={{.Image}} started={{.State.StartedAt}}'
image=sha256:3246c967fc89...  started=2026-09-06T23:33:35Z
$ docker images --no-trans | grep frontend
sha256:3246c967fc89...  arbitragex-v2-frontend:latest  2026-09-06 22:57:45 +0000 UTC
$ docker logs arbitragex-v2-frontend-1 --tail 8
▲ Next.js 14.2.35 — ✓ Ready in 510ms
```

- VPS `main` = d4d3ff63 = **GitHub main menos exactamente 1 commit, y ese commit es el PR de frontend #544**. El repo VPS no ha hecho fetch desde que #544 se mergeó → el build NO puede incluirlo.
- Imagen frontend construida HOY 22:57:45Z desde árbol limpio en d4d3ff63; contenedor healthy, reiniciado 23:33:35Z.
- Origen (127.0.0.1:5173) — cabeceras reales:

```
content-security-policy-report-only: default-src 'self'; script-src 'self' 'unsafe-inline'
  'unsafe-eval'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:
  https://raw.githubusercontent.com https://assets.coingecko.com https://coin-images.coingecko.com
  https://cdn.dexscreener.com; font-src 'self' data:; connect-src 'self' ws: wss:
  https://api.web3modal.org https://pulse.walletconnect.org; frame-ancestors 'none';
  base-uri 'self'; form-action 'self'; object-src 'none'
strict-transport-security: max-age=31536000; includeSubDomains   (HSTS SÍ presente)
x-frame-options: DENY | x-content-type-options: nosniff | referrer-policy: no-referrer
```

## 4. CAPA LIVE_DOMAIN (https://arbx.ape-tv.net) — DRIFT

Request 1 (`curl -sI https://arbx.ape-tv.net/`):

```
HTTP/1.1 200 OK — Server: cloudflare, CF-RAY: a37148ec3b94335b-MIA, cf-cache-status: DYNAMIC
content-security-policy-report-only: <política IDÉNTICA byte a byte a la del origen>
strict-transport-security: max-age=31536000; includeSubDomains
x-frame-options: DENY | nosniff | referrer-policy: no-referrer
Report-To/Nel: cloudflare (los reportes van al colector CF, no hay enforcement propio)
```

**Hallazgo semilla VALIDADO end-to-end**: código (`next.config.js:149`) → origen (5173) → Cloudflare tunnel → dominio público. La única política CSP del dominio es report-only ⇒ **NINGUNA restricción CSP se aplica a los navegadores reales** (protección XSS por CSP = 0).

**Paridad público↔contenedor (assets hasheados):**

```
$ diff <(assets localhost VPS) <(assets https://arbx.ape-tv.net/opportunities)
IDENTICOS: 0 diferencias   (28/28 chunks p.ej. /_next/static/chunks/1479-6e68f018730a3d13.js)
```

→ El dominio sirve EXACTAMENTE el build del contenedor (sin cacheo CF distorsionando: DYNAMIC).

**¿Sirve lo que ESTE código local compilaría? NO — prueba directa y conclusiva:**

- Request 4: `curl -s https://arbx.ape-tv.net/live-readiness` (112,806 bytes):
  - `grep -oE 'data-slot="go-no-go[^"]*"'` → **1× `go-no-go-panel`** (panel viejo, existía ya en d4d3ff63), **0× `go-no-go-signoff-card`**.
  - `grep -cE 'sign-off|Ledger generation|none persisted'` → **0**.
- El código local monta `<GoNoGoSignOffCard />` en `frontend/app/live-readiness/page.tsx:351`; el card NO tiene early-return null (renderiza su `<Card data-slot="go-no-go-signoff-card">` siempre) ⇒ si #544 estuviera desplegado, el marcador aparecería en el HTML SSR. No aparece.
- **El dominio sirve d4d3ff63; main/GitHub y el árbol local tienen 9ac06d2d. Drift = exactamente #544.**

**Identidad de build en el HTML servido:**

```
$ grep -oE '"buildId":"..."'  → 0 hits
$ grep -oiE 'buildsha|gitsha|d4d3ff63|9ac06d2d|version...'  → 0 hits
```

→ El build expone chunks content-hashed pero **ningún buildId/SHA/versión visible**. Un operador no puede verificar desde la página qué commit corre (el panel deploy del /operations consulta una API runtime, no algo "horneado" en el bundle).

---

## Drifts medidos (resumen)

1. **Live −1 PR frontend:** GitHub main 9ac06d2d vs deploy d4d3ff63 (5 archivos, +456 líneas: panel Go/No-Go A.9 ausente del dominio). Probado por HTML (0× `go-no-go-signoff-card`) y por git (objeto inexistente en repo VPS).
2. **Ref `main` local stale:** 28d48cdd (#531) — 14+ commits detrás de GitHub main; peligro de "build desde main local".
3. **CSP sin enforcing en producción pública** (report-only única política, en código+origen+dominio; `unsafe-inline`/`unsafe-eval` baked-in).
4. **Cero identidad de build expuesta** (sin SHA/versión/buildId en HTML).
5. **Árbol compartido moviéndose** entre branches por agentes concurrentes hoy (reflog 17:32/18:01/18:27) — riesgo §36.

## Riesgos

- **XSS sin mitigación CSP**: report-only significa que una inyección de script externo no se bloquea; y el flip directo a enforcing podría romper la app (unsafe-inline en string templates, sin nonce aún).
- **Deploys stale indetectables desde la UI**: este caso lo demuestra — el dominio lleva #544 sin desplegar y ninguna señal visible lo delata.
- **Acumulación de drift VPS↔GitHub**: el flujo RULE 01 exige pull en VPS; cada PR frontend nuevo quedará también sin servir hasta sincronizar.
- **Confusión de referencias locales** (main vs origin/main vs branches de agentes) para el operador y para futuros builds.

## Propuestas (what / why / priority / effort / gate)

1. **WHAT:** Hornear identidad de build en el frontend: `ARG GIT_SHA`/`ENV NEXT_PUBLIC_GIT_SHA` en el Dockerfile + `x-arbx-build-sha` header y/o meta/footer visible.
   **WHY:** hoy es imposible verificar qué commit corre (drifts 1 y 4); este round-table lo padeció.
   **PRIORITY:** P0 · **EFFORT:** S (Dockerfile + next.config headers + deploy.sh pasa SHA) · **GATE:** G5 deploy-veraz: post-deploy `curl -sI` header == `git rev-parse HEAD` en VPS (check automatizable).
2. **WHAT:** Sincronizar VPS a GitHub main (trae #544) + rebuild con RULE 03 (`--env-file .env ... build --no-cache frontend && up -d frontend`).
   **WHY:** cierra el drift 1 hoy mismo. **PRIORITY:** P1 · **EFFORT:** XS · **GATE:** L4 post-deploy + verificación `data-slot="go-no-go-signoff-card"` presente en `/live-readiness` (acción del operador; yo soy read-only).
3. **WHAT:** Flip CSP a enforcing en 2 fases: (a) duplicar header `content-security-policy` (idéntico) junto al report-only 48-72h monitoreando NEL, (b) retirar report-only; en paralelo migrar a nonce para eliminar `unsafe-inline`/`unsafe-eval`.
   **WHY:** hallazgo semilla confirmado; el comentario del propio código ("switch once clean ≥7 días") lleva meses vencido. **PRIORITY:** P1 · **EFFORT:** M · **GATE:** security-auditor + smoke e2e (Playwright) post-deploy + 0 regresiones en feed de reportes.
4. **WHAT:** Higiene del clone local: actualizar ref `main` a origin/main y anunciar congelamiento de checkout durante verificaciones (§36).
   **WHY:** drift 2/5 — "código local" debe significar una sola cosa. **PRIORITY:** P1 · **EFFORT:** XS · **GATE:** reporte al operador; sin gate técnico.
5. **WHAT:** Panel/alerta de drift de deploy en `/operations`: comparar `x-arbx-build-sha` servido vs HEAD de GitHub main y alertar si live < main.
   **WHY:** automatiza la detección del patrón de este incidente. **PRIORITY:** P2 · **EFFORT:** M · **GATE:** parity check en CI + readiness verifier existente (pr-1-csp como modelo).

## Nota de honestidad (R8)

- El vínculo imagen↔commit d4d3ff63 es inferencia con evidencia fuerte (árbol limpio en d4d3ff63, objeto 9ac06d2d ausente del repo VPS, imagen creada 22:57:45Z tras el último update de main en VPS, y HTML público sin marcadores de #544) — NO existe label de SHA en la imagen ni en el HTML para verificarlo directamente (eso ES el hallazgo 4).
- La ausencia de `go-no-go-signoff-card` en HTML público se verificó también contra el comportamiento SSR del componente local (sin early-return). No se navegó la app con navegador real (presupuesto HTTP limitado).
