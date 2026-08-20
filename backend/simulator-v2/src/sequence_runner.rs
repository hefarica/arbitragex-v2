//! Phase A.3.c.3 — Multi-step REVM sequence runner with persistent CacheDB.
//!
//! Builder-style API that keeps a `revm::EVM<CacheDB<LazyDb>>` alive across
//! multiple operations so state mutations from one transaction are visible
//! to the next. Required by the round-trip arbitrage simulator: the
//! backward swap's `amountIn` is the intermediate `balanceOf(caller,
//! token_out)` that only exists in REVM state AFTER the forward swap
//! commits.
//!
//! ## API shape — `SequenceContext`
//!
//! ```ignore
//! let mut ctx = SequenceContext::new(lazy_db, chain_id, block);
//! ctx.apply_storage(StorageOverride { ... })?;     // balanceOf prefund
//! ctx.apply_storage(StorageOverride { ... })?;     // forward allowance
//! let _ = ctx.call(SequenceCall { ... })?;         // forward swap
//! let intermediate = ctx.read_balance(token_out, caller, "intermediate")?;
//! ctx.apply_storage(StorageOverride { ... })?;     // backward allowance
//! // Backward calldata is built with the real `intermediate` value:
//! let backward_calldata = encode_v2_swap_exact_tokens_for_tokens(
//!     intermediate, U256::ZERO, &backward_path, caller, deadline,
//! );
//! let _ = ctx.call(SequenceCall { calldata: backward_calldata.into(), ... })?;
//! let final_in = ctx.read_balance(token_in, caller, "final")?;
//! let summary = ctx.finalize();
//! // summary.trace_hash != [0; 32], summary.gas_used_total > 0 — caller
//! // computes profit from `(final_in - amount_in) - gas_cost`.
//! ```
//!
//! ## Anti-fraud invariants
//!
//! 1. `finalize()` returns `trace_hash == [0; 32]` if and only if zero
//!    `call()` steps committed successfully. The caller's success-path
//!    guard rejects such results.
//! 2. `read_balance` returns the U256 value DECODED from REVM's real
//!    `balanceOf(account)` return data. There is no path that
//!    fabricates a non-zero balance.
//! 3. `apply_storage` directly calls `CacheDB::insert_account_storage`
//!    — the same API the caller uses against a live test fixture. No
//!    re-implementation, no second source of truth.
//! 4. `gas_used_total` is the sum of `gas_used` returned by REVM across
//!    every committed `call()`. `read_balance` does NOT commit and does
//!    NOT bump the gas counter.
//! 5. The caller (sim_multistep) computes `gross_profit = final_balance - amount_in`. Prefund is structurally excluded because the storage override establishes `amount_in` as the BASE, not as a bonus on top of pre-existing balance.

use revm::db::CacheDB;
use revm::primitives::{
    Address, BlockEnv, Bytes, CfgEnv, Env, ExecutionResult, SpecId, TransactTo, TxEnv, U256,
};
use revm::EVM;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use thiserror::Error;
use tracing::{debug, warn};

use crate::LazyDb;

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Typed errors for the sequence runner. Every variant carries a
/// `reason_tag()` mapping to the operator-visible counter and log fields.
#[derive(Debug, Error)]
pub enum SequenceError {
    #[error("EVM database handle was unexpectedly None at storage override")]
    EvmDbMissing,
    #[error("CacheDB.insert_account_storage failed: {0}")]
    StorageOverrideFailed(String),
    #[error("REVM transact_commit infrastructure error: {0}")]
    TransactInfra(String),
    #[error("balanceOf output too short ({len} bytes, expected ≥32)")]
    BalanceDecodeFailed { len: usize },
    #[error("balanceOf reverted: {0}")]
    BalanceReadReverted(String),
    #[error("balanceOf halted: {0}")]
    BalanceReadHalted(String),
    #[error("getAmountsOut output too short ({len} bytes, expected a uint256[] header)")]
    AmountsOutDecodeFailed { len: usize },
    #[error("getAmountsOut returned an empty array")]
    AmountsOutEmptyArray,
    #[error("getAmountsOut reverted: {0}")]
    AmountsOutReverted(String),
    #[error("getAmountsOut halted: {0}")]
    AmountsOutHalted(String),
}

