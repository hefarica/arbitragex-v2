//! Empirical objectives derived only from finalized, reconciled receipts.
//!
//! Cohorts are chain + strategy + input asset. These are historical loss ratios,
//! not a guarantee for the next route. Missing or inconsistent history is `None`.

use ethers::types::{Address, H256};
use ethers::utils::keccak256;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::str::FromStr;

pub const MAX_OBSERVATIONS: usize = 128;
pub const MIN_OBSERVATIONS: usize = 20;
pub const HISTORY_MAX_AGE_MS: i64 = 7 * 24 * 60 * 60 * 1000;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RealizedObservation {
    pub schema_version: u8,
    pub source: String,
    pub chain_id: u64,
    pub strategy_kind: String,
    pub token_in: String,
    pub tx_hash: String,
    pub block_hash: String,
    pub block_number: u64,
    pub observed_at_ms: i64,
    pub principal_usd: f64,
    pub net_profit_usd: f64,
    pub latency_ms: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EmpiricalRisk {
    pub samples: usize,
    pub loss_fraction_cvar95: f64,
    pub latency_p95_ms: f64,
}

pub fn history_key(chain_id: u64, strategy_kind: &str, token_in: &str) -> String {
    let key = format!(
        "{chain_id}|{strategy_kind}|{}",
        token_in.to_ascii_lowercase()
    );
    format!("arbx:risk:realized:{chain_id}:{:#x}", H256(keccak256(key)))
}

/// Uses at most the newest 128 input records. Invalid membership/age records do
/// not enter the sample; conflicting transactions or unrepresentable economics
/// invalidate the cohort rather than quietly removing an extreme loss.
pub fn empirical_risk(
    observations: &[RealizedObservation],
    chain_id: u64,
    strategy_kind: &str,
    token_in: &str,
    now_ms: i64,
) -> Option<EmpiricalRisk> {
    let asset = Address::from_str(token_in).ok().filter(|a| !a.is_zero())?;
    let mut seen: HashMap<H256, (&RealizedObservation, H256)> = HashMap::new();
    let mut losses = Vec::with_capacity(MAX_OBSERVATIONS);
    let mut latencies = Vec::with_capacity(MAX_OBSERVATIONS);
    for row in observations.iter().take(MAX_OBSERVATIONS) {
        if row.schema_version != 1
            || row.source != "finalized_receipt"
            || row.chain_id != chain_id
            || row.strategy_kind != strategy_kind
            || Address::from_str(&row.token_in).ok() != Some(asset)
            || row.observed_at_ms <= 0
            || now_ms
                .checked_sub(row.observed_at_ms)
                .is_none_or(|age| !(0..=HISTORY_MAX_AGE_MS).contains(&age))
        {
            continue;
        }
        let tx = match H256::from_str(&row.tx_hash) {
            Ok(h) if !h.is_zero() => h,
            _ => continue,
        };
        let block = match H256::from_str(&row.block_hash) {
            Ok(h) if !h.is_zero() => h,
            _ => continue,
        };
        if row.block_number == 0
            || !row.principal_usd.is_finite()
            || row.principal_usd <= 0.0
            || !row.net_profit_usd.is_finite()
            || !row.latency_ms.is_finite()
            || row.latency_ms < 0.0
        {
            return None;
        }
        if let Some((previous, previous_block)) = seen.get(&tx) {
            // Compare typed hashes so differently-cased hexadecimal encodings
            // cannot inflate the minimum sample count.
            if *previous_block != block
                || previous.block_number != row.block_number
                || previous.observed_at_ms != row.observed_at_ms
                || previous.principal_usd != row.principal_usd
                || previous.net_profit_usd != row.net_profit_usd
                || previous.latency_ms != row.latency_ms
            {
                return None;
            }
            continue;
        }
        let loss = if row.net_profit_usd < 0.0 {
            -row.net_profit_usd / row.principal_usd
        } else {
            0.0
        };
        if !loss.is_finite() {
            return None;
        }
        seen.insert(tx, (row, block));
        losses.push(loss);
        latencies.push(row.latency_ms);
    }
    let samples = losses.len();
    if samples < MIN_OBSERVATIONS {
        return None;
    }
    losses.sort_unstable_by(|a, b| b.total_cmp(a));
    latencies.sort_unstable_by(f64::total_cmp);
    let tail_mass = samples as f64 * 0.05;
    let mut included_mass = 0.0_f64;
    let mut cvar = 0.0_f64;
    for loss in losses {
        let weight = (tail_mass - included_mass).min(1.0);
        if weight <= 0.0 {
            break;
        }
        let next_mass = included_mass + weight;
        // Stable weighted mean: summing large finite losses can overflow, even
        // when their mean is representable.
        cvar += (loss - cvar) * (weight / next_mass);
        included_mass = next_mass;
    }
    let rank = (samples as f64 * 0.95).ceil() as usize;
    Some(EmpiricalRisk {
        samples,
        loss_fraction_cvar95: cvar,
        latency_p95_ms: latencies[rank.saturating_sub(1)],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    const NOW: i64 = 1_800_000_000_000;
    const ASSET: &str = "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    fn rows(n: usize) -> Vec<RealizedObservation> {
        (0..n)
            .map(|i| RealizedObservation {
                schema_version: 1,
                source: "finalized_receipt".into(),
                chain_id: 1,
                strategy_kind: "dex_arb".into(),
                token_in: ASSET.into(),
                tx_hash: format!("{:#x}", H256::from_low_u64_be(0xaa + i as u64)),
                block_hash: format!("{:#x}", H256::from_low_u64_be(1000 + i as u64)),
                block_number: 1000 + i as u64,
                observed_at_ms: NOW - 1000,
                principal_usd: 100.0,
                net_profit_usd: 1.0,
                latency_ms: i as f64 + 1.0,
            })
            .collect()
    }

    fn eval(rows: &[RealizedObservation]) -> Option<EmpiricalRisk> {
        empirical_risk(rows, 1, "dex_arb", ASSET, NOW)
    }

    #[test]
    fn empty_and_insufficient_history_are_unknown() {
        assert!(eval(&[]).is_none());
        assert!(eval(&rows(19)).is_none());
    }

    #[test]
    fn profitable_history_has_zero_observed_loss_and_nearest_rank_latency() {
        let value = eval(&rows(20)).expect("20 observations");
        assert_eq!(value.samples, 20);
        assert_eq!(value.loss_fraction_cvar95, 0.0);
        assert_eq!(value.latency_p95_ms, 19.0);
    }

    #[test]
    fn fractional_tail_uses_only_required_fraction_of_boundary_observation() {
        let mut observations = rows(21);
        observations[0].net_profit_usd = -20.0;
        observations[1].net_profit_usd = -10.0;
        let value = eval(&observations).expect("valid cohort");
        assert!((value.loss_fraction_cvar95 - (0.2 + 0.05 * 0.1) / 1.05).abs() < 1e-14);
        assert_eq!(value.latency_p95_ms, 20.0);
    }

    #[test]
    fn duplicate_hash_case_does_not_inflate_samples() {
        let mut observations = rows(19);
        let mut duplicate = observations[0].clone();
        duplicate.tx_hash = format!("0x{}", duplicate.tx_hash[2..].to_ascii_uppercase());
        observations.push(duplicate);
        assert!(eval(&observations).is_none());
    }

    #[test]
    fn conflicting_duplicate_invalidates_cohort() {
        let mut observations = rows(20);
        let mut duplicate = observations[0].clone();
        duplicate.net_profit_usd = -300.0;
        observations.push(duplicate);
        assert!(eval(&observations).is_none());
    }

    #[test]
    fn only_finalized_matching_fresh_cohort_is_used() {
        for change in 0..7 {
            let mut observations = rows(20);
            match change {
                0 => observations[0].source = "included_receipt".into(),
                1 => observations[0].chain_id = 10,
                2 => observations[0].strategy_kind = "liquidation".into(),
                3 => observations[0].token_in = format!("{:#x}", Address::from_low_u64_be(4)),
                4 => observations[0].observed_at_ms = NOW + 1,
                5 => observations[0].observed_at_ms = NOW - HISTORY_MAX_AGE_MS - 1,
                _ => observations[0].block_hash = format!("{:#x}", H256::zero()),
            }
            assert!(eval(&observations).is_none(), "change {change}");
        }
    }

    #[test]
    fn unrepresentable_loss_is_not_silently_removed() {
        let mut observations = rows(21);
        observations[0].principal_usd = f64::MIN_POSITIVE;
        observations[0].net_profit_usd = -f64::MAX;
        assert!(eval(&observations).is_none());
        observations[0].principal_usd = f64::NAN;
        assert!(eval(&observations).is_none());
    }

    #[test]
    fn mean_of_large_finite_losses_does_not_overflow() {
        let mut observations = rows(40);
        for row in &mut observations {
            row.principal_usd = 1.0;
            row.net_profit_usd = -f64::MAX;
        }
        assert_eq!(
            eval(&observations)
                .expect("finite mean")
                .loss_fraction_cvar95,
            f64::MAX
        );
    }

    #[test]
    fn bounded_history_and_keys_preserve_cohort_identity() {
        assert_eq!(
            eval(&rows(200)).expect("bounded sample").samples,
            MAX_OBSERVATIONS
        );
        assert_eq!(
            history_key(1, "dex_arb", ASSET),
            history_key(1, "dex_arb", &ASSET.to_ascii_uppercase())
        );
        assert_ne!(
            history_key(1, "dex_arb", ASSET),
            history_key(10, "dex_arb", ASSET)
        );
        assert_ne!(
            history_key(1, "dex_arb", ASSET),
            history_key(1, "triangular", ASSET)
        );
    }
}
