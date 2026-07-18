# Gate 6 — Hardening y Recuperación (Disaster Recovery Playbook)

> Estado: **BLUEPRINT** — diseño aprobado para implementación.  
> Última revisión: 2026-07-18.  
> Modo: shadow/paper/read-only. Capital expuesto = 0.

## 1. Resumen ejecutivo

Este playbook corrige los defectos de infraestructura identificados en la auditoría Gate 6 y establece procedimientos de recuperación ante desastres para los componentes críticos de ArbitrageX v2:

- Redis sin persistencia.
- Montura excesiva del repositorio en `api-server`.
- Token de administrador expuesto al runtime del frontend SSR.
- Healthchecks que aceptan cualquier HTTP `< 500` como saludable.
- Imágenes Docker referenciadas por tag mutable.
- Ausencia de procedimientos documentados de backup, restore, rollback y failover.

Todo el diseño se mantiene en modo **shadow/paper/read-only**. No se activa executor, capital, firmas ni broadcast on-chain.

## 2. Alcance y límites

- **Aplica a:** VPS de producción (`/opt/arbitragex-v2`) y entornos de staging que usen `docker/compose.prod.yml`.
- **No aplica a:** entorno local de desarrollo Windows (`docker/compose.dev.yml`), salvo que el operador decida probar el playbook en un sandbox.
- **Out of scope intencional:** rotación de secrets (ver `docs/operations/VAULT_SETUP.md`), migración a Kubernetes, cambios de red VPC o proveedor cloud. Este playbook asume un único VPS Hetzner con Docker Compose.

## 3. Redis — Persistencia y recuperación

### 3.1 Evaluación RDB vs AOF

| Modo | Ventaja | Desventaja | RPO típico |
|------|---------|------------|------------|
| RDB | Snapshot compacto, restore rápido, bajo overhead | Pérdida de datos desde el último snapshot | Minutos/horas |
| AOF | Máxima durabilidad, RPO cercano a cero | Archivos grandes, replay lento, más I/O | Segundos |
| **RDB + AOF (recomendado)** | Mejor de ambos: snapshot base + log incremental | Mayor uso de disco y CPU | ≤ 1 s con `appendfsync everysec` |

Para ArbitrageX v2 se recomienda **ambos** porque:

1. Las streams de Redis (`arbx:opps:detected`, `arbx:scoring:scored`) son estado transitorio de alta velocidad que se alimenta del scanner y se consume hacia PostgreSQL. Un crash sin persistencia deja al sistema sin contexto reciente.
2. Los contadores y keys de heartbeat (`arbx:heartbeat:*`, `arbx:topology:*`) se reconstruyen lentamente desde cero.
3. El volumen de escritura no es extremo: AOF everysec es aceptable.

### 3.2 Configuración propuesta

Crear `infra/redis/redis.conf` (montado read-only) y actualizar `docker/compose.prod.yml`:

```yaml
redis:
  image: redis:7.2@sha256:<digest-pinned>   # ver §7
  command: ["redis-server", "/usr/local/etc/redis/redis.conf"]
  volumes:
    - redis_data:/data
    - ../infra/redis/redis.conf:/usr/local/etc/redis/redis.conf:ro
  sysctls:
    - net.core.somaxconn=511
  healthcheck:
    test: ["CMD", "redis-cli", "--raw", "PING"]
    interval: 5s
    timeout: 3s
    retries: 20
```

Contenido mínimo de `infra/redis/redis.conf`:

```conf
# Persistencia dual RDB + AOF
save 900 1
save 300 10
save 60 10000
stop-writes-on-bgsave-error yes
rdbcompression yes
rdbchecksum yes
appendonly yes
appendfilename "appendonly.aof"
appendfsync everysec
no-appendfsync-on-rewrite no
auto-aof-rewrite-percentage 100
auto-aof-rewrite-min-size 64mb
aof-load-truncated yes
aof-use-rdb-preamble yes

# Seguridad básica
bind 0.0.0.0
protected-mode no
requirepass ${REDIS_PASSWORD}
rename-command FLUSHDB ""
rename-command FLUSHALL ""
```

