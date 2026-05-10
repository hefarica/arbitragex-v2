# Runbook — Kill-switch activated

**Owner:** on-call operator
**Severity:** warning (but often preceded by a critical)
**Alert:** `KillSwitchActivated` (monitoring/alerts.rules.yml)

## State precedence (audit B10, 2026-05-10)

The kill-switch state is read in this priority order:

1. **Redis key `arbx:killswitch:enabled`** (canonical, runtime-mutable). Set via
   `POST /admin/killswitch` or `redis-cli SET arbx:killswitch:enabled 1`.
2. **File `killswitch.json` at repo root** (legacy boot-time fallback). Read once
   at service boot if Redis is unreachable. NOT polled at runtime — Redis is the
   live source of truth.
3. **Default when both are absent**: `cfg.system.kill_switch_enabled_default` from
   `configs/app.toml` (defaults to `false` in dev, `true` in prod profile).

The file `killswitch.json` is a doctrine artifact: it documents that the project
ships with kill-switch state explicitly declared even when offline. Editing it
DOES NOT toggle a running service — Redis must be updated. Operators should
treat the file as read-only after first boot and use `POST /admin/killswitch` for
all state changes.

## Symptoms

- Slack `#arbx-alerts` receives: *"Kill switch is ON — The global kill switch
  is armed. All executions are refused."*
- Grafana "Platform Overview" panel "Kill-switch" is **red / ARMED**.
- `/killswitch` page in the operator console shows a banner "ARMED — executions
  blocked".
- `arbx_execution_total{status=~"submitted|included"}` stops increasing within
  ≤ 5 s of the toggle; `arbx_opportunity_total{status="detected"}` keeps going
  unless the RPC is also down.

## Immediate action (≤ 2 min)

You are looking at this because the kill-switch is already ON. Do NOT disable
it until you understand *why*.

1. Open `/risk` — check the `killswitch` banner for `reason` and `triggered_by`.
2. Open `/executions` — look at the last 50 rows for a pattern (same relay
   failing? same strategy reverting? one opportunity id repeated?).
3. Open `/recon` — check the "Revert rate" KPI in the last 1h window.

If `triggered_by` is a service name (e.g. `recon:anomaly_detector`), the
platform did this by policy — treat it as correct until proved otherwise.

## Diagnosis

Ordered, low-effort first.

1. **Who triggered it?**
   `psql $DATABASE_URL -c "select actor, action, before_state, after_state, created_at from audit_log where action = 'admin.killswitch' order by created_at desc limit 5;"`
   If `actor` is a human — call them. If it's a service, check its logs.

2. **Revert rate anomaly?**
   If `recon.auto_trip_on_high_revert_rate=true` in `configs/app.toml` and
   `risk_events` shows a recent row with `event_type='kill_switch'`, an anomaly
   tripped it. Look at the row payload.
   ```sql
   select payload, created_at
     from risk_events
    where event_type='kill_switch'
    order by created_at desc
    limit 5;
   ```

3. **Circuit breaker cascade?**
   `arbx_cb_state{name=...}` panel in the "Recon & risk" dashboard. If every CB
   is open (value 2), upstream (RPC, DB, Redis) is probably struggling. Fix
   upstream first.

4. **External cause?**
   - RPC provider outage (Alchemy status page).
   - Chain reorg (`eth_blockNumber` retreated — rare on mainnet).
   - DB or Redis down (`/status` page shows the services as DOWN).

## Remediation

Two cases:

### Case A — platform tripped itself correctly

Wait for the underlying cause to resolve (revert rate drops, external provider
recovers), then disarm explicitly and record reason:

1. Go to `/killswitch`.
2. Enter admin token, reason `"auto-trip recovered — $CAUSE"`, click **Disable**.
3. Watch `/status` for 2–3 minutes; `arbx_opportunity_total{status="detected"}`
   should resume.

### Case B — human armed it during investigation

1. Whoever armed it must own the disarm. Do not disarm on their behalf.
2. When they're ready: same UI as Case A, but reason names the incident ID.

Both cases: never skip the `reason` field. It goes into `audit_log.after_state`
and becomes the historical answer to "why were we down from 14:02 to 14:17".

## Post-incident

- Grab a copy of `audit_log` + `risk_events` for the incident window.
- If this was a false auto-trip (threshold too aggressive), open a PR against
  `configs/app.toml` `[recon].anomaly_revert_rate_pct` with a sign-off line.
  The change must also update `docs/governance/DATA-MATRIX.md` §M5.
- If a real incident: create `docs/incidents/YYYY-MM-DD-<slug>.md` with the
  timeline.

## Related

- Dashboard: `arbx-recon-risk`
- Alerts: `KillSwitchActivated`, `NoOpportunitiesDetectedLongWindow`
- Cross-references: `rpc-down.md`, `relay-degraded.md`.
