# RPC Provider Migration — Option 5 (Public RPCs + Hybrid Path)

**Date**: 2026-05-13
**Author**: workspace-verified during P2-c + A.8 + A.6 cycle
**Status**: Phase 5a applied on VPS · Phase 5b deferred to milestone trigger
**Scope**: Paper-mode only · zero capital · zero live submission

---

## 1. Why this migration

Alchemy multi-chain projection at current usage: **$900-1,800/mo for 6 chains**. Current burn (1 chain heavy): $300/mo. The platform is in paper-mode pre-A.4, so we don't need premium provider features (private mempool, dedicated nodes). Migration goal: zero waste, full capability preserved.

Trade-off matrix lived in `2026-05-13-rpc-providers-matrix.md` (this spec is the **implementation record** of Option 5).

## 2. Scaling triggers (operator-defined)

This migration is **temporary infrastructure** until the platform earns its way to better tools:

| Milestone | Action |
|-----------|--------|
| **$100 real profit** (live mainnet, post-A.9 sign-off) | Re-evaluate provider tier |
| **$1,000 paper trade profit** (A.5 paper-shadow cumulative) | Evaluate Phase 5b (Hetzner Reth mainnet) or jump to Option 3 (QuickNode+dRPC) |
| **$10,000 paper trade profit** | Evaluate Option 1 (Reth Fleet) for full multi-chain élite |

Below those thresholds, Option 5 is the right cost/capability ratio. Above, the platform earns the right to better infra.

## 3. Phase 5a — Applied 2026-05-13

### 3.1 What changed on VPS `.env`

All entries are **public URLs only** (no keys, no secrets). Multi-provider failover via `HttpRpcPool` (CSV `name=url,name=url`):

| Chain ID | HTTP providers | WS providers |
|----------|----------------|--------------|
| 1 (Ethereum) | publicnode + cloudflare + drpc + lava (already configured pre-migration) | publicnode + drpc |
| 10 (Optimism) | publicnode + drpc + mainnet.optimism.io | publicnode + drpc |
| 137 (Polygon) | publicnode + drpc + polygon-rpc.com | publicnode + drpc |
| 42161 (Arbitrum) | publicnode + drpc + arb1.arbitrum.io | publicnode + drpc |
| 8453 (Base) | publicnode + drpc + mainnet.base.org | publicnode + drpc |
| 56 (BSC) | publicnode + drpc + bsc-dataseed.binance.org | publicnode + drpc |

**Mempool**: `ARBX_MEMPOOL_MODE=filtered` with 12-router allowlist (UniV2/V3/V4 Universal/Permit2 + Sushi + Curve + 1inch + 0x + Balancer V2 + Pancake V3).

**Interval tuning**: `GAS_ORACLE_INTERVAL_MS=15000`, `RPC_HEALTH_INTERVAL_MS=15000`.

**Flashbots Protect URL**: `FLASHBOTS_RELAY_URL=https://rpc.flashbots.net` (informational; paper-mode does not submit).

### 3.2 What did NOT change

- `configs/app.toml` — operator has stash@{0} on it; respected
- TOML `[[chains]]` blocks with `enabled = false` for L2s — left as-is
- No code changes; pure env reconfiguration
- No backend services rebuilt; only `searcher-rs` recreated to pick up new env

### 3.3 Verification post-deploy

- ✅ `rpc_pool.ready` chain 1 with 3 providers alive (cloudflare dropped on boot — 429 rate limit, expected)
- ✅ `chain_client.subscribed_filtered allowlist_size=12` (filtered mempool active)
- ✅ `worker_orchestrator.gas_oracle_started interval_ms=15000`
- ✅ `/api/v1/risk/circuit-breakers/status` reports `rpc_health_breaker: PASS` with `9 providers configured, 9 alive`
- ✅ 16/16 containers UP, only `searcher-rs` recreated

### 3.4 What this delivers

| Capability | Before | After |
|------------|--------|-------|
| Alchemy dependency | implicit (could fall back) | **explicit removal** — no Alchemy keys in env |
| RPC provider count per chain | 4 (chain 1 only) | 2-4 per chain × 6 chains = ~15 endpoints |
| Mempool mode | Auto (could fall to firehose) | **filtered** with explicit allowlist |
| Gas oracle frequency | 10s | 15s |
| RPC health frequency | 5s | 15s |
| Monthly cost | ~$300 (Alchemy historical) | **~$0** (public RPCs free + existing Hetzner VPS) |
| Capability cost | — | preserved (paper-mode does not need premium features) |

### 3.5 What this does NOT deliver

- **Multi-chain active scanning**: `configs/app.toml` has chains 10/137/42161/8453 with `enabled = false`. Even with override `ARBX_ENABLED_CHAINS=...`, the TOML restriction wins. Operator must edit `[[chains]]` blocks to enable.
- **L2 pool data**: PG `pools` table has 60 rows for chain 1, 0 for L2s. Even if chains are enabled, workers boot blind. Operator must seed pool catalog per chain.
- **Archive on L2 public RPCs**: most L2 public endpoints don't expose `eth_call` at historical blocks. A.4 fork validation on L2 chains will fail. (Ethereum mainnet OK via publicnode archive.)
- **Private mempool L2**: Flashbots Protect is Ethereum-only. L2 MEV requires MEV-share/Eden/per-chain solutions.

## 4. Phase 5b — Deferred (operator-triggered)

**Trigger**: $1,000 paper trade profit OR explicit decision to run A.4 fork validation on mainnet.

