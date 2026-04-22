# Sprints 7 + 8 + Runtime-real — Consolidated Design

**Date:** 2026-04-21
**Status:** approved-for-execution (consolidated; no per-section gates)
**Scope:** Three parallel streams that move ArbitrageX v2 from "built & green in CI" to "running productively on the Hetzner VPS with real mempool data and operator-ready UX".

Previous completed sprints: S1 Foundations, S2 Detection Real, S3 Selector, S4 Simulation, S5 Execution (paper-mode rail), S6 Recon Learning.

---

## 1. Goals (what "done" means)

**Stream A — Runtime real (unblocks everything else)**
- `searcher-rs` connects to a real Ethereum mainnet WS endpoint and emits `arbx_opportunity_total{status="detected"}` from real pending txs.
- No fabricated data anywhere — if RPC is down, service stays healthy but idle (current `scanner::idle_chain_loop` behavior is preserved).
- Paper-mode stays ON. No capital at risk.

**Stream B — Sprint 7 (productive edge + operator UX + secrets)**
- Public edge: Cloudflare Worker `arbx-edge` deployed to `workers.dev` and also reachable via a custom domain served by a Cloudflare Tunnel into the VPS (for internal-only admin endpoints).
- Frontend: 8 operational pages consuming real edge endpoints. No synthesized data; pages show errors verbatim when upstream is unhealthy.
- Secrets: HashiCorp Vault running on the VPS (self-hosted, file backend, sealed), bootstraps service env files at container start. GitHub token + Alchemy key + Flashbots signer + JWT secret all move out of `.env` on disk into Vault with AppRole auth.

**Stream C — Sprint 8 (observability + reliability + docs)**
- Grafana: 4 provisioned dashboards populated exclusively from `arbx_*` metrics that actually exist today.
- Alertmanager: real Slack webhook + PagerDuty routing; critical routes to PD, warnings to Slack, info suppressed off-hours.
- E2E tests: Playwright against the compose stack hitting edge → frontend → status page; happy-path + kill-switch + RPC-down.
- Backups: nightly `pg_dump` to a local encrypted volume + rclone-to-B2 with 14-day retention; restore rehearsal script.
- Runbooks: `docs/runbooks/{killswitch,rpc-down,relay-degraded,db-restore,rotate-secrets}.md` — each with a "if you see alert X, run commands Y, validate Z".

---

## 2. Non-goals (for this spec)

- Chains beyond Ethereum mainnet. Polygon/Arbitrum/Base stay disabled in `configs/app.toml`.
- Real capital execution. Paper-mode guardrail stays on until S9.
- Multi-region HA. Single VPS is sufficient for this milestone.
- UI design system / polish. Pages are functional, not branded.

---

## 3. Stream A — Runtime real

### 3.1 RPC provider selection
- **Decision:** Alchemy free tier for both WS and HTTP. 300M compute units/mo is enough to observe mempool at mainnet rates. Fallback: Infura or QuickNode. Keys are injected via Vault → `.env` at container boot, never checked in.

### 3.2 Wiring
Currently `backend/searcher-rs/src/scanner.rs:43` reads `RPC_WS_{chain_id}` directly from env. We keep that. The Alchemy key lands in:

```
RPC_WS_1=wss://eth-mainnet.g.alchemy.com/v2/<KEY>
RPC_HTTP_1=https://eth-mainnet.g.alchemy.com/v2/<KEY>
```

`sim-ctl` additionally consumes `RPC_HTTP_1` for Anvil fork URL when `ANVIL_FORK_URL` is empty (lazy default). `relays-client` uses `RPC_HTTP_1` only for tx status lookups (optional).

### 3.3 Acceptance
- `docker compose --profile sim up -d` → `searcher-rs` logs `scanner.subscribed` (not `scanner.no_rpc`, not `scanner.idle`).
- After 60 s, `curl http://localhost:9001/metrics | grep opportunity_total` shows non-zero `detected` counter for `chain_id=1`.
- `psql` shows at least one row in `opportunities` table (if DB is up) or Redis stream has entries (`XLEN arbx:opportunities.stream`).
- Prometheus scrape target `searcher-rs:9001` is UP.
- Kill-switch still works: `curl -H 'x-arbx-admin-token: $ADMIN' -XPOST http://localhost:8080/admin/killswitch/on` → logs within 5 s show `scanner.paused`.

---

## 4. Stream B — Sprint 7

### 4.1 Cloudflare Tunnel + Worker deploy

**Architecture**

```
Internet
   │
   ├── arbx-edge.<your-domain>.workers.dev          ──►  Worker (read-only, cached)
   │
   └── ops.<your-domain>        ──►  CF Tunnel ──►  nginx on VPS (admin UI, basic auth)
```

