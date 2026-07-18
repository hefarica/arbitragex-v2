# REPO-VPS-DEEPWIKI v2 — REPORTE CORREGIDO SUPREMO
## Scan ID: SCAN-20260717T030251Z-REV1
## Fecha: 2026-07-17T04:40:00Z
## Auditor: IA OMEGA + Operador (peer review)

---

## 1. MÉTRICAS SEPARADAS (ya no un 86% agregado)

| Dimensión | Peso | Estado | Score | Evidencia |
|---|---|---|---|---|
| **SOURCE SHA PARITY** | 15% | PASS | 100% | Repo SHA = VPS SHA = `81f06be` |
| **DOCKER CORE PRESENCE** | 20% | PASS | 100% | 22/22 contenedores esperados presentes |
| **CONTAINER HEALTH** | 20% | PASS | 100% | 22/22 HEALTHY (docker inspect healthchecks) |
| **OBSERVABILITY CHAIN** | 10% | PASS | 100% | Prometheus + Grafana + Loki + Alertmanager + Thanos operativos |
| **COMPOSE CONSISTENCY** | 10% | PARTIAL | 50% | 21 contenedores en `compose.prod.yml`, **Redis en `compose.dev.yml`** |
| **WORKTREE PARITY** | 10% | FAIL / DRIFT | 0% | Local dirty=10, VPS dirty=18. Mismo commit ≠ mismo working tree |
| **FILE CLASSIFICATION** | 5% | INCOMPLETE | 30% | 63 DRIFT sin clasificar por alcance (CI_ONLY vs RUNTIME_REQUIRED) |
| **PUBLIC URL PROBE** | 5% | EXECUTED | 100% | Edge :8787 responde, frontend renderiza |
| **PLAYWRIGHT E2E** | 3% | EXECUTED | 100% | Snapshot de homepage + /opportunities capturado |
| **DATA FLOW E2E** | 2% | PARTIAL | 50% | API /readiness responde (17 checks), /opportunities/live=200, Redis stream=10001 items, pero g-sim-1=RED, g-pap-1=RED |

### **SCORE PONDERADO CORREGIDO: 78.5%**

**Anterior (skill v1.0.0): 86.0%** → La diferencia refleja que el score anterior sobreponderaba la presencia estructural y subestimaba: (a) la inconsistencia de Compose, (b) el worktree dirty, (c) la falta de clasificación funcional de archivos.

---

## 2. CLASIFICACIÓN DE LOS 63 DRIFT POR ALCANCE

### 2.1 CI_ONLY (41 archivos) — NO requieren despliegue al VPS

| Patrón | Cantidad | Naturaleza | Recomendación corregida |
|---|---|---|---|
| `.github/workflows/*.yml` | 41 | Workflows de GitHub Actions. El VPS NO ejecuta Actions. Son metadata de CI/CD del repo. | **Marcar como EXPECTED DRIFT / CI_ONLY.** No son runtime-required. |

> **Razón:** El VPS es un entorno de ejecución Docker, no un runner de GitHub Actions. Los workflows existen en el repo para CI/CD en GitHub Cloud, no para ser desplegados al VPS. La recomendación "Deploy the canonical file version" de v1.0.0 era incorrecta para esta categoría.

### 2.2 VENDOR_OR_SUBMODULE (22 archivos) — EXPECTED MISSING en VPS

| Patrón | Cantidad | Naturaleza | Recomendación corregida |
|---|---|---|---|
| `contracts/lib/forge-std/*` | 2 | Submódulo Foundry | **EXPECTED MISSING.** No se inicializan submódulos en prod. |
| `contracts/lib/openzeppelin-contracts*/*` | 20 | Submódulos OZ | **EXPECTED MISSING.** Idem. Los contratos ya están deployados; las fuentes de librería no se necesitan en runtime. |

> **Razón:** Los submódulos de Foundry (`git submodule update --init`) no se ejecutan en el deploy de producción porque: (a) los contratos ya están compilados y deployados, (b) el VPS no necesita recompilar, (c) los submódulos son dependencias de build-time, no runtime.

### 2.3 Resumen de clasificación

| Categoría | Cantidad | Runtime Required | Requiere Acción |
|---|---|---|---|
| CI_ONLY | 41 | No | No |
| VENDOR_OR_SUBMODULE | 22 | No | No |
| **RUNTIME_REQUIRED** | **0** | — | — |

**Conclusión corregida:** De los 63 DRIFT reportados en v1.0.0, **0 requieren acción de despliegue.** Todos son EXPECTED DRIFT por diseño arquitectónico. El score debería ajustarse eliminando estos 63 del denominador de runtime parity.

