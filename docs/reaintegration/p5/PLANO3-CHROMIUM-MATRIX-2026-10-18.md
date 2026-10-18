# P5 — Plano 3 (usuario real, Chromium/Playwright) + reconciliación del plano 2 — 2026-10-18

- **Sitio vivo:** `https://arbx.ape-tv.net` (frontend) y `https://edge-arbx.ape-tv.net` (edge).
- **Herramienta:** Playwright + Chromium headless shell 149.0.7827.55 (instalación sólo-Chromium, ~646 MB en `~/.cache/ms-playwright`).
- **Barridos:** 2, con la evidencia cruda adjunta.
  - `evidence/P5-scan-4viewports.json` — 58 rutas × 4 viewports (1440/1280/1024/390) = **232 filas**.
  - `evidence/P5-scan-freshcontext-1440.json` — 58 rutas, **contexto nuevo por ruta** (sin navegación cruzada, para no confundir abortos de prefetch con fallos reales).
- **Plano 1 (repo vivo con docker compose):** **DEUDA declarada** — `docker`/`docker-compose` no están disponibles en mi sandbox. No se simula.

## 1. Veredicto por campo

| Campo | Resultado |
|---|---|
| Rutas navegables (58 estáticas de las 59 `page.tsx`; se excluye `[entity]`) | **58/58 HTTP 200** |
| Errores de navegación (`page.goto`) | **2 timeouts de 232** (`/quant` @1280, `/paper/history` @1024); en el barrido de contexto limpio ambas responden **200** ⇒ flaky/tiempo, no rotura |
| SSR en el primer render | **230/232** filas con HTML de servidor sustantivo (`ssr_text_len ≥ 200`, marcador `next-flight` presente). Las 2 excepciones son exactamente los 2 timeouts |
| Contenido tras hidratar | **58/58** con `body_text_len ≥ 100` y elementos interactivos (`button/a/input/select` > 0) — ninguna página vacía |
| Excepciones JS no capturadas (`pageerror`) | **0** en las 58 rutas |
| HTTP ≥ 400 | **4 rutas**: 3 × 401 de API admin-gated + 1 par 400/403 de WalletConnect (§3) |
| Errores de consola | 2 causas: 401/400/403 reales + `Failed to fetch RSC payload … falling back to browser navigation` (§4) |
| Overflow horizontal 1440 / 1280 / 1024 | **0 / 0 / 1** (`/agent-insights`, 9 px) |
| Overflow horizontal **390** | **13 de 58** rutas (§5) — el peor `/live-readiness` con **915 px** |

## 2. Matriz campo por campo (58 rutas)

`st/text` = HTTP status / `document.body.innerText.length` tras hidratar. `overflow390` = `scrollWidth − clientWidth` a 390 px.

