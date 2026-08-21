# 06_INFRA.md — Config, Infra y Operación

> SHA: `35627908` · Vivo: 2026-08-14T04:35:00Z

## CI/CD — 48 workflows GitHub Actions

### Required checks (branch protection main, enforce_admins=True)

| # | Check | Workflow | Propósito |
|---|---|---|---|
| 1 | `CodeQL` | codeql.yml | Análisis de seguridad estático |
| 2 | `Doctrine grep gates` | omega8-m3-grep-gates.yml | No-hardcode + doctrina OMEGA |
| 3 | `PII wireado recursive gates` | omega8-pii-gates.yml | PII redaction en audit_log/audit_event |
| 4 | `Rust tests` | rust.yml | `cargo test --lib` |
| 5 | `TypeScript tests + typecheck` | typescript.yml | vitest + tsc api-server |
| 6 | `analyze (rust)` | rust.yml (~21min) | Clippy + análisis estático Rust |
| 7 | `analyze (typescript)` | typescript.yml | Análisis estático TS |
| 8 | `audit Dockerfiles for complete COPY coverage` | dockerfile-audit.yml | Verifica COPY completo en Dockerfiles |
| 9 | `lint` | ci.yml | ESLint global |
| 10 | `lint-and-test-contracts` | ci.yml | Solidity compilation + tests |
| 11 | `lint-and-test-frontend` | ci.yml | ESLint + vitest frontend |
| 12 | `lint-and-test-node (22)` | ci.yml | Node 22 compat |
| 13 | `lint-and-test-rust` | ci.yml | cargo fmt + clippy + test searcher-rs |
| 14 | `tsc --noEmit (all workspaces)` | ci.yml | Typecheck todos los workspaces |

### Workflows no-required (corren pero no bloquean merge)

| Workflow | Propósito | Estado típico |
|---|---|---|
| `security.yml` | cargo audit + npm audit + gitleaks | 🟡 fail (advisories pre-existentes) |
| `integration-tests.yml` | vitest integration (PG+Redis testcontainers) | 🟡 fail (opportunities-live 503) |
| `docker-build.yml` | Build imágenes Docker | ✅ pass |
| `frontend-build.yml` | next build producción | ✅ pass |
| `e2e.yml` | Playwright E2E | ✅ pass (cuando corre) |
| `auto-deploy-vps.yml` | Deploy automático post-merge a main | ⚠️ a veces fail (Docker race) |
| `ethics-guard.yml` | Escaneo de patrones no-éticos | ✅ pass |
| `opportunities-fidelity-gate.yml` | Fidelity gate de oportunidades | ✅ pass |

### Gobernanza CI

| Regla | Estado | Nota |
|---|---|---|
| `enforce_admins: true` | ✅ ACTIVADO (P-02) | Ni admins pueden push directo a main |
| `strict: true` | ✅ | Branch debe estar up-to-date con main |
| `required_pull_request_reviews` | ✅ | PR requerido |
| `allow_force_pushes` | ✗ | Bloqueado |
| `allow_deletions` | ✗ | Bloqueado |

## Dockerfiles (17, excluyendo worktrees)

| Dockerfile | Servicio | Base | Nota |
|---|---|---|---|
| `backend/api-server/Dockerfile` | api-server | node:20-slim | Multi-stage TS build |
| `backend/searcher-rs/Dockerfile` | searcher-rs | rust:1-bookworm | Multi-stage Rust build |
| `backend/searcher-rs/Dockerfile.edge` | searcher edge probe | — | auxiliar |
| `backend/selector-api/Dockerfile` | selector-api | node:20-slim | TS |
| `backend/sim-ctl/Dockerfile` | sim-ctl | rust:1-bookworm | Rust |
| `backend/recon/Dockerfile` | recon | rust:1-bookworm | Rust |
| `backend/relays-client/Dockerfile` | relays-client | rust:1-bookworm | Rust |
| `backend/math-engine/Dockerfile` | math-engine | rust:1-bookworm | Rust |
| `backend/token-enricher/Dockerfile` | token-enricher | rust:1-bookworm | Rust |
| `edge/worker/Dockerfile.node` | edge worker | node:20-bookworm-slim | Hono + @hono/node-server |
| `edge/dev-local/Dockerfile` | edge dev-local (legacy) | node:20-slim | Express (DEPRECATED en prod) |
| `frontend/Dockerfile` | frontend | node:20-bookworm-slim | Next.js 14 standalone |
| `docker/socket-proxy/Dockerfile` | socket-proxy | — | Redis socket proxy (security) |
| `infra/docker/Dockerfile.{backend,frontend,searcher}` | legacy | — | posiblemente OBSOLETO |

