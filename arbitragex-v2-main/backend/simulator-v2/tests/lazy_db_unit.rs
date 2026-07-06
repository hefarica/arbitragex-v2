//! Unit tests for `lazy_db` cache semantics.
//!
//! These tests exercise the `DashMap` cache layer without making any real RPC
//! calls.  They use:
//!   - `LazyDb::new("http://…", Some(block))` — succeeds because
//!     `Provider::try_from` only parses the URL; no TCP connection is made.
//!   - `lazy_db::seed_*` helpers to pre-populate caches.
//!   - `lazy_db::*_cache_len` helpers to assert cache growth.
//!
//! For tests that would require an actual provider call (cache-miss RPC count),
//! see `backend/simulator-v2/tests/fork_lazy_db.rs` (operator-run, `#[ignore]`).

#![allow(clippy::unwrap_used, clippy::expect_used)]

use revm::primitives::{AccountInfo, Address, Bytecode, B256, KECCAK_EMPTY, U256};
use revm::Database;
use simulator_v2::lazy_db::{
    account_cache_len, block_hash_cache_len, seed_account, seed_block_hash, seed_storage,
    storage_cache_len, LazyDb,
};

/// A local URL that parses correctly but will never be connected to.
/// Used for LazyDb construction in tests where no provider call is expected.
const UNREACHABLE_RPC: &str = "http://127.0.0.1:19999";
/// Pinned block number used across all tests.
const PINNED_BLOCK: u64 = 21_000_000;

fn make_address(byte: u8) -> Address {
    let mut b = [0u8; 20];
    b[19] = byte;
    Address::from(b)
}

fn make_account_info(balance_eth: u64) -> AccountInfo {
    AccountInfo {
        balance: U256::from(balance_eth) * U256::from(10u128.pow(18)),
        nonce: 1,
        code_hash: KECCAK_EMPTY,
        code: Some(Bytecode::new()),
    }
}

// ---------------------------------------------------------------------------
// Test 1: cache hit returns the cached value without re-fetching.
//
// Invariant: after seeding an account, calling basic() on the same address
// returns the seeded value and the cache length stays at 1 (no duplicate
// entry).  Because the URL is unreachable, any actual RPC call would panic or
// time out — the test passing proves the cache-hit path was taken.
// ---------------------------------------------------------------------------
#[test]
fn cache_hit_returns_seeded_account_without_rpc() {
    let mut db = LazyDb::new(UNREACHABLE_RPC, Some(PINNED_BLOCK))
        .expect("LazyDb::new should succeed with a valid URL and pinned block");

    let addr = make_address(0x01);
    let info = make_account_info(10);

    // Seed the cache directly (simulates a previously-resolved RPC result).
    seed_account(&db, addr, info.clone());
    assert_eq!(
        account_cache_len(&db),
        1,
        "cache should have 1 entry after seed"
    );

    // basic() should return the cached value without calling the provider.
    let result = db.basic(addr).expect("basic() should succeed on cache hit");
    let returned_info = result.expect("seeded account should be Some");

    assert_eq!(
        returned_info.balance, info.balance,
        "balance must match seeded value"
    );
    assert_eq!(
        returned_info.nonce, info.nonce,
        "nonce must match seeded value"
    );

    // Cache length must remain 1 — no duplicate insertion.
    assert_eq!(
        account_cache_len(&db),
        1,
        "cache hit must not insert a duplicate entry"
    );
}

// ---------------------------------------------------------------------------
// Test 2: a second call for the same address on a warm cache does NOT grow
// the cache.
//
// This is the observable proxy for "cache hit re-uses the cached value": the
// cache count stays the same after the second call.
// ---------------------------------------------------------------------------
#[test]
fn second_call_same_address_does_not_grow_cache() {
    let mut db =
        LazyDb::new(UNREACHABLE_RPC, Some(PINNED_BLOCK)).expect("LazyDb::new should succeed");

    let addr = make_address(0x02);
    seed_account(&db, addr, make_account_info(5));

    // First call (cache hit).
    let _ = db.basic(addr).expect("first basic() call");
    let len_after_first = account_cache_len(&db);

    // Second call (cache hit again).
    let _ = db.basic(addr).expect("second basic() call");
    let len_after_second = account_cache_len(&db);

    assert_eq!(
        len_after_first, len_after_second,
        "cache length must not grow on repeated cache-hit calls"
    );
}

