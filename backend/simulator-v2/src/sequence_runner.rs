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

use revm::context::{BlockEnv, CfgEnv, TxEnv};
use revm::context_interface::result::ExecutionResult;
use revm::context_interface::{ContextTr, TransactTo};
use revm::database::CacheDB;
use revm::handler::{MainBuilder, MainnetContext, MainnetEvm};
use revm::interpreter::interpreter::EthInterpreter;
use revm::interpreter::interpreter_types::Jumps;
use revm::interpreter::{CallInputs, CreateInputs, CreateOutcome, Interpreter};
use revm::primitives::{hardfork::SpecId, Address, Bytes, U256};
use revm::{Context, InspectCommitEvm, InspectEvm, Inspector, MainContext};
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
    #[error("read_call reverted: {0}")]
    CallReadReverted(String),
    #[error("read_call halted: {0}")]
    CallReadHalted(String),
    #[error("deploy() init_code is empty — a CREATE with no bytecode")]
    CreateInitCodeEmpty,
    #[error("create succeeded but REVM returned no created address")]
    CreateAddressMissing,
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
            Self::CallReadReverted(_) => "call_read_reverted",
            Self::CallReadHalted(_) => "call_read_halted",
            Self::CreateInitCodeEmpty => "create_init_code_empty",
            Self::CreateAddressMissing => "create_address_missing",
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
    Success {
        gas_used: u64,
        output: Vec<u8>,
    },
    /// REVERT-TRACE-01: `frame` names the contract address + pc + opcode of
    /// the frame that reverted (the innermost origin of the unwind).
    /// `None` = not computed (no interpreter frame ran — R8, never a
    /// fabricated empty frame).
    Reverted {
        gas_used: u64,
        reason: String,
        frame: Option<RevertFrame>,
    },
    Halted {
        gas_used: u64,
        reason: String,
    },
}

impl CallOutcome {
    pub fn is_success(&self) -> bool {
        matches!(self, Self::Success { .. })
    }
}

/// A contract-deployment transaction to execute via `transact_commit` with
/// `TxKind::Create`. The runner never synthesises init_code — the caller
/// supplies it (committed bytecode fixtures, G-SIM-1 WO-LR22.13 PR-B).
#[derive(Debug, Clone)]
pub struct SequenceDeploy {
    pub from: Address,
    pub init_code: Vec<u8>,
    pub gas_price_wei: u128,
    pub gas_limit: u64,
    /// Operator-visible label for logs (`"deploy_arbitrage_executor"`, etc).
    pub label: &'static str,
}

/// Outcome of a single `deploy()`. `Success` carries the CREATED address
/// decoded from REVM's `Output::Create` — never computed client-side.
#[derive(Debug, Clone)]
pub enum DeployOutcome {
    Success {
        address: Address,
        gas_used: u64,
    },
    /// REVERT-TRACE-01: `frame` names the reverting init-code frame
    /// (`address = "<create>"`, with its pc + opcode). `None` = not
    /// computed (R8).
    Reverted {
        gas_used: u64,
        reason: String,
        frame: Option<RevertFrame>,
    },
    Halted {
        gas_used: u64,
        reason: String,
    },
}

impl DeployOutcome {
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
///
/// `I` is the revm inspector attached to the inner `Evm`. The PRODUCTION
/// default ([`SequenceContext::new`]) is now the REVERT-TRACE-01
/// `RevertFrameInspector` — the revert stops being anonymous: every
/// `call()`/`deploy()` revert carries the failing frame's address + pc +
/// opcode, on the SAME single execution path (still no fork of the
/// production context, which is the recurring defect this repo keeps
/// paying for). `NoopInspector` remains available for callers that want a
/// zero-hook run, and the verbose stack-top tracer stays
/// `revert-trace`-gated.
pub struct SequenceContext<I = NoopInspector> {
    evm: MainnetEvm<MainnetContext<CacheDB<LazyDb>>, I>,
    reads: HashMap<String, U256>,
    gas_used_total: u64,
    hasher: Sha256,
    successful_calls: u64,
    /// REVERT-TRACE-01: reverting frames whose reverts were HANDLED by a
    /// parent frame (the transact still succeeded) — retained for
    /// diagnostics, never fatal. R8: empty = none observed, never
    /// fabricated.
    handled_revert_frames: Vec<RevertFrame>,
}

/// EVM block env for the paper path, taken from the pinned block header.
///
/// `timestamp` is the load-bearing field: with the default 0, any contract
/// that subtracts `block.timestamp - lastUpdate` — Aave's
/// `ReserveLogic.updateState` does exactly that on every reserve touch —
/// underflows and reverts `Panic(0x11)` before a single swap runs. That made
/// 100 % of the variance-benchmark population revert, amount- and
/// route-invariant. The other fields track the header so the env is not a
/// "genesis" context either, EXCEPT `basefee`: the paper path forwards an
/// explicit `gas_price` and revm rejects a tx whose gas price is below the
/// base fee, so the base fee is deliberately left at its default.
fn block_env_from_header(
    header: &ethers::types::Block<ethers::types::TxHash>,
    block_number: u64,
) -> BlockEnv {
    let mut block = BlockEnv {
        number: U256::from(block_number),
        ..BlockEnv::default()
    };
    block.timestamp = U256::from(header.timestamp.as_u64());
    if header.gas_limit <= ethers::types::U256::from(u64::MAX) {
        block.gas_limit = header.gas_limit.as_u64();
    }
    if let Some(a) = header.author {
        block.beneficiary = Address::from_slice(a.as_bytes());
    }
    if let Some(m) = header.mix_hash {
        block.prevrandao = Some(revm::primitives::B256::from(m.0));
    }
    block
}

/// No-op inspector: a zero-size type whose every hook is the `Inspector` trait
/// default. It exists only so `SequenceContext` can carry an inspector-capable
/// `Evm` without changing production behaviour or paying a runtime hook body.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoopInspector;

impl<CTX> Inspector<CTX, EthInterpreter> for NoopInspector {}

/// Bound for `SequenceContext<I>`. Implemented for every revm `Inspector`
/// over the mainnet context that is also `Default`, so both `NoopInspector`
/// and the production `RevertFrameInspector` fit without per-call-site
/// bounds (and without the signature ripple reaching every helper).
///
/// REVERT-TRACE-01: the bound now carries `take_revert_frames` so the
/// generic `call()`/`deploy()` can drain whatever the attached inspector
/// recorded. `NoopInspector` yields none (nothing is recorded); the frame
/// inspector yields the reverting frames of the last transact.
pub trait ContextInspector:
    Inspector<MainnetContext<CacheDB<LazyDb>>, EthInterpreter> + Default
{
    /// Drain the reverting frames recorded since the last transact
    /// (appearance order, innermost-first). Empty for inspectors that do
    /// not record frames — R8: absence of a frame is NEVER fabricated.
    fn take_revert_frames(&mut self) -> Vec<RevertFrame>;
}

impl ContextInspector for NoopInspector {
    fn take_revert_frames(&mut self) -> Vec<RevertFrame> {
        Vec::new()
    }
}

// ---------------------------------------------------------------------------
// REVERT-TRACE-01 — the production revert frame (address + pc + opcode)
// ---------------------------------------------------------------------------
//
// REA provenance (docs/rea/FICHA_REA_TRACE.md):
//   * frame semantics (innermost-first attribution, handled-vs-fatal):
//     flashbots/mev-inspect-rs@b6e2f0a4784d063375734e2773fa7196cf43174d
//     (archived; Tier 3 REFERENCE ONLY — zero lines of its code are
//     ported, and its parity traces carry no pc/opcode at all);
//   * Panic(uint256) selector 0x4e487b71, the ten canonical panic codes
//     and REVERT = instruction 0xfd: Solidity docs,
//     "Panic via assert and Error via require"
//     (docs.soliditylang.org/en/latest/control-structures.html);
//   * the decode vectors are DERIVED (docs/rea/verifica_vectores_rea_trace.py,
//     22/22 OK) — no upstream candidate carries an address+pc+opcode
//     vector; declared in the ficha, transcribed byte-literal into the
//     tests below.
//
// The revert was anonymous in production: `CallOutcome::Reverted { reason }`
// carried only the top-level decoded string, and the tracer that names the
// failing frame was `revert-trace`-gated (test-only). This section makes
// the frame a PRODUCTION artifact: ONE execution path (no noop-vs-tracer
// fork), captured on revert only.

/// Structured classification of revert data. The DISPLAY of
/// [`decode_revert_reason`] — the string that reaches `fail_reason` and the
/// three baseline tests (`decode_revert_reason_parses_error_string`,
/// `decode_revert_reason_hex_fallback`,
/// `decode_revert_reason_keeps_the_panic_code`) — is UNCHANGED; the
/// classification travels here, beside it (ficha §2.1: `deadbeef` is both a
/// valid custom-error selector and raw data — the ambiguity is not
/// resolvable without an ABI, so the display keeps the hex fallback while
/// the kind records the structured verdict).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RevertKind {
    /// ABI `Error(string)` (selector `0x08c379a0`) with the decoded UTF-8
    /// message — `require(cond, "msg")` / `revert("msg")`.
    ErrorString(String),
    /// ABI `Panic(uint256)` (selector `0x4e487b71`, exact 36-byte payload).
    /// `named` is `Some` for the ten canonical Solidity panic codes
    /// (0x00, 0x01, 0x11, 0x12, 0x21, 0x22, 0x31, 0x32, 0x41, 0x51).
    Panic {
        code: u64,
        named: Option<&'static str>,
    },
    /// 4+ bytes that are neither Error nor Panic — a custom-error selector
    /// (`revert CustomError(...)`) or raw data. NOT nameable without the
    /// contract ABI; only the selector is recorded.
    Selector([u8; 4]),
    /// Non-empty payload shorter than 4 bytes (no selector fits).
    Raw,
    /// `revert()` / `require(false)` — no data at all, not even a selector.
    Empty,
}

