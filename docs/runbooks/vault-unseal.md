# Runbook — Vault Unseal Procedure

| Field | Value |
|-------|-------|
| **Owner** | On-call operator + quorum of unseal key holders |
| **Severity** | Critical — the platform cannot start or recover secrets |
| **Alert** | `ServiceDown{service=~"api-server|searcher-rs|sim-ctl|relays-client"}` cascading |
| **ETA to respond** | 15 minutes |
| **Prerequisites** | SSH access to VPS, 3 of 5 unseal key shares, `docker compose` access |

## Purpose

This runbook describes the procedure for checking Vault seal status, unsealing Vault after a restart or shutdown, and recovering from key loss or storage destruction scenarios. Vault must be unsealed for any service to access runtime secrets.

## Architecture Reference

| Parameter | Value |
|-----------|-------|
| **Vault container** | `vault` (HashiCorp Vault) |
| **Storage** | File backend (`/vault/file`) |
| **Total shares** | 5 |
| **Threshold** | 3 |
| **Storage volume** | `vault_data` (Docker named volume) |
| **Audit logs** | `/vault/logs/audit.log` |
| **Address** | `http://vault:8200` (internal) |

## How to Check Seal Status

### Method 1: From the Host

```bash
# SSH into the VPS
ssh operator@195.201.235.70

# Check Vault container status
cd /opt/arbitragex-v2
docker compose -f docker/compose.prod.yml ps vault

# Expected (running):
# NAME      IMAGE          STATUS
# vault     hashicorp/vault   Up 2 minutes

# Check seal status
docker compose -f docker/compose.prod.yml exec vault vault status
```

Expected output when **sealed**:
```
Key                Value
---                -----
Seal Type          shamir
Initialized        true
Sealed             true
Total Shares       5
Threshold          3
Unseal Progress    0/3
Unseal Nonce       n/a
Version            1.15.x
Storage Type       file
HA Enabled         false
```

Expected output when **unsealed**:
```
Key                Value
---                -----
Seal Type          shamir
Initialized        true
Sealed             false
Total Shares       5
Threshold          3
Version            1.15.x
Storage Type       file
HA Enabled         false
```

### Method 2: From Inside the Vault Container

```bash
docker compose -f docker/compose.prod.yml exec vault /bin/sh
vault status
exit
```

### Method 3: Via API

```bash
curl -s http://localhost:8200/v1/sys/seal-status | jq .
```

## Normal Unseal Procedure

This procedure applies when Vault was sealed due to:
- Container restart
- Host reboot
- OOM kill
- Manual seal for maintenance

### Step 1: Verify Vault is Running

```bash
docker compose -f docker/compose.prod.yml ps vault
```

If the container is **stopped**:
```bash
docker compose -f docker/compose.prod.yml start vault
sleep 5
docker compose -f docker/compose.prod.yml exec vault vault status
```

If the container is **missing** (e.g., after `docker compose down`):
```bash
docker compose -f docker/compose.prod.yml up -d vault
sleep 10
docker compose -f docker/compose.prod.yml exec vault vault status
```

### Step 2: Coordinate Key Holders

This is a **multi-person operation**. Three key holders must be available simultaneously.

Communication protocol:
1. Operator opens the unseal bridge call (Signal/Zoom).
2. Each key holder confirms availability without revealing their share.
3. Key holders submit shares **one at a time** in any order.
4. **Never share your key over unencrypted channels** (Slack, unencrypted email, SMS).

### Step 3: Submit Unseal Shares

Each key holder runs the following command in sequence (wait for the previous to complete):

```bash
# Key Holder 1
docker compose -f docker/compose.prod.yml exec vault vault operator unseal
# Enter unseal key (will not be displayed on screen):
# Key (will be hidden):
# Key 1/3 entered. 2 more required.

# Key Holder 2
docker compose -f docker/compose.prod.yml exec vault vault operator unseal
# Key (will be hidden):
# Key 2/3 entered. 1 more required.

# Key Holder 3
docker compose -f docker/compose.prod.yml exec vault vault operator unseal
# Key (will be hidden):
# Key 3/3 entered. Vault unsealed.
```

After the third share, verify:
```bash
docker compose -f docker/compose.prod.yml exec vault vault status
# Sealed: false
```

### Step 4: Verify vault-agent Reconciliation

