# Runbook — Kill-Switch Activation and Deactivation

| Field | Value |
|-------|-------|
| **Owner** | On-call operator |
| **Severity** | Warning (auto-trip) to Critical (emergency arm) |
| **Alert** | `KillSwitchActivated` |
| **ETA to respond** | 2 minutes |
| **Prerequisites** | Admin token (`ARBX_ADMIN_TOKEN`), SSH access to VPS |

## Purpose

This runbook describes when and how to activate (arm) and deactivate (disarm) the global kill-switch that halts all execution on the ArbitrageX v2 platform. It also covers automated kill-switch events triggered by the `recon` anomaly detector and the post-incident review process.

## When to Activate (Manual Arm)

Activate the kill-switch immediately when any of the following conditions are observed:

| Condition | Severity | Indicator |
|-----------|----------|-----------|
| Revert rate > 5% sustained for > 60 seconds | Critical | Grafana "Revert Rate" panel red; `arbx_execution_total{status="reverted"}` spike |
| Anomalous opportunity volume (10x baseline) | Warning | May indicate fake volume attack or oracle manipulation |
| Relay degradation with > 50% submission failure | Critical | `arbx_execution_total{status="submitted"}` flat while `detected` rises |
| Suspicious token contract detected | Critical | Selector flags honeypot or blacklisted token but bypass suspected |
| Operator investigation in progress | Info | Any situation where you want to pause execution while investigating |
| External intelligence (compromised key, protocol hack) | Critical | News of DEX or protocol exploit that affects active strategies |

## How to Activate

### Method 1: API (Preferred)

```bash
# Set environment
export ARBX_ADMIN_TOKEN="<your-admin-token>"
export API_URL="http://195.201.235.70:8080"

# Arm the kill-switch with mandatory reason
curl -s -X POST "${API_URL}/admin/killswitch" \
  -H "Authorization: Bearer ${ARBX_ADMIN_TOKEN}" \
  -H "Content-Type: application/json" \
  -H "x-arbx-actor: <your-operator-id>" \
  -d '{
    "enabled": true,
    "reason": "High revert rate detected — investigating strategy S-0042",
    "triggered_by": "operator:john-doe"
  }' | jq .
```

Expected response (200 OK):
```json
{
  "enabled": true,
  "reason": "High revert rate detected — investigating strategy S-0042",
  "triggered_by": "operator:john-doe",
  "updated_at": "2026-05-17T14:32:00Z"
}
```

If you see `401 Unauthorized`, verify your `ARBX_ADMIN_TOKEN` is correct and has not expired.

### Method 2: Redis Direct (Emergency — API Unreachable)

If the API server is down but Redis is accessible:

```bash
# Connect to Redis
redis-cli -h 195.201.235.70 -p 6379

# Arm the kill-switch
SET arbx:killswitch:enabled '{"enabled":true,"reason":"Emergency arm via Redis — API down","triggered_by":"operator:john-doe","updated_at":"2026-05-17T14:32:00Z"}'

# Publish to change channel for immediate propagation
PUBLISH arbx:killswitch:changes '{"enabled":true,"reason":"Emergency arm via Redis"}'
```

> **Warning**: Direct Redis manipulation bypasses audit logging. Document the action in the incident log manually.

### Method 3: File Fallback (Boot-Time Only)

Edit `killswitch.json` at the repo root for the next boot:

```bash
# On the VPS host
cd /opt/arbitragex-v2
cat > killswitch.json << 'EOF'
{
  "enabled": true,
  "reason": "Pre-armed for maintenance window",
  "updated_at": "2026-05-17T14:32:00Z"
}
EOF
```

> **Note**: This only affects services on their next boot. Running services read from Redis (Layer 1). Restart services after editing: `docker compose restart api-server searcher-rs relays-client`.

## How to Verify Activation

Check all three indicators within 30 seconds of activation:

### 1. API Status

```bash
curl -s "${API_URL}/status" | jq '.killswitch'
```

Expected:
```json
{
  "enabled": true,
  "reason": "High revert rate detected — investigating strategy S-0042",
  "triggered_by": "operator:john-doe",
  "updated_at": "2026-05-17T14:32:00Z"
}
```

### 2. Grafana Dashboard

Open the "Platform Overview" dashboard. The "Kill-Switch" panel should show **red** with status "ARMED".

### 3. Slack Alert

The `#arbx-alerts` channel should receive:
> **Kill switch is ON** — The global kill switch is armed. All executions are refused. Triggered by: `operator:john-doe`

### 4. Execution Halt Verification

Within 5 seconds of activation, the `arbx_execution_total{status="submitted"}` counter should stop increasing. The `arbx_execution_total{status="refused"}` counter should increase if any executions were attempted.

