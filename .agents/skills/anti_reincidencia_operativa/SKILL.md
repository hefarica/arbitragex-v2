# Skill: Prevención de Reincidencia Operativa (Anti-Reincidencia)

> **Versión:** 3.0 — Actualizada 2026-05-03T08:48Z  
> **Origen:** Incidentes de hidratación React #425/#418/#423, fallo de WebSocket Upgrade Proxy, fuga de variables de entorno en producción, y ruptura silenciosa del pipeline E2E Alchemy→Dashboard por `DATABASE_URL` ausente.

---

## 1. Nombre de la skill

`anti_reincidencia_operativa`

## 2. Cuándo debe activarse

Esta skill se activa **automática e incondicionalmente** cuando el agente enfrente CUALQUIERA de estas situaciones:

- Corrección de un bug crítico en producción (VPS <VPS_IP>).
- Modificación de archivos en `frontend/app/`, `edge/dev-local/`, `backend/api-server/`, `backend/searcher-rs/`, `docker/`, `nginx`.
- Reconstrucción o redespliegue de contenedores Docker.
- Diagnóstico de errores de consola del navegador en la URL pública.
- Cualquier cambio que toque SSR, hidratación, WebSockets o variables de entorno.
- Diagnóstico de **datos faltantes** en el Dashboard (oportunidades, ejecuciones, alertas que no aparecen).
- Modificación de `docker/compose.dev.yml` (variables de entorno, dependencias entre servicios).

## 3. Qué errores o comportamientos debe prevenir

