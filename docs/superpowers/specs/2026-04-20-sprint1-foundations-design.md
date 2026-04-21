# ArbitrageX v2 — Sprint 1 "Foundations" — Design Spec

**Fecha**: 2026-04-20
**Autor**: MEV Systems Architect (ArbitrageX v2)
**Estado**: PROPUESTO — pendiente de revisión final del usuario
**Sprint**: 1 de 8
**Arquitectura elegida**: Opción **C** (Hybrid aligned to canon) — Rust hot-path + TS/Node control-plane + Cloudflare Workers edge + Next.js frontend

---

## 0. Propósito del Sprint 1

Sprint 1 NO implementa MEV real. Implementa la **base cableada y auditable** sobre la que los Sprints 2–8 construyen. Lo que S1 entrega operativo:

1. Arquitectura por capas alineada al canon del toolkit (backend / edge / frontend / automation).
2. Contratos API canónicos entre los 7 servicios.
3. Esquema DB completo (9 tablas + índices + migrations runner idempotente).
4. Config tipada central, sin hardcodes.
5. Secretos fuera del código + policy documentada.
6. Logging estructurado JSON + `/health` + `/metrics` en **todos** los servicios backend + edge.
7. Kill-switch global + circuit-breaker skeleton compartido.
8. Docker Compose que **levanta el stack completo localmente**.
9. Scripts `bootstrap.sh`, `health-check.sh`, `smoke-test.sh` que **fallan si algo no está operativo** (no placeholders).
10. Frontend Next.js scaffold que consume `/status` del edge y muestra salud real del sistema.

Lo que S1 **NO** hace (y dónde se entrega):

| Área | Sprint |
|---|---|
| Cliente ethers-rs real, mempool WS, calldata decode, detección de patrones | S2 |
| Scoring multi-factor, token-safety cache con proveedor real, blacklist runtime | S3 |
| Fork Anvil real, `debug_traceCall`, gas accuracy determinista | S4 |
| Flashbots SDK, signer, bundle builder, nonce manager, multi-relay routing | S5 |
| Tx trace, PnL real, variance, learning loop, scoring adaptativo | S6 |
| Edge con rate-limit distribuido (KV), auth JWT productiva, frontend completo | S7 |
| Grafana dashboards, Alertmanager rules, Loki pipeline, E2E tests, backup/restore | S8 |

---

## 1. Arquitectura

### 1.1 Topología de servicios

```
┌─────────────────────────────────────────────────────────────────────┐
│                     FRONTEND (Next.js 14 TS)                        │
│   dashboard operativo · consume SOLO edge público                   │
└─────────────────────────────────────────────────────────────────────┘
                             │  HTTPS + JWT
                             ▼
┌─────────────────────────────────────────────────────────────────────┐
│              EDGE  (Cloudflare Worker, TS + wrangler)               │
│   auth · rate-limit · sanitización · read-cache (KV) · CORS         │
│   bindings: KV (read-cache) · D1 (telemetría ligera)                │
│   dev fallback local: edge/dev-local/ (Express, solo desarrollo)    │
└─────────────────────────────────────────────────────────────────────┘
                             │  HTTP interno + token compartido
                             ▼
┌─────────────────────────────────────────────────────────────────────┐
│                     CONTROL-PLANE (Node.js + TS)                    │
│   selector-api (:3002)       api-server/gateway (:8080)             │
└─────────────────────────────────────────────────────────────────────┘
                    │  Redis pub/sub + HTTP interno
                    ▼
┌─────────────────────────────────────────────────────────────────────┐
│                   HOT-PATH (Rust + tokio)                           │
│   searcher-rs  ·  sim-ctl  ·  relays-client  ·  recon               │
└─────────────────────────────────────────────────────────────────────┘
                             │
                             ▼
┌─────────────────────────────────────────────────────────────────────┐
│   PostgreSQL 15  ·  Redis 7.2  ·  Geth/Anvil  ·  Loki               │
│   Prometheus  ·  Grafana  ·  Alertmanager  ·  Promtail              │
└─────────────────────────────────────────────────────────────────────┘
```

### 1.2 Decisiones estructurales (10)

1. **Rust** para `searcher-rs`, `sim-ctl`, `relays-client`, `recon`. S1 entrega andamiaje + contratos + health/metrics; la lógica de chain/fork/relay llega en S2/S4/S5/S6.
2. **Node.js + TypeScript estricto** para `selector-api` y `api-server`. Se migra el scaffold JS actual a TS en S1.
3. **Cloudflare Workers (TS)** para `edge/worker/` con `wrangler.toml` y bindings KV+D1. El Express actual queda como `edge/dev-local/` solo desarrollo.
4. **Next.js 14 App Router (TS, RSC)** para frontend. Una única página `/status` en S1; páginas funcionales en S7.
5. **Red privada Docker** entre control-plane y hot-path. Edge es el único componente con exposición pública.
6. **Redis Streams** como event-bus interno: streams `opps.detected`, `opps.validated`, `opps.simulated`, `opps.executed`, `opps.reconciled`.
7. **Secretos**: `.env` dev-only; prod via `docker secret` en S1, migración a Vault/1Password Connect documentada para S7. Nunca embebidos en código.
8. **Config tipada central** en `configs/app.toml` + JSON Schema + loaders que fallan al boot si schema no valida.
9. **Pipeline canónico**: `Detect → Validate-cheap → Simulate → Select/Rank → Fund → Execute → Recon → Learn`. Orden reflejado en contratos y dependencies.
10. **Kill-switch global** = flag en Redis (`arbx:killswitch:enabled`) + endpoint `POST /admin/killswitch` en `api-server`. Todos los servicios lo consultan con TTL cache 1 s antes de cada acción crítica.

