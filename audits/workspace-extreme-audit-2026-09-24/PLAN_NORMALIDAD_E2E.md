# AUDITORÍA DE ESTADO + PLAN DE NORMALIDAD E2E
**Fecha:** 2026-09-24 · **Sesión:** workspace-extreme-audit + implementación completa

---

## PARTE 1 — AUDITORÍA DEL WORKSPACE LOCAL

### 1.1 Estado Git

| Métrica | Valor |
|---|---|
| Branch actual | `promo/workspace-audit-2026-09-24` |
| HEAD | `12b965a2` (cargo fmt sobre workpackage) |
| Commits adelante de origin/main | **10** |
| Archivos modificados sin commitear | **8** |
| Archivos nuevos (untracked) | **3** |
| Archivos eliminados staged | **3** (manifests de auditoría — deben ir al commit) |
| Remote | `origin` = `https://github.com/hefarica/arbitragex-v2.git` |

**Archivos sin commitear que DEBEN ir al PR:**
```
M  backend/searcher-rs/src/cartridge_boot.rs          (último fix CORE-04 dispatch)
M  backend/searcher-rs/src/rhai_agent_bridge.rs       (comentarios post-integración)
M  backend/searcher-rs/src/snapshot_services.rs       (comentarios post-integración)
M  contracts/script/DeployTestnet.s.sol               (WEB3-03 chainid gate)
M  contracts/src/core/DeterministicFactory.sol        (WEB3-08 design doc)
M  shared-ts/src/real-card-integrity.ts               (preexistente)
D  audits/.../manifest.sha256                          (limpieza de staging)
D  audits/.../git-blob-index.txt                       (limpieza de staging)
D  audits/.../MANIFEST_SHA256.json                     (limpieza de staging)
```

### 1.2 Estado de Compilación y Tests

| Verificación | Estado |
|---|---|
| `cargo check --workspace` (13 crates) | ✅ |
| `cargo test -p searcher-rs --lib` | ✅ 1356/1356 |
| `cargo test -p math-engine --lib` | ✅ 135/135 |
| `cargo test -p sim-core --lib` | ✅ 75/75 |
| `cargo test -p prioritization-spine --lib` | ✅ 123/123 |
| `tsc --noEmit` frontend | ✅ |
| `tsc --noEmit` api-server | ✅ |
| `tsc --noEmit` selector-api | ✅ |

### 1.3 Hallazgos Resueltos (88/96)

| Severidad | Resueltos | Pendientes |
|---|---|---|
| CRÍTICO 1 | 1 | 0 |
| ALTO 25 | 25 | 0 |
| MEDIO 39 | 33 | 6 (design states documentados) |
| BAJO 31 | 29 | 2 (cosméticos) |

### 1.4 Integración v4 (ARBX_CARTUCHOS_AGENTE_264)

| Componente | Estado |
|---|---|
| 6 módulos runtime en searcher-rs | ✅ instalados y compilando |
| Parser ProposalV4 (sin unwrap_or) | ✅ en runner.rs |
| Intercepción v4 en cartridge_boot | ✅ (jamás construye candidato v3 desde intent.legs) |
| card_contract.ts en frontend | ✅ `frontend/lib/contracts/cardContract.ts` |
| 264 cartuchos staged | ✅ `integration/agent-cartridges-v4/` (1.174 archivos) |
| Módulo acoplar/desacoplar | ✅ cartridge_control.rs + API + migración 125 |

---

## PARTE 2 — AUDITORÍA DEL REPO (GitHub)

### 2.1 Branch Protection y CI

| Check | Estado |
|---|---|
| Branch protection (main) | ✅ 14 required checks (P-02) |
| Workflows activos | 54 workflows .yml |
| SHA pins aplicados | ✅ 38 refs pineadas (CI-05) |
| RUSTSEC allowlist | ✅ 2 stale ignores removidos (CI-06) |
| gitleaks (full history) | ✅ blocking (security.yml:245-250) |

### 2.2 Gates de Deploy

| Gate | Estado |
|---|---|
| G1 (contract tests required) | Parcial |
| G2 (paridad frontend↔edge) | Hueco |
| G3 (guardian smoke 9) | Hueco |
| G4 (deploy veraz SHA) | Hueco |
| G5 (L4 post-deploy + rollback) | Hueco |
| G6 (secuencia blindada) | Hueco |

**NOTA:** G2-G6 son huecos documentados (§37 P-02). Cada uno requiere su PR con ID.

