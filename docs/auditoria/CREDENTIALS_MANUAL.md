# ArbitrageX-V2: Manual de Credenciales y Variables de Entorno

> **Version:** 1.0.0 | **Fecha:** 2026-05-14 | **Classificacion:** T0-T3 (ver Secrets Policy)

---

## 1. Visión General

Este documento cataloga **todas las credenciales, API keys, tokens y variables de entorno** requeridas para operar la plataforma ArbitrageX-V2. Las credenciales se clasifican por tier de seguridad (T0-T3), obligatoriedad y modo (paper-shadow vs live).

### Fuentes de Verdad Consultadas

| Fuente | Proposito |
|--------|-----------|
| `.env.example` (raiz) | Variables backend principales |
| `frontend/.env.example` | Variables frontend |
| `docker/compose.prod.yml` | Requisitos produccion |
| `docker/compose.dev.yml` | Requisitos desarrollo |
| `configs/app.toml` | Configuracion canonical (no secretos) |
| `configs/secrets.policy.md` | Politica de clasificacion y rotacion |
| `backend/api-server/src/index.ts` | Validacion SECURE_BOOT |
| `killswitch.json` | Estado killswitch legacy |

---

## 2. Clasificacion de Secretos (Secrets Policy)

| Tier | Nivel | Ejemplos | Impacto si filtrado |
|------|-------|----------|---------------------|
| **T0** | Ejecucion critica | `FLASHBOTS_SIGNER_KEY`, `arbx_migrator` password | Perdida directa de capital o corrupcion total de DB |
| **T1** | Acceso critico | `ARBX_ADMIN_TOKEN`, `JWT_SECRET`, `ARBX_RW_PASSWORD` | Takeover administrativo, bypass de killswitch |
| **T2** | API critico | `ARBX_EDGE_TOKEN`, `GOPLUS_API_KEY`, RPC provider keys | Degradacion de servicio, exceso de quotas |
| **T3** | Bajo riesgo | Grafana read-only user, tokens internos scope-reducido | Leve disclosure de informacion |

---

## 3. Credenciales Obligatorias (Tier T0 - Sin estas el sistema NO arranca)

### 3.1 Tokens de Autenticacion de Servicios

| Variable | Formato | Generacion | Uso | Tier |
|----------|---------|------------|-----|------|
| `ARBX_ADMIN_TOKEN` | Base64, >= 32 bytes | `openssl rand -base64 48` | Admin endpoints: killswitch, config, audit logs, chains CRUD | T1 |
| `ARBX_EDGE_TOKEN` | Base64, >= 32 bytes | `openssl rand -base64 48` | Service-to-service auth (edge <> api-server) | T1 |
| `JWT_SECRET` | Base64, >= 32 bytes | `openssl rand -base64 64` | JWT para frontend -> edge authentication | T1 |

> **⚠️ ADVERTENCIA SEGURIDAD (SECURE_BOOT):** El api-server se NIEGA a arrancar si estos tokens estan vacios, son placeholders conocidos (`REPLACE_ME`, `change_me`) o tienen < 32 bytes de entropia. Mensaje de error: `config.boot fail missing=[ARBX_ADMIN_TOKEN]`.

### 3.2 Base de Datos PostgreSQL

| Variable | Formato | Ejemplo | Uso |
|----------|---------|---------|-----|
| `DATABASE_URL` | `postgres://<user>:<pass>@<host>:<port>/<db>` | `postgres://arbx_rw:REPLACE_ME@postgres:5432/arbitragex` | Conexion read-write para todos los servicios |
| `DATABASE_READONLY_URL` | `postgres://<user>:<pass>@<host>:<port>/<db>` | `postgres://arbx_ro:REPLACE_ME@postgres:5432/arbitragex` | Conexion read-only para recon/reporting |
| `DATABASE_MIGRATOR_URL` | `postgres://<user>:<pass>@<host>:<port>/<db>` | `postgres://arbx_migrator:REPLACE_ME@postgres:5432/arbitragex` | DDL/migraciones (superuser) |
| `ARBX_MIGRATOR_PASSWORD` | String seguro | Generado manualmente | Password del rol migrador |
| `ARBX_RW_PASSWORD` | String seguro | Generado manualmente | Password del rol read-write |
| `ARBX_RO_PASSWORD` | String seguro | Generado manualmente | Password del rol read-only |
| `POSTGRES_PASSWORD` | String seguro | Generado manualmente | Password del usuario postgres principal |

