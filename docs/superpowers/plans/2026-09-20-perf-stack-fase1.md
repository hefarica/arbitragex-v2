# PERF-STACK-2026-09-20 Fase 1 — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Eliminar DT-1/5/6/7 (build Rust subóptimo, allocador del sistema, Node runtime con dev-deps, PG/Redis/anvil sin tuning ni límites) con 3 PRs mode-invariant (§34.1), verificados localmente + CI + deploy per-service + E2E.

**Architecture:** PR-1 (WO-1+2+8) optimiza el toolchain Rust (perfil release, mimalloc, target-cpu). PR-2 (WO-3) aligera los runtimes Node. PR-3 (WO-4+5+6) tunea el data-plane en docker/compose.prod.yml. Ningún cambio de semántica económica ni de datos.

**Tech Stack:** Cargo workspace (rust 1.91), mimalloc 0.1, Docker multi-stage (node:20-bookworm-slim, rust:1.91-slim-bookworm), PostgreSQL 15, Redis 7.2, Foundry (anvil).

**Skills:** /superpowers:systematic-debugging (ante fallo: causa raíz ANTES de fix) · /webapp-testing (E2E post-deploy) · /arbitragex-omniscience · /arbx-live-engineering.

## Global Constraints

- Mode-invariant (§34.1): la matemática NO cambia; sin cambios en detección/gates/sizing.
- `synchronous_commit` queda ON (RECHAZADO off). `panic="abort"` PROHIBIDO (tonic/tokio catch_unwind).
- RULE 00 / R8: fail-honest; sin mocks ni hardcodes.
- R3: deploy SIEMPRE `docker compose --env-file .env -f docker/compose.prod.yml build --no-cache <svc>` + `up -d <svc>` (usar el compose de prod que aplique por servicio; seguir el flujo auto-deploy establecido).
- DISK-GUARD: `docker builder prune` ANTES de builds en VPS (disco 80%, 30G libres).
- §36: `git branch --show-current` antes de cada commit.
- Commits terminan `Co-Authored-By: Claude Code <noreply@anthropic.com>`; PRs terminan `🤖 Generated with [Claude Code](https://claude.com/claude-code)`.
- Cargo.lock: si cambia (mimalloc), commitearlo (CI usa `--locked`).
- Branch protection: tras cada merge a main, PRs abiertos requieren `PUT /pulls/{n}/update-branch`.

---

### Task 1 (PR-1 · WO-1): `[profile.release]` en el workspace

**Files:**
- Modify: `backend/Cargo.toml` (append al final)

**Interfaces:** Ninguna (solo perfil de compilación).

- [x] **Step 1.1: Verificar precondición**

Run: `grep -n "profile.release" backend/Cargo.toml || echo "ABSENT (esperado)"`
Expected: `ABSENT`

- [x] **Step 1.2: Append del perfil**

```toml

# PERF-STACK WO-1 (2026-09-20): release profile canónico. lto thin + cgu 1
# mejoran inlining del hot-path (~5-15% típico). panic=abort PROHIBIDO:
# tonic/tokio usan catch_unwind en tests (decisión audit PERF-STACK).
[profile.release]
lto = "thin"
codegen-units = 1
opt-level = 3
```

- [x] **Step 1.3: Verificar que compila**

Run: `cd backend && cargo check -p searcher-rs --locked 2>&1 | tail -3`
Expected: `Finished` (el perfil no afecta check, valida TOML)

- [x] **Step 1.4: Commit**

```bash
git add backend/Cargo.toml
git commit -m "perf(build): workspace release profile lto=thin cgu=1 opt=3 (PERF-STACK WO-1)

Co-Authored-By: Claude Code <noreply@anthropic.com>"
```

---

### Task 2 (PR-1 · WO-2): mimalloc en searcher-rs y sim-ctl

**Files:**
- Modify: `backend/searcher-rs/Cargo.toml` (tras `[dependencies]`, antes de comentarios de simulator-v2)
- Modify: `backend/sim-ctl/Cargo.toml` (tras `[dependencies]`)
- Modify: `backend/searcher-rs/src/main.rs` (tras el bloque `#![allow(...)]`)
- Modify: `backend/sim-ctl/src/main.rs` (tras `#![warn(...)]`)
- Modify: `backend/Cargo.lock` (generado por cargo)

**Interfaces:** Ninguna pública (allocator global, cfg linux).

- [x] **Step 2.1: Dependencia target-específica en searcher-rs/Cargo.toml**

Insertar INMEDIATAMENTE después de la línea `simulator-v2 = { path = "../simulator-v2" }`:

```toml

# PERF-STACK WO-2 (2026-09-20): allocator de hot-path. Solo linux (binario
# prod); en dev Windows no se compila.
[target.'cfg(target_os = "linux")'.dependencies]
mimalloc = "0.1"
```

- [x] **Step 2.2: Dependencia en sim-ctl/Cargo.toml**

Insertar después de `prioritization-spine = { workspace = true }`:

```toml

# PERF-STACK WO-2 (2026-09-20): allocator de hot-path (linux prod).
[target.'cfg(target_os = "linux")'.dependencies]
mimalloc = "0.1"
```

- [x] **Step 2.3: global_allocator en searcher-rs/src/main.rs**

Después del cierre del bloque `#![allow(...)]` (línea ~15):

```rust
// PERF-STACK WO-2 (2026-09-20): mimalloc — asignaciones del hot-path más
// rápidas que glibc malloc. cfg linux: el binario prod corre en Debian.
#[cfg(target_os = "linux")]
#[global_allocator]
static GLOBAL_ALLOC: mimalloc::MiMalloc = mimalloc::MiMalloc;
```

- [x] **Step 2.4: global_allocator en sim-ctl/src/main.rs**

Después de `#![warn(clippy::unwrap_used, clippy::expect_used)]` (línea 2):

```rust
// PERF-STACK WO-2 (2026-09-20): mimalloc (linux prod).
#[cfg(target_os = "linux")]
#[global_allocator]
static GLOBAL_ALLOC: mimalloc::MiMalloc = mimalloc::MiMalloc;
```

- [x] **Step 2.5: Verificar localmente**

Run: `cd backend && cargo check -p searcher-rs -p sim-ctl --locked`
Expected: `Finished` (en Windows la cfg no aplica — valida TOML/sintaxis).
Run: `git diff --stat backend/Cargo.lock` → mimalloc + deps agregadas al lock.

- [x] **Step 2.6: Commit**

```bash
git add backend/searcher-rs/Cargo.toml backend/sim-ctl/Cargo.toml backend/searcher-rs/src/main.rs backend/sim-ctl/src/main.rs backend/Cargo.lock
git commit -m "perf(alloc): mimalloc global allocator en searcher-rs y sim-ctl, linux-only (PERF-STACK WO-2)

Co-Authored-By: Claude Code <noreply@anthropic.com>"
```

---

### Task 3 (PR-1 · WO-8): RUSTFLAGS x86-64-v3 en Dockerfiles Rust

**Files (solo stages builder Rust; verificar con `grep -l "FROM rust" backend/*/Dockerfile`):**
- Modify: `backend/searcher-rs/Dockerfile`, `backend/sim-ctl/Dockerfile`, `backend/math-engine/Dockerfile`, `backend/relays-client/Dockerfile`, `backend/recon/Dockerfile`, `backend/token-enricher/Dockerfile`, `docker/socket-proxy/Dockerfile` (si tiene stage Rust), y cualquier otro que grep encuentre.
- NO tocar: api-server/selector-api (Node), frontend.

**Interfaces:** Ninguna. Precondición verificada: VPS EPYC-Rome tiene avx2+bmi2+fma → x86-64-v3 soportado. VPS es el ÚNICO destino de release (RULE 01: Docker solo en VPS).

- [x] **Step 3.1: Inventario real**

Run: `grep -ln "FROM rust" backend/*/Dockerfile docker/*/Dockerfile`
Expected: lista exacta de archivos a tocar.

- [x] **Step 3.2: En cada Dockerfile, después de la línea `FROM rust:...AS builder`, insertar**

```dockerfile
# PERF-STACK WO-8 (2026-09-20): EPYC-Rome soporta x86-64-v3 (avx2+bmi2+fma,
# verificado en VPS). Release SOLO se buildea en VPS (RULE 01) — sin riesgo
# de portabilidad. Dev local (Windows) NO usa esta imagen.
ENV RUSTFLAGS="-C target-cpu=x86-64-v3"
```

- [x] **Step 3.3: Verificar**

Run: `grep -A2 "FROM rust" backend/searcher-rs/Dockerfile` → ENV presente.
No hay verificación local posible (build es en VPS — se verifica en deploy Task 8).

- [x] **Step 3.4: Commit + push + PR-1**

```bash
git add backend/*/Dockerfile docker/socket-proxy/Dockerfile
git commit -m "perf(docker): RUSTFLAGS target-cpu=x86-64-v3 en builders Rust (PERF-STACK WO-8)

Co-Authored-By: Claude Code <noreply@anthropic.com>"
git push -u origin perf/rust-release
# PR-1 base main, título: "perf: Rust release profile + mimalloc + x86-64-v3 (PERF-STACK WO-1+2+8)"
```