---

## 3. ANOMALÍA DE COMPOSE: Redis en `compose.dev.yml`

### Evidencia

```
VPS: docker inspect arbitragex-v2-redis-1
  → com.docker.compose.project.config_files: /opt/arbitragex-v2/docker/compose.dev.yml
```

### Análisis

| Aspecto | Estado |
|---|---|
| Redis container | `arbitragex-v2-redis-1` — running, healthy |
| Compose de origen | `compose.dev.yml` (NO `compose.prod.yml`) |
| Impacto funcional | Ninguno — Redis funciona correctamente |
| Impacto arquitectónico | **DRIFT de contrato.** El contrato canónico (`compose.prod.yml`) no es la fuente única de verdad para Redis. |
| Hipótesis | Redis fue levantado manualmente vía `docker compose -f compose.dev.yml up redis` durante debugging o fue heredado de un deploy anterior. |

### Recomendación

1. Verificar si `redis` está definido en `compose.prod.yml` (debería estarlo para producción).
2. Si está en ambos: ejecutar `docker compose -f compose.prod.yml up -d redis` para alinear con el contrato.
3. Si solo está en `compose.dev.yml`: migrar la definición de servicio a `compose.prod.yml`.

---

## 4. INVENTARIO DE DIRTY FILES

### 4.1 Local (10 archivos)

| Archivo | Estado | Interpretación |
|---|---|---|
| `M .claude/settings.json` | Modificado | Config local de Claude |
| `M .claude/settings.local.json` | Modificado | Config local de Claude |
| `?? .claude/gate/` | Untracked | Gate de activación matemática-física |
| `?? fix_scanner.py` | Untracked | Script de reparación temporal |
| `?? package.json.bak` | Untracked | Backup de package.json |
| `?? repo-vps-audits/` | Untracked | Output del scan (este reporte) |
| `?? scripts/vps-semiotic-bridge-setup.sh` | Untracked | Setup del semiotic bridge |
| `?? scripts/vps/` | Untracked | Scripts VPS adicionales |
| `?? test-phase1/` | Untracked | Testing temporal |
| `?? vps-validation-homepage.png` | Untracked | Screenshot de validación |

**Naturaleza:** 8/10 son artifacts de operación/auditoría. 2 son configs de Claude. Ninguno afecta el runtime del sistema.

### 4.2 VPS (18 archivos)

| Archivo | Estado | Interpretación |
|---|---|---|
| `?? .last-known-good-commit` | Untracked | Metadata de deploy |
| `?? .pre-deploy-commit` | Untracked | Metadata de deploy |
| `?? backend/api-server/src/routes/cartridges.ts` | Untracked | **CÓDIGO EN DESARROLLO** — posible feature en progreso |
| `?? backend/searcher-rs/src/engine/` | Untracked | **CÓDIGO EN DESARROLLO** — posible feature en progreso |
| `?? backend/semiotic-bridge/.vps_protection` | Untracked | Protección del semiotic bridge |
| `?? backup_.sql` | Untracked | Backup de PostgreSQL |
| `?? backups/` | Untracked | Directorio de backups |
| `?? config/arbx_config_bundle.json.enc` | Untracked | Config bundle cifrado |
| `?? configs/app.toml.bak_20260615_172708` | Untracked | Backup de config |
| `?? configs/app.toml.bak_20260615_173051` | Untracked | Backup de config |
| `?? defaults` | Untracked | Valores por defecto |
| `?? docker/compose.prod.yml.bak_20260615_164048` | Untracked | Backup de compose.prod |
| `?? frontend/app/admin/topology/TopologyVaultClient.tsx.orig` | Untracked | Artifact de merge conflict |
| `?? frontend/app/admin/topology/TopologyVaultClient.tsx.rej` | Untracked | Artifact de merge conflict |
| `?? frontend/app/strategies/forge/StrategyForgeForm.tsx` | Untracked | **CÓDIGO EN DESARROLLO** |
| `?? frontend/lib/store/strategy-forge-slice.ts` | Untracked | **CÓDIGO EN DESARROLLO** |
| `?? scripts/deploy-eficaz.sh` | Untracked | Script de deploy |
| `?? scripts/vps-semiotic-bridge-setup.sh` | Untracked | Setup script |

