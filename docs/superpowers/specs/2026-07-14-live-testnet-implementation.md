# LIVE_TESTNET Implementation Specification

**Date:** 2026-07-14  
**Status:** APPROVED  
**Scope:** Enable real testnet execution for ArbitrageX v2  
**Target:** Sepolia (chain_id: 11155111)  
**Constraint:** Mainnet (chain_id: 1) permanently blocked  

---

## 1. Executive Summary

This specification enables ArbitrageX v2 to execute real transactions on Ethereum testnets (starting with Sepolia), providing a production-like environment for validating the complete execution pipeline without mainnet capital risk.

### Key Decisions

| Decision | Value | Rationale |
|----------|-------|-----------|
| Testnet | Sepolia | Recommended by Ethereum Foundation; active maintenance |
| Execution Mode | LIVE_TESTNET | Distinct from PAPER/SHADOW; real signatures and broadcasts |
| Contracts | AAVex2 + Uniswap V3 | Battle-tested protocols; no custom deployment needed |
| Account Model | EOA + Contract + Funder | Separation of concerns; mimics production |
| Observability | Redis Streams + PostgreSQL | Transactional outbox; fail-open webhooks |
| CI/CD | ops-live-testnet.yml | Manual trigger; dry-run support; evidence collection |

---

## 2. Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                     DETECTION LAYER                              │
│              searcher-rs (WebSocket RPC)                        │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                     SELECTION LAYER                              │
│            selector-api (Token Safety + Scoring)                │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                     SIMULATION LAYER                             │
│              SimulatorV2 (REVM-based)                           │
│         - Real calldata execution                               │
│         - State diff tracking                                   │
│         - Validation gates                                      │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                     EXECUTION LAYER                              │
│              LiveTestnetExecutor (Rust)                         │
│         - Nonce management with locking                         │
│         - EIP-1559 transaction signing                          │
│         - RPC broadcast with fallback                           │
│         - Receipt tracking & reconciliation                     │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                     STATE LAYER                                  │
│  Redis Streams (Events) ◄──► PostgreSQL (Audit)                 │
│         ▲                              ▲                        │
│         │                              │                        │
│    Webhook Worker              Transactional Outbox             │
│    (Slack/Discord)                                              │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                     PRESENTATION LAYER                           │
│              Frontend (Next.js + SSE)                           │
│         - Real-time transaction lifecycle                       │
│         - Kill switch control                                   │
│         - Explorer integration                                  │
└─────────────────────────────────────────────────────────────────┘
```

---

## 3. Configuration

### 3.1 Profile: configs/live-testnet.toml

```toml
[system]
env = "live-testnet"
service_name_prefix = "arbx"
kill_switch_enabled_default = false

[risk]
max_revert_rate_pct = 5.0
max_execution_variance_pct = 20.0
min_token_safety_score = 70
simulation_required_for_new_routes = true
max_gas_price_gwei = 200.0
max_slippage_pct = 1.5

[execution]
private_only = false
paper_mode = false
shadow_only = false
live_exec_enabled = true
max_parallel_executions = 8
retry_limit = 2
target_block_offset = 1
max_inclusion_wait_blocks = 5
max_value_eth = 10.0
flashbots_submit_timeout_ms = 5000
priority_fee_increment_pct = 10

[live_testnet]
enabled = true
chain_ids = [11155111]

# Testnet RPC Configuration
rpc_urls = ["https://sepolia.infura.io/v3/${INFURA_API_KEY}"]
rpc_fallback_urls = ["https://rpc.ankr.com/eth_sepolia"]

# External Contracts (Sepolia)
flash_loan_pool_address = "0xDA10009cBd5D07dd0CcFe720666A55bE9f4b48e2"
router_v3_address = "0xE592427A0AEce92De3Edeee1F18E0157C05861564"
factory_v3_address = "0x1F98431c8aD98523631AE4a59f267346ea31F984"

# Gas Management
min_gas_balance = "5000000000000000000"           # 0.005 ETH
target_gas_balance = "10000000000000000000"       # 0.01 ETH
max_topup_per_transfer = "5000000000000000000"   # 0.005 ETH
max_daily_topup = "30000000000000000000"         # 0.03 ETH
gas_funder_enabled = true

# Execution Gates
nonce_locking_ttl = 300                            # 5 minutes
wait_for_confirmation_blocks = 2
receipt_check_interval_ms = 5000

