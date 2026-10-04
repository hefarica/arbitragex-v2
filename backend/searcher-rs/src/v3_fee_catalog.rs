//! V3 fee catalog — FEE-TIER-AWARE-QUOTING (WO-06).
//!
//! QuoterV2.`quoteExactInputSingle` does NOT receive a pool address: it
//! derives the pool via `factory.getPool(tokenIn, tokenOut, fee)`, so the fee
//! tier (raw pips, uint24) is part of the call identity. Quoting with a tier
//! that has no real pool makes the quoter revert — which surfaced downstream
//! as `v3_quote_unavailable` (a transport-looking label hiding a tier gap).
//! The prior projector default (`fee_bps.unwrap_or(500)`) was exactly that:
//! a blind 0.05% guess.
//!
//! This catalog is the authoritative fee source for every V3 quote:
//!   * `load_from_redis` mirrors `arbx:pool_index_v3:<chain>:<symA>:<symB>`
//!     (the same wire contract the scanner already consumes; populated by
//!     pool_sync_worker from PG — one source of truth, no new PG reader).
//!   * `record_observed` passively reconciles after every successful RPC
//!     quote (covers Redis index lag / desync).
//!   * `resolve` decides Catalog / Mismatch / NotCatalogued; the projector
//!     turns NotCatalogued into a zero-RPC honest rejection instead of a
//!     blind quote (R8).

use std::collections::{BTreeSet, HashMap};
use std::sync::RwLock;
use std::time::{Duration, Instant};

use ethers::types::Address;
use tracing::{debug, warn};

use crate::amm_math::PoolAdmission;
use crate::reserves::V3PoolInfo;

/// Outcome of resolving a quote's fee tier against the catalog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeeResolution {
    /// Pool catalogued at this fee (raw pips) — quote with it.
    Catalog(u32),
    /// Caller offered a fee that differs from the catalog: the CATALOG wins
    /// (counted in `arbx_v3_fee_resolution_total{resolution="mismatch"}`).
    Mismatch { offered: u32, catalog: u32 },
    /// Pool address unknown to the catalog — must NOT be quoted blind.
    NotCatalogued,
}

/// In-memory mirror of the Redis V3 pool index. Thread-safe (`RwLock`);
/// loaded at boot + refreshed on a timer by the scanner, and passively
/// reconciled by `record_observed` after successful RPC quotes.
#[derive(Default)]
pub struct V3FeeCatalog {
    /// pool address → fee tier (raw pips: 100, 500, 3000, 10000).
    by_pool: RwLock<HashMap<Address, u32>>,
    /// Unordered token pair → known fee tiers. The Redis index is
    /// symbol-keyed, so the address-keyed pair side is seeded only by
    /// `record_observed` (post-RPC observations) — a pair with no observed
    /// tiers AND an uncatalogued pool is honestly rejected without RPC.
    by_pair: RwLock<HashMap<(Address, Address), BTreeSet<u32>>>,
    /// CATALOG-HYGIENE-01: the CHAIN's verdict per pool address, recorded from
    /// the `fee()` / `liquidity()` probes. An entry the chain disproved is not
    /// quoted at all, which removes both the wasted call and the fabricated
    /// default (see `resolve`).
    admissions: RwLock<HashMap<Address, AdmissionRecord>>,
}

/// One recorded verdict plus its observation time (the freshness windows below
/// are per-verdict class).
#[derive(Clone, Copy)]
struct AdmissionRecord {
    admission: PoolAdmission,
    recorded_at: Instant,
}

/// Freshness window for an `EmptyPool` verdict: `liquidity()` changes at every
/// block, so "empty" is a market snapshot, never a property. Measured context:
/// the live `0xf6a42a…` (correct tier 3000) answered 0 — it can be funded at any
/// block.
const DEFAULT_EMPTY_POOL_TTL_MS: u64 = 300_000;

/// Freshness window for a `NotAV3Pool` verdict. The observation itself is
/// definitive (the ABI answered, or refused to), but the verdict is deliberately
/// NOT eternal: it is bounded so a mislabelled catalogue row cannot be
/// condemned forever by one chain reading. The point is to stop the per-2s
/// churn, not to freeze a judgement — 1 h removes ~1,800 probes/hour/key.
const DEFAULT_NOT_A_POOL_TTL_MS: u64 = 3_600_000;

/// Cache cap for the verdict ledger (each record is ~40 bytes).
const ADMISSIONS_MAX: usize = 8_192;

/// Pure parse (unit-testable): ms knob → Duration, fail-honest on
/// unset/junk/zero.
fn parse_ttl_ms(raw: Option<String>, default_ms: u64) -> Duration {
    raw.and_then(|v| v.trim().parse::<u64>().ok())
        .filter(|ms| *ms > 0)
        .map(Duration::from_millis)
        .unwrap_or(Duration::from_millis(default_ms))
}

fn empty_pool_ttl() -> Duration {
    static V: std::sync::OnceLock<Duration> = std::sync::OnceLock::new();
    *V.get_or_init(|| {
        parse_ttl_ms(
            std::env::var("ARBX_V3_EMPTY_POOL_TTL_MS").ok(),
            DEFAULT_EMPTY_POOL_TTL_MS,
        )
    })
}

fn not_a_pool_ttl() -> Duration {
    static V: std::sync::OnceLock<Duration> = std::sync::OnceLock::new();
    *V.get_or_init(|| {
        parse_ttl_ms(
            std::env::var("ARBX_V3_NOT_A_POOL_TTL_MS").ok(),
            DEFAULT_NOT_A_POOL_TTL_MS,
        )
    })
}

/// QUOTE-TRUTH-ADMISSION-01: freshness for a `QuoteReverted` verdict. The
/// observation (metadata valid, quote reverted) is behavioural, so it is
/// bounded rather than eternal: a pool that recovers — the token re-syncs, the
/// pool is re-seeded — re-qualifies for quoting after the window instead of
/// staying condemned. Same default as `EmptyPool` because both describe pool
/// state that can change at any block; the knob is separate so an operator can
/// pace them independently.
const DEFAULT_QUOTE_REVERTED_TTL_MS: u64 = 300_000;

fn quote_reverted_ttl() -> Duration {
    static V: std::sync::OnceLock<Duration> = std::sync::OnceLock::new();
    *V.get_or_init(|| {
        parse_ttl_ms(
            std::env::var("ARBX_V3_QUOTE_REVERTED_TTL_MS").ok(),
            DEFAULT_QUOTE_REVERTED_TTL_MS,
        )
    })
}

/// Freshness gate for one verdict class, pure and unit-testable.
fn admission_is_fresh(admission: PoolAdmission, age: Duration) -> bool {
    match admission {
        PoolAdmission::EmptyPool { .. } => age < empty_pool_ttl(),
        PoolAdmission::NotAV3Pool => age < not_a_pool_ttl(),
        // QUOTE-TRUTH-ADMISSION-01: observed behaviour, self-healing window.
        PoolAdmission::QuoteReverted { .. } => age < quote_reverted_ttl(),
        // The chain's `fee()` is an immutable property of a deployed pool, so a
        // proven tier (agreement or correction) does not go stale.
        PoolAdmission::Admitted { .. } | PoolAdmission::TierMismatch { .. } => true,
        // A non-verdict is never stored, so it can never be fresh.
        PoolAdmission::Unprobed => false,
    }
}

/// Increment `arbx_v3_fee_resolution_total{resolution}` (R8). `pub(crate)`
/// so the projector can emit at the decision point (kept next to the enum so
/// the label strings never drift from the metric contract).
pub(crate) fn fee_resolution_metric(resolution: &str) {
    crate::metrics::V3_FEE_RESOLUTION_TOTAL
        .with_label_values(&[resolution])
        .inc();
}