> **⚠️ ADVERTENCIA SEGURIDAD:** En produccion (compose.prod.yml), todas usan `${VAR:?message}` - fallan inmediatamente si estan vacias. No hay valores por defecto.

### 3.3 Redis

| Variable | Formato | Ejemplo | Uso |
|----------|---------|---------|-----|
| `REDIS_URL` | `redis://<host>:<port>` | `redis://redis:6379` | Pub/sub, cache de killswitch, config hot-reload |
| `REDIS_DB_DEFAULT` | Integer | `0` | DB principal |
| `REDIS_DB_ADMIN` | Integer | `1` | DB administrativa |

### 3.4 RPC Endpoints (Mainnet - Chain 1)

| Variable | Formato | Ejemplo | Uso |
|----------|---------|---------|-----|
| `RPC_HTTP_1` | `name=url,name2=url2` | `alchemy=https://eth-mainnet...` | RPC HTTP para Ethereum mainnet (chain_id=1) |
| `RPC_WS_1` | `name=url,name2=url2` | `alchemy=wss://eth-mainnet...` | RPC WebSocket para mainnet |

> **⚠️ ADVERTENCIA SEGURIDAD:** Nunca expongas RPCs publicamente. Edge NO debe poder alcanzarlos directamente. Minimo DOS proveedores por chain para failover. Ver `G-RPC-1`.

---

## 4. Credenciales por Sprint/Fase

### 4.1 Sprint 1 (Foundations) - Actual

| Variable | Requerida | Formato | Uso |
|----------|-----------|---------|-----|
| `ENV` | Si | `development\|staging\|production` | Perfil de runtime |
| `RUST_LOG` | No | `info\|debug\|warn\|error` | Nivel de logging Rust |
| `NODE_ENV` | Si | `development\|production` | Nivel de logging Node |
| `SELECTOR_PORT` | Si | `3002` | Puerto selector-api |
| `SIM_PORT` | Si | `3003` | Puerto sim-ctl |
| `RECON_PORT` | Si | `3004` | Puerto recon |
| `RELAYS_PORT` | Si | `3005` | Puerto relays-client |
| `API_PORT` | Si | `8080` | Puerto api-server |
| `EDGE_PORT` | Si | `8787` | Puerto edge |
| `FRONTEND_PORT` | Si | `5173` | Puerto frontend dev |
| `PROMETHEUS_PORT` | Si | `9090` | Puerto Prometheus |
| `GRAFANA_PORT` | Si | `3000` | Puerto Grafana |
| `ALERTMANAGER_PORT` | Si | `9093` | Puerto Alertmanager |
| `LOKI_PORT` | Si | `3100` | Puerto Loki |

### 4.2 Sprint 2 (Detection) - RPC Real

| Variable | Requerida | Formato | Uso |
|----------|-----------|---------|-----|
| `RPC_HTTP_1` | **Si** | Comma-separated `name=url` | RPC HTTP mainnet |
| `RPC_WS_1` | **Si** | Comma-separated `name=url` | RPC WS mainnet |
| `RPC_HTTP_137` | No (si no habilitas Polygon) | `name=url` | RPC HTTP Polygon |
| `RPC_WS_137` | No | `name=url` | RPC WS Polygon |
| `RPC_HTTP_42161` | No (si no habilitas Arbitrum) | `name=url` | RPC HTTP Arbitrum |
| `RPC_WS_42161` | No | `name=url` | RPC WS Arbitrum |

**Formato de RPC variables:**
```bash
# Multi-proveedor (recomendado)
RPC_HTTP_1=alchemy=https://eth-mainnet.g.alchemy.com/v2/<KEY>,infura=https://mainnet.infura.io/v3/<KEY>,ankr=https://rpc.ankr.com/eth/<KEY>

# Proveedor unico (aceptado, emite warning en boot)
RPC_HTTP_1=https://eth-mainnet.g.alchemy.com/v2/<KEY>
```

