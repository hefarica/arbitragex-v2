---
id: R-SEC-001
title: Secret Rotation Procedure
severity: P1
duration: 20m
owner: Security Team / SRE
reviewed: 2025-01-15
---

# Secret Rotation Runbook

## Purpose

Rotate API keys, RPC credentials, wallet private keys, and admin tokens on a
scheduled basis or in response to a suspected compromise.

## Rotation Schedule

| Secret Type | Rotation Frequency | Owner |
|-------------|-------------------|-------|
| Admin tokens (`x-admin-token`) | Every 90 days | SRE |
| RPC endpoint credentials | Every 90 days | SRE |
| Wallet private keys | Every 180 days | Security |
| Database credentials | Every 90 days | SRE |
| TLS certificates | Before expiry (30d warning) | SRE |
| Docker registry tokens | Every 90 days | Platform |
| Third-party API keys | Per provider policy | Security |

---

## Preconditions

1. Access to HashiCorp Vault with `secret/arbitragex/*` write permissions.
2. `vault` CLI authenticated and unsealed.
3. `kubectl` access to the target Kubernetes cluster.
4. Maintenance window scheduled (P1 changes outside peak hours).
5. Kill-switch tested and ready (see [Kill-Switch Runbook](kill-switch.md)).

---

## Step 1 — Announce Maintenance

Post in #maintenance 15 minutes before starting:

```
:tools: Secret rotation starting in 15 minutes.
Affected: <list secret types>
Impact: Brief RPC reconnections, no trading halt expected
Window: <start> – <end> UTC
```

## Step 2 — Verify Current Secret Versions

```bash
vault kv list secret/arbitragex/
vault kv get -format=json secret/arbitragex/admin/prod | jq '.data.metadata'
```

Record current `version` and `created_time` for rollback reference.

## Step 3 — Rotate Admin Token

```bash
# Generate new token
NEW_ADMIN_TOKEN=$(openssl rand -hex 32)

# Store new version in Vault (creates version N+1)
vault kv put secret/arbitragex/admin/prod \
  token="${NEW_ADMIN_TOKEN}" \
  rotated_by="$(whoami)" \
  rotated_at="$(date -u +%Y-%m-%dT%H:%M:%SZ)"

# Verify write
vault kv get -field=token secret/arbitragex/admin/prod
```

## Step 4 — Update Kubernetes Secrets

```bash
# Patch the secret in the target namespace
kubectl create secret generic arbitragex-admin-token \
  --from-literal=token="${NEW_ADMIN_TOKEN}" \
  -n arbitragex-prod \
  --dry-run=client -o yaml | kubectl apply -f -

# Rollout restart to pick up the new secret
kubectl rollout restart deployment/arbitragex-api -n arbitragex-prod
kubectl rollout status deployment/arbitragex-api -n arbitragex-prod --timeout=120s
```

## Step 5 — Update Docker Secrets (if applicable)

```bash
# For Docker Swarm deployments
docker secret create arbitragex_admin_token_v2 - <<< "${NEW_ADMIN_TOKEN}"
# Update service to use new secret reference
docker service update \
  --secret-rm arbitragex_admin_token_v1 \
  --secret-add source=arbitragex_admin_token_v2,target=/run/secrets/admin_token \
  arbitragex_api
```

## Step 6 — Update .env Files (Development)

```bash
# Rotate local development token
sed -i 's/^ADMIN_TOKEN=.*/ADMIN_TOKEN='"${NEW_ADMIN_TOKEN}"'/' .env
# Verify
grep "^ADMIN_TOKEN=" .env
```

## Step 7 — Verify Health

```bash
# Wait for rollout to complete
sleep 10

# Verify readiness
curl -s http://localhost:8080/ready

# Verify new token works
curl -s -H "x-admin-token: ${NEW_ADMIN_TOKEN}" \
  http://localhost:8080/api/system/guard-state | jq .
```

Expected: HTTP 200, valid JSON response.

## Step 8 — Smoke Test

```bash
# Verify opportunities endpoint is accessible
curl -s -H "x-admin-token: ${NEW_ADMIN_TOKEN}" \
  "http://localhost:8080/api/opportunities?limit=1" | jq '.items | length'
```

Expected: `0` or `1` (endpoint responsive).

## Step 9 — Clean Up Old Versions

```bash
# Destroy old token version (Vault retains version history)
# DO NOT destroy the latest version
OLD_VERSION=$(vault kv get -format=json secret/arbitragex/admin/prod | jq '.data.metadata.version - 1')
vault kv destroy -versions="${OLD_VERSION}" secret/arbitragex/admin/prod
```

## Step 10 — Document Rotation

```bash
vault kv patch secret/arbitragex/admin/prod \
  rotation_log="$(vault kv get -format=json secret/arbitragex/admin/prod | jq -r '.data.data.rotation_log // empty'); $(date -u +%Y-%m-%dT%H:%M:%SZ) rotated by $(whoami)"
```

Post in #maintenance:

```
:white_check_mark: Secret rotation complete.
Rotated: <list>
Token version: <N>
Health checks: PASS
Status: CLOSED
```

---

## Rollback Procedure

If rotation causes issues:

1. Immediately restore the previous secret version in Vault:
   ```bash
   vault kv rollback -version=<N-1> secret/arbitragex/admin/prod
   ```
2. Re-run Step 4 (Kubernetes patch) with the restored token.
3. Verify health (Step 7).
4. Escalate to Security team before re-attempting.

## Success Criteria

- All targeted secrets have new values stored in Vault.
- Kubernetes secrets are updated and pods have restarted.
- Health probes return 200 OK after restart.
- Old token is rejected (HTTP 401) within 60 seconds of rollout.
- New token is accepted (HTTP 200) on all admin endpoints.
- Rotation is documented in Vault metadata and #maintenance channel.

## Related Runbooks

- [Kill-Switch](kill-switch.md) — Pre-rotation safety check
- [DR Drill](dr-drill.md) — Vault availability validation