impl V3FeeCatalog {
    pub fn new() -> Self {
        Self::default()
    }

    /// Canonical unordered pair key.
    fn pair_key(token0: Address, token1: Address) -> (Address, Address) {
        if token0 <= token1 {
            (token0, token1)
        } else {
            (token1, token0)
        }
    }

    fn set_pool_gauge(&self) {
        let len = self.by_pool.read().unwrap_or_else(|e| e.into_inner()).len();
        crate::metrics::V3_FEE_CATALOG_POOLS.set(len as i64);
    }

    /// Record a pool + its pair tier. Called after every successful RPC quote
    /// (passive reconciliation — the quoter answering at this tier proves the
    /// tier is real even if the Redis index lags) and directly by tests.
    pub fn record_observed(&self, pool: Address, token0: Address, token1: Address, fee_pips: u32) {
        {
            let mut by_pool = self.by_pool.write().unwrap_or_else(|e| e.into_inner());
            // V3-QUOTE-02: an OBSERVED tier is proven — the QuoterV2 answered at
            // it, which means `factory.getPool` derived a real pool for that
            // exact (t0,t1,fee) triple. It therefore replaces a contradicting
            // catalog entry. The contradiction is logged rather than silently
            // overwritten: that replaced value is precisely the stale tier that
            // kept the pool in the revert bucket, and a silent overwrite would
            // destroy the only witness of it.
            //
            // NOTE (measured 2026-10-03): this heal path can only fire on a
            // SUCCESS, and a wrong tier can never succeed — a pool whose
            // catalogued tier is wrong reverts forever and can never correct
            // itself here. That is why the index-generation fix above, not this
            // hook, is the load-bearing one.
            if let Some(existing) = by_pool.get(&pool) {
                if *existing != fee_pips {
                    warn!(
                        event = "v3_fee_catalog.observed_tier_corrects_catalog",
                        pool = %pool,
                        catalog = *existing,
                        observed = fee_pips,
                        "a successful on-chain quote proved a different tier — catalog corrected"
                    );
                }
            }
            by_pool.insert(pool, fee_pips);
        }
        {
            let mut by_pair = self.by_pair.write().unwrap_or_else(|e| e.into_inner());
            by_pair
                .entry(Self::pair_key(token0, token1))
                .or_default()
                .insert(fee_pips);
        }
        self.set_pool_gauge();
    }

    /// Catalog fee for a pool address (raw pips).
    pub fn fee_for_pool(&self, pool: Address) -> Option<u32> {
        self.by_pool
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .get(&pool)
            .copied()
    }

    /// Known fee tiers for an unordered token pair.
    pub fn tiers_for_pair(&self, token0: Address, token1: Address) -> BTreeSet<u32> {
        self.by_pair
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .get(&Self::pair_key(token0, token1))
            .cloned()
            .unwrap_or_default()
    }

    /// Record the CHAIN's verdict for one catalogue entry (CATALOG-HYGIENE-01).
    ///
    /// `Unprobed` is deliberately NOT stored: a missing answer is not evidence
    /// and must never turn into a lasting judgement (R8). Every other verdict is
    /// stored with its observation time and honoured by `resolve` while fresh.
    pub fn record_admission(&self, pool: Address, admission: PoolAdmission) {
        if matches!(admission, PoolAdmission::Unprobed) {
            return;
        }
        {
            let mut g = self.admissions.write().unwrap_or_else(|e| e.into_inner());
            if g.len() >= ADMISSIONS_MAX && !g.contains_key(&pool) {
                // Drop the expired first; if that is not enough, drop arbitrary
                // records. Bounded memory, no unbounded growth in a long-lived
                // process.
                let now = Instant::now();
                g.retain(|_, r| admission_is_fresh(r.admission, now.duration_since(r.recorded_at)));
                while g.len() >= ADMISSIONS_MAX {
                    let Some(k) = g.keys().next().copied() else {
                        break;
                    };
                    g.remove(&k);
                }
            }
            g.insert(
                pool,
                AdmissionRecord {
                    admission,
                    recorded_at: Instant::now(),
                },
            );
        }
        debug!(
            event = "v3_fee_catalog.admission_recorded",
            pool = %pool,
            verdict = ?admission,
        );
    }

    /// The chain's verdict for `pool`, if one was recorded and is still fresh.
    /// Expired verdicts read as `None` — the entry falls back to its catalogue
    /// value instead of staying condemned on stale evidence.
    pub fn admission_for(&self, pool: Address) -> Option<PoolAdmission> {
        let g = self.admissions.read().unwrap_or_else(|e| e.into_inner());
        let rec = g.get(&pool)?;
        admission_is_fresh(
            rec.admission,
            Instant::now().duration_since(rec.recorded_at),
        )
        .then_some(rec.admission)
    }

    /// Number of recorded verdicts (tests + diagnostics).
    pub fn admission_count(&self) -> usize {
        self.admissions
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .len()
    }

    /// Resolve the fee to quote `pool` with, given the caller's offered fee
    /// (`PoolRef::fee_bps`, also raw pips for V3 legs). The catalog always
    /// wins; a pool missing from the catalog is `NotCatalogued` (never a
    /// blind default — that is what this module exists to fix).
    ///
    /// CATALOG-HYGIENE-01: two chain-verified verdicts change the outcome:
    ///   * `NotAV3Pool` — measured on the `fee_tier = 30` rows, which are
    ///     Uniswap V2 pairs (`fee()` reverts, `getReserves()` answers). Such an
    ///     entry is NOT quotable: it resolves `NotCatalogued`, the projector
    ///     emits `v3_pool_not_catalogued` with ZERO RPC, and the key stops being
    ///     re-probed every negative-TTL window. No tier is fabricated to keep
    ///     it alive (RULE 00).
    ///   * `EmptyPool` — a real pool with `liquidity() == 0`: same zero-RPC
    ///     rejection while the verdict is fresh, because the call could only
    ///     ever revert.
    ///   * `TierMismatch` — the chain's `fee()` is immutable and outranks the
    ///     catalogue: the measured `0x464bd7…` row says 100 while `fee()`
    ///     answers 10000, and quoting at 100 makes QuoterV2 derive a pool that
    ///     does not exist.
    pub fn resolve(&self, pool: Address, offered: Option<u32>) -> FeeResolution {
        let admission = self.admission_for(pool);
        match admission {
            Some(PoolAdmission::NotAV3Pool) => {
                fee_resolution_metric("not_admitted_not_a_v3_pool");
                return FeeResolution::NotCatalogued;
            }
            Some(PoolAdmission::EmptyPool { .. }) => {
                fee_resolution_metric("not_admitted_empty_pool");
                return FeeResolution::NotCatalogued;
            }
            // QUOTE-TRUTH-ADMISSION-01: metadata valid, quote reverted. The
            // entry is not quotable — no tier is fabricated to keep it alive,
            // and the ~1,495 quote attempts/hour it generated stop happening.
            Some(PoolAdmission::QuoteReverted { .. }) => {
                fee_resolution_metric("not_admitted_quote_reverted");
                return FeeResolution::NotCatalogued;
            }
            _ => {}
        }
        match self.fee_for_pool(pool) {
            Some(catalog) => {
                if let Some(PoolAdmission::TierMismatch { onchain_fee, .. }) = admission {
                    fee_resolution_metric("chain_corrected_tier");
                    return FeeResolution::Catalog(onchain_fee);
                }
                match offered {
                    Some(offered) if offered != catalog => {
                        FeeResolution::Mismatch { offered, catalog }
                    }
                    _ => FeeResolution::Catalog(catalog),
                }
            }
            None => FeeResolution::NotCatalogued,
        }
    }

