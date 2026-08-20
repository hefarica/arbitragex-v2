use ethers::providers::{Middleware, Provider, Ws};
use ethers::types::{H160, H256, U256 as EthersU256};
use revm::{
    bytecode::Bytecode,
    primitives::{Address, B256, U256},
    state::AccountInfo,
    Database,
};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::runtime::Handle;
use tracing::{debug, warn};

/// Error type for the LazyRpcDatabase Database impl.
///
/// revm 42 (revm-database-interface) requires the `Database::Error` associated
/// type to implement `DBErrorMarker`. `String` is a foreign type so we cannot
/// impl the foreign trait on it (orphan rule); this newtype is the local
/// carrier. In practice every method below returns `Ok(...)` and never
/// materializes an `Err`, so this type exists solely to satisfy the trait
/// bound — it is constructible but never constructed by this impl.
#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct LazyDbError(pub String);

impl revm::database_interface::DBErrorMarker for LazyDbError {}

/// A Database for REVM that lazily fetches state from an RPC using ethers-rs.
/// It caches results locally in memory to avoid repeated network calls.
pub struct LazyRpcDatabase {
    pub provider: Arc<Provider<Ws>>,
    pub accounts: HashMap<Address, AccountInfo>,
    pub contracts: HashMap<B256, Bytecode>,
    pub storage: HashMap<Address, HashMap<U256, U256>>,
    pub block_hashes: HashMap<u64, B256>,
}

impl LazyRpcDatabase {
    pub fn new(provider: Arc<Provider<Ws>>) -> Self {
        Self {
            provider,
            accounts: HashMap::new(),
            contracts: HashMap::new(),
            storage: HashMap::new(),
            block_hashes: HashMap::new(),
        }
    }

    // Helper conversions between alloy (revm) and ethers types
    fn alloy_to_ethers_addr(addr: Address) -> H160 {
        H160::from(addr.into_array())
    }

    fn ethers_to_alloy_u256(u: EthersU256) -> U256 {
        let mut bytes = [0u8; 32];
        u.to_big_endian(&mut bytes);
        U256::from_be_bytes(bytes)
    }

    fn alloy_to_ethers_u256(u: U256) -> EthersU256 {
        EthersU256::from_big_endian(&u.to_be_bytes::<32>())
    }
}

impl Database for LazyRpcDatabase {
    type Error = LazyDbError;

    fn basic(&mut self, address: Address) -> Result<Option<AccountInfo>, Self::Error> {
        if let Some(acc) = self.accounts.get(&address) {
            return Ok(Some(acc.clone()));
        }

        let ethers_addr = Self::alloy_to_ethers_addr(address);
        let provider = self.provider.clone();

        // Block inside sync trait
        let (balance, nonce, code) = tokio::task::block_in_place(|| {
            Handle::current().block_on(async {
                let bal: EthersU256 = provider
                    .get_balance(ethers_addr, None)
                    .await
                    .unwrap_or_default();
                let non: EthersU256 = provider
                    .get_transaction_count(ethers_addr, None)
                    .await
                    .unwrap_or_default();
                let cod: ethers::types::Bytes = provider
                    .get_code(ethers_addr, None)
                    .await
                    .unwrap_or_default();
                (bal, non, cod)
            })
        });

        // Convert ethers::types::Bytes into bytes::Bytes then into revm::Bytecode
        let code_bytes = Bytecode::new_raw(code.0.into());
        let code_hash = code_bytes.hash_slow();

        let info = AccountInfo {
            balance: Self::ethers_to_alloy_u256(balance),
            nonce: nonce.as_u64(),
            code_hash,
            code: Some(code_bytes.clone()),
            ..Default::default()
        };

        self.accounts.insert(address, info.clone());
        self.contracts.insert(code_hash, code_bytes);

        debug!(event = "lazy_db.fetch_basic", address = %address, "Fetched account info from RPC");

        Ok(Some(info))
    }

    fn code_by_hash(&mut self, code_hash: B256) -> Result<Bytecode, Self::Error> {
        if let Some(code) = self.contracts.get(&code_hash) {
            return Ok(code.clone());
        }
        // If not found in cache, it's problematic because we can't query RPC by code_hash directly.
        // Usually, `basic` is called first, which populates `contracts`.
        warn!(event = "lazy_db.code_not_found", code_hash = %code_hash, "Code hash not found in local cache");
        Ok(Bytecode::new())
    }

    fn storage(&mut self, address: Address, index: U256) -> Result<U256, Self::Error> {
        if let Some(acc_storage) = self.storage.get(&address) {
            if let Some(val) = acc_storage.get(&index) {
                return Ok(*val);
            }
        }

        let ethers_addr = Self::alloy_to_ethers_addr(address);
        let ethers_idx = Self::alloy_to_ethers_u256(index);
        let ethers_idx_h256 = {
            let mut b = [0u8; 32];
            ethers_idx.to_big_endian(&mut b);
            H256::from(b)
        };

        let provider = self.provider.clone();

        let storage_val = tokio::task::block_in_place(|| {
            Handle::current().block_on(async {
                provider
                    .get_storage_at(ethers_addr, ethers_idx_h256, None)
                    .await
                    .unwrap_or_default()
            })
        });

        let alloy_val = U256::from_be_bytes(storage_val.to_fixed_bytes());

        self.storage
            .entry(address)
            .or_default()
            .insert(index, alloy_val);

        debug!(event = "lazy_db.fetch_storage", address = %address, index = %index, "Fetched storage slot from RPC");

        Ok(alloy_val)
    }

    fn block_hash(&mut self, _number: u64) -> Result<B256, Self::Error> {
        // Just return a dummy or mocked block hash to avoid extra RPC calls unless strictly necessary
        Ok(B256::ZERO)
    }
}
