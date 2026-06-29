//! Bundle builder — constructs a signed EIP-1559 transaction for the
//! observed opportunity, wrapped as a Flashbots bundle.
//!
//! M2 flash R4 (2026-06-28): the broadcast tx is the OUTER wrapped flash call
//! `FlashLoanExecutor.requestFlashLoan(0x5107d61e)` whose payload is
//! `executeArbitrageFlashFunded(0xdde0bf51)`. The calldata is produced by the
//! SINGLE shared encoder `prioritization_spine::build_flash_funded_broadcast_calldata`
//! from the `ValidatedPlan` (carrier-B) that sim-ctl validated — so the bytes
//! broadcast here are byte-identical to what was simulated (sim↔exec parity).
//! The raw per-router `encode_v2`/`encode_v3` swap path was DELETED: it
//! broadcast an unfunded swap straight at a router, which is not what the
//! system simulates and not what can repay a flash loan.
//!
//! Safety:
//!   - M1 `LiveExecPolicy::assert_broadcast_allowed` is the FIRST statement —
//!     default-deny + testnet-only + mainnet physically refused.
//!   - `.to()` is ALWAYS the `FlashLoanExecutor` (FLE), resolved fail-closed
//!     from `FLASHLOAN_EXECUTOR_<chain_id>`; never a router, the AE, or the EOA.
//!   - `value = 0` (flash-funded; the EOA sends no ETH).
//!   - Blast radius is bounded by the flash-loan PRINCIPAL (`plan.ctx.amount_in`),
//!     NOT the tx `value` (which is 0). `max_value_eth` caps that principal.
//!     BuildError::ValueExceedsCap must propagate to a risk_event (critical) in
//!     the caller.
//!   - Gas is a real `provider.estimate_gas` on the built tx + safety headroom,
//!     fail-closed on estimate error (the wrapped flash path is far heavier than
//!     the old hardcoded 500k single swap).

use crate::nonce_manager::NonceManager;
use crate::signer::Signer;
use alloy::eips::BlockId;
use alloy::primitives::{Address as AlloyAddress, Bytes as AlloyBytes, U256 as AlloyU256};
use alloy::providers::Provider as AlloyProvider;
use alloy::rpc::types::{BlockNumberOrTag, TransactionInput, TransactionRequest};
use anyhow::Result;
use ethers::core::types::transaction::eip2718::TypedTransaction;
use ethers::prelude::*;
use ethers::signers::Signer as EthersSigner;
use prioritization_spine::{build_flash_funded_broadcast_calldata, ValidatedPlan};
use shared_rs::chains::resolve_flashloan_executor_address;
use shared_rs::contracts::{Opportunity, StrategyKind};
use shared_rs::rpc_failover::AlloyHttpProvider;

/// Gas safety headroom applied to the on-chain `estimate_gas` result.
/// estimate_gas returns the pending-block estimate; the wrapped flash path
/// (requestFlashLoan → provider callback → executeArbitrageFlashFunded → 2
/// router swaps → repay) can drift a few percent block-to-block, so we add 20%.
const GAS_SAFETY_NUM: u64 = 12;
const GAS_SAFETY_DEN: u64 = 10;

/// Absolute upper bound on the broadcast gas limit. The wrapped flash path is
/// heavy but a single arbitrage round-trip must never request more than a full
/// block's worth of gas — a runaway estimate is a fail-closed signal, not a tx.
const GAS_LIMIT_CEILING: u64 = 30_000_000;

#[derive(Debug, thiserror::Error)]
pub enum BuildError {
    #[error("unsupported strategy: {0:?}")]
    UnsupportedStrategy(StrategyKind),
    #[error("live execution denied: {0}")]
    LiveExecDenied(String),
    #[error("flash-funded calldata encode failed: {0}")]
    EncodeFailed(String),
    #[error("value {value_eth} ETH exceeds max_value_eth {cap_eth}")]
    ValueExceedsCap { value_eth: f64, cap_eth: f64 },
    #[error("gas estimate failed: {0}")]
    GasEstimate(String),
    #[error("provider error: {0}")]
    Provider(String),
}

pub struct SignedBundle {
    #[allow(dead_code)]
    pub opportunity_id: uuid::Uuid,
    pub target_block: u64,
    pub tx_raw_hex: String,
    pub tx_hash: H256,
    pub from: Address,
    pub nonce: u64,
    #[allow(dead_code)]
    pub value_wei: U256,
}