    /// Number of pools currently catalogued.
    pub fn pool_count(&self) -> usize {
        self.by_pool.read().unwrap_or_else(|e| e.into_inner()).len()
    }

    /// Parse ONE `arbx:pool_index_v3` payload (a JSON array of `V3PoolInfo`)
    /// and merge its entries into `by_pool`. Returns the malformed-entry count
    /// for this payload (R8: unparseable / non-address entries are counted and
    /// skipped — never fabricated, never fatal).
    ///
    /// CATALOG-BACKFILL-01 (2026-09-17): extracted from `load_from_redis`
    /// so the wire-contract rejection cases (legacy `"address"`-field entries,
    /// `fee_bps: null`, missing fields) are unit-testable without Redis.
    /// EVIDENCE (Redis dump 2026-09-17): 93 of 186 keys hold pre-WO-06 entries
    /// like `[{"address":"0x…","fee_bps":30}]` — `V3PoolInfo` requires
    /// `pool_addr`, so serde rejects the whole array → key skipped → every pool
    /// in it resolves `NotCatalogued`.
    ///
    /// CATALOG-CANONICAL-CONFLICT-01: the production path no longer merges a
    /// single payload blindly — `load_from_redis` collects every statement first
    /// (`ingest_canonical_payloads`) so that two canonical keys contradicting
    /// each other cannot be resolved by arrival order. This blind single-payload
    /// merge is kept as the TEST-only primitive the pre-existing ingest tests
    /// pin, and it is gated to `cfg(test)` so the compiler still proves nothing
    /// in the library calls it.
    #[cfg(test)]
    fn ingest_index_payload(&self, json: &str) -> usize {
        let (malformed, entries) = Self::parse_index_payload_entries(json);
        let mut by_pool = self.by_pool.write().unwrap_or_else(|e| e.into_inner());
        for (addr, fee_bps) in entries {
            by_pool.insert(addr, fee_bps);
        }
        malformed
    }

    /// Parse ONE `arbx:pool_index_v3` payload into `(malformed_entries, entries)`
    /// WITHOUT touching the catalog: the canonical decision needs every statement
    /// in hand before one of them becomes the tier.
    fn parse_index_payload_entries(json: &str) -> (usize, Vec<(Address, u32)>) {
        let pools: Vec<V3PoolInfo> = match serde_json::from_str(json) {
            Ok(p) => p,
            Err(_) => return (1, Vec::new()),
        };
        let mut malformed = 0usize;
        let mut entries = Vec::with_capacity(pools.len());
        for info in pools {
            match info.pool_addr.parse::<Address>() {
                Ok(addr) => entries.push((addr, info.fee_bps)),
                Err(_) => malformed += 1,
            }
        }
        (malformed, entries)
    }

    /// The legacy generation (`bps`) read as a per-pool WITNESS map.
    ///
    /// "First statement wins" via `or_insert` is only deterministic on a SORTED
    /// key set, so the caller sorts the legacy keys — an unsorted SCAN feeding
    /// `or_insert` would be the very defect this module is closing.
    fn legacy_witness_bps(payloads: &[(String, String)]) -> HashMap<Address, u32> {
        let mut out: HashMap<Address, u32> = HashMap::new();
        for (_, json) in payloads {
            let (_, entries) = Self::parse_index_payload_entries(json);
            for (addr, bps) in entries {
                out.entry(addr).or_insert(bps);
            }
        }
        out
    }

    /// CATALOG-CANONICAL-CONFLICT-01 — merge the CANONICAL generation under the
    /// deterministic conflict rule.
    ///
    /// `payloads` is `(key, json)` in whatever order the SCAN produced them; the
    /// outcome is invariant to that order, which is the point of the function.
    /// `legacy_witness` carries the OLDER generation's value in BASIS POINTS: an
    /// independent writer, hence evidence rather than another competitor.
    /// Returns `(malformed_entries, canonical_conflicts)`.
    pub(crate) fn ingest_canonical_payloads(
        &self,
        payloads: &[(String, String)],
        legacy_witness: &HashMap<Address, u32>,
    ) -> (usize, usize) {
        let mut statements: HashMap<Address, Vec<CanonicalStatement>> = HashMap::new();
        let mut malformed = 0usize;
        for (key, json) in payloads {
            let (m, entries) = Self::parse_index_payload_entries(json);
            malformed += m;
            for (addr, tier_pips) in entries {
                statements
                    .entry(addr)
                    .or_default()
                    .push(CanonicalStatement {
                        key: key.clone(),
                        tier_pips,
                    });
            }
        }
        let mut conflicts = 0usize;
        let mut winners: Vec<(Address, u32)> = Vec::with_capacity(statements.len());
        for (addr, list) in &statements {
            let (tier, disputed) = pick_canonical_tier(list, legacy_witness.get(addr).copied());
            if disputed {
                conflicts += 1;
                let mut tiers: Vec<u32> = list.iter().map(|s| s.tier_pips).collect();
                tiers.sort_unstable();
                tiers.dedup();
                let mut keys: Vec<&str> = list.iter().map(|s| s.key.as_str()).collect();
                keys.sort_unstable();
                warn!(
                    event = "v3_fee_catalog.canonical_key_contradiction",
                    pool = %addr,
                    tiers = ?tiers,
                    keys = ?keys,
                    winner = tier,
                    "two CANONICAL keys describe the same pool with different pips — \
                     deterministic winner (legacy witness, else the writer-form key), \
                     never SCAN order"
                );
            }
            winners.push((*addr, tier));
        }
        {
            let mut by_pool = self.by_pool.write().unwrap_or_else(|e| e.into_inner());
            for (addr, tier) in winners {
                by_pool.insert(addr, tier);
            }
        }
        (malformed, conflicts)
    }

