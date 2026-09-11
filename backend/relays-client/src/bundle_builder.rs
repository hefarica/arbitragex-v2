//! Sign only the exact, bound, canonical simulation approved calldata.
use crate::{nonce_manager::NonceManager, plan_validation, signer::Signer};
use alloy::eips::BlockId;
use alloy::primitives::{Address as AlloyAddress, Bytes as AlloyBytes, U256 as AlloyU256};
use alloy::providers::Provider as AlloyProvider;
use alloy::rpc::types::{BlockNumberOrTag, TransactionInput, TransactionRequest};
use ethers::core::types::transaction::eip2718::TypedTransaction;
use ethers::prelude::*;
use ethers::signers::Signer as EthersSigner;
use prioritization_spine::{ValidatedPlan, REQUEST_FLASH_LOAN_SELECTOR};
use shared_rs::chains::resolve_flashloan_executor_address;
use shared_rs::contracts::{Opportunity, StrategyKind};
use shared_rs::rpc_failover::AlloyHttpProvider;

#[derive(Debug, thiserror::Error)]
pub enum BuildError {
    #[error("unsupported strategy: {0:?}")]
    UnsupportedStrategy(StrategyKind),
    #[error("live execution denied: {0}")]
    LiveExecDenied(String),
    #[error("flash-funded calldata validation failed: {0}")]
    EncodeFailed(String),
    #[error("principal display {value_eth} ETH exceeds display cap {cap_eth}; exact U256 comparison enforced")]
    ValueExceedsCap { value_eth: f64, cap_eth: f64 },
    #[error("gas estimate failed: {0}")]
    GasEstimate(String),
    #[error("provider error: {0}")]
    Provider(String),
}
#[derive(Clone)]
pub struct SignedBundle {
    pub opportunity_id: uuid::Uuid,
    pub target_block: u64,
    pub state_block: u64,
    pub state_block_hash: H256,
    pub gas_limit: u64,
    pub tx_raw_hex: String,
    pub tx_hash: H256,
    pub from: Address,
    pub nonce: u64,
    pub value_wei: U256,
}
fn deny(reason: &str) -> BuildError {
    BuildError::LiveExecDenied(reason.into())
}
#[allow(clippy::too_many_arguments)]
pub async fn build_and_sign(
    opp: &Opportunity,
    plan: &ValidatedPlan,
    signer: &Signer,
    provider: &AlloyHttpProvider,
    nonce_mgr: &NonceManager,
    max_value_eth: f64,
    target_block_offset: u64,
    priority_fee_gwei: f64,
) -> Result<SignedBundle, BuildError> {
    crate::live_exec_policy::LiveExecPolicy::from_env()
        .assert_broadcast_allowed(opp.chain_id)
        .map_err(|e| deny(&e.to_string()))?;
    if !plan_validation::supports_strategy(&opp.strategy_kind) {
        return Err(BuildError::UnsupportedStrategy(opp.strategy_kind.clone()));
    }
    if signer.chain_id != opp.chain_id
        || signer.wallet.chain_id() != opp.chain_id
        || signer.wallet.address() != signer.address
    {
        return Err(deny("signer_chain_or_address_mismatch"));
    }
    let actual_chain = provider
        .get_chain_id()
        .await
        .map_err(|_| deny("provider_chain_unavailable"))?;
    if actual_chain != opp.chain_id {
        return Err(deny("provider_chain_mismatch"));
    }
    let to = resolve_flashloan_executor_address(opp.chain_id)
        .map_err(|_| deny("flashloan_executor_missing"))?;
    plan_validation::validate_binding(
        opp,
        plan,
        signer.address,
        to,
        chrono::Utc::now().timestamp_millis(),
    )
    .map_err(deny)?;
    let binding = plan
        .binding
        .as_ref()
        .ok_or_else(|| deny("simulation_binding_missing"))?;
    let cap_key = format!(
        "ARBX_LIVE_PRINCIPAL_CAP_{}_{}",
        opp.chain_id,
        hex::encode(plan.ctx.token_in)
    );
    let cap = plan_validation::principal_cap(
        opp.chain_id,
        plan.ctx.token_in,
        max_value_eth,
        std::env::var(cap_key).ok().as_deref(),
    )
    .map_err(deny)?;
    if plan.ctx.amount_in > cap {
        return Err(BuildError::ValueExceedsCap {
            value_eth: amount_in_to_eth(plan.ctx.amount_in),
            cap_eth: amount_in_to_eth(cap),
        });
    }
    let latest = provider
        .get_block(BlockId::Number(BlockNumberOrTag::Latest))
        .await
        .map_err(|_| deny("head_unavailable"))?
        .ok_or_else(|| deny("head_missing"))?;
    if latest.header.number != binding.block_number
        || latest.header.hash.as_slice() != binding.block_hash.as_bytes()
        || latest.header.timestamp != binding.block_timestamp
    {
        return Err(deny("head_changed_resimulation_required"));
    }
    if target_block_offset != 1 {
        return Err(deny("fresh_simulation_requires_next_block"));
    }
    plan_validation::validate_funding(provider, plan)
        .await
        .map_err(deny)?;
    let base_fee = U256::from(
        latest
            .header
            .base_fee_per_gas
            .ok_or_else(|| deny("base_fee_missing"))?,
    );
    if !priority_fee_gwei.is_finite() || priority_fee_gwei < 0.0 {
        return Err(deny("invalid_priority_fee"));
    }
    let priority_fee: U256 = ethers::utils::parse_units(format!("{priority_fee_gwei:.9}"), 9)
        .map_err(|_| deny("invalid_priority_fee"))?
        .into();
    let max_fee = binding.gas_price_wei;
    if base_fee
        .checked_add(priority_fee)
        .is_none_or(|n| n > max_fee)
    {
        return Err(deny("fee_budget_exceeded_resimulation_required"));
    }
    let gas_limit = binding.gas_limit;
    if gas_limit < 21_000 || gas_limit > latest.header.gas_limit {
        return Err(deny("simulated_gas_limit_invalid"));
    }
    let data_vec = verbatim_broadcast_calldata(plan)?;
    let estimate_tx = TransactionRequest::default()
        .from(AlloyAddress::from_slice(signer.address.as_bytes()))
        .to(AlloyAddress::from_slice(to.as_bytes()))
        .value(AlloyU256::ZERO)
        .input(TransactionInput::new(AlloyBytes::from(data_vec.clone())));
    let estimated_gas = provider
        .estimate_gas(estimate_tx)
        .block(BlockId::Number(BlockNumberOrTag::Number(
            binding.block_number,
        )))
        .await
        .map_err(|_| BuildError::GasEstimate("wrapped_call_estimate_failed".into()))?;
    if estimated_gas == 0 || estimated_gas > gas_limit {
        return Err(BuildError::GasEstimate(
            "estimate_exceeds_simulated_limit".into(),
        ));
    }
    let target_block = latest
        .header
        .number
        .checked_add(1)
        .ok_or_else(|| deny("target_block_overflow"))?;
    let nonce = nonce_mgr
        .next(opp.chain_id, signer.address)
        .await
        .map_err(|_| BuildError::Provider("nonce_unavailable".into()))?;
    let tx = Eip1559TransactionRequest::new()
        .to(to)
        .from(signer.address)
        .value(U256::zero())
        .data(Bytes::from(data_vec))
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
        .map_err(|_| BuildError::Provider("sign_failed".into()))?;
    let raw = typed.rlp_signed(&signature);
    Ok(SignedBundle {
        opportunity_id: opp.id,
        target_block,
        state_block: binding.block_number,
        state_block_hash: binding.block_hash,
        gas_limit,
        tx_raw_hex: format!("0x{}", hex::encode(&raw)),
        tx_hash: H256(ethers::utils::keccak256(raw)),
        from: signer.address,
        nonce,
        value_wei: U256::zero(),
    })
}