[simulation]
provider = "revm"
validate_profit = false                            # Don't require real profit in testnet
validate_gas = true
validate_revert = true
validate_calldata = true
sim_timeout_ms = 5000
gas_limit_safety_factor = 1.3
max_slippage_for_pass_pct = 5.0

[event_streaming]
enabled = true
stream_prefix = "arbx:live-testnet"
max_stream_length = 10000
outbox_poll_interval_ms = 1000
outbox_batch_size = 100

[webhooks]
enabled = true
failure_mode = "continue"                          # "continue" | "block"
timeout_ms = 3000
max_retries = 8
base_delay_ms = 1000
max_delay_ms = 1800000                             # 30 minutes
backoff_multiplier = 2.0
dedup_ttl_seconds = 86400                          # 24 hours

[sse]
enabled = true
heartbeat_interval_seconds = 30
max_connections = 1000
```

### 3.2 Environment Variables

| Variable | Source | Description |
|----------|--------|-------------|
| `ARBX_TRADE_MODE` | Static | `live_testnet` |
| `ARBX_CONFIG_PROFILE` | Static | `live-testnet` |
| `ARBX_LIVE_EXEC_ENABLED` | Static | `true` |
| `ARBX_SIMULATOR_V2_READY` | Static | `true` |
| `ARBX_TESTNET_CHAIN_ID` | Static | `11155111` |
| `ARBX_TESTNET_SIGNER_PRIVATE_KEY` | GitHub Secret | EOA private key |
| `ARBX_TESTNET_SIGNER_ADDRESS` | Derived | EOA address |
| `ARBX_TESTNET_EXECUTOR_ADDRESS` | GitHub Secret | Contract address |
| `ARBX_TESTNET_GAS_FUNDER_ADDRESS` | GitHub Secret | Funder account |
| `SEPOLIA_RPC_URL` | GitHub Secret | Primary RPC endpoint |
| `SEPOLIA_RPC_FALLBACK_URL` | GitHub Secret | Fallback RPC |
| `INFURA_API_KEY` | GitHub Secret | RPC provider key |
| `SLACK_WEBHOOK_URL` | GitHub Secret | Optional notifications |
| `DISCORD_WEBHOOK_URL` | GitHub Secret | Optional notifications |

---

## 4. Security Controls

### 4.1 Hard Block: Mainnet Prevention

```rust
// backend/api-server/src/routes/live_testnet.rs
pub fn validate_chain_id(chain_id: u64) -> Result<(), LiveTestnetError> {
    if chain_id == 1 {
        return Err(LiveTestnetError::MainnetBlocked);
    }
    
    let allowed_testnets = [11155111, 421614, 11155420]; // Sepolia, Arb Sepolia, Op Sepolia
    if !allowed_testnets.contains(&chain_id) {
        return Err(LiveTestnetError::UnsupportedChainId(chain_id));
    }
    
    Ok(())
}
```

### 4.2 Kill Switch

- **Default:** DISARMED for LIVE_TESTNET
- **Activation:** `POST /admin/killswitch { enabled: true, reason: "..." }`
- **Effect:** Stops new signatures; pending transactions complete
- **Propagation:** < 100ms via Redis pub/sub

### 4.3 19 Functional Gates (Testnet-Appropriate)

| # | Gate | LIVE_TESTNET | LIVE_MAINNET |
|---|------|--------------|--------------|
| 1 | RPC available | Required | Required |
| 2 | WebSocket available | Required | Required |
| 3 | Chain ID correct | Required | Required |
| 4 | Contracts deployed | Required | Required |
| 5 | Bytecode present | Required | Required |
| 6 | Signer loaded | Required | Required |
| 7 | Signer has gas | Required | Required |
| 8 | SimulatorV2 ready | Required | Required |
| 9 | Calldata not empty | Required | Required |
| 10 | Plan hash valid | Required | Required |
| 11 | Executor active | Required | Required |
| 12 | Nonce available | Required | Required |
| 13 | Gas within limits | Required | Required |
| 14 | Kill switch disarmed | Required | Required |
| 15 | Redis available | Required | Required |
| 16 | PostgreSQL available | Required | Required |
| 17 | Execution queue available | Required | Required |
| 18 | Receipt tracker ready | Required | Required |
| 19 | Mainnet blocked | Required | Required |
| - | KMS/HSM | Not required | Required |
| - | Multi-sig | Not required | Required |
| - | Capital cap | Not required | Required |

---

## 5. State Machine

### 5.1 Transaction Lifecycle

```
┌───────────┐    ┌───────────┐    ┌───────────┐    ┌───────────┐
│ DETECTED  │───▶│  ENCODED  │───▶│ SIMULATED │───▶│  APPROVED │
└───────────┘    └───────────┘    └───────────┘    └─────┬─────┘
                                                          │