---

### Task 4 (PR-2 · WO-3): Node runtime sin dev-deps + NODE_ENV=production

**Files:**
- Modify: `backend/api-server/Dockerfile`
- Modify: `backend/selector-api/Dockerfile`

**Interfaces:** Ninguna. Patrón actual: builder hace `npm install --workspaces --include-workspace-root`, runtime copia `/build/node_modules` ENTERO (con dev-deps).

- [x] **Step 4.1: En AMBOS Dockerfiles, tras la línea `RUN npm run -w ... build` del builder, añadir prune de dev-deps**

api-server (después de `&& npm run -w @arbx/api-server build`):

```dockerfile
# PERF-STACK WO-3 (2026-09-20): runtime sin dev-deps (~60-70% menos node_modules).
RUN npm prune --workspaces --include-workspace-root --omit=dev
```

selector-api (después de `&& npm run -w @arbx/selector-api build`): mismo bloque con comentario.

- [x] **Step 4.2: En AMBOS runtime stages, añadir NODE_ENV**

Antes de `USER arbx`:

```dockerfile
# PERF-STACK WO-3 (2026-09-20): semántica de runtime prod (express/npm sin
# overhead dev; afecta mensajes de error, no datos).
ENV NODE_ENV=production
```

- [x] **Step 4.3: Riesgo conocido — verificación en deploy**

`npm prune --omit=dev` con workspaces puede remover paquetes que `dist/` requiere en runtime si algún import llega de dev-deps. Verificación OBLIGATORIA post-build VPS (Task 8): contenedor arranca + `/health` 200 + smoke R7 (`curl localhost:8787/api/opportunities/live`). Si falla por módulo faltante → systematic-debugging Fase 1 (leer stack, identificar módulo, corregir con dependency correcta en `dependencies`, NO revertir a dev-deps).

- [x] **Step 4.4: Commit (mismo branch perf/rust-release — PR único incluye Node; el plan de grouping decía PR-2 separado, pero un solo PR reduce 2× update-branch/CI de 34 checks; SEPARAR solo si CI amarra lints por servicio)**

```bash
git add backend/api-server/Dockerfile backend/selector-api/Dockerfile
git commit -m "perf(docker): Node runtime sin dev-deps + NODE_ENV=production (PERF-STACK WO-3)

Co-Authored-By: Claude Code <noreply@anthropic.com>"
```

---

### Task 5 (PR-3 · WO-4): PostgreSQL tuning

**Files:**
- Modify: `docker/compose.prod.yml` (servicio postgres, ~líneas 23-48; insertar `command` después de `image: postgres:15`)

**Interfaces:** Ninguna. Sizing basado en VPS medido: 16 GB RAM / 11 Gi libres.

- [x] **Step 5.1: Insertar command**

```yaml
    # PERF-STACK WO-4 (2026-09-20): tuning para 16GB/8core VPS. synchronous_commit
    # se mantiene ON (decisión audit: durabilidad kill-switch/ledger NO se negocia).
    command:
      - postgres
      - -c
      - shared_buffers=1GB
      - -c
      - work_mem=32MB
      - -c
      - effective_cache_size=3GB
      - -c
      - wal_compression=on
      - -c
      - max_connections=150
      - -c
      - maintenance_work_mem=512MB
```

- [x] **Step 5.2: Verificación post-deploy (Task 8)**

`ssh arbx docker exec <pg-container> psql -U postgres -c 'SHOW shared_buffers; SHOW work_mem;'` → 1GB / 32MB. PG logs sin errores de arranque.

- [x] **Step 5.3: Commit** — `git add docker/compose.prod.yml && git commit -m "perf(pg): shared_buffers 1GB, work_mem 32MB, wal_compression (PERF-STACK WO-4) ..."`

---

### Task 6 (PR-3 · WO-5): Redis maxmemory

**Files:**
- Modify: `docker/compose.prod.yml` (servicio redis ~líneas 50-72; command existente `redis-server --save '' --appendonly yes`)

- [x] **Step 6.1: Extender command existente (AOF queda ON — lección A5-STALL 2026-08-29)**

```yaml
      - redis-server
      - --save
      - ''
      - --appendonly
      - 'yes'
      # PERF-STACK WO-5 (2026-09-20): noeviction = fail-honest — cuando 1GB se
      # llena, los writes fallan ruidosamente en vez de evictar kill-switch/estado.
      - --maxmemory
      - 1gb
      - --maxmemory-policy
      - noeviction
```