impl SequenceError {
    pub fn reason_tag(&self) -> &'static str {
        match self {
            Self::EvmDbMissing => "evm_db_missing",
            Self::StorageOverrideFailed(_) => "storage_override_failed",
            Self::TransactInfra(_) => "transact_infra",
            Self::BalanceDecodeFailed { .. } => "balance_decode_failed",
            Self::BalanceReadReverted(_) => "balance_read_reverted",
            Self::BalanceReadHalted(_) => "balance_read_halted",
            Self::AmountsOutDecodeFailed { .. } => "amounts_out_decode_failed",
            Self::AmountsOutEmptyArray => "amounts_out_empty_array",
            Self::AmountsOutReverted(_) => "amounts_out_reverted",
            Self::AmountsOutHalted(_) => "amounts_out_halted",
        }
    }
}

// ---------------------------------------------------------------------------
// Inputs
// ---------------------------------------------------------------------------

/// Apply a paper-only storage override to the CacheDB before the next call.
#[derive(Debug, Clone, Copy)]
pub struct StorageOverride {
    pub contract: Address,
    pub slot: U256,
    pub value: U256,
    /// Operator-visible label for logs (`"balance_of"`, `"allowance"`, etc).
    pub label: &'static str,
}

/// A transaction to execute via `transact_commit`. The orchestrator
/// constructs the calldata; the runner does not synthesise any.
#[derive(Debug, Clone)]
pub struct SequenceCall {
    pub from: Address,
    pub to: Address,
    pub calldata: Vec<u8>,
    pub value_wei: u128,
    pub gas_price_wei: u128,
    pub gas_limit: u64,
    /// Operator-visible label for logs (`"forward_swap"`, etc).
    pub label: &'static str,
}

/// Outcome of a single `call()`.
#[derive(Debug, Clone)]
pub enum CallOutcome {
    Success { gas_used: u64, output: Vec<u8> },
    Reverted { gas_used: u64, reason: String },
    Halted { gas_used: u64, reason: String },
}

impl CallOutcome {
    pub fn is_success(&self) -> bool {
        matches!(self, Self::Success { .. })
    }
}

// ---------------------------------------------------------------------------
// Result
// ---------------------------------------------------------------------------

/// Aggregate summary produced by `finalize()`.
#[derive(Debug, Clone)]
pub struct SequenceResult {
    pub gas_used_total: u64,
    pub trace_hash: [u8; 32],
    pub reads: HashMap<String, U256>,
    /// Number of `call()` invocations that succeeded. Used by the caller
    /// to enforce the anti-fraud invariant "no SIM_SUCCESS without at
    /// least one successful committed transaction."
    pub successful_calls: u64,
}

// ---------------------------------------------------------------------------
// Context
// ---------------------------------------------------------------------------

/// Stateful builder that drives a multi-step REVM simulation. Holds the
/// `EVM<CacheDB<LazyDb>>` for the lifetime of the simulation so every
/// step sees the post-state of the previous step.
///
/// The struct is `!Send + !Sync` (because `EVM` is) — use it within a
/// single `tokio::task::spawn_blocking` invocation. The caller (scanner
/// hot path) wraps the entire simulation in `spawn_blocking` so the
/// tokio runtime is never parked on REVM.
pub struct SequenceContext {
    evm: EVM<CacheDB<LazyDb>>,
    reads: HashMap<String, U256>,
    gas_used_total: u64,
    hasher: Sha256,
    successful_calls: u64,
}

