//! ERC20 storage layout helpers — Phase 3 of REVM real implementation.
//!
//! When simulating a swap in REVM, the synthetic caller (EOA) starts with
//! zero token balance. To execute a real swap, we must pre-fund the caller
//! by overriding the storage slot that holds `balances[caller]` for the
//! token being sold. This module provides the slot lookup + computation
//! needed for that override.
//!
//! Two-tier API:
//!
//!   1. `balance_slot_for(token) -> Option<u32>`
//!      Hardcoded table of declaration-order slot indices for top tokens.
//!      Empirically derived from Etherscan source — there is no standard
//!      way to compute this from bytecode alone (compiler-dependent).
//!
//!   2. `balance_storage_slot_at(holder, slot_index) -> H256`
//!      Pure computation of keccak256(abi.encode(holder, slot_index)) —
//!      Solidity's canonical mapping storage layout.
//!
//! The convenience `balance_storage_slot(token, holder)` combines both.
//!
//! Phase 4 (round-trip executor) will consume this to populate
//! `LazyRpcDatabase::storage` before calling `evm.transact()`.
//!
//! See `docs/superpowers/plans/2026-05-05-revm-real-implementation.md`.

use ethers::types::{Address, H256, U256};
use ethers::utils::keccak256;
use shared_rs::chains::{
    AAVE_MAINNET_LC, DAI_MAINNET_LC, LINK_MAINNET_LC, UNI_MAINNET_LC, USDC_MAINNET_LC,
    WBTC_MAINNET_LC, WETH_MAINNET_LC,
};

/// Hardcoded balance mapping slot indices for top ERC20 tokens, derived
/// from Etherscan source inspection. Adding a new token requires reading
/// its source on Etherscan, finding the storage declaration of the
/// `balances` mapping, and counting its slot index from 0 (skipping any
/// preceding state vars).
///
/// Returns `None` for unknown tokens — the caller must either fall back
/// to a "whale balance" simulation strategy OR reject the simulation.
pub fn balance_slot_for(token: Address) -> Option<u32> {
    // Format the address as canonical lowercase 40-char hex with 0x prefix
    // for stable matching against the table below.
    let token_lower = format!("0x{:040x}", token);
    match token_lower.as_str() {
        // WETH9 (0xc02aaa39…) — slot 3
        WETH_MAINNET_LC => Some(3),
        // USDT TetherToken (0xdac17f95…) — slot 2
        "0xdac17f958d2ee523a2206206994597c13d831ec7" => Some(2),
        // USDC FiatTokenV2_2 proxy (0xa0b86991…) — slot 9
        USDC_MAINNET_LC => Some(9),
        // DAI (0x6b175474…) — slot 2
        DAI_MAINNET_LC => Some(2),
        // WBTC (0x2260fac5…) — slot 0
        WBTC_MAINNET_LC => Some(0),
        // LINK (0x51491077…) — slot 1
        LINK_MAINNET_LC => Some(1),
        // UNI (0x1f9840a8…) — slot 4
        UNI_MAINNET_LC => Some(4),
        // AAVE (0x7fc66500…) — slot 0
        AAVE_MAINNET_LC => Some(0),
        _ => None,
    }
}

/// Compute the EVM storage slot for `balances[holder]` in an ERC20 contract
/// where the balances mapping lives at `slot_index` (in declaration order).
///
/// Uses Solidity's canonical mapping layout:
///   `slot = keccak256(abi.encode(holder, slot_index))`
///
/// abi.encode pads `holder` to 32 bytes (left-padded with zeros) and
/// `slot_index` to 32 bytes (uint256 big-endian). The 64-byte concatenation
/// is hashed with keccak256 to yield the storage location.
pub fn balance_storage_slot_at(holder: Address, slot_index: u32) -> H256 {
    let mut input = [0u8; 64];
    // Left-pad address into the first 32 bytes (last 20 bytes hold the addr).
    input[12..32].copy_from_slice(holder.as_bytes());
    // Right side: uint256 big-endian of slot_index.
    let slot_u256 = U256::from(slot_index);
    let mut slot_bytes = [0u8; 32];
    slot_u256.to_big_endian(&mut slot_bytes);
    input[32..64].copy_from_slice(&slot_bytes);
    H256::from(keccak256(input))
}

