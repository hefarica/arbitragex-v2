//! Aave V3 core liquidation arithmetic in native token integers.
//!
//! This profile models the reviewed Aave v3-core HF/0.95 close-factor rules.
//! Selecting it requires a matching, configured implementation code hash.
//! Other revisions and eMode are not inferred from a deployment address.

use anyhow::{bail, ensure, Context, Result};
use ethers::abi::{encode, Token};
use ethers::types::{Address, Bytes, H256, U256, U512};
use ethers::utils::keccak256;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const MAX_PAIR_EVALUATIONS: usize = 4096;
pub const MAX_RESERVES: usize = 128;
pub const HF_ONE: u64 = 1_000_000_000_000_000_000;
pub const HF_FULL_CLOSE: u64 = 950_000_000_000_000_000;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LiquidationRevision {
    AaveV3CoreHf095,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReservePosition {
    pub asset: Address,
    pub decimals: u8,
    pub price_base: U256,
    pub collateral_balance: U256,
    pub stable_debt: U256,
    pub variable_debt: U256,
    pub usage_as_collateral: bool,
    pub liquidation_threshold_bps: u16,
    /// 10500 means collateral worth 105% of repaid debt, not a 105% bonus.
    pub liquidation_bonus_bps: u16,
    /// Fraction of the bonus collateral retained by the protocol.
    pub liquidation_protocol_fee_bps: u16,
    pub active: bool,
    pub paused: bool,
}
impl ReservePosition {
    pub fn debt(&self) -> Result<U256> {
        self.stable_debt
            .checked_add(self.variable_debt)
            .context("liquidation_debt_overflow")
    }
    pub fn is_collateral(&self) -> bool {
        self.usage_as_collateral
            && self.liquidation_threshold_bps > 0
            && !self.collateral_balance.is_zero()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct LiquidationSnapshot {
    pub schema_version: u8,
    pub chain_id: u64,
    pub protocol: String,
    pub pool: Address,
    pub oracle: Address,
    pub implementation: Address,
    pub implementation_code_hash: H256,
    pub revision: LiquidationRevision,
    pub borrower: Address,
    pub block_number: u64,
    pub block_hash: H256,
    pub block_timestamp: u64,
    pub observed_at_unix_ms: u64,
    pub health_factor_1e18: U256,
    pub total_collateral_base: U256,
    pub total_debt_base: U256,
    pub base_currency: Address,
    pub base_currency_unit: U256,
    pub user_emode: u8,
    /// All reserves with a nonzero user balance/debt, not just the first pair.
    pub reserves: Vec<ReservePosition>,
    pub reserve_count: usize,
    pub reserves_complete: bool,
}

impl LiquidationSnapshot {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.schema_version == 1 && self.protocol == "aave_v3",
            "liquidation_schema_invalid"
        );
        ensure!(
            self.chain_id > 0 && self.pool != Address::zero() && self.oracle != Address::zero(),
            "liquidation_identity_invalid"
        );
        ensure!(
            self.borrower != Address::zero()
                && self.implementation != Address::zero()
                && self.implementation_code_hash != H256::zero(),
            "liquidation_implementation_unverified"
        );
        ensure!(
            self.block_number > 0 && self.block_hash != H256::zero() && self.block_timestamp > 0,
            "liquidation_snapshot_unpinned"
        );
        ensure!(
            self.reserves_complete
                && self.reserve_count <= MAX_RESERVES
                && self.reserves.len() <= self.reserve_count,
            "liquidation_reserves_incomplete"
        );
        ensure!(
            !self.base_currency_unit.is_zero(),
            "liquidation_oracle_unit_invalid"
        );
        ensure!(
            self.user_emode == 0,
            "liquidation_emode_profile_unsupported"
        );
        let mut assets = std::collections::HashSet::new();
        for reserve in &self.reserves {
            ensure!(
                reserve.asset != Address::zero() && assets.insert(reserve.asset),
                "liquidation_reserve_identity_invalid"
            );
            ensure!(
                reserve.decimals <= 77 && !reserve.price_base.is_zero(),
                "liquidation_reserve_price_invalid"
            );
            ensure!(
                reserve.liquidation_threshold_bps <= 10000
                    && reserve.liquidation_protocol_fee_bps <= 10000,
                "liquidation_configuration_invalid"
            );
            if reserve.is_collateral() {
                ensure!(
                    reserve.liquidation_bonus_bps >= 10000,
                    "liquidation_bonus_invalid"
                );
            }
            reserve.debt()?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct LiquidationTerms {
    pub debt_asset: Address,
    pub collateral_asset: Address,
    pub debt_decimals: u8,
    pub collateral_decimals: u8,
    pub debt_to_cover_wei: U256,
    pub collateral_to_liquidator_wei: U256,
    pub protocol_fee_collateral_wei: U256,
    pub close_factor_bps: u16,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PairCoverage {
    pub total_pairs: usize,
    pub evaluated_pairs: usize,
    pub skipped_pairs: usize,
    pub limited: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LiquidationBatch {
    pub terms: Vec<LiquidationTerms>,
    pub coverage: PairCoverage,
}

fn narrow(value: U512) -> Result<U256> {
    ensure!(
        value <= U512::from(U256::MAX),
        "liquidation_arithmetic_overflow"
    );
    let mut bytes = [0u8; 64];
    value.to_big_endian(&mut bytes);
    Ok(U256::from_big_endian(&bytes[32..]))
}
/// Solidity PercentageMath uses round-half-up, including at raw-token granularity.
fn percent_mul(value: U256, bps: u16) -> Result<U256> {
    let product = value
        .checked_mul(U256::from(bps))
        .context("liquidation_arithmetic_overflow")?;
    let rounded = product
        .checked_add(U256::from(5000))
        .context("liquidation_arithmetic_overflow")?;
    narrow(U512::from(rounded) / U512::from(10000))
}
fn percent_div(value: U256, bps: u16) -> Result<U256> {
    ensure!(bps > 0, "liquidation_percentage_zero");
    let product = value
        .checked_mul(U256::from(10000))
        .context("liquidation_arithmetic_overflow")?;
    let rounded = product
        .checked_add(U256::from(bps / 2))
        .context("liquidation_arithmetic_overflow")?;
    narrow(U512::from(rounded) / U512::from(bps))
}
fn product_ratio(a: U256, b: U256, c: U256, d: U256, e: U256) -> Result<U256> {
    let numerator = a
        .checked_mul(b)
        .and_then(|v| v.checked_mul(c))
        .context("liquidation_arithmetic_overflow")?;
    let denominator = d
        .checked_mul(e)
        .filter(|v| !v.is_zero())
        .context("liquidation_denominator_invalid")?;
    narrow(U512::from(numerator) / U512::from(denominator))
}

pub fn close_factor_bps(snapshot: &LiquidationSnapshot) -> Result<u16> {
    snapshot.validate()?;
    ensure!(
        snapshot.health_factor_1e18 < U256::from(HF_ONE),
        "liquidation_health_factor_healthy"
    );
    Ok(match snapshot.revision {
        LiquidationRevision::AaveV3CoreHf095 => {
            if snapshot.health_factor_1e18 > U256::from(HF_FULL_CLOSE) {
                5000
            } else {
                10000
            }
        }
    })
}

pub fn liquidation_terms(
    snapshot: &LiquidationSnapshot,
    debt: &ReservePosition,
    collateral: &ReservePosition,
    cap: U256,
) -> Result<LiquidationTerms> {
    let close = close_factor_bps(snapshot)?;
    ensure!(
        snapshot.reserves.iter().any(|r| r == debt)
            && snapshot.reserves.iter().any(|r| r == collateral),
        "liquidation_reserve_not_in_snapshot"
    );
    ensure!(
        debt.active && collateral.active && !debt.paused && !collateral.paused,
        "liquidation_reserve_unavailable"
    );
    ensure!(
        collateral.is_collateral() && !debt.debt()?.is_zero() && !cap.is_zero(),
        "liquidation_pair_ineligible"
    );
    let debt_to_cover = percent_mul(debt.debt()?, close)?.min(cap);
    let collateral_unit = U256::exp10(collateral.decimals as usize);
    let debt_unit = U256::exp10(debt.decimals as usize);
    let base_collateral = product_ratio(
        debt.price_base,
        debt_to_cover,
        collateral_unit,
        collateral.price_base,
        debt_unit,
    )?;
    let max_collateral = percent_mul(base_collateral, collateral.liquidation_bonus_bps)?;
    let (seized, repaid) = if max_collateral > collateral.collateral_balance {
        let needed = product_ratio(
            collateral.price_base,
            collateral.collateral_balance,
            debt_unit,
            debt.price_base,
            collateral_unit,
        )?;
        (
            collateral.collateral_balance,
            percent_div(needed, collateral.liquidation_bonus_bps)?,
        )
    } else {
        (max_collateral, debt_to_cover)
    };
    ensure!(
        !seized.is_zero() && !repaid.is_zero() && repaid <= debt_to_cover,
        "liquidation_amount_rounds_to_zero"
    );
    let bonus_collateral = seized
        .checked_sub(percent_div(seized, collateral.liquidation_bonus_bps)?)
        .context("liquidation_bonus_underflow")?;
    let protocol_fee = percent_mul(bonus_collateral, collateral.liquidation_protocol_fee_bps)?;
    let received = seized
        .checked_sub(protocol_fee)
        .context("liquidation_fee_underflow")?;
    ensure!(!received.is_zero(), "liquidation_empty_collateral");
    Ok(LiquidationTerms {
        debt_asset: debt.asset,
        collateral_asset: collateral.asset,
        debt_decimals: debt.decimals,
        collateral_decimals: collateral.decimals,
        debt_to_cover_wei: repaid,
        collateral_to_liquidator_wei: received,
        protocol_fee_collateral_wei: protocol_fee,
        close_factor_bps: close,
    })
}

/// Evaluate every debt/collateral combination, bounded at 4096. Coverage is explicit.
/// A missing asset cap suppresses that pair; it never means unlimited capital.
pub fn enumerate_pairs(
    snapshot: &LiquidationSnapshot,
    caps: &std::collections::BTreeMap<Address, U256>,
) -> Result<LiquidationBatch> {
    snapshot.validate()?;
    if snapshot.health_factor_1e18 >= U256::from(HF_ONE) {
        return Ok(LiquidationBatch {
            terms: vec![],
            coverage: PairCoverage {
                total_pairs: 0,
                evaluated_pairs: 0,
                skipped_pairs: 0,
                limited: false,
            },
        });
    }
    let debts = snapshot
        .reserves
        .iter()
        .filter(|r| r.debt().map(|d| !d.is_zero()).unwrap_or(false))
        .collect::<Vec<_>>();
    let collateral = snapshot
        .reserves
        .iter()
        .filter(|r| r.is_collateral())
        .collect::<Vec<_>>();
    let total_pairs = debts
        .len()
        .checked_mul(collateral.len())
        .context("liquidation_pair_count_overflow")?;
    let mut terms = Vec::new();
    let mut evaluated = 0;
    let mut skipped = 0;
    'outer: for debt in debts {
        for coll in &collateral {
            if evaluated == MAX_PAIR_EVALUATIONS {
                break 'outer;
            }
            evaluated += 1;
            match caps
                .get(&debt.asset)
                .copied()
                .map(|cap| liquidation_terms(snapshot, debt, coll, cap))
            {
                Some(Ok(term)) => terms.push(term),
                _ => skipped += 1,
            }
        }
    }
    Ok(LiquidationBatch {
        terms,
        coverage: PairCoverage {
            total_pairs,
            evaluated_pairs: evaluated,
            skipped_pairs: skipped,
            limited: evaluated < total_pairs,
        },
    })
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct TokenApproval {
    pub asset: Address,
    pub spender: Address,
    pub amount_wei: U256,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SimulationCall {
    pub target: Address,
    pub calldata: Bytes,
    pub value_wei: U256,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct LiquidationUnwind {
    pub router: Address,
    pub path: Vec<Address>,
    pub amount_in_wei: U256,
    pub quoted_amount_out_wei: U256,
    pub minimum_amount_out_wei: U256,
    pub max_slippage_bps: u16,
    pub block_hash: H256,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LiquidationSimulationRequest {
    pub schema_version: u8,
    pub request_id: Uuid,
    pub strategy_id: String,
    pub chain_id: u64,
    pub source_block_number: u64,
    pub source_block_hash: H256,
    pub observed_at_unix_ms: u64,
    pub valid_through_block: u64,
    pub borrower: Address,
    pub caller: Address,
    pub pool: Address,
    pub health_factor_1e18: U256,
    pub implementation_code_hash: H256,
    pub revision: LiquidationRevision,
    pub terms: LiquidationTerms,
    pub recipient: Address,
    pub deadline: U256,
    pub unwind: Option<LiquidationUnwind>,
    pub calls: Vec<SimulationCall>,
    pub approvals: Vec<TokenApproval>,
    /// Neither Aave bonus nor an executable router quote is realized/net USD P&L.
    pub expected_profit_usd: Option<f64>,
    pub net_expected_profit_usd: Option<f64>,
    pub status: String,
}

pub fn encode_liquidation_call(
    collateral: Address,
    debt: Address,
    borrower: Address,
    amount: U256,
) -> Bytes {
    let mut calldata =
        keccak256("liquidationCall(address,address,address,uint256,bool)")[..4].to_vec();
    calldata.extend(encode(&[
        Token::Address(collateral),
        Token::Address(debt),
        Token::Address(borrower),
        Token::Uint(amount),
        Token::Bool(false),
    ]));
    calldata.into()
}

pub fn build_request(
    snapshot: &LiquidationSnapshot,
    terms: LiquidationTerms,
    caller: Address,
    recipient: Address,
    deadline: U256,
    unwind: Option<LiquidationUnwind>,
) -> Result<LiquidationSimulationRequest> {
    snapshot.validate()?;
    ensure!(
        caller != Address::zero() && recipient != Address::zero(),
        "liquidation_execution_identity_missing"
    );
    ensure!(
        deadline > U256::from(snapshot.block_timestamp),
        "liquidation_deadline_expired"
    );
    let debt = snapshot
        .reserves
        .iter()
        .find(|r| r.asset == terms.debt_asset)
        .context("liquidation_debt_missing")?;
    let coll = snapshot
        .reserves
        .iter()
        .find(|r| r.asset == terms.collateral_asset)
        .context("liquidation_collateral_missing")?;
    ensure!(
        liquidation_terms(snapshot, debt, coll, terms.debt_to_cover_wei)? == terms,
        "liquidation_terms_binding_mismatch"
    );
    let mut calls = vec![SimulationCall {
        target: snapshot.pool,
        calldata: encode_liquidation_call(
            terms.collateral_asset,
            terms.debt_asset,
            snapshot.borrower,
            terms.debt_to_cover_wei,
        ),
        value_wei: U256::zero(),
    }];
    let mut approvals = vec![TokenApproval {
        asset: terms.debt_asset,
        spender: snapshot.pool,
        amount_wei: terms.debt_to_cover_wei,
    }];
    if let Some(quote) = &unwind {
        ensure!(
            quote.block_hash == snapshot.block_hash && quote.router != Address::zero(),
            "liquidation_unwind_block_mismatch"
        );
        ensure!(
            (2..=8).contains(&quote.path.len())
                && quote.path.first() == Some(&terms.collateral_asset)
                && quote.path.last() == Some(&terms.debt_asset)
                && quote.path.iter().all(|a| *a != Address::zero()),
            "liquidation_unwind_path_invalid"
        );
        ensure!(
            quote.amount_in_wei == terms.collateral_to_liquidator_wei
                && !quote.quoted_amount_out_wei.is_zero(),
            "liquidation_unwind_amount_mismatch"
        );
        ensure!(
            quote.max_slippage_bps <= 50,
            "liquidation_slippage_exceeds_cap"
        );
        let expected_min = narrow(
            U512::from(quote.quoted_amount_out_wei) * U512::from(10000 - quote.max_slippage_bps)
                / U512::from(10000),
        )?;
        ensure!(
            quote.minimum_amount_out_wei == expected_min && !expected_min.is_zero(),
            "liquidation_unwind_minimum_mismatch"
        );
        let mut calldata =
            keccak256("swapExactTokensForTokens(uint256,uint256,address[],address,uint256)")[..4]
                .to_vec();
        calldata.extend(encode(&[
            Token::Uint(quote.amount_in_wei),
            Token::Uint(expected_min),
            Token::Array(quote.path.iter().copied().map(Token::Address).collect()),
            Token::Address(recipient),
            Token::Uint(deadline),
        ]));
        calls.push(SimulationCall {
            target: quote.router,
            calldata: calldata.into(),
            value_wei: U256::zero(),
        });
        approvals.push(TokenApproval {
            asset: terms.collateral_asset,
            spender: quote.router,
            amount_wei: terms.collateral_to_liquidator_wei,
        });
    } else if terms.collateral_asset != terms.debt_asset {
        bail!("liquidation_unwind_unavailable");
    }
    Ok(LiquidationSimulationRequest {
        schema_version: 1,
        request_id: Uuid::new_v4(),
        strategy_id: "MEV-08-012".into(),
        chain_id: snapshot.chain_id,
        source_block_number: snapshot.block_number,
        source_block_hash: snapshot.block_hash,
        observed_at_unix_ms: snapshot.observed_at_unix_ms,
        valid_through_block: snapshot.block_number.saturating_add(1),
        borrower: snapshot.borrower,
        caller,
        pool: snapshot.pool,
        health_factor_1e18: snapshot.health_factor_1e18,
        implementation_code_hash: snapshot.implementation_code_hash,
        revision: snapshot.revision,
        terms,
        recipient,
        deadline,
        unwind,
        calls,
        approvals,
        expected_profit_usd: None,
        net_expected_profit_usd: None,
        status: "pending_simulation".into(),
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    fn reserve(asset: u64, decimals: u8, price: u64) -> ReservePosition {
        ReservePosition {
            asset: Address::from_low_u64_be(asset),
            decimals,
            price_base: U256::from(price),
            collateral_balance: U256::zero(),
            stable_debt: U256::zero(),
            variable_debt: U256::zero(),
            usage_as_collateral: false,
            liquidation_threshold_bps: 8000,
            liquidation_bonus_bps: 10500,
            liquidation_protocol_fee_bps: 1000,
            active: true,
            paused: false,
        }
    }
    pub(super) fn snapshot() -> LiquidationSnapshot {
        let mut debt = reserve(1, 6, 100_000_000);
        debt.variable_debt = U256::from(1_000_000_000u64);
        let mut coll = reserve(2, 18, 2000_00000000u64);
        coll.collateral_balance = U256::exp10(18);
        coll.usage_as_collateral = true;
        LiquidationSnapshot {
            schema_version: 1,
            chain_id: 1,
            protocol: "aave_v3".into(),
            pool: Address::from_low_u64_be(3),
            oracle: Address::from_low_u64_be(4),
            implementation: Address::from_low_u64_be(5),
            implementation_code_hash: H256::from_low_u64_be(1),
            revision: LiquidationRevision::AaveV3CoreHf095,
            borrower: Address::from_low_u64_be(6),
            block_number: 100,
            block_hash: H256::from_low_u64_be(100),
            block_timestamp: 1000,
            observed_at_unix_ms: 1_000_000,
            health_factor_1e18: U256::from(960_000_000_000_000_000u64),
            total_collateral_base: U256::from(2000_00000000u64),
            total_debt_base: U256::from(1000_00000000u64),
            base_currency: Address::zero(),
            base_currency_unit: U256::from(100_000_000),
            user_emode: 0,
            reserves: vec![debt, coll],
            reserve_count: 2,
            reserves_complete: true,
        }
    }
    #[test]
    fn close_factor_respects_exact_threshold_and_healthy_boundary() {
        let mut s = snapshot();
        assert_eq!(close_factor_bps(&s).unwrap(), 5000);
        s.health_factor_1e18 = U256::from(HF_FULL_CLOSE);
        assert_eq!(close_factor_bps(&s).unwrap(), 10000);
        s.health_factor_1e18 += U256::one();
        assert_eq!(close_factor_bps(&s).unwrap(), 5000);
        s.health_factor_1e18 = U256::from(HF_ONE);
        assert!(close_factor_bps(&s).is_err());
    }
    #[test]
    fn mixed_decimals_fee_applies_only_to_bonus() {
        let s = snapshot();
        let t = liquidation_terms(&s, &s.reserves[0], &s.reserves[1], U256::MAX).unwrap();
        assert_eq!(t.debt_to_cover_wei, U256::from(500_000_000));
        assert_eq!(
            t.protocol_fee_collateral_wei,
            U256::from(1_250_000_000_000_000u64)
        );
        assert_eq!(
            t.collateral_to_liquidator_wei,
            U256::from(261_250_000_000_000_000u64)
        );
    }
    #[test]
    fn collateral_exhaustion_reduces_debt_with_protocol_rounding() {
        let mut s = snapshot();
        s.reserves[1].collateral_balance = U256::from(105_000_000_000_000_000u64);
        let t = liquidation_terms(&s, &s.reserves[0], &s.reserves[1], U256::MAX).unwrap();
        assert_eq!(t.debt_to_cover_wei, U256::from(200_000_000));
        assert_eq!(
            t.collateral_to_liquidator_wei,
            U256::from(104_500_000_000_000_000u64)
        );
    }
    #[test]
    fn missing_caps_and_emode_do_not_fabricate_terms() {
        let mut s = snapshot();
        let batch = enumerate_pairs(&s, &Default::default()).unwrap();
        assert!(batch.terms.is_empty());
        assert_eq!(batch.coverage.total_pairs, 1);
        assert_eq!(batch.coverage.skipped_pairs, 1);
        s.user_emode = 1;
        assert!(enumerate_pairs(&s, &Default::default()).is_err());
    }
    #[test]
    fn percent_rounding_matches_solidity_and_checks_overflow() {
        assert_eq!(percent_mul(U256::one(), 5000).unwrap(), U256::one());
        assert_eq!(percent_div(U256::from(21), 10500).unwrap(), U256::from(20));
        assert!(percent_mul(U256::MAX, 10000).is_err());
    }
    #[test]
    fn evaluates_all_asset_combinations_and_reports_cap() {
        let mut s = snapshot();
        s.reserves.clear();
        for id in 1..=65 {
            let mut r = reserve(id, 6, 100_000_000);
            r.variable_debt = U256::from(1_000_000_000u64);
            r.collateral_balance = U256::from(2_000_000_000u64);
            r.usage_as_collateral = true;
            s.reserves.push(r);
        }
        s.reserve_count = 65;
        let caps = s
            .reserves
            .iter()
            .map(|r| (r.asset, U256::from(1_000_000)))
            .collect();
        let batch = enumerate_pairs(&s, &caps).unwrap();
        assert_eq!(batch.coverage.total_pairs, 4225);
        assert_eq!(batch.coverage.evaluated_pairs, 4096);
        assert_eq!(batch.terms.len(), 4096);
        assert!(batch.coverage.limited);
    }
    #[test]
    fn request_is_bound_and_never_has_synthetic_profit_or_target() {
        let s = snapshot();
        let terms = liquidation_terms(&s, &s.reserves[0], &s.reserves[1], U256::MAX).unwrap();
        let unwind = LiquidationUnwind {
            router: Address::from_low_u64_be(8),
            path: vec![terms.collateral_asset, terms.debt_asset],
            amount_in_wei: terms.collateral_to_liquidator_wei,
            quoted_amount_out_wei: U256::from(520_000_000),
            minimum_amount_out_wei: U256::from(517_400_000),
            max_slippage_bps: 50,
            block_hash: s.block_hash,
        };
        let request = build_request(
            &s,
            terms.clone(),
            Address::from_low_u64_be(9),
            Address::from_low_u64_be(10),
            U256::from(1100),
            Some(unwind.clone()),
        )
        .unwrap();
        assert_eq!(request.calls.len(), 2);
        assert!(request.expected_profit_usd.is_none());
        assert!(request.net_expected_profit_usd.is_none());
        assert_eq!(request.status, "pending_simulation");
        let mut bad = unwind;
        bad.block_hash = H256::from_low_u64_be(101);
        assert!(build_request(
            &s,
            terms,
            request.caller,
            request.recipient,
            request.deadline,
            Some(bad)
        )
        .is_err());
    }
}