impl SequenceContext {
    /// Build a fresh context around a freshly-constructed `LazyDb`. The
    /// `chain_id` and `block_number` define the REVM environment that
    /// every step shares — they are NEVER mutated by `call()`.
    pub fn new(lazy_db: LazyDb, chain_id: u64, block_number: u64) -> Self {
        let cache_db = CacheDB::new(lazy_db);
        let mut cfg = CfgEnv::default();
        cfg.chain_id = chain_id;
        cfg.spec_id = SpecId::LATEST;
        let block = BlockEnv {
            number: U256::from(block_number),
            ..BlockEnv::default()
        };
        let env = Env {
            cfg,
            block,
            tx: TxEnv::default(),
        };
        let mut evm = EVM::with_env(env);
        evm.database(cache_db);
        Self {
            evm,
            reads: HashMap::new(),
            gas_used_total: 0,
            hasher: Sha256::new(),
            successful_calls: 0,
        }
    }

    /// Apply a paper-only storage override to the CacheDB. Returns an
    /// error if the EVM database handle is unexpectedly missing or if
    /// the underlying CacheDB rejects the insertion (only happens when
    /// the DatabaseRef fetch for the existing account fails).
    pub fn apply_storage(&mut self, over: StorageOverride) -> Result<(), SequenceError> {
        let db = self.evm.db().ok_or(SequenceError::EvmDbMissing)?;
        db.insert_account_storage(over.contract, over.slot, over.value)
            .map_err(|e| SequenceError::StorageOverrideFailed(format!("{e}")))?;
        debug!(
            event = "sequence_runner.storage_override",
            contract = ?over.contract,
            slot = %over.slot,
            label = %over.label,
            "storage override applied"
        );
        Ok(())
    }

    /// Execute `call` via `evm.transact_commit()`. Mutates the CacheDB
    /// so subsequent steps see the post-state. Returns the outcome —
    /// the orchestrator inspects it and decides whether to continue.
    pub fn call(&mut self, call: SequenceCall) -> Result<CallOutcome, SequenceError> {
        self.evm.env.tx = TxEnv {
            caller: call.from,
            transact_to: TransactTo::Call(call.to),
            data: Bytes::copy_from_slice(&call.calldata),
            value: U256::from(call.value_wei),
            gas_price: U256::from(call.gas_price_wei),
            gas_limit: call.gas_limit,
            nonce: None,
            chain_id: None,
            ..TxEnv::default()
        };

        let exec = self
            .evm
            .transact_commit()
            .map_err(|e| SequenceError::TransactInfra(format!("{e}")))?;

        let outcome = match exec {
            ExecutionResult::Success {
                gas_used, output, ..
            } => {
                self.gas_used_total += gas_used;
                self.successful_calls += 1;
                let bytes = output.into_data();
                // Fold calldata + output into the combined trace hash.
                self.hasher.update(&call.calldata);
                self.hasher.update(bytes.as_ref());
                debug!(
                    event = "sequence_runner.call_success",
                    label = %call.label,
                    gas_used,
                );
                CallOutcome::Success {
                    gas_used,
                    output: bytes.to_vec(),
                }
            }
            ExecutionResult::Revert { gas_used, output } => {
                self.gas_used_total += gas_used;
                let reason = decode_revert_reason(&output);
                warn!(
                    event = "sequence_runner.call_revert",
                    label = %call.label,
                    gas_used,
                    reason = %reason,
                );
                CallOutcome::Reverted { gas_used, reason }
            }
            ExecutionResult::Halt { reason, gas_used } => {
                self.gas_used_total += gas_used;
                warn!(
                    event = "sequence_runner.call_halt",
                    label = %call.label,
                    gas_used,
                    reason = ?reason,
                );
                CallOutcome::Halted {
                    gas_used,
                    reason: format!("{reason:?}"),
                }
            }
        };
        Ok(outcome)
    }