### 1.3 Matriz de fronteras de confianza (zero-trust)

| Origen | Destino | Estado | Mecanismo |
|---|---|---|---|
| Internet | edge | ✓ | Cloudflare TLS + WAF + rate-limit |
| Internet | cualquier otro servicio | ✗ BLOQUEADO | Docker network interna, no exposed ports en prod |
| edge | api-server | ✓ | Internal token header `X-ArbX-Edge-Token` + IP allowlist |
| edge | hot-path directo | ✗ BLOQUEADO | Debe pasar por api-server |
| api-server | selector-api | ✓ | HTTP interno + token |
| api-server | Redis | ✓ | AUTH + DB# separado para admin ops |
| selector-api | hot-path | ✓ | Redis Streams + HTTP interno |
| hot-path | PostgreSQL | ✓ | Role `arbx_rw` por servicio, migraciones con `arbx_migrator` |
| hot-path | Geth/Anvil | ✓ | Internal-only (`INTERNAL_GETH_HTTP`) |
| Cualquier servicio | Prometheus scrape | ✓ | `/metrics` vía red monitoring interna |

---

## 2. Componentes & Contratos API

### 2.1 Inventario de componentes (Sprint 1 scope)

| # | Servicio | Lenguaje | Puerto | Entregable S1 |
|---|---|---|---|---|
| 1 | `searcher-rs` | Rust | N/A (worker) | `/health` HTTP, `/metrics` Prometheus, config loader, Redis Streams publisher stub, logging tracing + JSON |
| 2 | `selector-api` | Node+TS | 3002 | Migrado de JS a TS, `/health`, `/metrics`, `/score`, validación Zod, DB pool, Redis client |
| 3 | `sim-ctl` | Rust | 3003 | `/health`, `/metrics`, `/simulate` devuelve `501 Not Implemented` con detalle estructurado; config loader |
| 4 | `relays-client` | Rust | 3005 | `/health`, `/metrics`, `/execute` devuelve `501 Not Implemented`; config loader |
| 5 | `recon` | Rust | 3004 | `/health`, `/metrics`, `/pnl/:opportunity_id` lee DB real (no hardcodes) |
| 6 | `api-server` (nuevo) | Node+TS | 8080 | `/health`, `/metrics`, `/admin/killswitch`, `/admin/config`, agregador upstream |
| 7 | `edge/worker` | CF Worker TS | 8787 (local) | `/health`, `/status`, `/api/opportunities/live` (proxy read-only), JWT verify stub, KV cache stub |
| 8 | `edge/dev-local` | Node Express | 8787 | Dev-only, misma interfaz que worker para pruebas sin CF |
| 9 | `frontend` | Next.js TS | 5173 | Scaffold App Router + página `/status` consumiendo edge |

> **Regla 501**: Cuando un endpoint requiere infra externa no configurada (Flashbots, Anvil fork, RPC real), devuelve `501 Not Implemented` con body `{"error":"not_implemented","requires":["FLASHBOTS_SIGNER_KEY",…],"sprint":"S5"}`. NUNCA devuelve datos fake.

### 2.2 Contrato de tipos compartidos (fuente de verdad)

Ubicación: `configs/schemas/` (JSON Schema) + `backend/shared-rs/` (Rust) + `shared-ts/` (TypeScript). Cada lenguaje genera/valida contra los JSON Schemas.

**Tipos principales** (definición abreviada, schemas completos en `configs/schemas/`):