┌───────────┐    ┌───────────┐    ┌───────────┐          │
│  FAILED   │◀───│ REVERTED  │◀───│  INCLUDED │◀─────────┘
└───────────┘    └─────┬─────┘    └─────┬─────┘
                       │                │
┌───────────┐          │         ┌──────▼──────┐
│  DROPPED  │◀─────────┴─────────│  CONFIRMED  │
└───────────┘                    └──────┬──────┘
┌───────────┐                           │
│ REPLACED  │◀──────────────────────────┤
└───────────┘                           │
┌───────────┐                    ┌──────▼──────┐
│ REORGED   │◀───────────────────│  FINALIZED  │
└─────┬─────┘                    └──────┬──────┘
      │                                 │
      └─────────────────────────────────┘
                                        │
                                 ┌──────▼──────┐
                                 │ RECONCILED  │
                                 └─────────────┘
```

### 5.2 State Transitions

| From | To | Trigger | Timeout |
|------|-----|---------|---------|
| DETECTED | ENCODED | Route calculated | 5s |
| ENCODED | SIMULATED | SimulatorV2 complete | 10s |
| SIMULATED | APPROVED | All gates pass | 2s |
| APPROVED | QUEUED | Executor available | 30s |
| QUEUED | SIGNING | Nonce acquired | 5s |
| SIGNING | SIGNED | EIP-1559 signature | 5s |
| SIGNED | SUBMITTED | RPC broadcast | 10s |
| SUBMITTED | PENDING | Mempool acceptance | 30s |
| PENDING | INCLUDED | Block inclusion | 120s |
| INCLUDED | CONFIRMED | N confirmations | 60s |
| CONFIRMED | FINALIZED | Finality reached | 300s |
| FINALIZED | RECONCILED | Balance updated | 10s |

---

## 6. Implementation Components

### 6.1 SimulatorV2 (REVM-Based)

**Location:** `backend/sim-ctl/src/simulator_v2/`

**Key Features:**
- Fork state at specific block number
- Execute calldata against real contract bytecode
- Track state diffs (balances, storage)
- Calculate gas consumption
- Capture revert reasons
- Validate execution gates

**Input:**
```rust
pub struct SimulationRequest {
    pub plan_hash: B256,
    pub calldata: Bytes,
    pub targets: Vec<Address>,
    pub values: Vec<U256>,
    pub chain_id: u64,
    pub block_number: u64,
    pub block_hash: B256,
    pub gas_limit: u64,
    pub sender: Address,
}
```

**Output:**
```rust
pub struct SimulationResult {
    pub success: bool,
    pub gas_used: u64,
    pub gas_refunded: u64,
    pub logs: Vec<Log>,
    pub output: Bytes,
    pub state_changes: StateDiff,
    pub revert_reason: Option<String>,
    pub balance_changes: Vec<BalanceChange>,
    pub simulation_time_ms: u64,
}
```

### 6.2 LiveTestnetExecutor

**Location:** `backend/relays-client/src/executor/`

**Responsibilities:**
- Opportunity queue consumption
- Nonce management with locking
- EIP-1559 transaction construction
- Local wallet signing
- RPC broadcast with fallback
- Receipt polling and tracking
- Automatic reconciliation

**Nonce Management:**
```rust
pub struct NonceManager {
    provider: Arc<Provider<Http>>,
    address: Address,
    next_nonce: U256,
    locked_nonces: VecDeque<(U256, Instant)>,
    lock_ttl: Duration,  // 5 minutes
}
```

**Idempotency:**
```rust
// Plan hash deduplication in Redis
SET idempotency:{plan_hash} "1" NX EX 86400
```

### 6.3 Event Streaming

**Transactional Outbox Pattern:**

1. Executor updates state in PostgreSQL (within transaction)
2. Executor inserts event into `event_outbox` table (same transaction)
3. Commit transaction
4. Outbox Worker polls unpublished events
5. Publisher pushes to Redis Stream
6. Mark as published in outbox

**Redis Stream Structure:**
```
Stream: arbx:live-testnet:events:11155111
Entry: { "event_id": "uuid", "event_type": "TX_INCLUDED", ... }
Consumer Groups: webhook-worker, sse-server
```

### 6.4 Frontend Dashboard

**Location:** `frontend/app/live-testnet/`

**Features:**
- LIVE_TESTNET mode badge
- Real-time transaction lifecycle (SSE)
- Chain info (Sepolia, RPC status)
- Signer info (address, balance)
- Kill switch control
- Explorer links
- Emergency stop button

**Mainnet Block:**
```typescript
if (chainId === 1) {
  throw new Error("MAINNET_BLOCKED_IN_TESTNET_PHASE");
}
```

---

## 7. Testing Requirements

### 7.1 Success Criteria

| Metric | Target | Minimum |
|--------|--------|---------|
| Transactions executed | 50 | 50 |
| Inclusion rate | 90% | 80% |
| Reconciliation rate | 100% | 100% |
| System uptime | 99% | 95% |
| Avg execution time | < 30s | < 60s |
| Zero mainnet attempts | 100% | 100% |
| Zero double submissions | 100% | 100% |

### 7.2 Test Matrix

#### Happy Path (LT-001 to LT-004)
- [ ] LT-001: Complete transaction lifecycle (detect → reconcile)
- [ ] LT-002: Flash loan arbitrage (borrow → swap → repay)
- [ ] LT-003: Multi-leg arbitrage (2+ DEXes)
- [ ] LT-004: Auto-reconciliation post-execution

#### Error Handling (LT-101 to LT-107)
- [ ] LT-101: Reverted transaction (slippage exceeded)
- [ ] LT-102: Insufficient gas balance
- [ ] LT-103: Nonce conflict (race condition)
- [ ] LT-104: Gas limit exceeded
- [ ] LT-105: Deadline expired
- [ ] LT-106: Invalid calldata
- [ ] LT-107: Contract not deployed

#### Infrastructure (LT-201 to LT-206)
- [ ] LT-201: RPC primary down → fallback
- [ ] LT-202: Redis temporarily unavailable
- [ ] LT-203: PostgreSQL temporarily unavailable
- [ ] LT-204: Receipt delayed (> 2 minutes)
- [ ] LT-205: Executor restart mid-flight
- [ ] LT-206: Double submission blocked (idempotency)

#### Security (LT-301 to LT-304)
- [ ] LT-301: Mainnet chain_id=1 rejected
- [ ] LT-302: Kill switch activation stops execution
- [ ] LT-303: Plan hash mismatch detected
- [ ] LT-304: Unsupported chain_id rejected

### 7.3 Playwright E2E Tests

**Location:** `tests/e2e/live-testnet/`

```typescript
// Critical path test
test("LT-001: Complete transaction lifecycle", async () => {
  // 1. Verify LIVE_TESTNET mode
  // 2. Inject test opportunity
  // 3. Monitor SSE events
  // 4. Verify state sequence
  // 5. Verify database records
  // 6. Verify explorer link
});
```

---

## 8. Deployment Procedure

### 8.1 Prerequisites

1. Contracts deployed on Sepolia
2. Test tokens minted and funded
3. Signer account with gas balance (>= 0.01 ETH)
4. GitHub Secrets configured
5. VPS environment ready

### 8.2 Activation

```bash
# Via GitHub Actions (recommended)
gh workflow run ops-live-testnet.yml \
  --field dry_run=false \
  --field chain_id=11155111 \
  --field duration_minutes=60 \
  --field max_tx_count=50
