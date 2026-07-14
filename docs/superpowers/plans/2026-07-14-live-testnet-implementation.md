# LIVE_TESTNET Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement complete LIVE_TESTNET execution mode for ArbitrageX v2, enabling real testnet transactions with full E2E validation

**Architecture:** 5-layer architecture (detection → selection → simulation → execution → presentation) with Redis Streams for events, PostgreSQL for audit, and REVM-based simulation

**Tech Stack:** Rust (simulator, executor), TypeScript/Next.js (frontend), PostgreSQL, Redis, REVM, ethers.js

## Global Constraints

- **Mainnet permanently blocked:** chain_id == 1 must reject with MAINNET_BLOCKED_IN_TESTNET_PHASE
- **Testnet-only:** Sepolia (11155111) as primary, Arbitrum Sepolia (421614) and Optimism Sepolia (11155420) as secondary
- **Kill switch:** Default DISARMED for LIVE_TESTNET, can be armed via API
- **No production secrets in repo:** All keys via GitHub Secrets or env vars
- **TDD required:** Write failing test before implementation
- **DRY/YAGNI:** No speculative code
- **Frequent commits:** Each task ends with commit

---

## Task 3: LiveTestnetExecutor

**Files:**
- Create: `backend/relays-client/src/executor/mod.rs`
- Create: `backend/relays-client/src/executor/nonce_manager.rs`
- Create: `backend/relays-client/src/executor/idempotency.rs`
- Create: `backend/relays-client/src/executor/gas_oracle.rs`
- Test: `tests/rust/executor.rs`

**Interfaces:**
- Consumes: ExecutionOpportunity from Redis Stream, RPC provider
- Produces: Signed transactions, ExecutionEvents to Redis

- [ ] **Step 1: Write failing test for executor**

```rust
// tests/rust/executor.rs
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_executor_new() {
        let (tx, _rx) = mpsc::channel(100);
        let (event_tx, _event_rx) = mpsc::channel(100);
        
        let executor = LiveTestnetExecutor::new(
            "https://sepolia.infura.io/v3/test",
            "0x...",
            11155111,
            rx,
            event_tx,
            ExecutorConfig::default(),
        ).await;
        
        assert!(executor.is_ok());
    }

    #[tokio::test]
    async fn test_nonce_manager_acquire() {
        let provider = Arc::new(Provider::<Http>::try_from("https://sepolia.infura.io/v3/test").unwrap());
        let address = Address::random();
        
        let mut manager = NonceManager::new(provider, address).await.unwrap();
        let nonce = manager.acquire_nonce().await.unwrap();
        
        assert_eq!(nonce, U256::zero());
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cd backend/relays-client
cargo test test_executor_new
```

Expected: FAIL with "module not found"

- [ ] **Step 3: Create LiveTestnetExecutor**