Within 30 seconds of unsealing, `vault-agent` should:
1. Authenticate via AppRole
2. Read secrets from KV v2
3. Render templates to `/run/secrets/arbx/`

Verify:
```bash
# Check vault-agent logs
docker compose -f docker/compose.prod.yml logs --since 1m vault-agent

# Verify secret files exist
ls -la /run/secrets/arbx/
# Expected:
# arbx.env
# searcher-rs.role-id
# searcher-rs.secret-id
# api-server.role-id
# api-server.secret-id
# ...
```

### Step 5: Start or Restart Services

```bash
# If services were waiting on secrets
docker compose -f docker/compose.prod.yml up -d

# Or restart specific unhealthy services
docker compose -f docker/compose.prod.yml restart api-server searcher-rs sim-ctl relays-client recon selector-api token-enricher edge

# Wait for health checks
sleep 30
docker compose -f docker/compose.prod.yml ps
```

### Step 6: Verify Full System Health

```bash
# Check the status endpoint
curl -s http://localhost:8080/status | jq .

# All services should show ok: true
# {
#   "ok": true,
#   "services": {
#     "selector-api": { "ok": true, "status": 200 },
#     "sim-ctl": { "ok": true, "status": 200 },
#     ...
#   },
#   "killswitch": { "enabled": false, ... }
# }
```

## What to Do if Keys Are Lost

### Scenario: 1 or 2 Shares Lost

If 1 or 2 key holders lose their shares, the remaining 3+ holders can still unseal. **This is the designed resilience.**

After unsealing:
1. Log in with the root token:
   ```bash
   docker compose -f docker/compose.prod.yml exec vault vault login
   # Enter root token
   ```
2. Re-key Vault to generate new shares:
   ```bash
   docker compose -f docker/compose.prod.yml exec vault vault operator rekey -init -key-shares=5 -key-threshold=3
   ```
3. Distribute new shares to 5 new holders (or re-confirm existing holders).
4. Rotate the root token:
   ```bash
   docker compose -f docker/compose.prod.yml exec vault vault operator generate-root -init
   ```

### Scenario: 3 or More Shares Lost (Insufficient Quorum)

If fewer than 3 shares are available, **Vault cannot be unsealed** with the existing data.

Options:

| Option | Data Loss | Effort | When to Use |
|--------|-----------|--------|-------------|
| **A. Recovery from backup** | None | Medium | Recent backup exists (< 24h old) |
| **B. Re-initialize Vault** | All secrets | High | No backup, but all secrets can be rotated |
| **C. Disaster recovery** | None | Very High | Vault Enterprise DR replication (not available in OSS) |

#### Option A — Recover from Backup

```bash
# Decrypt the latest backup
age -d -i /root/arbx.age-identity \
    -o /tmp/vault-file.tar.gz \
    /var/backups/arbx/vault-$(date +%Y%m%d)*.tar.gz.age

# Stop Vault
docker compose -f docker/compose.prod.yml stop vault

# Extract to the Docker volume
tar -xzf /tmp/vault-file.tar.gz -C /var/lib/docker/volumes/arbitragex-v2_vault_data/_data/

# Restart Vault
docker compose -f docker/compose.prod.yml start vault
sleep 5

# Unseal with the ORIGINAL keys (these are the pre-loss keys)
docker compose -f docker/compose.prod.yml exec vault vault operator unseal
# ... 3 of original shares

# Clean up
rm /tmp/vault-file.tar.gz
```

> **Note**: Backup restoration requires the original unseal keys. This is why shares must be stored in physically separate, secure locations.

#### Option B — Re-initialize Vault (Nuclear Option)

This destroys all existing secrets. Only proceed if Option A is impossible.

```bash
# 1. Stop Vault
docker compose -f docker/compose.prod.yml stop vault

# 2. Destroy the storage volume
docker compose -f docker/compose.prod.yml rm vault
docker volume rm arbitragex-v2_vault_data

# 3. Start fresh Vault
docker compose -f docker/compose.prod.yml up -d vault
sleep 10

# 4. Initialize with NEW shares
docker compose -f docker/compose.prod.yml exec vault vault operator init \
  -key-shares=5 -key-threshold=3
# Save the new unseal keys and root token SECURELY

# 5. Unseal with 3 new shares
docker compose -f docker/compose.prod.yml exec vault vault operator unseal
# ... 3 times

# 6. Re-create the KV secrets engine
docker compose -f docker/compose.prod.yml exec vault vault login
# Enter new root token
docker compose -f docker/compose.prod.yml exec vault vault secrets enable -path=arbx kv-v2

# 7. Rotate ALL secrets — this is mandatory
# See docs/runbooks/rotate-secrets.md — Case B emergency flow
# Every T0 and T1 secret must be regenerated and re-populated in Vault
```

