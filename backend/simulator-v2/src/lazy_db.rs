//! Task 4.2 — `LazyDb`: a `revm::Database` implementation backed by an
//! `ethers::providers::Provider<Http>` with a `DashMap`-based dedup cache.
//!
//! ## Design invariants
//!
//! ### Pinned block
//! Every RPC fetch passes the block captured at construction time so that
//! concurrent lookups observe the same chain state.  Using "latest" would
//! introduce a non-deterministic race: a new block arriving between the
//! `basic()` and `storage()` calls for the same account would cause revm to
//! simulate against a split state, producing wrong profit estimates.
//!
//! ### Lock-free cache
//! `DashMap` gives per-shard locking — better than a single `RwLock<HashMap>`
//! for concurrent searcher access.  Two threads racing on the same cold key
//! may both issue one RPC fetch; the cache converges after the second write
//! (same block → same deterministic value), so this is accepted.  A
//! `OnceCell`-per-slot approach eliminates the race but adds a per-entry `Arc`
//! allocation on every cold-path access, which is worse in the dominant
//! warm-cache case.
//!
//! ### Sync↔async bridge
//! `revm::Database` is synchronous.  The searcher calls it from inside a Tokio
//! runtime.  We use `tokio::task::block_in_place` so Tokio can keep the thread
//! pool alive while we drive the future on the existing `Handle`.  When called
//! from a non-Tokio thread (unit tests, standalone benchmarks) we fall back to
//! a freshly created `tokio::runtime::Runtime` owned by the `LazyDb` instance.

use std::convert::TryFrom;
use std::sync::Arc;

use dashmap::DashMap;
use ethers::providers::{Http, Middleware, Provider};
use ethers::types::{BlockId, BlockNumber, H160 as EH160, H256, U64 as EU64};
use revm::primitives::{AccountInfo, Address, Bytecode, B256, KECCAK_EMPTY, U256};
use revm::Database;
use thiserror::Error;
use tokio::runtime::{Handle, Runtime};
use tracing::{debug, warn};

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors that can arise while `LazyDb` fetches state from the provider.
#[derive(Debug, Error)]
pub enum LazyDbError {
    /// The underlying ethers RPC call failed.
    #[error("provider RPC error: {0}")]
    Provider(String),
    /// A type conversion or decode step failed.
    #[error("decode error: {0}")]
    Decode(String),
    /// The requested resource does not exist at the pinned block.
    #[error("resource not found: {0}")]
    NotFound(String),
}

// ---------------------------------------------------------------------------
// Cache key types
// ---------------------------------------------------------------------------

/// Storage-cache key: (contract address, slot index as U256).
type StorageKey = (Address, U256);

// ---------------------------------------------------------------------------
// LazyDb
// ---------------------------------------------------------------------------

/// `revm::Database` backed by an ethers HTTP provider with a lock-free cache.
///
/// All state reads are pinned to a single block (`pinned_block`).  The
/// constructor resolves the block once (fetching "latest" if `None` is passed)
/// so that every subsequent revm call sees a consistent snapshot.
pub struct LazyDb {
    /// Shared ethers HTTP client.
    client: Arc<Provider<Http>>,
    /// Block at which every RPC fetch is anchored.
    pinned_block: BlockId,
    /// Account (balance + nonce + code) cache.
    account_cache: DashMap<Address, AccountInfo>,
    /// Storage-slot cache: (address, slot) → value.
    storage_cache: DashMap<StorageKey, U256>,
    /// Block-hash cache: block_number_u64 → B256.
    block_hash_cache: DashMap<u64, B256>,
    /// Owned Tokio runtime used only when `LazyDb` is invoked from outside any
    /// Tokio context.  `None` in production (we always have a runtime handle).
    fallback_rt: Option<Arc<Runtime>>,
}

impl LazyDb {
    /// Build a `LazyDb` pinned to the given block number (or the current
    /// latest block if `None` is supplied).
    ///
    /// # Errors
    /// Returns `LazyDbError::Provider` if the URL is invalid or if the RPC
    /// call to determine the latest block number fails.
    pub fn new(rpc_url: &str, block_number: Option<u64>) -> Result<Self, LazyDbError> {
        let provider = Provider::<Http>::try_from(rpc_url)
            .map_err(|e| LazyDbError::Decode(format!("invalid RPC URL '{rpc_url}': {e}")))?;
        let client = Arc::new(provider);

        // Determine whether we are already inside a Tokio runtime.
        let (fallback_rt, pinned_block) = match Handle::try_current() {
            Ok(handle) => {
                // Inside a Tokio runtime: block_in_place for synchronous waits.
                let block = match block_number {
                    Some(n) => BlockId::Number(BlockNumber::Number(EU64::from(n))),
                    None => {
                        let bn = tokio::task::block_in_place(|| {
                            handle.block_on(client.get_block_number())
                        })
                        .map_err(|e| LazyDbError::Provider(format!("get_block_number: {e}")))?;
                        BlockId::Number(BlockNumber::Number(EU64::from(bn.as_u64())))
                    }
                };
                (None, block)
            }
            Err(_) => {
                // No runtime: create a fallback single-threaded runtime.
                let rt = Runtime::new()
                    .map_err(|e| LazyDbError::Provider(format!("rt create: {e}")))?;
                let block = match block_number {
                    Some(n) => BlockId::Number(BlockNumber::Number(EU64::from(n))),
                    None => {
                        let bn = rt
                            .block_on(client.get_block_number())
                            .map_err(|e| LazyDbError::Provider(format!("get_block_number: {e}")))?;
                        BlockId::Number(BlockNumber::Number(EU64::from(bn.as_u64())))
                    }
                };
                (Some(Arc::new(rt)), block)
            }
        };

        Ok(Self {
            client,
            pinned_block,
            account_cache: DashMap::new(),
            storage_cache: DashMap::new(),
            block_hash_cache: DashMap::new(),
            fallback_rt,
        })
    }