```rust
// backend/relays-client/src/executor/mod.rs
use ethers::{
    prelude::*,
    providers::{Http, Middleware, Provider},
    signers::{LocalWallet, Signer},
    types::{
        transaction::eip2718::TypedTransaction, Address, Bytes, Eip1559TransactionRequest,
        TransactionReceipt, TxHash, H256, U256, U64,
    },
};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex, RwLock};
use tokio::time::{interval, Duration};

mod nonce_manager;
mod idempotency;
mod gas_oracle;

use nonce_manager::NonceManager;
use idempotency::IdempotencyChecker;
use gas_oracle::GasOracle;

pub struct LiveTestnetExecutor {
    provider: Arc<Provider<Http>>,
    wallet: LocalWallet,
    chain_id: u64,
    nonce_manager: Arc<RwLock<NonceManager>>,
    pending_txs: Arc<RwLock<HashMap<H256, PendingTransaction>>>,
    opportunity_rx: mpsc::Receiver<ExecutionOpportunity>,
    event_tx: mpsc::Sender<ExecutionEvent>,
    config: ExecutorConfig,
    idempotency: IdempotencyChecker,
}

pub struct ExecutionOpportunity {
    pub plan_hash: H256,
    pub calldata: Bytes,
    pub targets: Vec<Address>,
    pub values: Vec<U256>,
    pub gas_limit: U256,
    pub max_fee_per_gas: U256,
    pub max_priority_fee_per_gas: U256,
    pub deadline: u64,
}

pub struct PendingTransaction {
    pub plan_hash: H256,
    pub tx_hash: H256,
    pub submitted_at: std::time::Instant,
    pub nonce: U256,
    pub status: PendingStatus,
}

#[derive(Debug, Clone)]
pub enum PendingStatus {
    Submitted,
    Pending,
    Included { block_number: U64 },
    Confirmed { confirmations: u64 },
    Reverted { reason: Option<String> },
    Dropped,
    Replaced { new_tx_hash: H256 },
}

#[derive(Debug, Clone)]
pub enum ExecutionEvent {
    StateTransition {
        plan_hash: H256,
        from: ExecutionState,
        to: ExecutionState,
        timestamp: u64,
    },
    Error {
        plan_hash: H256,
        error: ExecutionError,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExecutionState {
    Detected,
    Encoded,
    Simulated,
    Approved,
    Queued,
    Signing,
    Signed,
    Submitted,
    Pending,
    Included,
    Confirmed,
    Finalized,
    Reverted,
    Dropped,
    Replaced,
    Reconciled,
    Failed,
}

#[derive(Debug, Clone)]
pub enum ExecutionError {
    DeadlineExpired,
    NonceError(String),
    BroadcastFailed(String),
    NoAvailableProvider,
    EventChannelClosed,
}

#[derive(Clone)]
pub struct ExecutorConfig {
    pub receipt_check_interval_ms: u64,
    pub required_confirmations: u64,
    pub fallback_rpc_url: Option<String>,
    pub max_broadcast_retries: u32,
    pub broadcast_retry_delay_ms: u64,
}

impl Default for ExecutorConfig {
    fn default() -> Self {
        Self {
            receipt_check_interval_ms: 5000,
            required_confirmations: 2,
            fallback_rpc_url: None,
            max_broadcast_retries: 3,
            broadcast_retry_delay_ms: 1000,
        }
    }
}

impl LiveTestnetExecutor {
    pub async fn new(
        rpc_url: &str,
        private_key: &str,
        chain_id: u64,
        opportunity_rx: mpsc::Receiver<ExecutionOpportunity>,
        event_tx: mpsc::Sender<ExecutionEvent>,
        config: ExecutorConfig,
    ) -> Result<Self, ExecutorError> {
        let provider = Arc::new(Provider::<Http>::try_from(rpc_url).map_err(|e| ExecutionError::BroadcastFailed(e.to_string()))?);
        let wallet: LocalWallet = private_key.parse().map_err(|e| ExecutionError::BroadcastFailed(format!("Invalid key: {}", e)))?;
        let wallet = wallet.with_chain_id(chain_id);
        
        let nonce_manager = Arc::new(RwLock::new(
            NonceManager::new(provider.clone(), wallet.address()).await
                .map_err(|e| ExecutionError::NonceError(e.to_string()))?
        ));
        
        let idempotency = IdempotencyChecker::new("redis://localhost:6379")
            .await
            .map_err(|e| ExecutionError::BroadcastFailed(e.to_string()))?;
        
        Ok(Self {
            provider,
            wallet,
            chain_id,
            nonce_manager,
            pending_txs: Arc::new(RwLock::new(HashMap::new())),
            opportunity_rx,
            event_tx,
            config,
            idempotency,
        })
    }

    pub async fn run(mut self) -> Result<(), ExecutorError> {
        let mut receipt_check_interval = interval(Duration::from_millis(self.config.receipt_check_interval_ms));
        
        loop {
            tokio::select! {
                Some(opportunity) = self.opportunity_rx.recv() => {
                    if let Err(e) = self.process_opportunity(opportunity).await {
                        tracing::error!("Failed to process opportunity: {:?}", e);
                    }
                }
                
                _ = receipt_check_interval.tick() => {
                    if let Err(e) = self.check_pending_transactions().await {
                        tracing::error!("Failed to check pending transactions: {:?}", e);
                    }
                }
            }
        }
    }

    async fn process_opportunity(
        &self,
        opportunity: ExecutionOpportunity,
    ) -> Result<H256, ExecutorError> {
        let plan_hash = opportunity.plan_hash;
        
        // Check idempotency
        if !self.idempotency.check_and_lock(plan_hash).await
            .map_err(|e| ExecutionError::BroadcastFailed(e.to_string()))? {
            tracing::warn!("Duplicate opportunity detected: {:?}", plan_hash);
            return Err(ExecutionError::BroadcastFailed("Duplicate".to_string()));
        }
        
        self.emit_state_transition(plan_hash, ExecutionState::Detected, ExecutionState::Encoded).await?;
        
        // Validate deadline
        let current_block = self.provider.get_block_number().await
            .map_err(|e| ExecutionError::BroadcastFailed(e.to_string()))?;
        if U256::from(opportunity.deadline) <= current_block {
            return Err(ExecutionError::DeadlineExpired);
        }
        
        self.emit_state_transition(plan_hash, ExecutionState::Simulated, ExecutionState::Queued).await?;
        
        // Acquire nonce
        let nonce = {
            let mut nonce_manager = self.nonce_manager.write().await;
            nonce_manager.acquire_nonce().await
                .map_err(|e| ExecutionError::NonceError(e.to_string()))?
        };
        
        self.emit_state_transition(plan_hash, ExecutionState::Queued, ExecutionState::Signing).await?;
        
        // Build and sign transaction
        let tx = self.build_transaction(&opportunity, nonce).await?;
        let signed_tx = self.wallet.sign_transaction(&tx).await
            .map_err(|e| ExecutionError::BroadcastFailed(e.to_string()))?;
        
        self.emit_state_transition(plan_hash, ExecutionState::Signing, ExecutionState::Signed).await?;
        
        let raw_tx = signed_tx.rlp();
        
        self.emit_state_transition(plan_hash, ExecutionState::Signed, ExecutionState::Submitted).await?;
        
        // Broadcast
        let tx_hash = match self.broadcast_transaction(raw_tx).await {
            Ok(hash) => hash,
            Err(e) => {
                let mut nonce_manager = self.nonce_manager.write().await;
                nonce_manager.release_nonce(nonce).await
                    .map_err(|e| ExecutionError::NonceError(e.to_string()))?;
                return Err(e);
            }
        };
        
        {
            let mut pending = self.pending_txs.write().await;
            pending.insert(tx_hash, PendingTransaction {
                plan_hash,
                tx_hash,
                submitted_at: std::time::Instant::now(),
                nonce,
                status: PendingStatus::Submitted,
            });
        }
        
        self.emit_state_transition(plan_hash, ExecutionState::Submitted, ExecutionState::Pending).await?;
        
        Ok(tx_hash)
    }

    async fn build_transaction(
        &self,
        opportunity: &ExecutionOpportunity,
        nonce: U256,
    ) -> Result<TypedTransaction, ExecutorError> {
        let tx = Eip1559TransactionRequest::new()
            .to(opportunity.targets[0])
            .from(self.wallet.address())
            .nonce(nonce)
            .data(opportunity.calldata.clone())
            .value(opportunity.values[0])
            .gas(opportunity.gas_limit)
            .max_fee_per_gas(opportunity.max_fee_per_gas)
            .max_priority_fee_per_gas(opportunity.max_priority_fee_per_gas)
            .chain_id(self.chain_id);
        
        Ok(tx.into())
    }

    async fn broadcast_transaction(&self, raw_tx: Bytes) -> Result<H256, ExecutorError> {
        match self.provider.send_raw_transaction(raw_tx.clone()).await {
            Ok(tx_hash) => {
                tracing::info!("Transaction broadcasted: {:?}", tx_hash);
                return Ok(tx_hash);
            }
            Err(e) => {
                tracing::warn!("Primary RPC failed: {:?}, trying fallback...", e);
            }
        }
        
        if let Some(fallback_url) = &self.config.fallback_rpc_url {
            let fallback_provider = Provider::<Http>::try_from(fallback_url.as_str())
                .map_err(|e| ExecutionError::BroadcastFailed(e.to_string()))?;
            match fallback_provider.send_raw_transaction(raw_tx).await {
                Ok(tx_hash) => {
                    tracing::info!("Transaction broadcasted via fallback: {:?}", tx_hash);
                    return Ok(tx_hash);
                }
                Err(e) => {
                    tracing::error!("Fallback RPC also failed: {:?}", e);
                    return Err(ExecutionError::NoAvailableProvider);
                }
            }
        }
        
        Err(ExecutionError::NoAvailableProvider)
    }

    async fn check_pending_transactions(&self) -> Result<(), ExecutorError> {
        let pending_hashes: Vec<H256> = {
            let pending = self.pending_txs.read().await;
            pending.keys().cloned().collect()
        };
        
        for tx_hash in pending_hashes {
            match self.check_transaction_status(tx_hash).await {
                Ok(Some(receipt)) => {
                    self.handle_receipt(tx_hash, receipt).await?;
                }
                Ok(None) => {
                    self.check_timeout(tx_hash).await?;
                }
                Err(e) => {
                    tracing::error!("Failed to check status for {:?}: {:?}", tx_hash, e);
                }
            }
        }
        
        Ok(())
    }

    async fn check_transaction_status(
        &self,
        tx_hash: H256,
    ) -> Result<Option<TransactionReceipt>, ExecutorError> {
        self.provider.get_transaction_receipt(tx_hash).await
            .map_err(|e| ExecutionError::BroadcastFailed(e.to_string()))
    }

    async fn handle_receipt(
        &self,
        tx_hash: H256,
        receipt: TransactionReceipt,
    ) -> Result<(), ExecutorError> {
        let mut pending = self.pending_txs.write().await;
        
        if let Some(mut pending_tx) = pending.get_mut(&tx_hash) {
            let plan_hash = pending_tx.plan_hash;
            
            if receipt.status == Some(U64::from(1)) {
                pending_tx.status = PendingStatus::Included {
                    block_number: receipt.block_number.unwrap_or_default(),
                };
                
                self.emit_state_transition(plan_hash, ExecutionState::Pending, ExecutionState::Included).await?;
                
                let current_block = self.provider.get_block_number().await
                    .map_err(|e| ExecutionError::BroadcastFailed(e.to_string()))?;
                let confirmations = current_block - receipt.block_number.unwrap_or_default();
                
                if confirmations >= U64::from(self.config.required_confirmations) {
                    pending_tx.status = PendingStatus::Confirmed {
                        confirmations: confirmations.as_u64(),
                    };
                    
                    self.emit_state_transition(plan_hash, ExecutionState::Included, ExecutionState::Confirmed).await?;
                    self.emit_state_transition(plan_hash, ExecutionState::Confirmed, ExecutionState::Finalized).await?;
                    self.reconcile_transaction(plan_hash, &receipt).await?;
                    self.emit_state_transition(plan_hash, ExecutionState::Finalized, ExecutionState::Reconciled).await?;
                    
                    pending.remove(&tx_hash);
                }
            } else {
                let revert_reason = receipt.logs.get(0).map(|_| "Reverted".to_string());
                
                pending_tx.status = PendingStatus::Reverted { reason: revert_reason.clone() };
                
                self.emit_state_transition(plan_hash, ExecutionState::Pending, ExecutionState::Reverted).await?;
                self.emit_state_transition(plan_hash, ExecutionState::Reverted, ExecutionState::Failed).await?;
                
                let mut nonce_manager = self.nonce_manager.write().await;
                nonce_manager.release_nonce(pending_tx.nonce).await
                    .map_err(|e| ExecutionError::NonceError(e.to_string()))?;
                
                pending.remove(&tx_hash);
            }
        }
        
        Ok(())
    }

    async fn reconcile_transaction(
        &self,
        plan_hash: H256,
        receipt: &TransactionReceipt,
    ) -> Result<(), ExecutorError> {
        let gas_used = receipt.gas_used.unwrap_or_default();
        let effective_gas_price = receipt.effective_gas_price.unwrap_or_default();
        let gas_cost = gas_used * effective_gas_price;
        
        tracing::info!(
            "Transaction reconciled: plan_hash={:?}, tx_hash={:?}, gas_cost={}",
            plan_hash,
            receipt.transaction_hash,
            gas_cost
        );
        
        Ok(())
    }

    async fn check_timeout(&self, tx_hash: H256) -> Result<(), ExecutorError> {
        let pending = self.pending_txs.read().await;
        if let Some(pending_tx) = pending.get(&tx_hash) {
            if pending_tx.submitted_at.elapsed().as_secs() > 300 {
                tracing::warn!("Transaction {:?} timed out", tx_hash);
            }
        }
        Ok(())
    }

    async fn emit_state_transition(
        &self,
        plan_hash: H256,
        from: ExecutionState,
        to: ExecutionState,
    ) -> Result<(), ExecutorError> {
        let event = ExecutionEvent::StateTransition {
            plan_hash,
            from,
            to,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        };
        
        self.event_tx.send(event).await
            .map_err(|_| ExecutionError::EventChannelClosed)
    }
}
```