/// Convenience: compute the storage slot for `balances[holder]` for a known
/// token. Returns `None` if the token's balance slot index is not in the
/// hardcoded table — caller must handle the gap explicitly (no silent zero).
pub fn balance_storage_slot(token: Address, holder: Address) -> Option<H256> {
    balance_slot_for(token).map(|slot_index| balance_storage_slot_at(holder, slot_index))
}

// ─── Allowance overrides (Phase 5a of #6) ────────────────────────────────
//
// The simulator must override `allowances[caller][router]` so the router can
// `transferFrom` the pre-funded balance. Allowance is a NESTED mapping:
//   `mapping(address => mapping(address => uint256)) allowances`
// → storage slot = keccak256(spender_padded || keccak256(owner_padded || allowance_slot_index))

/// Hardcoded allowance mapping slot indices for top ERC20 tokens (parallel
/// to `balance_slot_for`). The allowance mapping usually lives one slot
/// AFTER the balance mapping in declaration order — but compiler-dependent,
/// so each entry is empirically verified.
///
/// Adding a new token: read source, find `mapping(address => mapping(address
/// => uint256))` declaration, count its slot index from 0.
pub fn allowance_slot_for(token: Address) -> Option<u32> {
    let token_lower = format!("0x{:040x}", token);
    match token_lower.as_str() {
        // WETH9 — slot 4 (after balances at 3)
        WETH_MAINNET_LC => Some(4),
        // USDT TetherToken — slot 5 (after balances at 2 + extra storage)
        "0xdac17f958d2ee523a2206206994597c13d831ec7" => Some(5),
        // USDC FiatTokenV2_2 proxy — slot 10 (after balances at 9)
        USDC_MAINNET_LC => Some(10),
        // DAI — slot 3 (after balances at 2)
        DAI_MAINNET_LC => Some(3),
        // WBTC — slot 1 (after balances at 0)
        WBTC_MAINNET_LC => Some(1),
        // LINK — slot 2 (after balances at 1)
        LINK_MAINNET_LC => Some(2),
        // UNI — slot 5 (after balances at 4)
        UNI_MAINNET_LC => Some(5),
        // AAVE — slot 1 (after balances at 0)
        AAVE_MAINNET_LC => Some(1),
        _ => None,
    }
}

/// Compute the EVM storage slot for `allowances[owner][spender]` in an ERC20
/// where the allowance nested mapping lives at `slot_index`.
///
/// Two-level keccak (Solidity nested-mapping layout):
///   inner = keccak256(abi.encode(owner, slot_index))
///   slot  = keccak256(abi.encode(spender, inner))
pub fn allowance_storage_slot_at(owner: Address, spender: Address, slot_index: u32) -> H256 {
    // Inner: keccak256(owner_padded || slot_index_padded)
    let mut inner_input = [0u8; 64];
    inner_input[12..32].copy_from_slice(owner.as_bytes());
    let slot_u256 = U256::from(slot_index);
    let mut slot_bytes = [0u8; 32];
    slot_u256.to_big_endian(&mut slot_bytes);
    inner_input[32..64].copy_from_slice(&slot_bytes);
    let inner = keccak256(inner_input);

    // Outer: keccak256(spender_padded || inner)
    let mut outer_input = [0u8; 64];
    outer_input[12..32].copy_from_slice(spender.as_bytes());
    outer_input[32..64].copy_from_slice(&inner);
    H256::from(keccak256(outer_input))
}

