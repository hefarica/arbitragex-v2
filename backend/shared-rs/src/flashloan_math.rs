//! Exact flash-loan fees and capacity at one canonical EIP-1898 state.
//! AaveV3Core uses PercentageMath.percentMul (half-up); Balancer V2 uses mulUp.
//! The executor supports its direct Aave path or its configured Balancer adapter.
//! A generic ERC-3156 lender has a different callback and is not interchangeable.

use anyhow::{anyhow, ensure, Result};
use ethers::{
    abi::{encode, Token},
    providers::{Http, Provider},
    types::{Address, Bytes, H256, U256},
    utils::id,
};
use serde::{Deserialize, Serialize};
use serde_json::json;

pub fn aave_premium(principal: U256, premium_bps: U256) -> Result<U256> {
    ensure!(
        premium_bps <= U256::from(10_000),
        "flashloan_invalid_aave_premium"
    );
    if principal.is_zero() || premium_bps.is_zero() {
        return Ok(U256::zero());
    }
    Ok(principal
        .checked_mul(premium_bps)
        .and_then(|v| v.checked_add(U256::from(5_000)))
        .ok_or_else(|| anyhow!("flashloan_fee_overflow"))?
        / U256::from(10_000))
}

pub fn balancer_premium(principal: U256, percentage_1e18: U256) -> Result<U256> {
    let wad = U256::exp10(18);
    ensure!(percentage_1e18 <= wad, "flashloan_invalid_balancer_premium");
    let product = principal
        .checked_mul(percentage_1e18)
        .ok_or_else(|| anyhow!("flashloan_fee_overflow"))?;
    if product.is_zero() {
        return Ok(U256::zero());
    }
    Ok((product - U256::one()) / wad + U256::one())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutorFundingKind {
    AaveDirect,
    BalancerAdapter,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutorFlashQuote {
    pub kind: ExecutorFundingKind,
    /// Pool (Aave) or Vault (Balancer), not the adapter address.
    pub provider_address: Address,
    pub adapter_address: Option<Address>,
    pub asset: Address,
    pub decimals: u8,
    pub principal_wei: U256,
    pub capacity_wei: U256,
    pub fee_wei: U256,
    pub block_hash: H256,
}

pub async fn call_bytes(
    provider: &Provider<Http>,
    target: Address,
    signature: &str,
    args: &[Token],
    block_hash: H256,
) -> Result<Vec<u8>> {
    ensure!(
        !target.is_zero() && !block_hash.is_zero(),
        "flashloan_invalid_state_identity"
    );
    let mut data = id(signature)[..4].to_vec();
    data.extend(encode(args));
    let value: Bytes = provider.request("eth_call", json!([
        {"to":target,"data":Bytes::from(data)}, {"blockHash":block_hash,"requireCanonical":true}
    ])).await.map_err(|_| anyhow!("flashloan_rpc_call_failed:{signature}"))?;
    Ok(value.to_vec())
}

pub async fn call_word(
    provider: &Provider<Http>,
    target: Address,
    signature: &str,
    args: &[Token],
    block_hash: H256,
) -> Result<U256> {
    let bytes = call_bytes(provider, target, signature, args, block_hash).await?;
    ensure!(bytes.len() == 32, "flashloan_invalid_abi_word:{signature}");
    Ok(U256::from_big_endian(&bytes))
}

fn word_address(value: U256) -> Result<Address> {
    ensure!(
        value >> 160 == U256::zero(),
        "flashloan_invalid_abi_address"
    );
    let mut bytes = [0u8; 32];
    value.to_big_endian(&mut bytes);
    Ok(Address::from_slice(&bytes[12..]))
}

pub async fn call_address(
    provider: &Provider<Http>,
    target: Address,
    signature: &str,
    args: &[Token],
    block_hash: H256,
) -> Result<Address> {
    word_address(call_word(provider, target, signature, args, block_hash).await?)
}

/// Read real FLE configuration, token units, availability and provider premium.
/// No admin state is changed. Unsupported provider adapters are rejected.
/// Aave reading follows the explicitly supported V3 Core reserve ABI/math;
/// complete EVM simulation must still validate deployment-specific behavior.
pub async fn resolve_executor_flash_quote(
    provider: &Provider<Http>,
    fle: Address,
    asset: Address,
    principal: U256,
    block_hash: H256,
) -> Result<ExecutorFlashQuote> {
    ensure!(
        !asset.is_zero() && !principal.is_zero(),
        "flashloan_invalid_principal"
    );
    let decimals = call_word(provider, asset, "decimals()", &[], block_hash).await?;
    ensure!(decimals <= U256::from(77), "flashloan_invalid_decimals");
    let adapter = call_address(provider, fle, "flashLoanProvider()", &[], block_hash).await?;
    let (kind, provider_address, adapter_address, capacity_wei, fee_wei) = if adapter.is_zero() {
        let pool = call_address(provider, fle, "aavePool()", &[], block_hash).await?;
        ensure!(!pool.is_zero(), "flashloan_no_aave_pool");
        let reserve = call_bytes(
            provider,
            pool,
            "getReserveData(address)",
            &[Token::Address(asset)],
            block_hash,
        )
        .await?;
        ensure!(
            reserve.len() >= 15 * 32 && reserve.len() % 32 == 0,
            "flashloan_unsupported_aave_reserve_abi"
        );
        let configuration = U256::from_big_endian(&reserve[..32]);
        ensure!(
            configuration.bit(56) && !configuration.bit(60) && configuration.bit(63),
            "flashloan_aave_reserve_disabled"
        );
        let a_token = word_address(U256::from_big_endian(&reserve[8 * 32..9 * 32]))?;
        ensure!(!a_token.is_zero(), "flashloan_missing_a_token");
        let capacity = call_word(
            provider,
            asset,
            "balanceOf(address)",
            &[Token::Address(a_token)],
            block_hash,
        )
        .await?;
        let premium =
            call_word(provider, pool, "FLASHLOAN_PREMIUM_TOTAL()", &[], block_hash).await?;
        (
            ExecutorFundingKind::AaveDirect,
            pool,
            None,
            capacity,
            aave_premium(principal, premium)?,
        )
    } else {
        // The local BalancerFlashAdapter forwards callbacks to FLE. A generic
        // ERC-3156 address cannot satisfy this ABI/callback contract.
        let vault = call_address(provider, adapter, "vault()", &[], block_hash).await?;
        let authorized_vault =
            call_address(provider, fle, "balancerVault()", &[], block_hash).await?;
        ensure!(
            !vault.is_zero() && vault == authorized_vault,
            "flashloan_incompatible_balancer_adapter"
        );
        let collector = call_address(
            provider,
            vault,
            "getProtocolFeesCollector()",
            &[],
            block_hash,
        )
        .await?;
        let percentage = call_word(
            provider,
            collector,
            "getFlashLoanFeePercentage()",
            &[],
            block_hash,
        )
        .await?;
        let capacity = call_word(
            provider,
            asset,
            "balanceOf(address)",
            &[Token::Address(vault)],
            block_hash,
        )
        .await?;
        (
            ExecutorFundingKind::BalancerAdapter,
            vault,
            Some(adapter),
            capacity,
            balancer_premium(principal, percentage)?,
        )
    };
    ensure!(capacity_wei >= principal, "flashloan_insufficient_capacity");
    principal
        .checked_add(fee_wei)
        .ok_or_else(|| anyhow!("flashloan_repayment_overflow"))?;
    Ok(ExecutorFlashQuote {
        kind,
        provider_address,
        adapter_address,
        asset,
        decimals: decimals.as_u32() as u8,
        principal_wei: principal,
        capacity_wei,
        fee_wei,
        block_hash,
    })
}

pub async fn resolve_executor_flash_fee(
    provider: &Provider<Http>,
    fle: Address,
    asset: Address,
    principal: U256,
    block_hash: H256,
) -> Result<U256> {
    Ok(
        resolve_executor_flash_quote(provider, fle, asset, principal, block_hash)
            .await?
            .fee_wei,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn protocol_rounding_differs_at_one_wei() {
        assert_eq!(
            aave_premium(U256::from(999), U256::from(5)).unwrap(),
            U256::zero()
        );
        assert_eq!(
            aave_premium(U256::from(1000), U256::from(5)).unwrap(),
            U256::one()
        );
        assert_eq!(
            balancer_premium(U256::one(), U256::one()).unwrap(),
            U256::one()
        );
        assert_eq!(
            balancer_premium(U256::MAX, U256::zero()).unwrap(),
            U256::zero()
        );
    }
    #[test]
    fn full_width_principal_and_overflow_are_not_truncated() {
        let principal = U256::one() << 200;
        assert_eq!(
            aave_premium(principal, U256::from(5)).unwrap(),
            (principal * 5 + 5000) / 10000
        );
        assert!(aave_premium(U256::MAX, U256::from(5)).is_err());
        assert!(balancer_premium(U256::MAX, U256::exp10(18)).is_err());
    }
}