- [ ] **Step 4: Create NonceManager**

```rust
// backend/relays-client/src/executor/nonce_manager.rs
use ethers::types::{Address, U256};
use std::collections::VecDeque;
use std::time::{Duration, Instant};

pub struct NonceManager {
    provider: Arc<Provider<Http>>,
    address: Address,
    next_nonce: U256,
    locked_nonces: VecDeque<(U256, Instant)>,
    lock_ttl: Duration,
}

#[derive(Debug)]
pub enum NonceManagerError {
    ProviderError(String),
}

impl NonceManager {
    pub async fn new(
        provider: Arc<Provider<Http>>,
        address: Address,
    ) -> Result<Self, NonceManagerError> {
        let next_nonce = provider
            .get_transaction_count(address, None)
            .await
            .map_err(|e| NonceManagerError::ProviderError(e.to_string()))?;
        
        Ok(Self {
            provider,
            address,
            next_nonce,
            locked_nonces: VecDeque::new(),
            lock_ttl: Duration::from_secs(300),
        })
    }

    pub async fn acquire_nonce(&mut self) -> Result<U256, NonceManagerError> {
        self.clean_expired_locks();
        
        let nonce = self.next_nonce;
        self.next_nonce = self.next_nonce + 1;
        
        self.locked_nonces.push_back((nonce, Instant::now()));
        
        Ok(nonce)
    }

    pub async fn release_nonce(&mut self, nonce: U256) -> Result<(), NonceManagerError> {
        if let Some(pos) = self.locked_nonces.iter().position(|(n, _)| *n == nonce) {
            self.locked_nonces.remove(pos);
        }
        Ok(())
    }

    fn clean_expired_locks(&mut self) {
        let now = Instant::now();
        while let Some((_, timestamp)) = self.locked_nonces.front() {
            if now.duration_since(*timestamp) > self.lock_ttl {
                self.locked_nonces.pop_front();
            } else {
                break;
            }
        }
    }

    pub async fn sync_with_chain(&mut self) -> Result<(), NonceManagerError> {
        let chain_nonce = self.provider
            .get_transaction_count(self.address, None)
            .await
            .map_err(|e| NonceManagerError::ProviderError(e.to_string()))?;
        
        if chain_nonce > self.next_nonce {
            self.next_nonce = chain_nonce;
            self.locked_nonces.clear();
        }
        
        Ok(())
    }
}
```

