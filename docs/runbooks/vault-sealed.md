# Runbook — Vault sealed (or unreachable)

**Owner:** on-call operator + any quorum-share-holder
**Severity:** critical — the platform cannot start
**Alert:** `ServiceDown{service=~"api-server|searcher-rs|…"}` cascading,
indirect indicator.

## Symptoms

- Multiple services fail at boot with log lines like
  `vault-agent: failed to auth via approle` or
  `template: rendering failed: vault: sealed`.
- `docker compose ... up` reports dependent services as unhealthy;
  `vault-agent` container is in `restart` loop.
- `vault status` from the VPS returns `Sealed: true`.
- `/status` page (if api-server came up before vault went down) shows
  cascading DOWN.

## Immediate action (≤ 2 min)

1. Open the unseal bridge call. This is a **multi-person** operation:
   unsealing Vault in production requires *k* of *n* Shamir key holders
   (k=3, n=5 per the doctrine). Do NOT share your share over unencrypted
   channels.
2. If the sealing was *unintentional* (restart, OOM), proceed to Diagnosis
   then Remediation A.
3. If the sealing was *defensive* (operator sealed on purpose — leak
   response, suspected intrusion) — do NOT unseal until the triggering
   incident is closed. This IS the defense.

## Diagnosis

1. **Is Vault running at all?**
   ```bash
   docker compose -f docker/compose.prod.yml ps vault
   docker compose -f docker/compose.prod.yml logs --since 5m vault
   ```
   If the container is stopped, that's cause #1. Start it before anything
   else: `docker compose start vault`.

2. **Is it just sealed?**
   ```bash
   docker compose -f docker/compose.prod.yml exec vault vault status
   ```
   Expected when sealed: `Sealed: true`, `Total Shares: 5`, `Threshold: 3`.

3. **Is the storage healthy?**
   ```bash
   docker compose -f docker/compose.prod.yml exec vault ls -lh /vault/file
   ```
   If the file backend is empty or truncated, the Vault data is lost. This
   is the disaster scenario: go to Remediation C.

4. **Did we lose AppRole secret IDs for services?**
   If vault is unsealed but services still can't auth, check the role's
   secret-id TTL. Long-lived secret IDs should be stored in
   `/run/secrets/arbx/<service>.role-id` and `<service>.secret-id` on the
   host, rendered by `automation/scripts/vault-init.sh` (planned).

## Remediation

### A — Unseal (3-of-5 shamir)

Each key holder runs, in series (Vault is single-node; one keeper submits at a time):

```bash
docker compose -f docker/compose.prod.yml exec vault vault operator unseal
# prompts for unseal key — holder pastes their share
```

After the threshold is reached (3 shares), `Sealed: false`. Within 30 s:

1. vault-agent reconciles and renders `/run/secrets/arbx/*`.
2. Services that were waiting boot.
3. `/status` turns green.

### B — Vault is running but AppRole broken

1. Log in with the *root* token (kept offline; should only surface for
   break-glass):
   ```bash
   docker compose exec vault vault login
   ```
2. Re-issue secret IDs per service:
   ```bash
   vault write -f auth/approle/role/searcher-rs/secret-id
   # ... repeat per service
   ```
3. Write them to `/run/secrets/arbx/<svc>.secret-id` and restart services.

### C — Storage backend destroyed

This is the disaster scenario the doctrine was built to make survivable.

1. Recover the Vault file from the latest encrypted backup:
   ```bash
   age -d -i /root/arbx.age-identity \
       -o /tmp/vault-file.tar.gz \
       /var/backups/arbx/vault-<YYYYMMDD-HHMM>.tar.gz.age
   tar -xzf /tmp/vault-file.tar.gz -C /var/lib/docker/volumes/arbx-vault/_data/
   ```
   (Backup of the Vault file backend is a follow-up to Phase 8 — add to
   `automation/scripts/backup-vault.sh` when Phase 8 lands.)
2. Restart Vault. Unseal per Remediation A.
3. If no Vault backup exists, **every secret must be rotated** (see
   `rotate-secrets.md` — Case B emergency flow). This includes
   `FLASHBOTS_SIGNER_KEY` (generate new + move funds).

## Post-incident

- If you used a break-glass root token: rotate the root token immediately
  (`vault operator generate-root`) and audit who had access.
- Write the incident to `docs/incidents/YYYY-MM-DD-vault-<short>.md`.
- If this was a disaster-recovery test: congratulations. Record wall-clock
  time from detection to full recovery; aim to bring it under 30 min.

## Related

- Dashboard: none direct; effects show in `arbx-platform-overview` via
  `arbx_service_up`.
- Alerts: `ServiceDown` cascade.
- Cross-references: `rotate-secrets.md`, `db-restore.md`.