> `REDIS_PASSWORD` se inyecta vía `.env` y se referencia en compose con `${REDIS_PASSWORD:?required}`. Nunca se hardcoddea en el archivo de configuración versionado.

### 3.3 RPO esperado

- Con `appendfsync everysec`: **RPO ≤ 1 segundo** para escrituras ya confirmadas.
- RDB snapshot cada 15 minutos como respaldo de recuperación rápida.
- En caso de corrupción de AOF: restaurar desde el último RDB válido; pérdida = datos desde el snapshot.

### 3.4 Backup programado de Redis

```bash
#!/usr/bin/env bash
# /opt/arbitragex-v2/scripts/vps/redis-bgsave.sh
set -euo pipefail
CONTAINER=arbitragex-v2-redis-1
BACKUP_DIR=/var/backups/arbx/redis
TS=$(date -u +%Y%m%dT%H%M%SZ)
mkdir -p "$BACKUP_DIR"
docker exec "$CONTAINER" redis-cli BGSAVE
# Esperar a que el fork termine (poll LASTSAVE)
LAST=$(docker exec "$CONTAINER" redis-cli LASTSAVE)
while [ "$(docker exec "$CONTAINER" redis-cli LASTSAVE)" -eq "$LAST" ]; do
  sleep 1
done
docker cp "$CONTAINER:/data/dump.rdb" "$BACKUP_DIR/dump-${TS}.rdb"
docker cp "$CONTAINER:/data/appendonly.aof" "$BACKUP_DIR/appendonly-${TS}.aof" 2>/dev/null || true
# Retener últimos 48 snapshots + 1 diario de los últimos 30 días
find "$BACKUP_DIR" -name 'dump-*.rdb' -mtime +2 -delete
find "$BACKUP_DIR" -name 'dump-*.rdb' -mtime +30 -not -name '*T000000Z*' -delete
```

Programar en crontab del VPS:

```cron
# Redis incremental AOF + RDB snapshot cada hora
0 * * * * /opt/arbitragex-v2/scripts/vps/redis-bgsave.sh >> /var/log/arbx/redis-bgsave.log 2>&1
```

### 3.5 Restauración desde RDB

1. Detener consumidores y productores que escriban a Redis.
2. Copiar el RDB más reciente validado:
   ```bash
   systemctl stop docker-compose@arbitragex  # o docker compose stop equivalente
   cp /var/backups/arbx/redis/dump-<TS>.rdb /var/lib/docker/volumes/arbitragex-v2_redis_data/_data/dump.rdb
   rm -f /var/lib/docker/volumes/arbitragex-v2_redis_data/_data/appendonly.aof*
   ```
3. Arrancar Redis con solo RDB (`appendonly no` temporal) para validar integridad.
4. Si es válido, reactivar AOF:
   ```bash
   docker exec arbitragex-v2-redis-1 redis-cli CONFIG SET appendonly yes
   ```

## 4. api-server — Reducción de superficie de montura

### 4.1 Estado actual

`docker/compose.prod.yml` monta:

```yaml
volumes:
  - type: bind
    source: /var/lib/arbx
    target: /var/lib/arbx
  - ..:/repo:ro
```

La montura `..:/repo:ro` expone el **repositorio completo** (código fuente, tests, docs, configuraciones) al contenedor. Esto viola el principio de mínimo privilegio y amplía la superficie de ataque.

### 4.2 Lo que realmente necesita api-server

| Recurso | Origen | Uso | Propuesta |
|---------|--------|-----|-----------|
| Binario compilado | `backend/api-server/dist/index.js` | Runtime Node.js | COPY en Dockerfile |
| `node_modules` de producción | builder stage | Dependencias | COPY en Dockerfile |
| Configuración `app.toml` | `configs/app.toml` | Config runtime | COPY en Dockerfile o montura read-only de un directorio de configs |
| Datos persistentes (si aplica) | `/var/lib/arbx` | Archivos de estado, audit logs local | Mantener bind mount |
| Source code / tests / docs | repo root | Verificadores de readiness en runtime | Mover a un **paquete de verificación inmutable** (ver §4.4) |