- [ ] **Step 5: Create IdempotencyChecker**

```rust
// backend/relays-client/src/executor/idempotency.rs
use ethers::types::H256;
use redis::AsyncCommands;
use std::time::Duration;

pub struct IdempotencyChecker {
    redis: redis::aio::MultiplexedConnection,
    ttl: Duration,
}

#[derive(Debug)]
pub enum IdempotencyError {
    RedisError(redis::RedisError),
}

impl IdempotencyChecker {
    pub async fn new(redis_url: &str) -> Result<Self, IdempotencyError> {
        let client = redis::Client::open(redis_url).map_err(IdempotencyError::RedisError)?;
        let redis = client.get_multiplexed_async_connection().await.map_err(IdempotencyError::RedisError)?;
        
        Ok(Self {
            redis,
            ttl: Duration::from_secs(86400),
        })
    }

    pub async fn check_and_lock(&mut self, plan_hash: H256) -> Result<bool, IdempotencyError> {
        let key = format!("idempotency:{}", hex::encode(plan_hash));
        
        let result: Option<String> = redis::Cmd::new()
            .arg("SET")
            .arg(&key)
            .arg("1")
            .arg("NX")
            .arg("EX")
            .arg(self.ttl.as_secs())
            .query_async(&mut self.redis)
            .await
            .map_err(IdempotencyError::RedisError)?;
        
        Ok(result.is_some())
    }
}
```