    /// Read `balanceOf(account)` against `token` via a non-committing
    /// `transact()` call. The returned `U256` is stored in `reads` under
    /// `label` so the orchestrator can reference it later (e.g., to
    /// resolve `AmountSource::FromReadLabel`).
    pub fn read_balance(
        &mut self,
        token: Address,
        account: Address,
        label: &str,
    ) -> Result<U256, SequenceError> {
        // ERC-20 `balanceOf(address)`: selector 0x70a08231 + 32-byte
        // padded account.
        let mut calldata: Vec<u8> = Vec::with_capacity(36);
        calldata.extend_from_slice(&[0x70, 0xa0, 0x82, 0x31]);
        let mut padded = [0u8; 32];
        padded[12..32].copy_from_slice(account.as_slice());
        calldata.extend_from_slice(&padded);

        // HAZARD (EIP-3607, latent): `caller: account` only passes validation
        // while `account` is code-less. The day a REAL FlashLoanExecutor
        // contract sits at that address (or an EIP-7702 delegation
        // 0xef0100... is set on it), this read trips RejectCallerWithCode —
        // the same class of failure A.4 uncovered in read_amounts_out
        // (caller: router, fixed to Address::ZERO in PR #431). Use a
        // code-less view caller here when the FLE becomes a contract.
        self.evm.env.tx = TxEnv {
            caller: account,
            transact_to: TransactTo::Call(token),
            data: Bytes::copy_from_slice(&calldata),
            value: U256::ZERO,
            gas_price: U256::ZERO,
            gas_limit: 5_000_000,
            nonce: None,
            chain_id: None,
            ..TxEnv::default()
        };

        let result = self
            .evm
            .transact()
            .map_err(|e| SequenceError::TransactInfra(format!("{e}")))?;

        let amount = match result.result {
            ExecutionResult::Success { output, .. } => {
                let bytes = output.into_data();
                if bytes.len() < 32 {
                    return Err(SequenceError::BalanceDecodeFailed { len: bytes.len() });
                }
                U256::from_be_slice(&bytes[..32])
            }
            ExecutionResult::Revert { output, .. } => {
                return Err(SequenceError::BalanceReadReverted(decode_revert_reason(
                    &output,
                )));
            }
            ExecutionResult::Halt { reason, .. } => {
                return Err(SequenceError::BalanceReadHalted(format!("{reason:?}")));
            }
        };

        debug!(
            event = "sequence_runner.read_balance",
            label = %label,
            token = ?token,
            account = ?account,
            amount = %amount,
        );
        self.reads.insert(label.to_string(), amount);
        Ok(amount)
    }

    /// Quote the forward leg via a non-committing `getAmountsOut(amountIn,
    /// path)` `transact()` against `router`, returning the LAST element of the
    /// returned `uint256[]` (the final output amount of the path — i.e. the
    /// intermediate `token_out` produced by swapping `amount_in` of the first
    /// token along `path`).
    ///
    /// This MIRRORS `read_balance`: a non-committing `transact()` (it does NOT
    /// bump `gas_used_total`, does NOT increment `successful_calls`, and does
    /// NOT mutate the CacheDB), so calling it before the committed forward swap
    /// does not perturb the reserves that swap subsequently reads — both read
    /// the same forked state.
    ///
    /// Fail-closed on every adverse path (revert / halt / too-short return /
    /// empty array) with a dedicated `SequenceError` variant.
    pub fn read_amounts_out(
        &mut self,
        router: Address,
        amount_in: U256,
        path: &[Address],
    ) -> Result<U256, SequenceError> {
        let calldata = build_get_amounts_out_calldata(amount_in, path);

        self.evm.env.tx = TxEnv {
            caller: router,
            transact_to: TransactTo::Call(router),
            data: Bytes::copy_from_slice(&calldata),
            value: U256::ZERO,
            gas_price: U256::ZERO,
            gas_limit: 5_000_000,
            nonce: None,
            chain_id: None,
            ..TxEnv::default()
        };

        let result = self
            .evm
            .transact()
            .map_err(|e| SequenceError::TransactInfra(format!("{e}")))?;

        let amount = match result.result {
            ExecutionResult::Success { output, .. } => {
                let bytes = output.into_data();
                decode_amounts_out_last(&bytes)?
            }
            ExecutionResult::Revert { output, .. } => {
                return Err(SequenceError::AmountsOutReverted(decode_revert_reason(
                    &output,
                )));
            }
            ExecutionResult::Halt { reason, .. } => {
                return Err(SequenceError::AmountsOutHalted(format!("{reason:?}")));
            }
        };

        debug!(
            event = "sequence_runner.read_amounts_out",
            router = ?router,
            amount_in = %amount_in,
            path_len = path.len(),
            amount_out = %amount,
        );
        Ok(amount)
    }