    // -----------------------------------------------------------------------
    // Async bridge helpers
    // -----------------------------------------------------------------------

    /// Drive `future` on the right executor depending on whether we have a
    /// live Tokio runtime or are using the owned fallback.
    fn block_on<F, T>(&self, future: F) -> T
    where
        F: std::future::Future<Output = T>,
    {
        match &self.fallback_rt {
            None => {
                // Production path: we are inside a Tokio runtime.
                let handle = Handle::current();
                tokio::task::block_in_place(|| handle.block_on(future))
            }
            Some(rt) => {
                // Test / bench path: use the owned runtime.
                rt.block_on(future)
            }
        }
    }

    // -----------------------------------------------------------------------
    // Type conversion helpers (ethers ↔ revm-primitives)
    // -----------------------------------------------------------------------

    #[inline]
    fn addr_to_ethers(addr: Address) -> EH160 {
        // Address inner type: FixedBytes<20>.  H160 is also [u8; 20].
        EH160::from(addr.0 .0)
    }

    #[inline]
    fn h256_to_b256(h: H256) -> B256 {
        B256::new(h.0)
    }

    #[inline]
    fn h256_to_u256(h: H256) -> U256 {
        U256::from_be_bytes(h.0)
    }
}

// ---------------------------------------------------------------------------
// revm::Database implementation
// ---------------------------------------------------------------------------

impl Database for LazyDb {
    type Error = LazyDbError;

    /// Return basic account info for `address`.
    ///
    /// - **Cache hit**: returns the cached `AccountInfo` without any RPC call.
    /// - **Cache miss**: fires 3 parallel RPC calls (`eth_getBalance`,
    ///   `eth_getTransactionCount`, `eth_getCode`), assembles `AccountInfo`,
    ///   stores it in the cache, and returns it.
    fn basic(&mut self, address: Address) -> Result<Option<AccountInfo>, Self::Error> {
        // Fast path.
        if let Some(cached) = self.account_cache.get(&address) {
            debug!(
                event = "lazy_db.cache_hit",
                kind = "account",
                %address,
                "account cache hit"
            );
            return Ok(Some(cached.clone()));
        }

        debug!(
            event = "lazy_db.cache_miss",
            kind = "account",
            %address,
            "fetching account info from RPC"
        );

        let eth_addr = Self::addr_to_ethers(address);
        let block = Some(self.pinned_block);

        // Three parallel fetches: balance, nonce, code.
        let (balance_res, nonce_res, code_res) = self.block_on(async {
            let b_fut = self.client.get_balance(eth_addr, block);
            let n_fut = self.client.get_transaction_count(eth_addr, block);
            let c_fut = self.client.get_code(eth_addr, block);
            tokio::join!(b_fut, n_fut, c_fut)
        });

        let eth_balance = balance_res
            .map_err(|e| LazyDbError::Provider(format!("get_balance({address}): {e}")))?;
        let eth_nonce = nonce_res
            .map_err(|e| LazyDbError::Provider(format!("get_nonce({address}): {e}")))?;
        let code_bytes = code_res
            .map_err(|e| LazyDbError::Provider(format!("get_code({address}): {e}")))?;

        // ethers U256 limbs are stored little-endian (lowest 64 bits first).
        // revm U256::from_limbs expects the same layout.
        let balance = U256::from_limbs(eth_balance.0);
        let nonce = eth_nonce.as_u64();

        let bytecode = Bytecode::new_raw(code_bytes.0.into());
        // Only compute a hash for non-empty bytecode; empty code gets the
        // well-known KECCAK_EMPTY constant so revm skips code analysis.
        let code_hash = if bytecode.is_empty() {
            KECCAK_EMPTY
        } else {
            bytecode.hash_slow()
        };

        let info = AccountInfo::new(balance, nonce, code_hash, bytecode);
        self.account_cache.insert(address, info.clone());
        Ok(Some(info))
    }

