# API-JSON-01 — `/api/*` no negocia contenido

**Decisión (2026-10-18):** eximir el prefijo `/api/` de `statusWantsJson()`. La
negociación sigue aplicando al resto de rutas (en particular a la PÁGINA `/status`).
**Gate:** `scripts/gates/g-api-json-01.sh`.

## El defecto, medido

Antes del arreglo, sobre los dos hosts vivos:

| petición | resultado |
|---|---|
| `/api/status` con UA de navegador + `Accept: text/html` | **404** `{"error":"not_found","detail":"html_served_by_spa"}` |
| `/api/status` con UA de CLI (`curl`) | **200** JSON, 7/7 upstreams |
| `/status` (página) con UA de navegador | 200 `text/html` (la SPA) |

Por qué importa: el **StatusClient del navegador** sondea `/api/status` atravesando
el rewrite `/api/*` de Next, así que envía UA de Chrome y `Accept: text/html`.
Recibía un 404 mientras una sonda de CLI sobre la MISMA URL recibía 200 — es decir,
la superficie parecía sana desde la terminal y rota desde la página.

Causa: la negociación se escribió para la ruta de **página** `/status` (que
colisiona con la página `/status` de Next, motivo por el que existe el alias
`/api/status`) y se **reutilizó** para el alias de API, donde no tiene sentido: un
path `/api/*` es una superficie de API por construcción.

## El arreglo

En `edge/worker/src/index.ts`, una función explícita en vez de repetir la decisión
en cada ruta:

```ts
export function pathWantsJson(c): boolean {
  if (new URL(c.req.url).pathname.startsWith("/api/")) return true;
  return statusWantsJson(c);
}
```

`/status` y `/api/status` usan ahora `pathWantsJson`; para `/status` el path no
empieza por `/api/`, así que su comportamiento es **idéntico** al de antes (lo
verifica el propio test). Sólo el prefijo `/api/` está exento: `/apiary` o
`/status/api` NO lo están (probado).

## La pista que confirmó el diagnóstico

`edge/dev-local/src/index.ts` —el espejo local del worker— **ya lo hacía bien**:

```ts
// ... Always proxy to api-server /status (no SPA fallback for /api/*).
// Mirrors edge/worker/src/index.ts (Cloudflare Hono).
app.get("/api/status", (req, res) => proxy("/status", req, res));
```

Su propio comentario dice que **espeja** el worker de Cloudflare, y no era verdad.
Este cambio hace el espejo real en vez de aspiracional: el espejo local era el
correcto y el de producción el que divergía.

## Evidencia

- **Tests**: `edge/worker` → **16 pasan, 0 fallan** (`npx vitest run`), incluidos 6
  nuevos en `src/index.test.ts` que cubren: el poll del navegador a `/api/status`,
  cualquier `/api/*` con UA/Accept de navegador, la negociación intacta de
  `/status` (navegador → HTML, CLI → JSON, `?format=json` → JSON), y que
  `/apiary`/`/status/api` NO están exentos.
- **Typecheck**: `tsc --noEmit -p tsconfig.json` → PASS.
- **Gate `G-API-JSON-01`**: falla HOY (404 en ambos hosts) y pasará en cuanto se
  despliegue — su salida antes del deploy queda registrada como prueba de que el
  gate detecta el defecto en vez de decorarlo.

## Revert

Un paso: revertir el commit devuelve `pathWantsJson` a su ausencia y ambas rutas a
`statusWantsJson` (comportamiento original).
