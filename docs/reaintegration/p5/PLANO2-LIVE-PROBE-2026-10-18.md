# P5 · PLANO 2 (DOMINIO VIVO) — sonda de protocolo, 2026-10-18

- **HEAD auditado:** `a14dd9e6d6a8ea5220c3636dc63cc27788df8bbd` (`main`, 2026-10-18) — es el merge de #955.
- **Ejecutor:** Jefe de equipo de Hector (sandbox del coordinador). **Método:** `curl` read-only contra los hosts públicos; **cero** mutaciones. No hay acceso SSH ni `docker` desde este sandbox.
- **Alcance:** plano 2 del BLOQUE 8 (dominio vivo). El **plano 1 (repo vivo con `docker compose`) queda como deuda declarada**: `docker` no existe en este sandbox. El **plano 3 (Chromium/Playwright)** se ejecuta por separado.
- **Regla:** cada veredicto lleva su comando reproducible y la salida real pegada. Lo que no se pudo comprobar se declara `NO VERIFICABLE`, no se simula (R8).

## 1. Veredictos

| # | Check (BLOQUE 8 · plano 2) | Comando | Salida | Veredicto |
|---|---|---|---|---|
| 1 | Edge responde | `curl -s https://edge-arbx.ape-tv.net/health` | `{"ok":true,"service":"edge-worker","env":"production"}` HTTP 200 | **PASS** |
| 2 | `/api/status` con los servicios | `curl -s https://edge-arbx.ape-tv.net/api/status` | `7/7` servicios `ok:true` (selector-api, sim-ctl, recon, relays-client, searcher-rs, math-engine, token-enricher); `killswitch.enabled=false` | **PASS** |
| 3 | `/api/v1/health` | `curl -s https://edge-arbx.ape-tv.net/api/v1/health` | `system_status:"healthy"`, `math_guardian:"passed"`, `entropy:0.5837`, chains ETH/ARB/BASE/POLY/OP, latencias 2-3 ms | **PASS** |
| 4 | `/api/` llega al edge (no al origen) | `curl -s -o /dev/null -w '%{http_code}' https://arbx.ape-tv.net/api/health` | `200` (proxy al edge/api-server) | **PASS** |
| 5 | Frontend SSR | `curl -s https://arbx.ape-tv.net/` | HTTP 200, HTML renderizado en el primer byte | **PASS** |
| 6 | `/status` renderiza | `curl -s https://arbx.ape-tv.net/status` | HTTP 200, 130 994 bytes de HTML SSR | **PASS** |
| 7 | **CSP con los CDNs de logo (CSP-IMG-1)** | `curl -s -D - -o /dev/null https://arbx.ape-tv.net/status \| grep -i content-security` | `img-src 'self' data: blob: https://raw.githubusercontent.com https://assets.coingecko.com https://coin-images.coingecko.com https://cdn.dexscreener.com` | **PASS** (política correcta) — **pero** el header es `content-security-policy-report-only`: **no aplica**, solo reporta |
| 8 | `/socket.io/*` llega al api-server (WS-POLL-1) | `curl -s 'https://arbx.ape-tv.net/socket.io/?EIO=4&transport=polling'` | `0{"sid":"dSWTqMBZ0f6fGSTnAADb","upgrades":["websocket"],...}` HTTP 200 | **PASS** (en el host de la app). En `edge-arbx.ape-tv.net/socket.io/` da **404** `{"error":"not_found"}` — el worker del edge solo enruta `/api/` |
| 9 | Sin `localhost` en el bundle de producción | `for u in $(grep -o '/_next/static/chunks/[^"]*\.js' …); do curl -s … \| grep -c localhost; done` | `0` en los 6 chunks comprobados | **PASS** |
| 10 | Cabeceras de seguridad | `curl -s -D - -o /dev/null https://arbx.ape-tv.net/` | `strict-transport-security: max-age=31536000; includeSubDomains`, `x-frame-options: DENY`, `x-content-type-options: nosniff`, `referrer-policy: no-referrer`, `permissions-policy: camera=(), microphone=(), geolocation=()` | **PASS** |
| 11 | **`/status` reporta el SHA exacto del deploy** | `curl -s https://edge-arbx.ape-tv.net/api/status` | `"deploy":{"sha":"unknown","id":"unknown","at":"unknown"}`; la UI lo declara: *"unknown — stack was created by a manual `compose up` without the workflow deploy exports. The running SHA cannot be asserted from inside the stack (R8: reported verbatim)"* | **FAIL** |
| 12 | Cookies `httpOnly`/`secure`/`sameSite` | `curl -s -D - -o /dev/null https://arbx.ape-tv.net/admin/signin` | Sin `Set-Cookie` en GET no autenticado | **NO VERIFICABLE** sin sesión autenticada (deuda declarada) |

## 2. Los dos huecos, con su arreglo

1. **`deploy.sha = unknown` (FAIL del plano 2).** La app lo declara honestamente (R8) y explica la causa: el stack se creó con un `compose up` manual, sin los exports que deja el workflow de deploy. Arreglo: recrear el stack **desde el workflow** (`deploy.yml`, `workflow_dispatch` con `target_sha`) para que el SHA quede exportado y `/status` lo pueda afirmar. Es una acción de deploy (la ejecuta el responsable de deploy, con el chequeo previo del VPS).
2. **CSP en modo `report-only`.** La política es la correcta (incluye los CDNs de logo) pero no bloquea. Arreglo propuesto (paso aditivo + revert de un paso): pasar el header a `Content-Security-Policy` (enforced) **después** de una ventana sin violaciones reportadas; si aparece cualquier violación, se revierte al estado actual. Requiere un gate: "sin violaciones CSP en la ventana de observación".

## 3. Reproducir

```bash
curl -s https://edge-arbx.ape-tv.net/health
curl -s https://edge-arbx.ape-tv.net/api/status
curl -s https://edge-arbx.ape-tv.net/api/v1/health
curl -s -D - -o /dev/null https://arbx.ape-tv.net/status | grep -i 'content-security\|strict-transport\|x-frame'
curl -s 'https://arbx.ape-tv.net/socket.io/?EIO=4&transport=polling'
curl -s -o /dev/null -w '%{http_code}\n' https://arbx.ape-tv.net/api/health
```

## 4. Reconciliación con el inventario

- `/login` y `/register` del enunciado del BLOQUE 8.2 **no existen** en el repo (HTTP 404): el login real es `/admin/signin` (200) y el onboarding va por `/onboarding/1-init … 5-production`. El inventario real (70 rutas, P0) manda sobre la lista ilustrativa del enunciado.
