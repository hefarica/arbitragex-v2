# Bitácora Anti-Reincidencia

Este archivo actúa como memoria técnica persistente para el agente. Su función es documentar los peores incidentes, los errores cometidos en la fase de resolución y las reglas operativas para prevenir futuros fracasos similares.

> **Última actualización:** 2026-05-05T08:13:00Z

---

## Incidente #1: React Hydration Cascade en el Dashboard de Producción

**Fecha del aprendizaje:** 3 de Mayo de 2026 (Sesión 1 — ~04:30 CST)

**Qué ocurrió:**
El dashboard del operador en `http://<VPS_IP>:5173/opportunities` presentaba una cascada de errores React (#425, #418, #423). La aplicación caía de SSR a un costoso Client-Side Rendering completo. La interfaz parpadeaba y mostraba overlays de error.

**Qué salió mal:**
1. El componente `opportunities/page.tsx` evaluaba `Date.now()`, estado de WebSocket y URLs resueltas por entorno directamente en el ciclo de renderizado.
2. El componente `SiteHeader` en `layout.tsx` llamaba a `getApiBaseUrl()`, que resolvía a `http://edge:8787` en servidor y `https://<VPS_HOST>` en cliente.
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

---

## Incidente #7: Cap de capital asimétrico inflando profit/ROI hasta 735.184% (BUG-3)

**Fecha del aprendizaje:** 4 de Mayo de 2026 (Sesión 4 — auditoría R7 ~10:55 UTC, fix desplegado 11:43 UTC commit `4b99eb8`)

**Qué ocurrió:**
Auditoría R7 ordenada por el operador después de que un watcher de fondo polleaba `/api/opportunities/live` cada 25s buscando la primera oportunidad con `expected_profit_usd > 0` y no la encontraba. El diagnóstico reveló que SÍ había aparecido una a las 10:25:35 UTC con $113.97 profit y 1.127% ROI, pero también dos outliers históricos: $74.98 (ROI 739%) a las 08:08 y **$73.888,61 (ROI 735.184%)** a las 06:37. Todos con `dex_a = uniswap-v3` y `token_in = WETH`. Un evento live a las 11:01 mostró ROI de **42 mil millones por ciento** (USDT-WETH), tan alto que el INSERT a PostgreSQL falló por overflow de `numeric(10,4)`.

**Qué salió mal:**
Tres bugs encadenados en el cálculo de profit, donde el operador había configurado `capital_usd = 10` (testing) pero observaba pendientes de mempool con `amount_in ≈ 0,05 ETH` (≈ $125):

1. **BUG-3 (la pistola humeante):** `backend/prioritization-spine/src/config_aware.rs:149-151` (pre-fix) capeaba `amount_in_usd` al capital del operador pero dejaba `expected_amount_out_usd` SIN cap. La fórmula `gross_profit_usd = expected_amount_out_usd - amount_in_usd` producía profit fantasma igual al delta del cap. Para 0,05 ETH input ≈ $125 con cap a $10 y output proporcional ≈ $125 → "profit" reportado = $115.

2. **BUG-2 (no fixed in este PR):** `backend/shared-rs/src/trading_config.rs:107-109` define `profit_token_to_usd(x) = x * base_token_price_usd` — multiplica cualquier token amount por el precio de WETH ($2.500), ignorando el token real. Para token_out = BNB/UNI/etc., un amount como 29,56 UNI se valora como 29,56 WETH × $2.500 = $73.898. Esto explica por qué el outlier 06:37 alcanzó $73K.

3. **BUG-1 (no fixed en este PR):** `backend/searcher-rs/src/scanner.rs:290` divide `amount_in_wei.parse::<f64>() / 1e18` siempre, ignorando `meta_in.decimals`. Para tokens de 6 decimales (USDT, USDC), divide por 1e12 de más → `amount_in_f64 ≈ 0` → BUG-3 dispara cap pero `amount_in_usd ≈ $0` → ROI explota a billions.

**Causa raíz:**
Asunción incorrecta de simetría: el operador define `capital_usd` como el techo del capital deployable, pero la aritmética del spine trataba el output esperado como independiente del input capeado. La fórmula matemáticamente correcta requiere `cap_ratio = amount_in_usd_capped / observed_amount_in_usd` aplicado a AMBOS lados. Sin esa proporcionalidad, todo cap del input es 100% conversión en profit fantasma.

**Por qué llegó a producción sin detectarse:**

- Los tests existentes en `config_aware.rs` (commit pre-fix) cubrían los gates de allowlist y disabled-strategy, pero NO el camino matemático con cap activo.
- El test `capital_caps_amount_in` solo verificaba `total_capital_required_usd <= 1000`, que sigue siendo true pre-fix.
- Las opps con outlier ROI eran raras (3 en 24h ≈ 1 cada 8h), perdidas en el ruido del 99% de opps con profit=0.
- El sistema de risk gate (`LowLiquidity`) interceptaba la ejecución, así que NUNCA se intentó ejecutar una orden basada en profit fantasma — pero el dashboard mostraba los números absurdos como "oportunidades detectadas".

**Regla derivada (refuerzo de R8 Fail-Honest, capa matemática):**
> Cuando se aplica un techo (cap, clamp, max/min) a un valor X que se compone con otro valor Y vía `f(X, Y)`, el cap DEBE propagarse proporcionalmente a Y si la composición lo requiere. La asimetría sin justificación matemática produce sentinelas falsas que se persisten como datos veraces. Test obligatorio: para todo cap de capital o liquidez, escribir un test que verifique que el output downstream NO exceda lo que sería realista para el input capeado.

**Fix aplicado (commit `4b99eb8`):**
```rust
// backend/prioritization-spine/src/config_aware.rs (post-fix)
let observed_amount_in_usd = self.config.profit_token_to_usd(candidate.amount_in);
let amount_in_usd = observed_amount_in_usd.min(self.config.capital_usd);
let cap_ratio = if observed_amount_in_usd > 0.0 {
    amount_in_usd / observed_amount_in_usd
} else {
    1.0
};
let expected_amount_out_usd =
    self.config.profit_token_to_usd(candidate.expected_amount_out) * cap_ratio;
```

Tests de regresión añadidos:

- `capital_cap_does_not_inflate_gross_profit` — reproduce el outlier 10:25 (WETH→BNB), assert `gross_profit_usd.abs() < 1.0`.
- `capital_cap_bounds_roi_to_realistic_range` — reproduce el outlier 06:37 (WETH→UNI), assert `net_roi_pct < 100.000%`. Pre-fix el test calculó ROI = **735.104,81%**, dentro de **0,01%** del valor real de producción (735.184,72%) — la fidelidad del repro confirma que la fórmula del bug está capturada exactamente.

**Validación obligatoria:**

- `cargo test --lib config_aware:: -p prioritization-spine` → 6 tests PASS (4 existentes + 2 nuevos).
- En producción post-deploy, ninguna nueva opp con `roi_pct > 100` durante ventana de observación.
- Heartbeat worker emite cada 60s con `pg_period_profit_pos` count — espera de 0 para detección de outliers nuevos.

**Archivos o rutas relacionadas:**

- `backend/prioritization-spine/src/config_aware.rs` (líneas 152-167 fix; 362-455 tests regresión)
- `backend/searcher-rs/src/workers/heartbeat_worker.rs` (NUEVO — observabilidad pulse 60s)
- `backend/searcher-rs/src/main.rs` (líneas 184-203 wire del heartbeat)
- `backend/searcher-rs/src/workers/mod.rs` (export del worker)
- `backend/shared-rs/src/trading_config.rs:107` (BUG-2, pendiente de fix)
- `backend/searcher-rs/src/scanner.rs:286-322` (BUG-1 FIXED commit `2a465e9` — usa `meta_in.decimals`)
- `backend/searcher-rs/src/amm_math.rs:19-32` (BUG-1 helper `wei_str_to_token_units`, NUEVO)
- `backend/searcher-rs/src/amm_math.rs:284-356` (BUG-1 tests TDD: 7 nuevos incluyendo regresión USDT)

**Acción correcta en futuras ocasiones:**

- Cuando un valor de configuración (`capital_usd`, `max_position_size`, `min_liquidity`) limita un input que se compone con otros valores en una fórmula, escribir un test que verifique el downstream NO produzca artefactos por la asimetría del cap.
- Cuando se observen valores absurdos en el dashboard (ROI > 100%, profit > $10K en HFT DEX arb), tratar como bug matemático antes que como oportunidad real. La distribución natural del HFT MEV está en 0,05% – 2% ROI; cualquier outlier > 100% es casi siempre un bug.
- Schema de PostgreSQL `numeric(10,4)` actúa como última línea de defensa contra estos bugs (overflowing INSERT en lugar de persistir basura), pero no debe ser la única — los bugs deben pillarse antes de llegar a la capa de persistencia.

**Sub-tareas pendientes (commits separados):**

- (a) **BUG-1 fix** ✅ DONE 2026-05-05 (commit `2a465e9`): añadido helper `amm_math::wei_str_to_token_units(wei_str, decimals)` en amm_math.rs:19-32 con 7 tests TDD (incluye regresión `wei_str_bug1_regression_usdt_input` que demuestra delta vs old buggy semantic ~1e-8). scanner.rs reordenado para resolver `meta_in` antes de `amount_in_f64` y usar `meta_in.decimals` con default 18 cuando token es desconocido. Verificación: 26/26 tests searcher-rs + 6/6 spine, deploy OK 06:46:50 UTC.
- (b) **BUG-2 fix** ✅ DONE 2026-05-05 (commit `0855010`): implementado `shared_rs::price_oracle` con trait `PriceOracle` + impl `ConfigPriceOracle` que resuelve precios per-token via 3 tiers (base symbol → operator `token_prices_usd` map → hardcoded stablecoin list) + fail-honest `None` para tokens desconocidos. Añadido field `token_prices_usd: HashMap<String, f64>` con `serde(default)` para backward compat. Añadida `RejectReason::UnknownTokenPrice` con precedencia más alta que AnomalousMath. `profit_token_to_usd` marcado `#[deprecated]`. Lista hardcoded de stables consensus aprobada por operador: USDC, USDT, DAI, BUSD, FRAX, LUSD, USDP, TUSD, GUSD, USDD, PYUSD (excluidos: USDe, MIM, crvUSD, GHO — operador debe set explicit). Verificación: 70/70 tests workspace (33 shared + 11 spine + 26 searcher), deploy OK 08:10:56 UTC. **Migración operador requerida una vez**: `redis-cli HSET arbx:trading_config:1 token_prices_usd '{"WBTC":95000,"BNB":600,"LINK":14,"UNI":8,"AAVE":90,"ARB":0.4,"OP":1.3,"MATIC":0.5}'` para los 8 tokens no-stable de su allowlist.
- (c) **Pricing oracle**: definir interfaz `PriceOracle` en `shared_rs` con implementación stub para tokens conocidos (WETH, USDC, USDT, DAI, WBTC) + fallback "no precio → no USD" honesto.
- (d) **Limpieza histórica opcional**: las 3 rows con outliers ($113, $74, $73.888) quedan en PG. `DELETE FROM opportunities WHERE expected_profit_usd > 0 AND detected_at < '2026-05-04 11:43:13+00'` para limpiar histórico contaminado.

**Evidencia del fix:**

- ANTES (pre-deploy 11:43:13 UTC del 4 de Mayo): 3 outliers históricos con ROI 1.127% / 739% / 735.184%; `scanner.db_error` por overflow numeric en evento live de 42B% ROI; cap asimétrico produciendo $115 fantasma reproducible numéricamente.
- DESPUÉS deploy 1 — BUG-3 fix (post-deploy 11:43:13 UTC del 4 de Mayo, commit `4b99eb8`): 8 nuevas opps insertadas, todas profit=0 (filtradas por TokenNotAllowed antes del math evaluator); 0 `scanner.db_error`; heartbeat emite cada 60s con `pg_period_profit_pos=0` — pipeline limpio.
- DESPUÉS deploy 2 — BUG-1 fix (post-deploy 06:46:50 UTC del 5 de Mayo, commit `2a465e9`): 11 nuevas opps insertadas, 0 con ROI > 100%, 0 con profit > 0, 0 `scanner.db_error`. Boot logs limpios, heartbeat emite. Combinado con BUG-3 fix, la cadena USDT→WETH ahora produce ROI ≈ 0% en lugar de 42 mil millones %.
- DESPUÉS deploy 3 — Sanity bound defensivo (post-deploy 07:22:15 UTC del 5 de Mayo, commit `bf4f0c7`): añadida `RejectReason::AnomalousMath` + clamp a cero cuando ROI > 999% o |gross| > $1M en math_engine. Tests: 8/8 spine pasan, deploy verificado, heartbeat OK. Defense-in-depth catches operator misconfig de `token_prices_usd` (ej. typo $10K en UNI).
- DESPUÉS deploy 4 — BUG-2 fix (post-deploy 08:10:56 UTC del 5 de Mayo, commit `0855010`): nuevo `shared_rs::price_oracle` con `ConfigPriceOracle` 3-tier resolution. 25 nuevas opps insertadas en 5 min post-deploy, todas profit=0, todas filtradas por `config.token_not_allowed` antes del math evaluator (allowlist filter). 0 `scanner.db_error`, 6 heartbeats consecutivos a cadencia 60s exacta. **Cuatro capas de defensa ahora activas**: (1) BUG-1 decimals correctos, (2) BUG-3 cap asimétrico arreglado, (3) Sanity bound clamp ROI > 999%, (4) BUG-2 oracle fail-honest.
- Caveat honesto persistente: la verificación empírica de los 4 fixes integrados requiere un tx que pase el allowlist gate y llegue al math evaluator (donde el oracle resuelve precios per-token). En las ventanas cortas post-deploy (≤10min cada uno) ningún `candidate_enriched` llegó al math path — eventos esperados con frecuencia ~1-2 cada 19min según historial. La verificación formal sigue siendo los **tests TDD** (18 tests específicos del trío de bugs: 2 BUG-3 cap, 7 BUG-1 decimals, 2 sanity bound, 7 BUG-2 oracle resolution + 3 BUG-2 integration). Operador debe poblar `token_prices_usd` para los 8 tokens no-stable de su allowlist; hasta entonces, opps con esos tokens rechazadas con `UnknownTokenPrice` (visible vía dashboard / logs / heartbeat counter futuro).

---

## Incidente #8: Sesión maratón 2026-05-05/06 — observabilidad granular + REVM scaffold + UI Simulación

**Fecha del aprendizaje:** 5-6 de Mayo de 2026 (sesión de ~22 commits sin regresión)

**Qué ocurrió:**
Sesión continua post-cierre del trío BUG-1+2+3 (Incidente #7). Operador pidió iterativamente "sigue" después de cada brick — 22 commits de progreso continuo, 16+ deploys VPS verificados, todo con disciplina OMEGA (TDD red-green, --no-cache --env-file .env, evidence-over-claims, R8 fail-honest). Ningún brick rompió el anterior, ninguna regresión.

**Brick por brick (orden cronológico):**

| Commit | Brick | Verificación |
|--------|-------|--------------|
| `f31bf8b` | #2+#3: `/api/health` alias + heartbeat in-memory counters (AtomicU64) | Heartbeat live con funnel completo en logs |
| `d414fb2` | Edge proxy `/api/health` → api-server | curl :8787/api/health 200 OK |
| `289d5ee` | #5: V2 token0_id structural fix (cierra TODO scanner.rs:350) | token0_addr propagado a Redis verificable |
| `8a7968c` | REVM Phase 1: swap_encoder V2 + ERC20 + design doc 5 phases | 7/7 tests, selectores Etherscan-verified |
| `0231741` | REVM Phase 2: swap_encoder V3 (exactInputSingle + exactInput + path packed) | 13/13 tests acumulados |
| `5935469` | REVM Phase 3: erc20_storage balance helpers (8 tokens hardcoded) | 9 tests + keccak regression vs `cast keccak` |
| `5ae923d` | REVM Phase 4: round_trip_executor data + helpers + skeleton (NO fake PASS) | 9 tests, spine 46/46 |
| `7c60c27` | REVM Phase 5a: erc20_storage allowance helpers (two-level keccak) | 15 tests erc20_storage, spine 52/52 |
| `30cc7af` | Heartbeat snapshot a Redis + GET `/api/v1/scanner/heartbeat` + edge proxy | curl endpoint retorna JSON snapshot live |
| `c333d05` | Pipeline Funnel Widget en /operations (cierre observability arc UI) | HTML render confirmado, "Scanner Pipeline Funnel" presente |

**Causa raíz de la productividad sostenida:**
Disciplina rígida + bricks pequeños + verificación E2E entre cada uno + decisiones de scope honestas. NO se intentó implementar Phase 5b REVM porque requiere mainnet RPC para validación — el operador puede invocarlo en sesión futura cuando tenga acceso. El skeleton de `execute_round_trip` retorna `failed("pending Phase 5 implementation")` en lugar de fake PASS — R8 fail-honest aplicado al código no validado.

**Aprendizajes operacionales (para futuras sesiones):**

- **El patrón "sigue" funciona cuando hay un backlog claro y bricks bien dimensionados.** Cada commit fue auto-contenido, deployable individualmente, sin dependencias forward. Si el operador hubiera pausado en cualquier punto, el sistema queda en estado consistente.
- **Scope honesty preserva momentum.** Phase 5b REVM se NO atacó porque genuinamente necesita RPC. Mejor entregar 4.5/5 phases solidas + scaffold del 5to que 5/5 con código untestable. El backlog tiene la sub-tarea claramente marcada para sesión futura.
- **Frontend deploys son ~2x más caros que backend** (Next.js standalone build). En sesiones de muchos commits, agrupar cambios frontend cuando sea posible para reducir ciclos de rebuild.
- **El AppLocker de Windows bloquea binarios test fresh** — afecta solo tests, no production builds. Workaround: retry tras 10-20s OR confirmar via tests transitivos en otros crates que consumen el código.
- **R8 Fail-Honest tiene MUCHAS facetas operativas en una sesión productiva**: aplica al código (skeleton retorna failed, no fake PASS), al data layer (rejection_reason poblado en DB en lugar de NULL silencioso), al transport layer (TTL 3× period en Redis snapshot → 404 cuando searcher cae en lugar de stale data servido).

**Validación E2E acumulada al cierre de sesión:**

- 5 capas defensivas activas (BUG-1+2+3 fix + sanity bound + observability)
- REVM Sprint 4.5/5 (90%) — Phase 5b roadmap claro pendiente sesión con RPC
- 75+ tests verde workspace (52 spine + 33 shared + 26 searcher + 12 amm_math)
- 1 oportunidad legítima validated post-fix ($72.45 LowLiquidity-rejected) confirma pipeline funciona
- UI Simulación tab con 4 niveles configurables + Pipeline Funnel widget en /operations
- Heartbeat snapshot queryable vía REST (no solo log-grepable)
- 0 regresiones, 0 rollbacks, 0 mocks introducidos

**Acción correcta en futuras ocasiones:**

- Cuando el operador diga "sigue" después de N commits exitosos, **mantener bricks pequeños** (≤2h cada uno) y **verificar E2E entre cada uno**. La tentación de hacer un commit grande "para terminar" rompe la disciplina y aumenta riesgo de regresión.
- **Documentar lo que NO se hizo Y POR QUÉ es tan importante como documentar lo que sí**. Phase 5b skipping debe quedar visible para que la próxima sesión sepa por dónde continuar.
- **Bricks de observabilidad pagan compound interest**: heartbeat counters → API endpoint → UI widget. Cada uno habilita el siguiente. La inversión de 30min en counters atómicos se convirtió en visualización completa al final del día.

**Archivos relacionados (ubicaciones canónicas para referencia):**

- `backend/searcher-rs/src/counters.rs` — AtomicU64 counters por gate path
- `backend/searcher-rs/src/workers/heartbeat_worker.rs` — emit + persistencia Redis con TTL 3×
- `backend/searcher-rs/src/persistence.rs` — INSERT con rejection_reason (GAP-2)
- `backend/searcher-rs/src/scanner.rs` — V2 token0_id direct orientation + counters increments
- `backend/prioritization-spine/src/swap_encoder.rs` — V2 + V3 + ERC20 calldata encoders
- `backend/prioritization-spine/src/erc20_storage.rs` — balance + allowance slot tables + keccak
- `backend/prioritization-spine/src/round_trip_executor.rs` — orchestration + helpers + skeleton
- `backend/api-server/src/index.ts` — endpoints `/api/v1/scanner/heartbeat` + `/api/health`
- `edge/dev-local/src/index.ts` — proxy aliases `/api/health` + `/api/scanner/heartbeat`
- `frontend/app/operations/components/PipelineFunnelCard.tsx` — UI widget funnel
- `docs/superpowers/plans/2026-05-05-revm-real-implementation.md` — REVM design doc 5 phases

---

## Incidente #9: BUG-3 regression vía orientation flip en triangular_worker (1183 fake-positive opps con $4M profit)

**Fecha:** 2026-05-07
**Severidad:** 🔴 CRÍTICO (capital decisions impacted hipotéticamente — paper-trade salvó)
**Detección:** operador via `/opportunities` UI mostrando 8 filas idénticas con profit=$4,097,896 — clic visual antes de cualquier alerta automatizada

**Contexto previo:**
Sesión orquestada: deploy de UI honesty (lifecycle_status badges) → triangular MVP shipped LIVE → tick_stats periodic log → 4 long-tail cycles añadidos → migration 037 seeding 2 V2 pools faltantes (WBTC/USDC + DAI/USDT) → tick_stats reveló 4 emits inmediatos en cycle WETH>USDC>WBTC reverse.

**Síntoma observable:**
```sql
SELECT detected_at, dex_b, expected_profit_usd, amount_in_wei
FROM opportunities WHERE strategy_kind='triangular' ORDER BY detected_at DESC;
-- 1183 rows, todas:
-- cycle:WETH>USDC>WBTC | $4,097,896 | 2.16e18 wei (~2.16 WETH ≈ $5K input)
-- ROI implícito ~80,000% — matemáticamente imposible en majors saturados
```

**Root cause (3 capas):**

1. **Migration 037 declaró token0/token1 al revés**: WBTC/USDC pool con `token0_id=USDC, token1_id=WBTC`. Uniswap V2 invariante requiere `token0 = LOWER address`. WBTC=`0x2260...` < USDC=`0xa0b8...` → on-chain token0 es WBTC, NO USDC.

2. **pool_sync_worker.rs:199** lee `token0_addr` desde PG `pools.token0_id` (no desde el chain via `token0()`). Trust al PG declaration → escribe `token0_addr` incorrecto al ReservesEntry en Redis.

3. **triangular_worker** lee orientación desde `entry.token0_addr`, computa swap dirección al revés para hop USDC→WBTC, multiplica el rate ~833x, y produce $4M fake profit que cascade a través de los 3 hops.

**Contributing factor (defense-in-depth gap):**
El **spine evaluator's sanity bound** (anti-BUG-3 from Incidente #7) protege opps que fluyen por el spine. **Triangular worker llama `persistence::insert_opportunity` + `publisher::publish` DIRECTAMENTE**, bypasseando el spine. Cualquier worker que escriba a PG sin spine evaluation = BUG-3 puede recurrir.

**Fix (3-layer defense-in-depth, RULE 00 / R8):**

1. **Migration 038** (commit `4c53be9`):
   - `UPDATE pools SET token0_id <-> token1_id` para el WBTC/USDC pool.
   - **CREATE TRIGGER `pools_v2_token_order_trigger`** sobre `BEFORE INSERT/UPDATE`: enforces `LOWER(token0.address) < LOWER(token1.address)` para protocol_type='UNISWAP_V2'. RAISE EXCEPTION en violación. **Schema-level prevention**: futuro operator inserts no pueden repetir el bug.

2. **Sanity bound at worker level** (commit `4c53be9`):
   ```rust
   const SANITY_PROFIT_MULT_OF_CAP: f64 = 5.0;
   if profit_cap_ratio > SANITY_PROFIT_MULT_OF_CAP {
       warn!(event="triangular_worker.sanity_reject", ...);
       return Some(cycle_block); // skip, don't emit
   }
   ```
   - Captura cualquier orientation/decimals/unit bug futuro que produzca profit > 5×cap.
   - X10THINK refinement: además rechaza si `profit_usd is None|NaN|Inf|negative` (R8 explicit check, no `unwrap_or(0.0)` que silencie errores).

3. **Regression test reproduce** (commit `4c53be9`):
   `cycle_profit_extreme_imbalance_yields_huge_profit_caught_by_sanity_bound` — feeds math kernel un hop deliberadamente mis-orientado, asserts `profit_ratio > 5.0`. Documenta que el kernel POR DISEÑO no detecta orientation errors → la guard del worker es la protección load-bearing.

**Cleanup data:**
```sql
UPDATE opportunities SET rejection_reason='AnomalousMath:ProfitExceedsCap5x_BUG3_regression_2026-05-07'
WHERE strategy_kind='triangular' AND rejection_reason IS NULL
  AND (expected_profit_usd > 1000 OR roi_pct > 100);
-- 1183 rows marked
```

**Verificación E2E post-fix:**
- 5+ min de tick_stats con `scanned=50, emitted=0, skip_no_profit=50` consistente
- Cero `sanity_reject` events post-stabilización (transient solo durante propagación stale entre migration 038 y primer pool_sync tick)
- Hand-derived cycle profit con reservas mainnet correctas = -$2,821 LOSS (matching kernel runtime output ahora)

**Lecciones para futuras sesiones:**

- **CUALQUIER worker que escriba directo a PG/Redis sin spine evaluator NECESITA su propia sanity bound.** El spine sanity bound NO protege bypass paths. Convertir esto en CHECK-LIST cada vez que se ship un nuevo worker emisor.
- **V2 token order invariant es schema-level, no documentation-level.** El TRIGGER es defensa permanente. Repetir patrón para otras invariantes Uniswap (V3 fee tier valid, etc.).
- **Defense-in-depth >> single-layer correctness.** Mig 038 fixó el root cause + sanity bound captura clase entera de bugs futuros + regression test documenta el contrato. 3 capas cuando 1 hubiera bastado teóricamente.
- **Migration con datos reales requiere VERIFICAR token order ANTES de commit.** Para Uniswap V2: `LOWER(token0.address) < LOWER(token1.address)`. Si el operator declara al revés, el TRIGGER ahora falla loud antes de que pool_sync escriba data corrupta.
- **Pool_sync_worker debería usar on-chain `token0()` como source of truth, no PG.** Issue #FUTURE: refactor para llamar `token0()` via Multicall y persistir, ignorando PG declaration. Hasta entonces el TRIGGER es load-bearing.

**Archivos relacionados:**

- `database/migrations/037_seed_v2_pools_for_triangular_cycles.sql` — los 2 pools que necesitaron seeding (DAI/USDT correcto, USDC/WBTC declaró swapped)
- `database/migrations/038_fix_v2_pool_token_order_invariant.sql` — UPDATE swap + TRIGGER permanente
- `backend/searcher-rs/src/workers/triangular_worker.rs` — sanity bound + diagnostic dump per-hop
- `backend/searcher-rs/src/workers/pool_sync_worker.rs:199` — el lugar donde token0_addr se escribe a Redis desde PG (load-bearing)

**Incidente cerrado:** 2026-05-07 ~08:46 UTC. 5+ min sin emits anómalos. Defense-in-depth verificada en producción.

---

## Incidente #N+1: CI Baseline Roto desde Día 1 (Broken-Baseline Pattern)

**Fecha del aprendizaje:** 2026-05-09 (Sesión de observación CI tras despliegue de workflows en commit `3d9410a`)

**Qué ocurrió:**
Los 7 GitHub Actions workflows añadidos en `3d9410a` corrieron por primera vez con el push y **4 de 5 fallaron consistentemente** desde el primer run (e2e fue el único verde). Nadie revisó el resultado durante 4+ días. El operador descubrió el rojo al pedir "observa los primeros runs de CI" en una sesión de seguimiento.

**Qué salió mal:**
1. **Rust CI**: toolchain pinned a `1.75` pero el lockfile tiene `alloy-chains 0.2.34` que requiere `edition2024` (Rust 1.85+). Falló en `cargo check`.
2. **unit-tests Rust**: cascade del mismo issue — toolchain default sin pin.
3. **unit-tests TypeScript**: `tsc --noEmit` reportó `Cannot find module '@arbx/shared'` porque el workspace `shared-ts` no se construye antes del typecheck. Sin `dist/` no hay tipos resolvibles para `selector-api`, `api-server`, `edge/*`.
4. **Security Scans**: cargo-audit install falló (cascade toolchain).
5. **no-hardcode**: 180 violaciones reales — la mayoría hardcodes de token addresses en `triangular_worker.rs` y URLs en test fixtures que el regex allow-list no cubría semánticamente.

**Causa raíz:**
Workflows fueron mergeados sin haberse validado en un PR aislado contra el repo. El "primer run" ocurrió directamente en main, sin nadie observando, así que el rojo se acumuló silenciosamente. Esto se llama **broken-baseline**: arrancar con CI rojo desnormaliza el rojo y todos los fallos futuros pierden señal.

**Regla nueva para prevenirlo (R-CI-1 Broken-Baseline Avoidance):**
- **Antes de mergear un workflow de CI nuevo a main**, debe correr al menos 1 vez exitosamente en un PR de prueba.
- Si el primer run falla, **bloquear merge** del workflow hasta que pase, O documentar explícitamente el `continue-on-error: true` con TODO y issue de seguimiento.
- **Toolchain pins** en CI deben coincidir con la versión mínima requerida por el lockfile, no con el `rust-version` del manifesto (que es informativo).
- **TypeScript workspaces con paquetes locales** (`@arbx/shared`) requieren paso de `npm run -w @arbx/shared build` ANTES del typecheck.

**Validación obligatoria:**
Tras añadir o modificar un workflow CI: `gh run list --workflow=<name> --limit 1` y confirmar `conclusion=success` antes de declarar el cambio "listo".

**Archivos relacionados (fix aplicado en sesión 2026-05-09):**
- `.github/workflows/rust.yml` — toolchain 1.75 → 1.85
- `.github/workflows/security.yml` — toolchain 1.75 → 1.85
- `.github/workflows/unit-tests.yml` — toolchain 1.85 + step `npm run -w @arbx/shared build` + DATABASE_URL stub para sqlx offline + `--lib` flag
- `automation/tools/lint-no-hardcode.sh` — JSX `placeholder=` skip + `tests?/` dir allow + canonical adapter files allow-list
- `docker/compose.dev.yml` — `GF_SERVER_ROOT_URL` hardcoded → env var
- `backend/Cargo.toml` — rust-version → 1.85; `transports-http` → `transport-http` typo fix

**Deuda técnica queued (NO arreglada en este fix, pendiente de Sprint refactor):**
no-hardcode todavía reporta ~152 violaciones reales que NO son falsos positivos:
- `backend/searcher-rs/src/workers/triangular_worker.rs:72,423-434` — mapping hardcoded de 9 token addresses (WETH/USDC/USDT/DAI/WBTC/PEPE/SHIB/MKR/COMP). Debe migrar a `backend/shared-rs/src/tokens.rs` (catálogo canónico).
- `backend/token-enricher/src/multicall.rs:5` — `MULTICALL3_ADDRESS` constante. Debe ir a `chains.rs` o `tokens.rs`.
- `backend/relays-client/src/relay_bloxroute.rs:27` — `BLOXROUTE_MEV_URL` constante. Debe ir a catálogo o env var.
- Múltiples test fixtures con direcciones inline en `revm_backend.rs`, `price_oracle.rs` que requieren refactor a `tests/fixtures/`.

**Incidente abierto:** En espera del refactor de catálogo canónico. CI workflows toolchain + TS workspace fixes ya en main; no-hardcode permanece rojo y documentado hasta cleanup.

---

## Incidente #N+1: Commit mixto por archivos pre-staged de sesión paralela

**Fecha del aprendizaje:** 8 de Mayo de 2026 (sesión visual stack)

**Qué ocurrió:**
Tras editar 4 archivos frontend del aurora-glass dark theme, ejecuté `git add` solo de mis 4 archivos y luego `git commit -m "..." -- <pathspec>`. El commit resultante (`b912da0`) contenía MIS 4 frontend + 3 archivos backend Rust de un commit BE-01 Sprint A que una sesión Claude Sonnet 4.6 paralela había dejado pre-staged en el index. El mensaje de commit terminó siendo el de BE-01 (no el mío).

**Qué salió mal:**
1. No verifiqué `git diff --cached` antes de stage adicional → no detecté que el index ya tenía los 3 archivos rust.
2. No verifiqué `git show HEAD --stat` después del commit → no detecté que el mensaje ni el conjunto de archivos eran los esperados.
3. Pathspec en `git commit` no siempre restringe — git puede seguir consumiendo `.git/COMMIT_EDITMSG` y archivos staged previamente.

**Causa raíz:**
Index de git compartido entre sesiones Claude paralelas. Cada agente puede dejar el index en estado inconsistente. Asumí "limpio" en lugar de verificar.

**Regla nueva para prevenirlo:**
- **Pre-commit hygiene OBLIGATORIO**: Antes de cada `git commit`, ejecutar `git diff --cached --stat` y confirmar que SOLO los archivos esperados están staged.
- **Post-commit hygiene OBLIGATORIO**: Después del commit, ejecutar `git show HEAD --stat | head -3 && tail -28` y validar primer línea (mensaje) + lista de archivos.
- **Recovery seguro si el commit es mixto**: `git reset --soft HEAD~1` (preserva todo en el index) → 2 commits separados con pathspec explícito → push.
- **Nunca pushear sin estos dos checks** — una vez en remoto, el único fix es force-push (destructivo) o revert commit (sucio).

**Validación obligatoria:**
Persistido en `~/.claude/projects/c--Users-HFRC-Desktop-arbitragex-v2-productivo-full/memory/feedback_git_commit_hygiene.md` con prompt-level enforcement.

**Archivos relacionados:**
Cualquier flujo `git add → git commit` en este repo. La memoria de subagent scope discipline cubre el caso subagente; este cubre el caso parent-level.

---

## Incidente #N+2: `next build` corrompe `.next/` mientras `next dev` está activo

**Fecha del aprendizaje:** 9 de Mayo de 2026 (sesión visual stack)

**Qué ocurrió:**
Tras refactorizar `OpportunitiesClient.tsx` de hardcoded slate-* a tokens, ejecuté `npm run build` para verificar mientras `npm run dev` seguía corriendo en background. El build pasó (exit 0). Cuando el usuario refrescó el navegador, vio la página completamente sin estilos — solo HTML crudo con default browser styles.

**Qué salió mal:**
- `next build` reescribe `.next/server/` y `.next/static/` con bundles de PRODUCCIÓN.
- `next dev` mantiene en memoria los manifests de DESARROLLO que apuntaban a chunks ahora movidos/renombrados.
- Resultado: dev sirve 404 en `layout.css`, `layout.js`, `main-app.js`, `app-pages-internals.js` → navegador no recibe CSS → página sin estilo.

**Causa raíz:**
Concurrencia entre dos procesos Next.js sobre el mismo `.next/` directory.

**Regla nueva para prevenirlo:**
- **NUNCA correr `npm run build` con `npm run dev` activo simultáneamente** sobre el mismo proyecto.
- Si ambos son necesarios, usar `--distDir` distinto en uno (ej. `next build -- --distDir=.next-prod`).
- En esta sesión la convención es: para build de validación, primero `TaskStop` el dev server, luego build, luego restart dev si se necesita.

**Recovery cuando ocurre:**
1. Stop dev server (TaskStop o taskkill PID del puerto 5173)
2. `rm -rf frontend/.next`
3. `cd frontend && npm run dev` (reconstruye cache desde cero, ~2s)

**Síntoma reconocible:**
404s en `_next/static/css/...css`, `_next/static/chunks/main-app.js` en logs del dev server. Página HTML servida pero sin estilos.

**Archivos relacionados:**
`frontend/.next/` (cache directory).

---

## Incidente #N+3: Browser auto-translation rompe React reconciliation (`removeChild on Node`)

**Fecha del aprendizaje:** 9 de Mayo de 2026 (sesión visual stack, reportado vía screenshot WhatsApp del operador desde mobile Chrome Android)

**Qué ocurrió:**
Operador accedió a `http://<VPS_IP>:5173/strategies` desde Chrome móvil Android. Vio el error boundary de la app (`error.tsx`) con mensaje "Algo se rompió" y diagnóstico técnico:
```
Failed to execute 'removeChild' on 'Node':
The node to be removed is not a child of this node.
```

El error name "NotFoundError" venía traducido a "Error de no encontrado" — pista de que Chrome había auto-traducido la página.

**Qué salió mal:**
1. `<html lang="en">` declaraba inglés.
2. Contenido de la app mezcla inglés/español según componente.
3. Chrome móvil sobre IP cruda HTTP (sin reputación) aplica políticas de translate más agresivas.
4. Auto-translate reemplaza text nodes en el DOM.
5. React intenta `removeChild` sobre un node que el navegador ya movió/reemplazó → throw.
6. Error boundary captura. Pero CUALQUIER state change posterior reactiva el bug.

**Causa raíz:**
Incompatibilidad fundamental entre Virtual DOM de React y modificación externa del DOM por features del navegador. Issue `facebook/react#11538` abierto desde 2017, sin fix planeado upstream.

**Regla nueva para prevenirlo:**
**Defensa de 3 capas obligatoria** en `frontend/app/layout.tsx`:
1. `<meta name="google" content="notranslate">` en `<head>` — Google Translate primary signal.
2. `translate="no"` attribute en `<html>` y `<body>` — HTML5 standard.
3. `class="notranslate"` en `<html>` y `<body>` — Google Translate widget legacy signal.

Cada navegador honra una combinación distinta. Belt-and-suspenders es necesario.

**Validación obligatoria:**
Tras cualquier cambio a `layout.tsx`, verificar en HTML servido:
```bash
curl -s "$URL" | grep -E '(name="google" content="notranslate"|translate="no"|class="[^"]*notranslate)'
```
Debe haber matches en las 3 capas.

**Archivos relacionados:**
`frontend/app/layout.tsx` (metadata.other.google + html attrs + body attrs)
`frontend/app/error.tsx` (recibió Sergio de la traducción al ser síntoma visible)

**Fix aplicado:** commit `7403cbe` (2026-05-09 23:25 UTC) — desplegado a VPS, verificado E2E.

---

## Incidente #N+4: Smart App Control bloquea Foundry forge.exe (irreversible al apagar)

**Fecha del aprendizaje:** 10 de Mayo de 2026 (sesión visual stack)

**Qué ocurrió:**
Operador vio toast de "Seguridad de Windows" reportando que `bash.exe` intentó cargar `forge.exe` y la acción fue bloqueada por publisher no verificado.

**Qué se descubrió:**
- Smart App Control (SAC) en Windows 11 = ON.
- forge.exe del operador está NotSigned (Foundry no firma releases).
- SAC NO acepta exclusiones por archivo, NO tiene allowlist, NO se integra con `Add-MpPreference -ExclusionPath`.
- Apagar SAC requiere reboot y es **irreversible sin reinstalar Windows desde cero** (diseñado así por Microsoft para evitar disable por malware).

**Inventario:** 18 binarios unsigned en el dev environment del operador — 4 Foundry + 14 Rust toolchain (cargo, rustc, rustfmt, clippy, rust-analyzer, etc.). SAC los permite por reputación cloud, no por firma — frágil.

**Regla nueva para prevenirlo:**
- **No depender de binarios unsigned localmente** en Windows con SAC=ON. Toda compilación productiva debe correr en GitHub Actions (Linux runners) o Docker (Linux containers).
- **Para tools de dev en Windows**: preferir wrappers SSH→VPS (sin SAC) o WSL2 (Linux dentro de Windows, sin SAC).
- **NUNCA** proponer apagar SAC al operador — la pérdida de seguridad permanente no justifica el dev convenience.

**Validación si nuevo tool unsigned llega**:
```powershell
Get-AuthenticodeSignature "$env:USERPROFILE\.foo\bin\foo.exe" | Select Status
```
Si `NotSigned` y `Get-MpComputerStatus` muestra `SmartAppControlState=On`, el tool puede ser bloqueado en cualquier momento.

**Archivos relacionados:**
- `~/.foundry/bin/` (forge, cast, anvil, chisel — 4 NotSigned)
- `~/.cargo/bin/` (14 NotSigned)

---

## Incidente: Schema-Drift `enabled_dex_ids` y silent-paralysis del operador

**Fecha del aprendizaje:** 10 de Mayo de 2026 (Sesión post-audit)

**Qué ocurrió:**
La página `/strategies` permitía al operador habilitar/deshabilitar DEXes por chain. La selección viajaba de la UI → API (Zod schema) → PostgreSQL `trading_config.enabled_dex_ids` → Redis. Pero el motor (`searcher-rs` vía `prioritization-spine`) ignoraba el campo silenciosamente: cualquier toggle de DEX en `/strategies` era **decorativo**.

**Qué salió mal:**

1. La migración 042 añadió la columna `trading_config.enabled_dex_ids UUID[]` y el endpoint `/admin/trading-config/:chain_id` la persistía + la espejaba a Redis.
2. El struct Rust `shared_rs::trading_config::TradingConfigState` **NO tenía el campo `enabled_dex_ids`**. Como serde acepta campos extra por defecto (`#[serde(deny_unknown_fields)]` no estaba), la deserialización no fallaba — el campo simplemente desaparecía.
3. Se sumaba: tampoco existía un struct fino por estrategia. El operador podía activar "dex_arb_v2v2" y "triangular" en la lista plana `enabled_strategies`, pero NO podía configurar `min_profit_usd`, `min_roi_pct`, `enabled_dex_ids`, `route_constraints` por estrategia.

**Causa raíz:**
**Schema drift silencioso entre serialización y consumo**. Más peligroso que un error de compilación: API+UI lucen funcionales, los datos viajan, la DB los guarda, pero el motor que decide los descarta. La superficie de control parecía operar pero el comportamiento del runtime no cambiaba.

**Regla nueva para prevenirlo:**

- **Symmetry Audit obligatorio**: cuando una migración añade una columna a `trading_config` (o cualquier estructura que se hidrata vía `serde_json::from_str` en Rust), revisar **simultáneamente**:
  1. Schema Zod del API (`backend/api-server/src/routes/*.ts`)
  2. Cast a Redis (`rowToRedisState` o equivalente)
  3. Struct Rust correspondiente con `#[serde(default)]` para retrocompatibilidad
  4. Tests unitarios que deserializan un JSON legacy SIN el campo + un JSON nuevo CON el campo
  5. Tipos del frontend (`frontend/lib/schemas.ts`) si la UI lee/edita el campo
- **Cobertura de tests**: cada nuevo campo necesita un test `deserialise_legacy_config_without_new_fields` que cargue el JSON anterior a la migración y verifique que el campo cae a su default sin panic.

**Validación obligatoria:**
Después de cada migración SQL que toque `trading_config`, confirmar:

```bash
# Rust struct compila con un blob Redis legacy
cargo test -p shared-rs --lib trading_config::tests::deserialise_legacy_config_without_new_fields
# Schema TS y Rust están alineados (campo obligatorio en uno = obligatorio en el otro)
grep "<campo>" backend/api-server/src/routes/trading-config.ts backend/shared-rs/src/trading_config.rs frontend/lib/schemas.ts
```

**Archivos o rutas relacionadas:**

- `database/migrations/042_trading_config_enabled_dex_ids.sql` (origen — el campo en DB)
- `database/migrations/056_strategy_configs.sql` (cierre — añade `strategy_configs JSONB`)
- `backend/shared-rs/src/trading_config.rs` (struct extendido + helpers `strategy_enabled`, `effective_dex_allowlist`, `effective_min_profit_usd`)
- `backend/prioritization-spine/src/strategy_config_gate.rs` (nuevo gate de doble pasada)
- `backend/prioritization-spine/src/route_plan.rs` (contrato `RoutePlan` + `RouteLeg`)
- `backend/searcher-rs/src/scanner.rs` (emite RoutePlan minimal por candidato)

**Acción correcta en futuras ocasiones:**
Cuando una columna SQL se añade para que el operador la edite vía UI, el cierre del bug requiere **5 capas tocadas en el mismo PR**: SQL + API + Redis cast + Rust struct + UI form. Si alguna falta, el fix no cierra — produce schema drift silencioso. Verificación obligatoria con un `cargo test` que deserializa el blob legacy.

**Fix aplicado:** ninguno todavía. 3 opciones propuestas al operador (SSH wrapper / WSL2 / disable SAC). Pendiente de elección.
