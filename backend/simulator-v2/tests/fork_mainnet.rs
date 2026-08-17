//! G-SIM-1 checklist item 3 — mainnet fork suite for simulator-v2's
//! multi-step REVM sequence runner (`sequence_runner`).
//!
//! THIS TEST IS `#[ignore]` BY DEFAULT — it requires external infrastructure
//! the unit-test sandbox does not have:
//!   * `RPC_HTTP_1` (or `ALCHEMY_HTTP_URL`) pointing at an Ethereum mainnet
//!     archive RPC that serves `eth_getBalance` / `eth_getCode` /
//!     `eth_getStorageAt` at a pinned block. It must be a single bare URL —
//!     `LazyDb` performs direct JSON-RPC and does NOT parse the multi-vendor
//!     `name=url,...` CSV form.
//!   * `FORK_BLOCK` (optional) — decimal mainnet block number to pin. When
//!     absent the suite resolves the latest block via the RPC and pins that.
//!
//! ## Running locally
//!
//! ```bash
//! export RPC_HTTP_1="https://eth-mainnet.g.alchemy.com/v2/<KEY>"
//! export FORK_BLOCK="<recent_mainnet_block>"   # optional
//! cargo test -p simulator-v2 --test fork_mainnet -- --ignored --nocapture
//! ```
//!
//! ## What the suite validates (fork-only, paper-only, read-only)
//!
//! 1. `LazyDb` pins to the requested block over the real RPC and feeds REVM
//!    real mainnet state (accounts + bytecode fetched on demand).
//! 2. A multi-step sequence through `SequenceContext` with cross-step state
//!    persistence in `CacheDB<LazyDb>`:
//!      * view read `balanceOf(WETH, whale)` — real forked state through REVM;
//!      * COMMITTED `WETH.deposit()` of 1 wei from a funded EOA (real WETH9
//!        bytecode executes, gas consumed);
//!      * view read proving the post-deposit balance is exactly `pre + 1`
//!        (the CacheDB persisted the mutation);
//!      * COMMITTED `WETH.withdraw(1)` and a final read proving the round
//!        trip restored the exact pre-balance.
//! 3. `finalize()` internal consistency: `successful_calls == 2`,
//!    `gas_used_total ==` sum of the committed calls' gas, non-zero trace
//!    hash (anti-fraud invariant from `sequence_runner`).
//!
//! Anti-hollow-pass contract (mirrors `backend/searcher-rs/tests/
//! multistep_fork.rs` + `scripts/run_a4_fork_validation.sh`): the test
//! prints exactly one machine-greppable `FORK_SUITE_OUTCOME=PASS` line ONLY
//! after the real assertions held; `.github/workflows/sim-fork-evidence.yml`
//! requires that marker (plus >= 1 passing libtest line) before recording
//! any evidence. Missing env PANICS (fail-honest) rather than skipping, so
//! an unconfigured run can never be recorded as a pass.
//!
//! DOCTRINE: fork validation only. NO signing, NO broadcast, NO capital.
//! The committed calls mutate ONLY the in-memory `CacheDB` of this process.
//! The whale address is a well-known public EOA (Binance hot wallet) used
//! as a caller label precisely because it is code-less (EIP-3607: REVM
//! rejects transactions from senders with deployed code) and funded — the
//! same paper-only convention `run_a4_fork_validation.sh` documents for
//! `EXECUTOR_1`.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::str::FromStr;

use simulator_v2::sequence_runner::{CallOutcome, SequenceCall, SequenceContext};
use simulator_v2::{AlloyAddress, AlloyU256, LazyDb};

/// Ethereum mainnet chain id (the fork target).
const CHAIN_ID_MAINNET: u64 = 1;

/// Value committed by the deposit() step, in wei. 1 wei: the smallest
/// possible real mutation — enough to prove state evolution without any
/// meaningful paper balance.
const DEPOSIT_WEI: u128 = 1;

/// Gas limit for each committed step. WETH deposit/withdraw need < 100k.
const GAS_LIMIT_PER_STEP: u64 = 200_000;

/// Canonical mainnet WETH9 (public contract address — test fixture, not
/// operator config).
fn weth9_mainnet() -> AlloyAddress {
    AlloyAddress::from_str("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2")
        .expect("canonical WETH9 address parses")
}

/// Well-known funded, code-less mainnet EOA (Binance 14 hot wallet). Used
/// only as the caller label for the paper-only committed calls — see the
/// module docs. Code-less matters: EIP-3607 rejects senders with deployed
/// code; funded matters: the 1-wei deposit must clear the balance check.
fn funded_eoa_mainnet() -> AlloyAddress {
    AlloyAddress::from_str("0x28c6c06298d514db089934071355e5743bf21d60")
        .expect("well-known mainnet EOA address parses")
}

/// `WETH9.deposit()` calldata — bare selector `0xd0e30db0` (no args).
fn deposit_calldata() -> Vec<u8> {
    vec![0xd0, 0xe3, 0x0d, 0xb0]
}

