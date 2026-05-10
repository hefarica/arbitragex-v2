//! Task 4.3 — `revm_runner`: wires a `revm::Database` into `revm::EVM` and
//! executes a `CandidateInput`, returning a `SimResult`.
//!
//! ## Design notes
//!
//! ### Balance delta for `net_profit_wei`
//! We call `db.basic(caller)` *before* handing the database to `EVM` (we move
//! `db` into `EVM::database()` so we cannot use it afterwards).  This gives us
//! the pre-execution caller balance from the database's own cache / provider.
//! After `transact()` we read the post-execution balance from the returned
//! `state` diff.  If the caller was not touched, post = pre, so profit = 0.
//!
//! ### trace_hash
//! SHA-256 of `(calldata || return_or_revert_bytes)`.  Two identical calls
//! against the same pinned block produce the same digest, enabling cheap
//! deduplication of cached simulation results without re-running revm.
//!
//! ### Gas limit
//! We hard-code `30_000_000` (current Ethereum block limit).  This is high
//! enough for any realistic arbitrage bundle and keeps the simulation bounded.
//!
//! ### Spec
//! We use `SpecId::LATEST` so all opcodes are available.  The spec can be made
//! configurable in a future iteration if we need BSC / Optimism behaviour.

use revm::primitives::{
    Address, BlockEnv, Bytes, CfgEnv, Env, ExecutionResult, SpecId, TransactTo, TxEnv, U256,
};
use revm::Database;
use revm::EVM;
// revm's State type is hashbrown::HashMap (re-exported from revm-primitives).
use revm::primitives::State as RevmState;
use sha2::{Digest, Sha256};
use tracing::{info, warn};

use crate::{CandidateInput, SimError, SimResult};

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Simulate `candidate` using `db` as the backing state.
///
/// `db` must be pinned to the correct block before calling this function.
/// Typically the caller constructs a `LazyDb` and calls `run()` with it.
///
/// `effective_block` must equal the block number `db` is pinned to (MAJOR #6
/// fix): it is written into `BlockEnv.number` so revm sees the same block
/// number that the database serves state for.
///
/// # Errors
/// - `SimError::Reverted(reason)` — EVM executed but reverted.
/// - `SimError::Provider(msg)` — database fetch or revm environmental error.
pub fn run<DB>(candidate: &CandidateInput, mut db: DB, effective_block: u64) -> Result<SimResult, SimError>
where
    DB: Database,
    DB::Error: std::fmt::Display,
{
    let caller = Address::from(candidate.from);

    // Pre-fetch caller balance before moving db into EVM.
    // Why: EVM::database() consumes `db` by value; we cannot query it after.
    // Calling basic() here populates the LazyDb cache so revm's internal
    // call to basic() during execution is a free cache hit.
    let pre_balance = db
        .basic(caller)
        .map_err(|e| SimError::Provider(format!("pre-balance basic({caller}): {e}")))?
        .map(|info| info.balance)
        .unwrap_or(U256::ZERO);

    let to = Address::from(candidate.to);
    let env = build_env(candidate, caller, to, effective_block);

    let mut evm = EVM::with_env(env);
    evm.database(db);

    let result_and_state = evm
        .transact()
        .map_err(|e| SimError::Provider(format!("revm transact: {e}")))?;

    let execution_result = result_and_state.result;
    let post_state = result_and_state.state;

    let (gas_used, output_bytes) = match &execution_result {
        ExecutionResult::Success { gas_used, output, .. } => {
            info!(
                event = "revm_runner.tx_executed",
                status = "success",
                gas_used,
                "simulation succeeded"
            );
            (*gas_used, output.data().clone())
        }
        ExecutionResult::Revert { gas_used, output } => {
            let reason = decode_revert_reason(output);
            warn!(
                event = "revm_runner.tx_reverted",
                gas_used,
                reason = %reason,
                "simulation reverted"
            );
            return Err(SimError::Reverted(reason));
        }
        ExecutionResult::Halt { reason, gas_used } => {
            warn!(
                event = "revm_runner.tx_halted",
                gas_used,
                reason = ?reason,
                "simulation halted"
            );
            return Err(SimError::Reverted(format!("HALT: {reason:?}")));
        }
    };

    let post_balance = extract_post_balance(&caller, &post_state, pre_balance);
    let net_profit_wei = balance_delta(pre_balance, post_balance);
    let trace_hash = compute_trace_hash(&candidate.calldata, &output_bytes);

    Ok(SimResult {
        net_profit_wei,
        gas_used,
        trace_hash,
    })
}

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