```

### 8.3 Verification

```bash
# 1. Check mode
curl $API_URL/api/v1/readiness/decision | jq '.mode'
# Expected: "LIVE_TESTNET"

# 2. Check kill switch
curl $API_URL/status | jq '.kill_switch_enabled'
# Expected: false

# 3. Check mainnet blocked
curl -X POST $API_URL/admin/config/live-testnet \
  -d '{"enabled":true,"chain_id":1}'
# Expected: 403 MAINNET_BLOCKED

# 4. Verify SSE stream
curl $API_URL/live-testnet/events?chain_id=11155111
# Expected: Event stream
```

### 8.4 Deactivation

```bash
# Normal shutdown
curl -X POST $API_URL/admin/config/live-testnet \
  -H "x-arbx-admin-token: $ADMIN_TOKEN" \
  -d '{"enabled":false,"chain_id":11155111}'

# Emergency kill switch
curl -X POST $API_URL/admin/killswitch \
  -H "x-arbx-admin-token: $ADMIN_TOKEN" \
  -d '{"enabled":true,"reason":"Emergency stop"}'
```

---

## 9. Monitoring & Observability

### 9.1 Metrics

| Metric | Type | Alert Threshold |
|--------|------|-----------------|
| `arbx_live_testnet_opportunities_detected` | Counter | - |
| `arbx_live_testnet_transactions_submitted` | Counter | - |
| `arbx_live_testnet_transactions_included` | Counter | - |
| `arbx_live_testnet_transactions_reverted` | Counter | > 10% rate |
| `arbx_live_testnet_execution_duration_ms` | Histogram | p99 > 60s |
| `arbx_live_testnet_gas_used_avg` | Gauge | - |
| `arbx_live_testnet_rpc_latency_ms` | Histogram | p99 > 5s |

### 9.2 Logs

**Structured JSON format:**
```json
{
  "timestamp": "2026-07-14T10:30:00Z",
  "level": "INFO",
  "service": "relays-client",
  "event_type": "TX_SUBMITTED",
  "plan_hash": "0x...",
  "tx_hash": "0x...",
  "chain_id": 11155111,
  "execution_mode": "LIVE_TESTNET",
  "correlation_id": "uuid"
}
```

### 9.3 Alerts

| Condition | Severity | Channel |
|-----------|----------|---------|
| Transaction reverted | WARNING | Slack |
| RPC fallback activated | WARNING | Slack |
| Kill switch activated | CRITICAL | Slack + PagerDuty |
| Mainnet attempt blocked | CRITICAL | Slack + Security |
| Inclusion rate < 80% | WARNING | Slack |
| System downtime > 1 min | CRITICAL | Slack + PagerDuty |

---

## 10. Appendix

### A. Contract Addresses (Sepolia)

| Contract | Address | Purpose |
|----------|---------|---------|
| AAVex2 FlashLoan Pool | `0xDA10009cBd5D07dd0CcFe720666A55bE9f4b48e2` | Flash loans |
| Uniswap V3 Router | `0xE592427A0AEce92De3Edeee1F18E0157C05861564` | Swaps |
| Uniswap V3 Factory | `0x1F98431c8aD98523631AE4a59f267346ea31F984` | Pool creation |
| WETH9 | `0x7b79995e5f793A07Bc00c21412e50Ecae098E7f9` | Wrapped ETH |

### B. RPC Endpoints

| Provider | URL | Priority |
|----------|-----|----------|
| Infura | `https://sepolia.infura.io/v3/${INFURA_API_KEY}` | Primary |
| Ankr | `https://rpc.ankr.com/eth_sepolia` | Fallback 1 |
| Alchemy | `https://eth-sepolia.g.alchemy.com/v2/${ALCHEMY_KEY}` | Fallback 2 |

### C. Explorer Links

- Sepolia Etherscan: `https://sepolia.etherscan.io`
- API: `https://api-sepolia.etherscan.io/api`

### D. Faucets

- Infura Sepolia Faucet: `https://www.infura.io/faucet/sepolia`
- Alchemy Sepolia Faucet: `https://sepoliafaucet.com`

---

## 11. Sign-off

| Role | Responsibility | Status |
|------|---------------|--------|
| Architecture | System design | APPROVED |
| Security | Risk assessment | PENDING |
| Operations | Deployment & monitoring | PENDING |
| QA | Testing & validation | PENDING |

---

## 12. References

- [ADR-001: Paper Mode Architecture](../../adr/001-paper-mode-architecture.md)
- [ADR-002: Kill-Switch Fail-Closed](../../adr/002-kill-switch-fail-closed.md)
- [OMEGA Pipeline Architecture](../../omega/pipeline-architecture.md)
- [Wallet Security Matrix](../../agent-sync/WALLET_SECURITY.md)

---

*Last updated: 2026-07-14*  
*Version: 1.0*  
*Status: APPROVED FOR IMPLEMENTATION*