### 4.3 Dockerfile ajustado

El `backend/api-server/Dockerfile` actual ya copia el binario y dependencias. Solo falta eliminar la montura del repo en compose:

```yaml
api-server:
  build:
    context: ..
    dockerfile: backend/api-server/Dockerfile
  volumes:
    - type: bind
      source: /var/lib/arbx
      target: /var/lib/arbx
    # ELIMINAR: ..:/repo:ro
    # Los verificadores de readiness que necesitan leer archivos del repo deben
    # usar un readiness-bundle inyectado en build time (§4.4).
```

### 4.4 Readiness-bundle inmutable

Los 17 verificadores de live-readiness que actualmente leen archivos del repo deben recibir un snapshot inmutable en build time. Ejemplo de patrón:

```dockerfile
# backend/api-server/Dockerfile (stage adicional)
FROM builder AS readiness-bundle
RUN mkdir -p /bundle && \
    cp -r /build/database/migrations /bundle/migrations && \
    cp -r /build/scripts /bundle/scripts && \
    git rev-parse HEAD > /bundle/REVISION

FROM runtime
# ... existing COPYs ...
COPY --from=readiness-bundle /bundle /repo
```

Así el contenedor sigue viendo `/repo` con solo lo necesario, y no el repositorio completo.

### 4.5 Acciones de seguridad adicionales

- Ejecutar como usuario no-root (`USER arbx` ya presente en Dockerfile).
- Cap-drop ALL y solo añadir `NET_BIND_SERVICE` si escucha en puerto < 1024 (no es el caso).
- Read-only root filesystem:
  ```yaml
  read_only: true
  tmpfs:
    - /tmp:noexec,nosuid,size=50m
  ```
- `security_opt: ["no-new-privileges:true"]`.

## 5. Frontend SSR — Reducción de privilegios del admin token

### 5.1 Uso actual del admin token en SSR

El único uso confirmado en Server Components es en `frontend/lib/api-client.ts::getAuditLogs()`:

```ts
const adminToken = typeof window === "undefined" ? process.env.ARBX_ADMIN_TOKEN : undefined;
```

La página `frontend/app/audit-logs/page.tsx` (Server Component) llama a `getAuditLogs()` y necesita autenticarse contra el edge `/admin/audit` mediante header `x-arbx-admin-token`. El token se toma de la variable de entorno `ARBX_ADMIN_TOKEN` del contenedor frontend.

Esto tiene dos problemas:

1. **Escalada de privilegios:** cualquier Server Component o data fetch del frontend puede potencialmente usar ese token para llamar a endpoints administrativos.
2. **Exposición de secretos en runtime:** aunque no se envía al navegador, el token de administración completo reside en memoria del contenedor frontend.

### 5.2 Diseño propuesto: token de servicio scoped de solo lectura

Introducir un nuevo secret `ARBX_FRONTEND_READONLY_TOKEN` con estas restricciones:

- **Permisos:** solo lectura de endpoints necesarios para SSR (`GET /admin/audit`, `GET /api/admin/chains`, etc.).
- **Tiempo de vida:** corto, rotado automáticamente por Vault/edge.
- **Audiencia:** restringido a `frontend-ssr`.
- **No puede:** mutar configuración, activar killswitch, ver secrets reales.

### 5.3 Cambios necesarios

1. Edge: aceptar un nuevo header `x-arbx-service-token` para llamadas SSR, mapeado internamente a un rol read-only.
2. api-server: verificar `x-arbx-service-token` en endpoints de solo lectura y rechazarlo en mutaciones.
3. Frontend: usar `process.env.ARBX_FRONTEND_READONLY_TOKEN` en lugar de `ARBX_ADMIN_TOKEN` en `getAuditLogs()`.
4. Eliminar `ARBX_ADMIN_TOKEN` del entorno del contenedor frontend por completo.

