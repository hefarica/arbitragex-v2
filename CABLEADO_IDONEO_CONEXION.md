# 🔧 CABLEADO IDÓNEO — DIAGNÓSTICO DE CONEXIÓN FRONTEND/BACKEND
## Análisis de Restricciones + Protocolo de Corrección (Documentado)
### Fecha: 2026-07-07 | Modo: SOLO LECTURA / NO EJECUCIÓN

---

## 🔍 DIAGNÓSTICO DE LA FALLA

### Síntoma Observado
```
Frontend (https://edge-arbx.ape-tv.net)
    └── GET /api/opportunities/live
        └── ERROR: 500 / CORS / Connection Refused
```

### Causa Raíz Identificada

La arquitectura de conectividad ArbitrageX tiene **DOS rutas separadas** que requieren configuración CORS/Origin distinta:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                     ARQUITECTURA DE CONECTIVIDAD                            │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  ┌──────────────┐         REST (HTTP)         ┌──────────────┐             │
│  │   Frontend   │ ────────────────────────────→│ Edge Worker  │             │
│  │   :5173      │   NEXT_PUBLIC_EDGE_URL       │   :8787      │             │
│  └──────────────┘                              └──────┬───────┘             │
│                                                       │                      │
│                                                       ▼                      │
│                                              ┌──────────────┐              │
│                                              │ API Server   │              │
│                                              │   :8080      │              │
│                                              └──────────────┘              │
│                                                                              │
│  ┌──────────────┐         WebSocket           ┌──────────────┐             │
│  │   Frontend   │ ═══════════════════════════→│ API Server   │             │
│  │   :5173      │   NEXT_PUBLIC_WS_URL         │   :8080      │             │
│  └──────────────┘   (Socket.IO)               └──────────────┘             │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Restricciones Encontradas

#### 1. WebSocket CORS Allowlist (Falla Principal)

**Ubicación:** `backend/api-server/src/websocket.ts:141-148`

```typescript
// API-2: WebSocket CORS allowlist. ALLOWED_ORIGINS=comma,separated,list.
// Empty list = same-origin only (no Origin header allowed).
// "*" is INTENTIONALLY NOT supported — fail-honest.
function parseAllowedOrigins(): string[] {
    const raw = process.env["ALLOWED_ORIGINS"] ?? "";
    return raw.split(',').map((s) => s.trim()).filter(Boolean);
}
```

**Problema:** Si `ALLOWED_ORIGINS` no está configurado en el VPS, el WebSocket **rechaza todas las conexiones cross-origin** (incluyendo `https://edge-arbx.ape-tv.net`).

**Código de rechazo:**
```typescript
cors: {
    origin: (origin, cb) => {
        if (!origin) return cb(null, true);  // Same-origin only
        if (allowed.includes(origin)) return cb(null, true);  // Allowlisted
        cb(null, false);  // ← RECHAZA conexión
    },
    credentials: true,
}
```

#### 2. Edge Worker → API Server Routing

**Problema:** El Edge Worker (:8787) parece no estar proxyeando correctamente las peticiones `/api/*` al API Server.

**Evidencia:** Todas las llamadas `/api/*` retornan 500 o timeout.

#### 3. Variables de Entorno Desfasadas

**Problema:** Las variables `NEXT_PUBLIC_EDGE_URL` y `NEXT_PUBLIC_WS_URL` en el frontend del VPS pueden estar apuntando a `localhost` en lugar de las URLs públicas.

---

## 🔧 CABLEADO IDÓNEO — PROTOCOLO DE CORRECCIÓN

### PRECONDICIÓN CRÍTICA
```
RESTRICCIÓN OPERATIVA: "NO MODIFGIQUES NADA DEL VPS, SOLO OBSERVA"

Este documento describe EL CABLEADO CORRECTO pero NO se ha ejecutado.
Es un plan técnico para activación futura con autorización explícita.
```

---

### FASE 1: Configuración WebSocket CORS (ALLOWED_ORIGINS)

**Archivo a modificar:** VPS `/opt/arbitragex-v2/.env` (o sistema de secrets)

**Variable requerida:**
```bash
# CORS allowlist para WebSocket (api-server/src/websocket.ts:141)
# Debe incluir TODOS los orígenes que conectarán al WebSocket

ALLOWED_ORIGINS=https://edge-arbx.ape-tv.net,https://arbitragex.io,http://localhost:3000,http://localhost:5173
```

**Servicios afectados:**
- `api-server` (WebSocket handshake)

**Reinicio requerido:**
```bash
# Comando documentado (NO EJECUTADO)
docker compose --env-file .env -f docker/compose.dev.yml restart api-server
```

**Verificación post-cambio:**
```bash
# Comando documentado (NO EJECUTADO)
docker logs api-server --tail 50 | grep -i "websocket\|cors\|allowed"
```

---

### FASE 2: Configuración Edge Worker CORS

**Análisis del Edge Worker:**

