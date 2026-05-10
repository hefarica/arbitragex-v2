//! Unit tests for `revm_runner::run()`.
//!
//! These tests use a `MapDb` — a simple in-memory `revm::Database` backed by
//! a `HashMap` — to exercise the runner without any RPC calls.
//!
//! The test contract bytecode is hand-crafted EVM bytecode:
//!
//! - `NOOP_BYTECODE`: A minimal contract that returns immediately (STOP opcode).
//!   Used to simulate a successful, no-side-effect call.
//!
//! - `REVERT_BYTECODE`: A contract whose fallback function always executes
//!   `REVERT(0, 0)` — returns zero bytes and reverts.
//!
//! - `BALANCE_RETURN_BYTECODE`: A contract that transfers 1 wei to the caller
//!   via selfdestruct (simplest way to move ETH in raw EVM bytecode without
//!   ABI encoding).  Actually we can't easily test ETH transfer in test EVM
//!   without a full hardfork config.  Instead, we verify that gas_used > 0
//!   for a successful call, and that the trace_hash is deterministic.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::HashMap;

use revm::primitives::{AccountInfo, Address, Bytecode, Bytes as RevmBytes, B256, KECCAK_EMPTY, U256};
use revm::Database;
use simulator_v2::revm_runner::run;
use simulator_v2::{CandidateInput, SimError};

// ---------------------------------------------------------------------------
// MapDb — minimal in-memory revm::Database for tests
// ---------------------------------------------------------------------------

/// A test-only `revm::Database` backed by `HashMap`s.
///
/// All accounts and slots that are not pre-loaded return empty defaults (zero
/// balance, zero nonce, empty code).  This mirrors what a real node returns
/// for non-existent accounts.
struct MapDb {
    accounts: HashMap<Address, AccountInfo>,
    storage: HashMap<(Address, U256), U256>,
}

impl MapDb {
    fn new() -> Self {
        Self {
            accounts: HashMap::new(),
            storage: HashMap::new(),
        }
    }

    fn insert_account(&mut self, addr: Address, info: AccountInfo) {
        self.accounts.insert(addr, info);
    }
}

impl Database for MapDb {
    type Error = String;

    fn basic(&mut self, address: Address) -> Result<Option<AccountInfo>, Self::Error> {
        Ok(Some(self.accounts.get(&address).cloned().unwrap_or_default()))
    }

    fn code_by_hash(&mut self, _code_hash: B256) -> Result<Bytecode, Self::Error> {
        Ok(Bytecode::new())
    }

    fn storage(&mut self, address: Address, index: U256) -> Result<U256, Self::Error> {
        Ok(*self.storage.get(&(address, index)).unwrap_or(&U256::ZERO))
    }

    fn block_hash(&mut self, _number: U256) -> Result<B256, Self::Error> {
        Ok(B256::ZERO)
    }
}

// ---------------------------------------------------------------------------
// Bytecode helpers
// ---------------------------------------------------------------------------

/// Return bytecode for a contract whose fallback just executes `STOP` (0x00).
/// When called, the EVM halts successfully with zero return data.
///
/// EVM bytecode: STOP (0x00)
///
/// A bare 0x00 byte is the STOP opcode — execution terminates successfully.
fn noop_deployed_bytecode() -> Bytecode {
    // STOP opcode.
    Bytecode::new_raw(RevmBytes::from_static(&[0x00]))
}

/// Return bytecode for a contract whose fallback always reverts with no data.
///
/// EVM bytecode: PUSH1 0x00, PUSH1 0x00, REVERT
///   60 00  (PUSH1 0)   ; size = 0
///   60 00  (PUSH1 0)   ; offset = 0
///   FD     (REVERT)
fn revert_deployed_bytecode() -> Bytecode {
    Bytecode::new_raw(RevmBytes::from_static(&[0x60, 0x00, 0x60, 0x00, 0xFD]))
}

// ---------------------------------------------------------------------------
// Address helpers
// ---------------------------------------------------------------------------

/// Create an address from a byte value placed in the HIGH byte (position 0)
/// to avoid the Ethereum precompile range (0x0000...0001 to 0x0000...0009).
/// E.g., make_addr(0x10) produces 0x1000000000000000000000000000000000000000.
fn make_addr(byte: u8) -> Address {
    let mut b = [0u8; 20];
    b[0] = byte;
    Address::from(b)
}

fn make_addr_arr(byte: u8) -> [u8; 20] {
    let mut b = [0u8; 20];
    b[0] = byte;
    b
}

