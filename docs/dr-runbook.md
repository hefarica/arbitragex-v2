# ArbitrageX v2 — Disaster Recovery Runbook

## Document Control

| Field | Value |
|---|---|
| Version | 1.0.0 |
| Status | ACTIVE |
| Last Reviewed | 2026-05-17 |
| Owner | SRE / Platform Team |
| Related | `docs/operations/VAULT_SETUP.md`, `docs/operations/SECRETS_POLICY.md`, `scripts/vault-operator-init.sh`, `scripts/dr-drill.sh` |

---

## Table of Contents

1. [Service Dependency Map](#1-service-dependency-map)
2. [Escalation Procedures](#2-escalation-procedures)
3. [Kill-switch Activation](#3-kill-switch-activation)
4. [Database Recovery Procedure](#4-database-recovery-procedure)
5. [Vault Recovery Procedure](#5-vault-recovery-procedure)
6. [Communication Template](#6-communication-template)
7. [DR Drill Schedule](#7-dr-drill-schedule)
8. [Post-Incident Review](#8-post-incident-review)

---

## 1. Service Dependency Map

### Boot Sequence

The ArbitrageX v2 platform has a strict initialization order. Services at layer N must be healthy before layer N+1 starts.

```
Layer 0 — Infrastructure (Docker, TLS certs, Vault)
  └─ vault ............................. TLS key: monitoring/vault/tls/
  └─ minio ............................. Object storage for long-term metrics

Layer 1 — Data Plane
  └─ postgres .......................... Application state, audit logs
  └─ redis ............................. Caching, session state, job queues

Layer 2 — Observability
  ├─ loki .............................. Log aggregation
  ├─ prometheus ........................ Metrics collection
  ├─ alertmanager ...................... Alert routing (PagerDuty, Slack)
  ├─ grafana ........................... Visualization dashboards
  ├─ thanos-sidecar .................... Prometheus long-term storage shipper
  ├─ thanos-store ...................... Historical metrics (needs /data writable, UID 1001)
  └─ thanos-query ...................... Unified query across time ranges

Layer 3 — Application Core
  ├─ api-server ........................ REST API, admin endpoints
  ├─ edge .............................. Public entry point (behind Cloudflare)
  ├─ searcher-rs ....................... Block scanning, opportunity detection
  ├─ executor-rs ....................... Transaction execution engine
  ├─ settlement-rs ..................... Settlement tracking
  ├─ reconciler-rs ..................... Balance reconciliation
  ├─ monitoring ........................ Health reporter
  └─ vault-agent ....................... Secret template renderer

Layer 4 — Frontend (optional)
  └─ (any frontend containers) ......... UI layer
```

### Critical Path

The shortest path to restoring trading capability is:

```
Vault → Postgres → Redis → api-server → edge → searcher-rs → executor-rs
```

### Failure Impact Matrix

| Service | Failure Impact | Max Downtime Tolerance | Recovery Priority |
|---------|--------------|----------------------|-------------------|
| Vault | All secrets unreadable. Full platform halt. | 0 min (seal = halt) | P0 |
| Postgres | No trades, no audit trail. | 5 min | P0 |
| Redis | Cache miss fallback to DB. Slower but functional. | 15 min | P1 |
| Edge | Public API unreachable. | 2 min | P0 |
| executor-rs | No trade execution. Detection still works. | 10 min | P1 |
| searcher-rs | No opportunity detection. Execution still works on existing signals. | 10 min | P1 |
| Prometheus / Grafana | Blind flying. No alerting. | 30 min | P2 |
| Thanos Store | Loss of historical metrics. Current metrics unaffected. | 60 min | P2 |
| Alertmanager | No PagerDuty/Slack alerts. Blind flying. | 30 min | P2 |

---

## 2. Escalation Procedures

### Severity Levels

| Level | Description | Response Time | Who |
|-------|------------|---------------|-----|
| SEV-0 | Complete platform down. No trades. Funds at risk. | 5 min | On-call SRE → Platform Lead → CTO |
| SEV-1 | Partial degradation. Kill-switch activated. Paper mode only. | 15 min | On-call SRE → Platform Lead |
| SEV-2 | Single service failure. No user impact. | 30 min | On-call SRE |
| SEV-3 | Non-critical service degraded. No production impact. | 2h | Next business day |

### Escalation Chain

```
1. PagerDuty alert fires → On-call SRE receives notification (5 min)
2. SRE acknowledges via PagerDuty or Slack (#alerts)
3. SEV-0 / SEV-1:
   a. SRE attempts standard recovery (see §4–§5)
   b. If unresolved in 15 min → escalate to Platform Lead
   c. If unresolved in 30 min → escalate to CTO
4. SEV-2:
   a. SRE creates incident ticket
   b. Attempt recovery during business hours
   c. Escalate if exceeds 2 hours
5. All incidents:
   a. Open #war-room Slack channel
   b. Post initial situation report (template: §6)
   c. Update every 15 min until resolved
```

### On-Call Checklist

- [ ] Acknowledge PagerDuty alert
- [ ] Open #war-room channel in Slack
- [ ] Determine SEV level
- [ ] Post initial situation report
- [ ] Execute relevant recovery procedure (§4 or §5)
- [ ] Post resolution or escalation update every 15 min
- [ ] Schedule post-incident review within 24h (SEV-0/1) or 72h (SEV-2)

---

## 3. Kill-switch Activation

### Purpose

The kill-switch is a circuit breaker that immediately halts all trading activity across the platform. When activated:

- All outbound transactions are blocked
- Searcher stops broadcasting opportunities
- Executor rejects new execution requests
- Existing positions are settled but no new positions opened

### When to Activate

- **SEV-0 incident** involving funds at risk
- **Suspected compromise** of signing keys or admin credentials
- **Critical smart contract vulnerability** discovered in production
- **Regulatory freeze** order
- **Operator decision** during uncontrolled loss event

### Activation Steps

```bash
# 1. Verify current state
export ARBX_ADMIN_TOKEN="<your-admin-token>"
curl -s -H "Authorization: Bearer $ARBX_ADMIN_TOKEN" \
  https://<edge-endpoint>/admin/killswitch/status

# 2. Activate kill-switch
response=$(curl -s -X POST \
  -H "Authorization: Bearer $ARBX_ADMIN_TOKEN" \
  https://<edge-endpoint>/admin/killswitch)
echo "$response" | jq .

# Expected response:
# {
#   "kill_switch": "ACTIVATED",
#   "timestamp": "2026-05-17T00:00:00Z",
#   "activated_by": "<operator-id>",
#   "reason": "manual_sev0"
# }
```

### Deactivation Steps

> **WARNING:** Kill-switch deactivation requires dual-authorization for SEV-0 incidents. Both On-call SRE AND Platform Lead must approve.

```bash
# 1. Verify incident is resolved
# 2. Obtain dual-authorization (SEV-0 only)
# 3. Deactivate
curl -s -X POST \
  -H "Authorization: Bearer $ARBX_ADMIN_TOKEN" \
  https://<edge-endpoint>/admin/killswitch/deactivate

# 4. Verify deactivation
curl -s -H "Authorization: Bearer $ARBX_ADMIN_TOKEN" \
  https://<edge-endpoint>/admin/killswitch/status
```

### Kill-switch Test (Non-Production)

Use the DR drill script for safe testing:

```bash
# Test kill-switch auth (dry-run, no actual toggle)
./scripts/dr-drill.sh --test killswitch

# Full DR drill (all tests, dry-run)
./scripts/dr-drill.sh
```

---

## 4. Database Recovery Procedure

### 4A — Routine Backup (Automated)

Backups run automatically via cron on the VPS:

```bash
# Backup schedule (crontab -l as root on VPS)
# 0 */4 * * * /opt/arbitragex-v2/scripts/backup-postgres.sh
# 0 */6 * * * /opt/arbitragex-v2/scripts/backup-redis.sh
```

Backup locations:
- PostgreSQL: `/backups/arbitragex/postgres/YYYY-MM-DD_HH-MM.sql.gz`
- Redis: `/backups/arbitragex/redis/YYYY-MM-DD_HH-MM.rdb`
- MinIO: `/backups/arbitragex/minio/buckets/`

### 4B — Point-in-Time Recovery

```bash
# 1. Stop dependent services
docker compose -f docker/compose.prod.yml stop edge api-server searcher-rs executor-rs

# 2. Stop PostgreSQL
docker compose -f docker/compose.prod.yml stop postgres

# 3. Mount backup volume and restore
docker run --rm \
  -v /backups/arbitragex/postgres/:/backups:ro \
  -v arbitragex-v2_postgres_data:/var/lib/postgresql/data \
  postgres:15 bash -c \
  "gunzip -c /backups/TARGET_BACKUP.sql.gz | psql -U postgres -d arbitragex"

# 4. Start PostgreSQL and verify
docker compose -f docker/compose.prod.yml up -d postgres
docker compose -f docker/compose.prod.yml exec postgres pg_isready -U postgres

# 5. Start dependent services
docker compose -f docker/compose.prod.yml up -d edge api-server searcher-rs executor-rs
```

### 4C — Complete Database Rebuild

If PostgreSQL data volume is corrupted:

```bash
# 1. Kill-switch ON (§3)
# 2. Remove corrupted volume
docker compose -f docker/compose.prod.yml down postgres
docker volume rm arbitragex-v2_postgres_data

# 3. Recreate volume
docker compose -f docker/compose.prod.yml up -d postgres

# 4. Restore from latest backup
LATEST_BACKUP=$(ls -t /backups/arbitragex/postgres/*.sql.gz | head -1)
docker run --rm -i \
  -v "${LATEST_BACKUP}:/backup.sql.gz:ro" \
  -v arbitragex-v2_postgres_data:/var/lib/postgresql/data \
  postgres:15 bash -c \
  "gunzip -c /backup.sql.gz | psql -U postgres -d arbitragex"

# 5. Apply any migrations that ran after the backup
docker compose -f docker/compose.prod.yml run --rm api-server \
  ./migrate up

# 6. Verify
docker compose -f docker/compose.prod.yml exec postgres \
  psql -U postgres -d arbitragex -c "SELECT COUNT(*) FROM trades;"
```

### 4D — Redis Recovery

```bash
# Redis is a cache — the simplest recovery is wipe and rebuild:
docker compose -f docker/compose.prod.yml stop redis
docker volume rm arbitragex-v2_redis_data
docker compose -f docker/compose.prod.yml up -d redis

# If you need to restore from RDB backup:
docker run --rm \
  -v /backups/arbitragex/redis/TARGET.rdb:/data/dump.rdb:ro \
  -v arbitragex-v2_redis_data:/data \
  redis:7.2 redis-server --save "" --appendonly no
```

---

## 5. Vault Recovery Procedure

### 5A — Vault Sealed (Normal Restart)

After any Vault container restart, Vault is **sealed** by design. This is a security feature, not a failure.

**Symptom:** `vault status` shows `Sealed: true`. Health endpoint returns HTTP 503.

**Recovery:**

```bash
# Option 1: Use the init script (idempotent — will skip init if already done)
./scripts/vault-operator-init.sh --unseal-only

# Option 2: Manual unseal (if you have the key shards)
export VAULT_ADDR=https://127.0.0.1:8200
export VAULT_CACERT=/opt/arbitragex-v2/monitoring/vault/tls/vault-ca.pem

# Submit 2 of 3 key shards
docker exec -e VAULT_ADDR -e VAULT_CACERT -it arbitragex-v2-vault-1 \
  vault operator unseal <UNSEAL_KEY_1>
docker exec -e VAULT_ADDR -e VAULT_CACERT -it arbitragex-v2-vault-1 \
  vault operator unseal <UNSEAL_KEY_2>

# Verify
docker exec -e VAULT_ADDR -e VAULT_CACERT -it arbitragex-v2-vault-1 \
  vault status
```

### 5B — Vault Uninitialized (First Deploy)

**Symptom:** `vault status` shows HTTP 501 or `Initialized: false`.

**Recovery:**

```bash
# 1. Ensure TLS certs exist
ls -la monitoring/vault/tls/
# Expected: vault-cert.pem, vault-key.pem, vault-ca.pem

# 2. If missing, regenerate
bash monitoring/vault/generate-tls.sh

# 3. Initialize (ONE-TIME operation)
./scripts/vault-operator-init.sh

# 4. Save keys offline — they are displayed ONCE
# 5. Restart vault-agent to pick up unsealed Vault
docker compose -f docker/compose.prod.yml restart vault-agent
```

### 5C — Vault Data Loss (Corrupted Backend)

If the Vault file backend is corrupted:

```bash
# 1. Kill-switch ON (§3) — all secrets will be unavailable

# 2. Backup current state (for forensics)
cp -r /var/lib/docker/volumes/arbitragex-v2_vault_data/_data \
  /backups/arbitragex/vault/corrupted-$(date +%Y%m%d-%H%M%S)

# 3. Re-initialize (this creates NEW unseal keys — all old keys are invalid)
./scripts/vault-operator-init.sh

# 4. Re-populate secrets (from offline backup — see Secrets Policy)
export VAULT_TOKEN="<new-root-token>"

# Example: re-create secrets
vault kv put secret/arbitragex/flashbots_signer key="<value-from-offline-backup>"
vault kv put secret/arbitragex/admin_token token="<value-from-offline-backup>"
# ... repeat for all secrets

# 5. Restart all services that depend on Vault secrets
docker compose -f docker/compose.prod.yml restart vault-agent api-server edge
```

### 5D — TLS Certificate Expiry

Certificates generated by `generate-tls.sh` are valid for 365 days.

**Before expiry (30-day warning):**

```bash
# 1. Check expiry
docker exec arbitragex-v2-vault-1 \
  openssl x509 -in /vault/tls/vault-cert.pem -noout -dates

# 2. Rotate
cd /opt/arbitragex-v2
mv monitoring/vault/tls/vault-{cert,key,ca}.pem /backups/tls/old/
bash monitoring/vault/generate-tls.sh
docker compose -f docker/compose.prod.yml restart vault

# 3. Unseal (Vault re-seals on restart)
./scripts/vault-operator-init.sh --unseal-only
```

---

## 6. Communication Template

### Situation Report (SitRep) — Initial

```
**INCIDENT — ArbitrageX v2 — [SEV-X]**
**Time:** 2026-05-17 00:00 UTC
**On-call:** @sre-oncall
**Channel:** #war-room

**Summary:** [One-sentence description]

**Impact:**
- Trading: [halted / paper-only / degraded]
- Services affected: [list]
- Estimated exposure: [USD at risk / none]

**Current Status:**
- Kill-switch: [OFF / ON / N/A]
- Paper mode: [ON / OFF]
- Funds at risk: [YES — $X / NO]

**Actions Taken:**
- [ ] Kill-switch activated
- [ ] PagerDuty acknowledged
- [ ] #war-room opened
- [ ] [Other actions]

**Next Update:** +15 min (00:15 UTC)
```

### Situation Report — Update

```
**UPDATE — [SEV-X] — +[N] min**
**Time:** 2026-05-17 00:15 UTC

**What changed:** [new information, progress, setbacks]

**Current state:**
- Root cause: [identified / investigating / unknown]
- Recovery ETA: [time estimate or "unknown"]
- Services restored: [list]
- Services still down: [list]

**Next Update:** +15 min (00:30 UTC)
```

### All-Clear / Resolution

```
**RESOLVED — [SEV-X]**
**Time:** 2026-05-17 01:00 UTC
**Duration:** 60 min

**Resolution:** [What fixed it]
**Root Cause:** [5 Whys summary]
**Post-Incident Review:** Scheduled for [date/time]
**Action Items:**
- [ ] [Follow-up item 1 — owner — due date]
- [ ] [Follow-up item 2 — owner — due date]
```

---

## 7. DR Drill Schedule

### Frequency

| Drill Type | Frequency | Participants |
|-----------|-----------|-------------|
| Full DR drill (all tests) | Monthly | SRE team |
| Kill-switch test | Weekly | On-call SRE |
| Vault unseal drill | Bi-weekly | On-call SRE |
| Postgres recovery test | Quarterly | SRE + Platform Lead |
| Vault data-loss recovery | Quarterly | SRE + Platform Lead |

### Running the Drill

```bash
# Full dry-run drill (safe, non-destructive)
cd /opt/arbitragex-v2
./scripts/dr-drill.sh

# Live drill (performs actual toggles — use with caution)
./scripts/dr-drill.sh --live

# Specific test only
./scripts/dr-drill.sh --test containers
./scripts/dr-drill.sh --test killswitch
./scripts/dr-drill.sh --test backups
./scripts/dr-drill.sh --test recovery
./scripts/dr-drill.sh --test rollback
```

### Drill Output

The drill generates a structured report to stdout. Capture and archive:

```bash
./scripts/dr-drill.sh 2>&1 | tee /var/log/arbitragex/dr-drill-$(date +%Y%m%d-%H%M%S).log
```

### Acceptance Criteria

A successful DR drill meets ALL of the following:

- [ ] All 21 containers running
- [ ] Kill-switch returns 401 without token
- [ ] Kill-switch toggles with valid token
- [ ] Paper mode is ON in System Guard
- [ ] pg_dump completes successfully
- [ ] Redis PING/BSAVE responds
- [ ] MinIO health endpoint responding
- [ ] All health endpoints (Vault, Grafana, Prometheus, Alertmanager, Edge) respond
- [ ] Rollback procedure documented (dry-run)
- [ ] Zero FAILED results in drill report

---

## 8. Post-Incident Review

### Template

Within 24 hours of SEV-0/1 resolution:

```markdown
# Post-Incident Review: [INCIDENT-ID]

## Timeline
| Time (UTC) | Event |
|-----------|-------|
| 00:00 | Alert fired |
| 00:05 | SRE acknowledged |
| 00:10 | Kill-switch activated |
| 00:30 | Root cause identified |
| 00:45 | Fix applied |
| 01:00 | All-clear declared |

## Root Cause (5 Whys)
1. Why did the outage occur? → [Answer]
2. Why did [Answer] happen? → [Answer]
3. Why did [Answer] happen? → [Answer]
4. Why did [Answer] happen? → [Answer]
5. Why did [Answer] happen? → [Root cause]

## What Went Well
- [Item]

## What Went Wrong
- [Item]

## Action Items
| # | Action | Owner | Due Date | Status |
|---|--------|-------|----------|--------|
| 1 | [Item] | [Owner] | [Date] | [Status] |

## Follow-Up
- [ ] Update runbook if procedures changed
- [ ] Update monitoring/alerting if detection was late
- [ ] Update automated tests if regression is possible
```

---

## Quick Reference Card

Print and keep at your workstation:

```
┌─────────────────────────────────────────────────────────────────┐
│  ARBITRAGEX v2 — EMERGENCY RESPONSE                            │
├─────────────────────────────────────────────────────────────────┤
│  Kill-switch:  POST /admin/killswitch (needs ARBX_ADMIN_TOKEN) │
│  Vault init:   ./scripts/vault-operator-init.sh                │
│  DR drill:     ./scripts/dr-drill.sh                           │
│  DB backup:    pg_dump -h 127.0.0.1 -U postgres -d arbitragex │
│  DB restore:   See §4B (gunzip + psql)                         │
│  Vault unseal: ./scripts/vault-operator-init.sh --unseal-only  │
│  All services: docker compose -f docker/compose.prod.yml ps    │
│  Restart all:  docker compose -f docker/compose.prod.yml up -d │
├─────────────────────────────────────────────────────────────────┤
│  Escalation:  PagerDuty → Platform Lead (15 min) → CTO (30m)  │
│  War room:    #war-room Slack channel                          │
│  SitRep:      Use template from §6                             │
└─────────────────────────────────────────────────────────────────┘
```

---

*End of Runbook — ArbitrageX v2 Disaster Recovery Procedures (PR #97)*