### 4.3 Sprint 4 (Simulacion) - Anvil/Tenderly

| Variable | Requerida | Formato | Uso |
|----------|-----------|---------|-----|
| `ANVIL_FORK_URL` | **Si (para sim)** | URL RPC | Fork de mainnet para simulaciones |
| `TENDERLY_PROJECT` | No | `usuario/proyecto` | Proyecto Tenderly |
| `TENDERLY_API_KEY` | No | String API key | Simulacion via Tenderly |

### 4.4 Sprint 5 (Ejecucion) - Flashbots y Relays

| Variable | Requerida | Formato | Uso |
|----------|-----------|---------|-----|
| `FLASHBOTS_SIGNER_KEY` | **Si (para live)** | Hex private key (0x...) | Firma de bundles Flashbots |
| `FLASHBOTS_RELAY_URL` | No | URL | Default: `https://relay.flashbots.net` |
| `BLOXROUTE_AUTH` | No | String auth | Autorizacion BloxRoute |
| `EDEN_AUTH` | No | String auth | Autorizacion Eden Network |

> **⚠️ ADVERTENCIA SEGURIDAD (T0):** `FLASHBOTS_SIGNER_KEY` es T0 - ejecucion critica. Esta clave NO debe tener fondos en S1-S4. Solo firma bundles, no paga gas. Rotar ANTES del primer test con capital real.

### 4.5 Sprint 3 (Selector + Risk) - APIs Externas

| Variable | Requerida | Formato | Uso |
|----------|-----------|---------|-----|
| `GOPLUS_API_KEY` | No | String API key | Token safety scoring |
| `HONEYPOT_IS_API_KEY` | No | String API key | Honeypot detection |

### 4.6 Produccion - Vault + Thanos + MinIO

| Variable | Requerida | Formato | Uso |
|----------|-----------|---------|-----|
| `MINIO_ROOT_USER` | **Si (prod)** | String | Usuario MinIO para Thanos |
| `MINIO_ROOT_PASSWORD` | **Si (prod)** | String seguro | Password MinIO |
| `SLACK_WEBHOOK_URL` | No | URL webhook | Alertas Slack |
| `PAGERDUTY_INTEGRATION_KEY` | No | String key | PagerDuty alerts |

### 4.7 Pool Enumeration + TheGraph (enumeracion de pools 24/7/365)

> Config read-only (T2/T3): no toca capital, signer ni broadcast. La ancla on-chain
> (Alchemy) se inyecta siempre; estas variables configuran las fuentes indexadoras.
> Ver `docs/reference/secret-flow.md`.

| Variable | Requerida | Formato | Uso |
|----------|-----------|---------|-----|
| `ARBX_POOL_ENUM_MODE` | **Si (prod)** | `shadow` | Spawnea el worker. **Debe ser `shadow`** (read-only). Nunca `live`/`on`. |
| `ARBX_POOL_ENUM_SOURCES` | No | CSV | Fuentes indexadoras (`thegraph,dexscreener,defillama,geckoterminal`). La ancla Alchemy se añade siempre. |
| `ARBX_POOL_ENUM_TOP_N` | No | int | Top-N pools por TVL (default 500). |
| `ARBX_POOL_ENUM_MIN_TVL_USD` | No | number | TVL mínimo (default 50000). |
| `ARBX_POOL_ENUM_MAX_NEW_PER_TICK` | No | int | Nuevos pools por tick (default 50). |
| `ARBX_POOL_ENUM_ONCHAIN_LOOKBACK` | No | int (blocks) | Ventana de scan de eventos factory de la ancla on-chain (default 7200, ~1 día mainnet). |
| `ARBX_POOL_ENUM_DEXSCREENER_BASE` | **Si (si dexscreener activo)** | URL host-root | **HOST ROOT solo** (`https://api.dexscreener.com`). NUNCA el path del oracle `DEXSCREENER_BASE_URL` (duplica el path → 404). |
| `ARBX_SUBGRAPH_URL_<chain>` | No | URL (key inline) | Endpoint TheGraph por chain. `subgraph_client.rs:54` la lee **literal** (sin sustituir `<KEY>`): pega la API key real inline. Free: `thegraph.com/studio` (100k queries/mes). |
| `THEGRAPH_API_KEY` | No | String | Referencia/documentación (el código lee la URL, no esta var). Tier T2. |
| `DEFILLAMA_BASE_URL` | No | URL | `https://yields.llama.fi` (la API yields usa IDs no-on-chain; aporta pocos). |
| `GECKOTERMINAL_BASE_URL` | No | URL | `https://api.geckoterminal.com/api/v2`. |


