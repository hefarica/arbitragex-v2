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
| R8 | (Propuesta) Cualquier "fix" que tape un error con `rand`/literal/sentinel hardcodeado upstream del consumidor de datos viola RULE 00 — el error debe propagar honestamente al frontend y la causa raíz se arregla en su capa de origen. | `anti_reincidencia.md §Incidente #6` |

---

## Incidente #6: Mock Random Profit Injection en searcher-rs (RULE 00 violada en hot-path)

**Fecha del aprendizaje:** 3 de Mayo de 2026 (Sesión 3 — auditoría E2E ~14:18 UTC)

**Qué ocurrió:**
Auditoría E2E ordenada por el operador ("haz una auditoria en las oportunidades y dime si todo está llegando, bien, si los caculos, son reales y las oportunidades tambien"). R7 ejecutado capa por capa:
- ✅ Alchemy WSS detecta pending txs reales (12ms latency)
- ✅ Decoder de calldata Univ2/Univ3 produce token addresses reales
- ✅ Redis Stream `arbx:opps:detected` XLEN=2621, fluyendo
- ✅ PostgreSQL persiste (1714 rows, latest hace 7s)
- ✅ api-server + Edge Worker sirven JSON consistente
- ❌ **`expected_profit_usd` mostrado en el dashboard es `rand::thread_rng().gen_range(5.0..55.0)`**

El dashboard mostraba "Net Profit $47.44 / ROI 1776%" — números puramente aleatorios. 364 rows persistidas en última hora con 34% reportando ROI > 100% (irreal para arbs DEX), 12% con ROI = 0 contra profit > 0, 100% con `block_number = NULL`.

**Qué salió mal:**
Tres mocks encadenados:

1. `backend/prioritization-spine/src/simulator.rs:33-44` — el simulador REVM ejecuta una transacción VACÍA contra dirección dummy `0x2222...2222` con calldata vacío. Comentario en código admite: *"For now, we set up a mock transaction environment to satisfy the Spine."* Siempre devuelve PASS. Por eso el spine recibe `gross_profit = 0`.

2. `backend/searcher-rs/src/scanner.rs:243-248` — al recibir `expected_profit_usd <= 0.0` del spine, el scanner inyecta `rand::thread_rng().gen_range(5.0..55.0)` y persiste el random. Comentario admite: *"MOCK: S2 currently outputs 0.0 profit. We inject a random positive profit to bypass the NegativeProfit spine error and allow dashboard visualization."*

3. Los inputs de `OpportunityEvidence` en `scanner.rs:272-296` están todos hardcoded: `gas_units=120000`, `gas_price=30 gwei`, `bribe=0`, `flashloan_fee=0`, `token_risk_score=1.0`, `liquidity_confidence=0.9`, `landing_probability=0.95`. Y la fórmula de ROI en `scanner.rs:309` (`amount_in_f64 * 2000.0`) asume `token_in` siempre WETH.

**Causa raíz:**
El fix `254f750 fix: mock expected profit to bypass spine NegativeProfit rejection` se hizo con buena intención pero violó la doctrina: **un dato mockeado en el upstream contamina TODA la cadena downstream, incluida la UI que el operador usa para tomar decisiones**. El simulador REVM stub debería haberse priorizado o el spine debería haber persistido las opps con `Reject(NegativeProfit)` honestamente, no taparse con `rand`.

**Regla nueva (R8 propuesta):**
> Cualquier "fix" que enmascare un error de capa N inyectando datos sintéticos antes de pasarlos a capa N+1 viola RULE 00. El error debe propagar a través del pipeline (sentinelas honestas tipo `roi=-1`, `risk=0`, status="rejected") hasta el frontend, y la causa raíz se arregla en la capa donde nace, no se tapa downstream. **Lo opuesto del mock pattern: fail-honest pattern.**

**Validación obligatoria:**
- Después de eliminar el bypass, en PostgreSQL las nuevas opps deben mostrar:
  ```sql
  SELECT expected_profit_usd, roi_pct, risk_score FROM opportunities 
  WHERE detected_at > NOW() - INTERVAL '5 minutes' LIMIT 5;
  -- Esperado: profit=0.00, roi_pct=-1.00, risk_score=0.00
  ```
- En logs del searcher: `event="spine.scoring_error"` + razón `NegativeProfit` cada vez que llega una opp.
- En dashboard frontend: filas siguen apareciendo pero con profit=$0.00, ROI=-1, score=0 (lectura inmediata: "no viable").

**Archivos o rutas relacionadas:**
- `backend/searcher-rs/src/scanner.rs` (líneas 244-248 eliminadas, comentario de honestidad agregado en su lugar; commit `dc5d376`)
- `backend/prioritization-spine/src/simulator.rs` (stub a reemplazar — sub-tarea separada)
- `backend/prioritization-spine/src/scoring.rs` (lógica del spine es correcta — devuelve `Err(NegativeProfit)` cuando `gross_profit - gas_cost - bribe - flashloan_fee <= 0`)
- Plan en sesión: `~/.claude/plans/618f8807-a40cf1775f329600-js-1-uncaught-silly-rabin.md` (decisión "Cambio A solamente" del operador 14:30 UTC)

**Acción correcta en futuras ocasiones:**
Cuando un componente upstream devuelve un valor "no útil" (0, null, error), NO inventarlo. Propagar el sentinel honestamente, mostrar en UI el estado "esperando datos veraces" o "no viable", y tracear la causa raíz hasta arreglarla en la capa de origen. **Nunca persistir `rand::thread_rng()` en una tabla que el operador consulta para tomar decisiones de capital.**

**Sub-tareas pendientes (commits separados):**
- (a) Implementar `LazyRpcDatabase` real en `simulator.rs` para fetch on-chain de pool reserves (V2: `getReserves()`) y `slot0`/`liquidity` (V3); construir calldata real de swap; ejecutar contra fork del block actual; retornar `expected_amount_out` real. → desbloquea `expected_profit_usd > 0` legítimo.
- (b) Reemplazar hardcodes de `gas_units`, `gas_price`, `bribe`, `flashloan_fee` por valores derivados (eth_estimateGas, gas oracle, builder bribe model).
- (c) Reemplazar hardcodes de `token_risk_score`, `liquidity_confidence`, `landing_probability` por inputs reales (token allowlist + safety filter, TVL del pool, builder landing rate histórica).
- (d) Corregir denominador del ROI en `scanner.rs:309`: ya no asumir `token_in == WETH`; usar el precio real del token_in (Chainlink/TWAP).
- (e) Cuando `block_number IS NULL` en mempool, marcar la opp con `status="pending_block"` y actualizar tras inclusión.

**Evidencia del fix:**
- ANTES: 1714 rows con profit aleatorio $5-55, ROI hasta 5482%, dashboard "lleno" pero mendaz.
- DESPUÉS (post-deploy `dc5d376`): nuevas rows con profit=0, ROI=-1, risk_score=0; logs spine.scoring_error continuos; dashboard muestra "no viable" en cada fila — verdad operacional.
- (Nota) Las 1714 rows históricas con profit mockeado quedan en PG. `TRUNCATE opportunities` opcional para limpiar el histórico.

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
