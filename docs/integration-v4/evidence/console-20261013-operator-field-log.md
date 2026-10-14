# Evidencia de campo — consola del operador (producción, binario previo deploy.sha 5f8ea3d6)

**Provenance**: pegado directo del operador desde https://arbx.ape-tv.net/opportunities (sesión de capitán, 2026-10-13). La consola del navegador NO incluye timestamps — DECLARADO: la correlación temporal (M2) requiere captura futura con timestamps; esta evidencia sirve para EXISTENCIA y FRECUENCIA del storm, no para correlación.

## Extracto verbatim — storm de desconexión (evidencia ℳ para ADM watchdog / "ADM-7")

```
page-06a05188808161c2.js:1 [PricesStream] silent >90000ms — forcing reconnect (1/3)
```
Ocurrencias: **6** (una por bloque de navegación; contador SIEMPRE en 1/3 — cada recarga de página reinicia el contador)

```
7093-d29c40cfaf826d9d.js:1 WebSocket is already in CLOSING or CLOSED state.
```
Ocurrencias: **1** (stack: doWrite → send → flush → _sendPacket — escritura a socket muerto)

```
opportunities:1 WebSocket connection to 'wss://arbx.ape-tv.net/socket.io/?EIO=4&transport=websocket[&sid=...]' failed: Page entered Back-Forward Cache.
```
Ocurrencias: **~14** (artefacto bfcache del navegador — consecuencia, no causa)

## Extracto verbatim — cluster 500 (control-plane, ventana sin timestamp)

```
page-06a05188808161c2.js:1  GET https://arbx.ape-tv.net/api/paper-mode/state 500 (Internal Server Error)   [×2]
305-e9938a2a0f3228e3.js:1  GET https://arbx.ape-tv.net/api/readiness/decision 500                          [×1]
305-e9938a2a0f3228e3.js:1  GET https://arbx.ape-tv.net/api/readiness/steps 500                              [×1]
305-e9938a2a0f3228e3.js:1  GET https://arbx.ape-tv.net/api/strategies/runtime-status?chain_id=1 500          [×1]
305-e9938a2a0f3228e3.js:1  GET https://arbx.ape-tv.net/api/readiness 500                                     [×1]
305-e9938a2a0f3228e3.js:1  GET https://arbx.ape-tv.net/api/scanner/heartbeat?chain_id=1 500                  [×1]
page-06a05188808161c2.js:1  GET https://edge-arbx.ape-tv.net/api/opportunities/live?viable_only=false&limit=50&max_age_seconds=300&order=profit_usd&route_representative=best_net 500  [×1]
```

## Extracto verbatim — simulate

```
api/v1/opportunities/3d5d90b6-da4f-4e9b-a18a-29ede1c62f79/simulate:1 Failed to load resource: the server responded with a status of 501 ()   [×3]
api/v1/opportunities/3d5d90b6-da4f-4e9b-a18a-29ede1c62f79/simulate:1 Failed to load resource: the server responded with a status of 503 ()   [×1]
```
Nota t28: con oportunidad VIVA, POST canónico reproduce `501 real_sim_unavailable: "SIM_BACKEND!=revm — real multi-step REVM path requires SIM_BACKEND=revm"` (fail-honest determinista por config). El 503×1 no reprodujo.

## Clasificación pendiente del operador (C1-C4)
- C1: baseline t16 n=20 ⇒ CI95 ~14% — "no estaba en la baseline" es CLAIM hasta M1 (n≥200/endpoint)
- C2: 500 ↔ reconnects probablemente UN evento (correlación ±10s pendiente — esta evidencia no tiene timestamps)
- C3: 501 ≠ expiry (expiry da 404/410); 501 = no-implementado determinista — M5 decide ruta vs método
- C4: la verificación post-rollback fue liveness no rutas — defecto de diseño §B a corregir (smoke de rutas)