| ruta | 1440 st/text | 1280 st/text | 1024 st/text | 390 st/text | overflow390 | http>=400 (1440) |
|---|---|---|---|---|---|---|
| `/` | 200/14046 | 200/16306 | 200/17255 | 200/15543 | 0 | — |
| `/admin/chains` | 200/4521 | 200/4497 | 200/4521 | 200/3679 | 0 | — |
| `/admin/signin` | 200/1316 | 200/3952 | 200/3996 | 200/3142 | 0 | — |
| `/admin/topology` | 200/4122 | 200/4106 | 200/4045 | 200/3262 | 0 | — |
| `/agent-insights` | 200/10741 | 200/10718 | 200/10665 | 200/9882 | 0 | — |
| `/apex/allocator` | 200/4150 | 200/4155 | 200/4074 | 200/3309 | 0 | — |
| `/audit-logs` | 200/4253 | 200/4285 | 200/4224 | 200/3469 | 0 | — |
| `/chains` | 200/4072 | 200/4053 | 200/4031 | 200/3272 | 0 | — |
| `/config` | 200/6836 | 200/6822 | 200/6796 | 200/6041 | 26 | 401 GET /api/admin/rpc-backend |
| `/config/trading` | 200/6754 | 200/6756 | 200/6724 | 200/5947 | 0 | — |
| `/deploy-pipeline` | 200/15491 | 200/15487 | 200/15491 | 200/14684 | 0 | — |
| `/dex-registry` | 200/6098 | 200/6104 | 200/6074 | 200/5301 | 0 | — |
| `/executions` | 200/4266 | 200/4239 | 200/4191 | 200/3418 | 0 | — |
| `/killswitch` | 200/4492 | 200/4449 | 200/4391 | 200/3622 | 0 | — |
| `/live-readiness` | 200/15984 | 200/16002 | 200/15944 | 200/15175 | 915 | — |
| `/live-testnet` | 200/3967 | 200/3969 | 200/3927 | 200/3156 | 0 | — |
| `/monitor` | 200/5135 | 200/5166 | 200/5114 | 200/4373 | 0 | — |
| `/omega-s5/adapters` | 200/4114 | 200/4095 | 200/4133 | 200/3332 | 0 | — |
| `/omega-s5/core` | 200/4175 | 200/4167 | 200/4195 | 200/3404 | 1 | — |
| `/omega-s5/crucible` | 200/4224 | 200/4177 | 200/4211 | 200/3432 | 85 | — |
| `/omega-s5/drift` | 200/4170 | 200/4160 | 200/4204 | 200/3415 | 35 | — |
| `/omega-s5/factory` | 200/4175 | 200/4123 | 200/4139 | 200/3314 | 33 | — |
| `/omega-s5/operator` | 200/4204 | 200/4162 | 200/4178 | 200/3347 | 57 | — |
| `/omega-s5/registry` | 200/4389 | 200/4353 | 200/4357 | 200/3532 | 51 | — |
| `/omega-s5/wallets` | 200/4147 | 200/4111 | 200/4139 | 200/3290 | 0 | — |
| `/onboarding` | 200/5728 | 200/5677 | 200/5736 | 200/4903 | 0 | — |
| `/onboarding/1-init` | 200/5528 | 200/5482 | 200/5541 | 200/4714 | 29 | — |
| `/onboarding/2-connect` | 200/4883 | 200/4837 | 200/4878 | 200/4033 | 6 | — |
| `/onboarding/3-advanced` | 200/4938 | 200/4927 | 200/4968 | 200/4113 | 6 | — |
| `/onboarding/4-testing` | 200/4994 | 200/5001 | 200/2302 | 200/4179 | 6 | — |
| `/onboarding/5-production` | 200/5637 | 200/5615 | 200/5572 | 200/4775 | 6 | — |
| `/operations` | 200/7651 | 200/7642 | 200/4823 | 200/6800 | 0 | 401 GET /api/admin/archive/status |
| `/operator` | 200/7047 | 200/7068 | 200/7027 | 200/6276 | 0 | — |
| `/operator/presets` | 200/3602 | 200/6247 | 200/6278 | 200/5454 | 0 | — |
| `/operator/self-test` | 200/7078 | 200/7048 | 200/4363 | 200/6232 | 0 | — |
| `/opportunities` | 200/5096 | 200/5092 | 200/2396 | 200/4277 | 0 | — |
| `/opportunities/by-strategy` | 200/2442 | 200/2506 | 200/1213 | 200/1653 | 0 | — |
| `/opportunities/exchange` | 200/5150 | 200/5134 | 200/2386 | 200/4262 | 0 | — |
| `/opportunities/live` | 200/2424 | 200/2424 | 200/2392 | 200/4302 | 0 | — |
| `/paper/history` | 200/5006 | 200/4936 | timeout/— | 200/4207 | 0 | — |
| `/pools` | 200/18107 | 200/18131 | 200/3964 | 200/17356 | 0 | — |
| `/quant` | 200/9015 | timeout/— | 200/9797 | 200/8465 | 246 | — |
| `/readiness` | 200/5628 | 200/5627 | 200/2915 | 200/4809 | 0 | — |
| `/recon` | 200/5016 | 200/4973 | 200/5030 | 200/4183 | 0 | — |
| `/risk` | 200/5485 | 200/4440 | 200/5455 | 200/4630 | 0 | — |
| `/route-outcomes` | 200/4803 | 200/4732 | 200/4760 | 200/3935 | 0 | — |
| `/routes/discovery` | 200/13897 | 200/13805 | 200/13807 | 200/12980 | 0 | — |
| `/rpcs` | 200/4466 | 200/4408 | 200/4405 | 200/3654 | 0 | — |
| `/sed` | 200/4152 | 200/4104 | 200/4162 | 200/3345 | 0 | — |
| `/settings` | 200/4937 | 200/4885 | 200/4961 | 200/4136 | 0 | — |
| `/settings/credentials` | 200/4227 | 200/4193 | 200/4263 | 200/3420 | 0 | — |
| `/status` | 200/5323 | 200/5249 | 200/5315 | 200/4506 | 0 | — |
| `/strategies` | 200/4682 | 200/4689 | 200/4714 | 200/3863 | 0 | — |
| `/strategies/forge` | 200/29700 | 200/29514 | 200/29462 | 200/28527 | 0 | 401 GET /api/cartridges |
| `/translator` | 200/4237 | 200/4185 | 200/4181 | 200/3374 | 0 | — |
| `/wallet` | 200/3026 | 200/5725 | 200/5700 | 200/4905 | 0 | 400 POST https://pulse.walletconnect.org/e?projectId=walletconnect_pro |
| `/wallets` | 200/4306 | 200/4277 | 200/4250 | 200/3449 | 0 | — |
| `/worker-health` | 200/4053 | 200/4060 | 200/4067 | 200/3248 | 0 | — |

## 3. HTTP ≥ 400 — qué es cada uno