```typescript
// Opportunity — producido por searcher-rs, consumido por selector-api
type Opportunity = {
  id: string;                 // UUID v4
  chain_id: number;           // 1 = mainnet, 137 = polygon, etc.
  strategy_kind: "dex_arb" | "triangular" | "backrun" | "liquidation" | "flashloan_arb";
  dex_a: string;              // "uniswap-v3", "curve", etc.
  dex_b: string | null;
  pair_symbol: string;        // "WETH/USDC"
  token_in: string;           // address checksum
  token_out: string;          // address checksum
  amount_in_wei: string;      // big-int as string
  expected_profit_usd: number;
  block_number: number;
  detected_at: string;        // ISO 8601
  trace_id: string;           // correlation id
};

// SimulationResult — producido por sim-ctl
type SimulationResult = {
  opportunity_id: string;
  passed: boolean;
  gas_estimate_wei: string;
  gas_price_wei: string;
  slippage_pct: number;
  revert_risk_pct: number;
  simulated_profit_usd: number;
  simulator: "anvil" | "tenderly" | "not_implemented";
  simulated_at: string;
  trace_id: string;
};

// ExecutionRequest — producido por selector-api, consumido por relays-client
type ExecutionRequest = {
  opportunity_id: string;
  simulation_id: string;
  relay_preference: string[]; // ordered
  max_gas_price_wei: string;
  deadline_block: number;
  trace_id: string;
};

// ExecutionResult — producido por relays-client
type ExecutionResult = {
  opportunity_id: string;
  status: "submitted" | "included" | "reverted" | "dropped" | "not_implemented";
  tx_hash: string | null;
  relay_used: string | null;
  block_included: number | null;
  gas_used_wei: string | null;
  actual_profit_usd: number | null;
  submitted_at: string;
  trace_id: string;
};

// ReconReport — producido por recon
type ReconReport = {
  opportunity_id: string;
  expected_profit_usd: number;
  actual_profit_usd: number;
  variance_usd: number;
  variance_pct: number;
  actual_gas_used_wei: string;
  notes: string | null;
  created_at: string;
  trace_id: string;
};

// KillSwitchState — consumido por todos
type KillSwitchState = {
  enabled: boolean;
  reason: string | null;
  triggered_by: string | null; // "manual" | "circuit_breaker:revert_rate" | ...
  updated_at: string;
};
```

### 2.3 Endpoints HTTP canónicos

**searcher-rs** (Rust, no HTTP server salvo health/metrics):
- `GET /health` → `{ok: true, service: "searcher-rs", version, uptime_s}`
- `GET /metrics` → Prometheus exposition

**selector-api** (Node TS :3002):
- `GET /health`
- `GET /metrics`
- `POST /score` → recibe `Opportunity + SimulationResult`, devuelve `{score, factors: {liquidity, depth, safety, slippage, gas, risk}, decision: "accept"|"reject", reason}`
- `GET /opportunities?status=detected|validated|simulated|executed&limit=50` → lee DB real

**sim-ctl** (Rust :3003):
- `GET /health`
- `GET /metrics`
- `POST /simulate` → S1 devuelve `501 {error:"not_implemented", requires:["ANVIL_FORK_URL"], sprint:"S4"}`

**relays-client** (Rust :3005):
- `GET /health`
- `GET /metrics`
- `POST /execute` → S1 devuelve `501 {error:"not_implemented", requires:["FLASHBOTS_SIGNER_KEY"], sprint:"S5"}`

**recon** (Rust :3004):
- `GET /health`
- `GET /metrics`
- `GET /pnl/:opportunity_id` → lee DB real; si no hay execution, `404`
- `GET /pnl/summary?since=<iso>` → agregados reales desde DB

**api-server** (Node TS :8080):
- `GET /health`
- `GET /metrics`
- `GET /status` → agrega health de todos los servicios upstream + killswitch state
- `POST /admin/killswitch` body `{enabled:bool, reason:string}` → requiere `X-ArbX-Admin-Token`
- `GET /admin/config` → devuelve config efectiva (secretos redactados)
- `GET /api/v1/opportunities/live` → proxy a selector-api con auth

**edge/worker** (CF Worker :8787 local / Workers URL prod):
- `GET /health`
- `GET /status` → proxy + cache (TTL 2s) a api-server `/status`
- `GET /api/opportunities/live` → proxy + cache + rate-limit a api-server
- `GET /api/risk/alerts` → proxy read-only
- Headers obligatorios hacia upstream: `X-ArbX-Edge-Token`, `X-ArbX-Trace-Id`
- Rate-limit S1: in-memory por-isolate (documentado como temporal); S7 → KV-backed distribuido

**frontend** (Next.js :5173):
- `/status` → consume edge `/status` y muestra salud real

---

## 3. Data Schema & Migrations

### 3.1 Tablas (9 totales)