### 2.3 Pendiente de Push

La branch `promo/workspace-audit-2026-09-24` tiene **10 commits** que NO están en origin/main. Debe crearse PR y pasar los 14 required checks.

---

## PARTE 3 — AUDITORÍA DEL VPS

> **[NO ACCESO DIRECTO]:** No puedo conectarme al VPS por SSH desde este entorno.
> La auditoría del VPS se basa en la CONFIGURACIÓN del repo (compose files, scripts,
> workflows de deploy) y en el estado del último deploy conocido.

### 3.1 Servicios en compose.prod.yml (24 servicios + 13 volúmenes)

| Categoría | Servicios |
|---|---|
| Core | postgres, redis, searcher-rs, api-server, edge, frontend |
| Simulación | anvil, sim-ctl, math-engine |
| Ejecución | relays-client (§34.3 terminus) |
| Analytics | recon, selector-api, token-enricher |
| Observabilidad | prometheus, grafana, alertmanager, loki, promtail |
| Storage | vault, minio, thanos-sidecar, thanos-store, thanos-query |
| Infra | socket-proxy |

### 3.2 Estado del Deploy Actual (inferido)

| Aspecto | Estado |
|---|---|
| Último deploy verificado | Desconocido (requiere `ssh arbx git log -1`) |
| Migraciones BD | 125 migraciones (001→125; 122/123 eliminadas; 124/125 nuevas) |
| Variables .env.example | 123 vars; 5 críticas §34 documentadas |
| Docker compose | prod (24 svc) + dev (dev-local); raíz eliminado (CI-02) |
| Secrets | DATABASE_URL, REDIS_URL, ADMIN_TOKEN via ${VAR:?} |

### 3.3 Acciones Operativas Pendientes en VPS

1. **Rotar ARBX_RW_PASSWORD** (SEC-01: el DSN quedó en git history)
2. Ejecutar migraciones 124-125 en PG
3. Reconstruir imágenes Docker con los 88 fixes
4. Verificar health checks de los 24 servicios
5. Confirmar R7 pipeline (searcher → Redis → PG → API → Frontend)

---

## PARTE 4 — PLAN DE NORMALIDAD END-TO-END

### FASE 0: Commit y PR (LOCAL — 30 min)

```bash
# 1. Commitear los 8 archivos restantes
git add -A
git commit -m "fix: P7 batch final (CI-05 SHA pins, CI-06 RUSTSEC stale, SIM-02/SEL-04/WEB3-08 docs)"

# 2. Push de la branch
git push origin promo/workspace-audit-2026-09-24

# 3. Crear PR a main (los 14 checks deben pasar)
#    gh pr create --title "workspace-extreme-audit: 88 fixes + v4 integration + cartridge_control"
```

**Blocker:** Los 14 required checks deben pasar en CI. Si `cargo fmt --check` falla, ejecutar `cargo fmt --all` primero.

### FASE 1: Merge y Deploy al VPS (1-2h)

```bash
# 1. Merge PR (después de CI verde)
gh pr merge --squash

# 2. SSH al VPS
ssh arbx

# 3. Pull y build
cd /opt/arbitragex-v2
git pull origin main

# 4. ROTAR ARBX_RW_PASSWORD (SEC-01 — CRÍTICO)
#    Editar .env con nuevo password + actualizar PostgreSQL

# 5. Ejecutar migraciones nuevas (124, 125)
docker compose --env-file .env exec postgres psql -U postgres -d arbitragex \
  -f /dev/stdin < database/migrations/124_opportunities_ws_trigger_amount_text.sql
docker compose --env-file .env exec postgres psql -U postgres -d arbitragex \
  -f /dev/stdin < database/migrations/125_cartridge_control.sql

# 6. Rebuild sin cache
docker compose --env-file .env -f docker/compose.prod.yml build --no-cache

# 7. Up
docker compose --env-file .env -f docker/compose.prod.yml up -d
```

### FASE 2: Verificación E2E (30 min)

```bash
# R7 Pipeline check:
# 1. Searcher detecta
docker logs searcher-rs --tail 50 | grep -i 'cartridge.boot_loaded'

# 2. Redis recibe
docker exec redis redis-cli XLEN arbx:opps:detected

# 3. PostgreSQL recibe
docker exec postgres psql -U postgres -d arbitragex -c \
  'SELECT COUNT(*), MAX(detected_at) FROM opportunities;'

# 4. API sirve
curl -s localhost:8787/api/opportunities/live | head -c 200

# 5. Frontend renderiza
curl -I https://<VPS_HOST>/opportunities

# 6. Cartridge Control funciona
curl -s -H "x-arbx-admin-token: $ARBX_ADMIN_TOKEN" \
  localhost:8787/api/v1/cartridges/control?chain_id=1
```