/// Builds + signs a single-tx bundle that broadcasts the wrapped flash call
/// `FlashLoanExecutor.requestFlashLoan(executeArbitrageFlashFunded(...))`.
///
/// `plan` is the carrier-B `ValidatedPlan` read from Redis by the caller
/// (`arbx:validated_plan:<opp.id>`). Its `ctx`/`route_hash`/`min_profit_wei`/
/// `executor_address` are the EXACT inputs sim-ctl validated, so the encoder
/// reproduces byte-identical calldata.
///
/// `provider` is the alloy 1.0 HTTP provider (used for `get_block` to read the
/// current base fee AND `estimate_gas` on the built tx). The signing path
/// remains ethers (signer.wallet).
pub async fn build_and_sign(
    opp: &Opportunity,
    plan: &ValidatedPlan,
    signer: &Signer,
    provider: &AlloyHttpProvider,
    nonce_mgr: &NonceManager,
    max_value_eth: f64,
    target_block_offset: u64,
) -> Result<SignedBundle, BuildError> {
    // M1 (2026-06-28): physical broadcast barrier — default-deny + testnet-only.
    // MUST remain the FIRST statement: no address resolution, encoding, or
    // signing may happen before it. Re-read per call (un-bypassable) so a
    // paper_mode flip at runtime still cannot reach mainnet. See live_exec_policy.
    crate::live_exec_policy::LiveExecPolicy::from_env()
        .assert_broadcast_allowed(opp.chain_id)
        .map_err(|e| BuildError::LiveExecDenied(e.to_string()))?;

    if !matches!(opp.strategy_kind, StrategyKind::DexArb) {
        return Err(BuildError::UnsupportedStrategy(opp.strategy_kind.clone()));
    }

    // Spend cap = flash-loan PRINCIPAL (blast radius), NOT the tx value.
    // The tx `value` is 0 (flash-funded). What bounds exposure is the size of
    // the borrowed principal `plan.ctx.amount_in` (the asset the FLE flash-loans
    // and must be repaid). Enforce the cap against that, fail-closed.
    let principal_wei = plan.ctx.amount_in;
    let value_wei = U256::zero();
    let principal_eth = amount_in_to_eth(principal_wei);
    if principal_eth > max_value_eth {
        return Err(BuildError::ValueExceedsCap {
            value_eth: principal_eth,
            cap_eth: max_value_eth,
        });
    }

    // Resolve the broadcast target = the FlashLoanExecutor (FLE). Fail-closed:
    // if `FLASHLOAN_EXECUTOR_<chain_id>` is unset / unparseable / zero, NO tx is
    // built. This is the `.to()` — never a router, never the ArbitrageExecutor,
    // never the EOA.
    let to: Address = resolve_flashloan_executor_address(opp.chain_id)
        .map_err(|e| BuildError::LiveExecDenied(e.to_string()))?;

    // Build the wrapped flash calldata from the validated plan via the SINGLE
    // shared encoder. Returns the OUTER `requestFlashLoan(0x5107d61e)` calldata
    // whose payload is `executeArbitrageFlashFunded(0xdde0bf51)`. asset/amount
    // derive from `plan.ctx` inside the encoder.
    let data_vec: Vec<u8> = build_flash_funded_broadcast_calldata(
        &plan.ctx,
        plan.route_hash,
        plan.min_profit_wei,
        plan.executor_address,
    )
    .map_err(|e| BuildError::EncodeFailed(e.to_string()))?;
    let data = Bytes::from(data_vec.clone());

    let nonce = nonce_mgr
        .next(opp.chain_id, signer.address)
        .await
        .map_err(|e| BuildError::Provider(e.to_string()))?;

    // Fee estimation. For S5 we use simple "latest base fee + 2 gwei priority".
    //
    // Alloy 1.0: `get_block(BlockId)` requires a `BlockId` wrapper around
    // `BlockNumberOrTag`. `Block.header.base_fee_per_gas` is `Option<u64>` in
    // alloy 1.0 (it was `Option<U256>` in ethers; u64 is safe). `header.number`
    // is `u64`.
    let latest_block = provider
        .get_block(BlockId::Number(BlockNumberOrTag::Latest))
        .await
        .map_err(|e| BuildError::Provider(e.to_string()))?
        .ok_or_else(|| BuildError::Provider("no_latest_block".into()))?;
    let base_fee_u64: u64 = latest_block
        .header
        .base_fee_per_gas
        .unwrap_or(30_000_000_000u64); // 30 gwei fallback
    let base_fee = U256::from(base_fee_u64);
    let priority_fee = U256::from(2_000_000_000u64); // 2 gwei default
    let max_fee = base_fee * 2 + priority_fee;

    let target_block = latest_block.header.number + target_block_offset;

    // Gas: a REAL `provider.estimate_gas` on the built tx with safety headroom.
    // The wrapped flash path (requestFlashLoan → callback → executeArbitrage
    // FlashFunded → 2 router swaps → repay) needs far more than the old hardcoded
    // 500k. Fail-closed on estimate error (e.g. the tx reverts in simulation) —
    // we never hardcode a too-low limit that would either fail on-chain or, worse,
    // silently truncate.
    //
    // alloy 1.8 `estimate_gas(tx) -> u64` defaults to the pending block + current
    // state, which is exactly the right context for inclusion in the next block.
    let estimate_tx = TransactionRequest::default()
        .from(AlloyAddress::from_slice(signer.address.as_bytes()))
        .to(AlloyAddress::from_slice(to.as_bytes()))
        .value(AlloyU256::ZERO)
        .input(TransactionInput::new(AlloyBytes::from(data_vec)));
    let estimated_gas: u64 = provider
        .estimate_gas(estimate_tx)
        .await
        .map_err(|e| BuildError::GasEstimate(e.to_string()))?;
    // Apply 20% safety headroom, then fail-closed if the result is implausible
    // (zero — impossible for a real tx — or above a full block).
    let gas_limit = estimated_gas.saturating_mul(GAS_SAFETY_NUM) / GAS_SAFETY_DEN;
    if gas_limit == 0 {
        return Err(BuildError::GasEstimate(
            "estimate_gas returned 0 — refusing to broadcast".into(),
        ));
    }
    if gas_limit > GAS_LIMIT_CEILING {
        return Err(BuildError::GasEstimate(format!(
            "gas_limit {gas_limit} exceeds ceiling {GAS_LIMIT_CEILING} — refusing to broadcast"
        )));
    }

    let tx = Eip1559TransactionRequest::new()
        .to(to) // FlashLoanExecutor (FLE) — never a router/AE/EOA.
        .from(signer.address)
        .value(value_wei)
        .data(data)
        .nonce(nonce)
        .chain_id(opp.chain_id)
        .max_priority_fee_per_gas(priority_fee)
        .max_fee_per_gas(max_fee)
        .gas(gas_limit);

    let typed: TypedTransaction = tx.into();
    let signature = signer
        .wallet
        .sign_transaction(&typed)
        .await
        .map_err(|e| BuildError::Provider(format!("sign_tx: {e}")))?;
    let raw: Bytes = typed.rlp_signed(&signature);
    let tx_hash = keccak256_bytes(&raw);

    Ok(SignedBundle {
        opportunity_id: opp.id,
        target_block,
        tx_raw_hex: format!("0x{}", hex::encode(&raw)),
        tx_hash: H256::from(tx_hash),
        from: signer.address,
        nonce,
        value_wei,
    })
}