```sql
-- 001_opportunities.sql
CREATE TABLE opportunities (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  chain_id INTEGER NOT NULL,
  strategy_kind TEXT NOT NULL CHECK (strategy_kind IN (
    'dex_arb','triangular','backrun','liquidation','flashloan_arb'
  )),
  dex_a TEXT NOT NULL,
  dex_b TEXT,
  pair_symbol TEXT,
  token_in TEXT NOT NULL,
  token_out TEXT NOT NULL,
  amount_in_wei NUMERIC(78,0) NOT NULL,
  expected_profit_usd NUMERIC(20,8),
  roi_pct NUMERIC(10,4),
  risk_score NUMERIC(10,4),
  block_number BIGINT,
  status TEXT NOT NULL DEFAULT 'detected' CHECK (status IN (
    'detected','validated','simulated','scored','executing',
    'executed','reconciled','rejected','failed'
  )),
  rejection_reason TEXT,
  trace_id UUID NOT NULL,
  detected_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX idx_opp_status_time ON opportunities(status, detected_at DESC);
CREATE INDEX idx_opp_chain_strategy ON opportunities(chain_id, strategy_kind);
CREATE INDEX idx_opp_trace ON opportunities(trace_id);

-- 002_simulations.sql
CREATE TABLE simulations (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  opportunity_id UUID NOT NULL REFERENCES opportunities(id) ON DELETE CASCADE,
  simulator TEXT NOT NULL CHECK (simulator IN ('anvil','tenderly','hardhat','not_implemented')),
  gas_estimate_wei NUMERIC(78,0),
  gas_price_wei NUMERIC(78,0),
  slippage_pct NUMERIC(10,4),
  revert_risk_pct NUMERIC(10,4),
  simulated_profit_usd NUMERIC(20,8),
  passed BOOLEAN NOT NULL DEFAULT FALSE,
  fail_reason TEXT,
  raw_trace JSONB,
  trace_id UUID NOT NULL,
  simulated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX idx_sim_opp ON simulations(opportunity_id);
CREATE INDEX idx_sim_passed_time ON simulations(passed, simulated_at DESC);

-- 003_executions.sql
CREATE TABLE executions (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  opportunity_id UUID NOT NULL REFERENCES opportunities(id) ON DELETE CASCADE,
  simulation_id UUID REFERENCES simulations(id),
  relay_name TEXT NOT NULL,
  tx_hash TEXT UNIQUE,
  bundle_hash TEXT,
  block_included BIGINT,
  expected_profit_usd NUMERIC(20,8),
  actual_profit_usd NUMERIC(20,8),
  gas_used_wei NUMERIC(78,0),
  gas_price_effective_wei NUMERIC(78,0),
  status TEXT NOT NULL CHECK (status IN (
    'submitted','included','reverted','dropped','replaced','not_implemented'
  )),
  error_message TEXT,
  raw_receipt JSONB,
  trace_id UUID NOT NULL,
  submitted_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  confirmed_at TIMESTAMPTZ
);
CREATE INDEX idx_exec_opp ON executions(opportunity_id);
CREATE INDEX idx_exec_status_time ON executions(status, submitted_at DESC);
CREATE INDEX idx_exec_relay_time ON executions(relay_name, submitted_at DESC);
CREATE INDEX idx_exec_tx ON executions(tx_hash) WHERE tx_hash IS NOT NULL;

-- 004_strategy_scores.sql
CREATE TABLE strategy_scores (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  strategy_kind TEXT NOT NULL,
  chain_id INTEGER NOT NULL,
  window_start TIMESTAMPTZ NOT NULL,
  window_end TIMESTAMPTZ NOT NULL,
  sample_count INTEGER NOT NULL,
  success_rate NUMERIC(10,6),
  avg_profit_usd NUMERIC(20,8),
  revert_rate NUMERIC(10,6),
  score NUMERIC(10,4),
  enabled BOOLEAN NOT NULL DEFAULT TRUE,
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  UNIQUE (strategy_kind, chain_id, window_end)
);
CREATE INDEX idx_strat_recent ON strategy_scores(chain_id, strategy_kind, window_end DESC);

-- 005_relay_scores.sql
CREATE TABLE relay_scores (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  relay_name TEXT NOT NULL,
  chain_id INTEGER NOT NULL,
  window_start TIMESTAMPTZ NOT NULL,
  window_end TIMESTAMPTZ NOT NULL,
  submitted INTEGER NOT NULL DEFAULT 0,
  included INTEGER NOT NULL DEFAULT 0,
  reverted INTEGER NOT NULL DEFAULT 0,
  dropped INTEGER NOT NULL DEFAULT 0,
  inclusion_rate NUMERIC(10,6),
  avg_latency_ms NUMERIC(12,2),
  score NUMERIC(10,4),
  enabled BOOLEAN NOT NULL DEFAULT TRUE,
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  UNIQUE (relay_name, chain_id, window_end)
);
CREATE INDEX idx_relay_recent ON relay_scores(chain_id, relay_name, window_end DESC);

-- 006_token_safety_cache.sql
CREATE TABLE token_safety_cache (
  chain_id INTEGER NOT NULL,
  token_address TEXT NOT NULL,
  safety_score INTEGER NOT NULL CHECK (safety_score BETWEEN 0 AND 100),
  flags JSONB NOT NULL DEFAULT '{}'::jsonb,
  source TEXT NOT NULL,      -- 'goplus','honeypot.is','internal','unknown'
  ttl_expires_at TIMESTAMPTZ NOT NULL,
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  PRIMARY KEY (chain_id, token_address)
);
CREATE INDEX idx_token_expiry ON token_safety_cache(ttl_expires_at);

-- 007_risk_events.sql
CREATE TABLE risk_events (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  event_type TEXT NOT NULL,  -- 'circuit_breaker','kill_switch','blacklist_hit','degradation'
  severity TEXT NOT NULL CHECK (severity IN ('info','warning','critical')),
  source_service TEXT NOT NULL,
  payload JSONB NOT NULL,
  trace_id UUID,
  opportunity_id UUID REFERENCES opportunities(id) ON DELETE SET NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX idx_risk_sev_time ON risk_events(severity, created_at DESC);
CREATE INDEX idx_risk_type_time ON risk_events(event_type, created_at DESC);

-- 008_incident_log.sql
CREATE TABLE incident_log (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  title TEXT NOT NULL,
  status TEXT NOT NULL CHECK (status IN ('open','investigating','mitigated','resolved')),
  severity TEXT NOT NULL,
  started_at TIMESTAMPTZ NOT NULL,
  resolved_at TIMESTAMPTZ,
  root_cause TEXT,
  remediation TEXT,
  related_risk_event_ids UUID[]
);
CREATE INDEX idx_incident_status ON incident_log(status, started_at DESC);

-- 009_audit_log.sql
CREATE TABLE audit_log (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  actor TEXT NOT NULL,          -- 'user:admin@domain' | 'service:api-server' | 'system'
  action TEXT NOT NULL,         -- 'killswitch.enable','config.update','opportunity.reject'
  target_kind TEXT,             -- 'opportunity','relay','strategy','config'
  target_id TEXT,
  before_state JSONB,
  after_state JSONB,
  ip_address INET,
  user_agent TEXT,
  trace_id UUID,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX idx_audit_actor_time ON audit_log(actor, created_at DESC);
CREATE INDEX idx_audit_action_time ON audit_log(action, created_at DESC);
```

