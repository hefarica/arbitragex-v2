# INF-9 — HashiCorp Vault Bootstrap Guide

Vault is deployed as a Docker Compose service. It starts **sealed** after every
restart. The operator holds the unseal keys and MUST complete the unseal flow
before any service can read secrets.

Auto-unseal is intentionally not configured. Requiring manual unseal is a
security property: if the VPS is compromised and the container restarts, Vault
stays sealed until an operator with key shards intervenes.

---

## First-time initialization

```bash
# 1. Start Vault only (other services do not depend on it yet).
docker compose -f docker/compose.dev.yml up -d vault
# or for production:
docker compose -f docker/compose.prod.yml up -d vault

# 2. Initialize. This is a ONE-TIME operation.
#    Vault will print 5 unseal key shards and a root token.
#    Save ALL of them immediately — they are shown ONCE and cannot be recovered.
docker exec -it arbitragex-v2-vault-1 vault operator init

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
docker exec -it arbitragex-v2-vault-1 vault operator unseal <shard-1>
docker exec -it arbitragex-v2-vault-1 vault operator unseal <shard-2>
docker exec -it arbitragex-v2-vault-1 vault operator unseal <shard-3>
# Container healthcheck now returns 200.
```

Verify sealed status at any time:

```bash
docker exec arbitragex-v2-vault-1 vault status
```

---

## Login and secrets setup

```bash
# Login with root token (only for initial setup; rotate to AppRole tokens after).
docker exec -it arbitragex-v2-vault-1 vault login <root-token>

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

Vault UI is available at `http://127.0.0.1:8200` on the VPS. Access via SSH
tunnel from your workstation:

```bash
ssh -L 8200:127.0.0.1:8200 arbx
# Then open http://localhost:8200 in your browser.
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

- TLS is disabled in the current config (`tls_disable: 1`). This is acceptable
  while Vault is bound to loopback only and accessed via SSH tunnel. Enable TLS
  before any network exposure beyond localhost.
- Rotate the root token after initial setup: create an admin AppRole, log in
  with it, then `vault token revoke <root-token>`.
- Never commit unseal keys, root tokens, or role secret-ids to git.
