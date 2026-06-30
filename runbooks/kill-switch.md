---
id: R-KS-001
title: Emergency Kill-Switch Activation/Deactivation
severity: P0
duration: 5m
owner: SRE On-Call
reviewed: 2025-01-15
---

# Kill-Switch Runbook

## Purpose

Immediately halt all trading operations in the ArbitrageX V2 system during
an emergency (critical bug, exploit detection, anomalous market conditions,
or infrastructure failure).

## Preconditions

1. You have `curl` or `httpie` installed on your workstation.
2. You possess a valid `x-admin-token` with kill-switch permissions.
3. Network connectivity to the target environment is confirmed.
4. You have access to the #incidents Slack channel.

---

## Activation Steps

### Step 1 — Assess Severity (0:00)

Determine whether the situation warrants a kill-switch. Criteria include:

- Funds at immediate risk
- Suspicious transaction patterns detected
- Critical smart-contract vulnerability reported
- Oracle manipulation or price-feed anomaly
- Infrastructure failure preventing safe execution

If in doubt, activate. **It is always safer to halt and investigate.**

### Step 2 — Notify Team (0:30)

Post in #incidents using the following template:

```
:rotating_light: KILL-SWITCH ACTIVATION IN PROGRESS
Reporter: @<your-handle>
Reason: <one-line description>
Impact: <affected chains / assets>
ETA: <expected resolution or "TBD">
```

### Step 3 — Identify Target Environment (0:45)

| Environment | URL | Admin Token Source |
|-------------|-----|---------------------|
| Production  | `https://api.arbitragex-v2.io` | `VAULT_PATH=secret/arbitragex/admin/prod` |
| Staging     | `http://<VPS_IP>:8080` | `VAULT_PATH=secret/arbitragex/admin/staging` |
| Local       | `http://localhost:8080` | `.env` file `ADMIN_TOKEN` |

### Step 4 — Execute Activation (1:00)

```bash
# Fetch admin token from Vault (production)
ADMIN_TOKEN=$(vault kv get -field=token secret/arbitragex/admin/prod)

# Activate kill-switch
curl -X POST "https://api.arbitragex-v2.io/admin/killswitch" \
  -H "Content-Type: application/json" \
  -H "x-admin-token: ${ADMIN_TOKEN}" \
  -d '{
    "active": true,
    "reason": "Suspicious oracle drift detected on ETH-USDC; halt all chains until review",
    "durationMs": 1800000
  }'
```

**Expected response (HTTP 200):**

```json
{
  "active": true,
  "changedAt": "2025-01-20T14:32:11Z",
  "changedBy": "sre-oncall",
  "reason": "Suspicious oracle drift detected on ETH-USDC; halt all chains until review",
  "autoResumeAt": "2025-01-20T15:02:11Z"
}
```

### Step 5 — Verify State (1:30)

```bash
curl -s "https://api.arbitragex-v2.io/admin/killswitch" \
  -H "x-admin-token: ${ADMIN_TOKEN}" | jq .
```

Confirm `"active": true`. If the response shows `"active": false`, retry
Step 4 once, then escalate to Platform Engineering.

### Step 6 — Confirm Halt (2:00)

Check the execution pipeline metrics:

```bash
# Verify zero pending/submitted executions
curl -s "https://api.arbitragex-v2.io/api/executions?status=pending&limit=1" \
  -H "x-admin-token: ${ADMIN_TOKEN}" | jq '.total'
```

Expected: `0`

### Step 7 — Communicate Confirmation (2:30)

Update the #incidents thread:

```
:white_check_mark: Kill-switch ACTIVATED at 2025-01-20T14:32:11Z
Auto-resume: 2025-01-20T15:02:11Z (30m) — will extend if needed
All trading operations HALTED. No pending executions.
Next update in 15 minutes or upon resolution.
```

---

## Deactivation Steps

### Step 8 — Confirm Resolution (variable)

Before deactivating, ALL of the following must be true:

- [ ] Root cause has been identified and documented
- [ ] Fix has been deployed or risk has been mitigated
- [ ] Monitoring shows normal system health for at least 10 minutes
- [ ] No anomalous execution patterns in the last 10 minutes
- [ ] A second SRE or engineering lead has reviewed the decision

### Step 9 — Execute Deactivation

```bash
curl -X POST "https://api.arbitragex-v2.io/admin/killswitch" \
  -H "Content-Type: application/json" \
  -H "x-admin-token: ${ADMIN_TOKEN}" \
  -d '{
    "active": false,
    "reason": "Oracle drift resolved; monitoring nominal. Resume trading."
  }'
```

### Step 10 — Verify Deactivation

```bash
curl -s "https://api.arbitragex-v2.io/admin/killswitch" \
  -H "x-admin-token: ${ADMIN_TOKEN}" | jq '.active'
```

Expected: `false`

### Step 11 — Monitor Recovery (0–5 min post-deactivation)

Watch the following for 5 minutes:

1. Guard state returns to `closed`
2. Opportunity feed resumes with normal patterns
3. Executions begin processing with normal gas usage
4. No spike in revert rates

```bash
# Poll guard state every 30s
watch -n 30 'curl -s https://api.arbitragex-v2.io/api/system/guard-state'
```

### Step 12 — Post-Incident Update

Update #incidents:

```
:white_check_mark: Kill-switch DEACTIVATED at <timestamp>
Resolution: <brief description>
Monitoring: Normal for 5 minutes post-resume
Status: RESOLVED
```

Schedule a post-mortem within 24 hours for any P0 activation.

---

## Rollback Procedure

If kill-switch activation causes unintended side effects:

1. **Immediate**: Re-activate the kill-switch (Step 4) to re-establish the safe state.
2. **Short-term**: Contact Platform Engineering to investigate state inconsistency.
3. **Document**: Capture logs, timestamps, and symptom descriptions before any restart.

## Success Criteria

- Kill-switch activation completes within 60 seconds of decision.
- Zero new trade submissions after activation is confirmed.
- Kill-switch deactivation only occurs after documented risk mitigation.
- Full trading recovery within 5 minutes of deactivation.
- All actions are logged in the incident tracker with timestamps.

## Related Runbooks

- [Incident Response](incident-response.md)
- [DR Drill](dr-drill.md)
- [Rotate Secrets](rotate-secrets.md)