- Worker: the existing `edge/worker/src/index.ts` is production-ready code. What's missing is deploy automation: `wrangler deploy --env production` with KV + D1 IDs injected from CI secrets (GitHub Actions). `ALLOWED_ORIGINS` tightens to `https://arbx.<domain>`.
- Tunnel: new component `infra/cloudflared/` with a compose service that holds the tunnel credential JSON (mounted from Vault-rendered file). Exposes `ops.<domain>` → `nginx:80`. Nginx routes `/api/admin/*` → `api-server:8080` with TLS terminated at CF, basic-auth at nginx.

**New files**
- `infra/cloudflared/config.yml` — tunnel → ingress mapping.
- `infra/cloudflared/Dockerfile` — pinned `cloudflare/cloudflared:2024.10.0`.
- `configs/nginx/ops.conf` — reverse proxy for admin routes.
- `.github/workflows/edge-deploy.yml` — deploys Worker on push to `main` when `edge/worker/**` changes.

### 4.2 Frontend — 8 operational pages

Pages (all Next.js app-router server components, each backed by a real edge endpoint):

| # | Route | Purpose | Edge endpoint |
|---|-------|---------|---------------|
| 1 | `/` | Operator landing + links | static |
| 2 | `/status` | System + kill-switch + per-service health | `GET /status` (exists) |
| 3 | `/opportunities` | Live feed (SSE) from mempool | `GET /api/opportunities/live` (exists) |
| 4 | `/executions` | Recent bundle submissions + outcomes | `GET /api/v1/executions/recent` (new in api-server) |
| 5 | `/risk` | Active alerts + circuit-breaker state | `GET /api/risk/alerts` (exists) |
| 6 | `/recon` | PnL aggregates + anomaly events | `GET /api/v1/recon/summary` (new) |
| 7 | `/config` | View `configs/app.toml` (read-only) + hot-reload trigger | `GET /api/v1/config/current` (new, admin) |
| 8 | `/killswitch` | Toggle UI (admin token required) | `POST /api/v1/admin/killswitch/{on,off}` (exists) |

Shared chrome (`app/layout.tsx`) gets a nav sidebar. Each page follows the same "if `res.ok=false` show error verbatim, never fabricate" pattern already in `/status`.

**Auth boundary:** pages 7 and 8 require `ARBX_ADMIN_TOKEN` entered in a session cookie via a `/signin` modal (edge issues short-lived JWT). For now, localhost dev skips this.

### 4.3 Vault

**Decision:** HashiCorp Vault 1.15 OSS, single-node, file backend, auto-unseal via 3-of-5 shamir keys that operator pastes at boot (no cloud KMS dependency). AppRole auth per service; role_id baked in, secret_id rotated weekly.

**Layout**
```
secret/arbitragex/prod/
  rpc/alchemy_key
  relays/flashbots_signer_key
  tokens/admin
  tokens/edge
  tokens/jwt
  tokens/cloudflare_tunnel
  grafana/admin_password
  postgres/password
```

**Integration** — a small init container (`infra/vault-agent/`) renders `.env` from templates at compose `up` and the other services `depends_on` it. If Vault is sealed → init container fails → compose stack fails to start (fail-closed).

**Migration path:** `.env` on disk stays as a development convenience only. `configs/secrets.policy.md` gets a "Vault mode" section.

### 4.4 Acceptance
- `curl -I https://arbx-edge.<user>.workers.dev/health` → 200 with `x-arbx-trace-id` header.
- `curl -I https://ops.<domain>/healthz` via tunnel → 200.
- All 8 frontend routes load in local compose; kill-switch route flips state and `/status` reflects within 5 s.
- `vault kv get secret/arbitragex/prod/rpc/alchemy_key` returns the key; removing it and `docker compose up searcher-rs` fails fast with a clear error.

---

## 5. Stream C — Sprint 8

### 5.1 Grafana dashboards (4)

All provisioned via `monitoring/grafana/dashboards/*.json`, loaded by the existing dashboards-provisioning volume. Each panel's query is grep-verified against `shared-rs/src/metrics.rs` + `shared-ts/src/metrics/index.ts` before commit.

1. **Platform overview** — `arbx_service_up` per service, `arbx_http_requests_total` rate/status breakdown, `arbx_killswitch_enabled`.
2. **Detection pipeline** — `arbx_opportunity_total{status}` rate, dedup hit/miss ratio, scanner reconnect count, decode-failed rate.
3. **Execution pipeline** — selector score histogram, sim pass/fail rate, bundle submit latency p50/p95/p99, revert rate.
4. **Recon & risk** — PnL over window, anomaly event count, circuit-breaker state table, token-safety cache hit rate.

