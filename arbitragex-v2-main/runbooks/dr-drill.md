---
id: R-DR-001
title: Monthly Disaster Recovery Drill
severity: P2
duration: 45m
owner: SRE On-Call + DR Lead
reviewed: 2025-01-15
---

# DR Drill Runbook

## Purpose

Verify the ArbitrageX V2 system can recover from infrastructure failure,
data loss, or region outage within the defined Recovery Time Objective (RTO)
of 15 minutes and Recovery Point Objective (RPO) of 5 minutes.

## Schedule

| Frequency | When | Participants |
|-----------|------|--------------|
| Monthly | 3rd Tuesday, 09:00 UTC | SRE On-Call, DR Lead, Platform Eng |
| Quarterly | Full DR failover test | All engineering |

## Preconditions

1. Maintenance window approved and announced in #maintenance.
2. Kill-switch is functional (pre-flight check passed).
3. Backup systems report healthy status.
4. A "drill" tag is applied to all metrics/alerts to suppress SEV-0 pages.
5. Incident commander (DR Lead) is designated.

---

## Step 1 — Pre-Flight Checklist (5 min)

Before starting the drill, confirm:

- [ ] All production services report `status: healthy` on `/health`.
- [ ] Latest database backup is within 5 minutes (check backup dashboard).
- [ ] Vault is unsealed and accessible.
- [ ] Kill-switch token is valid (quick `GET /admin/killswitch` smoke test).
- [ ] DR environment is provisioned and idle (no stray processes).
- [ ] #dr-drill Slack channel is active for real-time logging.

```bash
# Quick health check across all endpoints
for url in https://api.arbitragex-v2.io/health \
           https://api.arbitragex-v2.io/ready \
           https://api.arbitragex-v2.io/live; do
  echo "=== ${url} ==="
  curl -s -o /dev/null -w "%{http_code} %{time_total}s\n" "${url}"
done
```

## Step 2 — Container Census (10 min)

Document all running containers before the drill:

```bash
# Full container census
kubectl get pods -n arbitragex-prod -o wide > /tmp/dr-census-$(date +%Y%m%d).txt
kubectl get deployments -n arbitragex-prod -o yaml > /tmp/dr-deployments-$(date +%Y%m%d).yaml
kubectl get services -n arbitragex-prod -o yaml > /tmp/dr-services-$(date +%Y%m%d).yaml

# Capture image versions
kubectl get pods -n arbitragex-prod -o json | \
  jq -r '.items[] | "\(.metadata.name): \(.spec.containers[].image)"' | sort
```

Store these artifacts in the DR audit trail (S3: `s3://arbitragex-dr/audits/`).

### Census Checklist

- [ ] `arbitragex-api` — deployment count, image tag, restart count
- [ ] `arbitragex-engine` — pod count, queue depth, opportunity throughput
- [ ] `arbitragex-executor` — nonce state, pending tx count
- [ ] `arbitragex-risk` — guard state, exposure limits
- [ ] `arbitragex-ws` — WebSocket connection count
- [ ] `redis` — memory usage, replication lag
- [ ] `postgres` — replication status, lag in seconds

## Step 3 — Kill-Switch Test (5 min)

Activate the kill-switch in **test mode** (does not affect production trading):

```bash
ADMIN_TOKEN=$(vault kv get -field=token secret/arbitragex/admin/prod)

# Activate
curl -X POST "https://api.arbitragex-v2.io/admin/killswitch" \
  -H "Content-Type: application/json" \
  -H "x-admin-token: ${ADMIN_TOKEN}" \
  -d '{
    "active": true,
    "reason": "DR drill kill-switch test — no production impact",
    "durationMs": 120000
  }'
```

Verify:

- [ ] HTTP 200 response with `"active": true`
- [ ] Guard state transitions to `open`
- [ ] No new execution submissions accepted
- [ ] WebSocket `system.killswitch.activated` event broadcast

```bash
# Verify state
curl -s -H "x-admin-token: ${ADMIN_TOKEN}" \
  https://api.arbitragex-v2.io/admin/killswitch | jq '.active'
```

### Deactivate after verification

```bash
curl -X POST "https://api.arbitragex-v2.io/admin/killswitch" \
  -H "Content-Type: application/json" \
  -H "x-admin-token: ${ADMIN_TOKEN}" \
  -d '{"active": false, "reason": "DR drill kill-switch test complete"}'
```

