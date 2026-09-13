//! Bounded NSGA-II from finalized receipt cohorts, refreshed off discovery.
//! Unknown history retains Net_bps order and never implies zero risk.
use math_engine::operators::op_32_nsga2::core::{Nsga2Config, Nsga2Optimizer, Objectives};
use redis::aio::ConnectionManager;
use shared_rs::settlement_risk::{empirical_risk, history_key, EmpiricalRisk, RealizedObservation};
use std::{
    collections::BTreeMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};
const MAX_RANKED: usize = 128;
const REFRESH_AFTER: Duration = Duration::from_secs(10);
const EXPIRE_AFTER: Duration = Duration::from_secs(30);
#[derive(Debug, Clone)]
pub struct RankingInput {
    pub strategy_kind: String,
    pub token_in: String,
    pub principal_usd: f64,
    pub expected_profit_usd: f64,
}
impl RankingInput {
    fn valid(&self) -> bool {
        !self.strategy_kind.is_empty()
            && self
                .token_in
                .parse::<ethers::types::Address>()
                .ok()
                .is_some_and(|a| !a.is_zero())
            && self.principal_usd.is_finite()
            && self.principal_usd > 0.0
            && self.expected_profit_usd.is_finite()
    }
}
struct CachedRisk {
    at: Instant,
    risk: Option<EmpiricalRisk>,
}
#[derive(Default)]
struct Cache {
    entries: BTreeMap<String, CachedRisk>,
    last_attempt: Option<Instant>,
}
#[derive(Default)]
pub struct LiveRiskRanker {
    cache: Arc<Mutex<Cache>>,
    refreshing: Arc<AtomicBool>,
}
#[derive(Debug, Default, Clone, Copy)]
pub struct RankingSummary {
    pub considered: usize,
    pub with_history: usize,
    pub reordered: bool,
}
impl LiveRiskRanker {
    /// At most 128 entries; payloads stay attached. RPC/Redis never block here.
    pub fn rank<T>(
        &self,
        entries: &mut [T],
        chain: u64,
        redis: &ConnectionManager,
        describe: impl Fn(&T) -> Option<RankingInput>,
    ) -> RankingSummary {
        let inputs: Vec<_> = entries
            .iter()
            .take(MAX_RANKED)
            .map(|e| describe(e).filter(RankingInput::valid))
            .collect();
        let now = Instant::now();
        let mut values = vec![None; inputs.len()];
        let mut refresh = BTreeMap::new();
        if let Ok(cache) = self.cache.lock() {
            for (i, input) in inputs.iter().enumerate() {
                let Some(input) = input else { continue };
                let key = history_key(chain, &input.strategy_kind, &input.token_in);
                let cached = cache.entries.get(&key);
                if let Some(risk) = cached
                    .filter(|c| now.duration_since(c.at) < EXPIRE_AFTER)
                    .and_then(|c| c.risk)
                {
                    let loss = risk.loss_fraction_cvar95 * input.principal_usd;
                    if loss.is_finite() {
                        values[i] = Some(Objectives {
                            expected_profit: input.expected_profit_usd,
                            risk_cvar: loss,
                            latency_ms: risk.latency_p95_ms,
                        });
                    }
                }
                if cached.is_none_or(|c| now.duration_since(c.at) >= REFRESH_AFTER)
                    && refresh.len() < 32
                {
                    refresh
                        .entry(key)
                        .or_insert_with(|| (input.strategy_kind.clone(), input.token_in.clone()));
                }
            }
        }
        self.schedule_refresh(chain, redis, refresh);
        let mut summary = RankingSummary {
            considered: inputs.len(),
            with_history: values.iter().filter(|v| v.is_some()).count(),
            reordered: false,
        };
        if let Some(order) = available_order(&values) {
            summary.reordered = order.iter().enumerate().any(|(i, x)| i != *x);
            apply_permutation(&mut entries[..inputs.len()], &order);
        }
        summary
    }
    fn schedule_refresh(
        &self,
        chain: u64,
        redis: &ConnectionManager,
        cohorts: BTreeMap<String, (String, String)>,
    ) {
        if cohorts.is_empty() {
            return;
        }
        let Ok(runtime) = tokio::runtime::Handle::try_current() else {
            return;
        };
        let Ok(mut cache) = self.cache.try_lock() else {
            return;
        };
        let now = Instant::now();
        if cache
            .last_attempt
            .is_some_and(|t| now.duration_since(t) < Duration::from_secs(1))
        {
            return;
        }
        if self
            .refreshing
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return;
        }
        cache.last_attempt = Some(now);
        drop(cache);
        let cache = self.cache.clone();
        let flag = self.refreshing.clone();
        let mut redis = redis.clone();
        runtime.spawn(async move {
            struct Reset(Arc<AtomicBool>);
            impl Drop for Reset {
                fn drop(&mut self) {
                    self.0.store(false, Ordering::Release);
                }
            }
            let _reset = Reset(flag);
            let mut pipe = redis::pipe();
            for key in cohorts.keys() {
                pipe.cmd("LRANGE").arg(key).arg(0).arg(127);
            }
            let result: Result<redis::RedisResult<Vec<Vec<String>>>, _> =
                tokio::time::timeout(Duration::from_millis(50), pipe.query_async(&mut redis)).await;
            let Ok(Ok(rows)) = result else { return };
            if rows.len() != cohorts.len() {
                return;
            }
            let at = Instant::now();
            let now_ms = chrono::Utc::now().timestamp_millis();
            let computed: Vec<_> = cohorts
                .into_iter()
                .zip(rows)
                .map(|((key, (strategy, asset)), rows)| {
                    let observations: Vec<RealizedObservation> = rows
                        .into_iter()
                        .take(128)
                        .filter(|s| s.len() <= 4096)
                        .filter_map(|s| serde_json::from_str(&s).ok())
                        .collect();
                    (
                        key,
                        CachedRisk {
                            at,
                            risk: empirical_risk(&observations, chain, &strategy, &asset, now_ms),
                        },
                    )
                })
                .collect();
            if let Ok(mut cache) = cache.lock() {
                cache
                    .entries
                    .retain(|_, c| at.duration_since(c.at) < EXPIRE_AFTER);
                for (key, value) in computed {
                    if !cache.entries.contains_key(&key) && cache.entries.len() >= 128 {
                        let oldest = cache
                            .entries
                            .iter()
                            .min_by_key(|(_, c)| c.at)
                            .map(|(k, _)| k.clone());
                        if let Some(k) = oldest {
                            cache.entries.remove(&k);
                        }
                    }
                    cache.entries.insert(key, value);
                }
            };
        });
    }
}
/// Output slot -> original index. Unknown slots and every payload are retained.
fn available_order(values: &[Option<Objectives>]) -> Option<Vec<usize>> {
    if values.len() > MAX_RANKED {
        return None;
    }
    let positions: Vec<_> = values
        .iter()
        .enumerate()
        .filter_map(|(i, v)| v.map(|o| (i, o)))
        .collect();
    let mut order: Vec<_> = (0..values.len()).collect();
    if positions.len() < 2 {
        return Some(order);
    }
    let optimizer = Nsga2Optimizer::new(Nsga2Config {
        population_size: positions.len(),
        max_candidates: MAX_RANKED,
        generations: 0,
        ..Default::default()
    })
    .ok()?;
    let selected = optimizer
        .select(&positions.iter().map(|(_, o)| *o).collect::<Vec<_>>())
        .ok()?;
    for ((slot, _), chosen) in positions.iter().zip(selected.selected_indices) {
        order[*slot] = positions[chosen].0;
    }
    Some(order)
}
fn apply_permutation<T>(entries: &mut [T], order: &[usize]) {
    let mut current: Vec<_> = (0..entries.len()).collect();
    let mut location = current.clone();
    for (slot, &original) in order.iter().enumerate() {
        let from = location[original];
        entries.swap(slot, from);
        current.swap(slot, from);
        location[current[slot]] = slot;
        location[current[from]] = from;
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn objective(p: f64, r: f64, l: f64) -> Option<Objectives> {
        Some(Objectives {
            expected_profit: p,
            risk_cvar: r,
            latency_ms: l,
        })
    }
    #[test]
    fn risk_order_preserves_unknown_slots_and_ledgers() {
        let values = [
            objective(1., 5., 10.),
            None,
            objective(3., 1., 2.),
            None,
            objective(2., 2., 3.),
        ];
        let order = available_order(&values).unwrap();
        assert_eq!(order, [2, 1, 4, 3, 0]);
        let mut entries = [
            (0, "ledger0"),
            (1, "ledger1"),
            (2, "ledger2"),
            (3, "ledger3"),
            (4, "ledger4"),
        ];
        apply_permutation(&mut entries, &order);
        assert_eq!(
            entries,
            [
                (2, "ledger2"),
                (1, "ledger1"),
                (4, "ledger4"),
                (3, "ledger3"),
                (0, "ledger0")
            ]
        );
    }
    #[test]
    fn missing_and_invalid_history_do_not_invent_risk() {
        assert_eq!(available_order(&[None, None]), Some(vec![0, 1]));
        assert_eq!(
            available_order(&[None, objective(1., 1., 1.)]),
            Some(vec![0, 1])
        );
        assert!(available_order(&[objective(f64::NAN, 1., 1.), objective(1., 1., 1.)]).is_none());
        assert!(available_order(&vec![None; 129]).is_none());
    }
    #[test]
    fn permutations_do_not_lose_payloads() {
        for order in [
            vec![3, 0, 1, 2],
            vec![3, 2, 1, 0],
            vec![0, 1, 2, 3],
            vec![1, 0, 3, 2],
        ] {
            let mut v = vec![0, 1, 2, 3];
            apply_permutation(&mut v, &order);
            assert_eq!(v, order);
        }
    }
}