### Option C — Contact HashiCorp Support

For Vault Enterprise subscribers, HashiCorp offers professional services for key recovery. This is not applicable to Vault OSS.

## Emergency Recovery Options

### Emergency 1 — Vault Storage Corrupted

Symptoms: `vault status` shows `Initialized: false` or storage errors in logs.

```bash
# Check storage integrity
docker compose -f docker/compose.prod.yml exec vault ls -lh /vault/file/
docker compose -f docker/compose.prod.yml logs vault | tail -50

# If storage files are missing or truncated:
# Option A (backup recovery) is the only path. Follow the backup procedure above.
# If no backup exists, you must re-initialize (Option B above).
```

### Emergency 2 — Root Token Lost

```bash
# Generate a new root token using unseal keys
docker compose -f docker/compose.prod.yml exec vault vault operator generate-root -init
docker compose -f docker/compose.prod.yml exec vault vault operator generate-root
# Enter unseal key 1
docker compose -f docker/compose.prod.yml exec vault vault operator generate-root
# Enter unseal key 2
# ... until threshold reached
# New root token will be output
```

### Emergency 3 — AppRole Authentication Broken

If vault-agent cannot authenticate but Vault is unsealed:

```bash
# Log in as root
docker compose -f docker/compose.prod.yml exec vault vault login
# Enter root token

# Check AppRole status
docker compose -f docker/compose.prod.yml exec vault vault read auth/approle/role/searcher-rs

# Re-issue secret IDs
docker compose -f docker/compose.prod.yml exec vault vault write -f auth/approle/role/searcher-rs/secret-id
docker compose -f docker/compose.prod.yml exec vault vault write -f auth/approle/role/api-server/secret-id
# ... repeat per service

# Write to secret files and restart vault-agent
docker compose -f docker/compose.prod.yml restart vault-agent
```

## Post-Unseal Verification Checklist

- [ ] `vault status` shows `Sealed: false`
- [ ] `vault-agent` logs show successful authentication and template rendering
- [ ] `/run/secrets/arbx.env` exists and is non-empty
- [ ] All service containers show `STATUS: Up` and are healthy
- [ ] `/status` endpoint returns `ok: true` for all services
- [ ] `/api/v1/readiness` returns all 17 checks passing (score: 17/17)
- [ ] Grafana dashboard is accessible and showing data
- [ ] Prometheus is scraping all targets

## Post-Incident

After any unseal event, the following must be completed within 1 hour:

1. **Audit log review**: Check `/vault/logs/audit.log` for unauthorized access attempts during the sealed period.
2. **Root token rotation**: If the root token was used, generate a new one immediately.
3. **Incident report**: Create `docs/incidents/YYYY-MM-DD-vault-unseal-<reason>.md`.
4. **Backup validation**: Verify the most recent encrypted backup is recoverable. If not, trigger an immediate backup.

## Key Holder Responsibilities

| Responsibility | Detail |
|---------------|--------|
| **Storage** | Key shares must be stored offline (paper in safe, HSM, or encrypted USB in safe deposit box) |
| **Sharing** | Never transmit a key share over unencrypted channels |
| **Availability** | Each key holder must respond to an unseal request within 30 minutes during business hours, 2 hours outside |
| **Rotation** | After any re-key event, destroy old shares completely (shred paper, wipe USB) |
| **Compromise** | If a key holder suspects their share is compromised, they must report immediately. A re-key is then mandatory. |

## Related

- ADR-003: Vault Secrets Management
- `docs/adr/003-vault-secrets-management.md`
- `docs/runbooks/vault-sealed.md` (legacy reference)
- `docs/runbooks/rotate-secrets.md`
- `docs/operations/SECRETS_POLICY.md`
- `docs/operations/VAULT_SETUP.md`