### 5.4 Alternativa menor esfuerzo (transitoria)

Si no se puede implementar el token scoped de inmediato:

- Auditar todos los Server Components que usan `process.env.ARBX_ADMIN_TOKEN`.
- Asegurar que solo `getAuditLogs()` lo usa.
- Documentar que el frontend no ejecuta mutaciones administrativas en SSR (las mutaciones ya usan `x-arbx-admin-token` desde el cliente con el httpOnly cookie).
- Crear un test automatizado que falle si aparece un nuevo uso de `ARBX_ADMIN_TOKEN` en `frontend/app/**`.

## 6. Healthchecks exactos

### 6.1 Problema

`docker/compose.prod.yml` define para frontend:

```yaml
healthcheck:
  test:
    - CMD-SHELL
    - node -e "require('http').get('http://localhost:5173/',r=>process.exit(r.statusCode<500?0:1)).on('error',()=>process.exit(1))"
```

Esto acepta 401, 403 y 404 como saludables, ocultando problemas reales.

### 6.2 Backend — healthcheck funcional

Reemplazar el healthcheck genérico de `api-server` por uno que verifique DB + Redis + RPC:

```yaml
api-server:
  healthcheck:
    test:
      - CMD-SHELL
      - |
        node -e "
          const http = require('http');
          Promise.all([
            new Promise((res,rej)=>http.get('http://localhost:8080/api/v1/health',r=>res(r.statusCode)).on('error',rej)),
            new Promise((res,rej)=>http.get('http://localhost:8080/api/health',r=>res(r.statusCode)).on('error',rej))
          ]).then(([v1,basic])=>{
            if (v1===200 && basic===200) process.exit(0);
            console.error('health statuses:', {v1,basic});
            process.exit(1);
          }).catch(e=>{ console.error(e); process.exit(1); });
        "
    interval: 30s
    timeout: 10s
    retries: 3
    start_period: 30s
```

El endpoint `/api/v1/health` ya verifica PostgreSQL, Redis y servicios upstream en `backend/api-server/src/routes/health.ts`. Asegurar que devuelve `503` cuando postgres o redis están caídos (actualmente lo hace).

### 6.3 Frontend — healthcheck funcional

Verificar que la página carga, que el bundle está servido y que una ruta crítica devuelve 200:

```yaml
frontend:
  healthcheck:
    test:
      - CMD-SHELL
      - |
        node -e "
          const http = require('http');
          const checks = [
            ['/', 200],
            ['/api/health', 200],
            ['/_next/static/', 200]  // ajustar a un asset real o usar HEAD
          ];
          let ok = 0;
          Promise.all(checks.map(([p,expected])=>new Promise((res)=>{
            http.get('http://localhost:5173'+p,r=>{ if(r.statusCode===expected) ok++; res(); }).on('error',()=>res());
          }))).then(()=>process.exit(ok===checks.length?0:1));
        "
    interval: 30s
    timeout: 10s
    retries: 3
    start_period: 60s
```

> Nota: `/_next/static/` puede requerir una ruta con hash de build. Preferir generar un asset conocido en build time o usar el manifesto.

### 6.4 Healthcheck de servicios Rust

Para servicios sin endpoint HTTP completo (searcher-rs), mantener `/health` pero añadir una verificación semántica mínima:

```bash
# ejemplo para searcher-rs
wget -qO- http://localhost:9001/health | grep -q '"status":"ok"'
```

## 7. Imágenes Docker — Pin a digest

### 7.1 Problema

Las imágenes base usan tags mutables (`redis:7.2`, `postgres:15`, `ghcr.io/foundry-rs/foundry:latest`). Un tag re-publicado puede introducir código no auditado.

### 7.2 Política

- Todas las imágenes oficiales y third-party deben referenciarse por **digest SHA256**.
- Actualizar digests solo después de revisar release notes y, idealmente, prueba en staging.
- Registrar el digest usado en `.superpowers/sdd/gate6-image-digests.md` (o similar) con fecha y quien lo aprobó.