| Ruta | Respuesta | Lectura |
|---|---|---|
| `/config` | `401 GET /api/admin/rpc-backend` | **Correcto (fail-closed).** Endpoint admin-gated sin sesión. La página **no fabrica**: muestra `0 / 17 EFFECTIVE · source: unavailable` y el texto *"explicitly — never fabricated, never zero"* (R8 cumplido) |
| `/operations` | `401 GET /api/admin/archive/status` | Igual: admin-gated, fail-closed |
| `/strategies/forge` | `401 GET /api/cartridges` | Igual: admin-gated, fail-closed |
| `/wallet` | `400 POST https://pulse.walletconnect.org/…projectId=walletconnect_project_id_missing` y `403 GET https://api.web3modal.org/appkit/v1/config?projectId=walletconnect_project_id_missing` | **FALLO REAL — configuración incompleta.** La app llama a los endpoints de producción de WalletConnect/Reown con un **projectId placeholder** (`walletconnect_project_id_missing`). No es fail-closed de la casa: es una dependencia externa configurada con un valor de ejemplo. Requiere `NEXT_PUBLIC_WALLETCONNECT_PROJECT_ID` real o desactivar AppKit |

## 4. `Failed to fetch RSC payload … falling back to browser navigation`

Aparece en 5 rutas (`/recon`, `/sed`, `/operations`, `/route-outcomes`, `/routes/discovery`, 3 veces cada una) **incluso con contexto nuevo**, o sea que **no es artefacto de mi barrido**. La petición de prefetch (`?/…?_rsc=<hash>`) termina en `net::ERR_ABORTED` (sin status HTTP). Consecuencia medida: **ninguna** — Next cae a navegación normal y las 5 rutas cargan 200 con contenido. Se reporta como ruido de consola / prefetch abortado, no como página rota.

## 5. Overflow horizontal a 390 px (13 rutas)

| overflow px | ruta |
|---|---|
| **915** | `/live-readiness` |
| **265** | `/quant` |
| 85 | `/omega-s5/crucible` |
| 57 | `/omega-s5/operator` |
| 51 | `/omega-s5/registry` |
| 35 | `/omega-s5/drift` |
| 33 | `/omega-s5/factory` |
| 29 | `/onboarding/1-init` |
| 26 | `/config` |
| 6 | `/onboarding/2-connect`, `/onboarding/3-advanced`, `/onboarding/4-testing` |

Capturas a 390 px adjuntas: `evidence/shot-390liver.png`, `shot-390quant.png`, `shot-390crucible.png`, `shot-390wallet.png` (esta última sin overflow, tomada como control).

## 6. Reconciliación del plano 2 (no repetido)

La sonda de protocolo del Jefe de equipo (rama `REAINTEGRATION-P5-PLANO2-01` @ `e2c10537`, PR #958) ya cubre el plano 2: 11 checks → **10 PASS, 1 FAIL (`deploy.sha = unknown`), 1 no verificable (cookies sin sesión)**. No la repito. Aporto **dos lecturas que complementan** esa sonda, tomadas con `curl -D -` sobre los hosts vivos:

1. **`edge-arbx.ape-tv.net` emite DOS cabeceras `content-security-policy` contradictorias** en la misma respuesta: una `default-src 'none'; frame-ancestors 'none'` y otra `default-src 'self'; … connect-src 'self' ws: wss: http://127.0.0.1:8080 http://127.0.0.1:8787; …`. La segunda lleva **`127.0.0.1` en `connect-src`** — un localhost en una cabecera de producción. La sonda del bundle (0 `localhost` en 6 chunks) no cubre esto: el bundle está limpio, **la cabecera del edge no**.
2. **El handshake de socket.io se cierra en el host del frontend**: `GET https://arbx.ape-tv.net/socket.io/?EIO=4&transport=polling` → **200** con paquete `0{"sid":…,"upgrades":["websocket"]}`; el mismo path en `edge-arbx.ape-tv.net` → **404**. Confirma WS-POLL-1 (WS termina en api-server, nunca en el edge).
3. `edge-arbx.ape-tv.net/health` → **200** `{"ok":true,"service":"edge-worker","env":"production"}`.

## 7. FAILs, sin adornos

1. **`/wallet`** llama a WalletConnect/Reown producción con `projectId=walletconnect_project_id_missing` → 400 + 403. Configuración externa incompleta.
2. **`/live-readiness` desborda 915 px a 390 px** (y 12 rutas más desbordan). Es una página que no pinta bien en móvil.
3. **El edge sirve dos CSP contradictorias, una con `127.0.0.1`** en `connect-src`.
4. Plano 1 (docker) **no ejecutado** — deuda declarada.
5. `deploy.sha = unknown` en `/status` (heredado de la sonda del plano 2, confirmado).

## 8. Reproducir

```bash
# barrido 4 viewports (58 rutas x 1440/1280/1024/390)
cd tests/e2e && PLAYWRIGHT_BROWSERS_PATH=$HOME/.cache/ms-playwright node ../../scripts/p5/p5-live-scan.mjs
# barrido con contexto limpio por ruta (58 x 1440)
cd tests/e2e && PLAYWRIGHT_BROWSERS_PATH=$HOME/.cache/ms-playwright node ../../scripts/p5/p5-live-scan2.mjs
# cabeceras del edge (las dos CSP)
curl -s -o /dev/null -D - https://edge-arbx.ape-tv.net/ | grep -i content-security-policy
```