### 5.2 Alertmanager

Replace the placeholder webhook in `monitoring/alertmanager/alertmanager.yml` with:
- `critical` → PagerDuty (integration key from Vault).
- `warning` → Slack `#arbx-alerts` webhook.
- `info` → Slack only, inhibited 22:00–08:00 UTC.
- Existing rules in `monitoring/alerts.rules.yml` kept as-is; add `RevertRateAboveThreshold` and `RelaySubmitFailuresSpiking`.

### 5.3 E2E tests

`tests/e2e/` with Playwright:
- `smoke.spec.ts` — all 8 frontend pages load and don't show "edge unreachable".
- `killswitch.spec.ts` — toggle via UI, verify `/status` updates, revert.
- `rpc-down.spec.ts` — unset `RPC_WS_1`, restart searcher-rs, verify `/status` shows the chain as "idle" without lying about it.
- CI job `.github/workflows/e2e.yml` brings up compose, waits for health, runs Playwright, tears down.

### 5.4 Backups

- `automation/scripts/backup-pg.sh` — `pg_dump` → `/var/backups/arbx/pg-<YYYYMMDD-HHMM>.sql.gz`, encrypted with `age` using an operator public key stored at `configs/backup-recipient.age.pub`.
- Cron on VPS (systemd timer, not docker cron): daily 03:15 UTC.
- `automation/scripts/backup-offsite.sh` — rclone to Backblaze B2 bucket `arbx-backups` with 14-day retention (rclone lifecycle rule).
- `automation/scripts/restore-pg.sh` — takes a file path, decrypts, psql restores to a staging DB name. **Tested** as part of CI on each PR that touches `automation/scripts/backup-*.sh`.

### 5.5 Runbooks

`docs/runbooks/` — new dir. Each runbook follows this template:
```
# <alert name / incident>
## Symptoms
## Immediate action (≤ 2 min)
## Diagnosis
## Remediation
## Post-incident
```
Initial set: `killswitch-activated`, `rpc-down`, `relay-degraded`, `db-restore`, `rotate-secrets`, `vault-sealed`.

### 5.6 Acceptance
- All 4 Grafana dashboards render without "No data" panels when stack is up for >5 min with a real RPC.
- A `arbx_service_up == 0` test alert reaches Slack within 60 s.
- `npm run e2e` passes locally.
- `automation/scripts/backup-pg.sh && automation/scripts/restore-pg.sh` round-trips a test row.
- Each runbook has been walked through manually once on the VPS.

---

## 6. Dependencies on external/user-provided inputs

Required from operator before full execution:

| Need | Why | Blocks |
|------|-----|--------|
| Alchemy (or equivalent) API key | Real RPC | Stream A entirely |
| Cloudflare account + API token + domain | Tunnel + Worker custom domain | Stream B §4.1 |
| Slack incoming webhook URL | Warning/info alerts | Stream C §5.2 partial |
| PagerDuty integration key (Events API v2) | Critical alerts | Stream C §5.2 partial |
| Backblaze B2 App Key (or S3-compatible) | Off-site backups | Stream C §5.4 partial |
| age public key (operator's pubkey) | Encrypted backup envelope | Stream C §5.4 |

Everything else (dashboards, runbooks, E2E, frontend pages, Vault bootstrap, worker code, nginx) is implementable with no external credentials.

---

## 7. Execution order

1. **Runtime first.** Stream A lit — gives every downstream component real data to work against (dashboards without data are meaningless).
2. **S7 backbone in parallel with S8 instrumentation.** Frontend pages (no deps) + Grafana JSON (no deps) can land immediately. Vault + CF Tunnel + Alertmanager webhooks wait on operator creds.
3. **E2E + backups + runbooks last.** They validate the rest.

---

## 8. Risk register

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| Alchemy free tier throttles at mainnet pending-tx rate | medium | Monitor `arbx_rpc_rate_limited_total` (add metric); auto-backoff already in chain_client; document upgrade path to paid tier |
| CF Tunnel credential leaks | low | Stored in Vault, short-lived; tunnel has `TUNNEL_METRICS` localhost-only |
| Vault seal keys lost | critical | 3-of-5 shamir; operator keeps 3 offline, 2 in separate custodians; documented in `rotate-secrets` runbook |
| Grafana dashboard queries reference a metric we renamed | medium | Pre-commit check: grep every panel `expr:` against `arbx_*` symbol set; CI fails otherwise |
| Playwright flakiness gates merges | medium | Mark E2E as `non-blocking` on PRs for the first week; escalate to blocking after 5 consecutive green runs |