// ---------------------------------------------------------------------------
// Test 1: valid call returns Ok(SimResult) with non-zero gas_used.
// ---------------------------------------------------------------------------
#[test]
fn valid_call_returns_ok_sim_result_with_nonzero_gas() {
    let caller = make_addr(0x01);
    let contract = make_addr(0x02);

    let noop_code = noop_deployed_bytecode();
    let code_hash = noop_code.hash_slow();

    let mut db = MapDb::new();
    // Give the caller a balance so the EVM doesn't reject the transaction.
    db.insert_account(
        caller,
        AccountInfo {
            balance: U256::from(10_u64.pow(18)), // 1 ETH
            nonce: 0,
            code_hash: KECCAK_EMPTY,
            code: Some(Bytecode::new()),
        },
    );
    // Deploy a NOOP contract at `contract`.
    db.insert_account(
        contract,
        AccountInfo {
            balance: U256::ZERO,
            nonce: 1,
            code_hash,
            code: Some(noop_code),
        },
    );

    let candidate = CandidateInput {
        chain_id: 1,
        block_number: 21_000_000,
        from: make_addr_arr(0x01),
        to: make_addr_arr(0x02),
        calldata: vec![],
        value_wei: 0,
    };

    let result = run(&candidate, db).expect("valid call to NOOP contract should succeed");

    assert!(
        result.gas_used > 0,
        "gas_used must be non-zero for a successful call (got {})",
        result.gas_used
    );
    // trace_hash must be a non-zero 32-byte array (SHA-256 of empty calldata
    // || STOP return data is never all-zeros).
    assert_ne!(
        result.trace_hash,
        [0u8; 32],
        "trace_hash must not be all zeros"
    );
}

// ---------------------------------------------------------------------------
// Test 2: reverted call returns Err(SimError::Reverted(_)).
// ---------------------------------------------------------------------------
#[test]
fn reverted_call_returns_sim_error_reverted() {
    let caller = make_addr(0x03);
    let contract = make_addr(0x04);

    let revert_code = revert_deployed_bytecode();
    let code_hash = revert_code.hash_slow();

    let mut db = MapDb::new();
    db.insert_account(
        caller,
        AccountInfo {
            balance: U256::from(10_u64.pow(18)),
            nonce: 0,
            code_hash: KECCAK_EMPTY,
            code: Some(Bytecode::new()),
        },
    );
    db.insert_account(
        contract,
        AccountInfo {
            balance: U256::ZERO,
            nonce: 1,
            code_hash,
            code: Some(revert_code),
        },
    );

    let candidate = CandidateInput {
        chain_id: 1,
        block_number: 21_000_000,
        from: make_addr_arr(0x03),
        to: make_addr_arr(0x04),
        calldata: vec![],
        value_wei: 0,
    };

    let result = run(&candidate, db);

    match result {
        Err(SimError::Reverted(_)) => { /* expected */ }
        Err(e) => panic!("expected Reverted error, got: {e:?}"),
        Ok(r) => panic!("expected Reverted error, got Ok with gas_used={}", r.gas_used),
    }
}

// ---------------------------------------------------------------------------
// Test 3: net_profit_wei reflects balance delta of caller.
//
// Strategy: give the caller a starting balance, run a NOOP call with 0 value.
// Since gas_price = 0 (we set it in build_env), no ETH is spent, so the
// post-balance should equal the pre-balance and net_profit_wei = 0.
// ---------------------------------------------------------------------------
#[test]
fn net_profit_wei_reflects_balance_delta() {
    let initial_balance_wei: u128 = 5 * 10u128.pow(18); // 5 ETH
    let caller = make_addr(0x05);
    let contract = make_addr(0x06);

    let noop_code = noop_deployed_bytecode();
    let code_hash = noop_code.hash_slow();

    let mut db = MapDb::new();
    db.insert_account(
        caller,
        AccountInfo {
            balance: U256::from(initial_balance_wei),
            nonce: 0,
            code_hash: KECCAK_EMPTY,
            code: Some(Bytecode::new()),
        },
    );
    db.insert_account(
        contract,
        AccountInfo {
            balance: U256::ZERO,
            nonce: 1,
            code_hash,
            code: Some(noop_code),
        },
    );

    let candidate = CandidateInput {
        chain_id: 1,
        block_number: 21_000_000,
        from: make_addr_arr(0x05),
        to: make_addr_arr(0x06),
        calldata: vec![],
        value_wei: 0, // no ETH sent
    };

    let result = run(&candidate, db).expect("NOOP call should succeed");

    // With gas_price = 0 and value = 0, the caller's balance does not change.
    // net_profit_wei must be 0.
    assert_eq!(
        result.net_profit_wei, 0,
        "net_profit_wei should be 0 when balance is unchanged (gas_price=0, value=0)"
    );
}

