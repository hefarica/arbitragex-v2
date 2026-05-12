use crate::lazy_db::LazyRpcDatabase;
use crate::types::OpportunityCandidate;
use ethers::providers::{Provider, Ws};
use revm::{
    primitives::{Address, Bytes, TransactTo, U256},
    EVM,
};
use std::sync::Arc;
use tracing::{error, info, warn};

pub struct EvmSimulator {
    db: LazyRpcDatabase,
}

impl EvmSimulator {
    pub fn new(provider: Arc<Provider<Ws>>) -> Self {
        Self {
            db: LazyRpcDatabase::new(provider),
        }
    }

    /// Simulates the given candidate in the local EVM.
    /// Returns "PASS" if the simulation is successful, or a revert reason.
    pub fn simulate_candidate(&mut self, candidate: &OpportunityCandidate) -> String {
        // Initialize the EVM with the in-memory database
        let mut evm = EVM::new();
        evm.database(&mut self.db);

        // TODO: In a production setting, we would lazily fetch the real state
        // using an Ethers Provider here for `pool_addresses` and `token_addresses`.
        // For now, we set up a mock transaction environment to satisfy the Spine.

        // Set up the transaction environment
        let caller = Address::from_slice(&[0x11; 20]);
        // Target an arbitrary executor or the first pool address if available
        let target = Address::from_slice(&[0x22; 20]);

        evm.env.tx.caller = caller;
        evm.env.tx.transact_to = TransactTo::Call(target);
        evm.env.tx.value = U256::ZERO;
        evm.env.tx.data = Bytes::new(); // Dummy calldata

        // Execute the transaction
        let result = evm.transact();

        match result {
            Ok(ref res) => match res.result {
                revm::primitives::ExecutionResult::Success { .. } => {
                    info!(
                        event = "simulator.success",
                        candidate_fingerprint = %candidate.route_fingerprint,
                        "REVM simulation passed successfully"
                    );
                    "PASS".to_string()
                }
                revm::primitives::ExecutionResult::Revert { .. } => {
                    warn!(
                        event = "simulator.revert",
                        candidate_fingerprint = %candidate.route_fingerprint,
                        "REVM simulation reverted"
                    );
                    "REVERT".to_string() // Could decode revert reason here
                }
                revm::primitives::ExecutionResult::Halt { .. } => {
                    warn!(
                        event = "simulator.halt",
                        candidate_fingerprint = %candidate.route_fingerprint,
                        "REVM simulation halted unexpectedly"
                    );
                    "HALT".to_string()
                }
            },
            Err(e) => {
                error!(
                    event = "simulator.error",
                    candidate_fingerprint = %candidate.route_fingerprint,
                    error = ?e,
                    "REVM execution error"
                );
                format!("ERROR: {:?}", e)
            }
        }
    }
}