**Action**: provision Hetzner CX52 (~$50/mo) with Reth + Lighthouse for Ethereum mainnet. Switch `RPC_HTTP_1` first provider to local Reth (`http://<reth-private-ip>:8545`). Public RPCs remain as failover.

**Effort**: ~1-2 weeks (1 week sync, ~1 day setup/verification).

**Gain**: sub-10ms latency Ethereum, zero rate limits, full archive, p2p mempool subscription option.

**Setup script outline** (for when operator is ready):
```bash
# On new Hetzner CX52:
apt update && apt install -y docker.io docker-compose
mkdir -p /opt/reth/data /opt/lighthouse/data
# docker-compose with reth + lighthouse, exposing 8545 (HTTP) + 8546 (WS)
# Wait 5-7 days for full sync (archive mode)
# On VPS arbx, edit .env:
#   RPC_HTTP_1=local=http://<reth-ip>:8545,publicnode=https://...,drpc=https://...
#   RPC_WS_1=local=ws://<reth-ip>:8546,publicnode=wss://...
# Restart searcher-rs
```

## 5. Operator next-action checklist

In priority order:

1. **Disable Alchemy beacon endpoints** in Alchemy dashboard (3 endpoints "none in 7 days", zero use, surface attack reduction)
2. **Pay & rotate Alchemy key** if desired (the $50.01 invoice is historical; current bill = $0)
3. **Seed pool catalog for L2s** via admin endpoint or migration (prerequisite for multi-chain scanning)
4. **Edit `configs/app.toml`** to flip `enabled = true` for chains you actually want to scan (after seeding pools)
5. **Pop stash@{0}** when operator decides on `env` setting (development vs production-like)

## 6. Reversibility

- `.env.bak.<timestamp>` preserved on VPS at `/opt/arbitragex-v2/.env.bak.20260513_161429`
- To revert: `cp .env.bak.20260513_161429 .env && docker compose up -d searcher-rs`
- Zero code changes — pure config

## 7. Cost projection

| Scenario | Monthly cost | Notes |
|----------|--------------|-------|
| **Phase 5a (current, paper-mode chain 1 only)** | **~$0** | Public RPCs, existing VPS already paid |
| Phase 5a + L2 pools seeded + enabled | ~$0 | Public RPCs still free; rate limit at ~10 RPS per L2 |
| Phase 5b (Reth mainnet added) | ~$50/mo | +1 Hetzner CX52 for Reth |
| Phase 5b + L2 paid endpoints (QuickNode/dRPC) | ~$99-149/mo | When L2 public rate limits bite |
| Phase 5b + Reth fleet (Option 1) | ~$420/mo | Full sovereignty |

---

## Appendix A — Verified public RPC URLs (2026-05-13)

All endpoints tested for `eth_chainId` response. Some may rate-limit under sustained load.

### Ethereum Mainnet (chain 1) — archive available

- `https://ethereum-rpc.publicnode.com`
- `https://eth.drpc.org`
- `https://eth.merkle.io` (best archive depth)
- `https://eth.llamarpc.com`
- `https://eth1.lava.build`

### Optimism (10)

- `https://optimism-rpc.publicnode.com`
- `https://optimism.drpc.org`
- `https://mainnet.optimism.io` (official)

### Polygon (137)

- `https://polygon-bor-rpc.publicnode.com`
- `https://polygon.drpc.org`
- `https://polygon-rpc.com` (official)

### Arbitrum (42161)

- `https://arbitrum-one-rpc.publicnode.com`
- `https://arbitrum.drpc.org`
- `https://arb1.arbitrum.io/rpc` (official sequencer)

### Base (8453)

- `https://base-rpc.publicnode.com`
- `https://base.drpc.org`
- `https://mainnet.base.org` (official Coinbase)

### BSC (56)

- `https://bsc-rpc.publicnode.com`
- `https://bsc.drpc.org`
- `https://bsc-dataseed.binance.org` (official Binance)

### Mempool / MEV

- `https://rpc.flashbots.net` — Flashbots Protect (Ethereum, paper-mode informational)
- Flashbots MEV-Share: `https://mev-share.flashbots.net` (when A.7 implemented)

## Appendix B — Canonical Ethereum mainnet router allowlist (12 addresses)

```
0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D  # Uniswap V2 Router 02
0xE592427A0AEce92De3Edee1F18E0157C05861564  # Uniswap V3 SwapRouter
0x68b3465833fb72A70ecDF485E0e4C7bD8665Fc45  # Uniswap V3 SwapRouter02
0x3fC91A3afd70395Cd496C647d5a6CC9D4B2b7FAD  # Uniswap V3 Universal Router
0x66a9893cc07d91d95644aedd05d03f95e1dba8af  # Uniswap V4 Universal Router
0x000000000022D473030F116dDEE9F6B43aC78BA3  # Permit2
0xd9e1cE17f2641f24aE83637ab66a2cca9C378B9F  # SushiSwap Router
0x99a58482BD75cbab83b27EC03CA68fF489b5788f  # Curve Router
0x1111111254EEB25477B68fb85Ed929f73A960582  # 1inch Aggregation V5
0xDef1C0ded9bec7F1a1670819833240f027b25EfF  # 0x Exchange Proxy
0xBA12222222228d8Ba445958a75a0704d566BF2C8  # Balancer V2 Vault
0x13f4EA83D0bd40E75C8222255bc855a974568Dd4  # PancakeSwap V3 (mainnet)
```

All public, all on-chain visible. No secrets.
