# Runbook — Relay degraded (submissions failing)

**Owner:** on-call operator
**Severity:** warning
**Alert:** `HighHTTP5xxRate{service="relays-client"}`, future
`RelaySubmitFailuresSpiking`

## Symptoms

- Slack: *"Elevated 5xx on relays-client"* or the executions panel shows a
  sudden burst of `status="dropped"` / `status="reverted"` on a single relay.
- Grafana "Execution pipeline" → "Executions by status (by relay)" — one
  relay's line spikes red/orange.
- `arbx_cb_state{name="relay.<name>"}` gauge goes from 0 (closed) → 1 (half
  open) → 2 (open) within minutes.
- `/executions` page shows many rows with `error_message != null`.

## Immediate action (≤ 2 min)

1. If executions are going through another relay and overall revert rate is
   acceptable, you can let the circuit breaker do its job — skip to
   Diagnosis.
2. If the degraded relay is the *only* enabled one for its chain (check
   `/config` page `relays` table), arm the kill-switch with reason
   `"sole relay degraded — <name>"` and move to Diagnosis. The platform
   cannot execute without a healthy relay.

## Diagnosis

1. **Which relay, which chain, which error?**
   ```sql
   select relay_name, status, count(*), max(error_message)
     from executions
    where submitted_at >= now() - interval '30 minutes'
    group by relay_name, status
    order by count(*) desc;
   ```
   - Error class `429` → the relay is rate-limiting us. Back off or rotate auth.
   - `502` / `503` / timeouts → relay-side outage. Check their status.
   - `invalid_bundle` → our payload problem, not relay's. Check recent PRs to
     `relays-client` / `bundle_builder`.

2. **Is it systemic or single-relay?**
   Compare the relay's inclusion_rate over the last hour vs the prior 24h:
   ```sql
   select relay_name, window_end, inclusion_rate, score
     from relay_scores
    where chain_id = 1
      and window_end >= now() - interval '24 hours'
    order by relay_name, window_end desc;
   ```
   If every relay on the chain dropped at the same time, the cause is
   upstream (RPC, network, or chain itself). Go to `rpc-down.md`.

3. **Did we change auth recently?**
   ```sql
   select actor, action, before_state, after_state, created_at
     from audit_log
    where action like 'relay.%'
    order by created_at desc limit 10;
   ```

## Remediation

### A — Rate limited by the relay

1. Reduce priority of the relay (shift bundles elsewhere) via the admin API:
   ```bash
   curl -X PUT https://ops.<domain>/admin/relays/<id> \
        -H "x-arbx-admin-token: $ADMIN" \
        -d '{"priority": 200}'
   ```
2. Talk to the relay's support to raise the limit.

### B — Relay outage

1. Disable the relay (does NOT delete the row — preserves history + audit):
   ```bash
   curl -X PUT https://ops.<domain>/admin/relays/<id> \
        -H "x-arbx-admin-token: $ADMIN" \
        -d '{"enabled": false}'
   ```
2. Watch the CB state — it should go back to 0 (closed) once traffic stops.
3. Re-enable when their status page is green + do a probe bundle manually.

### C — Our bundle is malformed

1. Check the most recent deploy for changes under `backend/relays-client/src/bundle_builder.rs`.
2. If a bug — revert the commit first (`git revert`), deploy, then fix
   forward.
3. Do not patch production live while capital is at risk. Arm the kill-switch
   first.

## Post-incident

- Update `relay_scores` history is preserved automatically — nothing to do
  manually for the data side.
- If this was a recurring relay: add a `docs/known-issues/relay-<name>.md`
  pointing to this runbook from the symptoms section.
- Consider adding them as a *second* relay if they were previously primary —
  redundancy is cheaper than a 4am page.

## Related

- Dashboard: `arbx-execution`
- Alerts: `HighHTTP5xxRate{service="relays-client"}`, `RelaySubmitFailuresSpiking` (planned)
- Cross-references: `killswitch-activated.md`, `rpc-down.md`.