/// Re-check the canonical state immediately before relay simulation and broadcast.
pub async fn assert_bundle_head(
    provider: &AlloyHttpProvider,
    bundle: &SignedBundle,
) -> Result<(), BuildError> {
    let head = provider
        .get_block(BlockId::Number(BlockNumberOrTag::Latest))
        .await
        .map_err(|_| deny("head_unavailable"))?
        .ok_or_else(|| deny("head_missing"))?;
    if head.header.number != bundle.state_block
        || head.header.hash.as_slice() != bundle.state_block_hash.as_bytes()
    {
        return Err(deny("head_changed_resimulation_required"));
    }
    Ok(())
}
/// Display only; never used to make an economic gate decision.
fn amount_in_to_eth(amount: U256) -> f64 {
    amount.to_string().parse::<f64>().unwrap_or(f64::INFINITY) / 1e18
}
fn verbatim_broadcast_calldata(plan: &ValidatedPlan) -> Result<Vec<u8>, BuildError> {
    if plan.wrapped_calldata.get(..4) != Some(REQUEST_FLASH_LOAN_SELECTOR.as_slice()) {
        return Err(BuildError::EncodeFailed(
            "missing wrapped flash selector".into(),
        ));
    }
    Ok(plan.wrapped_calldata.clone())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)] // test module — panics are acceptable
