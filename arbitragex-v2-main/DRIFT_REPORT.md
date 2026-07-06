# DRIFT_REPORT — ArbitrageX v2 (Phase 0)

> **INTERNAL — do not push as-is** (references leaked infra). Generated 2026-06-29, read-only. Companion to `MASTER_AUDIT.md`. Provenance caveat applies: audited the local working tree, which may be behind `main` (memory claims PR #197 reconciled specs, PR #213 added mainnet lockout — found otherwise here; reconcile against live `main`).

Four cross-surface diffs were run by dedicated read-only auditors. **All four found drift.** Code is the source of truth; docs/specs lose until corrected (HARD RULE).

---

## DRIFT-A · Specs (OpenAPI/AsyncAPI) ↔ runtime (api-server + Socket.IO) — **~12/100, actively misleading**

The specs are not "doc lag" — they describe a **largely fictional API**.

| # | Sev | Finding | Spec side (A) | Runtime side (B) |
|---|-----|---------|---------------|------------------|
| A1 | critical | Every concrete REST path is fictional **except `/health`**. `/ready`,`/live`,`/api/config`,`/api/system/guard-state`,`/api/opportunities`,`/api/executions` don't exist; ~50 real `/api/v1/*` + `/admin/*` routes undocumented. | openapi.yaml:61,79,97,111-143,148,184 | grep of backend → no matches; real routes in index.ts + 50 files under routes/ |
| A2 | critical | AsyncAPI models **raw `ws://host:8081` path-channels**; runtime is **Socket.IO rooms** on the shared :8080 server with different event names. A consumer coding to the spec receives nothing. | asyncapi.yaml:13,24-26,60-62,93-95 | websocket.ts:211 (socket.io), rooms/events :257,348,576,674 |
| A3 | high | Admin auth header drift: spec `x-admin-token` vs runtime `x-arbx-admin-token`. Spec-built clients get 401 on every admin call. | openapi.yaml:291-295; asyncapi.yaml:125-130 | shared-ts/middleware/index.ts:124-133; websocket.ts:157-181 |
| A4 | high | **Kill-switch** (most safety-critical endpoint) mis-specified: spec body `{active,reason,durationMs}` vs runtime Zod `{enabled,reason?,triggered_by?}`. Spec-conformant kill call fails validation (400). | openapi.yaml:508-523 | index.ts:202-231 |
| A5 | high | AsyncAPI omits the entire real WS surface (convergence, cartridge telemetry, route-discovery, runtime_ack) and includes an `executions` channel with no producer. | asyncapi.yaml:60-92 | websocket.ts:348,409,576,674 |
| A6 | medium | No spec generation / drift-detection pipeline (no codegen, Spectral, oasdiff, contract test, CI ref) → drift is structurally undetectable and growing. | no .github match; referenced only by apps/frontend/README.md | not served by any route |
| A7 | medium | Server URLs leak the prod VPS IP + present unverified TLS hosts. | openapi.yaml:18-19; asyncapi.yaml:17 | (RULE 02: REST→edge:8787, WS→api-server:8080, never :8081) |

---

## DRIFT-B · Frontend ↔ API/Edge — **~45/100; happy path OK, control-plane dead**

The dashboard happy-path (opportunities, readiness, recon, operations, status, risk, killswitch, chains, scoring) **is wired correctly through both edges** with Zod-validated honest-failure clients. The drift is concentrated in the operator/control-plane and prod-vs-dev edge.

| # | Sev | Finding | FE side (A) | Edge/API side (B) |
|---|-----|---------|-------------|-------------------|
| B1 | high | **`/api/operator/me` is dead in BOTH edges** but consumed by a live page → `OperatorGate` never opens. | lib/operator/useOperator.ts:29 → OperatorGate.tsx → omega-s5/registry | edge dev-local:338-339 + worker:384-385 proxy only credentials/status/selftest; no `/api/operator/me` |
| B2 | high | **Prod data-dead**: a class of live FE routes is served by **dev-local Express but MISSING from the prod CF worker** (route-discovery/*, cartridges/status+telemetry, sed/status, paper/history, relays, chains, rpcs). These pages silently die in prod (the doctrinal prod edge is the worker). | app/routes/discovery, useRouteDiscoveryRest.ts:65,114; app/paper/history | edge/worker/src/index.ts route list has none of them |
| B3 | high | **Dev/prod edge divergence is the root cause** of B1/B2: dev-local (~90 routes) is a strict superset of the prod worker (~70). Anything validated locally may be dead in prod. | dev-local header claims "Mirrors the worker"; registers ~90 routes | worker registers ~70; missing route-discovery/*, cartridges/*, sed |
| B4 | medium | **Aspirational/dead APEX stack**: `frontend/lib/apex/` is a second never-wired client w/ a different auth model (`X-Operator-Token`/`X-Operator-Role` + sovereign_signature) calling `/api/v1/chains`, `/admin/chains/:id/toggle` that no edge serves. | lib/apex/api/client.ts:11-14,113; useChains.ts:127,178 | no edge route for `/api/v1/chains` or `/toggle` |
| B5 | low | Built-but-orphaned mutations: api-server `POST /api/operator/preferences` + `/feature-overrides` have full PG+audit txns but no FE caller and no edge proxy. | grep frontend/ → empty | operator.ts:77,154 not proxied |

---

## DRIFT-C · DB schema ↔ endpoints/UI — **phantom tables on the mainnet-promotion path**

72 `CREATE TABLE`'d tables cross-referenced against all consumers (api-server TS, edge, FE, Rust services).

| # | Sev | Finding | Schema/code side |
|---|-----|---------|------------------|
| C1 | **critical** | **Phantom table gating mainnet promotion**: `crucible_runs` has **no migration and no writer**, yet is the data source for the chain-qualification safety gate read by the sovereign promote path. | admin-promote-mainnet.ts:44 (`FROM crucible_runs`, no 42P01 guard); no `CREATE TABLE crucible_runs` anywhere |
| C2 | high | **Phantom column** in the promotion write: `INSERT INTO chains_runtime(chain_id, mode, …)` but mig 061 has no `mode` column (uses `enabled BOOLEAN`); no later migration adds it → the write fails. | admin-promote-mainnet.ts:166-174 vs mig 061:19-35 |
| C3 | high | **Orphan table proves price-validator not productized**: mig 098 `validator_divergences` (+GRANTs) has **zero consumers**; the crate has no sqlx/PgPool/INSERT. | 098_price_validator.sql:7-26 vs backend/price-validator/src/* (no DB code) |
| C4 | medium | Orphan/superseded tables (zero live readers/writers): `kill_switch_audit` (012, superseded by audit_event), `routes`/`route_legs`/`routers` (021/022, superseded by contract_registry), `risk_policy`/`execution_policy`/`scoring_weights`/`profit_reconciliation`/`pool_ticks`/`operator_session_state`/`cartridge_metrics_hourly`/`sed_entropy_metrics`. | migs 012/014/015/016/022/024/068/090 vs zero SQL refs in backend |
| C5 | low | Phantom table in a **different datastore** (Cloudflare D1): worker `INSERT INTO edge_telemetry` but no schema migration creates it and `database_id='REPLACE_ME_D1_ID'`. | edge/worker/src/index.ts:231-240; wrangler.toml:47-50 |

---

## DRIFT-D · Env ↔ compose ↔ docs — **a committed LIVE config + repo-wide infra leak**

> Path note: the real repo lives at `…/arbitragex-v2-main/arbitragex-v2-main/`; the outer `(17)/.git` tracks only ~10 files, so `git ls-files` is misleading — the working tree was audited directly.

| # | Sev | Finding | A | B |
|---|-----|---------|---|---|
| D1 | **critical** | **Committed `.env.edge` is a LIVE go-live config** contradicting `.env.example`'s paper/observer doctrine: `PAPER_MODE=false`, live private-key/treasury slots, "Capital expuesto controlado > 0", "NUNCA commitear". | .env.example:83-85,99-101,166-168,185 (DEFAULT-OFF, no signer/key/broadcast) | .env.edge:5-6,104-118,134,138-139 |
| D2 | **critical** | **Prod VPS IP `[REDACTED-VPS-IP]` leaked in 60+ versioned files** (operator/deploy docs, GH secret refs, specs); internal host `edge-arbx.ape-tv.net` in 29 files. Public repo. | docs/OPERATOR_RUNBOOK.md:22-27; docs/how-to/deploy-to-vps.md:43,173-174,213,315 | docker/compose.dev.yml:303; docker-compose.edge.yml; openapi.yaml:18 |
| D3 | high | `docs/reference/env-vars.md` (the "canonical" reference) is **almost entirely fictional** — documents an `AX_*` namespace (`AX_MODE`, `AX_PRIVATE_KEY`, `AX_RPC_PRIMARY`, `POSTGRES_USER=ax_*`) that exists nowhere in runtime. | docs/reference/env-vars.md:11,34,53-54,93-94 | .env.example:12,21,33,49,185 (real names) |
| D4 | high | Required prod vars **missing from `.env.example`**, so the doctrinal `cp .env.example .env` path fails-fast on first prod boot: `ENRICHER_CHAINS`, `MINIO_ROOT_USER/PASSWORD` (`${VAR:?required}`), `GITHUB_TOKEN`. | .env.example (absent) | compose.prod.yml:488,490,599-600 |
| D5 | medium | Staging/override env families add vars present in **no** template (`ARBX_ENV`, `ARBX_EXECUTION_ENABLED`, `ARBX_TOPOLOGY_*`, `ARBX_MEMPOOL_MODE`, `PAPER_MODE`, `WALLET_SIGNING_ENABLED`, `NEXT_PUBLIC_ARB_ENV`). | .env.example absent | config/topology/staging.env.example:5-25; compose.staging.override.yml:13-31 |
| D6 | medium | Doc-vs-doctrine port/URL contradictions: `SIM_SIGNER_ADDRESS` is "crash-on-boot if missing" (CLAUDE.md RULE 02) yet absent from `.env.example`; `frontend/.env.example` `NEXT_PUBLIC_WS_URL=http://localhost:3000` vs doctrine WS→8080. | CLAUDE.md:62; frontend/.env.example:19 | docs/* (WS=8080) |
| D7 | low | 11 `*_PORT` vars in `.env.example` are dead (every compose service hardcodes its host port). | .env.example:17-27 | compose.*.yml literal port maps |

---

## Drift remediation priority (feeds IMPLEMENTATION_PLAN Phase 1/4)

1. **D1 + D2 + C1/C2** — safety/secrets first: remove/quarantine `.env.edge`, repo-wide IP/host scrub, fix or gate the phantom mainnet-promotion path (it must not be reachable while pointing at non-existent schema).
2. **B1/B2/B3** — unify the prod CF worker with the dev-local route surface (or stop the FE depending on dev-only routes) and wire `operatorIdentityMiddleware`, before the operator console is trusted live.
3. **A1–A5** — regenerate OpenAPI/AsyncAPI from the runtime (or delete them) + add a blocking drift gate; do not ship a spec that lies about the kill-switch.
4. **D3/D4/D5** — make `.env.example` the true superset; delete the fictional `AX_*` reference doc.
5. **C3** — close when price-validator persistence lands (its own phase).