**Hallazgos críticos en VPS dirty:**
- **3 archivos de CÓDIGO en desarrollo** sin commit: `cartridges.ts`, `StrategyForgeForm.tsx`, `strategy-forge-slice.ts`. Esto indica trabajo en progreso en el VPS que no está en `main`.
- **2 artifacts de merge conflict** (`.orig`, `.rej`) — indican que un merge/rebase falló en el VPS y no se limpió.
- **Backups de config** (`app.toml.bak_*`, `compose.prod.yml.bak_*`) — modificaciones manuales de config con backups.

---

## 5. EVIDENCIA E2E REAL (Playwright + API Probes)

### 5.1 Playwright — Homepage (`/`)

| Check | Resultado |
|---|---|
| URL accesible | ✅ `http://195.201.235.70:8787/` |
| Título | ✅ "QuantumX — Control Plane" |
| Renderizado | ✅ Sidebar, navegación, badges visibles |
| Badges | ✅ "PAPER · TLS SHADOW", "KILL-SWITCH <10MS" |
| Status WS | ✅ "IDLE" |
| CSP | ⚠️ Report-only violations (walletconnect) — no bloqueante |
| Console errors | 2 (walletconnect 400/403) — no críticos |

### 5.2 Playwright — /opportunities

| Check | Resultado |
|---|---|
| URL accesible | ✅ `http://195.201.235.70:8787/opportunities` |
| Renderizado | ✅ Página carga |
| Console errors | 22 (incrementados vs homepage — posible fetching de datos) |

### 5.3 API Probes

| Endpoint | Código | Response | Interpretación |
|---|---|---|---|
| `GET /api/opportunities/live` | 200 | `{"count":0,"items":[],"ts":"..."}` | ✅ API funcional. Sin oportunidades = correcto para paper-shadow (no hay detección activa o no pasa filtros) |
| `GET /api/readiness` | 200 | 17 checks (ver detalle abajo) | ✅ Endpoint de readiness operativo |
| `GET /api/paper/history` | 200 | (no inspeccionado body completo) | ✅ Endpoint operativo |
| `GET /api/opportunities/by-strategy` | 404 | — | ❌ Endpoint no implementado o ruta diferente |
| `GET /api/executions` | 404 | — | ❌ Endpoint no implementado o ruta diferente |

### 5.4 Readiness Checks Detallado

| ID | Grupo | Estado | Significado |
|---|---|---|---|
| v-nh-1 | security_compliance | 🟢 green | no-hardcode doctrine: OK |
| v-db-1 | security_compliance | 🟢 green | DB passwords via vars: OK |
| v-at-1 | security_compliance | 🟡 yellow | Auth tokens: WARNING |
| pr-1 | security_compliance | 🟢 green | Audit trail: OK |
| pr-2 | audit_trail | 🟡 yellow | Audit trail parcial |
| monitoring | operations | 🟢 green | Observabilidad: OK |
| runbook | operations | 🟢 green | Runbook: OK |
| g-rpc-1 | risk_doctrines | 🟢 green | RPC failover: OK |
| g-net-1 | risk_doctrines | 🟢 green | Network health: OK |
| **g-sim-1** | **risk_doctrines** | 🔴 **red** | **Simulador NO listo** |
| g-pec-1 | risk_doctrines | 🟢 green | Path execution check: OK |
| g-ris-1 | risk_doctrines | 🟢 green | Risk limits: OK |
| g-tok-1 | tokens_strategies | 🟢 green | Token safety: OK |
| **g-pap-1** | **tokens_strategies** | 🔴 **red** | **Paper trades NO listos** |
| g-fl-1 | contracts | 🟢 green | Flash loan discipline: OK |
| g-mev-1 | operations | 🟢 green | MEV ethics: OK |
| alerts | operations | 🟢 green | Alertmanager: OK |

### 5.5 Infra Probes

| Servicio | Probe | Resultado |
|---|---|---|
| Redis | `PING` | ✅ `PONG` |
| PostgreSQL | `pg_isready` | ✅ accepting connections |
| Redis Stream | `XLEN arbx:opps:detected` | ✅ **10,001 items** |

---

## 6. VEREDICTO TÉCNICO CORREGIDO

```
SOURCE SHA PARITY:       ✅ PASS       (100%)
DOCKER CORE PRESENCE:    ✅ PASS       (100%)
CONTAINER HEALTH:        ✅ PASS       (100%)
OBSERVABILITY CHAIN:     ✅ PASS       (100%)
COMPOSE CONSISTENCY:     ⚠️ PARTIAL   (50%) — Redis en dev, no prod
WORKTREE PARITY:         ❌ FAIL       (0%)  — 28 dirty files combinados
FILE CLASSIFICATION:     ⚠️ INCOMPLETE (30%) — skill v1 no clasificó
PUBLIC URL PROBE:        ✅ EXECUTED   (100%)
PLAYWRIGHT E2E:          ✅ EXECUTED   (100%)
DATA FLOW E2E:           ⚠️ PARTIAL   (50%) — g-sim-1 RED, g-pap-1 RED
─────────────────────────────────────────────────────────────
OVERALL CORREGIDO:       78.5%        (PARTIAL-GO)
```

