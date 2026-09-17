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
    #[serde(flatten)]
    pool: crate::reserves::V3PoolInfo,
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
        if entry.pool.pool_addr.eq_ignore_ascii_case(address) {
            found = true;
            if entry.pool.fee_bps != fee.get() {
                entry.pool.fee_bps = fee.get();
                changed = true;
            }
        }
    }
    if !found {
        entries.push(V3PoolInfo {
            pool: crate::reserves::V3PoolInfo {
                pool_addr: address.to_ascii_lowercase(),
                fee_bps: fee.get(),
            },
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

/// Bounded read/merge/CAS. A conflict retries from a NEW observation; a read,
/// malformed index, or uncertain write error is never an empty or successful index.
pub(crate) async fn publish_v3_index<Read, ReadFuture, Cas, CasFuture>(
    address: &str,
    fee: V3FeePips,
    read: Read,
    compare_and_set: Cas,
) -> Result<usize, &'static str>
where
    Read: FnMut() -> ReadFuture,
    ReadFuture: std::future::Future<Output = Result<Option<String>, &'static str>>,
    Cas: FnMut(Option<String>, String) -> CasFuture,
    CasFuture: std::future::Future<Output = Result<bool, &'static str>>,
{
    publish_index_update(read, compare_and_set, |previous| {
        updated_v3_index(previous, address, fee).map_err(|_| "redis_v3_index_invalid")
    })
    .await
}

/// Bootstrap is an additive PG snapshot, NOT a newer on-chain observation.
/// Never erase a hydrated pool or overwrite its fee with a snapshot captured
/// earlier. Historical corrections/removals require a separately verified job.
fn merged_v3_bootstrap(
    previous: Option<&str>,
    snapshot: &[crate::reserves::V3PoolInfo],
) -> Result<Option<String>, &'static str> {
    let mut entries: Vec<V3PoolInfo> = match previous {
        Some(raw) => serde_json::from_str(raw).map_err(|_| "redis_v3_index_invalid")?,
        None => Vec::new(),
    };
    let mut known: std::collections::HashSet<String> = entries
        .iter()
        .map(|entry| entry.pool.pool_addr.to_ascii_lowercase())
        .collect();
    let mut changed = false;
    for row in snapshot {
        if row.pool_addr.len() != 42
            || !row.pool_addr.starts_with("0x")
            || !row.pool_addr.as_bytes()[2..]
                .iter()
                .all(u8::is_ascii_hexdigit)
            || row.fee_bps >= 1_000_000
        {
            return Err("v3_bootstrap_row_invalid");
        }
        let address = row.pool_addr.to_ascii_lowercase();
        if known.insert(address.clone()) {
            entries.push(V3PoolInfo {
                pool: crate::reserves::V3PoolInfo {
                    pool_addr: address,
                    fee_bps: row.fee_bps,
                },
                extra: Default::default(),
            });
            changed = true;
        }
    }
    if changed {
        serde_json::to_string(&entries)
            .map(Some)
            .map_err(|_| "redis_v3_index_invalid")
    } else {
        Ok(None)
    }
}

pub(crate) async fn publish_v3_bootstrap<Read, ReadFuture, Cas, CasFuture>(
    snapshot: &[crate::reserves::V3PoolInfo],
    read: Read,
    compare_and_set: Cas,
) -> Result<usize, &'static str>
where
    Read: FnMut() -> ReadFuture,
    ReadFuture: std::future::Future<Output = Result<Option<String>, &'static str>>,
    Cas: FnMut(Option<String>, String) -> CasFuture,
    CasFuture: std::future::Future<Output = Result<bool, &'static str>>,
{
    publish_index_update(read, compare_and_set, |previous| {
        merged_v3_bootstrap(previous, snapshot)
    })
    .await
}

/// CATALOG-BACKFILL-01 (2026-09-17): bootstrap that REPLACES a legacy
/// unparseable index instead of erroring forever.
///
/// Evidence (Redis dump 2026-09-17): 93 of 186 `arbx:pool_index_v3` keys hold
/// pre-WO-06 entries (`[{"address":"0x…","fee_bps":30}]`) that serde can never
/// parse as `Vec<V3PoolInfo>`. The additive `merged_v3_bootstrap` returns
/// `redis_v3_index_invalid` for them, so `set_pool_index_v3` has been failing
/// on those pairs at EVERY boot — the garbage is unpreserveable and the pools
/// in it can never be fixed by an additive merge. This variant treats an
/// unparseable previous value as ABSENT and writes the snapshot (whose tiers
/// are canonical-from-PG or on-chain `fee()`-verified by the caller) in its
/// place. The CAS still guards concurrency: the write only lands if the key is
/// unchanged since the read.
pub(crate) async fn publish_v3_bootstrap_repair<Read, ReadFuture, Cas, CasFuture>(
    snapshot: &[crate::reserves::V3PoolInfo],
    read: Read,
    compare_and_set: Cas,
) -> Result<usize, &'static str>
where
    Read: FnMut() -> ReadFuture,
    ReadFuture: std::future::Future<Output = Result<Option<String>, &'static str>>,
    Cas: FnMut(Option<String>, String) -> CasFuture,
    CasFuture: std::future::Future<Output = Result<bool, &'static str>>,
{
    publish_index_update(read, compare_and_set, |previous| {
        match merged_v3_bootstrap(previous, snapshot) {
            // Unparseable legacy index: retry the merge as if the key were
            // absent — the snapshot (fee-verified by the caller) replaces it.
            Err("redis_v3_index_invalid") => merged_v3_bootstrap(None, snapshot),
            other => other,
        }
    })
    .await
}

/// The single retry/CAS implementation used by BOTH bootstrap and hydration.
async fn publish_index_update<Read, ReadFuture, Cas, CasFuture, Merge>(
    mut read: Read,
    mut compare_and_set: Cas,
    merge: Merge,
) -> Result<usize, &'static str>
where
    Read: FnMut() -> ReadFuture,
    ReadFuture: std::future::Future<Output = Result<Option<String>, &'static str>>,
    Cas: FnMut(Option<String>, String) -> CasFuture,
    CasFuture: std::future::Future<Output = Result<bool, &'static str>>,
    Merge: Fn(Option<&str>) -> Result<Option<String>, &'static str>,
{
    for attempt in 1..=4 {
        let previous = read().await?;
        let Some(replacement) = merge(previous.as_deref())? else {
            return Ok(attempt);
        };
        if compare_and_set(previous, replacement).await? {
            return Ok(attempt);
        }
    }
    Err("redis_v3_index_conflict_exhausted")
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
                pool: crate::reserves::V3PoolInfo {
                    pool_addr: POOL.into(),
                    fee_bps: 3000,
                },
                extra: Default::default()
            }]
        );
    }

    #[test]
    fn rehydration_corrects_existing_fee_and_preserves_other_pool() {
        let before = serde_json::json!([
            {"pool_addr":POOL,"fee_bps":30}, {"pool_addr":OTHER,"fee_bps":500}
        ])
        .to_string();
        let after = updated_v3_index(Some(&before), POOL, observed(3000))
            .unwrap()
            .unwrap();
        assert_eq!(index(&after).len(), 2);
        assert_eq!(index(&after)[0].pool.fee_bps, 3000);
        assert_eq!(index(&after)[1], index(&before)[1]);
        assert_eq!(
            updated_v3_index(Some(&after), POOL, observed(3000)).unwrap(),
            None
        );
    }

    #[test]
    fn case_differences_do_not_create_a_new_pool() {
        let before =
            serde_json::json!([{"pool_addr":POOL.to_uppercase(),"fee_bps":30}]).to_string();
        let after = updated_v3_index(Some(&before), POOL, observed(3000))
            .unwrap()
            .unwrap();
        assert_eq!(index(&after).len(), 1);
        assert_eq!(index(&after)[0].pool.fee_bps, 3000);
    }

    #[test]
    fn legacy_duplicates_are_all_corrected_without_adding_more() {
        let before = serde_json::json!([
            {"pool_addr":POOL,"fee_bps":30}, {"pool_addr":POOL.to_uppercase(),"fee_bps":30}
        ])
        .to_string();
        let after = updated_v3_index(Some(&before), POOL, observed(3000))
            .unwrap()
            .unwrap();
        assert_eq!(index(&after).len(), 2);
        assert!(index(&after).iter().all(|p| p.pool.fee_bps == 3000));
    }

    #[test]
    fn malformed_existing_index_is_not_replaced_with_empty_success() {
        for raw in ["", "{", "null", "{}", "[1]", r#"[{"pool_addr":"pool"}]"#] {
            assert!(updated_v3_index(Some(raw), POOL, observed(3000)).is_err());
        }
    }

    #[test]
    fn future_index_metadata_is_preserved_when_fee_is_corrected() {
        let before = serde_json::json!([
            {"pool_addr":POOL,"fee_bps":30,"observed_block":"123","metadata":{"source":"fixture"}},
            {"pool_addr":OTHER,"fee_bps":500,"future_flag":true}
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
        assert_eq!(index(&after)[0].pool.fee_bps, 0);
    }
}

#[cfg(test)]
mod review_regression {
    #[test]
    fn v3_review_reads_canonical_pool_addr() {
        let mut word = [0u8; 32];
        word[28..].copy_from_slice(&3000u32.to_be_bytes());
        let fee = super::V3FeePips::from_abi_word(&word).unwrap();
        let before = r#"[{"pool_addr":"0x7995430a85156b2d40d5bb701608788cf84019e3","fee_bps":30}]"#;
        let result = super::updated_v3_index(
            Some(before),
            "0x7995430a85156b2d40d5bb701608788cf84019e3",
            fee,
        );
        assert!(
            result.is_ok(),
            "canonical reserves::V3PoolInfo rejected: {result:?}"
        );
    }
}

#[cfg(test)]
mod retry_regression {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use std::{
        cell::{Cell, RefCell},
        future::{ready, Future},
        task::{Context, Poll, Waker},
    };
    const A: &str = "0x7995430a85156b2d40d5bb701608788cf84019e3";
    const B: &str = "0x26f35b980f3b791ac3f7c09ff152815c0dcb5bf3";
    fn fee() -> V3FeePips {
        let mut word = [0u8; 32];
        word[28..].copy_from_slice(&3000u32.to_be_bytes());
        V3FeePips::from_abi_word(&word).unwrap()
    }
    fn immediate<F: Future>(future: F) -> F::Output {
        let mut future = std::pin::pin!(future);
        match future
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
        {
            Poll::Ready(value) => value,
            Poll::Pending => panic!("This laboratory adapter only returns ready futures"),
        }
    }

    /// CATALOG-BACKFILL-01: parametrized observed fee (the module-level
    /// `fee()` helper is hardwired to 3000).
    fn observed_fee(raw: u32) -> V3FeePips {
        let mut word = [0u8; 32];
        word[28..].copy_from_slice(&raw.to_be_bytes());
        V3FeePips::from_abi_word(&word).unwrap()
    }

    #[test]
    fn v3_review_real_consumer_dto_round_trip() {
        let previous = serde_json::to_string(&vec![crate::reserves::V3PoolInfo {
            pool_addr: A.to_string(),
            fee_bps: 30,
        }])
        .unwrap();
        let updated = updated_v3_index(Some(&previous), A, fee())
            .unwrap()
            .unwrap();
        let read_by_scanner: Vec<crate::reserves::V3PoolInfo> =
            serde_json::from_str(&updated).unwrap();
        assert_eq!(read_by_scanner[0].pool_addr, A);
        assert_eq!(read_by_scanner[0].fee_bps, 3000);
        assert_eq!(read_by_scanner.len(), 1);
        assert!(
            serde_json::from_str::<serde_json::Value>(&updated).unwrap()[0]
                .get("address")
                .is_none()
        );
    }

    #[test]
    fn v3_review_cas_conflict_rereads_and_preserves_both_writers() {
        let state = RefCell::new(None::<String>);
        let writes = Cell::new(0);
        let result = immediate(publish_v3_index(
            A,
            fee(),
            || ready(Ok(state.borrow().clone())),
            |previous, updated| {
                writes.set(writes.get() + 1);
                if writes.get() == 1 {
                    *state.borrow_mut() = Some(format!(r#"[{{"pool_addr":"{B}","fee_bps":500}}]"#));
                    return ready(Ok(false));
                }
                assert_eq!(previous, *state.borrow());
                *state.borrow_mut() = Some(updated);
                ready(Ok(true))
            },
        ));
        assert_eq!(result, Ok(2));
        let rows: Vec<crate::reserves::V3PoolInfo> =
            serde_json::from_str(state.borrow().as_ref().unwrap()).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].pool_addr, B);
        assert_eq!(rows[0].fee_bps, 500);
        assert_eq!(rows[1].pool_addr, A);
        assert_eq!(rows[1].fee_bps, 3000);
    }

    #[test]
    fn v3_review_cas_exhaustion_is_bounded_and_not_success() {
        let reads = Cell::new(0);
        let writes = Cell::new(0);
        let result = immediate(publish_v3_index(
            A,
            fee(),
            || {
                reads.set(reads.get() + 1);
                ready(Ok(None))
            },
            |_, _| {
                writes.set(writes.get() + 1);
                ready(Ok(false))
            },
        ));
        assert_eq!(result, Err("redis_v3_index_conflict_exhausted"));
        assert_eq!(reads.get(), 4);
        assert_eq!(writes.get(), 4);
    }

    #[test]
    fn v3_review_read_or_malformed_index_does_not_write() {
        for observed in [Err("redis_v3_index_read_failed"), Ok(Some("{".to_string()))] {
            let writes = Cell::new(0);
            let result = immediate(publish_v3_index(
                A,
                fee(),
                || ready(observed.clone()),
                |_, _| {
                    writes.set(writes.get() + 1);
                    ready(Ok(true))
                },
            ));
            assert!(result.is_err());
            assert_eq!(writes.get(), 0);
        }
    }

    #[test]
    fn v3_review_uncertain_write_is_not_retried_as_known_conflict() {
        let writes = Cell::new(0);
        let result = immediate(publish_v3_index(
            A,
            fee(),
            || ready(Ok(None)),
            |_, _| {
                writes.set(writes.get() + 1);
                ready(Err("redis_v3_index_write_failed"))
            },
        ));
        assert_eq!(result, Err("redis_v3_index_write_failed"));
        assert_eq!(writes.get(), 1);
    }

    #[test]
    fn v3_review_already_correct_is_idempotent_without_write() {
        let previous = updated_v3_index(None, A, fee()).unwrap().unwrap();
        let result = immediate(publish_v3_index(
            A,
            fee(),
            || ready(Ok(Some(previous.clone()))),
            |_, _| {
                panic!("unchanged index must not be written");
                #[allow(unreachable_code)]
                ready(Ok(true))
            },
        ));
        assert_eq!(result, Ok(1));
    }
    #[test]
    fn v3_review_bootstrap_preserves_hydrated_fee_metadata_and_other_pools() {
        let previous = format!(r#"[{{"pool_addr":"{A}","fee_bps":3000,"block":123}}]"#);
        let snapshot = vec![
            crate::reserves::V3PoolInfo {
                pool_addr: A.into(),
                fee_bps: 30,
            },
            crate::reserves::V3PoolInfo {
                pool_addr: B.into(),
                fee_bps: 500,
            },
        ];
        let next = merged_v3_bootstrap(Some(&previous), &snapshot)
            .unwrap()
            .unwrap();
        let rows: Vec<serde_json::Value> = serde_json::from_str(&next).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0]["fee_bps"], 3000);
        assert_eq!(rows[0]["block"], 123);
        assert_eq!(rows[1]["fee_bps"], 500);
    }

    #[test]
    fn v3_review_bootstrap_conflict_rereads_hydration_before_merging() {
        let state = RefCell::new(None::<String>);
        let writes = Cell::new(0);
        let snapshot = vec![
            crate::reserves::V3PoolInfo {
                pool_addr: A.into(),
                fee_bps: 30,
            },
            crate::reserves::V3PoolInfo {
                pool_addr: B.into(),
                fee_bps: 500,
            },
        ];
        let result = immediate(publish_v3_bootstrap(
            &snapshot,
            || ready(Ok(state.borrow().clone())),
            |previous, updated| {
                writes.set(writes.get() + 1);
                if writes.get() == 1 {
                    *state.borrow_mut() = updated_v3_index(None, A, fee()).unwrap();
                    return ready(Ok(false));
                }
                assert_eq!(previous, *state.borrow());
                *state.borrow_mut() = Some(updated);
                ready(Ok(true))
            },
        ));
        assert_eq!(result, Ok(2));
        let rows: Vec<crate::reserves::V3PoolInfo> =
            serde_json::from_str(state.borrow().as_ref().unwrap()).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].fee_bps, 3000);
        assert_eq!(rows[1].fee_bps, 500);
    }

    #[test]
    fn v3_review_bootstrap_never_erases_on_empty_snapshot() {
        let previous = updated_v3_index(None, A, fee()).unwrap().unwrap();
        assert_eq!(merged_v3_bootstrap(Some(&previous), &[]), Ok(None));
        assert_eq!(merged_v3_bootstrap(None, &[]), Ok(None));
    }

    #[test]
    fn v3_review_bootstrap_deduplicates_case_without_replacing_existing_tier() {
        let rows = vec![
            crate::reserves::V3PoolInfo {
                pool_addr: A.into(),
                fee_bps: 3000,
            },
            crate::reserves::V3PoolInfo {
                pool_addr: format!("0x{}", &A[2..].to_uppercase()),
                fee_bps: 30,
            },
        ];
        let next = merged_v3_bootstrap(None, &rows).unwrap().unwrap();
        let result: Vec<crate::reserves::V3PoolInfo> = serde_json::from_str(&next).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].fee_bps, 3000);
    }

    #[test]
    fn v3_review_bootstrap_errors_do_not_write_or_fabricate_success() {
        let snapshot = vec![crate::reserves::V3PoolInfo {
            pool_addr: A.into(),
            fee_bps: 500,
        }];
        for read in [
            Err("redis_v3_index_read_failed"),
            Ok(Some("broken".to_string())),
        ] {
            let result = immediate(publish_v3_bootstrap(
                &snapshot,
                || ready(read.clone()),
                |_, _| {
                    panic!("bad source must not write");
                    #[allow(unreachable_code)]
                    ready(Ok(true))
                },
            ));
            assert!(result.is_err());
        }
        for row in [
            crate::reserves::V3PoolInfo {
                pool_addr: "invalid".into(),
                fee_bps: 500,
            },
            crate::reserves::V3PoolInfo {
                pool_addr: A.into(),
                fee_bps: 1_000_000,
            },
        ] {
            assert_eq!(
                merged_v3_bootstrap(None, &[row]),
                Err("v3_bootstrap_row_invalid")
            );
        }
    }

    #[test]
    fn v3_review_bootstrap_conflicts_use_same_bounded_retry() {
        let snapshot = vec![crate::reserves::V3PoolInfo {
            pool_addr: A.into(),
            fee_bps: 500,
        }];
        let writes = Cell::new(0);
        let result = immediate(publish_v3_bootstrap(
            &snapshot,
            || ready(Ok(None)),
            |_, _| {
                writes.set(writes.get() + 1);
                ready(Ok(false))
            },
        ));
        assert_eq!(result, Err("redis_v3_index_conflict_exhausted"));
        assert_eq!(writes.get(), 4);
    }

    // ── CATALOG-BACKFILL-01 (2026-09-17): legacy-index repair ────────────────

    fn verified_snapshot() -> Vec<crate::reserves::V3PoolInfo> {
        vec![
            // On-chain fee()-verified canonical tier (3000 pips, NOT the
            // legacy bps-unit 30 the garbage key carried).
            crate::reserves::V3PoolInfo {
                pool_addr: A.into(),
                fee_bps: 3000,
            },
            crate::reserves::V3PoolInfo {
                pool_addr: B.into(),
                fee_bps: 100,
            },
        ]
    }

    #[test]
    fn catalog_backfill_repair_replaces_legacy_unparseable_index() {
        // The EXACT legacy shape found in 93 production keys (2026-09-17):
        // "address" field + bps-unit fee — unparsable as Vec<V3PoolInfo>.
        let legacy = r#"[{"address":"0x7995430a85156b2d40d5bb701608788cf84019e3","fee_bps":30}]"#;
        let state = RefCell::new(Some(legacy.to_string()));
        let result = immediate(publish_v3_bootstrap_repair(
            &verified_snapshot(),
            || ready(Ok(state.borrow().clone())),
            |previous, updated| {
                assert_eq!(previous.as_deref(), Some(legacy));
                *state.borrow_mut() = Some(updated);
                ready(Ok(true))
            },
        ));
        assert_eq!(result, Ok(1));
        let rows: Vec<crate::reserves::V3PoolInfo> =
            serde_json::from_str(state.borrow().as_ref().unwrap()).unwrap();
        assert_eq!(rows.len(), 2, "legacy garbage is replaced, not merged");
        assert_eq!(rows[0].pool_addr, A);
        assert_eq!(
            rows[0].fee_bps, 3000,
            "on-chain verified pips, not bps-unit 30"
        );
        assert_eq!(rows[1].fee_bps, 100);
    }

    #[test]
    fn catalog_backfill_repair_still_merges_additively_when_parseable() {
        // A healthy (parseable) index is NOT replaced — additive merge only.
        let healthy = updated_v3_index(None, B, observed_fee(500))
            .unwrap()
            .unwrap();
        let state = RefCell::new(Some(healthy.clone()));
        let result = immediate(publish_v3_bootstrap_repair(
            &verified_snapshot(),
            || ready(Ok(state.borrow().clone())),
            |previous, updated| {
                assert_eq!(previous.as_deref(), Some(healthy.as_str()));
                *state.borrow_mut() = Some(updated);
                ready(Ok(true))
            },
        ));
        assert_eq!(result, Ok(1));
        let rows: Vec<crate::reserves::V3PoolInfo> =
            serde_json::from_str(state.borrow().as_ref().unwrap()).unwrap();
        assert_eq!(rows.len(), 2, "parseable index keeps its entries (A added)");
        assert_eq!(rows[0].pool_addr, B);
        assert_eq!(rows[0].fee_bps, 500, "existing hydrated tier is preserved");
        assert_eq!(rows[1].pool_addr, A);
    }

    #[test]
    fn catalog_backfill_plain_bootstrap_still_fails_on_legacy_index() {
        // Guard: the ORIGINAL publish_v3_bootstrap keeps its contract — a
        // malformed index is an error, never a silent replacement (only the
        // explicitly-repairing caller may replace).
        let legacy = r#"[{"address":"0x7995430a85156b2d40d5bb701608788cf84019e3","fee_bps":30}]"#;
        let result = immediate(publish_v3_bootstrap(
            &verified_snapshot(),
            || ready(Ok(Some(legacy.to_string()))),
            |_, _| {
                panic!("plain bootstrap must not write over a malformed index");
                #[allow(unreachable_code)]
                ready(Ok(true))
            },
        ));
        assert_eq!(result, Err("redis_v3_index_invalid"));
    }
}
