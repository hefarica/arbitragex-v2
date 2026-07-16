pub mod gas_oracle;
pub mod idempotency;
pub mod nonce_manager;

use ethers::{
    prelude::*,
    providers::{Http, Middleware, Provider},
    signers::{LocalWallet, Signer},
    types::{
        transaction::eip2718::TypedTransaction, Address, Bytes, Eip1559TransactionRequest,
        TransactionReceipt, TxHash, H256, U256, U64,
    },
};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tokio::time::{interval, Duration};

use nonce_manager::NonceManager;
use idempotency::IdempotencyChecker;

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
    IdempotencyError(String),
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

/// Execution receipt for a paper-shadow / testnet opportunity.
/// `tx_hash` is None because the executor never broadcasts in testnet mode.
#[derive(Debug, Clone)]
pub struct ExecutionReceipt {
    pub plan_hash: H256,
    pub status: &'static str,
    pub tx_hash: Option<H256>,
    pub gas_used: Option<U256>,
    pub error: Option<ExecutionError>,
}

impl LiveTestnetExecutor {
    /// Construct a paper-shadow executor. No broadcast keys are required.
    pub fn new(
        _provider: Arc<Provider<Http>>,
        _wallet: LocalWallet,
        chain_id: u64,
        opportunity_rx: mpsc::Receiver<ExecutionOpportunity>,
        event_tx: mpsc::Sender<ExecutionEvent>,
        config: ExecutorConfig,
    ) -> Self {
        Self {
            provider: _provider,
            wallet: _wallet,
            chain_id,
            nonce_manager: Arc::new(RwLock::new(NonceManager::new())),
            pending_txs: Arc::new(RwLock::new(HashMap::new())),
            opportunity_rx,
            event_tx,
            config,
            idempotency: IdempotencyChecker::new(),
        }
    }

    /// Safe testnet entry point. Always returns a paper-shadow receipt and
    /// never submits a transaction to the network.
    pub async fn execute_testnet_opportunity(
        &self,
        opportunity: &ExecutionOpportunity,
    ) -> Result<ExecutionReceipt, ExecutionError> {
        let _ = self
            .idempotency
            .check_or_insert(opportunity.plan_hash)
            .await
            .map_err(|e| ExecutionError::IdempotencyError(e.to_string()))?;

        let receipt = ExecutionReceipt {
            plan_hash: opportunity.plan_hash,
            status: "paper_shadow_noop",
            tx_hash: None,
            gas_used: None,
            error: None,
        };

        let _ = self
            .event_tx
            .send(ExecutionEvent::StateTransition {
                plan_hash: opportunity.plan_hash,
                from: ExecutionState::Approved,
                to: ExecutionState::Finalized,
                timestamp: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
            })
            .await
            .map_err(|_| ExecutionError::EventChannelClosed)?;

        Ok(receipt)
    }
}