### 7.3 Ejemplo

```yaml
redis:
  image: redis:7.2@sha256:96c8e1d5a8d3f8b1f1c9e6a5b2e8c4d7f3a1b9e2c5d8f4a7b1e3c6d9f2a5b8c1
```

> Los digests reales deben obtenerse con `docker pull redis:7.2` y copiar el digest reportado. No inventar digests.

### 7.4 Procedimiento de actualización controlada

```bash
# 1. Obtener nuevo digest
NEW_DIGEST=$(docker pull redis:7.2 2>/dev/null | awk '/Digest:/{print $2}')
# 2. Validar en staging
ARBX_IMAGE_DIGEST_REDIS=$NEW_DIGEST docker compose -f docker/compose.prod.yml up -d redis
# 3. Actualizar compose.prod.yml con el nuevo digest
# 4. Commit + push + deploy
```

## 8. PostgreSQL — Backup y restore

### 8.1 Backup programado

```bash
#!/usr/bin/env bash
# /opt/arbitragex-v2/scripts/vps/pg-backup.sh
set -euo pipefail
BACKUP_DIR=/var/backups/arbx/postgres
TS=$(date -u +%Y%m%dT%H%M%SZ)
mkdir -p "$BACKUP_DIR"
PGPASSWORD="${ARBX_MIGRATOR_PASSWORD:?}" pg_dump \
  -h localhost -U arbx_migrator -d arbitragex \
  --format=custom --file="$BACKUP_DIR/arbitragex-${TS}.dump" \
  --verbose
# Retención
find "$BACKUP_DIR" -name 'arbitragex-*.dump' -mtime +7 -delete
```

Crontab:

```cron
# pg_dump completo cada 6 horas
0 */6 * * * /opt/arbitragex-v2/scripts/vps/pg-backup.sh >> /var/log/arbx/pg-backup.log 2>&1
```

### 8.2 Restore procedure

1. Detener todos los servicios que escriban a PostgreSQL.
2. Crear base de datos limpia:
   ```bash
   psql -h localhost -U postgres -c "DROP DATABASE IF EXISTS arbitragex_restore;"
   psql -h localhost -U postgres -c "CREATE DATABASE arbitragex_restore;"
   ```
3. Restaurar:
   ```bash
   pg_restore -h localhost -U arbx_migrator -d arbitragex_restore \
     --no-owner --role=arbx_rw /var/backups/arbx/postgres/arbitragex-<TS>.dump
   ```
4. Validar conteos y schema:
   ```bash
   psql -h localhost -U arbx_ro -d arbitragex_restore -c "SELECT count(*) FROM opportunities;"
   ```
5. Switch atómico:
   ```bash
   psql -h localhost -U postgres -c "SELECT pg_terminate_backend(pid) FROM pg_stat_activity WHERE datname='arbitragex';"
   psql -h localhost -U postgres -c "ALTER DATABASE arbitragex RENAME TO arbitragex_old;"
   psql -h localhost -U postgres -c "ALTER DATABASE arbitragex_restore RENAME TO arbitragex;"
   ```
6. Reiniciar servicios.

### 8.3 PITR (Point-in-Time Recovery)

PITR requiere WAL archiving. Para un VPS simple, habilitar WAL-G o `pg_basebackup` + archivo continuo a un bucket S3/Minio:

```yaml
postgres:
  command:
    - postgres
    - -c
    - wal_level=replica
    - -c
    - archive_mode=on
    - -c
    - archive_command='envdir /etc/wal-g/env /usr/local/bin/wal-g wal-push %p'
```

> Esto aumenta la complejidad operativa. Marcar como **MEDIUM priority** y no bloquear el resto del playbook.

## 9. VPS — Snapshots y rebuild

### 9.1 Snapshot schedule

| Frecuencia | Alcance | Retención |
|------------|---------|-----------|
| Diaria | Snapshot del disco completo del VPS | 7 días |
| Semanal | Snapshot + backup offsite (rsync a destino del operador) | 4 semanas |
| Pre-deploy | Snapshot manual antes de cualquier deploy | hasta próximo deploy exitoso |

