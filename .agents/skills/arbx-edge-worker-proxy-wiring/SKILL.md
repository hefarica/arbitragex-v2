---
name: arbx-edge-worker-proxy-wiring
description: Agregar rutas al Cloudflare Worker sin romper el proxy, cache ni headers.
---
# arbx-edge-worker-proxy-wiring

## Purpose
Agregar rutas al Cloudflare Worker de ArbitrageX sin romper el proxy subyacente, headers de autenticación, ni la dirección real de destino (`API_SERVER_URL`).

## When to use
Cuando un nuevo endpoint (ej. `runtime-status`) se ha expuesto en el `api-server` (puerto 8080) y necesita ser consumible por el frontend público a través de `https://edge-arbx.ape-tv.net`.

## Inputs needed
- Ruta pública deseada.
- Ruta interna real en `api-server`.
- TTL de caché en segundos según el dinamismo del endpoint.

## Files usually touched
- `edge/worker/src/index.ts`

## Commands
- `pnpm --filter edge-worker run dev`
- `curl -s "https://edge-arbx.ape-tv.net/api/strategies/runtime-status" | jq`

## Safety rules
- Nunca exponer puertos internos públicos directamente a 0.0.0.0. Todo pasa por el Worker/Proxy y Cloudflare Tunnel.
- No hardcodear el host del `api-server` en el código de ruta, debes confiar en la inyección de `env.API_SERVER_URL`.
- No inventar mapeos en `/api` que no tengan un endpoint correspondiente montado y funcional en `api-server`.

## Verification steps
1. Modifica `edge/worker/src/index.ts` agregando `app.get(...)` y usando `proxy(...)`.
2. Haz deploy o prueba en local mediante wrangler (`npm run start`).
3. Llama a la URL de Cloudflare Edge y comprueba el passthrough exacto y coherente.

## Failure modes
- Headers CORS rotos impidiendo el render desde Next.js.
- Fallo de caché de persistencia rápida debido a TTL omitido o mal configurado.
- Error 502 por proxy intentando alcanzar un destino caído u obsoleto.

## Golden output
```typescript
app.get("/api/strategies/runtime-status", (c) => proxy(c, "/api/v1/strategies/runtime-status", "arbx:cache:status", 5));
```

## Anti-patterns
- Reescribir la función `fetch()` cruda por cada endpoint en vez de usar el middleware o utilidad `proxy()` existente.
- Tocar archivos no existentes o deprecados (ej. `frontend/edge/src/index.ts`).

## Example prompt
"Usa arbx-edge-worker-proxy-wiring para exponer /api/v1/strategies/runtime-status del api-server en el worker bajo /api/strategies/runtime-status con 5 seg de cache."