/// `WETH9.withdraw(uint256 wad)` calldata — selector `0x2e1a7d4d` plus the
/// amount left-padded to one 32-byte ABI word.
fn withdraw_calldata(wad_wei: u128) -> Vec<u8> {
    let mut calldata = Vec::with_capacity(36);
    calldata.extend_from_slice(&[0x2e, 0x1a, 0x7d, 0x4d]);
    calldata.extend_from_slice(&AlloyU256::from(wad_wei).to_be_bytes::<32>());
    calldata
}

/// Resolve the mainnet RPC URL from the environment. `RPC_HTTP_1` first,
/// then `ALCHEMY_HTTP_URL`. Returns the list of missing keys on failure.
fn resolve_rpc_url() -> Result<String, Vec<&'static str>> {
    let mut missing = Vec::new();
    for key in ["RPC_HTTP_1", "ALCHEMY_HTTP_URL"] {
        match std::env::var(key) {
            Ok(v) if !v.trim().is_empty() => return Ok(v.trim().to_owned()),
            _ => missing.push(key),
        }
    }
    Err(missing)
}

/// Resolve the optional `FORK_BLOCK` pin. Absent/empty → `None` (latest).
/// Present but non-decimal → panic (fail-honest, no silent fallback).
fn resolve_fork_block() -> Option<u64> {
    match std::env::var("FORK_BLOCK") {
        Ok(v) if !v.trim().is_empty() => Some(v.trim().parse::<u64>().unwrap_or_else(|_| {
            panic!("FORK_BLOCK must be a decimal u64 block number, got {v:?}")
        })),
        _ => None,
    }
}

/// Expect-style unwrap of a `CallOutcome` for a committed step: a Revert or
/// Halt against real forked state is a genuine failure of the suite (there
/// is no honest "skip" branch once the RPC answered).
fn require_success(outcome: CallOutcome, label: &str) -> u64 {
    match outcome {
        CallOutcome::Success { gas_used, .. } => gas_used,
        CallOutcome::Reverted { gas_used, reason } => {
            panic!("{label} reverted on the fork (gas {gas_used}): {reason}")
        }
        CallOutcome::Halted { gas_used, reason } => {
            panic!("{label} halted on the fork (gas {gas_used}): {reason}")
        }
    }
}

// ---------------------------------------------------------------------------
// Fork suite (ignored)
// ---------------------------------------------------------------------------

