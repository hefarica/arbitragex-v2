# ADR-003: Vault Secrets Management

| Field | Value |
|-------|-------|
| **Status** | Accepted |
| **Date** | 2026-04-12 |
| **Author** | ArbitrageX Architecture Team |
| **Deciders** | Technical Lead, Security Officer, Operator |
| **Updated** | 2026-05-10 (audit M12) |

## Context

ArbitrageX v2 manages multiple classes of secrets that must be protected at rest and in transit:

| Secret Class | Examples | Compromise Impact |
|-------------|----------|-------------------|
| **T0 — Critical** | `FLASHBOTS_SIGNER_KEY`, `ARBX_ADMIN_TOKEN`, root seed phrase | Immediate fund loss or total system takeover |
| **T1 — High** | `DATABASE_URL`, `REDIS_URL`, `JWT_SECRET`, `ARBX_EDGE_TOKEN` | Data breach, unauthorized access, audit log tampering |
| **T2 — Medium** | RPC API keys, GoPlus API key, Tenderly credentials | Service degradation, rate limit exhaustion |
| **T3 — Low** | Grafana admin password, Slack webhook URL | Information disclosure, alert channel spam |

### Threat Model

1. **VPS compromise**: The production host (195.201.235.70) is a single VPS. Physical access is controlled by the provider; logical access must be hardened.
2. **Container escape**: A compromised service container could read environment variables or mounted files from other containers.
3. **Credential leakage in logs**: Secrets must never appear in application logs, Docker logs, or crash dumps.
4. **Rotation requirement**: After any personnel change or suspected breach, all T0/T1 secrets must be rotatable within 15 minutes.
5. **Bootstrap paradox**: The system that stores secrets must itself be secured, creating a chicken-and-egg problem at first boot.

## Decision

We will use **HashiCorp Vault** with the integrated file storage backend, TLS termination, and Shamir's Secret Sharing for unseal operations.

### Architecture

```mermaid
flowchart TB
    subgraph Host["VPS Host (195.201.235.70)"]
        subgraph DockerNet["Docker Bridge: arbx-net"]
            V["vault<br/>(HashiCorp Vault)"]
            VA["vault-agent<br/>(template renderer)"]
        end

        subgraph SecretsPath["/run/secrets/arbx/"]
            ENV["arbx.env<br/>(runtime secrets)"]
            SID1["searcher-rs.role-id<br/>searcher-rs.secret-id"]
            SID2["api-server.role-id<br/>api-server.secret-id"]
            SID3["... per service"]
        end

        subgraph KeyHolders["Unseal Key Holders"]
            K1["Keeper 1<br/>(offline share)"]
            K2["Keeper 2<br/>(offline share)"]
            K3["Keeper 3<br/>(offline share)"]
            K4["Keeper 4<br/>(offline share)"]
            K5["Keeper 5<br/>(offline share)"]
        end
    end

    subgraph Services["Application Services"]
        S1["searcher-rs"]
        S2["api-server"]
        S3["sim-ctl"]
        S4["relays-client"]
        S5["recon"]
        S6["selector-api"]
    end

    K1 -.->|"threshold 3"| V
    K2 -.->|"threshold 3"| V
    K3 -.->|"threshold 3"| V
    K4 -.->|"threshold 3"| V
    K5 -.->|"threshold 3"| V

    V -->|"AppRole auth"| VA
    VA -->|"template render"| ENV
    VA -->|"per-service creds"| SID1
    VA -->|"per-service creds"| SID2
    VA -->|"per-service creds"| SID3

    ENV -->|"--env-file"| S1
    ENV -->|"--env-file"| S2
    ENV -->|"--env-file"| S3
    SID1 -->|"Docker secret mount"| S1
    SID2 -->|"Docker secret mount"| S2
```

### Vault Configuration

| Parameter | Value | Rationale |
|-----------|-------|-----------|
| **Storage backend** | Integrated file (`/vault/file`) | No external dependency; survives single-node restart |
| **Unseal mechanism** | Shamir's Secret Sharing | No single point of failure; collusion-resistant |
| **Total shares** | 5 | Distributed across 5 individuals/offline locations |
| **Threshold** | 3 | Requires majority; prevents unilateral unseal by one compromised keeper |
| **TLS** | Enabled with self-signed or Let's Encrypt cert | Prevents plaintext secret transit on the Docker network |
| **Auth method** | AppRole | Machine authentication without human intervention post-unseal |
| **Secret engine** | KV v2 | Versioned secrets with full audit history |

### Boot Sequence