### 9.2 Rebuild from scratch

1. Provisionar nuevo VPS con la misma distribución y versión.
2. Configurar firewall (§11.2), fail2ban (§11.5), Docker y usuarios.
3. Restaurar `/opt/arbitragex-v2` desde el último backup git/github (`github/main`).
4. Copiar `.env` desde el vault de backups offsite.
5. Restaurar PostgreSQL desde el último `pg_dump` (§8.2).
6. Restaurar Redis desde el último RDB (§3.5).
7. Arrancar con `docker compose -f docker/compose.prod.yml up -d`.
8. Verificar healthchecks y readiness.

RTO objetivo: **< 4 horas** con backups automatizados y documentación actualizada.

## 10. Rollback de imágenes Docker

### 10.1 Principio

Cada deploy debe producir imágenes taggeadas con el SHA del commit y el timestamp:

```bash
export GIT_SHA=$(git rev-parse --short HEAD)
export IMAGE_TAG=${GIT_SHA}-$(date -u +%Y%m%dT%H%M%SZ)
docker compose -f docker/compose.prod.yml build --no-cache api-server
docker tag arbitragex-v2-api-server:latest ghcr.io/hefarica/arbitragex-v2/api-server:${IMAGE_TAG}
docker push ghcr.io/hefarica/arbitragex-v2/api-server:${IMAGE_TAG}
```

### 10.2 Rollback rápido

```bash
# 1. Listar tags disponibles
docker pull ghcr.io/hefarica/arbitragex-v2/api-server 2>/dev/null | tail
# 2. Editar compose.prod.yml para fijar la imagen anterior (incluyendo digest)
# 3. Re-deploy
export ARBX_API_SERVER_IMAGE=ghcr.io/hefarica/arbitragex-v2/api-server:<previous-tag>@sha256:<digest>
docker compose -f docker/compose.prod.yml up -d api-server
# 4. Verificar
sleep 10
curl -f http://127.0.0.1:8080/api/health
```

### 10.3 Registro de releases

Mantener `/opt/arbitragex-v2/.deploy/current` y `/opt/arbitragex-v2/.deploy/previous` con el digest y commit de cada deploy.

## 11. RPC failover

### 11.1 Estado actual

La aplicación usa variables `RPC_HTTP_1`, `RPC_WS_1`, etc. No hay evidencia de lógica de failover automático en el hot-path Rust. Cuando el RPC primario muere, el scanner se detiene.

### 11.2 Diseño de failover

1. **Configuración de múltiples endpoints:** declarar `RPC_HTTP_1`, `RPC_HTTP_2`, `RPC_WS_1`, `RPC_WS_2` en `.env`.
2. **Lógica de fallback en searcher-rs:** mantener una lista ordenada de providers; ante error de conexión o latencia > 500 ms, cambiar al siguiente.
3. **Healthcheck del RPC:** endpoint interno `/health` del searcher-rs ya verifica conectividad. Añadir métrica `rpc_provider_active`.
4. **Circuit breaker:** si 3 fallos consecutivos ocurren en < 30 s, marcar provider como degraded durante 60 s y alertar.
5. **Documento de referencia:** este diseño debe alinearse con la skill `arbx-rpc-failover-discipline`.

## 12. Security hardening checklist

### 12.1 TLS termination

- **Opción A (recomendada):** Cloudflare Tunnel o proxy reverso con certificado gestionado por Cloudflare. Termina TLS en el edge y comunicación interna por Docker network.
- **Opción B:** certbot + Let's Encrypt en el VPS con nginx/Caddy. Renovación automática vía cron.
- Una vez TLS esté activo, forzar `ARBX_TLS_ENABLED=true` en el build del frontend para que `next.config.js` emita HSTS.

### 12.2 Firewall (ufw)