/// Multi-step REVM sequence against a pinned real mainnet block:
/// read → deposit(1 wei) → read (+1 exactly) → withdraw(1) → read (back to
/// pre). See the module docs for the full honesty contract.
///
/// `multi_thread` flavor is REQUIRED: `LazyDb`'s sync-async bridge calls
/// `tokio::time::timeout` (lazy_db.rs), which needs an ambient reactor at
/// construction — a bare sync `#[test]` panics with "there is no reactor
/// running" (found by the first real CI dispatch of sim-fork-evidence.yml,
/// 2026-08-17), and the default current-thread `#[tokio::test]` parks its
/// timer driver behind the owned-runtime fallback.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "requires RPC_HTTP_1 (mainnet archive RPC) + optional FORK_BLOCK — see module docs"]
async fn fork_mainnet_weth_deposit_withdraw_round_trip() {
    let rpc_url = match resolve_rpc_url() {
        Ok(url) => url,
        Err(missing) => panic!(
            "set RPC_HTTP_1 (or ALCHEMY_HTTP_URL) to a single bare mainnet archive RPC URL, \
             and optionally FORK_BLOCK=<decimal mainnet block>, then rerun with -- --ignored. \
             Missing env: {missing:?}. Without them the suite cannot reach real chain state."
        ),
    };
    let fork_block = resolve_fork_block();

    // 1. LazyDb pinned over the real RPC. With FORK_BLOCK set, construction
    //    never resolves "latest"; with it unset, LazyDb resolves and
    //    memoizes the current tip once.
    let lazy = LazyDb::new(&rpc_url, fork_block)
        .unwrap_or_else(|e| panic!("LazyDb::new against the provided RPC failed: {e}"));
    let block = lazy.pinned_block_number();
    if let Some(requested) = fork_block {
        assert_eq!(block, requested, "LazyDb must honor the FORK_BLOCK pin");
    }

    // 2. Multi-step REVM sequence over the pinned fork state.
    let whale = funded_eoa_mainnet();
    let weth = weth9_mainnet();
    let mut ctx = SequenceContext::new(lazy, CHAIN_ID_MAINNET, block);

    // 2a. View read #1 — real balanceOf(WETH, whale) through REVM + LazyDb.
    let pre = ctx
        .read_balance(weth, whale, "weth_whale_pre")
        .unwrap_or_else(|e| panic!("read_balance(WETH, whale) against block {block}: {e}"));
    assert!(
        !pre.is_zero(),
        "whale {whale} holds zero WETH at block {block} — the fork state or the fixture EOA is wrong"
    );

    // 2b. Committed call #1 — WETH deposit() of DEPOSIT_WEI from the whale.
    //     gas_price 0 keeps the caller-balance requirement at exactly the
    //     1-wei value (paper-only accounting; no real fee deduction).
    let deposit_gas = require_success(
        ctx.call(SequenceCall {
            from: whale,
            to: weth,
            calldata: deposit_calldata(),
            value_wei: DEPOSIT_WEI,
            gas_price_wei: 0,
            gas_limit: GAS_LIMIT_PER_STEP,
            label: "weth_deposit",
        })
        .unwrap_or_else(|e| panic!("deposit transact_commit infra error: {e}")),
        "WETH deposit",
    );
    assert!(deposit_gas > 0, "committed deposit consumed zero gas");

    // 2c. View read #2 — post-deposit balance must be EXACTLY pre + deposit
    //     value: proves the CacheDB persisted the committed mutation across
    //     steps (sequence_runner invariant #2).
    let post = ctx
        .read_balance(weth, whale, "weth_whale_post")
        .unwrap_or_else(|e| panic!("read_balance(WETH, whale) post-deposit: {e}"));
    let expected_post = pre
        .checked_add(AlloyU256::from(DEPOSIT_WEI))
        .expect("u256 checked_add of 1 wei");
    assert_eq!(
        post, expected_post,
        "post-deposit WETH balance must equal pre + deposit value"
    );

    // 2d. Committed call #2 — WETH withdraw(DEPOSIT_WEI): burns the minted
    //     WETH and returns the wei to the caller.
    let withdraw_gas = require_success(
        ctx.call(SequenceCall {
            from: whale,
            to: weth,
            calldata: withdraw_calldata(DEPOSIT_WEI),
            value_wei: 0,
            gas_price_wei: 0,
            gas_limit: GAS_LIMIT_PER_STEP,
            label: "weth_withdraw",
        })
        .unwrap_or_else(|e| panic!("withdraw transact_commit infra error: {e}")),
        "WETH withdraw",
    );
    assert!(withdraw_gas > 0, "committed withdraw consumed zero gas");

    // 2e. View read #3 — round trip restored: final == pre, exactly.
    let final_balance = ctx
        .read_balance(weth, whale, "weth_whale_final")
        .unwrap_or_else(|e| panic!("read_balance(WETH, whale) post-withdraw: {e}"));
    assert_eq!(
        final_balance, pre,
        "post-withdraw WETH balance must return to the exact pre-deposit value"
    );

    // 3. Aggregate internal consistency (sequence_runner invariants #1/#4).
    let summary = ctx.finalize();
    assert_eq!(
        summary.successful_calls, 2,
        "exactly the two committed calls must have succeeded"
    );
    assert!(
        summary.gas_used_total > 0,
        "zero total gas after committed calls"
    );
    assert_eq!(
        summary.gas_used_total,
        deposit_gas + withdraw_gas,
        "gas_used_total must equal the sum of the committed calls' gas"
    );
    assert_ne!(
        summary.trace_hash, [0u8; 32],
        "anti-fraud: non-zero trace hash required after successful calls"
    );

    // The single machine-greppable marker — emitted ONLY after every
    // assertion above held. The evidence workflow requires this line.
    println!(
        "FORK_SUITE_OUTCOME=PASS block={block} chain={CHAIN_ID_MAINNET} \
         successful_calls={} gas_used_total={} weth_pre={pre} round_trip_wei={DEPOSIT_WEI}",
        summary.successful_calls, summary.gas_used_total
    );
}

// ---------------------------------------------------------------------------
// Smoke tests (NOT ignored) — verify the suite's pure helpers without any
// RPC or env access, so the test binary stays honest (>= 1 executed test)
// in the RPC-less unit-test job. No env-var mutation anywhere in this file.
// ---------------------------------------------------------------------------

#[test]
fn calldata_builders_produce_canonical_weth9_selectors() {
    // deposit() — bare selector 0xd0e30db0.
    assert_eq!(deposit_calldata(), vec![0xd0, 0xe3, 0x0d, 0xb0]);

    // withdraw(uint256) — selector 0x2e1a7d4d + amount as one 32-byte
    // big-endian word (left-padded).
    let calldata = withdraw_calldata(1);
    assert_eq!(calldata.len(), 36);
    assert_eq!(&calldata[0..4], &[0x2e, 0x1a, 0x7d, 0x4d]);
    assert_eq!(&calldata[4..35], &[0u8; 31]);
    assert_eq!(calldata[35], 1);
}

#[test]
fn fixture_addresses_parse_to_distinct_non_zero_accounts() {
    let weth = weth9_mainnet();
    let whale = funded_eoa_mainnet();
    assert_ne!(weth, whale);
    assert!(!weth.is_zero());
    assert!(!whale.is_zero());
    assert_eq!(weth.as_slice().len(), 20);
    assert_eq!(whale.as_slice().len(), 20);
}
