//! V3 immutable fee units at the hydration boundary.
//! `fee()` returns hundredths of a basis point (pips), NOT basis points.
//! Read provenance is mandatory: never infer the unit from the magnitude of
//! an existing database value or an indexer's normalized fee hint.

/// Compare against the value read before computing the JSON replacement.
/// Shared by the runtime and the disposable-Redis concurrency regression.
pub(crate) const INDEX_COMPARE_AND_SET: &str = r#"
local current = redis.call('GET', KEYS[1])
if ARGV[1] == '0' then
    if current then return 0 end
elseif ARGV[1] == '1' then
    if current ~= ARGV[2] then return 0 end
else
    return redis.error_reply('invalid_observation_flag')
end
redis.call('SET', KEYS[1], ARGV[3])
return 1
"#;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct V3FeePips(u32);

impl V3FeePips {
    /// Decode exactly one canonical ABI uint24 word returned by pool.fee().
    /// UniswapV3Factory accepts fee < 1_000_000, including an explicitly
    /// configured zero tier. Missing data is NOT an observed zero fee.
    pub(crate) fn from_abi_word(word: &[u8]) -> Result<Self, &'static str> {
        if word.len() != 32 || word[..29].iter().any(|byte| *byte != 0) {
            return Err("v3_fee_invalid_abi_word");
        }
        let value = u32::from_be_bytes([0, word[29], word[30], word[31]]);
        if value >= 1_000_000 {
            return Err("v3_fee_out_of_range");
        }
        Ok(Self(value))
    }

    pub(crate) fn get(self) -> u32 {
        self.0
    }
}

