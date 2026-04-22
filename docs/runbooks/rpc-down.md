# Runbook — RPC down (mempool detection idle)

**Owner:** on-call operator
**Severity:** warning after 15 min; critical after 30 min
**Alert:** `NoOpportunitiesDetectedLongWindow`

## Symptoms

- Slack: *"No opportunities detected in the last 15m — Searcher-rs may be
  idle or kill-switched; verify both."*
- `/status` page shows `searcher-rs` as **UP** (the service is alive) but
  `/opportunities` page is empty.
- Grafana "Detection pipeline": `arbx_searcher_pending_total` rate ≈ 0.
- `searcher-rs` logs contain repeating `scanner.idle` or `scanner.no_rpc`
  messages.

## Immediate action (≤ 2 min)

**Do NOT arm the kill-switch.** The platform is behaving correctly: it refuses
to fabricate data when the RPC is absent.

1. Check the RPC provider's status page (Alchemy / Infura / QuickNode).
2. Page the operator who owns the RPC key if the provider is healthy.

## Diagnosis

1. **Which state is searcher-rs in?**
   ```bash
   docker compose -f docker/compose.prod.yml logs --since 5m searcher-rs | tail -40
   ```
   - `scanner.no_rpc` → `RPC_WS_<chain>` env is empty. Missing credential.
   - `scanner.idle` → env set, but the loop is kill-switched (check
     `/killswitch`).
   - `scanner.connect_error` → env set, but the WS handshake is failing.
   - `scanner.subscription_error` → connected, then lost — upstream flapping.

2. **Is the env actually propagated into the container?**
   ```bash
   docker compose -f docker/compose.prod.yml exec searcher-rs env | grep RPC_
   ```
   If empty: vault-agent didn't render. See `vault-sealed.md`.

3. **Can the VPS reach the provider?**
   ```bash
   docker compose -f docker/compose.prod.yml exec searcher-rs \
     sh -c 'wget -qO- --timeout=5 "$RPC_HTTP_1" \
       --post-data="{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"eth_blockNumber\",\"params\":[]}" \
       --header="Content-Type: application/json"'
   ```
   Non-200 or empty → provider or network. Proceed to remediation A.
   200 with `{"result":"0x...."}` → provider is fine; the WS endpoint is the
   issue. Remediation B.

4. **Is the RPC rate-limiting us?**
   Prometheus: `sum(rate(arbx_rpc_rate_limited_total[5m]))` — planned for a
   future PR but not yet in shared-rs/metrics.rs. Until that metric lands,
   grep logs for `429` in searcher-rs output.

## Remediation

### A — RPC provider outage or credential wrong

1. Rotate to a backup provider:
   - Put the backup URL under Vault path `secret/arbitragex/prod/rpc/ws_1`
     (or `http_1`).
   - `docker compose -f docker/compose.prod.yml restart searcher-rs sim-ctl relays-client`.
2. Confirm `scanner.subscribed` appears in searcher-rs logs within 30 s.

### B — WS endpoint specifically failing

1. Check the provider's changelog for breaking changes to their WS surface.
2. If they deprecated `newPendingTransactions`, this is a bigger fix — open
   an incident, keep the platform in the current idle state, don't "hack"
   around it.

### C — Vault sealed / agent not rendering

See `vault-sealed.md`. The fix is there.

## Post-incident

- If this was a third-party outage: record the provider's incident URL in
  `docs/incidents/YYYY-MM-DD-<provider>-outage.md`.
- If this was a credential rotation we did ourselves: update
  `onboarding_progress.phase_2_rpc_probe_ok` timestamp (via the admin endpoint
  `POST /admin/onboarding/2/rotate`, landing in a later PR).

## Related

- Dashboard: `arbx-detection`
- Alerts: `NoOpportunitiesDetectedLongWindow`, future `RpcRateLimited`
- Cross-references: `killswitch-activated.md`, `vault-sealed.md`.
