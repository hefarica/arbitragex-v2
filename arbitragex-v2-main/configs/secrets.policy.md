# Secrets Policy — ArbitrageX v2

Governs classification, storage, transport, rotation and revocation of every secret
in the ArbitrageX v2 platform. Binding for all services and operators.

## 1. Classification

| Tier | Examples | Blast radius if leaked |
|------|----------|-------------------------|
| **T0 — Execution critical** | `FLASHBOTS_SIGNER_KEY`, operator private keys, DB `arbx_migrator` password | Direct capital loss or full DB corruption |
| **T1 — Access critical**    | `ARBX_ADMIN_TOKEN`, `JWT_SECRET`, DB `arbx_rw` password, Grafana admin password | Remote admin takeover, config tampering, kill-switch bypass |
| **T2 — API critical**       | `ARBX_EDGE_TOKEN`, `GOPLUS_API_KEY`, `TENDERLY_API_KEY`, RPC provider keys | Service degradation, quota exhaustion, privacy leak |
| **T3 — Low-risk**           | Service-internal tokens with narrow scope, Grafana read-only user | Minor information disclosure |

## 2. Storage — by environment

| Environment | T0 / T1 | T2 / T3 | Source of truth |
|-------------|---------|---------|-----------------|
| **development** (laptop) | `.env` (gitignored) | `.env` | File, dev-only placeholders |
| **staging** | `docker secret` + envFile outside repo | env vars | Docker secret store |
| **production** (S1 interim) | `docker secret` | env vars | Docker secret store |
| **production** (S7 target) | Vault / 1Password Connect with short-lived tokens | Vault | Vault |

**Never**:
- Commit a non-example `.env` to git (enforced by `.gitignore` + `gitleaks` pre-push hook).
- Paste secrets in chat, email, issue comments, or logs.
- Embed secrets in source code, Dockerfiles, config files, or CI variables exposed to PRs.
- Log secret values. Loaders MUST redact before logging (`*****` with length + hash prefix).

## 3. Transport

- Only over TLS in-flight (internal docker network is a weak boundary; prefer mTLS when crossing hosts).
- Never in URL query strings.
- Service-to-service auth: `X-ArbX-*-Token` headers over TLS.

## 4. Validation at boot (fail-closed)

Every service MUST:
1. Load config at startup via `shared-rs` (Rust) or `shared-ts` (TS) loader.
2. Validate presence, format and length of every required secret.
3. Exit with non-zero status and structured log (`{"event":"config.boot","result":"fail","missing":["ARBX_ADMIN_TOKEN"]}`) if validation fails.
4. Never fall back to default / hardcoded values for secrets.

## 5. Rotation

| Secret | Cadence | Triggers immediate rotation |
|---|---|---|
| T0 signer keys | Before first real-capital test; then on incident | Suspected exposure; operator offboarding |
| T0 DB migrator password | Every 90 days | Any migration failure investigation involving shell access |
| T1 admin tokens | Every 30 days + per deploy | Compromise, staff change, leaked to chat/logs |
| T1 DB rw password | Every 90 days | Same as above |
| T1 JWT secret | Every 90 days | Invalidates all sessions — schedule off-peak |
| T2 API keys | Every 180 days | Suspected abuse / anomaly in quotas |

## 6. Revocation

On suspected compromise:
1. Toggle the kill-switch ON via `POST /admin/killswitch {"enabled":true,"reason":"suspected_secret_leak"}`.
2. Invalidate the secret in its source of truth (Vault / docker secret).
3. Rotate the secret per Section 5.
4. Grep logs (Loki) for the prefix/hash signature of the old secret.
5. Open an `incident_log` row; link related `risk_events`.
6. Postmortem within 72 hours; update this policy if needed.

## 7. Audit

- Every admin action that reads or mutates a secret MUST write to `audit_log` with `actor`, `action`, `before_state` (redacted), `after_state` (redacted), `ip_address`, `trace_id`.
- `audit_log` is append-only (DELETE/UPDATE revoked from `arbx_rw` role — see migration 011).
- Alertmanager alerts on `increase(audit_log_rows{action="killswitch.enable"}[5m]) > 0`.

## 8. Responsibilities

- **Operators** rotate T0 and T1 secrets, respond to incidents.
- **Developers** never see prod T0 secrets; request ephemeral creds via Vault or sandboxed env.
- **CI/CD** uses its own T1 tokens with least-privilege scopes; not reused across pipelines.
