---
name: arbx-api-server-route-mounting
description: Crear y montar rutas en backend/api-server con Express sin romper index.ts.
---
# arbx-api-server-route-mounting

## Purpose
Crear de forma segura, acoplada y predecible rutas Express en el `api-server`, aislando la lógica en su archivo dedicado y conectándola apropiadamente en el enrutador principal (`index.ts`).

## When to use
Siempre que necesites exponer una nueva funcionalidad de lectura o escritura en el backend `api-server` (ej. GET `/api/v1/strategies/runtime-status`).

## Inputs needed
- URI de ruta completa en `/api/v1/...`.
- Módulo destino (ej. `backend/api-server/src/routes/status.ts`).
- Objecto de dependencias para el handler (`pool`, `redis`, `logger`).

## Files usually touched
- `backend/api-server/src/routes/*.ts`
- `backend/api-server/src/index.ts`

## Commands
- `pnpm --filter api-server build`
- `pnpm --filter api-server start` (o invocación directa a Node)

## Safety rules
- Utilizar Inyección de Dependencias en la inicialización (e.g. `mountRoute(app, deps)`).
- Validar `pool === null` antes de interrogar BD, retornando status 503 (`db_unavailable`).
- No propagar pánicos no manejados en Redis (`redis === null` o error de comandos). Si una dependencia secundaria falla, marcar campo afectado como `null` y responder 200/206 Partial.

## Verification steps
1. Escribe la firma del módulo con Inyección de Dependencias.
2. Inyéctalo correctamente en `index.ts`.
3. Valida compilación sin advertencias de Typescript (`pnpm build`).
4. Haz `curl` a localhost y evalúa el comportamiento normal y la resiliencia cerrando DB/Redis.

## Failure modes
- Referencias estáticas inseguras a conexiones de base de datos que crashean en el momento de request.
- Ausencia de clausuras de Error Boundaries en handlers, tumbando el proceso de Node con UnhandledPromiseRejection.

## Golden output
```typescript
export function mountStrategyRuntimeStatus(app: Application, deps: { pool: Pool | null; redis: Redis | null; logger: any }) {
  app.get("/api/v1/strategies/runtime-status", async (req, res) => {
    // Validar deps, ejecutar query, devolver 200
  });
}
```

## Anti-patterns
- Endpoints anónimos y masivos directamente hardcodeados dentro de `index.ts`.
- Capturas de excepciones silenciosas que ignoran métricas o el logger y devuelven 200 OK malformados.

## Example prompt
"Emplea arbx-api-server-route-mounting para montar la ruta runtime-status en index.ts asegurando que responda 503 si no hay Pool."