## Step 4 — Backup Verification (10 min)

### 4a — Database Backup Integrity

```bash
# Identify latest backup
LATEST=$(aws s3 ls s3://arbitragex-backups/postgres/ | sort | tail -1)
echo "Latest backup: ${LATEST}"

# Verify backup size is non-zero
aws s3 ls "s3://arbitragex-backups/postgres/${LATEST}" --human-readable

# (Quarterly only) Restore to DR instance and run checksum
# pg_restore --clean --if-exists --dbname=arbitragex_dr /tmp/latest.dump
# psql -d arbitragex_dr -c "SELECT COUNT(*) FROM executions;"
```

- [ ] Backup file size matches expected range
- [ ] Backup timestamp is within 5 minutes of current time
- [ ] (Quarterly) Restored row counts match production within 1%

### 4b — Configuration Backup

```bash
# Verify Kubernetes manifests are backed up
aws s3 ls s3://arbitragex-backups/k8s/ | tail -5

# Verify Vault export exists
vault kv get secret/arbitragex/admin/prod > /dev/null && echo "Vault accessible"
```

- [ ] K8s manifest backup exists and is within 24 hours
- [ ] Vault is accessible and secrets are readable

### 4c — Docker Image Availability

```bash
# Verify all production images are pullable from DR region
docker pull ghcr.io/arbitragex/api:$(cat /tmp/dr-census-$(date +%Y%m%d).txt | grep api | head -1 | awk '{print $2}')
```

- [ ] All critical images pull successfully from DR registry mirror

## Step 5 — Simulated Failover (10 min)

Simulate a regional outage by scaling down the primary API deployment:

```bash
# Scale down (simulated failure)
kubectl scale deployment arbitragex-api --replicas=0 -n arbitragex-prod

# Verify service is unreachable
curl -s -o /dev/null -w "%{http_code}" https://api.arbitragex-v2.io/health
# Expected: 503 or connection timeout
```

### Failover Checklist

- [ ] Load balancer health checks fail within 30 seconds
- [ ] DNS failover triggers (if configured)
- [ ] DR environment can be brought up within 15 minutes

### Recovery

```bash
# Scale back up (simulate recovery)
kubectl scale deployment arbitragex-api --replicas=3 -n arbitragex-prod
kubectl rollout status deployment/arbitragex-api -n arbitragex-prod --timeout=300s

# Verify recovery
curl -s https://api.arbitragex-v2.io/health | jq '.status'
```

- [ ] All pods reach `Ready` state within 5 minutes
- [ ] `/health` returns `healthy`
- [ ] `/ready` returns `ready`
- [ ] Opportunity feed resumes within 2 minutes

## Step 6 — Post-Drill Review (5 min)

Complete the DR drill scorecard:

| Metric | Target | Actual | Pass |
|--------|--------|--------|------|
| Kill-switch activation latency | < 10s | ___ | [ ] |
| Kill-switch deactivation latency | < 10s | ___ | [ ] |
| Backup age at start | < 5 min | ___ | [ ] |
| Backup restore time (quarterly) | < 15 min | ___ | [ ] |
| Service recovery time | < 5 min | ___ | [ ] |
| Full API recovery | < 15 min | ___ | [ ] |
| Zero data loss | RPO < 5 min | ___ | [ ] |

### Post-Drill Actions

- [ ] File scorecard in #dr-drill channel
- [ ] Create JIRA tickets for any failed metrics
- [ ] Update runbooks if issues were found
- [ ] Schedule follow-up if any metric failed

---

## Rollback Procedure

If the drill causes production impact:

1. **Immediately** activate the kill-switch ([Kill-Switch Runbook](kill-switch.md)).
2. Scale all deployments back to their pre-drill replica counts.
3. Restore from latest backup if data corruption is suspected.
4. Page the DR Lead and On-Call SRE.
5. Convert the drill into a live incident.

## Success Criteria

- All pre-flight checks pass before drill begins.
- Kill-switch activates and deactivates within 10 seconds each.
- Database backup is confirmed restorable (quarterly) or verified (monthly).
- Service recovery completes within the 15-minute RTO.
- Zero unplanned production impact during the drill.
- Scorecard is filed and any failures have follow-up tickets.

## Related Runbooks

- [Kill-Switch](kill-switch.md) — Emergency halt
- [Incident Response](incident-response.md) — If drill becomes live incident
- [Rotate Secrets](rotate-secrets.md) — Vault credential verification