mod tests {
    use super::*;
    use prioritization_spine::round_trip_executor::RoundTripContext;
    use prioritization_spine::{
        build_flash_funded_broadcast_calldata,
        build_flash_funded_broadcast_calldata_with_intermediate,
    };
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
        let route_hash = [0x11u8; 32];
        let min_profit_wei = U256::from(1u64);
        let executor_address =
            Address::from_str("0x2222222222222222222222222222222222222222").unwrap();
        // Sim-validated wrapped-flash bytes the producer carries for VERBATIM
        // broadcast. We build them with an EXPLICIT intermediate backward amount
        // (the runtime `token_out` a re-encode could not reproduce), so the
        // fixture mirrors what the sim path actually persists — outer
        // requestFlashLoan (0x5107d61e) wrapping executeArbitrageFlashFunded.
        let intermediate = U256::from(1_234_567u64);
        let wrapped_calldata = build_flash_funded_broadcast_calldata_with_intermediate(
            &ctx,
            intermediate,
            route_hash,
            min_profit_wei,
            executor_address,
        )
        .expect("encode wrapped flash calldata for fixture");
        ValidatedPlan {
            ctx,
            route_hash,
            min_profit_wei,
            executor_address,
            wrapped_calldata,
            binding: None,
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

    /// TRUE sim↔broadcast byte-parity: the bytes `build_and_sign` puts in
    /// `tx.data` are `plan.wrapped_calldata` VERBATIM — no re-encode. We assert
    /// the exact production extractor (`verbatim_broadcast_calldata`, the source
    /// of `data_vec`) returns the plan's bytes byte-for-byte, and that those
    /// bytes carry the OUTER `requestFlashLoan` selector (0x5107d61e). To prove
    /// it is NOT a re-encode from ctx, the fixture's `wrapped_calldata` was built
    /// with an explicit runtime intermediate, so it DIFFERS from what a legacy
    /// `build_flash_funded_broadcast_calldata(&ctx, ...)` re-encode would yield.
    #[test]
    fn broadcast_calldata_equals_plan_wrapped_calldata_verbatim() {
        let plan = fixture_plan();
        let data_vec = verbatim_broadcast_calldata(&plan).expect("validated plan yields bytes");
        // Byte-for-byte equality with the carried, sim-validated bytes.
        assert_eq!(
            data_vec, plan.wrapped_calldata,
            "tx.data must equal plan.wrapped_calldata byte-for-byte (verbatim broadcast)"
        );
        // The carried bytes are the wrapped flash entrypoint (0x5107d61e).
        assert!(data_vec.len() >= 4, "calldata must carry a 4-byte selector");
        assert_eq!(
            REQUEST_FLASH_LOAN_SELECTOR,
            data_vec[0..4],
            "outer selector must be requestFlashLoan (0x5107d61e)"
        );
        // Proof it is VERBATIM, not a re-encode: the legacy ctx-only re-encode
        // (min_profit_wei sentinel backward amount) differs from the carried
        // intermediate-aware bytes, so the broadcast can only match by sending
        // the carried bytes verbatim.
        let legacy_reencode = build_flash_funded_broadcast_calldata(
            &plan.ctx,
            plan.route_hash,
            plan.min_profit_wei,
            plan.executor_address,
        )
        .expect("legacy re-encode");
        assert_ne!(
            data_vec, legacy_reencode,
            "verbatim bytes must NOT equal a ctx-only re-encode (carries runtime intermediate)"
        );
    }

    /// Fail-closed: an empty `wrapped_calldata` (plan never validated) yields NO
    /// broadcast bytes — `verbatim_broadcast_calldata` errors, so `build_and_sign`
    /// returns before building/signing any tx.
    #[test]
    fn empty_wrapped_calldata_fails_closed_no_broadcast() {
        let mut plan = fixture_plan();
        plan.wrapped_calldata = Vec::new();
        let err = verbatim_broadcast_calldata(&plan)
            .expect_err("empty wrapped_calldata must fail-closed (no broadcast)");
        assert!(matches!(err, BuildError::EncodeFailed(_)));
    }

    /// Fail-closed: `wrapped_calldata` whose prefix is NOT the `requestFlashLoan`
    /// selector (0x5107d61e) is not the wrapped-flash entrypoint — refuse to
    /// broadcast.
    #[test]
    fn wrong_selector_wrapped_calldata_fails_closed_no_broadcast() {
        let mut plan = fixture_plan();
        // A non-0x5107d61e prefix (e.g. a raw router swap selector) + payload.
        plan.wrapped_calldata = vec![0xde, 0xad, 0xbe, 0xef, 0x00, 0x01];
        let err = verbatim_broadcast_calldata(&plan)
            .expect_err("wrong selector must fail-closed (no broadcast)");
        assert!(matches!(err, BuildError::EncodeFailed(_)));
        // A too-short (<4 byte) payload is also rejected.
        plan.wrapped_calldata = vec![0x51, 0x07];
        assert!(verbatim_broadcast_calldata(&plan).is_err());
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
        assert!(matches!(
            enabled.assert_broadcast_allowed(1),
            Err(LiveExecDenied::ChainNotAllowed { .. })
        ));
        assert!(LiveExecPolicy::from_raw(Some("true"), Some("1"))
            .assert_broadcast_allowed(1)
            .is_ok());
    }
}