---

## 5. Credenciales Frontend

| Variable | Formato | Obligatoria | Uso |
|----------|---------|-------------|-----|
| `NEXT_PUBLIC_EDGE_URL` | URL HTTP | **Si** | URL del edge server (REST API) |
| `NEXT_PUBLIC_WS_URL` | URL WS/WSS | **Si** | URL del WebSocket server |
| `NEXT_PUBLIC_WALLETCONNECT_PROJECT_ID` | String (hex) | No (degrada fail-honest) | ProjectId de WalletConnect/Reown (`cloud.walletconnect.com`). Si ausente, WalletConnect degrada (`walletconnect_project_id_missing`); MetaMask/EIP-6963 siguen funcionando. **Vive en `frontend/.env`, NO en el vault VPS** (build-time). |
| `NEXT_PUBLIC_DEFAULT_CHAIN_ID` | int | Si | Chain por defecto (p.ej. 1) |
| `NEXT_PUBLIC_SUPPORTED_CHAINS` | CSV int | Si | Chains soportadas |
| `NEXT_PUBLIC_WALLET_CONNECT_ENABLED` | bool | No | Habilita WalletConnect |

**Valores tipicos:**
```bash
# Desarrollo
NEXT_PUBLIC_EDGE_URL=http://localhost:8787
NEXT_PUBLIC_WS_URL=http://localhost:3000

# Produccion
NEXT_PUBLIC_EDGE_URL=https://edge.tu-dominio.com
NEXT_PUBLIC_WS_URL=wss://ws.tu-dominio.com
```

> **⚠️ ADVERTENCIA:** `NEXT_PUBLIC_*` se inyectan en BUILD TIME por Next.js. Cualquier cambio requiere re-build.

---

## 6. Credenciales Opcionales (Mejoran funcionalidad)

| Variable | Requerida | Formato | Mejora |
|----------|-----------|---------|--------|
| `GITHUB_TOKEN` | No | String token | Mejora rate limits del CDN de Trust Wallet (token enricher) |
| `GRAFANA_ADMIN_USER` | No | String | Default: `admin` |
| `GRAFANA_ADMIN_PASSWORD` | No (dev) / **Si (prod)** | String | Password admin Grafana. En prod via Docker secret |
| `GRAFANA_ROOT_URL` | No | URL | Default: `http://localhost:3000` |
| `INTERNAL_GETH_HTTP` | No | URL | Geth local interno |
| `INTERNAL_GETH_WS` | No | URL | Geth WS local interno |
| `ADMIN_RATE_LIMIT_PER_MIN` | No | Integer | Default: 30. Rate limit para endpoints /admin |
| `READINESS_CACHE_TTL_MS` | No | Integer | Default: 5000. TTL cache readiness |

---

## 7. Credenciales por Modo: Paper-Shadow vs Live

### 7.1 Paper-Shadow Mode (DEFAULT)

En modo paper, **NINGUNA credencial T0 de ejecucion es necesaria**:

```
NO se necesitan:
  - FLASHBOTS_SIGNER_KEY
  - BLOXROUTE_AUTH
  - EDEN_AUTH
  - TENDERLY_API_KEY (a menos que uses simulacion Tenderly)

SI se necesitan:
  - Toda credencial T1/T2 de infraestructura
  - RPC_HTTP_1 / RPC_WS_1 (para leer estado on-chain)
  - ARBX_ADMIN_TOKEN, ARBX_EDGE_TOKEN, JWT_SECRET
  - DATABASE_URL, REDIS_URL
```