impl RevertKind {
    /// The ten canonical `Panic(uint256)` code names, verbatim from the
    /// Solidity docs table ("Panic via assert and Error via require").
    pub fn panic_code_name(code: u64) -> Option<&'static str> {
        match code {
            0x00 => Some("generic compiler panic"),
            0x01 => Some("assert failed"),
            0x11 => Some("arithmetic underflow/overflow"),
            0x12 => Some("division or modulo by zero"),
            0x21 => Some("enum conversion out of range"),
            0x22 => Some("storage byte array incorrectly encoded"),
            0x31 => Some("pop on empty array"),
            0x32 => Some("array index out of bounds"),
            0x41 => Some("too much memory allocated"),
            0x51 => Some("zero-initialized internal function called"),
            _ => None,
        }
    }

    /// Classify revert payload bytes (the dispatch of ficha §2). Mirrors
    /// `decode_revert_reason`'s `Error(string)` validity checks (selector +
    /// length bounds + UTF-8) so the structured kind and the display string
    /// never disagree about what they saw.
    pub fn classify(data: &[u8]) -> Self {
        const ERROR_SELECTOR: [u8; 4] = [0x08, 0xc3, 0x79, 0xa0];
        const PANIC_SELECTOR: [u8; 4] = [0x4e, 0x48, 0x7b, 0x71];
        if data.is_empty() {
            return Self::Empty;
        }
        if data.len() >= 68 && data[0..4] == ERROR_SELECTOR {
            if let Ok(len_bytes) = <[u8; 4]>::try_from(&data[64..68]) {
                let msg_len = u32::from_be_bytes(len_bytes) as usize;
                if data.len() >= 68 + msg_len {
                    if let Ok(s) = std::str::from_utf8(&data[68..68 + msg_len]) {
                        return Self::ErrorString(s.to_owned());
                    }
                }
            }
        }
        if data.len() == 36 && data[0..4] == PANIC_SELECTOR {
            // Full 32-byte ABI word (the canonical codes fit one byte; a
            // wider word saturates instead of being truncated to its low
            // byte, and stays `named: None`).
            let word = U256::from_be_slice(&data[4..36]);
            let code = u64::try_from(word).unwrap_or(u64::MAX);
            return Self::Panic {
                code,
                named: Self::panic_code_name(code),
            };
        }
        if data.len() >= 4 {
            let mut selector = [0u8; 4];
            selector.copy_from_slice(&data[0..4]);
            return Self::Selector(selector);
        }
        Self::Raw
    }

    /// Human display of the classification (the derived-vector model of
    /// `docs/rea/verifica_vectores_rea_trace.py::decode_revert`). For
    /// `Selector`/`Raw` the display is the hex fallback of the payload
    /// (capped at 36 bytes, same rule as `decode_revert_reason`), because
    /// without an ABI the bytes carry no nameable meaning.
    pub fn display(&self, data: &[u8]) -> String {
        match self {
            Self::Empty => "empty".to_string(),
            Self::ErrorString(msg) => format!("Error(string): {msg}"),
            Self::Panic { code, named } => match named {
                Some(n) => format!("Panic(0x{code:02x}): {n}"),
                None => format!("Panic(0x{code:02x}): unknown code"),
            },
            Self::Selector(_) | Self::Raw => {
                let end = data.len().min(36);
                format!("0x{}", hex::encode(&data[..end]))
            }
        }
    }
}

/// The frame that produced a revert — REVERT-TRACE-01. The revert stops
/// being anonymous: `address` is the EXECUTING code address of the frame
/// that reverted (revm `inputs.bytecode_address` at `call_end`;
/// `"<create>"` for a reverting contract-creation frame), `pc`/`opcode`
/// are the last instruction that frame executed (the `REVERT` itself —
/// `0xfd` — per the Solidity docs' revert instruction, or `INVALID`
/// `0xfe` on legacy compiler panics).
///
/// Attribution (mev-inspect-rs semantics, ficha §3): frames unwrap
/// innermost-first, so the FIRST reverting frame observed is the ORIGIN —
/// a `Panic(uint256)` is attributed to the contract that panicked, not to
/// the `catch` that re-encoded it higher up the unwind. `fatal` is `true`
/// ONLY on the frame carried by a top-level `Reverted` outcome; a subcall
/// revert the parent HANDLED (low-level call returning false, try/catch)
/// is recorded with `fatal = false` and never fails the sequence — a
/// handled subtrace revert does not imply a reverted transaction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RevertFrame {
    /// Executing code address of the reverting frame (revm `Address` debug
    /// format).
    pub address: String,
    /// Call depth (0 = top-level).
    pub depth: usize,
    /// Program counter of the last instruction the frame executed.
    pub pc: usize,
    /// Opcode at that pc (`0xfd` REVERT / `0xfe` INVALID / ...).
    pub opcode: u8,
    /// Structured classification of the revert payload.
    pub kind: RevertKind,
    /// Raw revert payload (kept verbatim; revert data is tiny by nature).
    pub data: Vec<u8>,
    /// `true` only when this frame is the origin of a FATAL unwind (the
    /// top-level outcome itself reverted). `false` = handled subcall revert.
    pub fatal: bool,
}

impl RevertFrame {
    /// Raw payload as `0x`-prefixed hex (diagnostics display).
    pub fn data_hex(&self) -> String {
        format!("0x{}", hex::encode(&self.data))
    }

    /// Compact one-line annotation for the persisted `fail_reason`:
    /// `@<address> pc=0x<pc> op=0x<opcode> <kind display>` — the three
    /// coordinates that name the frame plus its classification.
    pub fn annotation(&self) -> String {
        format!(
            "@{} pc=0x{:x} op=0x{:02x} {}",
            self.address,
            self.pc,
            self.opcode,
            self.kind.display(&self.data)
        )
    }
}

/// The innermost reverting frame's last executed instruction.
#[derive(Debug, Clone, Copy)]
struct LastStep {
    pc: usize,
    opcode: u8,
}

/// REVERT-TRACE-01 production inspector: records the reverting frames of
/// every transact — address, depth, pc, opcode, structured kind — with
/// per-step bookkeeping limited to the last instruction's `pc`/`opcode`
/// (no per-step stack snapshot: the 12-word stack top stays in the
/// `revert-trace`-gated tracer for panic-operand forensics).
/// `call()`/`deploy()` drain the recorded frames after each transact and
/// attach the origin (first, innermost) to the `Reverted` outcome; frames
/// whose reverts were handled by a parent are exposed via
/// [`SequenceContext::handled_revert_frames`] instead.
#[derive(Debug, Default)]
pub struct RevertFrameInspector {
    depth: usize,
    last: Option<LastStep>,
    frames: Vec<RevertFrame>,
}

impl RevertFrameInspector {
    /// Drain the frames recorded since the last transact.
    fn take_frames(&mut self) -> Vec<RevertFrame> {
        std::mem::take(&mut self.frames)
    }

    fn capture(&mut self, address: String, data: &[u8]) {
        let last = self.last;
        // `self.depth` counts the call hooks entered (the top-level frame
        // fires `call` too, so it sits at 1); the reported depth is the
        // frame NESTING level (0 = top-level, 1 = one below, ...).
        let depth = self.depth.saturating_sub(1);
        self.frames.push(RevertFrame {
            address,
            depth,
            pc: last.map(|s| s.pc).unwrap_or(0),
            opcode: last.map(|s| s.opcode).unwrap_or(0),
            kind: RevertKind::classify(data),
            data: data.to_vec(),
            fatal: false,
        });
    }
}

impl<CTX> Inspector<CTX, EthInterpreter> for RevertFrameInspector {
    fn step(&mut self, interp: &mut Interpreter<EthInterpreter>, _context: &mut CTX) {
        self.last = Some(LastStep {
            pc: interp.bytecode.pc(),
            opcode: interp.bytecode.opcode(),
        });
    }