    /// Merge the Redis `arbx:pool_index_v3:<chain>:*` index into `by_pool`
    /// (SCAN + per-key GET — same access pattern as price_worker's pool-index
    /// scan). Entries never disappear on refresh: a pool dropped by Redis
    /// while still observable would otherwise flap between catalogued and
    /// rejected. Returns the post-merge pool count. A Redis error returns
    /// `Err` — callers keep the prior snapshot (R8, non-fatal by design).
    pub async fn load_from_redis(
        &self,
        redis: &mut redis::aio::ConnectionManager,
        chain_id: u64,
    ) -> anyhow::Result<usize> {
        let pattern = format!("arbx:pool_index_v3:{}:*", chain_id);
        let mut keys = Vec::new();
        {
            let mut iter: redis::AsyncIter<String> = redis::cmd("SCAN")
                .cursor_arg(0)
                .arg("MATCH")
                .arg(&pattern)
                .arg("COUNT")
                .arg(500)
                .clone()
                .iter_async(redis)
                .await
                .map_err(|e| anyhow::anyhow!("v3 fee catalog SCAN failed: {e}"))?;
            while let Some(k) = iter.next_item().await {
                keys.push(k);
            }
        }
        // `iter` (and its ConnectionManager borrow) dropped before the GETs.

        let mut malformed = 0usize;
        // V3-QUOTE-02 (2026-10-03): two-pass, deterministic ingest.
        //
        // The `arbx:pool_index_v3` index carries TWO generations of the same
        // wire field `fee_bps`:
        //   * CANONICAL keys — the exact form this crate writes
        //     (`arbx:pool_index_v3:<chain>:<sym_lo>:<sym_hi>`, both symbols
        //     lowercased, see `key_pool_index_v3`) whose value is RAW PIPS.
        //   * LEGACY keys — mixed-case symbols, written before the pips
        //     migration, whose value is the OLD unit, BASIS POINTS
        //     (1/5/30/100 instead of 100/500/3000/10000).
        //
        // Measured on the live index 2026-10-03: 573 keys, 70 legacy, and 44
        // pools described by BOTH generations with tiers differing by exactly
        // ×100 in 44/44 cases. The pre-fix single-pass ingest let SCAN order
        // decide the tier of those 44 pools, so QuoterV2 could be asked for a
        // fee the factory has no pool for — measured:
        // `getPool(WETH, XPR, 100)` → 0x0 while `getPool(WETH, XPR, 10000)` →
        // the very pool address the catalog held. The sub-call then reverted
        // and the candidate died as `v3_quote_unavailable`.
        //
        // Canonical keys win, unconditionally and independent of scan order.
        // Legacy keys only fill pools the canonical generation does not
        // describe, and every contradiction is COUNTED (R8: resolved
        // deterministically, never silently).
        //
        // CATALOG-CANONICAL-CONFLICT-01 (2026-10-04): that rule closes the
        // canonical/LEGACY class. The canonical/CANONICAL class was still open —
        // two canonical keys contradicting each other went through a blind
        // insert, so SCAN order picked the tier again. It is closed below by
        // `pick_canonical_tier`, and the same gauge counts both classes.
        let mut canonical_keys: Vec<String> = Vec::new();
        let mut legacy_keys: Vec<String> = Vec::new();
        for key in keys {
            if is_canonical_index_key(&key, chain_id) {
                canonical_keys.push(key);
            } else {
                legacy_keys.push(key);
            }
        }

        // CATALOG-CANONICAL-CONFLICT-01 (2026-10-04): BOTH generations are read
        // BEFORE anything is merged, because there is a third contradiction class
        // this loader used to miss — CANONICAL vs CANONICAL:
        //
        //   arbx:pool_index_v3:1:weth:wfc = 100   (pips)  ← the writer-form key
        //   arbx:pool_index_v3:1:wfc:weth = 10000 (pips)  ← foreign symbol order
        //   arbx:pool_index_v3:1:WETH:WFC = 100   (bps = 10000 pips, legacy)
        //
        // With one blind `by_pool.insert` per canonical key, the surviving tier
        // was decided by SCAN order — the exact mechanism V3-QUOTE-02 removed for
        // the canonical/legacy class, left open INSIDE the canonical generation.
        // MEASURED on the live index 2026-10-04 (589 keys, 565 with payload): 694
        // canonical pools, 124 legacy pools, **1** pool described by two
        // contradictory canonical statements (`0x6f9beaac…` at 100 and 10000) and
        // 0 legacy-legacy contradictions. On-chain `fee()` of that pool is 10000
        // (cast call, ethereum-rpc.publicnode.com), so the legacy witness is also
        // the truth here and the key carrying 100 is a canonical key holding a
        // BPS value — the ×100 residue of the pips migration.
        let mut canonical_payloads: Vec<(String, String)> =
            Vec::with_capacity(canonical_keys.len());
        for key in &canonical_keys {
            if let Some(json) = Self::get_index_payload(redis, key).await? {
                canonical_payloads.push((key.clone(), json));
            }
        }
        // Sorted: both the witness map and the gap-fill below resolve ties with a
        // "first statement wins" rule, which is only deterministic on a sorted
        // key set.
        legacy_keys.sort();
        let mut legacy_payloads: Vec<(String, String)> = Vec::with_capacity(legacy_keys.len());
        for key in &legacy_keys {
            if let Some(json) = Self::get_index_payload(redis, key).await? {
                legacy_payloads.push((key.clone(), json));
            }
        }
        let legacy_witness = Self::legacy_witness_bps(&legacy_payloads);
        let (canonical_malformed, canonical_conflicts) =
            self.ingest_canonical_payloads(&canonical_payloads, &legacy_witness);
        malformed += canonical_malformed;

        let mut conflicts = 0usize;
        for (key, json) in &legacy_payloads {
            let (m, c) = self.ingest_index_payload_legacy(json);
            malformed += m;
            conflicts += c;
            if c > 0 {
                warn!(
                    event = "v3_fee_catalog.legacy_key_contradiction",
                    chain_id,
                    key = %key,
                    contradictions = c,
                    "legacy pool_index_v3 key contradicts the canonical generation — \
                     canonical tier kept (R8: deterministic, never last-wins)"
                );
            }
        }
        // The gauge counts BOTH contradiction classes resolved deterministically:
        // the canonical/legacy one (as before) and — new here — the
        // canonical/canonical one, which used to be invisible to it.
        crate::metrics::V3_FEE_CATALOG_CONFLICTS.set((conflicts + canonical_conflicts) as i64);
        if canonical_conflicts > 0 {
            warn!(
                event = "v3_fee_catalog.canonical_key_contradictions",
                chain_id,
                canonical_conflicts,
                canonical_keys = canonical_keys.len(),
                "two CANONICAL keys describe the same pool with different pips — \
                 deterministic winner, never SCAN order (operator action: delete \
                 the stale key at the source)"
            );
        }
        if conflicts > 0 {
            warn!(
                event = "v3_fee_catalog.tier_contradictions",
                chain_id,
                conflicts,
                canonical_keys = canonical_keys.len(),
                legacy_keys = legacy_keys.len(),
                "index carries two generations of `fee_bps` (bps vs raw pips) — \
                 operator action: repair the stale keys at the source"
            );
        }
        if malformed > 0 {
            warn!(
                event = "v3_fee_catalog.entries_malformed",
                chain_id,
                malformed,
                "skipped malformed pool_index_v3 entries (R8: no fabricated tiers)"
            );
        }
        self.set_pool_gauge();
        Ok(self.pool_count())
    }

    /// GET one index key. Split out so the two-pass loader shares one error
    /// mapping (a Redis failure stays fatal for the whole refresh: callers keep
    /// the prior snapshot rather than a half-loaded one).
    async fn get_index_payload(
        redis: &mut redis::aio::ConnectionManager,
        key: &str,
    ) -> anyhow::Result<Option<String>> {
        redis::AsyncCommands::get(redis, key)
            .await
            .map_err(|e| anyhow::anyhow!("v3 fee catalog GET {key} failed: {e}"))
    }

    /// Ingest ONE LEGACY (non-canonical-key) payload. Same wire contract, OLD
    /// unit. Never overwrites a canonical tier: it only fills pools the
    /// canonical generation does not describe, and returns
    /// `(malformed_entries, contradictions)`.
    ///
    /// The contradiction count is the operator's signal that the index is still
    /// carrying stale data; it is deliberately NOT a reason to prefer the
    /// legacy value — a tier this crate cannot disambiguate must never be
    /// guessed (that guess is what produced the `v3_quote_unavailable` flood).
    pub(crate) fn ingest_index_payload_legacy(&self, json: &str) -> (usize, usize) {
        let pools: Vec<V3PoolInfo> = match serde_json::from_str(json) {
            Ok(p) => p,
            Err(_) => return (1, 0),
        };
        let mut malformed = 0usize;
        let mut conflicts = 0usize;
        let mut by_pool = self.by_pool.write().unwrap_or_else(|e| e.into_inner());
        for info in pools {
            match info.pool_addr.parse::<Address>() {
                Ok(addr) => match by_pool.get(&addr) {
                    Some(&existing) => {
                        if existing != info.fee_bps {
                            conflicts += 1;
                        }
                    }
                    None => {
                        by_pool.insert(addr, info.fee_bps);
                    }
                },
                Err(_) => malformed += 1,
            }
        }
        (malformed, conflicts)
    }
}

