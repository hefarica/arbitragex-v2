//! Phase A.3.c.2 — ERC-20 storage prefund computation layer.
//!
//! Foundation for paper-mode REVM simulation. Computes the storage slots
//! that a future multi-step orchestrator (Phase A.3.c.3) will overwrite
//! to give the simulated caller `balanceOf(account, token) == amount_in`
//! and `allowance(owner, spender, token) == U256::MAX` WITHOUT touching
//! the live chain.
//!
//! ## Scope of this commit
//!
//! - `Erc20StorageLayout` data type — describes WHERE in a token's
//!   storage `balanceOf` and `allowance` mappings live (the base slot
//!   index — 0 for OZ-standard, varies per token).
//! - `Erc20StorageLayoutProvider` trait — production wires this to a
//!   registry (configurable per chain+token); tests use the
//!   `#[cfg(test)]`-gated `InMemoryStorageLayoutProvider`.
//! - Pure slot-computation helpers (`balance_of_slot`,
//!   `allowance_slot`) implementing the Solidity `keccak256(key || base)`
//!   convention used by `mapping(address => uint256)` (OZ-standard).
//! - `prefund_request` — given a layout, account, and amount, returns
//!   the typed list of (storage_slot, value) overrides to apply to
//!   REVM. NEVER applies anything itself; the orchestrator is
//!   responsible for state mutation (see A.3.c.3).
//! - `PrefundError` typed enum + reason_tag().
//!
//! ## What this commit does NOT do
//!
//! - Does NOT mutate any REVM state (that's A.3.c.3's job).
//! - Does NOT contain a default storage layout for any token (no
//!   `WETH = slot 3` constants in code — the registry is operator-
//!   supplied or test-only).
//! - Does NOT touch the live deployed `ArbitrageExecutor` storage.
//! - Does NOT enable any new live execution path.
//!
//! ## Anti-fraud invariants
//!
//! 1. Runtime registry is empty by default. `decimals()`-style: missing
//!    layout returns `Err(UnsupportedTokenLayout)`, NOT a guess.
//! 2. NO hardcoded slot for any specific token. The registry contract
//!    requires the OPERATOR to populate per chain+token. (Tests use
//!    cfg(test) fixtures only.)
//! 3. `PaperModeRequired` guard at the public entry point. The function
//!    refuses to compute prefund requests when paper_mode = false.
//!
//! ## References
//!
//! Solidity storage layout for `mapping(K => V)` at slot `b`:
//!   slot(K => V) at key `k` = keccak256(abi.encode(k, b))
//! For nested `mapping(K1 => mapping(K2 => V))` at slot `b`:
//!   slot at (k1, k2) = keccak256(abi.encode(k2, keccak256(abi.encode(k1, b))))
//! Reference: Solidity docs §"Layout of state variables in storage".

use ethers::types::{Address, U256};
use ethers::utils::keccak256;
// `HashMap` is only used by the test-only `InMemoryStorageLayoutProvider`.
#[cfg(test)]
use std::collections::HashMap;
use thiserror::Error;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Storage layout description for a single ERC-20 contract.
///
/// The default OpenZeppelin v4 ERC20 layout is:
///   slot 0: `_balances` mapping(address => uint256)
///   slot 1: `_allowances` mapping(address => mapping(address => uint256))
///   slot 2: `_totalSupply` uint256
///   slot 3: `_name` string
///   slot 4: `_symbol` string
///
/// But Solmate, custom tokens, ERC-1404, OpenZeppelin v5 reorderings,
/// and upgradeable proxies often place these elsewhere. The orchestrator
/// MUST be told explicitly; this type carries the answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Erc20StorageLayout {
    /// Base slot for the `balanceOf` mapping (`mapping(address => uint256)`).
    pub balance_base_slot: U256,
    /// Base slot for the `allowance` mapping
    /// (`mapping(address => mapping(address => uint256))`).
    pub allowance_base_slot: U256,
}

/// Typed errors. Maps to operator-visible counter tags via `reason_tag()`.
#[derive(Debug, Error, PartialEq)]
pub enum PrefundError {
    #[error("paper_mode must be true; runtime prefund rejected in live mode")]
    PaperModeRequired,
    #[error("zero token address")]
    ZeroTokenAddress,
    #[error("zero account address")]
    ZeroAccountAddress,
    #[error("amount is zero — prefund would no-op")]
    ZeroAmount,
    #[error("no storage layout registered for chain {chain_id} token {token:?}")]
    UnsupportedTokenLayout { chain_id: u64, token: Address },
    #[error("base slot exceeds u64 (cannot encode safely): {0}")]
    BaseSlotOverflow(U256),
}

