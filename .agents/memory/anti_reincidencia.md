# Bitácora Anti-Reincidencia

Este archivo actúa como memoria técnica persistente para el agente. Su función es documentar los peores incidentes, los errores cometidos en la fase de resolución y las reglas operativas para prevenir futuros fracasos similares.

> **Última actualización:** 2026-05-03T08:48:00Z

---

## Incidente #1: React Hydration Cascade en el Dashboard de Producción

**Fecha del aprendizaje:** 3 de Mayo de 2026 (Sesión 1 — ~04:30 CST)

**Qué ocurrió:**
El dashboard del operador en `http://195.201.235.70:5173/opportunities` presentaba una cascada de errores React (#425, #418, #423). La aplicación caía de SSR a un costoso Client-Side Rendering completo. La interfaz parpadeaba y mostraba overlays de error.

**Qué salió mal:**
1. El componente `opportunities/page.tsx` evaluaba `Date.now()`, estado de WebSocket y URLs resueltas por entorno directamente en el ciclo de renderizado.
2. El componente `SiteHeader` en `layout.tsx` llamaba a `getApiBaseUrl()`, que resolvía a `http://edge:8787` en servidor y `https://edge-arbx.ape-tv.net` en cliente.
3. El despliegue al VPS se hizo sin `--env-file .env`, causando que `NEXT_PUBLIC_EDGE_URL` cayera al fallback `http://localhost:8787`.

**Causa raíz:**
Violación sistemática del contrato de hidratación SSR-CSR de React/Next.js. El HTML generado en servidor no coincidía con el primer render del cliente en múltiples puntos.

**Regla nueva para prevenirlo:**
- **Regla Inmutable de Hidratación (Cero Mismatch):** Server Component puro para snapshot, Client Component con `useState(initialSnapshot)`, toda mutación dinámica dentro de `useEffect`.
- **Compilación Hermética:** Todo build de Docker para frontend DEBE usar `--no-cache --env-file .env`.

**Validación obligatoria:**
Después de todo cambio en la interfaz, usar `browser_subagent` para visitar la URL pública y confirmar cero errores de hidratación en la consola.

**Archivos o rutas relacionadas:**
- `frontend/app/opportunities/page.tsx`
- `frontend/app/opportunities/OpportunitiesClient.tsx`
- `frontend/components/site-header.tsx`
- `docker/compose.dev.yml`

**Acción correcta en futuras ocasiones:**
Aplicar partición estricta Server/Client (Mounted Snapshot Pattern) y siempre usar `--no-cache` con `--env-file .env`.

---

## Incidente #2: Build-Time Guard y Fuga de Variables de Entorno

**Fecha del aprendizaje:** 3 de Mayo de 2026 (Sesión 2 — ~03:15 CST)

**Qué ocurrió:**
A pesar de haber corregido la hidratación en el Incidente #1, se identificó que el sistema seguía siendo vulnerable a reincidencia si un operador olvidaba pasar `--env-file .env` durante un rebuild futuro. La imagen de Docker se construiría con `localhost:8787` embebido en los chunks estáticos de Next.js, rompiendo silenciosamente la producción.

**Qué salió mal:**
No existía ninguna protección a nivel de build que impidiera la generación de una imagen defectuosa. El error solo era visible DESPUÉS de desplegar y abrir el navegador.

**Causa raíz:**
`docker/compose.dev.yml` define fallbacks de desarrollo (`${NEXT_PUBLIC_EDGE_URL:-http://localhost:8787}`) que son seguros para desarrollo local pero letales si se filtran al build de producción.

**Regla nueva para prevenirlo:**
Inyección de un **Build-Time Guard** en `frontend/next.config.js` que aborta `next build` si detecta localhost en `NEXT_PUBLIC_EDGE_URL` cuando `NODE_ENV=production`:
```javascript
if (process.env.NODE_ENV === "production") {
  if (EDGE_URL && /localhost|127\.0\.0\.1|0\.0\.0\.0/.test(EDGE_URL)) {
    throw new Error(`[CRITICAL] next build failed: NEXT_PUBLIC_EDGE_URL cannot point to localhost.`);
  }
}
```

**Validación obligatoria:**
- El guard en `next.config.js` es **inmutable**. No se puede comentar ni remover.
- Verificar su existencia antes de cada despliegue con `grep "CRITICAL.*localhost" frontend/next.config.js`.

**Archivos o rutas relacionadas:**
- `frontend/next.config.js` (líneas 8-13)

**Acción correcta en futuras ocasiones:**
Si un build de Docker falla con "[CRITICAL] next build failed", significa que las variables de entorno no fueron pasadas. Solución: agregar `--env-file .env` al comando de build. **Nunca** remover el guard.

---

## Incidente #3: WebSocket Upgrade Proxy No Completaba Handshake

**Fecha del aprendizaje:** 3 de Mayo de 2026 (Sesión 2 — ~03:20 CST)

**Qué ocurrió:**
El dashboard mostraba `feedStatus = "POLLING"` permanentemente en lugar de `"LIVE"`. Socket.IO degradaba a HTTP long-polling con 5 segundos de latencia en lugar de usar WebSocket nativo (sub-segundo). Los logs del `api-server` mostraban conexiones que se establecían y desconectaban inmediatamente en ciclos rápidos.

**Qué salió mal:**
Dos errores en `edge/dev-local/src/index.ts`:
1. El middleware `http-proxy-middleware` con `ws: true` se montaba en Express, pero Express **no re-emite** eventos HTTP `Upgrade` automáticamente. Sin `server.on('upgrade', wsProxy.upgrade)`, el handshake WebSocket nunca se completaba. El cliente recibía HTTP 400 en lugar de HTTP 101 Switching Protocols.
2. La configuración incluía `pathRewrite: { '^/': '/socket.io/' }`. Dado que el middleware ya estaba montado en `/socket.io`, la petición upstream llegaba como `/socket.io/socket.io/...` (ruta duplicada), lo que causaba un rechazo silencioso.

**Causa raíz:**
Desconocimiento de que `http-proxy-middleware` en modo Express requiere binding manual del evento `upgrade` al servidor HTTP. La documentación de Express no advierte que `Upgrade` headers son ignorados por el middleware stack normal.

**Regla nueva para prevenirlo (R4 — WebSocket Proxy Upgrade Binding):**
```typescript
// 1. Guardar instancia
const wsProxy = createProxyMiddleware({ target, ws: true, changeOrigin: true });
// 2. Montar en express
app.use('/socket.io', wsProxy);
// 3. Crear servidor HTTP (NO app.listen)
const server = app.listen(PORT, () => { ... });
// 4. Ligar upgrade
server.on('upgrade', wsProxy.upgrade);
// 5. NO usar pathRewrite si la ruta de montaje coincide con la upstream
```

**Validación obligatoria:**
- Después de modificar el proxy WebSocket, verificar en logs de `api-server` que las conexiones WebSocket se mantienen estables (no ciclos connect/disconnect rápidos).
- Verificar en el dashboard que el badge muestre `LIVE` o al menos `POLLING` sin errores de red.

**Archivos o rutas relacionadas:**
- `edge/dev-local/src/index.ts` (líneas 112-120 y 303-309)
- `edge/dev-local/src/audit-emit.ts` (importación de `auditEmitFailedTotal` removida por error de build)

**Acción correcta en futuras ocasiones:**
Al trabajar con proxies WebSocket en Express, siempre verificar que el evento `upgrade` esté ligado al servidor HTTP. Usar `docker logs <container>` para confirmar estabilidad de conexiones antes de declarar la corrección exitosa.

---

## Incidente #4: TypeScript Build Failure por Import Fantasma

**Fecha del aprendizaje:** 3 de Mayo de 2026 (Sesión 2 — ~03:18 CST)

**Qué ocurrió:**
Al intentar compilar `@arbx/edge-dev-local` después del fix de WebSocket, `tsc` falló con:
```
src/audit-emit.ts(14,10): error TS2305: Module '"@arbx/shared"' has no exported member 'auditEmitFailedTotal'.
```

**Qué salió mal:**
El archivo `audit-emit.ts` importaba `auditEmitFailedTotal` de `@arbx/shared`, pero ese símbolo nunca fue exportado por el paquete compartido. El código funcionaba en desarrollo porque `tsx` no hace type-checking, pero el build de producción (`tsc`) sí lo detecta.

**Causa raíz:**
Código escrito anticipando una exportación futura que nunca se materializó. El error estaba latente y solo se manifestó al recompilar el paquete.

**Regla nueva para prevenirlo:**
- Antes de hacer push de cualquier cambio, ejecutar `npm run build -w <workspace>` para verificar que TypeScript compila limpio.
- Nunca asumir que un import existe sin verificar las exportaciones del paquete fuente.

**Validación obligatoria:**
- `npm run build -w @arbx/edge-dev-local` → exit code 0.

**Archivos o rutas relacionadas:**
- `edge/dev-local/src/audit-emit.ts`
- `shared-ts/src/index.ts`

**Acción correcta en futuras ocasiones:**
Ejecutar build del workspace afectado ANTES de hacer commit/push. Si falla, resolver el error de tipos antes de desplegar.

---

## Resumen de Reglas Inmutables Acumuladas

| ID | Regla | Archivo de Referencia |
|----|-------|-----------------------|
| R1 | Cero Mismatch: Mounted Snapshot Pattern obligatorio | `SKILL.md` §5.R1 |
| R2 | Build-Time Guard: `next.config.js` aborta si localhost en prod | `SKILL.md` §5.R2 |
| R3 | Deploy con `--no-cache --env-file .env` siempre | `SKILL.md` §5.R3 |
| R4 | WebSocket: `server.on('upgrade', wsProxy.upgrade)` obligatorio | `SKILL.md` §5.R4 |
| R5 | Auditoría de componentes transitivos en toda corrección de mismatch | `SKILL.md` §5.R5 |
| R6 | Todo servicio productor DEBE tener `DATABASE_URL` + `depends_on: postgres` | `SKILL.md` §5.R6 |
| R7 | Trazabilidad E2E: auditoría capa-por-capa cuando datos no llegan al Dashboard | `SKILL.md` §5.R7 |

---

## Incidente #5: Pipeline E2E Roto — searcher-rs sin DATABASE_URL (824+ oportunidades perdidas)

**Fecha del aprendizaje:** 3 de Mayo de 2026 (Sesión 2 — ~03:37 CST)

**Qué ocurrió:**
El Dashboard en `/opportunities` mostraba solo 3 oportunidades viejas del 2 de Mayo. El usuario reportó que "no salen nuevas oportunidades". Se ejecutó una auditoría end-to-end completa de las 11 capas del pipeline: Alchemy WSS → searcher-rs → Pattern Match → REVM Sim → Scoring Engine → Redis XADD → PostgreSQL INSERT → api-server → edge → frontend.

**Qué salió mal:**
El servicio `searcher-rs` en `docker/compose.dev.yml` (líneas 113-127) definía:
```yaml
environment:
  ARBX_CONFIG_PATH: /app/configs/app.toml
  REDIS_URL: redis://redis:6379
  SEARCHER_HEALTH_PORT: "9001"
  # ← DATABASE_URL AUSENTE
```
Sin `DATABASE_URL`, el `main.rs` inicializaba `db_pool = None`. En `scanner.rs` línea 346:
```rust
if let Some(pool) = db {  // db = None → bloque NUNCA se ejecuta
    persistence::insert_opportunity(pool, &opportunity).await;
}
```
Las capas 1-7 funcionaban perfectamente: Alchemy entregaba txs, el scanner detectaba, REVM simulaba, el scoring puntuaba y Redis Stream acumulaba 827+ entries. Pero PostgreSQL solo tenía 3 rows del día anterior. El frontend lee de PostgreSQL via api-server, no de Redis Stream.

**Causa raíz:**
Omisión de `DATABASE_URL` en la sección `environment` de `searcher-rs` dentro de `compose.dev.yml`. El código de `main.rs` trata la conexión a PostgreSQL como opcional (línea 64-68: `warn!("DATABASE_URL not set; scanner will publish to stream but NOT persist")`), lo cual es correcto para desarrollo local pero catastrófico en producción.

**Agravante:** El único indicio era un `WARN` en el arranque del contenedor que se perdía entre miles de líneas de `PoolSyncWorker`. No había ningún `ERROR` ni fallo visible.

**Regla nueva para prevenirlo (R6 — Completitud de Variables en Docker Compose):**
- Todo servicio backend que produzca datos para el Dashboard DEBE tener `DATABASE_URL` en `compose.dev.yml`.
- DEBE tener `depends_on: postgres: { condition: service_healthy }`.
- DEBE verificarse al arranque que el log muestre `"db.connected"` o equivalente.

**Regla nueva para diagnosticar (R7 — Trazabilidad E2E):**
Cuando el Dashboard no muestra datos, ejecutar auditoría capa por capa:
1. `docker logs searcher-rs | grep simulator.success` (detecta?)
2. `redis-cli XLEN arbx:opps:detected` (Redis recibe?)
3. `psql -c 'SELECT MAX(detected_at) FROM opportunities'` (PG recibe?)
4. `curl localhost:8787/api/opportunities/live` (API sirve?)

**Validación obligatoria:**
- Después de agregar `DATABASE_URL` y reiniciar: ejecutar `SELECT COUNT(*) FROM opportunities` y confirmar que el número crece.
- Confirmar en logs de arranque: `"postgres pool up", event: "db.connected"`.
- Verificar en el Dashboard que aparecen oportunidades con `detected_at` reciente.

**Archivos o rutas relacionadas:**
- `docker/compose.dev.yml` (servicio `searcher-rs`, líneas 113-130)
- `backend/searcher-rs/src/main.rs` (líneas 51-68: `DATABASE_URL` handling)
- `backend/searcher-rs/src/scanner.rs` (línea 346: `if let Some(pool) = db`)
- `backend/searcher-rs/src/publisher.rs` (Redis XADD a `arbx:opps:detected`)
- `backend/searcher-rs/src/persistence.rs` (PostgreSQL INSERT)

**Acción correcta en futuras ocasiones:**
Antes de declarar que el pipeline está operativo, SIEMPRE ejecutar la secuencia R7 completa. Si Redis tiene datos pero PG no, revisar `DATABASE_URL` en el servicio productor. Confirmar con `SELECT COUNT(*)` que las filas crecen.

**Evidencia del fix:**
- Antes: 3 rows en PG, última del 2 de Mayo 23:05 UTC.
- Después: 10+ rows y creciendo, última de hace segundos.
- Endpoint `/api/opportunities/live`: `{ "count": 10, "items": [...] }` con datos frescos.