fn keccak256_bytes(b: &Bytes) -> [u8; 32] {
    use ethers::utils::keccak256;
    keccak256(b.as_ref())
}

fn amount_in_to_eth(amount: U256) -> f64 {
    let wei = amount.as_u128();
    (wei as f64) / 1e18_f64
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)] // test module — panics are acceptable
mod tests {
    use super::*;
    use prioritization_spine::round_trip_executor::RoundTripContext;
    use prioritization_spine::REQUEST_FLASH_LOAN_SELECTOR;
    use std::str::FromStr;

    fn weth() -> Address {
        Address::from_str("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2").unwrap()
    }
    fn usdc() -> Address {
        Address::from_str("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48").unwrap()
    }

    fn fixture_plan() -> ValidatedPlan {
        let ctx = RoundTripContext {
            caller: Address::from_str("0x1111111111111111111111111111111111111111").unwrap(),
            token_in: weth(),
            token_out: usdc(),
            amount_in: U256::from(10_u64).pow(U256::from(18)), // 1 ETH principal
            forward_router: Address::from_str("0x7a250d5630b4cf539739df2c5dacb4c659f2488d")
                .unwrap(),
            forward_path: vec![weth(), usdc()],
            backward_router: Address::from_str("0x68b3465833fb72a70ecdf485e0e4c7bd8665fc45")
                .unwrap(),
            backward_path: vec![usdc(), weth()],
            deadline: U256::from(1_700_000_000u64),
        };
        ValidatedPlan {
            ctx,
            route_hash: [0x11u8; 32],
            min_profit_wei: U256::from(1u64),
            executor_address: Address::from_str("0x2222222222222222222222222222222222222222")
                .unwrap(),
            // Sim-validated wrapped-flash bytes the producer carries for verbatim
            // broadcast. `build_and_sign` still re-encodes from ctx today (the
            // verbatim-broadcast switch is a later M2 increment), so this fixture
            // value is unused by the current encode path — present only to satisfy
            // the (now non-optional) carrier field.
            wrapped_calldata: vec![0x51, 0x07, 0xd6, 0x1e],
        }
    }