### 7.2 Live Mode (Transicion S5+)

```
TODAS las credenciales T0 se vuelven obligatorias:
  + FLASHBOTS_SIGNER_KEY (firma de bundles)
  + BLOXROUTE_AUTH (ejecucion via BloxRoute)
  + EDEN_AUTH (ejecucion via Eden)
  + Todas las RPC para cada chain habilitada
```

---

## 8. URLs de Servicios Externos

| Servicio | URL Interna (Docker) | URL Externa (Dev) |
|----------|---------------------|-------------------|
| PostgreSQL | `postgres://postgres:5432/arbitragex` | `127.0.0.1:5432` |
| Redis | `redis://redis:6379` | `127.0.0.1:6379` |
| api-server | `http://api-server:8080` | `http://localhost:8080` |
| selector-api | `http://selector-api:3002` | `http://localhost:3002` |
| sim-ctl | `http://sim-ctl:3003` | `http://localhost:3003` |
| recon | `http://recon:3004` | `http://localhost:3004` |
| relays-client | `http://relays-client:3005` | `http://localhost:3005` |
| searcher-rs | `http://searcher-rs:9001` | `http://localhost:9001` |
| edge | `http://edge:8787` | `http://localhost:8787` |
| frontend | `http://frontend:5173` | `http://localhost:5173` |
| Prometheus | `http://prometheus:9090` | `http://localhost:9090` |
| Grafana | `http://grafana:3000` | `http://localhost:3000` |
| Loki | `http://loki:3100` | `http://localhost:3100` |
| Alertmanager | `http://alertmanager:9093` | `http://localhost:9093` |
| Vault | `https://vault:8200` | `https://127.0.0.1:8200` |
| MinIO | `http://minio:9000` | `http://127.0.0.1:9000` |
| MinIO Console | `http://minio:9001` | `http://127.0.0.1:9001` |
| Anvil (sim) | `http://anvil:8545` | `http://localhost:8545` |

---

## 9. Rotacion de Secretos

| Secreto | Cadencia | Trigger inmediato |
|---------|----------|-------------------|
| T0 signer keys (`FLASHBOTS_SIGNER_KEY`) | Antes de primer test real; luego en incidente | Exposicion sospechada; offboarding de operator |
| T0 DB migrator password | Cada 90 dias | Investigacion de fallo de migracion con acceso shell |
| T1 admin tokens (`ARBX_ADMIN_TOKEN`, `ARBX_EDGE_TOKEN`) | Cada 30 dias + por deploy | Compromiso, cambio de staff, filtrado a chat/logs |
| T1 DB rw password | Cada 90 dias | Mismo que arriba |
| T1 JWT secret | Cada 90 dias | Invalida todas las sesiones - programar en baja demanda |
| T2 API keys (`GOPLUS_API_KEY`, RPC keys) | Cada 180 dias | Abuso sospechoso / anomalia en quotas |

### Procedimiento de Rotacion

```bash
# 1. Armar killswitch (si es emergencia)
curl -X POST http://localhost:8080/admin/killswitch \
  -H "Content-Type: application/json" \
  -H "x-arbx-admin-token: $ARBX_ADMIN_TOKEN" \
  -d '{"enabled":true,"reason":"rotacion de secretos - sospecha de filtrado"}'

# 2. Generar nuevo token
openssl rand -base64 48    # Para ARBX_ADMIN_TOKEN, ARBX_EDGE_TOKEN
openssl rand -base64 64    # Para JWT_SECRET

# 3. Actualizar en Vault (produccion)
vault kv put secret/arbitragex/prod/<path> value=<nuevo>

# 4. Restart servicios afectados
docker compose -f docker/compose.prod.yml restart \
  api-server edge relays-client searcher-rs recon selector-api sim-ctl

# 5. Verificar /status

# 6. Desarmar killswitch (si era emergencia)
```

---

## 10. Inventario Completo de Variables

### Variables de Entorno Backend