## Compose files (6 perfiles)

| Archivo | Propósito | Uso |
|---|---|---|
| `docker/compose.prod.yml` | **Producción** (VPS Hetzner) | ✅ VIVO |
| `docker/compose.dev.yml` | Desarrollo local | local |
| `docker/compose.staging.override.yml` | Staging | override |
| `docker/compose.hotpath-test.yml` | Hot-path testing | CI |
| `docker/compose.loopback.override.yml` | Loopback testing | CI |
| `docker/compose.noports.override.yml` | Sin puertos expuestos | CI |

## Topología nginx (inferida del vivo)

| Ruta nginx | Target | VIVO |
|---|---|---|
| `/` (arbx.ape-tv.net) | frontend:5173 | ✅ CONFIRMADO |
| `/api/*` (arbx.ape-tv.net) | edge:8787 | ✅ CONFIRMADO |
| `/socket.io/` (arbx.ape-tv.net) | api-server:8080 | ✅ (WS same-origin) |
| `/*` (edge-arbx.ape-tv.net) | edge:8787 directo | ✅ CONFIRMADO |

**Nota:** nginx config NO está en el repo (gestionado fuera, en el VPS directamente). Hallazgo de gobernanza.

## Deploy mechanics

| Paso | Mecanismo | Archivo |
|---|---|---|
| Push a main | Auto-deploy CI dispara | `.github/workflows/auto-deploy-vps.yml` |
| SSH al VPS | git pull + docker build + up -d | `scripts/deploy.sh` |
| Env deploy | Excel → gen_rpc_env → upsert VPS | `scripts/arbx-env-deploy/` |
| Frontend rebuild | `--no-cache` obligatorio (RULE 03) | `docker compose build --no-cache frontend` |
| Verificación post-deploy | SHA check (`git rev-parse HEAD`) | manual (R6-06: auto-deploy no confiable) |

## Monitoring stack

| Servicio | Puerto | Función |
|---|---|---|
| prometheus | 9090 | Metrics scraping |
| grafana | 3000 | Dashboards |
| loki | 3100 | Log aggregation |
| promtail | — | Log shipper |
| alertmanager | 9093 | Alert routing |
| thanos | — | Long-term metrics storage |
| minio | — | S3-compatible object storage (thanos) |

## Divergencias repo ↔ vivo (FASE 6)

| # | Hallazgo | Severidad |
|---|---|---|
| 1 | nginx config fuera del repo | 🟡 gobernanza (no reproducible) |
| 2 | 3 Dockerfiles legacy (`infra/docker/`) posiblemente OBSOLETOS | 🟢 BAJA |
| 3 | `edge/dev-local/Dockerfile` deprecated pero presente | 🟢 BAJA |
| 4 | auto-deploy a veces falla (Docker race) — requiere manual deploy | 🟡 OPERATIVA |
| 5 | `enforce_admins=true` bloquea push directo | ✅ gobernanza correcta |

## Checklist FASE 6

- [x] 48 CI workflows censados (14 required + 34 non-required)
- [x] Branch protection completo (enforce_admins, strict, required reviews)
- [x] 17 Dockerfiles catalogados
- [x] 6 compose profiles identificados
- [x] Topología nginx inferida del vivo
- [x] Deploy mechanics documentadas
- [x] Monitoring stack mapeado (7 servicios)
- [ ] nginx config real (fuera del repo — NO-EXPUESTO)

**Cobertura FASE 6: 90%**