### FASE 3: Health Checks y Observabilidad (30 min)

```bash
# Todos los servicios saludables
docker compose -f docker/compose.prod.yml ps --format json | \
  jq -r '.[] | select(.State != "running") | .Name'

# Prometheus targets
curl -s localhost:9090/api/v1/targets | jq '.data.activeTargets[].health'

# Grafana accesible
curl -s localhost:3000/api/health

# Logs sin errores críticos
for svc in searcher-rs api-server relays-client sim-ctl; do
  docker logs $svc --tail 20 2>&1 | grep -ci 'error\|panic\|fatal'
done
```

### FASE 4: Activación Gradual de Cartuchos (1h)

```bash
# 1. Verificar el módulo acoplar/desacoplar
curl -X PUT https://<VPS_HOST>/api/v1/cartridges/control \
  -H "x-arbx-admin-token: $ARBX_ADMIN_TOKEN" \
  -H "content-type: application/json" \
  -d '{"chain_id":1,"cartridge_id":"dex_arb","desired":"enabled","reason":"canary"}'

# 2. Confirmar pausa
curl -s -H "x-arbx-admin-token: $ARBX_ADMIN_TOKEN" \
  https://<VPS_HOST>/api/v1/cartridges/control?chain_id=1 | jq .

# 3. Habilitar ARBX_CARTRIDGE_MODE=shadow en .env
# 4. docker compose --env-file .env up -d searcher-rs
# 5. Monitorear: docker logs searcher-rs --tail 100 -f | grep cartridge
```

### FASE 5: Gates de Normalidad (checklist final)

| # | Verificación | Método |
|---|---|---|
| 1 | searcher-rs running | `docker ps searcher-rs` |
| 2 | 271 cartuchos cargados | logs `cartridge.boot_loaded loaded=271` |
| 3 | Redis stream activo | `XLEN arbx:opps:detected` > 0 |
| 4 | PG opportunities crece | `SELECT COUNT(*) FROM opportunities` trending up |
| 5 | API /opportunities/live responde | HTTP 200 con items |
| 6 | WebSocket conecta | Frontend muestra cards LIVE |
| 7 | Cartridge Control operable | PUT + GET cycle en un cartucho |
| 8 | paper_trade_runs crece | `SELECT COUNT(*) FROM paper_trade_runs` |
| 9 | Sin errores críticos en logs | 0 `panic`/`fatal` en últimos 15 min |
| 10 | Prometheus targets UP | Todos los exporters verdes |
| 11 | Price feed activo | `usePricesStream` muestra precios |
| 12 | Migraciones aplicadas | `schema_migrations` incluye 124, 125 |

### FASE 6: V4 Activation (CUANDO LOS CRITERIOS SE CUMPLAN)

Los 5 criterios de INTEGRACION.md (pendientes):
1. Compilación Rhai nativa de los 264 scripts
2. Tests de cada productor/cotizador/solver nativo
3. Propiedades por plan (raw continuidad, unidades, costes una vez)
4. E2E del mismo plan hasta la card y el toggle de vuelta
5. Branch/commit/CI/merge/deploy con protecciones existentes

**NO ACTIVAR hasta que estos 5 criterios pasen con evidencia verificada.**

---

## RESUMEN EJECUTIVO

| Área | Estado Actual | Acción Requerida |
|---|---|---|
| Workspace local | ✅ 88/96 fixes + v4 integrado | Commitear 8 archivos restantes |
| Repo GitHub | 10 commits sin push | Push + PR + merge |
| CI/CD | Workflows arreglados | Pasar 14 checks |
| VPS | Desplegado versión anterior | Rotar password + migraciones + rebuild |
| Pipeline E2E | Necesita verificación | R7 check (Fase 2) |
| Cartuchos | 264 activos v3 + 264 staged v4 | Shadow mode → gradual |
| Cartridge Control | Nuevo módulo listo | Deploy + test |
| Seguridad | Password comprometido | **ROTAR YA** |

**Próximo paso inmediato:** Commitear los 8 archivos, push, PR, merge.