### 3.2 Migrations runner

- Herramienta: **`node-pg-migrate`** invocada desde `automation/scripts/migrate.sh` (simple, Node ya está en control-plane).
- Ubicación: `database/migrations/*.sql` con prefijo `NNN_name.sql` (idempotente: `CREATE TABLE IF NOT EXISTS`, `CREATE INDEX IF NOT EXISTS`, bloque `DO $$ BEGIN … EXCEPTION WHEN duplicate_object THEN NULL; END $$` para checks/enums).
- Tabla de control: `schema_migrations (version TEXT PRIMARY KEY, applied_at TIMESTAMPTZ)`.
- Roles DB creados en `001_roles.sql`: `arbx_migrator` (DDL), `arbx_rw` (DML), `arbx_ro` (SELECT).

### 3.3 Seed data (dev-only)

`database/seed/dev_chains.sql` carga chains (1, 10, 137, 42161, 8453) y relays conocidos (`flashbots`, `bloxroute`, `eden`, `beaver`, `titan`) en tablas de lookup — S2 en adelante las usan. **Nunca se carga en prod.**

---

## 4. Observabilidad, Seguridad & Kill-switch

### 4.1 Logging estructurado

- **Formato**: JSON Lines a stdout. Campos obligatorios: `timestamp` (ISO 8601), `level`, `service`, `trace_id`, `opportunity_id` (si aplica), `message`, `context` (objeto).
- **Rust**: `tracing` + `tracing-subscriber` con formatter JSON (`tracing_subscriber::fmt::json()`).
- **Node TS**: `pino` con `pino-http` para servicios HTTP.
- **Trace propagation**: header `X-ArbX-Trace-Id` en **todos** los HTTP calls internos; si ausente, cada servicio genera uno y lo propaga.
- **Collection**: `promtail` scrapea stdout de contenedores Docker → **Loki** (añadido al stack). Grafana consume Loki como datasource.

### 4.2 Métricas Prometheus

Cada servicio expone `/metrics`. Counters/histogramas base en S1 (valores reales, no inventados):

| Métrica | Tipo | Labels | Servicio |
|---|---|---|---|
| `arbx_http_requests_total` | counter | `service`, `method`, `path`, `status` | todos HTTP |
| `arbx_http_request_duration_seconds` | histogram | `service`, `method`, `path` | todos HTTP |
| `arbx_opportunity_total` | counter | `chain_id`, `strategy_kind`, `status` | searcher-rs, selector-api |
| `arbx_simulation_total` | counter | `simulator`, `passed` | sim-ctl |
| `arbx_execution_total` | counter | `relay`, `status`, `chain_id` | relays-client |
| `arbx_killswitch_enabled` | gauge | — | api-server |
| `arbx_service_up` | gauge | `service` | todos |
| `arbx_config_reload_total` | counter | `service`, `result` | todos |

`prometheus.yml` apunta a todos los servicios via `scrape_configs` estáticos (container DNS).

### 4.3 Kill-switch & Circuit-breaker skeleton

**Kill-switch**:
- Estado canónico: Redis key `arbx:killswitch` (JSON = `KillSwitchState`).
- API: `POST /admin/killswitch` en api-server → valida `X-ArbX-Admin-Token` → update Redis + escribe a `audit_log` + publica a Redis channel `arbx:killswitch:changes`.
- Clientes (todos los servicios): `KillSwitchClient` con cache TTL 1s. Cada acción crítica (detect/publish, simulate call, execute submit) llama `client.check()` y aborta si `enabled=true`.
- Suscripción opcional al pub/sub para actualización inmediata (sin esperar TTL).

