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
//! (same block → same deterministic value), so this is accepted.
//!
//! ### Sync-async bridge (CRITICAL #1 fix)
//! `revm::Database` is synchronous.  We use `block_in_place` ONLY when the
//! runtime flavor is `MultiThread`.  Under `CurrentThread` (e.g., `#[tokio::test]`
//! default) `block_in_place` panics — we fall back to an owned `Runtime`
//! instead.  This logic lives in ONE place: `bridge::block_on_with_timeout()`.
//!
//! ### Provider timeout (MAJOR #4 fix)
//! The HTTP client is built with a 5-second timeout so a hanging RPC node
//! cannot permanently park a tokio worker thread.  `LazyDb::with_timeout()`
//! allows operators/tests to override the default.  Every `block_on` call also
//! wraps the future in `tokio::time::timeout` as defense-in-depth.

use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use dashmap::DashMap;
use ethers::providers::{Http, Middleware, Provider};
use ethers::types::{BlockId, BlockNumber, H160 as EH160, H256, U64 as EU64};
use revm::primitives::{AccountInfo, Address, Bytecode, B256, KECCAK_EMPTY, U256};
use revm::Database;
use thiserror::Error;
use tokio::runtime::{Handle, Runtime, RuntimeFlavor};
use tracing::{debug, warn};
use url::Url;

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
    /// The RPC call exceeded the configured timeout (MAJOR #4 fix).
    #[error("rpc timeout: {0}")]
    Timeout(String),
}

/// Default RPC timeout applied to every provider call (MAJOR #4 fix).
const DEFAULT_RPC_TIMEOUT_SECS: u64 = 5;

// ---------------------------------------------------------------------------
// Cache key types
// ---------------------------------------------------------------------------

type StorageKey = (Address, U256);

// ---------------------------------------------------------------------------
// Sync-async bridge — ONE canonical location for the runtime-flavor guard
// (CRITICAL #1 fix + MAJOR #4 fix)
// ---------------------------------------------------------------------------

/// All sync-async bridging goes through this module so the runtime-flavor
/// guard and the timeout are applied consistently in every RPC call path.
mod bridge {
    use super::*;

    /// Drive `fut` synchronously, wrapping it with a `timeout_secs` deadline.
    ///
    /// ## Runtime flavor guard (CRITICAL #1 fix)
    ///
    /// `tokio::task::block_in_place` requires the `MultiThread` scheduler
    /// and **panics** when called inside a `CurrentThread` runtime (the default
    /// for `#[tokio::test]`).
    ///
    /// Decision tree — ONE place, no duplication:
    /// - `MultiThread` handle found  → `block_in_place` + `handle.block_on`
    /// - `CurrentThread` or no handle → `owned_rt.block_on`
    ///
    /// The outer `tokio::time::timeout` is defense-in-depth: even if the HTTP
    /// client does not honour its deadline, this stops the worker parking.
    pub(super) fn block_on_with_timeout<F, T>(
        owned_rt: &Arc<Runtime>,
        timeout_secs: u64,
        fut: F,
        context: &str,
    ) -> Result<T, LazyDbError>
    where
        F: Future<Output = T>,
    {
        let timed = tokio::time::timeout(Duration::from_secs(timeout_secs), fut);
        let res = match Handle::try_current() {
            Ok(handle) if handle.runtime_flavor() == RuntimeFlavor::MultiThread => {
                // Safe: multi-thread scheduler keeps other workers alive.
                tokio::task::block_in_place(|| handle.block_on(timed))
            }
            // CurrentThread flavor or no ambient runtime: use owned fallback.
            _ => owned_rt.block_on(timed),
        };
        res.map_err(|_| LazyDbError::Timeout(format!("rpc timeout ({timeout_secs}s): {context}")))
    }
}

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
    /// Resolved block number (always set after construction).
    pinned_block_number: u64,
    /// Account (balance + nonce + code) cache.
    account_cache: DashMap<Address, AccountInfo>,
    /// Storage-slot cache: (address, slot) → value.
    storage_cache: DashMap<StorageKey, U256>,
    /// Block-hash cache: block_number_u64 → B256.
    block_hash_cache: DashMap<u64, B256>,
    /// Owned Tokio runtime — always present.  Serves as the fallback for both
    /// `CurrentThread` runtimes and "no runtime" contexts (unit tests).
    fallback_rt: Arc<Runtime>,
    /// Per-call RPC timeout in seconds.
    timeout_secs: u64,
}