| # | Variable | Tipo | Requerida | Tier | Notas |
|---|----------|------|-----------|------|-------|
| 1 | `ENV` | String | Si | T3 | `development`, `staging`, `production` |
| 2 | `RUST_LOG` | String | No | T3 | `info`, `debug`, `warn`, `error` |
| 3 | `NODE_ENV` | String | Si | T3 | `development`, `production` |
| 4 | `SELECTOR_PORT` | Integer | Si | T3 | Default: 3002 |
| 5 | `SIM_PORT` | Integer | Si | T3 | Default: 3003 |
| 6 | `RECON_PORT` | Integer | Si | T3 | Default: 3004 |
| 7 | `RELAYS_PORT` | Integer | Si | T3 | Default: 3005 |
| 8 | `API_PORT` | Integer | Si | T3 | Default: 8080 |
| 9 | `EDGE_PORT` | Integer | Si | T3 | Default: 8787 |
| 10 | `FRONTEND_PORT` | Integer | Si | T3 | Default: 5173 |
| 11 | `PROMETHEUS_PORT` | Integer | Si | T3 | Default: 9090 |
| 12 | `GRAFANA_PORT` | Integer | Si | T3 | Default: 3000 |
| 13 | `ALERTMANAGER_PORT` | Integer | Si | T3 | Default: 9093 |
| 14 | `LOKI_PORT` | Integer | Si | T3 | Default: 3100 |
| 15 | `DATABASE_URL` | URL | **Si** | T1 | Conexion RW PostgreSQL |
| 16 | `DATABASE_READONLY_URL` | URL | No | T1 | Conexion RO PostgreSQL |
| 17 | `DATABASE_MIGRATOR_URL` | URL | No | T0 | Solo migraciones |
| 18 | `ARBX_MIGRATOR_PASSWORD` | String | **Si (prod)** | T0 | Password rol migrador |
| 19 | `ARBX_RW_PASSWORD` | String | **Si (prod)** | T1 | Password rol RW |
| 20 | `ARBX_RO_PASSWORD` | String | **Si (prod)** | T1 | Password rol RO |
| 21 | `POSTGRES_PASSWORD` | String | **Si (prod)** | T1 | Password postgres principal |
| 22 | `REDIS_URL` | URL | **Si** | T1 | `redis://host:port` |
| 23 | `REDIS_DB_DEFAULT` | Integer | No | T2 | Default: 0 |
| 24 | `REDIS_DB_ADMIN` | Integer | No | T2 | Default: 1 |
| 25 | `RPC_HTTP_1` | CSV pairs | **Si (S2+)** | T2 | Mainnet HTTP RPCs |
| 26 | `RPC_WS_1` | CSV pairs | **Si (S2+)** | T2 | Mainnet WS RPCs |
| 27 | `RPC_HTTP_137` | CSV pairs | No | T2 | Polygon HTTP RPCs |
| 28 | `RPC_WS_137` | CSV pairs | No | T2 | Polygon WS RPCs |
| 29 | `ANVIL_FORK_URL` | URL | **Si (S4)** | T2 | RPC para fork Anvil |
| 30 | `TENDERLY_PROJECT` | String | No | T2 | `usuario/proyecto` |
| 31 | `TENDERLY_API_KEY` | String | No | T2 | API key Tenderly |
| 32 | `FLASHBOTS_SIGNER_KEY` | Hex | **Si (S5+)** | T0 | Clave privada firma bundles |
| 33 | `FLASHBOTS_RELAY_URL` | URL | No | T2 | Default: relay.flashbots.net |
| 34 | `BLOXROUTE_AUTH` | String | No | T2 | Auth BloxRoute |
| 35 | `EDEN_AUTH` | String | No | T2 | Auth Eden Network |
| 36 | `ARBX_ADMIN_TOKEN` | Base64 | **Si** | T1 | Admin API token |
| 37 | `ARBX_EDGE_TOKEN` | Base64 | **Si** | T1 | Edge API token |
| 38 | `JWT_SECRET` | Base64 | **Si** | T1 | JWT para frontend |
| 39 | `JWT_ISSUER` | String | No | T2 | Default: `arbitragex-v2` |
| 40 | `JWT_AUDIENCE` | String | No | T2 | Default: `arbitragex-frontend` |
| 41 | `GOPLUS_API_KEY` | String | No | T2 | Token safety API |
| 42 | `HONEYPOT_IS_API_KEY` | String | No | T2 | Honeypot detection API |
| 43 | `GRAFANA_ADMIN_USER` | String | No | T3 | Default: `admin` |
| 44 | `GRAFANA_ADMIN_PASSWORD` | String | No | T1 | Solo dev default |
| 45 | `GRAFANA_ROOT_URL` | URL | No | T3 | Default: localhost:3000 |
| 46 | `LOKI_URL` | URL | No | T3 | Default: `http://loki:3100` |
| 47 | `PROMETHEUS_URL` | URL | No | T3 | Default: `http://prometheus:9090` |
| 48 | `INTERNAL_GETH_HTTP` | URL | No | T2 | `http://geth:8545` |
| 49 | `INTERNAL_GETH_WS` | URL | No | T2 | `ws://geth:8546` |
| 50 | `ENRICHER_CHAINS` | CSV ints | **Si (prod)** | T2 | Ej: `1,137,42161` |
| 51 | `GITHUB_TOKEN` | String | No | T2 | Rate limits Trust Wallet CDN |
| 52 | `MINIO_ROOT_USER` | String | **Si (prod)** | T1 | Usuario MinIO |
| 53 | `MINIO_ROOT_PASSWORD` | String | **Si (prod)** | T1 | Password MinIO |
| 54 | `SLACK_WEBHOOK_URL` | URL | No | T2 | Alertas Slack |
| 55 | `PAGERDUTY_INTEGRATION_KEY` | String | No | T2 | PagerDuty key |