```mermaid
sequenceDiagram
    participant Op as Operator
    participant V as vault container
    participant VA as vault-agent
    participant S as Application Services

    Op->>V: docker compose start vault
    V-->>Op: Vault initialized: Sealed=true

    Op->>V: vault operator unseal (share 1)
    Op->>V: vault operator unseal (share 2)
    Op->>V: vault operator unseal (share 3)
    V-->>Op: Sealed=false

    VA->>V: AppRole auth
    V-->>VA: Client token
    VA->>V: Read secrets from KV v2
    V-->>VA: Secret data
    VA->>VA: Render templates to /run/secrets/arbx/

    Op->>S: docker compose up -d
    S->>VA: Read /run/secrets/arbx.env
    S->>S: Validate secrets (no placeholders)
    S-->>Op: All services healthy
```

### Secret Classification & Vault Paths

| Tier | Path Pattern | Access Pattern | Rotation SLA |
|------|-------------|----------------|-------------|
| T0 | `arbx/data/t0/flashbots_signer_key` | vault-agent only, never exposed to containers | 15 min |
| T0 | `arbx/data/t0/admin_token` | vault-agent → api-server env | 15 min |
| T1 | `arbx/data/t1/database_url` | vault-agent → all service env | 30 min |
| T1 | `arbx/data/t1/jwt_secret` | vault-agent → api-server + edge | 30 min |
| T2 | `arbx/data/t2/rpc_alchemy` | vault-agent → searcher-rs | 1 hour |
| T2 | `arbx/data/t2/tenderly_api_key` | vault-agent → sim-ctl | 1 hour |
| T3 | `arbx/data/t3/grafana_password` | vault-agent → grafana | 24 hours |

## Consequences

### Positive

- **No secrets in source control**: All credentials are externalized. The `.env.example` file contains only placeholder values like `REPLACE_ME`.
- **No secrets in container layers**: `docker history` on any image reveals no credential values. Secrets are mounted at runtime via bind mounts.
- **Audit trail**: Vault's audit device logs every secret read, write, and authentication attempt to `/vault/logs/audit.log`.
- **Dynamic credentials**: Vault can issue short-lived database credentials via the PostgreSQL secrets engine (future enhancement).
- **Emergency sealing**: A single `vault operator seal` command immediately revokes all active tokens, forcing a 3-of-5 unseal to restart. This is the nuclear option for suspected compromise.

### Negative

- **Bootstrap complexity**: Vault must be initialized and unsealed before any service can start. This adds ~5-10 minutes to disaster recovery.
- **Operational burden**: 3 of 5 key holders must be available for any unseal event. This is intentional but creates coordination overhead.
- **Single-node Vault**: Running Vault as a single container is not highly available. A host failure requires restoring from backup + full unseal ceremony.
- **Secret sprawl risk**: If operators bypass Vault and set env vars directly on containers, the security model breaks.

### Neutral

- **Vault resource footprint**: Vault requires ~256MB RAM and negligible CPU. It is not in the hot path of execution.
- **vault-agent sidecar**: The current design uses a separate vault-agent container that renders secrets to a shared volume. Services read from this volume; they never authenticate to Vault directly.

## Disaster Recovery

### Scenario A — Vault Restart (Intentional or OOM)

1. Vault container restarts in sealed state.
2. Three key holders run `vault operator unseal` with their shares.
3. vault-agent automatically re-authenticates and re-renders secrets.
4. Services that were waiting on secret files proceed to start.

### Scenario B — Host Failure (VPS Lost)

1. Provision new VPS with same Docker Compose stack.
2. Restore Vault file backend from latest encrypted backup (`age`-encrypted tarball).
3. Unseal with 3 of 5 shares.
4. If no backup exists: rotate **all** secrets (emergency flow in `rotate-secrets.md`).
5. Restart all services.

### Scenario C — Complete Key Loss

If all 5 unseal shares are lost, Vault data is **permanently unrecoverable**. Mitigation:
- Shares are distributed across 5 distinct physical/offline locations.
- At least 2 shares are in hardware security modules (HSM) or bank safe deposit boxes.
- Shares are generated with `vault operator init -key-shares=5 -key-threshold=3` and the output is split immediately.

## Related

- ADR-002: Kill-Switch Fail-Closed Design
- `docs/runbooks/vault-sealed.md`
- `docs/runbooks/vault-unseal.md` (this runbook)
- `docs/runbooks/rotate-secrets.md`
- `docs/operations/SECRETS_POLICY.md`
- `docs/operations/VAULT_SETUP.md`
