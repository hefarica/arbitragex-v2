# Runbook — Rotate secrets

**Owner:** security lead + on-call operator (two-person rule)
**Severity:** info (planned) / critical (leak suspected)
**Alert:** none — scheduled every 90 days, or ad-hoc on leak suspicion.

## Symptoms

- Scheduled quarterly rotation.
- Suspicion or confirmation that a token was exposed (commit push, screen-share
  leak, former-operator offboarding, phishing).

## Immediate action (≤ 2 min)

If a leak is confirmed or suspected:

1. **Arm the kill-switch.** Reason: `"secret rotation — suspected leak of
   <name>"`.
2. Inform the security lead.
3. If the leak is a **signing key** (FLASHBOTS_SIGNER_KEY or any wallet key):
   check the on-chain address's balance on a block explorer. If it has funds,
   drain to a cold wallet *immediately* before rotation.

If it's a planned rotation, skip the kill-switch — do it during a low-traffic
window.

## Diagnosis — what is leaking?

| Leaked secret | Impact | Time-to-exploit |
|---------------|--------|-----------------|
| `ARBX_ADMIN_TOKEN` | full admin API access (arm KS, change config, CRUD relays) | minutes |
| `ARBX_EDGE_TOKEN` | only useful if leaker also has network access to api-server (internal-only header) | needs VPS network |
| `JWT_SECRET` | forge operator-console sessions | minutes |
| `FLASHBOTS_SIGNER_KEY` | submit bundles signed as us (Flashbots reputation hit + possible fund loss if signer has balance) | minutes |
| `GRAFANA_ADMIN_PASSWORD` | edit dashboards / exfiltrate metrics | slow |
| `POSTGRES_PASSWORD` | exfiltrate audit log + opportunities + executions history | fast if network-reachable |
| Cloudflare API token | redirect public edge / exfiltrate logs | minutes |
| PagerDuty / Slack webhook | spam false alerts; reputational | slow |

## Remediation

### A — Regular rotation (planned)

Repeat for each secret in the list above:

1. Generate a new value using the appropriate method (see below).
2. `vault kv put secret/arbitragex/prod/<path> value=<new>`.
3. Restart the services that read this secret:
   ```bash
   docker compose -f docker/compose.prod.yml restart \
     api-server edge relays-client searcher-rs recon selector-api sim-ctl
   ```
   (Not every secret affects every service; the list above is the safe
   superset.)
4. Verify `/status` reports every service UP within 60 s.
5. Record the rotation:
   ```sql
   INSERT INTO audit_log (actor, action, target_kind, target_id, after_state)
   VALUES ('<operator>', 'secret.rotate', 'vault_path',
           'secret/arbitragex/prod/<path>',
           jsonb_build_object('ts', now(), 'reason', 'scheduled quarterly'));
   ```

### B — Emergency rotation (suspected leak)

Same as A, but:

1. Kill-switch stays ARMED until *all* rotations are complete.
2. After rotation, check `audit_log` for any admin action attributed to the
   leaked credential since you last rotated — any row there may be the
   attacker, not you.
3. For `FLASHBOTS_SIGNER_KEY`: keep the new key at zero balance for at least
   one week while you watch the *old* address on-chain. Move funds to the new
   one only after the watch period.

### Generation methods

| Secret | How |
|--------|-----|
| `ARBX_ADMIN_TOKEN`, `ARBX_EDGE_TOKEN` | `openssl rand -base64 48` (≥ 32 bytes) |
| `JWT_SECRET` | `openssl rand -base64 64` |
| `FLASHBOTS_SIGNER_KEY` | `cast wallet new --json` (foundry) — keep the private key in Vault, never write the mnemonic if you can avoid it |
| `GRAFANA_ADMIN_PASSWORD` | `openssl rand -base64 24` |
| `POSTGRES_PASSWORD`, `ARBX_RW_PASSWORD` | `openssl rand -base64 32` |
| Cloudflare API token | regenerate in CF dashboard with *minimum* scopes: Workers deploy, Tunnel route, DNS:edit on `<domain>` only |
| PagerDuty integration key | regenerate in PD service → integrations |
| Slack webhook | delete the old one in Slack → create a new one, same channel |

## Post-incident (if this was a leak)

1. Forensic: keep pre-rotation Vault audit logs + `audit_log` rows for 90 days.
2. Revoke any exchange API keys / cold-wallet access that shared the
   compromised environment.
3. Run `docs/runbooks/incident-postmortem.md` (planned — lives under
   `docs/incidents/<date>-<slug>.md`).
4. Update `onboarding_progress` — specifically phase-1 fields — to reflect
   the new rotation timestamp via
   `POST /admin/onboarding/1/complete` with a rotation note.

## Related

- Dashboard: none (secrets are invisible)
- Alerts: none automated — future PR adds `VaultKeyStale` for quarterly reminders.
- Cross-references: `vault-sealed.md`, `killswitch-activated.md`.