### Variables Frontend

| # | Variable | Formato | Requerida | Notas |
|---|----------|---------|-----------|-------|
| 56 | `NEXT_PUBLIC_EDGE_URL` | URL | **Si** | Edge REST API |
| 57 | `NEXT_PUBLIC_WS_URL` | URL | **Si** | WebSocket endpoint |

---

## 11. Red Flags de Seguridad

> **⚠️ NUNCA:**
> - Commitea un archivo `.env` real al repo (esta en `.gitignore` + `gitleaks` pre-push hook)
> - Pegar secretos en chat, email, issues, o logs
> - Incluir secretos en codigo fuente, Dockerfiles, o CI variables expuestas a PRs
> - Usar placeholders (`REPLACE_ME`, `changeme`) en produccion
> - Loggear valores secretos. Los loaders deben redactar antes de loggear (`*****` con prefijo de hash)

> **⚠️ VERIFICAR:**
> ```bash
> # Revisa que no hayas commiteado secretos accidentalmente
grep -r "sk-[a-zA-Z0-9]\{20,\}" . --include="*.ts" --include="*.js" --include="*.md"
grep -r "0x[0-9a-f]\{64\}" . --include="*.ts" --include="*.js" --include="*.md"
```

---

## 12. Troubleshooting

| Sintoma | Causa probable | Solucion |
|---------|---------------|----------|
| `config.boot fail missing=[ARBX_ADMIN_TOKEN]` | Token vacio o < 32 bytes | Generar con `openssl rand -base64 48` |
| `DATABASE_URL required` | Variable no definida en `.env` | Verificar `.env` contra `.env.example` |
| `POSTGRES_PASSWORD required for prod stack` | Usando compose.prod sin secretos | Vault-agent debe renderizar `/run/secrets/arbx.env` |
| `RPC_WS_1 required for mainnet detection` | Falta RPC para chain 1 | Agregar al menos un proveedor HTTP + WS |
| `single-vendor warn at boot` | Solo un proveedor RPC | Agregar segundo proveedor para failover |
| `Redis connection refused` | Redis no esta corriendo | `docker compose ps redis` / verificar REDIS_URL |
| `jwt malformed` en frontend | JWT_SECRET cambiado | Recargar pagina (token se regenera) |

---

*Documento generado el 2026-05-14. Verificar contra la ultima version en `docs/operations/SECRETS_POLICY.md`.*