/// Convenience: compute the storage slot for `allowances[owner][spender]` for
/// a known token. Returns `None` if the token's allowance slot index is not
/// in the hardcoded table.
pub fn allowance_storage_slot(token: Address, owner: Address, spender: Address) -> Option<H256> {
    allowance_slot_for(token).map(|idx| allowance_storage_slot_at(owner, spender, idx))
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared_rs::chains::WETH_MAINNET;
    use std::str::FromStr;

    fn addr(hex: &str) -> Address {
        Address::from_str(hex).expect("valid hex address")
    }

    #[test]
    fn known_tokens_have_correct_balance_slots() {
        // Each pair verified from the contract source on Etherscan.
        for (token, expected_slot) in [
            (WETH_MAINNET_LC, 3),                              // WETH
            ("0xdac17f958d2ee523a2206206994597c13d831ec7", 2), // USDT
            (USDC_MAINNET_LC, 9),                              // USDC
            (DAI_MAINNET_LC, 2),                               // DAI
            (WBTC_MAINNET_LC, 0),                              // WBTC
            (LINK_MAINNET_LC, 1),                              // LINK
            (UNI_MAINNET_LC, 4),                               // UNI
            (AAVE_MAINNET_LC, 0),                              // AAVE
        ] {
            assert_eq!(
                balance_slot_for(addr(token)),
                Some(expected_slot),
                "wrong balance slot for {token}",
            );
        }
    }

    #[test]
    fn unknown_token_returns_none() {
        // Never-deployed address — must fail-honest with None, not fabricate
        // a default slot (RULE 00).
        assert_eq!(
            balance_slot_for(addr("0x0000000000000000000000000000000000000001")),
            None,
        );
        // Random valid-looking address (PEPE) — also unknown.
        assert_eq!(
            balance_slot_for(addr("0x6982508145454ce325ddbe47a25d4ec3d2311933")),
            None,
        );
    }

    #[test]
    fn case_insensitive_token_lookup() {
        // The format!("{:040x}") output is always lowercase, so uppercase
        // input addresses still match. This avoids surprises when callers
        // pass a checksummed address.
        assert_eq!(balance_slot_for(addr(WETH_MAINNET)), Some(3));
    }

    #[test]
    fn storage_slot_computation_is_deterministic() {
        let holder = addr("0x0000000000000000000000000000000000000001");
        let s1 = balance_storage_slot_at(holder, 3);
        let s2 = balance_storage_slot_at(holder, 3);
        assert_eq!(s1, s2, "same input must yield same slot");
        assert_ne!(
            s1,
            H256::zero(),
            "non-trivial computation must not return zero"
        );
    }

    #[test]
    fn different_holders_yield_different_slots() {
        let h1 = addr("0x0000000000000000000000000000000000000001");
        let h2 = addr("0x0000000000000000000000000000000000000002");
        assert_ne!(
            balance_storage_slot_at(h1, 3),
            balance_storage_slot_at(h2, 3),
            "balances[a] and balances[b] must map to distinct slots",
        );
    }

    #[test]
    fn different_slot_indices_yield_different_slots() {
        let h = addr("0x0000000000000000000000000000000000000001");
        assert_ne!(
            balance_storage_slot_at(h, 3),
            balance_storage_slot_at(h, 9),
            "balances at slot_index 3 vs 9 must be distinct mappings",
        );
    }

    #[test]
    fn balance_storage_slot_resolves_for_known_token() {
        let weth = addr(WETH_MAINNET);
        let holder = addr("0x0000000000000000000000000000000000000001");
        let resolved = balance_storage_slot(weth, holder).expect("WETH should be known");
        assert_eq!(resolved, balance_storage_slot_at(holder, 3));
    }

    #[test]
    fn balance_storage_slot_returns_none_for_unknown_token() {
        let unknown = addr("0x0000000000000000000000000000000000000001");
        let holder = addr("0x0000000000000000000000000000000000000002");
        assert_eq!(balance_storage_slot(unknown, holder), None);
    }

    /// Regression: known reference value for keccak256(abi.encode(addr0x1, 0)).
    /// This is a well-known slot used in many MEV bot codebases — if our
    /// computation drifts from this, the entire override mechanism is broken.
    /// The expected value comes from the Solidity-equivalent computation
    /// performed by `forge` and `cast`.
    #[test]
    fn keccak_layout_matches_solidity_canonical() {
        let holder = addr("0x0000000000000000000000000000000000000001");
        let slot_at_zero = balance_storage_slot_at(holder, 0);
        // keccak256(0x000…0001 (32B) || 0x000…000 (32B)) =
        // 0xada5013122d395ba3c54772283fb069b10426056ef8ca54750cb9bb552a59e7d
        // (verified via `cast keccak 0x000…0001000…000` on mainnet fork)
        let expected = H256::from_slice(
            &hex::decode("ada5013122d395ba3c54772283fb069b10426056ef8ca54750cb9bb552a59e7d")
                .expect("valid hex"),
        );
        assert_eq!(
            slot_at_zero, expected,
            "keccak256 layout drift would break ALL storage overrides",
        );
    }

    // ─── Allowance helpers (Phase 5a of #6) ──────────────────────────────

    #[test]
    fn known_tokens_have_correct_allowance_slots() {
        for (token, expected_slot) in [
            (WETH_MAINNET_LC, 4),                              // WETH
            ("0xdac17f958d2ee523a2206206994597c13d831ec7", 5), // USDT
            (USDC_MAINNET_LC, 10),                             // USDC
            (DAI_MAINNET_LC, 3),                               // DAI
            (WBTC_MAINNET_LC, 1),                              // WBTC
            (LINK_MAINNET_LC, 2),                              // LINK
            (UNI_MAINNET_LC, 5),                               // UNI
            (AAVE_MAINNET_LC, 1),                              // AAVE
        ] {
            assert_eq!(
                allowance_slot_for(addr(token)),
                Some(expected_slot),
                "wrong allowance slot for {token}",
            );
        }
    }

    #[test]
    fn unknown_token_allowance_returns_none() {
        assert_eq!(
            allowance_slot_for(addr("0x0000000000000000000000000000000000000001")),
            None,
        );
    }

    #[test]
    fn allowance_slot_two_level_keccak_is_deterministic() {
        let owner = addr("0x0000000000000000000000000000000000000001");
        let spender = addr("0x0000000000000000000000000000000000000002");
        let s1 = allowance_storage_slot_at(owner, spender, 4);
        let s2 = allowance_storage_slot_at(owner, spender, 4);
        assert_eq!(s1, s2);
        assert_ne!(s1, H256::zero());
    }

    #[test]
    fn allowance_slot_distinguishes_owner_and_spender() {
        let a = addr("0x0000000000000000000000000000000000000001");
        let b = addr("0x0000000000000000000000000000000000000002");
        // allowances[a][b] != allowances[b][a]  — direction matters
        assert_ne!(
            allowance_storage_slot_at(a, b, 4),
            allowance_storage_slot_at(b, a, 4),
            "allowance ordering is owner-first then spender; swap must yield distinct slots",
        );
    }

    #[test]
    fn allowance_slot_distinct_from_balance_slot() {
        // balances[holder] and allowances[holder][holder] at the same slot_index
        // MUST yield distinct storage slots — different mapping shapes.
        let holder = addr("0x0000000000000000000000000000000000000001");
        let bal = balance_storage_slot_at(holder, 4);
        let allow = allowance_storage_slot_at(holder, holder, 4);
        assert_ne!(
            bal, allow,
            "balance vs allowance keccak layouts must produce distinct slots"
        );
    }

    #[test]
    fn allowance_storage_slot_resolves_for_known_token() {
        let weth = addr(WETH_MAINNET);
        let owner = addr("0x0000000000000000000000000000000000000001");
        let spender = addr("0x0000000000000000000000000000000000000002");
        let slot = allowance_storage_slot(weth, owner, spender).expect("WETH allowance known");
        // Should equal the at-slot-4 version (WETH's allowance index)
        assert_eq!(slot, allowance_storage_slot_at(owner, spender, 4));
    }
}