```bash
ufw default deny incoming
ufw default allow outgoing
ufw allow from <OPERATOR_HOME_IP> to any port 22 proto tcp
ufw allow 443/tcp   # HTTPS
ufw allow 80/tcp    # HTTP → redirección a HTTPS (si aplica)
# Los puertos internos de Docker (8080, 3002-3005, 6379, 5432) NO se exponen
# a 0.0.0.0; solo loopback o red interna de Docker.
ufw enable
```

### 12.3 CORS exacto

Actualmente `edge/dev-local/src/index.ts` y `edge/worker/src/index.ts` permiten headers dinámicos. En producción:

- `access-control-allow-origin` debe ser exactamente el dominio público (`https://edge-arbx.ape-tv.net`), no `*`.
- `access-control-allow-credentials: true` solo cuando origin coincida.
- Rechazar preflight con origin no allowlisteado.

### 12.4 Rate limiting por endpoint

| Endpoint | Límite | Ventana |
|----------|--------|---------|
| `/admin/session` | 5 intentos | 60 s por IP |
| `/admin/*` (mutaciones) | 30 | 60 s por admin token |
| `/api/opportunities/live` | 60 | 60 s por IP |
| `/api/health` | 30 | 60 s por IP |
| `/socket.io/*` | 20 conexiones | 60 s por IP |

El edge worker ya tiene rate limiting en memoria para `/admin/session`. Migrar a Redis-backed rate limiting para múltiples réplicas.

### 12.5 Fail2ban / SSH brute force

```ini
# /etc/fail2ban/jail.local
[sshd]
enabled = true
port = ssh
filter = sshd
logpath = /var/log/auth.log
maxretry = 3
bantime = 3600
findtime = 600
```

Adicionalmente:

- Deshabilitar login root por password.
- Forzar autenticación SSH por clave pública.
- Cambiar el puerto SSH a uno no estándar (opcional, no sustituye claves).

## 13. Infrastructure changes summary (tabla)

| Componente | Cambio | Archivo afectado |
|------------|--------|------------------|
| Redis | Activar RDB + AOF, restringir comandos peligrosos, requirepass | `docker/compose.prod.yml`, nuevo `infra/redis/redis.conf` |
| Redis | Healthcheck semántico (`PING` → esperar `PONG`) | `docker/compose.prod.yml` |
| Redis | Backup horario vía script + cron | nuevo `scripts/vps/redis-bgsave.sh` |
| api-server | Eliminar montura `..:/repo:ro` | `docker/compose.prod.yml` |
| api-server | Añadir readiness-bundle en build time | `backend/api-server/Dockerfile` |
| api-server | Read-only root fs, no-new-privileges, cap-drop | `docker/compose.prod.yml` |
| api-server | Healthcheck funcional (200 en `/api/v1/health` y `/api/health`) | `docker/compose.prod.yml` |
| Frontend SSR | Reemplazar `ARBX_ADMIN_TOKEN` por `ARBX_FRONTEND_READONLY_TOKEN` | `frontend/lib/api-client.ts`, `docker/compose.prod.yml` |
| Frontend SSR | Healthcheck exacto (200 en /, /api/health, bundle) | `docker/compose.prod.yml` |
| Frontend build | No exponer `ARBX_ADMIN_TOKEN` al entorno del frontend | `docker/compose.prod.yml` |
| Todas las imágenes | Pin a digest SHA256 | `docker/compose.prod.yml` |
| PostgreSQL | pg_dump programado cada 6h + retención | nuevo `scripts/vps/pg-backup.sh` |
| PostgreSQL | WAL-G para PITR (futuro) | `docker/compose.prod.yml` (post-MVP) |
| VPS | Snapshots diarios + backup offsite semanal | documentado en §9 |
| Deploy | Taggear imágenes con SHA + timestamp, registrar previous/current | nuevo `scripts/vps/deploy.sh` |
| RPC | Failover multi-provider en searcher-rs | `backend/searcher-rs` (cambio de código) |
| TLS | Cloudflare Tunnel o certbot + HSTS | infra del VPS |
| Firewall | ufw con puertos mínimos | VPS |
| CORS | Origin exacto, credentials controlado | `edge/worker/src/index.ts`, `edge/dev-local/src/index.ts` |
| Rate limiting | Límites por endpoint, Redis-backed | `edge/worker/src/index.ts` |
| SSH | fail2ban, key-only, root login deshabilitado | VPS |