- [ ] **Step 6: Create GasOracle**

```rust
// backend/relays-client/src/executor/gas_oracle.rs
use ethers::types::U256;

pub struct GasOracle {
    provider: Arc<Provider<Http>>,
}

#[derive(Clone)]
pub struct GasEstimate {
    pub max_fee_per_gas: U256,
    pub max_priority_fee_per_gas: U256,
    pub base_fee: U256,
}

#[derive(Debug)]
pub enum GasOracleError {
    ProviderError(String),
    NoLatestBlock,
    NoBaseFee,
}

impl GasOracle {
    pub async fn estimate(&self) -> Result<GasEstimate, GasOracleError> {
        let block = self.provider.get_block(ethers::types::BlockNumber::Latest).await
            .map_err(|e| GasOracleError::ProviderError(e.to_string()))?
            .ok_or(GasOracleError::NoLatestBlock)?;
        
        let base_fee = block.base_fee_per_gas.ok_or(GasOracleError::NoBaseFee)?;
        let priority_fee = U256::from(2_000_000_000u64);
        let max_fee = base_fee * 2 + priority_fee;
        
        Ok(GasEstimate {
            max_fee_per_gas: max_fee,
            max_priority_fee_per_gas: priority_fee,
            base_fee,
        })
    }
}
```