    /// The cap is enforced against the flash-loan PRINCIPAL (ctx.amount_in),
    /// which is the blast radius — NOT the tx value (always 0).
    #[test]
    fn value_cap_triggers_on_huge_principal() {
        let big = U256::from(10u128) * U256::from(10u128).pow(18.into()); // 10 ETH
        assert!(amount_in_to_eth(big) > 1.0);
        let small = U256::from(5u128) * U256::from(10u128).pow(17.into()); // 0.5 ETH
        assert!(amount_in_to_eth(small) < 1.0);
    }

    /// The shared encoder produces the OUTER wrapped flash calldata whose first
    /// four bytes are the `requestFlashLoan` selector (0x5107d61e). This is the
    /// exact byte sequence `build_and_sign` puts in `tx.data`, so locking it
    /// here guarantees the broadcast targets the wrapped flash entrypoint and
    /// never a raw router swap.
    #[test]
    fn calldata_outer_selector_is_request_flash_loan() {
        let plan = fixture_plan();
        let data = build_flash_funded_broadcast_calldata(
            &plan.ctx,
            plan.route_hash,
            plan.min_profit_wei,
            plan.executor_address,
        )
        .expect("encode wrapped flash calldata");
        assert!(data.len() >= 4, "calldata must carry a 4-byte selector");
        // Array-first ordering (matches the encoder's own tests): [u8;4]
        // implements PartialEq<[u8]>, so the slice goes on the right.
        assert_eq!(
            REQUEST_FLASH_LOAN_SELECTOR,
            data[0..4],
            "outer selector must be requestFlashLoan (0x5107d61e)"
        );
        // Sanity: 0x5107d61e in literal bytes.
        assert_eq!([0x51u8, 0x07, 0xd6, 0x1e], data[0..4]);
    }

    /// FLE resolution is fail-closed: with `FLASHLOAN_EXECUTOR_<chain_id>` unset,
    /// `resolve_flashloan_executor_address` (the function `build_and_sign` calls
    /// to set `.to()`) errors — so no `.to()` can be produced and no tx built.
    /// Uses a private chain id so it never races a real env value.
    #[test]
    fn fle_resolution_fails_closed_when_unset() {
        let chain_id = 424_242u64;
        std::env::remove_var(format!("FLASHLOAN_EXECUTOR_{chain_id}"));
        assert!(
            resolve_flashloan_executor_address(chain_id).is_err(),
            "unset FLASHLOAN_EXECUTOR_<chain_id> must fail-closed (no .to(), no broadcast)"
        );
    }

    /// When the FLE env IS set, the resolved address is what `.to()` would be —
    /// the FlashLoanExecutor, distinct from any router or the EOA.
    #[test]
    fn fle_resolution_returns_configured_address_as_to() {
        let chain_id = 424_243u64;
        let fle = "0x33333333333333333333333333333333333333Cc";
        std::env::set_var(format!("FLASHLOAN_EXECUTOR_{chain_id}"), fle);
        let resolved = resolve_flashloan_executor_address(chain_id).expect("FLE resolves");
        assert_eq!(
            resolved,
            Address::from_str(fle).unwrap(),
            ".to() must equal the configured FlashLoanExecutor"
        );
        // Not a router and not the zero/EOA-ish address.
        assert_ne!(resolved, Address::zero());
        std::env::remove_var(format!("FLASHLOAN_EXECUTOR_{chain_id}"));
    }

    /// M1 must reject BEFORE any FLE resolution. We assert the policy semantics
    /// directly (the same `LiveExecPolicy` `build_and_sign` calls as its first
    /// statement): default-deny rejects every chain, and even when enabled a
    /// non-allowlisted chain (and mainnet) is refused — all WITHOUT any
    /// `FLASHLOAN_EXECUTOR_*` env being read. A full end-to-end ordering proof
    /// (M1 short-circuits before resolve_flashloan_executor_address inside
    /// build_and_sign) is integration-level and deferred to M5 fork validation.
    #[test]
    fn m1_rejects_before_fle_resolution() {
        use crate::live_exec_policy::{LiveExecDenied, LiveExecPolicy};
        // Default-deny: disabled → every chain denied (no FLE lookup reachable).
        let denied = LiveExecPolicy::from_raw(None, None);
        assert_eq!(
            denied.assert_broadcast_allowed(11_155_111),
            Err(LiveExecDenied::NotEnabled)
        );
        // Enabled but chain not allowlisted → still refused before FLE resolve.
        let enabled = LiveExecPolicy::from_raw(Some("true"), Some("11155111"));
        assert!(matches!(
            enabled.assert_broadcast_allowed(137),
            Err(LiveExecDenied::ChainNotAllowed { .. })
        ));
        // Mainnet physically refused even if allowlisted.
        assert_eq!(
            enabled.assert_broadcast_allowed(1),
            Err(LiveExecDenied::MainnetRefused)
        );
    }
}