## 14. Operator actions

1. Generar contraseña fuerte de Redis y añadir `REDIS_PASSWORD` a `.env` (no en repo).
2. Crear `ARBX_FRONTEND_READONLY_TOKEN` en Vault/.env y retirar `ARBX_ADMIN_TOKEN` del servicio frontend.
3. Ejecutar `docker pull` de cada imagen base y reemplazar tags por digests en `docker/compose.prod.yml`.
4. Copiar los scripts `scripts/vps/redis-bgsave.sh` y `scripts/vps/pg-backup.sh` al VPS y programarlos en crontab.
5. Configurar directorio de backups `/var/backups/arbx` con permisos 0700.
6. Configurar ufw y fail2ban según §12.
7. Configurar TLS (Cloudflare Tunnel o certbot).
8. Probar restore de PostgreSQL y Redis en un entorno aislado antes de declarar Gate 6 cerrado.
9. Documentar RPO/RTO reales medidos en la primera prueba de restore.
10. Revisar y aprobar la skill `arbx-rpc-failover-discipline` antes de implementar failover multi-provider.

## 15. Riesgos residuales

| Riesgo | Severidad | Mitigación / Nota |
|--------|-----------|-------------------|
| AOF corruption no detectado | Medio | AOF auto-rewrite + RDB snapshot horario; restaurar desde RDB si AOF truncado |
| Backup offsite no implementado | Alto | Snapshot Hetzner no es offsite real; operador debe configurar rsync/s3 a destino propio |
| PITR no implementado | Medio | pg_dump cada 6h da RPO 6h; WAL-G es futuro |
| Failover RPC requiere cambio de código Rust | Medio | Diseñado pero no implementado; requiere `arbx-rpc-failover-discipline` |
| Token scoped requiere cambios en edge + api-server | Medio | Implementar en PR separado; alternativa transitoria documentada |
| Rate limiting in-memory no escala a múltiples edges | Medio | Usar Redis-backed rate limiting si se escala horizontalmente |
| Vault sealed/unseal manual | Bajo | Procedimiento ya documentado; auto-unseal deliberadamente no habilitado |
| Dependencia de Cloudflare para TLS | Bajo | Opción certbot documentada como alternativa |

## 16. Effort estimate

**L (Large)** — aproximadamente 1 semana de trabajo efectivo distribuido en:

- 2 días: cambios de compose/Dockerfile/healthchecks + tests de build.
- 2 días: implementación de token scoped y ajustes de edge/api-server.
- 1 día: scripts de backup, restore testing y documentación.
- 1 día: hardening de VPS (firewall, fail2ban, TLS).
- 1 día: buffer para imprevistos y validación de security-reviewer.

## 17. Validaciones obligatorias antes de cerrar Gate 6

- [ ] `docker compose -f docker/compose.prod.yml config` pasa sin errores.
- [ ] Build de api-server y frontend exitoso con las nuevas variables de entorno.
- [ ] Redis reinicia y recupera streams/keys desde RDB+AOF.
- [ ] Restore de PostgreSQL desde `pg_dump` en entorno aislado devuelve datos coherentes.
- [ ] Frontend healthcheck devuelve 200; 401/403/404 devuelven unhealthy.
- [ ] api-server healthcheck devuelve 503 cuando postgres o redis están caídos.
- [ ] `ARBX_ADMIN_TOKEN` no está presente en el entorno del contenedor frontend.
- [ ] Imágenes base usan digest SHA256.
- [ ] security-auditor aprueba con 0 bloqueadores CRITICAL/HIGH.

---

*Generado por IA OMEGA — Gate 6 Hardening & DR. Modo shadow/paper/read-only.*