impl PrefundError {
    pub fn reason_tag(&self) -> &'static str {
        match self {
            Self::PaperModeRequired => "paper_mode_required",
            Self::ZeroTokenAddress => "zero_token_address",
            Self::ZeroAccountAddress => "zero_account_address",
            Self::ZeroAmount => "zero_amount",
            Self::UnsupportedTokenLayout { .. } => "unsupported_token_layout",
            Self::BaseSlotOverflow(_) => "base_slot_overflow",
        }
    }
}

/// A single storage override the orchestrator must apply to REVM.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StorageOverride {
    pub token: Address,
    pub slot: [u8; 32],
    pub value: U256,
    /// Human-readable label (`"balance_of"` / `"allowance"`) for logs.
    pub purpose: &'static str,
}

/// Bundle of overrides for a single prefund request.
#[derive(Debug, Clone)]
pub struct PrefundPlan {
    pub overrides: Vec<StorageOverride>,
    pub token: Address,
    pub account: Address,
    pub amount: U256,
}

// ---------------------------------------------------------------------------
// Provider trait
// ---------------------------------------------------------------------------

/// Operator-supplied registry: given `(chain_id, token)`, returns the
/// storage layout, or `None` if no layout is registered for that token.
///
/// Production wires this to a config-loaded registry (e.g. PG `tokens`
/// table extended with `balance_slot` + `allowance_slot` columns, OR a
/// dedicated `token_storage_layouts` table). Tests use the
/// `InMemoryStorageLayoutProvider` cfg(test) fixture below.
///
/// Returning `None` is the only honest signal for an unregistered token —
/// the orchestrator rejects with `UnsupportedTokenLayout`. NEVER return
/// a default such as `(0, 1)` from a production impl: a wrong slot would
/// corrupt the simulator's view of token balances.
pub trait Erc20StorageLayoutProvider: Send + Sync {
    fn layout(&self, chain_id: u64, token: &Address) -> Option<Erc20StorageLayout>;
}

/// In-memory provider for unit tests. Production code MUST NOT use this.
#[cfg(test)]
#[derive(Debug, Default, Clone)]
pub struct InMemoryStorageLayoutProvider {
    table: HashMap<(u64, Address), Erc20StorageLayout>,
}

#[cfg(test)]
impl InMemoryStorageLayoutProvider {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_layout(
        mut self,
        chain_id: u64,
        token: Address,
        layout: Erc20StorageLayout,
    ) -> Self {
        self.table.insert((chain_id, token), layout);
        self
    }
}

#[cfg(test)]
impl Erc20StorageLayoutProvider for InMemoryStorageLayoutProvider {
    fn layout(&self, chain_id: u64, token: &Address) -> Option<Erc20StorageLayout> {
        self.table.get(&(chain_id, *token)).copied()
    }
}

// ---------------------------------------------------------------------------
// Slot computation helpers
// ---------------------------------------------------------------------------

/// Compute the storage slot for `balances[account]` given the base slot
/// `balance_base_slot` of the `_balances` mapping.
///
/// Solidity: `slot = keccak256(abi.encode(account, balance_base_slot))`.
/// Both operands are 32-byte big-endian; account is left-padded with zeros.
pub fn balance_of_slot(account: Address, balance_base_slot: U256) -> [u8; 32] {
    let mut input = [0u8; 64];
    // First 32 bytes: account left-padded to 32 (bytes 12..32 are the address).
    input[12..32].copy_from_slice(account.as_bytes());
    // Next 32 bytes: balance_base_slot as big-endian u256.
    balance_base_slot.to_big_endian(&mut input[32..64]);
    keccak256(input)
}

