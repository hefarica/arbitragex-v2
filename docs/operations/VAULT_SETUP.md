# INF-9 — HashiCorp Vault Bootstrap Guide

Vault is deployed as a Docker Compose service. It starts **sealed** after every
restart. The operator holds the unseal keys and MUST complete the unseal flow
before any service can read secrets.

Auto-unseal is intentionally not configured. Requiring manual unseal is a
security property: if the VPS is compromised and the container restarts, Vault
stays sealed until an operator with key shards intervenes.

---

## PREREQUISITE — Generate TLS certificates (N4, audit 2026-05-10)

`compose.prod.yml` requires TLS. The cert files are never committed to git
(gitignored under `monitoring/vault/tls/`). Run this **once on the VPS**
before the first `docker compose up`:

```bash
# From the repo root on the VPS:
bash monitoring/vault/generate-tls.sh
```

This generates a self-signed CA plus a 365-day server cert valid for the
`vault` Docker DNS name, `localhost`, and `127.0.0.1`. The CA private key is
deleted after signing — only the CA cert, server cert, and server key remain.

**Certificate rotation** (before expiry or on compromise):

```bash
rm monitoring/vault/tls/vault-{cert,key,ca}.pem
bash monitoring/vault/generate-tls.sh
docker compose -f docker/compose.prod.yml restart vault
# Re-run the unseal flow (Vault re-seals on every restart).
```

CLI access from the VPS or from within other containers must use `VAULT_CACERT`
pointing at the CA cert, or pass `-tls-skip-verify` for operator one-off
commands (acceptable inside SSH sessions; never in automated service code):

```bash
export VAULT_ADDR=https://127.0.0.1:8200
export VAULT_CACERT=/opt/arbitragex-v2/monitoring/vault/tls/vault-ca.pem
docker exec -e VAULT_ADDR -e VAULT_CACERT arbitragex-v2-vault-1 vault status
```

---

## First-time initialization

```bash
# 1. Generate TLS certs first (prod only — see PREREQUISITE section above).
bash monitoring/vault/generate-tls.sh

# 2. Start Vault only (other services do not depend on it yet).
docker compose -f docker/compose.dev.yml up -d vault
# or for production (requires TLS certs from step 1):
docker compose -f docker/compose.prod.yml up -d vault

# 3. Initialize. This is a ONE-TIME operation.
#    Vault will print 5 unseal key shards and a root token.
#    Save ALL of them immediately — they are shown ONCE and cannot be recovered.
#    In prod, pass VAULT_CACERT so the CLI can verify the server cert.
VAULT_CACERT=monitoring/vault/tls/vault-ca.pem \
  docker exec -e VAULT_CACERT -it arbitragex-v2-vault-1 vault operator init

# Output example (DO NOT commit these values anywhere):
#   Unseal Key 1: <shard-1>
#   Unseal Key 2: <shard-2>
#   Unseal Key 3: <shard-3>
#   Unseal Key 4: <shard-4>
#   Unseal Key 5: <shard-5>
#   Initial Root Token: hvs.<token>
```

Store the 5 key shards in separate, secure locations (e.g., separate password
manager entries, physically separate devices). The default threshold is 3 of 5:
any 3 shards are sufficient to unseal.

---

## Unsealing after every restart

Vault is sealed on every restart. Provide 3 different key shards:

```bash
# prod: Vault now serves HTTPS. Pass VAULT_CACERT to avoid TLS errors.
export VAULT_ADDR=https://127.0.0.1:8200
export VAULT_CACERT=/opt/arbitragex-v2/monitoring/vault/tls/vault-ca.pem

docker exec -e VAULT_ADDR -e VAULT_CACERT -it arbitragex-v2-vault-1 vault operator unseal <shard-1>
docker exec -e VAULT_ADDR -e VAULT_CACERT -it arbitragex-v2-vault-1 vault operator unseal <shard-2>
docker exec -e VAULT_ADDR -e VAULT_CACERT -it arbitragex-v2-vault-1 vault operator unseal <shard-3>
# Container healthcheck now returns 200.
```

Verify sealed status at any time:

```bash
docker exec -e VAULT_ADDR=https://127.0.0.1:8200 \
            -e VAULT_CACERT=/vault/tls/vault-ca.pem \
            arbitragex-v2-vault-1 vault status
```

---

## Login and secrets setup

```bash
# Login with root token (only for initial setup; rotate to AppRole tokens after).
docker exec -e VAULT_ADDR=https://127.0.0.1:8200 \
            -e VAULT_CACERT=/vault/tls/vault-ca.pem \
            -it arbitragex-v2-vault-1 vault login <root-token>

# Enable KV v2 secrets engine at path `arbx/`.
docker exec -it arbitragex-v2-vault-1 vault secrets enable -path=arbx kv-v2

# Migrate secrets from .env into Vault.
docker exec -it arbitragex-v2-vault-1 vault kv put arbx/relay \
  flashbots_signer_key=0x...

docker exec -it arbitragex-v2-vault-1 vault kv put arbx/admin \
  token=<ARBX_ADMIN_TOKEN>

docker exec -it arbitragex-v2-vault-1 vault kv put arbx/db \
  url=postgres://arbx_rw:<password>@postgres:5432/arbitragex \
  readonly_url=postgres://arbx_ro:<password>@postgres:5432/arbitragex

docker exec -it arbitragex-v2-vault-1 vault kv put arbx/rpc \
  ws_1=wss://eth-mainnet.g.alchemy.com/v2/<key> \
  http_1=https://eth-mainnet.g.alchemy.com/v2/<key>
```

---

## UI access

Vault UI is available at `https://127.0.0.1:8200` on the VPS (TLS since N4).
Access via SSH tunnel from your workstation:

```bash
ssh -L 8200:127.0.0.1:8200 arbx
# Then open https://localhost:8200 in your browser.
# Your browser will warn about the self-signed cert — accept the exception,
# or import monitoring/vault/tls/vault-ca.pem into your browser's trust store.
```

---

## Service wiring (next sprint)

Wiring Rust services to read secrets from Vault via AppRole tokens is deferred
to the follow-up sprint. The current flow remains: services read from `.env`
at container start. Vault is operational and ready for the wiring sprint.

Steps for the wiring sprint:
1. Create AppRole for each service (`vault auth enable approle`).
2. Write policy granting read-only access to `arbx/<service>/*`.
3. Generate role-id + secret-id per service.
4. Update Dockerfiles to fetch secrets from Vault at startup using
   `vault agent` or a lightweight sidecar.

---

## Security notes

- TLS is enabled in `compose.prod.yml` (N4 audit 2026-05-10). `tls_disable`
  has been removed. Self-signed cert is generated by `generate-tls.sh` and
  mounted read-only into the container. The cert directory is gitignored.
- Vault is bound to `127.0.0.1:8200` only. Operator access is via SSH tunnel.
- Rotate the root token after initial setup: create an admin AppRole, log in
  with it, then `vault token revoke <root-token>`.
- Never commit unseal keys, root tokens, or role secret-ids to git.
- Cert validity is 365 days. Set a calendar reminder to rotate before expiry.
  See "Certificate rotation" in the PREREQUISITE section above.