/// Is `key` in the exact form `key_pool_index_v3` writes?
///
/// The canonical form is `arbx:pool_index_v3:<chain>:<sym_lo>:<sym_hi>` with
/// BOTH symbols lowercased. Every other shape in the live index belongs to the
/// pre-pips-migration writer whose `fee_bps` is in basis points — a different
/// unit behind the same field name, which is precisely the ambiguity this
/// predicate lets the loader resolve without guessing.
pub(crate) fn is_canonical_index_key(key: &str, chain_id: u64) -> bool {
    let prefix = format!("arbx:pool_index_v3:{chain_id}:");
    let Some(rest) = key.strip_prefix(&prefix) else {
        return false;
    };
    let mut parts = rest.split(':');
    let (Some(a), Some(b), None) = (parts.next(), parts.next(), parts.next()) else {
        return false;
    };
    !a.is_empty() && !b.is_empty() && a == a.to_lowercase() && b == b.to_lowercase()
}

/// CATALOG-CANONICAL-CONFLICT-01: ONE canonical-key statement about a pool.
#[derive(Debug, Clone)]
pub(crate) struct CanonicalStatement {
    /// The canonical index key that carried it.
    pub(crate) key: String,
    /// Raw pips, as the canonical generation writes them.
    pub(crate) tier_pips: u32,
}

/// Does `key` carry its two symbols in the order `key_pool_index_v3` writes them
/// (`<sym_lo>:<sym_hi>`, that writer's own sort)? A key that violates it was not
/// produced by the current writer — it is a leftover or a hand-written key.
pub(crate) fn key_follows_writer_order(key: &str) -> bool {
    let mut it = key.rsplit(':');
    let (Some(hi), Some(lo)) = (it.next(), it.next()) else {
        return false;
    };
    lo <= hi
}