// ---------------------------------------------------------------------------
// Test 3: storage cache — cache hit returns seeded slot value without RPC.
// ---------------------------------------------------------------------------
#[test]
fn storage_cache_hit_returns_seeded_value() {
    let mut db =
        LazyDb::new(UNREACHABLE_RPC, Some(PINNED_BLOCK)).expect("LazyDb::new should succeed");

    let addr = make_address(0x10);
    let slot = U256::from(5u64);
    let value = U256::from(0xDEAD_BEEFu64);

    seed_storage(&db, addr, slot, value);
    assert_eq!(storage_cache_len(&db), 1);

    let result = db
        .storage(addr, slot)
        .expect("storage() should succeed on cache hit");
    assert_eq!(result, value, "storage cache hit must return seeded value");
    assert_eq!(
        storage_cache_len(&db),
        1,
        "storage cache hit must not insert a duplicate"
    );
}

// ---------------------------------------------------------------------------
// Test 4: block_hash for an unknown block (not in cache, not fetched) returns
// B256::ZERO via the fallback path when the seeded block number is absent.
//
// We test this by querying a block number that was never seeded.  Since the
// provider is unreachable, the only way this can succeed is if the code has
// a guard that avoids the RPC call for "unknown" blocks.
//
// BUT: block_hash() for an un-seeded block WOULD call the provider.  So we
// cannot test the "missing block returns zero" path without a running server.
//
// Instead, we test the *overflow guard*: block numbers > u64::MAX must not
// cause a panic or provider call, and must return KECCAK_EMPTY.
// ---------------------------------------------------------------------------
#[test]
fn block_hash_overflow_returns_keccak_empty() {
    let mut db =
        LazyDb::new(UNREACHABLE_RPC, Some(PINNED_BLOCK)).expect("LazyDb::new should succeed");

    // U256 value larger than u64::MAX.
    let huge_number = U256::from(u64::MAX) + U256::from(1u64);
    let result = db.block_hash(huge_number).expect("overflow must not error");

    assert_eq!(
        result, KECCAK_EMPTY,
        "block number > u64::MAX should return KECCAK_EMPTY"
    );
}

// ---------------------------------------------------------------------------
// Test 5: block_hash cache hit returns seeded hash without RPC.
// ---------------------------------------------------------------------------
#[test]
fn block_hash_cache_hit_returns_seeded_hash() {
    let mut db =
        LazyDb::new(UNREACHABLE_RPC, Some(PINNED_BLOCK)).expect("LazyDb::new should succeed");

    let block_num: u64 = 12_345_678;
    let expected_hash = B256::new([0xABu8; 32]);

    seed_block_hash(&db, block_num, expected_hash);
    assert_eq!(block_hash_cache_len(&db), 1);

    let result = db
        .block_hash(U256::from(block_num))
        .expect("block_hash() should succeed on cache hit");

    assert_eq!(
        result, expected_hash,
        "block_hash cache hit must return the seeded hash"
    );
    assert_eq!(
        block_hash_cache_len(&db),
        1,
        "cache hit must not insert a duplicate"
    );
}

// ---------------------------------------------------------------------------
// Test 6: code_by_hash returns an empty Bytecode (not an error).
//
// LazyDb.basic() always populates the code field, so code_by_hash is never
// called in normal operation.  The implementation must return Ok(Bytecode::new())
// as a defensive fallback rather than panicking.
// ---------------------------------------------------------------------------
#[test]
fn code_by_hash_returns_empty_bytecode_not_error() {
    let mut db =
        LazyDb::new(UNREACHABLE_RPC, Some(PINNED_BLOCK)).expect("LazyDb::new should succeed");

    let dummy_hash = B256::new([0x42u8; 32]);
    let result = db
        .code_by_hash(dummy_hash)
        .expect("code_by_hash must return Ok, never Err");

    assert!(
        result.is_empty(),
        "code_by_hash fallback should return empty Bytecode"
    );
}

// ---------------------------------------------------------------------------
// Test 7: multiple distinct accounts occupy separate cache slots.
// ---------------------------------------------------------------------------
#[test]
fn multiple_accounts_occupy_separate_cache_slots() {
    let mut db =
        LazyDb::new(UNREACHABLE_RPC, Some(PINNED_BLOCK)).expect("LazyDb::new should succeed");

    for i in 1u8..=5 {
        seed_account(&db, make_address(i), make_account_info(u64::from(i)));
    }
    assert_eq!(
        account_cache_len(&db),
        5,
        "five distinct addresses => five cache entries"
    );

    for i in 1u8..=5 {
        let result = db
            .basic(make_address(i))
            .expect("basic() on seeded address")
            .expect("seeded address must be Some");
        let expected_balance = U256::from(u64::from(i)) * U256::from(10u128.pow(18));
        assert_eq!(
            result.balance, expected_balance,
            "account {i} balance mismatch"
        );
    }
    // Cache length must not have grown (all hits).
    assert_eq!(account_cache_len(&db), 5);
}