    fn call(
        &mut self,
        _context: &mut CTX,
        _inputs: &mut CallInputs,
    ) -> Option<revm::interpreter::CallOutcome> {
        self.depth += 1;
        None
    }

    fn call_end(
        &mut self,
        _context: &mut CTX,
        inputs: &CallInputs,
        outcome: &mut revm::interpreter::CallOutcome,
    ) {
        if outcome.result.result.is_revert() {
            let address = format!("{:?}", inputs.bytecode_address);
            let data = outcome.result.output.clone();
            self.capture(address, data.as_ref());
        }
        self.depth = self.depth.saturating_sub(1);
    }

    fn create(&mut self, _context: &mut CTX, _inputs: &mut CreateInputs) -> Option<CreateOutcome> {
        self.depth += 1;
        None
    }

    fn create_end(
        &mut self,
        _context: &mut CTX,
        _inputs: &CreateInputs,
        outcome: &mut CreateOutcome,
    ) {
        if outcome.result.result.is_revert() {
            let data = outcome.result.output.clone();
            self.capture("<create>".to_string(), data.as_ref());
        }
        self.depth = self.depth.saturating_sub(1);
    }
}

impl ContextInspector for RevertFrameInspector {
    fn take_revert_frames(&mut self) -> Vec<RevertFrame> {
        self.take_frames()
    }
}

/// SIM-REVERT-TRACE-01 (feature `revert-trace`): a test-only inspector that
/// names the INNERMOST frame that reverted — address, depth, pc, opcode and
/// the raw revert data — and prints one greppable
/// `VARIANCE_BENCH_REVERT_FRAME=` line. The first `call_end`/`create_end`
/// whose result is a revert is the origin (frames unwrap innermost-first), so
/// a `Panic(uint256)` is attributed to the contract that PANICKED, not to the
/// `catch` that re-encoded it higher up. Compiled out entirely in production.
#[cfg(feature = "revert-trace")]
pub use revert_trace::RevertTracer;

#[cfg(feature = "revert-trace")]
mod revert_trace {
    use super::{EthInterpreter, Inspector};
    use revm::interpreter::interpreter_types::{InputsTr, Jumps};
    use revm::interpreter::{CallInputs, CreateInputs, CreateOutcome, Interpreter};

    /// The last instruction observed in the currently-executing frame.
    #[derive(Debug, Clone)]
    struct StepPoint {
        address: String,
        depth: usize,
        pc: usize,
        opcode: u8,
        stack_top: Option<String>,
        /// Top-of-stack words (top first), so the operands of a failed
        /// checked-arithmetic check remain visible at the shared panic REVERT.
        stack: Vec<String>,
    }

    /// The frame that produced the revert (the origin of the unwind).
    #[derive(Debug, Clone, Default)]
    pub struct OriginFrame {
        /// Executing code address of the reverting frame.
        pub address: String,
        /// Call depth (0 = top-level).
        pub depth: usize,
        /// Program counter of the last instruction before the revert.
        pub pc: usize,
        /// Opcode at that pc.
        pub opcode: u8,
        /// Top of the stack at that instruction (hex), when available.
        pub stack_top: Option<String>,
        /// Raw revert data the frame returned (hex).
        pub data: String,
    }

    /// Test-only tracer. Records the FIRST reverting frame (innermost) and
    /// prints it once.
    #[derive(Debug, Default)]
    pub struct RevertTracer {
        depth: usize,
        last: Option<StepPoint>,
        origin: Option<OriginFrame>,
        seen: usize,
    }

    impl RevertTracer {
        fn capture(&mut self, address: &str, data: &[u8]) {
            let seq = self.seen;
            self.seen += 1;
            let (pc, opcode, stack_top, depth, stack, step_target) = match &self.last {
                Some(s) => (
                    s.pc,
                    s.opcode,
                    s.stack_top.clone(),
                    s.depth,
                    s.stack.clone(),
                    Some(s.address.clone()),
                ),
                None => (0, 0, None, self.depth, Vec::new(), None),
            };
            let data_hex = format!("0x{}", hex::encode(data));
            // One line per reverting frame, innermost-first, so the whole
            // unwind chain is visible — not just the first.
            println!(
                "VARIANCE_BENCH_REVERT_FRAME={}",
                serde_json::json!({
                    "seq": seq,
                    "address": address,
                    "step_target": step_target,
                    "depth": depth,
                    "pc": pc,
                    "opcode": format!("0x{opcode:02x}"),
                    "stack_top": stack_top,
                    "stack": stack,
                    "data": data_hex,
                })
            );
            if self.origin.is_none() {
                self.origin = Some(OriginFrame {
                    address: address.to_string(),
                    depth,
                    pc,
                    opcode,
                    stack_top,
                    data: data_hex,
                });
            }
        }

        /// The captured origin frame, if a revert was seen.
        pub fn origin(&self) -> Option<&OriginFrame> {
            self.origin.as_ref()
        }
    }

    impl<CTX> Inspector<CTX, EthInterpreter> for RevertTracer {
        fn step(&mut self, interp: &mut Interpreter<EthInterpreter>, _context: &mut CTX) {
            let data = interp.stack.data();
            let stack: Vec<String> = data
                .iter()
                .rev()
                .take(12)
                .map(|v| format!("0x{v:x}"))
                .collect();
            let stack_top = stack.first().cloned();
            self.last = Some(StepPoint {
                address: format!("{:?}", interp.input.target_address()),
                depth: self.depth,
                pc: interp.bytecode.pc(),
                opcode: interp.bytecode.opcode(),
                stack_top,
                stack,
            });
        }

        fn call(
            &mut self,
            _context: &mut CTX,
            _inputs: &mut CallInputs,
        ) -> Option<revm::interpreter::CallOutcome> {
            self.depth += 1;
            None
        }

        fn call_end(
            &mut self,
            _context: &mut CTX,
            inputs: &CallInputs,
            outcome: &mut revm::interpreter::CallOutcome,
        ) {
            if outcome.result.result.is_revert() {
                let address = format!("{:?}", inputs.bytecode_address);
                let data = outcome.result.output.clone();
                self.capture(&address, data.as_ref());
            }
            self.depth = self.depth.saturating_sub(1);
        }

        fn create(
            &mut self,
            _context: &mut CTX,
            _inputs: &mut CreateInputs,
        ) -> Option<CreateOutcome> {
            self.depth += 1;
            None
        }

        fn create_end(
            &mut self,
            _context: &mut CTX,
            _inputs: &CreateInputs,
            outcome: &mut CreateOutcome,
        ) {
            if outcome.result.result.is_revert() {
                let data = outcome.result.output.clone();
                self.capture("<create>", data.as_ref());
            }
            self.depth = self.depth.saturating_sub(1);
        }
    }
}

#[cfg(feature = "revert-trace")]
impl ContextInspector for revert_trace::RevertTracer {
    // The gated tracer PRINTS its frames (VARIANCE_BENCH_REVERT_FRAME=…);
    // it does not expose them through the outcome — R8: it records none for
    // the generic drain, never a fabricated placeholder.
    fn take_revert_frames(&mut self) -> Vec<RevertFrame> {
        Vec::new()
    }
}

#[cfg(feature = "revert-trace")]
impl SequenceContext<revert_trace::RevertTracer> {
    /// Build a context whose inspector names the innermost reverting frame
    /// (address + depth + pc + opcode + raw data) and prints it once.
    pub fn new_with_revert_tracer(lazy_db: LazyDb, chain_id: u64, block_number: u64) -> Self {
        Self::new_with_inspector(
            lazy_db,
            chain_id,
            block_number,
            revert_trace::RevertTracer::default(),
        )
    }
}

impl SequenceContext<RevertFrameInspector> {
    /// Build a fresh context around a freshly-constructed `LazyDb`. The
    /// `chain_id` and `block_number` define the REVM environment that
    /// every step shares — they are NEVER mutated by `call()`.
    ///
    /// REVERT-TRACE-01: this PRODUCTION context carries the
    /// `RevertFrameInspector`, so every reverted `call()`/`deploy()` names
    /// its failing frame (address + pc + opcode + structured kind) instead
    /// of returning an anonymous decoded string. One execution path — the
    /// frame is captured in production, not behind a diagnostic feature.
    pub fn new(lazy_db: LazyDb, chain_id: u64, block_number: u64) -> Self {
        Self::new_with_inspector(
            lazy_db,
            chain_id,
            block_number,
            RevertFrameInspector::default(),
        )
    }

    /// Build from a canonical header using the chain's supported fork schedule.
    pub fn new_verified(lazy_db: LazyDb) -> Result<Self, SequenceError> {
        let snapshot = lazy_db
            .snapshot()
            .ok_or_else(|| SequenceError::TransactInfra("verified_snapshot_required".into()))?;
        let (block, spec) = crate::verified_environment::environment(snapshot)
            .map_err(|e| SequenceError::TransactInfra(e.into()))?;
        let chain_id = snapshot.chain_id;
        let evm = Context::mainnet()
            .with_db(CacheDB::new(lazy_db))
            .with_block(block)
            .modify_cfg_chained(|cfg| {
                cfg.chain_id = chain_id;
                cfg.set_spec_and_mainnet_gas_params(spec);
            })
            .build_mainnet_with_inspector(RevertFrameInspector::default());
        Ok(Self {
            evm,
            reads: HashMap::new(),
            gas_used_total: 0,
            hasher: Sha256::new(),
            successful_calls: 0,
            handled_revert_frames: Vec::new(),
        })
    }
}

