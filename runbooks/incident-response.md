---
id: R-IR-001
title: Incident Response Playbook
severity: P0
duration: Variable
owner: SRE On-Call
reviewed: 2025-01-15
---

# Incident Response Playbook

## Purpose

Provide a structured response framework for incidents affecting the
ArbitrageX V2 trading system. Covers severity classification, escalation
chains, communication protocols, and resolution procedures.

## Severity Levels

### SEV-0 — Critical (P0)

**Impact**: Complete trading halt, funds at risk, or active exploit.

**Examples**:
- All RPC endpoints down > 2 minutes
- Smart contract exploit detected
- Kill-switch activated unexpectedly
- Oracle manipulation causing bad executions
- Database corruption or data loss

**Response Time**: Immediate (page on-call within 2 minutes)
**Resolution Target**: 30 minutes

### SEV-1 — Major (P1)

**Impact**: Significant degradation, partial outage, or elevated risk.

**Examples**:
- Single chain RPC failure > 10 minutes
- Opportunity detection pipeline stalled
- Execution submission failure rate > 10%
- Memory leak causing pod restarts

**Response Time**: Within 15 minutes
**Resolution Target**: 2 hours

### SEV-2 — Moderate (P2)

**Impact**: Degraded performance or non-critical feature unavailable.

**Examples**:
- WebSocket latency > 500ms
- Opportunity throughput below 50% of baseline
- Non-critical background job failures
- Monitoring gaps or false alerts

**Response Time**: Within 1 hour
**Resolution Target**: 4 hours

### SEV-3 — Minor (P3)

**Impact**: Cosmetic issues, documentation gaps, or low-priority bugs.

**Examples**:
- UI display issues
- Log verbosity problems
- Minor metric discrepancies
- Documentation errors

**Response Time**: Next business day
**Resolution Target**: 1 week

---

## Escalation Chain

```
L1: SRE On-Call
  |___ No response in 5 min (SEV-0) / 15 min (SEV-1)
L2: SRE Lead + Platform Engineering Lead
  |___ No response in 10 min (SEV-0) / 30 min (SEV-1)
L3: Engineering Manager + CTO
  |___ No response in 15 min (SEV-0) / 1 hour (SEV-1)
L4: Executive team + Legal (if funds at risk)
```

### Contact Methods (in order)

1. PagerDuty page
2. Phone call (secondary number)
3. Slack DM
4. Personal phone (escalation only)

---

## Response Procedure

### Phase 1 — Detect & Assess (0–5 min)

1. Acknowledge the page in PagerDuty.
2. Join the auto-created #incidents-YYYY-MM-DD-HHMM Slack channel.
3. Classify severity using the levels above.
4. Verify the alert is not a false positive.
5. If SEV-0, immediately consider kill-switch activation.

### Phase 2 — Respond & Mitigate (5–30 min)

1. For SEV-0: Activate kill-switch if funds are at risk.
2. For SEV-1: Identify failing component and apply mitigations:
   - Restart degraded pods
   - Switch to backup RPC endpoints
   - Scale up affected services
   - Enable circuit breaker
3. Communicate status every 10 minutes in the incident channel.

### Phase 3 — Resolve & Verify (30 min–2 hours)

1. Apply the fix (code change, config update, or infrastructure change).
2. Verify the fix using health probes and smoke tests.
3. Monitor for 15 minutes post-fix.
4. Confirm resolution and downgrade severity.

### Phase 4 — Post-Incident (within 24 hours)

1. Write an incident summary in the channel.
2. Schedule a post-mortem for SEV-0/1 incidents.
3. Create follow-up JIRA tickets for action items.
4. Update runbooks if response gaps were found.

---

## Communication Templates

### Initial Alert (SEV-0)

```
:red_circle: SEV-0 Incident Declared
Time: <ISO-8601 timestamp>
Reporter: @<handle>
Symptom: <one-line description>
Impact: <affected chains, services, users>
Action: Kill-switch <activated / under evaluation>
Status: INVESTIGATING
Updates: Every 10 minutes in this thread
```

### Status Update

```
:arrow_right: Status Update — <timestamp>
Current: <what is happening now>
Progress: <what has been tried / ruled out>
Next: <next action and who is doing it>
ETA: <estimated time to resolution or "unknown">
```

### Resolution

```
:white_check_mark: RESOLVED — <timestamp>
Duration: <total incident duration>
Root Cause: <brief description>
Resolution: <what fixed it>
Follow-up: <post-mortem time or ticket numbers>
```

### Post-Mortem Template

```
# Post-Mortem: INC-<number> — <title>

## Timeline
- <time> — Detection
- <time> — Acknowledged
- <time> — Mitigation started
- <time> — Resolved

## Root Cause
<detailed description>

## Impact
- Affected chains:
- Missed opportunities (estimated):
- Failed executions:
- Financial impact (if any):

## What Went Well
- <item>

## What Went Poorly
- <item>

## Action Items
| ID | Action | Owner | Due Date |
|----|--------|-------|----------|
| AI-1 | <description> | @owner | YYYY-MM-DD |

## Lessons Learned
<insights for preventing recurrence>
```

---

## Communication Channels

| Channel | Use |
|---------|-----|
| #incidents | Live incident coordination |
| #incidents-comms | Customer-facing updates |
| #on-call | On-call handoffs and scheduling |
| #dr-drill | DR drill coordination |
| #maintenance | Planned maintenance windows |
| PagerDuty | Pages and escalations |
| Status Page | External customer updates (SEV-0/1) |

---

## Rollback Considerations

Before rolling back a deployment or configuration change:

1. Confirm the rollback target is known-good.
2. Check if the rollback introduces a different risk.
3. For SEV-0: prefer kill-switch + investigation over blind rollback.
4. Document the rollback decision and expected outcome.
5. Monitor for 15 minutes post-rollback.

## Success Criteria

- SEV-0 incidents are acknowledged within 2 minutes.
- Kill-switch is activated within 5 minutes when funds are at risk.
- Status updates are posted every 10 minutes during active incidents.
- Post-mortems are completed within 24 hours for SEV-0/1.
- All action items from post-mortems have owners and due dates.
- No repeat incidents of the same root cause within 30 days.

## Related Runbooks

- [Kill-Switch](kill-switch.md) — Emergency trading halt
- [DR Drill](dr-drill.md) — Infrastructure recovery validation
- [Rotate Secrets](rotate-secrets.md) — Credential rotation if compromise suspected