**Circuit-breaker skeleton** (implementación completa en S3/S5; S1 entrega la lib + wiring):
- Lib compartida `shared/circuit_breaker/` (Rust crate + npm package TS).
- Estados: `closed → open (N fallos en ventana T) → half_open (después de cooldown) → closed`.
- Métricas expuestas: `arbx_cb_state{name,service}`, `arbx_cb_trips_total{name,service}`.
- En S1: breakers instanciados pero sin triggers reales (no hay ejecuciones reales todavía).

### 4.4 Secret management

**S1 entrega**:
- `.env.example` sanitizado (sin valores, solo nombres + comentario de origen).
- `configs/secrets.policy.md` documentando:
  - Qué es secreto (claves privadas, tokens admin, API keys, passwords DB).
  - Dónde vive por entorno: `dev` → `.env` local gitignored; `staging/prod` → `docker secret` + envFile externo; `S7` → Vault.
  - Regla de rotación (90 días tokens admin, inmediata ante incidente, nunca por email/chat).
- Validación fail-fast: cada servicio valida vía Zod (TS) / `serde` + `validator` (Rust) al boot. Si falta un secreto requerido → exit 1 con mensaje claro (sin leak del valor).
- `.gitignore` actualizado: `.env`, `.env.*.local`, `secrets/`, `*.pem`, `*.key`.
- CI hook (documentado para S8): `gitleaks` pre-commit + pre-push.

### 4.5 Observabilidad stack actualizado

Añadido a `monitoring/`:
- `monitoring/loki/loki-config.yml`
- `monitoring/promtail/promtail-config.yml`

`docker-compose.prod-like.yml` añade servicios `loki` y `promtail`, monta `/var/lib/docker/containers` a promtail (ro) para scrape.

Grafana provisioning (datasources.yml + dashboards folder) apunta a Prometheus + Loki. Dashboards reales en S8.

### 4.6 Alertas base (Prometheus rules — S1 mínimo)

`monitoring/alerts.rules.yml` (expandido desde scaffold):
- `ServiceDown` — `arbx_service_up == 0 for 1m` → severity critical.
- `KillSwitchActivated` — `arbx_killswitch_enabled == 1` → severity warning.
- `HighHTTP5xxRate` — `rate(arbx_http_requests_total{status=~"5.."}[5m]) > 0.1` → severity warning.
- `ConfigReloadFailed` — `increase(arbx_config_reload_total{result="fail"}[15m]) > 0` → severity critical.

Alertmanager en S1 loguea solo; integración Slack/PagerDuty documentada para S8.

---

## 5. Testing, Verification & Automation

### 5.1 Estrategia de testing por capa (S1 scope)

| Capa | Framework | Alcance S1 |
|---|---|---|
| Rust (searcher-rs, sim-ctl, relays-client, recon) | `cargo test` + `tokio::test` | Unit tests para config loader, kill-switch client, health handler, JSON schema validation |
| Node TS (selector-api, api-server) | `vitest` + `supertest` | Unit + HTTP tests para `/health`, `/metrics`, `/score`, `/admin/killswitch` |
| CF Worker (edge) | `vitest` + `@cloudflare/workers-types` + `miniflare` | Integration tests contra worker local (rate-limit, cache, auth) |
| Frontend | `vitest` + Next.js test utils | Renderizado básico `/status` con mock fetch |
| DB | `pgtap` (opcional) o test en vitest con container Postgres ephemeral | Verifica que todas las migraciones aplican limpio + down/up idempotente |
| E2E S1 | `automation/scripts/smoke-test.sh` (bash) | Levanta stack con docker-compose, espera health-ready, ejecuta ~25 checks HTTP |

**No se escriben tests para código que no existe en S1** (lógica MEV real va en S2+).

### 5.2 Scripts automation (todos fallan con exit≠0 si algo falla)

**`automation/scripts/bootstrap.sh`**:
1. Verifica binarios: `docker`, `docker compose`, `node`, `cargo`, `psql`.
2. Verifica `.env` presente (si no, copia de `.env.example` y exit 2 con mensaje).
3. Valida config: `node tools/validate-config.js configs/app.toml configs/schemas/app.schema.json`.
4. Build de imágenes: `docker compose build`.
5. Levanta dependencias: `docker compose up -d postgres redis geth prometheus grafana alertmanager loki promtail`.
6. Espera DB ready (pg_isready loop, timeout 60s).
7. Ejecuta migraciones: `automation/scripts/migrate.sh`.
8. Levanta servicios aplicativos.
9. Ejecuta `health-check.sh`.

**`automation/scripts/health-check.sh`**:
- Hace `GET /health` a cada servicio (9 endpoints).
- Verifica DB reachable + schema_migrations count > 0.
- Verifica Redis reachable.
- Verifica Prometheus targets UP (via `/api/v1/targets`).
- Reporta tabla con OK/FAIL; exit code = #fallos.

