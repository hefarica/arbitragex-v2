# Secrets Rotation Policy

**OMEGA-102 Delta Pack · 2026-05-16**
**Owner:** @hefarica
**Companion docs:** `INCIDENT_RUNBOOK.md` (INC-04: leaked credential), `REPO_VISIBILITY_POLICY.md`

## Principle

Every secret in the system has:

1. A **canonical store** (where the source-of-truth value lives)
2. A **rotation interval** (max time between deliberate rotations)
3. A **compromise trigger** (events that force rotation outside the schedule)
4. A **rotation procedure** (the exact steps to swap the value across all consumers)

If any of those four is missing, the secret is mis-managed and must be hardened before next deploy.

## Inventory

| Secret | Canonical store | Consumers | Rotation interval | Compromise trigger |
|---|---|---|---|---|
| `GITHUB_TOKEN` (operator PAT) | 1Password / local file off-repo | gh CLI, CI bootstraps, this script set | 90 days | Token visible in chat / logs / shared screen |
| `VPS_SSH_KEY` (private) | GH Actions secret `VPS_SSH_KEY` | `hardened-vps-deploy.yml`, manual SSH | 180 days | VPS compromise suspected, operator laptop loss |
| Signer private keys (executor) | TEE attestation envelope / hardware wallet | searcher-rs runtime | 365 days (rotate signing address) | Address ever appears in a leaked tx, mempool exposure incident |
| RPC provider API keys (Alchemy, Ankr, etc.) | GH Actions secrets + VPS env | searcher-rs, edge worker | 180 days | Provider notifies of compromise, rate-limit anomalies |
| Database credentials (PostgreSQL) | VPS env (`.env.crucible`, `.env.edge`) | api-server, control-plane | 90 days | DB exposed publicly (incident class INC-07) |
| Redis password | VPS env | api-server, searcher-rs | 90 days | Same as DB |
| `ANVIL_FORK_URL` (CI fork RPC) | GH Actions secret | foundry.yml, e2e.yml | 365 days | Provider notifies |
| Cosign keyless (OIDC) | GitHub-managed (no manual rotation) | `hardened-vps-deploy.yml` build stage | N/A — auto-rotated | Federated identity changes |
| Telegram bot token (alerts) | GH Actions secret | alert workflows | 365 days | Bot compromise |

## Rotation procedure (generic)

For each secret:

1. **Generate** the new value in the canonical store (provider console, hardware wallet, etc.).
2. **Stage** the new value alongside the old in all consumers (dual-write): GH Actions secrets support same-name overwrites; VPS env requires deploy.
3. **Verify** the new value works (smoke-test a workflow run or VPS endpoint).
4. **Revoke** the old value at the provider.
5. **Record** rotation in `audits/rotations.log` (timestamp, operator, secret name, reason).

NEVER rotate by direct swap (delete old → create new) — always dual-write first, verify, then revoke.

## 2026 calendar (default schedule)

Quarterly anchors, adjust to actual operator availability:

| Quarter | Rotations due |
|---|---|
| **Q2 2026 (Apr–Jun)** | DB creds · Redis password · operator PAT |
| **Q3 2026 (Jul–Sep)** | VPS_SSH_KEY · RPC provider keys |
| **Q4 2026 (Oct–Dec)** | DB creds · Redis password · operator PAT |
| **Q1 2027 (Jan–Mar)** | VPS_SSH_KEY · RPC provider keys · Signer private keys |
| **Q2 2027 (Apr–Jun)** | DB creds · Redis password · operator PAT · `ANVIL_FORK_URL` · Telegram bot token |

The 90-day cadence for ops-plane secrets (PAT, DB, Redis) sits inside the 180-day cadence for VPS/RPC, which sits inside the 365-day cadence for signing/oracle. Layered rotation reduces the window in which any single leaked secret remains exploitable.

## Compromise response — accelerated rotation

If any compromise trigger fires, rotate the affected secret within **2 hours** and audit downstream:

```bash
# Step 1 (within 5 minutes): revoke at provider, even before staging new value.
#   GH Actions secrets: gh secret remove <NAME>
#   GitHub PAT: revoke at https://github.com/settings/tokens
#   Signer key: trigger killswitch (docs/INCIDENT_RUNBOOK.md INC-01)
#
# Step 2 (within 30 minutes): generate replacement, dual-write to consumers.
# Step 3 (within 2 hours): verify, revoke, record.
# Step 4 (within 24 hours): audit logs for any usage of compromised value
#   between leak time and revocation.
```

If a leaked PAT is publicly visible (e.g., copied into a chat transcript, public PR comment, or screenshot), **assume already harvested**. Rotate within 5 minutes regardless of perceived blast radius.

## What this policy does NOT cover

- **Operator wallet seed phrases** — never stored digitally, live on hardware wallets, rotated by full migration only (multi-month process).
- **TEE attestation root keys** — managed by the cloud / hardware vendor; rotation is N/A.
- **End-user PII** (post OMEGA-8/M3) — covered by `docs/policies/PII_POLICY.md` if it exists; otherwise refer to GDPR/CCPA baselines.

Open items: this policy currently has no automated rotation. All rotations are operator-driven. A future improvement (out of scope for OMEGA-102 delta) is to wire rotations into a scheduled GH Actions workflow that fires reminders + creates issues against this repo.