/// Deterministic tier for a pool described by one or more CANONICAL statements.
///
/// Order-invariant BY CONSTRUCTION — the order of `statements` never decides:
///   1. one distinct tier → that tier, no conflict;
///   2. ≥2 distinct tiers → the tier the LEGACY generation corroborates
///      (`pips == bps * 100`), because an independent writer agreeing outranks a
///      single writer's claim;
///   3. no witness → the tier carried by the key in the WRITER's own symbol
///      order, else the lexicographically smallest key.
///
/// Case 3 is a stable choice, NOT a claim of truth: two canonical keys that
/// contradict with no independent witness cannot be disambiguated from the index
/// alone, so the outcome is deterministic, COUNTED and logged, and the chain
/// admission probe (`resolve` → `TierMismatch` → `Catalog(onchain_fee)`) is what
/// corrects a tier the catalog could not decide.
/// Returns `(winner_pips, was_disputed)`.
pub(crate) fn pick_canonical_tier(
    statements: &[CanonicalStatement],
    legacy_bps: Option<u32>,
) -> (u32, bool) {
    let mut distinct: Vec<u32> = statements.iter().map(|s| s.tier_pips).collect();
    distinct.sort_unstable();
    distinct.dedup();
    let Some(&only) = distinct.first() else {
        return (0, false);
    };
    if distinct.len() == 1 {
        return (only, false);
    }
    if let Some(bps) = legacy_bps {
        let witness = bps.saturating_mul(100);
        if distinct.contains(&witness) {
            return (witness, true);
        }
    }
    let writer_form = statements
        .iter()
        .filter(|s| key_follows_writer_order(&s.key))
        .min_by_key(|s| s.key.as_str());
    let pick = writer_form.or_else(|| statements.iter().min_by_key(|s| s.key.as_str()));
    (pick.map(|s| s.tier_pips).unwrap_or(only), true)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::amm_math::{encode_quote_calldata, V3QuoteRequest};
    use ethers::types::{Address, U256};

    fn addr(n: u64) -> Address {
        Address::from_low_u64_be(n)
    }

    // ── Catalog resolution semantics ─────────────────────────────────────────

    #[test]
    fn resolve_catalog_mismatch_not_catalogued() {
        let c = V3FeeCatalog::new();
        c.record_observed(addr(1), addr(0xA), addr(0xB), 3000);
        assert_eq!(c.resolve(addr(1), None), FeeResolution::Catalog(3000));
        assert_eq!(c.resolve(addr(1), Some(3000)), FeeResolution::Catalog(3000));
        assert_eq!(
            c.resolve(addr(1), Some(100)),
            FeeResolution::Mismatch {
                offered: 100,
                catalog: 3000
            }
        );
        assert_eq!(c.resolve(addr(2), Some(500)), FeeResolution::NotCatalogued);
        assert_eq!(c.fee_for_pool(addr(1)), Some(3000));
        assert_eq!(c.fee_for_pool(addr(2)), None);
        assert_eq!(c.pool_count(), 1);
    }

    #[test]
    fn tiers_for_pair_is_order_insensitive() {
        let c = V3FeeCatalog::new();
        // Register the pair in BOTH argument orders — the key is canonical.
        c.record_observed(addr(1), addr(0xB), addr(0xA), 500);
        c.record_observed(addr(2), addr(0xA), addr(0xB), 3000);
        let tiers = c.tiers_for_pair(addr(0xA), addr(0xB));
        assert_eq!(tiers, BTreeSet::from([500, 3000]));
        assert_eq!(c.tiers_for_pair(addr(0xB), addr(0xA)), tiers);
        assert!(
            c.tiers_for_pair(addr(0xA), addr(0xC)).is_empty(),
            "unknown pair must have no tiers (R8)"
        );
    }

    // ── CATALOG-BACKFILL-01 (2026-09-17): wire-contract rejection cases ──────
    //
    // Anchored to the REAL Redis dump of 2026-09-17 (186 keys, 93 malformed):
    // every malformed payload must count as malformed and contribute NOTHING
    // to the catalog (R8: no fabricated tiers), while a canonical payload
    // populates `by_pool` with its exact pips.

    const WETH_POOL_3000: &str = "0x0000000000000000000000000000000000000001";

    #[test]
    fn catalog_backfill_null_fee_tier_is_malformed_and_not_catalogued() {
        let c = V3FeeCatalog::new();
        // (a) null fee_bps → serde rejects the payload → malformed, catalog untouched.
        let malformed = c.ingest_index_payload(
            r#"[{"pool_addr":"0x0000000000000000000000000000000000000009","fee_bps":null}]"#,
        );
        assert_eq!(malformed, 1, "null fee_bps payload must count as malformed");
        assert_eq!(c.pool_count(), 0, "null fee_bps must NOT enter the catalog");
    }

    #[test]
    fn catalog_backfill_legacy_address_field_is_malformed_and_not_catalogued() {
        let c = V3FeeCatalog::new();
        // Legacy pre-WO-06 schema (`"address"` instead of `pool_addr`) — the
        // exact shape of the 93 malformed keys observed in Redis.
        let malformed = c.ingest_index_payload(
            r#"[{"address":"0xaea3df60e99c4726abc1e7dd9a2fa570e4eed638","fee_bps":30}]"#,
        );
        assert_eq!(
            malformed, 1,
            "legacy address-field payload must be malformed"
        );
        assert_eq!(
            c.pool_count(),
            0,
            "legacy entries must NOT enter the catalog"
        );
    }

    #[test]
    fn catalog_backfill_canonical_payload_enters_with_exact_pips() {
        let c = V3FeeCatalog::new();
        let malformed = c.ingest_index_payload(&format!(
            r#"[{{"pool_addr":"{WETH_POOL_3000}","fee_bps":3000}}]"#
        ));
        assert_eq!(malformed, 0);
        assert_eq!(c.pool_count(), 1);
        assert_eq!(c.fee_for_pool(addr(1)), Some(3000));
        // A garbage fee value in a PARSEABLE payload still enters as-is — the
        // reader never second-guesses tiers; the writer-side on-chain
        // resolution (pool_sync_worker) is what fixes bad values at the source.
        let _ = c.ingest_index_payload(
            r#"[{"pool_addr":"0x0000000000000000000000000000000000000002","fee_bps":30}]"#,
        );
        assert_eq!(c.fee_for_pool(addr(2)), Some(30));
    }

    // ── T1: PIN ENCODING — fee word is raw pips, ABI-padded to 32 bytes ──────
    //
    // No-regression anchor for the bps-caused-reverts precedent: the calldata
    // is `selector || tokenIn || tokenOut || amountIn || fee || sqrtLimit`
    // (the single struct argument encodes inline — 5 words, no offset word),
    // so the fee word sits at bytes [100..132].

    #[test]
    fn t1_pin_encoding_fee_words_are_raw_pips() {
        let req = |fee: u32| V3QuoteRequest {
            pool_addr: addr(1),
            token_in: addr(2),
            token_out: addr(3),
            amount_in: U256::from(1_000u64),
            fee_bps: fee,
        };
        let cd3000 = encode_quote_calldata(&req(3000)).unwrap();
        assert_eq!(cd3000.len(), 164, "selector + 5 inline words");
        let mut expect_3000 = [0u8; 32];
        expect_3000[30] = 0x0b;
        expect_3000[31] = 0xb8; // 3000 = 0x0bb8
        assert_eq!(
            &cd3000[100..132],
            &expect_3000,
            "fee=3000 must encode as 0x0bb8"
        );
        let cd5 = encode_quote_calldata(&req(5)).unwrap();
        let mut expect_5 = [0u8; 32];
        expect_5[31] = 0x05; // 5 = 0x05
        assert_eq!(&cd5[100..132], &expect_5, "fee=5 must encode as 0x05");
    }

    // ── T7: ANCHORED VECTOR CAST — WETH/USDC (500 + 3000 tiers) ──────────────
    //
    // Anchor derivation (2026-09-17):
    //   * Selector 0xc6a5026a verified EXTERNALLY against the 4byte /
    //     openchain signature registries for
    //     quoteExactInputSingle((address,address,uint256,uint24,uint160)).
    //   * Words are canonical ABI for the inline struct argument:
    //     WETH  = 0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2,
    //     USDC  = 0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48,
    //     amountIn = 1e18 = 0x0de0b6b3a7640000, sqrtPriceLimitX96 = 0.
    //   * 500 and 3000 are the two real WETH/USDC mainnet pool tiers; the
    //     pool address is NOT part of the calldata (the quoter derives it
    //     from the factory — the root cause this module fixes).

    #[test]
    fn t7_anchored_quoterv2_calldata_weth_usdc() {
        let weth: Address = "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"
            .parse()
            .unwrap();
        let usdc: Address = "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"
            .parse()
            .unwrap();
        for (fee, fee_word) in [
            (
                500u32,
                "00000000000000000000000000000000000000000000000000000000000001f4",
            ),
            (
                3000u32,
                "0000000000000000000000000000000000000000000000000000000000000bb8",
            ),
        ] {
            let req = V3QuoteRequest {
                pool_addr: Address::zero(),
                token_in: weth,
                token_out: usdc,
                amount_in: U256::from(1_000_000_000_000_000_000u64),
                fee_bps: fee,
            };
            let cd = encode_quote_calldata(&req).unwrap();
            let expected = format!(
                "c6a5026a\
                 000000000000000000000000c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2\
                 000000000000000000000000a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48\
                 0000000000000000000000000000000000000000000000000de0b6b3a7640000\
                 {fee_word}\
                 0000000000000000000000000000000000000000000000000000000000000000"
            );
            assert_eq!(
                hex::encode(cd.as_ref()),
                expected,
                "anchored calldata mismatch for fee={fee}"
            );
        }
    }

    // ── V3-QUOTE-02 (2026-10-03): fee-index generation disambiguation ────────
    //
    // Everything below is anchored to the live `arbx:pool_index_v3:1:*` index
    // as measured on 2026-10-03 (573 keys, 70 legacy, 44 contradictory pools)
    // and to the on-chain / PostgreSQL truth fetched for the same addresses:
    //
    //   pool 0x540a6b18…5074  (WETH/XPR)  PG fee_tier=500   IUniswapV3Pool.fee()=500
    //       canonical key `arbx:pool_index_v3:1:weth:xpr`        → 500
    //       legacy    key `arbx:pool_index_v3:1:WETH:XPR`        → 5     (bps)
    //   pool 0x464bd7e6…76b3  (WETH/XPR)  PG fee_tier=100   IUniswapV3Pool.fee()=10000
    //       BOTH generations                                      → 100   (ambiguous, NOT repairable here)
    //
    // The load-bearing assertion is the first one: the tier the catalog hands to
    // QuoterV2 must be the one `IUniswapV3Factory.getPool(t0,t1,fee)` has a real
    // pool for. Measured on-chain: getPool(WETH,XPR,100) → 0x0 (reverts) while
    // getPool(WETH,XPR,10000) → 0x464b…76b3.

    #[test]
    fn canonical_index_key_matches_only_the_writer_form() {
        // The exact form `key_pool_index_v3` produces.
        assert!(is_canonical_index_key("arbx:pool_index_v3:1:weth:xpr", 1));
        assert!(is_canonical_index_key("arbx:pool_index_v3:1:cbeth:weth", 1));
        // A symbol with no case is still canonical.
        assert!(is_canonical_index_key("arbx:pool_index_v3:1:1:usdt", 1));
        // Legacy generation: uppercase anywhere in a symbol.
        assert!(!is_canonical_index_key("arbx:pool_index_v3:1:WETH:XPR", 1));
        assert!(!is_canonical_index_key(
            "arbx:pool_index_v3:1:WETH:cbETH",
            1
        ));
        assert!(!is_canonical_index_key(
            "arbx:pool_index_v3:1:weth:USDT.C",
            1
        ));
        // Wrong chain, wrong shape, wrong prefix.
        assert!(!is_canonical_index_key("arbx:pool_index_v3:2:weth:xpr", 1));
        assert!(!is_canonical_index_key("arbx:pool_index_v3:1:weth", 1));
        assert!(!is_canonical_index_key("arbx:pool_index_v3:1:a:b:c", 1));
        assert!(!is_canonical_index_key("arbx:pool_index:1:weth:xpr", 1));
        assert!(!is_canonical_index_key("arbx:pool_index_v3:1::weth", 1));
    }

    /// The production failure, reproduced at the catalog level: two sources
    /// describe pool 0x540a6b… with 500 (canonical, = PG = on-chain) and 5
    /// (legacy bps). The catalog MUST resolve 500 no matter what order the two
    /// payloads arrive in — Scan order deciding a tier is what made QuoterV2
    /// ask for a pool that does not exist.
    #[test]
    fn canonical_tier_wins_over_legacy_regardless_of_ingest_order() {
        const POOL: &str = "0x540a6b1825989d321b8554e6ae0d6a775de65074";
        let canonical = format!(r#"[{{"pool_addr":"{POOL}","fee_bps":500}}]"#);
        let legacy = format!(r#"[{{"pool_addr":"{POOL}","fee_bps":5}}]"#);

        // Order A: canonical first, legacy second (the production load order).
        let a = V3FeeCatalog::new();
        a.ingest_index_payload(&canonical);
        let (malformed, conflicts) = a.ingest_index_payload_legacy(&legacy);
        assert_eq!(malformed, 0);
        assert_eq!(conflicts, 1, "the ×100 contradiction must be counted");
        assert_eq!(
            a.resolve(addr_from(POOL), None),
            FeeResolution::Catalog(500),
            "canonical pips tier must win"
        );

        // Order B: legacy first, canonical second — same verdict. The pre-fix
        // last-wins merge produced 5 here.
        let b = V3FeeCatalog::new();
        let (_, conflicts_b) = b.ingest_index_payload_legacy(&legacy);
        assert_eq!(conflicts_b, 0, "nothing to contradict yet");
        b.ingest_index_payload(&canonical);
        assert_eq!(
            b.resolve(addr_from(POOL), None),
            FeeResolution::Catalog(500),
            "canonical pips tier must win in either arrival order"
        );
    }

    /// CATALOG-CANONICAL-CONFLICT-01 (VERIF-01 F1): the live pool
    /// `0x6f9beaac7d042a50008db301833abbe92e4f3a8f` is described by TWO canonical
    /// keys carrying different raw pips — `weth:wfc` = 100 and `wfc:weth` = 10000
    /// — and by a LEGACY key whose 100 bps is the same 10000 pips once the unit
    /// is normalised. The on-chain fee of that pool is 10000, measured:
    /// `cast call 0x6f9beaac… fee()(uint24) --rpc-url https://ethereum-rpc.publicnode.com`
    /// → `10000 [1e4]`.
    ///
    /// What this test PINS is the ORDER INVARIANCE: the same two payloads
    /// ingested in opposite orders must land on the same tier, and the
    /// contradiction must be counted in both. A single-order test does not prove
    /// determinism.
    #[test]
    fn canonical_canonical_conflict_is_resolved_by_the_legacy_witness_in_both_orders() {
        const POOL: &str = "0x6f9beaac7d042a50008db301833abbe92e4f3a8f";
        let writer_form = (
            "arbx:pool_index_v3:1:weth:wfc".to_string(),
            format!(r#"[{{"pool_addr":"{POOL}","fee_bps":100}}]"#),
        );
        let foreign_order = (
            "arbx:pool_index_v3:1:wfc:weth".to_string(),
            format!(r#"[{{"fee_bps":10000,"pool_addr":"{POOL}"}}]"#),
        );
        let mut witness: HashMap<Address, u32> = HashMap::new();
        witness.insert(addr_from(POOL), 100); // legacy `…:WETH:WFC` = 100 bps

        let a = V3FeeCatalog::new();
        let (am, ac) =
            a.ingest_canonical_payloads(&[writer_form.clone(), foreign_order.clone()], &witness);
        let b = V3FeeCatalog::new();
        let (bm, bc) =
            b.ingest_canonical_payloads(&[foreign_order.clone(), writer_form.clone()], &witness);

        assert_eq!((am, bm), (0, 0), "both payloads are well formed");
        assert_eq!(
            (ac, bc),
            (1, 1),
            "the contradiction is counted in both orders"
        );
        assert_eq!(
            a.fee_for_pool(addr_from(POOL)),
            Some(10_000),
            "legacy witness corroborates 100 bps × 100 = 10000 pips (the on-chain fee)"
        );
        assert_eq!(
            a.fee_for_pool(addr_from(POOL)),
            b.fee_for_pool(addr_from(POOL)),
            "SCAN order must not decide the tier"
        );
    }

    /// The same contradiction with NO independent witness must still be
    /// order-invariant: the fallback is a property of the KEY SET (the key in
    /// `key_pool_index_v3`'s own symbol order outranks a foreign order), never of
    /// the arrival order. This case claims stability and visibility — NOT truth:
    /// the chain admission probe (`TierMismatch` → `Catalog(onchain_fee)`) is what
    /// corrects a tier the index alone cannot decide.
    #[test]
    fn canonical_canonical_conflict_without_witness_is_still_order_invariant() {
        const POOL: &str = "0x6f9beaac7d042a50008db301833abbe92e4f3a8f";
        let writer_form = (
            "arbx:pool_index_v3:1:weth:wfc".to_string(),
            format!(r#"[{{"pool_addr":"{POOL}","fee_bps":100}}]"#),
        );
        let foreign_order = (
            "arbx:pool_index_v3:1:wfc:weth".to_string(),
            format!(r#"[{{"fee_bps":10000,"pool_addr":"{POOL}"}}]"#),
        );
        let no_witness: HashMap<Address, u32> = HashMap::new();

        let a = V3FeeCatalog::new();
        let (_, ac) =
            a.ingest_canonical_payloads(&[writer_form.clone(), foreign_order.clone()], &no_witness);
        let b = V3FeeCatalog::new();
        let (_, bc) =
            b.ingest_canonical_payloads(&[foreign_order.clone(), writer_form.clone()], &no_witness);

        assert_eq!((ac, bc), (1, 1));
        assert_eq!(
            a.fee_for_pool(addr_from(POOL)),
            b.fee_for_pool(addr_from(POOL)),
            "without a witness the tier is stable, not arbitrary"
        );
        assert_eq!(
            a.fee_for_pool(addr_from(POOL)),
            Some(100),
            "fallback = the writer-form key (lo:hi); the foreign order never wins"
        );
    }

    /// The writer-form predicate is the one `key_pool_index_v3` implements: the
    /// two lowermost segments must already be in that writer's sorted order.
    #[test]
    fn writer_form_detection_matches_the_canonical_key_writer() {
        assert!(key_follows_writer_order("arbx:pool_index_v3:1:weth:wfc"));
        assert!(!key_follows_writer_order("arbx:pool_index_v3:1:wfc:weth"));
        assert!(key_follows_writer_order("arbx:pool_index_v3:1:cbeth:weth"));
        assert!(key_follows_writer_order("arbx:pool_index_v3:1:1:usdt"));
    }

    /// A legacy entry that AGREES with the canonical one is not a contradiction
    /// (the index also holds pools present in both generations at the same
    /// value) — the gauge must not cry wolf.
    #[test]
    fn agreeing_generations_are_not_a_contradiction() {
        const POOL: &str = "0xbea615376d1184f3670a341b70f6f45d9d0fbaad";
        let canonical = format!(r#"[{{"pool_addr":"{POOL}","fee_bps":3000}}]"#);
        let legacy = format!(r#"[{{"pool_addr":"{POOL}","fee_bps":3000}}]"#);
        let c = V3FeeCatalog::new();
        c.ingest_index_payload(&canonical);
        let (malformed, conflicts) = c.ingest_index_payload_legacy(&legacy);
        assert_eq!((malformed, conflicts), (0, 0));
        assert_eq!(c.fee_for_pool(addr_from(POOL)), Some(3000));
    }

    /// A pool the canonical generation does not describe is still catalogued
    /// from the legacy key (no coverage loss) — the unit ambiguity is a KNOWN
    /// limitation, documented, not a silent guess: the value enters verbatim,
    /// exactly like a canonical entry with an unresolvable value does today.
    #[test]
    fn legacy_only_pool_still_enters_the_catalog() {
        const POOL: &str = "0x00000000000000000000000000000000000000ff";
        let legacy = format!(r#"[{{"pool_addr":"{POOL}","fee_bps":30}}]"#);
        let c = V3FeeCatalog::new();
        let (malformed, conflicts) = c.ingest_index_payload_legacy(&legacy);
        assert_eq!((malformed, conflicts), (0, 0));
        assert_eq!(c.pool_count(), 1);
        // Entered verbatim (30), NOT multiplied into a guessed 3000 — inventing
        // the unit is exactly what RULE 00/R8 forbid.
        assert_eq!(c.fee_for_pool(addr_from(POOL)), Some(30));
    }

    /// Malformed legacy payloads are counted and contribute nothing.
    #[test]
    fn legacy_ingest_counts_malformed_and_fabricates_nothing() {
        let c = V3FeeCatalog::new();
        let (malformed, conflicts) =
            c.ingest_index_payload_legacy(r#"[{"address":"0x1","fee_bps":30}]"#);
        assert_eq!((malformed, conflicts), (1, 0));
        assert_eq!(c.pool_count(), 0);
        let (malformed_bad_addr, conflicts_bad_addr) =
            c.ingest_index_payload_legacy(r#"[{"pool_addr":"not-an-address","fee_bps":30}]"#);
        assert_eq!((malformed_bad_addr, conflicts_bad_addr), (1, 0));
        assert_eq!(c.pool_count(), 0);
    }

    // ── CATALOG-HYGIENE-01: chain-verified admission ─────────────────────────
    //
    // Anchored to the 2026-10-03 measurements: the `fee_tier = 30` rows are
    // Uniswap V2 pairs (`fee()` reverts 9/9 sampled, `getReserves()` answers
    // 9/9, both captain-flagged addresses carry bytecode), the `0x464bd7…` row
    // says 100 while `fee()` answers 10000, and 0x6d029c/0x70b6e8/0xf6a42a are
    // real pools with `liquidity() == 0`.

    /// The phantom channel: an entry that IS in the catalogue map, but whose
    /// address the chain disproved, must not be quotable. Pre-fix it resolved
    /// `Catalog(30)` and every quote attempt reverted.
    #[test]
    fn disproved_entry_is_not_quotable_despite_being_catalogued() {
        let c = V3FeeCatalog::new();
        c.record_observed(addr(0xCB), addr(0xA), addr(0xB), 30);
        assert_eq!(c.resolve(addr(0xCB), Some(30)), FeeResolution::Catalog(30));

        c.record_admission(addr(0xCB), PoolAdmission::NotAV3Pool);
        assert_eq!(
            c.resolve(addr(0xCB), Some(30)),
            FeeResolution::NotCatalogued,
            "a disproved entry must be rejected with ZERO RPC, not quoted"
        );
        // The map itself is untouched: the verdict is the only thing excluded,
        // so an expired verdict restores the previous behaviour exactly.
        assert_eq!(c.fee_for_pool(addr(0xCB)), Some(30));
        assert_eq!(c.admission_count(), 1);
    }

    /// The empty-pool channel: a legitimate pool with `liquidity() == 0` cannot
    /// answer, so quoting it can only burn a call.
    #[test]
    fn empty_pool_is_not_quotable_while_the_verdict_is_fresh() {
        let c = V3FeeCatalog::new();
        c.record_observed(addr(0x6D), addr(0xA), addr(0xB), 3000);
        c.record_admission(addr(0x6D), PoolAdmission::EmptyPool { onchain_fee: 3000 });
        assert_eq!(c.resolve(addr(0x6D), None), FeeResolution::NotCatalogued);
    }

    /// The `0x464bd7…` channel: the chain corrects the catalogue. Quoting at the
    /// catalogue tier (100) makes QuoterV2 derive a pool that does not exist.
    #[test]
    fn tier_mismatch_quotes_the_chain_tier() {
        let c = V3FeeCatalog::new();
        c.record_observed(addr(0x46), addr(0xA), addr(0xB), 100);
        c.record_admission(
            addr(0x46),
            PoolAdmission::TierMismatch {
                catalogue_fee: 100,
                onchain_fee: 10000,
            },
        );
        assert_eq!(
            c.resolve(addr(0x46), Some(100)),
            FeeResolution::Catalog(10000),
            "the chain's immutable fee() outranks the catalogue value"
        );
    }

    /// A pool the chain ADMITTED resolves exactly as before — the new layer
    /// changes nothing on healthy entries.
    #[test]
    fn admitted_entry_resolves_as_before() {
        let c = V3FeeCatalog::new();
        c.record_observed(addr(0x88), addr(0xA), addr(0xB), 500);
        c.record_admission(addr(0x88), PoolAdmission::Admitted { onchain_fee: 500 });
        assert_eq!(c.resolve(addr(0x88), None), FeeResolution::Catalog(500));
        assert_eq!(
            c.resolve(addr(0x88), Some(3000)),
            FeeResolution::Mismatch {
                offered: 3000,
                catalog: 500
            }
        );
    }

    /// R8: a MISSING answer must never be recorded. `Unprobed` is not a verdict,
    /// and storing it would silently freeze a non-observation into a ruling.
    #[test]
    fn unprobed_is_never_recorded_and_changes_nothing() {
        let c = V3FeeCatalog::new();
        c.record_observed(addr(0x99), addr(0xA), addr(0xB), 3000);
        c.record_admission(addr(0x99), PoolAdmission::Unprobed);
        assert_eq!(c.admission_count(), 0, "a non-verdict must not be stored");
        assert_eq!(c.admission_for(addr(0x99)), None);
        assert_eq!(c.resolve(addr(0x99), None), FeeResolution::Catalog(3000));
    }

    /// Freshness is per verdict class: "empty" is a snapshot (short window),
    /// "not a V3 pool" is bounded but long, and a chain-proven tier never goes
    /// stale (`fee()` is immutable for a deployed pool).
    #[test]
    fn verdict_freshness_is_per_class() {
        let long = Duration::from_secs(10 * 24 * 3600);
        assert!(admission_is_fresh(
            PoolAdmission::NotAV3Pool,
            Duration::from_secs(1)
        ));
        assert!(!admission_is_fresh(PoolAdmission::NotAV3Pool, long));
        assert!(admission_is_fresh(
            PoolAdmission::EmptyPool { onchain_fee: 3000 },
            Duration::from_secs(1)
        ));
        assert!(!admission_is_fresh(
            PoolAdmission::EmptyPool { onchain_fee: 3000 },
            long
        ));
        assert!(admission_is_fresh(
            PoolAdmission::Admitted { onchain_fee: 500 },
            long
        ));
        assert!(admission_is_fresh(
            PoolAdmission::TierMismatch {
                catalogue_fee: 100,
                onchain_fee: 10000
            },
            long
        ));
        assert!(!admission_is_fresh(
            PoolAdmission::Unprobed,
            Duration::from_secs(0)
        ));
    }

    /// An EXPIRED verdict reads as absent, so the entry returns to its
    /// catalogue value — stale evidence never keeps condemning a pool.
    #[test]
    fn expired_verdict_reverts_to_the_catalogue_behaviour() {
        let c = V3FeeCatalog::new();
        c.record_observed(addr(0x77), addr(0xA), addr(0xB), 3000);
        c.record_admission(addr(0x77), PoolAdmission::EmptyPool { onchain_fee: 3000 });
        // Age the record past the empty-pool window by rewinding stored_at.
        {
            let mut g = c.admissions.write().unwrap_or_else(|e| e.into_inner());
            let rec = g.get_mut(&addr(0x77)).expect("record present");
            rec.recorded_at = Instant::now()
                .checked_sub(empty_pool_ttl() + Duration::from_secs(1))
                .expect("rewind");
        }
        assert_eq!(c.admission_for(addr(0x77)), None, "expired => not fresh");
        assert_eq!(c.resolve(addr(0x77), None), FeeResolution::Catalog(3000));
    }

    #[test]
    fn ttl_parse_is_fail_honest() {
        assert_eq!(parse_ttl_ms(None, 300_000), Duration::from_millis(300_000));
        assert_eq!(
            parse_ttl_ms(Some("junk".into()), 300_000),
            Duration::from_millis(300_000)
        );
        assert_eq!(
            parse_ttl_ms(Some("0".into()), 300_000),
            Duration::from_millis(300_000)
        );
        assert_eq!(
            parse_ttl_ms(Some(" 1500 ".into()), 300_000),
            Duration::from_millis(1_500)
        );
    }

    fn addr_from(s: &str) -> Address {
        s.parse::<Address>().expect("hex address")
    }
}