impl<I> SequenceContext<I>
where
    I: ContextInspector,
{
    /// Same as [`SequenceContext::new`] with an explicit inspector. With
    /// `I = NoopInspector` this is byte-for-byte the production context; with
    /// the `revert-trace` feature the caller passes a `RevertTracer`.
    pub fn new_with_inspector(
        lazy_db: LazyDb,
        chain_id: u64,
        block_number: u64,
        inspector: I,
    ) -> Self {
        // PAPER-BLOCK-CONTEXT-01 (merge note, #992 × #991): the block CONTEXT
        // must match the forked state, and it must be built HERE — inside
        // `new_with_inspector` — because this is the single constructor every
        // inspector goes through. `BlockEnv::default()` carries timestamp 0;
        // over fork state at block B that made Aave's reserve update compute
        // `uint256(block.timestamp) - uint256(lastUpdateTimestamp)` → underflow
        // → `Panic(0x11)` on EVERY route that touched a reserve, before any
        // swap. Resolving the generic-inspector refactor by rebuilding the env
        // here (instead of in `new`) is what keeps the production path and the
        // tracer on the SAME context: a tracer that measures a different block
        // env would diagnose a defect that production no longer has.
        let block = match lazy_db.pinned_header() {
            Ok(Some(header)) => block_env_from_header(&header, block_number),
            _ => BlockEnv {
                number: U256::from(block_number),
                ..BlockEnv::default()
            },
        };
        let cache_db = CacheDB::new(lazy_db);
        let mut cfg = CfgEnv::default();
        cfg.chain_id = chain_id;
        cfg.spec = SpecId::OSAKA; // SIM-11 note (2026-09-24): paper path pins OSAKA regardless of the forked block's actual hardfork; verified path derives from header;
        let evm = Context::mainnet()
            .with_db(cache_db)
            .with_block(block)
            .modify_cfg_chained(|c| {
                c.chain_id = cfg.chain_id;
                c.spec = cfg.spec;
            })
            .build_mainnet_with_inspector(inspector);
        Self {
            evm,
            reads: HashMap::new(),
            gas_used_total: 0,
            hasher: Sha256::new(),
            successful_calls: 0,
            handled_revert_frames: Vec::new(),
        }
    }

    pub fn assert_canonical(&self) -> Result<(), SequenceError> {
        self.evm
            .db()
            .db
            .assert_canonical()
            .map_err(|e| SequenceError::TransactInfra(e.to_string()))
    }

    /// Read-only calls use zero gas price without modifying the execution header
    /// or committed state. Restore validation before any real transaction.
    fn view(&mut self, mut tx: TxEnv) -> Result<ExecutionResult, SequenceError> {
        tx.nonce = self
            .evm
            .db_mut()
            .load_account(tx.caller)
            .map_err(|e| SequenceError::TransactInfra(e.to_string()))?
            .info
            .nonce;
        let previous = self.evm.ctx.cfg.disable_base_fee;
        self.evm.ctx.cfg.disable_base_fee = true;
        let result = self.evm.inspect_tx(tx);
        self.evm.ctx.cfg.disable_base_fee = previous;
        // REVERT-TRACE-01: view reverts surface as typed SequenceErrors on
        // the read helpers; drain whatever the inspector recorded so it can
        // never leak into the next committed call()'s frame attribution.
        let stray_frames = self.evm.inspector.take_revert_frames();
        if !stray_frames.is_empty() {
            debug!(
                event = "sequence_runner.view_revert_frames_discarded",
                count = stray_frames.len(),
                "read-only transact reverted; frames not attributed to any committed step"
            );
        }
        result
            .map(|r| r.result)
            .map_err(|e| SequenceError::TransactInfra(e.to_string()))
    }

    /// Apply a paper-only storage override to the CacheDB. Returns an
    /// error if the EVM database handle is unexpectedly missing or if
    /// the underlying CacheDB rejects the insertion (only happens when
    /// the DatabaseRef fetch for the existing account fails).
    pub fn apply_storage(&mut self, over: StorageOverride) -> Result<(), SequenceError> {
        let db = self.evm.db_mut();
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

    /// Set `account`'s native ETH balance to `balance_wei` in the CacheDB.
    /// Paper-only caller-funding primitive — the same class of CacheDB-direct
    /// mutation as `apply_storage`, answering the CALLER-GAS gap flagged in
    /// `sim_multistep` (the paper caller holds no forked mainnet ETH, so
    /// without this NO paper dispatch can ever pay gas). The sim-core
    /// orchestrator gates this behind `paper_mode && enable_storage_cheats`
    /// exactly like the role-grant override; the runner itself stays neutral.
    /// The seed is ABSOLUTE (sets, never adds) so repeated seeding is
    /// deterministic regardless of prior forked balance.
    pub fn seed_balance(
        &mut self,
        account: Address,
        balance_wei: U256,
        label: &'static str,
    ) -> Result<(), SequenceError> {
        let db = self.evm.db_mut();
        let mut info = db
            .load_account(account)
            .map_err(|e| SequenceError::TransactInfra(e.to_string()))?
            .info
            .clone();
        info.balance = balance_wei;
        db.insert_account_info(account, info);
        debug!(
            event = "sequence_runner.balance_seed",
            account = ?account,
            balance_wei = %balance_wei,
            label = %label,
            "account balance seeded (paper-only)"
        );
        Ok(())
    }

    /// Execute a contract deployment via `evm.transact_commit()` with
    /// `TxKind::Create`. Mirrors `call()` (nonce from CacheDB state, gas
    /// accounting, trace-hash folding, revert/halt decode) and additionally
    /// decodes the CREATED address from REVM's execution output. The deployed
    /// contract is immediately visible to subsequent `call()`/`read_balance()`
    /// steps — the CacheDB is shared.
    pub fn deploy(&mut self, deploy: SequenceDeploy) -> Result<DeployOutcome, SequenceError> {
        if deploy.init_code.is_empty() {
            return Err(SequenceError::CreateInitCodeEmpty);
        }
        let nonce = self
            .evm
            .db_mut()
            .load_account(deploy.from)
            .map_err(|e| SequenceError::TransactInfra(e.to_string()))?
            .info
            .nonce;
        let tx = TxEnv {
            nonce,
            chain_id: Some(self.evm.ctx.cfg.chain_id),
            caller: deploy.from,
            kind: TransactTo::Create,
            data: Bytes::copy_from_slice(&deploy.init_code),
            value: U256::ZERO,
            gas_price: deploy.gas_price_wei,
            gas_limit: deploy.gas_limit,
            ..TxEnv::default()
        };

        let exec = self
            .evm
            .inspect_tx_commit(tx)
            .map_err(|e| SequenceError::TransactInfra(format!("{e}")))?;

        // REVERT-TRACE-01: drain the frames the inspector recorded for THIS
        // transact before classifying the outcome — the origin (first,
        // innermost) rides the Reverted variant; handled subcall reverts are
        // retained for diagnostics.
        let frames = self.evm.inspector.take_revert_frames();

        Ok(match exec {
            ExecutionResult::Success { gas, output, .. } => {
                let gas_used = gas.tx_gas_used();
                self.gas_used_total += gas_used;
                if !frames.is_empty() {
                    debug!(
                        event = "sequence_runner.deploy_handled_subcall_reverts",
                        count = frames.len(),
                        "deploy succeeded while subcalls reverted (handled); frames retained"
                    );
                    self.handled_revert_frames.extend(frames);
                }
                // revm 42: `Output` is an enum — only the `Create` variant carries
                // the deployed address. Fail-closed on anything else.
                let address = match output {
                    revm::context_interface::result::Output::Create(_, Some(addr)) => addr,
                    revm::context_interface::result::Output::Create(_, None) => {
                        warn!(
                            event = "sequence_runner.deploy_address_missing",
                            label = %deploy.label,
                            gas_used,
                        );
                        return Ok(DeployOutcome::Halted {
                            gas_used,
                            reason: "create_address_missing".to_string(),
                        });
                    }
                    revm::context_interface::result::Output::Call(_) => {
                        warn!(
                            event = "sequence_runner.deploy_output_not_create",
                            label = %deploy.label,
                            gas_used,
                        );
                        return Ok(DeployOutcome::Halted {
                            gas_used,
                            reason: "create_output_expected".to_string(),
                        });
                    }
                };
                self.successful_calls += 1;
                self.hasher.update(&deploy.init_code);
                self.hasher.update(address.as_slice());
                debug!(
                    event = "sequence_runner.deploy_success",
                    label = %deploy.label,
                    address = ?address,
                    gas_used,
                );
                DeployOutcome::Success { address, gas_used }
            }
            ExecutionResult::Revert { gas, output, .. } => {
                let gas_used = gas.tx_gas_used();
                self.gas_used_total += gas_used;
                let reason = decode_revert_reason(&output);
                // The first recorded frame (innermost-first) is the origin
                // of the FATAL unwind — mark it fatal and carry it. Frames
                // recorded after it belong to the same unwind chain and are
                // logged, not carried (the origin names the cause).
                let mut frames = frames;
                let frame = frames.first_mut().map(|f| {
                    f.fatal = true;
                    f.clone()
                });
                if frames.len() > 1 {
                    debug!(
                        event = "sequence_runner.deploy_revert_unwind_chain",
                        deeper_frames = frames.len() - 1,
                        "additional reverting frames observed in the same unwind"
                    );
                }
                warn!(
                    event = "sequence_runner.deploy_revert",
                    label = %deploy.label,
                    gas_used,
                    reason = %reason,
                    frame = frame.as_ref().map(|f| f.annotation()).as_deref(),
                );
                DeployOutcome::Reverted {
                    gas_used,
                    reason,
                    frame,
                }
            }
            ExecutionResult::Halt { reason, gas, .. } => {
                let gas_used = gas.tx_gas_used();
                self.gas_used_total += gas_used;
                warn!(
                    event = "sequence_runner.deploy_halt",
                    label = %deploy.label,
                    gas_used,
                    reason = ?reason,
                );
                DeployOutcome::Halted {
                    gas_used,
                    reason: format!("{reason:?}"),
                }
            }
        })
    }

    /// Execute `call` via `evm.transact_commit()`. Mutates the CacheDB
    /// so subsequent steps see the post-state. Returns the outcome —
    /// the orchestrator inspects it and decides whether to continue.
    pub fn call(&mut self, call: SequenceCall) -> Result<CallOutcome, SequenceError> {
        let nonce = self
            .evm
            .db_mut()
            .load_account(call.from)
            .map_err(|e| SequenceError::TransactInfra(e.to_string()))?
            .info
            .nonce;
        let tx = TxEnv {
            nonce,
            chain_id: Some(self.evm.ctx.cfg.chain_id),
            caller: call.from,
            kind: TransactTo::Call(call.to),
            data: Bytes::copy_from_slice(&call.calldata),
            value: U256::from(call.value_wei),
            gas_price: call.gas_price_wei,
            gas_limit: call.gas_limit,
            ..TxEnv::default()
        };

        let exec = self
            .evm
            .inspect_tx_commit(tx)
            .map_err(|e| SequenceError::TransactInfra(format!("{e}")))?;

        // REVERT-TRACE-01: drain the frames the inspector recorded for THIS
        // transact before classifying the outcome — the origin (first,
        // innermost) rides the Reverted variant; handled subcall reverts are
        // retained for diagnostics.
        let frames = self.evm.inspector.take_revert_frames();

        let outcome = match exec {
            ExecutionResult::Success { gas, output, .. } => {
                let gas_used = gas.tx_gas_used();
                self.gas_used_total += gas_used;
                self.successful_calls += 1;
                if !frames.is_empty() {
                    debug!(
                        event = "sequence_runner.call_handled_subcall_reverts",
                        label = %call.label,
                        count = frames.len(),
                        "call succeeded while subcalls reverted (handled); frames retained"
                    );
                    self.handled_revert_frames.extend(frames);
                }
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
            ExecutionResult::Revert { gas, output, .. } => {
                let gas_used = gas.tx_gas_used();
                self.gas_used_total += gas_used;
                let reason = decode_revert_reason(&output);
                // The first recorded frame (innermost-first) is the origin
                // of the FATAL unwind — mark it fatal and carry it. Frames
                // recorded after it belong to the same unwind chain and are
                // logged, not carried (the origin names the cause). A revert
                // that ran no interpreter frames (e.g. validation-stage)
                // carries None — R8, never a fabricated frame.
                let mut frames = frames;
                let frame = frames.first_mut().map(|f| {
                    f.fatal = true;
                    f.clone()
                });
                if frames.len() > 1 {
                    debug!(
                        event = "sequence_runner.call_revert_unwind_chain",
                        label = %call.label,
                        deeper_frames = frames.len() - 1,
                        "additional reverting frames observed in the same unwind"
                    );
                }
                warn!(
                    event = "sequence_runner.call_revert",
                    label = %call.label,
                    gas_used,
                    reason = %reason,
                    frame = frame.as_ref().map(|f| f.annotation()).as_deref(),
                );
                CallOutcome::Reverted {
                    gas_used,
                    reason,
                    frame,
                }
            }
            ExecutionResult::Halt { reason, gas, .. } => {
                let gas_used = gas.tx_gas_used();
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
        let tx = TxEnv {
            caller: Address::ZERO,
            kind: TransactTo::Call(token),
            data: Bytes::copy_from_slice(&calldata),
            value: U256::ZERO,
            gas_price: 0,
            gas_limit: 5_000_000,
            ..TxEnv::default()
        };

        let result = self.view(tx)?;

        let amount = match result {
            ExecutionResult::Success { output, .. } => {
                let bytes = output.into_data();
                if bytes.len() != 32 {
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

        // Caller must be code-less: revm has EIP-3607 hard-enabled (no
        // `optional_eip3607` feature), so a caller with on-chain code is
        // rejected at validation (`RejectCallerWithCode` → TransactInfra).
        // On a real fork the router HAS code — unit-test mocks (code-less
        // router accounts) hide this. `getAmountsOut` has no msg.sender
        // dependency, so the canonical eth_call sender is safe here.
        let tx = TxEnv {
            caller: Address::ZERO,
            kind: TransactTo::Call(router),
            data: Bytes::copy_from_slice(&calldata),
            value: U256::ZERO,
            gas_price: 0,
            gas_limit: 5_000_000,
            ..TxEnv::default()
        };

        let result = self.view(tx)?;

        let amount = match result {
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

    /// Generic read-only call against `to` with `calldata` — the
    /// `eth_call`-equivalent of [`read_balance`] / [`read_amounts_out`].
    ///
    /// MIRRORS both: a non-committing `transact()` — it does NOT bump
    /// `gas_used_total`, does NOT increment `successful_calls`, does NOT touch
    /// the trace hash, and does NOT mutate the CacheDB (no nonce bump on any
    /// real account: the canonical `eth_call` sender `Address::ZERO` is used).
    /// It can therefore be issued mid-sequence without perturbing later
    /// committed steps, and returns the RAW return data (the caller decodes).
    ///
    /// Returns `Ok(None)` when the call SUCCEEDS with EMPTY return data — the
    /// canonical "no contract at `to`" signal. Callers fail closed on `None`;
    /// a revert or halt is a typed error instead.
    pub fn read_call(
        &mut self,
        to: Address,
        calldata: Vec<u8>,
        label: &str,
    ) -> Result<Option<Vec<u8>>, SequenceError> {
        // Code-less view caller: revm has EIP-3607 hard-enabled, so a caller
        // with on-chain code is rejected at validation (RejectCallerWithCode);
        // `Address::ZERO` is the canonical eth_call sender.
        let tx = TxEnv {
            caller: Address::ZERO,
            kind: TransactTo::Call(to),
            data: Bytes::copy_from_slice(&calldata),
            value: U256::ZERO,
            gas_price: 0,
            gas_limit: 5_000_000,
            ..TxEnv::default()
        };

        let result = self.view(tx)?;

        let output = match result {
            ExecutionResult::Success { output, .. } => output.into_data().to_vec(),
            ExecutionResult::Revert { output, .. } => {
                return Err(SequenceError::CallReadReverted(decode_revert_reason(
                    &output,
                )));
            }
            ExecutionResult::Halt { reason, .. } => {
                return Err(SequenceError::CallReadHalted(format!("{reason:?}")));
            }
        };

        debug!(
            event = "sequence_runner.read_call",
            label = %label,
            to = ?to,
            output_len = output.len(),
        );

        if output.is_empty() {
            Ok(None)
        } else {
            Ok(Some(output))
        }
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

    /// REVERT-TRACE-01: reverting frames whose reverts were HANDLED by a
    /// parent frame — the transact still succeeded, so they never ride an
    /// outcome and are never fatal. Measured diagnostics (a reverted
    /// subcall inside a successful swap is real signal, not noise); R8:
    /// empty = none observed, never fabricated.
    pub fn handled_revert_frames(&self) -> &[RevertFrame] {
        &self.handled_revert_frames
    }

    /// REVERT-TRACE-01: drain the handled-revert ledger (after inspecting
    /// it with [`Self::handled_revert_frames`]).
    pub fn take_handled_revert_frames(&mut self) -> Vec<RevertFrame> {
        std::mem::take(&mut self.handled_revert_frames)
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Attempt to decode a standard Solidity `Error(string)` revert payload;
/// fall back to a hex-encoded leading-slice. Matches the
/// behaviour of `revm_runner::decode_revert_reason` for consistency.
///
/// PANIC-CODE-01 (2026-10-19): the fallback used to cap at 32 bytes, which is
/// ONE BYTE SHORT of a `Panic(uint256)` payload (4-byte selector + one 32-byte
/// ABI word = 36). Measured on the G-SIM-1 variance harness: 15 of 21 routes
/// carried `multistep_call_revert:0x4e487b71` followed by 28 zero bytes, so the
/// tag said "Solidity panic" and lost the code (0x01 assert, 0x11 overflow,
/// 0x32 array bound). The cap is now 36 so the reason survives intact; longer
/// payloads still fall back to their leading 36 bytes.
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
    let end = output.len().min(36);
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
    use revm::bytecode::Bytecode;
    use revm::primitives::KECCAK_EMPTY;
    use revm::state::AccountInfo;

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

    #[test]
    fn execution_uses_and_increments_real_account_nonce() {
        let lazy = lazy_offline();
        let caller = Address::from([0xa1; 20]);
        let target = Address::from([0xb1; 20]);
        for address in [caller, target, Address::ZERO] {
            seed_account(
                &lazy,
                address,
                AccountInfo {
                    balance: U256::from(1_000_000u64),
                    nonce: if address == caller { 9 } else { 0 },
                    code_hash: KECCAK_EMPTY,
                    code: Some(Bytecode::new()),
                    ..Default::default()
                },
            );
        }
        let mut ctx = SequenceContext::new(lazy, 1, 100);
        for _ in 0..2 {
            let result = ctx
                .call(SequenceCall {
                    from: caller,
                    to: target,
                    calldata: vec![],
                    value_wei: 0,
                    gas_price_wei: 1,
                    gas_limit: 50000,
                    label: "nonce_regression",
                })
                .unwrap();
            assert!(result.is_success());
        }
        assert_eq!(
            ctx.evm.db_mut().load_account(caller).unwrap().info.nonce,
            11
        );
        assert_eq!(ctx.finalize().successful_calls, 2);
    }

    #[test]
    fn contract_balance_view_does_not_relax_execution_validation() {
        let lazy = lazy_offline();
        let token = Address::from([0xb2; 20]);
        let holder = Address::from([0xc2; 20]);
        for address in [Address::ZERO, holder] {
            seed_account(
                &lazy,
                address,
                AccountInfo {
                    code_hash: KECCAK_EMPTY,
                    code: Some(Bytecode::new()),
                    ..Default::default()
                },
            );
        }
        let code = Bytecode::new_raw(Bytes::from(vec![
            0x60, 42, 0x60, 0, 0x52, 0x60, 32, 0x60, 0, 0xf3,
        ]));
        seed_account(
            &lazy,
            token,
            AccountInfo {
                code_hash: code.hash_slow(),
                code: Some(code),
                ..Default::default()
            },
        );
        let mut ctx = SequenceContext::new(lazy, 1, 100);
        ctx.evm.ctx.block.basefee = 10;
        assert_eq!(
            ctx.read_balance(token, holder, "contract_balance").unwrap(),
            U256::from(42)
        );
        assert!(!ctx.evm.ctx.cfg.disable_base_fee);
        assert_eq!(ctx.finalize().successful_calls, 0);
    }

    /// Pool-dynamics read primitive: `read_call` returns the RAW return data of
    /// a read-only call, commits NOTHING (no gas, no `successful_calls`, no
    /// nonce bump on the sender), reports `None` for a code-less target (the
    /// "no contract" signal) and surfaces a revert as a typed error.
    #[test]
    fn read_call_returns_raw_output_and_commits_nothing() {
        let lazy = lazy_offline();
        let returns_word = Address::from([0xb3; 20]);
        let code_less = Address::from([0xb4; 20]);
        let reverts = Address::from([0xb5; 20]);
        for address in [Address::ZERO, code_less] {
            seed_account(
                &lazy,
                address,
                AccountInfo {
                    code_hash: KECCAK_EMPTY,
                    code: Some(Bytecode::new()),
                    ..Default::default()
                },
            );
        }
        // Returns the 32-byte word 42.
        let word_code = Bytecode::new_raw(Bytes::from(vec![
            0x60, 42, 0x60, 0, 0x52, 0x60, 32, 0x60, 0, 0xf3,
        ]));
        seed_account(
            &lazy,
            returns_word,
            AccountInfo {
                code_hash: word_code.hash_slow(),
                code: Some(word_code),
                ..Default::default()
            },
        );
        // Reverts unconditionally.
        let revert_code = Bytecode::new_raw(Bytes::from(vec![0x60, 0, 0x60, 0, 0xfd]));
        seed_account(
            &lazy,
            reverts,
            AccountInfo {
                code_hash: revert_code.hash_slow(),
                code: Some(revert_code),
                ..Default::default()
            },
        );

        let mut ctx = SequenceContext::new(lazy, 1, 100);
        ctx.evm.ctx.block.basefee = 10;

        let out = ctx
            .read_call(returns_word, vec![0xde, 0xad, 0xbe, 0xef], "get_pool")
            .unwrap()
            .expect("contract returns data");
        assert_eq!(out.len(), 32);
        assert_eq!(out[31], 42);

        // Code-less target → None (canonical eth_call empty-return signal).
        assert_eq!(
            ctx.read_call(code_less, vec![0x02, 0x6b, 0x1d, 0x5f], "no_code")
                .unwrap(),
            None
        );

        // Revert → typed error, never a silent None.
        assert!(matches!(
            ctx.read_call(reverts, vec![], "revert").unwrap_err(),
            SequenceError::CallReadReverted(_)
        ));

        // No commit side effects and the view restored validation.
        assert!(!ctx.evm.ctx.cfg.disable_base_fee);
        assert_eq!(
            ctx.evm
                .db_mut()
                .load_account(Address::ZERO)
                .unwrap()
                .info
                .nonce,
            0
        );
        let summary = ctx.finalize();
        assert_eq!(summary.successful_calls, 0);
        assert_eq!(summary.gas_used_total, 0);
        assert_eq!(summary.trace_hash, [0u8; 32]);
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
                ..Default::default()
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

    /// PANIC-CODE-01: a Solidity `Panic(uint256)` payload is 36 bytes
    /// (selector + one ABI word). The fallback must carry the WHOLE payload so
    /// the code (0x01 assert / 0x11 overflow / 0x32 array bound) is readable —
    /// the old 32-byte cap cut exactly that byte off and turned every panic
    /// into an indistinguishable `0x4e487b71…`.
    #[test]
    fn decode_revert_reason_keeps_the_panic_code() {
        let mut payload = vec![0x4e, 0x48, 0x7b, 0x71];
        let mut code = [0u8; 32];
        code[31] = 0x11; // Panic(0x11) = arithmetic overflow
        payload.extend_from_slice(&code);
        let reason = decode_revert_reason(&Bytes::from(payload));
        assert_eq!(reason.len(), 2 + 72, "selector + one ABI word, in hex");
        assert!(
            reason.ends_with("11"),
            "the panic code must survive the fallback, got {reason}"
        );
    }

    // -----------------------------------------------------------------------
    // REVERT-TRACE-01 — the production frame (address + pc + opcode)
    // -----------------------------------------------------------------------

    /// Seeds a code-less account with balance + nonce (the canonical caller
    /// for committed paper calls).
    fn seed_eoa(lazy: &LazyDb, address: Address, balance: u64) {
        seed_account(
            lazy,
            address,
            AccountInfo {
                balance: U256::from(balance),
                code_hash: KECCAK_EMPTY,
                code: Some(Bytecode::new()),
                ..Default::default()
            },
        );
    }

    /// Seeds a contract account with raw bytecode.
    fn seed_contract(lazy: &LazyDb, address: Address, code_bytes: Vec<u8>) {
        let code = Bytecode::new_raw(Bytes::from(code_bytes));
        seed_account(
            lazy,
            address,
            AccountInfo {
                code_hash: code.hash_slow(),
                code: Some(code),
                ..Default::default()
            },
        );
    }

    /// `PUSH1 0 PUSH1 0 REVERT` — reverts with NO data at pc=4, opcode 0xfd.
    const BARE_REVERT_CODE: [u8; 5] = [0x60, 0x00, 0x60, 0x00, 0xfd];

    /// ACCEPTANCE (REVERT-TRACE-01): a known revert produces a trace that
    /// names the contract ADDRESS, the PC of the REVERT (4 — two PUSH1 pairs
    /// precede it) and the OPCODE (0xfd, the Solidity revert instruction).
    /// The revert stops being anonymous: `Reverted` carries `Some(frame)`
    /// marked `fatal` (it IS the top-level outcome).
    #[test]
    fn revert_frame_names_address_pc_opcode() {
        let lazy = lazy_offline();
        let caller = Address::from([0xa7; 20]);
        let reverts = Address::from([0xb7; 20]);
        seed_eoa(&lazy, caller, 1_000_000);
        seed_eoa(&lazy, Address::ZERO, 0);
        seed_contract(&lazy, reverts, BARE_REVERT_CODE.to_vec());

        let mut ctx = SequenceContext::new(lazy, 1, 100);
        let outcome = ctx
            .call(SequenceCall {
                from: caller,
                to: reverts,
                calldata: vec![],
                value_wei: 0,
                gas_price_wei: 1,
                gas_limit: 100_000,
                label: "revert_frame_probe",
            })
            .unwrap();

        let CallOutcome::Reverted {
            frame: Some(frame), ..
        } = outcome
        else {
            panic!("expected Reverted with a frame, got {outcome:?}");
        };
        assert_eq!(frame.address, format!("{reverts:?}"));
        assert_eq!(frame.pc, 4, "REVERT sits at pc=4 after two PUSH1 pairs");
        assert_eq!(frame.opcode, 0xfd, "REVERT opcode is 0xfd (Solidity docs)");
        assert_eq!(frame.kind, RevertKind::Empty, "REVERT(0,0) returns no data");
        assert_eq!(frame.data_hex(), "0x");
        assert!(
            frame.fatal,
            "the top-level outcome reverted: origin is fatal"
        );
        assert_eq!(
            frame.depth, 0,
            "top-level frame is depth 0 (call hook depth minus one)"
        );
        // No handled-revert leakage into the ledger from a fatal revert.
        assert!(ctx.handled_revert_frames().is_empty());
    }

    /// A REAL EVM-executed `Panic(uint256)` payload: hand-assembled bytecode
    /// stores the 36-byte `Panic(0x11)` payload in memory and reverts with
    /// it. The frame must classify `Panic { code: 0x11, named }` (vector T2)
    /// with the REVERT at pc=76, and the annotation must carry the named
    /// classification.
    #[test]
    fn revert_frame_classifies_real_panic_payload_from_evm() {
        // word1 = 4e487b71 + 28 zero bytes; word2's FIRST four bytes are the
        // tail of the 36-byte payload — REVERT(0, 36) returns memory[0..36],
        // so the panic code byte must land at payload offset 35 (index 3 of
        // word2), NOT at word2's own last byte.
        let mut word1 = [0u8; 32];
        word1[0..4].copy_from_slice(&[0x4e, 0x48, 0x7b, 0x71]);
        let mut word2 = [0u8; 32];
        word2[3] = 0x11;
        let mut code: Vec<u8> = Vec::with_capacity(77);
        code.push(0x7f); // PUSH32 word1        (pc 0)
        code.extend_from_slice(&word1);
        code.extend_from_slice(&[0x60, 0x00]); // PUSH1 0     (pc 33)
        code.push(0x52); // MSTORE              (pc 35)
        code.push(0x7f); // PUSH32 word2        (pc 36)
        code.extend_from_slice(&word2);
        code.extend_from_slice(&[0x60, 0x20]); // PUSH1 32    (pc 69)
        code.push(0x52); // MSTORE              (pc 71)
        code.extend_from_slice(&[0x60, 0x24]); // PUSH1 36    (pc 72)
        code.extend_from_slice(&[0x60, 0x00]); // PUSH1 0     (pc 74)
        code.push(0xfd); // REVERT(0, 36)       (pc 76)

        let lazy = lazy_offline();
        let caller = Address::from([0xa8; 20]);
        let panics = Address::from([0xb8; 20]);
        seed_eoa(&lazy, caller, 1_000_000);
        seed_eoa(&lazy, Address::ZERO, 0);
        seed_contract(&lazy, panics, code);

        let mut ctx = SequenceContext::new(lazy, 1, 100);
        let outcome = ctx
            .call(SequenceCall {
                from: caller,
                to: panics,
                calldata: vec![],
                value_wei: 0,
                gas_price_wei: 1,
                gas_limit: 100_000,
                label: "panic_payload_probe",
            })
            .unwrap();

        let CallOutcome::Reverted {
            frame: Some(frame), ..
        } = outcome
        else {
            panic!("expected Reverted with a frame, got {outcome:?}");
        };
        assert_eq!(frame.address, format!("{panics:?}"));
        assert_eq!(
            frame.pc, 76,
            "REVERT(0,36) at pc=76 in the hand-assembled code"
        );
        assert_eq!(frame.opcode, 0xfd);
        assert_eq!(frame.data.len(), 36, "selector + one ABI word");
        assert_eq!(
            frame.kind,
            RevertKind::Panic {
                code: 0x11,
                named: Some("arithmetic underflow/overflow"),
            },
            "vector T2: Panic(0x11) with its canonical name"
        );
        let annotation = frame.annotation();
        assert!(
            annotation.contains("Panic(0x11): arithmetic underflow/overflow"),
            "annotation names the panic: {annotation}"
        );
        assert!(
            annotation.contains(&format!("@{panics:?}")),
            "annotation names the address: {annotation}"
        );
        assert!(
            annotation.contains("pc=0x4c") && annotation.contains("op=0xfd"),
            "annotation names pc (0x4c=76) and opcode: {annotation}"
        );
    }

    /// Handled vs fatal (ficha §3 rule 2, checklist P5): a subcall that
    /// reverts while the PARENT handles it (CALL pushes 0, parent POPs and
    /// STOPs) must NOT fail the sequence — the frame is recorded with
    /// `fatal=false` in the handled ledger, never carried by an outcome.
    #[test]
    fn handled_subcall_revert_is_not_fatal() {
        let lazy = lazy_offline();
        let caller = Address::from([0xa9; 20]);
        let child = Address::from([0xb9; 20]);
        let parent = Address::from([0xc9; 20]);
        seed_eoa(&lazy, caller, 1_000_000);
        seed_eoa(&lazy, Address::ZERO, 0);
        seed_contract(&lazy, child, BARE_REVERT_CODE.to_vec());

        // Parent: CALL(child) with zero value/args/ret, POP the status word,
        // STOP — the child's revert is HANDLED, the tx SUCCEEDS.
        let mut parent_code: Vec<u8> = Vec::new();
        parent_code.extend_from_slice(&[0x60, 0x00]); // PUSH1 0   retSize     (pc 0)
        parent_code.extend_from_slice(&[0x60, 0x00]); // PUSH1 0   retOffset   (pc 2)
        parent_code.extend_from_slice(&[0x60, 0x00]); // PUSH1 0   argsSize    (pc 4)
        parent_code.extend_from_slice(&[0x60, 0x00]); // PUSH1 0   argsOffset  (pc 6)
        parent_code.extend_from_slice(&[0x60, 0x00]); // PUSH1 0   value       (pc 8)
        parent_code.push(0x73); // PUSH20 child                                  (pc 10)
        parent_code.extend_from_slice(child.as_slice());
        parent_code.extend_from_slice(&[0x61, 0x75, 0x30]); // PUSH2 30000 gas (pc 31)
        parent_code.push(0xf1); // CALL                                          (pc 34)
        parent_code.push(0x50); // POP                                           (pc 35)
        parent_code.push(0x00); // STOP                                          (pc 36)
        seed_contract(&lazy, parent, parent_code);

        let mut ctx = SequenceContext::new(lazy, 1, 100);
        let outcome = ctx
            .call(SequenceCall {
                from: caller,
                to: parent,
                calldata: vec![],
                value_wei: 0,
                gas_price_wei: 1,
                gas_limit: 200_000,
                label: "handled_subcall_probe",
            })
            .unwrap();
        assert!(
            outcome.is_success(),
            "parent handled the child revert: the tx must succeed, got {outcome:?}"
        );

        let handled = ctx.handled_revert_frames();
        assert_eq!(
            handled.len(),
            1,
            "the child's revert is recorded, not silent"
        );
        let frame = &handled[0];
        assert!(!frame.fatal, "a handled subcall revert is never fatal");
        assert_eq!(
            frame.address,
            format!("{child:?}"),
            "attributed to the child"
        );
        assert_eq!(frame.pc, 4);
        assert_eq!(frame.opcode, 0xfd);
        assert_eq!(frame.kind, RevertKind::Empty);
        assert_eq!(frame.depth, 1, "one frame below the top level");

        // Same context, DIRECT call to the child: now the revert IS fatal
        // and rides the outcome — and the handled ledger did not leak.
        ctx.take_handled_revert_frames();
        let outcome = ctx
            .call(SequenceCall {
                from: caller,
                to: child,
                calldata: vec![],
                value_wei: 0,
                gas_price_wei: 1,
                gas_limit: 100_000,
                label: "fatal_revert_probe",
            })
            .unwrap();
        let CallOutcome::Reverted {
            frame: Some(frame), ..
        } = outcome
        else {
            panic!("expected Reverted with a frame, got {outcome:?}");
        };
        assert!(frame.fatal);
        assert_eq!(frame.address, format!("{child:?}"));
        assert!(ctx.handled_revert_frames().is_empty());
    }

    /// A reverting `deploy()` (init code that reverts) carries the create
    /// frame: `"<create>"` address, the REVERT pc/opcode, `Empty` kind.
    /// The deterministic CREATE address is seeded too: revm's EIP-684
    /// collision check loads the target account, which on an offline
    /// `LazyDb` would otherwise hit the never-RPC and fail the transact
    /// with `TransactInfra` instead of reaching the reverting init code.
    #[test]
    fn deploy_revert_carries_create_frame() {
        let lazy = lazy_offline();
        let caller = Address::from([0xaa; 20]);
        seed_eoa(&lazy, caller, 1_000_000);
        seed_eoa(&lazy, Address::ZERO, 0);
        let created_ethers = ethers::utils::get_contract_address(
            ethers::types::H160::from_slice(caller.as_slice()),
            0u64,
        );
        seed_eoa(&lazy, Address::from_slice(created_ethers.as_bytes()), 0);

        let mut ctx = SequenceContext::new(lazy, 1, 100);
        let outcome = ctx
            .deploy(SequenceDeploy {
                from: caller,
                init_code: BARE_REVERT_CODE.to_vec(),
                gas_price_wei: 1,
                gas_limit: 100_000,
                label: "reverting_deploy",
            })
            .unwrap();
        let DeployOutcome::Reverted {
            frame: Some(frame), ..
        } = outcome
        else {
            panic!("expected Reverted deploy with a frame, got {outcome:?}");
        };
        assert_eq!(frame.address, "<create>");
        assert_eq!(frame.pc, 4);
        assert_eq!(frame.opcode, 0xfd);
        assert_eq!(frame.kind, RevertKind::Empty);
        assert!(frame.fatal);
    }

    /// Derived REA-trace vectors T1–T5c, byte-literal (the dispatch of ficha
    /// §2; model: docs/rea/verifica_vectores_rea_trace.py, 22/22 OK). The
    /// DISPLAY contract of `decode_revert_reason` is UNCHANGED (T5b asserts
    /// both sides agree on `0xdeadbeef`).
    #[test]
    fn revert_kind_dispatch_vectors_t1_t5() {
        // T1 — Error(string) "insufficient_balance" (same payload as the
        // baseline test `decode_revert_reason_parses_error_string`).
        let msg = b"insufficient_balance";
        let mut t1 = Vec::new();
        t1.extend_from_slice(&[0x08, 0xc3, 0x79, 0xa0]);
        let mut offset = [0u8; 32];
        offset[31] = 32;
        t1.extend_from_slice(&offset);
        let mut len = [0u8; 32];
        len[28..32].copy_from_slice(&(msg.len() as u32).to_be_bytes());
        t1.extend_from_slice(&len);
        t1.extend_from_slice(msg);
        assert_eq!(
            RevertKind::classify(&t1),
            RevertKind::ErrorString("insufficient_balance".to_string())
        );
        assert_eq!(
            RevertKind::classify(&t1).display(&t1),
            "Error(string): insufficient_balance"
        );

        // T2 — Panic(0x11), 36 exact bytes.
        let mut t2 = vec![0x4e, 0x48, 0x7b, 0x71];
        let mut code_word = [0u8; 32];
        code_word[31] = 0x11;
        t2.extend_from_slice(&code_word);
        assert_eq!(
            RevertKind::classify(&t2),
            RevertKind::Panic {
                code: 0x11,
                named: Some("arithmetic underflow/overflow"),
            }
        );
        assert_eq!(
            RevertKind::classify(&t2).display(&t2),
            "Panic(0x11): arithmetic underflow/overflow"
        );

        // T3 — all ten canonical codes name; 0x99 stays unknown.
        for code in [
            0x00u64, 0x01, 0x11, 0x12, 0x21, 0x22, 0x31, 0x32, 0x41, 0x51,
        ] {
            let mut payload = vec![0x4e, 0x48, 0x7b, 0x71];
            let mut word = [0u8; 32];
            word[31] = code as u8;
            payload.extend_from_slice(&word);
            let kind = RevertKind::classify(&payload);
            let display = kind.display(&payload);
            assert!(
                display.starts_with(&format!("Panic(0x{code:02x}): ")),
                "canonical code 0x{code:02x} must be named: {display}"
            );
            assert_eq!(
                kind,
                RevertKind::Panic {
                    code,
                    named: RevertKind::panic_code_name(code),
                }
            );
        }
        let mut unknown = vec![0x4e, 0x48, 0x7b, 0x71];
        let mut word = [0u8; 32];
        word[31] = 0x99;
        unknown.extend_from_slice(&word);
        assert_eq!(
            RevertKind::classify(&unknown).display(&unknown),
            "Panic(0x99): unknown code"
        );

        // T4 — custom selector without ABI: kind Selector, display hex.
        let t4 = vec![0x2a, 0xc1, 0x16, 0x21];
        assert_eq!(
            RevertKind::classify(&t4),
            RevertKind::Selector([0x2a, 0xc1, 0x16, 0x21])
        );
        assert_eq!(RevertKind::classify(&t4).display(&t4), "0x2ac11621");

        // T5a — empty: Empty kind, "empty" display.
        assert_eq!(RevertKind::classify(&[]), RevertKind::Empty);
        assert_eq!(RevertKind::classify(&[]).display(&[]), "empty");

        // T5b — `deadbeef`: the destination's own compatibility case. The
        // kind is Selector; BOTH displays stay the invariant hex fallback.
        let t5b = vec![0xde, 0xad, 0xbe, 0xef];
        assert_eq!(
            RevertKind::classify(&t5b),
            RevertKind::Selector([0xde, 0xad, 0xbe, 0xef])
        );
        assert_eq!(RevertKind::classify(&t5b).display(&t5b), "0xdeadbeef");
        assert_eq!(
            decode_revert_reason(&Bytes::from(t5b.clone())),
            "0xdeadbeef"
        );

        // T5c — shorter than a selector: Raw.
        let t5c = vec![0xde, 0xad, 0xbe];
        assert_eq!(RevertKind::classify(&t5c), RevertKind::Raw);
        assert_eq!(RevertKind::classify(&t5c).display(&t5c), "0xdeadbe");

        // T6 — the opcode fact: REVERT is 0xfd (Solidity docs). Asserted
        // byte-literal in the EVM-execution tests above via frame.opcode.
        assert_eq!(0xfd, 253u8);
    }

    /// The annotation is the exact string that reaches the persisted
    /// `fail_reason` — pin its format once, byte-exact.
    #[test]
    fn revert_frame_annotation_format_is_pinned() {
        let mut data = vec![0x4e, 0x48, 0x7b, 0x71];
        let mut word = [0u8; 32];
        word[31] = 0x11;
        data.extend_from_slice(&word);
        let frame = RevertFrame {
            address: "0x00000000000000000000000000000000000000b7".to_string(),
            depth: 0,
            pc: 76,
            opcode: 0xfd,
            kind: RevertKind::classify(&data),
            data: data.clone(),
            fatal: true,
        };
        assert_eq!(
            frame.annotation(),
            "@0x00000000000000000000000000000000000000b7 pc=0x4c op=0xfd \
             Panic(0x11): arithmetic underflow/overflow"
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
        assert_eq!(
            SequenceError::CallReadReverted("x".into()).reason_tag(),
            "call_read_reverted"
        );
        assert_eq!(
            SequenceError::CallReadHalted("x".into()).reason_tag(),
            "call_read_halted"
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
            frame: None,
        };
        assert!(!r.is_success());
        let h = CallOutcome::Halted {
            gas_used: 0,
            reason: "x".into(),
        };
        assert!(!h.is_success());
    }

    /// The paper `BlockEnv` must carry the pinned header's timestamp/context,
    /// not the default 0 — a zero timestamp makes Aave's `updateState`
    /// underflow (`Panic(0x11)`) on every reserve-touching route.
    #[test]
    fn block_env_from_header_is_not_a_genesis_context() {
        let header = ethers::types::Block::<ethers::types::TxHash> {
            timestamp: ethers::types::U256::from(1_792_410_893u64),
            gas_limit: ethers::types::U256::from(30_000_000u64),
            author: Some(ethers::types::H160::repeat_byte(0x11)),
            mix_hash: Some(ethers::types::H256::repeat_byte(0x22)),
            ..Default::default()
        };
        let env = block_env_from_header(&header, 26_226_569);

        assert_eq!(env.number, U256::from(26_226_569u64));
        assert_eq!(env.timestamp, U256::from(1_792_410_893u64));
        assert_ne!(
            env.timestamp,
            U256::ZERO,
            "timestamp 0 is the Aave `updateState` underflow, not a valid block"
        );
        assert_eq!(env.gas_limit, 30_000_000);
        assert_eq!(env.beneficiary, Address::repeat_byte(0x11));
        assert_eq!(
            env.prevrandao,
            Some(revm::primitives::B256::repeat_byte(0x22))
        );
    }
}