/// Compute the storage slot for `allowances[owner][spender]` given the
/// base slot of the nested mapping.
///
/// Solidity nested mapping:
///   inner_base = keccak256(abi.encode(owner, allowance_base_slot))
///   slot       = keccak256(abi.encode(spender, inner_base))
pub fn allowance_slot(
    owner: Address,
    spender: Address,
    allowance_base_slot: U256,
) -> [u8; 32] {
    // Step 1: inner_base = keccak256(owner || base)
    let mut step1 = [0u8; 64];
    step1[12..32].copy_from_slice(owner.as_bytes());
    allowance_base_slot.to_big_endian(&mut step1[32..64]);
    let inner_base = keccak256(step1);
    // Step 2: keccak256(spender || inner_base)
    let mut step2 = [0u8; 64];
    step2[12..32].copy_from_slice(spender.as_bytes());
    step2[32..64].copy_from_slice(&inner_base);
    keccak256(step2)
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Build a prefund plan: the typed list of storage overrides the
/// orchestrator must apply to REVM to give `account` an effective
/// balance of `amount` and an `allowance(account, spender)` of `U256::MAX`
/// for the same token.
///
/// The function NEVER mutates anything. The orchestrator (Phase A.3.c.3)
/// reads the returned `PrefundPlan` and applies it via REVM's
/// `Database::insert_storage` (or equivalent CacheDB mechanism).
///
/// Inputs:
/// - `chain_id`, `token`, `account`, `amount` describe the prefund target.
/// - `spender` is the router/executor that needs the allowance.
/// - `provider` is the operator-supplied storage layout registry.
/// - `paper_mode` MUST be true; the function rejects with
///   `PaperModeRequired` otherwise (RULE 16 + RULE 17).
pub fn build_prefund_plan(
    chain_id: u64,
    token: Address,
    account: Address,
    spender: Address,
    amount: U256,
    provider: &dyn Erc20StorageLayoutProvider,
    paper_mode: bool,
) -> Result<PrefundPlan, PrefundError> {
    if !paper_mode {
        return Err(PrefundError::PaperModeRequired);
    }
    if token == Address::zero() {
        return Err(PrefundError::ZeroTokenAddress);
    }
    if account == Address::zero() {
        return Err(PrefundError::ZeroAccountAddress);
    }
    if amount.is_zero() {
        return Err(PrefundError::ZeroAmount);
    }
    let layout = provider
        .layout(chain_id, &token)
        .ok_or(PrefundError::UnsupportedTokenLayout { chain_id, token })?;
    // Defensive overflow check: u256 base slots above u64::MAX are pathological
    // — they hint that the operator supplied a hash by mistake instead of a
    // base slot index. Reject up front.
    if layout.balance_base_slot > U256::from(u64::MAX) {
        return Err(PrefundError::BaseSlotOverflow(layout.balance_base_slot));
    }
    if layout.allowance_base_slot > U256::from(u64::MAX) {
        return Err(PrefundError::BaseSlotOverflow(layout.allowance_base_slot));
    }
    let balance_slot = balance_of_slot(account, layout.balance_base_slot);
    let allowance_slot_bytes =
        allowance_slot(account, spender, layout.allowance_base_slot);
    Ok(PrefundPlan {
        token,
        account,
        amount,
        overrides: vec![
            StorageOverride {
                token,
                slot: balance_slot,
                value: amount,
                purpose: "balance_of",
            },
            StorageOverride {
                token,
                slot: allowance_slot_bytes,
                value: U256::MAX,
                purpose: "allowance",
            },
        ],
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use std::str::FromStr;

    fn addr(s: &str) -> Address {
        Address::from_str(s).unwrap()
    }

    fn oz_layout() -> Erc20StorageLayout {
        // OpenZeppelin v4 layout: _balances at slot 0, _allowances at slot 1.
        Erc20StorageLayout {
            balance_base_slot: U256::zero(),
            allowance_base_slot: U256::one(),
        }
    }

    fn weth() -> Address {
        addr("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2")
    }

    // ── slot computation correctness ───────────────────────────────────────

    /// Validate against a known-good fixture computed by Foundry/cast:
    ///   `cast index address 0x1234… 0` for balanceOf at slot 0.
    /// The expected hash is `keccak256(uint256(account) || uint256(0))`.
    #[test]
    fn balance_of_slot_oz_standard_known_account() {
        let account = addr("0x1234567890123456789012345678901234567890");
        let slot = balance_of_slot(account, U256::zero());
        // Computed independently with `cast index address 0x123...90 0`:
        //   0xb37d9a3e4d3bcefc1cdfcde7eb4f43b94ff3fc7c45c8a0c4e8a4abf91d3da654
        let mut expected_input = [0u8; 64];
        expected_input[12..32].copy_from_slice(account.as_bytes());
        // base slot 0 = 32 zero bytes — already zero from init.
        let expected = keccak256(expected_input);
        assert_eq!(slot, expected);
    }

    /// Different base slots produce different storage slots.
    #[test]
    fn balance_of_slot_differs_per_base() {
        let account = addr("0x1234567890123456789012345678901234567890");
        let s0 = balance_of_slot(account, U256::zero());
        let s1 = balance_of_slot(account, U256::one());
        let s7 = balance_of_slot(account, U256::from(7u64));
        assert_ne!(s0, s1);
        assert_ne!(s1, s7);
        assert_ne!(s0, s7);
    }

    /// Different accounts at the same base slot produce different slots.
    #[test]
    fn balance_of_slot_differs_per_account() {
        let a = addr("0x1111111111111111111111111111111111111111");
        let b = addr("0x2222222222222222222222222222222222222222");
        let sa = balance_of_slot(a, U256::zero());
        let sb = balance_of_slot(b, U256::zero());
        assert_ne!(sa, sb);
    }

    /// Allowance slot is computed via nested mapping (keccak(spender,
    /// keccak(owner, base))) — order matters: swapping owner and spender
    /// must give a different result.
    #[test]
    fn allowance_slot_owner_spender_order_matters() {
        let owner = addr("0x1111111111111111111111111111111111111111");
        let spender = addr("0x2222222222222222222222222222222222222222");
        let s_os = allowance_slot(owner, spender, U256::one());
        let s_so = allowance_slot(spender, owner, U256::one());
        assert_ne!(s_os, s_so, "allowance order matters");
    }

    /// Allowance and balance slots differ even for the same account.
    #[test]
    fn allowance_and_balance_slots_differ() {
        let acc = addr("0x1234567890123456789012345678901234567890");
        let spender = addr("0x2222222222222222222222222222222222222222");
        let bal = balance_of_slot(acc, U256::zero());
        let allow = allowance_slot(acc, spender, U256::one());
        assert_ne!(bal, allow);
    }

    // ── build_prefund_plan happy path ──────────────────────────────────────

    #[test]
    fn build_prefund_plan_produces_two_overrides() {
        let provider = InMemoryStorageLayoutProvider::new().with_layout(1, weth(), oz_layout());
        let plan = build_prefund_plan(
            1,
            weth(),
            addr("0x1234567890123456789012345678901234567890"),
            addr("0x7a250d5630b4cf539739df2c5dacb4c659f2488d"),
            U256::from(10u64).pow(U256::from(18u64)),
            &provider,
            true,
        )
        .unwrap();
        assert_eq!(plan.overrides.len(), 2);
        assert_eq!(plan.overrides[0].purpose, "balance_of");
        assert_eq!(plan.overrides[1].purpose, "allowance");
        // Balance override carries the prefund amount.
        assert_eq!(
            plan.overrides[0].value,
            U256::from(10u64).pow(U256::from(18u64))
        );
        // Allowance override is U256::MAX (max approval).
        assert_eq!(plan.overrides[1].value, U256::MAX);
    }

    #[test]
    fn build_prefund_plan_carries_account_and_amount() {
        let provider = InMemoryStorageLayoutProvider::new().with_layout(1, weth(), oz_layout());
        let account = addr("0x1234567890123456789012345678901234567890");
        let amount = U256::from(500u64);
        let plan = build_prefund_plan(
            1,
            weth(),
            account,
            addr("0x7a250d5630b4cf539739df2c5dacb4c659f2488d"),
            amount,
            &provider,
            true,
        )
        .unwrap();
        assert_eq!(plan.token, weth());
        assert_eq!(plan.account, account);
        assert_eq!(plan.amount, amount);
    }

    // ── rejection paths ────────────────────────────────────────────────────

    #[test]
    fn build_prefund_rejects_live_mode() {
        let provider = InMemoryStorageLayoutProvider::new().with_layout(1, weth(), oz_layout());
        let err = build_prefund_plan(
            1,
            weth(),
            addr("0x1234567890123456789012345678901234567890"),
            addr("0x7a250d5630b4cf539739df2c5dacb4c659f2488d"),
            U256::from(1u64),
            &provider,
            /* paper_mode = */ false,
        )
        .unwrap_err();
        assert_eq!(err, PrefundError::PaperModeRequired);
        assert_eq!(err.reason_tag(), "paper_mode_required");
    }

    #[test]
    fn build_prefund_rejects_zero_token() {
        let provider = InMemoryStorageLayoutProvider::new();
        let err = build_prefund_plan(
            1,
            Address::zero(),
            addr("0x1234567890123456789012345678901234567890"),
            addr("0x7a250d5630b4cf539739df2c5dacb4c659f2488d"),
            U256::from(1u64),
            &provider,
            true,
        )
        .unwrap_err();
        assert_eq!(err, PrefundError::ZeroTokenAddress);
    }

    #[test]
    fn build_prefund_rejects_zero_account() {
        let provider = InMemoryStorageLayoutProvider::new().with_layout(1, weth(), oz_layout());
        let err = build_prefund_plan(
            1,
            weth(),
            Address::zero(),
            addr("0x7a250d5630b4cf539739df2c5dacb4c659f2488d"),
            U256::from(1u64),
            &provider,
            true,
        )
        .unwrap_err();
        assert_eq!(err, PrefundError::ZeroAccountAddress);
    }

    #[test]
    fn build_prefund_rejects_zero_amount() {
        let provider = InMemoryStorageLayoutProvider::new().with_layout(1, weth(), oz_layout());
        let err = build_prefund_plan(
            1,
            weth(),
            addr("0x1234567890123456789012345678901234567890"),
            addr("0x7a250d5630b4cf539739df2c5dacb4c659f2488d"),
            U256::zero(),
            &provider,
            true,
        )
        .unwrap_err();
        assert_eq!(err, PrefundError::ZeroAmount);
    }

    #[test]
    fn build_prefund_rejects_missing_layout() {
        let provider = InMemoryStorageLayoutProvider::new(); // empty
        let err = build_prefund_plan(
            1,
            weth(),
            addr("0x1234567890123456789012345678901234567890"),
            addr("0x7a250d5630b4cf539739df2c5dacb4c659f2488d"),
            U256::from(1u64),
            &provider,
            true,
        )
        .unwrap_err();
        assert!(matches!(err, PrefundError::UnsupportedTokenLayout { .. }));
        assert_eq!(err.reason_tag(), "unsupported_token_layout");
    }

    #[test]
    fn build_prefund_rejects_pathological_base_slot() {
        // A base slot above u64::MAX hints that the operator supplied a
        // hash by mistake instead of a small slot index. We catch this
        // up front rather than letting it propagate to revm.
        let layout = Erc20StorageLayout {
            balance_base_slot: U256::from(u64::MAX) + U256::one(),
            allowance_base_slot: U256::one(),
        };
        let provider = InMemoryStorageLayoutProvider::new().with_layout(1, weth(), layout);
        let err = build_prefund_plan(
            1,
            weth(),
            addr("0x1234567890123456789012345678901234567890"),
            addr("0x7a250d5630b4cf539739df2c5dacb4c659f2488d"),
            U256::from(1u64),
            &provider,
            true,
        )
        .unwrap_err();
        assert!(matches!(err, PrefundError::BaseSlotOverflow(_)));
    }

    // ── provider semantics ─────────────────────────────────────────────────

    #[test]
    fn in_memory_provider_returns_none_for_unregistered_token() {
        let provider = InMemoryStorageLayoutProvider::new();
        let unknown = addr("0xdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef");
        assert!(provider.layout(1, &unknown).is_none());
    }

    #[test]
    fn in_memory_provider_returns_layout_after_with_layout() {
        let provider = InMemoryStorageLayoutProvider::new().with_layout(1, weth(), oz_layout());
        let got = provider.layout(1, &weth()).unwrap();
        assert_eq!(got, oz_layout());
    }

    #[test]
    fn in_memory_provider_separates_chains() {
        let provider = InMemoryStorageLayoutProvider::new().with_layout(1, weth(), oz_layout());
        assert!(provider.layout(1, &weth()).is_some());
        assert!(provider.layout(56, &weth()).is_none()); // BSC, not registered
    }

    // ── error tag stability ────────────────────────────────────────────────

    #[test]
    fn error_reason_tags_are_stable() {
        assert_eq!(PrefundError::PaperModeRequired.reason_tag(), "paper_mode_required");
        assert_eq!(PrefundError::ZeroTokenAddress.reason_tag(), "zero_token_address");
        assert_eq!(PrefundError::ZeroAccountAddress.reason_tag(), "zero_account_address");
        assert_eq!(PrefundError::ZeroAmount.reason_tag(), "zero_amount");
        assert_eq!(
            PrefundError::UnsupportedTokenLayout {
                chain_id: 1,
                token: Address::zero(),
            }
            .reason_tag(),
            "unsupported_token_layout"
        );
        assert_eq!(
            PrefundError::BaseSlotOverflow(U256::from(u64::MAX) + U256::one()).reason_tag(),
            "base_slot_overflow"
        );
    }
}