impl LazyDb {
    /// Build a `LazyDb` pinned to the given block number (or the current
    /// latest block if `None` is supplied).
    ///
    /// Uses the default 5-second RPC timeout. See `new_with_timeout` for
    /// custom timeout configuration.
    ///
    /// # Errors
    /// Returns `LazyDbError::Provider` if the URL is invalid or if the RPC
    /// call to determine the latest block number fails.
    /// Returns `LazyDbError::Timeout` if the "latest" resolution exceeds 5 s.
    pub fn new(rpc_url: &str, block_number: Option<u64>) -> Result<Self, LazyDbError> {
        Self::new_with_timeout(rpc_url, block_number, DEFAULT_RPC_TIMEOUT_SECS)
    }

    /// Same as `new()` but with an explicit RPC timeout (MAJOR #4 fix).
    ///
    /// Exposed for operators who need longer timeouts on slow endpoints and for
    /// tests that want to fail fast against unreachable servers.
    pub fn new_with_timeout(
        rpc_url: &str,
        block_number: Option<u64>,
        timeout_secs: u64,
    ) -> Result<Self, LazyDbError> {
        // Build an HTTP client with the caller-specified timeout (MAJOR #4).
        // reqwest 0.11 is used because that is the version ethers-providers 2.x
        // depends on internally — Http::new_with_client takes reqwest 0.11 Client.
        let http_client = reqwest::Client::builder()
            .timeout(Duration::from_secs(timeout_secs))
            .build()
            .map_err(|e| LazyDbError::Provider(format!("reqwest client build: {e}")))?;

        let url = Url::parse(rpc_url)
            .map_err(|e| LazyDbError::Decode(format!("invalid RPC URL '{rpc_url}': {e}")))?;

        let http = Http::new_with_client(url, http_client);
        let client = Arc::new(Provider::new(http));

        // Always create an owned fallback runtime.
        // It handles both CurrentThread contexts and no-runtime contexts.
        let fallback_rt = Arc::new(
            Runtime::new().map_err(|e| LazyDbError::Provider(format!("tokio rt create: {e}")))?,
        );

        let (pinned_block_number, pinned_block) = match block_number {
            Some(n) => (n, BlockId::Number(BlockNumber::Number(EU64::from(n)))),
            None => {
                // Resolve latest with runtime-flavor guard + timeout (CRITICAL #1).
                let bn_result = bridge::block_on_with_timeout(
                    &fallback_rt,
                    timeout_secs,
                    client.get_block_number(),
                    "get_block_number",
                )?;
                let n = bn_result
                    .map_err(|e| LazyDbError::Provider(format!("get_block_number: {e}")))?
                    .as_u64();
                (n, BlockId::Number(BlockNumber::Number(EU64::from(n))))
            }
        };

        Ok(Self {
            client,
            pinned_block,
            pinned_block_number,
            account_cache: DashMap::new(),
            storage_cache: DashMap::new(),
            block_hash_cache: DashMap::new(),
            fallback_rt,
            timeout_secs,
        })
    }