**`automation/scripts/smoke-test.sh`**:
- Corre `health-check.sh` primero.
- Prueba pipeline síncrono:
  - `POST /score` a selector-api con opportunity de prueba → verifica respuesta estructural.
  - `POST /simulate` a sim-ctl → verifica respuesta `501` bien formada.
  - `POST /execute` a relays-client → verifica respuesta `501` bien formada.
  - `GET /status` a api-server → verifica agregación de healths.
  - `GET /status` al edge (worker local o dev-local) → verifica cache headers.
  - `POST /admin/killswitch` habilitar → verifica propagación → deshabilitar.
- Suma total ~25 assertions.

**`automation/scripts/migrate.sh`**: invoca node-pg-migrate con `DATABASE_URL`, reporta migrations aplicadas.

**`automation/scripts/validate-config.sh`**: valida `app.toml` contra `app.schema.json`.

**`automation/scripts/rollback.sh`**: `docker compose down` + documentación de `pg_dump` previo y estrategia de restore (plan completo S8).

### 5.3 Criterios de aceptación Sprint 1 (verificables)

Sprint 1 se considera completo sólo si:

- [ ] `bootstrap.sh` termina con exit 0 y todos los contenedores `healthy`.
- [ ] `health-check.sh` reporta 9/9 OK.
- [ ] `smoke-test.sh` pasa 100% assertions.
- [ ] `cargo test --workspace` en backend/ pasa.
- [ ] `pnpm test` (o `npm test`) en cada servicio Node + frontend pasa.
- [ ] `wrangler dev` levanta el worker y responde `/health`.
- [ ] `psql ... -c "SELECT count(*) FROM schema_migrations"` ≥ 9.
- [ ] Prometheus `/targets` muestra todos los jobs como `UP`.
- [ ] Grep `rg "TODO|FIXME|XXX|HACK|MOCK" --type rust --type ts` no encuentra los marcadores de S2+; los restantes documentan Sprint explícito.
- [ ] Grep `rg "hardcode|placeholder|not_implemented"` solo aparece en endpoints documentados como 501.
- [ ] `gitleaks detect --source .` no encuentra secretos.
- [ ] Cada servicio tiene `/health` 200 y `/metrics` Prometheus-parseable.
- [ ] Matrix de trust boundaries (§1.3) verificada con `nmap`/`curl` desde fuera de la docker network.

---

## 6. Estructura de repo resultante tras Sprint 1

```
arbitragex_v2_productivo_full/
├── .env.example                    # sanitizado
├── .gitignore                      # actualizado
├── README.md                       # actualizado con orden de sprints
├── docker/
│   └── docker-compose.prod-like.yml    # + loki, promtail, api-server
├── configs/
│   ├── app.toml                    # config efectiva default
│   ├── schemas/                    # JSON Schemas fuente de verdad
│   │   ├── app.schema.json
│   │   ├── opportunity.schema.json
│   │   ├── simulation_result.schema.json
│   │   ├── execution_request.schema.json
│   │   ├── execution_result.schema.json
│   │   ├── recon_report.schema.json
│   │   └── killswitch_state.schema.json
│   ├── secrets.policy.md
│   └── nginx/nginx.conf            # dev-only reverse proxy
├── database/
│   ├── init/001_init.sql           # reescrito
│   ├── migrations/
│   │   ├── 001_roles.sql
│   │   ├── 002_schema_migrations.sql
│   │   ├── 003_opportunities.sql
│   │   ├── 004_simulations.sql
│   │   ├── 005_executions.sql
│   │   ├── 006_strategy_scores.sql
│   │   ├── 007_relay_scores.sql
│   │   ├── 008_token_safety_cache.sql
│   │   ├── 009_risk_events.sql
│   │   ├── 010_incident_log.sql
│   │   └── 011_audit_log.sql
│   └── seed/dev_chains.sql         # dev-only
├── backend/
│   ├── shared-rs/                  # nueva crate: config, kill-switch client, metrics, tracing
│   ├── searcher-rs/                # refactor: +health/metrics, +config, +kill-switch client
│   ├── sim-ctl/                    # REESCRITO a Rust desde Node JS
│   ├── relays-client/              # REESCRITO a Rust desde Node JS
│   ├── recon/                      # REESCRITO a Rust desde Node JS
│   ├── selector-api/               # migrado JS → TS
│   └── api-server/                 # NUEVO servicio Node TS
├── shared-ts/                      # npm workspace: config loader, kill-switch client, pino setup
├── edge/
│   ├── worker/                     # CF Worker TS + wrangler.toml
│   ├── dev-local/                  # Express dev-only (ex edge/src/)
│   └── README.md
├── frontend/
│   ├── app/                        # Next.js 14 App Router
│   ├── lib/api-client.ts
│   ├── next.config.js
│   └── package.json
├── monitoring/
│   ├── prometheus/prometheus.yml
│   ├── grafana/
│   │   ├── datasources/datasources.yml
│   │   └── dashboards/             # vacío S1, llenado S8
│   ├── alertmanager/alertmanager.yml
│   ├── loki/loki-config.yml        # NUEVO
│   ├── promtail/promtail-config.yml    # NUEVO
│   └── alerts.rules.yml            # reglas S1
├── automation/
│   ├── scripts/
│   │   ├── bootstrap.sh
│   │   ├── health-check.sh
│   │   ├── smoke-test.sh
│   │   ├── migrate.sh
│   │   ├── validate-config.sh
│   │   ├── rollback.sh
│   │   └── seed-dev.sh
│   └── tools/
│       └── validate-config.js
└── docs/
    ├── SOP_ENTERPRISE.md
    ├── ARQUITECTURA_TECNICA.md     # actualizado post-S1
    ├── API_CONTRACTS.md            # actualizado con pipeline correcto
    ├── TRUST_POLICY.md             # NUEVO (Verificado/Narrativo/No disponible)
    ├── CANONICAL_SOURCES.md        # NUEVO (mapeo a repos canónicos)
    ├── RISK_POLICY.md
    ├── ROADMAP_FASES.md            # actualizado con 8 sprints
    ├── MULTIAGENT_DECISION_TREE.mmd
    └── superpowers/specs/
        └── 2026-04-20-sprint1-foundations-design.md   # este doc
```