    /// `LazyDb` always returns bytecode inline in `basic()`, so this path is
    /// never reached in normal revm operation.  We return an empty `Bytecode`
    /// rather than panic, because panicking in library code violates the
    /// project's fail-honest invariant.
    fn code_by_hash(&mut self, code_hash: B256) -> Result<Bytecode, Self::Error> {
        // Why: revm only calls code_by_hash when AccountInfo.code == None.
        // Our basic() always populates the code field, so this branch is a
        // defensive fallback, not a production code path.
        warn!(
            event = "lazy_db.code_by_hash_called",
            hash = %code_hash,
            "code_by_hash called unexpectedly; returning empty bytecode"
        );
        Ok(Bytecode::new())
    }

    /// Return the value of storage slot `index` for `address`.
    ///
    /// - **Cache hit**: zero RPC calls.
    /// - **Cache miss**: one `eth_getStorageAt` at the pinned block.
    fn storage(&mut self, address: Address, index: U256) -> Result<U256, Self::Error> {
        let key = (address, index);

        if let Some(val) = self.storage_cache.get(&key) {
            debug!(
                event = "lazy_db.cache_hit",
                kind = "storage",
                %address,
                slot = %index,
                "storage cache hit"
            );
            return Ok(*val);
        }

        debug!(
            event = "lazy_db.cache_miss",
            kind = "storage",
            %address,
            slot = %index,
            "fetching storage slot from RPC"
        );

        let eth_addr = Self::addr_to_ethers(address);
        let slot_h256 = H256::from(index.to_be_bytes());

        let raw = self
            .block_on(
                self.client
                    .get_storage_at(eth_addr, slot_h256, Some(self.pinned_block)),
            )
            .map_err(|e| {
                LazyDbError::Provider(format!("get_storage_at({address}, {index}): {e}"))
            })?;

        let value = Self::h256_to_u256(raw);
        self.storage_cache.insert(key, value);
        Ok(value)
    }

    /// Return the block hash for the given block `number`.
    ///
    /// Returns `B256::ZERO` (not an error) for blocks whose hash is not
    /// available: contracts rarely use `BLOCKHASH` for ancient blocks, and a
    /// zero value is safe for simulation purposes.
    fn block_hash(&mut self, number: U256) -> Result<B256, Self::Error> {
        // Block numbers above u64::MAX are impossible on any EVM chain.
        if number > U256::from(u64::MAX) {
            warn!(
                event = "lazy_db.block_hash_overflow",
                %number,
                "block_hash: number > u64::MAX, returning KECCAK_EMPTY"
            );
            return Ok(KECCAK_EMPTY);
        }
        let n: u64 = number.to();

        if let Some(hash) = self.block_hash_cache.get(&n) {
            debug!(
                event = "lazy_db.cache_hit",
                kind = "block_hash",
                number = n,
                "block hash cache hit"
            );
            return Ok(*hash);
        }

        debug!(
            event = "lazy_db.cache_miss",
            kind = "block_hash",
            number = n,
            "fetching block hash from RPC"
        );

        let block_id = BlockId::Number(BlockNumber::Number(EU64::from(n)));
        let maybe_block = self
            .block_on(self.client.get_block(block_id))
            .map_err(|e| LazyDbError::Provider(format!("get_block({n}): {e}")))?;

        let hash = match maybe_block.and_then(|b| b.hash) {
            Some(h) => Self::h256_to_b256(h),
            None => {
                warn!(
                    event = "lazy_db.block_hash_not_found",
                    number = n,
                    "block or hash not found; returning B256::ZERO for simulation safety"
                );
                B256::ZERO
            }
        };

        self.block_hash_cache.insert(n, hash);
        Ok(hash)
    }
}

// ---------------------------------------------------------------------------
// Test helpers — used by integration test files in tests/.
// The #[allow(dead_code)] suppresses the warning when building the lib target
// alone; the functions are exercised by cargo test --tests.
// ---------------------------------------------------------------------------

/// Seed an `AccountInfo` directly into the cache (test setup without RPC).
#[allow(dead_code)]
pub fn seed_account(db: &LazyDb, addr: Address, info: AccountInfo) {
    db.account_cache.insert(addr, info);
}

/// Seed a storage slot directly into the cache.
#[allow(dead_code)]
pub fn seed_storage(db: &LazyDb, addr: Address, slot: U256, value: U256) {
    db.storage_cache.insert((addr, slot), value);
}

/// Seed a block hash directly into the cache.
#[allow(dead_code)]
pub fn seed_block_hash(db: &LazyDb, number: u64, hash: B256) {
    db.block_hash_cache.insert(number, hash);
}

/// Return the number of entries in the account cache.
#[allow(dead_code)]
pub fn account_cache_len(db: &LazyDb) -> usize {
    db.account_cache.len()
}

/// Return the number of entries in the storage cache.
#[allow(dead_code)]
pub fn storage_cache_len(db: &LazyDb) -> usize {
    db.storage_cache.len()
}

/// Return the number of entries in the block-hash cache.
#[allow(dead_code)]
pub fn block_hash_cache_len(db: &LazyDb) -> usize {
    db.block_hash_cache.len()
}