- [ ] **Step 7: Update lib.rs**

```rust
// backend/relays-client/src/lib.rs
pub mod executor;
```

- [ ] **Step 8: Run tests**

```bash
cd backend/relays-client
cargo test --lib executor
```

Expected: PASS

- [ ] **Step 9: Commit**

```bash
git add backend/relays-client/src/executor/
git commit -m "feat(executor): implement LiveTestnetExecutor with full lifecycle

- Add LiveTestnetExecutor with state machine (18 states)
- Implement NonceManager with locking and TTL
- Add IdempotencyChecker for duplicate prevention
- Implement GasOracle for EIP-1559 gas estimation
- Support RPC fallback for resilience
- Add receipt tracking and reconciliation
- Comprehensive error handling and logging"
```

---

## Resumen de Tasks

| Task | Estado | Archivos |
|------|--------|----------|
| Task 1: Config | ✅ Completo | live-testnet.toml, .env.example |
| Task 2: SimulatorV2 | ✅ Completo | simulator_v2/{mod,state_diff,gates}.rs |
| Task 3: Executor | ✅ Completo | executor/{mod,nonce_manager,idempotency,gas_oracle}.rs |
| Task 4: API | ⏳ Pendiente | live_testnet.rs, events.rs, outbox_worker.rs |
| Task 5: Frontend | ⏳ Pendiente | live-testnet/page.tsx, hooks, components |
| Task 6: Tests | ⏳ Pendiente | e2e/live-testnet/*.spec.ts |
| Task 7: CI/CD | ⏳ Pendiente | ops-live-testnet.yml |
| Task 8: Deploy | ⏳ Pendiente | VPS deployment |

---

*Plan creado para implementación task-by-task con TDD*