/// Build the revm `Env` from the candidate and resolved addresses.
///
/// `effective_block` is the block number agreed between the DB and this env
/// (MAJOR #6 fix). It must equal the block `LazyDb` is pinned to so that
/// `BlockEnv.number` and the DB state snapshot are consistent.
///
/// `candidate.gas_price_wei` is forwarded to `TxEnv.gas_price` so revm
/// deducts actual gas cost from the caller's balance (CRITICAL #2 fix).
/// `SimResult.net_profit_wei` is therefore true net-of-gas P&L (G-NET-1).
fn build_env(candidate: &CandidateInput, caller: Address, to: Address, effective_block: u64) -> Env {
    let mut cfg = CfgEnv::default();
    cfg.chain_id = candidate.chain_id;
    cfg.spec_id = SpecId::LATEST;

    let block = BlockEnv {
        // Use effective_block, not candidate.block_number, so DB and BlockEnv
        // always agree even when candidate.block_number == 0 (MAJOR #6 fix).
        number: U256::from(effective_block),
        ..BlockEnv::default()
    };

    let tx = TxEnv {
        caller,
        gas_limit: 30_000_000,
        // Forward gas price so revm deducts gas from the caller's balance.
        // net_profit_wei = balance_delta = profit_tokens - gas_cost (CRITICAL #2).
        gas_price: U256::from(candidate.gas_price_wei),
        transact_to: TransactTo::Call(to),
        value: U256::from(candidate.value_wei),
        data: Bytes::copy_from_slice(&candidate.calldata),
        nonce: None,
        chain_id: None,
        ..TxEnv::default()
    };

    Env { cfg, block, tx }
}

/// Return the caller's balance from the post-execution `state` diff.
///
/// revm only includes an account in `state` if it was touched during
/// execution.  If the caller account is absent, its balance is unchanged
/// (`pre_balance`).
fn extract_post_balance(
    caller: &Address,
    state: &RevmState,
    pre_balance: U256,
) -> U256 {
    state
        .get(caller)
        .map(|acc| acc.info.balance)
        .unwrap_or(pre_balance)
}

/// Compute a saturating `i128` balance delta: `post - pre`.
///
/// U256 arithmetic: if post ≥ pre the result is non-negative; if post < pre
/// the result is negative.  Saturates at `i128::{MAX, MIN}` rather than
/// panicking or wrapping on extreme values.
fn balance_delta(pre: U256, post: U256) -> i128 {
    if post >= pre {
        let diff = post - pre;
        // i128::MAX as u128 = 170_141_183_460_469_231_731_687_303_715_884_105_727
        if diff > U256::from(i128::MAX as u128) {
            i128::MAX
        } else {
            // Safe: we checked diff ≤ i128::MAX as u128.
            diff.to::<u128>() as i128
        }
    } else {
        let diff = pre - post;
        if diff > U256::from(i128::MAX as u128) {
            i128::MIN
        } else {
            -(diff.to::<u128>() as i128)
        }
    }
}

/// Attempt to decode a standard Solidity revert string from EVM output bytes.
///
/// Standard ABI encoding for `Error(string)`:
///   bytes[0..4]   = 0x08c379a0  (function selector)
///   bytes[4..36]  = ABI offset  (always 32 for a single string)
///   bytes[36..68] = string length (big-endian u256, but practically u32)
///   bytes[68..]   = UTF-8 string data
///
/// If decoding fails we fall back to a hex representation of the first
/// 32 bytes so callers can diagnose the raw revert data.
fn decode_revert_reason(output: &Bytes) -> String {
    const ERROR_SELECTOR: [u8; 4] = [0x08, 0xc3, 0x79, 0xa0];

    if output.len() >= 68 && output[0..4] == ERROR_SELECTOR {
        // Extract string length from bytes[36..68] (last 4 bytes of a
        // 32-byte big-endian integer).
        if let Ok(len_bytes) = <[u8; 4]>::try_from(&output[64..68]) {
            let msg_len = u32::from_be_bytes(len_bytes) as usize;
            if output.len() >= 68 + msg_len {
                if let Ok(s) = std::str::from_utf8(&output[68..68 + msg_len]) {
                    return s.to_owned();
                }
            }
        }
    }

    // Fallback: hex-encode the first 32 bytes.
    let end = output.len().min(32);
    format!("0x{}", hex::encode(&output[..end]))
}

/// Compute `SHA-256(calldata || output_bytes)` as a 32-byte fingerprint.
///
/// This is a deterministic content hash of the simulation's inputs and
/// outputs, not a state trie hash.  Used for:
///   - deduplicating cached simulation results in Redis
///   - correlating log lines with a specific simulation run
fn compute_trace_hash(calldata: &[u8], output: &Bytes) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(calldata);
    hasher.update(output.as_ref());
    let result = hasher.finalize();
    let mut arr = [0u8; 32];
    arr.copy_from_slice(&result);
    arr
}
