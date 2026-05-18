---
title: Rotate RPC Endpoints
description: Safely rotate RPC endpoints with zero-downtime failover, validation, and rollback procedures.
tags: [rpc, infrastructure, failover]
---

# How to Rotate RPC Endpoints

This guide describes the procedure for rotating RPC endpoints used by ArbitrageX v2. RPC rotation is necessary when endpoints degrade, rate-limit responses, or become unavailable. The platform supports zero-downtime rotation with automatic health validation.

---

## When to Rotate

| Indicator | Threshold | Action |
|-----------|-----------|--------|
| Average RPC latency | > 200 ms sustained for 5 min | Rotate to fallback |
| Error rate | > 5% non-200 responses | Rotate immediately |
| Rate limit (429) responses | > 10 per minute | Rotate immediately |
| Stale block height | > 3 blocks behind network | Rotate immediately |
| Provider maintenance notice | Any scheduled window | Rotate proactively |
| Provider migration | Provider change | Rotate with validation |

Monitor these indicators in Grafana via the **RPC Health** dashboard at `http://localhost:3001/d/rpc-health`.

---

## Step 1: Add the New Endpoint

Edit `.env` and append the new RPC endpoint to the fallback chain:

```bash
# Add new endpoint (example: Alchemy)
AX_RPC_FALLBACK_3=https://eth-mainnet.g.alchemy.com/v2/<NEW_KEY>
```

Or replace the primary:

```bash
# Promote a fallback to primary
AX_RPC_PRIMARY=https://eth-mainnet.g.alchemy.com/v2/<NEW_KEY>
AX_RPC_FALLBACK_1=https://mainnet.infura.io/v3/<KEY>
```

Supported provider URL formats:

| Provider | URL Format |
|----------|-----------|
| Alchemy | `https://eth-mainnet.g.alchemy.com/v2/<API_KEY>` |
| Infura | `https://mainnet.infura.io/v3/<PROJECT_ID>` |
| QuickNode | `https://<SUBDOMAIN>.quiknode.pro/<TOKEN>/` |
| Ankr | `https://rpc.ankr.com/eth/<TOKEN>` |
| Custom Geth | `http://<IP>:8545` |
| Custom Erigon | `http://<IP>:8545` |

---

## Step 2: Validate the New Endpoint

Before bringing the endpoint into rotation, validate it:

```bash
cd ~/arbitragex-v2
./scripts/validate-rpc.sh <RPC_URL>
```

This script performs the following checks:

| Check | Command | Pass Criteria |
|-------|---------|---------------|
| Chain ID | `eth_chainId` | Returns `0x1` (mainnet) |
| Sync status | `eth_syncing` | Returns `false` |
| Block height | `eth_blockNumber` | Within 2 blocks of network |
| Gas price | `eth_gasPrice` | Non-zero, < 500 gwei |
| Archive support | `eth_getBalance` at block N-10000 | Returns historical state |
| Response time | 10 sequential calls | P99 < 150 ms |

Example output:

```
[validate-rpc] Chain ID ................. PASS (1)
[validate-rpc] Sync status .............. PASS (synced)
[validate-rpc] Block height ............. PASS (18945233, diff: 0)
[validate-rpc] Gas price ................ PASS (18.2 gwei)
[validate-rpc] Archive support .......... PASS
[validate-rpc] Response time (P99) ...... PASS (42 ms)
[validate-rpc] All checks passed. Endpoint ready for rotation.
```

Manual validation using `curl`:

```bash
curl -X POST <RPC_URL> \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"eth_chainId","params":[],"id":1}'
# Expected: {"jsonrpc":"2.0","id":1,"result":"0x1"}
```

---

## Step 3: Apply the Rotation

Restart the RPC router to pick up the new endpoint:

```bash
docker compose up -d ax-rpc-router
```

The RPC router reads `.env` on startup and distributes connections across configured endpoints using a weighted round-robin algorithm.

### Verify Rotation

```bash
# Check which endpoints are active
curl http://localhost:3000/api/v1/system/rpc-status | jq
```

Expected response:

```json
{
  "primary": {
    "url": "https://eth-mainnet.g.alchemy.com/v2/<NEW_KEY>",
    "healthy": true,
    "latency_ms": 38,
    "block_height": 18945233,
    "requests_per_minute": 142
  },
  "fallbacks": [
    {
      "url": "https://mainnet.infura.io/v3/<KEY>",
      "healthy": true,
      "latency_ms": 52,
      "block_height": 18945233,
      "requests_per_minute": 23
    }
  ]
}
```

---

## Step 4: Monitor After Rotation

Watch the RPC Health dashboard for 10 minutes after rotation:

```bash
# Watch RPC latency in real-time
watch -n 5 'curl -s http://localhost:3000/api/v1/system/rpc-status | jq ".primary.latency_ms"'
```

Key metrics to observe:

| Metric | Healthy Range | Action if Out of Range |
|--------|--------------|------------------------|
| Latency (P50) | 20–100 ms | Rotate if > 200 ms |
| Latency (P99) | < 300 ms | Rotate if > 500 ms |
| Error rate | < 0.1% | Rotate if > 1% |
| Block drift | 0–1 blocks | Rotate if > 3 blocks |
| Active connections | Balanced across endpoints | Rebalance if skewed |

---

## Rollback Procedure

If the new endpoint degrades after rotation, roll back to the previous endpoint:

### Automatic Rollback

The RPC router has automatic failover enabled by default. If an endpoint exceeds error thresholds for 30 seconds, it is automatically removed from rotation. When all endpoints fail, the router enters **degraded mode** and queues requests until an endpoint recovers.

### Manual Rollback

```bash
# Revert .env to previous configuration
git checkout .env

# Restart the router
docker compose up -d ax-rpc-router

# Verify
curl http://localhost:3000/api/v1/system/rpc-status | jq
```

### Emergency Rollback

If the system is in a critical state and RPC is completely unavailable:

```bash
# Use local emergency RPC configuration
cp .env .env.rotated
cp .env.backup .env
docker compose restart ax-rpc-router ax-strategy-eval ax-evm-exec-1 ax-evm-exec-2 ax-evm-exec-3
```

---

## Multi-Chain RPC Rotation

ArbitrageX v2 supports Ethereum, Arbitrum, and Base. Each chain has independent RPC configuration:

| Chain | Environment Variable | Example |
|-------|---------------------|---------|
| Ethereum | `AX_RPC_PRIMARY` | `https://eth-mainnet.g.alchemy.com/v2/...` |
| Ethereum Fallback | `AX_RPC_FALLBACK_1` | `https://mainnet.infura.io/v3/...` |
| Arbitrum | `AX_RPC_ARBITRUM` | `https://arb-mainnet.g.alchemy.com/v2/...` |
| Base | `AX_RPC_BASE` | `https://base-mainnet.g.alchemy.com/v2/...` |

Rotation for each chain follows the same procedure independently.

---

## Automation

For production deployments, automate rotation with a cron job:

```bash
# Add to crontab
crontab -e

# Check RPC health every 5 minutes, rotate if degraded
*/5 * * * * /home/arbitragex/arbitragex-v2/scripts/auto-rotate-rpc.sh >> /var/log/ax-rpc.log 2>&1
```

The `auto-rotate-rpc.sh` script:

1. Queries the health endpoint
2. Compares current latency against thresholds
3. Switches to the lowest-latency healthy fallback if the primary exceeds thresholds
4. Logs the rotation event
5. Sends an alert via webhook (optional)