- [x] **Step 6.2: Verificación post-deploy**: `redis-cli CONFIG GET maxmemory` → 1073741824; policy → noeviction; `XLEN arbx:opps:detected` delta ≥ 0 (invariante §33.1.3).

- [x] **Step 6.3: Commit** — `perf(redis): maxmemory 1gb noeviction (PERF-STACK WO-5)`

---

### Task 7 (PR-3 · WO-6): límites de recursos + pin anvil

**Files:**
- Modify: `docker/compose.prod.yml` (anvil ~línea 119-127; añadir deploy.resources a postgres/redis/anvil si no existen)

- [x] **Step 7.1: Capturar digest ACTUAL de anvil en VPS (fail-honest: pin lo que CORRE hoy, no lo que dice latest)**

Run: `ssh arbx "docker inspect $(ssh arbx docker ps -qf name=anvil) --format '{{index .RepoDigests 0}}'"`
Expected: `ghcr.io/foundry-rs/foundry@sha256:<digest>` → usar ESE digest.

- [x] **Step 7.2: Reemplazar imagen + añadir límites**

```yaml
  anvil:
    # PERF-STACK WO-6 (2026-09-20): pin por digest (DT-5) — reproducibilidad;
    # `latest` podía mutar el runtime de simulación entre deploys.
    image: ghcr.io/foundry-rs/foundry@sha256:<DIGEST_CAPTURADO>
    deploy:
      resources:
        limits:
          memory: 2G
          cpus: "2.0"
        reservations:
          memory: 256M
```

Y para postgres/redis (patrón ya existente en otro servicio del mismo archivo):

```yaml
    deploy:
      resources:
        limits:
          memory: 4G        # postgres: shared_buffers 1G + work_mem 150conn
          cpus: "2.0"
        reservations:
          memory: 512M
```
```yaml
    deploy:
      resources:
        limits:
          memory: 1536M     # redis: maxmemory 1g + overhead AOF
          cpus: "1.0"
        reservations:
          memory: 256M
```

- [x] **Step 7.3: Verificación post-deploy**: `docker ps` muestra límites; anvil responde `eth_blockNumber`; `docker compose config` parsea YAML.

- [x] **Step 7.4: Commit + push + PR**

Branch `perf/dataplane-tuning` desde origin/main (compose separado del Rust PR).
`git commit -m "perf(infra): límites recursos postgres/redis/anvil + pin digest anvil (PERF-STACK WO-4+5+6)"` → PR-3.

---

### Task 8: CI + merge + deploy + verificación E2E (por PR)

- [x] **8.1** Monitor CI 34 checks por PR (patrón monitor guard python d==t>0).
- [x] **8.2** Merge (squash/merge según repo) → verificar SHA.
- [x] **8.3** Deploy per-service R3: antes `docker builder prune -f` (DISK-GUARD); searcher-rs/sim-ctl/math-engine/etc. con los Dockerfiles tocados + postgres/redis/anvil `up -d` (recreate por command/limits change).
- [x] **8.4** L4: `ssh arbx 'cd /opt/arbitragex-v2 && git rev-parse HEAD'` == SHA merged.
- [x] **8.5** R7 pipeline: searcher logs boot limpio → `XLEN arbx:opps:detected` crece → PG MAX(detected_at) fresco → `/api/opportunities/live` sirve.
- [x] **8.6** E2E webapp-testing: Playwright contra https://arbx.ape-tv.net/opportunities — networkidle, sin banner QUARANTINED, cards renderizan, WS conecta (screenshot evidencia a audits/perf-stack-2026-09-20/).
- [x] **8.7** Invariante §33.1.3: XLEN delta explicado solo por detección real.
- [x] **8.8** Actualizar BOARD GOAL-WORKORDERS.md por WO cerrado.

## Self-Review

1. **Cobertura**: WO-1✓(T1) WO-2✓(T2) WO-3✓(T4) WO-4✓(T5) WO-5✓(T6) WO-6✓(T7) WO-8✓(T3). WO-7→a8, WO-9/10→89, WO-11/12 post-medición (Fase 3, plan separado). ✓
2. **Placeholders**: `<DIGEST_CAPTURADO>` es un valor capturado en ejecución (7.1) — comando exacto dado, no un TBD. ✓
3. **Consistencia**: branch PR-1+PR-2 unificados en perf/rust-release (decisión 4.4 documentada); PR-3 en perf/dataplane-tuning. ✓