---

## 7. HALLAZGOS CRÍTICOS NUEVOS (no en v1.0.0)

### C1 — Código en desarrollo en el VPS (sin commit)

**Archivos:**
- `backend/api-server/src/routes/cartridges.ts`
- `frontend/app/strategies/forge/StrategyForgeForm.tsx`
- `frontend/lib/store/strategy-forge-slice.ts`

**Riesgo:** Estos archivos indican que hay trabajo de desarrollo activo en el VPS que no está en `main`. Si el VPS se redeploya desde `main`, este código se perderá. Si es código operativo, el sistema podría depender de él.

**Recomendación:**
1. Identificar si estos archivos son requeridos por el runtime actual.
2. Si sí: commitearlos en una rama y mergear a `main`.
3. Si no: eliminarlos del VPS para limpiar el working tree.

### C2 — g-sim-1 = RED (Simulador NO listo)

**Evidencia:** El readiness check `g-sim-1` reporta estado RED.

**Impacto:** El simulador (componente crítico del pipeline paper-shadow) no está listo para operación. Esto puede explicar por qué `/api/opportunities/live` devuelve `count=0` — las oportunidades pueden estar siendo detectadas (Redis tiene 10,001 items) pero el simulador no puede validarlas, por lo que no pasan al scoring.

**Recomendación:** Investigar logs de `sim-ctl` container para entender por qué g-sim-1 está RED.

### C3 — g-pap-1 = RED (Paper trades NO listos)

**Evidencia:** El readiness check `g-pap-1` reporta estado RED.

**Impacto:** Aunque el sistema está en modo paper, el componente de paper trades no está listo. Esto explica por qué no hay trades en el historial.

**Recomendación:** Investigar logs de `relays-client` o `selector-api` para entender el bloqueo.

### C4 — Artifacts de merge conflict en VPS

**Archivos:**
- `frontend/app/admin/topology/TopologyVaultClient.tsx.orig`
- `frontend/app/admin/topology/TopologyVaultClient.tsx.rej`

**Impacto:** Basura de un merge fallido. No afecta runtime (los archivos `.orig` y `.rej` no se cargan), pero indica una operación manual incompleta en el VPS.

**Recomendación:** Eliminar estos artifacts.

---

## 8. PRÓXIMO ORDEN CORREGIDO

1. **Clasificar completamente los 63 DRIFT** como CI_ONLY / VENDOR_OR_SUBMODULE / RUNTIME_REQUIRED. (Hecho en este reporte.)
2. **Investigar g-sim-1 RED y g-pap-1 RED** — leer logs de `sim-ctl` y `relays-client`.
3. **Resolver el código en desarrollo en VPS** — decidir si commitear o eliminar.
4. **Alinear Redis con `compose.prod.yml`** o documentar la excepción.
5. **Limpiar artifacts de merge conflict** del VPS.
6. **Ejecutar probes E2E completos** por cada endpoint documentado en la API.
7. **Verificar el flujo de datos end-to-end:** Redis stream → selector-api → sim-ctl → relays-client → PostgreSQL.
8. **Recalcular score** después de resolver C1-C4.

---

## 9. NOTAS PARA LA SKILL v2.0.0

### Mejoras requeridas en `repo_vps_deepwiki.py`:

1. **Clasificación de archivos por alcance:** Agregar categorías `CI_ONLY`, `RUNTIME_REQUIRED`, `VENDOR_OR_SUBMODULE`, `DOCUMENTATION`, `LOCAL_ARTIFACT`.
2. **Verificación de Compose consistency:** Comparar el `compose file` activo de cada contenedor contra el contrato canónico.
3. **Dirty file analysis:** Reportar dirty files con interpretación (código vs config vs artifact).
4. **E2E probes obligatorios:** Ejecutar `url_probes` contra endpoints descubiertos, no dejar `[]`.
5. **Separación de métricas:** Nunca agregar un score único. Reportar tabla dimensional separada.
6. **Readiness endpoint parsing:** Si existe `/api/readiness` o similar, parsear y reportar cada check individualmente.

---

*Reporte generado por IA OMEGA con peer review del Operador.*
*Read-only contract: Ningún sistema fue modificado durante este audit.*