| # | Error / Comportamiento | Severidad |
|---|------------------------|-----------|
| 1 | React Hydration Mismatch (#425, #418, #423) | **CRÍTICA** |
| 2 | Variables `NEXT_PUBLIC_*` apuntando a `localhost` en producción | **CRÍTICA** |
| 3 | WebSocket que se cierra antes de completar el upgrade (HTTP 400/101 fail) | **ALTA** |
| 4 | Chunks JS obsoletos sirviendo código viejo post-deploy | **ALTA** |
| 5 | "Ciclo de la muerte": arreglar síntoma sin documentar causa raíz | **MEDIA** |
| 6 | Build que pasa localmente pero rompe en Docker por dependencias faltantes | **MEDIA** |
| 7 | Despliegue sin invalidación de caché (`--no-cache` omitido) | **ALTA** |
| 8 | Conocimiento crítico que se pierde al terminar la conversación | **MEDIA** |
| 9 | Servicio Docker sin `DATABASE_URL` → datos fluyen pero nunca se persisten | **CRÍTICA** |
| 10 | Pipeline E2E silenciosamente roto: Redis recibe datos pero PG está vacío | **CRÍTICA** |

## 4. Causa raíz de los problemas detectados

### 4.1 Hidratación React Rota (#425 → #418 → #423)

**Causa arquitectónica:** Componentes marcados `"use client"` evaluaban expresiones no determinísticas directamente en el primer ciclo de renderizado:

- `Date.now()` producía timestamps diferentes en servidor vs cliente.
- `getApiBaseUrl()` resolvía a `http://edge:8787` en el servidor (Docker DNS) pero a `https://<VPS_HOST>` en el navegador.
- Estado de WebSocket (`feedStatus`) se inicializaba con valores que dependían del entorno de ejecución.
- `SiteHeader` en `layout.tsx` contenía la misma violación, contaminando TODAS las rutas.

**Cascada:** #425 ("Text content mismatch") → #418 ("Hydration failed") → #423 ("Outside Suspense → full client render"). Resultado: la app completa se desmontaba y re-renderizaba en el cliente, perdiendo estado y provocando parpadeos.

### 4.2 Fuga de Variables de Entorno (localhost en prod)

**Causa operacional:** `docker compose build` se ejecutó SIN `--env-file .env`. Next.js resolvió `NEXT_PUBLIC_EDGE_URL` al fallback de `compose.dev.yml`: `http://localhost:8787`. Este valor se inyectó estáticamente en el bundle JS de producción. Resultado: la API del dashboard apuntaba a un puerto vacío en el navegador del operador.

### 4.3 WebSocket Upgrade Proxy Fallando

**Causa técnica:** En `edge/dev-local/src/index.ts`, el middleware `http-proxy-middleware` se utilizaba con `createProxyMiddleware({ ws: true })` pero:

1. **No se ligaba al evento `upgrade` del servidor HTTP.** Express no re-emite eventos `upgrade` automáticamente. Sin `server.on('upgrade', wsProxy.upgrade)`, el handshake WebSocket nunca se completaba.
2. **`pathRewrite` duplicaba la ruta.** La app montaba el proxy en `/socket.io` Y el `pathRewrite` volvía a prefijar `/socket.io/`, haciendo que la petición upstream llegara como `/socket.io/socket.io/...`.

Resultado: Socket.IO degradaba a HTTP long-polling (5s latencia) en lugar de WebSocket nativo (sub-segundo).

### 4.4 Pipeline E2E Silenciosamente Roto (Alchemy → Dashboard)

**Causa operacional:** El servicio `searcher-rs` en `docker/compose.dev.yml` no tenía `DATABASE_URL` configurado. En `main.rs`, cuando `DATABASE_URL` está ausente, `db_pool = None`. En `scanner.rs` línea 346, el bloque de persistencia se salta silenciosamente:
```rust
if let Some(pool) = db {  // db es None → NUNCA se ejecuta
    persistence::insert_opportunity(pool, &opportunity).await;
}
```

**Consecuencia catastrófica:** El searcher detectaba oportunidades reales desde Alchemy, las simulaba con REVM, las puntuaba, y las publicaba exitosamente al Redis Stream (`arbx:opps:detected`, 827+ entries). Pero **nunca las persistía a PostgreSQL**. El frontend lee de PostgreSQL vía `api-server`, por lo que el dashboard mostraba solo 3 oportunidades viejas mientras cientos de nuevas se acumulaban invisibles en Redis.

**Agravante:** No existía ningún log de ERROR. El `main.rs` emite un `WARN "db.not_configured"` al arrancar, pero este mensaje se pierde entre miles de líneas de PoolSync. El fallo era completamente silencioso en operación normal.

## 5. Reglas obligatorias para que no vuelva a pasar

### R1 — Regla Inmutable de Hidratación (Cero Mismatch)
Toda página SSR en Next.js App Router debe seguir el patrón **Mounted Snapshot**:
- `page.tsx` = Server Component puro. Hace `fetch()` al edge para obtener un snapshot serializable.
- `*Client.tsx` = Client Component. Recibe `initialSnapshot` como prop. Usa `useState(initialSnapshot)`.
- Todo lo no determinístico (`Date.now()`, `WebSocket`, `window`, `navigator`, `localStorage`, `matchMedia`) queda **exclusivamente** dentro de `useEffect()`.
- Los textos que dependen de hora/locale usan `suppressHydrationWarning` **solo** en el `<span>` individual, NUNCA en contenedores.

### R2 — Compilación Hermética (Build-Time Guard)
`next.config.js` contiene un guard que **aborta el build** si `NEXT_PUBLIC_EDGE_URL` apunta a localhost en `NODE_ENV=production`:
```javascript
if (process.env.NODE_ENV === "production") {
  if (EDGE_URL && /localhost|127\.0\.0\.1|0\.0\.0\.0/.test(EDGE_URL)) {
    throw new Error(`[CRITICAL] next build failed: NEXT_PUBLIC_EDGE_URL cannot point to localhost in production.`);
  }
}
```
Esta regla es **inmutable**. No se puede remover ni comentar.

### R3 — Despliegue con Cache-Busting + Env Explícito
Todo despliegue correctivo al VPS DEBE seguir esta secuencia exacta:
```bash
docker compose -f docker/compose.dev.yml --env-file .env build --no-cache <servicio>
docker compose -f docker/compose.dev.yml --env-file .env up -d <servicio>
```
Nunca `docker compose build` a secas. Nunca `up` sin `--env-file`.

### R4 — WebSocket Proxy Upgrade Binding
Cuando se use `http-proxy-middleware` con `ws: true` en un servidor Express:
1. Guardar la instancia: `const wsProxy = createProxyMiddleware({ target, ws: true, changeOrigin: true });`
2. Montar en express: `app.use('/socket.io', wsProxy);`
3. Ligar upgrade: `server.on('upgrade', wsProxy.upgrade);`
4. **No** usar `pathRewrite` si la ruta de montaje ya coincide con la ruta upstream.

### R5 — Auditoría de Componentes Transitivos
Al corregir un mismatch en una página, auditar TODOS los componentes importados por esa página y por el `layout.tsx` padre:
- `SiteHeader`, `SiteFooter`, `Sidebar`, `Breadcrumb`, `MetricCard`, `StatusBadge`.
- Buscar: `Date.now()`, `new Date()`, `Math.random()`, `window.`, `document.`, `navigator.`, `localStorage`, `getApiBaseUrl()`.

### R6 — Completitud de Variables en Docker Compose (Pipeline E2E)
Todo servicio backend en `docker/compose.dev.yml` que persista datos DEBE tener:
1. `DATABASE_URL` apuntando a `postgres://...@postgres:5432/arbitragex` con credenciales explícitas.
2. `depends_on: postgres: { condition: service_healthy }` para garantizar orden de arranque.
3. Un log verificable al arrancar que confirme la conexión (`db.connected` o equivalente).

**Auditoría obligatoria al agregar un nuevo servicio:**
- ¿El servicio produce datos que el Dashboard necesita ver?
- ¿Tiene `DATABASE_URL`? Si no, los datos se pierden silenciosamente.
- ¿Tiene `REDIS_URL`? Si publica a streams, ¿alguien los consume?
- ¿Los `depends_on` incluyen TODOS los servicios de infraestructura que necesita?

### R7 — Trazabilidad E2E del Pipeline de Datos
Cuando el Dashboard muestra datos vacíos o estancados, ejecutar auditoría capa por capa:
```bash
# 1. ¿El searcher detecta?  (logs del scanner)
docker logs searcher-rs --tail 200 | grep -i 'simulator.success'
# 2. ¿Redis recibe?  (stream length)
docker exec redis redis-cli XLEN arbx:opps:detected
# 3. ¿PostgreSQL recibe?  (latest row)
docker exec postgres psql -U postgres -d arbitragex -c 'SELECT MAX(detected_at) FROM opportunities;'
# 4. ¿api-server sirve?  (endpoint directo)
curl localhost:8787/api/opportunities/live | head
```
Si Redis tiene datos pero PG no → falta `DATABASE_URL` en el productor.
Si PG tiene datos pero API no → error en el query del `api-server`.
Si API tiene datos pero Dashboard no → error de frontend/edge/proxy.

## 6. Procedimiento paso a paso antes de actuar

1. **PAUSAR.** Leer los logs completos (consola del navegador + `docker logs` del contenedor). No proponer código sin entender el flujo completo del error.
2. **REPRODUCIR.** Usar `browser_subagent` para visitar la URL pública y confirmar el error con evidencia visual.
3. **TRAZAR.** Identificar el archivo y la LÍNEA exacta del mismatch. No asumir; usar `grep_search` para buscar `Date.now`, `Math.random`, `getApiBaseUrl`, `window.` en el directorio `frontend/`.
4. **AUDITAR TRANSITIVOS.** Verificar `layout.tsx` y todos los componentes compartidos que se renderizan en la misma ruta.
5. **CORREGIR ESTRUCTURALMENTE.** Aplicar el patrón Mounted Snapshot. No parchar con `suppressHydrationWarning` en contenedores.
6. **COMPILAR LOCALMENTE.** Ejecutar `npm run build` en el workspace frontend. Verificar exit code 0.
7. **DESPLEGAR AL VPS.** Seguir R3 estrictamente.
8. **VERIFICAR EN PRODUCCIÓN.** Usar `browser_subagent` para confirmar consola limpia en la URL pública.
9. **DOCUMENTAR.** Actualizar `.agents/memory/anti_reincidencia.md` con el nuevo incidente.

## 7. Validaciones antes, durante y después de modificar archivos

### ANTES
- [ ] ¿Leí los logs completos del error?
- [ ] ¿Identifiqué si es error de hidratación, compilación, red o configuración?
- [ ] ¿Busqué código no determinístico en todos los componentes de la ruta?
- [ ] ¿Verifiqué que `.env` en el VPS tiene las variables correctas?

### DURANTE
- [ ] ¿La corrección sigue el patrón Mounted Snapshot?
- [ ] ¿Ningún valor no determinístico se evalúa fuera de `useEffect`?
- [ ] ¿La compilación local (`npm run build`) pasa sin errores?
- [ ] ¿La compilación TypeScript (`tsc --noEmit`) pasa sin errores?

### DESPUÉS
- [ ] ¿Se desplegó con `--no-cache` y `--env-file .env`?
- [ ] ¿Se verificó visualmente con `browser_subagent` que la consola está limpia?
- [ ] ¿El feed REST de fallback funciona si WebSocket falla?
- [ ] ¿Se actualizó la bitácora en `.agents/memory/anti_reincidencia.md`?
- [ ] ¿Se informó al usuario con evidencia concreta (screenshot o log)?

## 8. Forma correcta de documentar nuevos aprendizajes

Todo aprendizaje estructural descubierto durante un fixing loop debe documentarse **inmediatamente** en dos lugares:

1. **Bitácora de memoria:** `.agents/memory/anti_reincidencia.md` — formato: "Qué pasó → Por qué pasó → Regla inmutable".
2. **Skill actualizada:** Este archivo (`SKILL.md`) — agregar nueva regla a la sección 5, nueva causa raíz a la sección 4.

**Nunca** dejar conocimiento crítico solo en el chat. El chat es volátil. Los archivos persisten.

## 9. Checklist final antes de responder al usuario

- [ ] ¿Entendí la causa raíz o solo tapé el síntoma?
- [ ] ¿Mi solución viola alguna regla inmutable preexistente (R1–R7)?
- [ ] ¿El entorno productivo fue reconstruido con `--no-cache` y `--env-file .env`?
- [ ] ¿El guard de `next.config.js` sigue intacto (R2)?
- [ ] ¿WebSocket tiene upgrade binding correcto (R4)?
- [ ] ¿Todos los servicios productores tienen `DATABASE_URL` (R6)?
- [ ] ¿Verifiqué el pipeline E2E con la secuencia de R7?
- [ ] ¿Actualicé `.agents/memory/anti_reincidencia.md` con este incidente?
- [ ] ¿Corroboré visual o funcionalmente que la solución opera estable en el VPS?
- [ ] ¿La UI permanece funcional si el WebSocket falla (fallback REST)?
- [ ] ¿Mi respuesta incluye evidencia concreta, no solo afirmaciones?