```bash
# Query Prometheus
curl -s "http://195.201.235.70:9090/api/v1/query?query=arbx_execution_total%7Bstatus%3D%22refused%22%7D" | jq '.data.result[0].value[1]'
```

## How to Deactivate

### Prerequisites for Deactivation

Before disarming, the triggering condition must be resolved:

| Trigger Type | Resolution Required |
|-------------|---------------------|
| High revert rate | Revert rate < 1% for 5 consecutive minutes; root cause identified |
| Relay degradation | Relay health checks passing; submission success rate > 90% |
| Anomaly volume | Volume returned to baseline; not a market-wide event |
| Suspicious token | Token added to blacklist; strategy verified against token |
| Investigation | Investigation complete; fix deployed or false alarm confirmed |
| External intelligence | Threat no longer active; strategy not affected |

### Deactivation Command

```bash
curl -s -X POST "${API_URL}/admin/killswitch" \
  -H "Authorization: Bearer ${ARBX_ADMIN_TOKEN}" \
  -H "Content-Type: application/json" \
  -H "x-arbx-actor: <your-operator-id>" \
  -d '{
    "enabled": false,
    "reason": "Revert rate normalized — root cause: stale RPC cache, cleared",
    "triggered_by": "operator:john-doe"
  }' | jq .
```

### Post-Deactivation Verification

1. `/status` shows `killswitch.enabled = false`
2. Grafana "Kill-Switch" panel shows **green** with status "DISARMED"
3. `arbx_opportunity_total{status="detected"}` resumes increasing
4. `arbx_execution_total{status="submitted"}` resumes increasing

## Automated Kill-Switch (recon Auto-Trip)

The `recon` service can automatically arm the kill-switch when configured thresholds are breached.

### Configuration

In `configs/app.toml`:
```toml
[recon]
anomaly_revert_rate_pct = 5.0      # Auto-trip threshold
auto_trip_on_high_revert_rate = true  # Enable auto-trip
anomaly_detection_window_sec = 60     # Evaluation window
```

### Auto-Trip Response

When auto-trip fires:
1. The `actor` field shows `recon:anomaly_detector`, not a human operator.
2. The `reason` field contains the threshold breach details: `revert_rate=7.3% > threshold=5.0%`.
3. **Do not disarm until the underlying cause is resolved.** The auto-trip is almost always correct.

### Diagnosis Query

```bash
# Check recon risk events
psql "$DATABASE_URL" -c "
  SELECT payload, created_at
  FROM risk_events
  WHERE event_type = 'kill_switch'
  ORDER BY created_at DESC
  LIMIT 5;
"
```

## System Guard Banner

When the kill-switch is armed, every response from the API includes a `x-arbx-system-guard` header:

```
x-arbx-system-guard: ARMED — 2026-05-17T14:32:00Z — High revert rate detected
```

The frontend displays this as a persistent red banner across all pages.

## Post-Incident Review Checklist

Every kill-switch activation (manual or auto) requires a post-incident review within 24 hours.

### Checklist

- [ ] **Timeline documented**: Extract from `audit_log` — exact activation and deactivation timestamps
- [ ] **Trigger identified**: Root cause of the activation condition
- [ ] **Impact assessed**: Opportunities missed, simulations refused, estimated P&L impact
- [ ] **Resolution verified**: The fix or condition change that allowed safe deactivation
- [ ] **False positive review**: If auto-trip, was the threshold appropriate? Should `anomaly_revert_rate_pct` be adjusted?
- [ ] **Runbook update**: If the response revealed gaps in this runbook, open a PR to update it
- [ ] **Incident file created**: `docs/incidents/YYYY-MM-DD-<slug>.md` with full timeline and lessons learned

### Incident File Template

```markdown
# Incident Report — YYYY-MM-DD

| Field | Value |
|-------|-------|
| **Date** | YYYY-MM-DD HH:MM UTC |
| **Duration** | HH:MM |
| **Trigger** | operator:xxx / recon:auto / external |
| **Severity** | warning / critical |

## Timeline

- HH:MM:SS — Condition detected (describe)
- HH:MM:SS — Kill-switch armed by (actor)
- HH:MM:SS — Investigation began
- HH:MM:SS — Root cause identified
- HH:MM:SS — Kill-switch disarmed by (actor)

## Root Cause

(Explain the technical reason)

## Impact

- Opportunities missed: N
- Estimated P&L impact: X ETH
- No real capital at risk (paper mode)

## Resolution

(What was done to fix the underlying issue)

## Follow-up Actions

- [ ] Action item 1
- [ ] Action item 2
```

## Related

- ADR-002: Kill-Switch Fail-Closed Design
- `docs/adr/002-kill-switch-fail-closed.md`
- `docs/runbooks/killswitch-activated.md` (legacy reference)
- `docs/governance/RISK_POLICY.md`