    /// Consume the context and return a `SequenceResult` summary. The
    /// trace hash is `[0; 32]` if zero successful calls were committed —
    /// the caller treats that as a structural failure (no SIM_SUCCESS).
    pub fn finalize(self) -> SequenceResult {
        let trace_hash = if self.successful_calls == 0 {
            [0u8; 32]
        } else {
            let result = self.hasher.finalize();
            let mut out = [0u8; 32];
            out.copy_from_slice(&result);
            out
        };
        SequenceResult {
            gas_used_total: self.gas_used_total,
            trace_hash,
            reads: self.reads,
            successful_calls: self.successful_calls,
        }
    }

    /// Read-only access to the reads map (e.g., to resolve an
    /// `AmountSource::FromReadLabel` BEFORE building the backward call).
    pub fn reads(&self) -> &HashMap<String, U256> {
        &self.reads
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Attempt to decode a standard Solidity `Error(string)` revert payload;
/// fall back to a hex-encoded leading-32-byte slice. Matches the
/// behaviour of `revm_runner::decode_revert_reason` for consistency.
fn decode_revert_reason(output: &Bytes) -> String {
    const ERROR_SELECTOR: [u8; 4] = [0x08, 0xc3, 0x79, 0xa0];
    if output.len() >= 68 && output[0..4] == ERROR_SELECTOR {
        if let Ok(len_bytes) = <[u8; 4]>::try_from(&output[64..68]) {
            let msg_len = u32::from_be_bytes(len_bytes) as usize;
            if output.len() >= 68 + msg_len {
                if let Ok(s) = std::str::from_utf8(&output[68..68 + msg_len]) {
                    return s.to_owned();
                }
            }
        }
    }
    let end = output.len().min(32);
    format!("0x{}", hex::encode(&output[..end]))
}

/// Build ERC-20 `balanceOf(account)` calldata (exposed for tests + the
/// orchestrator's anti-fraud guards).
pub fn build_balance_of_calldata(account: Address) -> Vec<u8> {
    let mut calldata: Vec<u8> = Vec::with_capacity(36);
    calldata.extend_from_slice(&[0x70, 0xa0, 0x82, 0x31]);
    let mut padded = [0u8; 32];
    padded[12..32].copy_from_slice(account.as_slice());
    calldata.extend_from_slice(&padded);
    calldata
}

/// Build UniswapV2-style `getAmountsOut(uint256,address[])` calldata
/// (selector `0xd06ca61f`, cast-verified). ABI head: `amountIn` (word 0) +
/// the offset to the dynamic `address[]` data (word 1 = `0x40`); tail: the
/// array length then each path address (left-padded to 32 bytes). Exposed for
/// the orchestrator's anti-fraud guards + tests.
pub fn build_get_amounts_out_calldata(amount_in: U256, path: &[Address]) -> Vec<u8> {
    let mut calldata: Vec<u8> = Vec::with_capacity(4 + 32 * (3 + path.len()));
    // selector 0xd06ca61f
    calldata.extend_from_slice(&[0xd0, 0x6c, 0xa6, 0x1f]);
    // word 0: amountIn (uint256, big-endian).
    calldata.extend_from_slice(&amount_in.to_be_bytes::<32>());
    // word 1: offset to the address[] tail = 0x40 (two head words precede it).
    let mut offset = [0u8; 32];
    offset[31] = 0x40;
    calldata.extend_from_slice(&offset);
    // tail word 0: array length.
    let mut len = [0u8; 32];
    len[24..32].copy_from_slice(&(path.len() as u64).to_be_bytes());
    calldata.extend_from_slice(&len);
    // tail words 1..: each address left-padded to 32 bytes.
    for addr in path {
        let mut padded = [0u8; 32];
        padded[12..32].copy_from_slice(addr.as_slice());
        calldata.extend_from_slice(&padded);
    }
    calldata
}

/// Decode the ABI-encoded `uint256[]` return blob of `getAmountsOut` and
/// return its LAST element (the final output amount of the path). Layout:
/// word 0 = offset to the array data, then `[length, elem_0, elem_1, …]` at
/// that offset. Fails closed on a too-short header or an empty array.
fn decode_amounts_out_last(bytes: &[u8]) -> Result<U256, SequenceError> {
    // Need at least the offset word + the length word reachable at that offset.
    if bytes.len() < 64 {
        return Err(SequenceError::AmountsOutDecodeFailed { len: bytes.len() });
    }
    // word 0: offset to the array data (in bytes, from the start of the blob).
    let data_offset = U256::from_be_slice(&bytes[0..32]);
    // Offsets are bounded by the blob length in any well-formed return; reject
    // anything that would index past the buffer (fail closed, no panic).
    let data_offset: usize = match usize::try_from(data_offset) {
        Ok(o) => o,
        Err(_) => return Err(SequenceError::AmountsOutDecodeFailed { len: bytes.len() }),
    };
    let len_end = match data_offset.checked_add(32) {
        Some(e) => e,
        None => return Err(SequenceError::AmountsOutDecodeFailed { len: bytes.len() }),
    };
    if bytes.len() < len_end {
        return Err(SequenceError::AmountsOutDecodeFailed { len: bytes.len() });
    }
    let array_len = U256::from_be_slice(&bytes[data_offset..len_end]);
    let array_len: usize = match usize::try_from(array_len) {
        Ok(n) => n,
        Err(_) => return Err(SequenceError::AmountsOutDecodeFailed { len: bytes.len() }),
    };
    if array_len == 0 {
        return Err(SequenceError::AmountsOutEmptyArray);
    }
    // Last element starts at data_offset + 32 (length word) + 32*(len-1).
    let last_start = match len_end.checked_add(32usize.saturating_mul(array_len - 1)) {
        Some(s) => s,
        None => return Err(SequenceError::AmountsOutDecodeFailed { len: bytes.len() }),
    };
    let last_end = match last_start.checked_add(32) {
        Some(e) => e,
        None => return Err(SequenceError::AmountsOutDecodeFailed { len: bytes.len() }),
    };
    if bytes.len() < last_end {
        return Err(SequenceError::AmountsOutDecodeFailed { len: bytes.len() });
    }
    Ok(U256::from_be_slice(&bytes[last_start..last_end]))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::lazy_db::seed_account;
    use revm::primitives::{AccountInfo, Bytecode, KECCAK_EMPTY};

    fn lazy_offline() -> LazyDb {
        LazyDb::new("http://127.0.0.1:1/never", Some(1))
            .expect("LazyDb::new accepts a well-formed URL")
    }

    /// The critical trait-bound check — confirms `SequenceContext` can be
    /// constructed against `LazyDb` (which means `CacheDB<LazyDb>` works,
    /// which means `DatabaseRef for LazyDb` is in effect).
    #[test]
    fn sequence_context_constructs_from_lazy_db() {
        let lazy = lazy_offline();
        let _ctx = SequenceContext::new(lazy, 1, 100);
    }

    /// `apply_storage` succeeds without touching the chain when the
    /// account is already seeded in the cache.
    #[test]
    fn apply_storage_succeeds_for_seeded_account() {
        let lazy = lazy_offline();
        let token = Address::from([0xaau8; 20]);
        seed_account(
            &lazy,
            token,
            AccountInfo {
                balance: U256::ZERO,
                nonce: 0,
                code_hash: KECCAK_EMPTY,
                code: Some(Bytecode::new()),
            },
        );
        let mut ctx = SequenceContext::new(lazy, 1, 100);
        ctx.apply_storage(StorageOverride {
            contract: token,
            slot: U256::from(7u64),
            value: U256::from(42u64),
            label: "test",
        })
        .expect("storage override should succeed on seeded account");
    }

    /// `finalize` returns trace_hash = [0; 32] when no successful calls
    /// were committed. Anti-fraud guard.
    #[test]
    fn finalize_with_zero_calls_returns_zero_trace_hash() {
        let lazy = lazy_offline();
        let ctx = SequenceContext::new(lazy, 1, 100);
        let result = ctx.finalize();
        assert_eq!(result.trace_hash, [0u8; 32]);
        assert_eq!(result.successful_calls, 0);
        assert_eq!(result.gas_used_total, 0);
        assert!(result.reads.is_empty());
    }

    /// `build_balance_of_calldata` is deterministic and starts with the
    /// canonical ERC-20 selector.
    #[test]
    fn build_balance_of_calldata_starts_with_canonical_selector() {
        let account = Address::from([0x11u8; 20]);
        let calldata = build_balance_of_calldata(account);
        assert_eq!(calldata.len(), 36);
        assert_eq!(&calldata[0..4], &[0x70, 0xa0, 0x82, 0x31]);
        // Account at right-aligned position in the 32-byte slot.
        assert_eq!(&calldata[4..16], &[0u8; 12]);
        assert_eq!(&calldata[16..36], &[0x11u8; 20]);
    }

    /// Two different accounts produce different calldata (selector +
    /// padded address determinism).
    #[test]
    fn build_balance_of_calldata_differs_per_account() {
        let a = Address::from([0x11u8; 20]);
        let b = Address::from([0x22u8; 20]);
        assert_ne!(build_balance_of_calldata(a), build_balance_of_calldata(b),);
    }

    /// `getAmountsOut(uint256,address[])` selector is the cast-verified
    /// 0xd06ca61f, and the ABI head/tail layout is exactly: amountIn, offset
    /// (0x40), length, then each padded path address.
    #[test]
    fn build_get_amounts_out_calldata_layout_and_selector() {
        let token_a = Address::from([0xaau8; 20]);
        let token_b = Address::from([0xbbu8; 20]);
        let amount_in = U256::from(1_000_000u64);
        let calldata = build_get_amounts_out_calldata(amount_in, &[token_a, token_b]);

        // Selector (cast sig "getAmountsOut(uint256,address[])" == 0xd06ca61f).
        assert_eq!(&calldata[0..4], &[0xd0, 0x6c, 0xa6, 0x1f]);
        // 4 selector + 5 words (amountIn, offset, length, addr1, addr2) = 4 + 32*5.
        assert_eq!(calldata.len(), 4 + 32 * 5);
        // word 0: amountIn.
        assert_eq!(&calldata[4..36], &amount_in.to_be_bytes::<32>());
        // word 1: offset == 0x40.
        let mut expected_offset = [0u8; 32];
        expected_offset[31] = 0x40;
        assert_eq!(&calldata[36..68], &expected_offset);
        // word 2: array length == 2.
        let mut expected_len = [0u8; 32];
        expected_len[31] = 2;
        assert_eq!(&calldata[68..100], &expected_len);
        // word 3 / word 4: padded path addresses.
        assert_eq!(&calldata[100..112], &[0u8; 12]);
        assert_eq!(&calldata[112..132], token_a.as_slice());
        assert_eq!(&calldata[132..144], &[0u8; 12]);
        assert_eq!(&calldata[144..164], token_b.as_slice());
    }

    /// `decode_amounts_out_last` decodes a hand-built `uint256[]` return blob
    /// (offset 0x20, length 3, three elements) to its LAST element.
    #[test]
    fn decode_amounts_out_last_returns_final_element() {
        let elems = [
            U256::from(1_000_000u64),
            U256::from(2_500u64),
            U256::from(987_654_321u64), // the final output amount of the path.
        ];
        let mut blob: Vec<u8> = Vec::new();
        // word 0: offset to the array data == 0x20.
        let mut offset = [0u8; 32];
        offset[31] = 0x20;
        blob.extend_from_slice(&offset);
        // array length == 3.
        let mut len = [0u8; 32];
        len[31] = elems.len() as u8;
        blob.extend_from_slice(&len);
        // the elements.
        for e in &elems {
            blob.extend_from_slice(&e.to_be_bytes::<32>());
        }
        let last = decode_amounts_out_last(&blob).expect("well-formed uint256[] decodes");
        assert_eq!(last, U256::from(987_654_321u64));
    }

    /// `decode_amounts_out_last` fails closed on a too-short header and on an
    /// empty array (no panic, dedicated error variants).
    #[test]
    fn decode_amounts_out_last_fails_closed() {
        // Too short: only the offset word, no length reachable.
        assert!(matches!(
            decode_amounts_out_last(&[0u8; 16]),
            Err(SequenceError::AmountsOutDecodeFailed { .. })
        ));
        // Empty array: offset word == 0x20, length word (bytes 32..64) stays
        // zero → empty.
        let mut blob = vec![0u8; 64];
        blob[31] = 0x20;
        assert!(matches!(
            decode_amounts_out_last(&blob),
            Err(SequenceError::AmountsOutEmptyArray)
        ));
    }

    /// `decode_revert_reason` parses an ABI-encoded `Error(string)`.
    #[test]
    fn decode_revert_reason_parses_error_string() {
        let msg = b"insufficient_balance";
        let mut bytes = Vec::with_capacity(68 + msg.len());
        bytes.extend_from_slice(&[0x08, 0xc3, 0x79, 0xa0]);
        let mut offset = [0u8; 32];
        offset[31] = 32;
        bytes.extend_from_slice(&offset);
        let mut len = [0u8; 32];
        len[28..32].copy_from_slice(&(msg.len() as u32).to_be_bytes());
        bytes.extend_from_slice(&len);
        bytes.extend_from_slice(msg);
        assert_eq!(
            decode_revert_reason(&Bytes::from(bytes)),
            "insufficient_balance"
        );
    }

    /// Falls back to hex prefix for non-standard revert payloads.
    #[test]
    fn decode_revert_reason_hex_fallback() {
        assert_eq!(
            decode_revert_reason(&Bytes::from(vec![0xde, 0xad, 0xbe, 0xef])),
            "0xdeadbeef"
        );
    }

    /// Error reason_tag stability.
    #[test]
    fn error_reason_tags_are_stable() {
        assert_eq!(SequenceError::EvmDbMissing.reason_tag(), "evm_db_missing");
        assert_eq!(
            SequenceError::StorageOverrideFailed("x".into()).reason_tag(),
            "storage_override_failed"
        );
        assert_eq!(
            SequenceError::TransactInfra("x".into()).reason_tag(),
            "transact_infra"
        );
        assert_eq!(
            SequenceError::BalanceDecodeFailed { len: 0 }.reason_tag(),
            "balance_decode_failed"
        );
        assert_eq!(
            SequenceError::BalanceReadReverted("x".into()).reason_tag(),
            "balance_read_reverted"
        );
        assert_eq!(
            SequenceError::BalanceReadHalted("x".into()).reason_tag(),
            "balance_read_halted"
        );
        assert_eq!(
            SequenceError::AmountsOutDecodeFailed { len: 0 }.reason_tag(),
            "amounts_out_decode_failed"
        );
        assert_eq!(
            SequenceError::AmountsOutEmptyArray.reason_tag(),
            "amounts_out_empty_array"
        );
        assert_eq!(
            SequenceError::AmountsOutReverted("x".into()).reason_tag(),
            "amounts_out_reverted"
        );
        assert_eq!(
            SequenceError::AmountsOutHalted("x".into()).reason_tag(),
            "amounts_out_halted"
        );
    }

    /// CallOutcome::Success classifies correctly.
    #[test]
    fn call_outcome_success_is_success() {
        let s = CallOutcome::Success {
            gas_used: 1,
            output: vec![],
        };
        assert!(s.is_success());
        let r = CallOutcome::Reverted {
            gas_used: 0,
            reason: "x".into(),
        };
        assert!(!r.is_success());
        let h = CallOutcome::Halted {
            gas_used: 0,
            reason: "x".into(),
        };
        assert!(!h.is_success());
    }
}