**Resumen de cambios estructurales**:
- **Nuevos**: `backend/shared-rs/`, `backend/api-server/`, `shared-ts/`, `edge/worker/`, `monitoring/loki/`, `monitoring/promtail/`, 9 tablas SQL, 6 scripts automation, 7 JSON Schemas.
- **Reescritos**: `sim-ctl`, `relays-client`, `recon` migrados de Node JS a Rust.
- **Migrados**: `selector-api` JS → TS; `edge/src/` → `edge/dev-local/`; `frontend/` a Next.js 14.
- **Actualizados**: `docker-compose.prod-like.yml`, `.env.example`, `.gitignore`, docs.

---

## 7. Dependencias externas y lo que queda BLOQUEADO

Sprint 1 **no requiere** credenciales externas — es ejecutable end-to-end en laptop local. Queda explícito lo que en Sprints posteriores sí las requerirá:

| Dependencia externa | Sprint que la necesita | Efecto si falta |
|---|---|---|
| RPC WS real (Alchemy/Infura/self-hosted) | S2 | searcher-rs no detecta nada real |
| Clave Anvil fork o Tenderly API | S4 | sim-ctl permanece en 501 |
| Flashbots signer key + endpoints de relays | S5 | relays-client permanece en 501 |
| API key GoPlus/HoneyAPI (token safety) | S3 | token_safety_cache se pobla solo vía heurística interna |
| Dominio + Cloudflare account (edge prod) | S7 | edge solo corre en dev-local |
| Vault/1Password Connect endpoint | S7 | secretos vía docker secret + envFile |

---

## 8. Riesgos abiertos (estado a cierre de S1)

1. **Latencia no medida**: S1 expone métricas pero no hay carga real → no hay baseline. Mitigación: benchmarks sintéticos añadidos en S2.
2. **Concurrencia de kill-switch**: TTL 1s permite ventana de ~1s donde un servicio actúa con estado viejo. Mitigación: suscripción pub/sub a `arbx:killswitch:changes` para cierre inmediato; aceptamos la ventana como trade-off consciente.
3. **Rate-limit en edge (S1)**: in-memory per-isolate, no distribuido. Explícitamente documentado como temporal; reemplazo en S7 usando KV.
4. **No hay auth real en `/admin/*`**: S1 usa token estático (`X-ArbX-Admin-Token`). OAuth/SSO en S7.
5. **Fork simulation ausente**: cualquier path a ejecución real está físicamente bloqueado por respuestas 501 hasta S4/S5. Mitigación: kill-switch por default `enabled=true` en entornos no-dev.
6. **Single-region**: stack local, sin HA ni failover. Documentado para futuro S9+.

---

## 9. Criterios para avanzar al siguiente Sprint

S2 arranca cuando:
- Todos los criterios §5.3 verificados.
- Spec de S2 (detection real) aprobado por el usuario.
- Al menos 1 RPC WS de prueba disponible (Alchemy free tier sirve).

---

## 10. NO-MENTIRAS de cierre (lo que S1 **no** deja hecho)

- S1 **NO** captura ninguna oportunidad real.
- S1 **NO** simula transacciones reales.
- S1 **NO** envía nada a ningún relay.
- S1 **NO** calcula PnL real — `recon` devuelve 404 si no hay execution en DB.
- S1 **NO** protege frente a front-run, sandwich, MEV adversarial — eso es S3+S5+S6.
- S1 **NO** da autorización para operar con capital real — eso requiere S1→S8 completo + paper trading validado.

S1 deja el sistema listo para que **cada uno de los sprints siguientes tenga una base verificable** donde conectar su lógica específica.

---

**Fin del Spec Sprint 1.**