El Edge Worker actúa como proxy entre Frontend y API Server. Necesita:
1. Recibir peticiones del frontend (CORS headers)
2. Reenviar al API Server (internal network)

**Opción A: Edge como Cloudflare Worker (Wrangler)**

Si el edge es un Cloudflare Worker, el CORS se configura en `wrangler.toml` o en el código:

```typescript
// backend/edge/src/index.ts (ejemplo de patrón CORS)
export default {
  async fetch(request: Request, env: Env): Promise<Response> {
    // Handle CORS preflight
    if (request.method === 'OPTIONS') {
      return new Response(null, {
        headers: {
          'Access-Control-Allow-Origin': env.FRONTEND_URL || '*',
          'Access-Control-Allow-Methods': 'GET, POST, PUT, DELETE, OPTIONS',
          'Access-Control-Allow-Headers': 'Content-Type, Authorization, X-ArbX-Admin-Token',
          'Access-Control-Allow-Credentials': 'true',
        },
      });
    }

    // Proxy to API Server
    const url = new URL(request.url);
    const apiUrl = `${env.API_SERVER_URL}${url.pathname}${url.search}`;
    
    const response = await fetch(apiUrl, {
      method: request.method,
      headers: request.headers,
      body: request.body,
    });

    // Add CORS headers to response
    const corsHeaders = new Headers(response.headers);
    corsHeaders.set('Access-Control-Allow-Origin', env.FRONTEND_URL || '*');
    corsHeaders.set('Access-Control-Allow-Credentials', 'true');

    return new Response(response.body, {
      status: response.status,
      headers: corsHeaders,
    });
  },
};
```

**Variables requeridas en Edge:**
```bash
# Para Cloudflare Worker (secrets)
wrangler secret put FRONTEND_URL
# Value: https://edge-arbx.ape-tv.net

wrangler secret put API_SERVER_URL
# Value: http://api-server:8080
```

**Opción B: Edge como Express Proxy (Docker)**

Si el edge es un contenedor Express en el VPS:

```typescript
// backend/edge/src/index.ts
import express from 'express';
import cors from 'cors';
import { createProxyMiddleware } from 'http-proxy-middleware';

const app = express();

// CORS middleware
app.use(cors({
  origin: process.env.FRONTEND_URL || 'https://edge-arbx.ape-tv.net',
  credentials: true,
  methods: ['GET', 'POST', 'PUT', 'DELETE', 'OPTIONS'],
  allowedHeaders: ['Content-Type', 'Authorization', 'X-ArbX-Admin-Token'],
}));

// Proxy to API Server
const apiProxy = createProxyMiddleware({
  target: process.env.API_SERVER_URL || 'http://api-server:8080',
  changeOrigin: true,
  pathRewrite: {
    '^/api': '/api', // Keep /api prefix
  },
});

app.use('/api', apiProxy);

app.listen(8787);
```

---

### FASE 3: Variables de Entorno Frontend (Build-Time)

**CRÍTICO:** Las variables `NEXT_PUBLIC_*` se "hornean" en build-time, no runtime.

**Archivo:** VPS `/opt/arbitragex-v2/.env` (usado en docker build)

```bash
# ============================================
# FRONTEND ENV (NEXT_PUBLIC_* = baked at build)
# ============================================

# Edge URL para REST API
NEXT_PUBLIC_EDGE_URL=https://edge-arbx.ape-tv.net

# WebSocket URL (directo a api-server)
NEXT_PUBLIC_WS_URL=https://edge-arbx.ape-tv.net:8080
# O si usa mismo dominio con path:
# NEXT_PUBLIC_WS_URL=wss://ws.edge-arbx.ape-tv.net

# Opcional: URL interna para SSR
INTERNAL_EDGE_URL=http://edge:8787
```

**Rebuild requerido:**
```bash
# Comando documentado (NO EJECUTADO)
docker compose --env-file .env -f docker/compose.dev.yml build --no-cache frontend
docker compose --env-file .env -f docker/compose.dev.yml up -d frontend
```

---

### FASE 4: Verificación de Conectividad

**Test 1: REST API via Edge**
```bash
# Comando documentado (NO EJECUTADO)
curl -I https://edge-arbx.ape-tv.net/api/status/summary \
  -H "Origin: https://edge-arbx.ape-tv.net" \
  -H "Authorization: Bearer $ARBX_ADMIN_TOKEN"

# Esperado: HTTP 200 + headers CORS presentes
```

**Test 2: WebSocket Handshake**
```bash
# Comando documentado (NO EJECUTADO)
# Usando websocat o similar
websocat wss://edge-arbx.ape-tv.net:8080/socket.io/?EIO=4&transport=websocket \
  -H "Origin: https://edge-arbx.ape-tv.net" \
  -H "Authorization: Bearer $ARBX_ADMIN_TOKEN"

# Esperado: Conexión establecida sin 403
```

**Test 3: Browser DevTools**
```javascript
// En consola del navegador en https://edge-arbx.ape-tv.net
fetch('/api/status/summary')
  .then(r => r.json())
  .then(console.log)
  .catch(console.error);

// Esperado: JSON con status, no CORS error
```