    /// Override the RPC timeout after construction (builder pattern).
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout_secs = timeout.as_secs().max(1);
        self
    }

    /// Return the block number this `LazyDb` is pinned to.
    ///
    /// Used by `SimulatorV2` to memoize the resolved "latest" block across
    /// multiple `simulate()` calls (MAJOR #3 + #6 fix).
    pub fn pinned_block_number(&self) -> u64 {
        self.pinned_block_number
    }

    // -----------------------------------------------------------------------
    // Private RPC helper
    // -----------------------------------------------------------------------

    /// Execute a single provider future with the timeout + flavor guard.
    fn rpc<F, T>(&self, context: &str, fut: F) -> Result<T, LazyDbError>
    where
        F: Future<Output = Result<T, ethers::providers::ProviderError>>,
    {
        bridge::block_on_with_timeout(&self.fallback_rt, self.timeout_secs, fut, context)?
            .map_err(|e| LazyDbError::Provider(format!("{context}: {e}")))
    }

    // -----------------------------------------------------------------------
    // Type conversion helpers (ethers ↔ revm-primitives)
    // -----------------------------------------------------------------------

    #[inline]
    fn addr_to_ethers(addr: Address) -> EH160 {
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

// ---------------------------------------------------------------------------
// Shared read paths (&self) — Phase A.3.c.4
// ---------------------------------------------------------------------------
//
// Every public read path (`basic`, `storage`, `block_hash`, `code_by_hash`)
// only consumes `&self` semantically — the `DashMap` caches are concurrency-
// safe under shared references, the `client` is an `Arc`, and `pinned_block`,
// `timeout_secs`, `fallback_rt` are read-only after construction. The `&mut
// self` on the original `Database` impl is purely the trait signature.
//
// To support both `revm::Database` AND `revm::DatabaseRef` from the SAME
// underlying implementation we extract the bodies to `*_inner` helpers that
// take `&self`. Both traits delegate. This makes the equivalence between
// `Database` and `DatabaseRef` structural (one code path = one truth) and
// unlocks `revm::db::CacheDB<LazyDb>` for the multi-step REVM executor
// (Phase A.3.c.3).
impl LazyDb {
    /// Shared body for `Database::basic` and `DatabaseRef::basic_ref`.
    /// All DashMap operations are `&self`-safe.
    fn basic_inner(&self, address: Address) -> Result<Option<AccountInfo>, LazyDbError> {
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
        let client = self.client.clone();

        // Three parallel fetches: balance, nonce, code.
        // Uses bridge::block_on_with_timeout for runtime-flavor guard + timeout.
        let (balance_res, nonce_res, code_res) = bridge::block_on_with_timeout(
            &self.fallback_rt,
            self.timeout_secs,
            async move {
                let b_fut = client.get_balance(eth_addr, block);
                let n_fut = client.get_transaction_count(eth_addr, block);
                let c_fut = client.get_code(eth_addr, block);
                tokio::join!(b_fut, n_fut, c_fut)
            },
            &format!("basic({address})"),
        )?;

        let eth_balance = balance_res
            .map_err(|e| LazyDbError::Provider(format!("get_balance({address}): {e}")))?;
        let eth_nonce =
            nonce_res.map_err(|e| LazyDbError::Provider(format!("get_nonce({address}): {e}")))?;
        let code_bytes =
            code_res.map_err(|e| LazyDbError::Provider(format!("get_code({address}): {e}")))?;

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

    /// Shared body for `Database::code_by_hash` and
    /// `DatabaseRef::code_by_hash_ref`. Defensive fallback — `basic_inner`
    /// always populates bytecode inline so revm never reaches this branch
    /// in normal operation.
    fn code_by_hash_inner(&self, code_hash: B256) -> Result<Bytecode, LazyDbError> {
        warn!(
            event = "lazy_db.code_by_hash_called",
            hash = %code_hash,
            "code_by_hash called unexpectedly; returning empty bytecode"
        );
        Ok(Bytecode::new())
    }

    /// Shared body for `Database::storage` and `DatabaseRef::storage_ref`.
    fn storage_inner(&self, address: Address, index: U256) -> Result<U256, LazyDbError> {
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

        let raw = self.rpc(
            &format!("get_storage_at({address}, {index})"),
            self.client
                .get_storage_at(eth_addr, slot_h256, Some(self.pinned_block)),
        )?;

        let value = Self::h256_to_u256(raw);
        self.storage_cache.insert(key, value);
        Ok(value)
    }

    /// Shared body for `Database::block_hash` and `DatabaseRef::block_hash_ref`.
    fn block_hash_inner(&self, number: U256) -> Result<B256, LazyDbError> {
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
        let maybe_block = self.rpc(&format!("get_block({n})"), self.client.get_block(block_id))?;

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
// revm::Database — mutable-reference trait (legacy revm 3.5 contract).
// Delegates to the &self `*_inner` helpers above. Equivalent to the
// `DatabaseRef` impl below for the same input.
// ---------------------------------------------------------------------------
impl Database for LazyDb {
    type Error = LazyDbError;

    fn basic(&mut self, address: Address) -> Result<Option<AccountInfo>, Self::Error> {
        self.basic_inner(address)
    }

    fn code_by_hash(&mut self, code_hash: B256) -> Result<Bytecode, Self::Error> {
        self.code_by_hash_inner(code_hash)
    }

    fn storage(&mut self, address: Address, index: U256) -> Result<U256, Self::Error> {
        self.storage_inner(address, index)
    }

    fn block_hash(&mut self, number: U256) -> Result<B256, Self::Error> {
        self.block_hash_inner(number)
    }
}

// ---------------------------------------------------------------------------
// revm::DatabaseRef — shared-reference trait (revm 3.5+ contract). Required
// by `revm::db::CacheDB<DB>` so multi-step REVM executors can persist state
// between transactions (Phase A.3.c.3 sequence_runner). Delegates to the
// same `*_inner` helpers as `Database` — Database and DatabaseRef are
// structurally equivalent.
// ---------------------------------------------------------------------------
impl revm::db::DatabaseRef for LazyDb {
    type Error = LazyDbError;

    fn basic(&self, address: Address) -> Result<Option<AccountInfo>, Self::Error> {
        self.basic_inner(address)
    }

    fn code_by_hash(&self, code_hash: B256) -> Result<Bytecode, Self::Error> {
        self.code_by_hash_inner(code_hash)
    }

    fn storage(&self, address: Address, index: U256) -> Result<U256, Self::Error> {
        self.storage_inner(address, index)
    }

    fn block_hash(&self, number: U256) -> Result<B256, Self::Error> {
        self.block_hash_inner(number)
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

// ---------------------------------------------------------------------------
// Tests — Phase A.3.c.4 DatabaseRef foundation
// ---------------------------------------------------------------------------
#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use revm::db::{CacheDB, DatabaseRef};

    /// Construct a LazyDb without contacting the chain. `connect_lazy`-style
    /// setup: the constructor only fails on URL parse, so any well-formed
    /// URL works for the trait-bound checks below (no RPC calls happen).
    fn lazy_offline() -> LazyDb {
        // Use a URL that parses but is never reachable; tests below do not
        // issue any RPC call — they exercise cache-hit / trait-bound paths.
        LazyDb::new("http://127.0.0.1:1/never", Some(1)).expect("LazyDb::new should accept a well-formed URL")
    }

    /// THE CRITICAL TEST: this is the trait-bound that A.3.c.3's
    /// sequence_runner needs. If LazyDb does not implement `DatabaseRef`,
    /// this line fails to compile and A.3.c.3 cannot proceed.
    #[test]
    fn cache_db_accepts_lazy_db_database_ref() {
        let lazy = lazy_offline();
        // Pre-seed the account so `insert_account_storage` finds it via the
        // cache and never reaches the RPC backend (the offline URL would
        // refuse the connection otherwise).
        let addr = Address::from([0x42u8; 20]);
        seed_account(
            &lazy,
            addr,
            AccountInfo {
                balance: U256::from(1u64),
                nonce: 0,
                code_hash: KECCAK_EMPTY,
                code: Some(Bytecode::new()),
            },
        );
        // THE CRITICAL TRAIT-BOUND CHECK: CacheDB<LazyDb> must instantiate.
        // If `DatabaseRef` were missing this line would fail with the
        // exact E0277 error A.3.c.3 hit.
        let mut cache: CacheDB<LazyDb> = CacheDB::new(lazy);
        let slot = U256::from(0u64);
        let value = U256::from(123u64);
        cache
            .insert_account_storage(addr, slot, value)
            .expect("insert_account_storage on CacheDB<LazyDb> must compile and run");
    }

    /// Equivalence between `Database::basic` and `DatabaseRef::basic` for
    /// a seeded account. Uses `seed_account` so no RPC is needed.
    #[test]
    fn database_ref_basic_matches_database_basic_for_seeded_account() {
        let lazy = lazy_offline();
        let addr = Address::from([0x42u8; 20]);
        let info = AccountInfo {
            balance: U256::from(7777u64),
            nonce: 3,
            code_hash: KECCAK_EMPTY,
            code: Some(Bytecode::new()),
        };
        seed_account(&lazy, addr, info.clone());
        // DatabaseRef path
        let ref_result = DatabaseRef::basic(&lazy, addr).expect("basic_ref ok");
        // Database path (needs &mut)
        let mut lazy_mut = lazy;
        let mut_result = Database::basic(&mut lazy_mut, addr).expect("basic ok");
        assert_eq!(ref_result.map(|i| (i.balance, i.nonce)), mut_result.map(|i| (i.balance, i.nonce)));
    }

    /// Equivalence between `Database::storage` and `DatabaseRef::storage`
    /// for a seeded slot. Uses `seed_storage` so no RPC is needed.
    #[test]
    fn database_ref_storage_matches_database_storage_for_seeded_slot() {
        let lazy = lazy_offline();
        let addr = Address::from([0x07u8; 20]);
        let slot = U256::from(42u64);
        let value = U256::from(12345u64);
        seed_storage(&lazy, addr, slot, value);
        let ref_result = DatabaseRef::storage(&lazy, addr, slot).expect("storage_ref ok");
        let mut lazy_mut = lazy;
        let mut_result = Database::storage(&mut lazy_mut, addr, slot).expect("storage ok");
        assert_eq!(ref_result, mut_result);
        assert_eq!(ref_result, value);
    }

    /// Equivalence between `Database::block_hash` and
    /// `DatabaseRef::block_hash` for a seeded block. Uses `seed_block_hash`.
    #[test]
    fn database_ref_block_hash_matches_database_block_hash_for_seeded_block() {
        let lazy = lazy_offline();
        let block_n = 100u64;
        let h = B256::from([0xab; 32]);
        seed_block_hash(&lazy, block_n, h);
        let n_u256 = U256::from(block_n);
        let ref_result = DatabaseRef::block_hash(&lazy, n_u256).expect("block_hash_ref ok");
        let mut lazy_mut = lazy;
        let mut_result = Database::block_hash(&mut lazy_mut, n_u256).expect("block_hash ok");
        assert_eq!(ref_result, mut_result);
        assert_eq!(ref_result, h);
    }

    /// `code_by_hash` returns empty bytecode in both paths (defensive — see
    /// `code_by_hash_inner` docs).
    #[test]
    fn database_ref_code_by_hash_matches_database_code_by_hash() {
        let lazy = lazy_offline();
        let hash = B256::from([0u8; 32]);
        let ref_result = DatabaseRef::code_by_hash(&lazy, hash).expect("code_by_hash_ref ok");
        let mut lazy_mut = lazy;
        let mut_result = Database::code_by_hash(&mut lazy_mut, hash).expect("code_by_hash ok");
        assert!(ref_result.is_empty());
        assert!(mut_result.is_empty());
    }

    /// Two `DatabaseRef::basic` calls on the same seeded account hit the
    /// cache on the second call. Verified by reading via the cache helper.
    #[test]
    fn database_ref_basic_hits_cache_on_repeat() {
        let lazy = lazy_offline();
        let addr = Address::from([0xfeu8; 20]);
        let info = AccountInfo {
            balance: U256::from(99u64),
            nonce: 1,
            code_hash: KECCAK_EMPTY,
            code: Some(Bytecode::new()),
        };
        seed_account(&lazy, addr, info);
        // Two reads — cache populated on first, hit on second.
        let _ = DatabaseRef::basic(&lazy, addr).expect("first ok");
        let _ = DatabaseRef::basic(&lazy, addr).expect("second ok");
        assert_eq!(account_cache_len(&lazy), 1, "single entry expected");
    }

    /// Concurrent readers do not panic (DashMap is internally Sync).
    #[test]
    fn database_ref_concurrent_basic_reads_do_not_panic() {
        use std::sync::Arc;
        use std::thread;
        let lazy = Arc::new(lazy_offline());
        let addr = Address::from([0x11u8; 20]);
        let info = AccountInfo {
            balance: U256::from(1u64),
            nonce: 0,
            code_hash: KECCAK_EMPTY,
            code: Some(Bytecode::new()),
        };
        seed_account(&lazy, addr, info);
        let mut handles = Vec::new();
        for _ in 0..8 {
            let lazy_c = Arc::clone(&lazy);
            handles.push(thread::spawn(move || {
                for _ in 0..32 {
                    let _ = DatabaseRef::basic(lazy_c.as_ref(), addr).unwrap();
                }
            }));
        }
        for h in handles {
            h.join().expect("thread joined without panic");
        }
        assert_eq!(account_cache_len(&lazy), 1);
    }

    /// `pinned_block_number` is stable across `DatabaseRef` reads — A.3.c.3
    /// relies on every sequence step seeing the same block snapshot.
    #[test]
    fn database_ref_respects_pinned_block() {
        let lazy = LazyDb::new("http://127.0.0.1:1/never", Some(21_000_000)).expect("new ok");
        assert_eq!(lazy.pinned_block_number(), 21_000_000);
        // No method call mutates the pinned block; even after seeded reads,
        // the value is unchanged.
        let addr = Address::from([0u8; 20]);
        seed_account(
            &lazy,
            addr,
            AccountInfo {
                balance: U256::ZERO,
                nonce: 0,
                code_hash: KECCAK_EMPTY,
                code: Some(Bytecode::new()),
            },
        );
        let _ = DatabaseRef::basic(&lazy, addr);
        assert_eq!(lazy.pinned_block_number(), 21_000_000);
    }
}