// ---------------------------------------------------------------------------
// Test 4: trace_hash is deterministic — two identical calls produce the same
// hash.
// ---------------------------------------------------------------------------
#[test]
fn trace_hash_is_deterministic_for_identical_calls() {
    let caller = make_addr(0x07);
    let contract = make_addr(0x08);

    let noop_code = noop_deployed_bytecode();
    let code_hash = noop_code.hash_slow();

    let make_db = || {
        let mut db = MapDb::new();
        db.insert_account(
            caller,
            AccountInfo {
                balance: U256::from(10_u64.pow(18)),
                nonce: 0,
                code_hash: KECCAK_EMPTY,
                code: Some(Bytecode::new()),
            },
        );
        db.insert_account(
            contract,
            AccountInfo {
                balance: U256::ZERO,
                nonce: 1,
                code_hash,
                code: Some(noop_code.clone()),
            },
        );
        db
    };

    let candidate = || CandidateInput {
        chain_id: 1,
        block_number: 21_000_000,
        from: make_addr_arr(0x07),
        to: make_addr_arr(0x08),
        calldata: vec![0xAB, 0xCD],
        value_wei: 0,
    };

    let r1 = run(&candidate(), make_db()).expect("first call should succeed");
    let r2 = run(&candidate(), make_db()).expect("second call should succeed");

    assert_eq!(
        r1.trace_hash, r2.trace_hash,
        "trace_hash must be identical for two calls with same calldata and same return data"
    );
}

// ---------------------------------------------------------------------------
// Test 5: different calldata produces a different trace_hash.
// ---------------------------------------------------------------------------
#[test]
fn trace_hash_differs_for_different_calldata() {
    let caller = make_addr(0x09);
    let contract = make_addr(0x0A);

    let noop_code = noop_deployed_bytecode();
    let code_hash = noop_code.hash_slow();

    let make_db = || {
        let mut db = MapDb::new();
        db.insert_account(
            caller,
            AccountInfo {
                balance: U256::from(10_u64.pow(18)),
                nonce: 0,
                code_hash: KECCAK_EMPTY,
                code: Some(Bytecode::new()),
            },
        );
        db.insert_account(
            contract,
            AccountInfo {
                balance: U256::ZERO,
                nonce: 1,
                code_hash,
                code: Some(noop_code.clone()),
            },
        );
        db
    };

    let c1 = CandidateInput {
        chain_id: 1,
        block_number: 21_000_000,
        from: make_addr_arr(0x09),
        to: make_addr_arr(0x0A),
        calldata: vec![0x01, 0x02, 0x03],
        value_wei: 0,
    };
    let c2 = CandidateInput {
        calldata: vec![0x04, 0x05, 0x06],
        ..c1.clone()
    };

    let r1 = run(&c1, make_db()).expect("first call");
    let r2 = run(&c2, make_db()).expect("second call with different calldata");

    assert_ne!(
        r1.trace_hash, r2.trace_hash,
        "trace_hash must differ when calldata differs"
    );
}

// ---------------------------------------------------------------------------
// Test 6: provider error surfaces as SimError::Provider.
//
// We test this by making MapDb::basic() return Err for a specific address.
// ---------------------------------------------------------------------------
struct ErrorDb;

impl Database for ErrorDb {
    type Error = String;

    fn basic(&mut self, _address: Address) -> Result<Option<AccountInfo>, Self::Error> {
        Err("simulated RPC failure".to_owned())
    }

    fn code_by_hash(&mut self, _: B256) -> Result<Bytecode, Self::Error> {
        Err("simulated RPC failure".to_owned())
    }

    fn storage(&mut self, _: Address, _: U256) -> Result<U256, Self::Error> {
        Err("simulated RPC failure".to_owned())
    }

    fn block_hash(&mut self, _: U256) -> Result<B256, Self::Error> {
        Err("simulated RPC failure".to_owned())
    }
}

#[test]
fn provider_error_surfaces_as_sim_error_provider() {
    let candidate = CandidateInput {
        chain_id: 1,
        block_number: 21_000_000,
        from: make_addr_arr(0x0B),
        to: make_addr_arr(0x0C),
        calldata: vec![],
        value_wei: 0,
    };

    let result = run(&candidate, ErrorDb);

    match result {
        Err(SimError::Provider(_)) => { /* expected */ }
        Err(e) => panic!("expected Provider error, got: {e:?}"),
        Ok(_) => panic!("expected Provider error, got Ok"),
    }
}