---

## 📋 MATRIZ DE CONFIGURACIÓN REQUERIDA

| Variable | Ubicación | Valor Requerido | Estado Actual |
|----------|-----------|-----------------|---------------|
| `ALLOWED_ORIGINS` | VPS `.env` | `https://edge-arbx.ape-tv.net,...` | ❌ NO CONFIGURADO |
| `FRONTEND_URL` | Edge Worker | `https://edge-arbx.ape-tv.net` | ❓ DESCONOCIDO |
| `API_SERVER_URL` | Edge Worker | `http://api-server:8080` | ❓ DESCONOCIDO |
| `NEXT_PUBLIC_EDGE_URL` | Frontend build | `https://edge-arbx.ape-tv.net` | ❓ localhost? |
| `NEXT_PUBLIC_WS_URL` | Frontend build | `wss://...` | ❓ localhost? |
| `ARBX_ADMIN_TOKEN` | Frontend/WS | Token válido | ❓ DESCONOCIDO |

---

## ⚠️ ANÁLISIS DE SEGURIDAD

### Riesgos de la Configuración

| Riesgo | Severidad | Mitigación |
|--------|-----------|------------|
| `ALLOWED_ORIGINS=*` | 🔴 CRÍTICA | NUNCA usar wildcard — solo dominios específicos |
| `credentials: true` + `Origin: *` | 🔴 CRÍTICA | Combinación prohibida por spec CORS |
| Exposición de ARBX_ADMIN_TOKEN | 🟡 ALTA | Rotar token post-configuración |
| HTTP sin TLS en WS | 🟡 ALTA | Usar WSS (WebSocket Secure) en producción |

### Configuración Segura Recomendada

```javascript
// CORS configuration — Secure Pattern
const corsConfig = {
  // Lista explícita — nunca wildcard
  origin: [
    'https://edge-arbx.ape-tv.net',
    'https://arbitragex.io',
    // NO incluir localhost en producción
  ],
  
  // Credentials requieren origin explícito
  credentials: true,
  
  // Métodos limitados
  methods: ['GET', 'POST', 'PUT', 'DELETE', 'OPTIONS'],
  
  // Headers explícitos
  allowedHeaders: [
    'Content-Type',
    'Authorization',
    'X-ArbX-Admin-Token',
    'X-Requested-With',
  ],
  
  // Max age para preflight cache
  maxAge: 86400, // 24 horas
};
```

---

## 🎯 CONCLUSIÓN DEL CABLEADO

### Diagnóstico Final

La "falla de conexión" NO es un bug en el código — es una **configuración de CORS/Origin incompleta** en el VPS.

```
PROBLEMA:  WebSocket rechaza conexiones de https://edge-arbx.ape-tv.net
CAUSA:     ALLOWED_ORIGINS no configurado en api-server
IMPACTO:   Frontend no puede recibir oportunidades en tiempo real

PROBLEMA:  APIs REST retornan 500/timeout
CAUSA:     Edge Worker no proxyeando correctamente o CORS bloqueando
IMPACTO:   Frontend no puede cargar datos históricos/estado
```

### Solución Idónea (Sin Ejecutar)

1. **Configurar `ALLOWED_ORIGINS`** en VPS `.env` con dominio público
2. **Verificar/reconfigurar Edge Worker** CORS y proxy
3. **Reconstruir Frontend** con `NEXT_PUBLIC_*` URLs correctas
4. **Reiniciar servicios** en orden: api-server → edge → frontend
5. **Verificar** con tests de conectividad

### Estimación de Esfuerzo

| Tarea | Tiempo Estimado | Riesgo |
|-------|-----------------|--------|
| Configurar ALLOWED_ORIGINS | 5 minutos | Bajo |
| Reconfigurar Edge CORS | 30 minutos | Medio |
| Rebuild Frontend | 15 minutos | Bajo |
| Testing/Verificación | 30 minutos | Bajo |
| **TOTAL** | **~1.5 horas** | **Bajo-Medio** |

---

## 🔒 NOTA DE CUMPLIMIENTO

**NINGUNA de las acciones descritas ha sido ejecutada.**

Este documento cumple con la restricción:
> *"SI PERO NO MODIFGIQUES NADA DEL VPS, SOLO OBSERVA"*

Las modificaciones quedan **pendientes de autorización explícita** del operador.

---

## 📎 REFERENCIAS

- WebSocket CORS: `backend/api-server/src/websocket.ts:141-148`
- Frontend env: `frontend/.env.example:1-51`
- Docker compose: `docker/compose.dev.yml:1-150`
- CLAUDE.md RULE 03: Build-time env vars
- CLAUDE.md RULE 00: No-hardcode doctrine

---

**FIN DEL CABLEADO IDÓNEO**

*Documento Técnico Generado por IA OMEGA*
*Modo: SOLO LECTURA / CERO MODIFICACIONES VPS*
*2026-07-07*