#[derive(serde::Serialize, serde::Deserialize, Debug, PartialEq, Eq)]
struct V3PoolInfo {
    address: String,
    /// Legacy wire name. Consumers of pool_index_v3 pass this directly to
    /// QuoterV2, so its value MUST be the exact uint24 pips from fee().
    fee_bps: u32,
    #[serde(flatten)]
    extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// Prepare the corrected per-pair index value, preserving other entries.
/// Malformed JSON is an error, never an empty index to be overwritten.
/// A pool already present is updated, not appended with a duplicate address.
pub(crate) fn updated_v3_index(
    previous: Option<&str>,
    address: &str,
    fee: V3FeePips,
) -> Result<Option<String>, serde_json::Error> {
    let mut entries: Vec<V3PoolInfo> = match previous {
        Some(raw) => serde_json::from_str(raw)?,
        None => Vec::new(),
    };
    let mut found = false;
    let mut changed = false;
    for entry in &mut entries {
        if entry.address.eq_ignore_ascii_case(address) {
            found = true;
            if entry.fee_bps != fee.get() {
                entry.fee_bps = fee.get();
                changed = true;
            }
        }
    }
    if !found {
        entries.push(V3PoolInfo {
            address: address.to_ascii_lowercase(),
            fee_bps: fee.get(),
            extra: Default::default(),
        });
        changed = true;
    }
    if changed {
        serde_json::to_string(&entries).map(Some)
    } else {
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    const POOL: &str = "0x7995430a85156b2d40d5bb701608788cf84019e3";
    const OTHER: &str = "0x26f35b980f3b791ac3f7c09ff152815c0dcb5bf3";

    fn observed(raw: u32) -> V3FeePips {
        let mut word = [0u8; 32];
        word[28..].copy_from_slice(&raw.to_be_bytes());
        V3FeePips::from_abi_word(&word).unwrap()
    }

    fn index(value: &str) -> Vec<V3PoolInfo> {
        serde_json::from_str(value).unwrap()
    }

    #[test]
    fn common_tiers_remain_exact_pips() {
        for raw in [100, 500, 3000, 10000] {
            assert_eq!(observed(raw).get(), raw);
        }
    }

    #[test]
    fn observed_100_is_not_guessed_to_be_10000() {
        assert_eq!(observed(100).get(), 100);
    }

    #[test]
    fn nonstandard_and_fractional_bps_tiers_are_not_rounded() {
        for raw in [1, 150, 200, 250, 400, 999_999] {
            assert_eq!(observed(raw).get(), raw);
        }
    }

    #[test]
    fn explicit_zero_is_not_missing_data() {
        assert_eq!(observed(0).get(), 0);
        assert!(V3FeePips::from_abi_word(&[]).is_err());
    }

    #[test]
    fn truncated_or_extra_abi_data_is_rejected() {
        for length in [0, 1, 3, 31, 33, 64] {
            assert_eq!(
                V3FeePips::from_abi_word(&vec![0; length]),
                Err("v3_fee_invalid_abi_word")
            );
        }
    }

    #[test]
    fn noncanonical_uint24_padding_is_rejected() {
        for position in 0..29 {
            let mut word = [0u8; 32];
            word[position] = 1;
            assert_eq!(
                V3FeePips::from_abi_word(&word),
                Err("v3_fee_invalid_abi_word")
            );
        }
    }

    #[test]
    fn fees_at_or_above_one_hundred_percent_are_rejected() {
        for raw in [1_000_000u32, 16_777_215] {
            let mut word = [0u8; 32];
            word[28..].copy_from_slice(&raw.to_be_bytes());
            assert_eq!(V3FeePips::from_abi_word(&word), Err("v3_fee_out_of_range"));
        }
    }

    #[test]
    fn missing_index_gets_the_observed_fee_not_a_default() {
        let updated = updated_v3_index(None, POOL, observed(3000))
            .unwrap()
            .unwrap();
        assert_eq!(
            index(&updated),
            vec![V3PoolInfo {
                address: POOL.into(),
                fee_bps: 3000,
                extra: Default::default()
            }]
        );
    }

    #[test]
    fn rehydration_corrects_existing_fee_and_preserves_other_pool() {
        let before = serde_json::json!([
            {"address":POOL,"fee_bps":30}, {"address":OTHER,"fee_bps":500}
        ])
        .to_string();
        let after = updated_v3_index(Some(&before), POOL, observed(3000))
            .unwrap()
            .unwrap();
        assert_eq!(index(&after).len(), 2);
        assert_eq!(index(&after)[0].fee_bps, 3000);
        assert_eq!(index(&after)[1], index(&before)[1]);
        assert_eq!(
            updated_v3_index(Some(&after), POOL, observed(3000)).unwrap(),
            None
        );
    }

    #[test]
    fn case_differences_do_not_create_a_new_pool() {
        let before = serde_json::json!([{"address":POOL.to_uppercase(),"fee_bps":30}]).to_string();
        let after = updated_v3_index(Some(&before), POOL, observed(3000))
            .unwrap()
            .unwrap();
        assert_eq!(index(&after).len(), 1);
        assert_eq!(index(&after)[0].fee_bps, 3000);
    }

    #[test]
    fn legacy_duplicates_are_all_corrected_without_adding_more() {
        let before = serde_json::json!([
            {"address":POOL,"fee_bps":30}, {"address":POOL.to_uppercase(),"fee_bps":30}
        ])
        .to_string();
        let after = updated_v3_index(Some(&before), POOL, observed(3000))
            .unwrap()
            .unwrap();
        assert_eq!(index(&after).len(), 2);
        assert!(index(&after).iter().all(|p| p.fee_bps == 3000));
    }

    #[test]
    fn malformed_existing_index_is_not_replaced_with_empty_success() {
        for raw in ["", "{", "null", "{}", "[1]", r#"[{"address":"pool"}]"#] {
            assert!(updated_v3_index(Some(raw), POOL, observed(3000)).is_err());
        }
    }

    #[test]
    fn future_index_metadata_is_preserved_when_fee_is_corrected() {
        let before = serde_json::json!([
            {"address":POOL,"fee_bps":30,"observed_block":"123","metadata":{"source":"fixture"}},
            {"address":OTHER,"fee_bps":500,"future_flag":true}
        ])
        .to_string();
        let after = updated_v3_index(Some(&before), POOL, observed(3000))
            .unwrap()
            .unwrap();
        let old: serde_json::Value = serde_json::from_str(&before).unwrap();
        let new: serde_json::Value = serde_json::from_str(&after).unwrap();
        assert_eq!(new[0]["observed_block"], old[0]["observed_block"]);
        assert_eq!(new[0]["metadata"], old[0]["metadata"]);
        assert_eq!(new[1], old[1]);
    }

    #[test]
    fn observed_zero_remains_zero_in_the_index() {
        let after = updated_v3_index(None, POOL, observed(0)).unwrap().unwrap();
        assert_eq!(index(&after)[0].fee_bps, 0);
    }
}
